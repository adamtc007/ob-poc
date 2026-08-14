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
use crate::fold::control::{
    reconciled_control_edges, reconciled_economic_edges, reconciled_trust_edges, ControlState,
    ReconciledEconomicEdge, TrustRoleKind,
};
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
        state: &ControlState,
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
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        let edges = reconciled_economic_edges(state);

        // Build adjacency: to_entity → sorted vec of (from_entity, pct, originating_event_id).
        // BTreeMap ensures deterministic iteration order (Q6, K-16/18/33).
        // The edge's assertion event id is used (never random) for K-35.
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, f64, EventId)>> = BTreeMap::new();
        for e in &edges {
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
        let mut stack: Vec<(EntityId, f64, Vec<EntityId>, Option<EventId>)> =
            vec![(subject_entity_id, 100.0, vec![subject_entity_id], None)];
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
                    let entry = candidates.entry(parent_person_id).or_insert((
                        0.0,
                        path.clone(),
                        chain_orig,
                    ));
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

// ── Shared control-chain traversal (M4/TS.1/TS.2) ────────────────────────────

/// The shared control-chain DFS used by every control-axis strategy
/// (`ControlProngStrategy`, `TrustRoleStrategy`,
/// `FoundationCouncilStrategy`; `FundControlStrategy` reaches it by
/// delegating to `ControlProngStrategy`). Extracted verbatim from the
/// previously-duplicated `ControlProngStrategy`/`TrustRoleStrategy` bodies
/// (TS.2 — behavior-preserving refactor): the caller builds the adjacency
/// (`to_entity → (from_entity, originating_event_id)`) from whichever edge
/// set its ruling admits; this walks it.
///
/// Semantics (Q6, K-16/18/33 determinism contract):
/// - adjacency lists are sorted by `(from, orig)` so traversal order is
///   deterministic regardless of insertion order;
/// - DFS from `subject_entity_id` with a path-based cycle guard;
/// - a `from` that is a natural person becomes a candidate — first
///   deterministic path wins (control is binary, not summed);
/// - a `from` that is a legal entity is traversed further;
/// - every candidate is `Prong::ControlByOtherMeans` with
///   `effective_ownership_pct: None` — control carries no quantum.
fn resolve_chain_candidates(
    mut adj: BTreeMap<EntityId, Vec<(EntityId, EventId)>>,
    subject_entity_id: EntityId,
    natural_persons: &BTreeSet<PersonId>,
) -> Vec<ProngCandidate> {
    for neighbours in adj.values_mut() {
        neighbours.sort_by_key(|&(from, orig)| (from, orig));
    }

    // DFS: no percentage to carry, just the path (for ownership_chain / K-35)
    // and the earliest originating event.
    let mut stack: Vec<(EntityId, Vec<EntityId>, Option<EventId>)> =
        vec![(subject_entity_id, vec![subject_entity_id], None)];
    let mut candidates: BTreeMap<PersonId, (Vec<EntityId>, EventId)> = BTreeMap::new();

    while let Some((entity, path, first_orig)) = stack.pop() {
        if path.iter().filter(|&&e| e == entity).count() > 1 {
            continue; // cycle guard
        }
        for &(parent, edge_orig) in adj.get(&entity).unwrap_or(&vec![]) {
            let chain_orig = first_orig.unwrap_or(edge_orig);
            let parent_person_id = PersonId(parent.0);
            if natural_persons.contains(&parent_person_id) {
                // First deterministic path wins (control is binary, not summed).
                candidates
                    .entry(parent_person_id)
                    .or_insert_with(|| (path.clone(), chain_orig));
            } else {
                let mut new_path = path.clone();
                new_path.push(parent);
                stack.push((parent, new_path, Some(chain_orig)));
            }
        }
    }

    candidates
        .into_iter()
        .map(|(pid, (chain, orig))| ProngCandidate {
            person_id: pid,
            prong: Prong::ControlByOtherMeans,
            effective_ownership_pct: None,
            ownership_chain: chain,
            originating_event_id: orig,
        })
        .collect()
}

// ── ControlProngStrategy (M4 — control by other means) ───────────────────────

/// Resolves natural persons reachable via a chain of asserted-and-reconciled
/// **control** edges — voting rights, board appointment, GP statutory
/// control, LLP designated member, trust roles, dominant influence
/// (`EdgeKind`, `fold::control`) — the "control by other means" prong
/// (`Prong::ControlByOtherMeans`), distinct from the ownership-percentage
/// prong `OwnershipProngStrategy` computes.
///
/// Structurally mirrors `OwnershipProngStrategy`'s traversal (same
/// adjacency-from-edges construction, same cycle guard, same deterministic
/// `BTreeMap` ordering — Q6/K-16/18/33) but walks `reconciled_control_edges`
/// instead of `reconciled_economic_edges`, and does not multiply a quantum:
/// control is attributed as present/absent per chain, not accumulated.
/// `effective_ownership_pct` is always `None`; `threshold_pct` is accepted
/// for `DeterminationStrategy` signature parity but unused — raw control
/// edges carry no percentage to threshold against.
///
/// **Scope (M4 v1):** traverses control-kind edges only. Does NOT cross into
/// the economic axis when a controlling intermediate entity is itself only
/// reachable via ownership rather than a further control edge — e.g. an LLP
/// designated member that is a body corporate whose own UBOs are only
/// resolvable via that entity's ownership %. That mixed control→ownership
/// chain resolution is real, further-removed v2 work; this v1 closes the "no
/// control-prong strategy exists at all" gap with real `EdgeKind`-driven
/// determination logic for the common case — a controlling chain of natural
/// persons and/or control-linked entities.
pub struct ControlProngStrategy;

impl DeterminationStrategy for ControlProngStrategy {
    fn name(&self) -> &'static str {
        "control_prong_strategy"
    }

    fn resolve(
        &self,
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        _threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        let edges = reconciled_control_edges(state);

        // Adjacency: to_entity → sorted vec of (from_entity, originating_event_id).
        // Same determinism contract as OwnershipProngStrategy (Q6, K-16/18/33).
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, EventId)>> = BTreeMap::new();
        for e in &edges {
            adj.entry(e.to).or_default().push((e.from, e.originating_event_id));
        }
        resolve_chain_candidates(adj, subject_entity_id, natural_persons)
    }
}

