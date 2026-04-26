//! UBO registry lifecycle verbs (5 plugin verbs) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/kyc/ubo-registry.yaml`.
//!
//! The UBO registry drives beneficial-ownership determinations
//! through a state machine:
//!
//! ```text
//! CANDIDATE → IDENTIFIED → PROVABLE → PROVED → REVIEWED → APPROVED
//!     ↓            ↓           ↓         ↓         ↓
//!   WAIVED      WAIVED      WAIVED    WAIVED    WAIVED
//!   REJECTED    REJECTED    REJECTED  REJECTED  REJECTED
//!   EXPIRED     EXPIRED     EXPIRED   EXPIRED   EXPIRED
//! ```
//!
//! All writes run on `scope.executor()` (Sequencer-owned txn).
//! Result types kept as private serializable structs — they are
//! not consumed outside this file.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

const TERMINAL_STATUSES: &[&str] = &["APPROVED", "WAIVED", "REJECTED", "EXPIRED"];

fn is_terminal(status: &str) -> bool {
    TERMINAL_STATUSES.contains(&status)
}

async fn fetch_current_status(
    scope: &mut dyn TransactionScope,
    registry_id: Uuid,
) -> Result<String> {
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT status FROM "ob-poc".kyc_ubo_registry WHERE registry_id = $1"#)
            .bind(registry_id)
            .fetch_optional(scope.executor())
            .await?;
    row.map(|(s,)| s)
        .ok_or_else(|| anyhow!("UBO registry entry not found: {}", registry_id))
}

