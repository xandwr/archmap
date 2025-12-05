mod assets;
mod data;
mod routes;

pub use assets::generate_static_html;
pub use data::GraphData;
pub use routes::{WatchContext, serve, serve_with_watch};
