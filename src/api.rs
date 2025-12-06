//! Clean library API for archmap.
//!
//! This module provides a programmatic interface for using archmap as a Rust library.
//! Unlike the CLI commands which print output and return exit codes, these functions
//! return proper Result types that can be handled by calling code.
//!
//! # Example
//!
//! ```no_run
//! use archmap::{analyze, AnalysisOptions};
//! use std::path::Path;
//!
//! let result = analyze(Path::new("."), AnalysisOptions::default())?;
//! println!("Found {} modules", result.modules.len());
//! for issue in &result.issues {
//!     println!("Issue: {}", issue.message);
//! }
//! # Ok::<(), archmap::ArchmapError>(())
//! ```

use crate::analysis::{self, DependencyGraph, ImpactAnalysis, ImpactError};
use crate::cli::{AiOutputFormat, PriorityStrategy};
use crate::config::{Config, ConfigError};
use crate::fs::{FileSystem, default_fs};
use crate::model::AnalysisResult;
use crate::output::{AiOutput, OutputFormatter};
use crate::parser::ParserRegistry;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during archmap operations.
#[derive(Debug, Error)]
pub enum ArchmapError {
    /// The specified path could not be found or resolved.
    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    /// Configuration file error.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Impact analysis error.
    #[error("Impact analysis error: {0}")]
    Impact(#[from] ImpactError),

    /// IO error during analysis.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Options for the `analyze` function.
#[derive(Debug, Clone)]
pub struct AnalysisOptions {
    /// Languages to analyze (empty means all supported languages).
    pub languages: Vec<String>,

    /// Patterns to exclude from analysis.
    pub exclude: Vec<String>,

    /// Maximum dependency depth before flagging.
    pub max_depth: usize,

    /// Minimum cohesion score before flagging (0.0-1.0).
    pub min_cohesion: f64,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
            exclude: Vec::new(),
            max_depth: 5,
            min_cohesion: 0.3,
        }
    }
}

/// Options for the `impact` function.
#[derive(Debug, Clone)]
pub struct ImpactOptions {
    /// Languages to analyze (empty means all supported languages).
    pub languages: Vec<String>,

    /// Maximum depth to traverse (None means unlimited).
    pub depth: Option<usize>,
}

impl Default for ImpactOptions {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
            depth: None,
        }
    }
}

/// Options for the `ai_context` function.
#[derive(Debug, Clone)]
pub struct AiOptions {
    /// Languages to analyze (empty means all supported languages).
    pub languages: Vec<String>,

    /// Maximum tokens for output.
    pub tokens: Option<usize>,

    /// Output only architectural signatures (public API surface).
    pub signatures_only: bool,

    /// Use topological ordering (dependencies before dependents).
    pub topo_order: bool,

    /// Output format.
    pub format: AiFormat,

    /// Prioritization strategy for token budgeting.
    pub priority: Priority,
}

impl Default for AiOptions {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
            tokens: None,
            signatures_only: false,
            topo_order: true,
            format: AiFormat::Markdown,
            priority: Priority::FanIn,
        }
    }
}

/// Output format for AI context.
#[derive(Debug, Clone, Copy, Default)]
pub enum AiFormat {
    #[default]
    Markdown,
    Json,
    Xml,
}

impl From<AiFormat> for AiOutputFormat {
    fn from(f: AiFormat) -> Self {
        match f {
            AiFormat::Markdown => AiOutputFormat::Markdown,
            AiFormat::Json => AiOutputFormat::Json,
            AiFormat::Xml => AiOutputFormat::Xml,
        }
    }
}

/// Prioritization strategy for AI context.
#[derive(Debug, Clone, Copy, Default)]
pub enum Priority {
    /// Prioritize modules by number of dependents (most imported first).
    #[default]
    FanIn,
    /// Prioritize modules by number of dependencies.
    FanOut,
    /// Combined score using fan-in, fan-out, and data structures.
    Combined,
}

