use archmap::cli::{
    AiArgs, AnalyzeArgs, Cli, Command, DiffArgs, GraphArgs, ImpactArgs, InitArgs, OutputFormat,
    SnapshotArgs,
};
use archmap::config::{Config, generate_config_template};
use archmap::fs::{FileSystem, default_fs};
use archmap::model::IssueSeverity;
use archmap::output::{JsonOutput, MarkdownOutput, OutputFormatter};
use archmap::parser::ParserRegistry;
use archmap::style;
use clap::Parser;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Some(Command::Analyze(args)) => cmd_analyze(args),
        Some(Command::Ai(args)) => cmd_ai(args),
        Some(Command::Impact(args)) => cmd_impact(args),
        Some(Command::Snapshot(args)) => cmd_snapshot(args),
        Some(Command::Diff(args)) => cmd_diff(args),
        Some(Command::Graph(args)) => cmd_graph(args),
        Some(Command::Init(args)) => cmd_init(args),
        None => {
            // Backward compatibility: treat path as analyze command
            let args = AnalyzeArgs {
                path: cli.path,
                ..Default::default()
            };
            cmd_analyze(args)
        }
    };

    std::process::exit(exit_code);
}

fn cmd_init(args: InitArgs) -> i32 {
    cmd_init_with_fs(args, default_fs())
}

fn cmd_init_with_fs(args: InitArgs, fs: &dyn FileSystem) -> i32 {
    let config_path = args.path.join(".archmap.toml");
    if fs.exists(&config_path) {
        style::error(&format!(
            ".archmap.toml already exists at {}",
            style::path(&config_path)
        ));
        return 1;
    }

    let template = generate_config_template();
    if let Err(e) = fs.write(&config_path, &template) {
        style::error(&format!("Failed to write config file: {}", e));
        return 1;
    }

    style::success(&format!(
        "Created .archmap.toml at {}",
        style::path(&config_path)
    ));
    0
}

fn cmd_analyze(args: AnalyzeArgs) -> i32 {
    // Resolve the path
    let path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!(
                "Could not resolve path: {}",
                style::path(&args.path)
            ));
            return 1;
        }
    };

    // Load config
    let config = Config::load(&path).unwrap_or_else(|e| {
        style::warning(&format!("Failed to load config: {}. Using defaults.", e));
        Config::default()
    });

    // Set up parser registry
    let registry = match &args.lang {
        Some(langs) => ParserRegistry::with_languages(langs),
        None => ParserRegistry::new(),
    };

    if args.watch {
        run_watch_mode(&path, &config, &registry, &args);
        0
    } else {
        run_analysis(&path, &config, &registry, &args)
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

    let result = archmap::analysis::analyze(path, &effective_config, registry, &args.exclude);

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

fn cmd_ai(args: AiArgs) -> i32 {
    cmd_ai_with_fs(args, default_fs())
}

fn cmd_ai_with_fs(args: AiArgs, fs: &dyn FileSystem) -> i32 {
    // Resolve the path
    let path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!(
                "Could not resolve path: {}",
                style::path(&args.path)
            ));
            return 1;
        }
    };

    // Load config
    let config = Config::load(&path).unwrap_or_else(|e| {
        style::warning(&format!("Failed to load config: {}. Using defaults.", e));
        Config::default()
    });

    // Set up parser registry
    let registry = match &args.lang {
        Some(langs) => ParserRegistry::with_languages(langs),
        None => ParserRegistry::new(),
    };

    // Collect source files for AI output
    let sources = collect_sources_with_fs(&path, &registry, fs);

    // Run analysis
    let result = archmap::analysis::analyze(&path, &config, &registry, &[]);

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

    // Build AI output formatter
    let mut formatter = archmap::output::AiOutput::new(Some(path))
        .with_topo_order(args.topo_order)
        .with_signatures_only(args.signatures)
        .with_priority(args.priority)
        .with_format(args.format)
        .with_sources(sources);

    if let Some(tokens) = args.tokens {
        formatter = formatter.with_token_budget(tokens);
    }

    if let Err(e) = formatter.format(&result, &mut output) {
        style::error(&format!("Failed to write output: {}", e));
        return 1;
    }

    0
}

