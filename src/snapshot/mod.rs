mod diff;
mod serialize;

pub use diff::{SnapshotDiff, compute_diff, format_diff_json, format_diff_markdown};
pub use serialize::{Snapshot, load_snapshot, save_snapshot};
