//! Federated DSL bus durability — outbox + inbox CRUD.
//!
//! Two tables ([`outbox`] and [`inbox`]) and one set of typed CRUD
//! operations against them, per v0.6 §8. Both tables are **per-domain**:
//! every federated DSL participant (bpmn-lite, ob-poc, dmn-lite, …)
//! runs the migrations on its own Postgres and drives the same
//! operations from its own sender / receiver loops.
//!
//! ## Sender side (§8.5)
//!
//! The outbox sender enqueues an [`OutboxEntry`] when a verb invocation
//! or result delivery commits locally:
//!
//! - [`insert_outbox`] — writes a pending row; `idempotency_key` +
//!   `target_endpoint` are unique, so re-enqueueing the same payload is
//!   a no-op via [`InsertOutcome`].
//! - [`select_pending_outbox`] — claims up to `limit` rows for dispatch
//!   using `FOR UPDATE SKIP LOCKED`; **must run inside a transaction**.
//! - [`mark_outbox_submitted`] — records the receiver's
//!   `execution_id` and transitions `status → submitted`.
//! - [`mark_outbox_retry`] — bumps `attempt_count`, schedules
//!   `next_attempt_at`, records `last_error`; status returns to
//!   `pending` so the row appears on the next sender sweep.
//!
//! ## Receiver side (§8.6)
//!
//! The receiver's gRPC handler consults the inbox before doing any work:
//!
//! - [`insert_inbox`] — typed insert that returns `false` if the
//!   `idempotency_key` is already present (idempotent receive).
//! - [`lookup_inbox`] — returns the recorded [`InboxEntry`] so the
//!   handler can replay the `SubmissionAck` for a duplicate request.
//! - [`mark_inbox_processed`] — marks the row processed once the
//!   downstream effect commits.
//!
//! ## Executor parameter
//!
//! Most CRUD takes `impl sqlx::PgExecutor<'_>` — pass `&pool`,
//! `&mut *tx`, or any other Postgres executor.
//! [`select_pending_outbox`] is the exception: it requires
//! `&mut sqlx::PgConnection` because the `FOR UPDATE SKIP LOCKED`
//! semantics only hold *inside a transaction*, and pinning the
//! connection at the type level makes that obvious at the call site.

#![forbid(unsafe_code)]

mod inbox;
mod outbox;
mod types;

pub use inbox::{insert_inbox, lookup_inbox, mark_inbox_processed};
pub use outbox::{
    insert_outbox, mark_outbox_retry, mark_outbox_submitted, select_pending_outbox,
};
pub use types::{
    BusEndpoint, BusStorageError, InboxEntry, InboxStatus, InsertOutcome, OutboxEntry,
    OutboxStatus, Result,
};

#[cfg(test)]
mod tests;
