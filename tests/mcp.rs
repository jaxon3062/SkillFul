use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::TempDir;

fn temp_home() -> TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[test]
fn mcp_stdio_handles_initialize_and_tools_list() {
    let home = temp_home();
    let binary = env!("CARGO_BIN_EXE_skilltrace");
    let mut child = Command::new(binary)
        .arg("mcp")
        .env("HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn skilltrace mcp");

    {
        let mut stdin = child.stdin.take().expect("stdin");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize"}}"#).expect("write init");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#)
            .expect("write tools/list");
    }

    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    reader.read_line(&mut line).expect("read initialize response");
    let initialize: Value = serde_json::from_str(&line).expect("parse initialize");
    assert_eq!(initialize["result"]["serverInfo"]["name"], "skilltrace");

    line.clear();
    reader.read_line(&mut line).expect("read tools/list response");
    let tools_list: Value = serde_json::from_str(&line).expect("parse tools/list");
    let tools = tools_list["result"]["tools"].as_array().expect("tools array");
    assert!(tools.iter().any(|tool| tool["name"] == "skilltrace.record_skill_start"));

    let status = child.wait().expect("wait child");
    assert!(status.success());
}

#[test]
fn mcp_stdio_returns_parse_error_and_keeps_running() {
    let home = temp_home();
    let binary = env!("CARGO_BIN_EXE_skilltrace");
    let mut child = Command::new(binary)
        .arg("mcp")
        .env("HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn skilltrace mcp");

    {
        let mut stdin = child.stdin.take().expect("stdin");
        writeln!(stdin, "not-json").expect("write invalid");
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":3,"method":"initialize"}}"#)
            .expect("write initialize");
    }

    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    reader.read_line(&mut line).expect("read parse error");
    let parse_error: Value = serde_json::from_str(&line).expect("parse error response");
    assert_eq!(parse_error["error"]["code"], -32700);

    line.clear();
    reader.read_line(&mut line).expect("read initialize response");
    let initialize: Value = serde_json::from_str(&line).expect("parse initialize");
    assert_eq!(initialize["result"]["serverInfo"]["name"], "skilltrace");

    let status = child.wait().expect("wait child");
    assert!(status.success());
}
