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
use crate::types::{EntityId, EventId, Hash, PersonId, Principal};

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

// ── Declared-delegation register (EOP-DD-KYCUBO-TS.4 §4) ────────────────────

/// TS.4 §4: a strategy that names a structure class and forwards verbatim
/// to another strategy is "indistinguishable from a correct implementation
/// at every level the tests currently inspect" — the fix is to make the
/// claim explicit, not to ban delegation. A DECLARED delegate is
/// legitimate (`nominee_pierce_strategy`, Q1: keep the name as a framing
/// over the shared walk once its mechanism moves elsewhere); an
/// UNDECLARED one (`fund_control_strategy`, before this tranche) is the
/// "claimed-but-delegated" defect this register exists to make visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegationStatus {
    /// Distinct logic — even if structurally similar to another strategy
    /// (e.g. `state_owned_strategy` inlines the same walk as
    /// `control_prong_strategy` but is not a call-through), it is not
    /// *forwarding* to another named strategy.
    GenuineImplementation,
    /// Forwards verbatim to the named strategy — declared, not discovered
    /// by reading source.
    DeclaredDelegate { delegates_to: &'static str },
}

/// TS.4 §4's pin — one entry per `DeterminationStrategy` impl, by
/// `name()`. `strategy_delegation_is_exactly_known`
/// (`tests/kyc_ts4_fund_pivot_piercing.rs`) verifies this behaviorally: a
/// `DeclaredDelegate` must be byte-identical to its named target on shared
/// fixtures; a `GenuineImplementation` must diverge from a raw
/// `control_prong_strategy` walk somewhere a real domain difference exists.
pub const STRATEGY_DELEGATION_REGISTRY: &[(&str, DelegationStatus)] = &[
    ("ownership_prong_strategy", DelegationStatus::GenuineImplementation),
    ("control_prong_strategy", DelegationStatus::GenuineImplementation),
    ("trust_role_strategy", DelegationStatus::GenuineImplementation),
    // TS.4 §4 finding, Phase 1 (RED-honest pin, BEFORE Ruling A's fix):
    // `fund_control_strategy` forwards VERBATIM to `control_prong_strategy`
    // — claimed-but-delegated. Phase 2 makes this `GenuineImplementation`
    // (the fund pivot: provenance, per-pivot exhaustion, evidence stud) —
    // a conscious edit here, red/green-proofed in the same tranche.
    (
        "fund_control_strategy",
        DelegationStatus::DeclaredDelegate { delegates_to: "control_prong_strategy" },
    ),
    ("foundation_council_strategy", DelegationStatus::GenuineImplementation),
    ("state_owned_strategy", DelegationStatus::GenuineImplementation),
    ("cooperative_member_strategy", DelegationStatus::GenuineImplementation),
    (
        "nominee_pierce_strategy",
        DelegationStatus::DeclaredDelegate { delegates_to: "control_prong_strategy" },
    ),
];

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
/// through membership alone, so membership per se NEVER yields a UBO via
/// its ECONOMIC weight; control arises from **office** (board/management
/// edges), an anomalous concentrated voting arrangement, OR the membership
/// relationship itself construed as the co-op's own control axis
/// (EOP-DD-KYCUBO-KIT-TS0 §2.5, ratified 2026-08-12, **superseded in part
/// by EOP-DD-KYCUBO-TS.3 §3, ratified 2026-08-21**: V&S §6.4 names
/// "board; voting (often one-member-one-vote)" as the co-op control axis
/// and notes "economic % often meaningless" — TS.3 reads this as ruling
/// membership-rights edges INTO the control walk, not only office; the two
/// documents are reconciled, not contradictory — TS.0's "membership alone
/// never yields a UBO by ECONOMIC weight" stands, TS.3 adds that a
/// membership-rights edge is nonetheless a real, admitted control-kind
/// pipe (pipe 11) when economic % is meaningless).
///
/// Traverses ONLY active reconciled `EdgeKind::VotingRights` +
/// `EdgeKind::BoardAppointment` + `EdgeKind::DominantInfluence` +
/// **`EdgeKind::MembershipRights` (TS.3 — new)** edges — kind-filtered like
/// `FoundationCouncilStrategy` (its 2-kind filter plus voting_rights for the
/// anomalous-concentration case, plus membership_rights for the co-op axis
/// itself). Any other control-kind edge on a Cooperative subject is IGNORED
/// by this strategy — deliberate, mirroring the TrustRole/FoundationCouncil
/// stance: if the structure genuinely mixes, classification is wrong, and
/// reclassification is legal (matrix row 10). Before TS.3, expected COMMON
/// outcome was zero candidates → SMO fallback, same route as
/// `StateOwnedStrategy` (§2.4); TS.3 makes a co-op WITH membership-rights
/// edges resolve directly (`cooperative_resolves_via_membership`, TS.3 §7).
/// Every candidate is `Prong::ControlByOtherMeans`; `effective_ownership_pct`
/// is always `None`; `threshold_pct` is accepted for signature parity but
/// unused.
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

        // Admitted kinds (§2.5, widened by TS.3 §3): voting_rights +
        // board_appointment + dominant_influence + membership_rights.
        let edges = reconciled_control_edges(state);
        let mut adj: BTreeMap<EntityId, Vec<(EntityId, EventId)>> = BTreeMap::new();
        for e in &edges {
            if matches!(
                e.kind,
                EdgeKind::VotingRights
                    | EdgeKind::BoardAppointment
                    | EdgeKind::DominantInfluence
                    | EdgeKind::MembershipRights
            ) {
                adj.entry(e.to).or_default().push((e.from, e.originating_event_id));
            }
        }
        resolve_chain_candidates(adj, subject_entity_id, natural_persons)
    }
}

