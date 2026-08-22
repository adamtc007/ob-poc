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
use crate::fold::type_registry::TypeRegistryState;
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
    // ── TS.2 §5 — vocabulary convergence growth ─────────────────────────
    /// Officer / senior management appointment (TS.0 §3 pipe 6).
    OfficerAppointment,
    /// Management mandate — ManCo/AIFM/adviser with a governing mandate
    /// (TS.0 §3 pipe 7).
    ManagementMandate,
    /// Membership rights — cooperative/mutual (TS.0 §3 pipe 11).
    MembershipRights,
    /// Statutory authority — control by law, not ownership (TS.0 §3 pipe 12).
    StatutoryAuthority,
    /// Employment / delegated authority, natural-person-sourced (TS.0 §3
    /// pipe 14).
    Employment,
    /// Pooled-asset containment — umbrella → sub-fund (TS.0 §3 pipe 16).
    Containment,
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
    /// The event that superseded this edge (set by `kyc_ubo.assert.edge.supersession` —
    /// K-35 traceability on the supersession; TS.4, EOP-DD-KYCUBO-KIT-TS0
    /// §2.6: for a pierced nominee edge this is the `supersede` half of the
    /// `pierce-nominee` macro composition, TS.6 P2).
    #[serde(default)]
    pub superseded_by: Option<EventId>,
    /// Provenance: the nominee edge this edge was pierced FROM (set only
    /// when `kyc_ubo.assert.edge.control` is called with a `pierced-from` arg —
    /// TS.4 §2.6, K-8; the standalone `kyc_ubo.assert.edge.nominee-piercing` verb that
    /// originally set this field was retired TS.6 P2, folded into the
    /// `assert-control` + `supersede` macro composition, see
    /// `config/verb_schemas/macros/ubo.yaml`).
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

/// The subject's structure class, set by `kyc_ubo.assert.subject.structure-class`.
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
    /// Set if `kyc_ubo.assert.subject.structure-class` has fired.
    pub structure_class: Option<StructureClass>,
    /// Event that set the structure class (K-35 traceability).
    pub classify_event_id: Option<EventId>,
    /// Event id of the most-recent `kyc_ubo.assert.edge.reconciliation`.
    /// Required before `freeze` (K-14; `compute-fold` retired TS.6 P2).
    pub reconciliation_event_id: Option<EventId>,
    // `smo_person_id` / `smo_event_id` removed TS.6 §5 (2026-08-22) with
    // `ubo.determination.apply-smo-fallback`, their ONLY writer. Left in
    // place they would have been permanently `None` while two `match` sites
    // still branched on them — a fold-blind field pair, the same K-G7 class
    // as the retired verb itself. SMO now reaches a determination solely via
    // the traversal's pull-on-exhaustion, which populates `candidates`
    // (`Prong::SmoFallback`) — TS.3 §4a.
    /// Subject registration (if `kyc_ubo.assert.subject.register` has fired).
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

    /// True if a supported strategy is derivable from the structure class
    /// (K-4 precondition for fold/freeze). TS.6 P2: "strategy follows from
    /// entity type" (TS.0 §1) — retired `ubo.determination.select-strategy`,
    /// an explicit confirmation step that was redundant once
    /// `classify-structure` alone determines the strategy unambiguously
    /// (`strategy_for_structure_class`, the same 11-class/8-arm mapping
    /// `implemented_class_split_matches_strategy_arms` already pins).
    pub fn has_strategy(&self) -> bool {
        self.structure_class
            .as_ref()
            .is_some_and(|c| IMPLEMENTED_STRATEGY_CLASSES.contains(c))
    }
}

// ── Fold function ─────────────────────────────────────────────────────────────

/// Parse an `EdgeId` from a named field of the event payload.
fn edge_id_field(payload: &serde_json::Value, field: &str) -> Option<EdgeId> {
    payload
        .get(field)?
        .as_str()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(EdgeId)
}

