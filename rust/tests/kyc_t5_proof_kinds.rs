//! T5 gate tests — EOP-DD-UBO-PROOF-001 v0.2 §5 (RATIFIED 2026-08-28: §2,
//! the seven proof kinds; §3, the board collects facts, the policy rules on
//! adequacy). Closes EOP-VS-UBO-GAME-001 §3.1's `evidence`/`retract` moves
//! and §7's `verification` removal.
//!
//! RULING: **the board collects facts; the policy rules on adequacy.**
//! `EdgeStatus::Verified` and `TypeProofStatus::Proved`-as-a-state are gone.
//! Each assertion (edge or entity type) carries a set of proofs — kind,
//! source, date — keyed by the citing event. `evidence` logs one; `retract`
//! withdraws one. Nothing sets a status.
//!
//! Six gates (§5):
//! - `the_board_reports_proofs_not_adequacy` — structural, the point of the
//!   tranche: no build-game path returns a verified/proven boolean for an
//!   assertion. Grep-proof and type-level.
//! - `evidence_logs_kind_source_and_date` — all three recorded and readable.
//! - `retract_removes_a_proof_and_nothing_else` — the set shrinks; no
//!   status changes, because none exists.
//! - `assurance_reports_the_citation_set` — the profile names which proofs
//!   exist per assertion, with kinds and dates, not a verdict.
//! - `no_move_sets_verified` — no verb writes a proof status.
//! - `proof_kinds_are_exhaustive` — a new kind is a compile error until
//!   ruled.

const CONTROL_FOLD_SRC: &str =
    include_str!("../crates/ob-poc-kyc-substrate/src/fold/control.rs");
const TYPE_REGISTRY_FOLD_SRC: &str =
    include_str!("../crates/ob-poc-kyc-substrate/src/fold/type_registry.rs");
const DETERMINATION_SRC: &str = include_str!("../crates/ob-poc-kyc-substrate/src/determination.rs");
const EVALUATION_SRC: &str = include_str!("../crates/ob-poc-kyc-substrate/src/evaluation.rs");
const LEXICON_SRC: &str = include_str!("../crates/ob-poc-kyc-substrate/src/lexicon.rs");
const CANONICAL_SRC: &str = include_str!("../crates/ob-poc-kyc-seam/src/canonical.rs");
const KYC_STREAM_OPS_SRC: &str = include_str!("../src/domain_ops/kyc_stream_ops.rs");
const DSL_KYC_YAML: &str = include_str!("../config/verbs/kyc/dsl-kyc.yaml");

/// EOP-DD-UBO-PROOF-001 §5, structural: no build-game path returns a
/// verified/proven boolean for an assertion. The concept is ABSENT from
/// source, not merely unused — a grep-proof, not a runtime assertion,
/// because a state that can be reintroduced by one careless `match` arm is
/// not actually gone.
#[test]
fn the_board_reports_proofs_not_adequacy() {
    for (needle, file, label) in [
        ("EdgeStatus::Verified", CONTROL_FOLD_SRC, "fold/control.rs"),
        ("EdgeStatus::Evidenced", CONTROL_FOLD_SRC, "fold/control.rs"),
        ("EdgeStatus::Verified", DETERMINATION_SRC, "determination.rs"),
        ("TypeProofStatus", CONTROL_FOLD_SRC, "fold/control.rs"),
        ("TypeProofStatus", TYPE_REGISTRY_FOLD_SRC, "fold/type_registry.rs"),
        ("TypeProofStatus", DETERMINATION_SRC, "determination.rs"),
        ("TypeProofStatus", EVALUATION_SRC, "evaluation.rs"),
        ("EvidenceCited", LEXICON_SRC, "lexicon.rs"),
        ("VerifyWithoutEvidence", LEXICON_SRC, "lexicon.rs"),
        ("verification:", DSL_KYC_YAML, "dsl-kyc.yaml"),
    ] {
        assert!(
            !file.contains(needle),
            "{label} still contains {needle:?} — a verified/proven status \
             concept must be structurally absent, not merely unused"
        );
    }

    // canonical.rs/kyc_stream_ops.rs: the FQN itself legitimately survives
    // as a string literal in a "this must now be refused" test/retirement
    // comment (same discipline the T3-retired FQNs — economic-interest,
    // supersession, reconciliation — already follow there). What must be
    // ABSENT is a live match arm dispatching it — `"fqn" => {` — the actual
    // "still does something" shape.
    for (file, label) in [(CANONICAL_SRC, "canonical.rs"), (KYC_STREAM_OPS_SRC, "kyc_stream_ops.rs")] {
        assert!(
            !file.contains("\"kyc_ubo.assert.edge.verification\" =>"),
            "{label} still has a live match arm for kyc_ubo.assert.edge.verification — \
             the verb must have no dispatch target left, not merely no lexicon entry"
        );
    }
}

