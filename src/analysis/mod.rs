mod boundary;
mod circular;
mod coupling;
mod god_object;
mod graph;

pub use boundary::detect_boundary_violations;
pub use circular::detect_circular_dependencies;
pub use coupling::detect_high_coupling;
pub use god_object::detect_god_objects;
pub use graph::DependencyGraph;

use crate::config::Config;
use crate::model::{AnalysisResult, Module};
use crate::parser::ParserRegistry;
use ignore::WalkBuilder;
use std::path::Path;

pub fn analyze(path: &Path, config: &Config, registry: &ParserRegistry) -> AnalysisResult {
    let project_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    // Discover and parse all modules
    let modules = discover_modules(path, registry);

    // Build dependency graph
    let dep_graph = DependencyGraph::build(&modules);

    // Run all analyses
    let mut issues = Vec::new();

    // Circular dependencies
    issues.extend(detect_circular_dependencies(&dep_graph));

    // God objects
    issues.extend(detect_god_objects(&modules, config));

    // High coupling
    issues.extend(detect_high_coupling(&dep_graph, config));

    // Boundary violations
    issues.extend(detect_boundary_violations(&modules, config));

    AnalysisResult {
        project_name,
        modules,
        issues,
        dependency_graph: dep_graph.into_inner(),
    }
}

fn discover_modules(path: &Path, registry: &ParserRegistry) -> Vec<Module> {
    let mut modules = Vec::new();

    let walker = WalkBuilder::new(path).hidden(true).git_ignore(true).build();

    for entry in walker.flatten() {
        let file_path = entry.path();

        if !file_path.is_file() {
            continue;
        }

        if let Some(parser) = registry.find_parser(file_path) {
            if let Ok(source) = std::fs::read_to_string(file_path) {
                match parser.parse_module(file_path, &source) {
                    Ok(module) => modules.push(module),
                    Err(e) => {
                        eprintln!("Warning: Failed to parse {}: {}", file_path.display(), e);
                    }
                }
            }
        }
    }

    modules
}
