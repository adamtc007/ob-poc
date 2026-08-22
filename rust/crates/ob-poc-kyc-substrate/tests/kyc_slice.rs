//! KYC/UBO W1 vertical-slice tests — EOP-DD-KYCUBO-001 §7 exit criteria.
//!
//! These tests prove the **semantic** model for one structure class (private
//! company) in memory, without any schema migration.  They map 1:1 to the
//! gap-report §10 RED tests and the V&S success criteria.
//!
//! Exit criteria (must all pass for the slice to be DONE):
//! 1. Differential equality: ownership-prong output == today's compute-chains.
//! 2. Reconcile before fold: conflicting edges fail without reconcile.
//! 3. Proof ratchet: verify without evidence is rejected.
//! 4. SMO never-empty: empty ownership/control → SMO person or waiver.
//! 5. Replay determinism: append → supersede → recover prior at as_of.
//! 6. Originating event id: every candidate has K-35 traceability.
//! 7. Multi-role fold: one person, two roles → one subject, two obligations.

use std::collections::BTreeSet;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    check_control_preconditions, fold_control, fold_control_versioned,
    fold_obligations_versioned, freeze_determination, assembly_lexicon, reconciled_economic_edges,
    recover_determination_at, AuthorityRef, ControlState, DeterminationInProgress,
    DeterminationStrategy, EdgeId, EntityId, EventId, FoldImpl, FoldRegistry, Hash, IdemKey,
    IntentEvent, ObligationState, OwnershipProngStrategy, PersonId, Principal, Prong,
    RecoveryPin, SubjectId, TargetBinding, TypeRegistryState, V1FoldImpl,
};
use std::sync::Arc;

// ── Fixture helpers ───────────────────────────────────────────────────────────

/// Test helper: construct an `IntentEvent` with all fields.
///
/// Bridges the test fixtures (which pre-date the 7-arg constructor) to the
/// new builder API without touching each call site individually.  Call sites
/// still pass `idem("key")` for the idempotency arg.
#[allow(clippy::too_many_arguments)] // Explicit event fields keep fixture call sites auditable.
fn te(
    seq: u64,
    subject: SubjectId,
    verb: &str,
    lex_hash: Hash,
    actor: Principal,
    authority: AuthorityRef,
    target: TargetBinding,
    payload: serde_json::Value,
    idempotency_key: IdemKey,
    as_of: chrono::DateTime<Utc>,
) -> IntentEvent {
    IntentEvent::new(subject, verb, actor, authority, target, payload, as_of)
        .with_seq(seq)
        .with_lexicon_hash(lex_hash)
        .with_idempotency_key(idempotency_key)
}

/// Helper: build a `RecoveryPin` with the standard test values.
fn test_pin<'a>(policy: &'a str, lex_hash: Hash, ref_snap: Uuid) -> RecoveryPin<'a> {
    RecoveryPin {
        policy_version: policy,
        lexicon_manifest_hash: lex_hash,
        reference_snapshot_id: ref_snap,
        import_run_ids: std::collections::BTreeSet::new(),
        viewer: None,
    }
}

fn analyst() -> Principal {
    Principal::test_analyst()
}

fn authority() -> AuthorityRef {
    AuthorityRef("analyst.verify".into())
}

fn idem(s: &str) -> IdemKey {
    IdemKey::new(s)
}

fn ts(year: i32, month: u32, day: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
}

fn dummy_hash() -> Hash {
    Hash::of(b"test-lexicon")
}

// Entity IDs for the fixture.
fn entity_subject() -> EntityId {
    EntityId(Uuid::parse_str("10000000-0000-0000-0000-000000000001").unwrap())
}
fn entity_b() -> EntityId {
    EntityId(Uuid::parse_str("10000000-0000-0000-0000-000000000002").unwrap())
}
fn person_p1() -> PersonId {
    PersonId(Uuid::parse_str("20000000-0000-0000-0000-000000000001").unwrap())
}
fn person_p2() -> PersonId {
    PersonId(Uuid::parse_str("20000000-0000-0000-0000-000000000002").unwrap())
}
fn person_p3() -> PersonId {
    PersonId(Uuid::parse_str("20000000-0000-0000-0000-000000000003").unwrap())
}
// `person_smo()` retired with TS.6 §5: the only fixtures that named an SMO
// person did so by asserting `ubo.determination.apply-smo-fallback`. SMO is
// pulled on exhaustion from the officer population (TS.3 §4a), so a test
// wanting one now records an `OfficerAppointment` edge instead of naming a
// person out of band.

/// Deterministic edge id from label.
fn eid(label: &str) -> EdgeId {
    EdgeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, label.as_bytes()))
}

/// Build the standard private-company fixture:
///
/// ```
///   subject (entity A)
///     ← B owns 60% of A         (economic edge e_b_a)
///       ← P1 owns 80% of B      (P1 → subject = 60%×80% = 48% > 25%)
///       ← P2 owns 20% of B      (P2 → subject = 60%×20% = 12% < 25%)
///     ← P3 owns 40% of A direct (P3 → subject = 40% > 25%)
/// ```
///
/// UBOs at 25%: P1 (48%), P3 (40%).  P2 (12%) below threshold.
fn build_fixture_events(subject: SubjectId) -> Vec<IntentEvent> {
    let h = dummy_hash();
    let a = entity_subject();
    let b = entity_b();
    let p1 = person_p1();
    let p2 = person_p2();
    let p3 = person_p3();
    let t = ts(2026, 1, 1);

    vec![
        // Register the subject.
        te(
            0,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "is_natural_person": false, "role": "customer"}),
            idem("reg-a"),
            t,
        ),
        // Classify as private company.
        te(
            1,
            subject,
            "kyc_ubo.assert.subject.structure-class",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "structure_class": "private_company"}),
            idem("cls-a"),
            t,
        ),
        // Register persons as natural persons.
        te(
            2,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p1.0, "is_natural_person": true, "role": "ubo_candidate"}),
            idem("reg-p1"),
            t,
        ),
        te(
            3,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p2.0, "is_natural_person": true, "role": "ubo_candidate"}),
            idem("reg-p2"),
            t,
        ),
        te(
            4,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p3.0, "is_natural_person": true, "role": "ubo_candidate"}),
            idem("reg-p3"),
            t,
        ),
        // Economic edges.
        // B → A: 60%
        te(
            5,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({
                "edge_id": eid("b_a").0,
                "from_entity_id": b.0,
                "to_entity_id": a.0,
                "percentage": 60.0,
            }),
            idem("edge-b-a"),
            t,
        ),
        // P1 → B: 80%
        te(
            6,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({
                "edge_id": eid("p1_b").0,
                "from_entity_id": p1.0,
                "to_entity_id": b.0,
                "percentage": 80.0,
            }),
            idem("edge-p1-b"),
            t,
        ),
        // P2 → B: 20%
        te(
            7,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({
                "edge_id": eid("p2_b").0,
                "from_entity_id": p2.0,
                "to_entity_id": b.0,
                "percentage": 20.0,
            }),
            idem("edge-p2-b"),
            t,
        ),
        // P3 → A: 40%
        te(
            8,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({
                "edge_id": eid("p3_a").0,
                "from_entity_id": p3.0,
                "to_entity_id": a.0,
                "percentage": 40.0,
            }),
            idem("edge-p3-a"),
            t,
        ),
        // Reconcile conflict.
        te(
            9,
            subject,
            "kyc_ubo.assert.edge.reconciliation",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"note": "single-source; no conflict"}),
            idem("reconcile"),
            t,
        ),
        // Select strategy.
        te(
            10,
            subject,
            "ubo.determination.select-strategy",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"strategy": "ownership_prong_strategy"}),
            idem("select-strategy"),
            t,
        ),
    ]
}