// ── evidence_logs_kind_source_and_date ──────────────────────────────────────

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    compute_assurance, fold_control, AuthorityRef, ControlState, EdgeId, EntityId, IntentEvent,
    PersonId, Principal, ProofKind, Prong, ProngCandidate, SubjectId, TargetBinding,
};

fn as_of() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 28, 0, 0, 0).unwrap()
}

fn analyst() -> Principal {
    Principal::test_analyst()
}

fn authority() -> AuthorityRef {
    AuthorityRef("t5-gate-test".into())
}

/// One `connect` event asserting `from -> to` (`voting_rights`), and one
/// `evidence` event logging a `filed-document` proof against it, dated
/// "2026-01-01", sourced "Companies House".
fn connect_and_evidence_events(
    subject: SubjectId,
    edge: EdgeId,
    from: EntityId,
    to: EntityId,
) -> (IntentEvent, IntentEvent) {
    let connect = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.connect",
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({
            "kind": "voting_rights", "edge_id": edge.0,
            "from_entity_id": from.0, "to_entity_id": to.0,
        }),
        as_of(),
    );
    let evidence = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.evidence",
        analyst(),
        authority(),
        TargetBinding::for_edge(subject, edge),
        serde_json::json!({
            "kind": "filed-document", "source": "Companies House", "date": "2026-01-01",
        }),
        as_of(),
    );
    (connect, evidence)
}

fn place_event(subject: SubjectId, entity: EntityId, entity_type: &str) -> IntentEvent {
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.place",
        analyst(),
        authority(),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({ "entity_id": entity.0, "entity_type": entity_type }),
        as_of(),
    )
}

#[test]
fn evidence_logs_kind_source_and_date() {
    let subject = SubjectId(Uuid::new_v4());
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (EntityId(Uuid::new_v4()), EntityId(Uuid::new_v4()));
    let (connect, evidence) = connect_and_evidence_events(subject, edge, from, to);

    let state = fold_control(&[&connect, &evidence]);
    let e = state.edges.get(&edge).expect("edge must exist");
    assert_eq!(e.proofs.len(), 1, "exactly one proof must be logged");
    let proof = e.proofs.values().next().unwrap();
    assert_eq!(proof.kind, ProofKind::FiledDocument);
    assert_eq!(proof.source, "Companies House");
    assert_eq!(proof.date, "2026-01-01");
    assert_eq!(proof.event_id, evidence.id, "the proof is keyed by its citing event");
}

// ── retract_removes_a_proof_and_nothing_else ────────────────────────────────

#[test]
fn retract_removes_a_proof_and_nothing_else() {
    let subject = SubjectId(Uuid::new_v4());
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (EntityId(Uuid::new_v4()), EntityId(Uuid::new_v4()));
    let (connect, evidence) = connect_and_evidence_events(subject, edge, from, to);
    let retract = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.retract",
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        serde_json::json!({ "citation_id": evidence.id.0.to_string() }),
        as_of(),
    );

    let before = fold_control(&[&connect, &evidence]);
    let before_edge = before.edges.get(&edge).expect("edge exists before retract").clone();
    assert!(before_edge.has_proof(), "sanity: the proof must exist before retract");

    let after = fold_control(&[&connect, &evidence, &retract]);
    let after_edge = after.edges.get(&edge).expect("edge still exists after retract");

    assert!(!after_edge.has_proof(), "the set must shrink to empty");
    assert!(after_edge.proofs.is_empty());
    // "and nothing else happens" — every other field is byte-identical.
    assert_eq!(after_edge.status, before_edge.status, "no status to demote");
    assert_eq!(after_edge.kind, before_edge.kind);
    assert_eq!(after_edge.from, before_edge.from);
    assert_eq!(after_edge.to, before_edge.to);
    assert_eq!(after_edge.percentage, before_edge.percentage);
    assert_eq!(after_edge.originating_event_id, before_edge.originating_event_id);
    assert_eq!(after_edge.superseded_by, before_edge.superseded_by);
    assert!(after_edge.is_active(), "retract never supersedes the edge");
}

// ── assurance_reports_the_citation_set ──────────────────────────────────────

