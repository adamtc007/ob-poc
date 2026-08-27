//! Entity-type registry fold — EOP-DD-KYCUBO-TS.1 §3 moves 2 (`assert-type`)
//! and 7 (`correct-type`), plus moves 6 (`withdraw-member`) and 8
//! (`record-enquiry`).
//!
//! A THIRD fold axis alongside `ControlState`/`ObligationState`
//! (`fold::control`, `fold::obligation`), following the same "independent
//! pure fold over the per-subject event stream" pattern (T6.1(a)). Kept
//! independent rather than folded into `ControlState` because: (a) it is
//! read only by the NEW type-geometry gate (`placement`'s re-keyed
//! admission, TS.1 §5), never by the EXISTING `check_preconditions` stud
//! set, so merging it would widen that function's blast radius for no
//! reason; (b) `ControlState`'s own fold (`fold::control::
//! apply_one_control_event`) stays completely untouched by this tranche —
//! zero risk to the 21-verb lexicon's existing behaviour.
//!
//! **`admit-member` (TS.1 move 1) has no fold arm here.** P1 recon found
//! it already satisfied by the EXISTING `kyc_ubo.assert.subject.register` event,
//! which populates `ControlState.registered_entity_ids` — a group is
//! already, informally, "the set of entities registered under one
//! `subject_root`" (see that field's own doc comment). This axis therefore
//! tracks withdrawal only, which has no existing home.
//!
//! **`assert-linkage`/`withdraw-linkage` (moves 3/5) have no fold arm
//! here either** — deliberately. P1 recon found `kyc_ubo.assert.edge.control`/
//! `assert-economic-interest`/`kyc_ubo.assert.edge.supersession` are near-exact matches;
//! re-keying their EXISTING admission logic (in `placement`) to also check
//! type geometry satisfies TS.1 §5 without minting duplicate verbs. See
//! the tranche receipts for the one recorded gap this leaves (`supersede`
//! has no cessation-vs-correction payload distinction TS.1 §3 row 5 asks
//! for).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::event::IntentEvent;
use crate::geometry::EntityType;
use crate::types::{EdgeId, EntityId, EventId};

// ── Wire values (mirrors EDGE_KIND_WIRE_VALUES / STRUCTURE_CLASS_WIRE_VALUES) ─

/// Canonical `entity-type` wire-string set for `kyc_ubo.assert.subject.type` /
/// `kyc_ubo.assert.subject.type-correction` — one arm per `EntityType` variant (TS.0 §4),
/// fail-closed: any string outside this set is rejected, never guessed.
pub const ENTITY_TYPE_WIRE_VALUES: &[&str] = &[
    "natural_person",
    "sole_trader",
    "private_limited_company",
    "public_listed_company",
    "llc_us",
    "general_partnership",
    "limited_partnership",
    "llp",
    "oeic_icvc",
    "sicav",
    "unit_trust",
    "forty_act_fund",
    "lp_fund",
    "umbrella_with_sub_funds",
    "discretionary_trust",
    "fixed_bare_trust",
    "foundation",
    "pension_scheme",
    "cooperative_mutual",
    "charity_not_for_profit",
    "government_dept_statutory_corporation",
    "sovereign_wealth_vehicle",
];

pub fn entity_type_from_wire(s: &str) -> Option<EntityType> {
    use EntityType::*;
    Some(match s {
        "natural_person" => NaturalPerson,
        "sole_trader" => SoleTrader,
        "private_limited_company" => PrivateLimitedCompany,
        "public_listed_company" => PublicListedCompany,
        "llc_us" => LlcUs,
        "general_partnership" => GeneralPartnership,
        "limited_partnership" => LimitedPartnership,
        "llp" => Llp,
        "oeic_icvc" => OeicIcvc,
        "sicav" => Sicav,
        "unit_trust" => UnitTrust,
        "forty_act_fund" => FortyActFund,
        "lp_fund" => LpFund,
        "umbrella_with_sub_funds" => UmbrellaWithSubFunds,
        "discretionary_trust" => DiscretionaryTrust,
        "fixed_bare_trust" => FixedBareTrust,
        "foundation" => Foundation,
        "pension_scheme" => PensionScheme,
        "cooperative_mutual" => CooperativeMutual,
        "charity_not_for_profit" => CharityNotForProfit,
        "government_dept_statutory_corporation" => GovernmentDeptStatutoryCorporation,
        "sovereign_wealth_vehicle" => SovereignWealthVehicle,
        _ => return None,
    })
}

