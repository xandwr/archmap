use crate::cli::GraphArgs;
use crate::fs::{FileSystem, default_fs};
use crate::style;

use super::CommandContext;

pub fn cmd_graph(args: GraphArgs) -> i32 {
    let ctx = match CommandContext::new(&args.path, args.lang.as_deref()) {
        Ok(ctx) => ctx,
        Err(code) => return code,
    };

    // Run analysis
    let result = crate::analysis::analyze(&ctx.path, &ctx.config, &ctx.registry, &[]);

    // Build graph data
    let graph_data = crate::graph::GraphData::from_analysis(&result, &ctx.path);

    if args.serve || args.watch {
        // Start web server
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        if args.watch {
            // Watch mode with live updates
            let watch_ctx = crate::graph::WatchContext {
                path: ctx.path.clone(),
                config: ctx.config,
                registry: ctx.registry,
            };
            if let Err(e) = rt.block_on(crate::graph::serve_with_watch(
                graph_data, args.port, args.open, watch_ctx,
            )) {
                style::error(&format!("Server failed: {}", e));
                return 1;
            }
        } else {
            // Static serve mode
            if let Err(e) = rt.block_on(crate::graph::serve(graph_data, args.port, args.open)) {
                style::error(&format!("Server failed: {}", e));
                return 1;
            }
        }
    } else if let Some(export_path) = args.export {
        // Export static HTML
        let html = crate::graph::generate_static_html(&graph_data);
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
