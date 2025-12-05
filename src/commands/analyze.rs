use crate::cli::{AnalyzeArgs, OutputFormat};
use crate::config::Config;
use crate::fs::{FileSystem, default_fs};
use crate::model::IssueSeverity;
use crate::output::{JsonOutput, MarkdownOutput, OutputFormatter};
use crate::parser::ParserRegistry;
use crate::style;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use super::CommandContext;

pub fn cmd_analyze(args: AnalyzeArgs) -> i32 {
    let ctx = match CommandContext::new(&args.path, args.lang.as_deref()) {
        Ok(ctx) => ctx,
        Err(code) => return code,
    };

    if args.watch {
        run_watch_mode(&ctx.path, &ctx.config, &ctx.registry, &args);
        0
    } else {
        run_analysis(&ctx.path, &ctx.config, &ctx.registry, &args)
    }
}

fn run_analysis(
    path: &Path,
    config: &Config,
    registry: &ParserRegistry,
    args: &AnalyzeArgs,
) -> i32 {
    run_analysis_with_fs(path, config, registry, args, default_fs())
}

fn run_analysis_with_fs(
    path: &Path,
    config: &Config,
    registry: &ParserRegistry,
    args: &AnalyzeArgs,
    fs: &dyn FileSystem,
) -> i32 {
    // Run analysis with CLI overrides for thresholds
    let mut effective_config = config.clone();
    effective_config.thresholds.max_dependency_depth = args.max_depth;
    effective_config.thresholds.min_cohesion = args.min_cohesion;

    let result = crate::analysis::analyze(path, &effective_config, registry, &args.exclude);

    // Set up output
    let mut output: Box<dyn Write> = match &args.output {
        Some(output_path) => match fs.create_file(output_path) {
            Ok(writer) => writer,
            Err(e) => {
                style::error(&format!("Could not create output file: {}", e));
                return 1;
            }
        },
        None => Box::new(io::stdout()),
    };

    // Format output to string first
    let mut buffer = Vec::new();
    let format_result = match args.format {
        OutputFormat::Markdown => {
            let formatter = MarkdownOutput::new(args.min_severity, Some(path.to_path_buf()));
            formatter.format(&result, &mut buffer)
        }
        OutputFormat::Json => {
            let formatter = JsonOutput::new(Some(path.to_path_buf()));
            formatter.format(&result, &mut buffer)
        }
    };

    if let Err(e) = format_result {
        style::error(&format!("Failed to format output: {}", e));
        return 1;
    }

    let output_str = String::from_utf8_lossy(&buffer);

    // Render markdown nicely to terminal, or write plain text to file/pipe
    let write_result = if args.output.is_none() && args.format == OutputFormat::Markdown {
        style::render_markdown(&output_str, &mut output)
    } else {
        write!(output, "{}", output_str)
    };

    if let Err(e) = write_result {
        style::error(&format!("Failed to write output: {}", e));
        return 1;
    }

    // Exit code 0 = ran successfully (with or without warnings/info)
    // Exit code 1 = has errors (architectural violations that should block CI)
    // This allows using archmap in CI pipelines where warnings are informational
    let has_errors = result
        .issues
        .iter()
        .any(|issue| issue.severity == IssueSeverity::Error);

    if has_errors { 1 } else { 0 }
}

fn run_watch_mode(path: &Path, config: &Config, registry: &ParserRegistry, args: &AnalyzeArgs) {
    run_watch_mode_with_fs(path, config, registry, args, default_fs())
}

fn run_watch_mode_with_fs(
    path: &Path,
    config: &Config,
    registry: &ParserRegistry,
    args: &AnalyzeArgs,
    fs: &dyn FileSystem,
) {
    use std::collections::HashMap;

    style::status(&format!(
        "Watching {} for changes (Ctrl+C to stop)...",
        style::path(path)
    ));
    println!();

    // Initial scan using FileSystem abstraction
    fn scan_files(
        path: &Path,
        fs: &dyn FileSystem,
    ) -> HashMap<std::path::PathBuf, std::time::SystemTime> {
        let mut files = HashMap::new();
        let walker = ignore::WalkBuilder::new(path)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            let file_path = entry.path();
            if file_path.is_file() {
                if let Ok(modified) = fs.modified(file_path) {
                    files.insert(file_path.to_path_buf(), modified);
                }
            }
        }
        files
    }

    let mut last_modified = scan_files(path, fs);

    // Run initial analysis
    style::header("=== Initial Analysis ===");
    let _ = run_analysis_with_fs(path, config, registry, args, fs);
    println!();

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let current_files = scan_files(path, fs);
        let mut changed = false;

        // Check for new or modified files
        for (file_path, modified) in &current_files {
            let display_path = file_path
                .strip_prefix(path)
                .unwrap_or(file_path)
                .display()
                .to_string();
            match last_modified.get(file_path) {
                Some(last) if last != modified => {
                    println!("{}", style::file_changed(&display_path));
                    changed = true;
                }
                None => {
                    println!("{}", style::file_added(&display_path));
                    changed = true;
                }
                _ => {}
            }
        }

        // Check for deleted files
        for file_path in last_modified.keys() {
            if !current_files.contains_key(file_path) {
                let display_path = file_path
                    .strip_prefix(path)
                    .unwrap_or(file_path)
                    .display()
                    .to_string();
                println!("{}", style::file_deleted(&display_path));
                changed = true;
            }
        }

        if changed {
            println!();
            style::header("=== Re-analyzing ===");
            let _ = run_analysis_with_fs(path, config, registry, args, fs);
            println!();
            last_modified = current_files;
        }
    }
}