impl From<Priority> for PriorityStrategy {
    fn from(p: Priority) -> Self {
        match p {
            Priority::FanIn => PriorityStrategy::FanIn,
            Priority::FanOut => PriorityStrategy::FanOut,
            Priority::Combined => PriorityStrategy::Combined,
        }
    }
}

/// Result of impact analysis for a file.
///
/// This struct wraps the internal `ImpactAnalysis` and provides a clean API.
pub struct ImpactResult {
    /// The underlying impact analysis data.
    inner: ImpactAnalysis,
    /// Project root for relative path calculation.
    project_root: PathBuf,
}

impl ImpactResult {
    /// The target file that was analyzed.
    pub fn target(&self) -> &Path {
        &self.inner.target
    }

    /// Total number of files affected by changes to the target.
    pub fn total_affected(&self) -> usize {
        self.inner.total_affected
    }

    /// Maximum chain length (depth) from target to farthest dependent.
    pub fn max_chain_length(&self) -> usize {
        self.inner.max_chain_length
    }

    /// Files affected, organized by dependency depth from target.
    /// Index 0 = direct dependents (depth 1), index 1 = depth 2, etc.
    pub fn affected_by_depth(&self) -> &[Vec<PathBuf>] {
        &self.inner.affected_by_depth
    }

    /// Get all affected files as a flat list.
    pub fn all_affected(&self) -> Vec<&Path> {
        self.inner
            .affected_by_depth
            .iter()
            .flatten()
            .map(|p| p.as_path())
            .collect()
    }

    /// Format the result as markdown.
    pub fn to_markdown(&self, show_tree: bool) -> String {
        analysis::format_impact_markdown(&self.inner, Some(&self.project_root), show_tree)
    }

    /// Format the result as JSON.
    pub fn to_json(&self) -> String {
        analysis::format_impact_json(&self.inner, Some(&self.project_root))
    }

    /// Access the inner ImpactAnalysis for advanced use.
    pub fn inner(&self) -> &ImpactAnalysis {
        &self.inner
    }
}

/// Run architectural analysis on a codebase.
///
/// Analyzes the given path for architectural issues like circular dependencies,
/// high coupling, god objects, boundary violations, and more.
///
/// # Arguments
///
/// * `path` - The root path of the codebase to analyze.
/// * `options` - Analysis options controlling which languages to analyze, exclusions, and thresholds.
///
/// # Returns
///
/// An `AnalysisResult` containing modules, issues, and the dependency graph.
///
/// # Example
///
/// ```no_run
/// use archmap::{analyze, AnalysisOptions};
/// use std::path::Path;
///
/// let result = analyze(Path::new("."), AnalysisOptions::default())?;
/// println!("Analyzed {} modules", result.modules.len());
/// println!("Found {} issues", result.issues.len());
/// # Ok::<(), archmap::ArchmapError>(())
/// ```
pub fn analyze(path: &Path, options: AnalysisOptions) -> Result<AnalysisResult, ArchmapError> {
    let resolved_path = path
        .canonicalize()
        .map_err(|_| ArchmapError::PathNotFound(path.to_path_buf()))?;

    let mut config = Config::load(&resolved_path).unwrap_or_default();

    // Apply options to config
    config.thresholds.max_dependency_depth = options.max_depth;
    config.thresholds.min_cohesion = options.min_cohesion;

    let registry = if options.languages.is_empty() {
        ParserRegistry::new()
    } else {
        ParserRegistry::with_languages(&options.languages)
    };

    let result = analysis::analyze(&resolved_path, &config, &registry, &options.exclude);

    Ok(result)
}

