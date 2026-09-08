//! Board-move generator — Adam's fuzzability completion criterion
//! (EOP-VS-UBO-GAME-001 §3.4 R1-R9, C2): "if the board and moves are not
//! fuzzable, the job is not done."
//!
//! Distinct from `gen_events` (`lib.rs`): that generator builds
//! plausible-but-adversarial `IntentEvent`s directly against the FOLD layer
//! (`fold_control`), including deliberately-unknown verbs and garbage
//! payloads, to keep the total-dispatch fallthrough hot. This generator
//! instead draws ONLY from `enumerate_placement_set`'s real legal move set
//! at each step — the BOARD, not the fold — building actual legal move
//! sequences the same way a real session would: recompute the board,
//! pick one of the moves it genuinely offers, build the event that move
//! declares, thread any system-minted id (edge_id, an evidence event's own
//! id as a citation target) forward into later picks. No domain knowledge,
//! no hand-written valid sequences — every choice is a `Tape::choice` over
//! a declared, exported set (`ENTITY_TYPE_WIRE_VALUES`, `PROOF_KIND_WIRE_VALUES`)
//! or over the board's own offered move list.
//!
//! `kind_wire` for `connect` moves comes directly off the `LegalMove`'s own
//! `ProposedEdge` (concrete triples, per R9 — "connect offers... the pipes
//! valid out of it and the targets each may reach"), never re-derived —
//! there is nothing left to guess.

use ob_poc_kyc_substrate::{
    assembly_lexicon, check_preconditions, enumerate_placement_set, fold_control,
    fold_type_registry, AuthorityRef, ControlState, EdgeId, EntityId, EntityType, IntentEvent,
    LegalMove, Principal, SubjectId, TargetBinding, TypeRegistryState, ALL_ENTITY_TYPES,
    NONE_OF_THE_ABOVE, PROOF_KIND_WIRE_VALUES,
};
use uuid::Uuid;

use crate::Tape;

/// `EntityType` → its wire string. `ALL_ENTITY_TYPES` and
/// `ENTITY_TYPE_WIRE_VALUES` are declared in the same order (both doc'd as
/// "canonical order matching the catalogue's own family grouping",
/// `geometry.rs`/`fold/type_registry.rs`) — verified by a debug_assert
/// rather than trusted blindly, since a future reordering of either would
/// silently corrupt this mapping.
fn entity_type_wire(t: EntityType) -> &'static str {
    let idx = ALL_ENTITY_TYPES
        .iter()
        .position(|&e| e == t)
        .expect("proposed_entity_type is always a real ALL_ENTITY_TYPES member");
    debug_assert_eq!(ALL_ENTITY_TYPES.len(), ob_poc_kyc_substrate::ENTITY_TYPE_WIRE_VALUES.len());
    ob_poc_kyc_substrate::ENTITY_TYPE_WIRE_VALUES[idx]
}

fn garbage_text(tape: &mut Tape) -> String {
    let len = tape.choice(12);
    (0..len).map(|_| char::from(b'a' + (tape.u8() % 26))).collect()
}

/// Deterministic, tape-independent id minting. `EntityId`/`EdgeId` become
/// `BTreeMap`/`BTreeSet` keys throughout the substrate (`registered_entity_ids`,
/// `ControlState.edges`), so their concrete VALUES — not just their
/// presence — decide iteration order, which decides move-enumeration order,
/// which decides what `tape.choice(index)` picks. A `Uuid::new_v4()`-minted
/// id (system RNG, not tape-derived) makes the same tape bytes produce a
/// DIFFERENT sequence on every run — exactly the reproducibility libFuzzer's
/// minimize/replay model depends on. `Uuid::new_v5` over a fixed namespace
/// plus a monotonic tag is a pure function of `(tag, seq)` alone.
const DETERMINISTIC_NAMESPACE: Uuid = Uuid::from_bytes([
    0xb0, 0xa2, 0xd7, 0x3c, 0x5e, 0x1a, 0x4f, 0x8b, 0x9c, 0x2d, 0x6e, 0x71, 0x0a, 0x3f, 0xc4, 0x88,
]);

fn deterministic_uuid(tag: &str, seq: u64) -> Uuid {
    Uuid::new_v5(&DETERMINISTIC_NAMESPACE, format!("{tag}:{seq}").as_bytes())
}

/// One realized step: the actual `IntentEvent` built for a chosen
/// `LegalMove`, plus whatever id pool it feeds.
pub struct BoardStep {
    pub verb_fqn: String,
    pub event: IntentEvent,
}

