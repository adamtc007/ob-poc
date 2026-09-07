//! T1 gate test — `EOP-PLAN-KYCUBO-KIT-001` §T1: `sexpr_roundtrip_property`.
//!
//! Property: for a generated event across representative verb shapes,
//! `parse(render(event))` yields a `RawAtom` whose `kind` is the verb FQN and
//! whose slots are exactly the union of the event's target-binding fields and
//! its payload object entries. This is the CI equivalence gate for
//! `render_intent_event_to_sexpr` (KIT-1: the S-expression must be a real,
//! re-parseable DSL statement, not just a string that looks like one).

use chrono::{TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

use dsl_parser::{RawValue, Token};

use ob_poc_kyc_substrate::{
    render_intent_event_to_sexpr, AuthorityRef, EdgeId, EntityId, IntentEvent, ObligationId,
    PersonId, Principal, SubjectId, TargetBinding,
};

fn analyst() -> Principal {
    Principal {
        actor_id: Uuid::new_v4(),
        role: "analyst".into(),
    }
}

fn authority() -> AuthorityRef {
    AuthorityRef("analyst.test".into())
}

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 12, 9, 0, 0).unwrap()
}

/// Parse `src` with the real DSL lexer/parser and return the single top-level
/// atom, failing loudly (not silently) on any parse diagnostic.
fn parse_one_atom(src: &str) -> dsl_parser::RawAtom {
    let (source_file, diagnostics) = dsl_parser::parse(src);
    assert!(
        !diagnostics.has_errors(),
        "rendered source failed to re-parse: {src:?}\ndiagnostics: {diagnostics:?}"
    );
    assert_eq!(
        source_file.atoms.len(),
        1,
        "expected exactly one top-level atom, got {}: {src:?}",
        source_file.atoms.len()
    );
    source_file.atoms.into_iter().next().unwrap()
}

/// Reduce a parsed `RawValue` back to the `serde_json::Value` it must have
/// come from, for scalar/string/bool/int/float cases (the only shapes KYC
/// payloads use today — see `render.rs`'s doc comment).
fn raw_value_to_json(v: &RawValue) -> serde_json::Value {
    match v {
        RawValue::StringLit(s) => json!(s),
        RawValue::IntLit(n) => json!(n),
        RawValue::FloatLit(f) => json!(f),
        RawValue::BoolLit(b) => json!(b),
        RawValue::List(items) => serde_json::Value::Array(items.iter().map(raw_value_to_json).collect()),
        RawValue::Map(entries) => serde_json::Value::Object(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), raw_value_to_json(v)))
                .collect(),
        ),
        other => panic!("unexpected RawValue shape in KYC round-trip test: {other:?}"),
    }
}

/// Assert that parsing `render_intent_event_to_sexpr(event, None)` reproduces
/// exactly the event's verb FQN plus the union of target-binding fields and
/// payload entries — the round-trip equivalence property (T1 gate test).
fn assert_roundtrips(event: &IntentEvent) {
    let rendered = render_intent_event_to_sexpr(event, None);
    let atom = parse_one_atom(&rendered);

    assert_eq!(atom.kind, event.verb_fqn.0);

    let mut expected: Vec<(String, serde_json::Value)> = Vec::new();
    if let Some(id) = &event.target.subject_root {
        expected.push(("subject-id".to_string(), json!(id.0.to_string())));
    }
    if let Some(id) = &event.target.edge_id {
        expected.push(("edge-id".to_string(), json!(id.0.to_string())));
    }
    if let Some(id) = &event.target.entity_id {
        expected.push(("entity-id".to_string(), json!(id.0.to_string())));
    }
    if let Some(id) = &event.target.person_id {
        expected.push(("person-id".to_string(), json!(id.0.to_string())));
    }
    if let Some(id) = &event.target.obligation_id {
        expected.push(("obligation-id".to_string(), json!(id.0.to_string())));
    }
    if let serde_json::Value::Object(map) = &event.payload {
        for (k, v) in map {
            if v.is_null() {
                continue;
            }
            if let Some(existing) = expected.iter_mut().find(|(name, _)| name == k) {
                existing.1 = v.clone();
            } else {
                expected.push((k.clone(), v.clone()));
            }
        }
    }
    expected.sort_by(|a, b| a.0.cmp(&b.0));

    let mut actual: Vec<(String, serde_json::Value)> = atom
        .slots
        .iter()
        .map(|(name, v)| (name.clone(), raw_value_to_json(v)))
        .collect();
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(actual, expected, "round-trip mismatch for {}", event.verb_fqn.0);
}

#[test]
fn sexpr_roundtrip_property_edge_connect() {
    let subject = SubjectId(Uuid::new_v4());
    let edge = EdgeId(Uuid::new_v4());
    let event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.connect",
        analyst(),
        authority(),
        TargetBinding::for_edge(subject, edge),
        json!({
            "from_entity_id": Uuid::new_v4().to_string(),
            "to_entity_id": Uuid::new_v4().to_string(),
            "kind": "voting_rights",
            "percentage": 60.0,
        }),
        ts(),
    );
    assert_roundtrips(&event);
}

