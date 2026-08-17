//! Translates a validated `StateMachine` into bpmn-lite DSL source text.
//!
//! This module is pure and synchronous: no I/O, no calls into
//! `dsl-bpmn-frontend`. `compile.rs` is the only caller, and it runs the
//! emitted text through `dsl_migrate_verify::compile_to_spec` afterward.

use std::collections::{BTreeMap, BTreeSet};

use dsl_types::{StateDef, StateMachine, TransitionDef};
use serde_yaml::Value as YamlValue;

use crate::error::ShapeError;

const ANY_NON_TERMINAL: &str = "(any non-terminal)";

/// One resolved `(to, via)` transition after `from`-expansion, with its
/// merged set of source states (merged because two authored
/// `TransitionDef`s can name the same `(to, via)` pair with different
/// `from` values — that's still one logical edge target).
struct ResolvedTransition {
    to: String,
    verb: String,
    from_states: BTreeSet<String>,
}

/// Emit bpmn-lite DSL source for `state_machine`, or a [`ShapeError`]
/// naming exactly which authored shape this compiler does not translate.
pub(crate) fn emit_dsl_source(
    slot_id: &str,
    state_machine: &StateMachine,
) -> Result<String, ShapeError> {
    let entry_state = single_entry_state(slot_id, &state_machine.states)?;
    let state_ids: BTreeSet<String> = state_machine.states.iter().map(|s| s.id.clone()).collect();
    let terminal_states: BTreeSet<String> =
        state_machine.terminal_states.iter().cloned().collect();

    let resolved = resolve_transitions(
        slot_id,
        &state_machine.transitions,
        &state_ids,
        &terminal_states,
    )?;

    // Keyed by (to, verb) — deterministic iteration order via BTreeMap.
    let mut by_key: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for t in resolved {
        by_key
            .entry((t.to, t.verb))
            .or_default()
            .extend(t.from_states);
    }

    let task_id = |to: &str, verb: &str| format!("t__{}__{}", sanitize(to), sanitize(verb));
    let gateway_id = |state: &str| format!("gw__{}", sanitize(state));

    // producers_by_state[X] = node ids that, once reached, represent "we are
    // now in state X" — the entry state's start-event, plus the task node
    // of every transition whose `to == X`.
    let mut producers_by_state: BTreeMap<String, Vec<String>> = BTreeMap::new();
    producers_by_state
        .entry(entry_state.id.clone())
        .or_default()
        .push(entry_state.id.clone());
    // outgoing_by_state[X] = distinct (to, verb) pairs reachable from X.
    let mut outgoing_by_state: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();

    for ((to, verb), from_states) in &by_key {
        producers_by_state
            .entry(to.clone())
            .or_default()
            .push(task_id(to, verb));
        for from in from_states {
            outgoing_by_state
                .entry(from.clone())
                .or_default()
                .insert((to.clone(), verb.clone()));
        }
    }

    let mut lines = Vec::new();

    lines.push(format!("(node {} :kind start-event)", entry_state.id));
    for state in &terminal_states {
        lines.push(format!("(node {} :kind end-event)", state));
    }
    for (to, verb) in by_key.keys() {
        lines.push(format!(
            "(node {} :kind service-task :verb (invoke {}))",
            task_id(to, verb),
            verb
        ));
    }

    // Gateways for real fan-out (a state with >1 distinct outgoing target).
    for (state, targets) in &outgoing_by_state {
        if targets.len() > 1 {
            lines.push(format!("(gateway {} :kind exclusive)", gateway_id(state)));
        }
    }

    // Edges: producers -> (gateway | single task), then gateway -> each task.
    for (state, targets) in &outgoing_by_state {
        let producers = producers_by_state
            .get(state)
            .cloned()
            .unwrap_or_default();
        if targets.len() > 1 {
            let gw = gateway_id(state);
            for p in &producers {
                lines.push(format!("(flow {} -> {})", p, gw));
            }
            for (i, (to, verb)) in targets.iter().enumerate() {
                let tid = task_id(to, verb);
                if i == 0 {
                    lines.push(format!("(flow {} -> {} :default true)", gw, tid));
                } else {
                    lines.push(format!("(flow {} -> {} :condition \"{}\")", gw, tid, verb));
                }
            }
        } else if let Some((to, verb)) = targets.iter().next() {
            let tid = task_id(to, verb);
            for p in &producers {
                lines.push(format!("(flow {} -> {})", p, tid));
            }
        }
    }

    // Terminal arrival: each task landing on a terminal state flows into
    // that state's bare end-event node.
    for (to, verb) in by_key.keys() {
        if terminal_states.contains(to) {
            lines.push(format!("(flow {} -> {})", task_id(to, verb), to));
        }
    }

    Ok(lines.join("\n"))
}

