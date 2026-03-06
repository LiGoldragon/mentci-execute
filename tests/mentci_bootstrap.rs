use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;

mod atom_filesystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/atom_filesystem_capnp.rs"));
}

mod mentci_capnp {
    include!(concat!(env!("OUT_DIR"), "/mentci_capnp.rs"));
}

fn run_ok(cmd: &mut Command) {
    let output = cmd.output().expect("command should start");
    if !output.status.success() {
        panic!(
            "command failed:\nstatus={}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn run_capture(cmd: &mut Command) -> String {
    let output = cmd.output().expect("command should start");
    if !output.status.success() {
        panic!(
            "command failed:\nstatus={}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn write_bootstrap_request(
    path: &Path,
    repo_root: &Path,
    outputs_dir: &str,
    output_name: &str,
    working_bookmark: &str,
    target_bookmark: &str,
    commit_message: Option<&str>,
) {
    let mut message = capnp::message::Builder::new_default();
    {
        let mut root = message.init_root::<mentci_capnp::jail_bootstrap_request::Builder<'_>>();
        root.set_repo_root(&repo_root.to_string_lossy());
        root.set_outputs_dir(outputs_dir);
        root.set_output_name(output_name);
        root.set_working_bookmark(working_bookmark);
        root.set_target_bookmark(target_bookmark);
        root.set_commit_message(commit_message.unwrap_or(""));
    }

    let file = std::fs::File::create(path).expect("create capnp request file");
    let mut writer = BufWriter::new(file);
    capnp::serialize_packed::write_message(&mut writer, &message).expect("write capnp request");
}

fn request_path(temp: &Path, name: &str) -> PathBuf {
    temp.join(format!("{name}.capnp"))
}

#[test]
fn bootstrap_creates_jail_commit_bookmark_from_output_workspace() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&repo).expect("create repo dir");

    run_ok(Command::new("jj").args(["git", "init"]).arg(&repo));
    run_ok(
        Command::new("jj")
            .args(["bookmark", "create", "dev", "-r", "@", "-R"])
            .arg(&repo),
    );

    let request1 = request_path(temp.path(), "bootstrap-1");
    write_bootstrap_request(
        &request1,
        &repo,
        "Outputs",
        "mentci-ai",
        "dev",
        "jailCommit",
        None,
    );

    let bin = env!("CARGO_BIN_EXE_mentci-ai");
    run_ok(
        Command::new(bin)
            .args(["job/jails", "bootstrap"])
            .args(["--capnp"])
            .arg(&request1),
    );

    let workspace = repo.join("Outputs").join("mentci-ai");
    assert!(
        workspace.join(".jj").exists(),
        "expected workspace .jj at {:?}",
        workspace
    );

    fs::write(workspace.join("jail.txt"), "wrapped by rust bootstrap\n")
        .expect("write test change");

    let request2 = request_path(temp.path(), "bootstrap-2");
    write_bootstrap_request(
        &request2,
        &repo,
        "Outputs",
        "mentci-ai",
        "dev",
        "jailCommit",
        Some("intent: test jail commit"),
    );

    run_ok(
        Command::new(bin)
            .args(["job/jails", "bootstrap"])
            .args(["--capnp"])
            .arg(&request2),
    );

    let bookmarks = run_capture(
        Command::new("jj")
            .args([
                "log",
                "-r",
                "jailCommit",
                "-n",
                "1",
                "--no-graph",
                "-T",
                "bookmarks",
                "-R",
            ])
            .arg(&repo),
    );
    assert!(
        bookmarks.contains("jailCommit"),
        "expected jailCommit bookmark, got: {bookmarks}"
    );

    let description = run_capture(
        Command::new("jj")
            .args([
                "log",
                "-r",
                "jailCommit",
                "-n",
                "1",
                "--no-graph",
                "-T",
                "description",
                "-R",
            ])
            .arg(&repo),
    );
    assert!(
        description.contains("intent: test jail commit"),
        "unexpected description: {description}"
    );
}

#[test]
fn bootstrap_rejects_same_working_and_target_bookmarks() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&repo).expect("create repo dir");

    run_ok(Command::new("jj").args(["git", "init"]).arg(&repo));
    run_ok(
        Command::new("jj")
            .args(["bookmark", "create", "dev", "-r", "@", "-R"])
            .arg(&repo),
    );

    let request = request_path(temp.path(), "bootstrap-invalid");
    write_bootstrap_request(&request, &repo, "Outputs", "mentci-ai", "dev", "dev", None);

    let bin = env!("CARGO_BIN_EXE_mentci-ai");
    let output = Command::new(bin)
        .args(["job/jails", "bootstrap"])
        .args(["--capnp"])
        .arg(&request)
        .output()
        .expect("bootstrap command");

    assert!(
        !output.status.success(),
        "expected failure for same bookmarks"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must differ"),
        "expected bookmark mismatch error, got: {stderr}"
    );
}