// ── TrustRoleStrategy (TS.1 — control of a trust follows role) ───────────────

/// Resolves natural persons controlling a Trust-classified subject by trust
/// ROLE, not shareholding — a trust has no shares, so there is no ownership
/// prong (EOP-DD-KYCUBO-KIT-TS0 §2.1, ratified 2026-08-12). Every candidate
/// is `Prong::ControlByOtherMeans`; `effective_ownership_pct` is always
/// `None`; `threshold_pct` is accepted for signature parity but unused.
///
/// Per-role rulings (RATIFIED):
/// - **Trustee** — always a candidate (legal control of trust assets).
/// - **Protector** — always a candidate (veto/replacement power over
///   trustees).
/// - **Settlor** — candidate UNLESS the edge proves irrevocability:
///   excluded only when `trust_revocable == Some(false)`; `Some(true)` and
///   `None` (absent/unproven) both INCLUDE the settlor — fail-closed toward
///   inclusion (over-identify, never silently drop a controller).
/// - **Beneficiary** — NEVER a candidate: a quantified beneficiary interest
///   belongs on the economic axis (`assert-economic-interest` → ownership
///   prong); a discretionary beneficiary has neither control nor a quantum.
///
/// Traverses ONLY `EdgeKind::TrustRole(_)` edges (via
/// `reconciled_trust_edges`); non-trust control kinds on a Trust-classified
/// subject are ignored by this strategy — deliberate: if the structure
/// genuinely mixes, classification is wrong, and reclassification is legal
/// (matrix row 10).
///
/// **Scope (TS.1 v1):** chain semantics mirror `ControlProngStrategy` — where
/// a trust edge's `from` is a legal entity rather than a natural person, the
/// traversal continues through that entity's own qualifying trust edges only.
/// It does NOT cross into the economic axis (or the general control axis) to
/// resolve an intermediate corporate trustee's own UBOs — the same
/// further-removed v2 boundary `ControlProngStrategy` documents.
pub struct TrustRoleStrategy;

