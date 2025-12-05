use crate::model::{AnalysisResult, IssueKind, Module};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Complete architectural snapshot for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Snapshot version for forward compatibility
    pub version: u32,
    /// Timestamp when snapshot was created
    pub created_at: String,
    /// Project name from analysis
    pub project_name: String,
    /// All modules with their metadata
    pub modules: Vec<ModuleSnapshot>,
    /// All detected issues
    pub issues: Vec<IssueSnapshot>,
    /// Dependency graph as adjacency list (source -> [targets])
    pub dependencies: HashMap<String, Vec<String>>,
    /// Computed metrics for comparison
    pub metrics: SnapshotMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSnapshot {
    pub path: String,
    pub name: String,
    pub lines: usize,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    /// Hash of file content for detecting changes
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSnapshot {
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub locations: Vec<String>,
    /// Unique identifier for issue (hash of kind + locations)
    pub issue_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SnapshotMetrics {
    pub total_modules: usize,
    pub total_lines: usize,
    pub total_dependencies: usize,
    pub cycle_count: usize,
    pub avg_coupling: f64,
    pub max_coupling: usize,
    pub issue_counts: HashMap<String, usize>,
}

impl Snapshot {
    pub fn from_analysis(result: &AnalysisResult, project_root: &Path) -> Self {
        let created_at = chrono_lite_now();

        // Convert modules
        let modules: Vec<ModuleSnapshot> = result
            .modules
            .iter()
            .map(|m| {
                let relative_path = m
                    .path
                    .strip_prefix(project_root)
                    .unwrap_or(&m.path)
                    .display()
                    .to_string();

                let content_hash = compute_file_hash(&m.path);

                ModuleSnapshot {
                    path: relative_path,
                    name: m.name.clone(),
                    lines: m.lines,
                    imports: m.imports.clone(),
                    exports: m.exports.clone(),
                    content_hash,
                }
            })
            .collect();

        // Convert issues with stable IDs
        let issues: Vec<IssueSnapshot> = result
            .issues
            .iter()
            .map(|i| {
                let locations: Vec<String> = i
                    .locations
                    .iter()
                    .map(|l| {
                        l.path
                            .strip_prefix(project_root)
                            .unwrap_or(&l.path)
                            .display()
                            .to_string()
                    })
                    .collect();

                let issue_id = compute_issue_id(&i.kind, &locations);
                let kind_str = format!("{:?}", i.kind);

                IssueSnapshot {
                    kind: kind_str,
                    severity: i.severity.to_string(),
                    message: i.message.clone(),
                    locations,
                    issue_id,
                }
            })
            .collect();

        // Build dependency adjacency list
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
        for module in &result.modules {
            let from_path = module
                .path
                .strip_prefix(project_root)
                .unwrap_or(&module.path)
                .display()
                .to_string();

            // Find resolved dependencies
            let deps: Vec<String> = module
                .imports
                .iter()
                .filter_map(|imp| resolve_to_module(imp, &result.modules, project_root))
                .collect();

            dependencies.insert(from_path, deps);
        }

        // Compute metrics
        let metrics = compute_metrics(&modules, &issues, &dependencies);

        Self {
            version: 1,
            created_at,
            project_name: result.project_name.clone(),
            modules,
            issues,
            dependencies,
            metrics,
        }
    }
}

pub fn save_snapshot(snapshot: &Snapshot, path: &Path) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(snapshot)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)
}

pub fn load_snapshot(path: &Path) -> Result<Snapshot, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let snapshot: Snapshot = serde_json::from_str(&content)?;
    Ok(snapshot)
}

fn compute_file_hash(path: &PathBuf) -> String {
    use std::collections::hash_map::DefaultHasher;

    match std::fs::read_to_string(path) {
        Ok(content) => {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            format!("{:x}", hasher.finish())
        }
        Err(_) => String::new(),
    }
}

fn compute_issue_id(kind: &IssueKind, locations: &[String]) -> String {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    format!("{:?}", kind).hash(&mut hasher);
    for loc in locations {
        loc.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

fn resolve_to_module(import: &str, modules: &[Module], project_root: &Path) -> Option<String> {
    // Extract the first path segment
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

    modules.iter().find(|m| m.name == search_name).map(|m| {
        m.path
            .strip_prefix(project_root)
            .unwrap_or(&m.path)
            .display()
            .to_string()
    })
}

fn compute_metrics(
    modules: &[ModuleSnapshot],
    issues: &[IssueSnapshot],
    dependencies: &HashMap<String, Vec<String>>,
) -> SnapshotMetrics {
    let total_modules = modules.len();
    let total_lines: usize = modules.iter().map(|m| m.lines).sum();
    let total_dependencies: usize = dependencies.values().map(|v| v.len()).sum();

    // Count cycles from issues
    let cycle_count = issues
        .iter()
        .filter(|i| i.kind.contains("CircularDependency"))
        .count();

    // Compute coupling (fan-in for each module)
    let mut fan_ins: HashMap<&str, usize> = HashMap::new();
    for targets in dependencies.values() {
        for target in targets {
            *fan_ins.entry(target.as_str()).or_insert(0) += 1;
        }
    }
    let max_coupling = fan_ins.values().copied().max().unwrap_or(0);
    let avg_coupling = if !fan_ins.is_empty() {
        fan_ins.values().sum::<usize>() as f64 / fan_ins.len() as f64
    } else {
        0.0
    };

    // Count issues by kind
    let mut issue_counts: HashMap<String, usize> = HashMap::new();
    for issue in issues {
        // Extract the base kind (before any embedded data)
        let base_kind = issue.kind.split('(').next().unwrap_or(&issue.kind);
        *issue_counts.entry(base_kind.to_string()).or_insert(0) += 1;
    }

    SnapshotMetrics {
        total_modules,
        total_lines,
        total_dependencies,
        cycle_count,
        avg_coupling,
        max_coupling,
        issue_counts,
    }
}

/// Simple timestamp function (no chrono dependency)
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}
