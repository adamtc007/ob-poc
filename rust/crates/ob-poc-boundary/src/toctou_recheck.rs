//! Phase D.3 TOCTOU recheck scaffold (2026-04-22).
//!
//! Closes the TOCTOU window between gate decision and write. The
//! flow:
//!
//! 1. At gate time (Phase A envelope build), a
//!    [`StateGateHash`](ob_poc_types::StateGateHash) is computed over
//!    `(entity_id, row_version)` tuples plus workspace/catalogue ids.
//! 2. Between gate decision and execution, another transaction could
//!    `UPDATE` an entity row, bumping its `row_version` via the
//!    migration trigger (see
//!    `rust/migrations/20260422_row_version_entity_tables.sql`).
//! 3. After the Sequencer (B.2b) acquires advisory locks inside its
//!    outer transaction — which blocks concurrent writes — it calls
//!    [`verify_toctou`] to re-read `row_version` for each resolved
//!    entity and recompute the hash. If it differs from the envelope's
//!    `authorisation.state_gate_hash`, the state drifted, and the
//!    runbook rolls back with a [`ToctouDrift`] error.
//!
//! ## Dependency status
//!
//! - `row_version` column exists per entity table (Phase D.2 migration
//!   `20260422_row_version_entity_tables.sql` — **staged, pending
//!   operator approval** for zero-downtime backfill under live
//!   traffic). Dev environments can apply the migration directly;
//!   production needs a coordinated rollout.
//! - Real `GatedVerbEnvelope` construction at stage 6 (not yet wired;
//!   only shadow envelopes are emitted today from `envelope_builder.rs`).
//! - Sequencer B.2b outer scope landed (`execute_runbook_in_scope`),
//!   so the integration point for `verify_toctou` exists.
//!
//! This module is **exercisable today** via the mock
//! [`RowVersionProvider`] used in the unit tests. Production wiring
//! requires the three dependencies above.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_types::gated_envelope::state_gate_hash;
use ob_poc_types::{
    GatedVerbEnvelope, ResolvedEntities, ResolvedEntity, StateGateHash, WorkspaceSnapshotId,
};
use uuid::Uuid;

/// Abstract source of current `row_version` values per entity.
///
/// The production implementation reads from the DB inside the
/// Sequencer's outer scope (so the read sees the post-lock state).
/// Tests use an in-memory map.
#[async_trait]
pub trait RowVersionProvider: Send + Sync {
    /// Return the current `row_version` for the given entity.
    ///
    /// The `entity_kind` discriminator selects which table to read
    /// — the migration added `row_version` to `cbus`, `entities`,
    /// `cases`, `deals`, and `client_group`. Other kinds return an
    /// error (caller must surface to the Sequencer which aborts the
    /// runbook).
    async fn row_version(&self, entity_id: Uuid, entity_kind: &str) -> Result<u64>;
}

/// TOCTOU drift — the current state hash doesn't match the envelope's
/// expected hash. Caller (Sequencer) rolls back the outer scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToctouDrift {
    pub expected: StateGateHash,
    pub actual: StateGateHash,
}

impl std::fmt::Display for ToctouDrift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TOCTOU drift: expected state hash {:?}, actual {:?}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for ToctouDrift {}

/// Re-read `row_version` for every resolved entity in the envelope,
/// recompute the `StateGateHash` using the same envelope inputs
/// (dag_node_id, session_scope, workspace_snapshot_id,
/// catalogue_snapshot_id), and compare to the envelope's recorded
/// hash.
///
/// Returns `Ok(())` on match. Returns `Err(ToctouDrift)` when
/// `row_version` for any entity has changed since gate time — the
/// aggregate hash diverges.
/// Recompute the state gate hash from current DB state and compare to
/// the envelope's recorded value.
///
/// The envelope alone doesn't carry `workspace_snapshot_id` as a
/// top-level field (it's derived from `session_id` at build time per
/// `envelope_builder.rs`), so the caller passes it explicitly. Use
/// the same value that went into the original
/// `state_gate_hash::hash(...)` call at gate time.
pub async fn verify_toctou(
    envelope: &GatedVerbEnvelope,
    workspace_snapshot_id: WorkspaceSnapshotId,
    provider: &dyn RowVersionProvider,
) -> Result<(), anyhow::Error> {
    // Read current row_version for every entity.
    let mut current_entities = Vec::with_capacity(envelope.resolved_entities.0.len());
    for e in &envelope.resolved_entities.0 {
        let current_rv = provider
            .row_version(e.entity_id, &e.entity_kind)
            .await
            .map_err(|err| anyhow!("row_version lookup failed for {}: {}", e.entity_id, err))?;
        current_entities.push(ResolvedEntity {
            entity_id: e.entity_id,
            entity_kind: e.entity_kind.clone(),
            row_version: current_rv,
        });
    }

    let current_resolved = ResolvedEntities::sorted(current_entities);

    // Recompute the hash using the same envelope inputs (everything
    // except entity row_versions is stable between gate time and now).
    let current_hash = state_gate_hash::hash(
        envelope.envelope_version,
        &current_resolved,
        envelope.dag_position,
        envelope.dag_node_version,
        envelope.authorisation.session_scope,
        workspace_snapshot_id,
        envelope.catalogue_snapshot_id,
    );

    if current_hash == envelope.authorisation.state_gate_hash {
        Ok(())
    } else {
        Err(anyhow!(ToctouDrift {
            expected: envelope.authorisation.state_gate_hash,
            actual: current_hash,
        }))
    }
}

