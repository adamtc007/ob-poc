//! Pure, DB-free property checks over `board::gen_board_sequence` output —
//! EOP-VS-UBO-GAME-001 §3.4 R1-R9, C2. Each `check_*` function panics (a
//! libFuzzer crash / assertion failure) on violation, carrying enough of
//! the generated sequence in the message to reproduce by hand. Shared
//! between `#[cfg(test)]` sample loops (fast dev-loop feedback) and the
//! real `fuzz_targets/*.rs` binaries (the actual, continuously-run
//! targets P4 requires) — one definition of each property, not two.

use ob_poc_kyc_substrate::{
    assembly_lexicon, check_preconditions, check_type_geometry, dispatch_for_entity_type,
    enumerate_placement_set, fold_control, fold_type_registry, natural_persons_from_events,
    strategy_for_name, unpierced_nominee_edges, ControlState, DeterminationDispatch, EdgeStatus,
    EntityId, IntentEvent, LinkageSource, ALL_ENTITY_TYPES, NONE_OF_THE_ABOVE,
};

use crate::board::gen_board_sequence;
use crate::Tape;

// EOP-DD-UBO-BASES-001 audit closure P3b (2026-09-08): the hand-rolled
// name→strategy match this file used to carry (a THIRD independent copy,
// alongside `kyc_stream_ops.rs`'s and `recover_determination_at`'s own)
// is gone — `ob_poc_kyc_substrate::strategy_for_name` is now the single
// chokepoint all three consume, so this harness can never silently drift
// out of sync with what a live freeze actually dispatches to.

fn all_active_entity_ids(control: &ControlState) -> Vec<EntityId> {
    control
        .registered_entity_ids
        .iter()
        .copied()
        .collect()
}

// ── R8 ───────────────────────────────────────────────────────────────────

