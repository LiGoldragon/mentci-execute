use std::process::Command;

fn run_execute(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_execute"))
        .args(args)
        .output()
        .expect("execute command should start")
}

fn run_execute_with_env(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_execute"));
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("execute command should start")
}

fn run_execute_in_dir(args: &[&str], dir: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_execute"))
        .args(args)
        .current_dir(dir)
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
    assert!(stdout.contains("target-bookmark"), "missing target-bookmark: {stdout}");
    assert!(stdout.contains("session-meta"), "missing session-meta: {stdout}");
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

#[test]
fn execute_target_bookmark_prefers_env_var() {
    let output = run_execute_with_env(&["target-bookmark"], &[("MENTCI_TARGET_BOOKMARK", "feature-abc")]);
    assert!(output.status.success(), "expected success");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "feature-abc");
}

#[test]
fn execute_target_bookmark_falls_back_to_dir_suffix() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo_like = temp.path().join("mentci-ai--research");
    std::fs::create_dir_all(&repo_like).expect("create repo-like dir");

    let output = run_execute_in_dir(&["target-bookmark"], &repo_like);
    assert!(output.status.success(), "expected success");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "research");
}
