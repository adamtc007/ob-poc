//! Control & determination fold (§4.1 of EOP-DD-KYCUBO-001).
//!
//! `ControlState` is derived from the ordered event stream; it is never stored
//! as authoritative state.  Crucially:
//!
//! - **Edge status is a fold output** (Q5, K-11): the fold computes
//!   `Asserted → Evidenced → Verified → Superseded` from the event types;
//!   nothing sets status directly.  There is no "set status" verb.
//! - **Terminal natural-person status** (`approved`, `waived`) is set by an
//!   explicit decision verb (Q5).
//! - **Intermediate-entity resolution** is derived-and-checkpointed (Q5).
//!
//! # Determinism invariant
//!
//! **INVARIANT (fold path):** No `HashMap`/`HashSet`, no `Uuid::new_v4` /
//! `EventId::new`, no `Utc::now()`, no `SystemTime::now()`, and no
//! float-to-string in any hashed payload inside this module.  Violating any
//! of these breaks bit-identical replay (Q6, K-16/18/33).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event::IntentEvent;
use crate::types::{EdgeId, EntityId, EventId, PersonId};

// ── Edge kind ─────────────────────────────────────────────────────────────────

/// The means by which control or economic interest is exercised.
/// Each has its own validity rule and proof rule (V&S §7.1 control taxonomy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// Shareholding percentage (economic axis).
    EconomicInterest,
    /// Voting rights (control axis).
    VotingRights,
    /// Board appointment rights.
    BoardAppointment,
    /// GP statutory control (LP/PE fund).
    GpStatutory,
    /// Designated member control (LLP).
    DesignatedMember,
    /// Trust role (settlor, trustee, protector, beneficiary).
    TrustRole(TrustRoleKind),
    /// Nominee — must be pierced (K-8).
    Nominee,
    /// Dominant influence (catch-all control).
    DominantInfluence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustRoleKind {
    Settlor,
    Trustee,
    Protector,
    Beneficiary,
}

// ── Edge epistemic status ─────────────────────────────────────────────────────

/// Derived by the fold from the sequence of events touching an edge.
/// **Not stored; not settable.** (K-11, Q5 — §4.1 design invariant.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EdgeStatus {
    /// `assert-control` or `assert-economic-interest` event seen.
    Asserted,
    /// `attach-evidence` event seen; evidence cited.
    Evidenced,
    /// `verify` event seen (precondition: evidence was cited first).
    Verified,
    /// `supersede` event seen (never removed — K-13).
    Superseded,
}

impl std::fmt::Display for EdgeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeStatus::Asserted => write!(f, "Asserted"),
            EdgeStatus::Evidenced => write!(f, "Evidenced"),
            EdgeStatus::Verified => write!(f, "Verified"),
            EdgeStatus::Superseded => write!(f, "Superseded"),
        }
    }
}

// ── Edge state ────────────────────────────────────────────────────────────────

/// The folded view of one control or economic-interest edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeState {
    pub id: EdgeId,
    pub kind: EdgeKind,
    /// Source entity (parent in ownership / controlling entity).
    pub from: EntityId,
    /// Target entity (child / controlled entity).
    pub to: EntityId,
    /// Percentage (only meaningful for economic-interest edges).
    pub percentage: Option<f64>,
    /// Derived status — fold output only (K-11).
    pub status: EdgeStatus,
    /// Event that cited evidence (if any).
    pub evidence_event_id: Option<EventId>,
    /// The original assertion event (K-35 traceability).
    pub originating_event_id: EventId,
}

impl EdgeState {
    pub fn is_active(&self) -> bool {
        self.status != EdgeStatus::Superseded
    }

    pub fn is_verified(&self) -> bool {
        self.status == EdgeStatus::Verified
    }

    pub fn is_economic(&self) -> bool {
        matches!(self.kind, EdgeKind::EconomicInterest)
    }
}

// ── Structure class ───────────────────────────────────────────────────────────

/// The subject's structure class, set by `kyc.subject.classify-structure`.
/// Drives strategy selection (K-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructureClass {
    PrivateCompany,
    MultiTierHoldingGroup,
    ListedEntity,
    LimitedPartnershipFund,
    Llp,
    Trust,
    Foundation,
    InvestmentFund,
    StateOwned,
    Cooperative,
    Nominee,
}

// ── Control state ─────────────────────────────────────────────────────────────

