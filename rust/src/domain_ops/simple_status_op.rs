//! Generic `SimpleStatusOp` — a SemOsVerbOp that performs a single-column
//! status flip on a table by entity_id.
//!
//! The 117 plugin verbs surfaced by `test_plugin_verb_coverage` are
//! mostly state-machine transitions: take an entity id, write a new
//! status onto a single column. Rather than 117 individual structs (each
//! ~30-50 LOC of nearly-identical Rust), this module provides one
//! generic op + a config table.
//!
//! When to use SimpleStatusOp:
//!   - Verb takes one entity id arg (e.g. `cbu-id`, `deal-id`).
//!   - Verb sets ONE status column to a fixed target state.
//!   - No additional business logic (no fee calculations, no derived
//!     state writes, no fan-out — just the status flip).
//!
//! When NOT to use it:
//!   - Verb needs to inspect prior state to choose target state.
//!   - Verb writes multiple columns or multiple tables.
//!   - Verb has gating logic beyond what GatePipeline already provides.
//!   - Verb returns rich data (RecordSet, computed projection).
//!
//! For those cases, write a dedicated `SemOsVerbOp` impl.

use anyhow::Result;
use async_trait::async_trait;
use sem_os_postgres::ops::SemOsVerbOp;
use sqlx::types::chrono::Utc;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::json_extract_uuid;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

/// Optional companion column (timestamp) to set alongside the status.
/// Most state-machines have a `<state>_at` column tracking when the
/// transition occurred (e.g. `suspended_at`, `approved_at`).
#[derive(Debug, Clone)]
pub struct TimestampColumn {
    /// Column name to set to `now()`.
    pub column: &'static str,
}

/// Configuration for a single status-flip verb.
#[derive(Debug, Clone)]
pub struct SimpleStatusConfig {
    /// Verb FQN, e.g. `"cbu.suspend"`.
    pub fqn: &'static str,
    /// Table name (no schema prefix; the engine adds `"ob-poc"`).
    pub table: &'static str,
    /// Primary key column on `table`.
    pub pk_col: &'static str,
    /// Status column to update.
    pub state_col: &'static str,
    /// Target state value.
    pub target_state: &'static str,
    /// JSON arg name carrying the entity id (e.g. `"cbu-id"`).
    pub entity_arg: &'static str,
    /// Optional companion timestamp column to set to `now()`.
    pub timestamp: Option<TimestampColumn>,
}

/// Generic SemOsVerbOp that performs a status flip per [`SimpleStatusConfig`].
pub struct SimpleStatusOp {
    cfg: SimpleStatusConfig,
}

impl SimpleStatusOp {
    pub fn new(cfg: SimpleStatusConfig) -> Self {
        Self { cfg }
    }
}