/// Analyze the change impact of a specific file.
///
/// Determines which files would be affected if the target file changes,
/// by traversing the dependency graph in reverse (finding dependents).
///
/// # Arguments
///
/// * `project_path` - The root path of the codebase.
/// * `file` - The file to analyze for change impact.
/// * `options` - Impact analysis options.
///
/// # Returns
///
/// An `ImpactResult` containing affected files organized by dependency depth.
///
/// # Example
///
/// ```no_run
/// use archmap::{impact, ImpactOptions};
/// use std::path::Path;
///
/// let result = impact(
///     Path::new("."),
///     Path::new("src/lib.rs"),
///     ImpactOptions::default()
/// )?;
/// println!("{} files would be affected", result.total_affected());
/// # Ok::<(), archmap::ArchmapError>(())
/// ```
pub fn impact(
    project_path: &Path,
    file: &Path,
    options: ImpactOptions,
) -> Result<ImpactResult, ArchmapError> {
    let resolved_path = project_path
        .canonicalize()
        .map_err(|_| ArchmapError::PathNotFound(project_path.to_path_buf()))?;

    let target_file = if file.is_absolute() {
        file.to_path_buf()
    } else {
        resolved_path.join(file)
    };

    let target_file = target_file
        .canonicalize()
        .map_err(|_| ArchmapError::PathNotFound(file.to_path_buf()))?;

    let config = Config::load(&resolved_path).unwrap_or_default();

    let registry = if options.languages.is_empty() {
        ParserRegistry::new()
    } else {
        ParserRegistry::with_languages(&options.languages)
    };

    // Run analysis to build dependency graph
    let result = analysis::analyze(&resolved_path, &config, &registry, &[]);

    // Build dependency graph
    let graph = DependencyGraph::build(&result.modules);

    // Compute impact
    let impact_analysis = analysis::compute_impact(&graph, &target_file, options.depth)?;

    Ok(ImpactResult {
        inner: impact_analysis,
        project_root: resolved_path,
    })
}

/// Generate AI-optimized context output.
///
/// Produces a compact, AI-friendly representation of the codebase architecture
/// suitable for feeding to LLMs.
///
/// # Arguments
///
/// * `path` - The root path of the codebase to analyze.
/// * `options` - AI context options controlling format, token budget, and prioritization.
///
/// # Returns
///
/// A string containing the formatted AI context.
///
/// # Example
///
/// ```no_run
/// use archmap::{ai_context, AiOptions, AiFormat};
/// use std::path::Path;
///
/// let context = ai_context(Path::new("."), AiOptions {
///     format: AiFormat::Markdown,
///     tokens: Some(4000),
///     ..Default::default()
/// })?;
/// println!("{}", context);
/// # Ok::<(), archmap::ArchmapError>(())
/// ```
pub fn ai_context(path: &Path, options: AiOptions) -> Result<String, ArchmapError> {
    let resolved_path = path
        .canonicalize()
        .map_err(|_| ArchmapError::PathNotFound(path.to_path_buf()))?;

    let config = Config::load(&resolved_path).unwrap_or_default();

    let registry = if options.languages.is_empty() {
        ParserRegistry::new()
    } else {
        ParserRegistry::with_languages(&options.languages)
    };

    // Collect source files for AI output
    let sources = collect_sources(&resolved_path, &registry);

    // Run analysis
    let result = analysis::analyze(&resolved_path, &config, &registry, &[]);

    // Build AI output formatter
    let mut formatter = AiOutput::new(Some(resolved_path))
        .with_topo_order(options.topo_order)
        .with_signatures_only(options.signatures_only)
        .with_priority(options.priority.into())
        .with_format(options.format.into())
        .with_sources(sources);

    if let Some(tokens) = options.tokens {
        formatter = formatter.with_token_budget(tokens);
    }

    // Format to string
    let mut buffer = Cursor::new(Vec::new());
    formatter.format(&result, &mut buffer)?;

    let output = String::from_utf8_lossy(&buffer.into_inner()).to_string();
    Ok(output)
}

/// Collect source files for AI context generation.
fn collect_sources(path: &Path, registry: &ParserRegistry) -> HashMap<PathBuf, String> {
    let fs = default_fs();
    let mut sources = HashMap::new();
    let walker = ignore::WalkBuilder::new(path)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        let file_path = entry.path();
        if file_path.is_file() && registry.find_parser(file_path).is_some() {
            if let Ok(content) = fs.read_to_string(file_path) {
                sources.insert(file_path.to_path_buf(), content);
            }
        }
    }

    sources
}
