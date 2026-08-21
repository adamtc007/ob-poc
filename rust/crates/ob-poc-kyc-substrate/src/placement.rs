//! Placement-set generator — EOP-PLAN-KYCUBO-KIT-001 T2 (closes KIT-3).
//!
//! KYC-native board/move types shaped after `semantic-decision-contracts`'
//! gameboard discipline (T0.1 rec c — mirror, do not adopt the *graph-shaped*
//! types: `DesignPosition`/`LegalMove`/`GraphRevision`/`GraphDeltaPreview`
//! carry BPMN authoring-session concepts — focus/viewport, compiler profile,
//! policy identity, edit history — with no honest KYC equivalent; ours are
//! event-stream/precondition-native instead). The bare, graph-agnostic
//! vocabulary (`ABSTENTION_CANDIDATE_ID`, and — see `kyc_ramp_capture.rs` —
//! `GameDispositionKind`/`MoveAttemptOutcome`) genuinely is domain-agnostic
//! and *is* adopted directly from that crate: content-addressed move
//! identity, canonical ordering, an explicit abstention candidate, and a
//! single content hash for the whole placement set.
//!
//! **What "board" means here:** not a fixed layout — the authoritative,
//! inspectable state at one position, recomputed from the folded event
//! stream, over which the lexicon's preconditions determine the next legal
//! transformations (construction-game / graph-rewriting model, not chess).
//! Ratified vision + the instance-authoring/taxonomy-authoring recursive
//! split: `docs/todo/EOP-PLAN-KYCUBO-KIT-T7_Plain-English-Ramp_v0.1.md` §7.
//!
//! Pure: `(state, lexicon, subject) -> PlacementSet`. No I/O, no store, no
//! clock read — the precondition checker never reads `as_of`, so a fixed
//! probe timestamp keeps this generator side-effect-free by construction.
//!
//! **Scope (T2, precondition-tier only).** A move is admitted iff the
//! verb's `LexiconEntry.preconditions` hold against the folded
//! `ControlState` (`check_control_preconditions`, the same oracle the write
//! path uses). Verbs with an empty precondition list are therefore always
//! admitted — the K-G5 "geometry-free" gap from T0.3's pack-closure audit.
//! Likewise the universe enumerated here is `lexicon.entries` — the 21
//! verbs `phase1_lexicon()` declares — not the full dsl.kyc pack; the K-G6
//! declaration-drift gap is a lexicon-closure problem, not a placement-set
//! problem, and is out of this module's scope.
//!
//! **TS.1 §5 re-key (D1 tranche, EOP-DD-KYCUBO-TS.1).** The K-G5 gap above
//! is now closed for the two edge-asserting verbs: `type_geometry_gate`
//! runs FIRST, ahead of the existing stud probe (`check_preconditions`),
//! for `ubo.edge.assert-control`/`ubo.edge.assert-economic-interest` only
//! — TS.1 §1's two constraint layers, in order ("type geometry decides
//! whether a linkage kind is possible at all... the studs already ratified
//! then constrain whether a possible move is legal in this position").
//! `check_type_geometry` (`crate::geometry`) returns `GeometryError`, a
//! type with no relationship to `KycError` (`check_preconditions`'s error)
//! — the two constraint layers are structurally, not just textually,
//! distinct (TS.1 §6 `type_geometry_refuses_impossible_linkage`).
//!
//! **The four genuinely new TS.1 §3 moves** (`assert-type`, `correct-type`,
//! `withdraw-member`, `record-enquiry` — moves 2, 7, 6, 8) NOW join the
//! governed 25-verb `phase1_lexicon()` pack (D1 Part A, EOP-DD-KYCUBO-TS.1) —
//! YAML declaration, op registration, and DB persistence all wired; see
//! `kyc_stream_ops.rs`. They are still enumerated here via
//! `type_registry_candidates` rather than the main per-entry loop below,
//! because they are **entity-scoped** (`TargetBinding.entity_id`), unlike
//! every other subject-scoped verb the main loop probes with a single
//! bare-subject target. `type_registry_candidates` now uses the REAL
//! `LexiconEntry`s (`check_control_preconditions`, the same oracle the
//! write path uses) for the lexicon-declared studs (`EntityRegistered`),
//! layering TWO further positional checks that have no `Precondition`
//! primitive (membership-active for `withdraw-member`; a prior type
//! assertion for `correct-type`) directly against `TypeRegistryState` —
//! the same "no primitive exists, enforced op-layer" pattern
//! `ubo.edge.pierce-nominee`'s nominee-kind check already uses, applied
//! here at the enumeration layer instead of an op's `execute()`.
//! Moves 1 (`admit-member`), 4 (`attach-evidence`), 9 (`construct`) needed
//! no new machinery — P1 recon found them already satisfied by
//! `kyc.subject.register`, the existing `ubo.edge.attach-evidence` (now
//! also type-scoped, `fold/type_registry.rs`), and `preview()` respectively.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::event::IntentEvent;
use crate::fold::control::{check_control_preconditions, check_preconditions, ControlState};
use crate::fold::obligation::ObligationState;
use crate::fold::type_registry::TypeRegistryState;
use crate::geometry::{check_type_geometry, LinkageSource, ALL_PIPES};
use crate::lexicon::LexiconManifest;
use crate::types::{
    AuthorityRef, EdgeId, EntityId, Hash, Principal, SubjectId, TargetBinding, VerbFqn,
};

