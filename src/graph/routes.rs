use super::assets::INDEX_HTML;
use super::data::GraphData;
use crate::fs::{FileSystem, default_fs};
use crate::style;
use axum::{
    Json, Router,
    extract::State,
    response::{
        Html, IntoResponse,
        sse::{Event, Sse},
    },
    routing::get,
};
use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::WatchStream;
use tower_http::cors::{Any, CorsLayer};

/// Application state shared across handlers
pub struct AppState {
    pub graph_data: Arc<tokio::sync::RwLock<GraphData>>,
    pub update_rx: watch::Receiver<u64>,
}

/// Context needed to rebuild the graph
pub struct WatchContext {
    pub path: PathBuf,
    pub config: crate::config::Config,
    pub registry: crate::parser::ParserRegistry,
}

/// Start the HTTP server for graph visualization
pub async fn serve(
    graph_data: GraphData,
    port: u16,
    open_browser: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (update_tx, update_rx) = watch::channel(0u64);
    let state = Arc::new(AppState {
        graph_data: Arc::new(tokio::sync::RwLock::new(graph_data)),
        update_rx,
    });

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any);

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/graph", get(graph_handler))
        .route("/api/events", get(sse_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let url = format!("http://{}", addr);

    style::header("Starting archmap visualization server...");
    style::status(&format!("Open in browser: {}", style::url(&url)));
    println!("Press Ctrl+C to stop");

    // Keep update_tx alive but unused for non-watch mode
    drop(update_tx);

    if open_browser {
        if let Err(e) = open::that(&url) {
            style::warning(&format!("Could not open browser: {}", e));
        }
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Start the HTTP server with file watching enabled
pub async fn serve_with_watch(
    graph_data: GraphData,
    port: u16,
    open_browser: bool,
    watch_ctx: WatchContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let (update_tx, update_rx) = watch::channel(0u64);
    let graph_data = Arc::new(tokio::sync::RwLock::new(graph_data));

    let state = Arc::new(AppState {
        graph_data: graph_data.clone(),
        update_rx,
    });

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any);

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/graph", get(graph_handler))
        .route("/api/events", get(sse_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let url = format!("http://{}", addr);

    style::header("Starting archmap visualization server (watch mode)...");
    style::status(&format!("Open in browser: {}", style::url(&url)));
    style::status("Watching for file changes...");
    println!("Press Ctrl+C to stop");

    if open_browser {
        if let Err(e) = open::that(&url) {
            style::warning(&format!("Could not open browser: {}", e));
        }
    }

    // Spawn the file watcher task
    let watcher_graph = graph_data.clone();
    tokio::spawn(async move {
        watch_files(watch_ctx, watcher_graph, update_tx).await;
    });

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Watch for file changes and update the graph
async fn watch_files(
    ctx: WatchContext,
    graph_data: Arc<tokio::sync::RwLock<GraphData>>,
    update_tx: watch::Sender<u64>,
) {
    let mut last_modified: HashMap<PathBuf, std::time::SystemTime> = HashMap::new();
    let mut version = 0u64;

    // Initial scan
    scan_files(&ctx.path, &mut last_modified);

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut current_files: HashMap<PathBuf, std::time::SystemTime> = HashMap::new();
        scan_files(&ctx.path, &mut current_files);

        let mut changed = false;

        // Check for new or modified files
        for (file_path, modified) in &current_files {
            let display_path = file_path
                .strip_prefix(&ctx.path)
                .unwrap_or(file_path)
                .display()
                .to_string();
            match last_modified.get(file_path) {
                Some(last) if last != modified => {
                    println!("  {}", style::file_changed(&display_path));
                    changed = true;
                }
                None => {
                    println!("  {}", style::file_added(&display_path));
                    changed = true;
                }
                _ => {}
            }
        }

        // Check for deleted files
        for file_path in last_modified.keys() {
            if !current_files.contains_key(file_path) {
                let display_path = file_path
                    .strip_prefix(&ctx.path)
                    .unwrap_or(file_path)
                    .display()
                    .to_string();
                println!("  {}", style::file_deleted(&display_path));
                changed = true;
            }
        }

        if changed {
            style::status("Re-analyzing...");

            // Re-run analysis
            let result = crate::analysis::analyze(&ctx.path, &ctx.config, &ctx.registry, &[]);
            let new_graph = GraphData::from_analysis(&result, &ctx.path);

            // Update the shared graph data
            {
                let mut graph = graph_data.write().await;
                *graph = new_graph;
            }

            // Notify clients
            version += 1;
            let _ = update_tx.send(version);

            style::success(&format!("Graph updated (version {})", version));
            last_modified = current_files;
        }
    }
}

fn scan_files(path: &PathBuf, files: &mut HashMap<PathBuf, std::time::SystemTime>) {
    let fs = default_fs();
    let walker = ignore::WalkBuilder::new(path)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        let file_path = entry.path();
        if file_path.is_file() {
            if let Ok(modified) = fs.modified(file_path) {
                files.insert(file_path.to_path_buf(), modified);
            }
        }
    }
}

async fn index_handler() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn graph_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let graph = state.graph_data.read().await;
    Json(graph.clone())
}

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = WatchStream::new(state.update_rx.clone())
        .map(|version| Ok(Event::default().event("update").data(version.to_string())));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive"),
    )
}
