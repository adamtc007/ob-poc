//! Round-trip harness — Phase 0e skeleton.
//!
//! Implements the v0.3 §14 **effect-equivalence** contract:
//!
//! > A CRUD op is a candidate for Phase 6 dissolution only if it passes
//! > round-trip. Round-trip is effect-equivalence, not SQL-equivalence.
//!
//! For each `(args, pre-state fixture)` input, compare two runs:
//!
//! 1. Current Rust op impl against a fresh fixture DB.
//! 2. `PgCrudExecutor` interpreting the proposed YAML against an
//!    equivalent fresh fixture DB.
//!
//! Capture and compare **effect-identity**, not SQL text:
//!
//! - post-state row diff (byte-identical)
//! - returned values (structurally equal)
//! - side-effect summary (sequences, triggers, audit rows)
//! - `PendingStateAdvance` (byte-identical)
//! - `OutboxDraft` vector (set-equal on `idempotency_key`)
//!
//! # Phase 0e scope
//!
//! Scaffold: API shape + first-pass comparison logic. Fixture DB spin-up
//! and SQL diffing land in Phase 5e / Phase 6 (pre-dissolution tag).

use ob_poc_types::{
    IdempotencyKey, OutboxDraft, OutboxEffectKind, PendingStateAdvance, SideEffectSummary,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A row from one of the fixture tables, captured after op execution.
/// Keyed by `(table_name, primary_key_as_string)`. Columns in
/// lexicographic order so the diff is byte-deterministic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapturedRow {
    pub table: String,
    pub primary_key: String,
    pub columns: BTreeMap<String, serde_json::Value>,
}

/// A snapshot of all fixture-DB rows the op could have touched, after
/// execution. Sorted by `(table, primary_key)` for deterministic diff.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PostStateRows(pub Vec<CapturedRow>);

impl PostStateRows {
    /// Sort rows in canonical order. Callers MUST invoke before comparing.
    pub fn canonicalise(&mut self) {
        self.0.sort_by(|a, b| {
            (a.table.as_str(), a.primary_key.as_str())
                .cmp(&(b.table.as_str(), b.primary_key.as_str()))
        });
    }
}

/// The full observation from a single op execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoundTripObservation {
    pub label: String, // e.g. "rust-impl" or "metadata-crud"
    pub post_state: PostStateRows,
    pub returned_value: serde_json::Value,
    pub side_effects: SideEffectSummary,
    pub pending_state_advance: PendingStateAdvance,
    pub outbox_drafts: Vec<OutboxDraft>,
}

/// Comparison result between two observations over the same fixture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RoundTripOutcome {
    /// All five comparison axes identical. Op dissolves to metadata.
    Pass,
    /// At least one axis differs. Op reclassified to plugin for Phase 6.
    Fail { diffs: Vec<RoundTripDiff> },
}

/// Which comparison axis differed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "axis", rename_all = "snake_case")]
pub enum RoundTripDiff {
    /// Post-state rows differ. Caller gets row-level detail downstream.
    PostState { row_diff_count: usize },
    /// Returned values differ.
    ReturnedValue,
    /// Side-effect summary differs (e.g. different number of audit rows).
    SideEffects,
    /// PendingStateAdvance differs.
    PendingStateAdvance,
    /// Outbox draft set differs on idempotency_key.
    OutboxDraftSet,
}

/// Effect-equivalence comparator. Compares two observations captured from
/// the same fixture (Rust impl vs metadata-CRUD interpretation).
pub fn compare(lhs: &RoundTripObservation, rhs: &RoundTripObservation) -> RoundTripOutcome {
    let mut diffs = Vec::new();

    // --- post-state rows ---
    if lhs.post_state != rhs.post_state {
        let row_diff_count = lhs
            .post_state
            .0
            .iter()
            .zip(rhs.post_state.0.iter())
            .filter(|(l, r)| l != r)
            .count()
            + lhs.post_state.0.len().abs_diff(rhs.post_state.0.len());
        diffs.push(RoundTripDiff::PostState { row_diff_count });
    }

    // --- returned value ---
    if lhs.returned_value != rhs.returned_value {
        diffs.push(RoundTripDiff::ReturnedValue);
    }

    // --- side-effect summary ---
    if lhs.side_effects != rhs.side_effects {
        diffs.push(RoundTripDiff::SideEffects);
    }

    // --- pending state advance ---
    if lhs.pending_state_advance != rhs.pending_state_advance {
        diffs.push(RoundTripDiff::PendingStateAdvance);
    }

    // --- outbox set-equality on idempotency_key ---
    if !outbox_set_equal(&lhs.outbox_drafts, &rhs.outbox_drafts) {
        diffs.push(RoundTripDiff::OutboxDraftSet);
    }

    if diffs.is_empty() {
        RoundTripOutcome::Pass
    } else {
        RoundTripOutcome::Fail { diffs }
    }
}