/// Canonical candidate id for "none of these moves apply" — always present
/// in a `PlacementSet`. Re-exported from the shared, domain-agnostic
/// vocabulary crate rather than duplicated (Phase 2, T7 gameboard-vocabulary
/// adoption — `docs/todo/EOP-PLAN-KYCUBO-KIT-T7_Plain-English-Ramp_v0.1.md`).
pub const NONE_OF_THE_ABOVE: &str = semantic_decision_contracts::ABSTENTION_CANDIDATE_ID;

/// Fixed probe timestamp. `check_control_preconditions` never reads
/// `as_of` — this exists only so `IntentEvent::new` has a value, not to
/// carry meaning; using a clock read here would make this generator
/// impure for no reason.
fn probe_as_of() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is representable")
}

/// Content-addressed identity of one legal move: `"{verb_fqn}::{target}"`.
/// Two moves with the same verb and target always collide to the same id
/// (idempotent enumeration; also the sort key for canonical ordering).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MoveId(pub String);

/// One legal move on the board: a verb bound to a specific target,
/// admitted because its lexicon preconditions hold against the folded
/// state (or because it declares none — K-G5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalMove {
    pub move_id: MoveId,
    pub verb_fqn: VerbFqn,
    pub target: TargetBinding,
}

/// The full set of legal moves at one folded state, canonically ordered
/// and content-hashed. Same `(subject, state, lexicon)` ⇒ bit-identical
/// `PlacementSet` (K-16/33 determinism discipline, applied one layer up
/// from the fold).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementSet {
    pub subject: SubjectId,
    /// Ordered by `move_id` (`BTreeMap` iteration order — no `HashMap`
    /// anywhere in this module). Always includes the abstention move.
    pub moves: Vec<LegalMove>,
    /// Content hash over the ordered move-id list.
    pub board_hash: Hash,
}

impl PlacementSet {
    pub fn abstain_move_id() -> MoveId {
        MoveId(NONE_OF_THE_ABOVE.to_string())
    }

    /// True iff `verb_fqn` appears in the placement set bound to `target`.
    pub fn admits(&self, verb_fqn: &str, target: &TargetBinding) -> bool {
        self.moves
            .iter()
            .any(|m| m.verb_fqn.as_str() == verb_fqn && &m.target == target)
    }
}

/// Verbs whose target is a specific edge in the folded state, rather than
/// the subject itself. Mirrors the `TargetBinding` shape each verb's
/// dispatch handler actually reads (`ob-poc/src/domain_ops/kyc_stream_ops.rs`);
/// duplicated here (not derived from `LexiconEntry`, which carries no arg
/// schema — CLAUDE.md's documented K-G6-adjacent gap) rather than guessed.
fn is_edge_scoped(verb_fqn: &str) -> bool {
    matches!(
        verb_fqn,
        "ubo.edge.verify"
            | "ubo.edge.attach-evidence"
            | "ubo.edge.supersede"
            | "ubo.edge.pierce-nominee"
    )
}

fn move_id_for(verb_fqn: &str, target: &TargetBinding) -> MoveId {
    let target_key = match target.edge_id {
        Some(EdgeId(id)) => format!("edge:{id}"),
        None => "subject".to_string(),
    };
    MoveId(format!("{verb_fqn}::{target_key}"))
}

fn probe_event(subject: SubjectId, verb_fqn: &str, target: TargetBinding) -> IntentEvent {
    IntentEvent::new(
        subject,
        verb_fqn,
        Principal::test_analyst(),
        AuthorityRef("placement-probe".into()),
        target,
        serde_json::Value::Null,
        probe_as_of(),
    )
}

fn board_content_hash(moves: &[LegalMove]) -> Hash {
    let ids: Vec<&str> = moves.iter().map(|m| m.move_id.0.as_str()).collect();
    Hash::of_json(&serde_json::json!({ "moves": ids }))
}

/// Verbs gated by the type-geometry layer (TS.1 §1/§5) — checked BEFORE
/// `check_preconditions`, and only for these two. Every other verb's
/// admission is unchanged from before this tranche.
fn is_geometry_gated(verb_fqn: &str) -> bool {
    matches!(verb_fqn, "ubo.edge.assert-control" | "ubo.edge.assert-economic-interest")
}

