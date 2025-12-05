use crate::model::{Definition, DefinitionKind, Module, Visibility};
use crate::parser::{LanguageParser, ParseError};
use std::cell::RefCell;
use std::path::Path;
use tree_sitter::{Node, Parser};

thread_local! {
    static RUST_PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("Failed to set Rust language");
        parser
    });
}

pub struct RustParser;

impl RustParser {
    pub fn new() -> Self {
        Self
    }

    /// Check if a node has a visibility modifier (pub, pub(crate), etc.)
    fn get_visibility(node: &Node, source_bytes: &[u8]) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                if let Ok(text) = child.utf8_text(source_bytes) {
                    if text.contains("crate") {
                        return Visibility::Crate;
                    } else if text.starts_with("pub") {
                        return Visibility::Public;
                    }
                }
            }
        }
        Visibility::Private
    }

    /// Extract signature from a node up to the opening brace
    fn extract_signature(node: &Node, source: &str) -> Option<String> {
        let start = node.start_byte();
        let end = node.end_byte();
        let text = &source[start..end];

        // Find the opening brace and truncate there
        if let Some(brace_pos) = text.find('{') {
            let sig = text[..brace_pos].trim();
            Some(sig.to_string())
        } else {
            // No brace (e.g., unit struct or semicolon-terminated)
            let sig = text.trim_end_matches(';').trim();
            Some(sig.to_string())
        }
    }

    /// Extract full definition including body (for structs, enums)
    fn extract_full_definition(node: &Node, source: &str) -> Option<String> {
        let start = node.start_byte();
        let end = node.end_byte();
        Some(source[start..end].to_string())
    }
}

impl LanguageParser for RustParser {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn parse_module(&self, path: &Path, source: &str) -> Result<Module, ParseError> {
        let mut module = Module::new(path.to_path_buf());
        module.lines = source.lines().count();

        let tree = RUST_PARSER
            .with(|parser| parser.borrow_mut().parse(source, None))
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
                    let visibility = Self::get_visibility(&node, source_bytes);
                    let signature = Self::extract_signature(&node, source);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Function,
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "struct_item" => {
                    let visibility = Self::get_visibility(&node, source_bytes);
                    let signature = Self::extract_full_definition(&node, source);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Struct,
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "enum_item" => {
                    let visibility = Self::get_visibility(&node, source_bytes);
                    let signature = Self::extract_full_definition(&node, source);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Enum,
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "trait_item" => {
                    let visibility = Self::get_visibility(&node, source_bytes);
                    let signature = Self::extract_full_definition(&node, source);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Trait,
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "impl_item" => {
                    // For impl, try to get the type being implemented
                    let signature = Self::extract_signature(&node, source);

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
                                visibility: Visibility::Private, // impl blocks don't have visibility
                                signature,
                            });
                        }
                    }
                }
                "type_item" => {
                    let visibility = Self::get_visibility(&node, source_bytes);
                    let signature = Self::extract_full_definition(&node, source);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Type,
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "const_item" | "static_item" => {
                    let visibility = Self::get_visibility(&node, source_bytes);
                    let signature = Self::extract_full_definition(&node, source);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Type, // Using Type for constants
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "mod_item" => {
                    // Handle mod declarations for nested modules
                    let visibility = Self::get_visibility(&node, source_bytes);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
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