fn collect_sources_with_fs(
    path: &Path,
    registry: &ParserRegistry,
    fs: &dyn FileSystem,
) -> std::collections::HashMap<std::path::PathBuf, String> {
    let mut sources = std::collections::HashMap::new();
    let walker = ignore::WalkBuilder::new(path)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        let file_path = entry.path();
        if file_path.is_file() && registry.find_parser(file_path).is_some() {
            if let Ok(content) = fs.read_to_string(file_path) {
                sources.insert(file_path.to_path_buf(), content);
            }
        }
    }

    sources
}

fn cmd_impact(args: ImpactArgs) -> i32 {
    // Resolve the project path
    let project_path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!(
                "Could not resolve path: {}",
                style::path(&args.path)
            ));
            return 1;
        }
    };

    // Resolve the target file
    let target_file = if args.file.is_absolute() {
        args.file.clone()
    } else {
        project_path.join(&args.file)
    };

    let target_file = match target_file.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!("Could not find file: {}", style::path(&args.file)));
            return 1;
        }
    };

    // Load config
    let config = Config::load(&project_path).unwrap_or_else(|e| {
        style::warning(&format!("Failed to load config: {}. Using defaults.", e));
        Config::default()
    });

    // Set up parser registry
    let registry = match &args.lang {
        Some(langs) => ParserRegistry::with_languages(langs),
        None => ParserRegistry::new(),
    };

    // Run analysis to build dependency graph
    let result = archmap::analysis::analyze(&project_path, &config, &registry, &[]);

    // Build dependency graph
    let graph = archmap::analysis::DependencyGraph::build(&result.modules);

    // Compute impact
    let impact = match archmap::analysis::compute_impact(&graph, &target_file, args.depth) {
        Ok(i) => i,
        Err(e) => {
            style::error(&format!("{}", e));
            style::hint(
                "Make sure the file is a source file recognized by archmap (e.g., .rs, .ts, .py)",
            );
            return 1;
        }
    };

    // Set up output
    let mut output: Box<dyn Write> = match &args.output {
        Some(output_path) => match default_fs().create_file(output_path) {
            Ok(writer) => writer,
            Err(e) => {
                style::error(&format!("Could not create output file: {}", e));
                return 1;
            }
        },
        None => Box::new(io::stdout()),
    };

    // Format output
    let output_str = match args.format {
        OutputFormat::Markdown => {
            archmap::analysis::format_impact_markdown(&impact, Some(&project_path), args.tree)
        }
        OutputFormat::Json => archmap::analysis::format_impact_json(&impact, Some(&project_path)),
    };

    // Render markdown nicely to terminal, or write plain text to file/pipe
    let write_result = if args.output.is_none() && args.format == OutputFormat::Markdown {
        style::render_markdown(&output_str, &mut output)
    } else {
        writeln!(output, "{}", output_str)
    };

    if let Err(e) = write_result {
        style::error(&format!("Failed to write output: {}", e));
        return 1;
    }

    0
}

fn cmd_snapshot(args: SnapshotArgs) -> i32 {
    // Resolve the project path
    let path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!(
                "Could not resolve path: {}",
                style::path(&args.path)
            ));
            return 1;
        }
    };

    // Load config
    let config = Config::load(&path).unwrap_or_else(|e| {
        style::warning(&format!("Failed to load config: {}. Using defaults.", e));
        Config::default()
    });

    // Set up parser registry
    let registry = match &args.lang {
        Some(langs) => ParserRegistry::with_languages(langs),
        None => ParserRegistry::new(),
    };

    // Run analysis
    let result = archmap::analysis::analyze(&path, &config, &registry, &[]);

    // Create snapshot
    let snapshot = archmap::snapshot::Snapshot::from_analysis(&result, &path);

    // Use the save path from args
    let output_path = &args.save;

    // Save snapshot
    if let Err(e) = archmap::snapshot::save_snapshot(&snapshot, output_path) {
        style::error(&format!("Failed to save snapshot: {}", e));
        return 1;
    }

    style::success(&format!("Snapshot saved to: {}", style::path(output_path)));
    style::section("Summary");
    println!(
        "{}",
        style::metric("Modules", snapshot.metrics.total_modules)
    );
    println!("{}", style::metric("Lines", snapshot.metrics.total_lines));
    println!(
        "{}",
        style::metric("Dependencies", snapshot.metrics.total_dependencies)
    );
    println!("{}", style::metric("Issues", snapshot.issues.len()));

    0
}