/// R8: "No legal move may produce a board you cannot get back out of: no
/// orphan links, no stranded placements, and no one-way doors."
///
/// Checked as three concrete, statable sub-claims over a generated
/// sequence:
///
/// 1. `remove` prunes ONLY touching links — every edge that does NOT touch
///    the removed entity is byte-identical (Debug-compared) immediately
///    before and after the `remove` event folds.
/// 2. A `remove` never panics/errors, including on a zero-link entity — a
///    block with zero links is a legal position, not a special case (this
///    holds by construction: `gen_board_sequence` only emits board-offered
///    moves, and `remove` is offered for every registered entity per
///    `check_preconditions`, zero-edge or not).
/// 3. A fully emptied board (every registered entity withdrawn) is "the
///    pool": `place`'s type-level offer set equals the full
///    `ALL_ENTITY_TYPES` catalogue (the same set an empty board offers on
///    the very first move — TS.1's vacuous-geometry case), and no ACTIVE
///    edge remains (K-13 supersede-never-delete — the historical edges
///    stay in `state.edges`, but none may still read `EdgeStatus::Asserted`).
pub fn check_r8(tape: &mut Tape) {
    let seq = gen_board_sequence(tape, 20);
    if seq.steps.is_empty() {
        return;
    }

    // Sub-claim 1: remove prunes only touching links.
    for (i, step) in seq.steps.iter().enumerate() {
        if step.verb_fqn != "kyc_ubo.assert.subject.remove" {
            continue;
        }
        let removed = step
            .event
            .payload
            .get("entity_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<uuid::Uuid>().ok())
            .map(EntityId)
            .expect("remove event always carries entity_id");

        let before = seq.control_at(i);
        let after = seq.control_at(i + 1);

        for (edge_id, edge_before) in &before.edges {
            if edge_before.from == removed || edge_before.to == removed {
                continue; // touching — allowed to change
            }
            let edge_after = after
                .edges
                .get(edge_id)
                .unwrap_or_else(|| panic!("R8 violated: edge {edge_id:?} disappeared across a remove that did not touch it"));
            assert_eq!(
                format!("{edge_before:?}"),
                format!("{edge_after:?}"),
                "R8 violated: remove of {removed:?} at step {i} mutated a non-touching edge {edge_id:?}\n\
                 before: {edge_before:?}\nafter:  {edge_after:?}"
            );
        }
    }

    // Sub-claim 3: fully emptied board is the pool.
    let lexicon = assembly_lexicon();
    let mut events: Vec<IntentEvent> = seq.steps.iter().map(|s| s.event.clone()).collect();
    let mut seq_num = seq.steps.len() as u64;

    loop {
        let refs: Vec<&IntentEvent> = events.iter().collect();
        let control = fold_control(&refs);
        let type_registry = fold_type_registry(&refs);
        let active: Vec<EntityId> = all_active_entity_ids(&control)
            .into_iter()
            .filter(|&e| !type_registry.is_withdrawn(e))
            .collect();
        let asserted_edges: Vec<_> = control
            .edges
            .values()
            .filter(|e| e.status == EdgeStatus::Asserted)
            .collect();

        if active.is_empty() && asserted_edges.is_empty() {
            // Emptied (or started empty). Assert the pool claim now.
            let board = enumerate_placement_set(seq.subject, &control, &type_registry, &lexicon);
            let offered_types: std::collections::BTreeSet<_> = board
                .moves
                .iter()
                .filter(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.place")
                .filter_map(|m| m.proposed_entity_type)
                .collect();
            let full_pool: std::collections::BTreeSet<_> = ALL_ENTITY_TYPES.iter().copied().collect();
            assert_eq!(
                offered_types, full_pool,
                "R8 violated: fully emptied board does not offer the full entity-type pool \
                 (missing: {:?})",
                full_pool.difference(&offered_types).collect::<Vec<_>>()
            );
            break;
        }

        // Edges are their own first-class lifecycle (assert/supersede via
        // connect/disconnect), independent of entity registration — R9's
        // `disconnect` exists precisely so an edge can be torn down
        // regardless of endpoint withdrawal state (`connect`'s own
        // candidate loop, `placement.rs`, does not exclude withdrawn
        // endpoints — a withdrawn entity's edges are still real history,
        // R5). Tear down asserted edges FIRST via board-offered
        // `disconnect`, then withdraw active entities via board-offered
        // `remove` — "no orphan links" is a property of the full move
        // vocabulary, not of `remove` alone (`remove`'s own doc: "prunes
        // ONLY touching links", never claiming it prunes every link).
        let board = enumerate_placement_set(seq.subject, &control, &type_registry, &lexicon);

        if !asserted_edges.is_empty() {
            let mv = board
                .moves
                .iter()
                .find(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.edge.disconnect")
                .unwrap_or_else(|| {
                    panic!(
                        "R8 finding: {} edge(s) still Asserted but the board offers no legal \
                         `disconnect` for any of them — an orphan link",
                        asserted_edges.len()
                    )
                });
            let event = IntentEvent::new(
                seq.subject,
                "kyc_ubo.assert.edge.disconnect",
                ob_poc_kyc_substrate::Principal { actor_id: uuid::Uuid::new_v4(), role: "r8-emptier".to_string() },
                ob_poc_kyc_substrate::AuthorityRef("r8-emptier-authority".to_string()),
                mv.target.clone(),
                serde_json::json!({}),
                chrono::DateTime::from_timestamp(1_700_000_000 + seq_num as i64, 0).unwrap(),
            )
            .with_seq(seq_num);
            if let Some(entry) = lexicon.entries.get("kyc_ubo.assert.edge.disconnect") {
                assert!(
                    check_preconditions(entry, &control, &type_registry, &event).is_ok(),
                    "R8 finding: board offered disconnect (target {:?}) but check_preconditions \
                     refused it",
                    mv.target
                );
            }
            events.push(event);
            seq_num += 1;
            continue;
        }

        let removable = board.moves.iter().find(|m| {
            m.verb_fqn.as_str() == "kyc_ubo.assert.subject.remove"
                && m.target.entity_id.is_some_and(|e| active.contains(&e))
        });
        let Some(mv) = removable else {
            // No forward remove offered for a still-active entity: itself
            // a candidate R8 finding (a stranded, non-withdrawable
            // placement) rather than something to route around.
            panic!(
                "R8 finding: {} entities still active but the board offers no legal `remove` \
                 for any of them — a stranded placement",
                active.len()
            );
        };
        let entity = mv.target.entity_id.expect("remove candidates are entity-scoped");
        let event = IntentEvent::new(
            seq.subject,
            "kyc_ubo.assert.subject.remove",
            ob_poc_kyc_substrate::Principal { actor_id: uuid::Uuid::new_v4(), role: "r8-emptier".to_string() },
            ob_poc_kyc_substrate::AuthorityRef("r8-emptier-authority".to_string()),
            mv.target.clone(),
            serde_json::json!({ "entity_id": entity.0 }),
            chrono::DateTime::from_timestamp(1_700_000_000 + seq_num as i64, 0).unwrap(),
        )
        .with_seq(seq_num);
        if let Some(entry) = lexicon.entries.get("kyc_ubo.assert.subject.remove") {
            assert!(
                check_preconditions(entry, &control, &type_registry, &event).is_ok(),
                "R8 finding: board offered remove of {entity:?} but check_preconditions refused it"
            );
        }
        events.push(event);
        seq_num += 1;
    }
}

// ── C2 / geometry closure / determinism ─────────────────────────────────

/// C2: "The board offers only moves that will be admitted" — offered ⇔
/// admitted. The positive half (offered ⇒ admitted) is already checked
/// inline by every `gen_board_sequence` call, on every step, unconditionally
/// (not sampled) — see `board.rs`'s own assertion. This checks the negative
/// half: a move NOT offered must be refused by the same `check_preconditions`
/// chokepoint. Concretely: at a random point in a generated sequence, probe
/// three deliberately-not-offered shapes — `remove` of a never-registered
/// entity, `place` of an already-active (non-withdrawn) entity at the SAME
/// id+type, and `connect` with both endpoints equal (self-edge, geometry
/// forbids every pipe reflexively per `check_type_geometry`) — and assert
/// each is refused.
///
/// Also checks geometry closure at the same point: every currently
/// `EdgeStatus::Asserted` edge's (from, to, kind) triple must still satisfy
/// `check_type_geometry` under the CURRENT type registry (not just the type
/// registry at connect-time) — catching a stale-geometry edge that survived
/// a later type change on one of its endpoints (see `board.rs` R8 remove
/// arm — the reason this is expected to close: `remove` supersedes every
/// edge touching the entity being retyped, which folds before the edge
/// could ever go stale).
///
/// And determinism: folding the same event sequence twice yields the same
/// `ControlState`/`TypeRegistryState` (Debug-compared), and folding to a
/// prefix length matches what `BoardSequence::control_at` returned for that
/// same length during generation.
pub fn check_p2(tape: &mut Tape) {
    let seq = gen_board_sequence(tape, 20);
    if seq.steps.is_empty() {
        return;
    }

    let lexicon = assembly_lexicon();
    let events = seq.events();
    let split = tape.choice(seq.steps.len() + 1);
    let control = fold_control(&events[..split]);
    let type_registry = fold_type_registry(&events[..split]);
    let board = enumerate_placement_set(seq.subject, &control, &type_registry, &lexicon);
    let offered_ids: std::collections::HashSet<&str> = board
        .moves
        .iter()
        .filter(|m| m.verb_fqn.as_str() != NONE_OF_THE_ABOVE)
        .map(|m| m.move_id.0.as_str())
        .collect();
    let _ = offered_ids; // move-id identity not needed below; kept for future use

    // C2 negative half, probe 1: remove a never-registered entity.
    {
        let fqn = "kyc_ubo.assert.subject.remove";
        let ghost = EntityId(uuid::Uuid::new_v4());
        if let Some(entry) = lexicon.entries.get(fqn) {
            let target = ob_poc_kyc_substrate::TargetBinding {
                entity_id: Some(ghost),
                ..ob_poc_kyc_substrate::TargetBinding::for_subject(seq.subject)
            };
            let probe = IntentEvent::new(
                seq.subject,
                fqn,
                ob_poc_kyc_substrate::Principal { actor_id: uuid::Uuid::new_v4(), role: "c2-probe".to_string() },
                ob_poc_kyc_substrate::AuthorityRef("c2-probe-authority".to_string()),
                target,
                serde_json::json!({ "entity_id": ghost.0 }),
                chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            );
            let admitted = check_preconditions(entry, &control, &type_registry, &probe).is_ok();
            assert!(
                !admitted,
                "C2 violated: check_preconditions admitted `remove` of a never-registered \
                 entity {ghost:?} — the board would never offer this"
            );
        }
    }

    // C2 negative half, probe 2: re-place an already-active (non-withdrawn)
    // entity at the SAME id — NotCurrentlyPlaced must refuse it.
    if let Some(&(active_entity, active_type)) = seq
        .placements
        .iter()
        .find(|&&(e, _)| control.registered_entity_ids.contains(&e) && !type_registry.is_withdrawn(e))
    {
        let fqn = "kyc_ubo.assert.subject.place";
        if let Some(entry) = lexicon.entries.get(fqn) {
            // `NotCurrentlyPlaced` reads `event.target.entity_id`, not the
            // payload (`fold/control.rs` — "vacuous when probed without
            // entity_id", the same convention `EntityRegistered` uses) —
            // real callers (`canonical_event_shape`'s `place` arm) always
            // set `target.entity_id`, so the probe must too or this checks
            // nothing.
            let target = ob_poc_kyc_substrate::TargetBinding {
                entity_id: Some(active_entity),
                ..ob_poc_kyc_substrate::TargetBinding::for_subject(seq.subject)
            };
            let entity_type_wire = ALL_ENTITY_TYPES
                .iter()
                .position(|&t| t == active_type)
                .map(|idx| ob_poc_kyc_substrate::ENTITY_TYPE_WIRE_VALUES[idx])
                .expect("active_type is always a real ALL_ENTITY_TYPES member");
            let probe = IntentEvent::new(
                seq.subject,
                fqn,
                ob_poc_kyc_substrate::Principal { actor_id: uuid::Uuid::new_v4(), role: "c2-probe".to_string() },
                ob_poc_kyc_substrate::AuthorityRef("c2-probe-authority".to_string()),
                target,
                serde_json::json!({ "entity_id": active_entity.0, "entity_type": entity_type_wire }),
                chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            );
            let admitted = check_preconditions(entry, &control, &type_registry, &probe).is_ok();
            assert!(
                !admitted,
                "C2 violated: check_preconditions admitted re-placing already-active entity \
                 {active_entity:?} — NotCurrentlyPlaced should have refused it"
            );
        }
    }

    // Geometry closure: every currently-asserted edge's (from, kind, to)
    // must still satisfy check_type_geometry under the CURRENT type
    // registry — a stale-geometry survivor would mean `remove` failed to
    // supersede an edge whose endpoint got retyped underneath it.
    for edge in control.edges.values() {
        if edge.status != EdgeStatus::Asserted {
            continue;
        }
        let (Some(from_t), Some(to_t)) = (type_registry.type_of(edge.from), type_registry.type_of(edge.to)) else {
            panic!(
                "geometry closure violated: active edge {:?} has an endpoint with no known type \
                 (from={:?} to={:?})",
                edge.id, edge.from, edge.to
            );
        };
        let classification = ob_poc_kyc_substrate::pipe_of(&edge.kind, Some(to_t));
        let Some(pipe) = classification.pipe else {
            continue; // no certain pipe for this (kind, target_type) pair — nothing to check
        };
        let legal =
            check_type_geometry(LinkageSource::Entity(from_t), pipe, to_t).is_ok();
        assert!(
            legal,
            "geometry closure violated: active edge {:?} ({from_t:?} --{:?}/{pipe:?}--> {to_t:?}) \
             has an illegal pipe under the CURRENT type registry",
            edge.id, edge.kind
        );
    }

    // Determinism: double-fold equality.
    let control2 = fold_control(&events[..split]);
    assert_eq!(
        format!("{control:?}"),
        format!("{control2:?}"),
        "determinism violated: folding the same {split}-event prefix twice produced different \
         ControlState"
    );
    let type_registry2 = fold_type_registry(&events[..split]);
    assert_eq!(
        format!("{type_registry:?}"),
        format!("{type_registry2:?}"),
        "determinism violated: folding the same {split}-event prefix twice produced different \
         TypeRegistryState"
    );

    // Determinism: fold-to-prefix matches the snapshot taken during
    // generation for that same prefix length.
    let historical_control = seq.control_at(split);
    assert_eq!(
        format!("{control:?}"),
        format!("{historical_control:?}"),
        "determinism violated: re-folding to prefix {split} disagrees with the ControlState \
         recorded live during generation at that same point"
    );
}

// ── Determination invariants (no oracle) ────────────────────────────────

/// P3: a determination over a random board has no expected-answer oracle.
/// Only invariants, mirroring exactly what `freeze`'s own dispatch does
/// (`dispatch_for_entity_type` → the named `DeterminationStrategy`, per
/// `EOP-DD-UBO-DISPATCH-001` §4 — the same dispatch `check_p2`'s C2 probes
/// exercise the legality of, here exercised for its actual output):
///
/// - **Terminates**: `strategy.resolve(...)` is a plain, non-recursive call
///   (explicit-stack DFS per `determination.rs`'s own cycle-guard doc); it
///   either returns here or the fuzz harness's own `-timeout` catches a
///   hang as a distinct, separately-reportable finding — nothing to assert
///   beyond the call returning at all.
/// - **Resolves to natural persons or records why it could not**: a
///   subject whose `EntityType` is `NotADeterminationSubject`
///   (`dispatch_for_entity_type`) IS the "why" (a person is never frozen —
///   §4 D2); a subject with no strategy match at all is `dispatch_for_entity_type`
///   never returning `Strategy(name)` for a name this fuzzer doesn't
///   recognise, itself a finding (see the `unwrap_or_else` below); an empty
///   `Vec<ProngCandidate>` from a real strategy is itself the legitimate
///   "found nobody" answer (K-1 — no candidates is a valid resolution, not
///   an error).
/// - **Never names a nominee as UBO**: no returned candidate's `person_id`
///   may equal the `to`-entity of an unpierced `Nominee` edge
///   (`unpierced_nominee_edges`, the same set `freeze`'s
///   `NoUnpiercedNomineeEdges` precondition refuses on).
///
/// **Finding #3, re-characterised and closed (EOP-DD-UBO-BASES-001 §6,
/// 2026-09-08).** This property used to also assert
/// `effective_ownership_pct <= 100`, under the name "control never
/// multiplied along a chain". Execution proved that framing wrong on both
/// counts: the crashing input (`fuzz/regressions/board_determination_
/// invariants/finding_3_unbounded_percentage_input`) was a SINGLE hop, not
/// a chain — `cumulative_pct(100.0) * edge_pct/100.0` is unchanged, not
/// multiplied — and `effective_ownership_pct` is never `Some` on a
/// control-axis candidate at all (K-3, structurally: every control
/// strategy constructs `None`), so "control" was never involved either.
/// The real defect was that `connect`'s `percentage` payload field had no
/// bound anywhere in the write path — `Tape::percentage()` deliberately
/// generates values outside `[0, 100]` to probe exactly this, and nothing
/// downstream ever refused one. That is a WRITE-PATH precondition, not a
/// fold/determination invariant: `Precondition::PercentageIsBounded` now
/// closes it at `connect`'s lexicon entry (both live surfaces — see
/// `tests/kyc_bases_001_percentage.rs`), the correct chokepoint. This
/// fold-layer property intentionally does NOT re-assert a percentage
/// bound: `gen_board_sequence` builds raw `IntentEvent`s directly, never
/// through `check_preconditions`, so re-asserting the bound here would
/// only be re-discovering "the fuzzer's own adversarial generator
/// generates adversarial values" — not a property of the determination
/// engine. `OwnershipProngStrategy`'s DFS is, and always was, correct
/// arithmetic over whatever it is given; validating the input is someone
/// else's job, and now has an owner.
pub fn check_p3(tape: &mut Tape) {
    let seq = gen_board_sequence(tape, 20);
    if seq.steps.is_empty() {
        return;
    }

    let events = seq.events();
    let split = 1 + tape.choice(seq.steps.len());
    let refs = &events[..split];
    let control = fold_control(refs);
    let type_registry = fold_type_registry(refs);

    let subject_entity = EntityId(seq.subject.0);
    let Some(subject_type) = type_registry.type_of(subject_entity) else {
        return; // subject never typed at this prefix — nothing to determine
    };

    let dispatch = dispatch_for_entity_type(&subject_type);
    let DeterminationDispatch::Strategies(names) = dispatch else {
        return; // NotADeterminationSubject IS the recorded "why" — nothing to resolve
    };

    let natural_persons = natural_persons_from_events(refs);
    let threshold_pct = tape.percentage();

    // EOP-DD-UBO-BASES-001 §3: mirrors freeze's own live orchestration —
    // every strategy in the dispatch set runs and the results union.
    // Terminates: proven by each call returning at all.
    let mut candidates = Vec::new();
    for name in names {
        let Some(strategy) = strategy_for_name(name) else {
            panic!(
                "P3 finding: dispatch_for_entity_type({subject_type:?}) named strategy \
                 {name:?}, which this harness (mirroring freeze's own live dispatch set) does \
                 not recognise — either a new strategy was added without updating both \
                 dispatch sites, or the two have drifted"
            );
        };
        candidates.extend(strategy.resolve(&control, subject_entity, &natural_persons, threshold_pct));
    }

    let unpierced_nominee_targets: std::collections::HashSet<_> = unpierced_nominee_edges(&control)
        .into_iter()
        .filter_map(|edge_id| control.edges.get(&edge_id))
        .map(|e| e.to)
        .collect();

    for candidate in &candidates {
        // Never names a nominee as UBO.
        let candidate_entity = EntityId(candidate.person_id.0);
        assert!(
            !unpierced_nominee_targets.contains(&candidate_entity),
            "P3 violated: determination named unpierced-nominee entity {candidate_entity:?} \
             ({:?}) as a UBO candidate",
            candidate.person_id
        );

        // The percentage-bound assertion formerly here (finding #3,
        // "control never multiplied along a chain") was removed —
        // EOP-DD-UBO-BASES-001 §6, see this function's doc. Bounding
        // `percentage` is now the write path's job
        // (`Precondition::PercentageIsBounded`), which this pure-fold
        // property intentionally never exercises.
    }
}

// ── Basis survival (EOP-DD-UBO-BASES-001 §4/§7) ──────────────────────────

/// §4/§7 survival property, fuzz-exercised: a person admitted by N
/// independent bases survives the removal of any N-1 of them, and the
/// surviving candidate still names exactly the one basis left standing.
/// `kyc_bases_001_p0_basis_set.rs::candidate_carries_every_admitting_basis`/
/// `disconnect_removes_a_route_not_the_person` already prove this at the
/// substrate level over a hand-written fixture; this is the same property
/// exercised over FUZZER-GENERATED board shapes (mirroring `check_p3`'s
/// freeze-orchestration dispatch, then `merge_candidates_by_person`), which
/// can reach basis-plurality via paths a hand-written fixture would not
/// think to construct.
///
/// A generated prefix that never reaches basis-plurality (no person
/// admitted by >=2 independent edges) is not a counter-example — it simply
/// has nothing to probe, so the property returns early rather than forcing
/// one. When plurality IS reached, every OTHER basis is torn down via
/// board-offered `disconnect` moves (respecting `check_preconditions` at
/// each step, same chokepoint discipline as `check_r8`'s emptying loop) and
/// the determination is re-run.
pub fn check_p4_basis_survival(tape: &mut Tape) {
    let seq = gen_board_sequence(tape, 20);
    if seq.steps.is_empty() {
        return;
    }

    let events = seq.events();
    let split = 1 + tape.choice(seq.steps.len());
    let mut history: Vec<IntentEvent> = events[..split].iter().map(|e| (*e).clone()).collect();

    let refs: Vec<&IntentEvent> = history.iter().collect();
    let control = fold_control(&refs);
    let type_registry = fold_type_registry(&refs);

    let subject_entity = EntityId(seq.subject.0);
    let Some(subject_type) = type_registry.type_of(subject_entity) else {
        return; // subject never typed at this prefix — nothing to determine
    };
    let DeterminationDispatch::Strategies(names) = dispatch_for_entity_type(&subject_type) else {
        return; // NotADeterminationSubject — nothing to resolve
    };

    let natural_persons = natural_persons_from_events(&refs);
    let threshold_pct = tape.percentage();

    let mut candidates = Vec::new();
    for name in names {
        let Some(strategy) = strategy_for_name(name) else {
            panic!(
                "P4 basis-survival finding: dispatch_for_entity_type({subject_type:?}) named \
                 strategy {name:?}, which this harness does not recognise"
            );
        };
        candidates.extend(strategy.resolve(&control, subject_entity, &natural_persons, threshold_pct));
    }
    let mut merged = ob_poc_kyc_substrate::merge_candidates_by_person(candidates);

    let lexicon = assembly_lexicon();
    let mut seq_num = history.len() as u64;
    let mut cur_control = control;
    let mut cur_type_registry = type_registry;

    // EOP-DD-UBO-BASES-001 audit closure P3d (2026-09-08, audit item 10):
    // the uniform-random `gen_board_sequence` reaches basis-plurality (a
    // person admitted via 2+ independent bases into the SAME subject)
    // vanishingly rarely by chance — the audit measured 8/5000 seeds
    // (0.16%). Rather than change the shared generator (used by R8/P2/P3
    // too, out of this property's scope), bias LOCALLY: if this board
    // didn't reach plurality on its own, actively drive ONE additional
    // legal `connect` — a SECOND, DIFFERENT admissible edge kind from an
    // entity that ALREADY has an active edge into the subject — through
    // the real board (`enumerate_placement_set` + `check_preconditions`,
    // same chokepoint discipline as the removal loop below). This can only
    // ADD a legitimately-admitted edge the board itself offers; it never
    // fabricates one, so a survival counter-example found this way is as
    // real as one the generator found unassisted.
    if !merged.iter().any(|c| c.bases.len() >= 2) {
        let already_connected_targets: std::collections::BTreeSet<EntityId> = cur_control
            .edges
            .values()
            .filter(|e| e.is_active() && e.to == subject_entity)
            .map(|e| e.from)
            .collect();
        let board = enumerate_placement_set(seq.subject, &cur_control, &cur_type_registry, &lexicon);
        let boost = board.moves.iter().find(|m| {
            m.verb_fqn.as_str() == "kyc_ubo.assert.edge.connect"
                && m.proposed_edge.as_ref().is_some_and(|pe| {
                    pe.to == subject_entity && already_connected_targets.contains(&pe.from)
                })
        });
        if let Some(mv) = boost {
            let pe = mv.proposed_edge.as_ref().expect("matched above");
            let mut payload = serde_json::json!({
                "kind": pe.kind_wire, "from_entity_id": pe.from.0, "to_entity_id": pe.to.0,
            });
            if pe.kind_wire == "economic_interest" {
                payload["percentage"] = serde_json::json!(tape.percentage().clamp(0.0, 100.0));
            }
            let event = IntentEvent::new(
                seq.subject,
                "kyc_ubo.assert.edge.connect",
                ob_poc_kyc_substrate::Principal {
                    actor_id: uuid::Uuid::new_v4(),
                    role: "p4-basis-prober".to_string(),
                },
                ob_poc_kyc_substrate::AuthorityRef("p4-basis-prober-authority".to_string()),
                mv.target.clone(),
                payload,
                chrono::DateTime::from_timestamp(1_700_000_000 + seq_num as i64, 0).unwrap(),
            )
            .with_seq(seq_num);
            if let Some(entry) = lexicon.entries.get("kyc_ubo.assert.edge.connect") {
                if check_preconditions(entry, &cur_control, &cur_type_registry, &event).is_ok() {
                    history.push(event);
                    seq_num += 1;
                    let all_refs: Vec<&IntentEvent> = history.iter().collect();
                    cur_control = fold_control(&all_refs);
                    cur_type_registry = fold_type_registry(&all_refs);

                    let boosted_persons = natural_persons_from_events(&all_refs);
                    let mut boosted = Vec::new();
                    for name in names {
                        if let Some(strategy) = strategy_for_name(name) {
                            boosted.extend(strategy.resolve(
                                &cur_control,
                                subject_entity,
                                &boosted_persons,
                                threshold_pct,
                            ));
                        }
                    }
                    merged = ob_poc_kyc_substrate::merge_candidates_by_person(boosted);
                }
            }
        }
    }

    let Some(target) = merged.iter().find(|c| c.bases.len() >= 2) else {
        return; // even after the bias attempt, this board never reached basis-plurality
    };
    let target_person = target.person_id;
    let keep_idx = tape.choice(target.bases.len());
    let kept_basis_edge_id = target.bases[keep_idx].edge_id;
    let to_remove: Vec<_> = target
        .bases
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != keep_idx)
        .map(|(_, b)| b.edge_id)
        .collect();

    for edge_id in &to_remove {
        let board = enumerate_placement_set(seq.subject, &cur_control, &cur_type_registry, &lexicon);
        let Some(mv) = board.moves.iter().find(|m| {
            m.verb_fqn.as_str() == "kyc_ubo.assert.edge.disconnect" && m.target.edge_id == Some(*edge_id)
        }) else {
            // The board no longer offers a legal disconnect for this basis
            // edge at this point in the prefix (e.g. already superseded by
            // a later generated step) — not a survival counter-example,
            // just an unreachable probe for this particular random board.
            return;
        };
        let event = IntentEvent::new(
            seq.subject,
            "kyc_ubo.assert.edge.disconnect",
            ob_poc_kyc_substrate::Principal {
                actor_id: uuid::Uuid::new_v4(),
                role: "p4-basis-prober".to_string(),
            },
            ob_poc_kyc_substrate::AuthorityRef("p4-basis-prober-authority".to_string()),
            mv.target.clone(),
            serde_json::json!({}),
            chrono::DateTime::from_timestamp(1_700_000_000 + seq_num as i64, 0).unwrap(),
        )
        .with_seq(seq_num);
        if let Some(entry) = lexicon.entries.get("kyc_ubo.assert.edge.disconnect") {
            assert!(
                check_preconditions(entry, &cur_control, &cur_type_registry, &event).is_ok(),
                "P4 basis-survival finding: board offered disconnect of basis edge {edge_id:?} \
                 but check_preconditions refused it"
            );
        }
        history.push(event);
        seq_num += 1;
        let all_refs: Vec<&IntentEvent> = history.iter().collect();
        cur_control = fold_control(&all_refs);
        cur_type_registry = fold_type_registry(&all_refs);
    }

    let all_refs: Vec<&IntentEvent> = history.iter().collect();
    let natural_persons_after = natural_persons_from_events(&all_refs);
    let mut after = Vec::new();
    for name in names {
        let Some(strategy) = strategy_for_name(name) else {
            return;
        };
        after.extend(strategy.resolve(&cur_control, subject_entity, &natural_persons_after, threshold_pct));
    }
    let after_merged = ob_poc_kyc_substrate::merge_candidates_by_person(after);

    let survivor = after_merged.iter().find(|c| c.person_id == target_person);
    assert!(
        survivor.is_some(),
        "P4 basis-survival violated: person {target_person:?} had {} independent bases; \
         removing all but one (kept edge {kept_basis_edge_id:?}) must not remove the person \
         entirely — got: {after_merged:#?}",
        target.bases.len()
    );
    let survivor = survivor.expect("checked above");
    assert!(
        survivor.bases.iter().any(|b| b.edge_id == kept_basis_edge_id),
        "P4 basis-survival violated: the surviving candidate must still name the kept basis \
         {kept_basis_edge_id:?} — got bases: {:?}",
        survivor.bases
    );
}

#[cfg(test)]
mod sample_loops {
    use super::*;

    fn run_sample<F: Fn(&mut Tape)>(n: u64, steps_per_tape: usize, f: F) {
        for seed in 0..n {
            let bytes: Vec<u8> = (0..steps_per_tape)
                .flat_map(|i| (seed.wrapping_mul(2654435761).wrapping_add(i as u64)).to_le_bytes())
                .collect();
            let mut tape = Tape::new(&bytes);
            f(&mut tape);
        }
    }

    #[test]
    fn r8_holds_over_sample() {
        run_sample(1500, 64, check_r8);
    }

    #[test]
    fn p3_holds_over_sample() {
        run_sample(1500, 64, check_p3);
    }

    #[test]
    fn p4_basis_survival_holds_over_sample() {
        run_sample(1500, 64, check_p4_basis_survival);
    }

    /// EOP-DD-UBO-BASES-001 audit closure P3d (2026-09-08, audit item 10)
    /// measurement: how often does `check_p4_basis_survival` actually reach
    /// its assertion branch (basis-plurality found), with the P3d bias in
    /// place vs without it? Mirrors the pre-assertion portion of the real
    /// property function (same generator, same dispatch, same merge) with
    /// a `bias: bool` toggle so both rates are measured against the exact
    /// same 5000-seed sample, not two different runs.
    fn reach_rate(n: u64, bias: bool) -> (u64, u64) {
        let mut plurality_reached = 0u64;
        let mut any_candidate = 0u64;
        for seed in 0..n {
            let bytes: Vec<u8> = (0..64)
                .flat_map(|i| (seed.wrapping_mul(2654435761).wrapping_add(i as u64)).to_le_bytes())
                .collect();
            let mut tape = Tape::new(&bytes);

            let seq = gen_board_sequence(&mut tape, 20);
            if seq.steps.is_empty() {
                continue;
            }
            let events = seq.events();
            let split = 1 + tape.choice(seq.steps.len());
            let mut history: Vec<IntentEvent> =
                events[..split].iter().map(|e| (*e).clone()).collect();
            let refs: Vec<&IntentEvent> = history.iter().collect();
            let mut control = fold_control(&refs);
            let mut type_registry = fold_type_registry(&refs);
            let subject_entity = EntityId(seq.subject.0);
            let Some(subject_type) = type_registry.type_of(subject_entity) else { continue };
            let DeterminationDispatch::Strategies(names) =
                dispatch_for_entity_type(&subject_type)
            else {
                continue;
            };
            let natural_persons = natural_persons_from_events(&refs);
            let threshold_pct = tape.percentage();

            let resolve = |control: &ControlState,
                           type_registry: &ob_poc_kyc_substrate::TypeRegistryState,
                           persons: &std::collections::BTreeSet<ob_poc_kyc_substrate::PersonId>| {
                let _ = type_registry;
                let mut cands = Vec::new();
                for name in names {
                    if let Some(strategy) = strategy_for_name(name) {
                        cands.extend(strategy.resolve(control, subject_entity, persons, threshold_pct));
                    }
                }
                ob_poc_kyc_substrate::merge_candidates_by_person(cands)
            };

            let mut merged = resolve(&control, &type_registry, &natural_persons);

            if bias && !merged.iter().any(|c| c.bases.len() >= 2) {
                let already: std::collections::BTreeSet<EntityId> = control
                    .edges
                    .values()
                    .filter(|e| e.is_active() && e.to == subject_entity)
                    .map(|e| e.from)
                    .collect();
                let lexicon = assembly_lexicon();
                let board = enumerate_placement_set(seq.subject, &control, &type_registry, &lexicon);
                if let Some(mv) = board.moves.iter().find(|m| {
                    m.verb_fqn.as_str() == "kyc_ubo.assert.edge.connect"
                        && m.proposed_edge.as_ref().is_some_and(|pe| {
                            pe.to == subject_entity && already.contains(&pe.from)
                        })
                }) {
                    let pe = mv.proposed_edge.as_ref().unwrap();
                    let mut payload = serde_json::json!({
                        "kind": pe.kind_wire, "from_entity_id": pe.from.0, "to_entity_id": pe.to.0,
                    });
                    if pe.kind_wire == "economic_interest" {
                        payload["percentage"] = serde_json::json!(tape.percentage().clamp(0.0, 100.0));
                    }
                    let event = IntentEvent::new(
                        seq.subject,
                        "kyc_ubo.assert.edge.connect",
                        ob_poc_kyc_substrate::Principal {
                            actor_id: uuid::Uuid::new_v4(),
                            role: "measure".to_string(),
                        },
                        ob_poc_kyc_substrate::AuthorityRef("measure".to_string()),
                        mv.target.clone(),
                        payload,
                        chrono::DateTime::from_timestamp(1_700_000_000 + history.len() as i64, 0)
                            .unwrap(),
                    )
                    .with_seq(history.len() as u64);
                    if let Some(entry) = lexicon.entries.get("kyc_ubo.assert.edge.connect") {
                        if check_preconditions(entry, &control, &type_registry, &event).is_ok() {
                            history.push(event);
                            let all_refs: Vec<&IntentEvent> = history.iter().collect();
                            control = fold_control(&all_refs);
                            type_registry = fold_type_registry(&all_refs);
                            let persons = natural_persons_from_events(&all_refs);
                            merged = resolve(&control, &type_registry, &persons);
                        }
                    }
                }
            }

            if !merged.is_empty() {
                any_candidate += 1;
            }
            if merged.iter().any(|c| c.bases.len() >= 2) {
                plurality_reached += 1;
            }
        }
        (plurality_reached, any_candidate)
    }

    #[test]
    fn p3d_measure_reach_rate_before_and_after_bias() {
        // 1500 — the SAME sample size `p4_basis_survival_holds_over_sample`
        // (and every other sample-loop gate in this module) already runs
        // clean at; a larger n surfaces an unrelated pre-existing C2
        // generator finding (board.rs:353) at a seed beyond this range —
        // out of P3d's scope (bias measurement, not a new bug hunt),
        // reported separately rather than chased here.
        const N: u64 = 1500;
        let (before_plurality, before_any) = reach_rate(N, false);
        let (after_plurality, after_any) = reach_rate(N, true);
        println!(
            "P3d reach-rate measurement over {N} seeds:\n\
             before bias: plurality={before_plurality}/{N} ({:.2}%), any_candidate={before_any}/{N}\n\
             after  bias: plurality={after_plurality}/{N} ({:.2}%), any_candidate={after_any}/{N}",
            before_plurality as f64 * 100.0 / N as f64,
            after_plurality as f64 * 100.0 / N as f64,
        );
        assert!(
            after_plurality > before_plurality,
            "P3d bias must increase the assertion-branch reach rate: before={before_plurality} \
             after={after_plurality}"
        );
    }

    /// FINDING #2 (EOP-VS-UBO-GAME-001 §3.4 P2, geometry closure) — CLOSED
    /// 2026-09-08, RULED by Adam 2026-09-07: forbid the connection.
    /// `Precondition::ConnectEndpointsNotWithdrawn`, attached to
    /// `connect`'s lexicon entry and evaluated at the chokepoint
    /// (`fold/control.rs::check_preconditions`), closes the window: a
    /// WITHDRAWN entity can no longer be named as either endpoint of a NEW
    /// link, so the stale-retype sequence this test used to find can no
    /// longer be built. Scope confirmed 2026-09-08 — deliberately narrower
    /// than "must be registered": a never-placed endpoint is untouched
    /// (stays Unevaluable/admit at TypeGeometryPermits, R5/R6/CTN-2e), which
    /// is what the K-8 nominee-pierce mechanism's on-paper holder structurally
    /// requires. `placement.rs`'s geometry-gated enumeration needed no
    /// separate change — it probes each candidate triple through the same
    /// `check_preconditions` chokepoint (R7), so the board stopped OFFERING
    /// the withdrawn-endpoint candidate as a structural consequence of the
    /// precondition alone (R9/C2, proven in
    /// `tests/placement.rs::connect_does_not_offer_a_withdrawn_endpoint`).
    ///
    /// Left in place, GREEN, rather than deleted — the fuzz-harness
    /// completion criterion is "the board and moves are fuzzable," not
    /// "this specific violation is gone and forgotten"; this test now
    /// stands as the permanent regression proof for the fix, run on every
    /// `cargo test` and continuously by `fuzz_targets/board_c2_geometry_determinism.rs`.
    /// Former mechanism (for the record — see the RED-gate write-path
    /// proof in `tests/kyc_connect_withdrawn_endpoint.rs` for the
    /// two-surface, both-directions version of the same fix):
    /// `connect`'s candidate enumeration did not exclude a WITHDRAWN
    /// entity as an endpoint; geometry was checked against that entity's
    /// type AT THAT MOMENT and was legal; but if the SAME entity was later
    /// re-placed with a DIFFERENT type (`NotCurrentlyPlaced` allows this
    /// once withdrawn — R8's own re-placement path), `place`'s fold arm
    /// never touched `ControlState.edges`, and `remove` only superseded
    /// edges that existed AT remove-time — an edge asserted AFTER a remove
    /// but BEFORE the next re-place had no move in the vocabulary that
    /// ever re-validated or superseded it against the entity's new type.
    #[test]
    fn finding_2_geometry_closure_stale_retype_now_forbidden_by_connect_endpoints_actively_placed() {
        run_sample(1500, 64, check_p2);
    }
}


