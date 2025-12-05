//! Terminal styling utilities for consistent CLI output

use colored::Colorize;
use std::io::{self, IsTerminal, Write};

/// Print an error message to stderr
pub fn error(msg: &str) {
    eprintln!("{} {}", "error:".red().bold(), msg);
}

/// Print a warning message to stderr
pub fn warning(msg: &str) {
    eprintln!("{} {}", "warning:".yellow().bold(), msg);
}

/// Print a success message to stdout
pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

/// Print a hint message to stderr (dimmed)
pub fn hint(msg: &str) {
    eprintln!("{} {}", "hint:".dimmed(), msg.dimmed());
}

/// Print a status/info header
pub fn header(msg: &str) {
    println!("{}", msg.cyan().bold());
}

/// Print a status update (for watch mode, etc.)
pub fn status(msg: &str) {
    println!("{} {}", "→".blue(), msg);
}

/// Format a path for display (bright white)
pub fn path(p: &std::path::Path) -> String {
    p.display().to_string().bright_white().to_string()
}

/// Format a file change type with appropriate color
pub fn file_changed(path_str: &str) -> String {
    format!("{} {}", "modified:".yellow(), path_str)
}

pub fn file_added(path_str: &str) -> String {
    format!("{} {}", "added:".green(), path_str)
}

pub fn file_deleted(path_str: &str) -> String {
    format!("{} {}", "deleted:".red(), path_str)
}

/// Format a label-value pair for metrics display
pub fn metric(label: &str, value: impl std::fmt::Display) -> String {
    format!("  {}: {}", label.dimmed(), value.to_string().cyan())
}

/// Format a section header (for summaries, etc.)
pub fn section(title: &str) {
    println!("\n{}", title.bold());
}

/// Format a URL for display
pub fn url(u: &str) -> String {
    u.bright_blue().underline().to_string()
}

/// Check if stdout is a terminal (TTY)
pub fn is_terminal() -> bool {
    io::stdout().is_terminal()
}

/// Render markdown to the terminal with colors and formatting.
/// If not a TTY, writes plain markdown.
pub fn render_markdown(markdown: &str, output: &mut dyn Write) -> io::Result<()> {
    if io::stdout().is_terminal() {
        // Use termimad for beautiful terminal rendering
        let skin = create_skin();
        let rendered = skin.term_text(markdown);
        write!(output, "{}", rendered)
    } else {
        // Plain markdown for files/pipes
        write!(output, "{}", markdown)
    }
}

/// Render markdown to terminal, with explicit TTY flag
pub fn render_markdown_to_terminal(markdown: &str) {
    let skin = create_skin();
    let rendered = skin.term_text(markdown);
    print!("{}", rendered);
}

/// Create a custom termimad skin with our color scheme
fn create_skin() -> termimad::MadSkin {
    use termimad::*;

    let mut skin = MadSkin::default();

    // Headers - cyan and bold
    skin.set_headers_fg(crossterm::style::Color::Cyan);
    skin.bold.set_fg(crossterm::style::Color::White);

    // Bullet points
    skin.bullet = StyledChar::from_fg_char(crossterm::style::Color::Blue, '•');

    // Code blocks - use a subtle background
    skin.code_block.set_fg(crossterm::style::Color::Yellow);

    // Inline code
    skin.inline_code.set_fg(crossterm::style::Color::Yellow);

    // Bold text
    skin.bold.set_fg(crossterm::style::Color::White);

    // Italic (used for paths, etc.)
    skin.italic.set_fg(crossterm::style::Color::Magenta);

    // Horizontal rules
    skin.horizontal_rule = StyledChar::from_fg_char(crossterm::style::Color::DarkGrey, '─');

    skin
}
