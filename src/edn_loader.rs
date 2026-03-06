use crate::dot_loader::{Edge, Graph, Node};
use anyhow::{Context as AnyhowContext, Result};
use edn_rs::Edn;
use std::collections::HashMap;
use std::str::FromStr;

pub struct EdnLoader;

impl EdnLoader {
    pub fn parse(content: &str) -> Result<Graph> {
        let edn =
            Edn::from_str(content).map_err(|e| anyhow::anyhow!("Failed to parse EDN: {}", e))?;

        if let Edn::Vector(vec) = edn {
            let mut nodes = HashMap::new();
            let mut edges = Vec::new();

            let elements = vec.to_vec();
            let mut i = 0;
            let mut node_ids_in_order: Vec<String> = Vec::new();

            // First pass: extract all nodes
            while i < elements.len() {
                let item = &elements[i];
                let node = match item {
                    Edn::Symbol(s) => {
                        let id = s.to_string();
                        Some(Node {
                            id: id.clone(),
                            label: Some(id.clone()),
                            prompt: None,
                            shape: None,
                            node_type: None,
                            attributes: HashMap::new(),
                        })
                    }
                    Edn::List(l) => {
                        let list = l.clone().to_vec();
                        let id_sym = list.first().context("Empty list node")?;
                        let id = id_sym.to_string();

                        let mut attrs = HashMap::new();
                        if let Some(Edn::Map(m)) = list.get(1) {
                            for (k, v) in m.clone().to_map().iter() {
                                let key = k.to_string().trim_matches(':').to_string();
                                let val = v.to_string().trim_matches('"').to_string();
                                attrs.insert(key, val);
                            }
                        }

                        Some(Node {
                            id: id.clone(),
                            label: attrs.get("label").cloned().or(Some(id.clone())),
                            prompt: attrs.get("prompt").cloned(),
                            shape: attrs.get("shape").cloned(),
                            node_type: attrs.get("type").cloned(),
                            attributes: attrs,
                        })
                    }
                    Edn::Map(_) => None, // Handled in second pass
                    _ => None,
                };

                if let Some(n) = node {
                    let id = n.id.clone();
                    nodes.insert(id.clone(), n);
                    node_ids_in_order.push(id);
                }
                i += 1;
            }

            // Second pass: extract edges
            i = 0;
            let mut current_node_idx = 0;
            while i < elements.len() {
                let item = &elements[i];

                // If it's a node (Symbol or List)
                if matches!(item, Edn::Symbol(_) | Edn::List(_)) {
                    if current_node_idx >= node_ids_in_order.len() {
                        break;
                    }
                    let current_id = &node_ids_in_order[current_node_idx];
                    let next_node_id = node_ids_in_order.get(current_node_idx + 1);

                    // Peek for routing map
                    if let Some(Edn::Map(m)) = elements.get(i + 1) {
                        let mut has_next_override = false;
                        for (k, v) in m.clone().to_map().iter() {
                            let cond = k.to_string().trim_matches(':').to_string();
                            let target = v.to_string().trim_matches(':').to_string();

                            let final_target = if target == "next" {
                                has_next_override = true;
                                next_node_id
                                    .context("Used :next in map but no next node exists")?
                                    .clone()
                            } else {
                                target
                            };

                            edges.push(Edge {
                                from: current_id.clone(),
                                to: final_target,
                                label: Some(cond.clone()),
                                condition: Some(cond),
                                weight: None,
                                attributes: HashMap::new(),
                            });
                        }

                        if !has_next_override && next_node_id.is_some() {
                            // Add default implicit edge if not overridden
                            edges.push(Edge {
                                from: current_id.clone(),
                                to: next_node_id.unwrap().clone(),
                                label: None,
                                condition: None,
                                weight: None,
                                attributes: HashMap::new(),
                            });
                        }
                        i += 1; // Skip the map
                    } else if let Some(next_id) = next_node_id {
                        // Implicit edge
                        edges.push(Edge {
                            from: current_id.clone(),
                            to: next_id.clone(),
                            label: None,
                            condition: None,
                            weight: None,
                            attributes: HashMap::new(),
                        });
                    }
                    current_node_idx += 1;
                }
                i += 1;
            }

            Ok(Graph {
                id: None,
                goal: None,
                nodes,
                edges,
                attributes: HashMap::new(),
            })
        } else {
            Err(anyhow::anyhow!("Root element must be a vector"))
        }
    }
}
