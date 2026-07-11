//! `PgTransactionScope` — ob-poc's concrete impl of
//! [`TransactionScope`] wrapping a `sqlx::Transaction`.
//!
//! The Sequencer opens a `sqlx::Transaction` at stage 8, wraps it in a
//! [`PgTransactionScope`], and passes the scope as
//! `&mut dyn TransactionScope` through dispatch. Phase 5c-migrate Phase B
//! drives ops through [`TransactionScope::executor`] for statement
//! execution and [`TransactionScope::pool`] for services whose
//! Phase 5a-era bridge methods still take `&PgPool`.
//!
//! # Scope lifecycle
//!
//! ```ignore
//! let mut scope = PgTransactionScope::begin(&pool).await?;
//! // ... runtime dispatches against `&mut scope as &mut dyn TransactionScope` ...
//! if step_ok {
//!     scope.commit().await?;   // consumes self, commits the txn
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

use dsl_runtime::TransactionScope;
use ob_poc_control_plane::write_set::WriteSetProof;
use ob_poc_control_plane::write_set_attestation::{attest, AttestationOutcome, CapturedWrite};
use ob_poc_types::TransactionScopeId;
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;

/// Postgres-backed transaction scope. Owns a `sqlx::Transaction`, a
/// clone of the pool the txn was opened on, and a stable
/// [`TransactionScopeId`]; implements [`TransactionScope`].
///
/// The pool is held alongside the transaction so
/// [`TransactionScope::pool`] can return a reference without
/// allocation. Pool clones are Arc-internal — cheap.
///
/// `captured_writes`/`expected_write_set` are T5.1/T5.2
/// (EOP-PLAN-CONTROLPLANE-001): additive fields, empty/`None` by default,
/// with zero effect on the existing `commit()`/`rollback()` methods — only
/// the new `commit_attested()` reads them. No `SemOsVerbOp` calls
/// `record_write` today (that's real, unwired follow-on instrumentation
/// work, same posture as every other T1-T5 mechanism this plan has landed
/// without flipping a production path); this scope is proven correct via
/// its own fault-injection tests, which call `record_write` directly.
pub struct PgTransactionScope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
    captured_writes: Vec<CapturedWrite>,
    expected_write_set: Option<WriteSetProof>,
}

impl PgTransactionScope {
    /// Begin a new transaction on `pool` and wrap it in a scope. The
    /// scope id is generated fresh — callers can retrieve it via
    /// [`TransactionScope::scope_id`] for logging before any statement
    /// runs.
    pub async fn begin(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let tx = pool.begin().await?;
        // T11.0 (EOP-PLAN-CONTROLPLANE-002): C2 provenance metric. Always
        // `false` today — no CP-issued marker exists to be present until
        // T11.2's keyed doors land; see capability_provenance.rs's module
        // doc for why that's the expected, honest state pre-L2.
        crate::agent::capability_provenance::record_capability_invocation(
            "PgTransactionScope::begin",
            false,
        );
        Ok(Self {
            tx,
            pool: pool.clone(),
            id: TransactionScopeId::new(),
            captured_writes: Vec::new(),
            expected_write_set: None,
        })
    }

