//! Static file serving

use axum::response::Html;

/// Serve the main index.html (React app)
///
/// Tries to serve from React dist directory first (set via REACT_DIST_DIR env var),
/// falls back to the embedded placeholder HTML.
pub async fn serve_index() -> Html<String> {
    // Try to read from React dist directory
    let react_dist_dir = std::env::var("REACT_DIST_DIR").ok();

    if let Some(dist_dir) = react_dist_dir {
        let index_path = std::path::Path::new(&dist_dir).join("index.html");
        if let Ok(content) = std::fs::read_to_string(&index_path) {
            return Html(content);
        }
    }

    // Also check relative to manifest dir
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let react_paths = [
        format!("{}/../../ob-poc-ui-react/dist/index.html", manifest_dir),
        format!("{}/../../../ob-poc-ui-react/dist/index.html", manifest_dir),
    ];

    for path in &react_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            return Html(content);
        }
    }

    // Fallback to placeholder
    Html(include_str!("../../static/index.html").to_string())
}
