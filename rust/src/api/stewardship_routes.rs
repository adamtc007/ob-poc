//! Stewardship REST + SSE routes — Phase 1 Show Loop transport.
//!
//! ## Endpoints
//!
//! - `GET  /api/stewardship/session/:id/focus`            - Get current FocusState
//! - `PUT  /api/stewardship/session/:id/focus`            - Set FocusState (emits FocusChanged)
//! - `DELETE /api/stewardship/session/:id/focus`           - Delete FocusState
//! - `GET  /api/stewardship/session/:id/show`             - Compute ShowPacket (REST snapshot)
//! - `GET  /api/stewardship/session/:id/workbench-events` - SSE stream of WorkbenchPackets
//! - `POST /api/stewardship/session/:id/manifest`         - Capture ViewportManifest

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::sem_reg::stewardship::{
    focus::FocusStore,
    show_loop::ShowLoop,
    types::*,
};

// ── State ────────────────────────────────────────────────────

/// Shared state for stewardship routes.
#[derive(Clone)]
pub struct StewardshipState {
    pub pool: PgPool,
    /// Per-session broadcast channels for SSE delivery.
    pub channels: Arc<tokio::sync::RwLock<HashMap<Uuid, broadcast::Sender<WorkbenchPacket>>>>,
}

impl StewardshipState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            channels: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a broadcast channel for a session.
    async fn get_channel(&self, session_id: Uuid) -> broadcast::Sender<WorkbenchPacket> {
        {
            let channels = self.channels.read().await;
            if let Some(tx) = channels.get(&session_id) {
                return tx.clone();
            }
        }
        let mut channels = self.channels.write().await;
        // Double-check after acquiring write lock
        channels
            .entry(session_id)
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(64);
                tx
            })
            .clone()
    }
}

// ── Request/Response Types ───────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SetFocusRequest {
    pub changeset_id: Option<Uuid>,
    pub overlay_mode: Option<SetOverlayMode>,
    pub object_refs: Vec<ObjectRef>,
    pub taxonomy_focus: Option<TaxonomyFocus>,
    pub resolution_context: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SetOverlayMode {
    ActiveOnly,
    DraftOverlay { changeset_id: Uuid },
}

impl From<SetOverlayMode> for OverlayMode {
    fn from(val: SetOverlayMode) -> Self {
        match val {
            SetOverlayMode::ActiveOnly => OverlayMode::ActiveOnly,
            SetOverlayMode::DraftOverlay { changeset_id } => {
                OverlayMode::DraftOverlay { changeset_id }
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FocusResponse {
    pub focus: Option<FocusState>,
}

#[derive(Debug, Serialize)]
pub struct ShowResponse {
    pub show_packet: ShowPacket,
}

#[derive(Debug, Serialize)]
pub struct ManifestResponse {
    pub manifest_id: Uuid,
    pub viewport_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ── Router ───────────────────────────────────────────────────

/// Create the stewardship router — call from main.rs.
pub fn create_stewardship_router(pool: PgPool) -> Router<()> {
    let state = StewardshipState::new(pool);

    Router::new()
        .route(
            "/api/stewardship/session/{id}/focus",
            get(get_focus).put(set_focus).delete(delete_focus),
        )
        .route(
            "/api/stewardship/session/{id}/show",
            get(get_show),
        )
        .route(
            "/api/stewardship/session/{id}/workbench-events",
            get(workbench_sse),
        )
        .route(
            "/api/stewardship/session/{id}/manifest",
            post(capture_manifest),
        )
        .with_state(state)
}

// ── Handlers ─────────────────────────────────────────────────

/// GET /api/stewardship/session/:id/focus
async fn get_focus(
    State(state): State<StewardshipState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<FocusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let focus = FocusStore::get(&state.pool, session_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get focus: {}", e),
                }),
            )
        })?;

    Ok(Json(FocusResponse { focus }))
}

/// PUT /api/stewardship/session/:id/focus
async fn set_focus(
    State(state): State<StewardshipState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetFocusRequest>,
) -> Result<Json<FocusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let overlay_mode = req
        .overlay_mode
        .map(OverlayMode::from)
        .unwrap_or(OverlayMode::ActiveOnly);

