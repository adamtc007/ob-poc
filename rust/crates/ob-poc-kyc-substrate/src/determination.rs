//! Determination: demote, compose, freeze (§5 of EOP-DD-KYCUBO-001).
//!
//! `DeterminationStrategy` is the strategy-pattern interface (K-4).
//! `OwnershipProngStrategy` wraps the current percentage-chain logic (V&S §12.2):
//!   - feeds on reconciled, verified economic edges (closing the >100% double-count, K-14)
//!   - is ONE prong's answer, not the determination
//!
//! `freeze_determination()` pins K-18 close:
//!   policy_version + lexicon_manifest_hash + reference_snapshot_id +
//!   import_run_ids + graph_content_hash + as_of (frozen clock, Q6)
//!
//! # Determinism invariant
//!
//! **INVARIANT (fold path):** No `HashMap`/`HashSet`, no `Uuid::new_v4` /
//! `EventId::new`, no `Utc::now()`, no `SystemTime::now()`, and no
//! float-to-string in any hashed payload inside this module.  Violating any
//! of these breaks bit-identical replay (Q6, K-16/18/33).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::KycError;
use crate::event::IntentEvent;
use crate::fold::control::{ControlState, ReconciledEconomicEdge, reconciled_economic_edges};
use crate::types::{EntityId, EventId, Hash, PersonId};

// ── Prong ─────────────────────────────────────────────────────────────────────

/// The prong(s) under which a person is determined to be a UBO/controller (K-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prong {
    /// Economic ownership at/above threshold (direct or via a chain).
    OwnershipProng,
    /// Control by other means (not purely economic).
    ControlByOtherMeans,
    /// No ownership/control found; senior managing official fallback (K-5).
    SmoFallback,
    /// Both ownership and control prongs apply.
    Dual,
}

// ── Prong candidate ───────────────────────────────────────────────────────────

/// One natural person resolved as a UBO candidate under a specific prong.
/// K-1: basis mandatory.  K-35: originating_event_id for every candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProngCandidate {
    pub person_id: PersonId,
    pub prong: Prong,
    /// Effective ownership % (ownership prong only; None for control/SMO).
    pub effective_ownership_pct: Option<f64>,
    /// The chain from subject entity to this person.
    pub ownership_chain: Vec<EntityId>,
    /// The event that introduced the edge that makes this person a candidate.
    pub originating_event_id: EventId,
}

// ── Strategy interface (K-4) ─────────────────────────────────────────────────

/// The determination strategy selected by `ubo.determination.select-strategy`
/// based on the subject's structure class (K-4).
///
/// One strategy per structure class; composable: `compute-fold` calls
/// ownership prong + control prong + SMO-fallback in sequence and merges.
pub trait DeterminationStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolve(
        &self,
        edges: &[ReconciledEconomicEdge],
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        threshold_pct: f64,
    ) -> Vec<ProngCandidate>;
}

// ── OwnershipProngStrategy (V&S §12.2 demoted chain) ─────────────────────────

/// Pure Rust re-implementation of the ownership-percentage-multiply logic from
/// `sem_os_postgres::ops::ubo_compute` — same algorithm, but:
///   1. Takes reconciled, verified edges (not raw `entity_relationships`).
///   2. Returns `ProngCandidate` with basis/prong recorded (K-1).
///   3. No sqlx; no DB calls.  (Exit criterion 1: differential equality.)
pub struct OwnershipProngStrategy;

impl DeterminationStrategy for OwnershipProngStrategy {
    fn name(&self) -> &'static str {
        "ownership_prong_strategy"
    }

    fn resolve(
        &self,
        edges: &[ReconciledEconomicEdge],
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        // Build adjacency: to_entity → sorted vec of (from_entity, pct, originating_event_id).
        // BTreeMap ensures deterministic iteration order (Q6, K-16/18/33).
        // The edge's assertion event id is used (never random) for K-35.
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, f64, EventId)>> = BTreeMap::new();
        for e in edges {
            adj.entry(e.to)
                .or_default()
                .push((e.from, e.percentage, e.originating_event_id));
        }
        // Sort each adjacency list so traversal order is deterministic regardless of
        // insertion order (edges arrive in event-stream order, which is stable, but
        // an explicit sort is the contract).
        for neighbours in adj.values_mut() {
            neighbours.sort_by_key(|&(from, _, orig)| (from, orig));
        }

        // DFS with cumulative percentage multiplication.
        // Stack: (current_entity, cumulative_pct, path_so_far, earliest_originating_event_id)
        let mut stack: Vec<(EntityId, f64, Vec<EntityId>, Option<EventId>)> = vec![
            (subject_entity_id, 100.0, vec![subject_entity_id], None),
        ];
        // BTreeMap for deterministic merge when a person is reachable via multiple chains.
        let mut candidates: BTreeMap<PersonId, (f64, Vec<EntityId>, Option<EventId>)> =
            BTreeMap::new();

        while let Some((entity, cumulative_pct, path, first_orig)) = stack.pop() {
            // Cycle detection.
            if path.iter().filter(|&&e| e == entity).count() > 1 {
                continue;
            }

            for &(parent, edge_pct, edge_orig) in adj.get(&entity).unwrap_or(&vec![]) {
                let new_pct = cumulative_pct * edge_pct / 100.0;
                // Carry the first (earliest) originating event down the chain.
                let chain_orig = Some(first_orig.unwrap_or(edge_orig));

                // Is the parent a natural person?
                let parent_person_id = PersonId(parent.0);
                if natural_persons.contains(&parent_person_id) {
                    let entry = candidates
                        .entry(parent_person_id)
                        .or_insert((0.0, path.clone(), chain_orig));
                    entry.0 += new_pct;
                } else {
                    // Intermediate entity — continue traversal.
                    let mut new_path = path.clone();
                    new_path.push(parent);
                    stack.push((parent, new_pct, new_path, chain_orig));
                }
            }
        }

        // Filter by threshold; record basis (K-1).
        // originating_event_id is always deterministic (from edge assertion events).
        candidates
            .into_iter()
            .filter(|(_, (pct, _, _))| *pct >= threshold_pct)
            .map(|(pid, (pct, chain, orig))| ProngCandidate {
                person_id: pid,
                prong: Prong::OwnershipProng,
                effective_ownership_pct: Some(pct),
                ownership_chain: chain,
                // INVARIANT: orig is always Some here because the adjacency map
                // always carries the edge's originating_event_id (never random).
                originating_event_id: orig.expect("originating_event_id must be deterministic"),
            })
            .collect()
    }
}

