use super::assets::INDEX_HTML;
use super::data::GraphData;
use axum::{
    Json, Router,
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Application state shared across handlers
pub struct AppState {
    pub graph_data: GraphData,
}

/// Start the HTTP server for graph visualization
pub async fn serve(
    graph_data: GraphData,
    port: u16,
    open_browser: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState { graph_data });

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any);

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/graph", get(graph_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let url = format!("http://{}", addr);

    println!("Starting archmap visualization server...");
    println!("Open in browser: {}", url);
    println!("Press Ctrl+C to stop");

    if open_browser {
        if let Err(e) = open::that(&url) {
            eprintln!("Warning: Could not open browser: {}", e);
        }
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn graph_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.graph_data.clone())
}
