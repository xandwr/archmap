use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boundary {
    pub name: String,
    pub kind: BoundaryKind,
    pub indicators: Vec<String>,
    pub suggestion: String,
    /// Glob patterns for modules where this boundary crossing is allowed.
    /// e.g., ["**/fs.rs", "**/io/**"] for filesystem operations.
    #[serde(default)]
    pub allowed_in: Vec<String>,
    /// Threshold (0.0-1.0) for automatic ownership detection.
    /// If a single module has >= this fraction of all occurrences,
    /// it's considered the "owner" and excluded from violations.
    /// Default: 0.5 (50%)
    #[serde(default = "default_ownership_threshold")]
    pub ownership_threshold: f64,
}

fn default_ownership_threshold() -> f64 {
    0.5
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
            allowed_in: vec![
                "**/db/**".to_string(),
                "**/database/**".to_string(),
                "**/repository/**".to_string(),
                "**/repo/**".to_string(),
            ],
            ownership_threshold: default_ownership_threshold(),
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
            allowed_in: vec![
                "**/client/**".to_string(),
                "**/api/**".to_string(),
                "**/http/**".to_string(),
                "**/network/**".to_string(),
            ],
            ownership_threshold: default_ownership_threshold(),
        }
    }

    pub fn filesystem() -> Self {
        Self {
            name: "Filesystem".to_string(),
            kind: BoundaryKind::Filesystem,
            indicators: vec![
                // Rust
                "std::fs::".to_string(),
                "tokio::fs::".to_string(),
                // JavaScript/TypeScript (Node.js) - specific methods to avoid false positives
                "fs.readFile".to_string(),
                "fs.writeFile".to_string(),
                "fs.readFileSync".to_string(),
                "fs.writeFileSync".to_string(),
                "fs.promises".to_string(),
                // Python
                "open(".to_string(),
                "pathlib.Path(".to_string(),
                "shutil.".to_string(),
            ],
            suggestion: "Consider centralizing file operations or using dependency injection"
                .to_string(),
            allowed_in: vec![
                "**/fs.rs".to_string(),
                "**/io.rs".to_string(),
                "**/io/**".to_string(),
                "**/storage/**".to_string(),
            ],
            ownership_threshold: default_ownership_threshold(),
        }
    }

    pub fn default_boundaries() -> Vec<Self> {
        vec![Self::persistence(), Self::network(), Self::filesystem()]
    }

    /// Check if a path is allowed to cross this boundary.
    pub fn is_allowed(&self, path: &std::path::Path) -> bool {
        if self.allowed_in.is_empty() {
            return false;
        }
        let path_str = path.to_string_lossy();
        for pattern in &self.allowed_in {
            if glob_match(pattern, &path_str) {
                return true;
            }
        }
        false
    }
}

/// Simple glob matching supporting ** and * wildcards.
/// This is language-independent - just path pattern matching.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    // Normalize path separators
    let path = path.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");

    glob_match_recursive(&pattern, &path)
}

fn glob_match_recursive(pattern: &str, path: &str) -> bool {
    // Handle ** (match any path segments)
    if let Some(pos) = pattern.find("**") {
        let prefix = &pattern[..pos];
        let suffix = &pattern[pos + 2..];
        let suffix = suffix.strip_prefix('/').unwrap_or(suffix);

        // Prefix must match the start
        if !prefix.is_empty() && !path.starts_with(prefix) {
            return false;
        }

        let remaining = &path[prefix.len()..];

        // Try matching suffix at every position
        if suffix.is_empty() {
            return true;
        }

        for (i, _) in remaining.char_indices() {
            if glob_match_recursive(suffix, &remaining[i..]) {
                return true;
            }
        }
        // Also try matching at the very end
        glob_match_recursive(suffix, "")
    } else if let Some(pos) = pattern.find('*') {
        // Handle single * (match within one segment)
        let prefix = &pattern[..pos];
        let suffix = &pattern[pos + 1..];

        if !path.starts_with(prefix) {
            return false;
        }

        let remaining = &path[prefix.len()..];

        // * doesn't match path separators
        for (i, c) in remaining.char_indices() {
            if c == '/' {
                // Can only match up to here
                return glob_match_recursive(suffix, &remaining[i..]);
            }
            if glob_match_recursive(suffix, &remaining[i..]) {
                return true;
            }
        }
        glob_match_recursive(suffix, "")
    } else {
        // No wildcards - exact match or path ends with pattern
        pattern == path || path.ends_with(&format!("/{}", pattern))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        // ** matches any path
        assert!(glob_match("**/fs.rs", "src/fs.rs"));
        assert!(glob_match("**/fs.rs", "src/util/fs.rs"));
        assert!(glob_match("**/fs.rs", "fs.rs"));

        // ** in middle
        assert!(glob_match("src/**/fs.rs", "src/fs.rs"));
        assert!(glob_match("src/**/fs.rs", "src/util/fs.rs"));
        assert!(glob_match("src/**/fs.rs", "src/a/b/c/fs.rs"));

        // Single *
        assert!(glob_match("*.rs", "fs.rs"));
        assert!(glob_match("src/*.rs", "src/fs.rs"));
        assert!(!glob_match("src/*.rs", "src/util/fs.rs")); // * doesn't cross /

        // Directory patterns
        assert!(glob_match("**/io/**", "src/io/read.rs"));
        assert!(glob_match("**/io/**", "io/write.rs"));

        // No match
        assert!(!glob_match("**/fs.rs", "src/filesystem.rs"));
        assert!(!glob_match("**/db/**", "src/database.rs"));
    }
}
