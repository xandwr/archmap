use crate::analysis::DependencyGraph;
use crate::config::Config;
use crate::model::{Issue, glob_match};

pub fn detect_high_coupling(graph: &DependencyGraph, config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    for (path, _idx) in graph.node_indices() {
        let fan_in = graph.fan_in(path);

        if fan_in >= config.thresholds.coupling_fanin {
            // Check if this module is expected to have high coupling
            let path_str = path.to_string_lossy();
            let is_expected = config
                .expected_high_coupling
                .iter()
                .any(|pattern| glob_match(pattern, &path_str));

            if !is_expected {
                issues.push(Issue::high_coupling(path.clone(), fan_in));
            }
        }
    }

    issues
}