// ── NomineePierceStrategy (TS.4 = K-8 — resolve the UNDERLYING structure) ───

/// Resolves natural persons controlling a Nominee-classified subject — but
/// ONLY once every nominee arrangement has been pierced
/// (`ubo.edge.pierce-nominee`: the nominee edge is superseded and the
/// disclosed nominator's underlying edge is asserted in one governed event —
/// EOP-DD-KYCUBO-KIT-TS0 §2.6, ratified 2026-08-12; K-8: attributing control
/// to the nominee itself is exactly the wrong answer piercing exists to
/// prevent).
///
/// Post-piercing the subject resolves by the UNDERLYING structure: this
/// strategy is a thin delegate to `ControlProngStrategy`'s traversal
/// (mirroring `FundControlStrategy`'s delegation shape). Pierced nominee
/// edges never traverse by construction — `reconciled_control_edges`
/// excludes both `EdgeKind::Nominee` and superseded edges — and a
/// quantified underlying interest asserted as `economic_interest` stays on
/// the economic axis, feeding the normal ownership path. Every candidate is
/// `Prong::ControlByOtherMeans`; `effective_ownership_pct` is always `None`;
/// `threshold_pct` is accepted for signature parity but unused on the
/// control walk.
///
/// **Fail-closed guard (§2.6 "errors if unpierced nominee edges remain
/// active"):** `resolve()` returns `Vec<ProngCandidate>` and cannot signal
/// error, so the unpierced-nominee scan lives at the freeze dispatch site
/// (`kyc_stream_ops.rs`, `UboDeterminationFreeze`): freeze hard-errors
/// while any active `EdgeKind::Nominee` edge remains, BEFORE this strategy
/// ever runs. Callers reaching `resolve()` outside freeze (e.g. replay)
/// must apply the same scan themselves if they need the guard.
///
/// **Scope (TS.4 v1):** inherits `ControlProngStrategy`'s v1 boundary — no
/// crossing into the economic axis for an intermediate controlling entity's
/// own UBOs (v2).
pub struct NomineePierceStrategy;