/// TS.1 §1's FIRST constraint layer, as an existence check: does there
/// exist at least one geometrically-possible (source, pipe, target) triple
/// among the subject's currently-registered, currently-typed group members?
/// An entity with no type assertion at all has no derivable permitted-pipe
/// set (TS.0 §2 P3: "the proven type determines pipes") — such an entity
/// contributes nothing here until `assert-type` has fired for it, which is
/// a real, intended refusal, not a bug: `enumerate_placement_set` cannot
/// admit a linkage move it cannot evaluate.
fn geometrically_possible(state: &ControlState, type_registry: &TypeRegistryState) -> bool {
    let members: Vec<EntityId> = state.registered_entity_ids.iter().copied().collect();
    for &from in &members {
        let Some(from_type) = type_registry.type_of(from) else { continue };
        for &to in &members {
            if from == to {
                continue;
            }
            let Some(to_type) = type_registry.type_of(to) else { continue };
            for pipe in ALL_PIPES {
                if check_type_geometry(LinkageSource::Entity(from_type), *pipe, to_type).is_ok() {
                    return true;
                }
            }
        }
    }
    false
}

fn entity_move_id(verb_fqn: &str, entity: EntityId) -> MoveId {
    MoveId(format!("{verb_fqn}::entity:{}", entity.0))
}

const ASSERT_TYPE: &str = "kyc.subject.assert-type";
const CORRECT_TYPE: &str = "kyc.subject.correct-type";
const WITHDRAW_MEMBER: &str = "kyc.subject.withdraw-member";
const RECORD_ENQUIRY: &str = "kyc.subject.record-enquiry";
const TYPE_SCOPED_ATTACH_EVIDENCE: &str = "ubo.edge.attach-evidence";

/// Entity-scoped verbs (`TargetBinding.entity_id`) that the main
/// `enumerate_placement_set` loop below must NOT probe with its generic
/// bare-subject target — they are enumerated per-entity by
/// `type_registry_candidates` instead. (`record-enquiry` is subject-scoped,
/// not entity-scoped, and stays in this exclusion list only so its single
/// candidate is emitted exactly once, by `type_registry_candidates`, rather
/// than potentially twice.)
fn is_type_registry_move(fqn: &str) -> bool {
    matches!(fqn, ASSERT_TYPE | CORRECT_TYPE | WITHDRAW_MEMBER | RECORD_ENQUIRY)
}

/// The four D1 TS.1 §3 moves now in `phase1_lexicon()` (module doc), plus
/// the type-scoped half of `attach-evidence` — the edge-scoped half stays
/// handled entirely by the main `lexicon.entries` loop, unchanged. All
/// lexicon-declared studs — `EntityRegistered`, and (Phase 2 of the
/// tree-cleanup follow-up tranche, EOP-STATE-KYCUBO-D1 §4/§7)
/// `MembershipActive`/`PriorTypeAsserted` — are checked via the single REAL
/// `check_control_preconditions` oracle; no hand-rolled `TypeRegistryState`
/// check remains here for withdraw-member/correct-type.
fn type_registry_candidates(
    subject: SubjectId,
    state: &ControlState,
    type_registry: &TypeRegistryState,
    lexicon: &LexiconManifest,
) -> Vec<LegalMove> {
    let mut moves = Vec::new();

    // record-enquiry (row 8): "group exists" — true by construction, the
    // subject this workbook is open against IS the group. No precondition
    // declared, so no oracle probe needed.
    if lexicon.get(RECORD_ENQUIRY).is_some() {
        let subject_target = TargetBinding::for_subject(subject);
        moves.push(LegalMove {
            move_id: move_id_for(RECORD_ENQUIRY, &subject_target),
            verb_fqn: VerbFqn(RECORD_ENQUIRY.to_string()),
            target: subject_target,
        });
    }

    let assert_type_entry = lexicon.get(ASSERT_TYPE);
    let withdraw_member_entry = lexicon.get(WITHDRAW_MEMBER);
    let correct_type_entry = lexicon.get(CORRECT_TYPE);

    for &entity in &state.registered_entity_ids {
        let target = TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) };

        // assert-type (row 2): "entity exists" (EntityRegistered, checked
        // via the real oracle). Reclassification stays legal (last-wins,
        // mirrors `structure_class`), so always admitted once registered,
        // regardless of any prior type.
        if let Some(entry) = assert_type_entry {
            let probe = probe_event(subject, ASSERT_TYPE, target.clone());
            if check_control_preconditions(entry, state, type_registry, &probe).is_ok() {
                moves.push(LegalMove {
                    move_id: entity_move_id(ASSERT_TYPE, entity),
                    verb_fqn: VerbFqn(ASSERT_TYPE.to_string()),
                    target: target.clone(),
                });
            }
        }

        // withdraw-member (row 6): "membership exists and is active" —
        // EntityRegistered + MembershipActive, both via the oracle (Phase 2:
        // MembershipActive is a real Precondition now, not a hand-rolled
        // TypeRegistryState check here).
        if let Some(entry) = withdraw_member_entry {
            let probe = probe_event(subject, WITHDRAW_MEMBER, target.clone());
            if check_control_preconditions(entry, state, type_registry, &probe).is_ok() {
                moves.push(LegalMove {
                    move_id: entity_move_id(WITHDRAW_MEMBER, entity),
                    verb_fqn: VerbFqn(WITHDRAW_MEMBER.to_string()),
                    target: target.clone(),
                });
            }
        }

        // correct-type (row 7): EntityRegistered via the oracle, plus "a
        // type was already asserted" (no Precondition primitive — nothing
        // to correct otherwise, that is assert-type's job).
        if let Some(entry) = correct_type_entry {
            let probe = probe_event(subject, CORRECT_TYPE, target.clone());
            if check_control_preconditions(entry, state, type_registry, &probe).is_ok() {
                moves.push(LegalMove {
                    move_id: entity_move_id(CORRECT_TYPE, entity),
                    verb_fqn: VerbFqn(CORRECT_TYPE.to_string()),
                    target: target.clone(),
                });
            }
        }

        // attach-evidence, type-scoped half (row 4): "target assertion
        // exists and is not withdrawn" — a type is asserted, member active.
        if type_registry.type_of(entity).is_some() && !type_registry.is_withdrawn(entity) {
            moves.push(LegalMove {
                move_id: entity_move_id(TYPE_SCOPED_ATTACH_EVIDENCE, entity),
                verb_fqn: VerbFqn(TYPE_SCOPED_ATTACH_EVIDENCE.to_string()),
                target,
            });
        }
    }

    moves
}

