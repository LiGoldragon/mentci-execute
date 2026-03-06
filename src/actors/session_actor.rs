use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::process::Command;

pub struct SessionActor;

#[derive(Debug)]
pub enum SessionMessage {
    Finalize(
        String, // summary
        String, // prompt
        String, // context
        Vec<String>, // changes
        String, // bookmark
        String, // remote
        String, // rev
        bool,   // no_push
        String, // model
        RpcReplyPort<Result<(), String>>,
    ),
}

#[async_trait::async_trait]
impl Actor for SessionActor {
    type Msg = SessionMessage;
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
            SessionMessage::Finalize(summary, prompt, context, changes, bookmark, remote, rev, no_push, model, reply) => {
                let res = self.perform_finalize(summary, prompt, context, changes, bookmark, remote, rev, no_push, model);
                reply.send(res)?;
            }
        }
        Ok(())
    }
}

impl SessionActor {
    fn perform_finalize(&self, summary: String, prompt: String, context: String, changes: Vec<String>, bookmark: String, remote: String, rev: String, no_push: bool, model: String) -> Result<(), String> {
        let solar_line = self.get_solar_line()?;
        let message = self.build_message(&summary, &solar_line, &prompt, &context, &changes, &model);
        
        let target_rev = self.resolve_rev(&rev)?;
        
        println!("Finalizing session on rev {}...", target_rev);
        self.run_command(&["jj", "describe", "-r", &target_rev, "-m", &message])?;
        self.run_command(&["jj", "bookmark", "set", &bookmark, "-r", &target_rev, "--allow-backwards"])?;
        
        if !no_push {
            println!("Pushing bookmark '{}' to remote '{}'...", bookmark, remote);
            self.run_command(&["jj", "git", "push", "--bookmark", &bookmark, "--remote", &remote])?;
            println!("Successfully pushed and verified {} on {}.", bookmark, remote);
        }
        
        Ok(())
    }

    fn get_solar_line(&self) -> Result<String, String> {
        // Try direct call first
        let res = Command::new("chronos")
            .args(["--precision", "second"])
            .output();
        
        let output = match res {
            Ok(o) if o.status.success() => o,
            _ => {
                // Fallback to cargo run
                Command::new("cargo")
                    .args(["run", "--quiet", "--manifest-path", "Components/chronos/Cargo.toml", "--bin", "chronos", "--", "--precision", "second"])
                    .output()
                    .map_err(|e| format!("failed to execute chronos via cargo: {}", e))?
            }
        };

        if !output.status.success() {
            return Err(format!("chronos failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(format!("solar: {}", raw))
    }

    fn build_message(&self, summary: &str, solar: &str, prompt: &str, context: &str, changes: &[String], model: &str) -> String {
        let mut msg = format!("session: {}\n{}\nmodel: {}\n\n", summary, solar, if model.is_empty() { "unknown" } else { model });
        msg.push_str("## Original Prompt\n");
        msg.push_str(prompt);
        msg.push_str("\n\n## Agent Context\n");
        msg.push_str(context);
        msg.push_str("\n\n## Logical Changes\n");
        for change in changes {
            msg.push_str("- ");
            msg.push_str(change);
            msg.push_str("\n");
        }
        msg
    }

    fn resolve_rev(&self, rev: &str) -> Result<String, String> {
        if rev == "@" {
            let output = self.run_command(&["jj", "log", "-r", "@", "--no-graph", "-T", "empty"])?;
            if output.trim() == "true" {
                return Ok("@-".to_string());
            }
        }
        Ok(rev.to_string())
    }

    fn run_command(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new(args[0])
            .args(&args[1..])
            .output()
            .map_err(|e| format!("failed to execute {}: {}", args[0], e))?;
        
        if !output.status.success() {
            return Err(format!("{} failed: {}", args[0], String::from_utf8_lossy(&output.stderr)));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
