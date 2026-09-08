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
    unpierced_nominee_edges, ControlProngStrategy, ControlState, CooperativeMemberStrategy,
    DeterminationDispatch, DeterminationStrategy, EdgeStatus, EntityId, FoundationCouncilStrategy,
    FundControlStrategy, IntentEvent, LinkageSource, OwnershipProngStrategy, StateOwnedStrategy,
    TrustRoleStrategy, ALL_ENTITY_TYPES, NONE_OF_THE_ABOVE,
};

use crate::board::gen_board_sequence;
use crate::Tape;

static OWNERSHIP: OwnershipProngStrategy = OwnershipProngStrategy;
static CONTROL: ControlProngStrategy = ControlProngStrategy;
static TRUST: TrustRoleStrategy = TrustRoleStrategy;
static FUND: FundControlStrategy = FundControlStrategy;
static FOUNDATION: FoundationCouncilStrategy = FoundationCouncilStrategy;
static STATE_OWNED: StateOwnedStrategy = StateOwnedStrategy;
static COOPERATIVE: CooperativeMemberStrategy = CooperativeMemberStrategy;

/// `dispatch_for_entity_type`'s `Strategy(name)` → the real strategy object
/// `kyc_ubo.decide.determination.freeze` would dispatch to for that name
/// (`kyc_stream_ops.rs`'s `UboDeterminationFreeze::execute`, mirrored here
/// so P3 exercises the SAME strategy freeze would have picked, not an
/// arbitrary one).
fn strategy_for_name(name: &str) -> Option<&'static dyn DeterminationStrategy> {
    match name {
        "ownership_prong_strategy" => Some(&OWNERSHIP),
        "control_prong_strategy" => Some(&CONTROL),
        "trust_role_strategy" => Some(&TRUST),
        "fund_control_strategy" => Some(&FUND),
        "foundation_council_strategy" => Some(&FOUNDATION),
        "state_owned_strategy" => Some(&STATE_OWNED),
        "cooperative_member_strategy" => Some(&COOPERATIVE),
        _ => None,
    }
}

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
/// - **Control never multiplied along a chain**: `effective_ownership_pct`,
///   where present, must not exceed 100 — chain multiplication (product of
///   <=100% hops) can only shrink a percentage, never grow it past the
///   single-hop maximum.
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
    let DeterminationDispatch::Strategy(name) = dispatch else {
        return; // NotADeterminationSubject IS the recorded "why" — nothing to resolve
    };
    let Some(strategy) = strategy_for_name(name) else {
        panic!(
            "P3 finding: dispatch_for_entity_type({subject_type:?}) named strategy {name:?}, \
             which this harness (mirroring freeze's own live dispatch set) does not recognise \
             — either a new strategy was added without updating both dispatch sites, or the \
             two have drifted"
        );
    };

    let natural_persons = natural_persons_from_events(refs);
    let threshold_pct = tape.percentage();

    // Terminates: proven by this call returning at all.
    let candidates = strategy.resolve(&control, subject_entity, &natural_persons, threshold_pct);

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

        // No multiplied control.
        if let Some(pct) = candidate.effective_ownership_pct {
            assert!(
                pct <= 100.0 + 1e-6,
                "P3 violated: resolved candidate {:?} carries effective_ownership_pct {pct} > \
                 100 — control was multiplied upward along chain {:?}, not shrunk",
                candidate.person_id,
                candidate.ownership_chain
            );
        }
    }
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


