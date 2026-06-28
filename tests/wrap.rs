use std::{fs, path::PathBuf, process::Command};

use serde_json::Value;
use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_skilltrace"))
}

fn isolated_home() -> TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn jsonl_events(home: &TempDir) -> Vec<Value> {
    let jsonl = fs::read_to_string(home.path().join(".skilltrace/events.jsonl")).expect("jsonl");
    jsonl.lines().map(|line| serde_json::from_str(line).expect("valid event json")).collect()
}

#[test]
fn wrap_omits_raw_command_arguments_by_default() {
    let home = isolated_home();

    let status = Command::new(binary())
        .env("HOME", home.path())
        .args(["wrap", "/bin/echo", "super-secret-token"])
        .status()
        .expect("run wrap");

    assert!(status.success());

    let jsonl = fs::read_to_string(home.path().join(".skilltrace/events.jsonl")).expect("jsonl");
    assert!(!jsonl.contains("super-secret-token"));
    assert!(jsonl.contains("wrapped command: /bin/echo"));
}

#[test]
fn wrap_exposes_session_identity_to_child_environment() {
    let home = isolated_home();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .args(["wrap", "env"])
        .output()
        .expect("run wrap env");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let session_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("SKILLTRACE_SESSION_ID="))
        .expect("session id env var");
    assert!(!session_id.is_empty());
    assert!(stdout.contains("SKILLTRACE_AGENT=env\n"));
    assert!(stdout.contains("SKILLTRACE_ADAPTER=process-wrapper\n"));

    let events = jsonl_events(&home);
    assert_eq!(events[0]["session_id"], session_id);
}

#[test]
fn event_inside_wrapped_shell_records_into_wrapper_session() {
    let home = isolated_home();
    let child_command = format!("{} event skill_end --skill child_tool", binary().display());

    let status = Command::new(binary())
        .env("HOME", home.path())
        .args(["wrap", "/bin/sh", "-c", &child_command])
        .status()
        .expect("run wrapped shell");

    assert!(status.success());

    let events = jsonl_events(&home);
    let wrapper_session_id = events[0]["session_id"].as_str().expect("wrapper session");
    let child_event = events
        .iter()
        .find(|event| event["event_type"] == "skill_end" && event["skill"] == "child_tool")
        .expect("child skill_end event");
    assert_eq!(child_event["session_id"], wrapper_session_id);
}

#[test]
fn wrap_records_successful_command_boundaries() {
    let home = isolated_home();

    let status = Command::new(binary())
        .env("HOME", home.path())
        .args(["wrap", "/bin/echo", "super-secret-token"])
        .status()
        .expect("run wrap");

    assert!(status.success());

    let events = jsonl_events(&home);
    let event_types: Vec<_> =
        events.iter().map(|event| event["event_type"].as_str().expect("event type")).collect();
    assert_eq!(event_types, ["session_start", "command_start", "command_end", "session_end"]);

    let command_start = &events[1];
    assert_eq!(command_start["skill"], "wrapped_command");
    assert_eq!(command_start["input_summary"], "wrapped command: /bin/echo");
    assert!(!command_start.to_string().contains("super-secret-token"));

    let command_end = &events[2];
    assert_eq!(command_end["skill"], "wrapped_command");
    assert_eq!(command_end["success"], true);
    assert_eq!(command_end["output_summary"], "child exited with status code 0");
    assert!(!command_end.to_string().contains("super-secret-token"));
}

#[test]
fn wrap_returns_child_exit_status() {
    let home = isolated_home();

    let status = Command::new(binary())
        .env("HOME", home.path())
        .args(["wrap", "/bin/sh", "-c", "exit 23"])
        .status()
        .expect("run wrap");

    assert_eq!(status.code(), Some(23));
}

#[test]
fn wrap_records_failed_command_boundary_and_returns_child_status() {
    let home = isolated_home();

    let status = Command::new(binary())
        .env("HOME", home.path())
        .args(["wrap", "/bin/sh", "-c", "exit 23"])
        .status()
        .expect("run wrap");

    assert_eq!(status.code(), Some(23));

    let events = jsonl_events(&home);
    let event_types: Vec<_> =
        events.iter().map(|event| event["event_type"].as_str().expect("event type")).collect();
    assert_eq!(event_types, ["session_start", "command_start", "command_end", "session_end"]);

    let command_end = &events[2];
    assert_eq!(command_end["skill"], "wrapped_command");
    assert_eq!(command_end["success"], false);
    assert_eq!(command_end["output_summary"], "child exited with status code 23");
    assert!(!command_end.to_string().contains("exit 23"));
}
