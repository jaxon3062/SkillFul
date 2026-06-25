use std::{fs, path::PathBuf, process::Command};

use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_skilltrace"))
}

fn isolated_home() -> TempDir {
    tempfile::tempdir().expect("tempdir")
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
fn wrap_returns_child_exit_status() {
    let home = isolated_home();

    let status = Command::new(binary())
        .env("HOME", home.path())
        .args(["wrap", "/bin/sh", "-c", "exit 23"])
        .status()
        .expect("run wrap");

    assert_eq!(status.code(), Some(23));
}
