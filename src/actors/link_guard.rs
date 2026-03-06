use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
use crate::mentci_box_capnp::link_guard_config;
use edn_rs::Edn;
use std::str::FromStr;

pub struct LinkGuard;

#[derive(Debug)]
pub enum LinkGuardMessage {
    Check(RpcReplyPort<Result<(), Vec<String>>>),
    CheckWithConfig(Vec<u8>, RpcReplyPort<Result<(), Vec<String>>>),
}

#[async_trait::async_trait]
impl Actor for LinkGuard {
    type Msg = LinkGuardMessage;
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
            LinkGuardMessage::Check(reply) => {
                let errors = self.perform_check(None);
                reply.send(errors)?;
            }
            LinkGuardMessage::CheckWithConfig(data, reply) => {
                let errors = self.perform_check(Some(data));
                reply.send(errors)?;
            }
        }
        Ok(())
    }
}

impl LinkGuard {
    fn perform_check(&self, config_data: Option<Vec<u8>>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        let (roots, rules, allowlist): (Vec<String>, Vec<(String, Regex, String)>, Vec<String>) = if let Some(data) = config_data {
            self.load_capnp_config(data)?
        } else {
            self.load_sidecar_config()?
        };

        for root in roots {
            let path = Path::new(&root);
            if !path.exists() { continue; }
            
            for entry in walkdir::WalkDir::new(path).into_iter().flatten() {
                if entry.file_type().is_file() {
                    let path_str = entry.path().to_string_lossy().to_string();
                    if allowlist.iter().any(|a| path_str.contains(a)) {
                        continue;
                    }

                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        for (_, regex, msg) in &rules {
                            if regex.is_match(&content) {
                                if msg.contains("inputs/") && content.contains("inputs/outputs") {
                                    continue;
                                }
                                errors.push(format!("{}: {}", path_str, msg));
                            }
                        }
                    }
                }
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    fn load_capnp_config(&self, data: Vec<u8>) -> Result<(Vec<String>, Vec<(String, Regex, String)>, Vec<String>), Vec<String>> {
        let message = capnp::serialize_packed::read_message(
            &mut std::io::Cursor::new(data),
            capnp::message::ReaderOptions::new(),
        ).map_err(|e| vec![format!("failed to parse capnp config: {}", e)])?;
        
        let config = message.get_root::<link_guard_config::Reader<'_>>()
            .map_err(|e| vec![format!("failed to get config root: {}", e)])?;

        let roots = config.get_roots().map_err(|e| vec![e.to_string()])?
            .iter().map(|r| r.unwrap().to_str().unwrap().to_string()).collect();
        
        let mut rules_vec = Vec::new();
        for r in config.get_rules().map_err(|e| vec![e.to_string()])? {
            let name = r.get_name().unwrap().to_str().unwrap().to_string();
            let regex = Regex::new(r.get_regex().unwrap().to_str().unwrap()).unwrap();
            let msg = r.get_message().unwrap().to_str().unwrap().to_string();
            rules_vec.push((name, regex, msg));
        }

        let allowlist = config.get_allowlist().map_err(|e| vec![e.to_string()])?
            .iter().map(|a| a.unwrap().to_str().unwrap().to_string()).collect();

        Ok((roots, rules_vec, allowlist))
    }

    fn load_sidecar_config(&self) -> Result<(Vec<String>, Vec<(String, Regex, String)>, Vec<String>), Vec<String>> {
        let sidecar_path = PathBuf::from("Components/mentci-aid/src/actors/link_guard.edn");
        let content = fs::read_to_string(&sidecar_path)
            .map_err(|e| vec![format!("failed to read sidecar config {}: {}", sidecar_path.display(), e)])?;
        
        let edn = Edn::from_str(&content)
            .map_err(|e| vec![format!("failed to parse sidecar EDN: {}", e)])?;
        
        let map = match edn {
            Edn::Map(m) => m.to_map(),
            _ => return Err(vec!["sidecar EDN must be a map".to_string()]),
        };

        let roots = self.get_edn_vector(&map, ":roots")?;
        let allowlist = self.get_edn_vector(&map, ":allowlist")?;
        
        let mut rules_vec = Vec::new();
        let rules_edn = map.get(":rules").ok_or_else(|| vec!["missing :rules in sidecar".to_string()])?;
        if let Edn::Vector(v) = rules_edn {
            for item in v.clone().to_vec() {
                if let Edn::Map(m) = item {
                    let m = m.to_map();
                    let name = m.get(":name").and_then(|e| if let Edn::Str(s) = e { Some(s.clone().trim_matches('"').to_string()) } else { None })
                        .ok_or_else(|| vec!["rule missing :name".to_string()])?;
                    let regex_str = m.get(":regex").and_then(|e| if let Edn::Str(s) = e { Some(s.clone().trim_matches('"').to_string()) } else { None })
                        .ok_or_else(|| vec!["rule missing :regex".to_string()])?;
                    let message = m.get(":message").and_then(|e| if let Edn::Str(s) = e { Some(s.clone().trim_matches('"').to_string()) } else { None })
                        .ok_or_else(|| vec!["rule missing :message".to_string()])?;
                    
                    let regex = Regex::new(&regex_str).map_err(|e| vec![format!("invalid regex {}: {}", regex_str, e)])?;
                    rules_vec.push((name, regex, message));
                }
            }
        }

        Ok((roots, rules_vec, allowlist))
    }

    fn get_edn_vector(&self, map: &std::collections::BTreeMap<String, Edn>, key: &str) -> Result<Vec<String>, Vec<String>> {
        let val = map.get(key).ok_or_else(|| vec![format!("missing {} in sidecar", key)])?;
        if let Edn::Vector(v) = val {
            Ok(v.clone().to_vec().iter().map(|e| if let Edn::Str(s) = e { s.clone().trim_matches('"').to_string() } else { e.to_string() }).collect())
        } else {
            Err(vec![format!("{} must be a vector", key)])
        }
    }
}
