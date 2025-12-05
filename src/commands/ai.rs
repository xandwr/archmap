use crate::cli::AiArgs;
use crate::fs::{FileSystem, default_fs};
use crate::parser::ParserRegistry;
use crate::style;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::CommandContext;

pub fn cmd_ai(args: AiArgs) -> i32 {
    cmd_ai_with_fs(args, default_fs())
}

fn cmd_ai_with_fs(args: AiArgs, fs: &dyn FileSystem) -> i32 {
    let ctx = match CommandContext::new(&args.path, args.lang.as_deref()) {
        Ok(ctx) => ctx,
        Err(code) => return code,
    };

    // Collect source files for AI output
    let sources = collect_sources_with_fs(&ctx.path, &ctx.registry, fs);

    // Run analysis
    let result = crate::analysis::analyze(&ctx.path, &ctx.config, &ctx.registry, &[]);

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
    let mut formatter = crate::output::AiOutput::new(Some(ctx.path))
        .with_topo_order(args.topo_order)
        .with_signatures_only(args.signatures)
        .with_priority(args.priority)
        .with_format(args.format)
        .with_sources(sources);

    if let Some(tokens) = args.tokens {
        formatter = formatter.with_token_budget(tokens);
    }

    if let Err(e) = crate::output::OutputFormatter::format(&formatter, &result, &mut output) {
        style::error(&format!("Failed to write output: {}", e));
        return 1;
    }

    0
}

fn collect_sources_with_fs(
    path: &Path,
    registry: &ParserRegistry,
    fs: &dyn FileSystem,
) -> HashMap<PathBuf, String> {
    let mut sources = HashMap::new();
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
