//! Investor-role profile verbs (6 plugin verbs) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/investor-role.yaml`.
//!
//! Issuer-scoped holder role metadata used by UBO eligibility,
//! look-through policy, and temporal queries. The DB side is a
//! single `upsert_role_profile` stored proc with 15 args — each
//! op projects a subset.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

/// Full 15-arg bind for `"ob-poc".upsert_role_profile`. Shared by
/// `set` + the four convenience ops (nominee / fof / master-pool /
/// end-investor) — only the literal defaults differ.
#[allow(clippy::too_many_arguments)]
async fn upsert_role_profile(
    scope: &mut dyn TransactionScope,
    issuer: Uuid,
    holder: Uuid,
    role_type: &str,
    lookthrough_policy: &str,
    holder_affiliation: &str,
    bo_data_available: bool,
    is_ubo_eligible: bool,
    share_class_id: Option<Uuid>,
    group_container_entity_id: Option<Uuid>,
    group_label: Option<&str>,
    effective_from: Option<chrono::NaiveDate>,
    source: &str,
    source_reference: Option<&str>,
    notes: Option<&str>,
) -> Result<Uuid> {
    let row: (Uuid,) = sqlx::query_as(
        r#"SELECT "ob-poc".upsert_role_profile($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL)"#,
    )
    .bind(issuer)
    .bind(holder)
    .bind(role_type)
    .bind(lookthrough_policy)
    .bind(holder_affiliation)
    .bind(bo_data_available)
    .bind(is_ubo_eligible)
    .bind(share_class_id)
    .bind(group_container_entity_id)
    .bind(group_label)
    .bind(effective_from)
    .bind(source)
    .bind(source_reference)
    .bind(notes)
    .fetch_one(scope.executor())
    .await?;
    Ok(row.0)
}

// ── investor-role.set ─────────────────────────────────────────────────────────

pub struct Set;

