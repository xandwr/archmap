use archmap::cli::{Cli, OutputFormat};
use archmap::config::{Config, generate_config_template};
use archmap::model::IssueSeverity;
use archmap::output::{JsonOutput, MarkdownOutput, OutputFormatter};
use archmap::parser::ParserRegistry;
use clap::Parser;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::Duration;

fn main() {
    let cli = Cli::parse();

    // Handle --init command
    if cli.init {
        let config_path = cli.path.join(".archmap.toml");
        if config_path.exists() {
            eprintln!(
                "Error: .archmap.toml already exists at {}",
                config_path.display()
            );
            std::process::exit(1);
        }

        let template = generate_config_template();
        if let Err(e) = std::fs::write(&config_path, template) {
            eprintln!("Error: Failed to write config file: {}", e);
            std::process::exit(1);
        }

        println!("Created .archmap.toml at {}", config_path.display());
        return;
    }

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

    if cli.watch {
        run_watch_mode(&path, &config, &registry, &cli);
    } else {
        let exit_code = run_analysis(&path, &config, &registry, &cli);
        std::process::exit(exit_code);
    }
}

fn run_analysis(path: &Path, config: &Config, registry: &ParserRegistry, cli: &Cli) -> i32 {
    // Run analysis with CLI overrides for thresholds
    let mut effective_config = config.clone();
    effective_config.thresholds.max_dependency_depth = cli.max_depth;
    effective_config.thresholds.min_cohesion = cli.min_cohesion;

    let result = archmap::analysis::analyze(path, &effective_config, registry);

    // Set up output
    let mut output: Box<dyn Write> = match &cli.output {
        Some(output_path) => {
            let file = File::create(output_path).unwrap_or_else(|e| {
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
            let formatter = MarkdownOutput::new(cli.min_severity, Some(path.to_path_buf()));
            formatter.format(&result, &mut output)
        }
        OutputFormat::Json => {
            let formatter = JsonOutput::new(Some(path.to_path_buf()));
            formatter.format(&result, &mut output)
        }
    };

    if let Err(e) = format_result {
        eprintln!("Error: Failed to write output: {}", e);
        return 1;
    }

    // Calculate exit code based on severity threshold
    let has_issues_above_threshold = result
        .issues
        .iter()
        .any(|issue| issue.severity >= cli.min_severity);

    // Return non-zero if issues found at or above min_severity
    // Exit code 0 = no issues, 1 = has warnings, 2 = has errors
    if has_issues_above_threshold {
        let max_severity = result
            .issues
            .iter()
            .filter(|i| i.severity >= cli.min_severity)
            .map(|i| i.severity)
            .max()
            .unwrap_or(IssueSeverity::Info);

        match max_severity {
            IssueSeverity::Error => 2,
            IssueSeverity::Warn => 1,
            IssueSeverity::Info => 0,
        }
    } else {
        0
    }
}

fn run_watch_mode(path: &Path, config: &Config, registry: &ParserRegistry, cli: &Cli) {
    use std::collections::HashMap;
    use std::fs;

    println!(
        "Watching {} for changes (Ctrl+C to stop)...\n",
        path.display()
    );

    // Initial scan
    fn scan_files(path: &Path) -> HashMap<std::path::PathBuf, std::time::SystemTime> {
        let mut files = HashMap::new();
        let walker = ignore::WalkBuilder::new(path)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            let file_path = entry.path();
            if file_path.is_file() {
                if let Ok(metadata) = fs::metadata(file_path) {
                    if let Ok(modified) = metadata.modified() {
                        files.insert(file_path.to_path_buf(), modified);
                    }
                }
            }
        }
        files
    }

    let mut last_modified = scan_files(path);

    // Run initial analysis
    println!("=== Initial Analysis ===");
    let _ = run_analysis(path, config, registry, cli);
    println!();

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let current_files = scan_files(path);
        let mut changed = false;

        // Check for new or modified files
        for (file_path, modified) in &current_files {
            match last_modified.get(file_path) {
                Some(last) if last != modified => {
                    println!(
                        "Changed: {}",
                        file_path.strip_prefix(path).unwrap_or(file_path).display()
                    );
                    changed = true;
                }
                None => {
                    println!(
                        "Added: {}",
                        file_path.strip_prefix(path).unwrap_or(file_path).display()
                    );
                    changed = true;
                }
                _ => {}
            }
        }

        // Check for deleted files
        for file_path in last_modified.keys() {
            if !current_files.contains_key(file_path) {
                println!(
                    "Deleted: {}",
                    file_path.strip_prefix(path).unwrap_or(file_path).display()
                );
                changed = true;
            }
        }

        if changed {
            println!("\n=== Re-analyzing ===");
            let _ = run_analysis(path, config, registry, cli);
            println!();
            last_modified = current_files;
        }
    }
}
