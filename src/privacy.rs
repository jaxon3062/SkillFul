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
    event.metadata_json = sanitize_text(event.metadata_json.take());
}

fn sanitize_text(value: Option<String>) -> Option<String> {
    value.map(|value| sanitize_inline(&value))
}

fn sanitize_inline(value: &str) -> String {
    let mut sanitized = Vec::new();
    let mut tokens = value.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        sanitized.push(sanitize_token(token));

        if token.eq_ignore_ascii_case("Bearer") {
            if let Some(value) = tokens.next() {
                sanitized.push(sanitize_bearer_value(value));
            }
        }
    }

    sanitized.join(" ")
}

fn sanitize_token(token: &str) -> String {
    let trimmed = token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']'));
    if trimmed.starts_with("sha256:") {
        return token.to_string();
    }

    if let Some(sanitized) = sanitize_key_value_segments(token) {
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

fn sanitize_bearer_value(token: &str) -> String {
    let trimmed = token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']'));
    if trimmed.starts_with("sha256:") {
        return token.to_string();
    }

    token.replacen(trimmed, &hash_value(trimmed), 1)
}

fn sanitize_key_value_segments(token: &str) -> Option<String> {
    let mut output = String::new();
    let mut last_copied = 0;
    let mut index = 0;
    let mut changed = false;
    let mut saw_sensitive_pair = false;

    while index < token.len() {
        if let Some(match_span) = sensitive_pair_at(token, index) {
            saw_sensitive_pair = true;
            if !match_span.already_hashed {
                output.push_str(&token[last_copied..match_span.value_start]);
                output.push_str(&hash_value(&token[match_span.value_start..match_span.value_end]));
                last_copied = match_span.value_end;
                changed = true;
            }
            index = match_span.value_end;
        } else {
            index += next_char_len(token, index);
        }
    }

    if changed {
        output.push_str(&token[last_copied..]);
        Some(output)
    } else if saw_sensitive_pair {
        Some(token.to_string())
    } else {
        None
    }
}

struct SensitivePairMatch {
    value_start: usize,
    value_end: usize,
    already_hashed: bool,
}

fn sensitive_pair_at(token: &str, start: usize) -> Option<SensitivePairMatch> {
    let key_quote = quote_at(token, start);
    let key_start = key_quote.map_or(start, |quote| start + quote.len_utf8());
    let key_end = read_key_end(token, key_start);

    if key_end == key_start {
        return None;
    }

    if let Some(quote) = key_quote {
        if !token[key_end..].starts_with(quote) {
            return None;
        }
    }

    let delimiter_index = key_quote.map_or(key_end, |quote| key_end + quote.len_utf8());
    let delimiter = token[delimiter_index..].chars().next()?;
    if !matches!(delimiter, '=' | ':') {
        return None;
    }

    if !looks_sensitive_key(&token[key_start..key_end]) {
        return None;
    }

    let mut value_start = delimiter_index + delimiter.len_utf8();
    let value_quote = quote_at(token, value_start);
    if let Some(quote) = value_quote {
        value_start += quote.len_utf8();
    }

    let value_end = read_value_end(token, value_start, value_quote);
    if value_end == value_start {
        return None;
    }
    let already_hashed = token[value_start..value_end].starts_with("sha256:");

    Some(SensitivePairMatch { value_start, value_end, already_hashed })
}

fn read_key_end(token: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in token[start..].char_indices() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
            end = start + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn read_value_end(token: &str, start: usize, value_quote: Option<char>) -> usize {
    let mut end = start;
    for (offset, ch) in token[start..].char_indices() {
        if value_quote.is_some_and(|quote| ch == quote)
            || value_quote.is_none()
                && (ch.is_whitespace() || matches!(ch, '&' | ',' | ')' | ']' | '}' | '"' | '\''))
        {
            break;
        }
        end = start + offset + ch.len_utf8();
    }
    end
}

fn quote_at(token: &str, index: usize) -> Option<char> {
    token[index..].chars().next().filter(|ch| matches!(ch, '"' | '\''))
}

fn next_char_len(token: &str, index: usize) -> usize {
    token[index..].chars().next().map_or(1, char::len_utf8)
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
    fn sanitize_event_hashes_sensitive_key_value_variants() {
        let mut event = EventRecord::new(
            "decision".to_string(),
            "session-1".to_string(),
            None,
            None,
            "codex".to_string(),
            "manual".to_string(),
            None,
            None,
            Some("token:colon-secret".to_string()),
            0,
            Some(r#"token:"quoted-secret""#.to_string()),
            Some(r#""token":"json-secret","safe":"visible""#.to_string()),
            Some("Authorization: Bearer bearer-secret".to_string()),
            None,
            vec!["token=equals-secret".to_string()],
            None,
            None,
            None,
        );
        event.metadata_json = Some(r#"{"secret":"metadata-secret","safe":"visible"}"#.to_string());

        sanitize_event(&mut event, &privacy(true));

        assert!(event.error.as_deref().is_some_and(|value| {
            value.starts_with("token:sha256:") && !value.contains("colon-secret")
        }));
        assert!(event.input_summary.as_deref().is_some_and(|value| {
            value.starts_with(r#"token:"sha256:"#) && !value.contains("quoted-secret")
        }));
        assert!(event.output_summary.as_deref().is_some_and(|value| {
            value.contains(r#""token":"sha256:"#)
                && value.contains(r#""safe":"visible""#)
                && !value.contains("json-secret")
        }));
        assert!(event.planner_reason.as_deref().is_some_and(|value| {
            value.starts_with("Authorization: Bearer sha256:") && !value.contains("bearer-secret")
        }));
        assert_eq!(event.alternatives.len(), 1);
        assert!(event.alternatives[0].starts_with("token=sha256:"));
        assert!(!event.alternatives[0].contains("equals-secret"));
        assert!(event.metadata_json.as_deref().is_some_and(|value| {
            value.contains(r#""secret":"sha256:"#)
                && value.contains(r#""safe":"visible""#)
                && !value.contains("metadata-secret")
        }));
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