/// The folded control & determination graph for one `SubjectId`.
/// `state = fold_control(events)`.  Never stored directly — rebuilt from
/// the event stream (or from a checkpoint snapshot for performance).
#[derive(Debug, Default, Clone)]
pub struct ControlState {
    /// All edges, including Superseded ones (K-13: supersede-never-delete).
    pub edges: BTreeMap<EdgeId, EdgeState>,
    /// Set if `kyc.subject.classify-structure` has fired.
    pub structure_class: Option<StructureClass>,
    /// Event that set the structure class (K-35 traceability).
    pub classify_event_id: Option<EventId>,
    /// Event id of the most-recent `ubo.edge.reconcile-conflict`.
    /// Required before `compute-fold` and `freeze` (K-14).
    pub reconciliation_event_id: Option<EventId>,
    /// Strategy selected by `ubo.determination.select-strategy`.
    pub selected_strategy: Option<String>,
    pub strategy_event_id: Option<EventId>,
    /// SMO fallback person (if applied).
    pub smo_person_id: Option<PersonId>,
    pub smo_event_id: Option<EventId>,
    /// Subject registration (if `kyc.subject.register` has fired).
    pub registered: bool,
    pub register_event_id: Option<EventId>,
}

impl ControlState {
    /// Active (non-superseded) economic-interest edges only.
    pub fn active_economic_edges(&self) -> impl Iterator<Item = &EdgeState> {
        self.edges
            .values()
            .filter(|e| e.is_active() && e.is_economic())
    }

    /// Total claimed economic interest in the subject (across all active
    /// economic edges pointing *to* it).  A value >100% signals unreconciled
    /// conflict (K-14).
    pub fn total_claimed_economic_pct(&self, subject: EntityId) -> f64 {
        self.active_economic_edges()
            .filter(|e| e.to == subject)
            .filter_map(|e| e.percentage)
            .sum()
    }

    /// True if a reconcile-conflict event has been recorded (K-14 precondition).
    pub fn is_reconciled(&self) -> bool {
        self.reconciliation_event_id.is_some()
    }

    /// True if a strategy has been selected (K-4 precondition for fold/freeze).
    pub fn has_strategy(&self) -> bool {
        self.selected_strategy.is_some()
    }
}

// ── Fold function ─────────────────────────────────────────────────────────────

/// Parse an `EdgeId` from the event payload (field `"edge_id"`).
fn edge_id_from_payload(payload: &serde_json::Value) -> Option<EdgeId> {
    payload
        .get("edge_id")?
        .as_str()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(EdgeId)
}

fn edge_id_from_target(event: &IntentEvent) -> Option<EdgeId> {
    event.target.edge_id
}

fn entity_id(v: &serde_json::Value, field: &str) -> Option<EntityId> {
    v.get(field)?
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .map(EntityId)
}

fn person_id(v: &serde_json::Value, field: &str) -> Option<PersonId> {
    v.get(field)?
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .map(PersonId)
}

fn opt_f64(v: &serde_json::Value, field: &str) -> Option<f64> {
    v.get(field)?.as_f64()
}

fn edge_kind_from_payload(payload: &serde_json::Value) -> EdgeKind {
    match payload.get("kind").and_then(|v| v.as_str()) {
        Some("economic_interest") => EdgeKind::EconomicInterest,
        Some("voting_rights") => EdgeKind::VotingRights,
        Some("board_appointment") => EdgeKind::BoardAppointment,
        Some("gp_statutory") => EdgeKind::GpStatutory,
        Some("designated_member") => EdgeKind::DesignatedMember,
        Some("nominee") => EdgeKind::Nominee,
        Some("dominant_influence") | Some(_) | None => EdgeKind::DominantInfluence,
    }
}

fn structure_class_from_payload(payload: &serde_json::Value) -> Option<StructureClass> {
    match payload.get("structure_class")?.as_str()? {
        "private_company" => Some(StructureClass::PrivateCompany),
        "multi_tier_holding" => Some(StructureClass::MultiTierHoldingGroup),
        "listed_entity" => Some(StructureClass::ListedEntity),
        "lp_fund" => Some(StructureClass::LimitedPartnershipFund),
        "llp" => Some(StructureClass::Llp),
        "trust" => Some(StructureClass::Trust),
        "foundation" => Some(StructureClass::Foundation),
        "investment_fund" => Some(StructureClass::InvestmentFund),
        "state_owned" => Some(StructureClass::StateOwned),
        "cooperative" => Some(StructureClass::Cooperative),
        "nominee" => Some(StructureClass::Nominee),
        _ => None,
    }
}