impl DeterminationStrategy for NomineePierceStrategy {
    fn name(&self) -> &'static str {
        "nominee_pierce_strategy"
    }

    fn resolve(
        &self,
        state: &ControlState,
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>,
        threshold_pct: f64,
    ) -> Vec<ProngCandidate> {
        // Thin delegate (§2.6): the underlying structure resolves via the
        // control prong; the unpierced-nominee guard lives at the freeze
        // dispatch site (see type doc).
        ControlProngStrategy.resolve(state, subject_entity_id, natural_persons, threshold_pct)
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

// ── Statutory-authority stop (TS.3 §3/§4a — "Stop must record why") ─────────

/// One recorded halt: a `StatutoryAuthority`-kind edge reached `stopped_at`,
/// and the walk did NOT continue onward (`ControlAdmission::Stop`,
/// `fold::control::control_admission`). Not a silent exclusion — the entity
/// asserting the authority and the reason are both recorded (K-8-style
/// discipline extended to this admission class).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalStop {
    /// The entity the walk was AT when it hit the stop (typically the
    /// determination subject itself — see scope note on
    /// `detect_statutory_stops`).
    pub stopped_at: EntityId,
    /// The government/sovereign entity asserting statutory authority.
    pub authority_entity: EntityId,
    pub reason: String,
    pub originating_event_id: EventId,
}

/// TS.3 §3/§4a: scan for active `StatutoryAuthority` edges pointed at
/// `subject_entity_id` and record why the walk halted there instead of
/// continuing onward or silently dropping the edge (the pre-TS.3
/// behaviour — `StatutoryAuthority` was already excluded from
/// `reconciled_control_edges`, but nothing recorded that fact).
///
/// **Scope (v1):** direct-to-subject edges only, mirroring every other
/// control-axis strategy's documented v1 boundary in this file (no
/// crossing further into the graph for an intermediate entity's own
/// statutory relationships — v2).
pub fn detect_statutory_stops(
    state: &ControlState,
    subject_entity_id: EntityId,
) -> Vec<TraversalStop> {
    crate::fold::control::edges_of_kind_into(
        state,
        crate::fold::control::EdgeKind::StatutoryAuthority,
        subject_entity_id,
    )
    .into_iter()
    .map(|e| TraversalStop {
        stopped_at: subject_entity_id,
        authority_entity: e.from,
        reason: "target reached via statutory authority — routes to SMO/special-handling, \
                 not further traversal (V&S §6.4 state-owned row; TS.3 §3 pipe 12)"
            .to_string(),
        originating_event_id: e.originating_event_id,
    })
    .collect()
}

// ── SMO pull on exhaustion (TS.3 §4a) ────────────────────────────────────────

/// TS.3 §4a: the audit record that the SMO/officer population was PULLED by
/// the strategy on exhaustion, never PUSHED by edge admission. Never the
/// sole carrier of the pulled candidates — those are ordinary
/// `ProngCandidate`s (`Prong::SmoFallback`) folded into the same candidate
/// list `pull_smo_on_exhaustion` returns alongside this record; this struct
/// exists so the pull is never silent (K-8 discipline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmoPullRecord {
    /// The entity/entities the walk exhausted at (TS.2 Ruling 2a — the
    /// mandate holder, not the fund, when a chain is involved). More than
    /// one only in the (currently untested) case of a genuinely
    /// multi-branch exhaustion.
    pub exhausted_at: Vec<EntityId>,
    pub reason: String,
    pub pulled_officer_count: usize,
}

