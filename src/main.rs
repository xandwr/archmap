use archmap::cli::{AnalyzeArgs, Cli, Command};
use archmap::{cmd_ai, cmd_analyze, cmd_diff, cmd_graph, cmd_impact, cmd_init, cmd_snapshot};
use clap::Parser;

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
