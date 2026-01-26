//! Adapter to make CandleEmbedder work with PgClientGroupResolver
//!
//! The `ob_semantic_matcher::client_group_resolver` module defines its own `Embedder` trait
//! to avoid coupling to specific embedding implementations. This adapter bridges
//! `CandleEmbedder` to that trait.

use async_trait::async_trait;
use std::sync::Arc;

use crate::agent::learning::embedder::CandleEmbedder;

/// Wrapper to make CandleEmbedder implement the ob_semantic_matcher Embedder trait
pub struct ClientGroupEmbedderAdapter(pub Arc<CandleEmbedder>);

#[async_trait]
impl ob_semantic_matcher::client_group_resolver::Embedder for ClientGroupEmbedderAdapter {
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let text_owned = text.to_string();
        let embedder = self.0.clone();
        tokio::task::spawn_blocking(move || embedder.embed_query_blocking(&text_owned))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    }

    async fn embed_target(&self, text: &str) -> Result<Vec<f32>, String> {
        let text_owned = text.to_string();
        let embedder = self.0.clone();
        tokio::task::spawn_blocking(move || embedder.embed_target_blocking(&text_owned))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    }

    async fn embed_batch_targets(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let texts_owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let embedder = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let refs: Vec<&str> = texts_owned.iter().map(|s| s.as_str()).collect();
            embedder.embed_batch_targets_blocking(&refs)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
    }
}
