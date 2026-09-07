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
//! verbs `assembly_lexicon()` declares — not the full dsl.kyc pack; the K-G6
//! declaration-drift gap is a lexicon-closure problem, not a placement-set
//! problem, and is out of this module's scope.
//!
//! **TS.1 §5 re-key (D1 tranche, EOP-DD-KYCUBO-TS.1) — CORRECTED by TS.5
//! (2026-08-22).** This block previously named a function,
//! `type_geometry_gate`, that was never actually built anywhere in the
//! crate — the K-G5 gap it claimed to close was, in fact, still open:
//! `geometrically_possible` (below) was an EXISTENCE-only scan ("does any
//! legal triple exist among typed registered members anywhere"), and
//! `check_preconditions` — the real append-path oracle this generator also
//! uses — never called `check_type_geometry` at all. Proven live against
//! Postgres 2026-08-21 (an illegal `ManagementMandate`/natural-person edge
//! appended through the real governed op). **EOP-DD-KYCUBO-TS.5 (RATIFIED
//! 2026-08-22) closes it for real:** `Precondition::TypeGeometryPermits`
//! (R1) is now a genuine `check_preconditions` arm — attached to
//! `kyc_ubo.assert.edge.control` and `kyc_ubo.assert.edge.economic-interest`
//! (TS.5 §6 Q1/Q2; a piercing-mode `assert-control` call reaches it too,
//! same entry, since `kyc_ubo.assert.edge.nominee-piercing`'s own `LexiconEntry` was
//! retired TS.6 P2, folded into a macro composing the two) — so both this
//! generator and
//! the real append path are covered BY CONSTRUCTION, through the one
//! function both already called. `geometrically_possible` (R2) now calls
//! the identical evaluation helper (`fold::control::evaluate_type_geometry`)
//! the precondition arm uses, rather than a parallel hand-rolled scan, so
//! the two cannot drift apart again. `check_type_geometry`
//! (`crate::geometry`) still returns `GeometryError`, distinct from
//! `KycError` (`check_preconditions`'s error) — the two constraint layers
//! are structurally, not just textually, distinct (TS.1 §6
//! `type_geometry_refuses_impossible_linkage`; TS.5 §5
//! `geometry_refusal_is_distinguishable_from_stud_refusal`).
//!
//! **The four genuinely new TS.1 §3 moves** (`assert-type`, `correct-type`,
//! `withdraw-member`, `record-enquiry` — moves 2, 7, 6, 8) NOW join the
//! governed 25-verb `assembly_lexicon()` pack (D1 Part A, EOP-DD-KYCUBO-TS.1) —
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
//! `kyc_ubo.assert.edge.control`'s pierced-from nominee-kind check (the
//! `pierce-nominee` macro's admission gate, TS.6 P2) already uses, applied
//! here at the enumeration layer instead of an op's `execute()`.
//! Moves 1 (`admit-member`), 4 (`attach-evidence`), 9 (`construct`) needed
//! no new machinery — P1 recon found them already satisfied by
//! `kyc_ubo.assert.subject.register`, the existing `kyc_ubo.assert.edge.evidence` (now
//! also type-scoped, `fold/type_registry.rs`), and `preview()` respectively.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::event::IntentEvent;
use crate::fold::control::{check_preconditions, ControlState};
use crate::fold::type_registry::TypeRegistryState;
use crate::geometry::{check_type_geometry, EntityType, LinkageSource, ALL_ENTITY_TYPES, ALL_PIPES};
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
    /// The specific `(from, kind, to)` triple this move proposes, for the two
    /// geometry-gated edge verbs. `None` for every other verb.
    ///
    /// **TS.5 R2, closed 2026-08-22.** The board used to emit ONE bare-subject
    /// candidate per geometry-gated verb, so there was nowhere to put a triple
    /// — and a per-triple gate with nothing per-triple to gate is a
    /// contradiction. TS.1 §1 defines the legal move set as *derived from* the
    /// type geometry, which decides possibility "between two types"; a move set
    /// derived from that is per-triple by construction.
    ///
    /// The triple lives HERE and not on `TargetBinding` deliberately:
    /// `TargetBinding` answers "what does this event target" — for
    /// `assert.edge.control` that is a freshly-minted `edge_id`, not the
    /// endpoints. The triple is what the board PROPOSES, i.e. payload. Putting
    /// it in `TargetBinding` would conflate the two and ripple through every
    /// event, the append path and the projections.
    pub proposed_edge: Option<ProposedEdge>,
    /// `Some(_)` only for `place` candidates (2026-09-07, audit item 3).
    /// `place` offers the entity TYPES that may legally be added
    /// (EOP-VS-UBO-GAME-001 §3.4 R9) — never a pre-enumerated set of known
    /// entity ids, since a brand-new entity has no id yet for the board to
    /// enumerate (§2: "the game writes metadata about entities, never
    /// entities; which specific entity is a lookup"). `target.entity_id` is
    /// `None` on these candidates by construction — the caller supplies the
    /// real id as an argument to `KycWorkbook::stage()`, matched against
    /// this field, not against `target`.
    pub proposed_entity_type: Option<EntityType>,
}

