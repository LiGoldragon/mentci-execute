use anyhow::Result;
use dot_parser::ast::Graph as AstGraph;
use dot_parser::canonical::AttrStmt;
use dot_parser::canonical::Graph as CanonicalGraph;
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub label: Option<String>,
    pub prompt: Option<String>,
    pub shape: Option<String>,     // Defines Handler Type
    pub node_type: Option<String>, // Explicit override
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub condition: Option<String>,
    pub weight: Option<i32>,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub id: Option<String>,
    pub goal: Option<String>,
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
    pub attributes: HashMap<String, String>,
}

pub struct DotLoader;

impl DotLoader {
    pub fn parse(content: &str) -> Result<Graph> {
        // Parse into AST
        let ast_graph = AstGraph::try_from(content)
            .map_err(|e| anyhow::anyhow!("Failed to parse DOT: {}", e))?;

        // Convert to Canonical Graph for easier processing
        let canonical_graph = CanonicalGraph::from(ast_graph);

        let mut nodes = HashMap::new();
        let mut edges = Vec::new();
        let mut graph_attrs = HashMap::new();
        let graph_id = canonical_graph
            .name
            .clone()
            .map(|name| name.trim_matches('"').to_string());

        for attr in &canonical_graph.attr {
            if let AttrStmt::Graph((k, v)) = attr {
                let key: String = k.clone().into();
                let value: String = v.clone().into();
                graph_attrs.insert(
                    key.trim_matches('"').to_string(),
                    value.trim_matches('"').to_string(),
                );
            }
        }

        // Process Attributes (Graph level)
        // Canonical graph might flatten attributes differently.
        // Let's assume we can access them if needed, but for now focus on nodes/edges.

        // Process Nodes
        for c_node in canonical_graph.nodes.set.into_values() {
            let id = c_node.id.to_string();
            let mut attrs = HashMap::new();

            for (k, v) in c_node.attr.elems {
                let key: String = k.into();
                let value: String = v.into();
                attrs.insert(key, value.trim_matches('"').to_string());
            }

            let label = attrs.get("label").cloned();
            let prompt = attrs.get("prompt").cloned();
            let shape = attrs.get("shape").cloned();
            let node_type = attrs.get("type").cloned();

            nodes.insert(
                id.clone(),
                Node {
                    id,
                    label,
                    prompt,
                    shape,
                    node_type,
                    attributes: attrs,
                },
            );
        }

        // Process Edges
        for c_edge in canonical_graph.edges.set {
            let from = c_edge.from.to_string();
            let to = c_edge.to.to_string();

            let mut attrs = HashMap::new();
            for (k, v) in c_edge.attr.elems {
                let key: String = k.into();
                let value: String = v.into();
                attrs.insert(key, value.trim_matches('"').to_string());
            }

            let label = attrs.get("label").cloned();
            let condition = attrs.get("condition").cloned();
            let weight = attrs.get("weight").and_then(|w| w.parse().ok());

            edges.push(Edge {
                from,
                to,
                label,
                condition,
                weight,
                attributes: attrs,
            });
        }

        let goal = graph_attrs.get("goal").cloned();

        Ok(Graph {
            id: graph_id,
            goal,
            nodes,
            edges,
            attributes: graph_attrs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::DotLoader;

    #[test]
    fn parses_basic_dot_workflow() {
        let dot = r#"
digraph "TestFlow" {
  start [shape="Mdiamond"];
  plan [shape="box" label="Plan" prompt="Write a plan"];
  exit [shape="Msquare"];

  start -> plan [label="ok" weight="2"];
  plan -> exit [condition="outcome=success"];
}
"#;

        let graph = DotLoader::parse(dot).expect("DOT parse should succeed");

        assert_eq!(graph.id.as_deref(), Some("TestFlow"));
        assert!(graph.nodes.contains_key("start"));
        assert!(graph.nodes.contains_key("plan"));
        assert!(graph.nodes.contains_key("exit"));

        let plan = graph.nodes.get("plan").expect("plan node");
        assert_eq!(plan.label.as_deref(), Some("Plan"));
        assert_eq!(plan.prompt.as_deref(), Some("Write a plan"));
        assert_eq!(plan.shape.as_deref(), Some("box"));

        assert_eq!(graph.edges.len(), 2);
        let start_edge = graph
            .edges
            .iter()
            .find(|e| e.from == "start" && e.to == "plan")
            .expect("start -> plan edge");
        assert_eq!(start_edge.label.as_deref(), Some("ok"));
        assert_eq!(start_edge.weight, Some(2));

        let exit_edge = graph
            .edges
            .iter()
            .find(|e| e.from == "plan" && e.to == "exit")
            .expect("plan -> exit edge");
        assert_eq!(exit_edge.condition.as_deref(), Some("outcome=success"));
    }
}
