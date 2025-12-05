use crate::model::{Definition, DefinitionKind, Module, Visibility};
use crate::parser::{LanguageParser, ParseError};
use std::cell::RefCell;
use std::path::Path;
use tree_sitter::{Node, Parser};

thread_local! {
    static TS_PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()).expect("Failed to set TS language");
        parser
    });
    static TSX_PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into()).expect("Failed to set TSX language");
        parser
    });
}

pub struct TypeScriptParser;

impl TypeScriptParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract signature from node up to opening brace
    fn extract_signature(node: &Node, source: &str) -> Option<String> {
        let start = node.start_byte();
        let end = node.end_byte();
        let text = &source[start..end];

        if let Some(brace_pos) = text.find('{') {
            Some(text[..brace_pos].trim().to_string())
        } else {
            text.lines().next().map(|s| s.trim().to_string())
        }
    }

    /// Extract full definition
    fn extract_full_definition(node: &Node, source: &str) -> Option<String> {
        let start = node.start_byte();
        let end = node.end_byte();
        Some(source[start..end].to_string())
    }
}

impl LanguageParser for TypeScriptParser {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx"]
    }

    fn parse_module(&self, path: &Path, source: &str) -> Result<Module, ParseError> {
        let mut module = Module::new(path.to_path_buf());
        module.lines = source.lines().count();

        // Use TSX parser for .tsx files, TS parser for everything else
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let tree = if ext == "tsx" {
            TSX_PARSER.with(|parser| parser.borrow_mut().parse(source, None))
        } else {
            TS_PARSER.with(|parser| parser.borrow_mut().parse(source, None))
        }
        .ok_or_else(|| ParseError::Parse("Failed to parse file".to_string()))?;

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        // Walk the tree to extract imports and definitions
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                "import_statement" => {
                    if let Ok(text) = node.utf8_text(source_bytes) {
                        // Extract the import path
                        let import = extract_import_path(text);
                        if !import.is_empty() {
                            module.imports.push(import);
                        }
                    }
                }
                "export_statement" => {
                    // Handle export declarations - these are public
                    let mut child_cursor = node.walk();
                    for child in node.children(&mut child_cursor) {
                        extract_definition(&child, source_bytes, source, &mut module, true);
                    }
                }
                "function_declaration" => {
                    let signature = Self::extract_signature(&node, source);
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Function,
                                line: node.start_position().row + 1,
                                visibility: Visibility::Private, // Not exported
                                signature,
                            });
                        }
                    }
                }
                "class_declaration" => {
                    let signature = Self::extract_full_definition(&node, source);
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Class,
                                line: node.start_position().row + 1,
                                visibility: Visibility::Private,
                                signature,
                            });
                        }
                    }
                }
                "interface_declaration" => {
                    let signature = Self::extract_full_definition(&node, source);
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Interface,
                                line: node.start_position().row + 1,
                                visibility: Visibility::Private,
                                signature,
                            });
                        }
                    }
                }
                "type_alias_declaration" => {
                    let signature = Self::extract_full_definition(&node, source);
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Type,
                                line: node.start_position().row + 1,
                                visibility: Visibility::Private,
                                signature,
                            });
                        }
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    // Handle const/let/var declarations
                    let mut child_cursor = node.walk();
                    for child in node.children(&mut child_cursor) {
                        if child.kind() == "variable_declarator" {
                            if let Some(name_node) = child.child_by_field_name("name") {
                                if let Ok(name) = name_node.utf8_text(source_bytes) {
                                    let signature = Self::extract_full_definition(&node, source);
                                    module.definitions.push(Definition {
                                        name: name.to_string(),
                                        kind: DefinitionKind::Function, // Could be a const function
                                        line: node.start_position().row + 1,
                                        visibility: Visibility::Private,
                                        signature,
                                    });
                                }
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

fn extract_import_path(import_text: &str) -> String {
    // Extract path from: import ... from "path" or import "path"
    if let Some(start) = import_text.find('"').or_else(|| import_text.find('\'')) {
        let rest = &import_text[start + 1..];
        if let Some(end) = rest.find('"').or_else(|| rest.find('\'')) {
            return rest[..end].to_string();
        }
    }
    String::new()
}

fn extract_definition(
    node: &Node,
    source_bytes: &[u8],
    source: &str,
    module: &mut Module,
    is_exported: bool,
) {
    let visibility = if is_exported {
        Visibility::Public
    } else {
        Visibility::Private
    };

    match node.kind() {
        "function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    let signature = TypeScriptParser::extract_signature(node, source);
                    module.definitions.push(Definition {
                        name: name.to_string(),
                        kind: DefinitionKind::Function,
                        line: node.start_position().row + 1,
                        visibility,
                        signature,
                    });
                    if is_exported {
                        module.exports.push(name.to_string());
                    }
                }
            }
        }
        "class_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    let signature = TypeScriptParser::extract_full_definition(node, source);
                    module.definitions.push(Definition {
                        name: name.to_string(),
                        kind: DefinitionKind::Class,
                        line: node.start_position().row + 1,
                        visibility,
                        signature,
                    });
                    if is_exported {
                        module.exports.push(name.to_string());
                    }
                }
            }
        }
        "interface_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    let signature = TypeScriptParser::extract_full_definition(node, source);
                    module.definitions.push(Definition {
                        name: name.to_string(),
                        kind: DefinitionKind::Interface,
                        line: node.start_position().row + 1,
                        visibility,
                        signature,
                    });
                    if is_exported {
                        module.exports.push(name.to_string());
                    }
                }
            }
        }
        "type_alias_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_bytes) {
                    let signature = TypeScriptParser::extract_full_definition(node, source);
                    module.definitions.push(Definition {
                        name: name.to_string(),
                        kind: DefinitionKind::Type,
                        line: node.start_position().row + 1,
                        visibility,
                        signature,
                    });
                    if is_exported {
                        module.exports.push(name.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}
