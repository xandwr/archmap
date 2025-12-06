pub mod analysis;
pub mod api;
pub mod cli;
pub mod commands;
pub mod config;
pub mod fs;
pub mod graph;
pub mod model;
pub mod output;
pub mod parser;
pub mod snapshot;
pub mod style;

// =============================================================================
// Library API (for use as a Rust dependency)
// =============================================================================

// Core library functions
pub use api::{ai_context, analyze, impact};

// Options types for library functions
pub use api::{
    AiFormat, AiOptions, AnalysisOptions, ArchmapError, ImpactOptions, ImpactResult, Priority,
};

// Core model types
pub use model::{
    AnalysisResult, Definition, DefinitionKind, Issue, IssueKind, IssueSeverity, Location, Module,
    Visibility,
};

// Configuration
pub use config::Config;

// Re-export ImpactAnalysis for advanced use cases
pub use analysis::ImpactAnalysis;

// =============================================================================
// CLI API (for building CLI tools)
// =============================================================================

pub use cli::Cli;
pub use commands::{
    cmd_ai, cmd_analyze, cmd_diff, cmd_graph, cmd_impact, cmd_init, cmd_mcp, cmd_snapshot,
};
