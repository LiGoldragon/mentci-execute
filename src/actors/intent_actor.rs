use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::process::Command;

pub struct IntentActor;

#[derive(Debug)]
pub enum IntentMessage {
    Initialize(String, RpcReplyPort<Result<String, String>>),
}

#[async_trait::async_trait]
impl Actor for IntentActor {
    type Msg = IntentMessage;
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
            IntentMessage::Initialize(name, reply) => {
                let res = self.perform_init(name);
                reply.send(res)?;
            }
        }
        Ok(())
    }
}

impl IntentActor {
    fn perform_init(&self, name: String) -> Result<String, String> {
        let slug = self.sanitize_name(&name);
        let hash = self.generate_hash();
        let bookmark = format!("{}-{}", hash, slug);
        
        println!("Initializing Unique Intent: {}...", bookmark);
        
        // jj new dev -m "intent: <name>"
        self.run_command(&["jj", "new", "dev", "-m", &format!("intent: {}", name)])?;
        // jj bookmark create <bookmark> -r @
        self.run_command(&["jj", "bookmark", "create", &bookmark, "-r", "@"])?;
        
        Ok(bookmark)
    }

    fn sanitize_name(&self, name: &str) -> String {
        name.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-")
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    fn generate_hash(&self) -> String {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        format!("{:x}", now).chars().take(8).collect()
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