/// Outbox drafts compare as sets keyed by idempotency_key + effect_kind.
/// Per v0.3 §14 "OutboxDraft vector — set-equal on `idempotency_key`".
fn outbox_set_equal(lhs: &[OutboxDraft], rhs: &[OutboxDraft]) -> bool {
    let mut lhs_keys: Vec<(&IdempotencyKey, OutboxEffectKind)> = lhs
        .iter()
        .map(|d| (&d.idempotency_key, d.effect_kind))
        .collect();
    let mut rhs_keys: Vec<(&IdempotencyKey, OutboxEffectKind)> = rhs
        .iter()
        .map(|d| (&d.idempotency_key, d.effect_kind))
        .collect();
    lhs_keys.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));
    rhs_keys.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));
    lhs_keys == rhs_keys
}

// ---------------------------------------------------------------------------
// Tests — meta-tests verifying comparator logic.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_types::TraceId;
    use uuid::Uuid;

    fn sample_observation(label: &str) -> RoundTripObservation {
        RoundTripObservation {
            label: label.into(),
            post_state: PostStateRows::default(),
            returned_value: serde_json::json!({}),
            side_effects: SideEffectSummary::default(),
            pending_state_advance: PendingStateAdvance::default(),
            outbox_drafts: vec![],
        }
    }

    #[test]
    fn identical_observations_pass() {
        let a = sample_observation("rust");
        let b = sample_observation("metadata");
        // Labels differ but only the five effect axes are compared.
        assert!(matches!(compare(&a, &b), RoundTripOutcome::Pass));
    }

    #[test]
    fn returned_value_mismatch_fails() {
        let mut a = sample_observation("rust");
        let mut b = sample_observation("metadata");
        a.returned_value = serde_json::json!({"id": 1});
        b.returned_value = serde_json::json!({"id": 2});
        let out = compare(&a, &b);
        match out {
            RoundTripOutcome::Fail { diffs } => {
                assert!(diffs
                    .iter()
                    .any(|d| matches!(d, RoundTripDiff::ReturnedValue)));
            }
            _ => panic!("expected Fail"),
        }
    }

    #[test]
    fn outbox_set_equality_ignores_order() {
        let key_a = IdempotencyKey::from_parts("narrate", TraceId(Uuid::from_u128(1)), "s");
        let key_b = IdempotencyKey::from_parts("narrate", TraceId(Uuid::from_u128(2)), "s");
        let draft = |k: IdempotencyKey| OutboxDraft {
            effect_kind: OutboxEffectKind::Narrate,
            payload: serde_json::json!(null),
            idempotency_key: k,
        };
        let lhs = vec![draft(key_a.clone()), draft(key_b.clone())];
        let rhs = vec![draft(key_b), draft(key_a)];
        assert!(outbox_set_equal(&lhs, &rhs));
    }

    #[test]
    fn post_state_canonicalise_orders_by_table_then_pk() {
        let mut rs = PostStateRows(vec![
            CapturedRow {
                table: "cbus".into(),
                primary_key: "b".into(),
                columns: Default::default(),
            },
            CapturedRow {
                table: "cbus".into(),
                primary_key: "a".into(),
                columns: Default::default(),
            },
            CapturedRow {
                table: "entities".into(),
                primary_key: "x".into(),
                columns: Default::default(),
            },
        ]);
        rs.canonicalise();
        assert_eq!(rs.0[0].primary_key, "a");
        assert_eq!(rs.0[1].primary_key, "b");
        assert_eq!(rs.0[2].table, "entities");
    }
}