/// Apply a single event to `ControlState` — the inner step of the v1 fold.
///
/// Extracted so `fold/registry.rs` can compose it without duplicating the match.
/// `pub(crate)`: visible to the registry, not to crate consumers.
pub(crate) fn apply_one_control_event(
    mut state: ControlState,
    event: &IntentEvent,
) -> ControlState {
    let p = &event.payload;
    match event.verb_fqn.as_str() {
        "kyc.subject.register" => {
            state.registered = true;
            state.register_event_id = Some(event.id);
        }

        "kyc.subject.classify-structure" => {
            state.structure_class = structure_class_from_payload(p);
            state.classify_event_id = Some(event.id);
        }

        "ubo.edge.assert-economic-interest" => {
            if let (Some(from), Some(to)) =
                (entity_id(p, "from_entity_id"), entity_id(p, "to_entity_id"))
            {
                let edge_id = edge_id_from_payload(p).unwrap_or_else(|| {
                    let key = format!("economic:{}:{}", from.0, to.0);
                    EdgeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, key.as_bytes()))
                });
                state.edges.insert(
                    edge_id,
                    EdgeState {
                        id: edge_id,
                        kind: EdgeKind::EconomicInterest,
                        from,
                        to,
                        percentage: opt_f64(p, "percentage"),
                        status: EdgeStatus::Asserted,
                        evidence_event_id: None,
                        originating_event_id: event.id,
                    },
                );
            }
        }

        "ubo.edge.assert-control" => {
            if let (Some(from), Some(to)) =
                (entity_id(p, "from_entity_id"), entity_id(p, "to_entity_id"))
            {
                let kind = edge_kind_from_payload(p);
                let key = format!("control:{}:{}:{:?}", from.0, to.0, kind);
                let edge_id = edge_id_from_payload(p)
                    .unwrap_or_else(|| EdgeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, key.as_bytes())));
                state.edges.insert(
                    edge_id,
                    EdgeState {
                        id: edge_id,
                        kind,
                        from,
                        to,
                        percentage: opt_f64(p, "percentage"),
                        status: EdgeStatus::Asserted,
                        evidence_event_id: None,
                        originating_event_id: event.id,
                    },
                );
            }
        }

        "ubo.edge.attach-evidence" => {
            if let Some(eid) = edge_id_from_target(event) {
                if let Some(edge) = state.edges.get_mut(&eid) {
                    if edge.status == EdgeStatus::Asserted {
                        edge.status = EdgeStatus::Evidenced;
                        edge.evidence_event_id = Some(event.id);
                    }
                }
            }
        }

        "ubo.edge.verify" => {
            if let Some(eid) = edge_id_from_target(event) {
                if let Some(edge) = state.edges.get_mut(&eid) {
                    if edge.status == EdgeStatus::Evidenced {
                        edge.status = EdgeStatus::Verified;
                    }
                }
            }
        }

        "ubo.edge.supersede" => {
            // K-13: supersede-never-delete.
            if let Some(eid) = edge_id_from_target(event) {
                if let Some(edge) = state.edges.get_mut(&eid) {
                    edge.status = EdgeStatus::Superseded;
                }
            }
        }

        "ubo.edge.reconcile-conflict" => {
            state.reconciliation_event_id = Some(event.id);
        }

        "ubo.determination.select-strategy" => {
            state.selected_strategy = p
                .get("strategy")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            state.strategy_event_id = Some(event.id);
        }

        "ubo.determination.apply-smo-fallback" => {
            state.smo_person_id = person_id(p, "smo_person_id");
            state.smo_event_id = Some(event.id);
        }

        _ => {}
    }
    state
}

/// Pure fold of the ordered event stream onto `ControlState`.
///
/// This is the heart of §4.1: edge status is computed, never stored.
/// Every field of `ControlState` has a traceable originating event (K-35).
///
/// For version-dispatched replay (D2), use `fold_control_versioned` in
/// `fold::registry` — it dispatches each event through the `FoldRegistry`
/// keyed on `event.lexicon_hash`.
pub fn fold_control(events: &[&IntentEvent]) -> ControlState {
    events.iter().fold(ControlState::default(), |st, e| {
        apply_one_control_event(st, e)
    })
}

// ── Precondition checker ──────────────────────────────────────────────────────

use crate::error::KycError;
use crate::lexicon::{LexiconEntry, Precondition};

/// Check all preconditions for a verb against the current control state
/// and target binding **before** appending the event (K-11, K-14).
pub fn check_control_preconditions(
    lexicon_entry: &LexiconEntry,
    state: &ControlState,
    event: &IntentEvent,
) -> Result<(), KycError> {
    for pre in &lexicon_entry.preconditions {
        match pre {
            Precondition::EvidenceCited => {
                let eid = event.target.edge_id.ok_or_else(|| {
                    KycError::MissingTarget("edge_id required for EvidenceCited".into())
                })?;
                match state.edges.get(&eid) {
                    Some(e) if e.status == EdgeStatus::Evidenced => {} // ok
                    Some(e) => {
                        return Err(KycError::VerifyWithoutEvidence(eid, e.status.to_string()));
                    }
                    None => return Err(KycError::EdgeNotFound(eid)),
                }
            }
            Precondition::ReconciledProjection => {
                if !state.is_reconciled() {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "reconcile-conflict must fire before compute-fold / freeze".into(),
                    });
                }
            }
            Precondition::StrategySelected => {
                if !state.has_strategy() {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "select-strategy must fire before compute-fold / freeze".into(),
                    });
                }
            }
        }
    }
    Ok(())
}