/// Parse an `EdgeId` from the event payload (field `"edge_id"`).
fn edge_id_from_payload(payload: &serde_json::Value) -> Option<EdgeId> {
    edge_id_field(payload, "edge_id")
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

/// The canonical `kind` wire-string set for `kyc_ubo.assert.edge.control` —
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
    // TS.2 §5 growth — closes `every_geometry_pipe_is_assertable` for the
    // six pipes previously declared in geometry but unassertable.
    "officer_appointment",
    "management_mandate",
    "membership_rights",
    "statutory_authority",
    "employment",
    "containment",
];

fn edge_kind_from_payload(payload: &serde_json::Value) -> EdgeKind {
    match payload.get("kind").and_then(|v| v.as_str()) {
        Some(wire) => edge_kind_from_wire(wire),
        // Total-dispatch backstop for HISTORICAL events only (the fold must
        // stay infallible — D2). The live append path can no longer reach
        // this with an absent kind: the op-normalizer rejects anything
        // outside `EDGE_KIND_WIRE_VALUES` before the event exists (TS.1 §1b).
        None => EdgeKind::DominantInfluence,
    }
}

/// Wire-string → `EdgeKind`, total over `EDGE_KIND_WIRE_VALUES` plus the
/// same historical-event backstop as `edge_kind_from_payload` (which now
/// delegates here). `pub(crate)` so `placement.rs`'s R2 existence check
/// (TS.5) can iterate every assertable kind without constructing throwaway
/// JSON payloads.
pub(crate) fn edge_kind_from_wire(wire: &str) -> EdgeKind {
    match wire {
        "economic_interest" => EdgeKind::EconomicInterest,
        "voting_rights" => EdgeKind::VotingRights,
        "board_appointment" => EdgeKind::BoardAppointment,
        "gp_statutory" => EdgeKind::GpStatutory,
        "designated_member" => EdgeKind::DesignatedMember,
        "trust_settlor" => EdgeKind::TrustRole(TrustRoleKind::Settlor),
        "trust_trustee" => EdgeKind::TrustRole(TrustRoleKind::Trustee),
        "trust_protector" => EdgeKind::TrustRole(TrustRoleKind::Protector),
        "trust_beneficiary" => EdgeKind::TrustRole(TrustRoleKind::Beneficiary),
        "nominee" => EdgeKind::Nominee,
        "officer_appointment" => EdgeKind::OfficerAppointment,
        "management_mandate" => EdgeKind::ManagementMandate,
        "membership_rights" => EdgeKind::MembershipRights,
        "statutory_authority" => EdgeKind::StatutoryAuthority,
        "employment" => EdgeKind::Employment,
        "containment" => EdgeKind::Containment,
        // Covers "dominant_influence" and any other string. See
        // `edge_kind_from_payload`'s doc: unreachable from the live append
        // path (op-normalizer fail-closed gate); kept total for
        // historical-event replay.
        _ => EdgeKind::DominantInfluence,
    }
}

// ── Type geometry at the write path (EOP-DD-KYCUBO-TS.5 R1/R2) ────────────────

/// Extract the (from, kind, to) triple a geometry-gated event asserts, per
/// verb payload shape (TS.5 §6 Q1/Q2). Shared by `check_preconditions`'s
/// `TypeGeometryPermits` arm (the append/preview chokepoint) and
/// `placement.rs`'s R2 existence check — ONE extraction, not two.
fn geometry_triple_for_event(event: &IntentEvent) -> Option<(EntityId, EntityId, EdgeKind)> {
    match event.verb_fqn.as_str() {
        "kyc_ubo.assert.edge.economic-interest" => entity_id(&event.payload, "from_entity_id")
            .zip(entity_id(&event.payload, "to_entity_id"))
            .map(|(from, to)| (from, to, EdgeKind::EconomicInterest)),
        // "kyc_ubo.assert.edge.nominee-piercing" special case RETIRED (TS.6 P2, K-G7):
        // the old bespoke verb derived `to` from the pierced edge (needing
        // the folded `ControlState`, since its own payload never carried
        // `to_entity_id`). The macro composition that replaced it
        // (`config/verb_schemas/macros/ubo.yaml`) issues an ordinary
        // `kyc_ubo.assert.edge.control` call with an explicit `to_entity_id` —
        // indistinguishable from any other assert-control call, so it falls
        // through to the generic arm below with no special-casing and no
        // `ControlState` lookup needed (the now-unused `control` parameter
        // was removed with this arm). A historical event still bearing the
        // retired verb_fqn falls through too and evaluates as `None` —
        // `Unevaluable`, which admits — not a crash.
        _ => entity_id(&event.payload, "from_entity_id")
            .zip(entity_id(&event.payload, "to_entity_id"))
            .map(|(from, to)| (from, to, edge_kind_from_payload(&event.payload))),
    }
}

