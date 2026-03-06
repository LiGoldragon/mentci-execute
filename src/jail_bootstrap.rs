use anyhow::{bail, Context, Result};
use capnp::message::ReaderOptions;
use capnp::serialize_packed;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
struct BootstrapConfig {
    repo_root: PathBuf,
    outputs_dir: String,
    output_name: String,
    working_bookmark: String,
    target_bookmark: String,
    commit_message: Option<String>,
    policy_path: Option<PathBuf>,
}

pub fn run_from_args(mut args: Vec<String>) -> Result<()> {
    if args.first().map(String::as_str) == Some("bootstrap") {
        args.remove(0);
    }
    let cfg = parse_args(args).context("failed to parse bootstrap args")?;
    run(cfg)
}

fn parse_args(args: Vec<String>) -> Result<BootstrapConfig> {
    let mut capnp_path: Option<PathBuf> = None;
    let mut cli_repo_root: Option<PathBuf> = None;
    let mut cli_outputs_dir: Option<String> = None;
    let mut cli_output_name: Option<String> = None;
    let mut cli_working_bookmark: Option<String> = None;
    let mut cli_target_bookmark: Option<String> = None;
    let mut cli_commit_message: Option<String> = None;
    let mut cli_policy_path: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--capnp" => {
                i += 1;
                let value = args.get(i).context("missing value for --capnp")?;
                capnp_path = Some(PathBuf::from(value));
            }
            "--repo-root" => {
                i += 1;
                let value = args.get(i).context("missing value for --repo-root")?;
                cli_repo_root = Some(PathBuf::from(value));
            }
            "--outputs-dir" => {
                i += 1;
                let value = args.get(i).context("missing value for --outputs-dir")?;
                cli_outputs_dir = Some(value.clone());
            }
            "--output-name" => {
                i += 1;
                let value = args.get(i).context("missing value for --output-name")?;
                cli_output_name = Some(value.clone());
            }
            "--working-bookmark" => {
                i += 1;
                let value = args
                    .get(i)
                    .context("missing value for --working-bookmark")?;
                cli_working_bookmark = Some(value.clone());
            }
            "--target-bookmark" => {
                i += 1;
                let value = args.get(i).context("missing value for --target-bookmark")?;
                cli_target_bookmark = Some(value.clone());
            }
            "--commit-message" => {
                i += 1;
                let value = args.get(i).context("missing value for --commit-message")?;
                cli_commit_message = Some(value.clone());
            }
            "--policy-path" => {
                i += 1;
                let value = args.get(i).context("missing value for --policy-path")?;
                cli_policy_path = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }

    let capnp_file = capnp_path.context("missing required --capnp <path> argument")?;
    let capnp_cfg = Some(load_capnp_request(&capnp_file)?);

    let repo_root = cli_repo_root
        .or_else(|| capnp_cfg.as_ref().map(|cfg| cfg.repo_root.clone()))
        .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
    let outputs_dir = cli_outputs_dir
        .or_else(|| capnp_cfg.as_ref().map(|cfg| cfg.outputs_dir.clone()))
        .unwrap_or_else(|| "Outputs".to_string());
    let output_name = cli_output_name
        .or_else(|| capnp_cfg.as_ref().map(|cfg| cfg.output_name.clone()))
        .unwrap_or_else(|| "mentci-ai".to_string());
    let working_bookmark = cli_working_bookmark
        .or_else(|| capnp_cfg.as_ref().map(|cfg| cfg.working_bookmark.clone()))
        .unwrap_or_else(|| "dev".to_string());
    let target_bookmark = cli_target_bookmark
        .or_else(|| capnp_cfg.as_ref().map(|cfg| cfg.target_bookmark.clone()))
        .unwrap_or_else(|| "jailCommit".to_string());
    let commit_message = cli_commit_message.or_else(|| {
        capnp_cfg
            .as_ref()
            .and_then(|cfg| cfg.commit_message.clone())
    });
    let policy_path =
        cli_policy_path.or_else(|| capnp_cfg.as_ref().and_then(|cfg| cfg.policy_path.clone()));

    Ok(BootstrapConfig {
        repo_root,
        outputs_dir,
        output_name,
        working_bookmark,
        target_bookmark,
        commit_message,
        policy_path,
    })
}