/// Enumerate the legal placement set for `subject` given its folded control,
/// obligation, and type-registry `state` and the `lexicon`'s registered verb
/// set. Takes all three folds (T6.1(a) extended to the type axis — see
/// module doc) even though only the two geometry-gated verbs read
/// `type_registry` today.
///
/// For edge-scoped verbs, one candidate move is probed per edge present in
/// `state.edges` (including superseded edges — K-13 supersede-never-delete
/// means they remain addressable targets; precondition checks, not this
/// enumeration, are what should eventually exclude them, per T6). For all
/// other verbs, one subject-scoped candidate is probed — EXCEPT the two
/// geometry-gated verbs, which are additionally required to have at least
/// one geometrically-possible triple among typed group members before the
/// existing stud probe even runs (TS.1 §1: type geometry first, position
/// second).
pub fn enumerate_placement_set(
    subject: SubjectId,
    state: &ControlState,
    obligation: &ObligationState,
    type_registry: &TypeRegistryState,
    lexicon: &LexiconManifest,
) -> PlacementSet {
    let mut candidates: BTreeMap<MoveId, LegalMove> = BTreeMap::new();

    for entry in lexicon.entries.values() {
        let fqn = entry.fqn.as_str();
        if is_type_registry_move(fqn) {
            // Entity-scoped; enumerated per-entity below, not with this
            // loop's single bare-subject probe target.
            continue;
        }
        if is_geometry_gated(fqn) && !geometrically_possible(state, type_registry) {
            continue;
        }
        let targets: Vec<TargetBinding> = if is_edge_scoped(fqn) {
            state
                .edges
                .keys()
                .map(|edge_id| TargetBinding::for_edge(subject, *edge_id))
                .collect()
        } else {
            vec![TargetBinding::for_subject(subject)]
        };

        for target in targets {
            let probe = probe_event(subject, fqn, target.clone());
            if check_preconditions(entry, state, obligation, type_registry, &probe).is_ok() {
                let id = move_id_for(fqn, &target);
                candidates.insert(
                    id.clone(),
                    LegalMove {
                        move_id: id,
                        verb_fqn: entry.fqn.clone(),
                        target,
                    },
                );
            }
        }
    }

    for m in type_registry_candidates(subject, state, type_registry, lexicon) {
        candidates.insert(m.move_id.clone(), m);
    }

    let abstain_id = PlacementSet::abstain_move_id();
    candidates.insert(
        abstain_id.clone(),
        LegalMove {
            move_id: abstain_id,
            verb_fqn: VerbFqn(NONE_OF_THE_ABOVE.to_string()),
            target: TargetBinding::default(),
        },
    );

    // BTreeMap<MoveId, _> iteration is already canonically (move_id-)sorted.
    let moves: Vec<LegalMove> = candidates.into_values().collect();
    let board_hash = board_content_hash(&moves);

    PlacementSet {
        subject,
        moves,
        board_hash,
    }
}
