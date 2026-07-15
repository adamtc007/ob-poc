//! Shared implementation of the "dispatching-fold" verb pattern (see
//! CLAUDE.md, "Dispatching-fold pattern" and "Selector-arg dispatching-fold
//! pattern"): one discoverable, registered verb that folds N specialist
//! `SemOsVerbOp` structs behind a `<noun>-type` selector arg, keeping every
//! specialist's write-path while deduplicating the discoverable/registered
//! surface.
//!
//! Before this module, each fold verb (`cbu.assign-role`, `client-group.*`,
//! `gleif.*`) hand-rolled its own `json_extract_string_opt(args,
//! "...-type").map(|s| s.to_uppercase())` + `match` block — three
//! independent implementations, three different arg-name conventions
//! (`role-type`, `action`, `target-type`), no shared contract. That gap is
//! exactly what let a caller (`cbu.create`'s fund/manco cascade) target a
//! pre-fold FQN that had been intentionally unregistered, silently, for
//! weeks — see docs/research/control-plane-ownership-ledger.md.
//!
//! Centralizing extraction + matching here does two things: it removes the
//! duplication, and it turns each fold verb's arm table into a real,
//! locatable Rust value (a `&[(&[&str], &dyn SemOsVerbOp)]` literal passed
//! to a known function) instead of pattern-match arms — which is what lets
//! `cargo x registry-graph` extend its static extraction to this pattern
//! (see `xtask/src/registry_graph.rs`'s fold-verb detection) rather than
//! needing to parse arbitrary match statements.

use anyhow::{anyhow, Result};
use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use serde_json::Value;

use super::SemOsVerbOp;

/// Resolution outcome for a selector-arg lookup — deliberately returned
/// rather than immediately dispatched, so callers with a generic fallback
/// (e.g. `cbu.assign-role`, whose unmatched case is ~25 lines of bespoke
/// role-binding logic, not another `SemOsVerbOp`) can still use the shared
/// extraction/matching without being forced into an all-or-nothing dispatch
/// call. Callers with no fallback (`client-group.*`, `gleif.*`) should use
/// [`dispatch_selector`] instead, which folds this straight into a
/// dispatch-or-error call.
pub enum SelectorMatch<'a> {
    /// `arg_name` was present in `args` and matched one of `arms`.
    Matched(&'a dyn SemOsVerbOp),
    /// `arg_name` was absent from `args` entirely.
    Absent,
    /// `arg_name` was present but didn't match any declared arm.
    Unrecognized(String),
}

/// Extract `arg_name` from `args`, uppercase it, and match it against
/// `arms` — each arm a `(accepted values, specialist op)` pair (multiple
/// accepted spellings per arm, e.g. `TRUST`/`TRUST_ROLE`, are supported
/// directly rather than requiring a normalization step first).
pub fn resolve_selector<'a>(
    args: &Value,
    arg_name: &str,
    arms: &[(&[&str], &'a dyn SemOsVerbOp)],
) -> SelectorMatch<'a> {
    let Some(raw) = args.get(arg_name).and_then(|v| v.as_str()) else {
        return SelectorMatch::Absent;
    };
    let normalized = raw.to_uppercase();
    for (values, op) in arms {
        if values.contains(&normalized.as_str()) {
            return SelectorMatch::Matched(*op);
        }
    }
    SelectorMatch::Unrecognized(raw.to_string())
}

/// The common exhaustive-fold shape (no generic fallback — an
/// absent/unrecognized selector is always a hard error): resolves and
/// dispatches in one call, with a uniform error message listing every
/// accepted value.
pub async fn dispatch_selector(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
    arg_name: &str,
    arms: &[(&[&str], &dyn SemOsVerbOp)],
) -> Result<VerbExecutionOutcome> {
    match resolve_selector(args, arg_name, arms) {
        SelectorMatch::Matched(op) => op.execute(args, ctx, scope).await,
        SelectorMatch::Absent => Err(anyhow!(
            "{arg_name} required ({})",
            valid_values_list(arms)
        )),
        SelectorMatch::Unrecognized(v) => Err(anyhow!(
            "unknown {arg_name} '{v}'. Valid: {}",
            valid_values_list(arms)
        )),
    }
}

fn valid_values_list(arms: &[(&[&str], &dyn SemOsVerbOp)]) -> String {
    arms.iter()
        .flat_map(|(values, _)| values.iter())
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct Op(&'static str);
    #[async_trait]
    impl SemOsVerbOp for Op {
        fn fqn(&self) -> &str {
            self.0
        }
        async fn execute(
            &self,
            _args: &Value,
            _ctx: &mut VerbExecutionContext,
            _scope: &mut dyn TransactionScope,
        ) -> Result<VerbExecutionOutcome> {
            Ok(VerbExecutionOutcome::Void)
        }
    }

    #[test]
    fn matches_case_insensitively() {
        let a = Op("a");
        let b = Op("b");
        let arms: &[(&[&str], &dyn SemOsVerbOp)] =
            &[(&["FOO"], &a as &dyn SemOsVerbOp), (&["BAR", "BAZ"], &b as &dyn SemOsVerbOp)];
        let args = serde_json::json!({ "kind": "foo" });
        match resolve_selector(&args, "kind", arms) {
            SelectorMatch::Matched(op) => assert_eq!(op.fqn(), "a"),
            _ => panic!("expected a match"),
        }
        let args = serde_json::json!({ "kind": "baz" });
        match resolve_selector(&args, "kind", arms) {
            SelectorMatch::Matched(op) => assert_eq!(op.fqn(), "b"),
            _ => panic!("expected a match"),
        }
    }

    #[test]
    fn absent_vs_unrecognized() {
        let a = Op("a");
        let arms: &[(&[&str], &dyn SemOsVerbOp)] = &[(&["FOO"], &a as &dyn SemOsVerbOp)];
        assert!(matches!(
            resolve_selector(&serde_json::json!({}), "kind", arms),
            SelectorMatch::Absent
        ));
        assert!(matches!(
            resolve_selector(&serde_json::json!({"kind": "nope"}), "kind", arms),
            SelectorMatch::Unrecognized(v) if v == "nope"
        ));
    }
}
