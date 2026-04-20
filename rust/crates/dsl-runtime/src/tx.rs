//! Transaction-scope contract between the Sequencer (ob-poc) and the runtime (dsl-runtime).
//!
//! The Sequencer owns *scope* — when a transaction begins, when it commits,
//! when it rolls back, what the boundary includes. The runtime owns
//! *mechanics* — statement execution, pool checkout, row accounting,
//! deadlock retries — inside the scope the Sequencer supplies. Both are
//! primary owners of non-overlapping concerns (per three-plane v0.3 §7.1).
//!
//! # Layering (2026-04-20 correction)
//!
//! The v0.3 spec originally placed this trait in `ob-poc-types`. That
//! created a latent contradiction: the boundary crate is supposed to be
//! logic-free and carry values only, but an executor-access method forces
//! it to depend on `sqlx` (or whatever future runtime backend). The
//! contradiction was caught at Phase 0b (the trait was defined
//! `scope_id()`-only and `executor()` deferred), but the clean resolution
//! is to move the trait here — the execution-plane crate — and leave
//! `ob-poc-types` with only the data-shaped [`TransactionScopeId`]
//! newtype.
//!
//! # Phase 5c-prep (2026-04-20)
//!
//! Phase 5c is split into two sub-slices (see v0.3 §13):
//!
//! - **5c-prep (this commit):** extend the trait with
//!   [`TransactionScope::transaction`], add the concrete
//!   [`crate::tx::PgTransactionScope`]-shaped impl in ob-poc (see
//!   `ob_poc::sequencer_tx`), document the new method on the trait.
//!   Plugin op signatures are unchanged; the new method exists so the
//!   Sequencer can begin/commit a txn and pass a trait-object scope
//!   through dispatch, ready for 5c-migrate.
//! - **5c-migrate (future):** mass-rewrite `CustomOperation::execute_json`
//!   signatures from `pool: &PgPool` to `scope: &mut dyn TransactionScope`,
//!   adopt the trait method inside every plugin op body. Staged per-op
//!   or per-domain, not big-bang.
//!
//! Phase 0b rationale for trait object safety is preserved: the trait is
//! dyn-compatible today and stays that way post-extension. The
//! `transaction` method returns a concrete
//! `&mut sqlx::Transaction<'static, Postgres>` — every production
//! transaction in ob-poc begins via `pool.begin()` which yields a
//! `'static` lifetime (connection owned by the pool).

use ob_poc_types::TransactionScopeId;

/// A transaction-scope handle supplied by the Sequencer to the runtime at
/// dispatch time.
///
/// # Post-5c-prep surface
///
/// - [`Self::scope_id`] — correlation id for logs / traces / replay.
///   Stable across a scope's lifetime.
/// - [`Self::transaction`] — the underlying `sqlx::Transaction`. Plugin
///   ops after 5c-migrate take `scope: &mut dyn TransactionScope` and
///   call `scope.transaction()` wherever they previously took
///   `pool: &PgPool` and used it directly.
///
/// The trait stays dyn-compatible after the extension — both methods are
/// object-safe.
pub trait TransactionScope: Send + Sync {
    /// Scope id, for logs and traces. Available regardless of storage
    /// backend. Stable across a scope's lifetime.
    fn scope_id(&self) -> TransactionScopeId;

    /// The underlying Postgres transaction handle the scope owns.
    ///
    /// Phase 5c-migrate adopts this signature: plugin ops replace their
    /// `pool: &PgPool` binding with `scope.transaction()` where needed
    /// for statement execution. Until then this method exists on the
    /// trait surface but no plugin op consumes it — the Sequencer's
    /// begin/commit path can already thread a scope through dispatch.
    ///
    /// `'static` lifetime: all production transactions begin via
    /// `PgPool::begin()` which yields `Transaction<'static, Postgres>`
    /// (the pool owns the connection). Tests may supply a different
    /// lifetime via a custom impl; the concrete `PgTransactionScope`
    /// in ob-poc standardises on `'static`.
    fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // We can't instantiate a real `sqlx::Transaction` without a live
    // database, so the dyn-compatibility proof here uses a borrowed
    // reference to a hypothetical one via a helper trait technique —
    // construction goes via the ob-poc-side `PgTransactionScope` impl
    // exercised against a real pool in ob-poc integration tests.

    #[test]
    fn trait_is_object_safe() {
        // Proof-by-compilation: if `&mut dyn TransactionScope` compiles,
        // the trait is object-safe. Both methods are object-safe (no
        // Self-by-value, no generics, no Self-in-return beyond &mut).
        fn takes_dyn(_: &mut dyn TransactionScope) {}
        // No construction — a compile-only check is sufficient. The
        // actual trait-object dispatch is exercised in ob-poc's
        // PgTransactionScope integration test where a real
        // sqlx::Transaction is available.
        let _ = takes_dyn;
    }
}
