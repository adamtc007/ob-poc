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
use crate::EntityType;

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

// ── Proof kinds (EOP-DD-UBO-PROOF-001 §2, RATIFIED 2026-08-28) ─────────────────

/// A proof is a **kind, a source, and a date** (§1) — not a document with a
/// type attached. Seven kinds, exhaustive: adding one is a compile error
/// until ruled (D3's compile-time-guarantee pattern, matching
/// `DeterminationDispatch`'s exhaustive match over `EntityType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProofKind {
    /// A companies registry, GLEIF, a regulator's register — a lookup, not
    /// necessarily a document.
    RegistryExtract,
    /// Articles, trust deed, partnership agreement, foundation charter —
    /// what the vehicle *is* and how it is governed.
    ConstitutionalDocument,
    /// The register itself, or a certified extract — who holds what.
    ShareRegister,
    /// Board minute, appointment filing, statutory return — an act
    /// recorded with an authority.
    FiledDocument,
    /// Management agreement, nominee declaration, mandate — a relationship
    /// created by agreement.
    Contract,
    /// Passport, national identity document — natural persons.
    IdentityDocument,
    /// Certification by a regulated third party, or by the client —
    /// someone stands behind a fact.
    Attestation,
}

/// Wire strings for `ProofKind`, in declaration order — the op-layer
/// normalizer's `valid_values` source (CLAUDE.md: "Selector-arg
/// `valid_values` is mandatory, always the wire string").
pub const PROOF_KIND_WIRE_VALUES: &[&str] = &[
    "registry-extract",
    "constitutional-document",
    "share-register",
    "filed-document",
    "contract",
    "identity-document",
    "attestation",
];

/// Wire-string → `ProofKind`, total over `PROOF_KIND_WIRE_VALUES`. Fail-closed
/// on anything else (`None`) — same discipline as `entity_type_from_wire`,
/// never a silent best-effort guess (D2 corrective tranche precedent).
pub fn proof_kind_from_wire(wire: &str) -> Option<ProofKind> {
    match wire {
        "registry-extract" => Some(ProofKind::RegistryExtract),
        "constitutional-document" => Some(ProofKind::ConstitutionalDocument),
        "share-register" => Some(ProofKind::ShareRegister),
        "filed-document" => Some(ProofKind::FiledDocument),
        "contract" => Some(ProofKind::Contract),
        "identity-document" => Some(ProofKind::IdentityDocument),
        "attestation" => Some(ProofKind::Attestation),
        _ => None,
    }
}

/// One proof logged against an assertion (§1): a kind, a source (free text
/// today — §6 Q3: "structure when a check needs to read it"), and a date
/// (when it was obtained — distinct from the citing event's own
/// `committed_at`; an analyst may log today evidence of a lookup performed
/// last week, or a document dated months ago). Keyed in its owning
/// collection by `event_id` — the citing event IS the citation `retract`
/// targets (§3.1's move table: "`retract` | that proof no longer stands |
/// group, citation").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofRecord {
    pub kind: ProofKind,
    pub source: String,
    pub date: String,
    pub event_id: EventId,
}

/// Build a `ProofRecord` from an `evidence` event's payload, keyed by the
/// event's own id (§3.1's move table: the citing event IS the citation).
/// `None` if `kind` is absent or unrecognized — tolerant of the 5 real
/// pre-T5 `evidence` events (P0c census), whose payloads predate this
/// concept entirely (`{"doc_id": ...}` or `{}`); replaying them logs no
/// proof, which is honest (they logged nothing a proof kind could name),
/// never fabricated (K-35). `source`/`date` default to empty string when
/// absent — a proof with a known kind but an unrecorded source/date is
/// still a fact worth keeping, unlike a proof with no kind at all.
pub(crate) fn proof_record_from_payload(
    p: &serde_json::Value,
    event_id: EventId,
) -> Option<ProofRecord> {
    let kind = p.get("kind")?.as_str().and_then(proof_kind_from_wire)?;
    let source = p
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let date = p
        .get("date")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    Some(ProofRecord {
        kind,
        source,
        date,
        event_id,
    })
}

// ── Edge epistemic status ─────────────────────────────────────────────────────

/// Derived by the fold from the sequence of events touching an edge.
/// **Not stored; not settable.** (K-11, Q5 — §4.1 design invariant.)
///
/// EOP-DD-UBO-PROOF-001 §4 (T5, 2026-08-28): `Evidenced`/`Verified` are
/// GONE — "the board collects facts; the policy rules on adequacy." What an
/// edge has instead is its `proofs` set (below); status now distinguishes
/// only whether the edge is on the board at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EdgeStatus {
    /// `connect` event seen; the edge is on the board.
    Asserted,
    /// `disconnect` event seen (never removed — K-13).
    Superseded,
}

