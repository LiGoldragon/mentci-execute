use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::collections::{HashSet, BTreeMap};
use std::fs;
use std::path::PathBuf;
use edn_rs::Edn;
use std::str::FromStr;

pub struct RootGuard;

#[derive(Debug)]
pub enum RootGuardMessage {
    Check(RpcReplyPort<Result<(), Vec<String>>>),
}

#[async_trait::async_trait]
impl Actor for RootGuard {
    type Msg = RootGuardMessage;
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
            RootGuardMessage::Check(reply) => {
                let res = self.perform_check();
                reply.send(res)?;
            }
        }
        Ok(())
    }
}

impl RootGuard {
    fn perform_check(&self) -> Result<(), Vec<String>> {
        let config = self.load_sidecar_config()?;
        
        let allowed_domain_dirs: HashSet<String> = self.get_edn_vector(&config, ":allowed-domain-dirs")?.into_iter().collect();
        let allowed_runtime_dirs: HashSet<String> = self.get_edn_vector(&config, ":allowed-runtime-dirs")?.into_iter().collect();
        let allowed_top_files: HashSet<String> = self.get_edn_vector(&config, ":allowed-top-files")?.into_iter().collect();

        let mut errors = Vec::new();
        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = entry.path().is_dir();

                if is_dir {
                    if !allowed_domain_dirs.contains(&name) && !allowed_runtime_dirs.contains(&name) && !name.starts_with(".jj_") {
                        errors.push(format!("unexpected top-level directory: {}", name));
                    }
                } else if !allowed_top_files.contains(&name) {
                    errors.push(format!("unexpected top-level file: {}", name));
                }
            }
        }
        
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    fn load_sidecar_config(&self) -> Result<BTreeMap<String, Edn>, Vec<String>> {
        let sidecar_path = PathBuf::from("Components/mentci-aid/src/actors/root_guard.edn");
        let content = fs::read_to_string(&sidecar_path)
            .map_err(|e| vec![format!("failed to read sidecar config {}: {}", sidecar_path.display(), e)])?;
        
        let edn = Edn::from_str(&content)
            .map_err(|e| vec![format!("failed to parse sidecar EDN: {}", e)])?;
        
        match edn {
            Edn::Map(m) => Ok(m.to_map()),
            _ => Err(vec!["sidecar EDN must be a map".to_string()]),
        }
    }

    fn get_edn_vector(&self, map: &BTreeMap<String, Edn>, key: &str) -> Result<Vec<String>, Vec<String>> {
        let val = map.get(key).ok_or_else(|| vec![format!("missing {} in sidecar", key)])?;
        if let Edn::Vector(v) = val {
            Ok(v.clone().to_vec().iter().map(|e| if let Edn::Str(s) = e { s.clone().trim_matches('"').to_string() } else { e.to_string() }).collect())
        } else {
            Err(vec![format!("{} must be a vector", key)])
        }
    }
}
