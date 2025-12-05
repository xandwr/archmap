use crate::cli::{ImpactArgs, OutputFormat};
use crate::fs::{FileSystem, default_fs};
use crate::style;
use std::io::{self, Write};

use super::CommandContext;

pub fn cmd_impact(args: ImpactArgs) -> i32 {
    let ctx = match CommandContext::new(&args.path, args.lang.as_deref()) {
        Ok(ctx) => ctx,
        Err(code) => return code,
    };

    // Resolve the target file
    let target_file = if args.file.is_absolute() {
        args.file.clone()
    } else {
        ctx.path.join(&args.file)
    };

    let target_file = match target_file.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            style::error(&format!("Could not find file: {}", style::path(&args.file)));
            return 1;
        }
    };

    // Run analysis to build dependency graph
    let result = crate::analysis::analyze(&ctx.path, &ctx.config, &ctx.registry, &[]);

    // Build dependency graph
    let graph = crate::analysis::DependencyGraph::build(&result.modules);

    // Compute impact
    let impact = match crate::analysis::compute_impact(&graph, &target_file, args.depth) {
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
            crate::analysis::format_impact_markdown(&impact, Some(&ctx.path), args.tree)
        }
        OutputFormat::Json => crate::analysis::format_impact_json(&impact, Some(&ctx.path)),
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