/// Natural persons for the fixture.
/// Returns `BTreeSet` for deterministic iteration order in fold/determination paths.
fn fixture_natural_persons() -> BTreeSet<PersonId> {
    [person_p1(), person_p2(), person_p3()]
        .into_iter()
        .collect()
}

fn fixture_subject_id() -> SubjectId {
    SubjectId(Uuid::parse_str("30000000-0000-0000-0000-000000000001").unwrap())
}

// ── Exit criterion 1: Fixture differential (ownership-prong algorithm) ────────
//
// **SCOPE NOTE (W7 still owed):** This test is a *fixture differential* — it
// verifies that `OwnershipProngStrategy` produces the expected candidates for
// a hand-authored private-company fixture (same chain arithmetic as
// `ubo_compute.rs::ComputeChains`).  It does NOT constitute the live-oracle
// differential proof against the running `ubo.compute-chains` DB verb; that
// proof is W7 (separate work).  A green ec1 means "the algorithm is equivalent
// on this fixture", NOT "the demotion is proven end-to-end".
//
// (gap-report §10 Test 1, V&S Criterion 2)

#[test]
fn ec1_ownership_prong_differential_equality() {
    let subject = fixture_subject_id();
    let events = build_fixture_events(subject);
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);
    let natural_persons = fixture_natural_persons();

    let strategy = OwnershipProngStrategy;
    let mut candidates = strategy.resolve(&control, entity_subject(), &natural_persons, 25.0);
    candidates.sort_by_key(|candidate| candidate.person_id.0);

    // P1: 60% × 80% = 48.0% — above threshold.
    // P3: 40% direct — above threshold.
    // P2: 60% × 20% = 12.0% — BELOW threshold, must be absent.
    assert_eq!(
        candidates.len(),
        2,
        "expected P1 and P3 only; got: {:?}",
        candidates
    );

    let p1_cand = candidates
        .iter()
        .find(|c| c.person_id == person_p1())
        .expect("P1 must be a candidate");
    let p3_cand = candidates
        .iter()
        .find(|c| c.person_id == person_p3())
        .expect("P3 must be a candidate");

    let tolerance = 0.01;
    assert!(
        (p1_cand.effective_ownership_pct.unwrap() - 48.0).abs() < tolerance,
        "P1 effective% should be ~48, got {:?}",
        p1_cand.effective_ownership_pct,
    );
    assert!(
        (p3_cand.effective_ownership_pct.unwrap() - 40.0).abs() < tolerance,
        "P3 effective% should be ~40, got {:?}",
        p3_cand.effective_ownership_pct,
    );

    // All candidates must be under the ownership prong (K-1: basis mandatory).
    for c in &candidates {
        assert_eq!(
            c.prong,
            Prong::OwnershipProng,
            "prong must be OwnershipProng (K-1)"
        );
    }
}

// ── Exit criterion 2: Reconcile before fold ───────────────────────────────────
//
// "A determination over conflicting source edges reconciles first and never
// sums >100%" (K-14, gap-report Test 2, V&S Criterion 3).

#[test]
fn ec2_conflicting_edges_fail_without_reconcile() {
    let lexicon = assembly_lexicon();
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let t = ts(2026, 1, 1);

    // Two sources claim 70% and 60% — combined 130% (conflict).
    let events = [
        te(
            0,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "is_natural_person": false}),
            idem("reg"),
            t,
        ),
        te(
            1,
            subject,
            "kyc_ubo.assert.subject.structure-class",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "structure_class": "private_company"}),
            idem("cls"),
            t,
        ),
        te(
            2,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": eid("src1_a").0,
                "from_entity_id": Uuid::new_v4(), "to_entity_id": a.0, "percentage": 70.0}),
            idem("e1"),
            t,
        ),
        te(
            3,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": eid("src2_a").0,
                "from_entity_id": Uuid::new_v4(), "to_entity_id": a.0, "percentage": 60.0}),
            idem("e2"),
            t,
        ),
        // NO reconcile-conflict, NO select-strategy.
        // Trying to freeze must fail.
    ];
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);

    // Without reconcile: the total claimed is >100%.
    let total = control.total_claimed_economic_pct(entity_subject());
    assert!(
        total > 100.0,
        "total claimed % should exceed 100: got {total}"
    );

    // Check precondition for freeze: ReconciledProjection must fail.
    // (`ubo.determination.compute-fold` retired TS.6 P2 — freeze already
    // independently declares the identical precondition pair.)
    let freeze_entry = lexicon
        .get("kyc_ubo.decide.determination.freeze")
        .expect("freeze in lexicon");
    let dummy_event = te(
        4,
        subject,
        "kyc_ubo.decide.determination.freeze",
        h,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({}),
        idem("fold"),
        t,
    );
    let result =
        check_control_preconditions(freeze_entry, &control, &TypeRegistryState::default(), &dummy_event);
    assert!(result.is_err(), "freeze without reconcile must fail");
}

#[test]
fn ec2_reconciled_edges_do_not_exceed_100_percent() {
    let subject = fixture_subject_id();
    let events = build_fixture_events(subject); // has reconcile event
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);

    // The fixture uses a single source per edge, so no conflict.
    // After reconcile the direct % into subject A: 60% (from B) + 40% (P3) = 100%.
    let total_to_a = control.total_claimed_economic_pct(entity_subject());
    assert!(
        total_to_a <= 100.0 + f64::EPSILON,
        "total claimed into subject must not exceed 100%: got {total_to_a}",
    );
}

// ── Exit criterion 3: Proof ratchet ───────────────────────────────────────────
//
// "`kyc_ubo.assert.edge.verification` without a cited-evidence event is rejected; status is
// never settable" (K-11, gap-report Test 3).

