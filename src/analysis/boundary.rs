use crate::config::Config;
use crate::fs::{FileSystem, default_fs};
use crate::model::{Boundary, Issue, Location, Module};
use std::collections::HashMap;
use std::path::PathBuf;

/// Check if the indicator appears inside a string literal definition (e.g., in a config array).
/// This filters out false positives from config files that define boundary indicators.
fn is_string_literal_definition(line: &str, indicator: &str) -> bool {
    // Find where the indicator appears in the line
    if let Some(pos) = line.find(indicator) {
        // Check if there's a quote immediately before the indicator
        // This catches patterns like: "sqlx::", 'reqwest::', `fetch(`
        let before = &line[..pos];
        let trimmed = before.trim_end();
        if trimmed.ends_with('"') || trimmed.ends_with('\'') || trimmed.ends_with('`') {
            return true;
        }
    }
    false
}

pub fn detect_boundary_violations(modules: &[Module], config: &Config) -> Vec<Issue> {
    detect_boundary_violations_with_fs(modules, config, default_fs())
}

pub fn detect_boundary_violations_with_fs(
    modules: &[Module],
    config: &Config,
    fs: &dyn FileSystem,
) -> Vec<Issue> {
    let mut issues = Vec::new();

    // For each boundary, track where it's crossed
    for boundary in &config.boundaries {
        let mut occurrences_by_module: HashMap<PathBuf, Vec<Location>> = HashMap::new();

        for module in modules {
            // Skip modules that are explicitly allowed to cross this boundary
            if boundary.is_allowed(&module.path) {
                continue;
            }

            // Read the file content to scan for indicators
            let content = match fs.read_to_string(&module.path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_num, line) in content.lines().enumerate() {
                for indicator in &boundary.indicators {
                    if line.contains(indicator) && !is_string_literal_definition(line, indicator) {
                        occurrences_by_module
                            .entry(module.path.clone())
                            .or_default()
                            .push(Location {
                                path: module.path.clone(),
                                line: Some(line_num + 1),
                                context: Some(line.trim().to_string()),
                            });
                        break; // Only count once per line
                    }
                }
            }
        }

        // Apply ownership detection: if one module has most of the occurrences,
        // it's the designated owner and shouldn't be flagged
        let filtered_occurrences = apply_ownership_filter(&occurrences_by_module, boundary);

        // Re-group after filtering
        let modules_affected: HashMap<_, Vec<_>> =
            filtered_occurrences
                .iter()
                .fold(HashMap::new(), |mut acc, loc| {
                    acc.entry(&loc.path).or_default().push(loc);
                    acc
                });

        // If boundary is crossed in multiple modules, it's a violation
        if modules_affected.len() >= config.thresholds.boundary_violation_min {
            issues.push(Issue::boundary_violation(
                boundary.name.clone(),
                filtered_occurrences,
                boundary.suggestion.clone(),
            ));
        }
    }

    issues
}

/// Detect if a single module "owns" this boundary (has majority of occurrences)
/// and filter it out from violations. This is language-independent - just counting.
fn apply_ownership_filter(
    occurrences_by_module: &HashMap<PathBuf, Vec<Location>>,
    boundary: &Boundary,
) -> Vec<Location> {
    if occurrences_by_module.is_empty() {
        return Vec::new();
    }

    let total_occurrences: usize = occurrences_by_module.values().map(|v| v.len()).sum();
    if total_occurrences == 0 {
        return Vec::new();
    }

    // Find the module with the most occurrences
    let (owner_path, owner_count) = occurrences_by_module
        .iter()
        .max_by_key(|(_, locs)| locs.len())
        .map(|(path, locs)| (path.clone(), locs.len()))
        .unwrap();

    let ownership_ratio = owner_count as f64 / total_occurrences as f64;

    // If one module owns enough of the boundary, exclude it from violations
    if ownership_ratio >= boundary.ownership_threshold {
        occurrences_by_module
            .iter()
            .filter(|(path, _)| **path != owner_path)
            .flat_map(|(_, locs)| locs.clone())
            .collect()
    } else {
        // No clear owner - report all occurrences
        occurrences_by_module
            .values()
            .flat_map(|locs| locs.clone())
            .collect()
    }
}