// ── Type-proof status (TS.0 §2 P2) ──────────────────────────────────────────

/// TS.0 §2's P2 exit status: "type proved, or still alleged."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeProofStatus {
    Alleged,
    Proved,
}

#[derive(Debug, Clone)]
pub struct EntityTypeRecord {
    pub entity_type: EntityType,
    pub proof: TypeProofStatus,
    /// The event that last ASSERTED this type (`assert-type` or
    /// `type-correction`) — stays pinned to that event even after a later
    /// `attach-evidence` flips `proof` to `Proved`; it does not become "the
    /// event that proved it."
    pub originating_event_id: EventId,
    /// The event that flipped `proof` to `Proved` (`attach-evidence`,
    /// type-scoped target) — `None` while `proof` is still `Alleged`. This,
    /// not `originating_event_id`, is the fact a `Proved` conclusion rests
    /// on (D2.0 §4: "findings citing the facts relied on").
    pub proof_event_id: Option<EventId>,
}

/// One `correct-type` cascade record (TS.1 §4). Accumulates — corrections
/// are never overwritten or deleted (K-13-style discipline extended to the
/// type axis, TS.0 §6.1).
#[derive(Debug, Clone)]
pub struct TypeCorrectionRecord {
    pub entity: EntityId,
    pub previous_type: Option<EntityType>,
    pub corrected_type: EntityType,
    /// Edges (touching `entity` as either endpoint) that the geometry
    /// matrix no longer permits under the corrected type — demoted
    /// (flagged for re-assertion), never deleted (TS.1 §4).
    pub invalidated_edges: Vec<EdgeId>,
    pub event_id: EventId,
}

/// One `record-enquiry` diligence record (TS.0 §6.3 — completeness is an
/// assertion, never a proof). Accumulates.
#[derive(Debug, Clone)]
pub struct EnquiryRecord {
    pub sources_consulted: Vec<String>,
    pub searches_run: Vec<String>,
    pub event_id: EventId,
}

/// Folded view of the type-registry axis for one subject.
/// `state = fold_type_registry(events)`. Never stored directly.
#[derive(Debug, Default, Clone)]
pub struct TypeRegistryState {
    /// Current type assertion per entity — last-wins on `assert-type`
    /// (same "reclassify stays legal" discipline `structure_class`
    /// already uses), demoted-and-replaced on `correct-type`.
    pub types: BTreeMap<EntityId, EntityTypeRecord>,
    /// Entities explicitly withdrawn from group membership (TS.1 move 6),
    /// keyed to the `member-withdrawal` event that caused it — the same
    /// discipline as `EntityTypeRecord::proof_event_id`, not a reuse of
    /// `originating_event_id`: this is the actual cause, not the
    /// assertion. Never removes the entity from
    /// `ControlState.registered_entity_ids` — that set is untouched by
    /// this axis (TS.1 §2c: the board is a basket of references;
    /// withdrawal flags, never deletes).
    pub withdrawn_members: BTreeMap<EntityId, EventId>,
    pub corrections: Vec<TypeCorrectionRecord>,
    pub enquiries: Vec<EnquiryRecord>,
    /// Edges flagged for re-assertion by the most recent cascade touching
    /// them. The edge itself, in `ControlState.edges`, is untouched by
    /// this fold — flagging is additive bookkeeping on a disjoint axis.
    pub flagged_edges: BTreeSet<EdgeId>,
    /// True once any correction has fired. D1 scope: recorded as a fact
    /// for D2/re-determination to read; never interpreted here (TS.1 §4:
    /// "any determination that traversed them is marked stale").
    pub determination_stale: bool,
}

impl TypeRegistryState {
    pub fn type_of(&self, entity: EntityId) -> Option<EntityType> {
        self.types.get(&entity).map(|r| r.entity_type)
    }

    pub fn is_withdrawn(&self, entity: EntityId) -> bool {
        self.withdrawn_members.contains_key(&entity)
    }

