use crate::model::IssueSeverity;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "archmap")]
#[command(about = "Generate architectural context for AI agents")]
#[command(version)]
pub struct Cli {
    /// Path to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format
    #[arg(short, long, default_value = "markdown")]
    pub format: OutputFormat,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Minimum severity to report
    #[arg(long, default_value = "info")]
    pub min_severity: IssueSeverity,

    /// Languages to analyze (comma-separated: rust,typescript,python)
    #[arg(long, value_delimiter = ',')]
    pub lang: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
}
