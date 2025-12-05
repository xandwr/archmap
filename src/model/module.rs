use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub path: PathBuf,
    pub name: String,
    pub lines: usize,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub definitions: Vec<Definition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub name: String,
    pub kind: DefinitionKind,
    pub line: usize,
    /// Visibility of the definition (public, private, crate-visible)
    #[serde(default)]
    pub visibility: Visibility,
    /// Full signature text (for functions, structs, etc.)
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Visibility {
    Public,
    #[default]
    Private,
    /// pub(crate) in Rust
    Crate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DefinitionKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Class,
    Interface,
    Type,
    Constant,
}

impl Module {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self {
            path,
            name,
            lines: 0,
            imports: Vec::new(),
            exports: Vec::new(),
            definitions: Vec::new(),
        }
    }

    /// Add a definition to the module, automatically updating exports if public.
    pub fn add_definition(&mut self, def: Definition) {
        if def.visibility == Visibility::Public {
            self.exports.push(def.name.clone());
        }
        self.definitions.push(def);
    }
}
