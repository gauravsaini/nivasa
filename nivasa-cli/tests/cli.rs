use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_nivasa")
}

fn temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nivasa-cli-{prefix}-{nanos}"));
    fs::create_dir_all(&path).expect("temp dir should be creatable");
    path
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("nivasa command should run")
}

#[test]
fn nivasa_help_lists_top_level_commands() {
    let output = run_cli(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output should be utf-8");
    assert!(stdout.contains("info"));
    assert!(stdout.contains("generate"));
    assert!(stdout.contains("statechart"));
}

#[test]
fn nivasa_statechart_help_lists_subcommands() {
    let output = run_cli(&["statechart", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output should be utf-8");
    assert!(stdout.contains("validate"));
    assert!(stdout.contains("parity"));
    assert!(stdout.contains("visualize"));
    assert!(stdout.contains("diff"));
    assert!(stdout.contains("inspect"));
}

#[test]
fn nivasa_info_reports_missing_rustc_as_error() {
    let path = temp_dir("no-rustc");
    let output = Command::new(binary())
        .arg("info")
        .env("PATH", &path)
        .output()
        .expect("nivasa info should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("failed to run rustc --version"));
}

#[test]
fn nivasa_statechart_validate_rejects_all_and_file() {
    let root = temp_dir("validate-all");
    let file = root.join("demo.scxml");
    fs::write(
        &file,
        r#"<?xml version="1.0"?>
<scxml version="1.0" name="Demo" initial="idle" xmlns="http://www.w3.org/2005/07/scxml">
  <state id="idle"/>
</scxml>"#,
    )
    .expect("temp scxml should be writable");

    let output = Command::new(binary())
        .args(["statechart", "validate", "--all"])
        .arg(&file)
        .output()
        .expect("nivasa validate should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("use either `--all` or a single file path, not both"));
}

#[test]
fn nivasa_statechart_inspect_reports_connection_error() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("free port should exist");
    let port = listener.local_addr().expect("listener should have addr").port();
    drop(listener);

    let output = Command::new(binary())
        .args(["statechart", "inspect", "--port", &port.to_string()])
        .output()
        .expect("nivasa inspect should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("failed to connect to 127.0.0.1"));
}