/// A `(from, kind, to)` triple the board is offering as a legal move.
/// `kind_wire` is the wire string from `EDGE_KIND_WIRE_VALUES` — the same
/// vocabulary the real op accepts, so a caller can lift it straight into the
/// payload it submits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProposedEdge {
    pub from: EntityId,
    pub to: EntityId,
    pub kind_wire: String,
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
/// `kyc_ubo.assert.edge.nominee-piercing` dropped TS.6 P2 (K-G7) — its `LexiconEntry`
/// (and the verb it enumerated) no longer exists; this generator only
/// iterates real lexicon entries, so it never reaches this function for
/// that fqn.
fn is_edge_scoped(verb_fqn: &str) -> bool {
    matches!(verb_fqn, "kyc_ubo.assert.edge.evidence" | "kyc_ubo.assert.edge.disconnect")
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
/// `check_preconditions`. EOP-VS-UBO-GAME-001 T3 merged the former two
/// FQNs (`control`, `economic-interest`) into one (`connect`) — same gate,
/// one name now. Every other verb's admission is unchanged from before
/// this tranche.
fn is_geometry_gated(verb_fqn: &str) -> bool {
    matches!(verb_fqn, "kyc_ubo.assert.edge.connect")
}

/// TS.5 R2 (closed 2026-08-22): there is no separate existence scan any more.
/// `geometrically_possible(state, type_registry) -> bool` is DELETED, not
/// rewritten. It asked "does ANY legal triple exist among typed members
/// anywhere" and could not ask "is THIS triple legal", because the probe event
/// it gated carried `payload: Null` — so the geometry precondition found no
/// triple, returned `Unevaluable`, and admitted by the CTN-2e default. The
/// coarse scan was bolted on top to compensate.
///
/// The replacement probes ONE EVENT PER PERMITTED TRIPLE, with the triple in
/// the payload, so `check_preconditions` evaluates the real triple. Preview and
/// append then consult the same geometry for the same triple **by
/// construction** — not because two functions were kept in step, which is
/// exactly the drift R1's single-chokepoint design exists to prevent.
///
/// An entity with no type assertion contributes no candidate here (TS.0 §2 P3:
/// "the proven type determines pipes"). That is a real, intended silence in the
/// PREVIEW, not a refusal: R5/R6 are untouched, and an untyped endpoint the
/// caller names explicitly still ADMITS at the append, recording
/// geometry-unevaluable. Enforcement narrows what may be ASSERTED; the board
/// simply cannot propose a triple it has no types to evaluate.
fn geometry_probe_payload(from: EntityId, to: EntityId, kind_wire: &str) -> serde_json::Value {
    serde_json::json!({
        "from_entity_id": from.0.to_string(),
        "to_entity_id": to.0.to_string(),
        "edge_id": uuid::Uuid::nil().to_string(),
        "kind": kind_wire,
    })
}

fn entity_move_id(verb_fqn: &str, entity: EntityId) -> MoveId {
    MoveId(format!("{verb_fqn}::entity:{}", entity.0))
}

const PLACE: &str = "kyc_ubo.assert.subject.place";
const REMOVE: &str = "kyc_ubo.assert.subject.remove";
// `kyc_ubo.assert.subject.type-correction` DISSOLVED (EOP-VS-UBO-GAME-001
// T2 §8 Q1, 2026-08-27) — its candidate-enumeration block, `CORRECT_TYPE`
// const, and `is_type_registry_move` membership are all deleted in this
// diff, alongside its lexicon entry, fold arm, and op. Correcting a type is
// `remove` then `place`, two ordinary moves through the arms below.
const RECORD_ENQUIRY: &str = "kyc_ubo.assert.subject.enquiry";
const TYPE_SCOPED_ATTACH_EVIDENCE: &str = "kyc_ubo.assert.edge.evidence";

/// Entity-scoped verbs (`TargetBinding.entity_id`) that the main
/// `enumerate_placement_set` loop below must NOT probe with its generic
/// bare-subject target — they are enumerated per-entity by
/// `place_and_remove_candidates` instead. (`record-enquiry` is
/// subject-scoped, not entity-scoped, and stays in this exclusion list only
/// so its single candidate is emitted exactly once, rather than potentially
/// twice.)
fn is_type_registry_move(fqn: &str) -> bool {
    matches!(fqn, PLACE | REMOVE | RECORD_ENQUIRY)
}

/// `place`/`remove` (EOP-VS-UBO-GAME-001 §3.1/§3.2), plus the type-scoped
/// half of `attach-evidence` — the edge-scoped half stays handled entirely
/// by the main `lexicon.entries` loop, unchanged. All lexicon-declared
/// studs are checked via the single REAL `check_control_preconditions`
/// oracle; no hand-rolled `TypeRegistryState` check lives here.
///
/// **T1's P4 deferral, closed for the re-placeable population (T2 P2
/// finding — reported before implementing, see state-of-play §5n):**
/// `place` candidates are entity-scoped and real, not the old generic
/// bare-subject "register anything" candidate — "already placed" is now a
/// board property (§8 Q1's `NotCurrentlyPlaced`), not a precondition that
/// only fired at commit. The board can enumerate exactly two populations:
/// the subject's own not-yet-placed entity (always known, no discovery data
/// needed) and every currently-withdrawn (previously-placed, now
/// re-placeable) member. A brand-new, non-subject entity's first-ever
/// placement has no board candidate — there is no data source anywhere in
/// the fold for "entities that could exist but never have" — so it is not
/// reachable through `KycWorkbook::stage()`'s frontier gate, only through
/// the op layer directly (`check_preconditions` only, no board gate there).
fn place_and_remove_candidates(
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
            proposed_edge: None,
            proposed_entity_type: None,
        });
    }

    let remove_entry = lexicon.get(REMOVE);

    // place (2026-09-07, audit item 3 — "a human can place exactly ONE
    // block ever"): type-level candidates, NOT a pre-enumerated set of
    // known entity ids (EOP-VS-UBO-GAME-001 §3.4 R9 — "place offers the
    // entity types that may be added"; §2 — "the game writes metadata
    // about entities, never entities; which specific entity is a lookup").
    // A brand-new entity has no id yet for the board to enumerate — the
    // caller supplies it as an argument to `KycWorkbook::stage()`, matched
    // against `proposed_entity_type`, not `target`.
    //
    // `place`'s only lexicon precondition (`NotCurrentlyPlaced`) is vacuous
    // without a concrete `entity_id` (`fold::control::check_preconditions`),
    // so it cannot narrow a type-level probe — that check runs for real
    // against the caller's actual id at `stage()`/append time instead
    // (same discipline as every other vacuous-when-probed-abstractly stud;
    // see that function's doc comments). What DOES narrow the type-level
    // offer is the type-GEOMETRY layer (TS.1 §1's first constraint layer,
    // `geometry::check_type_geometry` — the same table `connect`'s own
    // candidate loop above reads forward): on an empty board nothing exists
    // yet to connect to, so every catalogued type is a legal first
    // placement; once the board has typed active members, a candidate type
    // is offered only if some pipe legally connects it (either direction)
    // to at least one of them.
    if lexicon.get(PLACE).is_some() {
        let active_typed_members: Vec<EntityType> = state
            .registered_entity_ids
            .iter()
            .filter(|&&e| !type_registry.is_withdrawn(e))
            .filter_map(|&e| type_registry.type_of(e))
            .collect();

        for &candidate_type in ALL_ENTITY_TYPES {
            let legal = active_typed_members.is_empty()
                || active_typed_members.iter().any(|&member_type| {
                    ALL_PIPES.iter().any(|&pipe| {
                        check_type_geometry(LinkageSource::Entity(candidate_type), pipe, member_type)
                            .is_ok()
                            || check_type_geometry(LinkageSource::Entity(member_type), pipe, candidate_type)
                                .is_ok()
                    })
                });
            if legal {
                moves.push(LegalMove {
                    move_id: MoveId(format!("{PLACE}::type:{candidate_type:?}")),
                    verb_fqn: VerbFqn(PLACE.to_string()),
                    target: TargetBinding::for_subject(subject),
                    proposed_edge: None,
                    proposed_entity_type: Some(candidate_type),
                });
            }
        }
    }

    // remove: every registered entity. Whether a candidate is ACTUALLY
    // still active (and so removable) is decided solely by
    // `check_control_preconditions`'s `MembershipActive` arm below, not a
    // hand-rolled duplicate here (no_stud_is_duplicated).
    if let Some(entry) = remove_entry {
        for &entity in &state.registered_entity_ids {
            let target = TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) };
            let probe = probe_event(subject, REMOVE, target.clone());
            if check_preconditions(entry, state, type_registry, &probe).is_ok() {
                moves.push(LegalMove {
                    move_id: entity_move_id(REMOVE, entity),
                    verb_fqn: VerbFqn(REMOVE.to_string()),
                    target,
                    proposed_edge: None,
                    proposed_entity_type: None,
                });
            }
        }
    }

    // correct-type: DISSOLVED (EOP-VS-UBO-GAME-001 T2 §8 Q1, 2026-08-27) —
    // no candidate block survives it here, alongside its deleted lexicon
    // entry, fold arm, and op. Correcting a type is `remove` then `place`,
    // two ordinary moves through the blocks above.

    // attach-evidence, type-scoped half (row 4): "target assertion exists
    // and is not withdrawn" — a type is asserted, member active.
    for &entity in &state.registered_entity_ids {
        if type_registry.type_of(entity).is_some() && !type_registry.is_withdrawn(entity) {
            let target = TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) };
            moves.push(LegalMove {
                move_id: entity_move_id(TYPE_SCOPED_ATTACH_EVIDENCE, entity),
                verb_fqn: VerbFqn(TYPE_SCOPED_ATTACH_EVIDENCE.to_string()),
                target,
                proposed_edge: None,
                proposed_entity_type: None,
            });
        }
    }

    moves
}