impl std::fmt::Display for EdgeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeStatus::Asserted => write!(f, "Asserted"),
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
    /// Derived status — fold output only (K-11). On-board vs. superseded
    /// ONLY (EOP-DD-UBO-PROOF-001 §4, T5) — proof status lives in `proofs`.
    pub status: EdgeStatus,
    /// The proofs logged against this edge (EOP-DD-UBO-PROOF-001 §1/§4),
    /// keyed by the citing `evidence` event's id — the "citation"
    /// `retract` removes by (§3.1's move table: "group, citation"). An
    /// empty map is a fact ("nothing logged"), not a status.
    #[serde(default)]
    pub proofs: BTreeMap<EventId, ProofRecord>,
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

    /// EOP-DD-UBO-PROOF-001 §4 (T5): existence, not adequacy — "at least
    /// one proof has been logged," the same fact TS.2 Ruling 2f gates
    /// `freeze` on (a fund pivot must rest on an established mandate; K-1
    /// makes the basis mandatory). Never asks WHICH kind, or how many.
    pub fn has_proof(&self) -> bool {
        !self.proofs.is_empty()
    }

    /// EOP-DD-UBO-DISPATCH-001 §3a (T4, 2026-08-28): `Containment` carries
    /// economic weight too. Q2 rules an umbrella "holds participation
    /// shares in the funds it pools" — a holding, economic, no control —
    /// and P0's trace confirmed T3 had encoded pipe 16
    /// (`Pipe::PooledAssetContainment`) as a pure scoping boundary
    /// (excluded here, `NotControl` in `control_admission`), matching
    /// TS.0's description but contradicting Q2. The pipe keeps its name and
    /// position in TS.0's 17-pipe catalogue — only what it economically
    /// MEANS changes ("it changes what the pipe IS rather than what it is
    /// called," §3a). `control_admission`'s `NotControl` classification
    /// for `Containment` is unchanged (Q2: "carries no control").
    pub fn is_economic(&self) -> bool {
        matches!(self.kind, EdgeKind::EconomicInterest | EdgeKind::Containment)
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
    // `reconciliation_event_id` REMOVED (EOP-VS-UBO-GAME-001 T3, §3.3,
    // 2026-08-27) with `kyc_ubo.assert.edge.reconciliation`, its sole
    // writer, and `Precondition::ReconciledProjection`/the K-14 gate on
    // freeze, its sole reader. See fold/control.rs's former reconciliation
    // fold arm (deleted) for the full reasoning.
    // `smo_person_id` / `smo_event_id` removed TS.6 §5 (2026-08-22) with
    // `ubo.determination.apply-smo-fallback`, their ONLY writer. Left in
    // place they would have been permanently `None` while two `match` sites
    // still branched on them — a fold-blind field pair, the same K-G7 class
    // as the retired verb itself. SMO now reaches a determination solely via
    // the traversal's pull-on-exhaustion, which populates `candidates`
    // (`Prong::SmoFallback`) — TS.3 §4a.
    /// Subject registration (if `kyc_ubo.assert.subject.place` has fired).
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

    // `is_reconciled()` REMOVED (EOP-VS-UBO-GAME-001 T3, §3.3) with
    // `reconciliation_event_id` above — see that field's retirement comment.

    // `has_strategy()` RETIRED (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28)
    // alongside `IMPLEMENTED_STRATEGY_CLASSES`/`strategy_for_structure_class`
    // — superseded by `dispatch_for_entity_type` (below), which the
    // `EntityTypeSupportsStrategy` precondition and
    // `determination::recover_determination_at` both call directly against
    // `TypeRegistryState`, not `ControlState.structure_class`.
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

/// Parse the `EventId` of the proof `retract` targets (§3.1's move table:
/// "group, citation" — the citation IS the id of the `evidence` event that
/// logged it).
pub(crate) fn citation_event_id_from_payload(p: &serde_json::Value) -> Option<EventId> {
    p.get("citation_id")?
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .map(EventId)
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
    // "kyc_ubo.assert.edge.economic-interest" special case RETIRED
    // (EOP-VS-UBO-GAME-001 T3, §3.2): the old FQN's payload never carried
    // `kind` (it was implicitly always EconomicInterest), so this used to
    // be a per-FQN match with a hardcoded arm. `connect` (the live FQN,
    // §3.2) always carries `kind` — including "economic_interest" as an
    // ordinary value — so every verb_fqn now reads the same way, with no
    // special-casing needed. P0c census: 0 real committed events under the
    // old FQN, so there is no historical stream this special case would
    // still need to serve.
    // "kyc_ubo.assert.edge.nominee-piercing" special case RETIRED (TS.6 P2, K-G7):
    // the old bespoke verb derived `to` from the pierced edge (needing the
    // folded `ControlState`, since its own payload never carried
    // `to_entity_id`). The macro composition that replaced it
    // (`config/verb_schemas/macros/ubo.yaml`) issues an ordinary
    // `kyc_ubo.assert.edge.connect` call with an explicit `to_entity_id` —
    // indistinguishable from any other connect call, so it reads the same
    // generic way, with no special-casing and no `ControlState` lookup
    // needed (the now-unused `control` parameter was removed with that
    // arm). A historical event still bearing the retired verb_fqn reads the
    // same way too and evaluates as `None` — `Unevaluable`, which admits —
    // not a crash.
    entity_id(&event.payload, "from_entity_id")
        .zip(entity_id(&event.payload, "to_entity_id"))
        .map(|(from, to)| (from, to, edge_kind_from_payload(&event.payload)))
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

// `STRUCTURE_CLASS_WIRE_VALUES` RETIRED (EOP-DD-UBO-DISPATCH-001 T4,
// 2026-08-28) alongside the `kyc_ubo.assert.subject.structure-class` verb —
// it gated ONLY the op-side write-path validation in
// `ob-poc-kyc-seam::canonical`, which is gone with the verb.
//
// `structure_class_from_payload` and the fold arm that called it REMOVED
// (EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07, Q2): R5 required keeping
// this arm to preserve replay fidelity over a REAL pre-existing stream
// (10 committed events, P0 census). R5 itself is unchanged and governs
// everything written from the epoch forward (Adam, 2026-09-07) — but its
// premise here (a stream to replay) is permanently false after the clean
// start cleared it. `ControlState.structure_class`/`classify_event_id`
// stay on the struct (read by `evaluation.rs`'s `StructureClassPresent`
// applicability condition, out of this tranche's scope — the
// inspect-game phase) but are now permanently `None`: no code anywhere
// sets them, the same structural dead-end `JurisdictionPresent`/
// `RiskAtOrAbove` already are (disclosed, not fixed — belongs to the
// inspect-game phase, EOP-DD-UBO-CLEANOUT-001 T6 P0c).

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
        // `kyc_ubo.assert.subject.register`'s R5-historical arm REMOVED
        // (EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07, Q2) — identical
        // effect to `place` below, so removing it has no live consequence;
        // R5's replay-fidelity premise (a real stream to replay) is
        // permanently false after the clean start. A historical event
        // bearing this FQN now falls through to the catch-all `_ => {}`.
        //
        // §3.2: `place` absorbs register + assert-type — one event, both
        // axes. This arm writes the ControlGraph axis (membership);
        // `fold::type_registry::apply_one_type_registry_event` writes the
        // TypeRegistry axis (type + un-withdraws on re-placement) from the
        // SAME event, independently (T6.1(a) — two pure folds over one stream).
        "kyc_ubo.assert.subject.place" => {
            state.registered = true;
            state.register_event_id = Some(event.id);
            if let Some(eid) = entity_id(p, "entity_id") {
                state.registered_entity_ids.insert(eid);
            }
        }

        // `kyc_ubo.assert.subject.structure-class`'s R5-historical arm
        // REMOVED (EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07, Q2) — see
        // this file's header note above `apply_one_control_event` for the
        // `structure_class`/`classify_event_id` consequence.

        // §3.4 R8 (EOP-VS-UBO-GAME-001 T3, corrected 2026-08-27): `remove`
        // is never refused — it PRUNES the links that touch the removed
        // block, computed here at fold time from prior state, never in the
        // payload (R6 purity — the event carries only `entity_id`, same
        // shape T2 gave it; `canonical_event_shape`'s `remove` arm is
        // unchanged by this correction). Only edges with this entity at
        // either endpoint go inactive; nothing propagates past them (no
        // cascade), and the far-end blocks keep every other link they had.
        // This is the ControlGraph half of `remove`'s two-axis fold —
        // `fold::type_registry`'s arm (unchanged by this correction)
        // records the withdrawal itself.
        "kyc_ubo.assert.subject.remove" => {
            if let Some(eid) = entity_id(p, "entity_id") {
                for edge in state.edges.values_mut() {
                    if edge.is_active() && (edge.from == eid || edge.to == eid) {
                        edge.status = EdgeStatus::Superseded;
                        edge.superseded_by = Some(event.id);
                    }
                }
            }
        }

        // `kyc_ubo.assert.edge.economic-interest` — RETIRED (EOP-VS-UBO-GAME-001
        // T3 §3.2, absorbed into `connect` below). P0c census: 0 real
        // committed events under this FQN — K-G7 full deletion of the arm.
        //
        // `kyc_ubo.assert.edge.control`'s R5-historical arm REMOVED
        // (EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07, Q2) — `connect`
        // below is, and was already, the live arm; this one existed only
        // to replay a real stream (4 committed events, P0c census) that
        // the clean start has cleared. A historical event bearing either
        // FQN now falls through to the catch-all `_ => {}`.

        // §3.2: `connect` absorbs assert-control + assert-economic-interest
        // — one merged verb, kind (including "economic_interest") always
        // present in the payload, geometry validates the classified pipe
        // (TS.5 R1) the same way for every kind. The live arm going
        // forward; identical shape to `control`'s historical arm above,
        // minus the special-casing economic-interest's old FQN needed
        // (kind is never absent on a live `connect` event — the
        // op-normalizer rejects anything outside `EDGE_KIND_WIRE_VALUES`
        // before append, TS.1 §1b).
        "kyc_ubo.assert.edge.connect" => {
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
                        proofs: BTreeMap::new(),
                        originating_event_id: event.id,
                        // TS.1 §2.1: revocability proof for settlor edges
                        // (see the field's polarity doc on `EdgeState`).
                        trust_revocable: p.get("trust_revocable").and_then(|v| v.as_bool()),
                        superseded_by: None,
                        // TS.6 P2 (K-G7): set only when this connect call is
                        // the first half of the `nominee-piercing` macro
                        // composition (op-layer stamps `pierced_from` from
                        // the caller's `pierced-from` arg).
                        pierced_from: edge_id_field(p, "pierced_from"),
                    },
                );
            }
        }

        // Move 4 (TS.1 §3, redefined EOP-DD-UBO-PROOF-001 §1/§4, T5): logs
        // one proof — kind, source, date — against an edge. No ratchet: the
        // old status-ladder rung this replaces is gone from `EdgeStatus`
        // itself; an edge can accumulate any number of proofs regardless of
        // status (`Precondition::EdgeActive` already refuses this at the op
        // layer for a superseded edge — nothing to re-check here).
        "kyc_ubo.assert.edge.evidence" => {
            if let Some(eid) = edge_id_from_target(event) {
                if let Some(edge) = state.edges.get_mut(&eid) {
                    if let Some(proof) = proof_record_from_payload(p, event.id) {
                        edge.proofs.insert(event.id, proof);
                    }
                }
            }
        }

        // `kyc_ubo.assert.edge.verification` RETIRED (EOP-DD-UBO-PROOF-001
        // §3/§4, T5, 2026-08-28): "the board collects facts; the policy
        // rules on adequacy" — there is no ratchet from evidenced to
        // verified to compute, because there is no `Verified` state left to
        // ratchet into. P0c census: 0 real committed events under this FQN
        // — K-G7 full deletion (unlike `evidence`, which has 5 real events
        // and keeps the arm above, extended rather than replaced). A
        // historical event still bearing this verb_fqn falls through to the
        // catch-all `_ => {}`, a no-op — there is nothing to replay.

        // Move 5 (§3.1's move table: "that proof no longer stands — group,
        // citation", EOP-DD-UBO-PROOF-001 §4, T5): removes one proof by the
        // event id that logged it. The set shrinks; nothing else happens —
        // there is no status to demote. Scans every edge because the
        // citation alone (not an edge/entity target) identifies the proof;
        // `fold::type_registry`'s mirrored arm does the same over entity
        // type records, from the same event, independently (T6.1(a)
        // discipline). At most one of the two axes will actually hold the
        // key — removing from the other is a harmless no-op.
        "kyc_ubo.assert.edge.retract" => {
            if let Some(citation) = citation_event_id_from_payload(p) {
                for edge in state.edges.values_mut() {
                    edge.proofs.remove(&citation);
                }
            }
        }

        // `kyc_ubo.assert.edge.supersession` — RETIRED (EOP-VS-UBO-GAME-001
        // T3 §3.2, renamed to `disconnect` below). P0c census: 0 real
        // committed events under this FQN — K-G7 full deletion of the arm.
        // A historical event still bearing this verb_fqn falls through to
        // the catch-all `_ => {}`, a no-op.

        // §3.2: `disconnect` absorbs supersession — pure rename, identical
        // shape (K-13: supersede-never-delete still holds — the edge stays,
        // status flips).
        "kyc_ubo.assert.edge.disconnect" => {
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
        // (`kyc_ubo.assert.edge.connect`'s `pierced_from` read) and below
        // (`kyc_ubo.assert.edge.disconnect`), composed by the `kyc_ubo.assert.edge.nominee-piercing`
        // MACRO (config/verb_schemas/macros/ubo.yaml), not a single event
        // under this verb_fqn. A historical event still bearing this
        // verb_fqn falls through to the catch-all `_ => {}` below, a
        // no-op — replay-faithful for events already in the stream (Q7/
        // K-18/K-31), same discipline as select-strategy/compute-fold's
        // retirement.

        // `kyc_ubo.assert.edge.reconciliation` — RETIRED (EOP-VS-UBO-GAME-001
        // T3 §3.3, 2026-08-27, K-G7 full deletion): "No reconcile ... a
        // third path to what two moves already do." P0c census found 2
        // real committed events under this FQN — unlike `control`
        // (kept R5) this is deleted anyway, because the concept it fed
        // (the K-14 freeze gate, `ControlState::is_reconciled()`) is
        // dissolved in the SAME tranche (see the `Precondition` enum and
        // `check_preconditions`'s former `ReconciledProjection` arm, both
        // deleted). Nothing downstream ever reads `reconciliation_event_id`
        // again once that gate is gone, so there is nothing left to keep
        // the field or the arm alive FOR — R5's replay-fidelity concern
        // only matters when some consumer still reads the reconstructed
        // state; here none does. A historical event still bearing this
        // verb_fqn falls through to the catch-all `_ => {}`, a no-op.

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
use crate::lexicon::{LexiconEntry, Precondition};

// `IMPLEMENTED_STRATEGY_CLASSES`/`strategy_for_structure_class` RETIRED
// (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) — the 11-class/8-arm
// `StructureClass` mapping they encoded is superseded by
// `dispatch_for_entity_type` (below), the ratified 21-type/7-strategy
// mapping (§2). `StructureClass` the TYPE and its fold arm
// (`apply_one_control_event`, `structure_class_from_payload`) stay —
// R5-historical, 10 real committed `structure-class` events (P0 census) —
// but nothing dispatches off it any longer.

// ── EOP-DD-UBO-DISPATCH-001 — the type is the dispatch key ────────────────────

/// The outcome of dispatching a determination by the subject's `EntityType`
/// (§2). Two variants, not a bare `Option<&str>`: §4 D2 requires the
/// terminal case to be an explicit, distinguishable arm — never an omission
/// that happens to read the same as "no strategy" would.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeterminationDispatch {
    /// EOP-DD-UBO-BASES-001 §3 (revises DISPATCH-001 §2): this type
    /// resolves via the SET of named `DeterminationStrategy`s that apply —
    /// every prior row is a one-element set with the same member as before;
    /// only `PrivateLimitedCompany`/`PublicListedCompany` widen to two
    /// (`ownership_prong_strategy`, `control_prong_strategy` — the PSC
    /// limbs apply together, not as alternatives selected by vehicle type,
    /// §0/§1). `kyc_stream_ops.rs`'s `UboDeterminationFreeze::execute`
    /// dispatch recognises exactly these 7 names post-T4:
    /// `ownership_prong_strategy`, `control_prong_strategy`,
    /// `trust_role_strategy`, `fund_control_strategy`,
    /// `foundation_council_strategy`, `state_owned_strategy`,
    /// `cooperative_member_strategy`. `nominee_pierce_strategy` is retired as
    /// a dispatch TARGET — no `EntityType` produces it (§2 "Where nominee_
    /// pierce_strategy went. Nowhere — deliberately"); the struct and its
    /// freeze-dispatch arm stay, unreachable from here, keeping the
    /// unconditional pre-freeze unpierced-nominee guard (TS.4 §3 Ruling B)
    /// and the traversal rule intact. Every strategy in the set runs and the
    /// results union (§3) — a person found by more than one carries every
    /// admitting basis (§4), not multiple candidates.
    Strategies(&'static [&'static str]),
    /// §4 D2: `NaturalPerson`/`SoleTrader` — a person is never a
    /// determination subject; a sole trader collapses to the person. An
    /// explicit arm, never an omission a future type could silently inherit.
    NotADeterminationSubject,
}