    /// Begin a new transaction with a wall-clock timeout on the pool.begin() call.
    ///
    /// Returns `Err` if the pool cannot acquire a connection within `timeout`.
    /// Callers should use this instead of `begin()` in production dispatch paths
    /// to prevent indefinite hangs when the connection pool is exhausted.
    pub async fn begin_timeout(pool: &PgPool, timeout: Duration) -> Result<Self, anyhow::Error> {
        let result = tokio::time::timeout(timeout, pool.begin())
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "pool.begin() timed out after {:?} — connection pool likely exhausted",
                    timeout
                )
            })?
            .map(|tx| Self {
                tx,
                pool: pool.clone(),
                id: TransactionScopeId::new(),
                captured_writes: Vec::new(),
                expected_write_set: None,
            })
            .map_err(Into::into);
        // T11.0: same instrumentation as `begin()`, recorded regardless of
        // outcome — a timed-out acquisition attempt is still a capability
        // invocation attempt, and undercounting attempts would understate
        // the mesh remainder, not just its provenance split.
        crate::agent::capability_provenance::record_capability_invocation(
            "PgTransactionScope::begin_timeout",
            false,
        );
        result
    }

    /// Commit the transaction. Consumes the scope. Unattested — does not
    /// consult `captured_writes`/`expected_write_set` even if set; prefer
    /// [`Self::commit_attested`] when a `WriteSetProof` bound is available.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }

    /// Roll the transaction back. Consumes the scope.
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.tx.rollback().await
    }

    /// T5.2: attach the declared bound this scope's writes must stay
    /// within. Optional — a scope with no expected write set behaves
    /// identically under `commit_attested` and `commit` (nothing to
    /// compare against, so it commits unconditionally); this is the
    /// backward-compatible default for every caller that doesn't opt in.
    pub fn set_expected_write_set(&mut self, proof: WriteSetProof) {
        self.expected_write_set = Some(proof);
    }

    /// T5.2/T5.3: commit only if every captured write is covered by the
    /// expected write set (when one was attached via
    /// [`Self::set_expected_write_set`]); otherwise this behaves exactly
    /// like [`Self::commit`]. On breach, rolls back instead of committing
    /// — the exit criterion's "aborts... no durable row" — and returns
    /// `Err(CommitAttestationError::Breach)` carrying every excess write.
    /// Persists a `control_plane_write_attestations` row (best-effort,
    /// T5.3) in both the bounded and breach cases when an expectation was
    /// set; persistence failure never masks the commit/rollback outcome
    /// itself (attestation is audit trail, not the enforcement mechanism —
    /// the transaction boundary is).
    pub async fn commit_attested(
        self,
        session_id: Option<Uuid>,
        verb_fqn: Option<&str>,
    ) -> Result<(), CommitAttestationError> {
        let Some(expected) = self.expected_write_set.clone() else {
            self.tx.commit().await.map_err(CommitAttestationError::Db)?;
            return Ok(());
        };

        let outcome = attest(&self.captured_writes, &expected);
        let scope_id = self.id;
        let pool = self.pool.clone();
        let captured = self.captured_writes.clone();

        match outcome {
            AttestationOutcome::Bounded => {
                self.tx.commit().await.map_err(CommitAttestationError::Db)?;
                crate::agent::control_plane_write_attestation_store::persist_attestation(
                    &pool,
                    scope_id,
                    session_id,
                    verb_fqn,
                    &captured,
                    true,
                    &[],
                )
                .await;
                Ok(())
            }
            AttestationOutcome::Breach { excess } => {
                // Roll back FIRST — the exit criterion is "no durable
                // row," so the abort must happen before anything else,
                // not be racing the attestation-record insert.
                self.tx.rollback().await.map_err(CommitAttestationError::Db)?;
                crate::agent::control_plane_write_attestation_store::persist_attestation(
                    &pool,
                    scope_id,
                    session_id,
                    verb_fqn,
                    &captured,
                    false,
                    &excess,
                )
                .await;
                Err(CommitAttestationError::Breach { excess })
            }
        }
    }
}

/// Outcome of [`PgTransactionScope::commit_attested`].
#[derive(Debug)]
pub enum CommitAttestationError {
    /// The commit or rollback itself failed at the database layer.
    Db(sqlx::Error),
    /// The transaction was rolled back because captured writes exceeded
    /// the declared `WriteSetProof` bound.
    Breach { excess: Vec<CapturedWrite> },
}

impl std::fmt::Display for CommitAttestationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitAttestationError::Db(e) => write!(f, "commit_attested: db error: {e}"),
            CommitAttestationError::Breach { excess } => {
                write!(f, "commit_attested: write-set breach, {} excess write(s), rolled back", excess.len())
            }
        }
    }
}

impl std::error::Error for CommitAttestationError {}

impl TransactionScope for PgTransactionScope {
    fn scope_id(&self) -> TransactionScopeId {
        self.id
    }

    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> {
        &mut self.tx
    }

    fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// T5.1 (promoted onto the trait by T10.3 — see `dsl-runtime::tx`'s
    /// module doc for why): self-report a write this scope's caller
    /// performed via `scope.executor()`/`scope.transaction()`.
    /// `commit_attested` can only catch what gets reported here.
    fn record_write(&mut self, table: &str, entity_id: Uuid, columns: &[String]) {
        self.captured_writes.push(CapturedWrite {
            table: table.to_string(),
            entity_id,
            columns: columns.to_vec(),
        });
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

/// T5 exit criterion, verbatim: "fault-injection test — op writing an
/// undeclared table aborts with breach event, no durable row." Uses the
/// real `PgTransactionScope` against a live database — not a mock — and
/// writes into an existing, low-risk control-plane table
/// (`"ob-poc".control_plane_envelopes`) rather than a business table, so
/// the test needs no throwaway schema and cannot corrupt production-shaped
/// data.
#[cfg(all(test, feature = "database"))]
mod t5_write_set_attestation_tests {
    use super::*;

    async fn test_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        PgPool::connect(&url).await.expect("connect")
    }

    async fn envelope_row_count(pool: &PgPool, envelope_id: Uuid) -> i64 {
        sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".control_plane_envelopes WHERE envelope_id = $1"#)
            .bind(envelope_id)
            .fetch_one(pool)
            .await
            .expect("count query")
    }

    /// The exit criterion itself: an op declares a write-set bound that
    /// does NOT cover `control_plane_envelopes`, then (honestly)
    /// self-reports a write to it via `record_write`, then actually
    /// performs that write via `scope.executor()` — proving both that
    /// `commit_attested` detects the breach AND that the real SQL insert,
    /// despite "succeeding" at the statement level, never becomes durable
    /// because the whole transaction rolls back.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn write_to_undeclared_table_aborts_with_no_durable_row() {
        let pool = test_pool().await;
        let envelope_id = Uuid::new_v4();

        let mut scope = PgTransactionScope::begin(&pool).await.expect("begin");
        // Declared bound covers only "ob-poc.cbus" — control_plane_envelopes is undeclared.
        let expected = ob_poc_control_plane::write_set::tests_support::proof(
            vec![],
            vec![],
            vec!["ob-poc.cbus".to_string()],
            vec!["status".to_string()],
            "fault-injection-1",
        );
        scope.set_expected_write_set(expected);

        // The actual, real write — into the undeclared table.
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, 'fault-injection', $2, 'test.undeclared-write', 'sealed', now(), now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(Uuid::new_v4())
        .execute(scope.executor())
        .await
        .expect("the raw INSERT statement itself succeeds inside the still-open transaction");

        scope.record_write(
            "ob-poc.control_plane_envelopes",
            envelope_id,
            &["status".to_string()],
        );

        let result = scope.commit_attested(None, Some("test.undeclared-write")).await;
        assert!(
            matches!(result, Err(CommitAttestationError::Breach { .. })),
            "expected a Breach, got {result:?}"
        );

        // The exit criterion's second half: no durable row, despite the
        // INSERT having "succeeded" moments ago inside the transaction.
        assert_eq!(
            envelope_row_count(&pool, envelope_id).await,
            0,
            "the transaction must have rolled back — the row must not be durable"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn write_within_declared_bound_commits_durably() {
        let pool = test_pool().await;
        let envelope_id = Uuid::new_v4();

        let mut scope = PgTransactionScope::begin(&pool).await.expect("begin");
        let expected = ob_poc_control_plane::write_set::tests_support::proof(
            vec![envelope_id],
            vec![],
            vec!["ob-poc.control_plane_envelopes".to_string()],
            vec!["status".to_string()],
            "fault-injection-2",
        );
        scope.set_expected_write_set(expected);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, 'fault-injection', $2, 'test.declared-write', 'sealed', now(), now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(Uuid::new_v4())
        .execute(scope.executor())
        .await
        .expect("insert");

        scope.record_write(
            "ob-poc.control_plane_envelopes",
            envelope_id,
            &["status".to_string()],
        );

        scope
            .commit_attested(None, Some("test.declared-write"))
            .await
            .expect("write within the declared bound must commit, not breach");

        assert_eq!(
            envelope_row_count(&pool, envelope_id).await,
            1,
            "a bounded write must actually persist"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn no_expected_write_set_commits_unconditionally_like_plain_commit() {
        let pool = test_pool().await;
        let envelope_id = Uuid::new_v4();

        let mut scope = PgTransactionScope::begin(&pool).await.expect("begin");
        // No set_expected_write_set() call — backward-compatible default.
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".control_plane_envelopes (
                envelope_id, content_hash, session_id, verb_fqn,
                status, not_before, not_after
            ) VALUES ($1, 'fault-injection', $2, 'test.unbounded-write', 'sealed', now(), now() + interval '5 minutes')
            "#,
        )
        .bind(envelope_id)
        .bind(Uuid::new_v4())
        .execute(scope.executor())
        .await
        .expect("insert");

        scope
            .commit_attested(None, None)
            .await
            .expect("no expectation set -> commits unconditionally, same as plain commit()");

        assert_eq!(envelope_row_count(&pool, envelope_id).await, 1);
    }

    /// T11.0: proves `PgTransactionScope::begin` — the real production
    /// entry point, not the in-process counter module tested in isolation
    /// — actually increments the C2 provenance metric on every real scope
    /// open, with `has_cp_provenance = false` (honest, since no marker
    /// exists yet pre-T11.2). Reads the snapshot before and after to
    /// isolate this test's own contribution from whatever else in the
    /// process (other tests, if run non-serially) also opened a scope.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn begin_records_a_capability_invocation_without_cp_provenance() {
        let pool = test_pool().await;

        let before = crate::agent::capability_provenance::capability_invocations_without_cp_provenance()
            .into_iter()
            .find(|r| r.capability_entry == "PgTransactionScope::begin")
            .map(|r| r.without_provenance)
            .unwrap_or(0);

        let scope = PgTransactionScope::begin(&pool).await.expect("begin");
        scope.rollback().await.expect("rollback");

        let after = crate::agent::capability_provenance::capability_invocations_without_cp_provenance()
            .into_iter()
            .find(|r| r.capability_entry == "PgTransactionScope::begin")
            .map(|r| r.without_provenance)
            .unwrap_or(0);

        assert_eq!(
            after,
            before + 1,
            "a real PgTransactionScope::begin() call must record exactly one \
             without-provenance capability invocation"
        );
    }
}