#[async_trait]
impl SemOsVerbOp for Set {
    fn fqn(&self) -> &str {
        "investor-role.set"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer = json_extract_uuid(args, ctx, "issuer")?;
        let holder = json_extract_uuid(args, ctx, "holder")?;
        let role_type = json_extract_string(args, "role-type")?;
        let lookthrough_policy = json_extract_string_opt(args, "lookthrough-policy")
            .unwrap_or_else(|| "NONE".to_string());
        let holder_affiliation = json_extract_string_opt(args, "holder-affiliation")
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let bo_data_available = json_extract_bool_opt(args, "bo-data-available").unwrap_or(false);
        let is_ubo_eligible = json_extract_bool_opt(args, "is-ubo-eligible").unwrap_or(true);
        let share_class_id = json_extract_uuid_opt(args, ctx, "share-class");
        let group_container_entity_id = json_extract_uuid_opt(args, ctx, "group-container");
        let group_label = json_extract_string_opt(args, "group-label");
        let effective_from: Option<chrono::NaiveDate> = json_extract_string_opt(args, "effective-from")
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
        let source =
            json_extract_string_opt(args, "source").unwrap_or_else(|| "MANUAL".to_string());
        let source_reference = json_extract_string_opt(args, "source-reference");
        let notes = json_extract_string_opt(args, "notes");

        let id = upsert_role_profile(
            scope,
            issuer,
            holder,
            &role_type,
            &lookthrough_policy,
            &holder_affiliation,
            bo_data_available,
            is_ubo_eligible,
            share_class_id,
            group_container_entity_id,
            group_label.as_deref(),
            effective_from,
            &source,
            source_reference.as_deref(),
            notes.as_deref(),
        )
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── investor-role.read-as-of ──────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct RoleProfileRow {
    id: Uuid,
    issuer_entity_id: Uuid,
    holder_entity_id: Uuid,
    share_class_id: Option<Uuid>,
    role_type: String,
    lookthrough_policy: String,
    holder_affiliation: String,
    beneficial_owner_data_available: bool,
    is_ubo_eligible: bool,
    group_container_entity_id: Option<Uuid>,
    group_label: Option<String>,
    effective_from: chrono::NaiveDate,
    effective_to: Option<chrono::NaiveDate>,
    source: Option<String>,
    source_reference: Option<String>,
    notes: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<RoleProfileRow> for Value {
    fn from(row: RoleProfileRow) -> Self {
        json!({
            "id": row.id,
            "issuer_entity_id": row.issuer_entity_id,
            "holder_entity_id": row.holder_entity_id,
            "share_class_id": row.share_class_id,
            "role_type": row.role_type,
            "lookthrough_policy": row.lookthrough_policy,
            "holder_affiliation": row.holder_affiliation,
            "beneficial_owner_data_available": row.beneficial_owner_data_available,
            "is_ubo_eligible": row.is_ubo_eligible,
            "group_container_entity_id": row.group_container_entity_id,
            "group_label": row.group_label,
            "effective_from": row.effective_from.to_string(),
            "effective_to": row.effective_to.map(|d| d.to_string()),
            "source": row.source,
            "source_reference": row.source_reference,
            "notes": row.notes,
            "created_at": row.created_at.to_rfc3339(),
            "updated_at": row.updated_at.to_rfc3339(),
        })
    }
}

pub struct ReadAsOf;

#[async_trait]
impl SemOsVerbOp for ReadAsOf {
    fn fqn(&self) -> &str {
        "investor-role.read-as-of"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer = json_extract_uuid(args, ctx, "issuer")?;
        let holder = json_extract_uuid(args, ctx, "holder")?;
        let as_of_str = json_extract_string(args, "as-of-date")?;
        let as_of_date = chrono::NaiveDate::parse_from_str(&as_of_str, "%Y-%m-%d")
            .map_err(|e| anyhow!("Invalid as-of-date: {}", e))?;

        let row: Option<RoleProfileRow> = sqlx::query_as(
            r#"
            SELECT
                id, issuer_entity_id, holder_entity_id, share_class_id,
                role_type, lookthrough_policy, holder_affiliation,
                beneficial_owner_data_available, is_ubo_eligible,
                group_container_entity_id, group_label,
                effective_from, effective_to,
                source, source_reference, notes,
                created_at, updated_at
            FROM "ob-poc".investor_role_profiles
            WHERE issuer_entity_id = $1
              AND holder_entity_id = $2
              AND effective_from <= $3
              AND (effective_to IS NULL OR effective_to > $3)
            ORDER BY effective_from DESC
            LIMIT 1
            "#,
        )
        .bind(issuer)
        .bind(holder)
        .bind(as_of_date)
        .fetch_optional(scope.executor())
        .await?;

        Ok(match row {
            Some(p) => VerbExecutionOutcome::Record(p.into()),
            None => VerbExecutionOutcome::Void,
        })
    }
}

// ── investor-role convenience variants ────────────────────────────────────────

pub struct MarkAsNominee;

#[async_trait]
impl SemOsVerbOp for MarkAsNominee {
    fn fqn(&self) -> &str {
        "investor-role.mark-as-nominee"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer = json_extract_uuid(args, ctx, "issuer")?;
        let holder = json_extract_uuid(args, ctx, "holder")?;
        let notes = json_extract_string_opt(args, "notes");
        let id = upsert_role_profile(
            scope, issuer, holder, "NOMINEE", "NONE", "UNKNOWN", false, false, None, None, None,
            None, "MANUAL", None, notes.as_deref(),
        )
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

pub struct MarkAsFof;

#[async_trait]
impl SemOsVerbOp for MarkAsFof {
    fn fqn(&self) -> &str {
        "investor-role.mark-as-fof"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer = json_extract_uuid(args, ctx, "issuer")?;
        let holder = json_extract_uuid(args, ctx, "holder")?;
        let bo_data_available = json_extract_bool_opt(args, "bo-data-available").unwrap_or(false);
        let notes = json_extract_string_opt(args, "notes");
        let id = upsert_role_profile(
            scope,
            issuer,
            holder,
            "INTERMEDIARY_FOF",
            "ON_DEMAND",
            "UNKNOWN",
            bo_data_available,
            false,
            None,
            None,
            None,
            None,
            "MANUAL",
            None,
            notes.as_deref(),
        )
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

pub struct MarkAsMasterPool;

#[async_trait]
impl SemOsVerbOp for MarkAsMasterPool {
    fn fqn(&self) -> &str {
        "investor-role.mark-as-master-pool"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer = json_extract_uuid(args, ctx, "issuer")?;
        let holder = json_extract_uuid(args, ctx, "holder")?;
        let holder_affiliation = json_extract_string_opt(args, "holder-affiliation")
            .unwrap_or_else(|| "INTRA_GROUP".to_string());
        let notes = json_extract_string_opt(args, "notes");
        let id = upsert_role_profile(
            scope,
            issuer,
            holder,
            "MASTER_POOL",
            "AUTO_IF_DATA",
            &holder_affiliation,
            false,
            false,
            None,
            None,
            None,
            None,
            "MANUAL",
            None,
            notes.as_deref(),
        )
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

pub struct MarkAsEndInvestor;

#[async_trait]
impl SemOsVerbOp for MarkAsEndInvestor {
    fn fqn(&self) -> &str {
        "investor-role.mark-as-end-investor"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer = json_extract_uuid(args, ctx, "issuer")?;
        let holder = json_extract_uuid(args, ctx, "holder")?;
        let holder_affiliation = json_extract_string_opt(args, "holder-affiliation")
            .unwrap_or_else(|| "EXTERNAL".to_string());
        let notes = json_extract_string_opt(args, "notes");
        let id = upsert_role_profile(
            scope,
            issuer,
            holder,
            "END_INVESTOR",
            "NONE",
            &holder_affiliation,
            false,
            true,
            None,
            None,
            None,
            None,
            "MANUAL",
            None,
            notes.as_deref(),
        )
        .await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}
