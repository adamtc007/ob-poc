//! Source-level invariant tests for the runbook module.
//!
//! These tests use `include_str!` to scan source files for structural
//! invariants. They must be internal to the crate because they reference
//! source files by relative path.

use super::*;

// =============================================================================
// INV-8, INV-10: Lock event logging in executor source
// =============================================================================

#[test]
fn test_executor_lock_event_logging() {
    let executor_source = include_str!("executor.rs");

    assert!(
        executor_source.contains("\"lock_acquired\""),
        "INV-10: executor must log lock_acquired events"
    );
    assert!(
        executor_source.contains("\"lock_released\""),
        "INV-10: executor must log lock_released events"
    );
    assert!(
        executor_source.contains("\"lock_contention\""),
        "INV-10: executor must log lock_contention events"
    );
    assert!(
        executor_source.contains("holder_runbook_id"),
        "INV-10: LockError::Contention must carry holder_runbook_id"
    );
}

// =============================================================================
// INV-1, INV-11: All execution paths gated through compiled runbook
// =============================================================================

#[test]
fn test_execution_gate_source_invariants() {
    // INV-11: Both Chat API and REPL reference execute_runbook
    let agent_source = include_str!("../api/agent_service.rs");
    let repl_source = include_str!("../repl/orchestrator_v2.rs");

    assert!(
        agent_source.contains("execute_runbook"),
        "INV-11: Chat API (agent_service.rs) must reference execute_runbook"
    );
    assert!(
        repl_source.contains("execute_runbook"),
        "INV-11: REPL (orchestrator_v2.rs) must reference execute_runbook"
    );

    // INV-1: No ungated execute_dsl calls in agent_service.rs
    let mut in_not_gate = false;
    let mut ungated_calls: Vec<(usize, String)> = Vec::new();
    for (i, line) in agent_source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.contains("cfg(not(feature = \"runbook-gate-vnext\"))") {
            in_not_gate = true;
        }
        if (trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub async fn "))
            && !in_not_gate
        {
            in_not_gate = false;
        }
        if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("///") {
            continue;
        }
        if (trimmed.contains("execute_dsl(") || trimmed.contains(".execute_dsl(")) && !in_not_gate {
            ungated_calls.push((i + 1, line.to_string()));
        }
    }
    assert!(
        ungated_calls.is_empty(),
        "INV-1: Found ungated execute_dsl calls in agent_service.rs: {:?}",
        ungated_calls
    );
}

// =============================================================================
// INV-2: No HashMap in canonical types
// =============================================================================

#[test]
fn test_no_hashmap_in_canonical_types() {
    let types_source = include_str!("types.rs");
    let envelope_source = include_str!("envelope.rs");

    let hashmap_in_types: Vec<&str> = types_source
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//")
                && !t.starts_with("///")
                && !t.starts_with("*")
                && t.contains("HashMap")
                && !t.contains("BTreeMap")
                && !t.contains("#[cfg(test)]")
        })
        .collect();
    assert!(
        hashmap_in_types.is_empty(),
        "INV-2: No HashMap in runbook types.rs (use BTreeMap), found: {:?}",
        hashmap_in_types
    );

    let hashmap_in_envelope: Vec<&str> = envelope_source
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//")
                && !t.starts_with("///")
                && !t.starts_with("*")
                && t.contains("HashMap")
                && !t.contains("BTreeMap")
                && !t.contains("#[cfg(test)]")
                && !t.contains("use std::collections")
        })
        .collect();
    assert!(
        hashmap_in_envelope.is_empty(),
        "INV-2: No HashMap in runbook envelope.rs (use BTreeMap), found: {:?}",
        hashmap_in_envelope
    );
}

// =============================================================================
// INV-5: DAG assembly present in compiler
// =============================================================================

#[test]
fn test_dag_assembly_in_compiler() {
    let assembler_source = include_str!("compiler.rs");
    assert!(
        assembler_source.contains("assemble_plan") || assembler_source.contains("toposort"),
        "INV-5: Compilation must use DAG assembly (Kahn's algorithm)"
    );
}

// =============================================================================
// INV-7: All CompilationErrorKind variants
// =============================================================================

#[test]
fn test_all_compilation_error_variants() {
    let variants: Vec<CompilationErrorKind> = vec![
        CompilationErrorKind::ExpansionFailed {
            reason: "test".into(),
        },
        CompilationErrorKind::CycleDetected {
            cycle: vec!["A".into()],
        },
        CompilationErrorKind::LimitsExceeded {
            detail: "test".into(),
        },
        CompilationErrorKind::DagError {
            reason: "test".into(),
        },
        CompilationErrorKind::PackConstraint {
            verb: "test".into(),
            explanation: "test".into(),
        },
        CompilationErrorKind::SemRegDenied {
            verb: "test".into(),
            reason: "test".into(),
        },
        CompilationErrorKind::StoreFailed {
            reason: "test".into(),
        },
    ];
    assert_eq!(
        variants.len(),
        7,
        "INV-7: Must have exactly 7 error variants"
    );
    for v in &variants {
        assert!(
            !v.to_string().is_empty(),
            "All variants must produce non-empty Display"
        );
    }
}

// =============================================================================
// INV-13: execute_runbook never re-expands macros
// =============================================================================

#[test]
fn test_replay_never_re_expands() {
    let executor_source = include_str!("executor.rs");

    assert!(
        !executor_source.contains("expand_macro(")
            && !executor_source.contains("expand_macro_fixpoint("),
        "INV-13: execute_runbook must never call expand_macro — \
         the stored artefact is the only executable truth"
    );

    assert!(
        executor_source.contains("store.get("),
        "INV-13: execute_runbook must read from store"
    );
}
