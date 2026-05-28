//! SlotStateProvider — runtime cross-workspace slot-state lookups.
//!
//! v1.3 cross-workspace mechanisms (CrossWorkspaceConstraint Mode A,
//! DerivedCrossWorkspaceState Mode B) need to read state from one
//! workspace's slot to gate or compose with another workspace's
//! transition. Today each domain has its own SQL queries scattered
//! through ops modules. This trait gives a unified runtime contract:
//! given (workspace, slot, entity_id) → return the slot's current state.
//!
//! The trait is small and synchronous in shape but the lookups are
//! async (DB-backed). Caller threads a Postgres pool. A single entity_id
//! parameter is the intended-narrow case (lookup state for one specific
//! row); for set-of-rows or computed cardinality we'd extend with a
//! richer signature later.
//!
//! See: docs/backlog/catalogue-platform-refinement-v1_3.md §3.3
//! (runtime impact for V1.3-1 + V1.3-2)

use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Consumer-supplied slot state table
// ---------------------------------------------------------------------------

/// Key: `"workspace.slot"`, Value: `(table, status_column, pk_column)`.
pub(crate) type SlotStateTable = HashMap<String, (String, String, String)>;

static SLOT_STATE_TABLE: OnceLock<SlotStateTable> = OnceLock::new();

/// Register the consumer's slot state table.
///
/// Loaded from `config/slot_state_table.yaml` via
/// `ConfigLoader::load_slot_state_table()`. Subsequent calls are ignored
/// (OnceLock semantics).
pub fn set_slot_state_table(table: SlotStateTable) {
    let _ = SLOT_STATE_TABLE.set(table);
}

/// Read the current state of a slot for a specific entity.
#[async_trait]
pub trait SlotStateProvider: Send + Sync {
    /// Look up the current state value for (workspace, slot, entity_id).
    ///
    /// Returns:
    ///   - `Ok(Some(state))` — entity exists, state is `state`.
    ///   - `Ok(None)` — entity row exists but state column is NULL,
    ///     OR the (workspace, slot) pair has no state semantic
    ///     (stateless slot — caller should handle as "unknown").
    ///   - `Err(_)` — slot lookup not implemented for this (workspace,
    ///     slot) pair, OR entity not found, OR DB error.
    async fn read_slot_state(
        &self,
        workspace: &str,
        slot: &str,
        entity_id: Uuid,
        pool: &PgPool,
    ) -> Result<Option<String>>;
}

// ---------------------------------------------------------------------------
// Built-in Postgres-backed implementation
// ---------------------------------------------------------------------------

/// Default implementation that maps (workspace, slot) → table+column via
/// a static dispatch table.
///
/// Coverage as of T3 close: the slots that participate in declared
/// cross-workspace constraints + tollgate aggregates. Adding a new
/// (workspace, slot) lookup is one match arm.
#[derive(Debug, Default, Clone)]
pub struct PostgresSlotStateProvider;

#[async_trait]
impl SlotStateProvider for PostgresSlotStateProvider {
    async fn read_slot_state(
        &self,
        workspace: &str,
        slot: &str,
        entity_id: Uuid,
        pool: &PgPool,
    ) -> Result<Option<String>> {
        let (table, column, pk_column) = resolve_slot_table(workspace, slot)?;
        let sql = format!(
            r#"SELECT {col}::text AS state FROM "ob-poc".{tbl} WHERE {pk} = $1"#,
            col = column,
            tbl = table,
            pk = pk_column,
        );
        let row = sqlx::query(&sql)
            .bind(entity_id)
            .fetch_optional(pool)
            .await?;
        match row {
            None => Ok(None),
            Some(r) => Ok(r.try_get::<Option<String>, _>("state").unwrap_or(None)),
        }
    }
}

/// Resolve (workspace, slot) → (table, state_column, pk_column).
///
/// Looks up the consumer-registered slot state table (set via
/// [`set_slot_state_table`], loaded from `config/slot_state_table.yaml`).
/// Returns `Err` if no table is registered or the pair is not found.
pub fn resolve_slot_table(workspace: &str, slot: &str) -> Result<(String, String, String)> {
    let key = format!("{workspace}.{slot}");
    SLOT_STATE_TABLE
        .get()
        .and_then(|t| t.get(&key))
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "no SlotStateProvider mapping for ({workspace}, {slot}) — \
                 register an entry via set_slot_state_table() / slot_state_table.yaml"
            )
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_slot_known_pairs() {
        // Register a minimal ob-poc slot table for this test.
        // OnceLock: idempotent; safe across test runs.
        set_slot_state_table(
            [
                ("cbu.cbu", ("cbus", "status", "cbu_id")),
                ("kyc.kyc_case", ("cases", "status", "case_id")),
                ("deal.deal", ("deals", "deal_status", "deal_id")),
                (
                    "instrument_matrix.trading_profile",
                    ("cbu_trading_profiles", "status", "profile_id"),
                ),
                (
                    "semos_maintenance.manco",
                    (
                        "manco_regulatory_status",
                        "regulatory_status",
                        "manco_entity_id",
                    ),
                ),
            ]
            .into_iter()
            .map(|(k, (t, sc, pk))| {
                (
                    k.to_string(),
                    (t.to_string(), sc.to_string(), pk.to_string()),
                )
            })
            .collect(),
        );

        let cases = [
            (("cbu", "cbu"), ("cbus", "status", "cbu_id")),
            (("kyc", "kyc_case"), ("cases", "status", "case_id")),
            (("deal", "deal"), ("deals", "deal_status", "deal_id")),
            (
                ("instrument_matrix", "trading_profile"),
                ("cbu_trading_profiles", "status", "profile_id"),
            ),
            (
                ("semos_maintenance", "manco"),
                (
                    "manco_regulatory_status",
                    "regulatory_status",
                    "manco_entity_id",
                ),
            ),
        ];
        for ((ws, slot), (t, sc, pk)) in cases {
            let got = resolve_slot_table(ws, slot)
                .unwrap_or_else(|_| panic!("({ws}, {slot}) should resolve"));
            assert_eq!(got.0, t, "table mismatch for ({ws}, {slot})");
            assert_eq!(got.1, sc, "status_col mismatch for ({ws}, {slot})");
            assert_eq!(got.2, pk, "pk mismatch for ({ws}, {slot})");
        }
    }

    #[test]
    fn resolve_slot_unknown_pair_errors() {
        let err = resolve_slot_table("cbu", "nonexistent").unwrap_err();
        assert!(
            err.to_string().contains("no SlotStateProvider mapping"),
            "got: {err}"
        );
    }

    #[test]
    fn resolve_slot_unknown_workspace_errors() {
        let err = resolve_slot_table("notaworkspace", "cbu").unwrap_err();
        assert!(err.to_string().contains("no SlotStateProvider mapping"));
    }
}
