use crate::model::{Definition, DefinitionKind, Module, Visibility};
use crate::parser::{LanguageParser, ParseError};
use std::cell::RefCell;
use std::path::Path;
use tree_sitter::{Node, Parser};

thread_local! {
    static PYTHON_PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_python::LANGUAGE.into()).expect("Failed to set Python language");
        parser
    });
}

pub struct PythonParser;

impl PythonParser {
    pub fn new() -> Self {
        Self
    }

    /// In Python, names starting with _ are considered private
    fn get_visibility(name: &str) -> Visibility {
        if name.starts_with('_') {
            Visibility::Private
        } else {
            Visibility::Public
        }
    }

    /// Extract function signature (def line)
    fn extract_signature(node: &Node, source: &str) -> Option<String> {
        let start = node.start_byte();
        let end = node.end_byte();
        let text = &source[start..end];

        // Find the colon and take everything before it
        if let Some(colon_pos) = text.find(':') {
            Some(text[..colon_pos].trim().to_string())
        } else {
            text.lines().next().map(|s| s.trim().to_string())
        }
    }
}

impl LanguageParser for PythonParser {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn parse_module(&self, path: &Path, source: &str) -> Result<Module, ParseError> {
        let mut module = Module::new(path.to_path_buf());
        module.lines = source.lines().count();

        let tree = PYTHON_PARSER
            .with(|parser| parser.borrow_mut().parse(source, None))
            .ok_or_else(|| ParseError::Parse("Failed to parse file".to_string()))?;

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        // Walk the tree to extract imports and definitions
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                "import_statement" => {
                    // import foo, bar
                    let mut child_cursor = node.walk();
                    for child in node.children(&mut child_cursor) {
                        if child.kind() == "dotted_name" {
                            if let Ok(name) = child.utf8_text(source_bytes) {
                                module.imports.push(name.to_string());
                            }
                        }
                    }
                }
                "import_from_statement" => {
                    // from foo import bar
                    if let Some(module_node) = node.child_by_field_name("module_name") {
                        if let Ok(name) = module_node.utf8_text(source_bytes) {
                            module.imports.push(name.to_string());
                        }
                    }
                }
                "function_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            let visibility = Self::get_visibility(name);
                            let signature = Self::extract_signature(&node, source);

                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Function,
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            // In Python, top-level functions are typically exported
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "class_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
                            let visibility = Self::get_visibility(name);
                            let signature = Self::extract_signature(&node, source);

                            module.definitions.push(Definition {
                                name: name.to_string(),
                                kind: DefinitionKind::Class,
                                line: node.start_position().row + 1,
                                visibility,
                                signature,
                            });
                            // In Python, top-level classes are typically exported
                            if visibility == Visibility::Public {
                                module.exports.push(name.to_string());
                            }
                        }
                    }
                }
                "decorated_definition" => {
                    // Handle decorated functions/classes
                    let mut child_cursor = node.walk();
                    for child in node.children(&mut child_cursor) {
                        match child.kind() {
                            "function_definition" => {
                                if let Some(name_node) = child.child_by_field_name("name") {
                                    if let Ok(name) = name_node.utf8_text(source_bytes) {
                                        let visibility = Self::get_visibility(name);
                                        let signature = Self::extract_signature(&child, source);

                                        module.definitions.push(Definition {
                                            name: name.to_string(),
                                            kind: DefinitionKind::Function,
                                            line: child.start_position().row + 1,
                                            visibility,
                                            signature,
                                        });
                                        if visibility == Visibility::Public {
                                            module.exports.push(name.to_string());
                                        }
                                    }
                                }
                            }
                            "class_definition" => {
                                if let Some(name_node) = child.child_by_field_name("name") {
                                    if let Ok(name) = name_node.utf8_text(source_bytes) {
                                        let visibility = Self::get_visibility(name);
                                        let signature = Self::extract_signature(&child, source);

                                        module.definitions.push(Definition {
                                            name: name.to_string(),
                                            kind: DefinitionKind::Class,
                                            line: child.start_position().row + 1,
                                            visibility,
                                            signature,
                                        });
                                        if visibility == Visibility::Public {
                                            module.exports.push(name.to_string());
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(module)
    }
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new()
    }
}
