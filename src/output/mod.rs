mod json;
mod markdown;

pub use json::JsonOutput;
pub use markdown::MarkdownOutput;

use crate::model::AnalysisResult;
use std::io::Write;

pub trait OutputFormatter {
    fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()>;
}
