use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::fs;
use std::path::Path;
use serde_json::Value;
use std::process::Command;

pub struct Launcher;

#[derive(Debug)]
pub enum LauncherMessage {
    Launch(RpcReplyPort<Result<(), String>>),
}

#[async_trait::async_trait]
impl Actor for Launcher {
    type Msg = LauncherMessage;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            LauncherMessage::Launch(reply) => {
                let res = self.perform_launch();
                reply.send(res)?;
            }
        }
        Ok(())
    }
}

impl Launcher {
    fn perform_launch(&self) -> Result<(), String> {
        println!("Initializing Mentci-AI Level 5 Jail Environment (Rust/Actor)...");
        
        let attrs_path = std::env::var("NIX_ATTRS_JSON_FILE").unwrap_or_else(|_| ".attrs.json".to_string());
        let attrs_file = Path::new(&attrs_path);
        
        let full_config: Value = if attrs_file.exists() {
            let content = fs::read_to_string(attrs_file).map_err(|e| e.to_string())?;
            serde_json::from_str(&content).map_err(|e| e.to_string())?
        } else if let Ok(env_val) = std::env::var("jailConfig") {
            serde_json::from_str(&env_val).map_err(|e| e.to_string())?
        } else {
            return Err("Configuration not found (no .attrs.json or jailConfig env)".to_string());
        };

        let config = self.find_jail_config(&full_config).ok_or("jailConfig not found in attributes")?;
        
        let sources_path = config["sourcesPath"].as_str().unwrap_or("Sources");
        let source_manifest = config["sourceManifest"].as_object().ok_or("sourceManifest missing")?;
        
        let mentci_mode = std::env::var("MENTCI_MODE").unwrap_or_default();
        let is_impure = mentci_mode == "ADMIN" || Path::new("/usr/local").exists();
        
        fs::create_dir_all(sources_path).map_err(|e| e.to_string())?;

        for (name, manifest_entry) in source_manifest {
            let source_path = manifest_entry["sourcePath"].as_str().ok_or("sourcePath missing")?;
            let src_path = manifest_entry["srcPath"].as_str();
            let final_source = src_path.unwrap_or(source_path);
            let target_path = Path::new(sources_path).join(name);

            if is_impure {
                println!("Materializing {} (Mutable)...", name);
                let mut cmd = Command::new("rsync");
                cmd.args(["-aL", "--chmod=u+w", &format!("{}/", final_source), &target_path.to_string_lossy()]);
                let status = cmd.status().map_err(|e| e.to_string())?;
                if !status.success() {
                    eprintln!("Error materializing {}: rsync failed", name);
                }
            } else {
                if target_path.exists() {
                    let _ = fs::remove_file(&target_path);
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::symlink;
                    if let Err(e) = symlink(final_source, &target_path) {
                        eprintln!("Error linking {}: {}", name, e);
                    }
                }
            }
        }

        println!("Jail environment ready.");
        Ok(())
    }

    fn find_jail_config<'a>(&self, val: &'a Value) -> Option<&'a Value> {
        if val.is_object() {
            if val.get("sourcesPath").is_some() {
                return Some(val);
            }
            for v in val.as_object().unwrap().values() {
                if let Some(res) = self.find_jail_config(v) {
                    return Some(res);
                }
            }
        } else if val.is_array() {
            for v in val.as_array().unwrap() {
                if let Some(res) = self.find_jail_config(v) {
                    return Some(res);
                }
            }
        }
        None
    }
}
