//! SSE streaming endpoint for agent chat
//!
//! NOTE: Streaming is not currently implemented. The chat panel uses
//! `/api/session/:id/chat` for request/response style chat.
//! This endpoint is kept as a stub for future streaming support.

use axum::{
    extract::Query,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct StreamParams {
    #[allow(dead_code)]
    pub id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)] // Variants for SSE protocol, not all paths exercised yet
pub enum StreamChunk {
    #[serde(rename = "chunk")]
    Chunk { content: String },
    #[serde(rename = "dsl")]
    Dsl { source: String },
    #[serde(rename = "ast")]
    Ast { statements: Vec<serde_json::Value> },
    #[serde(rename = "done")]
    Done { can_execute: bool },
    #[serde(rename = "error")]
    Error { message: String },
}

/// SSE streaming endpoint (stub - streaming not yet implemented)
///
/// Currently returns an immediate "not implemented" error and closes.
/// The chat panel falls back to polling `/api/session/:id/chat`.
pub async fn chat_stream(
    Query(_params): Query<StreamParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Return a single error event and close the stream
    let stream = stream::once(async {
        let chunk = StreamChunk::Error {
            message: "Streaming not implemented. Use /api/session/:id/chat instead.".to_string(),
        };
        let event = Event::default()
            .event("message")
            .data(serde_json::to_string(&chunk).unwrap());
        Ok::<_, Infallible>(event)
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
