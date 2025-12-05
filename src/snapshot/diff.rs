use super::serialize::{IssueSnapshot, ModuleSnapshot, Snapshot};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    pub baseline_created_at: String,
    pub current_created_at: String,

    /// Modules added since baseline
    pub added_modules: Vec<String>,

    /// Modules removed since baseline
    pub removed_modules: Vec<String>,

    /// Modules with changed content (same path, different hash)
    pub modified_modules: Vec<ModuleChange>,

    /// Dependencies added
    pub added_dependencies: Vec<(String, String)>,

    /// Dependencies removed
    pub removed_dependencies: Vec<(String, String)>,

    /// New issues (by issue_id)
    pub new_issues: Vec<IssueSnapshot>,

    /// Resolved issues (present in baseline, absent in current)
    pub resolved_issues: Vec<IssueSnapshot>,

    /// Metric deltas
    pub metric_changes: MetricChanges,
}

#[derive(Debug, Clone)]
pub struct ModuleChange {
    pub path: String,
    pub old_lines: usize,
    pub new_lines: usize,
    pub imports_added: Vec<String>,
    pub imports_removed: Vec<String>,
    pub exports_added: Vec<String>,
    pub exports_removed: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MetricChanges {
    pub module_count_delta: i64,
    pub line_count_delta: i64,
    pub dependency_count_delta: i64,
    pub cycle_count_delta: i64,
    pub coupling_delta: f64,
    pub new_issue_count: usize,
    pub resolved_issue_count: usize,
}

pub fn compute_diff(baseline: &Snapshot, current: &Snapshot) -> SnapshotDiff {
    // Module comparison
    let baseline_paths: HashSet<&str> = baseline.modules.iter().map(|m| m.path.as_str()).collect();
    let current_paths: HashSet<&str> = current.modules.iter().map(|m| m.path.as_str()).collect();

    let added_modules: Vec<String> = current_paths
        .difference(&baseline_paths)
        .map(|s| s.to_string())
        .collect();

    let removed_modules: Vec<String> = baseline_paths
        .difference(&current_paths)
        .map(|s| s.to_string())
        .collect();

    // Modified modules (same path, different content hash)
    let baseline_map: HashMap<&str, &ModuleSnapshot> = baseline
        .modules
        .iter()
        .map(|m| (m.path.as_str(), m))
        .collect();
    let current_map: HashMap<&str, &ModuleSnapshot> = current
        .modules
        .iter()
        .map(|m| (m.path.as_str(), m))
        .collect();

    let modified_modules: Vec<ModuleChange> = baseline_paths
        .intersection(&current_paths)
        .filter_map(|path| {
            let base = baseline_map.get(path)?;
            let curr = current_map.get(path)?;

            if base.content_hash != curr.content_hash {
                let base_imports: HashSet<&String> = base.imports.iter().collect();
                let curr_imports: HashSet<&String> = curr.imports.iter().collect();
                let base_exports: HashSet<&String> = base.exports.iter().collect();
                let curr_exports: HashSet<&String> = curr.exports.iter().collect();

                Some(ModuleChange {
                    path: path.to_string(),
                    old_lines: base.lines,
                    new_lines: curr.lines,
                    imports_added: curr_imports
                        .difference(&base_imports)
                        .map(|s| (*s).clone())
                        .collect(),
                    imports_removed: base_imports
                        .difference(&curr_imports)
                        .map(|s| (*s).clone())
                        .collect(),
                    exports_added: curr_exports
                        .difference(&base_exports)
                        .map(|s| (*s).clone())
                        .collect(),
                    exports_removed: base_exports
                        .difference(&curr_exports)
                        .map(|s| (*s).clone())
                        .collect(),
                })
            } else {
                None
            }
        })
        .collect();

    // Dependency changes
    let baseline_deps: HashSet<(String, String)> = flatten_dependencies(&baseline.dependencies);
    let current_deps: HashSet<(String, String)> = flatten_dependencies(&current.dependencies);

    let added_dependencies: Vec<(String, String)> =
        current_deps.difference(&baseline_deps).cloned().collect();
    let removed_dependencies: Vec<(String, String)> =
        baseline_deps.difference(&current_deps).cloned().collect();

    // Issue changes
    let baseline_issue_ids: HashSet<&str> = baseline
        .issues
        .iter()
        .map(|i| i.issue_id.as_str())
        .collect();
    let current_issue_ids: HashSet<&str> =
        current.issues.iter().map(|i| i.issue_id.as_str()).collect();

    let new_issues: Vec<IssueSnapshot> = current
        .issues
        .iter()
        .filter(|i| !baseline_issue_ids.contains(i.issue_id.as_str()))
        .cloned()
        .collect();

    let resolved_issues: Vec<IssueSnapshot> = baseline
        .issues
        .iter()
        .filter(|i| !current_issue_ids.contains(i.issue_id.as_str()))
        .cloned()
        .collect();

    // Metric changes
    let metric_changes = MetricChanges {
        module_count_delta: current.metrics.total_modules as i64
            - baseline.metrics.total_modules as i64,
        line_count_delta: current.metrics.total_lines as i64 - baseline.metrics.total_lines as i64,
        dependency_count_delta: current.metrics.total_dependencies as i64
            - baseline.metrics.total_dependencies as i64,
        cycle_count_delta: current.metrics.cycle_count as i64 - baseline.metrics.cycle_count as i64,
        coupling_delta: current.metrics.avg_coupling - baseline.metrics.avg_coupling,
        new_issue_count: new_issues.len(),
        resolved_issue_count: resolved_issues.len(),
    };

    SnapshotDiff {
        baseline_created_at: baseline.created_at.clone(),
        current_created_at: current.created_at.clone(),
        added_modules,
        removed_modules,
        modified_modules,
        added_dependencies,
        removed_dependencies,
        new_issues,
        resolved_issues,
        metric_changes,
    }
}

fn flatten_dependencies(deps: &HashMap<String, Vec<String>>) -> HashSet<(String, String)> {
    deps.iter()
        .flat_map(|(from, tos)| tos.iter().map(move |to| (from.clone(), to.clone())))
        .collect()
}

/// Format diff as markdown
pub fn format_diff_markdown(diff: &SnapshotDiff) -> String {
    let mut output = String::new();

    output.push_str("# Architectural Diff\n\n");

    output.push_str(&format!(
        "**Baseline**: {} | **Current**: {}\n\n",
        diff.baseline_created_at, diff.current_created_at
    ));

    // Summary
    output.push_str("## Summary\n\n");

    let metrics = &diff.metric_changes;
    output.push_str(&format!(
        "- **Modules**: {} ({})\n",
        format_delta(metrics.module_count_delta),
        format!(
            "+{} added, -{} removed, {} modified",
            diff.added_modules.len(),
            diff.removed_modules.len(),
            diff.modified_modules.len()
        )
    ));
    output.push_str(&format!(
        "- **Lines**: {}\n",
        format_delta(metrics.line_count_delta)
    ));
    output.push_str(&format!(
        "- **Dependencies**: {} (+{} / -{})\n",
        format_delta(metrics.dependency_count_delta),
        diff.added_dependencies.len(),
        diff.removed_dependencies.len()
    ));
    output.push_str(&format!(
        "- **Cycles**: {}\n",
        format_delta(metrics.cycle_count_delta)
    ));
    output.push_str(&format!(
        "- **Avg Coupling**: {:+.2}\n\n",
        metrics.coupling_delta
    ));

    // New Issues
    if !diff.new_issues.is_empty() {
        output.push_str(&format!("## New Issues ({})\n\n", diff.new_issues.len()));
        for issue in &diff.new_issues {
            output.push_str(&format!(
                "- **{}** [{}]: {}\n",
                issue.kind, issue.severity, issue.message
            ));
            for loc in &issue.locations {
                output.push_str(&format!("  - `{}`\n", loc));
            }
        }
        output.push('\n');
    }

    // Resolved Issues
    if !diff.resolved_issues.is_empty() {
        output.push_str(&format!(
            "## Resolved Issues ({})\n\n",
            diff.resolved_issues.len()
        ));
        for issue in &diff.resolved_issues {
            output.push_str(&format!("- ~~**{}**: {}~~\n", issue.kind, issue.message));
        }
        output.push('\n');
    }

    // Added Modules
    if !diff.added_modules.is_empty() {
        output.push_str(&format!(
            "## Added Modules ({})\n\n",
            diff.added_modules.len()
        ));
        for module in &diff.added_modules {
            output.push_str(&format!("- `{}`\n", module));
        }
        output.push('\n');
    }

    // Removed Modules
    if !diff.removed_modules.is_empty() {
        output.push_str(&format!(
            "## Removed Modules ({})\n\n",
            diff.removed_modules.len()
        ));
        for module in &diff.removed_modules {
            output.push_str(&format!("- `{}`\n", module));
        }
        output.push('\n');
    }

    // Modified Modules
    if !diff.modified_modules.is_empty() {
        output.push_str(&format!(
            "## Modified Modules ({})\n\n",
            diff.modified_modules.len()
        ));
        for module in &diff.modified_modules {
            let line_delta = module.new_lines as i64 - module.old_lines as i64;
            output.push_str(&format!(
                "### `{}` ({} lines)\n",
                module.path,
                format_delta(line_delta)
            ));
            if !module.imports_added.is_empty() {
                output.push_str(&format!(
                    "- Imports added: {}\n",
                    module.imports_added.join(", ")
                ));
            }
            if !module.imports_removed.is_empty() {
                output.push_str(&format!(
                    "- Imports removed: {}\n",
                    module.imports_removed.join(", ")
                ));
            }
            if !module.exports_added.is_empty() {
                output.push_str(&format!(
                    "- Exports added: {}\n",
                    module.exports_added.join(", ")
                ));
            }
            if !module.exports_removed.is_empty() {
                output.push_str(&format!(
                    "- Exports removed: {}\n",
                    module.exports_removed.join(", ")
                ));
            }
            output.push('\n');
        }
    }

    output
}

/// Format diff as JSON
pub fn format_diff_json(diff: &SnapshotDiff) -> String {
    use serde_json::json;

    let output = json!({
        "baseline_created_at": diff.baseline_created_at,
        "current_created_at": diff.current_created_at,
        "summary": {
            "module_count_delta": diff.metric_changes.module_count_delta,
            "line_count_delta": diff.metric_changes.line_count_delta,
            "dependency_count_delta": diff.metric_changes.dependency_count_delta,
            "cycle_count_delta": diff.metric_changes.cycle_count_delta,
            "coupling_delta": diff.metric_changes.coupling_delta,
            "new_issue_count": diff.metric_changes.new_issue_count,
            "resolved_issue_count": diff.metric_changes.resolved_issue_count
        },
        "added_modules": diff.added_modules,
        "removed_modules": diff.removed_modules,
        "modified_modules": diff.modified_modules.iter().map(|m| {
            json!({
                "path": m.path,
                "old_lines": m.old_lines,
                "new_lines": m.new_lines,
                "imports_added": m.imports_added,
                "imports_removed": m.imports_removed,
                "exports_added": m.exports_added,
                "exports_removed": m.exports_removed
            })
        }).collect::<Vec<_>>(),
        "added_dependencies": diff.added_dependencies.iter().map(|(from, to)| {
            json!({"from": from, "to": to})
        }).collect::<Vec<_>>(),
        "removed_dependencies": diff.removed_dependencies.iter().map(|(from, to)| {
            json!({"from": from, "to": to})
        }).collect::<Vec<_>>(),
        "new_issues": diff.new_issues.iter().map(|i| {
            json!({
                "kind": i.kind,
                "severity": i.severity,
                "message": i.message,
                "locations": i.locations
            })
        }).collect::<Vec<_>>(),
        "resolved_issues": diff.resolved_issues.iter().map(|i| {
            json!({
                "kind": i.kind,
                "severity": i.severity,
                "message": i.message,
                "locations": i.locations
            })
        }).collect::<Vec<_>>()
    });

    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

fn format_delta(delta: i64) -> String {
    if delta > 0 {
        format!("+{}", delta)
    } else if delta < 0 {
        format!("{}", delta)
    } else {
        "0".to_string()
    }
}
