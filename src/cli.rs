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

    /// Generate a starter .archmap.toml configuration file
    #[arg(long)]
    pub init: bool,

    /// Watch for file changes and re-analyze
    #[arg(short, long)]
    pub watch: bool,

    /// Maximum dependency chain depth before flagging (default: 5)
    #[arg(long, default_value = "5")]
    pub max_depth: usize,

    /// Minimum cohesion score before flagging (0.0-1.0, default: 0.3)
    #[arg(long, default_value = "0.3")]
    pub min_cohesion: f64,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
}