/// TS.3 §4a: ownership and control have exhausted (`prior_candidates` is
/// empty) — ask for the officer/SMO population of the entity/entities the
/// walk actually exhausted at, not the subject blindly. Returns `None` if
/// `prior_candidates` is non-empty (§4a: pull fires ONLY on exhaustion,
/// never pushed — `officers_contribute_only_on_exhaustion`) or if no
/// `OfficerAppointment` edge exists at the exhaustion frontier (the
/// existing manual `apply-smo-fallback` / authorised-waiver route remains
/// the K-5 escape hatch in that case).
///
/// **Frontier, independently derived:** re-walks `reconciled_control_edges`
/// from `subject_entity_id` (the SAME admitted-edge set every control-axis
/// strategy shares) to find the exact set of non-person nodes the walk
/// reached and could go no further from — this reconstructs the frontier a
/// strategy's own DFS would have stopped at, without requiring
/// `DeterminationStrategy::resolve()`'s signature to expose its internal
/// walk state (kept out of scope — see `docs/todo/EOP-DD-KYCUBO-TS.3...
/// §6`, which names only the admission function as the code surface this
/// tranche changes). **v1 boundary:** uses the FULL `reconciled_control_
/// edges` set uniformly, not each strategy's own narrower kind filter
/// (`TrustRoleStrategy`/`FoundationCouncilStrategy`/`CooperativeMember
/// Strategy` each additionally restrict beyond the shared set) — for every
/// fixture this tranche's gates construct, the narrower strategies have no
/// OTHER kind of edge present, so the frontier coincides; a stricter
/// per-strategy-aware pull is v2.
pub fn pull_smo_on_exhaustion(
    state: &ControlState,
    subject_entity_id: EntityId,
    natural_persons: &BTreeSet<PersonId>,
    prior_candidates: &[ProngCandidate],
) -> Option<(Vec<ProngCandidate>, SmoPullRecord)> {
    if !prior_candidates.is_empty() {
        return None;
    }

    let edges = reconciled_control_edges(state);
    let mut adj: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    for e in &edges {
        adj.entry(e.to).or_default().push(e.from);
    }
    for parents in adj.values_mut() {
        parents.sort();
    }

    // Re-derive the frontier: nodes the walk reached (from subject_entity_id,
    // over the same adjacency every control strategy shares) that have NO
    // further admitted parent edges — a dead end, i.e. where the walk
    // actually exhausted. Cycle-guarded like `resolve_chain_candidates`.
    let mut visited: BTreeSet<EntityId> = BTreeSet::new();
    let mut frontier: BTreeSet<EntityId> = BTreeSet::new();
    let mut stack = vec![subject_entity_id];
    while let Some(node) = stack.pop() {
        if !visited.insert(node) {
            continue;
        }
        match adj.get(&node) {
            None => {
                frontier.insert(node);
            }
            Some(parents) if parents.is_empty() => {
                frontier.insert(node);
            }
            Some(parents) => {
                for &parent in parents {
                    // A natural-person parent would already be a candidate —
                    // if prior_candidates is empty, none of the admitted
                    // edges reach one (consistency guard, not expected to
                    // trigger in well-formed input).
                    if !natural_persons.contains(&PersonId(parent.0)) {
                        stack.push(parent);
                    }
                }
            }
        }
    }

    let mut pulled: Vec<ProngCandidate> = Vec::new();
    for &entity in &frontier {
        for e in crate::fold::control::edges_of_kind_into(
            state,
            crate::fold::control::EdgeKind::OfficerAppointment,
            entity,
        ) {
            let candidate_person = PersonId(e.from.0);
            if natural_persons.contains(&candidate_person) {
                pulled.push(ProngCandidate {
                    person_id: candidate_person,
                    prong: Prong::SmoFallback,
                    effective_ownership_pct: None,
                    ownership_chain: vec![subject_entity_id, entity],
                    originating_event_id: e.originating_event_id,
                });
            }
        }
    }
    if pulled.is_empty() {
        return None;
    }
    pulled.sort_by_key(|c| c.person_id.0);

    let exhausted_at: Vec<EntityId> = pulled
        .iter()
        .map(|c| *c.ownership_chain.last().expect("pull chain always has >=2 entries"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let record = SmoPullRecord {
        pulled_officer_count: pulled.len(),
        reason: "ownership and control exhausted with no natural-person candidate — officer/SMO \
                 population pulled from the entity the walk exhausted at (TS.3 §4a); never \
                 pushed by edge admission"
            .to_string(),
        exhausted_at,
    };
    Some((pulled, record))
}

// ── Provisionality through traversal decisions (TS.3 §2a) ───────────────────

/// TS.3 §2a: WHY a determination is provisional — three distinct
/// remediation tasks, never collapsed into one flag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProvisionalityReason {
    /// A control-kind edge touching the determination is not yet
    /// `EdgeStatus::Verified`.
    AllegedEdge { entity: EntityId, edge_kind_label: String },
    /// A participant entity's own type assertion
    /// (`TypeRegistryState::proof_of`) is `TypeProofStatus::Alleged`, not
    /// yet `Proved`.
    AllegedType { entity: EntityId },
    /// An admission/stop decision (`control_admission`) was taken while the
    /// entity that decision turned on carries only an alleged type — the
    /// narrow case §2a calls out by name: a `Stop` at a believed-but-
    /// unproven state body.
    AdmissionOnAllegedType { entity: EntityId, edge_kind_label: String },
}

/// The assurance surface for one determination: empty means fully proved
/// (every implicated edge Verified, every implicated entity's type Proved);
/// non-empty means provisional, with each reason distinct (§2a: "different
/// remediation tasks... must not collapse into one flag").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterminationAssurance {
    pub reasons: Vec<ProvisionalityReason>,
}