#[test]
fn sexpr_roundtrip_property_subject_place() {
    let subject = SubjectId(Uuid::new_v4());
    let entity = EntityId(Uuid::new_v4());
    let event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.place",
        analyst(),
        authority(),
        TargetBinding {
            entity_id: Some(entity),
            ..Default::default()
        },
        json!({"entity_id": entity.0.to_string(), "entity_type": "private_limited_company"}),
        ts(),
    );
    assert_roundtrips(&event);
}

// This exercises the `TargetBinding { person_id, obligation_id }` field pair
// round-tripping through the s-expr renderer/parser — a shape no live verb's
// real contract populates (the obligation-track verbs are subject-scoped,
// per `canonical.rs`), but `render_intent_event_to_sexpr` is generic over
// whatever `IntentEvent` it is handed, so the property holds independent of
// which verb produced the shape. FQN is a live label only, not a claim about
// what `kyc_ubo.assert.entity.identity` actually targets in production.
#[test]
fn sexpr_roundtrip_property_obligation_person_bound() {
    let subject = SubjectId(Uuid::new_v4());
    let person = PersonId(Uuid::new_v4());
    let obligation = ObligationId(Uuid::new_v4());
    let event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.entity.identity",
        analyst(),
        authority(),
        TargetBinding {
            subject_root: Some(subject),
            person_id: Some(person),
            obligation_id: Some(obligation),
            ..Default::default()
        },
        json!({}),
        ts(),
    );
    assert_roundtrips(&event);
}

#[test]
fn sexpr_roundtrip_property_string_escaping() {
    let subject = SubjectId(Uuid::new_v4());
    let event = IntentEvent::new(
        subject,
        "kyc_ubo.decide.obligation.waiver",
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        json!({"reason": "quotes \" and a backslash \\ and a newline\nhere"}),
        ts(),
    );
    assert_roundtrips(&event);
}

#[test]
fn sexpr_roundtrip_deterministic_same_event_same_text() {
    let subject = SubjectId(Uuid::new_v4());
    let event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.economic-interest",
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        json!({"from_entity_id": "a", "to_entity_id": "b", "percentage": 25.5}),
        ts(),
    );
    let r1 = render_intent_event_to_sexpr(&event, None);
    let r2 = render_intent_event_to_sexpr(&event, None);
    assert_eq!(r1, r2, "rendering must be deterministic (K-16/K-33)");
}

// Sanity check that the lexer accepts the kind of tokens we emit at all,
// independent of the substrate — guards against a future lexer-grammar
// change silently breaking round-trip without a substrate-level failure.
#[test]
fn lexer_accepts_underscore_keywords() {
    let toks: Vec<Token> = dsl_parser::lex(":from_entity_id")
        .into_iter()
        .filter_map(|t| t.ok())
        .collect();
    assert_eq!(toks, vec![Token::Keyword("from_entity_id".to_owned())]);
}

// ── T1 gate test — history_replayable_as_dsl ─────────────────────────────────
//
// Isolates exactly the property the renderer owns: reconstructing an event's
// `target`+`payload` from parse(render(event)) must fold to bit-identical
// `ControlState` as the original. Everything else on the event (id, seq,
// lexicon_hash, actor, authority, as_of) is carried forward unchanged from
// the original, matching how a real replay rehydrates full stored rows and
// only re-derives the domain fields from source text — the renderer never
// claimed to own actor/authority/as_of identity (those are supplied by the
// executing context, not encoded in the DSL call, exactly as
// `IntentEventDraft` already splits "domain bits" from "identity" in
// `ob-poc-kyc-seam`).

use std::sync::Arc;

use ob_poc_kyc_substrate::{
    fold_control_versioned, assembly_lexicon, FoldRegistry, V1FoldImpl,
};

fn registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

/// Invert `render_intent_event_to_sexpr`'s target/payload merge: known
/// canonical target-arg names peel off into `TargetBinding`; everything else
/// becomes the payload object.
fn reconstruct_target_and_payload(atom: &dsl_parser::RawAtom) -> (TargetBinding, serde_json::Value) {
    let mut target = TargetBinding::default();
    let mut payload = serde_json::Map::new();

    for (name, value) in &atom.slots {
        let json_value = raw_value_to_json(value);
        match name.as_str() {
            "subject-id" => {
                target.subject_root = Some(SubjectId(
                    Uuid::parse_str(json_value.as_str().unwrap()).unwrap(),
                ));
            }
            "edge-id" => {
                target.edge_id = Some(EdgeId(
                    Uuid::parse_str(json_value.as_str().unwrap()).unwrap(),
                ));
            }
            "entity-id" => {
                target.entity_id = Some(EntityId(
                    Uuid::parse_str(json_value.as_str().unwrap()).unwrap(),
                ));
            }
            "person-id" => {
                target.person_id = Some(PersonId(
                    Uuid::parse_str(json_value.as_str().unwrap()).unwrap(),
                ));
            }
            "obligation-id" => {
                target.obligation_id = Some(ObligationId(
                    Uuid::parse_str(json_value.as_str().unwrap()).unwrap(),
                ));
            }
            _ => {
                payload.insert(name.clone(), json_value);
            }
        }
    }

    (target, serde_json::Value::Object(payload))
}

