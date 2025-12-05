use archmap::cli::{Cli, OutputFormat};
use archmap::config::Config;
use archmap::output::{JsonOutput, MarkdownOutput, OutputFormatter};
use archmap::parser::ParserRegistry;
use clap::Parser;
use std::fs::File;
use std::io::{self, BufWriter, Write};

fn main() {
    let cli = Cli::parse();

    // Resolve the path
    let path = cli.path.canonicalize().unwrap_or_else(|_| {
        eprintln!("Error: Could not resolve path: {}", cli.path.display());
        std::process::exit(1);
    });

    // Load config
    let config = Config::load(&path).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {}. Using defaults.", e);
        Config::default()
    });

    // Set up parser registry
    let registry = match &cli.lang {
        Some(langs) => ParserRegistry::with_languages(langs),
        None => ParserRegistry::new(),
    };

    // Run analysis
    let result = archmap::analysis::analyze(&path, &config, &registry);

    // Set up output
    let mut output: Box<dyn Write> = match &cli.output {
        Some(path) => {
            let file = File::create(path).unwrap_or_else(|e| {
                eprintln!("Error: Could not create output file: {}", e);
                std::process::exit(1);
            });
            Box::new(BufWriter::new(file))
        }
        None => Box::new(io::stdout()),
    };

    // Format and write output
    let format_result = match cli.format {
        OutputFormat::Markdown => {
            let formatter = MarkdownOutput::new(cli.min_severity);
            formatter.format(&result, &mut output)
        }
        OutputFormat::Json => {
            let formatter = JsonOutput::new();
            formatter.format(&result, &mut output)
        }
    };

    if let Err(e) = format_result {
        eprintln!("Error: Failed to write output: {}", e);
        std::process::exit(1);
    }
}
