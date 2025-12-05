use crate::cli::{DiffArgs, OutputFormat};
use crate::fs::{FileSystem, default_fs};
use crate::style;
use std::io::{self, Write};

use super::CommandContext;

pub fn cmd_diff(args: DiffArgs) -> i32 {
    // Load baseline snapshot
    let baseline = match crate::snapshot::load_snapshot(&args.baseline) {
        Ok(s) => s,
        Err(e) => {
            style::error(&format!("Failed to load baseline snapshot: {}", e));
            return 1;
        }
    };

    let ctx = match CommandContext::new(&args.path, args.lang.as_deref()) {
        Ok(ctx) => ctx,
        Err(code) => return code,
    };

    // Run current analysis
    let result = crate::analysis::analyze(&ctx.path, &ctx.config, &ctx.registry, &[]);

    // Create current snapshot
    let current = crate::snapshot::Snapshot::from_analysis(&result, &ctx.path);

    // Compute diff
    let diff = crate::snapshot::compute_diff(&baseline, &current);

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
        OutputFormat::Markdown => crate::snapshot::format_diff_markdown(&diff),
        OutputFormat::Json => crate::snapshot::format_diff_json(&diff),
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
