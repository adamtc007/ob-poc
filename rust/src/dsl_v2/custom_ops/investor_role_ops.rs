//! Investor role profile plugin operations.
//!
//! Manages issuer-scoped holder role metadata for:
//! - UBO eligibility determination
//! - Look-through policy control
//! - Temporal versioning for point-in-time queries

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract a required UUID argument from verb call
#[cfg(feature = "database")]
fn get_required_uuid(
    verb_call: &VerbCall,
    key: &str,
    ctx: &ExecutionContext,
) -> Result<uuid::Uuid> {
    use uuid::Uuid;

    let arg = verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))?;

    // Try as symbol reference first
    if let Some(ref_name) = arg.value.as_symbol() {
        let resolved = ctx
            .resolve(ref_name)
            .ok_or_else(|| anyhow::anyhow!("Unresolved reference @{}", ref_name))?;
        return Ok(resolved);
    }

    // Try as UUID directly
    if let Some(uuid_val) = arg.value.as_uuid() {
        return Ok(uuid_val);
    }

    // Try as string (may be UUID string)
    if let Some(str_val) = arg.value.as_string() {
        return Uuid::parse_str(str_val)
            .map_err(|e| anyhow::anyhow!("Invalid UUID for :{}: {}", key, e));
    }

    Err(anyhow::anyhow!(":{} must be a UUID or @reference", key))
}

/// Extract an optional UUID argument from verb call
#[cfg(feature = "database")]
fn get_optional_uuid(
    verb_call: &VerbCall,
    key: &str,
    ctx: &ExecutionContext,
) -> Option<uuid::Uuid> {
    get_required_uuid(verb_call, key, ctx).ok()
}

/// Extract an optional string argument from verb call
#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Extract a required string argument from verb call
#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    get_optional_string(verb_call, key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

/// Extract an optional boolean argument from verb call
#[cfg(feature = "database")]
fn get_optional_bool(verb_call: &VerbCall, key: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_boolean())
}

/// Extract an optional date argument from verb call
#[cfg(feature = "database")]
fn get_optional_date(verb_call: &VerbCall, key: &str) -> Option<chrono::NaiveDate> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}

// ============================================================================
// InvestorRoleSetOp - Set or update holder role profile
// ============================================================================

/// Set or update a holder role profile with temporal versioning.
/// Closes existing active profile and creates new version.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Required arguments
        let issuer_entity_id = get_required_uuid(verb_call, "issuer", ctx)?;
        let holder_entity_id = get_required_uuid(verb_call, "holder", ctx)?;
        let role_type = get_required_string(verb_call, "role-type")?;

        // Optional arguments with defaults
        let lookthrough_policy = get_optional_string(verb_call, "lookthrough-policy")
            .unwrap_or_else(|| "NONE".to_string());
        let holder_affiliation = get_optional_string(verb_call, "holder-affiliation")
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let bo_data_available = get_optional_bool(verb_call, "bo-data-available").unwrap_or(false);
        let is_ubo_eligible = get_optional_bool(verb_call, "is-ubo-eligible").unwrap_or(true);
        let share_class_id = get_optional_uuid(verb_call, "share-class", ctx);
        let group_container_entity_id = get_optional_uuid(verb_call, "group-container", ctx);
        let group_label = get_optional_string(verb_call, "group-label");
        let effective_from = get_optional_date(verb_call, "effective-from");
        let source =
            get_optional_string(verb_call, "source").unwrap_or_else(|| "MANUAL".to_string());
        let source_reference = get_optional_string(verb_call, "source-reference");
        let notes = get_optional_string(verb_call, "notes");

        // Call the upsert function which handles temporal versioning
        let result: (uuid::Uuid,) = sqlx::query_as(
            r#"
            SELECT kyc.upsert_role_profile(
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

        Ok(ExecutionResult::Uuid(result.0))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "investor-role.set requires database feature"
        ))
    }
}

// ============================================================================
// InvestorRoleReadAsOfOp - Point-in-time query
// ============================================================================

/// Read role profile as of a specific date.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = get_required_uuid(verb_call, "issuer", ctx)?;
        let holder_entity_id = get_required_uuid(verb_call, "holder", ctx)?;
        let as_of_date = get_optional_date(verb_call, "as-of-date")
            .ok_or_else(|| anyhow::anyhow!("Missing required argument :as-of-date"))?;

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
            FROM kyc.investor_role_profiles
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
            Some(profile) => Ok(ExecutionResult::Record(profile.into())),
            None => Ok(ExecutionResult::Void),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "investor-role.read-as-of requires database feature"
        ))
    }
}

// ============================================================================
// Row type for query results
// ============================================================================

#[cfg(feature = "database")]
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

#[cfg(feature = "database")]
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
// Registration
// ============================================================================

/// Register investor role operations with the registry
pub fn register_investor_role_ops(
    registry: &mut crate::dsl_v2::custom_ops::CustomOperationRegistry,
) {
    registry.register(Arc::new(InvestorRoleSetOp));
    registry.register(Arc::new(InvestorRoleReadAsOfOp));
}
