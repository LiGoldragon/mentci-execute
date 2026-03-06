//! mentci-aid (Mentci-AI Daemon / Aid)
//! 
//! The core execution engine for the Mentci-AI project.
//! - Daemon: Background pipeline execution.
//! - Aid: Symbolic help for the human mind.
//!
//! STATUS: Not in a running state.

use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tracing::{error, info};

use mentci_aid::dot_loader::DotLoader;
use mentci_aid::edn_loader::EdnLoader;
use mentci_aid::attractor_validator::AttractorValidator;
use mentci_aid::jail_bootstrap;
use mentci_aid::sandbox;

// --- Execution Environment ---

pub trait ExecutionEnvironment {
    fn read_file(&self, path: &PathBuf) -> Result<String>;
    fn write_file(&self, path: &PathBuf, content: &str) -> Result<()>;
    fn exec_command(&self, command: &str) -> Result<ExecResult>;
    fn working_directory(&self) -> PathBuf;
}

pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct LocalExecutionEnvironment {
    pub workdir: PathBuf,
}

impl LocalExecutionEnvironment {
    pub fn new(path: PathBuf) -> Self {
        Self { workdir: path }
    }
}

impl ExecutionEnvironment for LocalExecutionEnvironment {
    fn read_file(&self, path: &PathBuf) -> Result<String> {
        let full_path = self.workdir.join(path);
        Ok(std::fs::read_to_string(full_path)?)
    }

    fn write_file(&self, path: &PathBuf, content: &str) -> Result<()> {
        let full_path = self.workdir.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full_path, content)?;
        Ok(())
    }

    fn exec_command(&self, command: &str) -> Result<ExecResult> {
        let output = Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(&self.workdir)
            .output()?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn working_directory(&self) -> PathBuf {
        self.workdir.clone()
    }
}

// --- Context & State ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub values: HashMap<String, String>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub status: StageStatus,
    pub notes: Option<String>,
    pub context_updates: HashMap<String, String>,
    pub preferred_label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageStatus {
    Success,
    PartialSuccess,
    Fail,
    Retry,
    Skipped,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub node_id: String,
    pub timestamp: u64,
    pub context: Context,
    pub outcome: Outcome,
}

pub struct CheckpointManager {
    pub checkpoint_dir: PathBuf,
}

impl CheckpointManager {
    pub fn new(workdir: &PathBuf) -> Self {
        let checkpoint_dir = workdir.join(".checkpoints");
        if !checkpoint_dir.exists() {
            let _ = std::fs::create_dir_all(&checkpoint_dir);
        }
        Self { checkpoint_dir }
    }

    pub fn save(&self, checkpoint: &Checkpoint) -> Result<()> {
        let filename = format!("{}_{}.json", checkpoint.timestamp, checkpoint.node_id);
        let path = self.checkpoint_dir.join(filename);
        let json = serde_json::to_string_pretty(checkpoint)?;
        std::fs::write(&path, json)?;
        info!("Checkpoint saved: {:?}", path);
        Ok(())
    }
}

// --- Handlers ---

pub struct TaskContext<'a> {
    pub context: &'a mut Context,
    pub env: &'a dyn ExecutionEnvironment,
}

pub trait Handler {
    fn execute(&self, task: &mut TaskContext) -> Result<Outcome>;
}

struct StartHandler;
impl Handler for StartHandler {
    fn execute(&self, _task: &mut TaskContext) -> Result<Outcome> {
        Ok(Outcome {
            status: StageStatus::Success,
            notes: Some("Start node executed.".to_string()),
            context_updates: HashMap::new(),
            preferred_label: None,
        })
    }
}

struct ExitHandler;
impl Handler for ExitHandler {
    fn execute(&self, _task: &mut TaskContext) -> Result<Outcome> {
        Ok(Outcome {
            status: StageStatus::Success,
            notes: Some("Workflow exited.".to_string()),
            context_updates: HashMap::new(),
            preferred_label: None,
        })
    }
}

struct CodergenHandler {
    prompt: String,
}
impl Handler for CodergenHandler {
    fn execute(&self, _task: &mut TaskContext) -> Result<Outcome> {
        info!("Executing Codergen: {}", self.prompt);
        // Simulate LLM call
        let response = format!("// Generated code for: {}", self.prompt);
        let mut updates = HashMap::new();
        updates.insert("last_response".to_string(), response);

        Ok(Outcome {
            status: StageStatus::Success,
            notes: Some("Codergen completed.".to_string()),
            context_updates: updates,
            preferred_label: None,
        })
    }
}

