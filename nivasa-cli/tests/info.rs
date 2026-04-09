use std::process::Command;

#[test]
fn nivasa_info_outputs_version_information() {
    let binary = env!("CARGO_BIN_EXE_nivasa");
    let output = Command::new(binary)
        .arg("info")
        .output()
        .expect("failed to run nivasa info");

    assert!(
        output.status.success(),
        "nivasa info failed: status={:?}, stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("nivasa info output must be utf-8");

    assert!(stdout.contains("Nivasa Framework v"));
    assert!(stdout.contains("Rust "));
    assert!(stdout.contains("OS "));
}
