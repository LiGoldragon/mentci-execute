use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::process::Command;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ReportActor;

#[derive(Debug)]
pub enum ReportMessage {
    Emit(
        String, // prompt
        String, // answer
        String, // subject
        String, // title
        String, // kind
        RpcReplyPort<Result<PathBuf, String>>,
    ),
}

#[async_trait::async_trait]
impl Actor for ReportActor {
    type Msg = ReportMessage;
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
            ReportMessage::Emit(prompt, answer, subject, title, kind, reply) => {
                let res = self.perform_emit(prompt, answer, subject, title, kind);
                reply.send(res)?;
            }
        }
        Ok(())
    }
}

impl ReportActor {
    fn perform_emit(&self, prompt: String, answer: String, subject: String, title: String, kind: String) -> Result<PathBuf, String> {
        let solar = self.get_solar_timestamp()?;
        
        // Clean solar timestamp for filename: AM year + zodiac ordinal + deg + min + sec
        // Format from chronos: 5919.12.6.5.30
        let cleaned_solar = solar.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
        let filename = format!("{}_{}_{}.md", cleaned_solar, kind, title);
        
        let tier = self.get_subject_tier(&subject);
        let target_dir = PathBuf::from(format!("Research/{}/{}", tier, subject));
        fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
        
        let target_path = target_dir.join(filename);
        
        let mut content = format!("# Research Artifact: {}\n\n", title.replace('-', " "));
        content.push_str(&format!("- **Solar:** {}\n", solar));
        content.push_str(&format!("- **Subject:** `{}`\n", subject));
        content.push_str(&format!("- **Title:** `{}`\n", title));
        content.push_str(&format!("- **Status:** `finalized`\n\n"));
        
        content.push_str("## 1. Intent\n");
        content.push_str(&prompt);
        content.push_str("\n\n## 2. Answer\n");
        content.push_str(&answer);
        content.push_str("\n");
        
        fs::write(&target_path, content).map_err(|e| e.to_string())?;
        
        Ok(target_path)
    }

    fn get_solar_timestamp(&self) -> Result<String, String> {
        let res = Command::new("chronos")
            .args(["--format", "am", "--precision", "second"])
            .output();
        
        let output = match res {
            Ok(o) if o.status.success() => o,
            _ => {
                Command::new("cargo")
                    .args(["run", "--quiet", "--manifest-path", "Components/chronos/Cargo.toml", "--bin" , "chronos", "--", "--format", "am", "--precision", "second"])
                    .output()
                    .map_err(|e| format!("failed to execute chronos via cargo: {}", e))?
            }
        };

        if !output.status.success() {
            return Err(format!("chronos failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_subject_tier(&self, subject: &str) -> String {
        for tier in ["high", "medium", "low"] {
            if Path::new(&format!("Development/{}/{}", tier, subject)).exists() ||
               Path::new(&format!("Research/{}/{}", tier, subject)).exists() {
                return tier.to_string();
            }
        }
        "high".to_string()
    }
}
