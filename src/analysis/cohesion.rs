use crate::analysis::DependencyGraph;
use crate::config::Config;
use crate::model::{Issue, Module};
use std::collections::HashSet;
use std::path::Path;

/// Calculate module cohesion score as ratio of internal vs external dependencies.
/// Low cohesion = module is doing too many unrelated things.
pub fn detect_low_cohesion(
    modules: &[Module],
    _graph: &DependencyGraph,
    config: &Config,
) -> Vec<Issue> {
    let mut issues = Vec::new();
    let min_cohesion = config.thresholds.min_cohesion;

    // Group modules by their parent directory (package/namespace)
    let mut packages: std::collections::HashMap<String, Vec<&Module>> =
        std::collections::HashMap::new();

    for module in modules {
        let package = get_package_name(&module.path);
        packages.entry(package).or_default().push(module);
    }

    // For each module, calculate cohesion
    for module in modules {
        let package = get_package_name(&module.path);
        let siblings: HashSet<String> = packages
            .get(&package)
            .map(|p| p.iter().map(|m| m.name.clone()).collect())
            .unwrap_or_default();

        // Skip modules with no imports (they're perfectly cohesive by default)
        if module.imports.is_empty() {
            continue;
        }

        // Count internal vs external imports
        let mut internal_imports = 0;
        let mut external_imports = 0;

        for import in &module.imports {
            let import_name = extract_module_name(import);
            if siblings.contains(&import_name) || is_relative_import(import) {
                internal_imports += 1;
            } else {
                external_imports += 1;
            }
        }

        let total_imports = internal_imports + external_imports;
        if total_imports == 0 {
            continue;
        }

        let cohesion_score = internal_imports as f64 / total_imports as f64;

        // Flag modules with low cohesion
        if cohesion_score < min_cohesion && external_imports >= 3 {
            issues.push(Issue::low_cohesion(
                module.path.clone(),
                cohesion_score,
                internal_imports,
                external_imports,
            ));
        }
    }

    // Sort by cohesion score (lowest first)
    issues.sort_by(|a, b| {
        // Extract cohesion from message for sorting
        let score_a = extract_cohesion_score(&a.message);
        let score_b = extract_cohesion_score(&b.message);
        score_a
            .partial_cmp(&score_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    issues
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

fn extract_cohesion_score(message: &str) -> f64 {
    // Parse score from message format "Cohesion score: 0.XX"
    message
        .split("Cohesion score: ")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0)
}