#[test]
fn history_replayable_as_dsl() {
    let subject = SubjectId(Uuid::new_v4());
    let entity_a = EntityId(Uuid::new_v4());
    let to = Uuid::new_v4();
    let edge1 = EdgeId(Uuid::new_v4());

    let e1 = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.place",
        analyst(),
        authority(),
        TargetBinding {
            entity_id: Some(entity_a),
            ..Default::default()
        },
        json!({"entity_id": entity_a.0.to_string(), "entity_type": "private_limited_company"}),
        ts(),
    )
    .with_seq(0)
    .with_lexicon_hash(assembly_lexicon().hash);

    let e2 = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.connect",
        analyst(),
        authority(),
        TargetBinding::for_edge(subject, edge1),
        json!({
            "from_entity_id": Uuid::new_v4().to_string(),
            "to_entity_id": to.to_string(),
            "kind": "voting_rights",
            "percentage": 60.0,
        }),
        ts(),
    )
    .with_seq(1)
    .with_lexicon_hash(assembly_lexicon().hash);

    let original = [e1, e2];
    let original_refs: Vec<&IntentEvent> = original.iter().collect();
    let original_state = fold_control_versioned(&original_refs, &registry()).unwrap();

    // Replay: for each original event, render -> parse -> reconstruct target
    // + payload, keeping every other field (id, seq, lexicon_hash, actor,
    // authority, as_of) exactly as the original had it.
    let replayed: Vec<IntentEvent> = original
        .iter()
        .map(|ev| {
            let source = render_intent_event_to_sexpr(ev, None);
            let atom = parse_one_atom(&source);
            let (target, payload) = reconstruct_target_and_payload(&atom);
            let mut fresh = IntentEvent::new(
                ev.subject_root,
                atom.kind.as_str(),
                ev.actor.clone(),
                ev.authority.clone(),
                target,
                payload,
                ev.as_of,
            )
            .with_seq(ev.seq)
            .with_lexicon_hash(ev.lexicon_hash);
            fresh.id = ev.id;
            fresh
        })
        .collect();
    let replayed_refs: Vec<&IntentEvent> = replayed.iter().collect();
    let replayed_state = fold_control_versioned(&replayed_refs, &registry()).unwrap();

    assert_eq!(
        format!("{original_state:?}"),
        format!("{replayed_state:?}"),
        "re-executing rendered DSL source must reproduce the folded state bit-identically"
    );
}

/// Regression pin for the nested-null render defect (formerly a fenced red at
/// render.rs:102): control-basis strategies emit `ProngCandidate`s with
/// `pct: null`, which reach `render_value` *inside* the candidates array —
/// below the call site's top-level null filter. Nulls must be omitted at every
/// depth (the DSL grammar has no null literal), and the result must still be a
/// real, re-parseable statement.
#[test]
fn sexpr_render_omits_nested_nulls_at_every_depth() {
    let subject = SubjectId(Uuid::new_v4());
    let person = Uuid::new_v4().to_string();
    let event = IntentEvent::new(
        subject,
        "kyc_ubo.decide.determination.freeze",
        analyst(),
        authority(),
        TargetBinding::for_subject(subject),
        json!({
            "strategy": "control_prong_strategy",
            "candidates": [
                { "person_id": person, "pct": null, "prong": "control_by_other_means" },
                null,
            ],
            "basis": { "axis": "control", "quantum": null },
        }),
        ts(),
    );

    let rendered = render_intent_event_to_sexpr(&event, None);
    assert!(!rendered.contains("pct"), "null entry must be omitted: {rendered}");
    assert!(!rendered.contains("quantum"), "nested null entry must be omitted: {rendered}");

    let atom = parse_one_atom(&rendered);
    assert_eq!(atom.kind, "kyc_ubo.decide.determination.freeze");
    let candidates = atom
        .slots
        .iter()
        .find(|(name, _)| name == "candidates")
        .map(|(_, v)| raw_value_to_json(v))
        .expect("candidates slot survives the round trip");
    assert_eq!(
        candidates,
        json!([{ "person_id": person, "prong": "control_by_other_means" }]),
        "null array element skipped, null object entry dropped, rest intact"
    );
}