#[test]
fn ec3_verify_without_evidence_is_rejected() {
    let lexicon = assembly_lexicon();
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let b = entity_b();
    let t = ts(2026, 1, 1);

    let edge = eid("b_a");
    let events = [
        te(
            0,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge.0, "from_entity_id": b.0,
                "to_entity_id": a.0, "percentage": 60.0}),
            idem("assert"),
            t,
        ),
        // Deliberately NO attach-evidence event.
    ];
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);

    // Edge must be Asserted, not Evidenced.
    let edge_state = control.edges.get(&edge).expect("edge must exist");
    assert_eq!(
        edge_state.status,
        ob_poc_kyc_substrate::EdgeStatus::Asserted,
        "edge without attach-evidence must be Asserted",
    );

    // Attempting verify must fail the precondition.
    let verify_entry = lexicon.get("kyc_ubo.assert.edge.verification").expect("verify in lexicon");
    let verify_event = te(
        1,
        subject,
        "kyc_ubo.assert.edge.verification",
        h,
        analyst(),
        authority(),
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({}),
        idem("verify-attempt"),
        t,
    );
    let result =
        check_control_preconditions(verify_entry, &control, &TypeRegistryState::default(), &verify_event);
    assert!(result.is_err(), "verify without evidence must fail (K-11)");

    // Verify the error is the right kind.
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Asserted") || err_str.contains("evidence") || err_str.contains("Verify"),
        "error message should mention evidence state; got: {err_str}",
    );
}

#[test]
fn ec3_verify_after_evidence_succeeds() {
    let lexicon = assembly_lexicon();
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let b = entity_b();
    let t = ts(2026, 1, 1);

    let edge = eid("b_a");
    let events = vec![
        te(
            0,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge.0, "from_entity_id": b.0,
                "to_entity_id": a.0, "percentage": 60.0}),
            idem("assert"),
            t,
        ),
        // Attach evidence.
        te(
            1,
            subject,
            "kyc_ubo.assert.edge.evidence",
            h,
            analyst(),
            authority(),
            TargetBinding::for_edge(subject, edge),
            serde_json::json!({"doc_id": Uuid::new_v4()}),
            idem("attach"),
            t,
        ),
    ];
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);

    // Edge must now be Evidenced.
    let edge_state = control.edges.get(&edge).expect("edge must exist");
    assert_eq!(
        edge_state.status,
        ob_poc_kyc_substrate::EdgeStatus::Evidenced
    );

    // Verify must now pass the precondition.
    let verify_entry = lexicon.get("kyc_ubo.assert.edge.verification").expect("verify in lexicon");
    let verify_event = te(
        2,
        subject,
        "kyc_ubo.assert.edge.verification",
        h,
        analyst(),
        authority(),
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({}),
        idem("verify"),
        t,
    );
    let result =
        check_control_preconditions(verify_entry, &control, &TypeRegistryState::default(), &verify_event);
    assert!(
        result.is_ok(),
        "verify after evidence must pass: {:?}",
        result
    );

    // Apply the verify event and check fold output.
    let mut all_events = events;
    all_events.push(verify_event);
    let event_refs2: Vec<&IntentEvent> = all_events.iter().collect();
    let control2 = fold_control(&event_refs2);
    let edge_state2 = control2.edges.get(&edge).expect("edge must still exist");
    assert_eq!(
        edge_state2.status,
        ob_poc_kyc_substrate::EdgeStatus::Verified,
        "edge after verify event must be Verified",
    );
}

// ── Exit criterion 4: SMO never-empty (K-5) ───────────────────────────────────
//
// "An empty ownership+control result yields an SMO person or authorised waiver,
// never silence" (K-5, gap-report Test 4).

// `ec4_smo_fallback_when_no_ubos_found` RETIRED (TS.6 §5, 2026-08-22).
//
// Its entire subject was the MANUAL SMO path: assert
// `ubo.determination.apply-smo-fallback`, then check
// `ControlState.smo_person_id` was set. That verb is retired — SMO is now
// PULLED on exhaustion by the traversal (`determination.rs` walks
// OfficerAppointment edges into the frontier and emits `Prong::SmoFallback`
// straight into `candidates`, TS.3 §4a), so `smo_person_id` has no writer
// and is permanently `None`. The scenario this test named ("no UBOs found →
// SMO") still exists; it is now reached by the pull, exercised in the TS.3
// state-owned/cooperative suite rather than by asserting a fact here.


#[test]
fn ec4_freeze_without_candidates_or_smo_fails() {
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let t = ts(2026, 1, 1);

    let events = [
        te(
            0,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0}),
            idem("reg"),
            t,
        ),
        te(
            1,
            subject,
            "kyc_ubo.assert.edge.reconciliation",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("reconcile"),
            t,
        ),
        te(
            2,
            subject,
            "ubo.determination.select-strategy",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"strategy": "ownership_prong_strategy"}),
            idem("strategy"),
            t,
        ),
        te(
            3,
            subject,
            "kyc_ubo.decide.determination.freeze",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("freeze"),
            t,
        ),
    ];
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);

    let det = DeterminationInProgress {
        strategy: control
            .structure_class
            .as_ref()
            .map(|c| ob_poc_kyc_substrate::strategy_for_structure_class(c).to_string()),
        candidates: vec![], // EMPTY
        smo_result: None,   // NO SMO
        compute_event_id: None,
        stops: vec![],
        smo_pull: None,
    };
    let freeze_event = events.last().unwrap();
    let result = freeze_determination(
        &det,
        &control,
        &TypeRegistryState::default(),
        freeze_event,
        "v1.0",
        dummy_hash(),
        Uuid::new_v4(),
        BTreeSet::new(),
        None,
    );
    assert!(
        result.is_err(),
        "freeze with no candidates and no SMO must fail (K-5)"
    );
}

// ── Exit criterion 5: Replay determinism (K-16/18/33) ────────────────────────
//
// "Append → supersede → re-freeze → recover the prior determination bit-
// identically at its as_of, pinned to lexicon-manifest + graph-hash" (Test 6).

