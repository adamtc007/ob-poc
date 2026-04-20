//! Investor role profile plugin operations.
//!
//! Manages issuer-scoped holder role metadata for:
//! - UBO eligibility determination
//! - Look-through policy control
//! - Temporal versioning for point-in-time queries

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

// ============================================================================
// InvestorRoleSetOp - Set or update holder role profile
// ============================================================================

/// Set or update a holder role profile with temporal versioning.
/// Closes existing active profile and creates new version.
#[register_custom_op]
pub struct InvestorRoleSetOp;

#[async_trait]
impl CustomOperation for InvestorRoleSetOp {
    fn domain(&self) -> &'static str {
        "investor-role"
    }

    fn verb(&self) -> &'static str {
        "set"
    }

    fn rationale(&self) -> &'static str {
        "Temporal versioning requires custom upsert logic to close existing profile and create new version"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder")?;
        let role_type = json_extract_string(args, "role-type")?;

        let lookthrough_policy =
            json_extract_string_opt(args, "lookthrough-policy").unwrap_or_else(|| "NONE".to_string());
        let holder_affiliation = json_extract_string_opt(args, "holder-affiliation")
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let bo_data_available = json_extract_bool_opt(args, "bo-data-available").unwrap_or(false);
        let is_ubo_eligible = json_extract_bool_opt(args, "is-ubo-eligible").unwrap_or(true);
        let share_class_id = json_extract_uuid_opt(args, ctx, "share-class");
        let group_container_entity_id = json_extract_uuid_opt(args, ctx, "group-container");
        let group_label = json_extract_string_opt(args, "group-label");
        let effective_from: Option<chrono::NaiveDate> = json_extract_string_opt(args, "effective-from")
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
        let source = json_extract_string_opt(args, "source").unwrap_or_else(|| "MANUAL".to_string());
        let source_reference = json_extract_string_opt(args, "source-reference");
        let notes = json_extract_string_opt(args, "notes");

        let result: (uuid::Uuid,) = sqlx::query_as(
            r#"
            SELECT "ob-poc".upsert_role_profile(
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL
            )
            "#,
        )
        .bind(issuer_entity_id)
        .bind(holder_entity_id)
        .bind(&role_type)
        .bind(&lookthrough_policy)
        .bind(&holder_affiliation)
        .bind(bo_data_available)
        .bind(is_ubo_eligible)
        .bind(share_class_id)
        .bind(group_container_entity_id)
        .bind(group_label.as_deref())
        .bind(effective_from)
        .bind(&source)
        .bind(source_reference.as_deref())
        .bind(notes.as_deref())
        .fetch_one(pool)
        .await?;

        Ok(VerbExecutionOutcome::Uuid(result.0))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// InvestorRoleReadAsOfOp - Point-in-time query
// ============================================================================

/// Read role profile as of a specific date.
#[register_custom_op]
pub struct InvestorRoleReadAsOfOp;

#[async_trait]
impl CustomOperation for InvestorRoleReadAsOfOp {
    fn domain(&self) -> &'static str {
        "investor-role"
    }

    fn verb(&self) -> &'static str {
        "read-as-of"
    }

    fn rationale(&self) -> &'static str {
        "Point-in-time query requires custom temporal logic with effective_from/effective_to comparison"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder")?;
        let as_of_str = json_extract_string(args, "as-of-date")?;
        let as_of_date = chrono::NaiveDate::parse_from_str(&as_of_str, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("Invalid as-of-date: {}", e))?;

        let row = sqlx::query_as::<_, RoleProfileRow>(
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
        .bind(issuer_entity_id)
        .bind(holder_entity_id)
        .bind(as_of_date)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(profile) => Ok(VerbExecutionOutcome::Record(profile.into())),
            None => Ok(VerbExecutionOutcome::Void),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// Row type for query results
// ============================================================================

#[derive(sqlx::FromRow)]
struct RoleProfileRow {
    id: uuid::Uuid,
    issuer_entity_id: uuid::Uuid,
    holder_entity_id: uuid::Uuid,
    share_class_id: Option<uuid::Uuid>,
    role_type: String,
    lookthrough_policy: String,
    holder_affiliation: String,
    beneficial_owner_data_available: bool,
    is_ubo_eligible: bool,
    group_container_entity_id: Option<uuid::Uuid>,
    group_label: Option<String>,
    effective_from: chrono::NaiveDate,
    effective_to: Option<chrono::NaiveDate>,
    source: Option<String>,
    source_reference: Option<String>,
    notes: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<RoleProfileRow> for serde_json::Value {
    fn from(row: RoleProfileRow) -> Self {
        serde_json::json!({
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
            "updated_at": row.updated_at.to_rfc3339()
        })
    }
}

// ============================================================================
// Convenience Verbs (delegate to set with hardcoded defaults)
// ============================================================================

/// Mark a holder as a nominee (custodian, nominee company, etc.).
/// Sets role_type=NOMINEE, is_ubo_eligible=false, lookthrough_policy=NONE
#[register_custom_op]
pub struct InvestorRoleMarkAsNomineeOp;

#[async_trait]
impl CustomOperation for InvestorRoleMarkAsNomineeOp {
    fn domain(&self) -> &'static str {
        "investor-role"
    }

    fn verb(&self) -> &'static str {
        "mark-as-nominee"
    }

    fn rationale(&self) -> &'static str {
        "Convenience verb that sets nominee defaults (role_type=NOMINEE, is_ubo_eligible=false, lookthrough_policy=NONE)"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder")?;
        let notes = json_extract_string_opt(args, "notes");

        let result: (uuid::Uuid,) = sqlx::query_as(
            r#"SELECT "ob-poc".upsert_role_profile($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL)"#,
        )
        .bind(issuer_entity_id).bind(holder_entity_id)
        .bind("NOMINEE").bind("NONE").bind("UNKNOWN")
        .bind(false).bind(false)
        .bind(None::<uuid::Uuid>).bind(None::<uuid::Uuid>).bind(None::<String>)
        .bind(None::<chrono::NaiveDate>).bind("MANUAL").bind(None::<String>)
        .bind(notes.as_deref())
        .fetch_one(pool).await?;

        Ok(VerbExecutionOutcome::Uuid(result.0))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Mark a holder as a fund-of-funds intermediary.
/// Sets role_type=INTERMEDIARY_FOF, is_ubo_eligible=false, lookthrough_policy=ON_DEMAND
#[register_custom_op]
pub struct InvestorRoleMarkAsFofOp;

#[async_trait]
impl CustomOperation for InvestorRoleMarkAsFofOp {
    fn domain(&self) -> &'static str {
        "investor-role"
    }

    fn verb(&self) -> &'static str {
        "mark-as-fof"
    }

    fn rationale(&self) -> &'static str {
        "Convenience verb that sets FoF defaults (role_type=INTERMEDIARY_FOF, is_ubo_eligible=false, lookthrough_policy=ON_DEMAND)"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder")?;
        let bo_data_available = json_extract_bool_opt(args, "bo-data-available").unwrap_or(false);
        let notes = json_extract_string_opt(args, "notes");

        let result: (uuid::Uuid,) = sqlx::query_as(
            r#"SELECT "ob-poc".upsert_role_profile($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL)"#,
        )
        .bind(issuer_entity_id).bind(holder_entity_id)
        .bind("INTERMEDIARY_FOF").bind("ON_DEMAND").bind("UNKNOWN")
        .bind(bo_data_available).bind(false)
        .bind(None::<uuid::Uuid>).bind(None::<uuid::Uuid>).bind(None::<String>)
        .bind(None::<chrono::NaiveDate>).bind("MANUAL").bind(None::<String>)
        .bind(notes.as_deref())
        .fetch_one(pool).await?;

        Ok(VerbExecutionOutcome::Uuid(result.0))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Mark a holder as a master pooling vehicle.
