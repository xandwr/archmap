use crate::config::Config;
use crate::model::{Issue, Location, Module};
use std::collections::HashMap;

pub fn detect_boundary_violations(modules: &[Module], config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    // For each boundary, track where it's crossed
    for boundary in &config.boundaries {
        let mut occurrences: Vec<Location> = Vec::new();

        for module in modules {
            // Read the file content to scan for indicators
            let content = match std::fs::read_to_string(&module.path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_num, line) in content.lines().enumerate() {
                for indicator in &boundary.indicators {
                    if line.contains(indicator) {
                        occurrences.push(Location {
                            path: module.path.clone(),
                            line: Some(line_num + 1),
                            context: Some(line.trim().to_string()),
                        });
                        break; // Only count once per line
                    }
                }
            }
        }

        // Group by module to see how scattered it is
        let modules_affected: HashMap<_, Vec<_>> =
            occurrences.iter().fold(HashMap::new(), |mut acc, loc| {
                acc.entry(&loc.path).or_default().push(loc);
                acc
            });

        // If boundary is crossed in multiple modules, it's a violation
        if modules_affected.len() >= config.thresholds.boundary_violation_min {
            issues.push(Issue::boundary_violation(
                boundary.name.clone(),
                occurrences,
                boundary.suggestion.clone(),
            ));
        }
    }

    issues
}
