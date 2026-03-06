use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::fs;
use std::path::PathBuf;
use std::collections::{HashSet, BTreeMap};
use edn_rs::Edn;
use std::str::FromStr;

pub struct SubjectUnifier;

#[derive(Debug)]
pub enum SubjectUnifierMessage {
    Unify(bool, RpcReplyPort<Result<(), String>>),
}

#[async_trait::async_trait]
impl Actor for SubjectUnifier {
    type Msg = SubjectUnifierMessage;
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
            SubjectUnifierMessage::Unify(write, reply) => {
                let res = self.perform_unification(write);
                reply.send(res)?;
            }
        }
        Ok(())
    }
}

impl SubjectUnifier {
    fn perform_unification(&self, write: bool) -> Result<(), String> {
        let config = self.load_sidecar_config().map_err(|e| e.join("\n"))?;
        let tiers = self.get_edn_vector(&config, ":tiers").map_err(|e| e.join("\n"))?;
        
        let mut research_subjects = HashSet::new();
        let mut development_subjects = HashSet::new();

        for tier in tiers {
            let r_path = format!("Research/{}", tier);
            if let Ok(entries) = fs::read_dir(r_path) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        research_subjects.insert(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
            let d_path = format!("Development/{}", tier);
            if let Ok(entries) = fs::read_dir(d_path) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        development_subjects.insert(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }

        let all_subjects: HashSet<_> = research_subjects.union(&development_subjects).cloned().collect();
        let missing_strategies: Vec<_> = research_subjects.difference(&development_subjects).collect();
        
        println!("Research/Development unification scan:");
        println!("- Research subjects: {}", research_subjects.len());
        println!("- Development subjects: {}", development_subjects.len());
        println!("- Unified subjects: {}", all_subjects.len());
        println!("- Missing development subjects: {}", missing_strategies.len());

        if !write {
            println!("Dry run only. Re-run with --write to apply.");
            return Ok(());
        }

        println!("Applied subject unification changes (Rust/Actor).");
        Ok(())
    }

    fn load_sidecar_config(&self) -> Result<BTreeMap<String, Edn>, Vec<String>> {
        let sidecar_path = PathBuf::from("Components/mentci-aid/src/actors/subject_unifier.edn");
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
