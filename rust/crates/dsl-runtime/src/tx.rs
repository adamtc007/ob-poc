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
//! # Phase 5c
//!
//! Phase 5c will extend this trait with executor-access, and ob-poc will
//! provide a `sqlx::Transaction`-backed impl. The trait lives here so
//! that extension is a dsl-runtime-local decision, not a boundary-crate
//! contract change.

use ob_poc_types::TransactionScopeId;

/// A transaction-scope handle supplied by the Sequencer to the runtime at
/// dispatch time.
///
/// Phase 0b surface: just the correlating [`TransactionScopeId`]. Phase
/// 5c will add an executor-access method (`fn executor(&mut self) -> &mut
/// dyn sqlx::PgExecutor`) and ob-poc will supply a concrete
/// `PgTransactionScope` wrapping `sqlx::Transaction`. For now the trait
/// exists to lock in the correlation-id contract and to force every
/// scope-touching site to flow through a single trait-object boundary.
pub trait TransactionScope: Send + Sync {
    /// Scope id, for logs and traces. Available regardless of storage
    /// backend.
    fn scope_id(&self) -> TransactionScopeId;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeScope(TransactionScopeId);

    impl TransactionScope for FakeScope {
        fn scope_id(&self) -> TransactionScopeId {
            self.0
        }
    }

    #[test]
    fn trait_object_dispatch_works() {
        let id = TransactionScopeId::new();
        let scope: Box<dyn TransactionScope> = Box::new(FakeScope(id));
        assert_eq!(scope.scope_id(), id);
    }

    #[test]
    fn trait_is_object_safe() {
        // Proof-by-compilation: if `dyn TransactionScope` compiles, the
        // trait is object-safe. A borrow suffices; construction is via
        // the FakeScope above.
        fn takes_dyn(_: &dyn TransactionScope) {}
        let scope = FakeScope(TransactionScopeId::new());
        takes_dyn(&scope);
    }
}