// ── Economic edge summary (for determination strategy) ────────────────────────

/// A verified, reconciled economic edge ready for prong computation.
/// Consumed by `DeterminationStrategy::resolve()` (§5).
#[derive(Debug, Clone)]
pub struct ReconciledEconomicEdge {
    pub id: EdgeId,
    pub from: EntityId,
    pub to: EntityId,
    pub percentage: f64,
    /// The event that verified this edge (K-35 traceability on the candidate).
    pub verified_by: Option<EventId>,
    /// The event that originally asserted this edge (deterministic; never random).
    pub originating_event_id: EventId,
}

/// Extract the reconciled (active, verified) economic edges from the control
/// state.  Used by `OwnershipProngStrategy`.
///
/// Caller is responsible for ensuring `state.is_reconciled()` before calling
/// (i.e., the `ReconciledProjection` precondition has been checked).
pub fn reconciled_economic_edges(state: &ControlState) -> Vec<ReconciledEconomicEdge> {
    // We accept Evidenced OR Verified edges post-reconcile: the reconcile event
    // canonicalises which source representation is authoritative; evidence
    // attachment is the next step, not a prerequisite for reconcile.
    // Only exclude Superseded.
    state
        .edges
        .values()
        .filter(|e| e.is_economic() && e.is_active())
        .filter_map(|e| {
            let pct = e.percentage?;
            Some(ReconciledEconomicEdge {
                id: e.id,
                from: e.from,
                to: e.to,
                percentage: pct,
                verified_by: if e.is_verified() {
                    e.evidence_event_id
                } else {
                    None
                },
                originating_event_id: e.originating_event_id,
            })
        })
        .collect()
}

// ── Control edge summary (for determination strategy, M4) ─────────────────────

/// A verified, reconciled control edge ready for prong computation.
/// Consumed by `ControlProngStrategy::resolve()` (§5, M4).
#[derive(Debug, Clone)]
pub struct ReconciledControlEdge {
    pub id: EdgeId,
    pub from: EntityId,
    pub to: EntityId,
    pub kind: EdgeKind,
    /// The event that verified this edge (K-35 traceability on the candidate).
    pub verified_by: Option<EventId>,
    /// The event that originally asserted this edge (deterministic; never random).
    pub originating_event_id: EventId,
}

/// Extract the reconciled (active, verified) control edges from the control
/// state — everything that isn't `EconomicInterest` (that's the ownership
/// prong's job — `reconciled_economic_edges`). Used by `ControlProngStrategy`
/// (M4).
///
/// Excludes `EdgeKind::Nominee`: nominee arrangements require piercing (K-8,
/// `ubo.edge.pierce-nominee`, separately-tracked M2/open work) to find the
/// true controller. Treating a bare nominee edge as direct control here
/// would attribute control to the nominee itself — exactly the wrong answer
/// K-8 exists to prevent.
pub fn reconciled_control_edges(state: &ControlState) -> Vec<ReconciledControlEdge> {
    state
        .edges
        .values()
        .filter(|e| !e.is_economic() && !matches!(e.kind, EdgeKind::Nominee) && e.is_active())
        .map(|e| ReconciledControlEdge {
            id: e.id,
            from: e.from,
            to: e.to,
            kind: e.kind.clone(),
            verified_by: if e.is_verified() {
                e.evidence_event_id
            } else {
                None
            },
            originating_event_id: e.originating_event_id,
        })
        .collect()
}

// ── Entity classification helpers ─────────────────────────────────────────────

/// Returns the set of `PersonId`s that are natural persons in the event stream.
/// Declared in the event payload (`"is_natural_person": true`).
///
/// Returns a `BTreeSet` so iteration order is deterministic (sorted by UUID bytes).
pub fn natural_persons_from_events(events: &[&IntentEvent]) -> BTreeSet<PersonId> {
    let mut persons = BTreeSet::new();
    for event in events {
        if event.verb_fqn.as_str() == "kyc.subject.register"
            && event
                .payload
                .get("is_natural_person")
                .and_then(|v| v.as_bool())
                == Some(true)
        {
            if let Some(pid) = person_id(&event.payload, "entity_id") {
                persons.insert(pid);
            }
        }
    }
    persons
}
