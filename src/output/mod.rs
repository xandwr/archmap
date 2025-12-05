mod ai;
mod json;
mod markdown;

pub use ai::AiOutput;
pub use json::JsonOutput;
pub use markdown::MarkdownOutput;

use crate::model::AnalysisResult;
use std::io::Write;
use std::path::{Path, PathBuf};

pub trait OutputFormatter {
    fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()>;
}

/// Convert an absolute path to a relative path based on project root.
/// Returns the path as-is if no root is provided or if strip_prefix fails.
pub fn relative_path(path: &Path, project_root: Option<&PathBuf>) -> String {
    if let Some(root) = project_root {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    } else {
        path.display().to_string()
    }
}