/// Enumerate the legal placement set for `subject` given its folded control
/// and type-registry `state` and the `lexicon`'s registered verb set. Was
/// three folds (T6.1(a) extended to the type axis) — the obligation
/// parameter dropped with `ObligationState` (EOP-DD-UBO-CLEANOUT-001 T6 P2,
/// 2026-09-07); only the two geometry-gated verbs read `type_registry` today.
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
        if is_geometry_gated(fqn) {
            // TS.5 R2: one probe per candidate triple, each carrying the triple
            // in its payload, so `check_preconditions` evaluates THAT triple.
            // EOP-VS-UBO-GAME-001 T3: `connect`'s payload always carries a
            // real `kind` (including "economic_interest" as an ordinary
            // value now) — no more single-wire special case; every kind in
            // `EDGE_KIND_WIRE_VALUES` is enumerated uniformly.
            let members: Vec<EntityId> = state.registered_entity_ids.iter().copied().collect();
            let wires: &[&str] = crate::fold::control::EDGE_KIND_WIRE_VALUES;
            for &from in &members {
                if type_registry.type_of(from).is_none() {
                    continue;
                }
                for &to in &members {
                    if from == to || type_registry.type_of(to).is_none() {
                        continue;
                    }
                    for wire in wires {
                        let target = TargetBinding::for_subject(subject);
                        let mut probe = probe_event(subject, fqn, target.clone());
                        probe.payload = geometry_probe_payload(from, to, wire);
                        if check_preconditions(entry, state, type_registry, &probe)
                            .is_err()
                        {
                            continue;
                        }
                        let proposed = ProposedEdge {
                            from,
                            to,
                            kind_wire: (*wire).to_string(),
                        };
                        let id = MoveId(format!(
                            "{fqn}::triple:{}:{wire}:{}",
                            from.0, to.0
                        ));
                        candidates.insert(
                            id.clone(),
                            LegalMove {
                                move_id: id,
                                verb_fqn: entry.fqn.clone(),
                                target,
                                proposed_edge: Some(proposed),
                                proposed_entity_type: None,
                            },
                        );
                    }
                }
            }
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
            if check_preconditions(entry, state, type_registry, &probe).is_ok() {
                let id = move_id_for(fqn, &target);
                candidates.insert(
                    id.clone(),
                    LegalMove {
                        move_id: id,
                        verb_fqn: entry.fqn.clone(),
                        target,
                        proposed_edge: None,
                        proposed_entity_type: None,
                    },
                );
            }
        }
    }

    for m in place_and_remove_candidates(subject, state, type_registry, lexicon) {
        candidates.insert(m.move_id.clone(), m);
    }

    let abstain_id = PlacementSet::abstain_move_id();
    candidates.insert(
        abstain_id.clone(),
        LegalMove {
            move_id: abstain_id,
            verb_fqn: VerbFqn(NONE_OF_THE_ABOVE.to_string()),
            target: TargetBinding::default(),
            proposed_edge: None,
            proposed_entity_type: None,
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