/// A generated legal-move sequence plus the bookkeeping needed to state
/// R8/geometry/determinism properties over it without re-deriving anything
/// the generator already knows.
pub struct BoardSequence {
    pub subject: SubjectId,
    pub lexicon: ob_poc_kyc_substrate::LexiconManifest,
    pub steps: Vec<BoardStep>,
    /// Every (entity, type) this sequence ever placed, in placement order.
    /// An entity may appear more than once (withdrawn and re-placed).
    pub placements: Vec<(EntityId, EntityType)>,
}

impl BoardSequence {
    pub fn events(&self) -> Vec<&IntentEvent> {
        self.steps.iter().map(|s| &s.event).collect()
    }

    pub fn control_at(&self, upto: usize) -> ControlState {
        fold_control(&self.events()[..upto.min(self.steps.len())])
    }

    pub fn type_registry_at(&self, upto: usize) -> TypeRegistryState {
        fold_type_registry(&self.events()[..upto.min(self.steps.len())])
    }
}

/// Build the real `IntentEvent` a chosen `LegalMove` declares. Returns
/// `None` only for moves this generator has no payload rule for yet — see
/// the exhaustive match; an unmatched arm is a P0 finding (a board move
/// that cannot be generated from declared vocabulary alone), not silently
/// skipped.
#[allow(clippy::too_many_arguments)]
fn build_event(
    tape: &mut Tape,
    subject: SubjectId,
    mv: &LegalMove,
    edge_pool: &[EdgeId],
    citation_pool: &[Uuid],
    reusable_entity_ids: &[EntityId],
    seq: u64,
) -> (IntentEvent, Option<EdgeId>, Option<(EntityId, EntityType)>) {
    let fqn = mv.verb_fqn.as_str();
    let actor = Principal { actor_id: Uuid::new_v4(), role: "board-fuzz".to_string() };
    let authority = AuthorityRef("board-fuzz-authority".to_string());
    let as_of = chrono::DateTime::from_timestamp(1_700_000_000 + seq as i64, 0)
        .expect("fixed-base timestamp is always in range");

    let (payload, minted_edge, placed): (serde_json::Value, Option<EdgeId>, Option<(EntityId, EntityType)>) =
        match fqn {
            "kyc_ubo.assert.subject.place" => {
                let entity_type = mv
                    .proposed_entity_type
                    .expect("place candidates always carry proposed_entity_type");
                // The board offers `place` type-level only (R9 — "place
                // offers types, not ids"); WHICH entity gets that type is
                // the caller's choice — `KycWorkbook::stage()` re-checks
                // `NotCurrentlyPlaced` against whatever real id the caller
                // supplies. Most of the time mint a fresh id (builds a
                // multi-entity ownership graph `connect` can then wire
                // up); sometimes reuse a currently-legal id from
                // `reusable_entity_ids` — which always offers
                // `EntityId(subject.0)` first when the subject isn't
                // currently placed, mirroring `canonical_event_shape`'s
                // real default (`entity-id` omitted → `subject.0`,
                // `ob-poc-kyc-seam/src/canonical.rs`'s
                // `place_defaults_entity_id_to_subject` test). Without
                // ever exercising that default the subject itself never
                // gets a type, and `freeze`'s `EntityTypeSupportsStrategy`
                // precondition (reads `EntityId(subject.0)` specifically)
                // can never pass — no generated sequence would ever reach
                // `freeze` at all.
                let entity = if !reusable_entity_ids.is_empty() && tape.choice(3) == 0 {
                    reusable_entity_ids[tape.choice(reusable_entity_ids.len())]
                } else {
                    EntityId(deterministic_uuid("entity", seq))
                };
                let payload = serde_json::json!({
                    "entity_id": entity.0,
                    "entity_type": entity_type_wire(entity_type),
                });
                (payload, None, Some((entity, entity_type)))
            }
            "kyc_ubo.assert.subject.remove" => {
                let entity = mv.target.entity_id.expect("remove candidates are entity-scoped");
                (serde_json::json!({ "entity_id": entity.0 }), None, None)
            }
            "kyc_ubo.assert.subject.enquiry" => {
                let payload = if tape.bool() {
                    serde_json::json!({
                        "sources_consulted": [garbage_text(tape)],
                        "searches_run": [garbage_text(tape)],
                    })
                } else {
                    serde_json::json!({})
                };
                (payload, None, None)
            }
            "kyc_ubo.assert.edge.connect" => {
                let proposed = mv
                    .proposed_edge
                    .as_ref()
                    .expect("connect candidates always carry a proposed_edge triple");
                let edge = EdgeId(deterministic_uuid("edge", seq));
                let mut payload = serde_json::json!({
                    "edge_id": edge.0,
                    "from_entity_id": proposed.from.0,
                    "to_entity_id": proposed.to.0,
                    "kind": proposed.kind_wire,
                });
                // EOP-DD-UBO-BASES-001 §6, closure tranche P3a (2026-09-08):
                // `PercentageIsBounded` now REFUSES a percentage outright on
                // any kind that carries no quantity — only
                // `economic_interest`/`voting_rights` are bounded, not
                // merely-ignored, for every other kind. Attaching an
                // adversarial percentage there would make the board offer
                // an ILLEGAL move (C2 violation) — this generator must
                // match the precondition's scope, not just its old
                // (unqualified) shape. Percentage is generated ONLY for the
                // two quantity-carrying kinds, and bounded for both.
                let carries_quantity =
                    matches!(proposed.kind_wire.as_str(), "economic_interest" | "voting_rights");
                if carries_quantity && tape.bool() {
                    payload["percentage"] = serde_json::json!(tape.bounded_percentage());
                }
                (payload, Some(edge), None)
            }
            "kyc_ubo.assert.edge.evidence" => {
                let kind = PROOF_KIND_WIRE_VALUES[tape.choice(PROOF_KIND_WIRE_VALUES.len())];
                let payload = serde_json::json!({
                    "kind": kind,
                    "source": garbage_text(tape),
                    "date": "2026-09-08",
                });
                (payload, None, None)
            }
            "kyc_ubo.assert.edge.disconnect" => (serde_json::json!({}), None, None),
            "kyc_ubo.assert.edge.retract" => {
                // The board offers `retract` bare-subject with no
                // precondition (record-a-fact move) regardless of whether a
                // real citation exists yet — draw a real one when the pool
                // has grown one, garbage otherwise (both are legal calls;
                // only the FOLD's citation lookup differs, never the move's
                // legality).
                let citation = if !citation_pool.is_empty() && tape.bool() {
                    citation_pool[tape.choice(citation_pool.len())]
                } else {
                    Uuid::new_v4()
                };
                (serde_json::json!({ "citation_id": citation }), None, None)
            }
            "kyc_ubo.assert.entity.identity"
            | "kyc_ubo.assert.entity.screening"
            | "kyc_ubo.assert.entity.risk" => {
                let payload = serde_json::json!({
                    "obligation-id": Uuid::new_v4(),
                    "subject-id": subject.0,
                    "state": "in_progress",
                });
                (payload, None, None)
            }
            "kyc_ubo.decide.determination.freeze" => (serde_json::json!({}), None, None),
            other => panic!(
                "P0 finding: board offered {other} but the generator has no payload rule for it \
                 — either a new move needs one, or this FQN should not be board-offered at all"
            ),
        };

    let target = if fqn == "kyc_ubo.assert.edge.connect" {
        // R9/TS.5 R2: connect's target is bare-subject; the triple is the
        // move's payload, not its address (`placement.rs`'s own doc — a
        // per-triple target would make the freshly-minted edge id
        // unmatchable against any legal move).
        TargetBinding::for_subject(subject)
    } else if fqn == "kyc_ubo.assert.subject.place" {
        // `place`'s board candidate is bare-subject (type-level, R9 — "the
        // board offers types, not ids") but the REAL caller always sets
        // `target.entity_id` to whatever id it picked
        // (`canonical_event_shape`'s `place` arm, `ob-poc-kyc-seam`).
        // Without this, `NotCurrentlyPlaced` (which reads
        // `event.target.entity_id`, not the payload) is vacuous on every
        // generated `place` — silently letting the generator re-type an
        // already-active entity with no `remove` in between, which is not
        // a legal move the real system would ever accept.
        let entity = placed.map(|(e, _)| e).expect("place always sets `placed`");
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) }
    } else if fqn == "kyc_ubo.assert.edge.evidence" && mv.target.edge_id.is_none() && mv.target.entity_id.is_none()
    {
        // Defensive: every real evidence candidate carries edge_id or
        // entity_id; if this ever fires it is itself a finding.
        TargetBinding::for_subject(subject)
    } else {
        mv.target.clone()
    };
    let _ = edge_pool; // reserved for future edge-targeted-move variety

    let event = IntentEvent::new(subject, fqn, actor, authority, target, payload, as_of).with_seq(seq);
    (event, minted_edge, placed)
}

