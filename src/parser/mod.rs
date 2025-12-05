mod common;
mod python;
mod rust;
mod typescript;

use crate::model::Module;
use std::path::Path;
use thiserror::Error;

pub use common::{extract_full_definition, extract_signature_to_brace};
pub use python::PythonParser;
pub use rust::RustParser;
pub use typescript::TypeScriptParser;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse: {0}")]
    Parse(String),
    #[error("Unsupported language for file: {0}")]
    UnsupportedLanguage(String),
}

pub trait LanguageParser: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn parse_module(&self, path: &Path, source: &str) -> Result<Module, ParseError>;
}

pub struct ParserRegistry {
    parsers: Vec<Box<dyn LanguageParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: vec![
                Box::new(RustParser::new()),
                Box::new(TypeScriptParser::new()),
                Box::new(PythonParser::new()),
            ],
        }
    }

    pub fn with_languages(languages: &[String]) -> Self {
        let mut parsers: Vec<Box<dyn LanguageParser>> = Vec::new();

        for lang in languages {
            match lang.to_lowercase().as_str() {
                "rust" | "rs" => parsers.push(Box::new(RustParser::new())),
                "typescript" | "ts" | "javascript" | "js" => {
                    parsers.push(Box::new(TypeScriptParser::new()))
                }
                "python" | "py" => parsers.push(Box::new(PythonParser::new())),
                _ => {}
            }
        }

        if parsers.is_empty() {
            return Self::new();
        }

        Self { parsers }
    }

    pub fn find_parser(&self, path: &Path) -> Option<&dyn LanguageParser> {
        let ext = path.extension()?.to_str()?;
        self.parsers
            .iter()
            .find(|p| p.extensions().contains(&ext))
            .map(|p| p.as_ref())
    }

    pub fn supported_extensions(&self) -> Vec<&str> {
        self.parsers
            .iter()
            .flat_map(|p| p.extensions().iter().copied())
            .collect()
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}
