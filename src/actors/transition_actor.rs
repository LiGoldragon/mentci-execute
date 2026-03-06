use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::process::Command;
use std::fs::File;
use std::path::PathBuf;
use capnp::serialize_packed;
use anyhow::{Context, Result};
use crate::mentci_capnp;

pub struct TransitionActor;

#[derive(Debug)]
pub enum TransitionMessage {
    Run(RpcReplyPort<Result<(), String>>),
}

#[async_trait::async_trait]
impl Actor for TransitionActor {
    type Msg = TransitionMessage;
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
            TransitionMessage::Run(reply) => {
                let res = self.perform_transition().await;
                reply.send(res.map_err(|e| e.to_string()))?;
            }
        }
        Ok(())
    }
}

impl TransitionActor {
    async fn perform_transition(&self) -> Result<()> {
        println!("Checking repository integrity before transition...");
        
        // 1. Run checks
        self.run_check("root-guard")?;
        self.run_check("link-guard")?;
        self.run_check("session-guard")?;
        
        println!("Integrity checks passed. Preparing launch request...");

        // 2. Prepare MentciLaunchRequest
        let run_id = format!("transition-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs());
        let request_path = PathBuf::from(format!("/tmp/{}.capnp", run_id));
        
        let mut message = capnp::message::Builder::new_default();
        {
            let mut request = message.init_root::<mentci_capnp::mentci_launch_request::Builder>();
            request.set_run_id(&run_id);
            request.set_working_directory(std::env::current_dir()?.to_str().unwrap_or("."));
            request.set_launch_mode(mentci_capnp::MentciLaunchMode::Terminal);
            request.set_systemd_target(mentci_capnp::MentciLaunchSystemdTarget::UserScope);
            request.set_terminal_program("foot");
            request.set_agent_interface(mentci_capnp::MentciAgentInterface::PiTypescript);
            
            // We need a dummy box request for now or a real one if available
            request.set_box_request_capnp_path("/dev/null"); 
        }

        let mut file = File::create(&request_path)?;
        serialize_packed::write_message(&mut file, &message)?;

        println!("Launch request written to {}. Executing mentci-launch...", request_path.display());

        // 3. Execute mentci-launch
        let status = Command::new("mentci-launch")
            .arg(&request_path)
            .status()?;

        if !status.success() {
            anyhow::bail!("mentci-launch failed with status {}", status);
        }

        println!("Transition initiated in a new terminal.");
        Ok(())
    }

    fn run_check(&self, command: &str) -> Result<()> {
        let status = Command::new("execute")
            .arg(command)
            .status()
            .with_context(|| format!("failed to run check: {}", command))?;
            
        if !status.success() {
            anyhow::bail!("Check failed: {}", command);
        }
        Ok(())
    }
}