#[async_trait]
impl SemOsVerbOp for SimpleStatusOp {
    fn fqn(&self) -> &str {
        self.cfg.fqn
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id: Uuid = json_extract_uuid(args, ctx, self.cfg.entity_arg)?;

        let sql = match &self.cfg.timestamp {
            Some(ts) => format!(
                r#"UPDATE "ob-poc".{tbl}
                   SET {state_col} = $1, {ts_col} = $2, updated_at = $2
                   WHERE {pk_col} = $3"#,
                tbl = self.cfg.table,
                state_col = self.cfg.state_col,
                ts_col = ts.column,
                pk_col = self.cfg.pk_col,
            ),
            None => format!(
                r#"UPDATE "ob-poc".{tbl}
                   SET {state_col} = $1, updated_at = $2
                   WHERE {pk_col} = $3"#,
                tbl = self.cfg.table,
                state_col = self.cfg.state_col,
                pk_col = self.cfg.pk_col,
            ),
        };

        let now = Utc::now();
        let affected = sqlx::query(&sql)
            .bind(self.cfg.target_state)
            .bind(now)
            .bind(entity_id)
            .execute(scope.executor())
            .await?
            .rows_affected();

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

/// Registration table — one row per status-flip verb. Add new verbs by
/// appending to `STATUS_FLIP_VERBS`; the registration helper at the
/// bottom registers all of them automatically.
pub const STATUS_FLIP_VERBS: &[SimpleStatusConfig] = &[
    // ── cbu.* ────────────────────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "cbu.suspend",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "operational_status",
        target_state: "suspended",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.reinstate",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "operational_status",
        target_state: "actively_trading",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.restrict",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "operational_status",
        target_state: "restricted",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.unrestrict",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "operational_status",
        target_state: "actively_trading",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.begin-winding-down",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "operational_status",
        target_state: "winding_down",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.complete-offboard",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "operational_status",
        target_state: "offboarded",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.flag-for-remediation",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "disposition_status",
        target_state: "under_remediation",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.clear-remediation",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "disposition_status",
        target_state: "active",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.soft-delete",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "disposition_status",
        target_state: "soft_deleted",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.restore",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "disposition_status",
        target_state: "active",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu.hard-delete",
        table: "cbus",
        pk_col: "cbu_id",
        state_col: "disposition_status",
        target_state: "hard_deleted",
        entity_arg: "cbu-id",
        timestamp: None,
    },
    // ── cbu-ca.* (CBU corporate actions) ─────────────────────────────────────
    SimpleStatusConfig {
        fqn: "cbu-ca.submit-for-review",
        table: "cbu_corporate_action_events",
        pk_col: "id",
        state_col: "status",
        target_state: "under_review",
        entity_arg: "event-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu-ca.approve",
        table: "cbu_corporate_action_events",
        pk_col: "id",
        state_col: "status",
        target_state: "approved",
        entity_arg: "event-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu-ca.reject",
        table: "cbu_corporate_action_events",
        pk_col: "id",
        state_col: "status",
        target_state: "rejected",
        entity_arg: "event-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu-ca.withdraw",
        table: "cbu_corporate_action_events",
        pk_col: "id",
        state_col: "status",
        target_state: "withdrawn",
        entity_arg: "event-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "cbu-ca.mark-implemented",
        table: "cbu_corporate_action_events",
        pk_col: "id",
        state_col: "status",
        target_state: "implemented",
        entity_arg: "event-id",
        timestamp: None,
    },
    // ── deal.* ───────────────────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "deal.submit-for-bac",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "BAC_APPROVAL",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.bac-approve",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "KYC_CLEARANCE",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.bac-reject",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "REJECTED",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.reject",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "REJECTED",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.mark-lost",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "LOST",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.mark-withdrawn",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "WITHDRAWN",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.suspend",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "SUSPENDED",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.reinstate",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "ACTIVE",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.begin-winding-down",
        table: "deals",
        pk_col: "deal_id",
        state_col: "deal_status",
        target_state: "WINDING_DOWN",
        entity_arg: "deal-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.submit-for-pricing-approval",
        table: "deal_rate_cards",
        pk_col: "rate_card_id",
        state_col: "status",
        target_state: "PENDING_INTERNAL_APPROVAL",
        entity_arg: "rate-card-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.pricing-approve",
        table: "deal_rate_cards",
        pk_col: "rate_card_id",
        state_col: "status",
        target_state: "APPROVED_INTERNALLY",
        entity_arg: "rate-card-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.pricing-reject",
        table: "deal_rate_cards",
        pk_col: "rate_card_id",
        state_col: "status",
        target_state: "DRAFT",
        entity_arg: "rate-card-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.start-sla-remediation",
        table: "deal_slas",
        pk_col: "sla_id",
        state_col: "sla_status",
        target_state: "IN_REMEDIATION",
        entity_arg: "sla-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.resolve-sla-breach",
        table: "deal_slas",
        pk_col: "sla_id",
        state_col: "sla_status",
        target_state: "RESOLVED",
        entity_arg: "sla-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "deal.waive-sla-breach",
        table: "deal_slas",
        pk_col: "sla_id",
        state_col: "sla_status",
        target_state: "WAIVED",
        entity_arg: "sla-id",
        timestamp: None,
    },
    // ── booking-principal-clearance.* (R3) ───────────────────────────────────
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.create",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "PENDING",
        entity_arg: "clearance-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.start-screening",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "SCREENING",
        entity_arg: "clearance-id",
        timestamp: Some(TimestampColumn {
            column: "screening_started_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.approve",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "APPROVED",
        entity_arg: "clearance-id",
        timestamp: Some(TimestampColumn {
            column: "approved_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.reject",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "REJECTED",
        entity_arg: "clearance-id",
        timestamp: Some(TimestampColumn {
            column: "rejected_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.reopen",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "PENDING",
        entity_arg: "clearance-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.activate",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "ACTIVE",
        entity_arg: "clearance-id",
        timestamp: Some(TimestampColumn {
            column: "activated_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.suspend",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "SUSPENDED",
        entity_arg: "clearance-id",
        timestamp: Some(TimestampColumn {
            column: "suspended_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.reinstate",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "ACTIVE",
        entity_arg: "clearance-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "booking-principal-clearance.revoke",
        table: "booking_principal_clearances",
        pk_col: "id",
        state_col: "clearance_status",
        target_state: "REVOKED",
        entity_arg: "clearance-id",
        timestamp: Some(TimestampColumn {
            column: "revoked_at",
        }),
    },
    // ── manco.* (8) ──────────────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "manco.approve",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "APPROVED",
        entity_arg: "manco-entity-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "manco.reject",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "TERMINATED",
        entity_arg: "manco-entity-id",
        timestamp: Some(TimestampColumn {
            column: "terminated_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "manco.flag-regulatory",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "UNDER_INVESTIGATION",
        entity_arg: "manco-entity-id",
        timestamp: Some(TimestampColumn {
            column: "flagged_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "manco.clear-regulatory",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "APPROVED",
        entity_arg: "manco-entity-id",
        timestamp: Some(TimestampColumn {
            column: "cleared_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "manco.suspend",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "SUSPENDED",
        entity_arg: "manco-entity-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "manco.partial-reinstate",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "UNDER_INVESTIGATION",
        entity_arg: "manco-entity-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "manco.begin-sunset",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "SUNSET",
        entity_arg: "manco-entity-id",
        timestamp: Some(TimestampColumn {
            column: "sunset_started_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "manco.terminate",
        table: "manco_regulatory_status",
        pk_col: "manco_entity_id",
        state_col: "regulatory_status",
        target_state: "TERMINATED",
        entity_arg: "manco-entity-id",
        timestamp: Some(TimestampColumn {
            column: "terminated_at",
        }),
    },
    // ── service-consumption.* (6) ────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "service-consumption.provision",
        table: "cbu_service_consumption",
        pk_col: "consumption_id",
        state_col: "status",
        target_state: "provisioned",
        entity_arg: "consumption-id",
        timestamp: Some(TimestampColumn {
            column: "provisioned_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "service-consumption.activate",
        table: "cbu_service_consumption",
        pk_col: "consumption_id",
        state_col: "status",
        target_state: "active",
        entity_arg: "consumption-id",
        timestamp: Some(TimestampColumn {
            column: "activated_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "service-consumption.suspend",
        table: "cbu_service_consumption",
        pk_col: "consumption_id",
        state_col: "status",
        target_state: "suspended",
        entity_arg: "consumption-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "service-consumption.reinstate",
        table: "cbu_service_consumption",
        pk_col: "consumption_id",
        state_col: "status",
        target_state: "active",
        entity_arg: "consumption-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "service-consumption.begin-winddown",
        table: "cbu_service_consumption",
        pk_col: "consumption_id",
        state_col: "status",
        target_state: "winding_down",
        entity_arg: "consumption-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "service-consumption.retire",
        table: "cbu_service_consumption",
        pk_col: "consumption_id",
        state_col: "status",
        target_state: "retired",
        entity_arg: "consumption-id",
        timestamp: Some(TimestampColumn {
            column: "retired_at",
        }),
    },
    // ── share-class.* (6) ────────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "share-class.launch",
        table: "share_classes",
        pk_col: "share_class_id",
        state_col: "lifecycle_status",
        target_state: "OPEN",
        entity_arg: "share-class-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "share-class.soft-close",
        table: "share_classes",
        pk_col: "share_class_id",
        state_col: "lifecycle_status",
        target_state: "SOFT_CLOSED",
        entity_arg: "share-class-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "share-class.reopen",
        table: "share_classes",
        pk_col: "share_class_id",
        state_col: "lifecycle_status",
        target_state: "OPEN",
        entity_arg: "share-class-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "share-class.hard-close",
        table: "share_classes",
        pk_col: "share_class_id",
        state_col: "lifecycle_status",
        target_state: "HARD_CLOSED",
        entity_arg: "share-class-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "share-class.lift-hard-close",
        table: "share_classes",
        pk_col: "share_class_id",
        state_col: "lifecycle_status",
        target_state: "SOFT_CLOSED",
        entity_arg: "share-class-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "share-class.begin-winddown",
        table: "share_classes",
        pk_col: "share_class_id",
        state_col: "lifecycle_status",
        target_state: "WINDING_DOWN",
        entity_arg: "share-class-id",
        timestamp: None,
    },
    // ── reconciliation.* (5) ─────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "reconciliation.create-config",
        table: "cbu_reconciliation_configs",
        pk_col: "config_id",
        state_col: "status",
        target_state: "draft",
        entity_arg: "config-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "reconciliation.activate",
        table: "cbu_reconciliation_configs",
        pk_col: "config_id",
        state_col: "status",
        target_state: "active",
        entity_arg: "config-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "reconciliation.suspend",
        table: "cbu_reconciliation_configs",
        pk_col: "config_id",
        state_col: "status",
        target_state: "suspended",
        entity_arg: "config-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "reconciliation.reactivate",
        table: "cbu_reconciliation_configs",
        pk_col: "config_id",
        state_col: "status",
        target_state: "active",
        entity_arg: "config-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "reconciliation.retire",
        table: "cbu_reconciliation_configs",
        pk_col: "config_id",
        state_col: "status",
        target_state: "retired",
        entity_arg: "config-id",
        timestamp: None,
    },
    // ── collateral-management.* (5) ──────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "collateral-management.configure",
        table: "cbu_collateral_management",
        pk_col: "collateral_id",
        state_col: "status",
        target_state: "configured",
        entity_arg: "collateral-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "collateral-management.activate",
        table: "cbu_collateral_management",
        pk_col: "collateral_id",
        state_col: "status",
        target_state: "active",
        entity_arg: "collateral-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "collateral-management.suspend",
        table: "cbu_collateral_management",
        pk_col: "collateral_id",
        state_col: "status",
        target_state: "suspended",
        entity_arg: "collateral-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "collateral-management.reactivate",
        table: "cbu_collateral_management",
        pk_col: "collateral_id",
        state_col: "status",
        target_state: "active",
        entity_arg: "collateral-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "collateral-management.terminate",
        table: "cbu_collateral_management",
        pk_col: "collateral_id",
        state_col: "status",
        target_state: "terminated",
        entity_arg: "collateral-id",
        timestamp: None,
    },
    // ── service-intent.* (3) ─────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "service-intent.suspend",
        table: "cbu_service_intent",
        pk_col: "intent_id",
        state_col: "status",
        target_state: "suspended",
        entity_arg: "intent-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "service-intent.resume",
        table: "cbu_service_intent",
        pk_col: "intent_id",
        state_col: "status",
        target_state: "active",
        entity_arg: "intent-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "service-intent.cancel",
        table: "cbu_service_intent",
        pk_col: "intent_id",
        state_col: "status",
        target_state: "cancelled",
        entity_arg: "intent-id",
        timestamp: None,
    },
    // ── service-resource.* (1 lifecycle alias) ───────────────────────────────
    SimpleStatusConfig {
        fqn: "service-resource.reactivate",
        table: "cbu_lifecycle_instances",
        pk_col: "instance_id",
        state_col: "status",
        target_state: "ACTIVE",
        entity_arg: "instance-id",
        timestamp: None,
    },
    // ── delivery.* (2) ───────────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "delivery.start",
        table: "service_delivery_map",
        pk_col: "delivery_id",
        state_col: "delivery_status",
        target_state: "IN_PROGRESS",
        entity_arg: "delivery",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "delivery.cancel",
        table: "service_delivery_map",
        pk_col: "delivery_id",
        state_col: "delivery_status",
        target_state: "CANCELLED",
        entity_arg: "delivery",
        timestamp: None,
    },
    // ── corporate-action-event.* (2) ─────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "corporate-action-event.elect",
        table: "corporate_action_events",
        pk_col: "event_id",
        state_col: "status",
        target_state: "elected",
        entity_arg: "event-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "corporate-action-event.attach",
        table: "corporate_action_events",
        pk_col: "event_id",
        state_col: "status",
        target_state: "election_pending",
        entity_arg: "event-id",
        timestamp: None,
    },
    // ── settlement-chain.* lifecycle (6) ─────────────────────────────────────
    SimpleStatusConfig {
        fqn: "settlement-chain.request-review",
        table: "cbu_settlement_chains",
        pk_col: "chain_id",
        state_col: "status",
        target_state: "reviewed",
        entity_arg: "chain",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "settlement-chain.enter-parallel-run",
        table: "cbu_settlement_chains",
        pk_col: "chain_id",
        state_col: "status",
        target_state: "parallel_run",
        entity_arg: "chain",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "settlement-chain.go-live",
        table: "cbu_settlement_chains",
        pk_col: "chain_id",
        state_col: "status",
        target_state: "live",
        entity_arg: "chain",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "settlement-chain.abort-parallel-run",
        table: "cbu_settlement_chains",
        pk_col: "chain_id",
        state_col: "status",
        target_state: "reviewed",
        entity_arg: "chain",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "settlement-chain.suspend",
        table: "cbu_settlement_chains",
        pk_col: "chain_id",
        state_col: "status",
        target_state: "suspended",
        entity_arg: "chain",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "settlement-chain.reactivate",
        table: "cbu_settlement_chains",
        pk_col: "chain_id",
        state_col: "status",
        target_state: "live",
        entity_arg: "chain",
        timestamp: None,
    },
    // ── trade-gateway.* (2 lifecycle) ────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "trade-gateway.reactivate-gateway",
        table: "cbu_gateway_connectivity",
        pk_col: "connectivity_id",
        state_col: "status",
        target_state: "ACTIVE",
        entity_arg: "connectivity-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "trade-gateway.retire-gateway",
        table: "cbu_gateway_connectivity",
        pk_col: "connectivity_id",
        state_col: "status",
        target_state: "DECOMMISSIONED",
        entity_arg: "connectivity-id",
        timestamp: None,
    },
    // ── trading-profile.* lifecycle subset (5) ───────────────────────────────
    SimpleStatusConfig {
        fqn: "trading-profile.enter-parallel-run",
        table: "cbu_trading_profiles",
        pk_col: "profile_id",
        state_col: "status",
        target_state: "PARALLEL_RUN",
        entity_arg: "profile",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "trading-profile.go-live",
        table: "cbu_trading_profiles",
        pk_col: "profile_id",
        state_col: "status",
        target_state: "ACTIVE",
        entity_arg: "profile",
        timestamp: Some(TimestampColumn {
            column: "activated_at",
        }),
    },
    SimpleStatusConfig {
        fqn: "trading-profile.abort-parallel-run",
        table: "cbu_trading_profiles",
        pk_col: "profile_id",
        state_col: "status",
        target_state: "APPROVED",
        entity_arg: "profile",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "trading-profile.suspend",
        table: "cbu_trading_profiles",
        pk_col: "profile_id",
        state_col: "status",
        target_state: "SUSPENDED",
        entity_arg: "profile",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "trading-profile.reactivate",
        table: "cbu_trading_profiles",
        pk_col: "profile_id",
        state_col: "status",
        target_state: "ACTIVE",
        entity_arg: "profile",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "trading-profile.supersede",
        table: "cbu_trading_profiles",
        pk_col: "profile_id",
        state_col: "status",
        target_state: "SUPERSEDED",
        entity_arg: "profile",
        timestamp: None,
    },
    // ── holding.* (4) ────────────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "holding.restrict",
        table: "holdings",
        pk_col: "holding_id",
        state_col: "holding_status",
        target_state: "RESTRICTED",
        entity_arg: "holding-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "holding.lift-restriction",
        table: "holdings",
        pk_col: "holding_id",
        state_col: "holding_status",
        target_state: "ACTIVE",
        entity_arg: "holding-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "holding.pledge",
        table: "holdings",
        pk_col: "holding_id",
        state_col: "holding_status",
        target_state: "PLEDGED",
        entity_arg: "holding-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "holding.release-pledge",
        table: "holdings",
        pk_col: "holding_id",
        state_col: "holding_status",
        target_state: "ACTIVE",
        entity_arg: "holding-id",
        timestamp: None,
    },
    // ── book.* (4) ───────────────────────────────────────────────────────────
    SimpleStatusConfig {
        fqn: "book.create",
        table: "client_books",
        pk_col: "book_id",
        state_col: "status",
        target_state: "proposed",
        entity_arg: "book-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "book.select-structure",
        table: "client_books",
        pk_col: "book_id",
        state_col: "status",
        target_state: "structure_chosen",
        entity_arg: "book-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "book.mark-ready",
        table: "client_books",
        pk_col: "book_id",
        state_col: "status",
        target_state: "ready_for_deal",
        entity_arg: "book-id",
        timestamp: None,
    },
    SimpleStatusConfig {
        fqn: "book.abandon",
        table: "client_books",
        pk_col: "book_id",
        state_col: "status",
        target_state: "abandoned",
        entity_arg: "book-id",
        timestamp: None,
    },
];

/// Register all status-flip verbs in the canonical registry.
///
/// Called from `domain_ops::extend_registry` so it's part of the
/// startup wiring.
pub fn register_simple_status_ops(registry: &mut sem_os_postgres::ops::SemOsVerbOpRegistry) {
    use std::sync::Arc;
    for cfg in STATUS_FLIP_VERBS {
        registry.register(Arc::new(SimpleStatusOp::new(cfg.clone())));
    }
}