fn print_help() {
    println!("Usage: mentci-ai job/jails [bootstrap] [options]");
    println!("  --capnp <path>               Cap'n Proto JailBootstrapRequest file (required)");
    println!("  --repo-root <path>           Repository root (default: cwd)");
    println!("  --outputs-dir <name>         Outputs directory (default: Outputs)");
    println!("  --output-name <name>         Output workspace name (default: mentci-ai)");
    println!("  --working-bookmark <name>    Working bookmark (default: dev)");
    println!("  --target-bookmark <name>     Commit target bookmark (default: jailCommit)");
    println!("  --commit-message <message>   Optional commit message for immediate jail commit");
    println!("  --policy-path <path>         Read-only jail policy JSON path");
}

fn load_capnp_request(path: &Path) -> Result<BootstrapConfig> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open capnp request file {:?}", path))?;
    let mut reader = BufReader::new(file);
    let message = serialize_packed::read_message(&mut reader, ReaderOptions::new())
        .with_context(|| format!("failed to read packed capnp message {:?}", path))?;
    let request = message
        .get_root::<crate::mentci_capnp::jail_bootstrap_request::Reader<'_>>()
        .context("failed to decode JailBootstrapRequest root")?;

    let repo_root = request.get_repo_root()?.to_str()?.to_string();
    let outputs_dir = request.get_outputs_dir()?.to_str()?.to_string();
    let output_name = request.get_output_name()?.to_str()?.to_string();
    let working_bookmark = request.get_working_bookmark()?.to_str()?.to_string();
    let target_bookmark = request.get_target_bookmark()?.to_str()?.to_string();
    let commit_message_raw = request.get_commit_message()?.to_str()?.to_string();
    let policy_path_raw = request.get_policy_path()?.to_str()?.to_string();

    Ok(BootstrapConfig {
        repo_root: PathBuf::from(repo_root),
        outputs_dir,
        output_name,
        working_bookmark,
        target_bookmark,
        commit_message: if commit_message_raw.is_empty() {
            None
        } else {
            Some(commit_message_raw)
        },
        policy_path: if policy_path_raw.is_empty() {
            None
        } else {
            Some(PathBuf::from(policy_path_raw))
        },
    })
}

