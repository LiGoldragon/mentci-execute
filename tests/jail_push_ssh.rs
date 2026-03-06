use std::fs;
use std::io::BufWriter;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

mod atom_filesystem_capnp {
    include!(concat!(env!("OUT_DIR"), "/atom_filesystem_capnp.rs"));
}

mod mentci_capnp {
    include!(concat!(env!("OUT_DIR"), "/mentci_capnp.rs"));
}

fn has_tool(name: &str) -> bool {
    Command::new("sh")
        .args(["-lc", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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

fn run_check(cmd: &mut Command) -> Result<(), String> {
    let output = cmd
        .output()
        .map_err(|e| format!("failed to start command: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "status={}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
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

fn reserve_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind random port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

fn start_sshd(
    config_path: &Path,
    log_path: &Path,
    env_home: &Path,
    username: &str,
) -> Result<Child, String> {
    Command::new("sshd")
        .arg("-D")
        .arg("-f")
        .arg(config_path)
        .arg("-E")
        .arg(log_path)
        .env("HOME", env_home)
        .env("USER", username)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start sshd: {e}"))
}

#[test]
#[ignore = "requires local sshd/bb/jj tooling and network loopback"]
fn jail_commit_pushes_to_ssh_git_remote() {
    if !has_tool("sshd")
        || !has_tool("ssh-keygen")
        || !has_tool("bb")
        || !has_tool("jj")
        || !has_tool("git")
    {
        eprintln!("skipping test: required tools missing");
        return;
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let repo = root.join("repo");
    let bare = root.join("remote.git");
    let ssh_dir = root.join("ssh");
    fs::create_dir_all(&repo).expect("create repo");
    fs::create_dir_all(&ssh_dir).expect("create ssh dir");

    run_ok(Command::new("jj").args(["git", "init"]).arg(&repo));
    run_ok(
        Command::new("jj")
            .args(["config", "set", "--repo", "user.name", "Jail Tester", "-R"])
            .arg(&repo),
    );
    run_ok(
        Command::new("jj")
            .args([
                "config",
                "set",
                "--repo",
                "user.email",
                "jail@test.local",
                "-R",
            ])
            .arg(&repo),
    );
    run_ok(
        Command::new("jj")
            .args(["bookmark", "create", "dev", "-r", "@", "-R"])
            .arg(&repo),
    );

    run_ok(Command::new("git").args(["init", "--bare"]).arg(&bare));

    let host_key = ssh_dir.join("host_ed25519");
    let client_key = ssh_dir.join("client_ed25519");
    run_ok(
        Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", "", "-f"])
            .arg(&host_key),
    );
    run_ok(
        Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", "", "-f"])
            .arg(&client_key),
    );

    let authorized_keys = ssh_dir.join("authorized_keys");
    fs::copy(client_key.with_extension("pub"), &authorized_keys).expect("copy authorized key");

    let port = reserve_port();
    let user = std::env::var("USER").unwrap_or_else(|_| "li".to_string());
    let sshd_config = ssh_dir.join("sshd_config");
    fs::write(
        &sshd_config,
        format!(
            "Port {port}\n\
             ListenAddress 127.0.0.1\n\
             HostKey {}\n\
             AuthorizedKeysFile {}\n\
             PasswordAuthentication no\n\
             KbdInteractiveAuthentication no\n\
             ChallengeResponseAuthentication no\n\
             UsePAM no\n\
             PermitRootLogin no\n\
             AllowUsers {user}\n\
             StrictModes no\n\
             PidFile {}\n\
             LogLevel VERBOSE\n",
            host_key.display(),
            authorized_keys.display(),
            ssh_dir.join("sshd.pid").display()
        ),
    )
    .expect("write sshd config");

    let log_path = ssh_dir.join("sshd.log");
    let mut sshd = match start_sshd(&sshd_config, &log_path, root, &user) {
        Ok(child) => child,
        Err(err) => {
            eprintln!("skipping test: {err}");
            return;
        }
    };
    if !wait_for_port(port, Duration::from_secs(5)) {
        let logs = fs::read_to_string(&log_path).unwrap_or_else(|_| "<no logs>".to_string());
        eprintln!("skipping test: sshd did not start on 127.0.0.1:{port}\n{logs}");
        let _ = sshd.kill();
        let _ = sshd.wait();
        return;
    }

    let result = (|| -> Result<(), String> {
        let remote_url = format!("ssh://{user}@127.0.0.1:{port}{}", bare.display());
        run_check(
            Command::new("jj")
                .args(["git", "remote", "add", "origin", &remote_url, "-R"])
                .arg(&repo),
        )?;

        let capnp_path = root.join("jail-request.capnp");
        write_bootstrap_request(
            &capnp_path,
            &repo,
            "Outputs",
            "mentci-ai",
            "dev",
            "jailCommit",
            None,
        );

        let daemon = env!("CARGO_BIN_EXE_mentci-ai");
        run_check(
            Command::new(daemon)
                .args(["job/jails", "bootstrap", "--capnp"])
                .arg(&capnp_path),
        )?;

        let workspace = repo.join("Outputs").join("mentci-ai");
        fs::write(workspace.join("payload.txt"), "jail push payload\n").expect("write payload");

        let policy_path = root.join("jail-policy.json");
        fs::write(&policy_path, r#"{"allowedPushBookmarks":["jailCommit"]}"#)
            .expect("write policy");

        let repo_scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../scripts/commit/main.clj");
        let runtime_path = workspace.join(".mentci/runtime.json");
        let runtime_json = serde_json::json!({
            "repoRoot": repo.to_string_lossy(),
            "workspaceRoot": workspace.to_string_lossy(),
            "workingBookmark": "dev",
            "targetBookmark": "jailCommit",
            "policyPath": policy_path.to_string_lossy(),
        });
        fs::write(
            &runtime_path,
            serde_json::to_vec_pretty(&runtime_json).expect("encode runtime json"),
        )
        .expect("write runtime file");

        run_check(
            Command::new("bb")
                .arg(repo_scripts)
                .arg("--runtime")
                .arg(&runtime_path)
                .arg("intent: ssh jail push"),
        )?;

        let repo_jj = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../scripts/jj_workflow/main.clj");
        run_check(
            Command::new("bb")
                .arg(repo_jj)
                .arg("--runtime")
                .arg(&runtime_path)
                .args(["push", "origin", "jailCommit"])
                .env(
                    "GIT_SSH_COMMAND",
                    format!(
                        "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p {port}",
                        client_key.display()
                    ),
                ),
        )?;

        run_check(Command::new("git").arg("--git-dir").arg(&bare).args([
            "show-ref",
            "--verify",
            "refs/heads/jailCommit",
        ]))?;
        Ok(())
    })();

    let _ = sshd.kill();
    let _ = sshd.wait();

    if let Err(err) = result {
        panic!("jail ssh push flow failed: {err}");
    }
}
