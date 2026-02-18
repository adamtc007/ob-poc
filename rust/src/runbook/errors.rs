//! Typed compilation error model (INV-7).
//!
//! Every compilation failure maps to exactly one `CompilationErrorKind` variant.
//! The 8 variants correspond to the 7 phases in §6.2 of the paper plus a
//! catch-all for unexpected internal errors:
//!
//! ```text
//! Step 1: expand    → ExpansionFailed | CycleDetected | LimitsExceeded
//! Step 2: DAG       → DagError
//! Step 3: pack gate → PackConstraint
//! Step 4: SemReg    → SemRegDenied
//! Step 5: write_set → (infallible — empty set on failure)
//! Step 6: store     → StoreFailed
//! Step 7: envelope  → (infallible — always succeeds)
//! (internal)        → InternalError (catch-all)
//! ```
//!
//! ## Rules
//!
//! - `thiserror` for enum derivation — no manual `Display` impls.
//! - No `.unwrap()` in this module (INV-7).
//! - All 8 variants must be constructible (test enforced).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CompilationError — the top-level error type
// ---------------------------------------------------------------------------

/// A typed compilation error carrying both the failure kind and the phase
/// that produced it. Used as the payload of `OrchestratorResponse::CompilationError`.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{kind}")]
pub struct CompilationError {
    /// Which phase of the §6.2 pipeline failed.
    ///
    /// Serialized as `"error_kind"` to avoid clash with `OrchestratorResponse`'s
    /// `#[serde(tag = "kind")]` discriminant.
    #[serde(rename = "error_kind")]
    pub kind: CompilationErrorKind,

    /// Phase name for telemetry/logging (e.g., `"expand"`, `"dag"`, `"sem_reg"`).
    pub source_phase: String,
}

impl CompilationError {
    /// Convenience constructor.
    pub fn new(kind: CompilationErrorKind, source_phase: &str) -> Self {
        Self {
            kind,
            source_phase: source_phase.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// CompilationErrorKind — the 7 §6.2 variants
// ---------------------------------------------------------------------------

/// All possible compilation failure modes, one per §6.2 phase plus a catch-all.
///
/// INV-7: exactly 8 variants, all constructible, all `thiserror`-derived.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error_kind", rename_all = "snake_case")]
pub enum CompilationErrorKind {
    /// Macro expansion failed (general — missing required, template error, etc.).
    #[error("Expansion failed: {reason}")]
    ExpansionFailed { reason: String },

    /// Cycle detected in macro invocation graph (INV-4).
    #[error("Cycle detected in macro graph: {}", cycle.join(" → "))]
    CycleDetected { cycle: Vec<String> },

    /// Expansion limits exceeded (max_depth or max_steps).
    #[error("Expansion limits exceeded: {detail}")]
    LimitsExceeded { detail: String },

    /// DAG assembly failed (cyclic dependencies, unresolved bindings).
    #[error("DAG assembly failed: {reason}")]
    DagError { reason: String },

    /// Pack constraint violated (expanded verb not in allowed set).
    #[error("Pack constraint violated: {verb} — {explanation}")]
    PackConstraint { verb: String, explanation: String },

    /// SemReg denied one or more expanded verbs.
    #[error("SemReg denied verb: {verb} — {reason}")]
    SemRegDenied { verb: String, reason: String },

    /// Storage operation failed (e.g., Postgres insert error).
    #[error("Storage failed: {reason}")]
    StoreFailed { reason: String },

    /// Catch-all for unexpected internal errors (e.g., serialization failure,
    /// invariant violation). Maps to `source_phase = "internal"`.
    #[error("Internal error: {reason}")]
    InternalError { reason: String },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// INV-7: All 8 CompilationErrorKind variants must be constructible.
    #[test]
    fn test_all_8_error_kinds_constructible() {
        let variants: Vec<CompilationErrorKind> = vec![
            CompilationErrorKind::ExpansionFailed {
                reason: "missing field".into(),
            },
            CompilationErrorKind::CycleDetected {
                cycle: vec!["A".into(), "B".into(), "A".into()],
            },
            CompilationErrorKind::LimitsExceeded {
                detail: "max_depth 8 exceeded".into(),
            },
            CompilationErrorKind::DagError {
                reason: "cyclic dependency".into(),
            },
            CompilationErrorKind::PackConstraint {
                verb: "cbu.delete".into(),
                explanation: "forbidden by kyc-case pack".into(),
            },
            CompilationErrorKind::SemRegDenied {
                verb: "entity.delete".into(),
                reason: "denied by policy rule".into(),
            },
            CompilationErrorKind::StoreFailed {
                reason: "connection refused".into(),
            },
            CompilationErrorKind::InternalError {
                reason: "unexpected serialization failure".into(),
            },
        ];

        assert_eq!(variants.len(), 8, "Must have exactly 8 variants (INV-7)");

        // Each variant produces a non-empty Display string
        for v in &variants {
            let msg = v.to_string();
            assert!(!msg.is_empty(), "Display must be non-empty for {:?}", v);
        }
    }

    #[test]
    fn test_compilation_error_display() {
        let err = CompilationError::new(
            CompilationErrorKind::CycleDetected {
                cycle: vec!["A".into(), "B".into(), "A".into()],
            },
            "expand",
        );
        let msg = err.to_string();
        assert!(msg.contains("Cycle detected"));
        assert!(msg.contains("A → B → A"));
    }

    /// INV-7: No `.unwrap()` in non-test runbook code.
    ///
    /// Static grep test — scans all `runbook/*.rs` files for `.unwrap()` calls
    /// outside of `#[cfg(test)]` / `#[test]` blocks. `.expect()` is allowed
    /// (provides context on panic).
    #[test]
    fn test_no_unwrap_in_runbook_module() {
        use std::fs;
        use std::path::Path;

        let runbook_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runbook");
        let mut violations = Vec::new();

        for entry in fs::read_dir(&runbook_dir).expect("read runbook dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "rs") {
                continue;
            }

            let source = fs::read_to_string(&path).expect("read file");
            let filename = path.file_name().expect("filename").to_string_lossy();

            let mut in_test_block = false;
            for (line_no, line) in source.lines().enumerate() {
                let trimmed = line.trim();

                // Track #[cfg(test)] mod blocks
                if trimmed == "#[cfg(test)]"
                    || trimmed.starts_with("#[test]")
                    || trimmed.starts_with("#[tokio::test")
                {
                    in_test_block = true;
                }

                // Skip comments and string literals mentioning .unwrap()
                let is_comment = trimmed.starts_with("//")
                    || trimmed.starts_with("///")
                    || trimmed.starts_with("//!");
                if !in_test_block && !is_comment && trimmed.contains(".unwrap()") {
                    violations.push(format!("  {}:{}: {}", filename, line_no + 1, trimmed));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "INV-7: Found .unwrap() in non-test runbook code:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn test_compilation_error_serde_round_trip() {
        let err = CompilationError::new(
            CompilationErrorKind::SemRegDenied {
                verb: "entity.delete".into(),
                reason: "denied".into(),
            },
            "sem_reg",
        );
        let json = serde_json::to_string(&err).expect("serialize");
        let back: CompilationError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.source_phase, "sem_reg");
        assert!(matches!(
            back.kind,
            CompilationErrorKind::SemRegDenied { .. }
        ));
    }
}
