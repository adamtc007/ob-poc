//! `pack.answer` — SemOS-side YAML-first re-implementation.
//!
//! # Contract (from `rust/config/verbs/pack.yaml`)
//!
//! ```yaml
//! pack:
//!   verbs:
//!     answer:
//!       description: Record an answer to a pack question
//!       behavior: plugin
//!       metadata:
//!         tier: intent
//!         source_of_truth: session
//!         noun: pack_answer
//!         side_effects: state_write   # session-local, no DB
//!       args:
//!         - name: field    required=true  string
//!         - name: value    required=true  string
//!         - name: pack-id  required=false string
//!       returns:
//!         type: record
//!         fields: [field, value, accepted]
//! ```
//!
//! Re-implemented fresh against that contract — the op records a pack Q&A
//! answer on the runbook so accumulated answers are derivable by fold
//! (Invariant I-1). No database effects; the scoped txn supplied by the
//! dispatcher is unused.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Serialize;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

const FQN: &str = "pack.answer";

/// The shape returned to the caller — exactly the fields the YAML
/// contract declares.
#[derive(Debug, Clone, Serialize)]
struct PackAnswerOutput {
    field: String,
    value: String,
    accepted: bool,
}

pub struct PackAnswer;

#[async_trait]
impl SemOsVerbOp for PackAnswer {
    fn fqn(&self) -> &str {
        FQN
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let field = args
            .get("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("{FQN}: required arg `field` missing or not a string"))?
            .to_string();

        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("{FQN}: required arg `value` missing or not a string"))?
            .to_string();

        let output = PackAnswerOutput {
            field,
            value,
            accepted: true,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(output)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::principal::Principal;

    struct NullScope;
    impl TransactionScope for NullScope {
        fn scope_id(&self) -> ob_poc_types::TransactionScopeId {
            ob_poc_types::TransactionScopeId::new()
        }
        fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres> {
            unreachable!("pack.answer should not touch the database")
        }
        fn pool(&self) -> &sqlx::PgPool {
            unreachable!("pack.answer does not need the pool")
        }
    }

    #[tokio::test]
    async fn returns_record_with_yaml_declared_fields() {
        let op = PackAnswer;
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let mut scope = NullScope;
        let args = serde_json::json!({"field": "jurisdiction", "value": "LU"});

        let outcome = op.execute(&args, &mut ctx, &mut scope).await.unwrap();
        let VerbExecutionOutcome::Record(rec) = outcome else {
            panic!("expected Record outcome");
        };
        assert_eq!(rec["field"], "jurisdiction");
        assert_eq!(rec["value"], "LU");
        assert_eq!(rec["accepted"], true);
        assert_eq!(rec.as_object().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn missing_field_errors() {
        let op = PackAnswer;
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let mut scope = NullScope;
        let args = serde_json::json!({"value": "LU"});

        let err = op.execute(&args, &mut ctx, &mut scope).await.unwrap_err();
        assert!(err.to_string().contains("field"));
    }

    #[tokio::test]
    async fn missing_value_errors() {
        let op = PackAnswer;
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let mut scope = NullScope;
        let args = serde_json::json!({"field": "jurisdiction"});

        let err = op.execute(&args, &mut ctx, &mut scope).await.unwrap_err();
        assert!(err.to_string().contains("value"));
    }
}
