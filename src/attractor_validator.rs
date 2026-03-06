use anyhow::Result;
use dot_parser::ast::{Graph as AstGraph, Stmt};
use std::collections::HashSet;
use std::convert::TryFrom;

pub struct AttractorValidator;

#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub node_count: usize,
    pub edge_count: usize,
}

impl AttractorValidator {
    pub fn validate(graph_str: &str) -> Result<ValidationResult> {
        let graph = AstGraph::try_from(graph_str)
            .map_err(|e| anyhow::anyhow!("DOT Parse Error: {}", e))?;

        let mut errors = Vec::new();
        let mut nodes = HashSet::new();
        let mut edges_count = 0;
        let mut has_start = false;
        let mut has_exit = false;

        // AstGraph is just a struct, not a vector of graphs.
        // It represents one graph.
        for stmt in &graph.stmts {
            match stmt {
                Stmt::NodeStmt(n) => {
                    let id = n.node.id.as_str().to_string();
                    nodes.insert(id.clone());
                    if id == "start" {
                        has_start = true;
                    }
                    if id == "exit" {
                        has_exit = true;
                    }
                }
                Stmt::EdgeStmt(_) => {
                    edges_count += 1;
                }
                _ => {}
            }
        }

        if !has_start {
            errors.push("Missing required node: 'start'".to_string());
        }
        if !has_exit {
            errors.push("Missing required node: 'exit'".to_string());
        }

        if nodes.is_empty() {
            errors.push("Graph is empty".to_string());
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            node_count: nodes.len(),
            edge_count: edges_count,
        })
    }
}