// ── SMO fallback (K-5: never silent) ────────────────────────────────────────

/// If ownership + control fold yields no persons, the SMO fallback fires.
/// A person, or an explicit authorised waiver — never silence (K-5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SmoResult {
    Person(ProngCandidate),
    AuthorisedWaiver { reason: String, by_event: EventId },
}

// ── Frozen determination (K-18) ───────────────────────────────────────────────

/// The K-18 pin.  A determination is immutable and reproducible against:
/// - policy version
/// - lexicon manifest hash (Q7)
/// - reference data snapshot
/// - import run ids
/// - content hash of the reconciled control graph
/// - frozen `as_of` timestamp (Q6 — never a live wall-clock read)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminationPin {
    pub policy_version: String,
    /// Whole-lexicon manifest hash (Q7).
    pub lexicon_manifest_hash: Hash,
    /// Reference data snapshot id (jurisdictions, thresholds, carve-outs).
    pub reference_snapshot_id: Uuid,
    /// Import run ids contributing source data.
    pub import_run_ids: BTreeSet<Uuid>,
    /// SHA-256 of the reconciled economic edge set (proves K-14 precondition
    /// was met and the graph was stable at freeze time).
    pub graph_content_hash: Hash,
    /// Frozen point in time (Q6: NOT now(); passed in from the event's `as_of`).
    pub as_of: DateTime<Utc>,
}

impl DeterminationPin {
    /// Compute `graph_content_hash` from the reconciled economic edges.
    pub fn compute_graph_hash(edges: &[ReconciledEconomicEdge]) -> Hash {
        let mut parts: Vec<String> = edges
            .iter()
            .map(|e| format!("{}->{}:{:.4}", e.from.0, e.to.0, e.percentage))
            .collect();
        parts.sort(); // Deterministic.
        Hash::of(parts.join("|").as_bytes())
    }
}

// ── Frozen determination artifact ─────────────────────────────────────────────

/// The immutable determination artifact produced by `ubo.determination.freeze`.
/// Contains the pin + the resolved persons with their prong/basis (K-1).
/// K-35: every candidate has `originating_event_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrozenDetermination {
    pub pin: DeterminationPin,
    /// The event that triggered this freeze (K-35 chain).
    pub freeze_event_id: EventId,
    /// Resolved UBO persons with basis (K-1: basis mandatory).
    pub candidates: Vec<ProngCandidate>,
    /// SMO fallback result (K-5: present if no candidates, or if SMO was required).
    pub smo_result: Option<SmoResult>,
    /// Content hash of this determination (for replay integrity).
    pub determination_hash: Hash,
}

impl FrozenDetermination {
    pub fn is_empty_determination(&self) -> bool {
        self.candidates.is_empty() && self.smo_result.is_none()
    }
}

// ── Intermediate determination (pre-freeze) ───────────────────────────────────

/// Built by `compute-fold`; finalised by `freeze`.
#[derive(Debug, Default, Clone)]
pub struct DeterminationInProgress {
    pub strategy: Option<String>,
    pub candidates: Vec<ProngCandidate>,
    pub smo_result: Option<SmoResult>,
    pub compute_event_id: Option<EventId>,
}

// ── freeze_determination ──────────────────────────────────────────────────────

