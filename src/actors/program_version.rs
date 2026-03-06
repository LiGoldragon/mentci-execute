use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use sha2::{Sha256, Digest};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use edn_rs::Edn;
use std::str::FromStr;
use std::collections::BTreeMap;

pub struct ProgramVersion;

#[derive(Debug)]
pub enum ProgramVersionMessage {
    Get(RpcReplyPort<String>),
}

#[async_trait::async_trait]
impl Actor for ProgramVersion {
    type Msg = ProgramVersionMessage;
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
            ProgramVersionMessage::Get(reply) => {
                let version = self.calculate_version();
                reply.send(version)?;
            }
        }
        Ok(())
    }
}

impl ProgramVersion {
    fn calculate_version(&self) -> String {
        let config = self.load_sidecar_config().unwrap_or_default();
        let core_paths = self.get_edn_vector(&config, ":core-paths").unwrap_or_else(|_| vec!["Core".to_string()]);
        let alphabet = config.get(":alphabet").and_then(|e| Some(e.to_string().trim_matches('"').to_string())).unwrap_or_else(|| "zkwpqrstnmvxlhgybjdf0123456789".to_string());

        let mut actual_path = "Core";
        for p in core_paths {
            if Path::new(&p).exists() {
                actual_path = Box::leak(p.into_boxed_str());
                break;
            }
        }

        let mut files: Vec<_> = WalkDir::new(actual_path)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .collect();
        files.sort_by_key(|e| e.path().to_path_buf());

        let mut hasher = Sha256::new();
        for file in files {
            if let Ok(content) = fs::read(file.path()) {
                hasher.update(content);
            }
        }
        let hash = hasher.finalize();
        self.encode_jj(&hash, 8, &alphabet)
    }

    fn encode_jj(&self, bytes: &[u8], length: usize, alphabet: &str) -> String {
        let mut big_int = num_bigint::BigUint::from_bytes_be(bytes);
        let base = num_bigint::BigUint::from(alphabet.len());
        let mut res = String::new();

        while big_int > num_bigint::BigUint::from(0u32) && res.len() < length {
            let rem = &big_int % &base;
            let idx = rem.to_u32_digits().first().cloned().unwrap_or(0) as usize;
            res.push(alphabet.chars().nth(idx).unwrap());
            big_int /= &base;
        }
        res.chars().rev().collect()
    }

    fn load_sidecar_config(&self) -> Result<BTreeMap<String, Edn>, Vec<String>> {
        let sidecar_path = PathBuf::from("Components/mentci-aid/src/actors/program_version.edn");
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
