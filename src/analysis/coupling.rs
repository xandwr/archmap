use crate::analysis::DependencyGraph;
use crate::config::Config;
use crate::model::Issue;

pub fn detect_high_coupling(graph: &DependencyGraph, config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    for (path, _idx) in graph.node_indices() {
        let fan_in = graph.fan_in(path);

        if fan_in >= config.thresholds.coupling_fanin {
            issues.push(Issue::high_coupling(path.clone(), fan_in));
        }
    }

    issues
}
