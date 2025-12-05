use crate::fs::{FileSystem, default_fs};
use crate::model::{Boundary, BoundaryKind};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub thresholds: Thresholds,
    pub boundaries: Vec<Boundary>,
    /// Glob patterns for modules where high coupling is expected (e.g., core domain models).
    /// Modules matching these patterns won't be flagged for high fan-in.
    pub expected_high_coupling: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Thresholds {
    pub god_object_lines: usize,
    pub coupling_fanin: usize,
    pub boundary_violation_min: usize,
    pub max_dependency_depth: usize,
    pub min_cohesion: f64,
    /// Minimum lines for a module to be considered a "fat module"
    pub fat_module_lines: usize,
    /// Minimum private functions to trigger fat module detection
    pub fat_module_private_functions: usize,
    /// Maximum lines per export before flagging as fat
    pub fat_module_lines_per_export: f64,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    thresholds: Option<RawThresholds>,
    boundaries: Option<HashMap<String, RawBoundary>>,
    #[serde(default)]
    expected_high_coupling: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawThresholds {
    god_object_lines: Option<usize>,
    coupling_fanin: Option<usize>,
    boundary_violation_min: Option<usize>,
    max_dependency_depth: Option<usize>,
    min_cohesion: Option<f64>,
    fat_module_lines: Option<usize>,
    fat_module_private_functions: Option<usize>,
    fat_module_lines_per_export: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawBoundary {
    name: Option<String>,
    indicators: Vec<String>,
    suggestion: Option<String>,
    #[serde(default)]
    allowed_in: Vec<String>,
    ownership_threshold: Option<f64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            thresholds: Thresholds::default(),
            boundaries: Boundary::default_boundaries(),
            expected_high_coupling: default_expected_high_coupling(),
        }
    }
}

fn default_expected_high_coupling() -> Vec<String> {
    vec![
        "**/model/**".to_string(),
        "**/models/**".to_string(),
        "**/types/**".to_string(),
        "**/config.rs".to_string(),
        "**/config.ts".to_string(),
        "**/config.py".to_string(),
        "**/lib.rs".to_string(),
        "**/mod.rs".to_string(),
        "**/index.ts".to_string(),
        "**/index.js".to_string(),
        "**/__init__.py".to_string(),
        "**/fs.rs".to_string(),     // Centralized filesystem abstraction
        "**/utils/**".to_string(),  // Utility modules
        "**/common/**".to_string(), // Common/shared modules
    ]
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            god_object_lines: 500,
            coupling_fanin: 5,
            boundary_violation_min: 2,
            max_dependency_depth: 5,
            min_cohesion: 0.3,
            fat_module_lines: 400,
            fat_module_private_functions: 8,
            fat_module_lines_per_export: 100.0,
        }
    }
}

impl Config {
    pub fn load(project_path: &Path) -> Result<Self, ConfigError> {
        Self::load_with_fs(project_path, default_fs())
    }

