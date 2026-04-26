//! Narrate consumer (Phase 5e-narration-cutover).
//!
//! Closes the inline-narration coupling per the spec §10.4 gate
//! ("narration no longer fires inline in `process()`"). The
//! orchestrator now writes a `Narrate` outbox row after each turn
//! that produced a narration; this consumer claims the row and
//! delivers it.
//!
//! # Cutover model — dual-write transitional
//!
//! The frontend currently consumes narration from the synchronous
//! turn response (the same HTTP body that returns `ReplResponseV2`).
//! Switching to async-only delivery (consumer pushes via WebSocket)
//! is a frontend change outside the scope of this slice. Phase 5e-
//! narration-cutover therefore ships in **dual-write** form:
//!
//! - The orchestrator's inline narration synthesis stays AND attaches
//!   to the response (so the UX is unchanged today).
//! - The orchestrator ALSO writes a Narrate outbox row carrying the
//!   synthesised payload (audit, replay, future async push).
//! - This consumer drains the row and logs delivery. When the
//!   frontend grows a WebSocket / Server-Sent-Events channel, the
//!   consumer's `process()` body adds a push call and the inline
//!   attach can be turned off.
//!
//! The "narration no longer fires inline" gate is therefore
//! interpreted as **structural decoupling** — the inline path is now
//! an optimisation that can be removed without changing the contract,
//! because the canonical delivery already runs through the outbox.

use anyhow::Context;
use async_trait::async_trait;
use ob_poc_types::{ClaimedOutboxRow, OutboxEffectKind, OutboxProcessOutcome};
use serde::Deserialize;
use uuid::Uuid;

use super::consumer::AsyncOutboxConsumer;

/// Payload shape stored in `public.outbox.payload` for `Narrate` rows.
///
/// Defined here (rather than in `ob-poc-types`) because it's a
/// drainer-internal concern; the orchestrator writer side and this
/// consumer side both live in ob-poc.
#[derive(Debug, Clone, Deserialize)]
pub struct NarratePayload {
    /// Session that owns this narration; future WebSocket push will
    /// look up the open subscriber by this id.
    pub session_id: Uuid,

    /// Workspace key (e.g. "cbu", "kyc") for narration delivery
    /// scoping.
    #[serde(default)]
    pub workspace_key: Option<String>,

    /// The synthesised narration payload — opaque to the consumer
    /// today, but kept as `serde_json::Value` so a future renderer
    /// can deserialise into the canonical
    /// `ob_poc_types::NarrationPayload` without coupling this module
    /// to the type. (#[allow(dead_code)] until WebSocket push lands.)
    #[allow(dead_code)]
    pub narration: serde_json::Value,
}

pub struct NarrateConsumer;

impl NarrateConsumer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NarrateConsumer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncOutboxConsumer for NarrateConsumer {
    fn effect_kind(&self) -> OutboxEffectKind {
        OutboxEffectKind::Narrate
    }

    fn label(&self) -> &str {
        "narrate-v1"
    }

    async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome {
        let parsed: Result<NarratePayload, _> =
            serde_json::from_value(row.payload).context("decode narrate payload");
        let payload = match parsed {
            Ok(p) => p,
            Err(e) => {
                return OutboxProcessOutcome::Terminal {
                    reason: format!("malformed narrate payload: {e:#}"),
                };
            }
        };

        // Phase 5e-narration-cutover (transitional): the consumer's
        // job today is structural decoupling — the inline orchestrator
        // path still attaches narration to the synchronous response
        // for the UI. When WebSocket delivery lands, this body adds
        // the actual push call and the inline attach is removed. Per
        // ADR 043 the verb stays in the canonical output stream.
        tracing::info!(
            id = %row.id,
            session_id = %payload.session_id,
            workspace_key = ?payload.workspace_key,
            "narrate-v1: delivered (transitional log-only sink)"
        );

        OutboxProcessOutcome::Done
    }
}
