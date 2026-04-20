//! `PgTransactionScope` — ob-poc's concrete impl of
//! [`dsl_runtime::tx::TransactionScope`] wrapping a `sqlx::Transaction`.
//!
//! The Sequencer opens a `sqlx::Transaction` at stage 8, wraps it in a
//! [`PgTransactionScope`], and (in the future Phase 5c-migrate) passes
//! the scope as `&mut dyn TransactionScope` through dispatch. Phase 5c
//! proper — the op-level migration that actually adopts the scope in
//! plugin bodies — is a separate future slice. This file lands as part
//! of Phase 5c-prep so the primitive exists and the Sequencer can start
//! threading it through its stage-8 boundary.
//!
//! # Scope lifecycle
//!
//! ```ignore
//! let mut scope = PgTransactionScope::begin(&pool).await?;
//! // ... runtime dispatches against `&mut scope as &mut dyn TransactionScope` ...
//! if step_ok {
//!     scope.commit().await?;  // consumes self, commits the txn
//! } else {
//!     scope.rollback().await?; // consumes self, rolls back
//! }
//! ```
//!
//! # Why the concrete impl is here (ob-poc) rather than dsl-runtime
//!
//! Per v0.3 §10.3 (2026-04-20 correction): the trait lives in
//! `dsl-runtime::tx`; the concrete impl that opens sqlx transactions
//! lives with the Sequencer (which owns scope per §7.1). dsl-runtime
//! already has sqlx as a dep, so the impl would compile there too —
//! but architecturally the txn-opener is a composition-plane concern
//! (the Sequencer), not a runtime-plane concern.

use dsl_runtime::tx::TransactionScope;
use ob_poc_types::TransactionScopeId;
use sqlx::{PgPool, Postgres, Transaction};

/// Postgres-backed transaction scope. Owns a `sqlx::Transaction` and
/// stable [`TransactionScopeId`]; implements [`TransactionScope`].
pub struct PgTransactionScope {
    tx: Transaction<'static, Postgres>,
    id: TransactionScopeId,
}

impl PgTransactionScope {
    /// Begin a new transaction on `pool` and wrap it in a scope. The
    /// scope id is generated fresh — callers can retrieve it via
    /// [`TransactionScope::scope_id`] for logging before any statement
    /// runs.
    pub async fn begin(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let tx = pool.begin().await?;
        Ok(Self {
            tx,
            id: TransactionScopeId::new(),
        })
    }

    /// Commit the transaction. Consumes the scope.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }

    /// Roll the transaction back. Consumes the scope.
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.tx.rollback().await
    }
}

impl TransactionScope for PgTransactionScope {
    fn scope_id(&self) -> TransactionScopeId {
        self.id
    }

    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> {
        &mut self.tx
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;

    // The begin / commit / rollback round-trip is exercised in the
    // db-integration test suite (`tests/sequencer_tx_integration.rs`)
    // against a live pool. Here we only prove the type shape compiles
    // and implements the trait — no live connection needed.

    #[test]
    fn impls_transaction_scope_trait() {
        fn assert_impl<T: TransactionScope>() {}
        assert_impl::<PgTransactionScope>();
    }

    #[test]
    fn dyn_compatible() {
        fn takes_dyn(_: &mut dyn TransactionScope) {}
        let _ = takes_dyn;
    }
}
