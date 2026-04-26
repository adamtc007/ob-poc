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
//! See: docs/todo/catalogue-platform-refinement-v1_3.md §3.3
//! (runtime impact for V1.3-1 + V1.3-2)

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

/// Static mapping from (workspace, slot) → (table, state_column, pk_column).
///
/// Returned tuple is `(table_name, state_column, primary_key_column)`.
/// All names are unquoted-identifier-safe (alphanumeric + underscores).
///
/// Public so other modules in `cross_workspace::*` (e.g. the SQL
/// predicate resolver) can resolve the same mapping when constructing
/// predicate-driven queries.
pub fn resolve_slot_table(
    workspace: &str,
    slot: &str,
) -> Result<(&'static str, &'static str, &'static str)> {
    let mapping: &[((&str, &str), (&str, &str, &str))] = &[
        // CBU workspace
        (("cbu", "cbu"), ("cbus", "status", "cbu_id")),
        (("cbu", "cbu_evidence"), ("cbu_evidence", "verification_status", "evidence_id")),
        (("cbu", "service_consumption"), ("cbu_service_consumption", "status", "consumption_id")),
        (("cbu", "trading_activity"), ("cbu_trading_activity", "activity_state", "cbu_id")),
        (("cbu", "investor"), ("investors", "lifecycle_state", "investor_id")),
        (("cbu", "investor_kyc"), ("investors", "kyc_status", "investor_id")),
        (("cbu", "holding"), ("holdings", "holding_status", "holding_id")),
        // Deal workspace
        (("deal", "deal"), ("deals", "deal_status", "deal_id")),
        (("deal", "deal_product"), ("deal_products", "product_status", "deal_product_id")),
        (("deal", "deal_rate_card"), ("deal_rate_cards", "status", "rate_card_id")),
        (("deal", "deal_onboarding_request"), ("deal_onboarding_requests", "request_status", "request_id")),
        (("deal", "deal_document"), ("deal_documents", "document_status", "document_id")),
        (("deal", "deal_ubo_assessment"), ("deal_ubo_assessments", "assessment_status", "assessment_id")),
        (("deal", "billing_profile"), ("fee_billing_profiles", "status", "profile_id")),
        (("deal", "billing_period"), ("fee_billing_periods", "calc_status", "period_id")),
        (("deal", "deal_sla"), ("deal_slas", "sla_status", "sla_id")),
        // KYC workspace
        (("kyc", "kyc_case"), ("cases", "status", "case_id")),
        (("kyc", "entity_workstream"), ("entity_workstreams", "status", "workstream_id")),
        (("kyc", "screening"), ("screenings", "status", "screening_id")),
        // IM workspace
        (("instrument_matrix", "trading_profile"), ("cbu_trading_profiles", "status", "profile_id")),
        (("instrument_matrix", "trading_activity"), ("cbu_trading_activity", "activity_state", "cbu_id")),
        // SemOS workspace
        (("semos_maintenance", "changeset"), ("changesets", "status", "changeset_id")),
        (("semos_maintenance", "attribute_def"), ("attribute_defs", "lifecycle_status", "attribute_def_id")),
        (("semos_maintenance", "manco"), ("manco_regulatory_status", "regulatory_status", "manco_entity_id")),
        // Product-maintenance workspace (R2 — service catalogue lifecycle)
        (("product_maintenance", "service"), ("services", "lifecycle_status", "service_id")),
        (("product_maintenance", "service_version"), ("service_versions", "lifecycle_status", "id")),
        // Lifecycle Resources workspace (Tranche 4 R1)
        (("lifecycle_resources", "application_instance"), ("application_instances", "lifecycle_status", "id")),
        (("lifecycle_resources", "capability_binding"), ("capability_bindings", "binding_status", "id")),
    ];
    for ((ws, sl), value) in mapping {
        if *ws == workspace && *sl == slot {
            return Ok(*value);
        }
    }
    Err(anyhow!(
        "no SlotStateProvider mapping for ({workspace}, {slot}) — \
         add an entry in cross_workspace::slot_state::resolve_slot"
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_slot_known_pairs() {
        // Spot-check that the canonical cross-workspace lookups resolve.
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
                ("manco_regulatory_status", "regulatory_status", "manco_entity_id"),
            ),
        ];
        for ((ws, slot), expected) in cases {
            let got = resolve_slot_table(ws, slot).expect(&format!("({ws}, {slot}) should resolve"));
            assert_eq!(got, expected, "mismatch for ({ws}, {slot})");
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
