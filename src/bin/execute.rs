use ractor::Actor;
use std::env;
use mentci_aid::actors::{Orchestrator, SymbolicMessage};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    
    if args.is_empty() {
        println!("execute: actor-based symbolic orchestrator");
        println!("usage: execute <command> [args...]");
        println!("commands: root-guard, link-guard, session-guard, version, unify, intent, report, finalize, transition");
        return Ok(());
    }

    let (orchestrator, _handle) = Actor::spawn(None, Orchestrator, ()).await?;

    match args[0].as_str() {
        "root-guard" => {
            let res = ractor::call!(orchestrator, SymbolicMessage::ValidateRoot)?;
            match res {
                Ok(_) => println!("Root guard passed."),
                Err(errors) => {
                    eprintln!("Root guard failed:");
                    for err in errors { eprintln!("- {}", err); }
                    std::process::exit(1);
                }
            }
        }
        "link-guard" => {
            let res = ractor::call!(orchestrator, SymbolicMessage::ValidateLinks)?;
            match res {
                Ok(_) => println!("Reference guard passed."),
                Err(errors) => {
                    eprintln!("Reference guard failed:");
                    for err in errors { eprintln!("- {}", err); }
                    std::process::exit(1);
                }
            }
        }
        "session-guard" => {
            let res = ractor::call!(orchestrator, SymbolicMessage::ValidateSession)?;
            match res {
                Ok(_) => println!("Session guard passed."),
                Err(errors) => {
                    eprintln!("Session guard failed:");
                    for err in errors { eprintln!("- {}", err); }
                    std::process::exit(1);
                }
            }
        }
        "version" => {
            let version = ractor::call!(orchestrator, SymbolicMessage::GetProgramVersion)?;
            println!("{}", version);
        }
        "unify" => {
            let write = args.contains(&"--write".to_string());
            let res = ractor::call!(orchestrator, SymbolicMessage::UnifySubjects, write)?;
            if let Err(e) = res {
                eprintln!("Unification failed: {}", e);
                std::process::exit(1);
            }
        }
        "intent" => {
            if args.len() < 2 {
                eprintln!("usage: execute intent <name>");
                std::process::exit(1);
            }
            let res = ractor::call!(orchestrator, SymbolicMessage::InitializeIntent, args[1].clone())?;
            match res {
                Ok(bookmark) => println!("{}", bookmark),
                Err(e) => {
                    eprintln!("Intent initialization failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "report" => {
            let mut prompt = String::new();
            let mut answer = String::new();
            let mut subject = String::new();
            let mut title = String::new();
            let mut kind = "answer".to_string();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--prompt" => { i += 1; prompt = args.get(i).cloned().unwrap_or_default(); }
                    "--answer" => { i += 1; answer = args.get(i).cloned().unwrap_or_default(); }
                    "--subject" => { i += 1; subject = args.get(i).cloned().unwrap_or_default(); }
                    "--title" => { i += 1; title = args.get(i).cloned().unwrap_or_default(); }
                    "--kind" => { i += 1; kind = args.get(i).cloned().unwrap_or("answer".to_string()); }
                    _ => {}
                }
                i += 1;
            }

            let res = ractor::call!(orchestrator, SymbolicMessage::EmitReport, prompt, answer, subject, title, kind)?;
            match res {
                Ok(path) => println!("Report emitted to: {}", path.display()),
                Err(e) => {
                    eprintln!("Report emission failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "launcher" => {
            let res = ractor::call!(orchestrator, SymbolicMessage::LaunchJail)?;
            if let Err(e) = res {
                eprintln!("Launch failed: {}", e);
                std::process::exit(1);
            }
        }
        "transition" => {
            let res = ractor::call!(orchestrator, SymbolicMessage::TransitionSession)?;
            if let Err(e) = res {
                eprintln!("Transition failed: {}", e);
                std::process::exit(1);
            }
        }
        "finalize" => {
            let mut summary = String::new();
            let mut prompt = String::new();
            let mut context = String::new();
            let mut changes = Vec::new();
            let mut bookmark = "dev".to_string();
            let mut remote = "origin".to_string();
            let mut rev = "@".to_string();
            let mut no_push = false;
            let mut model = String::new();

            if let Ok(content) = std::fs::read_to_string(".mentci/session.json") {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(s) = data.get("summary").and_then(|v| v.as_str()) { summary = s.to_string(); }
                    if let Some(s) = data.get("prompt").and_then(|v| v.as_str()) { prompt = s.to_string(); }
                    if let Some(s) = data.get("context").and_then(|v| v.as_str()) { context = s.to_string(); }
                    if let Some(s) = data.get("model").and_then(|v| v.as_str()) { model = s.to_string(); }
                    if let Some(c) = data.get("changes").and_then(|v| v.as_array()) {
                        changes = c.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    }
                }
            }

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--summary" => { i += 1; summary = args.get(i).cloned().unwrap_or_default(); }
                    "--prompt" => { i += 1; prompt = args.get(i).cloned().unwrap_or_default(); }
                    "--context" => { i += 1; context = args.get(i).cloned().unwrap_or_default(); }
                    "--change" => { i += 1; if let Some(c) = args.get(i) { changes.push(c.clone()); } }
                    "--bookmark" => { i += 1; bookmark = args.get(i).cloned().unwrap_or("dev".to_string()); }
                    "--remote" => { i += 1; remote = args.get(i).cloned().unwrap_or("origin".to_string()); }
                    "--rev" => { i += 1; rev = args.get(i).cloned().unwrap_or("@".to_string()); }
                    "--no-push" => { no_push = true; }
                    "--model" => { i += 1; model = args.get(i).cloned().unwrap_or_default(); }
                    _ => {}
                }
                i += 1;
            }

            let res = ractor::call!(orchestrator, SymbolicMessage::FinalizeSession, summary, prompt, context, changes, bookmark, remote, rev, no_push, model)?;
            if let Err(e) = res {
                eprintln!("Finalization failed: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Unknown command: {}", args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}