/// Sets role_type=MASTER_POOL, is_ubo_eligible=false, lookthrough_policy=AUTO_IF_DATA
#[register_custom_op]
pub struct InvestorRoleMarkAsMasterPoolOp;

#[async_trait]
impl CustomOperation for InvestorRoleMarkAsMasterPoolOp {
    fn domain(&self) -> &'static str {
        "investor-role"
    }

    fn verb(&self) -> &'static str {
        "mark-as-master-pool"
    }

    fn rationale(&self) -> &'static str {
        "Convenience verb that sets master pool defaults (role_type=MASTER_POOL, is_ubo_eligible=false, lookthrough_policy=AUTO_IF_DATA)"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder")?;
        let holder_affiliation = json_extract_string_opt(args, "holder-affiliation")
            .unwrap_or_else(|| "INTRA_GROUP".to_string());
        let notes = json_extract_string_opt(args, "notes");

        let result: (uuid::Uuid,) = sqlx::query_as(
            r#"SELECT "ob-poc".upsert_role_profile($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL)"#,
        )
        .bind(issuer_entity_id).bind(holder_entity_id)
        .bind("MASTER_POOL").bind("AUTO_IF_DATA").bind(&holder_affiliation)
        .bind(false).bind(false)
        .bind(None::<uuid::Uuid>).bind(None::<uuid::Uuid>).bind(None::<String>)
        .bind(None::<chrono::NaiveDate>).bind("MANUAL").bind(None::<String>)
        .bind(notes.as_deref())
        .fetch_one(pool).await?;

        Ok(VerbExecutionOutcome::Uuid(result.0))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Mark a holder as an end investor (terminal beneficial owner).
/// Sets role_type=END_INVESTOR, is_ubo_eligible=true, lookthrough_policy=NONE
#[register_custom_op]
pub struct InvestorRoleMarkAsEndInvestorOp;

#[async_trait]
impl CustomOperation for InvestorRoleMarkAsEndInvestorOp {
    fn domain(&self) -> &'static str {
        "investor-role"
    }

    fn verb(&self) -> &'static str {
        "mark-as-end-investor"
    }

    fn rationale(&self) -> &'static str {
        "Convenience verb that sets end investor defaults (role_type=END_INVESTOR, is_ubo_eligible=true, lookthrough_policy=NONE)"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer")?;
        let holder_entity_id = json_extract_uuid(args, ctx, "holder")?;
        let holder_affiliation = json_extract_string_opt(args, "holder-affiliation")
            .unwrap_or_else(|| "EXTERNAL".to_string());
        let notes = json_extract_string_opt(args, "notes");

        let result: (uuid::Uuid,) = sqlx::query_as(
            r#"SELECT "ob-poc".upsert_role_profile($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL)"#,
        )
        .bind(issuer_entity_id).bind(holder_entity_id)
        .bind("END_INVESTOR").bind("NONE").bind(&holder_affiliation)
        .bind(false).bind(true)
        .bind(None::<uuid::Uuid>).bind(None::<uuid::Uuid>).bind(None::<String>)
        .bind(None::<chrono::NaiveDate>).bind("MANUAL").bind(None::<String>)
        .bind(notes.as_deref())
        .fetch_one(pool).await?;

        Ok(VerbExecutionOutcome::Uuid(result.0))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
