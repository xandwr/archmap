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
    BoundaryViolation { boundary_name: String },
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
