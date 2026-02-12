//! UBO Registry Operations
//!
//! Plugin operations for managing UBO registry entry lifecycle transitions.
//! The UBO registry tracks beneficial ownership determinations through a
//! state machine:
//!
//! ```text
//! CANDIDATE → IDENTIFIED → PROVABLE → PROVED → REVIEWED → APPROVED
//!     ↓            ↓           ↓         ↓         ↓
//!   WAIVED      WAIVED      WAIVED    WAIVED    WAIVED
//!   REJECTED    REJECTED    REJECTED  REJECTED  REJECTED
//!   EXPIRED     EXPIRED     EXPIRED   EXPIRED   EXPIRED
//! ```
//!
//! ## Rationale
//! These operations require custom code because:
//! - State machine validation prevents illegal transitions
//! - Each transition sets a specific timestamp column
//! - Waive requires authority and reason tracking
//! - Current status must be validated before any transition

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helpers::{extract_string, extract_string_opt, extract_uuid};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// Result Types
// =============================================================================

/// Result of promoting a UBO registry entry from CANDIDATE to IDENTIFIED.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboRegistryPromoteResult {
    pub registry_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
}

/// Result of advancing a UBO registry entry through the state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboRegistryAdvanceResult {
    pub registry_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
}

/// Result of waiving a UBO registry entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboRegistryWaiveResult {
    pub registry_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
    pub waived_by: String,
    pub waiver_reason: String,
}

/// Result of rejecting a UBO registry entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboRegistryRejectResult {
    pub registry_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
    pub reason: String,
}

/// Result of expiring a UBO registry entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboRegistryExpireResult {
    pub registry_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
}

// =============================================================================
// Constants
// =============================================================================

/// Terminal statuses that cannot transition further.
const TERMINAL_STATUSES: &[&str] = &["APPROVED", "WAIVED", "REJECTED", "EXPIRED"];

// =============================================================================
// Helpers
// =============================================================================

/// Fetch the current status of a UBO registry entry. Returns an error if not found.
#[cfg(feature = "database")]
async fn fetch_current_status(pool: &PgPool, registry_id: Uuid) -> Result<String> {
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT status FROM kyc.ubo_registry WHERE registry_id = $1"#)
            .bind(registry_id)
            .fetch_optional(pool)
            .await?;

    row.map(|(s,)| s)
        .ok_or_else(|| anyhow!("UBO registry entry not found: {}", registry_id))
}

/// Check whether a status is terminal (no further forward transitions allowed).
fn is_terminal(status: &str) -> bool {
    TERMINAL_STATUSES.contains(&status)
}

/// Validate that the advance transition is legal per the state machine.
/// Returns the timestamp column name to set for the new status.
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

// =============================================================================
// UboRegistryPromoteOp — CANDIDATE → IDENTIFIED
// =============================================================================

/// Promotes a UBO registry entry from CANDIDATE to IDENTIFIED.
///
/// This is a convenience operation for the most common initial transition.
/// Sets `identified_at` to the current timestamp.
#[register_custom_op]
pub struct UboRegistryPromoteOp;

