//! Outbox drainer (Phase 5e — three-plane v0.3 §10.4).
//!
//! Post-commit effect dispatch. The drainer polls `public.outbox`,
//! claims `pending` rows, dispatches each to its registered
//! [`AsyncOutboxConsumer`] by [`OutboxEffectKind`], and marks each
//! row done / retry / terminal based on the consumer's outcome.
//!
//! # Why a separate task
//!
//! Outbox rows are written INSIDE the stage-8 runtime transaction (so
//! they commit atomically with the writes that caused them). Effects
//! must NOT fire inline because:
//!
//! 1. They may be slow (subprocess spawn, HTTP POST, narration
//!    synthesis) and would block the request path.
//! 2. They have external side effects that must run AFTER the
//!    transaction commits — never before, never as part of a rolled-
//!    back txn.
//! 3. They need at-least-once semantics with idempotency, which a
//!    polling drainer + idempotency_key dedupe enforces cleanly.
//!
//! # Concurrency model
//!
//! - Single drainer task per process (Phase 5e foundation; sharding
//!   per effect_kind is a Phase 5e+ extension).
//! - Polling loop with configurable interval.
//! - Claim batch: `UPDATE ... SET status='processing' WHERE id IN (
//!   SELECT id FROM ... FOR UPDATE SKIP LOCKED LIMIT N) RETURNING *`
//!   gives mutual exclusion across multiple drainer instances if
//!   we ever shard.
//! - Stale `processing` rows (claimed_at > now() - claim_timeout) are
//!   recycled to `pending` at the start of each poll cycle to recover
//!   from worker crashes.
//!
//! # Backoff and retry
//!
//! - `Retryable` outcomes increment `attempts` and set status back to
//!   `failed_retryable`. Next poll picks them up again.
//! - After `max_attempts`, a retryable row is auto-promoted to
//!   `failed_terminal` with the last error captured.
//! - `Terminal` outcomes are immediate failures (e.g. malformed
//!   payload, missing consumer). No retry; alerting trigger.

mod bpmn_signal;
mod consumer;
mod drainer;
mod maintenance_spawn;
mod narrate;
pub mod narration_emit;

pub use bpmn_signal::{BpmnCancelConsumer, BpmnSignalConsumer};
pub use consumer::AsyncOutboxConsumer;
pub use drainer::{OutboxDrainerImpl, OutboxDrainerConfig, OutboxDrainerHandle};
pub use maintenance_spawn::MaintenanceSpawnConsumer;
pub use narrate::NarrateConsumer;
