use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boundary {
    pub name: String,
    pub kind: BoundaryKind,
    pub indicators: Vec<String>,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BoundaryKind {
    Persistence,
    Network,
    Filesystem,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryViolation {
    pub boundary: Boundary,
    pub occurrences: Vec<BoundaryOccurrence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryOccurrence {
    pub path: PathBuf,
    pub line: usize,
    pub indicator_matched: String,
    pub context: String,
}

impl Boundary {
    pub fn persistence() -> Self {
        Self {
            name: "Persistence".to_string(),
            kind: BoundaryKind::Persistence,
            indicators: vec![
                "sqlx::".to_string(),
                "diesel::".to_string(),
                "sea_orm::".to_string(),
                "prisma.".to_string(),
                "SELECT ".to_string(),
                "INSERT ".to_string(),
                "UPDATE ".to_string(),
                "DELETE ".to_string(),
            ],
            suggestion: "Consider centralizing in a repository/data access layer".to_string(),
        }
    }

    pub fn network() -> Self {
        Self {
            name: "Network".to_string(),
            kind: BoundaryKind::Network,
            indicators: vec![
                "reqwest::".to_string(),
                "hyper::".to_string(),
                "fetch(".to_string(),
                "axios.".to_string(),
                "requests.".to_string(),
                "http.get".to_string(),
                "http.post".to_string(),
            ],
            suggestion: "Consider centralizing in an API client service".to_string(),
        }
    }

    pub fn filesystem() -> Self {
        Self {
            name: "Filesystem".to_string(),
            kind: BoundaryKind::Filesystem,
            indicators: vec![
                "std::fs::".to_string(),
                "tokio::fs::".to_string(),
                "fs.read".to_string(),
                "fs.write".to_string(),
                "open(".to_string(),
            ],
            suggestion: "Consider centralizing file operations or using dependency injection"
                .to_string(),
        }
    }

    pub fn default_boundaries() -> Vec<Self> {
        vec![Self::persistence(), Self::network(), Self::filesystem()]
    }
}