    pub fn load_with_fs(project_path: &Path, fs: &dyn FileSystem) -> Result<Self, ConfigError> {
        let config_path = project_path.join(".archmap.toml");

        if !fs.exists(&config_path) {
            return Ok(Self::default());
        }

        let content = fs.read_to_string(&config_path)?;
        let raw: RawConfig = toml::from_str(&content)?;

        let thresholds = match raw.thresholds {
            Some(t) => Thresholds {
                god_object_lines: t.god_object_lines.unwrap_or(500),
                coupling_fanin: t.coupling_fanin.unwrap_or(5),
                boundary_violation_min: t.boundary_violation_min.unwrap_or(2),
                max_dependency_depth: t.max_dependency_depth.unwrap_or(5),
                min_cohesion: t.min_cohesion.unwrap_or(0.3),
                fat_module_lines: t.fat_module_lines.unwrap_or(400),
                fat_module_private_functions: t.fat_module_private_functions.unwrap_or(8),
                fat_module_lines_per_export: t.fat_module_lines_per_export.unwrap_or(100.0),
            },
            None => Thresholds::default(),
        };

        let boundaries = match raw.boundaries {
            Some(map) => map
                .into_iter()
                .map(|(key, raw_b)| {
                    let kind = match key.as_str() {
                        "persistence" => BoundaryKind::Persistence,
                        "network" => BoundaryKind::Network,
                        "filesystem" => BoundaryKind::Filesystem,
                        _ => BoundaryKind::Custom(key.clone()),
                    };

                    // Get defaults for this boundary kind if available
                    let defaults = get_boundary_defaults(&kind);

                    Boundary {
                        name: raw_b.name.unwrap_or_else(|| capitalize(&key)),
                        kind,
                        indicators: raw_b.indicators,
                        suggestion: raw_b
                            .suggestion
                            .unwrap_or_else(|| format!("Consider centralizing {} operations", key)),
                        allowed_in: if raw_b.allowed_in.is_empty() {
                            defaults.0
                        } else {
                            raw_b.allowed_in
                        },
                        ownership_threshold: raw_b.ownership_threshold.unwrap_or(defaults.1),
                    }
                })
                .collect(),
            None => Boundary::default_boundaries(),
        };

        let expected_high_coupling = if raw.expected_high_coupling.is_empty() {
            default_expected_high_coupling()
        } else {
            raw.expected_high_coupling
        };

        Ok(Self {
            thresholds,
            boundaries,
            expected_high_coupling,
        })
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

/// Get default allowed_in patterns and ownership_threshold for known boundary types
fn get_boundary_defaults(kind: &BoundaryKind) -> (Vec<String>, f64) {
    match kind {
        BoundaryKind::Persistence => (
            vec![
                "**/db/**".to_string(),
                "**/database/**".to_string(),
                "**/repository/**".to_string(),
                "**/repo/**".to_string(),
            ],
            0.5,
        ),
        BoundaryKind::Network => (
            vec![
                "**/client/**".to_string(),
                "**/api/**".to_string(),
                "**/http/**".to_string(),
                "**/network/**".to_string(),
            ],
            0.5,
        ),
        BoundaryKind::Filesystem => (
            vec![
                "**/fs.rs".to_string(),
                "**/io.rs".to_string(),
                "**/io/**".to_string(),
                "**/storage/**".to_string(),
            ],
            0.5,
        ),
        BoundaryKind::Custom(_) => (Vec::new(), 0.5),
    }
}

/// Generate a starter .archmap.toml configuration file with all defaults documented
pub fn generate_config_template() -> String {
    r#"# Archmap Configuration
# This file configures architectural analysis for your project.

[thresholds]
# Maximum lines before a file is flagged as a "god object"
# Default: 500
god_object_lines = 500

# Maximum number of modules importing a single module before flagging high coupling
# Default: 5
coupling_fanin = 5

# Minimum number of boundary violations before reporting
# Default: 2
boundary_violation_min = 2

# Maximum dependency chain depth before flagging (A → B → C → D → E)
# Default: 5
max_dependency_depth = 5

# Minimum cohesion score (ratio of internal vs external dependencies)
# Range: 0.0 to 1.0. Lower scores indicate module is doing too many unrelated things.
# Default: 0.3
min_cohesion = 0.3

# Fat module detection - identifies files with excessive internal complexity
# These are files with many private functions but few exports (hidden sprawl)
# Unlike god objects which have many exports, fat modules hide their complexity
# Test files are automatically excluded from this check
# Default: 400 lines minimum
fat_module_lines = 400
# Default: 8 private functions minimum
fat_module_private_functions = 8
# Default: 100 lines per export maximum
fat_module_lines_per_export = 100.0

# Expected High Coupling
# Glob patterns for modules where high fan-in is expected and shouldn't be flagged.
# Core domain models, config files, and index/entry modules typically have high coupling.
# Default patterns cover common conventions across languages.
expected_high_coupling = [
    "**/model/**",
    "**/models/**",
    "**/types/**",
    "**/config.rs",
    "**/config.ts",
    "**/config.py",
    "**/lib.rs",
    "**/mod.rs",
    "**/index.ts",
    "**/index.js",
    "**/__init__.py",
]

# Architectural Boundaries
# Define patterns that indicate crossing architectural boundaries.
# Scattered boundary crossings often indicate missing abstraction layers.
#
# Each boundary supports:
# - indicators: strings to search for in source code
# - allowed_in: glob patterns for modules where this boundary is allowed (e.g., gateway modules)
# - ownership_threshold: if one module has >= this fraction of occurrences, it's the "owner"
#                        and won't be flagged (default: 0.5)

[boundaries.persistence]
name = "Persistence"
indicators = [
    "sqlx::",
    "diesel::",
    "sea_orm::",
    "prisma.",
    "SELECT ",
    "INSERT ",
    "UPDATE ",
    "DELETE ",
]
suggestion = "Consider centralizing in a repository/data access layer"
# Modules matching these patterns are allowed to cross this boundary
allowed_in = ["**/db/**", "**/database/**", "**/repository/**", "**/repo/**"]

[boundaries.network]
name = "Network"
indicators = [
    "reqwest::",
    "hyper::",
    "fetch(",
    "axios.",
    "requests.",
    "http.get",
    "http.post",
]
suggestion = "Consider centralizing in an API client service"
allowed_in = ["**/client/**", "**/api/**", "**/http/**", "**/network/**"]

[boundaries.filesystem]
name = "Filesystem"
indicators = [
    # Rust
    "std::fs::",
    "tokio::fs::",
    # JavaScript/TypeScript (Node.js)
    "fs.readFile",
    "fs.writeFile",
    "fs.readFileSync",
    "fs.writeFileSync",
    "fs.promises",
    # Python
    "open(",
    "pathlib.Path(",
    "shutil.",
]
suggestion = "Consider centralizing file operations or using dependency injection"
allowed_in = ["**/fs.rs", "**/io.rs", "**/io/**", "**/storage/**"]

# Custom boundaries example (uncomment to use):
# [boundaries.logging]
# name = "Logging"
# indicators = ["log::", "tracing::", "console.log", "print("]
# suggestion = "Consider using a centralized logging facade"
# allowed_in = ["**/logger/**", "**/logging/**"]
# ownership_threshold = 0.6  # Higher threshold = stricter ownership detection
"#
    .to_string()
}