#[async_trait]
impl CustomOperation for UboRegistryPromoteOp {
    fn domain(&self) -> &'static str {
        "ubo.registry"
    }

    fn verb(&self) -> &'static str {
        "promote"
    }

    fn rationale(&self) -> &'static str {
        "Validates current status is CANDIDATE before transitioning to IDENTIFIED with timestamp"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let registry_id = extract_uuid(verb_call, ctx, "registry-id")?;
        let notes = extract_string_opt(verb_call, "notes");

        let current_status = fetch_current_status(pool, registry_id).await?;

        if current_status != "CANDIDATE" {
            return Err(anyhow!(
                "Cannot promote: entry is in status '{}', expected 'CANDIDATE'",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE kyc.ubo_registry
            SET status = 'IDENTIFIED',
                identified_at = NOW(),
                notes = COALESCE($2, notes),
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
        )
        .bind(registry_id)
        .bind(&notes)
        .execute(pool)
        .await?;

        let result = UboRegistryPromoteResult {
            registry_id,
            previous_status: current_status,
            new_status: "IDENTIFIED".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// =============================================================================
// UboRegistryAdvanceOp — General forward state transitions
// =============================================================================

/// Advances a UBO registry entry through the forward state machine.
///
/// Valid transitions:
/// - CANDIDATE -> IDENTIFIED
/// - IDENTIFIED -> PROVABLE
/// - PROVABLE -> PROVED
/// - PROVED -> REVIEWED
/// - REVIEWED -> APPROVED
///
/// Sets the appropriate timestamp column for the target status.
#[register_custom_op]
pub struct UboRegistryAdvanceOp;

#[async_trait]
impl CustomOperation for UboRegistryAdvanceOp {
    fn domain(&self) -> &'static str {
        "ubo.registry"
    }

    fn verb(&self) -> &'static str {
        "advance"
    }

    fn rationale(&self) -> &'static str {
        "Validates state machine transitions and sets the appropriate timestamp column per target status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let registry_id = extract_uuid(verb_call, ctx, "registry-id")?;
        let new_status = extract_string(verb_call, "new-status")?;
        let notes = extract_string_opt(verb_call, "notes");

        let current_status = fetch_current_status(pool, registry_id).await?;
        let timestamp_col = validate_advance_transition(&current_status, &new_status)?;

        // Build the UPDATE dynamically to set the correct timestamp column.
        // The column name comes from our own validated constant, not user input.
        let sql = format!(
            r#"
            UPDATE kyc.ubo_registry
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
            .execute(pool)
            .await?;

        let result = UboRegistryAdvanceResult {
            registry_id,
            previous_status: current_status,
            new_status,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// =============================================================================
// UboRegistryWaiveOp — Any non-terminal → WAIVED
// =============================================================================

/// Waives a UBO registry entry with reason and authority.
///
/// Any non-terminal status can transition to WAIVED. Records the authority
/// who approved the waiver and the reason.
#[register_custom_op]
pub struct UboRegistryWaiveOp;

#[async_trait]
impl CustomOperation for UboRegistryWaiveOp {
    fn domain(&self) -> &'static str {
        "ubo.registry"
    }

    fn verb(&self) -> &'static str {
        "waive"
    }

    fn rationale(&self) -> &'static str {
        "Waiver requires authority and reason tracking with audit trail, validates non-terminal source status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let registry_id = extract_uuid(verb_call, ctx, "registry-id")?;
        let reason = extract_string(verb_call, "reason")?;
        let authority = extract_string(verb_call, "authority")?;

        let current_status = fetch_current_status(pool, registry_id).await?;

        if is_terminal(&current_status) {
            return Err(anyhow!(
                "Cannot waive: entry is in terminal status '{}'. \
                 Only non-terminal statuses (CANDIDATE, IDENTIFIED, PROVABLE, PROVED, REVIEWED) can be waived.",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE kyc.ubo_registry
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
        .execute(pool)
        .await?;

        let result = UboRegistryWaiveResult {
            registry_id,
            previous_status: current_status,
            new_status: "WAIVED".to_string(),
            waived_by: authority,
            waiver_reason: reason,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// =============================================================================
// UboRegistryRejectOp — Any non-terminal → REJECTED
// =============================================================================

/// Rejects a UBO registry entry with a reason.
///
/// Any non-terminal status can transition to REJECTED. This is a terminal state.
#[register_custom_op]
pub struct UboRegistryRejectOp;

#[async_trait]
impl CustomOperation for UboRegistryRejectOp {
    fn domain(&self) -> &'static str {
        "ubo.registry"
    }

    fn verb(&self) -> &'static str {
        "reject"
    }

    fn rationale(&self) -> &'static str {
        "Rejection validates non-terminal source status and records reason with timestamp"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let registry_id = extract_uuid(verb_call, ctx, "registry-id")?;
        let reason = extract_string(verb_call, "reason")?;

        let current_status = fetch_current_status(pool, registry_id).await?;

        if is_terminal(&current_status) {
            return Err(anyhow!(
                "Cannot reject: entry is in terminal status '{}'. \
                 Only non-terminal statuses (CANDIDATE, IDENTIFIED, PROVABLE, PROVED, REVIEWED) can be rejected.",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE kyc.ubo_registry
            SET status = 'REJECTED',
                rejected_at = NOW(),
                notes = $2,
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
        )
        .bind(registry_id)
        .bind(&reason)
        .execute(pool)
        .await?;

        let result = UboRegistryRejectResult {
            registry_id,
            previous_status: current_status,
            new_status: "REJECTED".to_string(),
            reason,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// =============================================================================
// UboRegistryExpireOp — Any non-terminal → EXPIRED
// =============================================================================

/// Expires a UBO registry entry.
///
/// Any non-terminal status can transition to EXPIRED. This is a terminal state
/// typically triggered by time-based validity lapse.
#[register_custom_op]
pub struct UboRegistryExpireOp;

#[async_trait]
impl CustomOperation for UboRegistryExpireOp {
    fn domain(&self) -> &'static str {
        "ubo.registry"
    }

    fn verb(&self) -> &'static str {
        "expire"
    }

    fn rationale(&self) -> &'static str {
        "Expiry validates non-terminal source status and sets expired_at timestamp"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let registry_id = extract_uuid(verb_call, ctx, "registry-id")?;
        let reason = extract_string_opt(verb_call, "reason");

        let current_status = fetch_current_status(pool, registry_id).await?;

        if is_terminal(&current_status) {
            return Err(anyhow!(
                "Cannot expire: entry is in terminal status '{}'. \
                 Only non-terminal statuses (CANDIDATE, IDENTIFIED, PROVABLE, PROVED, REVIEWED) can be expired.",
                current_status
            ));
        }

        sqlx::query(
            r#"
            UPDATE kyc.ubo_registry
            SET status = 'EXPIRED',
                expired_at = NOW(),
                notes = COALESCE($2, notes),
                updated_at = NOW()
            WHERE registry_id = $1
            "#,
        )
        .bind(registry_id)
        .bind(&reason)
        .execute(pool)
        .await?;

        let result = UboRegistryExpireResult {
            registry_id,
            previous_status: current_status,
            new_status: "EXPIRED".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}