/// Outcome of evaluating type geometry for one (from, kind, to) triple.
/// `Unevaluable` and `Permitted` both ADMIT (R5/R6) — only `Refused`
/// blocks. Kept as three distinct outcomes (not a `bool`) so a caller can
/// tell "affirmatively legal" from "could not be evaluated, admitted by
/// CTN-2e default" — the same distinction `ProvisionalityReason` preserves
/// downstream at determination time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeometryEvaluation {
    Permitted,
    /// Either endpoint's type is unasserted, or the edge's `Pipe`
    /// classification did not resolve (`pipe_of` returned `provisional`).
    Unevaluable,
    Refused(crate::geometry::GeometryError),
}

/// TS.5 R1/R2 — the single geometry evaluation both the write-path
/// precondition and the board preview call. Pure: no store, no clock.
pub(crate) fn evaluate_type_geometry(
    from: EntityId,
    to: EntityId,
    kind: &EdgeKind,
    type_registry: &TypeRegistryState,
) -> GeometryEvaluation {
    let (Some(from_type), Some(to_type)) = (type_registry.type_of(from), type_registry.type_of(to))
    else {
        return GeometryEvaluation::Unevaluable;
    };
    let classification = crate::geometry::pipe_of(kind, Some(to_type));
    let Some(pipe) = classification.pipe else {
        return GeometryEvaluation::Unevaluable;
    };
    match crate::geometry::check_type_geometry(
        crate::geometry::LinkageSource::Entity(from_type),
        pipe,
        to_type,
    ) {
        Ok(()) => GeometryEvaluation::Permitted,
        Err(e) => GeometryEvaluation::Refused(e),
    }
}

