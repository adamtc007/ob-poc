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
//! it already satisfied by the EXISTING `kyc.subject.register` event,
//! which populates `ControlState.registered_entity_ids` — a group is
//! already, informally, "the set of entities registered under one
//! `subject_root`" (see that field's own doc comment). This axis therefore
//! tracks withdrawal only, which has no existing home.
//!
//! **`assert-linkage`/`withdraw-linkage` (moves 3/5) have no fold arm
//! here either** — deliberately. P1 recon found `ubo.edge.assert-control`/
//! `assert-economic-interest`/`ubo.edge.supersede` are near-exact matches;
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

/// Canonical `entity-type` wire-string set for `kyc.subject.assert-type` /
/// `kyc.subject.correct-type` — one arm per `EntityType` variant (TS.0 §4),
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
    pub originating_event_id: EventId,
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
    /// Entities explicitly withdrawn from group membership (TS.1 move 6).
    /// Never removes the entity from `ControlState.registered_entity_ids`
    /// — that set is untouched by this axis (TS.1 §2c: the board is a
    /// basket of references; withdrawal flags, never deletes).
    pub withdrawn_members: BTreeSet<EntityId>,
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
        self.withdrawn_members.contains(&entity)
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

/// Compute which of an entity's touching edges the geometry matrix no
/// longer permits once its type becomes `corrected_type` (TS.1 §4
/// cascade). `touching_edges` is `(edge_id, pipe, other_end_type,
/// this_entity_is_source)` — reduced by the caller from `ControlState`
/// (this module doesn't depend on `fold::control`, to avoid a fold-to-fold
/// coupling; the caller, which already holds both folds at the write
/// path, does the reduction — mirrors how `ubo.edge.pierce-nominee`'s
/// op-layer, not the fold, resolves its own cross-references). Pure.
pub fn edges_invalidated_by_correction(
    corrected_type: EntityType,
    touching_edges: &[(EdgeId, crate::geometry::Pipe, EntityType, bool)],
) -> Vec<EdgeId> {
    touching_edges
        .iter()
        .filter_map(|(edge_id, pipe, other_end_type, this_is_source)| {
            let permitted = if *this_is_source {
                crate::geometry::check_type_geometry(
                    crate::geometry::LinkageSource::Entity(corrected_type),
                    *pipe,
                    *other_end_type,
                )
                .is_ok()
            } else {
                crate::geometry::check_type_geometry(
                    crate::geometry::LinkageSource::Entity(*other_end_type),
                    *pipe,
                    corrected_type,
                )
                .is_ok()
            };
            if permitted {
                None
            } else {
                Some(*edge_id)
            }
        })
        .collect()
}

/// Apply one type-registry event. Total-dispatch (unrecognised verbs are a
/// no-op — same discipline as `fold::control::apply_one_control_event`).
pub(crate) fn apply_one_type_registry_event(
    mut state: TypeRegistryState,
    event: &IntentEvent,
) -> TypeRegistryState {
    let p = &event.payload;
    match event.verb_fqn.as_str() {
        "kyc.subject.assert-type" => {
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
        "ubo.edge.attach-evidence" if event.target.edge_id.is_none() => {
            if let Some(eid) = event.target.entity_id {
                if let Some(record) = state.types.get_mut(&eid) {
                    record.proof = TypeProofStatus::Proved;
                }
            }
        }
        "kyc.subject.correct-type" => {
            if let (Some(eid), Some(corrected_type)) =
                (entity_id(p, "entity_id"), entity_type_from_payload(p))
            {
                let previous_type = state.types.get(&eid).map(|r| r.entity_type);
                let invalidated_edges: Vec<EdgeId> = p
                    .get("invalidated_edge_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .filter_map(|s| uuid::Uuid::parse_str(s).ok())
                            .map(EdgeId)
                            .collect()
                    })
                    .unwrap_or_default();
                state.types.insert(
                    eid,
                    EntityTypeRecord {
                        entity_type: corrected_type,
                        proof: TypeProofStatus::Alleged,
                        originating_event_id: event.id,
                    },
                );
                for e in &invalidated_edges {
                    state.flagged_edges.insert(*e);
                }
                if !invalidated_edges.is_empty() {
                    state.determination_stale = true;
                }
                state.corrections.push(TypeCorrectionRecord {
                    entity: eid,
                    previous_type,
                    corrected_type,
                    invalidated_edges,
                    event_id: event.id,
                });
            }
        }
        "kyc.subject.withdraw-member" => {
            if let Some(eid) = entity_id(p, "entity_id") {
                state.withdrawn_members.insert(eid);
            }
        }
        "kyc.subject.record-enquiry" => {
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