fn cmd_diff(args: DiffArgs) -> i32 {
    // Load baseline snapshot
    let baseline = match archmap::snapshot::load_snapshot(&args.baseline) {
        Ok(s) => s,
        Err(e) => {
            style::error(&format!("Failed to load baseline snapshot: {}", e));
            return 1;
        }
    };

    // Resolve the project path
    let path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!(
                "Could not resolve path: {}",
                style::path(&args.path)
            ));
            return 1;
        }
    };

    // Load config
    let config = Config::load(&path).unwrap_or_else(|e| {
        style::warning(&format!("Failed to load config: {}. Using defaults.", e));
        Config::default()
    });

    // Set up parser registry
    let registry = match &args.lang {
        Some(langs) => ParserRegistry::with_languages(langs),
        None => ParserRegistry::new(),
    };

    // Run current analysis
    let result = archmap::analysis::analyze(&path, &config, &registry, &[]);

    // Create current snapshot
    let current = archmap::snapshot::Snapshot::from_analysis(&result, &path);

    // Compute diff
    let diff = archmap::snapshot::compute_diff(&baseline, &current);

    // Set up output
    let mut output: Box<dyn Write> = match &args.output {
        Some(output_path) => match default_fs().create_file(output_path) {
            Ok(writer) => writer,
            Err(e) => {
                style::error(&format!("Could not create output file: {}", e));
                return 1;
            }
        },
        None => Box::new(io::stdout()),
    };

    // Format output
    let output_str = match args.format {
        OutputFormat::Markdown => archmap::snapshot::format_diff_markdown(&diff),
        OutputFormat::Json => archmap::snapshot::format_diff_json(&diff),
    };

    // Render markdown nicely to terminal, or write plain text to file/pipe
    let write_result = if args.output.is_none() && args.format == OutputFormat::Markdown {
        style::render_markdown(&output_str, &mut output)
    } else {
        writeln!(output, "{}", output_str)
    };

    if let Err(e) = write_result {
        style::error(&format!("Failed to write output: {}", e));
        return 1;
    }

    0
}

fn cmd_graph(args: GraphArgs) -> i32 {
    // Resolve the project path
    let path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!(
                "Could not resolve path: {}",
                style::path(&args.path)
            ));
            return 1;
        }
    };

    // Load config
    let config = Config::load(&path).unwrap_or_else(|e| {
        style::warning(&format!("Failed to load config: {}. Using defaults.", e));
        Config::default()
    });

    // Set up parser registry
    let registry = match &args.lang {
        Some(langs) => ParserRegistry::with_languages(langs),
        None => ParserRegistry::new(),
    };

    // Run analysis
    let result = archmap::analysis::analyze(&path, &config, &registry, &[]);

    // Build graph data
    let graph_data = archmap::graph::GraphData::from_analysis(&result, &path);

    if args.serve || args.watch {
        // Start web server
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        if args.watch {
            // Watch mode with live updates
            let watch_ctx = archmap::graph::WatchContext {
                path: path.clone(),
                config,
                registry,
            };
            if let Err(e) = rt.block_on(archmap::graph::serve_with_watch(
                graph_data, args.port, args.open, watch_ctx,
            )) {
                style::error(&format!("Server failed: {}", e));
                return 1;
            }
        } else {
            // Static serve mode
            if let Err(e) = rt.block_on(archmap::graph::serve(graph_data, args.port, args.open)) {
                style::error(&format!("Server failed: {}", e));
                return 1;
            }
        }
    } else if let Some(export_path) = args.export {
        // Export static HTML
        let html = archmap::graph::generate_static_html(&graph_data);
        if let Err(e) = default_fs().write(&export_path, &html) {
            style::error(&format!("Failed to write export file: {}", e));
            return 1;
        }
        style::success(&format!("Graph exported to: {}", style::path(&export_path)));
    } else {
        style::error(
            "Use --serve to start the visualization server, or --export to save static HTML",
        );
        return 1;
    }

    0
}