/// The canonical `structure-class` wire-string set for
/// `kyc_ubo.assert.subject.structure-class` — exactly the strings
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
        "kyc_ubo.assert.subject.register" => {
            state.registered = true;
            state.register_event_id = Some(event.id);
            if let Some(eid) = entity_id(p, "entity_id") {
                state.registered_entity_ids.insert(eid);
            }
        }

        "kyc_ubo.assert.subject.structure-class" => {
            state.structure_class = structure_class_from_payload(p);
            state.classify_event_id = Some(event.id);
        }

        "kyc_ubo.assert.edge.economic-interest" => {
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

        "kyc_ubo.assert.edge.control" => {
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
                        // TS.6 P2 (K-G7): set only when this assert-control
                        // call is the first half of the `pierce-nominee`
                        // macro composition (op-layer stamps `pierced_from`
                        // from the caller's `pierced-from` arg).
                        pierced_from: edge_id_field(p, "pierced_from"),
                    },
                );
            }
        }

        "kyc_ubo.assert.edge.evidence" => {
            if let Some(eid) = edge_id_from_target(event) {
                if let Some(edge) = state.edges.get_mut(&eid) {
                    if edge.status == EdgeStatus::Asserted {
                        edge.status = EdgeStatus::Evidenced;
                        edge.evidence_event_id = Some(event.id);
                    }
                }
            }
        }

        "kyc_ubo.assert.edge.verification" => {
            if let Some(eid) = edge_id_from_target(event) {
                if let Some(edge) = state.edges.get_mut(&eid) {
                    if edge.status == EdgeStatus::Evidenced {
                        edge.status = EdgeStatus::Verified;
                    }
                }
            }
        }

        "kyc_ubo.assert.edge.supersession" => {
            // K-13: supersede-never-delete.
            if let Some(eid) = edge_id_from_target(event) {
                if let Some(edge) = state.edges.get_mut(&eid) {
                    edge.status = EdgeStatus::Superseded;
                    edge.superseded_by = Some(event.id);
                }
            }
        }

        // "kyc_ubo.assert.edge.nominee-piercing" RETIRED (TS.6 P2, K-G7, 2026-08-22): the
        // bespoke two-effect fold arm is gone — the same two effects (assert
        // the nominator's real edge with `pierced_from` provenance +
        // supersede the nominee edge) are now two ordinary fold arms above
        // (`kyc_ubo.assert.edge.control`'s `pierced_from` read) and below
        // (`kyc_ubo.assert.edge.supersession`), composed by the `kyc_ubo.assert.edge.nominee-piercing`
        // MACRO (config/verb_schemas/macros/ubo.yaml), not a single event
        // under this verb_fqn. A historical event still bearing this
        // verb_fqn falls through to the catch-all `_ => {}` below, a
        // no-op — replay-faithful for events already in the stream (Q7/
        // K-18/K-31), same discipline as select-strategy/compute-fold's
        // retirement.

        "kyc_ubo.assert.edge.reconciliation" => {
            state.reconciliation_event_id = Some(event.id);
        }

        // TS.6 P2 (K-G7): "ubo.determination.select-strategy" retired —
        // strategy is now derived from `structure_class`
        // (`strategy_for_structure_class`), never asserted. A historical
        // event under this verb_fqn falls through to the catch-all `_ =>
        // {}` below, a no-op — replay-faithful for events already in the
        // stream, no special handling needed (Q7/K-18/K-31).
        // TS.6 §5 (K-G7, RATIFIED): "ubo.determination.apply-smo-fallback"
        // fold arm RETIRED 2026-08-22 with the verb. `smo_person_id` /
        // `smo_event_id` therefore stay `None` for good: the ONLY writer is
        // gone, and SMO now reaches a determination solely via the
        // traversal's pull-on-exhaustion, which populates `candidates`
        // (Prong::SmoFallback) rather than these fields (TS.3 §4a). The 54
        // historical events under this verb_fqn fall through to the
        // catch-all `_ => {}` below — replay-faithful, and provably
        // determination-neutral: none of those 54 subjects carries a freeze,
        // so no determination was ever computed from a manual SMO.

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
/// at TS.4 (§2.6 = K-8 — piercing via the `kyc_ubo.assert.edge.nominee-piercing` macro
/// (assert-control + supersede, TS.6 P2 — see `config/verb_schemas/macros/
/// ubo.yaml`) + `NomineePierceStrategy`, whose unpierced-nominee guard
/// fail-closes at the freeze dispatch site). The guard itself
/// (`StructureClassSupported`)
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

