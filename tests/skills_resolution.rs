use std::{fs, path::PathBuf, process::Command};

use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_skilltrace"))
}

fn setup_repo() -> (TempDir, PathBuf) {
    let home = tempfile::tempdir().expect("tempdir");
    let repo = home.path().join("repo");
    fs::create_dir_all(repo.join("catalog")).expect("create repo");
    fs::write(
        repo.join("catalog/skills.toml"),
        "[[skill]]\nname = \"repo_search\"\ndescription = \"Search\"\ncategory = \"retrieval\"\n",
    )
    .expect("write skills");
    fs::create_dir_all(home.path().join(".skilltrace")).expect("create config dir");
    fs::write(
        home.path().join(".skilltrace/config.toml"),
        "[storage]\nbackend = \"sqlite\"\npath = \"~/.skilltrace/skilltrace.db\"\njsonl_mirror = true\n\n[agents.codex]\nenabled = true\nadapter = \"wrapper\"\n\n[mcp]\nenabled = true\ntransport = \"stdio\"\n\n[privacy]\ncapture_raw_prompts = false\ncapture_raw_outputs = false\nhash_sensitive_values = true\n\n[skills]\ndefinition_file = \"catalog/skills.toml\"\n",
    )
    .expect("write config");
    (home, repo)
}

#[test]
fn unused_uses_configured_definition_file_by_default() {
    let (home, repo) = setup_repo();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .current_dir(&repo)
        .args(["unused"])
        .output()
        .expect("run unused");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("repo_search"));
}

#[test]
fn recommend_uses_configured_definition_file() {
    let (home, repo) = setup_repo();

    let output = Command::new(binary())
        .env("HOME", home.path())
        .current_dir(&repo)
        .args(["recommend"])
        .output()
        .expect("run recommend");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("repo_search"));
}
