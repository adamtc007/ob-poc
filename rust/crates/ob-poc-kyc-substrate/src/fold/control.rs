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
    /// Whether the trust IS revocable (meaningful on
    /// `TrustRole(Settlor)` edges only; captured from the assert-control
    /// payload key `trust_revocable` — TS.1, EOP-DD-KYCUBO-KIT-TS0 §2.1).
    ///
    /// **Polarity:** `Some(false)` means the trust is PROVEN irrevocable —
    /// the only value that excludes the settlor from
    /// `trust_role_strategy` candidates. `Some(true)` and `None` (absent /
    /// unproven) both treat the trust as revocable, so the settlor is
    /// INCLUDED — fail-closed toward inclusion: over-identify, never
    /// silently drop a controller.
    #[serde(default)]
    pub trust_revocable: Option<bool>,
    /// The event that superseded this edge (set by `ubo.edge.supersede` and
    /// `ubo.edge.pierce-nominee` — K-35 traceability on the supersession;
    /// TS.4, EOP-DD-KYCUBO-KIT-TS0 §2.6: for a pierced nominee edge this
    /// points at the pierce event).
    #[serde(default)]
    pub superseded_by: Option<EventId>,
    /// Provenance: the nominee edge this edge was pierced FROM (set only on
    /// edges created by `ubo.edge.pierce-nominee` — TS.4 §2.6, K-8).
    #[serde(default)]
    pub pierced_from: Option<EdgeId>,
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
    /// Entity ids registered so far under this subject_root (T6 row-9 fix,
    /// 2026-08-17, corrects EOP-DD-KYCUBO-KIT-T6 §5). `registered` alone
    /// can't distinguish self-registration from the N natural-person
    /// candidate registrations that share one subject_root — see
    /// `Precondition::NotAlreadyRegistered`, which is keyed off this set.
    pub registered_entity_ids: BTreeSet<EntityId>,
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

/// The canonical `kind` wire-string set for `ubo.edge.assert-control` —
/// exactly the strings `edge_kind_from_payload` recognizes with a dedicated
/// arm (TS.1, EOP-DD-KYCUBO-KIT-TS0 §1a/§1b). ONE source of truth: the
/// op-normalizer (`kyc_stream_ops.rs`) rejects any `kind` outside this set
/// BEFORE append (fail-closed), and the wire-value closure tooth
/// (`tests/kyc_pack_closure.rs::edge_kind_wire_values_are_exactly_known`)
/// pins this const against the YAML `valid_values` — adding an `EdgeKind`
/// wire value is a conscious three-way edit (arm + const + YAML), never a
/// silent widening.
pub const EDGE_KIND_WIRE_VALUES: &[&str] = &[
    "economic_interest",
    "voting_rights",
    "board_appointment",
    "gp_statutory",
    "designated_member",
    "trust_settlor",
    "trust_trustee",
    "trust_protector",
    "trust_beneficiary",
    "nominee",
    "dominant_influence",
];

fn edge_kind_from_payload(payload: &serde_json::Value) -> EdgeKind {
    match payload.get("kind").and_then(|v| v.as_str()) {
        Some("economic_interest") => EdgeKind::EconomicInterest,
        Some("voting_rights") => EdgeKind::VotingRights,
        Some("board_appointment") => EdgeKind::BoardAppointment,
        Some("gp_statutory") => EdgeKind::GpStatutory,
        Some("designated_member") => EdgeKind::DesignatedMember,
        Some("trust_settlor") => EdgeKind::TrustRole(TrustRoleKind::Settlor),
        Some("trust_trustee") => EdgeKind::TrustRole(TrustRoleKind::Trustee),
        Some("trust_protector") => EdgeKind::TrustRole(TrustRoleKind::Protector),
        Some("trust_beneficiary") => EdgeKind::TrustRole(TrustRoleKind::Beneficiary),
        Some("nominee") => EdgeKind::Nominee,
        // Total-dispatch backstop for HISTORICAL events only (the fold must
        // stay infallible — D2). The live append path can no longer reach
        // this with an unknown/absent kind: the op-normalizer rejects
        // anything outside `EDGE_KIND_WIRE_VALUES` before the event exists
        // (TS.1 §1b).
        Some("dominant_influence") | Some(_) | None => EdgeKind::DominantInfluence,
    }
}

