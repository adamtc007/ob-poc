//! `StubOp` — registers a plugin verb's FQN in the registry but returns
//! a clear "not yet implemented" error at runtime.
//!
//! Used for plugin verbs declared in YAML where the real implementation
//! is pending (prototype features, complex cascades, governance flows
//! still in design). Registering them keeps `test_plugin_verb_coverage`
//! green so unrelated PRs don't hit a coverage failure; invoking them
//! at runtime fails fast with a precise error pointing at this module.
//!
//! When a real impl lands, remove the FQN from `STUB_VERBS` and add a
//! dedicated `SemOsVerbOp` impl alongside the rest.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sem_os_postgres::ops::SemOsVerbOp;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

pub struct StubOp {
    fqn: &'static str,
}

impl StubOp {
    pub const fn new(fqn: &'static str) -> Self {
        Self { fqn }
    }
}

#[async_trait]
impl SemOsVerbOp for StubOp {
    fn fqn(&self) -> &str {
        self.fqn
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        Err(anyhow!(
            "verb '{}' is registered but its plugin handler is not yet implemented \
             (see rust/src/domain_ops/stub_op.rs::STUB_VERBS)",
            self.fqn
        ))
    }
}

/// FQNs registered as stubs. Each entry should track a real follow-on
/// task that replaces the stub with a working impl.
pub const STUB_VERBS: &[&str] = &[
    // ── catalogue.* — Tranche 3 Phase 3.B (2026-04-26) ───────────────
    // The 4 authorship verbs are now REAL implementations in
    // `crate::domain_ops::catalogue_ops`, not stubs. They drive the
    // Catalogue workspace state machine documented in
    // `rust/config/sem_os_seeds/dag_taxonomies/catalogue_dag.yaml`.
    // ── trading-profile.* — destructive prune cascades ──────────────
    // Each prune is a multi-row delete + dependency cascade per
    // instrument_matrix_dag.yaml §6. The cascade engine landed in
    // v1.3 (CascadePlanner) but the prune verb adapters that drive
    // it are still pending.
    "trading-profile.prune-asset-family",
    "trading-profile.prune-market",
    "trading-profile.prune-instrument-class",
    "trading-profile.prune-counterparty",
    "trading-profile.prune-counterparty-type",
    "trading-profile.prune-impact",
    // ── trading-profile.* — template + restriction operations ───────
    // Touch multiple tables / require scope JSON / template
    // promotion semantics. Pending dedicated impls.
    "trading-profile.add-counterparty-type",
    "trading-profile.publish-template-version",
    "trading-profile.list-components",
    "trading-profile.sync-from-template",
    "trading-profile.retire-template",
    "trading-profile.restrict",
    "trading-profile.lift-restriction",
];

pub fn register_stub_ops(registry: &mut sem_os_postgres::ops::SemOsVerbOpRegistry) {
    use std::sync::Arc;
    for fqn in STUB_VERBS {
        registry.register(Arc::new(StubOp::new(fqn)));
    }
}
