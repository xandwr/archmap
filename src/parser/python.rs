use crate::define_parser;
use crate::model::{Definition, DefinitionKind, Module, Visibility};
use crate::parser::{LanguageParser, ParseError};
use std::path::Path;
use tree_sitter::Node;

define_parser!(PYTHON_PARSER, tree_sitter_python::LANGUAGE);

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

    /// Handle a function definition node, adding it to the module.
    fn handle_function(node: &Node, source_bytes: &[u8], source: &str, module: &mut Module) {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name) = name_node.utf8_text(source_bytes) {
                let visibility = Self::get_visibility(name);
                let signature = Self::extract_signature(node, source);

                module.add_definition(Definition {
                    name: name.to_string(),
                    kind: DefinitionKind::Function,
                    line: node.start_position().row + 1,
                    visibility,
                    signature,
                });
            }
        }
    }

    /// Handle a class definition node, adding it to the module.
    fn handle_class(node: &Node, source_bytes: &[u8], source: &str, module: &mut Module) {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name) = name_node.utf8_text(source_bytes) {
                let visibility = Self::get_visibility(name);
                let signature = Self::extract_signature(node, source);

                module.add_definition(Definition {
                    name: name.to_string(),
                    kind: DefinitionKind::Class,
                    line: node.start_position().row + 1,
                    visibility,
                    signature,
                });
            }
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
                    Self::handle_function(&node, source_bytes, source, &mut module);
                }
                "class_definition" => {
                    Self::handle_class(&node, source_bytes, source, &mut module);
                }
                "decorated_definition" => {
                    // Handle decorated functions/classes
                    let mut child_cursor = node.walk();
                    for child in node.children(&mut child_cursor) {
                        match child.kind() {
                            "function_definition" => {
                                Self::handle_function(&child, source_bytes, source, &mut module);
                            }
                            "class_definition" => {
                                Self::handle_class(&child, source_bytes, source, &mut module);
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
