use std::fs::{self, OpenOptions};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn has_tool(name: &str) -> bool {
    Command::new("sh")
        .args(["-lc", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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

fn reserve_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind random port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn wait_for_port(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("attractor server did not start listening on 127.0.0.1:{port}");
}

fn start_attractor_server(attractor_dir: &Path, port: u16, log_path: &Path) -> Child {
    let stdout_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("open attractor log for stdout");
    let stderr_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("open attractor log for stderr");

    Command::new("bun")
        .args(["run", "bin/attractor-server.ts"])
        .current_dir(attractor_dir)
        .env("ATTRACTOR_HOST", "127.0.0.1")
        .env("ATTRACTOR_PORT", port.to_string())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .expect("start attractor server")
}

fn curl_json(url: &str, request_json_path: Option<&Path>) -> serde_json::Value {
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "-X"]);
    if request_json_path.is_some() {
        cmd.arg("POST");
    } else {
        cmd.arg("GET");
    }
    cmd.arg(url);
    if let Some(json_path) = request_json_path {
        cmd.args(["-H", "Content-Type: application/json", "--data-binary"]);
        cmd.arg(format!("@{}", json_path.display()));
    }
    let body = run_capture(&mut cmd);
    serde_json::from_str::<serde_json::Value>(&body)
        .unwrap_or_else(|e| panic!("invalid JSON from {url}: {e}\nbody={body}"))
}

#[test]
#[ignore = "requires bun/curl and local attractor dependencies"]
fn attractor_harness_can_run_minimal_dot_pipeline() {
    if !has_tool("bun") || !has_tool("curl") {
        eprintln!("skipping test: required tools missing (bun/curl)");
        return;
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let log_path = root.join("attractor.log");
    let request_path = root.join("pipeline.json");
    let attractor_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("Sources/untyped/brynary-attractor/attractor");
    if !attractor_dir.exists() {
        eprintln!(
            "skipping test: attractor directory missing at {}",
            attractor_dir.display()
        );
        return;
    }

    let port = reserve_port();
    let mut server = start_attractor_server(&attractor_dir, port, &log_path);
    wait_for_port(port, Duration::from_secs(15));

    let result = (|| -> Result<(), String> {
        let dot = r#"digraph Minimal {
  start [shape=Mdiamond]
  exit [shape=Msquare]
  start -> exit
}"#;
        let request = serde_json::json!({ "dot": dot });
        fs::write(
            &request_path,
            serde_json::to_vec_pretty(&request).expect("encode request json"),
        )
        .map_err(|e| format!("failed writing request json: {e}"))?;

        let create_url = format!("http://127.0.0.1:{port}/pipelines");
        let create_response = curl_json(&create_url, Some(&request_path));
        let pipeline_id = create_response
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("missing pipeline id in response: {create_response}"))?
            .to_string();

        let status_url = format!("http://127.0.0.1:{port}/pipelines/{pipeline_id}");
        let mut final_status = String::new();
        for _ in 0..80 {
            let status = curl_json(&status_url, None);
            final_status = status
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if final_status != "running" {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        if final_status != "completed" {
            let status = curl_json(&status_url, None);
            return Err(format!("unexpected pipeline status: {status}"));
        }

        let graph_url = format!("http://127.0.0.1:{port}/pipelines/{pipeline_id}/graph");
        let graph = run_capture(Command::new("curl").args(["-sS", &graph_url]));
        if graph.trim().is_empty() {
            return Err("graph endpoint returned empty body".to_string());
        }

        let context_url = format!("http://127.0.0.1:{port}/pipelines/{pipeline_id}/context");
        let context = curl_json(&context_url, None);
        if context.get("context").is_none() {
            return Err(format!(
                "context response missing `context` field: {context}"
            ));
        }

        let checkpoint_url = format!("http://127.0.0.1:{port}/pipelines/{pipeline_id}/checkpoint");
        let checkpoint = curl_json(&checkpoint_url, None);
        if checkpoint.get("checkpoint").is_none() {
            return Err(format!(
                "checkpoint response missing `checkpoint` field: {checkpoint}"
            ));
        }
        Ok(())
    })();

    let _ = server.kill();
    let _ = server.wait();

    if let Err(err) = result {
        let logs = fs::read_to_string(&log_path).unwrap_or_else(|_| "<no logs>".to_string());
        panic!("attractor harness failed: {err}\nserver log:\n{logs}");
    }
}
