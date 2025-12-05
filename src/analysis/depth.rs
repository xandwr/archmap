use crate::analysis::DependencyGraph;
use crate::config::Config;
use crate::model::Issue;
use petgraph::graph::NodeIndex;
use std::collections::hash_map::RandomState;

/// Detect deeply nested import chains (A → B → C → D → E).
/// Long dependency chains often indicate missing abstraction layers.
pub fn detect_deep_dependency_chains(graph: &DependencyGraph, config: &Config) -> Vec<Issue> {
    use petgraph::algo::all_simple_paths;
    use std::collections::HashSet;

    let mut issues = Vec::new();
    let max_depth = config.thresholds.max_dependency_depth;
    let pg = graph.graph();
    let indices = graph.node_indices();

    // Track chains we've already reported to avoid duplicates
    let mut reported_chains: HashSet<Vec<String>> = HashSet::new();

    // For each node, find all paths to other nodes
    for (_start_path, &start_idx) in indices {
        for (_end_path, &end_idx) in indices {
            if start_idx == end_idx {
                continue;
            }

            // Find all simple paths between these nodes
            let paths: Vec<Vec<NodeIndex>> = all_simple_paths::<Vec<NodeIndex>, _, RandomState>(
                pg,
                start_idx,
                end_idx,
                0,
                Some(max_depth + 2),
            )
            .collect();

            for path in paths {
                // Only flag chains that exceed the threshold
                if path.len() > max_depth {
                    // Create a normalized key for deduplication
                    let chain_key: Vec<String> = path
                        .iter()
                        .map(|&idx| pg[idx].display().to_string())
                        .collect();

                    if reported_chains.contains(&chain_key) {
                        continue;
                    }
                    reported_chains.insert(chain_key);

                    let chain_paths: Vec<_> = path.iter().map(|&idx| pg[idx].clone()).collect();

                    issues.push(Issue::deep_dependency_chain(chain_paths, max_depth));
                }
            }
        }
    }

    // Sort by chain length (longest first) and limit to top 10 to avoid noise
    issues.sort_by(|a, b| b.locations.len().cmp(&a.locations.len()));
    issues.truncate(10);

    issues
}