/// TS.6 P2 — "strategy follows from entity type" (TS.0 §1), realised: the
/// SAME class→strategy mapping already ratified and pinned by
/// `tests/kyc_pack_closure.rs::implemented_class_split_matches_strategy_arms`
/// (11 classes, 8 arms, TOTAL — every class implemented has exactly one
/// strategy), now promoted from test-only pinned data into the actual
/// freeze-dispatch source. Retires `ubo.determination.select-strategy`
/// (TS.6 §5, K-G7): an explicit human "confirm the strategy" step is
/// redundant once `classify-structure` alone determines it unambiguously —
/// there was never a second legitimate strategy for any of the 11 classes
/// to choose between. `IMPLEMENTED_STRATEGY_CLASSES` still gates the
/// fail-closed guard for any class with no arm; this function is total only
/// over that guarded set, by construction (an unimplemented class never
/// reaches here — `StructureClassSupported` refuses first).
pub fn strategy_for_structure_class(class: &StructureClass) -> &'static str {
    match class {
        StructureClass::PrivateCompany
        | StructureClass::MultiTierHoldingGroup
        | StructureClass::ListedEntity => "ownership_prong_strategy",
        StructureClass::LimitedPartnershipFund | StructureClass::Llp => "control_prong_strategy",
        StructureClass::Trust => "trust_role_strategy",
        StructureClass::InvestmentFund => "fund_control_strategy",
        StructureClass::Foundation => "foundation_council_strategy",
        StructureClass::StateOwned => "state_owned_strategy",
        StructureClass::Cooperative => "cooperative_member_strategy",
        StructureClass::Nominee => "nominee_pierce_strategy",
    }
}

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
    type_registry: &TypeRegistryState,
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
                        reason: "reconcile-conflict must fire before freeze".into(),
                    });
                }
            }
            Precondition::SubjectRegistered => {
                if !control.registered {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "subject must be registered (kyc_ubo.assert.subject.register) before this verb"
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
                        reason: "structure class must be set (kyc_ubo.assert.subject.structure-class) \
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
                let kind = if event.verb_fqn.as_str() == "kyc_ubo.assert.edge.economic-interest" {
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
                             {to:?}; use kyc_ubo.assert.edge.supersession, never a contradicting assert (K-13)"
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
            Precondition::EntityRegistered => {
                // TS.1 §3 rows 2/6/7 ("entity exists"). Vacuous when the
                // probe carries no `entity_id` — same convention as
                // `NoDuplicateActiveEdge`/`NotAlreadyRegistered` above: a
                // real call site always supplies `target.entity_id`
                // (declared required on assert-type/correct-type/
                // withdraw-member); absence only ever means an abstract
                // subject-scoped probe, never a real candidate.
                if let Some(EntityId(eid)) = event.target.entity_id {
                    if !control.registered_entity_ids.contains(&EntityId(eid)) {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!(
                                "entity {eid:?} is not a registered group member \
                                 (kyc_ubo.assert.subject.register) — TS.1 §3 rows 2/6/7"
                            ),
                        });
                    }
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
            Precondition::MembershipActive => {
                // TS.1 §3 row 6 ("membership must be active" — not already
                // withdrawn). Vacuous when the probe carries no `entity_id`,
                // same convention as `EntityRegistered` above.
                if let Some(EntityId(eid)) = event.target.entity_id {
                    if type_registry.is_withdrawn(EntityId(eid)) {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!(
                                "entity {eid:?} is already withdrawn — membership must be \
                                 active (TS.1 §3 row 6)"
                            ),
                        });
                    }
                }
            }
            Precondition::PriorTypeAsserted => {
                // TS.1 §3 row 7 ("a type was already asserted" — nothing to
                // correct otherwise, that's assert-type's job). Vacuous when
                // the probe carries no `entity_id`, same convention as
                // `EntityRegistered` above.
                if let Some(EntityId(eid)) = event.target.entity_id {
                    if type_registry.type_of(EntityId(eid)).is_none() {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!(
                                "entity {eid:?} has no prior type assertion to correct — use \
                                 kyc_ubo.assert.subject.type instead (TS.1 §3 row 7)"
                            ),
                        });
                    }
                }
            }
            Precondition::TypeGeometryPermits => {
                // TS.5 R1 — TS.1 §1's FIRST constraint layer, applied to the
                // exact (from, kind, to) triple this event asserts. Extracted
                // as `geometry_triple_for_event`/`evaluate_type_geometry`
                // (below) so `placement.rs`'s R2 preview check can call the
                // IDENTICAL logic instead of a hand-rolled duplicate — one
                // chokepoint, reused, not two implementations to keep in sync.
                let Some((from, to, kind)) = geometry_triple_for_event(event) else {
                    // Vacuous when the probe carries no resolvable endpoints
                    // — same convention as `NoDuplicateActiveEdge`/
                    // `EntityRegistered` above.
                    continue;
                };
                if let GeometryEvaluation::Refused(geo_err) =
                    evaluate_type_geometry(from, to, &kind, type_registry)
                {
                    return Err(KycError::GeometryRefused {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!("{from:?} --{kind:?}--> {to:?} is not a move (TS.1 §2/§2a): {geo_err:?}"),
                    });
                }
                // R5/R6 (`Unevaluable`/`Permitted`): an alleged OR untyped
                // endpoint, or an unresolved pipe classification, ADMITS
                // provisionally — this precondition must never fail closed
                // on missing proof (CTN-2e). Provisionality is recorded
                // downstream, at determination time, by
                // `determination::compute_assurance`
                // (`ProvisionalityReason::AllegedType`/`GeometryUnevaluable`),
                // not here — this checker has no side channel to record into.
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
    type_registry: &TypeRegistryState,
    event: &IntentEvent,
) -> Result<(), KycError> {
    check_preconditions(lexicon_entry, state, &ObligationState::default(), type_registry, event)
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

/// The four-way admission class a control-kind edge falls into
/// (EOP-DD-KYCUBO-TS.3 §4, RATIFIED 2026-08-21).
///
/// Supersedes the boolean `is_admitted_as_control` (landed `c7ef69ca`) — TS.3
/// found that boolean one distinction short: `Stop` (traversal halts,
/// recording why — `StatutoryAuthority`) is not the same thing as
/// `NotControl` (never part of the walk — `OfficerAppointment` et al.), and
/// neither is a `bool`'s "false" arm strong enough to say which. `Pierce`
/// (`Nominee`) is listed for completeness (TS.3 §4) — it is a traversal rule
/// available during any strategy (TS.0 §5), not a member of the admission
/// set `reconciled_control_edges` filters to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlAdmission {
    /// Walk through; continue to the next node.
    Traverse,
    /// Traversal halts here; the caller must record why (K-8-style
    /// discipline — not a silent halt). See `detect_statutory_stops`
    /// (`determination.rs`).
    Stop,
    /// Never part of the control walk. May still be a terminal/population
    /// source pulled by a strategy on exhaustion (§4a — `OfficerAppointment`),
    /// never by admission.
    NotControl,
    /// Substitute the underlying holder and continue (K-8). Not filtered by
    /// `reconciled_control_edges` — pierce-and-substitute is handled by the
    /// `kyc_ubo.assert.edge.nominee-piercing` macro composition (TS.6 P2), before any
    /// strategy ever runs.
    Pierce,
}

