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
//! # Phase 5c-migrate (2026-04-21)
//!
//! The trait now carries four methods:
//! - [`TransactionScope::scope_id`] — correlation id (logs / traces / replay).
//! - [`TransactionScope::transaction`] — the underlying `sqlx::Transaction`.
//! - [`TransactionScope::executor`] — convenience deref to
//!   `&mut sqlx::PgConnection`; migrated plugin ops call
//!   `sqlx::query("…").execute(scope.executor())` directly.
//! - [`TransactionScope::pool`] — transitional accessor for services
//!   whose Phase 5a-era bridge methods still take `&PgPool` directly
//!   (`PhraseService`, `AttributeService`, `ViewService`, etc.).
//!   Removed once those services adopt scope-aware signatures.
//!
//! Dyn-compatibility is preserved — every method is object-safe.

use ob_poc_types::TransactionScopeId;

/// A transaction-scope handle supplied by the Sequencer to the runtime at
/// dispatch time.
pub trait TransactionScope: Send + Sync {
    /// Scope id, for logs and traces. Available regardless of storage
    /// backend. Stable across a scope's lifetime.
    fn scope_id(&self) -> TransactionScopeId;

    /// The underlying Postgres transaction handle the scope owns.
    ///
    /// `'static` lifetime: all production transactions begin via
    /// `PgPool::begin()` which yields `Transaction<'static, Postgres>`
    /// (the pool owns the connection). Tests may supply a different
    /// lifetime via a custom impl; the concrete `PgTransactionScope`
    /// in ob-poc standardises on `'static`.
    fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres>;

    /// Convenience: the underlying `&mut PgConnection` that sqlx
    /// statement executors consume.
    ///
    /// Migrated plugin ops write `sqlx::query("…").execute(scope.executor())`
    /// and `.fetch_optional(scope.executor())` etc. Each call re-borrows,
    /// so sequential statements against the same scope compose without
    /// fighting the borrow checker.
    fn executor(&mut self) -> &mut sqlx::PgConnection {
        use std::ops::DerefMut;
        self.transaction().deref_mut()
    }

    /// Transitional accessor returning the pool the scope was opened on.
    ///
    /// Used by SemOS-migrated ops whose downstream service-trait dispatch
    /// still takes `&PgPool` (the nine Phase 5a service traits —
    /// `PhraseService`, `AttributeService`, `ViewService`, `SessionService`,
    /// `ServicePipelineService`, `StewardshipDispatch`, etc.). Queries
    /// executed via the returned pool reference acquire fresh connections
    /// — they do NOT participate in `self.transaction()`; commit/rollback
    /// on the scope has no effect on them. Removed once every service
    /// takes `&mut dyn TransactionScope` or `&mut Transaction` directly.
    fn pool(&self) -> &sqlx::PgPool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_is_object_safe() {
        // Proof-by-compilation: if `&mut dyn TransactionScope` compiles,
        // the trait is object-safe.
        fn takes_dyn(_: &mut dyn TransactionScope) {}
        let _ = takes_dyn;
    }
}
