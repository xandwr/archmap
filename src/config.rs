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
}

#[derive(Debug, Clone)]
pub struct Thresholds {
    pub god_object_lines: usize,
    pub coupling_fanin: usize,
    pub boundary_violation_min: usize,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    thresholds: Option<RawThresholds>,
    boundaries: Option<HashMap<String, RawBoundary>>,
}

#[derive(Debug, Deserialize)]
struct RawThresholds {
    god_object_lines: Option<usize>,
    coupling_fanin: Option<usize>,
    boundary_violation_min: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RawBoundary {
    name: Option<String>,
    indicators: Vec<String>,
    suggestion: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            thresholds: Thresholds::default(),
            boundaries: Boundary::default_boundaries(),
        }
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            god_object_lines: 500,
            coupling_fanin: 5,
            boundary_violation_min: 2,
        }
    }
}

impl Config {
    pub fn load(project_path: &Path) -> Result<Self, ConfigError> {
        let config_path = project_path.join(".archmap.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)?;
        let raw: RawConfig = toml::from_str(&content)?;

        let thresholds = match raw.thresholds {
            Some(t) => Thresholds {
                god_object_lines: t.god_object_lines.unwrap_or(500),
                coupling_fanin: t.coupling_fanin.unwrap_or(5),
                boundary_violation_min: t.boundary_violation_min.unwrap_or(2),
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

                    Boundary {
                        name: raw_b.name.unwrap_or_else(|| capitalize(&key)),
                        kind,
                        indicators: raw_b.indicators,
                        suggestion: raw_b
                            .suggestion
                            .unwrap_or_else(|| format!("Consider centralizing {} operations", key)),
                    }
                })
                .collect(),
            None => Boundary::default_boundaries(),
        };

        Ok(Self {
            thresholds,
            boundaries,
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
