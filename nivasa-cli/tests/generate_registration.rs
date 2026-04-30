use std::fs;
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

fn run_cli_in_dir(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("nivasa command should run")
}

#[test]
fn generate_commands_register_items_into_module_files() {
    let root = temp_dir("generate-register");

    let new_output = run_cli_in_dir(&root, &["new", "demo-app"]);
    assert!(new_output.status.success());

    let project_dir = root.join("demo-app");

    let module_output = run_cli_in_dir(
        &project_dir,
        &[
            "generate",
            "module",
            "users",
            "--parent-module-file",
            "src/app_module.rs",
        ],
    );
    assert!(module_output.status.success());

    let controller_output = run_cli_in_dir(
        &project_dir,
        &[
            "generate",
            "controller",
            "users",
            "--module-file",
            "users/users_module.rs",
        ],
    );
    assert!(controller_output.status.success());

    let service_output = run_cli_in_dir(
        &project_dir,
        &[
            "generate",
            "service",
            "users",
            "--module-file",
            "users/users_module.rs",
        ],
    );
    assert!(service_output.status.success());

    let app_module = fs::read_to_string(project_dir.join("src/app_module.rs"))
        .expect("app module should be readable");
    assert!(app_module.contains("#[path = \"../users/users_module.rs\"]"));
    assert!(app_module.contains("mod users_module;"));
    assert!(app_module.contains("use users_module::UsersModule;"));
    assert!(app_module.contains("imports: ["));
    assert!(app_module.contains("UsersModule,"));

    let users_module = fs::read_to_string(project_dir.join("users/users_module.rs"))
        .expect("users module should be readable");
    assert!(users_module.contains("#[path = \"users_controller.rs\"]"));
    assert!(users_module.contains("mod users_controller;"));
    assert!(users_module.contains("use users_controller::UsersController;"));
    assert!(users_module.contains("#[path = \"users_service.rs\"]"));
    assert!(users_module.contains("mod users_service;"));
    assert!(users_module.contains("use users_service::UsersService;"));
    assert!(users_module.contains("controllers: ["));
    assert!(users_module.contains("UsersController,"));
    assert!(users_module.contains("providers: ["));
    assert!(users_module.contains("UsersService,"));
}

#[test]
fn generate_controller_removes_file_when_module_registration_fails() {
    let root = temp_dir("generate-register-fail");
    let invalid_module = root.join("bad_module.rs");
    fs::write(&invalid_module, "pub struct NotAModule;\n")
        .expect("invalid module file should exist");

    let output = run_cli_in_dir(
        &root,
        &[
            "generate",
            "controller",
            "users",
            "--module-file",
            "bad_module.rs",
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("target file missing #[module(...)] attribute"));
    assert!(!root.join("users/users_controller.rs").exists());
}

#[test]
fn generate_module_removes_file_when_parent_registration_fails() {
    let root = temp_dir("generate-module-register-fail");
    let invalid_parent = root.join("bad_parent.rs");
    fs::write(&invalid_parent, "pub struct NotAModule;\n")
        .expect("invalid parent file should exist");

    let output = run_cli_in_dir(
        &root,
        &[
            "generate",
            "module",
            "users",
            "--parent-module-file",
            "bad_parent.rs",
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("target file missing #[module(...)] attribute"));
    assert!(!root.join("users/users_module.rs").exists());
}

#[test]
fn generate_service_removes_file_when_module_registration_fails() {
    let root = temp_dir("generate-service-register-fail");
    let invalid_module = root.join("bad_module.rs");
    fs::write(&invalid_module, "pub struct NotAModule;\n")
        .expect("invalid module file should exist");

    let output = run_cli_in_dir(
        &root,
        &[
            "generate",
            "service",
            "users",
            "--module-file",
            "bad_module.rs",
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("target file missing #[module(...)] attribute"));
    assert!(!root.join("users/users_service.rs").exists());
}
