mod context;
mod json;
mod markdown;
mod xml;

pub use context::AiContext;
pub use json::JsonFormatter;
pub use markdown::MarkdownFormatter;
pub use xml::XmlFormatter;

use crate::cli::{AiOutputFormat, PriorityStrategy};
use crate::model::AnalysisResult;
use crate::output::OutputFormatter;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

/// AI-optimized output formatter - facade that delegates to specific formatters
pub struct AiOutput {
    pub project_root: Option<PathBuf>,
    pub topo_order: bool,
    pub signatures_only: bool,
    pub token_budget: Option<usize>,
    pub priority_strategy: PriorityStrategy,
    pub format: AiOutputFormat,
    pub sources: HashMap<PathBuf, String>,
}

impl AiOutput {
    pub fn new(project_root: Option<PathBuf>) -> Self {
        Self {
            project_root,
            topo_order: true,
            signatures_only: false,
            token_budget: None,
            priority_strategy: PriorityStrategy::FanIn,
            format: AiOutputFormat::Markdown,
            sources: HashMap::new(),
        }
    }

    pub fn with_topo_order(mut self, enabled: bool) -> Self {
        self.topo_order = enabled;
        self
    }

    pub fn with_signatures_only(mut self, enabled: bool) -> Self {
        self.signatures_only = enabled;
        self
    }

    pub fn with_token_budget(mut self, tokens: usize) -> Self {
        self.token_budget = Some(tokens);
        self
    }

    pub fn with_priority(mut self, strategy: PriorityStrategy) -> Self {
        self.priority_strategy = strategy;
        self
    }

    pub fn with_format(mut self, format: AiOutputFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_sources(mut self, sources: HashMap<PathBuf, String>) -> Self {
        self.sources = sources;
        self
    }

    fn build_context(&self) -> AiContext {
        AiContext {
            project_root: self.project_root.clone(),
            topo_order: self.topo_order,
            signatures_only: self.signatures_only,
            token_budget: self.token_budget,
            sources: self.sources.clone(),
        }
    }
}

impl OutputFormatter for AiOutput {
    fn format<W: Write>(&self, result: &AnalysisResult, writer: &mut W) -> std::io::Result<()> {
        let ctx = self.build_context();

        match self.format {
            AiOutputFormat::Markdown => MarkdownFormatter::new(ctx).format(result, writer),
            AiOutputFormat::Json => JsonFormatter::new(ctx).format(result, writer),
            AiOutputFormat::Xml => XmlFormatter::new(ctx).format(result, writer),
        }
    }
}
