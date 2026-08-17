//! Renders a governed `IntentEvent` back into DSL S-expression source text
//! (T1, `EOP-PLAN-KYCUBO-KIT-001` — closes KIT-1). Pure: no I/O, no clock,
//! no randomness — the same event always renders to the same text.
//!
//! The payload's JSON keys ARE the verb's DSL argument names verbatim: the
//! executing op passes the parsed verb args straight through as the event
//! payload (see `domain_ops::kyc_stream_ops`'s `stream_append` and its inline
//! call sites, which do `payload: args.clone()`) — so there is no separate
//! arg-name schema to consult here. `LexiconEntry` governs taxonomy/folds/
//! preconditions/authority, not argument shape (it has no args field);
//! `lexicon_entry` is `Option` because a caller may render an event whose
//! verb predates full lexicon coverage (historical/backfilled events) —
//! T0.3 gap K-G6 closed 2026-08-12 (T6.0), so every currently-registered
//! dsl.kyc verb has an entry today, but the signature stays optional for
//! that historical case. Note: `lexicon_entry`, when present, is only
//! used for a debug-only fqn-match assertion below — it does not change
//! the rendered text (rendering is driven entirely by `event.target`/
//! `event.payload`), so entry presence is a coverage/precondition/manifest
//! concern, not a rendered-output concern.

use serde_json::Value;

use crate::event::IntentEvent;
use crate::lexicon::LexiconEntry;

/// Render `event` as re-parseable DSL source: `(verb.fqn :k v :k2 v2 ...)`.
///
/// Slot order is canonical (sorted by name) so the same logical event always
/// renders to bit-identical text (K-16/K-33 replay-determinism discipline).
/// `lexicon_entry`, when present, is asserted (debug-only) to govern this
/// event's verb; it is not otherwise consulted.
pub fn render_intent_event_to_sexpr(
    event: &IntentEvent,
    lexicon_entry: Option<&LexiconEntry>,
) -> String {
    if let Some(entry) = lexicon_entry {
        debug_assert_eq!(
            entry.fqn.0, event.verb_fqn.0,
            "render_intent_event_to_sexpr: lexicon_entry does not govern this event's verb"
        );
    }
    // EOP-FUZZ-KYCUBO-001 §5 finding #1: an empty verb_fqn renders to a bare
    // `(` with no atom-kind symbol, which the real DSL parser correctly
    // rejects — silently violating this function's own KIT-1 guarantee ("a
    // real, re-parseable DSL statement"). Every real op stamps `verb_fqn`
    // from a hardcoded string literal (`stream_append`'s first arg), so this
    // is unreachable via any live write path — a debug-only assert, matching
    // the lexicon_entry check above, rather than a `Result` return that
    // would ripple through every production caller for a case that can't
    // actually occur there.
    debug_assert!(
        !event.verb_fqn.0.is_empty(),
        "render_intent_event_to_sexpr: empty verb_fqn cannot render to valid re-parseable DSL"
    );

    let mut slots: Vec<(String, Value)> = Vec::new();

    // Target-binding fields render under their canonical DSL arg names first;
    // a payload entry of the same name (some ops also echo it into payload)
    // overrides rather than duplicates.
    if let Some(subject_root) = &event.target.subject_root {
        slots.push(("subject-id".to_string(), Value::String(subject_root.0.to_string())));
    }
    if let Some(edge_id) = &event.target.edge_id {
        slots.push(("edge-id".to_string(), Value::String(edge_id.0.to_string())));
    }
    if let Some(entity_id) = &event.target.entity_id {
        slots.push(("entity-id".to_string(), Value::String(entity_id.0.to_string())));
    }
    if let Some(person_id) = &event.target.person_id {
        slots.push(("person-id".to_string(), Value::String(person_id.0.to_string())));
    }
    if let Some(obligation_id) = &event.target.obligation_id {
        slots.push(("obligation-id".to_string(), Value::String(obligation_id.0.to_string())));
    }

    if let Value::Object(map) = &event.payload {
        for (k, v) in map {
            if v.is_null() {
                continue; // absent optional arg — omit rather than emit an unparseable null
            }
            if let Some(existing) = slots.iter_mut().find(|(name, _)| name == k) {
                existing.1 = v.clone();
            } else {
                slots.push((k.clone(), v.clone()));
            }
        }
    }

    slots.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = format!("({}", event.verb_fqn.0);
    for (name, value) in &slots {
        out.push(' ');
        out.push(':');
        out.push_str(name);
        out.push(' ');
        out.push_str(&render_value(value));
    }
    out.push(')');
    out
}

/// Render one payload value as a DSL literal. Verified against `dsl-parser`'s
/// lexer grammar (`crates/dsl-parser/src/lexer.rs`): string/int/float/bool
/// literals and `[...]`/`{:k v}` list/map forms are real tokens; there is no
/// null/nil literal. Nulls are therefore *omitted* at every depth, mirroring
/// the top-level absent-optional-arg rule: a null object entry is dropped
/// (e.g. a `ProngCandidate` with `pct: null` from a control-basis strategy),
/// and a null array element is skipped. Only a direct top-level `Value::Null`
/// argument is a caller bug (the call site above filters those).
fn render_value(v: &Value) -> String {
    match v {
        Value::String(s) => render_string_literal(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => {
            debug_assert!(false, "render_value: unexpected null (should be filtered upstream)");
            String::new()
        }
        Value::Array(items) => {
            let rendered: Vec<String> = items
                .iter()
                .filter(|item| !item.is_null())
                .map(render_value)
                .collect();
            format!("[{}]", rendered.join(" "))
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> =
                map.iter().filter(|(_, v)| !v.is_null()).collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let rendered: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!(":{} {}", k, render_value(v)))
                .collect();
            format!("{{{}}}", rendered.join(" "))
        }
    }
}

/// String-literal escaping matching `dsl-parser`'s `lex_string` inverse:
/// `\\` -> `\`, `\"` -> `"`, `\n` -> newline (so this must produce the mirror
/// image on the way out).
fn render_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}
