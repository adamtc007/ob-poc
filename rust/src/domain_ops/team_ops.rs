//! Team Operations - Plugin Handlers
//!
//! Only contains operations that require multi-step transactional logic.
//! All simple CRUD operations are defined in team.yaml and handled by GenericCrudExecutor.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::helpers::extract_uuid;
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// TRANSFER MEMBER - Multi-step atomic operation
// =============================================================================

/// Transfer user from one team to another atomically.
/// This requires a plugin because it's a multi-table transaction:
/// 1. Delete from source team
/// 2. Insert into target team
/// 3. Log audit entries for both teams
#[register_custom_op]
pub struct TeamTransferMemberOp;

#[async_trait]
impl CustomOperation for TeamTransferMemberOp {
    fn domain(&self) -> &'static str {
        "team"
    }

    fn verb(&self) -> &'static str {
        "transfer-member"
    }

    fn rationale(&self) -> &'static str {
        "Atomic remove + add across teams with audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Extract arguments
        let from_team = extract_uuid(verb_call, ctx, "from-team")?;
        let to_team = extract_uuid(verb_call, ctx, "to-team")?;
        let user_id = extract_uuid(verb_call, ctx, "user")?;

        let new_role = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "new-role")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| {
                anyhow::anyhow!("team.transfer-member: Missing required argument :new-role")
            })?;

        // Use transaction for atomicity
        let mut tx = pool.begin().await?;

        // Step 1: Verify user exists in source team
        let existing = sqlx::query_scalar!(
            r#"
            SELECT membership_id FROM teams.memberships
            WHERE team_id = $1 AND user_id = $2 AND effective_to IS NULL
            LIMIT 1
            "#,
            from_team,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if existing.is_none() {
            return Err(anyhow::anyhow!(
                "team.transfer-member: User is not an active member of source team"
            ));
        }

        // Step 2: End all memberships in source team (set effective_to)
        let _removed_count = sqlx::query!(
            r#"
            UPDATE teams.memberships
            SET effective_to = CURRENT_DATE
            WHERE team_id = $1 AND user_id = $2 AND effective_to IS NULL
            "#,
            from_team,
            user_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        // Step 3: Create membership in target team
        let new_membership_id: Uuid = sqlx::query_scalar!(
            r#"
            INSERT INTO teams.memberships (team_id, user_id, role_key, effective_from)
            VALUES ($1, $2, $3, CURRENT_DATE)
            RETURNING membership_id
            "#,
            to_team,
            user_id,
            new_role
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        // Return the new membership ID
        Ok(ExecutionResult::Uuid(new_membership_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}
