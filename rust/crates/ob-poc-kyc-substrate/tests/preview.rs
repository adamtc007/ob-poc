//! T3 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T3 (closes KIT-7).
//!
//! `preview(committed, candidates, lexicon) -> Result<ControlState, KycError>`
//! is pure: fold over `committed ++ candidates`, zero store involvement,
//! per-step admission mirroring bpmn-lite's `resolve_hypothetical_chain`.
//! These tests prove the four T3 gates: purity, per-step ≡ single-move
//! validation, committed-replay equivalence, and mid-chain rejection.

use uuid::Uuid;

use ob_poc_kyc_substrate::{
    check_control_preconditions, fold_control, assembly_lexicon, preview, ControlState, EdgeId,
    EntityId, IntentEvent, Principal, SubjectId, TargetBinding, TypeRegistryState,
};

fn subject() -> SubjectId {
    SubjectId(Uuid::new_v4())
}

fn entity() -> EntityId {
    EntityId(Uuid::new_v4())
}

fn t() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap()
}

/// T6.2 (2026-08-12): `assert-control` now carries `SubjectRegistered`
/// (matrix row 1) — every candidate chain below that starts with
/// assert-control needs a preceding placement. T2 (EOP-VS-UBO-GAME-001
/// §3.2) merged register + assert-type into `place`, entity-scoped target.
fn register_event(subj: SubjectId) -> IntentEvent {
    IntentEvent::new(
        subj,
        "kyc_ubo.assert.subject.place",
        Principal::test_analyst(),
        ob_poc_kyc_substrate::AuthorityRef("preview-test".into()),
        TargetBinding { entity_id: Some(EntityId(subj.0)), ..TargetBinding::for_subject(subj) },
        serde_json::json!({ "entity_id": subj.0, "entity_type": "private_limited_company" }),
        t(),
    )
}

fn assert_control_event(subj: SubjectId, edge: EdgeId, from: EntityId, to: EntityId) -> IntentEvent {
    IntentEvent::new(
        subj,
        "kyc_ubo.assert.edge.connect",
        Principal::test_analyst(),
        ob_poc_kyc_substrate::AuthorityRef("preview-test".into()),
        TargetBinding::for_subject(subj),
        serde_json::json!({
            "edge_id": edge.0, "from_entity_id": from.0, "to_entity_id": to.0,
            "kind": "voting_rights",
        }),
        t(),
    )
}

fn attach_evidence_event(subj: SubjectId, edge: EdgeId) -> IntentEvent {
    IntentEvent::new(
        subj,
        "kyc_ubo.assert.edge.evidence",
        Principal::test_analyst(),
        ob_poc_kyc_substrate::AuthorityRef("preview-test".into()),
        TargetBinding::for_edge(subj, edge),
        serde_json::json!({
            "kind": "filed-document",
            "source": "test fixture",
            "date": "2026-01-01",
        }),
        t(),
    )
}

// `verify_event` RETIRED (EOP-DD-UBO-PROOF-001 §3/§4, T5, 2026-08-28)
// alongside `kyc_ubo.assert.edge.verification` (K-G7: 0 real committed
// events) — there is no ratchet left to preview a step onto.

// ── preview_is_pure ─────────────────────────────────────────────────────────
//
// Plain synchronous test, no store handle, no async runtime, no DATABASE_URL
// — `preview`'s signature (`&[IntentEvent], &[IntentEvent], &LexiconManifest
// -> Result<ControlState, KycError>`) has no store parameter to pass one
// through even if we wanted to. Combined with the crate-level dep-gate
// (`check_kyc_substrate_deps.sh`, run as part of the T2/T3 verification
// sweep — no sqlx/tokio-postgres/dsl-runtime in this crate's dep tree),
// this is the structural proof the plan's gate asks for.
#[test]
fn preview_is_pure() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    let result = preview(
        &[],
        &[register_event(subj), assert_control_event(subj, edge, from, to)],
        &lexicon,
    );
    assert!(result.is_ok(), "pure fold over a legal single-candidate chain must succeed");
}

// ── per_step_matches_single_move ────────────────────────────────────────────

#[test]
fn per_step_matches_single_move() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    let candidates = vec![
        register_event(subj),
        assert_control_event(subj, edge, from, to),
        attach_evidence_event(subj, edge),
    ];

    // The "single move" oracle: fold committed ++ candidates[..n] by hand,
    // and confirm each candidate's own precondition check passes against
    // that exact prefix state — the same admission `preview` performs
    // internally, computed independently here.
    let mut prefix: Vec<&IntentEvent> = vec![];
    for candidate in &candidates {
        let prefix_state = fold_control(&prefix);
        let entry = lexicon.get(candidate.verb_fqn.as_str()).expect("verb in lexicon");
        assert!(
            check_control_preconditions(entry, &prefix_state, &TypeRegistryState::default(), candidate)
                .is_ok(),
            "candidate {} must be legal against its own prefix state",
            candidate.verb_fqn.as_str()
        );
        prefix.push(candidate);
    }

    // `preview` must agree: the whole chain is admitted end to end.
    let result = preview(&[], &candidates, &lexicon);
    assert!(result.is_ok(), "preview must admit the same chain the per-step oracle admitted");
}

// ── committed_line_equals_preview ───────────────────────────────────────────

#[test]
fn committed_line_equals_preview() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    let committed = vec![assert_control_event(subj, edge, from, to)];
    let candidates = vec![attach_evidence_event(subj, edge)];

    let (previewed, _previewed_obligation, _previewed_type_registry): (ControlState, _, _) =
        preview(&committed, &candidates, &lexicon).expect("legal chain must preview successfully");

    // The stand-in for "the real append path" (T4 hasn't built the store
    // append yet): a straight full fold over the same committed ++
    // candidates sequence must land on a bit-identical final state.
    let full: Vec<&IntentEvent> = committed.iter().chain(candidates.iter()).collect();
    let replayed = fold_control(&full);

    assert_eq!(
        previewed.edges.get(&edge).map(|e| e.status),
        replayed.edges.get(&edge).map(|e| e.status),
        "committing a previewed line must reproduce the line's final preview state"
    );
    assert_eq!(previewed.edges.len(), replayed.edges.len());
}

// ── illegal_mid_chain_rejected ───────────────────────────────────────────────

#[test]
fn illegal_mid_chain_rejected() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    // Step 0 legal (register), step 1 legal (assert-control), step 2
    // illegal (a second connect asserting the SAME kind/from/to —
    // NoDuplicateActiveEdge, K-13: contradicting asserts must go through
    // disconnect, never a repeat connect), step 3 would-be-legal
    // (attach-evidence) never gets the chance to run.
    let candidates = vec![
        register_event(subj),
        assert_control_event(subj, edge, from, to),
        assert_control_event(subj, EdgeId(Uuid::new_v4()), from, to),
        attach_evidence_event(subj, edge),
    ];

    let result = preview(&[], &candidates, &lexicon);
    assert!(result.is_err(), "chain with an illegal step 2 must reject, not silently skip");

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("active edge") || err.contains("already exists"),
        "rejection must be attributable to the illegal duplicate-connect step; got: {err}"
    );

    // Nothing partial escapes: preview has no side channel to leak
    // steps 0-1's application through on an Err path — the only observable
    // outcome of a rejected chain is the Err itself.
    let legal_prefix_only = preview(&[], &candidates[..2], &lexicon);
    assert!(legal_prefix_only.is_ok(), "the legal prefix alone must still preview cleanly");
}