/// The canonical `structure-class` wire-string set for
/// `kyc.subject.classify-structure` — exactly the strings
/// `structure_class_from_payload` recognizes with a dedicated arm, mirroring
/// `EDGE_KIND_WIRE_VALUES` (EOP-FUZZ-KYCUBO-001 §5 finding #2: this table
/// didn't exist before, so `structure-class` had no op-side fail-closed gate
/// the way `kind` does — the op-normalizer now rejects any `structure-class`
/// outside this set BEFORE append, closing the same defect class TS.1 closed
/// for `EdgeKind`).
pub const STRUCTURE_CLASS_WIRE_VALUES: &[&str] = &[
    "private_company",
    "multi_tier_holding",
    "listed_entity",
    "lp_fund",
    "llp",
    "trust",
    "foundation",
    "investment_fund",
    "state_owned",
    "cooperative",
    "nominee",
];

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
            if let Some(eid) = entity_id(p, "entity_id") {
                state.registered_entity_ids.insert(eid);
            }
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
                        trust_revocable: None,
                        superseded_by: None,
                        pierced_from: None,
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
                        // TS.1 §2.1: revocability proof for settlor edges
                        // (see the field's polarity doc on `EdgeState`).
                        trust_revocable: p.get("trust_revocable").and_then(|v| v.as_bool()),
                        superseded_by: None,
                        pierced_from: None,
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
                    edge.superseded_by = Some(event.id);
                }
            }
        }

        "ubo.edge.pierce-nominee" => {
            // TS.4 (K-8, EOP-DD-KYCUBO-KIT-TS0 §2.6): TWO effects in ONE
            // governed event. (a) the target nominee edge is SUPERSEDED
            // (the existing supersession lifecycle — K-13
            // supersede-never-contradict; `superseded_by` points at this
            // pierce event); (b) a NEW control edge from the disclosed
            // nominator is asserted with the UNDERLYING kind (payload
            // `kind` — what the nominator actually holds; the op-normalizer
            // rejects `nominee` fail-closed before append) and
            // `pierced_from` provenance. Total-dispatch discipline: if the
            // target edge or nominator is unresolvable (historical/garbage
            // event), the arm applies nothing — the fold stays infallible;
            // the live append path can't reach that state (EdgeExists/
            // EdgeActive preconditions + the op-layer nominee-kind check).
            if let (Some(eid), Some(nominator)) = (
                edge_id_from_target(event),
                entity_id(p, "nominator_entity_id"),
            ) {
                if let Some(to) = state.edges.get(&eid).map(|e| e.to) {
                    if let Some(edge) = state.edges.get_mut(&eid) {
                        edge.status = EdgeStatus::Superseded;
                        edge.superseded_by = Some(event.id);
                    }
                    let kind = edge_kind_from_payload(p);
                    // Edge-id derivation follows the assert-control scheme
                    // with the UNDERLYING kind (deterministic, never random).
                    let key = format!("control:{}:{}:{:?}", nominator.0, to.0, kind);
                    let new_id = EdgeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, key.as_bytes()));
                    state.edges.insert(
                        new_id,
                        EdgeState {
                            id: new_id,
                            kind,
                            from: nominator,
                            to,
                            percentage: opt_f64(p, "percentage"),
                            status: EdgeStatus::Asserted,
                            evidence_event_id: None,
                            originating_event_id: event.id,
                            trust_revocable: p.get("trust_revocable").and_then(|v| v.as_bool()),
                            superseded_by: None,
                            pierced_from: Some(eid),
                        },
                    );
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
use crate::fold::obligation::{obligation_id_from_payload, ObligationState};
use crate::lexicon::{LexiconEntry, Precondition};

/// The structure classes with an implemented `DeterminationStrategy` **today**
/// (T6.1(c) exemplar — `StructureClassSupported`, matrix rows 6a/8a).
///
/// **TOTAL as of TS.4** — every one of the 11 `StructureClass` variants is
/// served; no class remains fail-closed. Derived, not guessed:
/// `kyc_stream_ops.rs`'s `UboDeterminationFreeze::execute` dispatch
/// recognises exactly eight strategy names (`ownership_prong_strategy`,
/// `control_prong_strategy`, `trust_role_strategy`, `fund_control_strategy`,
/// `foundation_council_strategy`, `state_owned_strategy`,
/// `cooperative_member_strategy`, `nominee_pierce_strategy`). Build-out
/// history: `Trust` joined at TS.1 (`TrustRoleStrategy`,
/// EOP-DD-KYCUBO-KIT-TS0 §2.1); `InvestmentFund`/`Foundation` at TS.2
/// (§2.2/§2.3); `StateOwned`/`Cooperative` at TS.3 (§2.4/§2.5); `Nominee`
/// at TS.4 (§2.6 = K-8 — `ubo.edge.pierce-nominee` +
/// `NomineePierceStrategy`, whose unpierced-nominee guard fail-closes at
/// the freeze dispatch site). The guard itself (`StructureClassSupported`)
/// is retained: it now fail-closes the UNKNOWN-class case (a garbage wire
/// string folds to `None`) and any future `StructureClass` widening that
/// lands without a strategy. Widen this set ONLY when a new
/// `DeterminationStrategy` impl lands for the class — two teeth pin the
/// lockstep: the "seventh tooth"
/// (`tests/kyc_pack_closure.rs::precondition_and_strategy_coverage_is_exactly_known`,
/// strategy count) and the TS.1 split pin
/// (`implemented_class_split_matches_strategy_arms`, arm↔classes-served
/// mapping) — both must be consciously updated together with this const.
pub const IMPLEMENTED_STRATEGY_CLASSES: &[StructureClass] = &[
    StructureClass::PrivateCompany,
    StructureClass::MultiTierHoldingGroup,
    StructureClass::ListedEntity,
    StructureClass::LimitedPartnershipFund,
    StructureClass::Llp,
    StructureClass::Trust,
    StructureClass::InvestmentFund,
    StructureClass::Foundation,
    StructureClass::StateOwned,
    StructureClass::Cooperative,
    StructureClass::Nominee,
];

/// Check all preconditions for a verb against the current control state,
/// obligation state, and target binding **before** appending the event
/// (K-11, K-14). The unified two-fold checker (T6.1(a)): a single evaluator
/// sees both folds so a stud can read either axis — e.g. an obligation verb
/// reading `ControlState.registered`, or an obligation-basis stud reading
/// `ObligationState` — without forking the precondition discipline into two
/// parallel checkers.
pub fn check_preconditions(
    lexicon_entry: &LexiconEntry,
    control: &ControlState,
    obligation: &ObligationState,
    event: &IntentEvent,
) -> Result<(), KycError> {
    for pre in &lexicon_entry.preconditions {
        match pre {
            Precondition::EvidenceCited => {
                let eid = event.target.edge_id.ok_or_else(|| {
                    KycError::MissingTarget("edge_id required for EvidenceCited".into())
                })?;
                match control.edges.get(&eid) {
                    Some(e) if e.status == EdgeStatus::Evidenced => {} // ok
                    Some(e) => {
                        return Err(KycError::VerifyWithoutEvidence(eid, e.status.to_string()));
                    }
                    None => return Err(KycError::EdgeNotFound(eid)),
                }
            }
            Precondition::ReconciledProjection => {
                if !control.is_reconciled() {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "reconcile-conflict must fire before compute-fold / freeze".into(),
                    });
                }
            }
            Precondition::StrategySelected => {
                if !control.has_strategy() {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "select-strategy must fire before compute-fold / freeze".into(),
                    });
                }
            }
            Precondition::SubjectRegistered => {
                if !control.registered {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "subject must be registered (kyc.subject.register) before this verb"
                            .into(),
                    });
                }
            }
            Precondition::NotAlreadyRegistered => {
                // T6 row-9 fix (2026-08-17, corrects EOP-DD-KYCUBO-KIT-T6
                // §5): the bare `control.registered` flag can't distinguish
                // self-registration from the N natural-person candidate
                // registrations that share one subject_root (real
                // production shape — `kyc_m3_remediation.rs`'s multi-person
                // fixture) — key the check off the incoming event's own
                // `entity_id` instead. Same discipline as
                // `NoDuplicateActiveEdge` below: a probe with no
                // `entity_id` (`placement::probe_event`'s payload is always
                // `Null`) is vacuously satisfied, never vetoed.
                if let Some(eid) = entity_id(&event.payload, "entity_id") {
                    if control.registered_entity_ids.contains(&eid) {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!(
                                "entity {eid:?} is already registered under this subject; \
                                 re-registration of the same entity is not a supported move"
                            ),
                        });
                    }
                }
            }
            Precondition::StructureClassified => {
                if control.structure_class.is_none() {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "structure class must be set (kyc.subject.classify-structure) \
                                 before this verb"
                            .into(),
                    });
                }
            }
            Precondition::StructureClassSupported => match &control.structure_class {
                Some(class) if IMPLEMENTED_STRATEGY_CLASSES.contains(class) => {}
                Some(class) => {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "structure class {class:?} has no implemented determination \
                             strategy yet — fail-closed guard (see \
                             OwnershipProngStrategy/ControlProngStrategy scope notes; \
                             TS.0-TS.4 tracks the build-out)"
                        ),
                    });
                }
                None => {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "structure class must be set before this verb — no class means \
                                 no strategy can be confirmed supported"
                            .into(),
                    });
                }
            },
            Precondition::NoDuplicateActiveEdge => {
                let (from, to) = (
                    entity_id(&event.payload, "from_entity_id"),
                    entity_id(&event.payload, "to_entity_id"),
                );
                // T6.2 finding (2026-08-12): unlike every other precondition
                // variant, `NoDuplicateActiveEdge` reads `event.payload`, not
                // just target-addressed state — but this is "the same
                // oracle" (placement.rs's own docstring) the T2 placement
                // generator uses to PROBE legality with a generic/`Null`
                // payload (`placement::probe_event`), never the caller's
                // real one. Erroring here on missing from/to made
                // `assert-control`/`assert-economic-interest` permanently
                // unrecognisable through `enumerate_placement_set` (and
                // therefore through `KycWorkbook::stage()`, and the whole
                // T4.5 REPL surface — no workaround exists at that layer,
                // unlike the T4 Rust-level `manual_staged_move` escape
                // hatch), even for a state that would have genuinely
                // admitted the move. A REAL candidate can never reach this
                // arm with from/to actually absent — both assert verbs
                // declare `from_entity_id`/`to_entity_id` as required args,
                // enforced upstream of this checker — so "missing" only ever
                // means "this is an abstract probe, not a real candidate";
                // treating it as vacuously satisfied (skip, don't veto)
                // costs nothing at the real write path (still governed by
                // this exact check, still exercised end-to-end by
                // `kyc_t62_studs.rs`'s block/admit gates against the REAL
                // op) while restoring probe-based recognisability.
                let (Some(from), Some(to)) = (from, to) else {
                    continue;
                };
                let kind = if event.verb_fqn.as_str() == "ubo.edge.assert-economic-interest" {
                    EdgeKind::EconomicInterest
                } else {
                    edge_kind_from_payload(&event.payload)
                };
                let duplicate = control
                    .edges
                    .values()
                    .any(|e| e.is_active() && e.from == from && e.to == to && e.kind == kind);
                if duplicate {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "an active edge of kind {kind:?} already exists from {from:?} to \
                             {to:?}; use ubo.edge.supersede, never a contradicting assert (K-13)"
                        ),
                    });
                }
            }
            Precondition::EdgeExists => {
                let eid = event.target.edge_id.ok_or_else(|| {
                    KycError::MissingTarget("edge_id required for EdgeExists".into())
                })?;
                if !control.edges.contains_key(&eid) {
                    return Err(KycError::EdgeNotFound(eid));
                }
            }
            Precondition::EdgeActive => {
                let eid = event.target.edge_id.ok_or_else(|| {
                    KycError::MissingTarget("edge_id required for EdgeActive".into())
                })?;
                match control.edges.get(&eid) {
                    Some(e) if e.is_active() => {}
                    Some(_) => {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!("edge {eid:?} is superseded"),
                        });
                    }
                    None => return Err(KycError::EdgeNotFound(eid)),
                }
            }
            Precondition::ObligationExists => {
                let oid = obligation_id_from_payload(&event.payload).ok_or_else(|| {
                    KycError::MissingTarget("obligation_id required for ObligationExists".into())
                })?;
                if !obligation.obligations.contains_key(&oid) {
                    return Err(KycError::ObligationNotFound(oid));
                }
            }
            Precondition::SubjectNotDecided => {
                if let crate::fold::obligation::SubjectOverallState::Approved { .. }
                | crate::fold::obligation::SubjectOverallState::Rejected { .. } =
                    obligation.derive_subject_state(event.subject_root)
                {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "subject has already been decided (approved/rejected); the \
                                 decision is final (K-23)"
                            .into(),
                    });
                }
            }
            Precondition::SubjectAllTerminal => {
                if obligation.derive_subject_state(event.subject_root)
                    != crate::fold::obligation::SubjectOverallState::AllTerminal
                {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "not all required obligation tracks are terminal (K-23 gate)"
                            .into(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Thin delegate over [`check_preconditions`] for callers that don't need the
/// obligation axis yet (T6.1(a) — shrinks the diff for call sites where no
/// attached precondition reads `ObligationState`; the real append chokepoint
/// (`ob-poc-kyc-store::PgKycEventStore::append`) uses the two-fold function
/// directly with a genuinely folded `ObligationState`, never this delegate).
pub fn check_control_preconditions(
    lexicon_entry: &LexiconEntry,
    state: &ControlState,
    event: &IntentEvent,
) -> Result<(), KycError> {
    check_preconditions(lexicon_entry, state, &ObligationState::default(), event)
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

// ── Trust edge summary (for TrustRoleStrategy, TS.1) ─────────────────────────

/// An active trust-role edge ready for the role-based prong computation.
/// Consumed by `TrustRoleStrategy::resolve()` (EOP-DD-KYCUBO-KIT-TS0 §2.1).
#[derive(Debug, Clone)]
pub struct ReconciledTrustEdge {
    pub id: EdgeId,
    pub from: EntityId,
    pub to: EntityId,
    pub role: TrustRoleKind,
    /// Revocability proof carried on the edge (settlor edges only — see the
    /// polarity doc on `EdgeState::trust_revocable`).
    pub trust_revocable: Option<bool>,
    /// The event that verified this edge (K-35 traceability on the candidate).
    pub verified_by: Option<EventId>,
    /// The event that originally asserted this edge (deterministic; never random).
    pub originating_event_id: EventId,
}

/// Extract the active `EdgeKind::TrustRole(_)` edges from the control state —
/// the symmetric sibling of `reconciled_control_edges` for the trust-role
/// axis (TS.1). Per-role admissibility (trustee/protector always; settlor
/// unless proven irrevocable; beneficiary never) is the STRATEGY's ruling,
/// not this extractor's — this returns every active trust edge so the
/// strategy's exclusions stay visible in one place (`TrustRoleStrategy`).
pub fn reconciled_trust_edges(state: &ControlState) -> Vec<ReconciledTrustEdge> {
    state
        .edges
        .values()
        .filter(|e| e.is_active())
        .filter_map(|e| match &e.kind {
            EdgeKind::TrustRole(role) => Some(ReconciledTrustEdge {
                id: e.id,
                from: e.from,
                to: e.to,
                role: role.clone(),
                trust_revocable: e.trust_revocable,
                verified_by: if e.is_verified() {
                    e.evidence_event_id
                } else {
                    None
                },
                originating_event_id: e.originating_event_id,
            }),
            _ => None,
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
