//! P3 — 2026-09-07 audit item 2 (K-8 pierce guard hoist), re-running C2
//! ("the board offers only moves the append will admit", `EOP-VS-UBO-GAME-001`
//! §3.4 R7/C2) on a MIXED board, against the single checker both the op
//! layer and the workbook now consult (`check_preconditions`).
//!
//! Before P1/P2, the audit found this property held at the precondition
//! layer while FAILING against the op's hand-rolled `pierced-from` guard —
//! a rule enforced by one surface (the op) and silently absent from the
//! other (the workbook), because no `Precondition` primitive expressed it.
//! With `PiercedFromIsActiveNominee`/`NoUnpiercedNomineeEdges` now declared,
//! the property is re-run here directly against `check_preconditions` — the
//! one function both `UboEdgeConnect::execute` (via `stream_append`) and
//! `KycWorkbook::commit` (via `append_in_scope`) call — so there is only one
//! layer left for it to hold or fail against.

use uuid::Uuid;

use ob_poc_kyc_substrate::{
    assembly_lexicon, check_control_preconditions, enumerate_placement_set, fold_control,
    fold_type_registry, AuthorityRef, EdgeId, EntityId, IntentEvent, KycError, ObligationState,
    Principal, SubjectId, TargetBinding,
};

fn as_of() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap()
}

fn place_event(subject: SubjectId, entity: EntityId, entity_type: &str) -> IntentEvent {
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.place",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({ "entity_id": entity.0, "entity_type": entity_type }),
        as_of(),
    )
}

#[allow(clippy::too_many_arguments)]
fn connect_event(
    subject: SubjectId,
    edge_id: EdgeId,
    from: EntityId,
    to: EntityId,
    kind: &str,
    pierced_from: Option<EdgeId>,
) -> IntentEvent {
    let mut payload = serde_json::json!({
        "edge_id": edge_id.0,
        "from_entity_id": from.0,
        "to_entity_id": to.0,
        "kind": kind,
    });
    if let Some(pf) = pierced_from {
        payload["pierced_from"] = serde_json::json!(pf.0);
    }
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.connect",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding::for_subject(subject),
        payload,
        as_of(),
    )
}

/// A MIXED board: subject placed, three further members placed (a real
/// nominee holder, a real voting-rights holder, a far end), one real
/// nominee edge, one real voting-rights edge — not the empty board every
/// P0-P2 gate used.
#[test]
fn c2_holds_against_the_single_checker_on_a_mixed_board() {
    let subj = SubjectId(Uuid::new_v4());
    let lexicon = assembly_lexicon();
    let nominee_holder = EntityId(Uuid::new_v4());
    let voter = EntityId(Uuid::new_v4());
    let far_end = EntityId(Uuid::new_v4());

    let real_nominee_edge = EdgeId(Uuid::new_v4());
    let real_voting_edge = EdgeId(Uuid::new_v4());

    let events: Vec<IntentEvent> = vec![
        place_event(subj, EntityId(subj.0), "private_limited_company"),
        place_event(subj, nominee_holder, "natural_person"),
        place_event(subj, voter, "natural_person"),
        place_event(subj, far_end, "private_limited_company"),
        connect_event(subj, real_nominee_edge, nominee_holder, EntityId(subj.0), "nominee", None),
        connect_event(subj, real_voting_edge, voter, EntityId(subj.0), "voting_rights", None),
    ];
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&refs);
    let type_registry = fold_type_registry(&refs);
    let obligation = ObligationState::default();

    let placement_set = enumerate_placement_set(subj, &control, &obligation, &type_registry, &lexicon);

    // ── offered ⇒ admitted: every concrete place/remove/enquiry candidate
    // the board offers, probed against the single checker with a REAL
    // target, is admitted. ────────────────────────────────────────────────
    let mut checked_offered = 0usize;
    for mv in &placement_set.moves {
        let Some(entry) = lexicon.get(mv.verb_fqn.as_str()) else { continue };
        if mv.verb_fqn.as_str() == "kyc_ubo.assert.edge.connect" {
            // connect is a coarse per-verb offer (geometry says SOME legal
            // triple exists); it does not pin a concrete triple the way
            // place/remove do — probed separately below with concrete,
            // caller-chosen triples instead.
            continue;
        }
        let probe = IntentEvent::new(
            subj,
            mv.verb_fqn.as_str(),
            Principal::test_analyst(),
            AuthorityRef("probe".into()),
            mv.target.clone(),
            serde_json::Value::Null,
            as_of(),
        );
        let result = check_control_preconditions(entry, &control, &type_registry, &probe);
        assert!(
            result.is_ok(),
            "C2: the board OFFERED {} against target {:?} but the checker refused it: {result:?}",
            mv.verb_fqn.as_str(),
            mv.target
        );
        checked_offered += 1;
    }
    assert!(checked_offered > 0, "the mixed board must offer at least one concrete candidate");

    // ── not-offered/illegal ⇒ refused: a pierce citing the REAL, non-nominee
    // voting-rights edge above must be refused — this is the audit's own
    // defect, re-proven at the single checker layer both surfaces share. ──
    let connect_entry = lexicon.get("kyc_ubo.assert.edge.connect").unwrap();
    let illegal_pierce = connect_event(
        subj,
        EdgeId(Uuid::new_v4()),
        voter,
        far_end,
        "voting_rights",
        Some(real_voting_edge),
    );
    let refused = check_control_preconditions(connect_entry, &control, &type_registry, &illegal_pierce);
    assert!(
        matches!(refused, Err(KycError::PreconditionFailed { .. })),
        "a pierce citing a genuinely non-nominee edge must be refused; got {refused:?}"
    );

    // ── the true positive: piercing the REAL nominee edge, with a distinct
    // triple (avoiding NoDuplicateActiveEdge), must be ADMITTED — the guard
    // must not overreach. ───────────────────────────────────────────────────
    let legal_pierce = connect_event(
        subj,
        EdgeId(Uuid::new_v4()),
        nominee_holder,
        far_end,
        "voting_rights",
        Some(real_nominee_edge),
    );
    let admitted = check_control_preconditions(connect_entry, &control, &type_registry, &legal_pierce);
    assert!(
        admitted.is_ok(),
        "piercing a genuinely active nominee edge must be admitted; got {admitted:?}"
    );

    // ── a non-offered move (remove on an entity never placed) must be
    // refused — the other half of C2, unrelated to piercing, re-confirmed
    // on this same mixed board. ─────────────────────────────────────────────
    let never_placed = EntityId(Uuid::new_v4());
    let remove_entry = lexicon.get("kyc_ubo.assert.subject.remove").unwrap();
    let bogus_remove = IntentEvent::new(
        subj,
        "kyc_ubo.assert.subject.remove",
        Principal::test_analyst(),
        AuthorityRef("probe".into()),
        TargetBinding { entity_id: Some(never_placed), ..TargetBinding::for_subject(subj) },
        serde_json::Value::Null,
        as_of(),
    );
    let refused2 = check_control_preconditions(remove_entry, &control, &type_registry, &bogus_remove);
    assert!(
        refused2.is_err(),
        "removing an entity that was never placed must be refused; got {refused2:?}"
    );
    assert!(
        !placement_set
            .moves
            .iter()
            .any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.remove"
                && m.target.entity_id == Some(never_placed)),
        "C2 sanity: the board must not have offered remove for an entity it never placed"
    );
}
