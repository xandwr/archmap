use crate::analysis::DependencyGraph;
use crate::model::Issue;
use petgraph::algo::tarjan_scc;

pub fn detect_circular_dependencies(graph: &DependencyGraph) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Find strongly connected components
    let sccs = tarjan_scc(graph.graph());

    for scc in sccs {
        // A cycle exists if SCC has more than one node, or a single node with self-loop
        if scc.len() > 1 {
            let cycle: Vec<_> = scc
                .iter()
                .filter_map(|idx| graph.graph().node_weight(*idx).cloned())
                .collect();

            if !cycle.is_empty() {
                issues.push(Issue::circular_dependency(cycle));
            }
        } else if scc.len() == 1 {
            // Check for self-loop
            let idx = scc[0];
            if graph
                .graph()
                .neighbors_directed(idx, petgraph::Direction::Outgoing)
                .any(|n| n == idx)
            {
                if let Some(path) = graph.graph().node_weight(idx) {
                    issues.push(Issue::circular_dependency(vec![path.clone()]));
                }
            }
        }
    }

    issues
}
