//! Static file serving

use axum::response::Html;

/// Serve the main index.html (egui app)
pub async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../../static/index.html"))
}
