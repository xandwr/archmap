use crate::define_parser;
use crate::model::{Definition, DefinitionKind, Module, Visibility};
use crate::parser::{
    LanguageParser, ParseError, extract_full_definition, extract_signature_to_brace,
};
use std::path::Path;
use tree_sitter::Node;

define_parser!(RUST_PARSER, tree_sitter_rust::LANGUAGE);

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

    /// Extract a named definition from a node (struct, enum, trait, type, const, static).
    /// Returns None if the name cannot be extracted.
    fn extract_named_definition(
        node: &Node,
        source: &str,
        source_bytes: &[u8],
        kind: DefinitionKind,
    ) -> Option<Definition> {
        let visibility = Self::get_visibility(node, source_bytes);
        let signature = extract_full_definition(node, source);
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source_bytes).ok()?;

        Some(Definition {
            name: name.to_string(),
            kind,
            line: node.start_position().row + 1,
            visibility,
            signature,
        })
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
                    let signature = extract_signature_to_brace(&node, source);

                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(source_bytes) {
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
                "struct_item" => {
                    if let Some(def) = Self::extract_named_definition(
                        &node,
                        source,
                        source_bytes,
                        DefinitionKind::Struct,
                    ) {
                        module.add_definition(def);
                    }
                }
                "enum_item" => {
                    if let Some(def) = Self::extract_named_definition(
                        &node,
                        source,
                        source_bytes,
                        DefinitionKind::Enum,
                    ) {
                        module.add_definition(def);
                    }
                }
                "trait_item" => {
                    if let Some(def) = Self::extract_named_definition(
                        &node,
                        source,
                        source_bytes,
                        DefinitionKind::Trait,
                    ) {
                        module.add_definition(def);
                    }
                }
                "impl_item" => {
                    // For impl, try to get the type being implemented
                    let signature = extract_signature_to_brace(&node, source);

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
                            // impl blocks don't have visibility, so we push directly
                            module.definitions.push(Definition {
                                name,
                                kind: DefinitionKind::Impl,
                                line: node.start_position().row + 1,
                                visibility: Visibility::Private,
                                signature,
                            });
                        }
                    }
                }
                "type_item" => {
                    if let Some(def) = Self::extract_named_definition(
                        &node,
                        source,
                        source_bytes,
                        DefinitionKind::Type,
                    ) {
                        module.add_definition(def);
                    }
                }
                "const_item" | "static_item" => {
                    if let Some(def) = Self::extract_named_definition(
                        &node,
                        source,
                        source_bytes,
                        DefinitionKind::Constant,
                    ) {
                        module.add_definition(def);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_analyze_command() {
        // Test against a file with substantial content (commands/analyze.rs has the most logic)
        let parser = RustParser::new();
        let source = std::fs::read_to_string("src/commands/analyze.rs").unwrap();
        let module = parser
            .parse_module(Path::new("src/commands/analyze.rs"), &source)
            .unwrap();

        println!("Definitions found: {}", module.definitions.len());
        for def in &module.definitions {
            println!(
                "  {:?} {:?} {} at line {}",
                def.visibility, def.kind, def.name, def.line
            );
        }

        // analyze.rs should have functions like cmd_analyze, run_analysis, run_watch_mode, etc.
        assert!(
            !module.definitions.is_empty(),
            "Should find function definitions in commands/analyze.rs"
        );

        let fn_count = module
            .definitions
            .iter()
            .filter(|d| d.kind == DefinitionKind::Function)
            .count();
        assert!(
            fn_count >= 4,
            "commands/analyze.rs should have at least 4 functions, found {}",
            fn_count
        );
    }

    #[test]
    fn test_parses_private_functions() {
        let parser = RustParser::new();
        let source = r#"
fn private_fn() {}
pub fn public_fn() {}
pub(crate) fn crate_fn() {}
"#;
        let module = parser.parse_module(Path::new("test.rs"), source).unwrap();

        assert_eq!(module.definitions.len(), 3);

        let private = module
            .definitions
            .iter()
            .find(|d| d.name == "private_fn")
            .unwrap();
        assert_eq!(private.visibility, Visibility::Private);

        let public = module
            .definitions
            .iter()
            .find(|d| d.name == "public_fn")
            .unwrap();
        assert_eq!(public.visibility, Visibility::Public);

        let crate_vis = module
            .definitions
            .iter()
            .find(|d| d.name == "crate_fn")
            .unwrap();
        assert_eq!(crate_vis.visibility, Visibility::Crate);
    }
}
