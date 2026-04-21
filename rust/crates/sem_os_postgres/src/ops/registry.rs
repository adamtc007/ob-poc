//! Runtime registry of [`SemOsVerbOp`] implementations, keyed by FQN.
//!
//! Populated at startup from an explicit builder (no `inventory::collect!`
//! — the SemOS side keeps registration manual so startup is fully
//! predictable and the build order is not linker-dependent).

use std::collections::HashMap;
use std::sync::Arc;

use super::SemOsVerbOp;

/// Manual registry keyed by `(domain, verb)` fully-qualified name.
#[derive(Default)]
pub struct SemOsVerbOpRegistry {
    ops: HashMap<String, Arc<dyn SemOsVerbOp>>,
}

impl SemOsVerbOpRegistry {
    /// Empty registry — add ops via [`Self::register`].
    pub fn empty() -> Self {
        Self {
            ops: HashMap::new(),
        }
    }

    /// Register an op. Panics on duplicate FQN — catches double-registration
    /// bugs at startup.
    pub fn register(&mut self, op: Arc<dyn SemOsVerbOp>) -> &mut Self {
        let fqn = op.fqn().to_string();
        if self.ops.contains_key(&fqn) {
            panic!("Duplicate SemOsVerbOp registration: {fqn}");
        }
        self.ops.insert(fqn, op);
        self
    }

    /// Look up an op by FQN.
    pub fn get(&self, fqn: &str) -> Option<&Arc<dyn SemOsVerbOp>> {
        self.ops.get(fqn)
    }

    /// Is this FQN registered?
    pub fn has(&self, fqn: &str) -> bool {
        self.ops.contains_key(fqn)
    }

    /// Number of registered ops.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Sorted FQN snapshot for diagnostics.
    pub fn manifest(&self) -> Vec<String> {
        let mut fqns: Vec<_> = self.ops.keys().cloned().collect();
        fqns.sort();
        fqns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use dsl_runtime::tx::TransactionScope;
    use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

    struct StubOp(&'static str);

    #[async_trait]
    impl SemOsVerbOp for StubOp {
        fn fqn(&self) -> &str {
            self.0
        }
        async fn execute(
            &self,
            _args: &serde_json::Value,
            _ctx: &mut VerbExecutionContext,
            _scope: &mut dyn TransactionScope,
        ) -> Result<VerbExecutionOutcome> {
            Ok(VerbExecutionOutcome::Void)
        }
    }

    #[test]
    fn empty_registry_has_zero_ops() {
        let r = SemOsVerbOpRegistry::empty();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(!r.has("entity.ghost"));
    }

    #[test]
    fn register_and_lookup_roundtrip() {
        let mut r = SemOsVerbOpRegistry::empty();
        r.register(Arc::new(StubOp("entity.ghost")));
        assert!(r.has("entity.ghost"));
        assert!(r.get("entity.ghost").is_some());
        assert_eq!(r.len(), 1);
        assert_eq!(r.manifest(), vec!["entity.ghost".to_string()]);
    }

    #[test]
    #[should_panic(expected = "Duplicate SemOsVerbOp registration")]
    fn duplicate_registration_panics() {
        let mut r = SemOsVerbOpRegistry::empty();
        r.register(Arc::new(StubOp("entity.ghost")));
        r.register(Arc::new(StubOp("entity.ghost")));
    }
}