impl TrustRoleStrategy {
    /// The ratified per-role admissibility rule (see type doc for polarity).
    fn edge_qualifies(role: &TrustRoleKind, trust_revocable: Option<bool>) -> bool {
        match role {
            TrustRoleKind::Trustee | TrustRoleKind::Protector => true,
            TrustRoleKind::Settlor => trust_revocable != Some(false),
            TrustRoleKind::Beneficiary => false,
        }
    }
}

impl DeterminationStrategy for TrustRoleStrategy {
    fn name(&self) -> &'static str {
        "trust_role_strategy"
    }

    fn resolve(
        &self,
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        _threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        // Adjacency over QUALIFYING trust edges only — the per-role
        // exclusions apply at the edge, so an excluded edge (beneficiary,
        // proven-irrevocable settlor) is never traversed at all.
        // Same determinism contract as ControlProngStrategy (Q6, K-16/18/33).
        let edges = reconciled_trust_edges(state);
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, EventId)>> = BTreeMap::new();
        for e in &edges {
            if Self::edge_qualifies(&e.role, e.trust_revocable) {
                adj.entry(e.to).or_default().push((e.from, e.originating_event_id));
            }
        }
        resolve_chain_candidates(adj, subject_entity_id, natural_persons)
    }
}

// ── FundControlStrategy (TS.2 — fund control sits with the manager) ──────────

/// Resolves natural persons controlling an InvestmentFund-classified subject
/// — the corporate-form fund (SICAV/OEIC/unit trust) whose control edge is
/// the MANAGEMENT relationship (ManCo/AIFM/GP-analog), not its investors
/// (EOP-DD-KYCUBO-KIT-TS0 §2.2, ratified 2026-08-12).
///
/// **No new `EdgeKind`:** the management relationship asserts as
/// `dominant_influence` (or `board_appointment` where literal). The strategy
/// is a thin delegate to `ControlProngStrategy`'s traversal with fund
/// framing: same `reconciled_control_edges` walk, same natural-person chain
/// resolution. Investor `economic_interest` edges are NOT traversed — they
/// stay on the economic axis and feed the existing ownership prong only when
/// someone genuinely crosses the threshold. Every candidate is
/// `Prong::ControlByOtherMeans`; `effective_ownership_pct` is always `None`;
/// `threshold_pct` is accepted for signature parity but unused.
///
/// **Why a NAMED strategy rather than mapping InvestmentFund →
/// `control_prong_strategy` directly** (the §2.2 alternative, rejected at
/// ratification): auditability — the freeze pin records WHICH model ran.
/// `"fund_control_strategy"` on the frozen determination says "this subject
/// was resolved under the fund-control basis (manager, not investors)", not
/// merely "some control walk happened".
///
/// **Scope (TS.2 v1):** inherits `ControlProngStrategy`'s v1 boundary — does
/// not cross into the economic axis for an intermediate controlling entity's
/// own UBOs (v2).
pub struct FundControlStrategy;

impl DeterminationStrategy for FundControlStrategy {
    fn name(&self) -> &'static str {
        "fund_control_strategy"
    }

    fn resolve(
        &self,
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        // Thin delegate (§2.2): same traversal machinery, fund framing.
        ControlProngStrategy.resolve(state, subject_entity_id, natural_persons, threshold_pct)
    }
}

// ── FoundationCouncilStrategy (TS.2 — control sits with the council) ─────────

/// Resolves natural persons controlling a Foundation-classified subject —
/// a foundation has NO owners by construction, so control sits with the
/// council/board (EOP-DD-KYCUBO-KIT-TS0 §2.3, ratified 2026-08-12).
///
/// Traverses ONLY active reconciled `EdgeKind::BoardAppointment` +
/// `EdgeKind::DominantInfluence` edges — narrower than the full control
/// walk. A stray `voting_rights` (or any other control-kind) edge on a
/// Foundation subject is IGNORED by this strategy — deliberate, mirroring
/// `TrustRoleStrategy`'s kind-filtering stance: if the structure genuinely
/// mixes, classification is wrong, and reclassification is legal (matrix
/// row 10). Every candidate is `Prong::ControlByOtherMeans`;
/// `effective_ownership_pct` is always `None`; `threshold_pct` is accepted
/// for signature parity but unused.
///
/// **Scope (TS.2 v1):** same natural-person chain resolution and the same
/// v1 boundary as the other control-axis strategies — an intermediate legal
/// entity is traversed through its own qualifying (council-kind) edges
/// only; no crossing into the economic axis (v2).
pub struct FoundationCouncilStrategy;

