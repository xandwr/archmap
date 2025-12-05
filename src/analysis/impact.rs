use crate::analysis::DependencyGraph;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImpactError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    #[error("File not in dependency graph: {0}")]
    NotInGraph(PathBuf),
}

/// Result of impact analysis for a file
#[derive(Debug)]
pub struct ImpactAnalysis {
    /// The target file being analyzed
    pub target: PathBuf,
    /// Files affected, organized by dependency depth from target
    /// affected_by_depth[0] = direct dependents (depth 1)
    /// affected_by_depth[1] = dependents of dependents (depth 2), etc.
    pub affected_by_depth: Vec<Vec<PathBuf>>,
    /// Total unique files affected
    pub total_affected: usize,
    /// Maximum chain length (depth) from target to farthest dependent
    pub max_chain_length: usize,
    /// Dependency tree for visualization
    pub tree: ImpactNode,
}

/// Node in the impact tree
#[derive(Debug, Clone)]
pub struct ImpactNode {
    pub path: PathBuf,
    pub depth: usize,
    pub children: Vec<ImpactNode>,
}

/// Compute the impact of changes to a target file
/// Returns all modules that directly or transitively depend on the target
pub fn compute_impact(
    graph: &DependencyGraph,
    target: &Path,
    max_depth: Option<usize>,
) -> Result<ImpactAnalysis, ImpactError> {
    // Check if target is in the graph
    let target_canonical = target.to_path_buf();
    if !graph.contains(&target_canonical) {
        return Err(ImpactError::NotInGraph(target_canonical));
    }

    // BFS to find all dependents
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut depth_map: HashMap<PathBuf, usize> = HashMap::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    let mut parent_map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    // Start with direct dependents at depth 1
    let direct = graph.direct_dependents(&target_canonical);
    for dep in direct {
        if !visited.contains(&dep) {
            visited.insert(dep.clone());
            depth_map.insert(dep.clone(), 1);
            parent_map.insert(dep.clone(), vec![target_canonical.clone()]);
            queue.push_back((dep, 1));
        }
    }

    // BFS traversal
    while let Some((node, depth)) = queue.pop_front() {
        // Check depth limit
        if let Some(max) = max_depth {
            if depth >= max {
                continue;
            }
        }

        // Find dependents of this node (who imports this node)
        let dependents = graph.direct_dependents(&node);
        for dep in dependents {
            if !visited.contains(&dep) {
                visited.insert(dep.clone());
                depth_map.insert(dep.clone(), depth + 1);
                parent_map.insert(dep.clone(), vec![node.clone()]);
                queue.push_back((dep.clone(), depth + 1));
            }
        }
    }

    // Organize results by depth
    let max_chain_length = depth_map.values().copied().max().unwrap_or(0);
    let mut affected_by_depth: Vec<Vec<PathBuf>> = vec![Vec::new(); max_chain_length];

    for (path, depth) in &depth_map {
        if *depth > 0 && *depth <= max_chain_length {
            affected_by_depth[depth - 1].push(path.clone());
        }
    }

    // Sort each depth level for consistent output
    for level in &mut affected_by_depth {
        level.sort();
    }

    // Build tree for visualization
    let tree = build_impact_tree(&target_canonical, graph, max_depth);

    Ok(ImpactAnalysis {
        target: target_canonical,
        affected_by_depth,
        total_affected: visited.len(),
        max_chain_length,
        tree,
    })
}

fn build_impact_tree(
    root: &PathBuf,
    graph: &DependencyGraph,
    max_depth: Option<usize>,
) -> ImpactNode {
    build_tree_recursive(root, graph, 0, max_depth, &mut HashSet::new())
}

fn build_tree_recursive(
    node: &PathBuf,
    graph: &DependencyGraph,
    depth: usize,
    max_depth: Option<usize>,
    visited: &mut HashSet<PathBuf>,
) -> ImpactNode {
    let mut children = Vec::new();

    // Check depth limit
    if max_depth.map_or(false, |max| depth >= max) {
        return ImpactNode {
            path: node.clone(),
            depth,
            children,
        };
    }

    // Add to visited to prevent cycles
    visited.insert(node.clone());

    // Get direct dependents
    for dep in graph.direct_dependents(node) {
        if !visited.contains(&dep) {
            let child = build_tree_recursive(&dep, graph, depth + 1, max_depth, visited);
            children.push(child);
        }
    }

    // Sort children for consistent output
    children.sort_by(|a, b| a.path.cmp(&b.path));

    ImpactNode {
        path: node.clone(),
        depth,
        children,
    }
}