/// Generate a legal board-move sequence of `1..=cap` steps. At each step,
/// recompute the real board (`enumerate_placement_set`) and pick uniformly
/// among its offered moves (excluding the synthetic abstention candidate —
/// abstaining forever is not a move worth spending tape on). Panics (a
/// libFuzzer crash) if the board ever offers a move this generator cannot
/// build, or if a move it built and probed with the SAME checker the board
/// used to offer it is refused (C2's positive half, checked inline so a
/// violation is caught at the earliest possible point, not just at the
/// consuming fuzz target).
pub fn gen_board_sequence(tape: &mut Tape, cap: usize) -> BoardSequence {
    // `deterministic_uuid`, not `Uuid::new_v4()` — this was the actual
    // remaining non-determinism in the generator (found while chasing why
    // the P2-tranche regression corpus entry stopped reproducing: the
    // earlier reproducibility fix covered entity/edge ids minted mid-walk
    // but missed the ONE id minted up front, before any tape byte is
    // read). A single sequence has exactly one subject, so a fixed tag
    // with no per-call counter is correct — the whole point is that this
    // value must be the same every time `gen_board_sequence` runs, tape
    // bytes being equal.
    let subject = SubjectId(deterministic_uuid("subject", 0));
    let lexicon = assembly_lexicon();
    let mut steps: Vec<BoardStep> = Vec::new();
    let mut edge_pool: Vec<EdgeId> = Vec::new();
    let mut citation_pool: Vec<Uuid> = Vec::new();
    let mut placements: Vec<(EntityId, EntityType)> = Vec::new();

    let n = 1 + tape.choice(cap);
    for i in 0..n {
        let refs: Vec<&IntentEvent> = steps.iter().map(|s| &s.event).collect();
        let control = fold_control(&refs);
        let type_registry = fold_type_registry(&refs);
        let board = enumerate_placement_set(subject, &control, &type_registry, &lexicon);

        let real_moves: Vec<&LegalMove> = board
            .moves
            .iter()
            .filter(|m| m.verb_fqn.as_str() != NONE_OF_THE_ABOVE)
            .collect();
        if real_moves.is_empty() {
            break;
        }
        let mv = real_moves[tape.choice(real_moves.len())];

        // `NotCurrentlyPlaced` (fold/control.rs): legal to (re-)place iff
        // NOT (registered && not withdrawn). Offer the subject's own id
        // first when legal (the only path to `freeze`'s
        // `EntityTypeSupportsStrategy`), then every previously-placed
        // entity currently withdrawn (R8's re-placement path).
        let subject_entity = EntityId(subject.0);
        let subject_reusable = !control.registered_entity_ids.contains(&subject_entity)
            || type_registry.is_withdrawn(subject_entity);
        let mut reusable_entity_ids: Vec<EntityId> = Vec::new();
        if subject_reusable {
            reusable_entity_ids.push(subject_entity);
        }
        for &(entity, _) in &placements {
            if type_registry.is_withdrawn(entity) && !reusable_entity_ids.contains(&entity) {
                reusable_entity_ids.push(entity);
            }
        }

        let (event, minted_edge, placed) = build_event(
            tape,
            subject,
            mv,
            &edge_pool,
            &citation_pool,
            &reusable_entity_ids,
            i as u64,
        );

        // C2 positive half, inline: the board OFFERED this move — the same
        // checker it used to decide that must ADMIT the exact event built
        // for it.
        if let Some(entry) = lexicon.entries.get(mv.verb_fqn.as_str()) {
            let admitted = check_preconditions(entry, &control, &type_registry, &event).is_ok();
            assert!(
                admitted,
                "C2 violated: board offered {} (target {:?}) but check_preconditions refused \
                 the event built for it",
                mv.verb_fqn.as_str(),
                mv.target
            );
        }

        if let Some(eid) = minted_edge {
            edge_pool.push(eid);
        }
        if event.verb_fqn.as_str() == "kyc_ubo.assert.edge.evidence" {
            citation_pool.push(event.id.0);
        }
        if let Some(p) = placed {
            placements.push(p);
        }

        steps.push(BoardStep { verb_fqn: mv.verb_fqn.as_str().to_string(), event });
    }

    BoardSequence { subject, lexicon, steps, placements }
}