impl DeterminationStrategy for FoundationCouncilStrategy {
    fn name(&self) -> &'static str {
        "foundation_council_strategy"
    }

    fn resolve(
        &self,
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        _threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        use crate::fold::control::EdgeKind;

        // Council-kind edges only (§2.3): board_appointment + dominant_influence.
        let edges = reconciled_control_edges(state);
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, EventId)>> = BTreeMap::new();
        for e in &edges {
            if matches!(e.kind, EdgeKind::BoardAppointment | EdgeKind::DominantInfluence) {
                adj.entry(e.to).or_default().push((e.from, e.originating_event_id));
            }
        }
        resolve_chain_candidates(adj, subject_entity_id, natural_persons)
    }
}

// ── StateOwnedStrategy (TS.3 — the terminal answer is usually SMO) ──────────

/// Resolves natural persons controlling a StateOwned-classified subject —
/// the controller is a state organ, not a natural person, so the terminal
/// answer is usually **SMO** (senior managing official)
/// (EOP-DD-KYCUBO-KIT-TS0 §2.4, ratified 2026-08-12).
///
/// The strategy runs the full control-chain traversal (same edge-kind
/// admission as `ControlProngStrategy` — voting rights, board appointment,
/// GP statutory, LLP designated member, trust roles, dominant influence)
/// for the RARE genuine natural-person controller. When no candidate
/// crosses, the result is an empty Vec and the EXISTING
/// `apply-smo-fallback` path (already stage-gated by `ReconciledProjection`
/// + `StrategySelected`, matrix row 7) supplies the determination — that
/// is, `state_owned_strategy` legitimizes the SMO route for this class
/// rather than inventing a new resolution model; NO new SMO machinery.
/// Candidates it does emit are `Prong::ControlByOtherMeans`;
/// `effective_ownership_pct` is always `None`; `threshold_pct` is accepted
/// for signature parity but unused. BODS `UboType::StateOwned`
/// (`dsl-runtime/src/bods/types.rs`) is reference vocabulary for projection
/// rendering, not a dependency.
///
/// **Scope (TS.3 v1):** inherits `ControlProngStrategy`'s v1 boundary — no
/// crossing into the economic axis for an intermediate controlling entity's
/// own UBOs (v2).
pub struct StateOwnedStrategy;

impl DeterminationStrategy for StateOwnedStrategy {
    fn name(&self) -> &'static str {
        "state_owned_strategy"
    }

    fn resolve(
        &self,
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        _threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        // Full control-edge admission (§2.4: same as control-prong), walked
        // by the shared DFS. Same determinism contract (Q6, K-16/18/33).
        let edges = reconciled_control_edges(state);
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, EventId)>> = BTreeMap::new();
        for e in &edges {
            adj.entry(e.to).or_default().push((e.from, e.originating_event_id));
        }
        resolve_chain_candidates(adj, subject_entity_id, natural_persons)
    }
}

// ── CooperativeMemberStrategy (TS.3 — control from office, never membership) ─

/// Resolves natural persons controlling a Cooperative-classified subject —
/// one-member-one-vote: by construction no member holds ≥25% of votes
/// through membership alone, so membership per se NEVER yields a UBO;
/// control arises only from **office** (board/management edges) or an
/// anomalous concentrated voting arrangement (EOP-DD-KYCUBO-KIT-TS0 §2.5,
/// ratified 2026-08-12).
///
/// Traverses ONLY active reconciled `EdgeKind::VotingRights` +
/// `EdgeKind::BoardAppointment` + `EdgeKind::DominantInfluence` edges —
/// kind-filtered like `FoundationCouncilStrategy` (its 2-kind filter plus
/// voting_rights for the anomalous-concentration case). Any other
/// control-kind edge on a Cooperative subject is IGNORED by this strategy —
/// deliberate, mirroring the TrustRole/FoundationCouncil stance: if the
/// structure genuinely mixes, classification is wrong, and reclassification
/// is legal (matrix row 10). Expected COMMON outcome is zero candidates →
/// SMO fallback, same route as `StateOwnedStrategy` (§2.4). Every candidate
/// is `Prong::ControlByOtherMeans`; `effective_ownership_pct` is always
/// `None`; `threshold_pct` is accepted for signature parity but unused.
///
/// **Scope (TS.3 v1):** same natural-person chain resolution and v1
/// boundary as the other control-axis strategies — an intermediate legal
/// entity is traversed through its own qualifying (admitted-kind) edges
/// only; no crossing into the economic axis (v2).
pub struct CooperativeMemberStrategy;

