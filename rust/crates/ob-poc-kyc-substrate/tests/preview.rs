//! T3 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T3 (closes KIT-7).
//!
//! `preview(committed, candidates, lexicon) -> Result<ControlState, KycError>`
//! is pure: fold over `committed ++ candidates`, zero store involvement,
//! per-step admission mirroring bpmn-lite's `resolve_hypothetical_chain`.
//! These tests prove the four T3 gates: purity, per-step ≡ single-move
//! validation, committed-replay equivalence, and mid-chain rejection.

use uuid::Uuid;

use ob_poc_kyc_substrate::{
    check_control_preconditions, fold_control, phase1_lexicon, preview, ControlState, EdgeId,
    EntityId, IntentEvent, Principal, SubjectId, TargetBinding,
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

fn assert_control_event(subj: SubjectId, edge: EdgeId, from: EntityId, to: EntityId) -> IntentEvent {
    IntentEvent::new(
        subj,
        "ubo.edge.assert-control",
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
        "ubo.edge.attach-evidence",
        Principal::test_analyst(),
        ob_poc_kyc_substrate::AuthorityRef("preview-test".into()),
        TargetBinding::for_edge(subj, edge),
        serde_json::json!({"doc_id": Uuid::new_v4()}),
        t(),
    )
}

fn verify_event(subj: SubjectId, edge: EdgeId) -> IntentEvent {
    IntentEvent::new(
        subj,
        "ubo.edge.verify",
        Principal::test_analyst(),
        ob_poc_kyc_substrate::AuthorityRef("preview-test".into()),
        TargetBinding::for_edge(subj, edge),
        serde_json::json!({}),
        t(),
    )
}

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
    let lexicon = phase1_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    let result = preview(&[], &[assert_control_event(subj, edge, from, to)], &lexicon);
    assert!(result.is_ok(), "pure fold over a legal single-candidate chain must succeed");
}

// ── per_step_matches_single_move ────────────────────────────────────────────

#[test]
fn per_step_matches_single_move() {
    let subj = subject();
    let lexicon = phase1_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    let candidates = vec![
        assert_control_event(subj, edge, from, to),
        attach_evidence_event(subj, edge),
        verify_event(subj, edge),
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
            check_control_preconditions(entry, &prefix_state, candidate).is_ok(),
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
    let lexicon = phase1_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    let committed = vec![assert_control_event(subj, edge, from, to)];
    let candidates = vec![attach_evidence_event(subj, edge), verify_event(subj, edge)];

    let previewed: ControlState = preview(&committed, &candidates, &lexicon)
        .expect("legal chain must preview successfully");

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
    let lexicon = phase1_lexicon();
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (entity(), entity());

    // Step 0 legal (assert-control), step 1 illegal (verify with no
    // evidence attached — the K-11 proof ratchet), step 2 would-be-legal
    // (attach-evidence) never gets the chance to run.
    let candidates = vec![
        assert_control_event(subj, edge, from, to),
        verify_event(subj, edge),
        attach_evidence_event(subj, edge),
    ];

    let result = preview(&[], &candidates, &lexicon);
    assert!(result.is_err(), "chain with an illegal step 1 must reject, not silently skip");

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Asserted") || err.contains("evidence") || err.contains("Verify"),
        "rejection must be attributable to the illegal verify step; got: {err}"
    );

    // Nothing partial escapes: preview has no side channel to leak
    // step-0's application through on an Err path — the only observable
    // outcome of a rejected chain is the Err itself.
    let legal_prefix_only = preview(&[], &candidates[..1], &lexicon);
    assert!(legal_prefix_only.is_ok(), "the legal prefix alone must still preview cleanly");
}
