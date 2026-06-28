use std::{fs, path::PathBuf, process::Command};

use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_skilltrace"))
}

fn isolated_home() -> TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn event_hashes_sensitive_summaries_by_default() {
    let home = isolated_home();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .args([
            "event",
            "error",
            "--error",
            "token=super-secret-token",
            "--input-summary",
            "Bearer sk-live-secret",
            "--output-summary",
            "api_key=abc123secret",
        ])
        .output()
        .expect("run event");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let jsonl = fs::read_to_string(home.path().join(".skilltrace/events.jsonl")).expect("jsonl");

    assert!(!stdout.contains("super-secret-token"));
    assert!(!jsonl.contains("super-secret-token"));
    assert!(!jsonl.contains("sk-live-secret"));
    assert!(jsonl.contains("sha256:"));
}

#[test]
fn event_hashes_plain_bearer_input_summary_by_default() {
    let home = isolated_home();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .args(["event", "error", "--input-summary", "Bearer opaquevalue"])
        .output()
        .expect("run event");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let jsonl = fs::read_to_string(home.path().join(".skilltrace/events.jsonl")).expect("jsonl");

    assert!(!stdout.contains("opaquevalue"));
    assert!(!jsonl.contains("opaquevalue"));
    assert!(jsonl.contains("Bearer sha256:"));
}

#[test]
fn event_hashes_plain_authorization_bearer_output_summary_by_default() {
    let home = isolated_home();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .args(["event", "error", "--output-summary", "Authorization: Bearer opaquevalue"])
        .output()
        .expect("run event");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let jsonl = fs::read_to_string(home.path().join(".skilltrace/events.jsonl")).expect("jsonl");

    assert!(!stdout.contains("opaquevalue"));
    assert!(!jsonl.contains("opaquevalue"));
    assert!(jsonl.contains("Authorization: Bearer sha256:"));
}

#[test]
fn privacy_event_hashes_url_query_secrets_by_default() {
    let home = isolated_home();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .args([
            "event",
            "error",
            "--input-summary",
            "GET https://example.test/path?token=super-secret-token&safe=value",
        ])
        .output()
        .expect("run event");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let jsonl = fs::read_to_string(home.path().join(".skilltrace/events.jsonl")).expect("jsonl");

    assert!(!stdout.contains("super-secret-token"));
    assert!(!jsonl.contains("super-secret-token"));
    assert!(jsonl.contains("token=sha256:"));
    assert!(jsonl.contains("safe=value"));
}

#[test]
fn privacy_event_hashes_json_like_secret_fields_by_default() {
    let home = isolated_home();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .args([
            "event",
            "error",
            "--output-summary",
            r#"{"api_key":"abc123secret","safe":"visible"}"#,
        ])
        .output()
        .expect("run event");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let jsonl = fs::read_to_string(home.path().join(".skilltrace/events.jsonl")).expect("jsonl");

    assert!(!stdout.contains("abc123secret"));
    assert!(!jsonl.contains("abc123secret"));
    assert!(jsonl.contains("api_key"));
    assert!(jsonl.contains("sha256:"));
    assert!(jsonl.contains(r#"\"safe\":\"visible\""#));
}