impl DeterminationStrategy for CooperativeMemberStrategy {
    fn name(&self) -> &'static str {
        "cooperative_member_strategy"
    }

    fn resolve(
        &self,
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        _threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        use crate::fold::control::EdgeKind;

        // Admitted kinds only (§2.5): voting_rights + board_appointment +
        // dominant_influence.
        let edges = reconciled_control_edges(state);
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, EventId)>> = BTreeMap::new();
        for e in &edges {
            if matches!(
                e.kind,
                EdgeKind::VotingRights | EdgeKind::BoardAppointment | EdgeKind::DominantInfluence
            ) {
                adj.entry(e.to).or_default().push((e.from, e.originating_event_id));
            }
        }
        resolve_chain_candidates(adj, subject_entity_id, natural_persons)
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

    // Find the subject entity from the first classify event.
    let subject_entity = find_subject_entity(events)?;

    let candidates = strategy.resolve(&control, subject_entity, natural_persons, threshold_pct);

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
    let freeze_event = events
        .iter()
        .rev()
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
    )
    .ok()
}

/// Replay the determination **bitemporally** (T5, EOP-SA-OBP-001 §I.5): the
/// constructor is parameterised by *both* time axes explicitly — `valid_at`
/// ("what was true as of this business time") and `known_at` ("what we knew,
/// given everything recorded by this transaction time") — rather than a single
/// axis-selector switching between the two framings §I.5 describes. Passing
/// both timestamps is a strict generalisation of a single-axis parameter: it
/// answers either question (`known_at = +inf` for a pure valid-time query,
/// `valid_at = +inf` for a pure knowledge-time query — same as
/// `recover_determination_at`) and the general bitemporal question the two
/// aligned. **Design judgment call** (flagged per plan instruction; the plan's
/// literal signature `recover_determination_bitemporal(subject, valid_at,
/// known_at)` names no third `axis` argument): this crate's pure `IntentEvent`
/// carries `as_of` (valid-time) and `committed_at` (knowledge-time, additively
/// added in this tranche — see `event.rs`), so both axes are simple per-event
/// predicates; no separate axis-selector type was introduced.
///
/// Filters `events` to the bitemporal slice — `as_of <= valid_at AND
/// committed_at <= known_at` — then delegates to `recover_determination_at`
/// **unchanged** over the filtered slice, reusing the SAME fold (no duplicated
/// determination logic). `recover_determination_bitemporal(events, .., now,
/// now)` is therefore identical to `recover_determination_at(events, ..)` when
/// every event's `as_of`/`committed_at` predate `now` — the transaction-time-
/// only case both axes collapse to.
#[allow(clippy::too_many_arguments)]
pub fn recover_determination_bitemporal(
    events: &[&IntentEvent],
    strategy: &dyn DeterminationStrategy,
    natural_persons: &BTreeSet<PersonId>,
    threshold_pct: f64,
    pin: RecoveryPin<'_>,
    valid_at: DateTime<Utc>,
    known_at: DateTime<Utc>,
) -> Option<FrozenDetermination> {
    let filtered: Vec<&IntentEvent> = events
        .iter()
        .copied()
        .filter(|e| e.as_of <= valid_at && e.committed_at <= known_at)
        .collect();

    recover_determination_at(&filtered, strategy, natural_persons, threshold_pct, pin)
}

/// Find the subject's own `EntityId`, recorded on `kyc.subject.classify-structure`
/// (payload field `entity_id`). Shared by `recover_determination_at` (replay) and
/// the live `ubo.determination.freeze` verb (EOP-DD-KYCUBO-003 remediation) so
/// both paths resolve the subject entity identically.
pub fn find_subject_entity(events: &[&IntentEvent]) -> Option<EntityId> {
    events
        .iter()
        .find(|e| e.verb_fqn.as_str() == "kyc.subject.classify-structure")
        .and_then(|e| {
            e.payload
                .get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .map(EntityId)
        })
}
