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
use std::fs::File;
use std::io::{self, BufWriter, Write};
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
    // Run analysis with CLI overrides for thresholds
    let mut effective_config = config.clone();
    effective_config.thresholds.max_dependency_depth = args.max_depth;
    effective_config.thresholds.min_cohesion = args.min_cohesion;

    let result = archmap::analysis::analyze(path, &effective_config, registry, &args.exclude);

    // Set up output
    let mut output: Box<dyn Write> = match &args.output {
        Some(output_path) => {
            let file = match File::create(output_path) {
                Ok(f) => f,
                Err(e) => {
                    style::error(&format!("Could not create output file: {}", e));
                    return 1;
                }
            };
            Box::new(BufWriter::new(file))
        }
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
    use std::collections::HashMap;
    use std::fs;

    style::status(&format!(
        "Watching {} for changes (Ctrl+C to stop)...",
        style::path(path)
    ));
    println!();

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
    style::header("=== Initial Analysis ===");
    let _ = run_analysis(path, config, registry, args);
    println!();

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let current_files = scan_files(path);
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
            let _ = run_analysis(path, config, registry, args);
            println!();
            last_modified = current_files;
        }
    }
}

fn cmd_ai(args: AiArgs) -> i32 {
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
    let sources = collect_sources(&path, &registry);

    // Run analysis
    let result = archmap::analysis::analyze(&path, &config, &registry, &[]);

    // Set up output
    let mut output: Box<dyn Write> = match &args.output {
        Some(output_path) => {
            let file = match File::create(output_path) {
                Ok(f) => f,
                Err(e) => {
                    style::error(&format!("Could not create output file: {}", e));
                    return 1;
                }
            };
            Box::new(BufWriter::new(file))
        }
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

fn collect_sources(
    path: &Path,
    registry: &ParserRegistry,
) -> std::collections::HashMap<std::path::PathBuf, String> {
    collect_sources_with_fs(path, registry, default_fs())
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
        Some(output_path) => {
            let file = match File::create(output_path) {
                Ok(f) => f,
                Err(e) => {
                    style::error(&format!("Could not create output file: {}", e));
                    return 1;
                }
            };
            Box::new(BufWriter::new(file))
        }
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
        Some(output_path) => {
            let file = match File::create(output_path) {
                Ok(f) => f,
                Err(e) => {
                    style::error(&format!("Could not create output file: {}", e));
                    return 1;
                }
            };
            Box::new(BufWriter::new(file))
        }
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
        let html = generate_static_html(&graph_data);
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

fn generate_static_html(graph_data: &archmap::graph::GraphData) -> String {
    let json_data = serde_json::to_string(graph_data).unwrap_or_else(|_| "{}".to_string());

    // Generate a standalone HTML file with embedded data
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Archmap - Dependency Graph</title>
    <script src="https://d3js.org/d3.v7.min.js"></script>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #1a1a2e; color: #eee; overflow: hidden; }}
        #container {{ display: flex; height: 100vh; }}
        #graph {{ flex: 1; background: #16213e; }}
        #sidebar {{ width: 320px; background: #1a1a2e; border-left: 1px solid #333; padding: 20px; overflow-y: auto; }}
        h1 {{ font-size: 1.4em; margin-bottom: 10px; color: #00d9ff; }}
        h2 {{ font-size: 1.1em; margin: 15px 0 10px; color: #888; text-transform: uppercase; letter-spacing: 1px; }}
        .stat {{ display: flex; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid #333; }}
        .stat-value {{ color: #00d9ff; font-weight: bold; }}
        #node-info {{ display: none; margin-top: 20px; padding: 15px; background: #16213e; border-radius: 8px; }}
        #node-info.visible {{ display: block; }}
        #node-info h3 {{ color: #00d9ff; margin-bottom: 10px; word-break: break-all; }}
        .node-stat {{ display: flex; justify-content: space-between; padding: 5px 0; font-size: 0.9em; }}
        .exports-list {{ margin-top: 10px; font-size: 0.85em; }}
        .exports-list span {{ display: inline-block; background: #333; padding: 2px 8px; border-radius: 4px; margin: 2px; }}
        .legend {{ display: flex; flex-wrap: wrap; gap: 10px; margin-top: 15px; }}
        .legend-item {{ display: flex; align-items: center; gap: 5px; font-size: 0.85em; }}
        .legend-color {{ width: 12px; height: 12px; border-radius: 50%; }}
        .node {{ cursor: pointer; }}
        .node circle {{ stroke: #fff; stroke-width: 1.5px; }}
        .node text {{ font-size: 10px; fill: #fff; pointer-events: none; }}
        .node.highlighted circle {{ stroke: #00d9ff; stroke-width: 3px; }}
        .link {{ stroke: #555; stroke-opacity: 0.6; }}
        .link.cycle {{ stroke: #ff4444; stroke-width: 2px; stroke-dasharray: 5, 5; }}
        .link.highlighted {{ stroke: #00d9ff; stroke-opacity: 1; }}
        .tooltip {{ position: absolute; background: rgba(0, 0, 0, 0.9); color: #fff; padding: 10px; border-radius: 6px; font-size: 12px; pointer-events: none; max-width: 250px; z-index: 1000; }}
    </style>
</head>
<body>
    <div id="container">
        <div id="graph"></div>
        <div id="sidebar">
            <h1>Archmap</h1>
            <div id="project-name"></div>
            <h2>Summary</h2>
            <div id="stats">
                <div class="stat"><span>Modules</span><span class="stat-value" id="stat-modules">-</span></div>
                <div class="stat"><span>Dependencies</span><span class="stat-value" id="stat-deps">-</span></div>
                <div class="stat"><span>Issues</span><span class="stat-value" id="stat-issues">-</span></div>
                <div class="stat"><span>Cycles</span><span class="stat-value" id="stat-cycles">-</span></div>
            </div>
            <h2>Legend</h2>
            <div class="legend">
                <div class="legend-item"><div class="legend-color" style="background: #4ecdc4"></div><span>Index/Lib</span></div>
                <div class="legend-item"><div class="legend-color" style="background: #ff6b6b"></div><span>Entry</span></div>
                <div class="legend-item"><div class="legend-color" style="background: #ffe66d"></div><span>Config</span></div>
                <div class="legend-item"><div class="legend-color" style="background: #c9b1ff"></div><span>Model</span></div>
                <div class="legend-item"><div class="legend-color" style="background: #95e1d3"></div><span>Analysis</span></div>
                <div class="legend-item"><div class="legend-color" style="background: #f38181"></div><span>Parser</span></div>
                <div class="legend-item"><div class="legend-color" style="background: #6c5ce7"></div><span>Output</span></div>
                <div class="legend-item"><div class="legend-color" style="background: #74b9ff"></div><span>Module</span></div>
            </div>
            <div id="node-info">
                <h3 id="node-name"></h3>
                <div class="node-stat"><span>Lines</span><span id="node-lines">-</span></div>
                <div class="node-stat"><span>Fan-in</span><span id="node-fan-in">-</span></div>
                <div class="node-stat"><span>Fan-out</span><span id="node-fan-out">-</span></div>
                <div class="node-stat"><span>Issues</span><span id="node-issues">-</span></div>
                <div class="exports-list"><strong>Exports:</strong><div id="node-exports"></div></div>
            </div>
        </div>
    </div>
    <div class="tooltip" style="display: none;"></div>
    <script>
        const graphData = {json_data};
        const categoryColors = {{ 'index': '#4ecdc4', 'entry': '#ff6b6b', 'config': '#ffe66d', 'model': '#c9b1ff', 'analysis': '#95e1d3', 'parser': '#f38181', 'output': '#6c5ce7', 'cli': '#fdcb6e', 'test': '#a29bfe', 'module': '#74b9ff' }};
        let simulation, svg, g, link, node, label;
        let nodeScale = 1;

        function init() {{
            document.getElementById('project-name').textContent = graphData.metadata.project_name;
            document.getElementById('stat-modules').textContent = graphData.metadata.total_modules;
            document.getElementById('stat-deps').textContent = graphData.metadata.total_dependencies;
            document.getElementById('stat-issues').textContent = graphData.metadata.total_issues;
            document.getElementById('stat-cycles').textContent = graphData.metadata.cycle_count;
            createGraph();
        }}

        function createGraph() {{
            const container = document.getElementById('graph');
            const width = container.clientWidth;
            const height = container.clientHeight;
            svg = d3.select('#graph').append('svg').attr('width', width).attr('height', height);
            const zoom = d3.zoom().scaleExtent([0.1, 4]).on('zoom', (event) => {{ g.attr('transform', event.transform); }});
            svg.call(zoom);
            g = svg.append('g');
            svg.append('defs').append('marker').attr('id', 'arrowhead').attr('viewBox', '-0 -5 10 10').attr('refX', 20).attr('refY', 0).attr('orient', 'auto').attr('markerWidth', 6).attr('markerHeight', 6).append('path').attr('d', 'M 0,-5 L 10,0 L 0,5').attr('fill', '#555');
            link = g.append('g').selectAll('line').data(graphData.links).enter().append('line').attr('class', d => d.is_cycle ? 'link cycle' : 'link').attr('marker-end', 'url(#arrowhead)');
            node = g.append('g').selectAll('.node').data(graphData.nodes).enter().append('g').attr('class', 'node').call(d3.drag().on('start', dragstarted).on('drag', dragged).on('end', dragended));
            node.append('circle').attr('r', d => getNodeRadius(d)).attr('fill', d => categoryColors[d.category] || '#74b9ff');
            label = node.append('text').attr('dy', -12).attr('text-anchor', 'middle').text(d => d.name);
            const tooltip = d3.select('.tooltip');
            node.on('mouseover', function(event, d) {{ tooltip.style('display', 'block').html(`<strong>${{d.name}}</strong><br>${{d.path}}<br>Lines: ${{d.lines}}<br>Fan-in: ${{d.fan_in}} | Fan-out: ${{d.fan_out}}`).style('left', (event.pageX + 10) + 'px').style('top', (event.pageY - 10) + 'px'); highlightConnections(d); }}).on('mouseout', function() {{ tooltip.style('display', 'none'); clearHighlights(); }}).on('click', function(event, d) {{ showNodeInfo(d); }});
            simulation = d3.forceSimulation(graphData.nodes).force('link', d3.forceLink(graphData.links).id(d => d.id).distance(100)).force('charge', d3.forceManyBody().strength(-300)).force('center', d3.forceCenter(width / 2, height / 2)).force('collision', d3.forceCollide().radius(d => getNodeRadius(d) + 5)).on('tick', ticked);
        }}

        function getNodeRadius(d) {{ const base = Math.sqrt(d.lines) / 2 + 5; return Math.min(Math.max(base, 8), 30) * nodeScale; }}
        function ticked() {{ link.attr('x1', d => d.source.x).attr('y1', d => d.source.y).attr('x2', d => d.target.x).attr('y2', d => d.target.y); node.attr('transform', d => `translate(${{d.x}},${{d.y}})`); }}
        function dragstarted(event) {{ if (!event.active) simulation.alphaTarget(0.3).restart(); event.subject.fx = event.subject.x; event.subject.fy = event.subject.y; }}
        function dragged(event) {{ event.subject.fx = event.x; event.subject.fy = event.y; }}
        function dragended(event) {{ if (!event.active) simulation.alphaTarget(0); event.subject.fx = null; event.subject.fy = null; }}
        function highlightConnections(d) {{ const connected = new Set(); connected.add(d.id); link.each(function(l) {{ if (l.source.id === d.id || l.target.id === d.id) {{ connected.add(l.source.id); connected.add(l.target.id); d3.select(this).classed('highlighted', true); }} }}); node.classed('highlighted', n => connected.has(n.id)); }}
        function clearHighlights() {{ link.classed('highlighted', false); node.classed('highlighted', false); }}
        function showNodeInfo(d) {{ document.getElementById('node-info').classList.add('visible'); document.getElementById('node-name').textContent = d.path; document.getElementById('node-lines').textContent = d.lines; document.getElementById('node-fan-in').textContent = d.fan_in; document.getElementById('node-fan-out').textContent = d.fan_out; document.getElementById('node-issues').textContent = d.issue_count; const exportsDiv = document.getElementById('node-exports'); exportsDiv.innerHTML = d.exports && d.exports.length > 0 ? d.exports.map(e => `<span>${{e}}</span>`).join('') : '<em>None</em>'; }}
        window.addEventListener('resize', () => {{ const container = document.getElementById('graph'); svg.attr('width', container.clientWidth).attr('height', container.clientHeight); simulation.force('center', d3.forceCenter(container.clientWidth / 2, container.clientHeight / 2)); simulation.alpha(0.3).restart(); }});
        init();
    </script>
</body>
</html>
"#,
        json_data = json_data
    )
}