#[test]
fn ec5_replay_determinism_after_supersede() {
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let b = entity_b();
    let t1 = ts(2026, 1, 1);
    let t2 = ts(2026, 6, 1); // Later timestamp for second freeze.

    let edge_b_a = eid("b_a");
    let p1 = person_p1();
    let p1_entity = EntityId(p1.0);

    let natural_persons: BTreeSet<PersonId> = [p1].into();

    // Phase 1: build state with P1 owning B which owns A.
    let mut events: Vec<IntentEvent> = vec![
        te(
            0,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "is_natural_person": false}),
            idem("reg-a"),
            t1,
        ),
        te(
            1,
            subject,
            "kyc_ubo.assert.subject.structure-class",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "structure_class": "private_company"}),
            idem("cls"),
            t1,
        ),
        te(
            2,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p1.0, "is_natural_person": true}),
            idem("reg-p1"),
            t1,
        ),
        // B → A: 60%
        te(
            3,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge_b_a.0,
                "from_entity_id": b.0, "to_entity_id": a.0, "percentage": 60.0}),
            idem("edge-b-a"),
            t1,
        ),
        // P1 → B: 100%
        te(
            4,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": eid("p1_b").0,
                "from_entity_id": p1_entity.0, "to_entity_id": b.0, "percentage": 100.0}),
            idem("edge-p1-b"),
            t1,
        ),
        te(
            5,
            subject,
            "kyc_ubo.assert.edge.reconciliation",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("reconcile1"),
            t1,
        ),
        te(
            6,
            subject,
            "ubo.determination.select-strategy",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"strategy": "ownership_prong_strategy"}),
            idem("strategy"),
            t1,
        ),
        // First freeze at seq=7 (t1).
        te(
            7,
            subject,
            "kyc_ubo.decide.determination.freeze",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("freeze1"),
            t1,
        ),
    ];

    // Compute the PRIOR determination by replaying up to seq=7.
    let lexicon_hash = dummy_hash();
    let ref_snap = Uuid::new_v4();
    let strategy = OwnershipProngStrategy;

    // Snapshot the first-freeze event set as owned so we can keep borrowing later.
    let events_phase1: Vec<IntentEvent> = events.to_vec();
    let events_at_freeze1: Vec<&IntentEvent> = events_phase1.iter().collect();
    let prior_det = recover_determination_at(
        &events_at_freeze1,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", lexicon_hash, ref_snap),
    )
    .expect("should recover determination at first freeze");

    // P1 at 60%: above 25% threshold.
    assert_eq!(
        prior_det.candidates.len(),
        1,
        "prior determination should have P1"
    );
    assert_eq!(prior_det.candidates[0].person_id, p1);
    assert!(prior_det.candidates[0].effective_ownership_pct.unwrap() >= 25.0);

    // Phase 2: supersede the B→A edge (ownership changed).
    events.push(te(
        8,
        subject,
        "kyc_ubo.assert.edge.supersession",
        h,
        analyst(),
        authority(),
        TargetBinding::for_edge(subject, edge_b_a),
        serde_json::json!({"reason": "company sold"}),
        idem("supersede-b-a"),
        t2,
    ));
    events.push(te(
        9,
        subject,
        "kyc_ubo.assert.edge.reconciliation",
        h,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({}),
        idem("reconcile2"),
        t2,
    ));
    // No ownership UBOs after restructuring, and no officer appointments for
    // the TS.3 §4a pull to walk → nothing to conclude with.
    //
    // TS.6 §5 (2026-08-22): this fixture used to insert a manual
    // `ubo.determination.apply-smo-fallback` here to give the second freeze a
    // result. That verb is retired — SMO is PULLED on exhaustion by the
    // traversal (TS.3 §4a), never asserted — so the second freeze is now
    // attempted against an empty determination, which is exactly the case K-5
    // exists to refuse. The refusal is asserted below.
    // Attempt to freeze again at t2 — K-5 must refuse this.
    events.push(te(
        10,
        subject,
        "kyc_ubo.decide.determination.freeze",
        h,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({}),
        idem("freeze2"),
        t2,
    ));

    let all_refs: Vec<&IntentEvent> = events.iter().collect();

    // Phase 2 gate (K-5, "a determination must never be silent"): the
    // supersede broke the only ownership chain, and there are no officer
    // appointments for the TS.3 §4a pull-on-exhaustion to walk. The freeze
    // therefore has nothing to conclude and MUST be refused — `freeze_
    // determination` returns `Err`, which `recover_determination_at` surfaces
    // as `None`.
    //
    // Before TS.6 §5 this was masked: the fixture asserted a manual
    // `apply-smo-fallback` to hand the freeze a result. With that verb retired
    // there is no way to fabricate one, so the K-5 refusal is now visible on
    // the replay path — a stronger assertion than the one it replaces.
    let latest_det = recover_determination_at(
        &all_refs,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", lexicon_hash, ref_snap),
    );
    assert!(
        latest_det.is_none(),
        "post-supersede: K-5 must refuse a determination with no candidates \
         and no pulled SMO; got {:?}",
        latest_det.map(|d| d.candidates),
    );

    // Replay prior at the prior graph-hash — must be bit-identical.
    let replay_events: Vec<&IntentEvent> = events_phase1.iter().collect();
    let replay_det = recover_determination_at(
        &replay_events,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", lexicon_hash, ref_snap),
    )
    .expect("replay must succeed");
    assert_eq!(
        prior_det.determination_hash, replay_det.determination_hash,
        "replay must produce bit-identical determination hash (K-16/18/33)",
    );
}

// ── Exit criterion 6: Originating event id (K-35) ────────────────────────────
//
// "Every node in the determination/obligation projections has an originating
// event id" (K-35, gap-report Test 7).

#[test]
fn ec6_every_candidate_has_originating_event_id() {
    let subject = fixture_subject_id();
    let events = build_fixture_events(subject);
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);
    let natural_persons = fixture_natural_persons();

    let strategy = OwnershipProngStrategy;
    let candidates = strategy.resolve(&control, entity_subject(), &natural_persons, 25.0);

    // Every candidate must have a non-nil originating_event_id (K-35).
    for c in &candidates {
        // EventId is a UUID; Uuid::nil() = all zeros.
        assert_ne!(
            c.originating_event_id.0,
            EventId::default().0,
            "candidate {:?} has nil originating_event_id (K-35 violated)",
            c.person_id,
        );
    }

    // Every edge in the control state must have an originating_event_id.
    for (eid, edge) in &control.edges {
        assert_ne!(
            edge.originating_event_id.0,
            EventId::default().0,
            "edge {:?} has nil originating_event_id (K-35 violated)",
            eid,
        );
    }
}

// ── Exit criterion 7: RETIRED (EOP-DD-KYCUBO-D2.0 §5, K-G7, 2026-08-22) ────
//
// `ec7_multi_role_person_one_subject_two_obligations` proved "the same
// person as shareholder + director folds into one subject with distinct
// basis-obligations" (K-21, K-22, gap-report Test 8) by appending two
// `kyc_ubo.assert.obligation.creation` events. `creation` — the only writer
// of a new `ObligationTracks` entry — is dissolved: the events now fall to
// the fold's catch-all, `obl_state.subjects` stays empty, and every
// assertion in this test would fail against the current (correct) fold.
// K-21/K-22 are not retired as principles — a run's findings will need the
// same "one identity, many distinct bases" shape once the check catalogue
// (D2.0 §7 Q2, out of scope here) exists — but there is no obligation-fold
// mechanism left to prove them against today.

// ── K-13: Supersede-never-delete ─────────────────────────────────────────────