fn single_entry_state<'a>(
    slot_id: &str,
    states: &'a [StateDef],
) -> Result<&'a StateDef, ShapeError> {
    let entries: Vec<&StateDef> = states.iter().filter(|s| s.entry).collect();
    match entries.len() {
        0 => Err(ShapeError::NoEntryState {
            slot_id: slot_id.to_string(),
        }),
        1 => Ok(entries[0]),
        n => Err(ShapeError::MultipleEntryStates {
            slot_id: slot_id.to_string(),
            count: n,
            states: entries
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

fn resolve_transitions(
    slot_id: &str,
    transitions: &[TransitionDef],
    state_ids: &BTreeSet<String>,
    terminal_states: &BTreeSet<String>,
) -> Result<Vec<ResolvedTransition>, ShapeError> {
    let mut out = Vec::with_capacity(transitions.len());
    for t in transitions {
        let verb = resolve_verb(slot_id, t)?;
        if !state_ids.contains(&t.to) {
            return Err(ShapeError::TransitionToUnknownState {
                slot_id: slot_id.to_string(),
                via: verb,
                to: t.to.clone(),
            });
        }
        let from_states = resolve_from(slot_id, t, &verb, state_ids, terminal_states)?;
        out.push(ResolvedTransition {
            to: t.to.clone(),
            verb,
            from_states,
        });
    }
    Ok(out)
}

fn resolve_verb(slot_id: &str, t: &TransitionDef) -> Result<String, ShapeError> {
    let from_label = yaml_display(&t.from);
    match &t.via {
        None => Err(ShapeError::TransitionViaMissing {
            slot_id: slot_id.to_string(),
            from: from_label,
            to: t.to.clone(),
        }),
        Some(YamlValue::Sequence(seq)) => Err(ShapeError::TransitionViaIsList {
            slot_id: slot_id.to_string(),
            from: from_label,
            to: t.to.clone(),
            verbs: seq.iter().map(yaml_display).collect::<Vec<_>>().join(", "),
        }),
        Some(YamlValue::String(s)) => {
            if is_verb_fqn(s) {
                Ok(s.clone())
            } else {
                Err(ShapeError::TransitionViaNotAVerb {
                    slot_id: slot_id.to_string(),
                    from: from_label,
                    to: t.to.clone(),
                    raw: s.clone(),
                })
            }
        }
        Some(other) => Err(ShapeError::TransitionViaNotAVerb {
            slot_id: slot_id.to_string(),
            from: from_label,
            to: t.to.clone(),
            raw: yaml_display(other),
        }),
    }
}

fn resolve_from(
    slot_id: &str,
    t: &TransitionDef,
    verb: &str,
    state_ids: &BTreeSet<String>,
    terminal_states: &BTreeSet<String>,
) -> Result<BTreeSet<String>, ShapeError> {
    let unresolvable = || ShapeError::TransitionFromUnresolvable {
        slot_id: slot_id.to_string(),
        from_raw: yaml_display(&t.from),
        to: t.to.clone(),
        via: verb.to_string(),
    };

    match &t.from {
        YamlValue::String(s) if s == ANY_NON_TERMINAL => Ok(state_ids
            .difference(terminal_states)
            .cloned()
            .collect()),
        YamlValue::String(s) if state_ids.contains(s) => {
            let mut set = BTreeSet::new();
            set.insert(s.clone());
            Ok(set)
        }
        YamlValue::Sequence(seq) => {
            let mut set = BTreeSet::new();
            for v in seq {
                match v {
                    YamlValue::String(s) if state_ids.contains(s) => {
                        set.insert(s.clone());
                    }
                    _ => return Err(unresolvable()),
                }
            }
            if set.is_empty() {
                Err(unresolvable())
            } else {
                Ok(set)
            }
        }
        _ => Err(unresolvable()),
    }
}

/// Heuristic: a verb FQN is dotted (`domain.verb` or `domain.sub.verb`) and
/// contains no whitespace or parentheses. Free-text `via` annotations
/// observed in `kyc_dag.yaml` are parenthesized prose
/// (`"(backend: entity lookup / GLEIF import)"`) or hyphen-only with no dot
/// (`"(time-decay)"`, also parenthesized) — both rejected by this check.
fn is_verb_fqn(s: &str) -> bool {
    !s.is_empty()
        && s.contains('.')
        && !s.contains(' ')
        && !s.contains('(')
        && !s.contains(')')
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c == '.' { '_' } else { c })
        .collect()
}

fn yaml_display(v: &YamlValue) -> String {
    match v {
        YamlValue::String(s) => s.clone(),
        other => serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}
