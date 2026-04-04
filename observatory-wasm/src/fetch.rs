//! Async HTTP fetch for WASM — uses ehttp for browser-compatible requests.
//!
//! Pattern: trigger fetch → set AsyncSlot::Pending → ehttp callback writes to
//! a shared Arc<Mutex<>> → process_results() at top of update() moves data
//! into ObservatoryState.

use std::sync::{Arc, Mutex};

use ob_poc_types::graph_scene::GraphSceneModel;

use crate::state::{AsyncSlot, HealthMetrics, ObservatoryState};

/// Shared mailbox for async fetch results.
/// Arc<Mutex> is Send (required by ehttp). WASM is single-threaded so no contention.
#[derive(Default, Clone)]
pub struct FetchMailbox {
    pub orientation: Arc<Mutex<Option<Result<serde_json::Value, String>>>>,
    pub show_packet: Arc<Mutex<Option<Result<serde_json::Value, String>>>>,
    pub graph_scene: Arc<Mutex<Option<Result<GraphSceneModel, String>>>>,
    pub health: Arc<Mutex<Option<Result<HealthMetrics, String>>>>,
}

/// Process completed async fetch results. Called at top of update().
pub fn process_results(state: &mut ObservatoryState) {
    if let Ok(mut slot) = state.mailbox.orientation.lock() {
        if let Some(result) = slot.take() {
            match result {
                Ok(v) => state.fetch.orientation = AsyncSlot::Ready(v),
                Err(e) => state.fetch.orientation = AsyncSlot::Error(e),
            }
        }
    }

    if let Ok(mut slot) = state.mailbox.show_packet.lock() {
        if let Some(result) = slot.take() {
            match result {
                Ok(v) => state.fetch.show_packet = AsyncSlot::Ready(v),
                Err(e) => state.fetch.show_packet = AsyncSlot::Error(e),
            }
        }
    }

    if let Ok(mut slot) = state.mailbox.graph_scene.lock() {
        if let Some(result) = slot.take() {
            match result {
                Ok(v) => {
                    state.scene_cache.invalidate();
                    state.fetch.graph_scene = AsyncSlot::Ready(v);
                }
                Err(e) => state.fetch.graph_scene = AsyncSlot::Error(e),
            }
        }
    }

    if let Ok(mut slot) = state.mailbox.health.lock() {
        if let Some(result) = slot.take() {
            match result {
                Ok(v) => state.fetch.health = AsyncSlot::Ready(v),
                Err(e) => state.fetch.health = AsyncSlot::Error(e),
            }
        }
    }
}

/// Fetch orientation contract from server.
pub fn fetch_orientation(state: &mut ObservatoryState, ctx: egui::Context) {
    if state.session_id.is_empty() {
        return;
    }
    let url = format!(
        "{}/api/observatory/session/{}/orientation",
        state.base_url, state.session_id
    );
    state.fetch.orientation = AsyncSlot::Pending;
    let slot = state.mailbox.orientation.clone();
    do_fetch::<serde_json::Value>(url, slot, ctx);
}

/// Fetch full ShowPacket with viewports.
pub fn fetch_show_packet(state: &mut ObservatoryState, ctx: egui::Context) {
    if state.session_id.is_empty() {
        return;
    }
    let url = format!(
        "{}/api/observatory/session/{}/show-packet",
        state.base_url, state.session_id
    );
    state.fetch.show_packet = AsyncSlot::Pending;
    let slot = state.mailbox.show_packet.clone();
    do_fetch::<serde_json::Value>(url, slot, ctx);
}

/// Fetch GraphSceneModel for constellation canvas.
pub fn fetch_graph_scene(state: &mut ObservatoryState, ctx: egui::Context) {
    if state.session_id.is_empty() {
        return;
    }
    let url = format!(
        "{}/api/observatory/session/{}/graph-scene",
        state.base_url, state.session_id
    );
    state.fetch.graph_scene = AsyncSlot::Pending;
    let slot = state.mailbox.graph_scene.clone();
    do_fetch::<GraphSceneModel>(url, slot, ctx);
}

/// Fetch health metrics for Mission Control.
pub fn fetch_health(state: &mut ObservatoryState, ctx: egui::Context) {
    let url = format!("{}/api/observatory/health", state.base_url);
    state.fetch.health = AsyncSlot::Pending;
    let slot = state.mailbox.health.clone();
    do_fetch::<HealthMetrics>(url, slot, ctx);
}

/// Generic fetch helper: GET url → parse JSON → write to mailbox slot.
fn do_fetch<T: serde::de::DeserializeOwned + Send + 'static>(
    url: String,
    slot: Arc<Mutex<Option<Result<T, String>>>>,
    ctx: egui::Context,
) {
    let request = ehttp::Request::get(&url);
    ehttp::fetch(request, move |result| {
        let parsed = match result {
            Ok(response) if response.ok => {
                match response.text() {
                    Some(text) => serde_json::from_str(text)
                        .map_err(|e| format!("Parse error: {e}")),
                    None => Err("Empty response body".into()),
                }
            }
            Ok(response) => Err(format!("HTTP {}", response.status)),
            Err(e) => Err(e),
        };
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(parsed);
        }
        ctx.request_repaint();
    });
}