impl DeterminationAssurance {
    pub fn is_provisional(&self) -> bool {
        !self.reasons.is_empty()
    }
}

/// TS.3 §2a: propagate provisionality through the traversal DECISIONS, not
/// only the edges. Walks every entity implicated in the final answer —
/// every candidate's `ownership_chain`, every stop's `stopped_at`/
/// `authority_entity` — and checks two independent signals:
/// 1. is any control-kind edge touching an implicated entity pair still
///    only `Asserted`/`Evidenced` (not yet `Verified`)? → `AllegedEdge`.
/// 2. is any implicated entity's own type assertion still `Alleged`
///    (not yet `Proved`)? → `AllegedType`; additionally, for a `Stop`
///    specifically, `AdmissionOnAllegedType` — the STOP decision itself
///    rested on believing (not yet proving) that entity is a state body.
///
/// Deterministic by construction: iterates only `BTreeMap`/`BTreeSet`
/// collections (Q6, K-16/18/33 — no `HashMap`/`HashSet` in this module).
pub fn compute_assurance(
    candidates: &[ProngCandidate],
    stops: &[TraversalStop],
    control_state: &ControlState,
    type_registry: &crate::fold::type_registry::TypeRegistryState,
) -> DeterminationAssurance {
    use crate::fold::type_registry::TypeProofStatus;

    let mut touched: BTreeSet<EntityId> = BTreeSet::new();
    for c in candidates {
        touched.extend(c.ownership_chain.iter().copied());
    }
    for s in stops {
        touched.insert(s.stopped_at);
        touched.insert(s.authority_entity);
    }

    let mut reasons: BTreeSet<ProvisionalityReason> = BTreeSet::new();

    for edge in control_state.edges.values() {
        if !edge.is_active() {
            continue;
        }
        if !matches!(edge.status, crate::fold::control::EdgeStatus::Verified)
            && (touched.contains(&edge.from) || touched.contains(&edge.to))
        {
            reasons.insert(ProvisionalityReason::AllegedEdge {
                entity: edge.to,
                edge_kind_label: format!("{:?}", edge.kind),
            });
        }
    }

    for &entity in &touched {
        if type_registry.proof_of(entity) == Some(TypeProofStatus::Alleged) {
            reasons.insert(ProvisionalityReason::AllegedType { entity });
        }
    }

    for s in stops {
        if type_registry.proof_of(s.stopped_at) == Some(TypeProofStatus::Alleged) {
            reasons.insert(ProvisionalityReason::AdmissionOnAllegedType {
                entity: s.stopped_at,
                edge_kind_label: "statutory_authority".to_string(),
            });
        }
    }

    DeterminationAssurance { reasons: reasons.into_iter().collect() }
}

