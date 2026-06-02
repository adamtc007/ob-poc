//! CBU group membership verbs.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime::TransactionScope;
use dsl_runtime::{json_extract_bool_opt, json_extract_uuid};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use serde_json::Value;

use super::SemOsVerbOp;

/// Remove or terminate CBU group memberships for a CBU.
///
/// # Examples
///
/// ```rust,ignore
/// let op = sem_os_postgres::ops::cbu_group::RemoveMember;
/// assert_eq!(op.fqn(), "cbu-group.remove-member");
/// ```
pub struct RemoveMember;

#[async_trait]
impl SemOsVerbOp for RemoveMember {
    fn fqn(&self) -> &str {
        "cbu-group.remove-member"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let hard_delete = json_extract_bool_opt(args, "hard-delete").unwrap_or(false);
        let affected = if hard_delete {
            sqlx::query(r#"DELETE FROM "ob-poc".cbu_group_members WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(scope.executor())
                .await?
                .rows_affected()
        } else {
            sqlx::query(
                r#"
                UPDATE "ob-poc".cbu_group_members
                SET effective_to = CURRENT_DATE
                WHERE cbu_id = $1
                  AND effective_to IS NULL
                "#,
            )
            .bind(cbu_id)
            .execute(scope.executor())
            .await?
            .rows_affected()
        };

        if affected > 0 {
            dsl_runtime::emit_pending_state_advance(
                ctx,
                cbu_id,
                "cbu-group-member:removed",
                "cbu/group-membership",
                "cbu-group.remove-member",
            );
        }

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}