#[test]
fn k13_superseded_edges_remain_in_fold() {
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let b = entity_b();
    let t = ts(2026, 1, 1);
    let edge = eid("b_a");

    let events = [
        te(
            0,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge.0, "from_entity_id": b.0,
                "to_entity_id": a.0, "percentage": 60.0}),
            idem("assert"),
            t,
        ),
        te(
            1,
            subject,
            "kyc_ubo.assert.edge.supersession",
            h,
            analyst(),
            authority(),
            TargetBinding::for_edge(subject, edge),
            serde_json::json!({"reason": "sold"}),
            idem("supersede"),
            t,
        ),
    ];
    let event_refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&event_refs);

    // Edge must still be present (never deleted — K-13).
    let edge_state = control
        .edges
        .get(&edge)
        .expect("superseded edge must remain in fold (K-13)");
    assert_eq!(
        edge_state.status,
        ob_poc_kyc_substrate::EdgeStatus::Superseded,
        "superseded edge must have Superseded status",
    );
    // And must not be in the active economic edge list.
    let active: Vec<_> = control.active_economic_edges().collect();
    assert!(
        !active.iter().any(|e| e.id == edge),
        "superseded edge must not appear in active edges",
    );
}

// ── K-30 lint: every Phase-1 verb has governing taxonomy ─────────────────────

#[test]
fn k30_all_phase1_verbs_declare_governing_taxonomy() {
    let lexicon = assembly_lexicon();
    // If this list is empty, the lexicon builder is broken.
    assert!(!lexicon.entries.is_empty(), "lexicon must not be empty");
    for (fqn, entry) in &lexicon.entries {
        // Governing taxonomy is always Some (it's a non-optional field).
        // Just verifying the entry exists and was constructed properly.
        let _ = &entry.governing_taxonomy;
        assert!(!entry.intent.is_empty(), "verb {fqn} must have an intent");
        assert!(
            !entry.writes.is_empty(),
            "verb {fqn} must declare at least one writes-fold"
        );
        assert!(
            !entry.fqn.0.is_empty(),
            "verb {fqn} must have a non-empty FQN"
        );
    }
}

// ── Q7: Lexicon manifest hash is stable ───────────────────────────────────────

#[test]
fn q7_lexicon_manifest_hash_is_stable() {
    let lex1 = assembly_lexicon();
    let lex2 = assembly_lexicon();
    assert_eq!(
        lex1.hash, lex2.hash,
        "lexicon manifest hash must be stable across calls (Q7)",
    );
}

// ── Phase-1 cleanup: fold determinism stress ─────────────────────────────────
//
// Folding the same event stream twice must yield bit-identical graph hashes.
// This is the stress test that catches the class of bug fixed in Phase 1
// (HashMap/HashSet non-deterministic iteration → different originating_event_id
// selection → different candidate set hash → different determination hash).
//
// Runs the full fixture 10 times and asserts all graph hashes match.

#[test]
fn phase1_fold_determinism_stress_graph_hash() {
    let subject = fixture_subject_id();

    // Collect graph hashes from 10 independent fold runs.
    let hashes: Vec<ob_poc_kyc_substrate::Hash> = (0..10)
        .map(|_| {
            let events = build_fixture_events(subject);
            let event_refs: Vec<&IntentEvent> = events.iter().collect();
            let control = fold_control(&event_refs);
            let edges = reconciled_economic_edges(&control);
            ob_poc_kyc_substrate::DeterminationPin::compute_graph_hash(&edges)
        })
        .collect();

    let first = hashes[0];
    for (i, h) in hashes.iter().enumerate() {
        assert_eq!(
            *h, first,
            "fold run {i} produced a different graph hash — determinism broken",
        );
    }
}