fn validate_advance_transition(current: &str, new_status: &str) -> Result<&'static str> {
    match (current, new_status) {
        ("CANDIDATE", "IDENTIFIED") => Ok("identified_at"),
        ("IDENTIFIED", "PROVABLE") => Ok("provable_at"),
        ("PROVABLE", "PROVED") => Ok("proved_at"),
        ("PROVED", "REVIEWED") => Ok("reviewed_at"),
        ("REVIEWED", "APPROVED") => Ok("approved_at"),
        _ => Err(anyhow!(
            "Invalid state transition: {} -> {}. Valid forward transitions are: \
             CANDIDATE->IDENTIFIED, IDENTIFIED->PROVABLE, PROVABLE->PROVED, \
             PROVED->REVIEWED, REVIEWED->APPROVED",
            current,
            new_status
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PromoteResult {
    registry_id: Uuid,
    previous_status: String,
    new_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AdvanceResult {
    registry_id: Uuid,
    previous_status: String,
    new_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WaiveResult {
    registry_id: Uuid,
    previous_status: String,
    new_status: String,
    waived_by: String,
    waiver_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RejectResult {
    registry_id: Uuid,
    previous_status: String,
    new_status: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExpireResult {
    registry_id: Uuid,
    previous_status: String,
    new_status: String,
}

// ── ubo.registry.promote (CANDIDATE → IDENTIFIED) ─────────────────────────────

pub struct Promote;

#[async_trait]
impl SemOsVerbOp for Promote {
    fn fqn(&self) -> &str {
        "ubo.registry.promote"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let registry_id = json_extract_uuid(args, ctx, "registry-id")?;
        let notes = json_extract_string_opt(args, "notes");

        let current_status = fetch_current_status(scope, registry_id).await?;
        if current_status != "CANDIDATE" {
            return Err(anyhow!(
                "Cannot promote: entry is in status '{}', expected 'CANDIDATE'",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE "ob-poc".kyc_ubo_registry
            SET status = 'IDENTIFIED',
                identified_at = NOW(),
                notes = COALESCE($2, notes),
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
        )
        .bind(registry_id)
        .bind(&notes)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            PromoteResult {
                registry_id,
                previous_status: current_status,
                new_status: "IDENTIFIED".to_string(),
            },
        )?))
    }
}

// ── ubo.registry.advance (general forward transitions) ────────────────────────

pub struct Advance;

#[async_trait]
impl SemOsVerbOp for Advance {
    fn fqn(&self) -> &str {
        "ubo.registry.advance"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let registry_id = json_extract_uuid(args, ctx, "registry-id")?;
        let new_status = json_extract_string(args, "new-status")?;
        let notes = json_extract_string_opt(args, "notes");

        let current_status = fetch_current_status(scope, registry_id).await?;
        let timestamp_col = validate_advance_transition(&current_status, &new_status)?;

        let sql = format!(
            r#"
            UPDATE "ob-poc".kyc_ubo_registry
            SET status = $2,
                {} = NOW(),
                notes = COALESCE($3, notes),
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
            timestamp_col
        );

        sqlx::query(&sql)
            .bind(registry_id)
            .bind(&new_status)
            .bind(&notes)
            .execute(scope.executor())
            .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            AdvanceResult {
                registry_id,
                previous_status: current_status,
                new_status,
            },
        )?))
    }
}

// ── ubo.registry.waive (any non-terminal → WAIVED) ────────────────────────────

pub struct Waive;

#[async_trait]
impl SemOsVerbOp for Waive {
    fn fqn(&self) -> &str {
        "ubo.registry.waive"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let registry_id = json_extract_uuid(args, ctx, "registry-id")?;
        let reason = json_extract_string(args, "reason")?;
        let authority = json_extract_string(args, "authority")?;

        let current_status = fetch_current_status(scope, registry_id).await?;
        if is_terminal(&current_status) {
            return Err(anyhow!(
                "Cannot waive: entry is in terminal status '{}'. \
                 Only non-terminal statuses can be waived.",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE "ob-poc".kyc_ubo_registry
            SET status = 'WAIVED',
                waived_at = NOW(),
                waived_by = $2,
                waiver_reason = $3,
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
        )
        .bind(registry_id)
        .bind(&authority)
        .bind(&reason)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            WaiveResult {
                registry_id,
                previous_status: current_status,
                new_status: "WAIVED".to_string(),
                waived_by: authority,
                waiver_reason: reason,
            },
        )?))
    }
}

// ── ubo.registry.reject (any non-terminal → REJECTED) ─────────────────────────

pub struct Reject;

#[async_trait]
impl SemOsVerbOp for Reject {
    fn fqn(&self) -> &str {
        "ubo.registry.reject"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let registry_id = json_extract_uuid(args, ctx, "registry-id")?;
        let reason = json_extract_string(args, "reason")?;

        let current_status = fetch_current_status(scope, registry_id).await?;
        if is_terminal(&current_status) {
            return Err(anyhow!(
                "Cannot reject: entry is in terminal status '{}'. \
                 Only non-terminal statuses can be rejected.",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE "ob-poc".kyc_ubo_registry
            SET status = 'REJECTED',
                rejected_at = NOW(),
                notes = $2,
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
        )
        .bind(registry_id)
        .bind(&reason)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            RejectResult {
                registry_id,
                previous_status: current_status,
                new_status: "REJECTED".to_string(),
                reason,
            },
        )?))
    }
}

// ── ubo.registry.expire (any non-terminal → EXPIRED) ──────────────────────────

pub struct Expire;

#[async_trait]
impl SemOsVerbOp for Expire {
    fn fqn(&self) -> &str {
        "ubo.registry.expire"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let registry_id = json_extract_uuid(args, ctx, "registry-id")?;
        let reason = json_extract_string_opt(args, "reason");

        let current_status = fetch_current_status(scope, registry_id).await?;
        if is_terminal(&current_status) {
            return Err(anyhow!(
                "Cannot expire: entry is in terminal status '{}'. \
                 Only non-terminal statuses can be expired.",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE "ob-poc".kyc_ubo_registry
            SET status = 'EXPIRED',
                expired_at = NOW(),
                notes = COALESCE($2, notes),
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
        )
        .bind(registry_id)
        .bind(&reason)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            ExpireResult {
                registry_id,
                previous_status: current_status,
                new_status: "EXPIRED".to_string(),
            },
        )?))
    }
}
