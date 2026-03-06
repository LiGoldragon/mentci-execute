use std::process::Command;

fn run_execute(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_execute"))
        .args(args)
        .output()
        .expect("execute command should start")
}

#[test]
fn execute_without_args_prints_command_catalog() {
    let output = run_execute(&[]);
    assert!(output.status.success(), "expected success");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("commands:"), "missing command header: {stdout}");
    assert!(stdout.contains("root-guard"), "missing root-guard: {stdout}");
    assert!(stdout.contains("session-guard"), "missing session-guard: {stdout}");
    assert!(stdout.contains("finalize"), "missing finalize: {stdout}");
}

#[test]
fn execute_unknown_command_fails() {
    let output = run_execute(&["does-not-exist"]);
    assert!(!output.status.success(), "expected failure");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command"),
        "expected unknown command error, got: {stderr}"
    );
}

#[test]
fn execute_version_returns_hash_like_output() {
    let output = run_execute(&["version"]);
    assert!(output.status.success(), "expected success");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout.trim();
    assert!(!version.is_empty(), "version output must not be empty");
    assert!(
        version
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
        "version must be lowercase alnum: {version}"
    );
    assert!(version.len() >= 6, "version unexpectedly short: {version}");
}