#[test]
fn assurance_reports_the_citation_set() {
    let subject = SubjectId(Uuid::new_v4());
    let edge = EdgeId(Uuid::new_v4());
    let (from, to) = (EntityId(Uuid::new_v4()), EntityId(Uuid::new_v4()));
    let (connect, evidence) = connect_and_evidence_events(subject, edge, from, to);
    // A fully-typed pair of endpoints — isolates the citation signal from
    // TS.5 R6's separate `GeometryUnevaluable` reason (untyped endpoints),
    // which is not what this gate is about.
    let place_from = place_event(subject, from, "natural_person");
    let place_to = place_event(subject, to, "private_limited_company");

    let control: ControlState = fold_control(&[&connect, &evidence]);
    let type_registry = ob_poc_kyc_substrate::fold_type_registry(&[&place_from, &place_to]);
    let candidate = ProngCandidate {
        person_id: PersonId(from.0),
        prong: Prong::ControlByOtherMeans,
        effective_ownership_pct: None,
        ownership_chain: vec![to, from],
        originating_event_id: connect.id,
        pivot: None,
        pierces: Vec::new(),
        bases: Vec::new(),
    };

    let assurance = compute_assurance(&[candidate], &[], &control, &type_registry);

    // Not a verdict: the citation channel reports the proof regardless of
    // whether `reasons` flags anything — this edge IS cited, so it must
    // never appear as `UncitedEdge` in `reasons` (the endpoints' own types
    // are deliberately left uncited here — an orthogonal `UncitedType`
    // signal, not this gate's concern), but `edge_citations` still names
    // the proof explicitly (kind + date), which `reasons` alone never did.
    let citations = assurance.edge_citations.get(&edge).expect("edge must be reported");
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].kind, ProofKind::FiledDocument);
    assert_eq!(citations[0].date, "2026-01-01");
    assert!(
        !assurance.reasons.iter().any(|r| matches!(
            r,
            ob_poc_kyc_substrate::ProvisionalityReason::UncitedEdge { .. }
        )),
        "a cited edge must never be flagged UncitedEdge: {:?}",
        assurance.reasons
    );
}

// ── no_move_sets_verified ────────────────────────────────────────────────────

/// EOP-DD-UBO-PROOF-001 §4/§5: no verb writes a proof status — there is no
/// status left to write. Structural: exactly one call site in each fold
/// file inserts into `.proofs` (the `evidence` arm) and exactly one removes
/// from it (the `retract` arm). More than one insert site would mean some
/// OTHER move can also mint a proof, unauditably; a mismatched insert/remove
/// count would mean retract isn't the sole undo path.
#[test]
fn no_move_sets_verified() {
    for (file, label) in [
        (CONTROL_FOLD_SRC, "fold/control.rs"),
        (TYPE_REGISTRY_FOLD_SRC, "fold/type_registry.rs"),
    ] {
        let inserts = file.matches(".proofs.insert(").count();
        let removes = file.matches(".proofs.remove(").count();
        assert_eq!(inserts, 1, "{label}: exactly one move (evidence) may insert a proof, found {inserts}");
        assert_eq!(removes, 1, "{label}: exactly one move (retract) may remove a proof, found {removes}");
    }
}

// ── proof_kinds_are_exhaustive ───────────────────────────────────────────────

/// D3's compile-time-guarantee pattern: an exhaustive match with NO
/// catch-all arm. Adding an eighth `ProofKind` variant is a compile error
/// here until this match is consciously extended — the same discipline
/// `DeterminationDispatch`'s exhaustive `EntityType` match uses.
#[test]
fn proof_kinds_are_exhaustive() {
    fn wire_of(k: ProofKind) -> &'static str {
        match k {
            ProofKind::RegistryExtract => "registry-extract",
            ProofKind::ConstitutionalDocument => "constitutional-document",
            ProofKind::ShareRegister => "share-register",
            ProofKind::FiledDocument => "filed-document",
            ProofKind::Contract => "contract",
            ProofKind::IdentityDocument => "identity-document",
            ProofKind::Attestation => "attestation",
        }
    }
    assert_eq!(ob_poc_kyc_substrate::PROOF_KIND_WIRE_VALUES.len(), 7);
    for &wire in ob_poc_kyc_substrate::PROOF_KIND_WIRE_VALUES {
        let kind = ob_poc_kyc_substrate::proof_kind_from_wire(wire)
            .unwrap_or_else(|| panic!("{wire} must parse to a ProofKind"));
        assert_eq!(wire_of(kind), wire, "wire round-trip must match for {wire}");
    }
}
