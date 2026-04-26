//! Team verbs — SemOS-side YAML-first re-implementation.
//!
//! One non-trivial verb (`team.transfer-member`) that atomically moves
//! a user between teams. The legacy impl opened its own nested txn
//! via `pool.begin()`; the SemOS version runs in the Sequencer-owned
//! scope, so there is no nested `BEGIN` — the outer scope commits the
//! two table writes together. All other team CRUD is handled by the
//! generic executor against `team.yaml`.

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_string, json_extract_uuid};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct TransferMember;

#[async_trait]
impl SemOsVerbOp for TransferMember {
    fn fqn(&self) -> &str {
        "team.transfer-member"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let from_team = json_extract_uuid(args, ctx, "from-team")?;
        let to_team = json_extract_uuid(args, ctx, "to-team")?;
        let user_id = json_extract_uuid(args, ctx, "user")?;
        let new_role = json_extract_string(args, "new-role")?;

        // Step 1: Verify user exists as an active member of source team.
        // Runtime `sqlx::query_scalar` (not the macro) so the SQL doesn't
        // need a new sqlx-offline cache entry keyed by this crate.
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT membership_id FROM "ob-poc".memberships
            WHERE team_id = $1 AND user_id = $2 AND effective_to IS NULL
            LIMIT 1
            "#,
        )
        .bind(from_team)
        .bind(user_id)
        .fetch_optional(scope.executor())
        .await?;

        if existing.is_none() {
            return Err(anyhow::anyhow!(
                "team.transfer-member: User is not an active member of source team"
            ));
        }

        // Step 2: End all active memberships in source team.
        sqlx::query(
            r#"
            UPDATE "ob-poc".memberships
            SET effective_to = CURRENT_DATE
            WHERE team_id = $1 AND user_id = $2 AND effective_to IS NULL
            "#,
        )
        .bind(from_team)
        .bind(user_id)
        .execute(scope.executor())
        .await?;

        // Step 3: Create membership in target team.
        let new_membership_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".memberships (team_id, user_id, role_key, effective_from)
            VALUES ($1, $2, $3, CURRENT_DATE)
            RETURNING membership_id
            "#,
        )
        .bind(to_team)
        .bind(user_id)
        .bind(&new_role)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(new_membership_id))
    }
}
