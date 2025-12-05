use archmap::cli::{AnalyzeArgs, Cli, Command};
use archmap::{cmd_ai, cmd_analyze, cmd_diff, cmd_graph, cmd_impact, cmd_init, cmd_snapshot};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    // Handle --mcp-manifest flag
    if cli.mcp_manifest {
        print_mcp_manifest();
        std::process::exit(0);
    }

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

fn print_mcp_manifest() {
    let exe_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "archmap".to_string());

    let manifest = serde_json::json!({
        "name": "archmap",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Architectural analysis and code understanding for AI agents",
        "command": exe_path,
        "tools": [
            {
                "name": "analyze",
                "description": "Run full architectural analysis with coupling metrics and issue detection",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to analyze (defaults to current directory)"
                        },
                        "format": {
                            "type": "string",
                            "enum": ["markdown", "json"],
                            "description": "Output format"
                        },
                        "min_severity": {
                            "type": "string",
                            "enum": ["info", "warning", "error"],
                            "description": "Minimum severity to report"
                        }
                    }
                }
            },
            {
                "name": "ai",
                "description": "Generate AI-optimized compact context output for LLM consumption",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to analyze (defaults to current directory)"
                        },
                        "tokens": {
                            "type": "integer",
                            "description": "Maximum tokens for output"
                        },
                        "signatures": {
                            "type": "boolean",
                            "description": "Output only architectural signatures (public API surface)"
                        },
                        "format": {
                            "type": "string",
                            "enum": ["markdown", "json", "xml"],
                            "description": "Output format"
                        },
                        "priority": {
                            "type": "string",
                            "enum": ["fan-in", "fan-out", "combined"],
                            "description": "Prioritization strategy for token budgeting"
                        }
                    }
                }
            },
            {
                "name": "impact",
                "description": "Analyze change impact for a specific file - shows what depends on it",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "description": "File to analyze for change impact"
                        },
                        "path": {
                            "type": "string",
                            "description": "Project path (defaults to current directory)"
                        },
                        "depth": {
                            "type": "integer",
                            "description": "Maximum depth to traverse"
                        },
                        "tree": {
                            "type": "boolean",
                            "description": "Show ASCII tree visualization"
                        }
                    },
                    "required": ["file"]
                }
            },
            {
                "name": "diff",
                "description": "Compare current architecture against a baseline snapshot",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "baseline": {
                            "type": "string",
                            "description": "Baseline snapshot file to compare against"
                        },
                        "path": {
                            "type": "string",
                            "description": "Path to analyze (defaults to current directory)"
                        },
                        "fail_on_regression": {
                            "type": "boolean",
                            "description": "Exit with error if architectural regressions are found"
                        }
                    },
                    "required": ["baseline"]
                }
            }
        ],
        "mcpServers": {
            "archmap": {
                "type": "stdio",
                "command": exe_path,
                "args": [],
                "description": "Architectural analysis and code understanding for AI agents"
            }
        }
    });

    println!("{}", serde_json::to_string_pretty(&manifest).unwrap());
}