/// EOP-DD-UBO-DISPATCH-001 §2 — the ratified 21-type mapping, exhaustive
/// over `EntityType` (§4 D3: a new variant is a compile error here, not a
/// silent fallthrough — no catch-all arm exists). §4 D4 pins this same
/// table independently in `tests/kyc_t4_dispatch.rs::mapping_matches_the_
/// ratified_table` as data, so a drift here is caught by a second,
/// hand-authored copy, not by trusting this match's own arms.
///
/// Five types dispatch differently than the old 11-`StructureClass`/8-arm
/// mapping would have bucketed them (identified in T4's P0 recon — 3 are
/// explicit "RULED 2026-08-27" rows in §2, 2 more are explicit "tension"
/// callouts in the table text): `LlcUs` (control_prong, not the ownership
/// bucket an LLC would default to absent its own class), `LpFund` (splits
/// off the old singular "lp_fund" bucket into fund_control, distinct from
/// plain `LimitedPartnership`'s control_prong), `UnitTrust` (fund_control,
/// not trust_role, despite the superficial trustee framing), `UmbrellaWithSubFunds`
/// (newly a determination subject at all — fund_control), and
/// `CharityNotForProfit` (trust_role, not foundation_council).
pub fn dispatch_for_entity_type(entity_type: &EntityType) -> DeterminationDispatch {
    use DeterminationDispatch::*;
    use EntityType::*;
    match entity_type {
        // §2 rows 1-2, D2: terminal by design, not by omission.
        NaturalPerson | SoleTrader => NotADeterminationSubject,
        // EOP-DD-UBO-BASES-001 §3 (revises DISPATCH-001 §2 rows 3-4): the
        // PSC limbs apply TOGETHER to a company, not as alternatives keyed
        // by vehicle type — more than 25% of shares OR voting rights OR the
        // right to appoint/remove a majority of the board OR significant
        // influence/control by other means. `ownership_prong_strategy`
        // covers the equity limb; `control_prong_strategy` covers the rest
        // (board appointment, officer/employment-with-delegated-authority
        // once §5 lands, dominant influence). The ONLY row this table
        // changes relative to DISPATCH-001 — every other row is unchanged.
        PrivateLimitedCompany | PublicListedCompany => {
            Strategies(&["ownership_prong_strategy", "control_prong_strategy"])
        }
        // §2 rows 5-8: control by designation/appointment, not equity.
        // `LlcUs` RULED 2026-08-27 (§3 Q1) — partners/appointed officers,
        // same mechanism as a partnership.
        LlcUs | GeneralPartnership | LimitedPartnership | Llp => {
            Strategies(&["control_prong_strategy"])
        }
        // §2 rows 9-14: the directing mind sits outside the vehicle, under a
        // governing mandate — pivot and re-anchor there (TS.4 Ruling A).
        // `UnitTrust` (row 11) is trust-SHAPED but fund-DIRECTED: the
        // trustee holds legal title, does not direct. `LpFund` (row 13) is
        // distinguished from plain `LimitedPartnership` by being a fund —
        // GP AND ManCo, the mandate pivot is the meaningful path.
        // `UmbrellaWithSubFunds` RULED 2026-08-27 (§3 Q2) — a fund like any
        // other, directed by a ManCo exactly as its sub-funds are.
        OeicIcvc | Sicav | UnitTrust | FortyActFund | LpFund | UmbrellaWithSubFunds => {
            Strategies(&["fund_control_strategy"])
        }
        // §2 rows 15-16, 18, 20: role enumeration, no owners by construction.
        // `CharityNotForProfit` RULED 2026-08-27 (§3 Q3) — charities are
        // trusts, not foundation-shaped council governance.
        DiscretionaryTrust | FixedBareTrust | PensionScheme | CharityNotForProfit => {
            Strategies(&["trust_role_strategy"])
        }
        // §2 row 17: founder/council/beneficiaries — distinct from trust
        // role enumeration (Foundation has no settlor/trustee shape).
        Foundation => Strategies(&["foundation_council_strategy"]),
        // §2 row 19: one member one vote; economic percentage is meaningless.
        CooperativeMutual => Strategies(&["cooperative_member_strategy"]),
        // §2 row 21: traversal terminates in a public body; record the stop
        // and fall to officials.
        GovernmentDeptStatutoryCorporation => Strategies(&["state_owned_strategy"]),
    }
}

