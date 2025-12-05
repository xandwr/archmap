use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub kind: IssueKind,
    pub severity: IssueSeverity,
    pub locations: Vec<Location>,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueKind {
    CircularDependency,
    GodObject,
    HighCoupling,
    BoundaryViolation {
        boundary_name: String,
    },
    DeepDependencyChain {
        depth: usize,
    },
    LowCohesion {
        score: f64,
    },
    /// Module with excessive internal complexity relative to its public interface
    FatModule {
        private_functions: usize,
        public_functions: usize,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub context: Option<String>,
}

impl Issue {
    pub fn circular_dependency(cycle: Vec<PathBuf>) -> Self {
        let locations: Vec<Location> = cycle
            .iter()
            .map(|p| Location {
                path: p.clone(),
                line: None,
                context: None,
            })
            .collect();

        let cycle_str: Vec<_> = cycle
            .iter()
            .filter_map(|p| p.file_stem().and_then(|s| s.to_str()))
            .collect();

        Self {
            kind: IssueKind::CircularDependency,
            severity: IssueSeverity::Error,
            locations,
            message: format!("Circular dependency: {}", cycle_str.join(" → ")),
            suggestion: Some(
                "Break the cycle by extracting shared types or using dependency injection"
                    .to_string(),
            ),
        }
    }

    pub fn god_object(path: PathBuf, lines: usize, responsibilities: Vec<String>) -> Self {
        Self {
            kind: IssueKind::GodObject,
            severity: IssueSeverity::Warn,
            locations: vec![Location {
                path,
                line: None,
                context: None,
            }],
            message: format!(
                "{} lines with mixed responsibilities: {}",
                lines,
                responsibilities.join(", ")
            ),
            suggestion: Some("Consider splitting into smaller, focused modules".to_string()),
        }
    }

    pub fn high_coupling(path: PathBuf, fan_in: usize) -> Self {
        Self {
            kind: IssueKind::HighCoupling,
            severity: IssueSeverity::Warn,
            locations: vec![Location {
                path,
                line: None,
                context: None,
            }],
            message: format!("Imported by {} other modules", fan_in),
            suggestion: Some("High coupling makes changes risky. Consider if this module has too many responsibilities".to_string()),
        }
    }

    pub fn boundary_violation(
        boundary_name: String,
        locations: Vec<Location>,
        suggestion: String,
    ) -> Self {
        let location_count = locations.len();
        Self {
            kind: IssueKind::BoundaryViolation {
                boundary_name: boundary_name.clone(),
            },
            severity: IssueSeverity::Warn,
            locations,
            message: format!(
                "{} boundary crossed in {} locations",
                boundary_name, location_count
            ),
            suggestion: Some(suggestion),
        }
    }

    pub fn deep_dependency_chain(chain: Vec<PathBuf>, max_depth: usize) -> Self {
        let depth = chain.len();
        let locations: Vec<Location> = chain
            .iter()
            .map(|p| Location {
                path: p.clone(),
                line: None,
                context: None,
            })
            .collect();

        let chain_str: Vec<_> = chain
            .iter()
            .filter_map(|p| p.file_stem().and_then(|s| s.to_str()))
            .collect();

        Self {
            kind: IssueKind::DeepDependencyChain { depth },
            severity: IssueSeverity::Warn,
            locations,
            message: format!(
                "Dependency chain of depth {} (threshold: {}): {}",
                depth,
                max_depth,
                chain_str.join(" → ")
            ),
            suggestion: Some(
                "Consider introducing an abstraction layer to reduce coupling depth".to_string(),
            ),
        }
    }

    pub fn low_cohesion(
        path: PathBuf,
        score: f64,
        internal_imports: usize,
        external_imports: usize,
    ) -> Self {
        Self {
            kind: IssueKind::LowCohesion { score },
            severity: IssueSeverity::Info,
            locations: vec![Location {
                path,
                line: None,
                context: None,
            }],
            message: format!(
                "Cohesion score: {:.2} ({} internal, {} external imports)",
                score, internal_imports, external_imports
            ),
            suggestion: Some(
                "Low cohesion suggests this module may be doing too many unrelated things. Consider splitting into focused modules.".to_string(),
            ),
        }
    }

    /// Improved cohesion metric that considers dependency diversity
    pub fn low_cohesion_v2(
        path: PathBuf,
        score: f64,
        internal_imports: usize,
        total_external: usize,
        unique_crates: usize,
        top_crates: Vec<String>,
    ) -> Self {
        let crates_str = if top_crates.is_empty() {
            String::new()
        } else {
            format!(" ({})", top_crates.join(", "))
        };

        Self {
            kind: IssueKind::LowCohesion { score },
            severity: IssueSeverity::Info,
            locations: vec![Location {
                path,
                line: None,
                context: None,
            }],
            message: format!(
                "Cohesion score: {:.2} ({} internal imports, {} external from {} different crates{})",
                score, internal_imports, total_external, unique_crates, crates_str
            ),
            suggestion: Some(
                "This module depends on many different external crates, suggesting scattered concerns. Consider splitting by responsibility.".to_string(),
            ),
        }
    }

    /// Fat module: excessive internal complexity hidden behind a small interface
    pub fn fat_module(
        path: PathBuf,
        lines: usize,
        private_functions: usize,
        public_functions: usize,
        exports: usize,
    ) -> Self {
        Self {
            kind: IssueKind::FatModule {
                private_functions,
                public_functions,
            },
            severity: IssueSeverity::Info,
            locations: vec![Location {
                path,
                line: None,
                context: None,
            }],
            message: format!(
                "{} lines with {} private functions but only {} exports",
                lines, private_functions, exports
            ),
            suggestion: Some(
                "This module has significant internal complexity hidden behind a small interface. \
                Consider extracting related functions into submodules."
                    .to_string(),
            ),
        }
    }
}

impl std::fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueSeverity::Info => write!(f, "info"),
            IssueSeverity::Warn => write!(f, "warn"),
            IssueSeverity::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for IssueSeverity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(IssueSeverity::Info),
            "warn" | "warning" => Ok(IssueSeverity::Warn),
            "error" => Ok(IssueSeverity::Error),
            _ => Err(format!("Unknown severity: {}", s)),
        }
    }
}