#[test]
fn phase1_fold_determinism_stress_determination_hash() {
    let subject = fixture_subject_id();
    let natural_persons = fixture_natural_persons();
    let h = dummy_hash();
    // ref_snap is fixed outside all iterations — it contributes to the pin hash.
    let ref_snap = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
    let strategy = OwnershipProngStrategy;

    // Build the event stream ONCE; clone it for each fold run.
    // The determinism invariant is: given the SAME events, folding N times yields
    // the SAME determination hash.  (New random IDs per run would test something
    // different — that fold output is insensitive to event id values — which is not
    // the invariant.  Event ids are structural inputs, not noise.)
    let mut base_events = build_fixture_events(subject);
    let t = ts(2026, 1, 1);
    base_events.push(te(
        11,
        subject,
        "kyc_ubo.decide.determination.freeze",
        h,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({}),
        idem("freeze-stress"),
        t,
    ));

    // Collect determination hashes from 10 independent fold+freeze runs over the
    // SAME event stream.
    let hashes: Vec<ob_poc_kyc_substrate::Hash> = (0..10)
        .map(|_| {
            let event_refs: Vec<&IntentEvent> = base_events.iter().collect();
            recover_determination_at(
                &event_refs,
                &strategy,
                &natural_persons,
                25.0,
                test_pin("v1.0", dummy_hash(), ref_snap),
            )
            .expect("determination must succeed")
            .determination_hash
        })
        .collect();

    let first = hashes[0];
    for (i, det_hash) in hashes.iter().enumerate() {
        assert_eq!(
            *det_hash, first,
            "fold run {i} produced a different determination hash — determinism broken (Q6, K-16/18/33)",
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// D2 — FoldRegistry version-dispatch tests (EOP-DD-KYCUBO-002 §3.5)
// ══════════════════════════════════════════════════════════════════════════════

// ── Phase 2: TOTAL dispatch — unregistered hash is a hard error ───────────────
//
// The single most important test.  The temptation is to fall back to
// "latest registered version" when a hash isn't found — that single "helpful"
// default silently destroys replay-faithfulness.  This test proves the contract:
// an unknown `lexicon_hash` is `KycError::UnregisteredLexiconHash`, not a fold.

#[test]
fn d2_phase2_unregistered_hash_is_hard_error_not_silent_fold() {
    let subject = fixture_subject_id();
    let a = entity_subject();
    let t = ts(2026, 1, 1);

    // An unrecognised hash — not in any registry.
    let unknown_hash = Hash::of(b"this-version-was-never-registered");

    let event = te(
        0,
        subject,
        "kyc_ubo.assert.subject.register",
        unknown_hash,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({"entity_id": a.0, "is_natural_person": false}),
        idem("reg"),
        t,
    );

    // Empty registry — nothing registered.
    let registry = FoldRegistry::new();
    let events = vec![&event as &IntentEvent];

    let result = fold_control_versioned(&events, &registry);
    assert!(
        result.is_err(),
        "fold_control_versioned with an unregistered hash must return Err, not silently succeed",
    );

    // The error must be the specific replay-integrity variant, not a generic error.
    match result.unwrap_err() {
        ob_poc_kyc_substrate::KycError::UnregisteredLexiconHash(h) => {
            assert_eq!(h, unknown_hash, "error must name the offending hash");
        }
        other => panic!("expected UnregisteredLexiconHash, got: {other:?}"),
    }
}

#[test]
fn d2_phase2_obligation_unregistered_hash_is_hard_error() {
    let subject = fixture_subject_id();
    let t = ts(2026, 1, 1);
    let unknown_hash = Hash::of(b"obligation-unknown-version");

    let event = te(
        0,
        subject,
        "kyc_ubo.assert.subject.register",
        unknown_hash,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({"entity_id": Uuid::new_v4()}),
        idem("reg"),
        t,
    );

    let registry = FoldRegistry::new();
    let events = vec![&event as &IntentEvent];

    let result = fold_obligations_versioned(&events, &registry);
    assert!(
        result.is_err(),
        "fold_obligations_versioned must reject unregistered hash"
    );
    assert!(
        matches!(
            result.unwrap_err(),
            ob_poc_kyc_substrate::KycError::UnregisteredLexiconHash(_)
        ),
        "must be UnregisteredLexiconHash",
    );
}

// ── Phase 3 Test A: Two versions actually dispatch differently ─────────────────
//
// Registers v1 (the real FoldImpl) and v2 (a no-op that ignores all events).
// Events tagged with v1's hash produce real state; events tagged with v2's hash
// produce empty state — proving dispatch IS by hash, not always-latest.

/// A no-op FoldImpl for testing.  Ignores every event; state is always the
/// default.  If this were the fallback it would hide the version-dispatch bug;
/// as an explicit registration it proves dispatch is by hash.
struct NoopFoldImpl;

impl FoldImpl for NoopFoldImpl {
    fn apply_control(&self, state: ControlState, _event: &IntentEvent) -> ControlState {
        state // intentionally ignore
    }
    fn apply_obligation(&self, state: ObligationState, _event: &IntentEvent) -> ObligationState {
        state // intentionally ignore
    }
}

#[test]
fn d2_phase3a_two_versions_dispatch_to_different_impls() {
    let subject = fixture_subject_id();
    let a = entity_subject();
    let b = entity_b();
    let t = ts(2026, 1, 1);

    // v1 hash = real phase1 lexicon manifest hash.
    let v1_hash = assembly_lexicon().hash;
    // v2 hash = a different content-addressed hash for a "second version".
    let v2_hash = Hash::of(b"v2-noop-lexicon-different-hash");

    let mut registry = FoldRegistry::new();
    registry.register(v1_hash, Arc::new(V1FoldImpl));
    registry.register(v2_hash, Arc::new(NoopFoldImpl));

    assert_eq!(registry.len(), 2, "both versions registered");

    // Build an event tagged with v1 that asserts a real economic edge.
    let v1_event = te(
        0,
        subject,
        "kyc_ubo.assert.edge.economic-interest",
        v1_hash,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({
            "edge_id": eid("b_a").0,
            "from_entity_id": b.0,
            "to_entity_id": a.0,
            "percentage": 60.0,
        }),
        idem("edge-v1"),
        t,
    );

    // Build the same event but tagged with v2 (the no-op version).
    let v2_event = te(
        0,
        subject,
        "kyc_ubo.assert.edge.economic-interest",
        v2_hash,
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({
            "edge_id": eid("b_a").0,
            "from_entity_id": b.0,
            "to_entity_id": a.0,
            "percentage": 60.0,
        }),
        idem("edge-v2"),
        t,
    );

    // v1 stream → V1FoldImpl → edge is asserted in the state.
    let v1_refs = vec![&v1_event as &IntentEvent];
    let v1_state =
        fold_control_versioned(&v1_refs, &registry).expect("v1 stream must fold successfully");
    assert_eq!(v1_state.edges.len(), 1, "v1 fold must record the edge");

    // v2 stream → NoopFoldImpl → event is ignored → no edges.
    let v2_refs = vec![&v2_event as &IntentEvent];
    let v2_state =
        fold_control_versioned(&v2_refs, &registry).expect("v2 stream must fold successfully");
    assert_eq!(
        v2_state.edges.len(),
        0,
        "v2 (no-op) fold must produce no edges — dispatch is by hash, not always-v1",
    );
}

// ── Phase 3 Test B: Replay determinism through the registry ───────────────────
//
// The ec5 class, now through the FoldRegistry: fold the same v1 stream twice
// and assert the resulting ControlState's graph-hash is bit-identical.
// This proves the registry doesn't introduce non-determinism.

#[test]
fn d2_phase3b_registry_replay_is_bit_identical() {
    let subject = fixture_subject_id();
    let events = build_fixture_events(subject); // build once, fold twice

    let v1_hash = assembly_lexicon().hash;
    let mut registry = FoldRegistry::new();
    registry.register(v1_hash, Arc::new(V1FoldImpl));

    // Tag every event in the fixture with v1_hash.
    let tagged: Vec<IntentEvent> = events
        .into_iter()
        .map(|mut e| {
            e.lexicon_hash = v1_hash;
            e
        })
        .collect();

    let refs: Vec<&IntentEvent> = tagged.iter().collect();

    let state1 = fold_control_versioned(&refs, &registry).expect("first fold must succeed");
    let state2 = fold_control_versioned(&refs, &registry).expect("second fold must succeed");

    // Compare via the deterministic graph-hash.
    use ob_poc_kyc_substrate::DeterminationPin;

    let edges1 = reconciled_economic_edges(&state1);
    let edges2 = reconciled_economic_edges(&state2);

    let hash1 = DeterminationPin::compute_graph_hash(&edges1);
    let hash2 = DeterminationPin::compute_graph_hash(&edges2);

    assert_eq!(
        hash1, hash2,
        "registry fold must produce bit-identical graph-hash on two runs (Q6, K-16/18/33)",
    );
    assert_eq!(state1.edges.len(), state2.edges.len(), "same edge count");
}

// ── T5 — Bitemporal recovery gate tests (EOP-PLAN-KYCUBO-KIT-001 §T5) ────────
//
// `recover_determination_bitemporal(events, .., valid_at, known_at)` filters
// the event slice by BOTH axes (as_of <= valid_at AND committed_at <=
// known_at) before delegating to `recover_determination_at` unchanged.

use ob_poc_kyc_substrate::recover_determination_bitemporal;

#[test]
fn axes_diverge_on_correction() {
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let p1 = person_p1();
    let p2 = person_p2();
    let p1_entity = EntityId(p1.0);
    let p2_entity = EntityId(p2.0);

    let t1 = ts(2026, 1, 1); // original facts asserted + first freeze
    let t2 = ts(2026, 6, 1); // correction's valid-time (P1's edge is superseded)
    let known_at = ts(2026, 8, 1); // both queries share full knowledge of the correction

    let before_correction = ts(2026, 3, 1); // strictly between t1 and t2
    let after_correction = ts(2026, 7, 1); // strictly after t2

    let edge_p1_a = eid("p1_a");
    let edge_p2_a = eid("p2_a");
    let natural_persons: BTreeSet<PersonId> = [p1, p2].into();

    let mut events: Vec<IntentEvent> = vec![
        te(
            0,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "is_natural_person": false}),
            idem("reg-a"),
            t1,
        ),
        te(
            1,
            subject,
            "kyc_ubo.assert.subject.structure-class",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "structure_class": "private_company"}),
            idem("cls"),
            t1,
        ),
        te(
            2,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p1.0, "is_natural_person": true}),
            idem("reg-p1"),
            t1,
        ),
        te(
            3,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p2.0, "is_natural_person": true}),
            idem("reg-p2"),
            t1,
        ),
        // P1 -> A: 30% (above 25% threshold; will be superseded at t2).
        te(
            4,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge_p1_a.0,
                "from_entity_id": p1_entity.0, "to_entity_id": a.0, "percentage": 30.0}),
            idem("edge-p1-a"),
            t1,
        ),
        // P2 -> A: 40% (above threshold; survives both windows).
        te(
            5,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge_p2_a.0,
                "from_entity_id": p2_entity.0, "to_entity_id": a.0, "percentage": 40.0}),
            idem("edge-p2-a"),
            t1,
        ),
        te(
            6,
            subject,
            "kyc_ubo.assert.edge.reconciliation",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("reconcile1"),
            t1,
        ),
        te(
            7,
            subject,
            "ubo.determination.select-strategy",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"strategy": "ownership_prong_strategy"}),
            idem("strategy"),
            t1,
        ),
        // First freeze at t1 — both P1 and P2 active.
        te(
            8,
            subject,
            "kyc_ubo.decide.determination.freeze",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("freeze1"),
            t1,
        ),
    ];

    // Correction: P1's edge is superseded, effective (valid-time) t2, but not
    // recorded/committed until well after — the divergence-producing event.
    events.push(
        te(
            9,
            subject,
            "kyc_ubo.assert.edge.supersession",
            h,
            analyst(),
            authority(),
            TargetBinding::for_edge(subject, edge_p1_a),
            serde_json::json!({"reason": "P1 sold its stake"}),
            idem("supersede-p1-a"),
            t2,
        )
        .with_committed_at(t2),
    );
    events.push(
        te(
            10,
            subject,
            "kyc_ubo.assert.edge.reconciliation",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("reconcile2"),
            t2,
        )
        .with_committed_at(t2),
    );
    // Second freeze at t2 — P2 remains active alone.
    events.push(
        te(
            11,
            subject,
            "kyc_ubo.decide.determination.freeze",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("freeze2"),
            t2,
        )
        .with_committed_at(t2),
    );

    let refs: Vec<&IntentEvent> = events.iter().collect();
    let strategy = OwnershipProngStrategy;
    let lexicon_hash = dummy_hash();
    let ref_snap = Uuid::new_v4();

    // Window 1: valid_at BEFORE the correction's effective date, but known_at
    // is fully caught up. The correction's as_of (t2) is excluded — P1 + P2
    // both stand.
    let det_before = recover_determination_bitemporal(
        &refs,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", lexicon_hash, ref_snap),
        before_correction,
        known_at,
    )
    .expect("should recover a determination before the correction's valid-time");

    // Window 2: valid_at AFTER the correction's effective date, same known_at.
    // The correction is now in-window — only P2 stands.
    let det_after = recover_determination_bitemporal(
        &refs,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", lexicon_hash, ref_snap),
        after_correction,
        known_at,
    )
    .expect("should recover a determination after the correction's valid-time");

    assert_eq!(
        det_before.candidates.len(),
        2,
        "before the correction's valid-time, both P1 and P2 stand: {:?}",
        det_before.candidates
    );
    assert_eq!(
        det_after.candidates.len(),
        1,
        "after the correction's valid-time, only P2 stands: {:?}",
        det_after.candidates
    );
    assert_eq!(
        det_after.candidates[0].person_id, p2,
        "the surviving candidate after correction must be P2"
    );
    assert_ne!(
        det_before.determination_hash, det_after.determination_hash,
        "the two axis windows must produce PROVABLY DIFFERENT determinations"
    );
}