// ---------------------------------------------------------------------------
// Production SQL-backed provider (behind #[cfg(feature = "database")]).
// Requires the D.2 migration to be applied to the target database.
// ---------------------------------------------------------------------------

#[cfg(feature = "database")]
pub struct SqlRowVersionProvider<'a> {
    pub pool: &'a sqlx::PgPool,
}

#[cfg(feature = "database")]
#[async_trait]
impl<'a> RowVersionProvider for SqlRowVersionProvider<'a> {
    async fn row_version(&self, entity_id: Uuid, entity_kind: &str) -> Result<u64> {
        // Table + primary-key column per kind. The migration covers
        // five tables; unknown kinds surface as an actionable error.
        let (table, pk) = match entity_kind {
            "cbu" => ("cbus", "cbu_id"),
            "entity" => ("entities", "entity_id"),
            "case" => ("cases", "case_id"),
            "deal" => ("deals", "deal_id"),
            "client_group" => ("client_group", "id"),
            other => {
                return Err(anyhow!(
                    "toctou_recheck: row_version not available for entity_kind `{}` \
                     — migration 20260422_row_version_entity_tables.sql only covers \
                     cbu / entity / case / deal / client_group. Extend the migration \
                     OR add this kind to the gate-surface audit.",
                    other
                ))
            }
        };

        let sql = format!(
            r#"SELECT row_version FROM "ob-poc".{} WHERE {} = $1"#,
            table, pk
        );

        let rv: i64 = sqlx::query_scalar(&sql)
            .bind(entity_id)
            .fetch_one(self.pool)
            .await
            .map_err(|e| {
                anyhow!(
                    "toctou_recheck: row_version lookup in `{}` for entity {} failed: {}",
                    table,
                    entity_id,
                    e
                )
            })?;

        // row_version is declared NOT NULL DEFAULT 1 bigint, so safe
        // to cast. Negative would indicate schema corruption.
        if rv < 0 {
            return Err(anyhow!(
                "toctou_recheck: negative row_version {} for entity {} in `{}` — schema corruption?",
                rv,
                entity_id,
                table
            ));
        }
        Ok(rv as u64)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use ob_poc_types::{
        AuthorisationProof, CatalogueSnapshotId, ClosedLoopMarker, DagNodeId, DagNodeVersion,
        DiscoverySignals, EnvelopeVersion, LogicalClock, SessionScopeRef, VerbArgs, VerbRef,
        WorkspaceSnapshotId,
    };

    /// In-memory row_version map for tests.
    struct MockProvider(Mutex<HashMap<Uuid, u64>>);

    #[async_trait]
    impl RowVersionProvider for MockProvider {
        async fn row_version(&self, entity_id: Uuid, _kind: &str) -> Result<u64> {
            self.0
                .lock()
                .unwrap()
                .get(&entity_id)
                .copied()
                .ok_or_else(|| anyhow!("no row_version for {}", entity_id))
        }
    }

    fn make_envelope(entity_id: Uuid, row_version_at_gate: u64) -> GatedVerbEnvelope {
        // Build ResolvedEntities with the given row_version.
        let resolved = ResolvedEntities::sorted(vec![ResolvedEntity {
            entity_id,
            entity_kind: "cbu".into(),
            row_version: row_version_at_gate,
        }]);

        // Compute the hash the gate would have produced.
        let envelope_version = EnvelopeVersion::CURRENT;
        let dag_position = DagNodeId(Uuid::nil());
        let dag_node_version = DagNodeVersion(1);
        let session_scope = SessionScopeRef(Uuid::nil());
        let workspace_snapshot_id = WorkspaceSnapshotId(Uuid::nil());
        let catalogue_snapshot_id = CatalogueSnapshotId(1);

        let state_gate_hash = state_gate_hash::hash(
            envelope_version,
            &resolved,
            dag_position,
            dag_node_version,
            session_scope,
            workspace_snapshot_id,
            catalogue_snapshot_id,
        );

        GatedVerbEnvelope {
            envelope_version,
            catalogue_snapshot_id,
            verb: VerbRef("test.verb".into()),
            args: VerbArgs::new(serde_json::Value::Null),
            dag_position,
            dag_node_version,
            resolved_entities: resolved,
            trace_id: ob_poc_types::TraceId(Uuid::nil()),
            authorisation: AuthorisationProof {
                issued_at: LogicalClock(0),
                session_scope,
                state_gate_hash,
                recheck_required: true,
            },
            discovery_signals: DiscoverySignals::default(),
            closed_loop_marker: ClosedLoopMarker {
                writes_since_push_at_gate: 0,
            },
        }
    }

    #[tokio::test]
    async fn verify_matches_when_row_version_unchanged() {
        let entity_id = Uuid::new_v4();
        let envelope = make_envelope(entity_id, 42);
        let workspace_snapshot_id = WorkspaceSnapshotId(Uuid::nil());

        let mut map = HashMap::new();
        map.insert(entity_id, 42);
        let provider = MockProvider(Mutex::new(map));

        verify_toctou(&envelope, workspace_snapshot_id, &provider)
            .await
            .expect("same row_version → no drift");
    }

    #[tokio::test]
    async fn verify_detects_drift_when_row_version_bumped() {
        let entity_id = Uuid::new_v4();
        let envelope = make_envelope(entity_id, 42);
        let workspace_snapshot_id = WorkspaceSnapshotId(Uuid::nil());

        // Concurrent writer bumped row_version from 42 to 43.
        let mut map = HashMap::new();
        map.insert(entity_id, 43);
        let provider = MockProvider(Mutex::new(map));

        let err = verify_toctou(&envelope, workspace_snapshot_id, &provider)
            .await
            .expect_err("row_version changed → drift expected");

        // Error wraps the typed ToctouDrift.
        let drift = err
            .downcast_ref::<ToctouDrift>()
            .expect("error should be ToctouDrift");
        assert_eq!(drift.expected, envelope.authorisation.state_gate_hash);
        assert_ne!(drift.actual, drift.expected);
    }

    #[tokio::test]
    async fn verify_surfaces_provider_error_when_row_missing() {
        let entity_id = Uuid::new_v4();
        let envelope = make_envelope(entity_id, 1);
        let workspace_snapshot_id = WorkspaceSnapshotId(Uuid::nil());

        // Empty map — provider will error.
        let provider = MockProvider(Mutex::new(HashMap::new()));

        let err = verify_toctou(&envelope, workspace_snapshot_id, &provider)
            .await
            .expect_err("missing entity → provider error propagates");
        assert!(err.to_string().contains("row_version lookup failed"));
    }

    #[tokio::test]
    async fn verify_with_multiple_entities_all_must_match() {
        let e1 = Uuid::new_v4();
        let e2 = Uuid::new_v4();

        // Build envelope with two entities at row_versions (5, 10).
        let resolved = ResolvedEntities::sorted(vec![
            ResolvedEntity {
                entity_id: e1,
                entity_kind: "cbu".into(),
                row_version: 5,
            },
            ResolvedEntity {
                entity_id: e2,
                entity_kind: "entity".into(),
                row_version: 10,
            },
        ]);
        let envelope_version = EnvelopeVersion::CURRENT;
        let dag_position = DagNodeId(Uuid::nil());
        let dag_node_version = DagNodeVersion(1);
        let session_scope = SessionScopeRef(Uuid::nil());
        let workspace_snapshot_id = WorkspaceSnapshotId(Uuid::nil());
        let catalogue_snapshot_id = CatalogueSnapshotId(1);

        let state_gate_hash = state_gate_hash::hash(
            envelope_version,
            &resolved,
            dag_position,
            dag_node_version,
            session_scope,
            workspace_snapshot_id,
            catalogue_snapshot_id,
        );

        let envelope = GatedVerbEnvelope {
            envelope_version,
            catalogue_snapshot_id,
            verb: VerbRef("test.verb".into()),
            args: VerbArgs::new(serde_json::Value::Null),
            dag_position,
            dag_node_version,
            resolved_entities: resolved,
            trace_id: ob_poc_types::TraceId(Uuid::nil()),
            authorisation: AuthorisationProof {
                issued_at: LogicalClock(0),
                session_scope,
                state_gate_hash,
                recheck_required: true,
            },
            discovery_signals: DiscoverySignals::default(),
            closed_loop_marker: ClosedLoopMarker {
                writes_since_push_at_gate: 0,
            },
        };

        // Case 1: both unchanged → match.
        let mut map = HashMap::new();
        map.insert(e1, 5);
        map.insert(e2, 10);
        let provider = MockProvider(Mutex::new(map));
        verify_toctou(&envelope, workspace_snapshot_id, &provider)
            .await
            .expect("match");

        // Case 2: e1 bumped → drift.
        let mut map = HashMap::new();
        map.insert(e1, 6); // bumped
        map.insert(e2, 10);
        let provider = MockProvider(Mutex::new(map));
        let err = verify_toctou(&envelope, workspace_snapshot_id, &provider).await;
        assert!(err.is_err(), "one entity drifted → drift detected");
    }
}
