use crate::model::IssueSeverity;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "archmap")]
#[command(about = "Generate architectural context for AI agents")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to analyze (defaults to current directory)
    /// Used when no subcommand is specified for backward compatibility
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Run full architectural analysis (default behavior)
    Analyze(AnalyzeArgs),

    /// Generate AI-optimized context output
    Ai(AiArgs),

    /// Analyze change impact for a specific file
    Impact(ImpactArgs),

    /// Save an architectural snapshot
    Snapshot(SnapshotArgs),

    /// Compare current state against a baseline snapshot
    Diff(DiffArgs),

    /// Launch interactive graph visualization
    Graph(GraphArgs),

    /// Generate a starter .archmap.toml configuration file
    Init(InitArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct AnalyzeArgs {
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

impl Default for AnalyzeArgs {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            format: OutputFormat::Markdown,
            output: None,
            min_severity: IssueSeverity::Info,
            lang: None,
            watch: false,
            max_depth: 5,
            min_cohesion: 0.3,
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct AiArgs {
    /// Path to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Maximum tokens for output (uses tiktoken for accurate counting)
    #[arg(long)]
    pub tokens: Option<usize>,

    /// Output only architectural signatures (public API surface)
    #[arg(long)]
    pub signatures: bool,

    /// Use topological ordering (dependencies before dependents)
    #[arg(long, default_value = "true")]
    pub topo_order: bool,

    /// Output format
    #[arg(short, long, default_value = "markdown")]
    pub format: AiOutputFormat,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Prioritization strategy for token budgeting
    #[arg(long, default_value = "fan-in")]
    pub priority: PriorityStrategy,

    /// Languages to analyze (comma-separated: rust,typescript,python)
    #[arg(long, value_delimiter = ',')]
    pub lang: Option<Vec<String>>,
}

#[derive(Parser, Debug, Clone)]
pub struct ImpactArgs {
    /// File to analyze for change impact
    pub file: PathBuf,

    /// Project path (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Maximum depth to traverse (unlimited if not specified)
    #[arg(short, long)]
    pub depth: Option<usize>,

    /// Output format
    #[arg(short, long, default_value = "markdown")]
    pub format: OutputFormat,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Show ASCII tree visualization
    #[arg(long)]
    pub tree: bool,

    /// Languages to analyze (comma-separated: rust,typescript,python)
    #[arg(long, value_delimiter = ',')]
    pub lang: Option<Vec<String>>,
}

#[derive(Parser, Debug, Clone)]
pub struct SnapshotArgs {
    /// Save snapshot to this file
    #[arg(long)]
    pub save: PathBuf,

    /// Path to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Languages to analyze (comma-separated: rust,typescript,python)
    #[arg(long, value_delimiter = ',')]
    pub lang: Option<Vec<String>>,
}

#[derive(Parser, Debug, Clone)]
pub struct DiffArgs {
    /// Baseline snapshot file to compare against
    pub baseline: PathBuf,

    /// Path to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format
    #[arg(short, long, default_value = "markdown")]
    pub format: OutputFormat,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Languages to analyze (comma-separated: rust,typescript,python)
    #[arg(long, value_delimiter = ',')]
    pub lang: Option<Vec<String>>,

    /// Exit with error if architectural regressions are found
    #[arg(long)]
    pub fail_on_regression: bool,
}

#[derive(Parser, Debug, Clone)]
pub struct GraphArgs {
    /// Start HTTP server for interactive visualization
    #[arg(long)]
    pub serve: bool,

    /// Port for HTTP server
    #[arg(long, default_value = "3000")]
    pub port: u16,

    /// Path to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Open browser automatically
    #[arg(long)]
    pub open: bool,

    /// Export graph as static HTML file instead of serving
    #[arg(long)]
    pub export: Option<PathBuf>,

    /// Languages to analyze (comma-separated: rust,typescript,python)
    #[arg(long, value_delimiter = ',')]
    pub lang: Option<Vec<String>>,
}

#[derive(Parser, Debug, Clone)]
pub struct InitArgs {
    /// Path where to create .archmap.toml (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum AiOutputFormat {
    #[default]
    Markdown,
    Json,
    Xml,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum PriorityStrategy {
    /// Prioritize modules by number of dependents (most imported first)
    #[default]
    FanIn,
    /// Prioritize modules by number of dependencies
    FanOut,
    /// Combined score using fan-in, fan-out, and data structures
    Combined,
}
