//! T11.0 (EOP-PLAN-CONTROLPLANE-002): the C2 provenance metric — "measure
//! before retire."
//!
//! v0.4.1 §15.3, C2: "every capability entry point counts invocations by
//! provenance. The metric `capability_invocations_without_cp_provenance`
//! is on the assurance plane (§6.14) with an alert threshold of zero."
//!
//! # Pre-L2, this is instrumented, not structural
//!
//! L2's keyed doors (T11.2) don't exist yet — there is no
//! `CapabilityInvocation` context type anywhere in the codebase for a
//! capability entry point to check for. So "provenance" here cannot yet
//! mean "a CP-issued token was present" (there is no such token to be
//! present or absent). It means what the plan's own T11.0 text says it
//! means during this window: an execution-context marker set by
//! CP-mediated paths, checked on a best-effort basis. Today, nothing in
//! the codebase sets such a marker anywhere (confirmed by MCA-002: the
//! sole CP-evaluation call site, `sequencer.rs:7103`, runs async/after
//! dispatch already completed — it cannot mark a context dispatch itself
//! will later see). So every invocation recorded by this module today is
//! `has_cp_provenance = false` — the metric's number will be the full
//! count of every instrumented capability-entry invocation, not a
//! meaningfully partitioned "some have it, some don't" count. That is the
//! expected, honest state of this measurement pre-T11.2 — per T11.0's own
//! exit criteria: "The number will be large; that is the point."
//!
//! This module becomes structural, not merely instrumented, the moment
//! T11.2 lands: the keyed door itself IS the marker (holding a
//! `CapabilityInvocation` becomes the thing this module checks for,
//! replacing the always-false placeholder below).
//!
//! # Coverage scope of this first slice
//!
//! Wired into `PgTransactionScope::begin`/`begin_timeout`
//! (`src/sequencer_tx.rs`) — every scope-mediated capability invocation,
//! which covers the SemOS-native-ops and CRUD-fast-path dispatch branches
//! (`ObPocVerbExecutor::execute_verb_in_open_scope`/
//! `execute_verb_admitting_envelope`) and any other production caller of
//! `PgTransactionScope::begin`.
//!
//! **NOT covered by this slice** (known gap, not silently claimed as
//! measured): direct pool-based capability access that never opens a
//! `PgTransactionScope` at all — e.g. `ob_poc_sage::session_context`'s
//! raw `sqlx::PgPool` queries (MCA-001's AB4 finding), and the legacy
//! pool-based `execute_crud`/`check_admission`/`try_consume` variants
//! (ownership ledger, T9.2's "permanent, not debt" primitives). These are
//! real, known-uninstrumented mesh — counted honestly as zero here, not
//! folded into this metric's total, so the metric never overstates its
//! own coverage. Widening instrumentation to those call sites is
//! follow-on work for whichever T11 sub-tranche touches them (T11.2/T11.3
//! naturally reduce this gap as a side effect of keying/lensing those
//! exact call sites).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde::Serialize;

/// Per-capability-entry invocation counts, split by whether a CP
/// provenance marker was present at the time of the call.
#[derive(Debug, Clone, Copy, Default)]
struct ProvenanceCounts {
    with_provenance: u64,
    without_provenance: u64,
}

static REGISTRY: LazyLock<Mutex<HashMap<String, ProvenanceCounts>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Records one capability invocation against `capability_entry` (a stable
/// identifier for the instrumented call site, e.g.
/// `"PgTransactionScope::begin"`), tagged with whether a CP provenance
/// marker was present.
///
/// In-process only (an `AtomicU64`-backed counter, not a DB write) —
/// deliberately: this is telemetry on a hot path (`PgTransactionScope::begin`
/// runs on every transaction workspace-wide), and adding synchronous DB
/// I/O to it purely for a counter would violate the same "never blocks
/// the calling turn" discipline the shadow-evaluation mechanism already
/// established (T10.1). The assurance-plane API reads this in-process
/// state directly (`snapshot`, below) rather than querying a table.
pub(crate) fn record_capability_invocation(capability_entry: &str, has_cp_provenance: bool) {
    let mut registry = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let counts = registry.entry(capability_entry.to_string()).or_default();
    if has_cp_provenance {
        counts.with_provenance += 1;
    } else {
        counts.without_provenance += 1;
    }
}

/// One row of the C2 metric: a capability entry point's invocation counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityProvenanceCount {
    pub capability_entry: String,
    pub with_provenance: u64,
    pub without_provenance: u64,
}

/// v0.4.1 §15.3 C2: `capability_invocations_without_cp_provenance`,
/// broken down per instrumented capability entry point. Sums to the
/// mesh-remainder headline number when totalled across rows.
///
/// Process-lifetime counters — reset on restart, which is the correct
/// semantics for a live gauge/counter pair on the assurance plane (same
/// posture as any in-process Prometheus-style counter), not a historical
/// audit log (that role belongs to `control_plane_shadow_decisions` et
/// al., which this module does not duplicate).
pub(crate) fn capability_invocations_without_cp_provenance() -> Vec<CapabilityProvenanceCount> {
    let registry = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows: Vec<CapabilityProvenanceCount> = registry
        .iter()
        .map(|(entry, counts)| CapabilityProvenanceCount {
            capability_entry: entry.clone(),
            with_provenance: counts.with_provenance,
            without_provenance: counts.without_provenance,
        })
        .collect();
    rows.sort_by(|a, b| a.capability_entry.cmp(&b.capability_entry));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run serially — the registry is a shared process-global static, and
    /// parallel tests incrementing the same or different keys would race
    /// on each other's counts (same PIR-D-004 shape this session's other
    /// fixture races were). Uses a run-unique capability_entry name to
    /// stay isolated from any other test in this binary, following the
    /// established pattern (run-unique verb_fqn in the DB-backed metrics
    /// tests) rather than a lock.
    #[test]
    fn records_and_snapshots_without_provenance_counts() {
        let entry = format!("test-entry-{}", uuid::Uuid::new_v4());
        record_capability_invocation(&entry, false);
        record_capability_invocation(&entry, false);
        record_capability_invocation(&entry, true);

        let rows = capability_invocations_without_cp_provenance();
        let row = rows
            .iter()
            .find(|r| r.capability_entry == entry)
            .unwrap_or_else(|| panic!("expected a row for {entry}, got {rows:?}"));
        assert_eq!(row.without_provenance, 2);
        assert_eq!(row.with_provenance, 1);
    }

    #[test]
    fn unrecorded_entry_is_absent_not_zeroed() {
        let entry = format!("never-recorded-{}", uuid::Uuid::new_v4());
        let rows = capability_invocations_without_cp_provenance();
        assert!(
            !rows.iter().any(|r| r.capability_entry == entry),
            "an entry with zero recorded invocations must not appear as a fabricated zero row"
        );
    }
}