struct AgentHandler {
    prompt: String,
    command: String,
}
impl Handler for AgentHandler {
    fn execute(&self, task: &mut TaskContext) -> Result<Outcome> {
        let command = if self.command.contains("{prompt}") {
            let escaped = shell_quote(&self.prompt);
            self.command.replace("{prompt}", &escaped)
        } else {
            self.command.clone()
        };

        info!("Executing Agent command.");
        let result = task.env.exec_command(&command)?;
        if result.exit_code != 0 {
            return Ok(Outcome {
                status: StageStatus::Fail,
                notes: Some(format!("Agent command failed: {}", result.stderr)),
                context_updates: HashMap::new(),
                preferred_label: None,
            });
        }

        let mut updates = HashMap::new();
        updates.insert(
            "agent_response".to_string(),
            result.stdout.trim().to_string(),
        );

        Ok(Outcome {
            status: StageStatus::Success,
            notes: Some("Agent command completed.".to_string()),
            context_updates: updates,
            preferred_label: None,
        })
    }
}

struct WaitHumanHandler;
impl Handler for WaitHumanHandler {
    fn execute(&self, _task: &mut TaskContext) -> Result<Outcome> {
        info!("Waiting for human intervention...");
        println!(">>> HUMAN GATE <<<");
        println!("Press Enter to continue, or type 'fail' to abort.");

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            if input.trim().to_lowercase() == "fail" {
                return Ok(Outcome {
                    status: StageStatus::Fail,
                    notes: Some("Aborted by user.".to_string()),
                    context_updates: HashMap::new(),
                    preferred_label: None,
                });
            }
        }
        Ok(Outcome {
            status: StageStatus::Success,
            notes: Some("Approved by user.".to_string()),
            context_updates: HashMap::new(),
            preferred_label: None,
        })
    }
}

// --- Engine ---

pub struct Node {
    pub id: String,
    pub handler: Box<dyn Handler>,
    pub prompt: Option<String>,
}

pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub condition: Option<String>,
    pub weight: i32,
}

pub struct Graph {
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
    pub start_node: String,
}

pub struct RoutingContext<'a> {
    pub current_node_id: &'a str,
    pub outcome: &'a Outcome,
    pub context: &'a Context,
}

pub struct PipelineEngine {
    pub graph: Graph,
    pub env: Box<dyn ExecutionEnvironment>,
    pub checkpoint_manager: CheckpointManager,
}

