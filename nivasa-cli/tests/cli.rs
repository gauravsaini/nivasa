use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
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

fn run_cli_in_dir(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("nivasa command should run")
}

fn fake_rustc_dir(prefix: &str, script_body: &[u8]) -> PathBuf {
    let dir = temp_dir(prefix);
    let script = dir.join("rustc");
    fs::write(&script, script_body).expect("fake rustc should be writable");
    let mut permissions = fs::metadata(&script)
        .expect("fake rustc metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("fake rustc should be executable");
    dir
}

fn spawn_inspect_server(responses: Vec<&'static str>) -> (u16, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("free port should exist");
    let port = listener
        .local_addr()
        .expect("listener should have addr")
        .port();
    let handle = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("inspect client should connect");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(response.as_bytes())
                .expect("inspect response should write");
        }
    });

    (port, handle)
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
fn nivasa_new_creates_project_from_cli() {
    let root = temp_dir("new-cli");

    let output = run_cli_in_dir(&root, &["new", "demo-app"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("created demo-app"));

    let project_dir = root.join("demo-app");
    assert!(project_dir.join("Cargo.toml").is_file());
    assert!(project_dir.join("src/main.rs").is_file());
    assert!(project_dir
        .join("statecharts/nivasa.request.scxml")
        .is_file());
}

#[test]
fn nivasa_generate_alias_creates_resource_bundle() {
    let root = temp_dir("generate-alias");

    let output = run_cli_in_dir(&root, &["g", "resource", "users"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("created"));

    let resource_dir = root.join("users");
    assert!(resource_dir.join("users_module.rs").is_file());
    assert!(resource_dir.join("users_controller.rs").is_file());
    assert!(resource_dir.join("users_service.rs").is_file());
    assert!(resource_dir.join("dto/create_users_dto.rs").is_file());
    assert!(resource_dir.join("dto/update_users_dto.rs").is_file());
}

#[test]
fn nivasa_generate_named_file_commands_create_expected_files() {
    let cases = [
        ("guard", "auth", "auth/auth_guard.rs"),
        ("interceptor", "audit", "audit/audit_interceptor.rs"),
        ("pipe", "trim", "trim/trim_pipe.rs"),
        ("filter", "http", "http/http_filter.rs"),
        ("middleware", "auth", "auth/auth_middleware.rs"),
    ];

    for (command, name, relative_path) in cases {
        let root = temp_dir(command);
        let output = run_cli_in_dir(&root, &["generate", command, name]);

        assert!(output.status.success(), "{command} should succeed");
        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
        assert!(
            stdout.contains("created"),
            "{command} should report created file"
        );
        assert!(
            root.join(relative_path).is_file(),
            "{command} should create expected file"
        );
    }
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
fn nivasa_info_reports_unsuccessful_rustc_exit() {
    let path = fake_rustc_dir("bad-rustc-exit", b"#!/bin/sh\nexit 9\n");
    let output = Command::new(binary())
        .arg("info")
        .env("PATH", &path)
        .output()
        .expect("nivasa info should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("rustc --version exited unsuccessfully"));
}

#[test]
fn nivasa_info_reports_non_utf8_rustc_output() {
    let path = fake_rustc_dir("bad-rustc-utf8", b"#!/bin/sh\nprintf '\\377'\n");
    let output = Command::new(binary())
        .arg("info")
        .env("PATH", &path)
        .output()
        .expect("nivasa info should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("rustc --version returned non-utf8 output"));
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
fn nivasa_statechart_validate_specific_file_succeeds() {
    let root = temp_dir("validate-one");
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
        .args(["statechart", "validate"])
        .arg(&file)
        .output()
        .expect("nivasa validate should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("demo.scxml: valid"));
}

#[test]
fn nivasa_statechart_validate_without_args_validates_all_files() {
    let output = run_cli(&["statechart", "validate"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("nivasa.application.scxml: valid"));
    assert!(stdout.contains("nivasa.module.scxml: valid"));
    assert!(stdout.contains("nivasa.provider.scxml: valid"));
    assert!(stdout.contains("nivasa.request.scxml: valid"));
}

#[test]
fn nivasa_statechart_inspect_reports_connection_error() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("free port should exist");
    let port = listener
        .local_addr()
        .expect("listener should have addr")
        .port();
    drop(listener);

    let output = Command::new(binary())
        .args(["statechart", "inspect", "--port", &port.to_string()])
        .output()
        .expect("nivasa inspect should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("failed to connect to 127.0.0.1"));
}

#[test]
fn nivasa_statechart_inspect_falls_back_to_later_debug_endpoint() {
    let (port, server) = spawn_inspect_server(vec![
        "HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
    ]);

    let output = Command::new(binary())
        .args(["statechart", "inspect", "--port", &port.to_string()])
        .output()
        .expect("nivasa inspect should run");

    server.join().expect("inspect server should exit cleanly");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("/_nivasa/statechart/transitions"));
    assert!(stdout.contains("\"ok\": true"));
}

#[test]
fn nivasa_statechart_inspect_reports_last_http_error_when_all_endpoints_fail() {
    let (port, server) = spawn_inspect_server(vec![
        "HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
    ]);

    let output = Command::new(binary())
        .args(["statechart", "inspect", "--port", &port.to_string()])
        .output()
        .expect("nivasa inspect should run");

    server.join().expect("inspect server should exit cleanly");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("returned HTTP 500 Internal Server Error"));
}

#[test]
fn nivasa_statechart_visualize_renders_specific_file() {
    let output = run_cli(&["statechart", "visualize", "nivasa.application.scxml"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("nivasa.application.scxml"));
    assert!(stdout.contains("<svg"));
    assert!(stdout.contains("arrowhead"));
}

#[test]
fn nivasa_statechart_visualize_without_file_renders_all_statecharts() {
    let output = run_cli(&["statechart", "visualize"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("nivasa.application.scxml"));
    assert!(stdout.contains("nivasa.module.scxml"));
    assert!(stdout.contains("nivasa.provider.scxml"));
    assert!(stdout.contains("nivasa.request.scxml"));
}

#[test]
fn nivasa_statechart_visualize_reports_missing_file() {
    let output = run_cli(&["statechart", "visualize", "missing-statechart.scxml"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("statechart file not found: missing-statechart.scxml"));
}

#[test]
fn nivasa_statechart_parity_reports_generated_registry_matches_sources() {
    let output = run_cli(&["statechart", "parity"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("nivasa.application.scxml: parity ok"));
    assert!(stdout.contains("nivasa.module.scxml: parity ok"));
    assert!(stdout.contains("nivasa.provider.scxml: parity ok"));
    assert!(stdout.contains("nivasa.request.scxml: parity ok"));
}

#[test]
fn nivasa_statechart_diff_reports_invalid_revision() {
    let output = run_cli(&["statechart", "diff", "definitely-not-a-real-revision"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("definitely-not-a-real-revision"));
}

#[test]
fn nivasa_statechart_diff_head_reports_no_changes() {
    let output = run_cli(&["statechart", "diff", "HEAD"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("No SCXML differences found against HEAD."));
}