fn run(cfg: BootstrapConfig) -> Result<()> {
    if cfg.target_bookmark == cfg.working_bookmark {
        bail!(
            "target bookmark '{}' must differ from working bookmark '{}'",
            cfg.target_bookmark,
            cfg.working_bookmark
        );
    }

    let repo_root = fs::canonicalize(&cfg.repo_root)
        .with_context(|| format!("failed to canonicalize repo root {:?}", cfg.repo_root))?;
    if !repo_root.join(".jj").exists() {
        bail!("repo root {:?} is not a jj repository", repo_root);
    }

    ensure_revision_exists(&repo_root, &cfg.working_bookmark)?;

    let outputs_root = repo_root.join(&cfg.outputs_dir);
    fs::create_dir_all(&outputs_root)
        .with_context(|| format!("failed to create outputs dir {:?}", outputs_root))?;

    let workspace_path = outputs_root.join(&cfg.output_name);
    let workspace_name = format!("output-{}", sanitize_workspace_name(&cfg.output_name));

    if !workspace_path.join(".jj").exists() {
        run_jj(
            &repo_root,
            &[
                "workspace",
                "add",
                workspace_path
                    .to_str()
                    .context("workspace path contains invalid utf-8")?,
                "--name",
                &workspace_name,
                "--revision",
                &cfg.working_bookmark,
            ],
        )
        .context("failed to create output workspace")?;
    }

    if let Some(message) = &cfg.commit_message {
        run_jj(&workspace_path, &["describe", "-m", message])
            .context("failed to describe workspace commit")?;
        run_jj(
            &workspace_path,
            &["bookmark", "set", &cfg.target_bookmark, "-r", "@"],
        )
        .context("failed to set target bookmark from workspace")?;
    }

    let runtime_dir = workspace_path.join(".mentci");
    fs::create_dir_all(&runtime_dir)
        .with_context(|| format!("failed to create runtime dir {:?}", runtime_dir))?;
    let runtime_path = runtime_dir.join("runtime.json");
    let runtime_json = serde_json::json!({
        "repoRoot": repo_root.to_string_lossy(),
        "workspaceRoot": workspace_path.to_string_lossy(),
        "workingBookmark": cfg.working_bookmark,
        "targetBookmark": cfg.target_bookmark,
        "policyPath": cfg.policy_path.as_ref().map(|p| p.to_string_lossy().to_string()),
    });
    fs::write(
        &runtime_path,
        serde_json::to_vec_pretty(&runtime_json).context("failed to encode runtime json")?,
    )
    .with_context(|| format!("failed to write runtime file {:?}", runtime_path))?;

    println!("workspaceRoot={}", workspace_path.display());
    println!("workingBookmark={}", cfg.working_bookmark);
    println!("targetBookmark={}", cfg.target_bookmark);
    println!("runtimeFile={}", runtime_path.display());
    Ok(())
}

fn sanitize_workspace_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn ensure_revision_exists(repo_root: &Path, revset: &str) -> Result<()> {
    run_jj(
        repo_root,
        &[
            "log",
            "-r",
            revset,
            "-n",
            "1",
            "--no-graph",
            "-T",
            "commit_id",
        ],
    )
    .with_context(|| format!("working bookmark/revset '{revset}' does not exist"))?;
    Ok(())
}

fn run_jj(repo_root: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("jj")
        .args(args)
        .arg("-R")
        .arg(repo_root)
        .output()
        .with_context(|| format!("failed to run jj {:?}", args))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "jj {:?} failed (code {}): {}{}",
            args,
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use capnp::message::Builder;
    use capnp::serialize_packed;
    use std::io::BufWriter;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parses_bootstrap_config_from_capnp_file() {
        let temp = tempdir().expect("tempdir");
        let request_path = temp.path().join("jail-request.capnp");

        let mut message = Builder::new_default();
        {
            let mut root =
                message.init_root::<crate::mentci_capnp::jail_bootstrap_request::Builder<'_>>();
            root.set_repo_root("/tmp/repo");
            root.set_outputs_dir("Outputs");
            root.set_output_name("mentci-ai");
            root.set_working_bookmark("dev");
            root.set_target_bookmark("jailCommit");
            root.set_commit_message("intent: capnp bootstrap");
            root.set_policy_path("/tmp/policy.json");
        }

        let file = std::fs::File::create(&request_path).expect("create request file");
        let mut writer = BufWriter::new(file);
        serialize_packed::write_message(&mut writer, &message).expect("write request message");
        writer.flush().expect("flush request message");

        let parsed = parse_args(vec![
            "--capnp".to_string(),
            request_path.to_string_lossy().to_string(),
        ])
        .expect("parse args");

        assert_eq!(parsed.repo_root.to_string_lossy(), "/tmp/repo");
        assert_eq!(parsed.outputs_dir, "Outputs");
        assert_eq!(parsed.output_name, "mentci-ai");
        assert_eq!(parsed.working_bookmark, "dev");
        assert_eq!(parsed.target_bookmark, "jailCommit");
        assert_eq!(
            parsed.commit_message.as_deref(),
            Some("intent: capnp bootstrap")
        );
        assert_eq!(
            parsed
                .policy_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            Some("/tmp/policy.json".to_string())
        );
    }
}
