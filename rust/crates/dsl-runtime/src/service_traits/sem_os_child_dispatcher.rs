//! Service trait for in-scope SemOS child-verb dispatch.

use crate::tx::TransactionScope;
use crate::{VerbExecutionContext, VerbExecutionOutcome};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Dispatches a child SemOS verb through the host's existing verb registry.
///
/// Parent plugin ops use this service when they need a child write to happen
/// inside the same [`TransactionScope`]. The trait deliberately exposes only
/// the existing SemOS verb-dispatch shape; it is not a separate cascade engine.
///
/// # Examples
///
/// ```rust,ignore
/// let dispatcher = ctx.service::<dyn SemOsChildDispatcher>()?;
/// dispatcher
///     .dispatch_child("parent.verb", "child.verb", &serde_json::json!({}), ctx, scope)
///     .await?;
/// ```
#[async_trait]
pub trait SemOsChildDispatcher: Send + Sync {
    /// Dispatch `child_fqn` using `args`, mutating the supplied context and
    /// transaction scope in place.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// dispatcher
    ///     .dispatch_child(parent_fqn, child_fqn, args, ctx, scope)
    ///     .await?;
    /// ```
    async fn dispatch_child(
        &self,
        parent_fqn: &str,
        child_fqn: &str,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome>;
}
