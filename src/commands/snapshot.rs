use crate::cli::SnapshotArgs;
use crate::style;

use super::CommandContext;

pub fn cmd_snapshot(args: SnapshotArgs) -> i32 {
    let ctx = match CommandContext::new(&args.path, args.lang.as_deref()) {
        Ok(ctx) => ctx,
        Err(code) => return code,
    };

    // Run analysis
    let result = crate::analysis::analyze(&ctx.path, &ctx.config, &ctx.registry, &[]);

    // Create snapshot
    let snapshot = crate::snapshot::Snapshot::from_analysis(&result, &ctx.path);

    // Use the save path from args
    let output_path = &args.save;

    // Save snapshot
    if let Err(e) = crate::snapshot::save_snapshot(&snapshot, output_path) {
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
