//! BPMN-Lite fire-and-forget consumers (Phase F.1b, 2026-04-22).
//!
//! Closes the loop opened in the Phase F.1 partial: the
//! `bpmn.signal` / `bpmn.cancel` verbs write a row into `public.outbox`
//! with `effect_kind = 'bpmn_signal'` / `'bpmn_cancel'` inside the
//! ambient transaction scope instead of calling the gRPC client
//! directly. These consumers drain those rows post-commit and perform
//! the actual gRPC call.
//!
//! # Fire-and-forget contract
//!
//! Both verbs have no return value — callers don't wait on the BPMN
//! service to acknowledge. The consumer's contract is:
//!   - Exactly-once delivery from the operator's perspective, backed
//!     by `(idempotency_key, effect_kind)` UNIQUE dedupe at enqueue
//!     plus drainer claim semantics (`FOR UPDATE SKIP LOCKED` +
//!     stale-claim recycler).
//!   - On gRPC error, the row is marked `failed_retryable` so the
//!     drainer retries. After `max_attempts` the row is auto-promoted
//!     to `failed_terminal` with alerting (Phase 5e foundation).
//!   - The gRPC client fetches its target endpoint from
//!     `BPMN_LITE_GRPC_URL` via `BpmnLiteConnection::from_env`, same
//!     as the pre-refactor verb body.

use async_trait::async_trait;
use ob_poc_types::{ClaimedOutboxRow, OutboxEffectKind, OutboxProcessOutcome};
use serde::Deserialize;
use uuid::Uuid;

use super::consumer::AsyncOutboxConsumer;

// ---------------------------------------------------------------------------
// bpmn_signal
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct BpmnSignalPayload {
    instance_id: Uuid,
    message_name: String,
    #[serde(default)]
    payload: Option<String>,
}

pub struct BpmnSignalConsumer;

impl BpmnSignalConsumer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BpmnSignalConsumer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncOutboxConsumer for BpmnSignalConsumer {
    fn effect_kind(&self) -> OutboxEffectKind {
        OutboxEffectKind::BpmnSignal
    }

    fn label(&self) -> &str {
        "bpmn-signal-v1"
    }

    async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome {
        let payload: BpmnSignalPayload = match serde_json::from_value(row.payload) {
            Ok(p) => p,
            Err(e) => {
                return OutboxProcessOutcome::Terminal {
                    reason: format!("malformed bpmn_signal payload: {e}"),
                };
            }
        };

        let client = match crate::bpmn_integration::client::BpmnLiteConnection::from_env() {
            Ok(c) => c,
            Err(e) => {
                // Connection setup failure — BPMN_LITE_GRPC_URL unset
                // or unreachable. Retry on the next cycle; if the
                // service is genuinely down for hours, max_attempts
                // promotes to terminal.
                return OutboxProcessOutcome::Retryable {
                    reason: format!("bpmn client init failed: {e}"),
                };
            }
        };

        tracing::info!(
            id = %row.id,
            instance_id = %payload.instance_id,
            message_name = %payload.message_name,
            "bpmn-signal-v1: sending signal"
        );

        let result = client
            .signal(
                payload.instance_id,
                &payload.message_name,
                payload.payload.as_ref().map(|p| p.as_bytes()),
            )
            .await;

        match result {
            Ok(()) => OutboxProcessOutcome::Done,
            Err(e) => OutboxProcessOutcome::Retryable {
                reason: format!("bpmn signal failed: {e}"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// bpmn_cancel
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct BpmnCancelPayload {
    instance_id: Uuid,
    reason: String,
}

pub struct BpmnCancelConsumer;

impl BpmnCancelConsumer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BpmnCancelConsumer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncOutboxConsumer for BpmnCancelConsumer {
    fn effect_kind(&self) -> OutboxEffectKind {
        OutboxEffectKind::BpmnCancel
    }

    fn label(&self) -> &str {
        "bpmn-cancel-v1"
    }

    async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome {
        let payload: BpmnCancelPayload = match serde_json::from_value(row.payload) {
            Ok(p) => p,
            Err(e) => {
                return OutboxProcessOutcome::Terminal {
                    reason: format!("malformed bpmn_cancel payload: {e}"),
                };
            }
        };

        let client = match crate::bpmn_integration::client::BpmnLiteConnection::from_env() {
            Ok(c) => c,
            Err(e) => {
                return OutboxProcessOutcome::Retryable {
                    reason: format!("bpmn client init failed: {e}"),
                };
            }
        };

        tracing::info!(
            id = %row.id,
            instance_id = %payload.instance_id,
            reason = %payload.reason,
            "bpmn-cancel-v1: cancelling instance"
        );

        let result = client.cancel(payload.instance_id, &payload.reason).await;

        match result {
            Ok(()) => OutboxProcessOutcome::Done,
            Err(e) => OutboxProcessOutcome::Retryable {
                reason: format!("bpmn cancel failed: {e}"),
            },
        }
    }
}