#[test]
fn bitemporal_matches_txtime_when_axes_align() {
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let b = entity_b();
    let t1 = ts(2026, 1, 1);
    let far_future = ts(2027, 1, 1); // both axes aligned "now" — no filtering effect

    let edge_b_a = eid("b_a");
    let p1 = person_p1();
    let p1_entity = EntityId(p1.0);
    let natural_persons: BTreeSet<PersonId> = [p1].into();

    let events: Vec<IntentEvent> = vec![
        te(
            0,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "is_natural_person": false}),
            idem("reg-a"),
            t1,
        ),
        te(
            1,
            subject,
            "kyc_ubo.assert.subject.structure-class",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "structure_class": "private_company"}),
            idem("cls"),
            t1,
        ),
        te(
            2,
            subject,
            "kyc_ubo.assert.subject.register",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p1.0, "is_natural_person": true}),
            idem("reg-p1"),
            t1,
        ),
        // B -> A: 60%
        te(
            3,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge_b_a.0,
                "from_entity_id": b.0, "to_entity_id": a.0, "percentage": 60.0}),
            idem("edge-b-a"),
            t1,
        ),
        // P1 -> B: 100%
        te(
            4,
            subject,
            "kyc_ubo.assert.edge.economic-interest",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": eid("p1_b").0,
                "from_entity_id": p1_entity.0, "to_entity_id": b.0, "percentage": 100.0}),
            idem("edge-p1-b"),
            t1,
        ),
        te(
            5,
            subject,
            "kyc_ubo.assert.edge.reconciliation",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("reconcile1"),
            t1,
        ),
        te(
            6,
            subject,
            "ubo.determination.select-strategy",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"strategy": "ownership_prong_strategy"}),
            idem("strategy"),
            t1,
        ),
        te(
            7,
            subject,
            "kyc_ubo.decide.determination.freeze",
            h,
            analyst(),
            authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}),
            idem("freeze1"),
            t1,
        ),
    ];

    let refs: Vec<&IntentEvent> = events.iter().collect();
    let strategy = OwnershipProngStrategy;
    let lexicon_hash = dummy_hash();
    let ref_snap = Uuid::new_v4();

    let det_txtime = recover_determination_at(
        &refs,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", lexicon_hash, ref_snap),
    )
    .expect("recover_determination_at must succeed on the aligned fixture");

    let det_bitemporal = recover_determination_bitemporal(
        &refs,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", lexicon_hash, ref_snap),
        far_future,
        far_future,
    )
    .expect("recover_determination_bitemporal must succeed when both axes are aligned with now");

    assert_eq!(
        det_txtime.determination_hash, det_bitemporal.determination_hash,
        "bitemporal recovery with both axes aligned to 'now' must match pure transaction-time recovery"
    );
    assert_eq!(det_txtime.candidates.len(), det_bitemporal.candidates.len());
    assert_eq!(
        det_txtime.candidates[0].person_id,
        det_bitemporal.candidates[0].person_id
    );
}