/// Format impact analysis as markdown
pub fn format_impact_markdown(
    analysis: &ImpactAnalysis,
    project_root: Option<&Path>,
    show_tree: bool,
) -> String {
    let mut output = String::new();

    let target_path = relative_path(&analysis.target, project_root);

    output.push_str(&format!("# Change Impact Analysis: {}\n\n", target_path));

    output.push_str("## Summary\n\n");
    output.push_str(&format!(
        "- **Total Affected Files**: {}\n",
        analysis.total_affected
    ));
    output.push_str(&format!(
        "- **Maximum Chain Length**: {}\n\n",
        analysis.max_chain_length
    ));

    if analysis.total_affected == 0 {
        output.push_str("*No files depend on this module.*\n");
        return output;
    }

    output.push_str("## Affected Files by Distance\n\n");

    for (idx, files) in analysis.affected_by_depth.iter().enumerate() {
        let depth = idx + 1;
        let label = if depth == 1 {
            "Direct Dependents".to_string()
        } else {
            format!("Depth {}", depth)
        };

        output.push_str(&format!("### {} ({})\n\n", label, files.len()));

        if files.is_empty() {
            output.push_str("*(none)*\n\n");
        } else {
            for file in files {
                let path = relative_path(file, project_root);
                output.push_str(&format!("- `{}`\n", path));
            }
            output.push('\n');
        }
    }

    if show_tree {
        output.push_str("## Impact Tree\n\n");
        output.push_str("```\n");
        output.push_str(&format_tree(&analysis.tree, project_root, "", true));
        output.push_str("```\n");
    }

    output
}

/// Format impact analysis as JSON
pub fn format_impact_json(analysis: &ImpactAnalysis, project_root: Option<&Path>) -> String {
    use serde_json::json;

    let target_path = relative_path(&analysis.target, project_root);

    let by_depth: Vec<_> = analysis
        .affected_by_depth
        .iter()
        .enumerate()
        .map(|(idx, files)| {
            let paths: Vec<_> = files
                .iter()
                .map(|f| relative_path(f, project_root))
                .collect();
            json!({
                "depth": idx + 1,
                "files": paths
            })
        })
        .collect();

    let all_affected: Vec<_> = analysis
        .affected_by_depth
        .iter()
        .flatten()
        .map(|f| relative_path(f, project_root))
        .collect();

    let output = json!({
        "target": target_path,
        "summary": {
            "total_affected": analysis.total_affected,
            "max_chain_length": analysis.max_chain_length
        },
        "by_depth": by_depth,
        "all_affected": all_affected,
        "tree": format_tree_json(&analysis.tree, project_root)
    });

    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

fn format_tree_json(node: &ImpactNode, project_root: Option<&Path>) -> serde_json::Value {
    use serde_json::json;

    let path = relative_path(&node.path, project_root);
    let children: Vec<_> = node
        .children
        .iter()
        .map(|c| format_tree_json(c, project_root))
        .collect();

    json!({
        "path": path,
        "depth": node.depth,
        "children": children
    })
}

fn format_tree(
    node: &ImpactNode,
    project_root: Option<&Path>,
    prefix: &str,
    is_last: bool,
) -> String {
    let mut output = String::new();

    let path = relative_path(&node.path, project_root);

    if node.depth == 0 {
        // Root node
        output.push_str(&format!("{} (TARGET)\n", path));
    } else {
        let connector = if is_last { "└── " } else { "├── " };
        output.push_str(&format!("{}{}{}\n", prefix, connector, path));
    }

    let child_prefix = if node.depth == 0 {
        "".to_string()
    } else if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };

    for (idx, child) in node.children.iter().enumerate() {
        let is_last_child = idx == node.children.len() - 1;
        output.push_str(&format_tree(
            child,
            project_root,
            &child_prefix,
            is_last_child,
        ));
    }

    output
}

fn relative_path(path: &Path, root: Option<&Path>) -> String {
    if let Some(r) = root {
        path.strip_prefix(r).unwrap_or(path).display().to_string()
    } else {
        path.display().to_string()
    }
}
