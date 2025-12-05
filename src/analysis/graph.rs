use crate::model::Module;
use petgraph::Direction;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet, VecDeque};
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
                .neighbors_directed(*idx, Direction::Incoming)
                .count()
        } else {
            0
        }
    }

    pub fn fan_out(&self, path: &PathBuf) -> usize {
        if let Some(idx) = self.node_indices.get(path) {
            self.graph
                .neighbors_directed(*idx, Direction::Outgoing)
                .count()
        } else {
            0
        }
    }

    /// Returns modules in topological order (dependencies before dependents).
    /// Returns None if the graph has cycles.
    pub fn topological_order(&self) -> Option<Vec<PathBuf>> {
        toposort(&self.graph, None).ok().map(|indices| {
            indices
                .into_iter()
                .map(|idx| self.graph[idx].clone())
                .collect()
        })
    }

    /// Returns modules in topological order, handling cycles by breaking them.
    /// Cycle members are placed at their earliest valid position.
    pub fn topological_order_with_cycles(&self) -> Vec<PathBuf> {
        match self.topological_order() {
            Some(order) => order,
            None => self.kahn_with_cycle_handling(),
        }
    }

    /// Kahn's algorithm variant that handles cycles
    fn kahn_with_cycle_handling(&self) -> Vec<PathBuf> {
        let mut in_degree: HashMap<NodeIndex, usize> = HashMap::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        // Calculate in-degrees
        for idx in self.graph.node_indices() {
            let degree = self
                .graph
                .neighbors_directed(idx, Direction::Incoming)
                .count();
            in_degree.insert(idx, degree);
            if degree == 0 {
                queue.push_back(idx);
            }
        }

        // Process nodes with in-degree 0
        while let Some(idx) = queue.pop_front() {
            if visited.contains(&idx) {
                continue;
            }
            visited.insert(idx);
            result.push(self.graph[idx].clone());

            for neighbor in self.graph.neighbors_directed(idx, Direction::Outgoing) {
                if let Some(degree) = in_degree.get_mut(&neighbor) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 && !visited.contains(&neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        // Add remaining nodes (part of cycles)
        for idx in self.graph.node_indices() {
            if !visited.contains(&idx) {
                result.push(self.graph[idx].clone());
            }
        }

        result
    }

    /// Get all direct dependents (modules that import this module)
    pub fn direct_dependents(&self, path: &PathBuf) -> Vec<PathBuf> {
        if let Some(idx) = self.node_indices.get(path) {
            self.graph
                .neighbors_directed(*idx, Direction::Incoming)
                .filter_map(|idx| self.graph.node_weight(idx).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if a path exists in the graph
    pub fn contains(&self, path: &PathBuf) -> bool {
        self.node_indices.contains_key(path)
    }

    /// Get importance score for a module (higher = more important for context)
    /// Prioritizes modules with high fan-in (many dependents)
    pub fn importance_score(&self, path: &PathBuf, modules: &[Module]) -> f64 {
        let fan_in = self.fan_in(path) as f64;
        let fan_out = self.fan_out(path) as f64;

        // Find the module for additional scoring
        let module = modules.iter().find(|m| &m.path == path);

        // Bonus for model/types modules (core data structures)
        let model_bonus = if let Some(m) = module {
            if m.name.contains("model")
                || m.name.contains("types")
                || m.name.contains("schema")
                || m.name.contains("entity")
            {
                10.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Bonus for having struct/enum definitions
        let data_structure_bonus = if let Some(m) = module {
            m.definitions
                .iter()
                .filter(|d| {
                    matches!(
                        d.kind,
                        crate::model::DefinitionKind::Struct | crate::model::DefinitionKind::Enum
                    )
                })
                .count() as f64
                * 1.5
        } else {
            0.0
        };

        // Fan-in weighted more heavily (dependents matter more)
        fan_in * 2.0 + fan_out + model_bonus + data_structure_bonus
    }
}

fn resolve_import(import: &str, modules: &[Module]) -> Option<PathBuf> {
    // Extract the path segments (e.g., "crate::model::Module" -> ["crate", "model", "Module"])
    let segments: Vec<&str> = import.split("::").collect();

    if segments.is_empty() {
        return None;
    }

    // Handle crate:: prefix - get the module path segments after "crate"
    let module_segments = if segments[0] == "crate" && segments.len() > 1 {
        &segments[1..]
    } else if segments[0] == "super" || segments[0] == "self" {
        // Skip relative imports for now
        return None;
    } else {
        // External crate import - skip
        return None;
    };

    if module_segments.is_empty() {
        return None;
    }

    // Try to find a matching module by path components
    // For "crate::model::Module", we want to match "src/model/mod.rs" or "src/model.rs"
    let first_segment = module_segments[0].to_lowercase();

    // Find modules whose path contains this segment as a directory or file name
    modules
        .iter()
        .find(|m| {
            let path_str = m.path.to_string_lossy().to_lowercase();

            // Check for exact module name match (e.g., "model" matches "src/model/mod.rs" or "src/model.rs")
            // The module name in the path should be a directory containing mod.rs or a .rs file
            let is_mod_file = path_str.ends_with(&format!("/{}/mod.rs", first_segment))
                || path_str.ends_with(&format!("\\{}\\mod.rs", first_segment));
            let is_direct_file = path_str.ends_with(&format!("/{}.rs", first_segment))
                || path_str.ends_with(&format!("\\{}.rs", first_segment));

            // Also check for submodule files like "src/model/issue.rs" when importing "crate::model::issue"
            let is_submodule = if module_segments.len() > 1 {
                let second_segment = module_segments[1].to_lowercase();
                path_str.ends_with(&format!("/{}/{}.rs", first_segment, second_segment))
                    || path_str.ends_with(&format!("\\{}\\{}.rs", first_segment, second_segment))
            } else {
                false
            };

            is_mod_file || is_direct_file || is_submodule
        })
        .map(|m| m.path.clone())
}