#[cfg(test)]
mod p0_generator_reaches_every_move {
    use super::*;
    use std::collections::HashSet;

    /// P0 completion criterion, made executable rather than asserted by
    /// inspection: every move `assembly_lexicon()` declares must actually
    /// be reachable from the generator over a real sample of tapes. Any
    /// FQN never hit here — after a generous sample and long sequences —
    /// is finding #1, named by the test failure itself, not inferred.
    #[test]
    fn every_assembly_move_is_generated() {
        let mut seen: HashSet<String> = HashSet::new();
        for seed in 0u64..4000 {
            let bytes = seed.to_le_bytes().repeat(64);
            let mut tape = Tape::new(&bytes);
            let seq = gen_board_sequence(&mut tape, 24);
            for step in &seq.steps {
                seen.insert(step.verb_fqn.clone());
            }
        }

        let expected: HashSet<String> = assembly_lexicon()
            .entries
            .keys()
            .cloned()
            .collect();

        let missing: Vec<&String> = expected.difference(&seen).collect();
        assert!(
            missing.is_empty(),
            "P0 finding #1: assembly-lexicon move(s) never generated from declared \
             vocabulary alone: {missing:?} — either the board never legally offers \
             them from an empty start within 24 steps, or this generator has no \
             payload rule for them (see build_event's match)"
        );
    }
}