    /// The real `EventId` of the `member-withdrawal` event that withdrew
    /// `entity` — `None` if it was never withdrawn. This, not
    /// `originating_event_id_of`, is the fact a `Fail` verdict citing
    /// withdrawal-without-proof rests on (D2.0 §4).
    pub fn withdrawal_event_id_of(&self, entity: EntityId) -> Option<EventId> {
        self.withdrawn_members.get(&entity).copied()
    }

    /// `entity`'s type-proof status — `None` if no type has been asserted
    /// at all (a different, "unresolved" state from `Alleged`; see
    /// `PipeClassification`'s doc). Used by TS.3 §2a's provisionality
    /// propagation: a traversal decision resting on an `Alleged` type is
    /// itself provisional, distinctly from an alleged EDGE
    /// (`determination::compute_assurance`).
    pub fn proof_of(&self, entity: EntityId) -> Option<TypeProofStatus> {
        self.types.get(&entity).map(|r| r.proof)
    }

    /// The real `EventId` of the event that produced `entity`'s current
    /// type-proof record (whichever of `assert-type`/`attach-evidence` set
    /// it last) — `None` if no type has been asserted at all. This is the
    /// fact a check citing a type-proof conclusion must cite (D2.0 §4:
    /// "findings citing the facts relied on").
    pub fn originating_event_id_of(&self, entity: EntityId) -> Option<EventId> {
        self.types.get(&entity).map(|r| r.originating_event_id)
    }

    /// The real `EventId` of the event that PROVED `entity`'s type —
    /// `None` when the entity has no type record, or its type is still
    /// `Alleged`. Distinct from `originating_event_id_of`, which names the
    /// assertion, not the proof. This is the fact a `Proved` conclusion
    /// rests on (D2.0 §4).
    pub fn proof_event_id_of(&self, entity: EntityId) -> Option<EventId> {
        self.types.get(&entity).and_then(|r| r.proof_event_id)
    }
}

fn entity_id(v: &serde_json::Value, field: &str) -> Option<EntityId> {
    v.get(field)?
        .as_str()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(EntityId)
}

fn entity_type_from_payload(v: &serde_json::Value) -> Option<EntityType> {
    entity_type_from_wire(v.get("entity_type")?.as_str()?)
}

