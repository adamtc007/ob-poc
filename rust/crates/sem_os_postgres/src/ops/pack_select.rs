//! `pack.select` — SemOS-side YAML-first re-implementation.
//!
//! # Contract (from `rust/config/verbs/pack.yaml`)
//!
//! ```yaml
//! pack:
//!   verbs:
//!     select:
//!       description: Select a journey pack for the current session
//!       behavior: plugin
//!       metadata:
//!         tier: intent
//!         source_of_truth: session
//!         noun: pack
//!         side_effects: state_write   # session-local, no DB
//!       args:
//!         - name: pack-id         required=true  string
//!         - name: pack-version    required=false string  # defaults to "latest"
//!         - name: manifest-hash   required=false string
//!         - name: handoff-from    required=false string
//!       returns:
//!         type: record
//!         fields: [pack_id, pack_name, pack_version]
//! ```
//!
//! Re-implemented fresh against that contract — the op records pack
//! selection on the runbook so session state is derivable by fold
//! (Invariant I-1). No database effects; the scoped txn supplied by the
//! dispatcher is unused. This is the Phase A smoke-test op that proves
//! the SemOS-first dispatch path works end-to-end.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Serialize;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

const FQN: &str = "pack.select";

/// The shape returned to the caller — exactly the fields the YAML
/// contract declares.
#[derive(Debug, Clone, Serialize)]
struct PackSelectOutput {
    pack_id: String,
    pack_name: String,
    pack_version: String,
}

pub struct PackSelect;

#[async_trait]
impl SemOsVerbOp for PackSelect {
    fn fqn(&self) -> &str {
        FQN
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pack_id = args
            .get("pack-id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("{FQN}: required arg `pack-id` missing or not a string"))?
            .to_string();

        let pack_version = args
            .get("pack-version")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| "latest".to_string());

        let output = PackSelectOutput {
            pack_name: pack_id.clone(),
            pack_id,
            pack_version,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(output)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::principal::Principal;

    /// A minimal [`TransactionScope`] stub — `pack.select` has no DB
    /// effects so the transaction / pool methods are `unreachable!`.
    struct NullScope;

    impl TransactionScope for NullScope {
        fn scope_id(&self) -> ob_poc_types::TransactionScopeId {
            ob_poc_types::TransactionScopeId::new()
        }
        fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres> {
            unreachable!("pack.select should not touch the database")
        }
        fn pool(&self) -> &sqlx::PgPool {
            unreachable!("pack.select does not need the pool")
        }
    }

    #[tokio::test]
    async fn returns_record_with_yaml_declared_fields() {
        let op = PackSelect;
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let mut scope = NullScope;
        let args = serde_json::json!({"pack-id": "lux-ucits-sicav"});

        let outcome = op
            .execute(&args, &mut ctx, &mut scope)
            .await
            .expect("execute Ok");

        let VerbExecutionOutcome::Record(rec) = outcome else {
            panic!("expected Record outcome");
        };
        assert_eq!(rec["pack_id"], "lux-ucits-sicav");
        assert_eq!(rec["pack_name"], "lux-ucits-sicav");
        assert_eq!(rec["pack_version"], "latest");
        // YAML contract declares exactly 3 fields on the record.
        assert_eq!(rec.as_object().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn honours_explicit_pack_version() {
        let op = PackSelect;
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let mut scope = NullScope;
        let args = serde_json::json!({"pack-id": "book-setup", "pack-version": "v2"});

        let outcome = op.execute(&args, &mut ctx, &mut scope).await.unwrap();
        let VerbExecutionOutcome::Record(rec) = outcome else {
            unreachable!()
        };
        assert_eq!(rec["pack_version"], "v2");
    }

    #[tokio::test]
    async fn missing_pack_id_errors() {
        let op = PackSelect;
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let mut scope = NullScope;
        let args = serde_json::json!({});

        let err = op.execute(&args, &mut ctx, &mut scope).await.unwrap_err();
        assert!(err.to_string().contains("pack-id"));
    }
}
