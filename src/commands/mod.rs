mod ai;
mod analyze;
mod diff;
mod graph;
mod impact;
mod init;
mod snapshot;

pub use ai::cmd_ai;
pub use analyze::cmd_analyze;
pub use diff::cmd_diff;
pub use graph::cmd_graph;
pub use impact::cmd_impact;
pub use init::cmd_init;
pub use snapshot::cmd_snapshot;

use crate::config::Config;
use crate::parser::ParserRegistry;
use crate::style;
use std::path::{Path, PathBuf};

/// Shared context for command execution, reducing boilerplate across commands.
pub struct CommandContext {
    pub path: PathBuf,
    pub config: Config,
    pub registry: ParserRegistry,
}

impl CommandContext {
    /// Create a new command context by resolving the path, loading config, and setting up parsers.
    /// Returns Err(exit_code) if setup fails.
    pub fn new(path: &Path, lang: Option<&[String]>) -> Result<Self, i32> {
        let resolved_path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                style::error(&format!("Could not resolve path: {}", style::path(path)));
                return Err(1);
            }
        };

        let config = Config::load(&resolved_path).unwrap_or_else(|e| {
            style::warning(&format!("Failed to load config: {}. Using defaults.", e));
            Config::default()
        });

        let registry = match lang {
            Some(langs) => ParserRegistry::with_languages(langs),
            None => ParserRegistry::new(),
        };

        Ok(Self {
            path: resolved_path,
            config,
            registry,
        })
    }
}
