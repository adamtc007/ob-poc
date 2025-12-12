//! SSE streaming endpoint for agent chat
//!
//! Provides real-time streaming of agent responses to the HTML chat panel.

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StreamParams {
    pub id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)] // Variants used for SSE protocol, not all paths exercised yet
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

pub async fn chat_stream(
    Query(params): Query<StreamParams>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream_id = params.id;

    // Create a stream that polls for chunks
    let stream = stream::unfold(
        (state, stream_id, false),
        |(state, stream_id, mut done)| async move {
            if done {
                return None;
            }

            // Check for pending chunks
            let streams = state.pending_streams.read().await;
            if let Some(pending) = streams.get(&stream_id) {
                if pending.complete {
                    done = true;
                    let chunk = StreamChunk::Done { can_execute: true };
                    let event = Event::default()
                        .event("message")
                        .data(serde_json::to_string(&chunk).unwrap());
                    return Some((Ok(event), (state.clone(), stream_id, done)));
                }

                // Return any pending chunks
                if !pending.chunks.is_empty() {
                    let content = pending.chunks.join("");
                    let chunk = StreamChunk::Chunk { content };
                    let event = Event::default()
                        .event("message")
                        .data(serde_json::to_string(&chunk).unwrap());
                    return Some((Ok(event), (state.clone(), stream_id, done)));
                }
            }
            drop(streams);

            // No data yet, send keepalive ping
            tokio::time::sleep(Duration::from_millis(100)).await;
            let event = Event::default().comment("keepalive");
            Some((Ok(event), (state.clone(), stream_id, done)))
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}