fn string_list_from_payload(v: &serde_json::Value, field: &str) -> Vec<String> {
    v.get(field)
        .and_then(|x| x.as_array())
        .map(|arr| arr.iter().filter_map(|e| e.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

// `edges_invalidated_by_correction` (the TS.1 §4 correct-type cascade)
// DELETED — EOP-VS-UBO-GAME-001 T2 (2026-08-27, §8 Q1) DISSOLVED
// `kyc_ubo.assert.subject.type-correction`, this function's sole caller
// (`KycSubjectCorrectType`, also deleted). Correcting a type is now `remove`
// then `place`, two ordinary moves — there is no computed cascade to run.

/// Apply one type-registry event. Total-dispatch (unrecognised verbs are a
/// no-op — same discipline as `fold::control::apply_one_control_event`).
pub(crate) fn apply_one_type_registry_event(
    mut state: TypeRegistryState,
    event: &IntentEvent,
) -> TypeRegistryState {
    let p = &event.payload;
    match event.verb_fqn.as_str() {
        // §3.2: `place` absorbs register + assert-type. Writes the SAME
        // TypeRegistry axis `assert-type` used to; `fold::control::
        // apply_one_control_event`'s `place` arm writes the ControlGraph
        // axis (membership) independently, from the same event (T6.1(a)).
        // Always Alleged (CTN-2f, unchanged from `type`'s own discipline).
        // Un-withdraws on re-placement: §2 — "the entity is untouched and
        // remains available to be placed again" after `remove`; without
        // clearing `withdrawn_members` here, a re-placed entity would stay
        // flagged withdrawn forever, contradicting that.
        "kyc_ubo.assert.subject.place" => {
            if let (Some(eid), Some(entity_type)) =
                (entity_id(p, "entity_id"), entity_type_from_payload(p))
            {
                state.types.insert(
                    eid,
                    EntityTypeRecord {
                        entity_type,
                        proof: TypeProofStatus::Alleged,
                        originating_event_id: event.id,
                        proof_event_id: None,
                    },
                );
                state.withdrawn_members.remove(&eid);
            }
        }

        // §3.2: `remove` absorbs member-withdrawal. Flags the placement
        // withdrawn; never touches `ControlState.registered_entity_ids`
        // (§2 — remove withdraws a placement, never an entity).
        "kyc_ubo.assert.subject.remove" => {
            if let Some(eid) = entity_id(p, "entity_id") {
                state.withdrawn_members.insert(eid, event.id);
            }
        }

        // Historical only (EOP-VS-UBO-GAME-001 T2 retired the verb; the arm
        // stays so any pre-existing stream with real `type` events still
        // folds correctly — R5). No new event of this kind can be produced
        // going forward.
        "kyc_ubo.assert.subject.type" => {
            // CTN-2f: status is computed, never asserted (`no_move_sets_status`,
            // TS.1 §6). Every `assert-type` starts `Alleged` — there is no
            // payload flag to skip that (contrast an earlier draft of this
            // arm, which read a `proof` field directly off the payload; that
            // would have let a caller assert its way straight to `Proved`,
            // the exact thing CTN-2f forbids). `Proved` is reachable only by
            // deriving it from a subsequent `attach-evidence` event, below —
            // mirrors `fold::control`'s `EdgeStatus` derivation exactly.
            if let (Some(eid), Some(entity_type)) =
                (entity_id(p, "entity_id"), entity_type_from_payload(p))
            {
                state.types.insert(
                    eid,
                    EntityTypeRecord {
                        entity_type,
                        proof: TypeProofStatus::Alleged,
                        originating_event_id: event.id,
                        proof_event_id: None,
                    },
                );
            }
        }
        // Move 4 (TS.1 §3): "this document/source evidences a TYPE OR A
        // LINKAGE" — one verb, two possible targets. Edge-scoped evidencing
        // (`target.edge_id` set) is handled entirely by `fold::control`,
        // unchanged. This arm handles ONLY the type-scoped case: a target
        // naming `entity_id` with no `edge_id` evidences that entity's
        // current type assertion, deriving `Proved` — never set directly.
        "kyc_ubo.assert.edge.evidence" if event.target.edge_id.is_none() => {
            if let Some(eid) = event.target.entity_id {
                if let Some(record) = state.types.get_mut(&eid) {
                    record.proof = TypeProofStatus::Proved;
                    record.proof_event_id = Some(event.id);
                }
            }
        }
        // `kyc_ubo.assert.subject.type-correction` DISSOLVED (T2, §8 Q1,
        // 2026-08-27) — no match arm survives it here. UNLIKE the other
        // T2-retired verbs, this one had 0 real committed events (confirmed
        // by DB query before deletion), so there is no historical stream
        // this arm needs to keep folding correctly — full K-G7 deletion,
        // not an R5 historical-replay-only retirement. A historical event
        // still bearing this verb_fqn falls through to no arm at all and
        // contributes nothing to the fold (same "falls through as a no-op,
        // not a crash" discipline `geometry_triple_for_event`'s retired
        // `nominee-piercing` special case documents).
        // Historical only (EOP-VS-UBO-GAME-001 T2 retired the verb in favor
        // of `remove`, above — identical arm, kept so any pre-existing
        // stream with real `member-withdrawal` events still folds
        // correctly — R5). No new event of this kind can be produced
        // going forward.
        "kyc_ubo.assert.subject.member-withdrawal" => {
            if let Some(eid) = entity_id(p, "entity_id") {
                state.withdrawn_members.insert(eid, event.id);
            }
        }
        "kyc_ubo.assert.subject.enquiry" => {
            state.enquiries.push(EnquiryRecord {
                sources_consulted: string_list_from_payload(p, "sources_consulted"),
                searches_run: string_list_from_payload(p, "searches_run"),
                event_id: event.id,
            });
        }
        _ => {}
    }
    state
}

/// Pure fold of the ordered event stream onto `TypeRegistryState`.
pub fn fold_type_registry(events: &[&IntentEvent]) -> TypeRegistryState {
    events.iter().fold(TypeRegistryState::default(), |st, e| {
        apply_one_type_registry_event(st, e)
    })
}