/// R4 (`EOP-PLAN-GAMEBOARD-001`) gate: `legal_set_at_past_time_is_reproducible`.
/// Same fixture as `bitemporal_matches_txtime_when_axes_align` (same events,
/// same pin, same axes) — two independent replays must be bit-identical,
/// including the two new R4 pin fields, not just the pre-existing
/// `determination_hash`.
#[test]
fn r4_legal_set_at_past_time_is_reproducible() {
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let b = entity_b();
    let t1 = ts(2026, 1, 1);
    let far_future = ts(2027, 1, 1);

    let edge_b_a = eid("b_a");
    let p1 = person_p1();
    let p1_entity = EntityId(p1.0);
    let natural_persons: BTreeSet<PersonId> = [p1].into();

    let events: Vec<IntentEvent> = vec![
        te(
            0, subject, "kyc_ubo.assert.subject.register", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "is_natural_person": false}),
            idem("r4-reg-a"), t1,
        ),
        te(
            1, subject, "kyc_ubo.assert.subject.structure-class", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "structure_class": "private_company"}),
            idem("r4-cls"), t1,
        ),
        te(
            2, subject, "kyc_ubo.assert.subject.register", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p1.0, "is_natural_person": true}),
            idem("r4-reg-p1"), t1,
        ),
        te(
            3, subject, "kyc_ubo.assert.edge.economic-interest", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge_b_a.0, "from_entity_id": b.0, "to_entity_id": a.0, "percentage": 60.0}),
            idem("r4-edge-b-a"), t1,
        ),
        te(
            4, subject, "kyc_ubo.assert.edge.economic-interest", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": eid("r4_p1_b").0, "from_entity_id": p1_entity.0, "to_entity_id": b.0, "percentage": 100.0}),
            idem("r4-edge-p1-b"), t1,
        ),
        te(
            5, subject, "kyc_ubo.assert.edge.reconciliation", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}), idem("r4-reconcile1"), t1,
        ),
        te(
            6, subject, "ubo.determination.select-strategy", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"strategy": "ownership_prong_strategy"}),
            idem("r4-strategy"), t1,
        ),
        te(
            7, subject, "kyc_ubo.decide.determination.freeze", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}), idem("r4-freeze1"), t1,
        ),
    ];

    let refs: Vec<&IntentEvent> = events.iter().collect();
    let strategy = OwnershipProngStrategy;
    let lexicon_hash = dummy_hash();
    let ref_snap = Uuid::new_v4();

    let replay = || {
        recover_determination_bitemporal(
            &refs,
            &strategy,
            &natural_persons,
            25.0,
            test_pin("v1.0", lexicon_hash, ref_snap),
            far_future,
            far_future,
        )
        .expect("recover_determination_bitemporal must succeed on this fixture")
    };

    let det1 = replay();
    let det2 = replay();

    assert_eq!(
        det1.determination_hash, det2.determination_hash,
        "two independent replays of the same (events, pin, axes) must be bit-identical"
    );
    assert_eq!(
        det1.pin.kit_version, det2.pin.kit_version,
        "kit_version must be identical across replays of the same build"
    );
    assert_eq!(
        det1.pin.constructor_version, det2.pin.constructor_version,
        "constructor_version must be identical across replays of the same strategy"
    );
}

/// R4 gate: `legality_pins_its_ruleset`. The pin must honestly name which
/// build (`kit_version`) and which strategy (`constructor_version`) produced
/// the determination — this is the KIT-6 pin-set gap R6 found missing
/// (viewer/kit-version/constructor-version; `axis` was already handled by
/// `recover_determination_bitemporal`'s explicit valid_at/known_at params).
#[test]
fn r4_legality_pins_its_ruleset() {
    let subject = fixture_subject_id();
    let h = dummy_hash();
    let a = entity_subject();
    let t1 = ts(2026, 1, 1);
    let p1 = person_p1();
    let natural_persons: BTreeSet<PersonId> = [p1].into();

    let events: Vec<IntentEvent> = vec![
        te(
            0, subject, "kyc_ubo.assert.subject.register", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "is_natural_person": false}),
            idem("r4b-reg-a"), t1,
        ),
        te(
            1, subject, "kyc_ubo.assert.subject.structure-class", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": a.0, "structure_class": "private_company"}),
            idem("r4b-cls"), t1,
        ),
        te(
            2, subject, "kyc_ubo.assert.subject.register", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"entity_id": p1.0, "is_natural_person": true}),
            idem("r4b-reg-p1"), t1,
        ),
        te(
            3, subject, "kyc_ubo.assert.edge.economic-interest", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": eid("r4b_p1_a").0, "from_entity_id": p1.0, "to_entity_id": a.0, "percentage": 100.0}),
            idem("r4b-edge"), t1,
        ),
        te(
            4, subject, "kyc_ubo.assert.edge.reconciliation", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}), idem("r4b-reconcile"), t1,
        ),
        te(
            5, subject, "ubo.determination.select-strategy", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({"strategy": "ownership_prong_strategy"}),
            idem("r4b-strategy"), t1,
        ),
        te(
            6, subject, "kyc_ubo.decide.determination.freeze", h, analyst(), authority(),
            TargetBinding::for_subject(subject),
            serde_json::json!({}), idem("r4b-freeze"), t1,
        ),
    ];

    let refs: Vec<&IntentEvent> = events.iter().collect();
    let strategy = OwnershipProngStrategy;

    let det = recover_determination_at(
        &refs,
        &strategy,
        &natural_persons,
        25.0,
        test_pin("v1.0", dummy_hash(), Uuid::new_v4()),
    )
    .expect("recover_determination_at must succeed on this fixture");

    assert_eq!(
        det.pin.kit_version,
        env!("CARGO_PKG_VERSION"),
        "kit_version must name the real build that computed this determination"
    );
    assert_eq!(
        det.pin.constructor_version.as_deref(),
        Some("ownership_prong_strategy"),
        "constructor_version must honestly name the strategy that actually resolved candidates, \
         not leave it implicit in policy_version's ambiguous semantics"
    );
    assert!(
        det.pin.viewer.is_none(),
        "viewer is additive metadata, not a new required field — None is a legitimate, \
         honest answer when no actor context was threaded through (as here)"
    );

    // A pin from a DIFFERENT kit_version must not silently compare equal —
    // this is what makes a future ruleset drift detectable rather than
    // silently accepted (the mechanism half of `legality_pins_its_ruleset`;
    // this crate has no live version-bump to exercise within one test run,
    // so this proves the field is load-bearing in equality, not decorative).
    let mut drifted_pin = det.pin.clone();
    drifted_pin.kit_version = "0.0.0-simulated-drift".to_string();
    assert_ne!(
        det.pin, drifted_pin,
        "a pin with a different kit_version must not compare equal to the original — \
         drift must be visible, not silently absorbed"
    );
}
