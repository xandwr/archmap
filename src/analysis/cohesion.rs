use crate::analysis::DependencyGraph;
use crate::config::Config;
use crate::model::{Issue, Module};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Calculate module cohesion score based on dependency diversity.
///
/// The key insight: low cohesion isn't about using external libraries, it's about
/// using *many different* external libraries (scattered concerns). A module that
/// heavily uses petgraph is *specialized*, not unfocused.
///
/// We measure "dependency diversity" - how many distinct external crates are used.
/// A module using 5 imports from 1 crate is more cohesive than one using 5 imports
/// from 5 different crates.
pub fn detect_low_cohesion(
    modules: &[Module],
    _graph: &DependencyGraph,
    config: &Config,
) -> Vec<Issue> {
    let mut issues = Vec::new();
    let min_cohesion = config.thresholds.min_cohesion;

    // Group modules by their parent directory (package/namespace)
    let mut packages: HashMap<String, Vec<&Module>> = HashMap::new();

    for module in modules {
        let package = get_package_name(&module.path);
        packages.entry(package).or_default().push(module);
    }

    // For each module, calculate cohesion
    for module in modules {
        // Skip re-export hub modules - they're designed to have low internal cohesion
        if is_reexport_hub(module) {
            continue;
        }

        let package = get_package_name(&module.path);
        let siblings: HashSet<String> = packages
            .get(&package)
            .map(|p| p.iter().map(|m| m.name.clone()).collect())
            .unwrap_or_default();

        // Skip modules with no imports (they're perfectly cohesive by default)
        if module.imports.is_empty() {
            continue;
        }

        // Count internal imports and track unique external crates
        let mut internal_imports = 0;
        let mut external_crates: HashMap<String, usize> = HashMap::new();

        for import in &module.imports {
            let import_name = extract_module_name(import);
            if siblings.contains(&import_name) || is_relative_import(import) {
                internal_imports += 1;
            } else {
                // Extract the root crate name (e.g., "petgraph" from "petgraph::graph")
                let crate_name = extract_crate_name(import);
                *external_crates.entry(crate_name).or_insert(0) += 1;
            }
        }

        let total_external = external_crates.values().sum::<usize>();
        let unique_external_crates = external_crates.len();

        // Skip if no external dependencies
        if unique_external_crates == 0 {
            continue;
        }

        // Calculate cohesion based on dependency diversity
        // Formula: We penalize having many *different* external crates, not many imports from one crate
        //
        // A module with 5 petgraph imports has diversity = 1 (focused)
        // A module with 5 imports from 5 crates has diversity = 5 (scattered)
        //
        // cohesion = internal_weight / (internal_weight + diversity_penalty)
        // where diversity_penalty scales with unique crate count
        let internal_weight = (internal_imports as f64) + 1.0; // +1 to avoid division issues
        let diversity_penalty = unique_external_crates as f64;

        let cohesion_score = internal_weight / (internal_weight + diversity_penalty);

        // Flag modules with low cohesion (many different external dependencies)
        // Require at least 3 unique external crates to flag - using 1-2 external libs is normal
        if cohesion_score < min_cohesion && unique_external_crates >= 3 {
            issues.push(Issue::low_cohesion_v2(
                module.path.clone(),
                cohesion_score,
                internal_imports,
                total_external,
                unique_external_crates,
                top_crates(&external_crates, 3),
            ));
        }
    }

    // Sort by cohesion score (lowest first)
    issues.sort_by(|a, b| {
        let score_a = extract_cohesion_score(&a.message);
        let score_b = extract_cohesion_score(&b.message);
        score_a
            .partial_cmp(&score_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    issues
}

/// Extract the root crate name from an import path
fn extract_crate_name(import: &str) -> String {
    // Handle different import styles:
    // "petgraph::graph::DiGraph" -> "petgraph"
    // "std::collections::HashMap" -> "std"
    // "serde::Deserialize" -> "serde"
    // "./foo" or "../bar" -> "relative"
    // "super::foo" or "crate::bar" -> "crate"

    if import.starts_with("./") || import.starts_with("../") {
        return "relative".to_string();
    }

    if import.starts_with("super::")
        || import.starts_with("self::")
        || import.starts_with("crate::")
    {
        return "crate".to_string();
    }

    // For Rust-style paths, take the first segment
    if let Some(first) = import.split("::").next() {
        // Normalize common std library submodules
        if first == "std" || first == "core" || first == "alloc" {
            return "std".to_string();
        }
        return first.to_string();
    }

    // For JS/TS/Python style imports
    if let Some(first) = import.split('/').next() {
        // Handle scoped packages like @foo/bar
        if first.starts_with('@') {
            if let Some(second) = import.split('/').nth(1) {
                return format!("{}/{}", first, second);
            }
        }
        return first.to_string();
    }

    import.to_string()
}

/// Get the top N most-used crates
fn top_crates(crates: &HashMap<String, usize>, n: usize) -> Vec<String> {
    let mut sorted: Vec<_> = crates.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    sorted.into_iter().take(n).map(|(k, _)| k.clone()).collect()
}

fn get_package_name(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("root")
        .to_string()
}

fn extract_module_name(import: &str) -> String {
    import
        .split("::")
        .last()
        .unwrap_or(import)
        .split('/')
        .last()
        .unwrap_or(import)
        .split('.')
        .next()
        .unwrap_or(import)
        .to_string()
}

fn is_relative_import(import: &str) -> bool {
    import.starts_with("super::")
        || import.starts_with("self::")
        || import.starts_with("crate::")
        || import.starts_with("./")
        || import.starts_with("../")
}

/// Check if module is a re-export hub (lib.rs, mod.rs, main.rs, index.ts, __init__.py).
/// These modules are designed to aggregate and re-export from other modules,
/// so low cohesion is expected and not a code smell.
fn is_reexport_hub(module: &Module) -> bool {
    let file_name = module
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    matches!(
        file_name,
        "lib.rs" | "mod.rs" | "main.rs" | "index.ts" | "index.js" | "__init__.py"
    )
}

fn extract_cohesion_score(message: &str) -> f64 {
    // Parse score from message format "Cohesion score: 0.XX"
    message
        .split("Cohesion score: ")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0)
}