// ── Frozen determination (K-18) ───────────────────────────────────────────────

/// The K-18 pin.  A determination is immutable and reproducible against:
/// - policy version
/// - lexicon manifest hash (Q7)
/// - reference data snapshot
/// - import run ids
/// - content hash of the reconciled control graph
/// - frozen `as_of` timestamp (Q6 — never a live wall-clock read)
/// - who asked (`viewer`), which build resolved it (`kit_version`), and
///   which strategy produced it (`constructor_version`) — R4
///   (`EOP-PLAN-GAMEBOARD-001`, closing the KIT-6 pin-set gap the KIT plan's
///   own T5/R6 named: "viewer/axis/kit-version/constructor-version").
///   `axis` is deliberately not a field here — see
///   `recover_determination_bitemporal`'s own doc for why passing explicit
///   `valid_at`/`known_at` is a strict generalisation of an axis selector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// R4: who requested this determination/replay. `None` for the common
    /// case where no actor context is threaded through yet (this is
    /// additive metadata, not a new access-control gate).
    #[serde(default)]
    pub viewer: Option<Principal>,
    /// R4: this crate's own build version at freeze/recovery time
    /// (`env!("CARGO_PKG_VERSION")`) — pins which fold-registry/strategy
    /// code computed the result, distinct from `lexicon_manifest_hash`
    /// (which pins the lexicon *data*, not the code that reads it).
    #[serde(default)]
    pub kit_version: String,
    /// R4: the `DeterminationStrategy::name()` that actually resolved
    /// candidates (`det.strategy` at freeze time) — was previously only
    /// implicit in `policy_version`'s ambiguous semantics; now recorded
    /// explicitly and honestly, `None` when no strategy had been selected
    /// yet (e.g. a determination that only ran the SMO fallback).
    #[serde(default)]
    pub constructor_version: Option<String>,
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
    /// Resolved UBO persons with basis (K-1: basis mandatory). Includes any
    /// SMO-pulled officers (`Prong::SmoFallback`, TS.3 §4a) — the pull
    /// folds directly into this list; `smo_pull` (below) is the separate
    /// audit record that a pull happened, never candidates' sole carrier.
    pub candidates: Vec<ProngCandidate>,
    /// SMO fallback result (K-5: present if no candidates, or if SMO was required).
    pub smo_result: Option<SmoResult>,
    /// TS.3 §3/§4a: recorded statutory-authority stops (empty if none).
    #[serde(default)]
    pub stops: Vec<TraversalStop>,
    /// TS.3 §4a: the audit record that the SMO population was pulled on
    /// exhaustion (`None` if no pull occurred — either candidates were
    /// non-empty, or nothing was found to pull).
    #[serde(default)]
    pub smo_pull: Option<SmoPullRecord>,
    /// TS.3 §2a: why (if at all) this determination is provisional.
    #[serde(default)]
    pub assurance: DeterminationAssurance,
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
    /// TS.3 §3 (`detect_statutory_stops`) — populated by the caller
    /// alongside `candidates`, before `freeze_determination` runs.
    pub stops: Vec<TraversalStop>,
    /// TS.3 §4a (`pull_smo_on_exhaustion`) — populated by the caller when
    /// the pull fired; the pulled candidates themselves are already folded
    /// into `candidates` by the time this is set.
    pub smo_pull: Option<SmoPullRecord>,
}

// ── freeze_determination ──────────────────────────────────────────────────────

