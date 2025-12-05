use crate::model::{Definition, DefinitionKind, Module};
use crate::parser::{LanguageParser, ParseError};
use std::path::Path;
use tree_sitter::Parser;

pub struct RustParser;

impl RustParser {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageParser for RustParser {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn parse_module(&self, path: &Path, source: &str) -> Result<Module, ParseError> {
        let mut module = Module::new(path.to_path_buf());
        module.lines = source.lines().count();

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to set Rust language");

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| ParseError::Parse("Failed to parse file".to_string()))?;

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        // Walk the tree to extract imports and definitions
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                "use_declaration" => {
                    if let Ok(text) = node.utf8_text(source_bytes) {
                        let import = text
                            .trim_start_matches("use ")
                            .trim_end_matches(';')
                            .trim()
                            .to_string();
                        module.imports.push(import);
                    }
                }
                "function_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Function,
                                line: node.start_position().row + 1,
                            });
                            module.exports.push(name.to_string());
                        }
                    }
                }
                "struct_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Struct,
                                line: node.start_position().row + 1,
                            });
                            module.exports.push(name.to_string());
                        }
                    }
                }
                "enum_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Enum,
                                line: node.start_position().row + 1,
                            });
                            module.exports.push(name.to_string());
                        }
                    }
                }
                "trait_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Trait,
                                line: node.start_position().row + 1,
                            });
                            module.exports.push(name.to_string());
                        }
                    }
                }
                "impl_item" => {
                    // For impl, try to get the type being implemented
                    if let Ok(impl_text) = node.utf8_text(source_bytes) {
                        let name = impl_text
                            .lines()
                            .next()
                            .unwrap_or("impl")
                            .trim_start_matches("impl")
                            .split('{')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        if !name.is_empty() {
                            module.definitions.push(Definition {
                                name,
                                kind: DefinitionKind::Impl,
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                }
                "mod_item" => {
                    // Handle mod declarations for nested modules
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.exports.push(name.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(module)
    }
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}
