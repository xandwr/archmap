use crate::model::Module;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct DependencyGraph {
    graph: DiGraph<PathBuf, ()>,
    node_indices: HashMap<PathBuf, NodeIndex>,
}

impl DependencyGraph {
    pub fn build(modules: &[Module]) -> Self {
        let mut graph = DiGraph::new();
        let mut node_indices = HashMap::new();

        // Add all modules as nodes
        for module in modules {
            let idx = graph.add_node(module.path.clone());
            node_indices.insert(module.path.clone(), idx);
        }

        // Add edges based on imports
        for module in modules {
            let from_idx = match node_indices.get(&module.path) {
                Some(idx) => *idx,
                None => continue,
            };

            for import in &module.imports {
                // Try to resolve import to a module path
                if let Some(target_path) = resolve_import(import, modules) {
                    if let Some(to_idx) = node_indices.get(&target_path) {
                        graph.add_edge(from_idx, *to_idx, ());
                    }
                }
            }
        }

        Self {
            graph,
            node_indices,
        }
    }

    pub fn graph(&self) -> &DiGraph<PathBuf, ()> {
        &self.graph
    }

    pub fn node_indices(&self) -> &HashMap<PathBuf, NodeIndex> {
        &self.node_indices
    }

    pub fn into_inner(self) -> DiGraph<PathBuf, ()> {
        self.graph
    }

    pub fn fan_in(&self, path: &PathBuf) -> usize {
        if let Some(idx) = self.node_indices.get(path) {
            self.graph
                .neighbors_directed(*idx, petgraph::Direction::Incoming)
                .count()
        } else {
            0
        }
    }

    pub fn fan_out(&self, path: &PathBuf) -> usize {
        if let Some(idx) = self.node_indices.get(path) {
            self.graph
                .neighbors_directed(*idx, petgraph::Direction::Outgoing)
                .count()
        } else {
            0
        }
    }
}

fn resolve_import(import: &str, modules: &[Module]) -> Option<PathBuf> {
    // Extract the first path segment (crate name or module name)
    let segments: Vec<&str> = import.split("::").collect();

    if segments.is_empty() {
        return None;
    }

    // Handle crate:: prefix
    let search_name = if segments[0] == "crate" && segments.len() > 1 {
        segments[1]
    } else if segments[0] == "super" || segments[0] == "self" {
        // Skip relative imports for now
        return None;
    } else {
        segments[0]
    };

    // Look for a module with matching name
    modules
        .iter()
        .find(|m| m.name == search_name)
        .map(|m| m.path.clone())
}