/// Check all preconditions for a verb against the current control state and
/// target binding **before** appending the event (K-11, K-14).
///
/// Was the "unified two-fold checker" (T6.1(a)) over `ControlState` AND
/// `ObligationState` — narrowed to control-only (EOP-DD-UBO-CLEANOUT-001
/// T6 P2, 2026-09-07): `ObligationState`/`fold/obligation.rs` deleted, both
/// obligation-reading arms (`ObligationExists`, `SubjectAllTerminal`) went
/// with it — see this crate's `lib.rs` module doc for the full reasoning
/// (the obligation fold's only two write-triggering FQNs were both already
/// retired/dissolved; nothing anywhere constructs an `ObligationTracks`).
/// This absorbed the former `check_control_preconditions` delegate, which
/// was already identical to this function once the obligation axis is gone.
pub fn check_preconditions(
    lexicon_entry: &LexiconEntry,
    control: &ControlState,
    type_registry: &TypeRegistryState,
    event: &IntentEvent,
) -> Result<(), KycError> {
    for pre in &lexicon_entry.preconditions {
        match pre {
            Precondition::SubjectRegistered => {
                if !control.registered {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: "subject must be registered (kyc_ubo.assert.subject.place) before this verb"
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
            // `StructureClassified`/`StructureClassSupported` RETIRED
            // (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) — see the enum
            // declaration in `lexicon.rs` for the reasoning. Superseded by
            // `EntityTypeSupportsStrategy` below.
            // EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28): the subject's own
            // `EntityType` is `EntityId(event.subject_root.0)` by construction
            // (`canonical_event_shape`'s `place` arm already defaults an
            // omitted `entity-id` to `subject.0` — no event scan needed, the
            // same fact `determination::find_subject_entity` now derives
            // directly instead of hunting for a retired `structure-class`
            // event). §4 D1: refuses by name; §4 D2: the terminal arm is its
            // own explicit branch, not folded into "unknown".
            Precondition::EntityTypeSupportsStrategy => {
                let subject_entity = EntityId(event.subject_root.0);
                match type_registry.type_of(subject_entity) {
                    None => {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: "no entity type known for the subject — place it with an \
                                     entity-type before freezing"
                                .into(),
                        });
                    }
                    Some(t) => match dispatch_for_entity_type(&t) {
                        DeterminationDispatch::Strategies(_) => {}
                        DeterminationDispatch::NotADeterminationSubject => {
                            return Err(KycError::PreconditionFailed {
                                verb: lexicon_entry.fqn.clone(),
                                reason: format!(
                                    "{t:?} is not a determination subject (EOP-DD-UBO-DISPATCH-001 \
                                     §4 D2) — a person is never frozen; a sole trader collapses to \
                                     the person"
                                ),
                            });
                        }
                    },
                }
            }
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
                // EOP-VS-UBO-GAME-001 T3: the old economic-interest FQN's
                // special case (its payload never carried `kind`) is gone —
                // `connect`, the live FQN, always carries `kind`, so
                // `edge_kind_from_payload` alone is correct now.
                let kind = edge_kind_from_payload(&event.payload);
                let duplicate = control
                    .edges
                    .values()
                    .any(|e| e.is_active() && e.from == from && e.to == to && e.kind == kind);
                if duplicate {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "an active edge of kind {kind:?} already exists from {from:?} to \
                             {to:?}; use kyc_ubo.assert.edge.disconnect, never a contradicting assert (K-13)"
                        ),
                    });
                }
            }
            Precondition::EdgeExists => {
                // 2026-08-24 corrective tranche (Item 3a): vacuous when the
                // probe carries no `edge_id` — same "vacuous when probed
                // without the field it checks" convention as
                // `EntityRegistered`/`MembershipActive`/`PriorTypeAsserted`
                // below. This is what makes `kyc_ubo.assert.edge.evidence`'s
                // ENTITY-scoped half (evidencing a type, not an edge)
                // reachable: that call shape never supplies `edge_id`, so
                // this edge-only check correctly does not apply to it.
                // `kyc_ubo.assert.edge.supersession` (the only other user of
                // this precondition) always supplies `edge_id`, so this is a
                // no-op change for it.
                if let Some(eid) = event.target.edge_id {
                    if !control.edges.contains_key(&eid) {
                        return Err(KycError::EdgeNotFound(eid));
                    }
                }
            }
            Precondition::EdgeActive => {
                // Vacuous when probed without `edge_id` — see `EdgeExists` above.
                if let Some(eid) = event.target.edge_id {
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
                                 (kyc_ubo.assert.subject.place) — TS.1 §3 rows 2/6/7"
                            ),
                        });
                    }
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
                                "entity {eid:?} has no prior type assertion — assert one via \
                                 kyc_ubo.assert.subject.place first (TS.1 §3 row 7)"
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
            Precondition::PiercedFromIsActiveNominee => {
                // 2026-09-07 (audit item 2, K-8, §3.4 R7): hoisted from
                // `UboEdgeConnect::execute`'s hand-rolled check — the
                // workbook path never called that op struct, so the rule
                // never ran there. Vacuous when the probe carries no
                // `pierced_from` (payload-keyed, same convention as
                // `NoDuplicateActiveEdge`/`TypeGeometryPermits` above).
                let Some(pf) = event
                    .payload
                    .get("pierced_from")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<uuid::Uuid>().ok())
                    .map(EdgeId)
                else {
                    continue;
                };
                match control.edges.get(&pf) {
                    None => {
                        return Err(KycError::EdgeNotFound(pf));
                    }
                    Some(e) if !matches!(e.kind, EdgeKind::Nominee) => {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!(
                                "pierced-from edge {pf:?} is not a nominee edge (kind \
                                 {:?}) — only EdgeKind::Nominee arrangements can be \
                                 pierced (K-8, fail-closed)",
                                e.kind
                            ),
                        });
                    }
                    Some(e) if !e.is_active() => {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!("pierced-from edge {pf:?} is not active"),
                        });
                    }
                    Some(_) => {}
                }
            }
            Precondition::NoUnpiercedNomineeEdges => {
                // 2026-09-07 (audit item 2, P2): hoisted from `freeze`'s
                // hand-rolled pre-dispatch scan (TS.4 §3 Ruling B, K-8).
                // Event-independent — a pure `control` scan, same
                // discipline as `SubjectAllTerminal`.
                let unpierced = unpierced_nominee_edges(control);
                if !unpierced.is_empty() {
                    let ids: Vec<String> = unpierced.iter().map(|id| id.0.to_string()).collect();
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "unpierced nominee edge(s) remain active: [{}] — pierce each via \
                             kyc_ubo.assert.edge.nominee-piercing before freezing (TS.4 §3 \
                             Ruling B, K-8, fail-closed)",
                            ids.join(", ")
                        ),
                    });
                }
            }
            Precondition::NotCurrentlyPlaced => {
                // EOP-VS-UBO-GAME-001 §8 Q1 — `place` never carries update
                // semantics. Vacuous when probed without `entity_id` — same
                // convention as `EntityRegistered` above.
                if let Some(EntityId(eid)) = event.target.entity_id {
                    let currently_placed = control.registered_entity_ids.contains(&EntityId(eid))
                        && !type_registry.is_withdrawn(EntityId(eid));
                    if currently_placed {
                        return Err(KycError::PreconditionFailed {
                            verb: lexicon_entry.fqn.clone(),
                            reason: format!(
                                "entity {eid:?} is already placed on the board; place never \
                                 carries update semantics (§8 Q1) — remove it first, then \
                                 place the correction"
                            ),
                        });
                    }
                }
            }
            Precondition::ConnectEndpointsNotWithdrawn => {
                // EOP-VS-UBO-GAME-001 §3.4 geometry-closure finding #2,
                // RULED 2026-09-07, scope confirmed 2026-09-08 — see the
                // `Precondition` variant's doc. Vacuous when either endpoint
                // is absent from the payload — same convention as
                // `NoDuplicateActiveEdge`/`TypeGeometryPermits` above. A
                // never-registered endpoint is NOT refused here — only a
                // currently-WITHDRAWN one; `TypeGeometryPermits` still
                // governs the never-registered case (Unevaluable/admit,
                // R5/R6/CTN-2e, e.g. the K-8 nominee holder).
                let (from, to) = (
                    entity_id(&event.payload, "from_entity_id"),
                    entity_id(&event.payload, "to_entity_id"),
                );
                let (Some(from), Some(to)) = (from, to) else {
                    continue;
                };
                if type_registry.is_withdrawn(from) {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "entity {from:?} (from) is withdrawn — connecting to a block that \
                             is not on the board is incoherent (§3.4 geometry-closure ruling)"
                        ),
                    });
                }
                if type_registry.is_withdrawn(to) {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "entity {to:?} (to) is withdrawn — connecting to a block that is \
                             not on the board is incoherent (§3.4 geometry-closure ruling)"
                        ),
                    });
                }
            }
            Precondition::PercentageIsBounded => {
                // EOP-DD-UBO-BASES-001 §6/§7 closure tranche P3a
                // (2026-09-08, audit item 7) — widened from the original
                // kind-scoped-to-"economic_interest"-only shape. §6's text
                // is unqualified ("a Precondition bounding percentage to
                // [0,100] on connect"); the original narrowing's own
                // justification ("a non-economic connect never carries
                // percentage at all") was FALSE — the fuzzer's own
                // generator sends adversarial percentages on every kind,
                // and `voting_rights` is a genuine quantity-carrying kind
                // (the PSC "more than 25% of... voting rights" limb).
                //
                // Every OTHER wire kind carries no quantity a strategy
                // ever reads (`Pipe::classify_economic_interest`'s three
                // ratified buckets — non-voting shares, LP interest, unit
                // issuance — are all sub-classifications of the
                // "economic_interest" wire kind itself, keyed off the
                // TARGET's type, not a distinct kind string; there is no
                // separate wire value for any of them). So the bounded set
                // is exactly {economic_interest, voting_rights}; a stray
                // `percentage` on anything else is refused outright, not
                // merely bounded — it should never have been asserted.
                let kind_str = event.payload.get("kind").and_then(|k| k.as_str());
                let carries_quantity =
                    matches!(kind_str, Some("economic_interest") | Some("voting_rights"));
                let Some(pct) = opt_f64(&event.payload, "percentage") else {
                    continue;
                };
                if !carries_quantity {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "percentage {pct} was asserted on kind {kind_str:?}, which carries \
                             no quantity a determination strategy ever reads — refused outright, \
                             not bounded (EOP-DD-UBO-BASES-001 §1/§6, closure tranche P3a)"
                        ),
                    });
                }
                if !(0.0..=100.0).contains(&pct) {
                    return Err(KycError::PreconditionFailed {
                        verb: lexicon_entry.fqn.clone(),
                        reason: format!(
                            "percentage {pct} is out of bounds — must lie in [0, 100] \
                             (EOP-DD-UBO-BASES-001 §6, finding #3)"
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

// `check_control_preconditions` REMOVED (EOP-DD-UBO-CLEANOUT-001 T6 P2,
// 2026-09-07): it was a thin delegate calling `check_preconditions` with a
// default `&ObligationState` — once the obligation axis is gone, that made
// it byte-for-byte identical to `check_preconditions` itself. Callers
// updated to call `check_preconditions` directly.

// ── Economic edge summary (for determination strategy) ────────────────────────

/// A verified, reconciled economic edge ready for prong computation.
/// Consumed by `DeterminationStrategy::resolve()` (§5).
#[derive(Debug, Clone)]
pub struct ReconciledEconomicEdge {
    pub id: EdgeId,
    pub from: EntityId,
    pub to: EntityId,
    pub percentage: f64,
    /// EOP-DD-UBO-BASES-001 §4: the real edge kind (`EconomicInterest` or
    /// `Containment` — `is_economic()`'s two members), not assumed —
    /// carried through so a `ProngCandidate`'s `AdmittingBasis` records
    /// what actually admitted it, not a guess.
    pub kind: EdgeKind,
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
                kind: e.kind.clone(),
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
    /// Never part of the control walk (`EconomicInterest` — the ownership
    /// prong's job; `Containment` — structural scoping). `OfficerAppointment`
    /// moved OUT of this class to `Traverse` (EOP-DD-UBO-BASES-001 §5 R-A,
    /// 2026-09-08) — an edge kind can still be a terminal/population source
    /// pulled by `pull_smo_on_exhaustion` (§4a) without being `NotControl`;
    /// the pull and this admission class are independent mechanisms, not a
    /// pairing.
    NotControl,
    /// Substitute the underlying holder and continue (K-8). Not filtered by
    /// `reconciled_control_edges` — pierce-and-substitute is handled by the
    /// `kyc_ubo.assert.edge.nominee-piercing` macro composition (TS.6 P2), before any
    /// strategy ever runs.
    Pierce,
}

/// The TS.3 §3/§4 admission ruling, per `EdgeKind` — REVISED by
/// EOP-DD-UBO-BASES-001 §5 R-A (2026-09-08; recorded in the state-of-play,
/// not by editing the ratified TS.3 document itself — the second such
/// revision to a TS.3 classification, after `ManagementMandate` on
/// 2026-08-20).
///
/// **This is a SAFETY guard, not a semantics ruling.** Before this function
/// existed (as the boolean `is_admitted_as_control`, landed `c7ef69ca`), the
/// filter was an EXCLUSION list — everything except
/// `EconomicInterest`/`Nominee` passed — which silently admitted every
/// *future* `EdgeKind` variant as generic control the moment it became
/// assertable, with no domain ruling behind the admission. TS.3 closed the
/// semantics question the whitelist tranche deliberately left open,
/// reconciling to V&S v0.6 §6.4's control-axis column rather than inventing:
/// `ManagementMandate` and `MembershipRights` are named as control axes for
/// funds and cooperatives respectively and move to `Traverse`;
/// `StatutoryAuthority` routes to SMO/special-handling per §6.4's
/// state-owned row, so it `Stop`s rather than being walked or silently
/// dropped; `EconomicInterest` is out because that's the ownership prong's
/// job (`reconciled_economic_edges`); `Containment` stays out (structural
/// scoping, not control, per §3); `Nominee` is `Pierce`, never a plain
/// admission.
///
/// **R-A (2026-09-08):** `OfficerAppointment` and `Employment` move from
/// `NotControl` to `Traverse`. TS.3's original ruling (2026-08-21) held
/// `OfficerAppointment` out of the walk as "may not be needed or relevant",
/// available only for the §4a pull-on-exhaustion, never by admission —
/// EOP-DD-UBO-BASES-001 §0 found that ruling under-identified: the office,
/// and employment carrying delegated authority to run the business, are
/// each an INDEPENDENTLY SUFFICIENT door to control (§1 — "control has
/// several doors, and any one opens"), not a fallback-of-last-resort
/// reserved for when nobody else is found. Adam's words: "employment with
/// delegated authority is the thing." R-B (`pull_smo_on_exhaustion`'s own
/// doc) records the companion consequence: the exhaustion pull must not be
/// confused with — or suppressed by, or a suppressor of — this admission.
///
/// The match is exhaustive with NO catch-all arm: adding a new `EdgeKind`
/// variant anywhere in this enum is a compile error here, never a silent
/// admission.
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
        | EdgeKind::MembershipRights
        | EdgeKind::OfficerAppointment
        | EdgeKind::Employment => Traverse,
        EdgeKind::StatutoryAuthority => Stop,
        EdgeKind::EconomicInterest | EdgeKind::Containment => NotControl,
        EdgeKind::Nominee => Pierce,
    }
}

/// Extract the reconciled (active) control edges from the control state —
/// the `Traverse`-classified edges (`control_admission`), active. Used by
/// `ControlProngStrategy` (M4) and its delegates/siblings. Nothing here
/// gates on citation status — a bare-`Asserted`, uncited edge still
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
                originating_event_id: e.originating_event_id,
            }),
            _ => None,
        })
        .collect()
}