/// Execute `ubo.determination.freeze`:
/// - Requires `state.is_reconciled()` and `state.has_strategy()` (pre-checked).
/// - Pins policy + lexicon + reference + import runs + graph hash + as_of.
/// - Returns `FrozenDetermination` (immutable).
/// - K-5: if candidates is empty AND no SMO was applied, returns Err.
/// - TS.3 §2a: computes `assurance` from `det.candidates`/`det.stops`
///   against `type_registry` — never gates on it (`determination_
///   runs_at_any_board_state`); a fully-alleged board still freezes,
///   labelled provisional.
#[allow(clippy::too_many_arguments)]
pub fn freeze_determination(
    det: &DeterminationInProgress,
    control_state: &ControlState,
    type_registry: &crate::fold::type_registry::TypeRegistryState,
    freeze_event: &IntentEvent,
    policy_version: &str,
    lexicon_manifest_hash: Hash,
    reference_snapshot_id: Uuid,
    import_run_ids: BTreeSet<Uuid>,
    viewer: Option<Principal>,
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
        viewer,
        kit_version: env!("CARGO_PKG_VERSION").to_string(),
        constructor_version: det.strategy.clone(),
    };

    // Determination hash covers pin + candidates.
    let pin_json = serde_json::to_value(&pin).expect("pin is serialisable");
    let cands_json = serde_json::to_value(&det.candidates).expect("candidates are serialisable");
    let combined = serde_json::json!({ "pin": pin_json, "candidates": cands_json });
    let determination_hash = Hash::of_json(&combined);

    let assurance = compute_assurance(&det.candidates, &det.stops, control_state, type_registry);

    Ok(FrozenDetermination {
        pin,
        freeze_event_id: freeze_event.id,
        candidates: det.candidates.clone(),
        smo_result: det.smo_result.clone(),
        stops: det.stops.clone(),
        smo_pull: det.smo_pull.clone(),
        assurance,
        determination_hash,
    })
}

// ── Replay / point-in-time recovery ──────────────────────────────────────────

/// Pin parameters required to freeze/recover a determination (K-18).
/// Groups the reference-plane inputs so `recover_determination_at` stays under 8 args.
/// `viewer` (R4): who is asking for this replay — threaded straight into the
/// resulting `DeterminationPin`, not otherwise interpreted here (this crate
/// stays "no sem_os_core — heavy/DB-adjacent" per its own Cargo.toml; access
/// control on `viewer` is a caller/higher-layer concern).
pub struct RecoveryPin<'a> {
    pub policy_version: &'a str,
    pub lexicon_manifest_hash: Hash,
    pub reference_snapshot_id: Uuid,
    pub import_run_ids: BTreeSet<Uuid>,
    pub viewer: Option<Principal>,
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
    use crate::fold::type_registry::fold_type_registry;

    let control = fold_control(events);
    if !control.is_reconciled() || !control.has_strategy() {
        return None;
    }
    let type_registry = fold_type_registry(events);

    // Find the subject entity from the first classify event.
    let subject_entity = find_subject_entity(events)?;

    let mut candidates =
        strategy.resolve(&control, subject_entity, natural_persons, threshold_pct);

    // TS.3 §3: record any statutory-authority stop at the subject.
    let stops = detect_statutory_stops(&control, subject_entity);

    // TS.3 §4a: pull the officer/SMO population on exhaustion — ONLY fires
    // when the strategy's own walk produced nothing (never pushes).
    let smo_pull = match pull_smo_on_exhaustion(&control, subject_entity, natural_persons, &candidates) {
        Some((pulled, record)) => {
            candidates.extend(pulled);
            Some(record)
        }
        None => None,
    };

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
        stops,
        smo_pull,
    };

    freeze_determination(
        &det,
        &control,
        &type_registry,
        freeze_event,
        pin.policy_version,
        pin.lexicon_manifest_hash,
        pin.reference_snapshot_id,
        pin.import_run_ids,
        pin.viewer,
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
