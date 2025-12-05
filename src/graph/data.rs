use crate::model::{AnalysisResult, IssueKind, Module};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Graph data in D3.js force-directed graph format
#[derive(Debug, Clone, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
    pub metadata: GraphMetadata,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub lines: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub issue_count: usize,
    pub category: String,
    pub exports: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub is_cycle: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphMetadata {
    pub project_name: String,
    pub total_modules: usize,
    pub total_dependencies: usize,
    pub total_issues: usize,
    pub cycle_count: usize,
}

impl GraphData {
    pub fn from_analysis(result: &AnalysisResult, project_root: &Path) -> Self {
        // Build fan-in counts
        let mut fan_ins: HashMap<String, usize> = HashMap::new();
        for module in &result.modules {
            let path = relative_path(&module.path, project_root);
            for import in &module.imports {
                // Try to resolve import to a module path
                if let Some(target) = resolve_import(import, &result.modules, project_root) {
                    *fan_ins.entry(target).or_insert(0) += 1;
                }
            }
            fan_ins.entry(path).or_insert(0);
        }

        // Build issue counts per module
        let mut issue_counts: HashMap<String, usize> = HashMap::new();
        for issue in &result.issues {
            for loc in &issue.locations {
                let path = relative_path(&loc.path, project_root);
                *issue_counts.entry(path).or_insert(0) += 1;
            }
        }

        // Build nodes
        let nodes: Vec<GraphNode> = result
            .modules
            .iter()
            .map(|m| {
                let path = relative_path(&m.path, project_root);
                let fan_in = fan_ins.get(&path).copied().unwrap_or(0);
                let fan_out = m.imports.len();
                let issue_count = issue_counts.get(&path).copied().unwrap_or(0);
                let category = categorize_module(&m.path, project_root);

                GraphNode {
                    id: path.clone(),
                    name: m.name.clone(),
                    path,
                    lines: m.lines,
                    fan_in,
                    fan_out,
                    issue_count,
                    category,
                    exports: m.exports.clone(),
                }
            })
            .collect();

        // Build links
        let mut links: Vec<GraphLink> = Vec::new();
        let mut cycle_edges: HashSet<(String, String)> = HashSet::new();

        // Identify cycle edges from CircularDependency issues
        for issue in &result.issues {
            if matches!(issue.kind, IssueKind::CircularDependency) {
                let paths: Vec<_> = issue
                    .locations
                    .iter()
                    .map(|loc| relative_path(&loc.path, project_root))
                    .collect();
                for i in 0..paths.len() {
                    let from = paths[i].clone();
                    let to = paths[(i + 1) % paths.len()].clone();
                    cycle_edges.insert((from, to));
                }
            }
        }

        // Build dependency links
        for module in &result.modules {
            let source = relative_path(&module.path, project_root);
            for import in &module.imports {
                if let Some(target) = resolve_import(import, &result.modules, project_root) {
                    let is_cycle = cycle_edges.contains(&(source.clone(), target.clone()));
                    links.push(GraphLink {
                        source: source.clone(),
                        target,
                        is_cycle,
                    });
                }
            }
        }

        // Metadata
        let metadata = GraphMetadata {
            project_name: result.project_name.clone(),
            total_modules: result.modules.len(),
            total_dependencies: links.len(),
            total_issues: result.issues.len(),
            cycle_count: result
                .issues
                .iter()
                .filter(|i| matches!(i.kind, IssueKind::CircularDependency))
                .count(),
        };

        GraphData {
            nodes,
            links,
            metadata,
        }
    }
}

fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn resolve_import(import: &str, modules: &[Module], project_root: &Path) -> Option<String> {
    // Extract the first meaningful path segment
    let segments: Vec<&str> = import.split("::").collect();
    if segments.is_empty() {
        return None;
    }

    let search_name = if segments[0] == "crate" && segments.len() > 1 {
        segments[1]
    } else if segments[0] == "super" || segments[0] == "self" {
        return None;
    } else {
        segments[0]
    };

    modules
        .iter()
        .find(|m| m.name == search_name)
        .map(|m| relative_path(&m.path, project_root))
}

fn categorize_module(path: &Path, project_root: &Path) -> String {
    let rel_path = path.strip_prefix(project_root).unwrap_or(path);
    let path_str = rel_path.display().to_string();

    if path_str.contains("test") {
        "test".to_string()
    } else if path_str.contains("mod.rs") || path_str.contains("lib.rs") {
        "index".to_string()
    } else if path_str.contains("main.rs") {
        "entry".to_string()
    } else if path_str.contains("config") {
        "config".to_string()
    } else if path_str.contains("model") || path_str.contains("types") {
        "model".to_string()
    } else if path_str.contains("cli") || path_str.contains("args") {
        "cli".to_string()
    } else if path_str.contains("output") || path_str.contains("format") {
        "output".to_string()
    } else if path_str.contains("parser") || path_str.contains("parse") {
        "parser".to_string()
    } else if path_str.contains("analysis") || path_str.contains("check") {
        "analysis".to_string()
    } else {
        "module".to_string()
    }
}
