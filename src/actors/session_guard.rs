use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::process::Command;

pub struct SessionGuard;

#[derive(Debug)]
pub enum SessionGuardMessage {
    Check(RpcReplyPort<Result<(), Vec<String>>>),
}

#[async_trait::async_trait]
impl Actor for SessionGuard {
    type Msg = SessionGuardMessage;
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
            SessionGuardMessage::Check(reply) => {
                let errors = self.perform_check();
                if errors.is_empty() {
                    reply.send(Ok(()))?;
                } else {
                    reply.send(Err(errors))?;
                }
            }
        }
        Ok(())
    }
}

impl SessionGuard {
    fn perform_check(&self) -> Vec<String> {
        let mut errors = Vec::new();
        
        // 1. Check for trailing intents
        // Fixed string literal concatenation issue
        if let Ok(log) = self.run_command(&["jj", "log", "-r", "::@", "--no-graph", "-T", "description.first_line() ++ \"\\n\""]) {
            let lines: Vec<_> = log.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            let mut intent_count = 0;
            for line in lines {
                if line.starts_with("session:") {
                    if intent_count > 0 {
                        errors.push(format!("found {} trailing intent commits above the latest session commit", intent_count));
                    }
                    break;
                } else if line.starts_with("intent:") {
                    intent_count += 1;
                }
            }
        }

        // 2. Validate session message
        if let Ok(desc) = self.run_command(&["git", "log", "-n", "1", "--pretty=%B"]) {
            if !desc.trim().starts_with("session:") {
                errors.push("HEAD commit is not a session commit".to_string());
            } else {
                let required = ["## Original Prompt", "## Agent Context", "## Logical Changes"];
                for req in required {
                    if !desc.contains(req) {
                        errors.push(format!("session commit missing required section: {}", req));
                    }
                }
            }
        }

        // 3. Validate push
        if let Ok(local_hash) = self.run_command(&["git", "rev-parse", "HEAD"]) {
            if let Ok(remote_out) = self.run_command(&["git", "ls-remote", "--heads", "origin", "dev"]) {
                let remote_hash = remote_out.split_whitespace().next().unwrap_or("");
                if local_hash.trim() != remote_hash.trim() {
                    errors.push(format!("HEAD is not pushed to origin/dev (local: {}, remote: {})", local_hash.trim(), remote_hash));
                }
            }
        }

        errors
    }

    fn run_command(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new(args[0])
            .args(&args[1..])
            .output()
            .map_err(|e| format!("failed to execute {}: {}", args[0], e))?;
        
        if !output.status.success() {
            return Err(format!("{} failed", args[0]));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