/// The TS.3 §3/§4 admission ruling, per `EdgeKind`.
///
/// **This is a SAFETY guard, not a semantics ruling.** Before this function
/// existed (as the boolean `is_admitted_as_control`, landed `c7ef69ca`), the
/// filter was an EXCLUSION list — everything except
/// `EconomicInterest`/`Nominee` passed — which silently admitted every
/// *future* `EdgeKind` variant as generic control the moment it became
/// assertable, with no domain ruling behind the admission. TS.3 closes the
/// semantics question the whitelist tranche deliberately left open,
/// reconciling to V&S v0.6 §6.4's control-axis column rather than inventing:
/// `ManagementMandate` and `MembershipRights` are named as control axes for
/// funds and cooperatives respectively and move to `Traverse`;
/// `StatutoryAuthority` routes to SMO/special-handling per §6.4's
/// state-owned row, so it `Stop`s rather than being walked or silently
/// dropped; `OfficerAppointment` stays out of the WALK (ruled 2026-08-21 —
/// "may not be needed or relevant") but is available for the §4a pull on
/// exhaustion; `Employment`/`Containment` stay out (obligation basis /
/// structural scoping, not control, per §3); `EconomicInterest` is out
/// because that's the ownership prong's job (`reconciled_economic_edges`);
/// `Nominee` is `Pierce`, never a plain admission. The match is exhaustive
/// with NO catch-all arm: adding a new `EdgeKind` variant anywhere in this
/// enum is a compile error here, never a silent admission.
pub fn control_admission(kind: &EdgeKind) -> ControlAdmission {
    use ControlAdmission::*;
    match kind {
        EdgeKind::VotingRights
        | EdgeKind::BoardAppointment
        | EdgeKind::GpStatutory
        | EdgeKind::DesignatedMember
        | EdgeKind::TrustRole(_)
        | EdgeKind::DominantInfluence
        | EdgeKind::ManagementMandate
        | EdgeKind::MembershipRights => Traverse,
        EdgeKind::StatutoryAuthority => Stop,
        EdgeKind::EconomicInterest
        | EdgeKind::OfficerAppointment
        | EdgeKind::Employment
        | EdgeKind::Containment => NotControl,
        EdgeKind::Nominee => Pierce,
    }
}