// ── Entity classification helpers ─────────────────────────────────────────────

/// Returns the set of `PersonId`s that are natural persons in the event stream.
///
/// **The entity type is the single source of truth for personhood** (RULED
/// 2026-09-07, closing the 2026-08-28 audit's A1b divergence): an entity is
/// a natural person iff its asserted `EntityType` is `NaturalPerson`. The
/// old implementation read only the payload flag `is_natural_person` — a
/// second vocabulary for one concept, which let a correctly-typed 50% owner
/// placed without the redundant flag vanish silently from a frozen
/// determination of record (T4's thesis is "the type is the dispatch key";
/// the traversal terminus was still keying on a parallel boolean that could
/// contradict it).
///
/// **R5-historical fallback, scoped to type-less entities only:** 207
/// committed register-era events carry ONLY the flag (`register` never had
/// an entity-type payload key), so for an entity with NO type assertion
/// anywhere in the stream the historical flag is still read — dropping it
/// would fabricate non-personhood on replay. Wherever a type exists, the
/// type wins and the flag is ignored (0 committed events disagree — P0b
/// census, 2026-09-07). The live path can no longer reach the fallback:
/// `place` requires a validated entity-type, and the flag is no longer a
/// declared argument on any verb.
///
/// Returns a `BTreeSet` so iteration order is deterministic (sorted by UUID bytes).
pub fn natural_persons_from_events(events: &[&IntentEvent]) -> BTreeSet<PersonId> {
    let type_registry = crate::fold::type_registry::fold_type_registry(events);
    let mut persons: BTreeSet<PersonId> = type_registry
        .types
        .iter()
        .filter(|(_, rec)| rec.entity_type == crate::geometry::EntityType::NaturalPerson)
        .map(|(eid, _)| PersonId(eid.0))
        .collect();

    // Fallback guarded on "no type assertion exists" — reachable only by
    // a `place` event whose wire type failed the fail-closed parse (the
    // `kyc_ubo.assert.subject.register`-era arm of this check REMOVED,
    // EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07: `register`'s own R5
    // fold arm is gone, so no event bearing that FQN can occur here any
    // more — see `apply_one_control_event`'s header note).
    for event in events {
        if event.verb_fqn.as_str() == "kyc_ubo.assert.subject.place"
            && event
                .payload
                .get("is_natural_person")
                .and_then(|v| v.as_bool())
                == Some(true)
        {
            if let Some(pid) = person_id(&event.payload, "entity_id") {
                if type_registry.type_of(EntityId(pid.0)).is_none() {
                    persons.insert(pid);
                }
            }
        }
    }
    persons
}