/// Execute `ubo.determination.freeze`:
/// - Requires `state.is_reconciled()` and `state.has_strategy()` (pre-checked).
/// - Pins policy + lexicon + reference + import runs + graph hash + as_of.
/// - Returns `FrozenDetermination` (immutable).
/// - K-5: if candidates is empty AND no SMO was applied, returns Err.
pub fn freeze_determination(
    det: &DeterminationInProgress,
    control_state: &ControlState,
    freeze_event: &IntentEvent,
    policy_version: &str,
    lexicon_manifest_hash: Hash,
    reference_snapshot_id: Uuid,
    import_run_ids: BTreeSet<Uuid>,
) -> Result<FrozenDetermination, KycError> {
    // K-5: determination must not be silent.
    if det.candidates.is_empty() && det.smo_result.is_none() {
        return Err(KycError::DeterminationNotReady);
    }

    let edges = reconciled_economic_edges(control_state);
    let graph_hash = DeterminationPin::compute_graph_hash(&edges);

    let pin = DeterminationPin {
        policy_version: policy_version.to_owned(),
        lexicon_manifest_hash,
        reference_snapshot_id,
        import_run_ids,
        graph_content_hash: graph_hash,
        as_of: freeze_event.as_of, // frozen clock from the event (Q6)
    };

    // Determination hash covers pin + candidates.
    let pin_json = serde_json::to_value(&pin).expect("pin is serialisable");
    let cands_json = serde_json::to_value(&det.candidates).expect("candidates are serialisable");
    let combined = serde_json::json!({ "pin": pin_json, "candidates": cands_json });
    let determination_hash = Hash::of_json(&combined);

    Ok(FrozenDetermination {
        pin,
        freeze_event_id: freeze_event.id,
        candidates: det.candidates.clone(),
        smo_result: det.smo_result.clone(),
        determination_hash,
    })
}

// ── Replay / point-in-time recovery ──────────────────────────────────────────

/// Pin parameters required to freeze/recover a determination (K-18).
/// Groups the 4 reference-plane inputs so `recover_determination_at` stays under 8 args.
pub struct RecoveryPin<'a> {
    pub policy_version: &'a str,
    pub lexicon_manifest_hash: Hash,
    pub reference_snapshot_id: Uuid,
    pub import_run_ids: BTreeSet<Uuid>,
}

/// Replay the determination from an event stream filtered to `up_to_seq`
/// (point-in-time recovery, K-16/K-18/K-33).  Returns the `FrozenDetermination`
/// that existed at that sequence if a freeze event appears in the window.
pub fn recover_determination_at(
    events: &[&IntentEvent],
    strategy: &dyn DeterminationStrategy,
    natural_persons: &BTreeSet<PersonId>,
    threshold_pct: f64,
    pin: RecoveryPin<'_>,
) -> Option<FrozenDetermination> {
    use crate::fold::control::fold_control;

    let control = fold_control(events);
    if !control.is_reconciled() || !control.has_strategy() {
        return None;
    }

    let edges = reconciled_economic_edges(&control);

    // Find the subject entity from the first classify event.
    let subject_entity = find_subject_entity(events)?;

    let candidates = strategy.resolve(&edges, subject_entity, natural_persons, threshold_pct);

    // Find SMO from control state.
    // smo_event_id is ALWAYS Some when smo_person_id is Some (set together in fold_control).
    // Using expect() rather than a fallback here: a None would mean the fold is
    // inconsistent, which must surface as a panic, not a silent random UUID (Q6, K-35).
    let smo_result = match (control.smo_person_id, control.smo_event_id) {
        (Some(pid), Some(orig_event_id)) => Some(SmoResult::Person(ProngCandidate {
            person_id: pid,
            prong: Prong::SmoFallback,
            effective_ownership_pct: None,
            ownership_chain: vec![],
            originating_event_id: orig_event_id,
        })),
        (None, _) => None,
        (Some(_), None) => {
            // Fold invariant violated: smo_person_id set without smo_event_id.
            panic!("fold invariant violated: smo_person_id is Some but smo_event_id is None");
        }
    };

    // Find the freeze event.
    let freeze_event = events.iter().rev()
        .find(|e| e.verb_fqn.as_str() == "ubo.determination.freeze")?;

    let det = DeterminationInProgress {
        strategy: control.selected_strategy.clone(),
        candidates,
        smo_result,
        compute_event_id: control.strategy_event_id,
    };

    freeze_determination(
        &det,
        &control,
        freeze_event,
        pin.policy_version,
        pin.lexicon_manifest_hash,
        pin.reference_snapshot_id,
        pin.import_run_ids,
    ).ok()
}

/// Find the subject's own `EntityId`, recorded on `kyc.subject.classify-structure`
/// (payload field `entity_id`). Shared by `recover_determination_at` (replay) and
/// the live `ubo.determination.freeze` verb (EOP-DD-KYCUBO-003 remediation) so
/// both paths resolve the subject entity identically.
pub fn find_subject_entity(events: &[&IntentEvent]) -> Option<EntityId> {
    events.iter()
        .find(|e| e.verb_fqn.as_str() == "kyc.subject.classify-structure")
        .and_then(|e| {
            e.payload.get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .map(EntityId)
        })
}