fn shell_quote(value: &str) -> String {
    let escaped = value.replace('\'', r#"'\''"#);
    format!("'{}'", escaped)
}

impl PipelineEngine {
    pub fn new(graph: Graph, env: Box<dyn ExecutionEnvironment>) -> Self {
        let workdir = env.working_directory();
        Self {
            graph,
            env,
            checkpoint_manager: CheckpointManager::new(&workdir),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut context = Context::new();
        let mut current_node_id = self.graph.start_node.clone();

        loop {
            let node = self
                .graph
                .nodes
                .get(&current_node_id)
                .ok_or_else(|| anyhow::anyhow!("Node not found: {}", current_node_id))?;

            info!(">>> NODE: {} <<<", node.id);

            let mut task = TaskContext {
                context: &mut context,
                env: self.env.as_ref(),
            };

            let outcome = node.handler.execute(&mut task)?;

            // Checkpoint
            let checkpoint = Checkpoint {
                node_id: current_node_id.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs(),
                context: context.clone(),
                outcome: outcome.clone(),
            };
            self.checkpoint_manager.save(&checkpoint)?;

            // Apply updates
            for (k, v) in &outcome.context_updates {
                context.values.insert(k.clone(), v.clone());
            }

            if outcome.status == StageStatus::Fail {
                error!("Node {} failed.", current_node_id);
                break;
            }

            // Routing
            let routing = RoutingContext {
                current_node_id: &current_node_id,
                outcome: &outcome,
                context: &context,
            };

            match self.select_next_node(routing) {
                Ok(next_id) => {
                    if next_id == "exit" || self.graph.nodes.get(&next_id).is_none() {
                        // Exit implicit or explicit
                        // If explicit exit node exists, we run it next loop.
                        // But if select_next_node returns "exit" and it's NOT in nodes, we stop.
                        if !self.graph.nodes.contains_key(&next_id) {
                            info!("Exiting workflow (terminal).");
                            break;
                        }
                    }
                    current_node_id = next_id;
                }
                Err(e) => {
                    // Check if we are at a terminal node (no outgoing edges)
                    info!("Workflow ended: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    fn select_next_node(&self, routing: RoutingContext) -> Result<String> {
        let mut candidates = Vec::new();
        for edge in &self.graph.edges {
            if edge.from == routing.current_node_id {
                candidates.push(edge);
            }
        }

        if candidates.is_empty() {
            return Err(anyhow::anyhow!("No outgoing edges"));
        }

        // Simple routing: Look for preferred label match, else take first unconditional
        if let Some(preferred) = &routing.outcome.preferred_label {
            for edge in &candidates {
                if edge.label.as_ref() == Some(preferred) {
                    return Ok(edge.to.clone());
                }
            }
        }

        // Fallback to first edge (naive weight support)
        // Sort by weight desc?
        candidates.sort_by(|a, b| b.weight.cmp(&a.weight));

        Ok(candidates[0].to.clone())
    }
}

// --- Main ---

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "job/jails" {
        let bootstrap_args = args.into_iter().skip(2).collect::<Vec<_>>();
        return jail_bootstrap::run_from_args(bootstrap_args);
    }
    if args.len() >= 3 && args[1] == "job" && args[2] == "jails" {
        let bootstrap_args = args.into_iter().skip(3).collect::<Vec<_>>();
        return jail_bootstrap::run_from_args(bootstrap_args);
    }
    if args.len() >= 2 && args[1] == "sandbox" {
        let sandbox_args = args.into_iter().skip(2).collect::<Vec<_>>();
        return sandbox::run_from_args(sandbox_args);
    }
    if args.len() >= 3 && args[1] == "execute" && args[2] == "sandbox" {
        let sandbox_args = args.into_iter().skip(3).collect::<Vec<_>>();
        return sandbox::run_from_args(sandbox_args);
    }

    if args.len() < 2 {
        println!("mentci-aid (Mentci-AI Daemon) - STATUS: NOT IN A RUNNING STATE");
        println!("Usage: mentci-aid <workflow.dot|workflow.aski-flow|workflow.edn>");
        println!("       mentci-aid job/jails [bootstrap] [options]");
        println!("       mentci-aid sandbox [options] -- <command> [args]");
        println!("       mentci-aid execute sandbox [options] -- <command> [args]");
        return Ok(());
    }

    let path = PathBuf::from(&args[1]);
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    let content = std::fs::read_to_string(&path).context("Failed to read workflow file")?;

    // --- Validation Step ---
    if extension == "dot" {
        info!("Validating Attractor DOT structure...");
        match AttractorValidator::validate(&content) {
            Ok(result) => {
                if !result.is_valid {
                    error!("Validation failed:");
                    for err in result.errors {
                        error!("- {}", err);
                    }
                    std::process::exit(1);
                }
                info!("Validation passed: {} nodes, {} edges.", result.node_count, result.edge_count);
            }
            Err(e) => {
                error!("Validation error: {}", e);
                std::process::exit(1);
            }
        }
    }

    let dot_graph = match extension {
        "dot" => DotLoader::parse(&content).context("Failed to parse DOT")?,
        "edn" | "aski-flow" => EdnLoader::parse(&content).context("Failed to parse Aski-Flow")?,
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported workflow format: .{}",
                extension
            ))
        }
    };

    // Hydrate Graph
    let mut nodes = HashMap::new();
    let mut start_node = None;

    for (id, d_node) in dot_graph.nodes {
        // Determine Handler
        let shape = d_node.shape.as_deref().unwrap_or("box");
        let n_type = d_node.node_type.as_deref().unwrap_or("");
        let agent_command = d_node
            .attributes
            .get("agent_command")
            .cloned()
            .or_else(|| std::env::var("MENTCI_AGENT_COMMAND").ok());

        let handler: Box<dyn Handler> = if id == "start" || shape == "Mdiamond" || n_type == "start"
        {
            if start_node.is_none() {
                start_node = Some(id.clone());
            }
            Box::new(StartHandler)
        } else if id == "exit" || shape == "Msquare" || n_type == "exit" {
            Box::new(ExitHandler)
        } else if n_type == "wait.human" || n_type == "wait_human" || shape == "hexagon" {
            Box::new(WaitHumanHandler)
        } else if n_type == "agent" || agent_command.is_some() {
            let prompt = d_node
                .prompt
                .clone()
                .unwrap_or_else(|| d_node.label.clone().unwrap_or(id.clone()));
            let command = agent_command
                .ok_or_else(|| anyhow::anyhow!("agent_command is required for agent nodes"))?;
            Box::new(AgentHandler { prompt, command })
        } else {
            // Default to Codergen
            let prompt = d_node
                .prompt
                .clone()
                .unwrap_or_else(|| d_node.label.clone().unwrap_or(id.clone()));
            Box::new(CodergenHandler { prompt })
        };

        nodes.insert(
            id.clone(),
            Node {
                id,
                handler,
                prompt: d_node.prompt,
            },
        );
    }

    // Edges
    let mut edges = Vec::new();
    for d_edge in dot_graph.edges {
        edges.push(Edge {
            from: d_edge.from,
            to: d_edge.to,
            label: d_edge.label,
            condition: d_edge.condition,
            weight: d_edge.weight.unwrap_or(0),
        });
    }

    let start_node_id = start_node.unwrap_or_else(|| "start".to_string());
    if !nodes.contains_key(&start_node_id) {
        error!("Start node '{}' not found in graph.", start_node_id);
        std::process::exit(1);
    }

    let graph = Graph {
        nodes,
        edges,
        start_node: start_node_id,
    };

    info!(
        "Loaded graph with {} nodes and {} edges.",
        graph.nodes.len(),
        graph.edges.len()
    );

    let env = Box::new(LocalExecutionEnvironment::new(std::env::current_dir()?));
    let mut engine = PipelineEngine::new(graph, env);

    engine.run()?;

    Ok(())
}
