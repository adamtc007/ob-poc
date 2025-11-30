//! UI routes - server-rendered HTML pages

use axum::{response::Html, routing::get, Router};

use super::pages;

/// Create router for UI pages
pub fn create_ui_router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/verbs", get(verbs))
}

async fn index() -> Html<String> {
    Html(pages::index_page(None))
}

async fn verbs() -> Html<String> {
    Html(pages::verbs_page())
}
