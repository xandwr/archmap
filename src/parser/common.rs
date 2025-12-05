use tree_sitter::Node;

/// Extract signature from a node up to the opening brace.
/// Used by Rust and TypeScript parsers.
pub fn extract_signature_to_brace(node: &Node, source: &str) -> Option<String> {
    let start = node.start_byte();
    let end = node.end_byte();
    let text = &source[start..end];

    if let Some(brace_pos) = text.find('{') {
        Some(text[..brace_pos].trim().to_string())
    } else {
        // No brace (e.g., unit struct, semicolon-terminated, or single-line)
        let sig = text.trim_end_matches(';').trim();
        Some(sig.to_string())
    }
}

/// Extract full definition text from a node.
/// Used by parsers for structs, enums, interfaces, etc.
pub fn extract_full_definition(node: &Node, source: &str) -> Option<String> {
    let start = node.start_byte();
    let end = node.end_byte();
    Some(source[start..end].to_string())
}

/// Macro to define a thread-local parser with a given language.
/// Usage: `define_parser!(PARSER_NAME, language_fn)`
#[macro_export]
macro_rules! define_parser {
    ($name:ident, $language:expr) => {
        thread_local! {
            static $name: std::cell::RefCell<tree_sitter::Parser> = std::cell::RefCell::new({
                let mut parser = tree_sitter::Parser::new();
                parser.set_language(&$language.into()).expect(concat!("Failed to set ", stringify!($name), " language"));
                parser
            });
        }
    };
}