/// Extract the reconciled (active) control edges from the control state —
/// the `Traverse`-classified edges (`control_admission`), active. Used by
/// `ControlProngStrategy` (M4) and its delegates/siblings. Nothing here
/// gates on `EdgeStatus::Verified` — a merely-`Asserted` edge still
/// traverses (TS.3 §2a: a determination runs at any board state; what
/// changes is not whether it is produced but how well it is known).
pub fn reconciled_control_edges(state: &ControlState) -> Vec<ReconciledControlEdge> {
    state
        .edges
        .values()
        .filter(|e| control_admission(&e.kind) == ControlAdmission::Traverse && e.is_active())
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

/// Active edges of `kind`, active, pointed at `target` — the shared shape
/// `detect_statutory_stops`/`pull_smo_on_exhaustion` (`determination.rs`,
/// TS.3 §3/§4a) both need: a `Stop`- or `NotControl`-classified kind is by
/// definition excluded from `reconciled_control_edges`, so those two
/// call sites need their own direct scan rather than the Traverse-only set.
pub fn edges_of_kind_into(
    state: &ControlState,
    kind: EdgeKind,
    target: EntityId,
) -> Vec<ReconciledControlEdge> {
    state
        .edges
        .values()
        .filter(|e| e.kind == kind && e.to == target && e.is_active())
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

/// TS.4 §2 Ruling A: active edges, pointed at `target`, whose kind is a
/// governing-mandate BASIS — `ManagementMandate` (ManCo/AIFM/adviser) or
/// `GpStatutory` (GP-of-LP; TS.2 Ruling 2 "basis, not the label 'ManCo'" —
/// a limited partnership's governing mandate is its general partner's
/// statutory control, not a separate management-contract edge). Both kinds
/// are already `ControlAdmission::Traverse` (TS.3), so this is a
/// kind-filtered view of the same edges `reconciled_control_edges` admits —
/// not a new admission class — used by `fund_pivot_resolve` to find WHERE
/// a subject re-anchors before delegating to the shared control walk.
pub fn governing_mandate_edges_into(
    state: &ControlState,
    target: EntityId,
) -> Vec<ReconciledControlEdge> {
    state
        .edges
        .values()
        .filter(|e| {
            matches!(e.kind, EdgeKind::ManagementMandate | EdgeKind::GpStatutory)
                && e.to == target
                && e.is_active()
        })
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

/// TS.4 §3 Ruling B: every active, unpierced `EdgeKind::Nominee` edge in
/// the ENTIRE control state — not scoped to any one strategy's traversal.
/// Ruling B widens the freeze-time K-8 guard from "only when
/// `nominee_pierce_strategy` was selected" to unconditional: a nominee
/// sitting mid-chain inside a fund, trust, or corporate structure must be
/// pierced before freeze succeeds under ANY strategy, not only when the
/// subject itself classified as `Nominee`. Extracted here (rather than left
/// inline at the op-layer call site) so the same check is directly testable
/// without a DB (`determination_never_terminates_at_a_nominee`).
pub fn unpierced_nominee_edges(state: &ControlState) -> Vec<EdgeId> {
    state
        .edges
        .values()
        .filter(|e| e.is_active() && matches!(e.kind, EdgeKind::Nominee))
        .map(|e| e.id)
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
        if event.verb_fqn.as_str() == "kyc_ubo.assert.subject.register"
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
