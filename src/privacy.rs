use sha2::{Digest, Sha256};

use crate::{config::PrivacyConfig, event::EventRecord};

pub fn sanitize_event(event: &mut EventRecord, privacy: &PrivacyConfig) {
    if !privacy.hash_sensitive_values {
        return;
    }

    event.input_summary = sanitize_text(event.input_summary.take());
    event.output_summary = sanitize_text(event.output_summary.take());
    event.error = sanitize_text(event.error.take());
    event.planner_reason = sanitize_text(event.planner_reason.take());
    event.alternatives =
        event.alternatives.drain(..).map(|value| sanitize_inline(&value)).collect();
}

fn sanitize_text(value: Option<String>) -> Option<String> {
    value.map(|value| sanitize_inline(&value))
}

fn sanitize_inline(value: &str) -> String {
    value.split_whitespace().map(sanitize_token).collect::<Vec<_>>().join(" ")
}

fn sanitize_token(token: &str) -> String {
    let trimmed = token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']'));
    if trimmed.starts_with("sha256:") {
        return token.to_string();
    }

    if let Some(sanitized) = sanitize_key_value(token) {
        return sanitized;
    }

    if let Some(rest) = token.strip_prefix("Bearer ") {
        return format!("Bearer {}", hash_value(rest));
    }

    if looks_sensitive_token(trimmed) {
        return token.replacen(trimmed, &hash_value(trimmed), 1);
    }

    token.to_string()
}

fn sanitize_key_value(token: &str) -> Option<String> {
    for delimiter in ['=', ':'] {
        let (key, value) = token.split_once(delimiter)?;
        if !looks_sensitive_key(key) || value.is_empty() {
            continue;
        }

        return Some(format!("{key}{delimiter}{}", hash_value(value)));
    }

    None
}

fn looks_sensitive_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    ["token", "secret", "password", "api_key", "apikey", "key", "bearer"]
        .iter()
        .any(|candidate| lowered.contains(candidate))
}

fn looks_sensitive_token(token: &str) -> bool {
    let lowered = token.to_ascii_lowercase();
    lowered.starts_with("sk-")
        || lowered.starts_with("ghp_")
        || lowered.starts_with("github_pat_")
        || lowered.starts_with("xoxb-")
        || lowered.starts_with("xoxp-")
        || lowered.contains("secret")
        || lowered.contains("token")
        || (token.len() >= 16 && token.chars().any(|ch| ch.is_ascii_digit()))
}

fn hash_value(value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(value.as_bytes());
    let encoded = format!("{:x}", digest.finalize());
    format!("sha256:{}", &encoded[..16])
}

#[cfg(test)]
mod tests {
    use crate::{config::PrivacyConfig, event::EventRecord};

    use super::sanitize_event;

    fn privacy(hash_sensitive_values: bool) -> PrivacyConfig {
        PrivacyConfig {
            capture_raw_prompts: false,
            capture_raw_outputs: false,
            hash_sensitive_values,
        }
    }

    #[test]
    fn sanitize_event_hashes_sensitive_values() {
        let mut event = EventRecord::new(
            "decision".to_string(),
            "session-1".to_string(),
            None,
            None,
            "codex".to_string(),
            "manual".to_string(),
            None,
            None,
            Some("token=super-secret-token".to_string()),
            0,
            Some("Bearer sk-live-secret".to_string()),
            Some("api_key=abc123secret".to_string()),
            Some("try secret=alpha".to_string()),
            None,
            vec!["password=hunter2".to_string()],
            None,
            None,
            None,
        );

        sanitize_event(&mut event, &privacy(true));

        assert!(event.error.as_deref().is_some_and(|value| {
            value.starts_with("token=sha256:") && !value.contains("super-secret-token")
        }));
        assert!(event.input_summary.as_deref().is_some_and(|value| {
            value.starts_with("Bearer sha256:") && !value.contains("sk-live-secret")
        }));
        assert!(event.output_summary.as_deref().is_some_and(|value| {
            value.starts_with("api_key=sha256:") && !value.contains("abc123secret")
        }));
        assert!(event.planner_reason.as_deref().is_some_and(|value| {
            value.starts_with("try secret=sha256:") && !value.contains("alpha")
        }));
        assert_eq!(event.alternatives.len(), 1);
        assert!(event.alternatives[0].starts_with("password=sha256:"));
        assert!(!event.alternatives[0].contains("hunter2"));
    }

    #[test]
    fn sanitize_event_preserves_values_when_hashing_disabled() {
        let mut event = EventRecord::new(
            "decision".to_string(),
            "session-1".to_string(),
            None,
            None,
            "codex".to_string(),
            "manual".to_string(),
            None,
            None,
            Some("token=super-secret-token".to_string()),
            0,
            Some("input".to_string()),
            Some("output".to_string()),
            None,
            None,
            Vec::new(),
            None,
            None,
            None,
        );

        sanitize_event(&mut event, &privacy(false));

        assert_eq!(event.error.as_deref(), Some("token=super-secret-token"));
        assert_eq!(event.input_summary.as_deref(), Some("input"));
        assert_eq!(event.output_summary.as_deref(), Some("output"));
    }
}
