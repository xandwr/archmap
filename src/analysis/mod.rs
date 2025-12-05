mod boundary;
mod circular;
mod cohesion;
mod coupling;
mod depth;
mod god_object;
mod graph;
mod impact;

pub use boundary::{detect_boundary_violations, detect_boundary_violations_with_fs};
pub use circular::detect_circular_dependencies;
pub use cohesion::detect_low_cohesion;
pub use coupling::detect_high_coupling;
pub use depth::detect_deep_dependency_chains;
pub use god_object::detect_god_objects;
pub use graph::DependencyGraph;
pub use impact::{
    ImpactAnalysis, ImpactError, compute_impact, format_impact_json, format_impact_markdown,
};

use crate::config::Config;
use crate::fs::{FileSystem, default_fs};
use crate::model::{AnalysisResult, Module};
use crate::parser::ParserRegistry;
use crate::style;
use ignore::{WalkBuilder, WalkState};
use std::path::Path;
use std::sync::Mutex;

pub fn analyze(
    path: &Path,
    config: &Config,
    registry: &ParserRegistry,
    exclude: &[String],
) -> AnalysisResult {
    analyze_with_fs(path, config, registry, exclude, default_fs())
}

pub fn analyze_with_fs(
    path: &Path,
    config: &Config,
    registry: &ParserRegistry,
    exclude: &[String],
    fs: &dyn FileSystem,
) -> AnalysisResult {
    let project_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    // Discover and parse all modules
    let modules = discover_modules(path, registry, exclude, fs);

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
    issues.extend(detect_boundary_violations_with_fs(&modules, config, fs));

    // Deep dependency chains
    issues.extend(detect_deep_dependency_chains(&dep_graph, config));

    // Low cohesion modules
    issues.extend(detect_low_cohesion(&modules, &dep_graph, config));

    AnalysisResult {
        project_name,
        modules,
        issues,
        dependency_graph: dep_graph.into_inner(),
    }
}

fn discover_modules(
    path: &Path,
    registry: &ParserRegistry,
    exclude: &[String],
    fs: &dyn FileSystem,
) -> Vec<Module> {
    let modules = Mutex::new(Vec::new());
    let exclude: Vec<String> = exclude.to_vec();

    // Use parallel walker from ignore crate - much faster than sequential + rayon
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(true)
        .git_ignore(true)
        .threads(num_cpus())
        .filter_entry(move |entry| {
            // Check if this entry matches any exclusion pattern
            let path = entry.path();
            for pattern in &exclude {
                if path.ends_with(pattern)
                    || path.to_string_lossy().contains(&format!("/{}/", pattern))
                {
                    return false;
                }
            }
            true
        });

    let walker = builder.build_parallel();

    walker.run(|| {
        Box::new(|entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };

            let file_path = entry.path();

            // Skip non-files
            if !file_path.is_file() {
                return WalkState::Continue;
            }

            // Find parser for this file type
            let parser = match registry.find_parser(file_path) {
                Some(p) => p,
                None => return WalkState::Continue,
            };

            // Read and parse using the FileSystem abstraction
            let source = match fs.read_to_string(file_path) {
                Ok(s) => s,
                Err(_) => return WalkState::Continue,
            };

            match parser.parse_module(file_path, &source) {
                Ok(module) => {
                    modules.lock().unwrap().push(module);
                }
                Err(e) => {
                    style::warning(&format!("Failed to parse {}: {}", file_path.display(), e));
                }
            }

            WalkState::Continue
        })
    });

    modules.into_inner().unwrap()
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
