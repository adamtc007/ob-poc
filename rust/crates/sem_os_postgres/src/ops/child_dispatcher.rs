//! Registry-backed implementation of the SemOS child-dispatch service.

use crate::ops::SemOsVerbOpRegistry;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime::SemOsChildDispatcher;
use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use serde_json::Value;
use std::sync::Arc;

/// Dispatches child verbs through an existing [`SemOsVerbOpRegistry`].
///
/// The dispatcher does not open, commit, or roll back transactions. It reuses
/// the caller-provided [`TransactionScope`] so parent/child writes remain in
/// the same execution boundary.
///
/// # Examples
///
/// ```rust,ignore
/// let registry = std::sync::Arc::new(sem_os_postgres::ops::build_registry());
/// let dispatcher = sem_os_postgres::ops::RegistryChildDispatcher::new(registry);
/// ```
pub struct RegistryChildDispatcher {
    registry: Arc<SemOsVerbOpRegistry>,
}

impl RegistryChildDispatcher {
    /// Create a dispatcher backed by `registry`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let registry = std::sync::Arc::new(sem_os_postgres::ops::build_registry());
    /// let dispatcher = sem_os_postgres::ops::RegistryChildDispatcher::new(registry);
    /// ```
    pub fn new(registry: Arc<SemOsVerbOpRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl SemOsChildDispatcher for RegistryChildDispatcher {
    async fn dispatch_child(
        &self,
        parent_fqn: &str,
        child_fqn: &str,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        tracing::debug!(
            parent_fqn,
            child_fqn,
            "dispatching SemOS child verb through registry"
        );
        let op = self.registry.get(child_fqn).ok_or_else(|| {
            anyhow!("child SemOS verb `{child_fqn}` is not registered for parent `{parent_fqn}`")
        })?;
        op.execute(args, ctx, scope).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::SemOsVerbOp;
    use async_trait::async_trait;
    use dsl_runtime::TransactionScope;
    use ob_poc_types::TransactionScopeId;
    use sqlx::{PgPool, Postgres, Transaction};
    use uuid::Uuid;

    struct NullScope;

    impl TransactionScope for NullScope {
        fn scope_id(&self) -> TransactionScopeId {
            TransactionScopeId::new()
        }

        fn transaction(&mut self) -> &mut Transaction<'static, Postgres> {
            unimplemented!("test child op does not access the transaction")
        }

        fn pool(&self) -> &PgPool {
            unimplemented!("test child op does not access the pool")
        }
    }

    struct BindingChild;

    #[async_trait]
    impl SemOsVerbOp for BindingChild {
        fn fqn(&self) -> &str {
            "child.bind"
        }

        async fn execute(
            &self,
            _args: &Value,
            ctx: &mut VerbExecutionContext,
            _scope: &mut dyn TransactionScope,
        ) -> Result<VerbExecutionOutcome> {
            ctx.bind("child_seen", Uuid::nil());
            Ok(VerbExecutionOutcome::Affected(1))
        }
    }

    #[tokio::test]
    async fn dispatch_child_uses_registry_and_preserves_context_mutations() {
        let mut registry = SemOsVerbOpRegistry::empty();
        registry.register(Arc::new(BindingChild));
        let dispatcher = RegistryChildDispatcher::new(Arc::new(registry));
        let mut ctx = VerbExecutionContext::default();
        let mut scope = NullScope;

        let outcome = dispatcher
            .dispatch_child(
                "parent.test",
                "child.bind",
                &serde_json::json!({}),
                &mut ctx,
                &mut scope,
            )
            .await
            .expect("child dispatch should succeed");

        assert!(matches!(outcome, VerbExecutionOutcome::Affected(1)));
        assert_eq!(ctx.resolve("child_seen"), Some(Uuid::nil()));
    }

    #[tokio::test]
    async fn dispatch_child_errors_when_child_is_missing() {
        let dispatcher = RegistryChildDispatcher::new(Arc::new(SemOsVerbOpRegistry::empty()));
        let mut ctx = VerbExecutionContext::default();
        let mut scope = NullScope;

        let err = dispatcher
            .dispatch_child(
                "parent.test",
                "child.missing",
                &serde_json::json!({}),
                &mut ctx,
                &mut scope,
            )
            .await
            .expect_err("missing child should error");

        assert!(err.to_string().contains("child.missing"));
    }
}