    let focus = FocusState {
        session_id,
        changeset_id: req.changeset_id,
        overlay_mode,
        object_refs: req.object_refs,
        taxonomy_focus: req.taxonomy_focus,
        resolution_context: req.resolution_context,
        updated_at: Utc::now(),
        updated_by: FocusUpdateSource::UserNavigation,
    };

    FocusStore::set(
        &state.pool,
        &focus,
        FocusUpdateSource::UserNavigation,
        req.changeset_id,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to set focus: {}", e),
            }),
        )
    })?;

    // After focus change, compute and broadcast ShowPacket
    let show_packet = ShowLoop::compute_show_packet(&state.pool, &focus, "rest_api", None)
        .await
        .ok();

    if let Some(packet) = show_packet {
        let workbench = WorkbenchPacket {
            packet_id: Uuid::new_v4(),
            session_id,
            timestamp: Utc::now(),
            frame_type: "workbench".to_string(),
            kind: WorkbenchPacketKind::Show,
            payload: WorkbenchPayload::ShowPayload { show_packet: packet },
        };
        let tx = state.get_channel(session_id).await;
        // Best-effort send — if no SSE clients connected, this is a no-op
        let _ = tx.send(workbench);
    }

    // Return the persisted focus
    let persisted = FocusStore::get(&state.pool, session_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read back focus: {}", e),
                }),
            )
        })?;

    Ok(Json(FocusResponse { focus: persisted }))
}

/// DELETE /api/stewardship/session/:id/focus
async fn delete_focus(
    State(state): State<StewardshipState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    FocusStore::delete(&state.pool, session_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to delete focus: {}", e),
                }),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/stewardship/session/:id/show — REST snapshot of ShowPacket
async fn get_show(
    State(state): State<StewardshipState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ShowResponse>, (StatusCode, Json<ErrorResponse>)> {
    let focus = FocusStore::get(&state.pool, session_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to load focus: {}", e),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "No focus state for session".to_string(),
                }),
            )
        })?;

    let show_packet = ShowLoop::compute_show_packet(&state.pool, &focus, "rest_api", None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to compute show packet: {}", e),
                }),
            )
        })?;

    Ok(Json(ShowResponse { show_packet }))
}

/// GET /api/stewardship/session/:id/workbench-events — SSE stream
async fn workbench_sse(
    State(state): State<StewardshipState>,
    Path(session_id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let tx = state.get_channel(session_id).await;
    let rx = tx.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(packet) => {
            let json = serde_json::to_string(&packet).unwrap_or_default();
            Some(Ok(Event::default().event("workbench").data(json)))
        }
        Err(_) => None, // Lagged — skip missed messages
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// POST /api/stewardship/session/:id/manifest — capture ViewportManifest
async fn capture_manifest(
    State(state): State<StewardshipState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ManifestResponse>, (StatusCode, Json<ErrorResponse>)> {
    let focus = FocusStore::get(&state.pool, session_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to load focus: {}", e),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "No focus state for session".to_string(),
                }),
            )
        })?;

    let show_packet = ShowLoop::compute_show_packet(&state.pool, &focus, "rest_api", None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to compute show packet: {}", e),
                }),
            )
        })?;

    // Build ViewportModels from the ShowPacket's ViewportSpecs
    let viewport_models: Vec<ViewportModel> = show_packet
        .viewports
        .iter()
        .map(|spec| ViewportModel {
            id: spec.id.clone(),
            kind: spec.kind.clone(),
            status: ViewportStatus::Ready,
            data: spec.params.clone(),
            meta: ViewportMeta {
                updated_at: Utc::now(),
                sources: vec![],
                overlay_mode: focus.overlay_mode.clone(),
            },
        })
        .collect();

    let manifest = ShowLoop::compute_manifest(&focus, &viewport_models, None);
    let manifest_id = manifest.manifest_id;
    let viewport_count = manifest.rendered_viewports.len();

    ShowLoop::persist_manifest(&state.pool, &manifest)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to persist manifest: {}", e),
                }),
            )
        })?;

    Ok(Json(ManifestResponse {
        manifest_id,
        viewport_count,
    }))
}
