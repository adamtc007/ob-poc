//! Async consumer trait for the outbox drainer.
//!
//! Wraps the sync-signature placeholder
//! [`ob_poc_types::OutboxConsumer`] from Phase 0d with the
//! `async_trait` binding the drainer actually needs.

use async_trait::async_trait;
use ob_poc_types::{ClaimedOutboxRow, OutboxEffectKind, OutboxProcessOutcome};

/// One implementation per [`OutboxEffectKind`]: maintenance spawner,
/// narration synthesiser, WebSocket UI pusher, constellation
/// broadcaster, external HTTP notifier.
///
/// # Idempotency contract
///
/// Consumers MUST be idempotent against the row's `idempotency_key`.
/// The drainer's at-least-once semantics means a row can be handed to
/// the consumer more than once across worker crashes or claim-timeout
/// recoveries. A consumer that has already produced the effect for an
/// idempotency key should return [`OutboxProcessOutcome::Deduped`].
#[async_trait]
pub trait AsyncOutboxConsumer: Send + Sync {
    /// The effect kind this consumer handles. Must be unique per
    /// drainer instance — registration enforces.
    fn effect_kind(&self) -> OutboxEffectKind;

    /// Stable label for logging / tracing (e.g. "maintenance-spawn-v1").
    fn label(&self) -> &str;

    /// Process one claimed row. Errors should be returned via
    /// [`OutboxProcessOutcome::Retryable`] or
    /// [`OutboxProcessOutcome::Terminal`] rather than propagated as
    /// `Err` — the drainer only logs the outcome and updates row
    /// status. Panics in this method are caught and logged as
    /// terminal failures.
    async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome;
}
