//! F3 target (EOP-FUZZ-KYCUBO-001 §4 P0): O1 (no-panic) + O3 (decode/encode
//! roundtrip) over `render_intent_event_to_sexpr` / `dsl_parser::parse`.
//!
//! Mirrors `tests/render.rs`'s `sexpr_roundtrip_property_*` fixtures (5
//! hand-picked examples today) but over generator-produced events whose
//! string fields (`kind`, `structure_class`, `reason`, garbage UUID
//! fields, …) can contain arbitrary bytes-as-chars — including control
//! characters `render_string_literal` doesn't explicitly special-case
//! (only `"`, `\`, and `\n` are escaped) — which a hand-written fixture
//! bank would never think to include.
#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::json;

use dsl_parser::RawValue;
use ob_poc_kyc_substrate::render_intent_event_to_sexpr;
use ob_poc_kyc_substrate_fuzz::{gen_events, Tape};

/// Mirrors `tests/render.rs::raw_value_to_json` — the only `RawValue`
/// shapes KYC payloads use. Any other shape reaching here would itself be
/// a round-trip defect (the source rendered a form `render_value` never
/// emits), so it maps to `Null` rather than panicking — the mismatch then
/// surfaces via the `assert_eq!` below instead of an unrelated panic.
fn raw_value_to_json(v: &RawValue) -> serde_json::Value {
    match v {
        RawValue::StringLit(s) => json!(s),
        RawValue::IntLit(n) => json!(n),
        RawValue::FloatLit(f) => json!(f),
        RawValue::BoolLit(b) => json!(b),
        RawValue::List(items) => {
            serde_json::Value::Array(items.iter().map(raw_value_to_json).collect())
        }
        RawValue::Map(entries) => serde_json::Value::Object(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), raw_value_to_json(v)))
                .collect(),
        ),
        _ => serde_json::Value::Null,
    }
}

fuzz_target!(|data: &[u8]| {
    let mut tape = Tape::new(data);
    let events = gen_events(&mut tape);
    let Some(event) = events.first() else {
        return;
    };
    // `render_intent_event_to_sexpr` now documents (debug_assert,
    // EOP-FUZZ-KYCUBO-001 §5 finding #1) that a non-empty verb_fqn is a
    // caller precondition, not something it must tolerate — every real op
    // stamps a hardcoded FQN literal, so this is genuinely unreachable in
    // production. `fold_replay`/`determination_resolve` still exercise the
    // empty-FQN case (ALL_VERBS) for total-dispatch fold coverage; only this
    // target's contract excludes it.
    if event.verb_fqn.0.is_empty() {
        return;
    }

    // O1: rendering itself must not panic.
    let rendered = render_intent_event_to_sexpr(event, None);

    // O1: re-parsing the rendered text must not panic, and must not error —
    // a rendered event that fails to re-parse is exactly KIT-1's defect.
    let (source_file, diagnostics) = dsl_parser::parse(&rendered);
    assert!(
        !diagnostics.has_errors(),
        "O3: rendered source failed to re-parse: {rendered:?}\ndiagnostics: {diagnostics:?}"
    );
    assert_eq!(
        source_file.atoms.len(),
        1,
        "O3: expected exactly one top-level atom, got {}: {rendered:?}",
        source_file.atoms.len()
    );
    let atom = source_file.atoms.into_iter().next().unwrap();

    assert_eq!(
        atom.kind, event.verb_fqn.0,
        "O3: verb FQN did not round-trip: {rendered:?}"
    );

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

    assert_eq!(
        actual, expected,
        "O3: round-trip mismatch for {} — rendered: {rendered:?}",
        event.verb_fqn.0
    );
});
