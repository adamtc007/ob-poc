//! Translates a validated `StateMachine` into bpmn-lite DSL source text.
//!
//! This module is pure and synchronous: no I/O, no calls into
//! `dsl-bpmn-frontend`. `compile.rs` is the only caller, and it runs the
//! emitted text through `dsl_migrate_verify::compile_to_spec` afterward.

use std::collections::{BTreeMap, BTreeSet};

use dsl_types::{
    AggregateReduce, AwaitOutcome, AwaitSource, Dag, EntryVia, SlotStateMachine, StateDef,
    StateMachine, TransitionDef,
};
use serde_yaml::Value as YamlValue;

use crate::error::ShapeError;

const ANY_NON_TERMINAL: &str = "(any non-terminal)";

/// One resolved `(to, via)` transition after `from`-expansion, with its
/// merged set of source states (merged because two authored
/// `TransitionDef`s can name the same `(to, via)` pair with different
/// `from` values — that's still one logical edge target) and the
/// precondition text to use as this edge's BPMN condition, if any.
struct ResolvedTransition {
    to: String,
    verb: String,
    from_states: BTreeSet<String>,
    /// Same as `from_states`, except empty when this transition's `from`
    /// was the `(any non-terminal)` wildcard. A wildcard-sourced edge is a
    /// real flow (included in `from_states`, still emitted) but does not
    /// count toward the `awaits`/outgoing-transitions mutual exclusivity
    /// check — see `StateDef.awaits`'s doc comment.
    explicit_from_states: BTreeSet<String>,
    precondition: Option<String>,
}

/// `entry_via` node kind + trigger label for one non-entry state whose only
/// path in is external (not a callable verb). `Verb` is excluded — that
/// variant means "reached by a real transition", which the transitions
/// list already models.
fn entry_via_catch(entry_via: &EntryVia) -> Option<(&'static str, String)> {
    match entry_via {
        EntryVia::Verb => None,
        EntryVia::Signal { source } => Some(("signal", source.clone())),
        EntryVia::Trigger { name } => Some(("message", name.clone())),
        EntryVia::Scheduler { name } => Some(("timer", name.clone())),
        // Cascade's `parent` is a real verb executing elsewhere (a
        // different slot/process), not a raw external signal — modeled as
        // a signal catch since that's the closest of the four available
        // intermediate-catch kinds to "something else in the system
        // notified us", without claiming a message-queue semantics this
        // DAG data doesn't actually assert.
        EntryVia::Cascade { parent } => Some(("signal", parent.clone())),
    }
}

/// Emit bpmn-lite DSL source for `state_machine`, or a [`ShapeError`]
/// naming exactly which authored shape this compiler does not translate.
pub(crate) fn emit_dsl_source(
    slot_id: &str,
    dag: &Dag,
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
    // The condition text is the first non-empty `precondition` found for
    // this (to, verb) pair across its merged `from` rows, if any.
    let mut by_key: BTreeMap<(String, String), (BTreeSet<String>, Option<String>)> =
        BTreeMap::new();
    // Mirrors `by_key`'s `from_states` but excludes wildcard-sourced states
    // — used only for the `awaits` mutual-exclusivity check below, never
    // for flow emission (a wildcard-sourced edge still needs its flow).
    let mut explicit_by_key: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for t in resolved {
        let key = (t.to.clone(), t.verb.clone());
        explicit_by_key
            .entry(key.clone())
            .or_default()
            .extend(t.explicit_from_states);
        let entry = by_key.entry(key).or_default();
        entry.0.extend(t.from_states);
        if entry.1.is_none() {
            entry.1 = t.precondition;
        }
    }

    let task_id = |to: &str, verb: &str| format!("t__{}__{}", sanitize(to), sanitize(verb));
    let gateway_id = |state: &str| format!("gw__{}", sanitize(state));
    let catch_id = |state: &str| format!("ic__{}", sanitize(state));

    // producers_by_state[X] = node ids that, once reached, represent "we are
    // now in state X" — the entry state's start-event, the task node of
    // every transition whose `to == X`, and (below) any entry_via catch
    // node for X.
    let mut producers_by_state: BTreeMap<String, Vec<String>> = BTreeMap::new();
    producers_by_state
        .entry(entry_state.id.clone())
        .or_default()
        .push(entry_state.id.clone());
    // outgoing_by_state[X] = distinct (to, verb) pairs reachable from X.
    let mut outgoing_by_state: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();

    for ((to, verb), (from_states, _precondition)) in &by_key {
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

    // explicit_outgoing_by_state[X] = same, but excludes wildcard-sourced
    // edges — the set `emit_awaits`'s mutual-exclusivity check consults.
    let mut explicit_outgoing_by_state: BTreeMap<String, BTreeSet<(String, String)>> =
        BTreeMap::new();
    for ((to, verb), from_states) in &explicit_by_key {
        for from in from_states {
            explicit_outgoing_by_state
                .entry(from.clone())
                .or_default()
                .insert((to.clone(), verb.clone()));
        }
    }

    // entry_via states: a non-entry state reached by something other than
    // a callable verb (Trigger/Scheduler/Signal/Cascade — Verb is excluded,
    // that's the transitions list's job). Each gets a standalone
    // intermediate-catch node fed directly off the process's real entry
    // state (this compiler resolves exactly one start node — see
    // dsl-bpmn-frontend's `find_start_node` — so multiple genuine start
    // events aren't an option; wiring the catch node in right after entry
    // is the closest reachable approximation available today) and is
    // registered as an extra producer for its state, same as a real task.
    let mut catch_nodes: Vec<(String, &'static str, String)> = Vec::new(); // (node_id, kind, state)
    for state in &state_machine.states {
        if state.id == entry_state.id {
            continue;
        }
        let Some(via) = &state.entry_via else { continue };
        let Some((kind, label)) = entry_via_catch(via) else {
            continue;
        };
        let node_id = catch_id(&state.id);
        catch_nodes.push((node_id.clone(), kind, label));
        producers_by_state
            .entry(state.id.clone())
            .or_default()
            .push(node_id);
    }

    let awaits_lines = emit_awaits(
        slot_id,
        dag,
        state_machine,
        &state_ids,
        &terminal_states,
        &explicit_outgoing_by_state,
        &mut producers_by_state,
    )?;

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
    for (node_id, kind, label) in &catch_nodes {
        lines.push(format!(
            "(node {} :kind intermediate-catch-{}) ; {}",
            node_id, kind, label
        ));
        lines.push(format!("(flow {} -> {})", entry_state.id, node_id));
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
                let condition = by_key
                    .get(&(to.clone(), verb.clone()))
                    .and_then(|(_, precondition)| precondition.as_deref())
                    .unwrap_or(verb);
                if i == 0 {
                    lines.push(format!("(flow {} -> {} :default true)", gw, tid));
                } else {
                    lines.push(format!(
                        "(flow {} -> {} :condition \"{}\")",
                        gw, tid, condition
                    ));
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
    // Same for a terminal state whose only path in is an entry_via catch
    // node (e.g. a state that times out straight to a terminal status,
    // with no real verb ever driving it).
    for (node_id, _kind, _label) in &catch_nodes {
        // catch_nodes and producers_by_state were built from the same
        // states iteration, so recovering the state id from the node id's
        // suffix isn't needed — instead, check directly which terminal
        // states have this node registered as a producer.
        for state in &terminal_states {
            if producers_by_state
                .get(state)
                .is_some_and(|producers| producers.contains(node_id))
            {
                lines.push(format!("(flow {} -> {})", node_id, state));
            }
        }
    }

    lines.extend(awaits_lines);

    Ok(lines.join("\n"))
}

/// Emit the call-out + switch move for every `awaits`-bearing state:
///
/// ```text
/// (node awt_src__S :kind service-task :verb (invoke V))      ; VerbSwitch
///   -- or --
/// (node awt_src__S :kind intermediate-catch-signal) ; awaits <slot>
///
/// (flow <producer-of-S> -> awt_src__S)
///
/// ; single arm: no gateway needed
/// (flow awt_src__S -> <to>)
///
/// ; N>1 arms: exclusive gateway, first arm default, rest conditioned
/// (gateway awt_gw__S :kind exclusive)
/// (flow awt_src__S -> awt_gw__S)
/// (flow awt_gw__S -> <to> :default true)              ; terminal to
/// (node awt_arr__S__outcome :kind intermediate-catch-signal)  ; non-terminal to
/// (flow awt_gw__S -> awt_arr__S__outcome :condition "outcome")
/// ```
///
/// A non-terminal `to` gets a small marker node registered as a producer for
/// `to`, rather than registering the gateway itself — a single gateway node
/// can have arms landing on *different* destination states, and reusing it
/// directly as a shared producer would over-connect the graph (every arm's
/// target would spuriously see every other arm's downstream edges too).
/// Terminal targets skip the marker: their bare end-event node already
/// exists and never has outgoing edges, so a direct flow is safe.
#[allow(clippy::too_many_arguments)]
fn emit_awaits(
    slot_id: &str,
    dag: &Dag,
    state_machine: &StateMachine,
    state_ids: &BTreeSet<String>,
    terminal_states: &BTreeSet<String>,
    explicit_outgoing_by_state: &BTreeMap<String, BTreeSet<(String, String)>>,
    producers_by_state: &mut BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>, ShapeError> {
    let mut lines = Vec::new();

    for state in &state_machine.states {
        let Some(awaits) = &state.awaits else {
            continue;
        };

        // Only a transition whose `from` explicitly names this state
        // conflicts with `awaits` — a wildcard-sourced "(any non-terminal)"
        // edge (operator interrupt/override moves) is allowed to coexist,
        // see `StateDef.awaits`'s doc comment.
        if explicit_outgoing_by_state.contains_key(&state.id) {
            return Err(ShapeError::AwaitsAndTransitionsBothPresent {
                slot_id: slot_id.to_string(),
                state_id: state.id.clone(),
            });
        }

        validate_await_cases(slot_id, &state.id, dag, state_ids, awaits)?;

        let source_id = format!("awt_src__{}", sanitize(&state.id));
        lines.push(await_source_node_line(&source_id, awaits));

        for producer in producers_by_state
            .get(&state.id)
            .cloned()
            .unwrap_or_default()
        {
            lines.push(format!("(flow {} -> {})", producer, source_id));
        }

        if awaits.cases.len() == 1 {
            let case = &awaits.cases[0];
            if terminal_states.contains(&case.to) {
                lines.push(format!("(flow {} -> {})", source_id, case.to));
            } else {
                producers_by_state
                    .entry(case.to.clone())
                    .or_default()
                    .push(source_id.clone());
            }
            continue;
        }

        let gateway_id = format!("awt_gw__{}", sanitize(&state.id));
        lines.push(format!("(gateway {} :kind exclusive)", gateway_id));
        lines.push(format!("(flow {} -> {})", source_id, gateway_id));

        for (i, case) in awaits.cases.iter().enumerate() {
            let flow_suffix = if i == 0 {
                ":default true".to_string()
            } else {
                format!(":condition \"{}\"", case.outcome)
            };

            if terminal_states.contains(&case.to) {
                lines.push(format!(
                    "(flow {} -> {} {})",
                    gateway_id, case.to, flow_suffix
                ));
            } else {
                let arrival_id =
                    format!("awt_arr__{}__{}", sanitize(&state.id), sanitize(&case.outcome));
                lines.push(format!(
                    "(node {} :kind intermediate-catch-signal) ; awaits-arrival {} -> {}",
                    arrival_id, case.outcome, case.to
                ));
                lines.push(format!(
                    "(flow {} -> {} {})",
                    gateway_id, arrival_id, flow_suffix
                ));
                producers_by_state
                    .entry(case.to.clone())
                    .or_default()
                    .push(arrival_id);
            }
        }
    }

    Ok(lines)
}

fn await_source_node_line(source_id: &str, awaits: &AwaitOutcome) -> String {
    match &awaits.source {
        AwaitSource::VerbSwitch { verb } => format!(
            "(node {} :kind service-task :verb (invoke {}))",
            source_id, verb
        ),
        AwaitSource::SlotTerminalState { workspace, slot } => format!(
            "(node {} :kind intermediate-catch-signal) ; awaits {}{}",
            source_id,
            slot,
            workspace
                .as_ref()
                .map(|w| format!(" (workspace {w})"))
                .unwrap_or_default()
        ),
        AwaitSource::SlotTerminalStateAggregate {
            workspace,
            slot,
            reduce,
            ..
        } => {
            let reduce_label = match reduce {
                AggregateReduce::WorstOf => "worst_of",
                AggregateReduce::AllAgree => "all_agree",
            };
            format!(
                "(node {} :kind intermediate-catch-signal) ; awaits {} (aggregate: {}){}",
                source_id,
                slot,
                reduce_label,
                workspace
                    .as_ref()
                    .map(|w| format!(" (workspace {w})"))
                    .unwrap_or_default()
            )
        }
    }
}

/// Validate an `awaits` block's `cases` against this state machine (`to`
/// must be a known state id) and, where the source references another slot
/// in the *same* workspace, against that slot's own declared
/// `terminal_states` (each `outcome` must be one of them, and — the other
/// direction — every one of that slot's terminal states must have a
/// matching arm; an unhandled terminal outcome would leave the board with
/// no move to make if it ever occurs).
///
/// A `workspace` naming a *different* workspace than this `Dag` can't be
/// validated from a single `Dag` value — skipped, not rejected.
fn validate_await_cases(
    slot_id: &str,
    state_id: &str,
    dag: &Dag,
    state_ids: &BTreeSet<String>,
    awaits: &AwaitOutcome,
) -> Result<(), ShapeError> {
    for case in &awaits.cases {
        if !state_ids.contains(&case.to) {
            return Err(ShapeError::AwaitCaseTargetUnknown {
                slot_id: slot_id.to_string(),
                state_id: state_id.to_string(),
                outcome: case.outcome.clone(),
                to: case.to.clone(),
            });
        }
    }

    let (workspace, await_slot) = match &awaits.source {
        AwaitSource::VerbSwitch { .. } => return Ok(()),
        AwaitSource::SlotTerminalState { workspace, slot } => (workspace, slot),
        AwaitSource::SlotTerminalStateAggregate {
            workspace, slot, ..
        } => (workspace, slot),
    };

    if let Some(ws) = workspace {
        if ws != &dag.workspace {
            return Ok(());
        }
    }

    let target_slot = dag
        .slots
        .iter()
        .find(|s| &s.id == await_slot)
        .ok_or_else(|| ShapeError::AwaitTargetSlotNotFound {
            slot_id: slot_id.to_string(),
            state_id: state_id.to_string(),
            await_slot: await_slot.clone(),
        })?;

    let Some(SlotStateMachine::Structured(target_sm)) = &target_slot.state_machine else {
        return Ok(());
    };

    let target_terminal_states: BTreeSet<String> =
        target_sm.terminal_states.iter().cloned().collect();

    for case in &awaits.cases {
        if !target_terminal_states.contains(&case.outcome) {
            return Err(ShapeError::AwaitCaseNotATerminalState {
                slot_id: slot_id.to_string(),
                state_id: state_id.to_string(),
                await_slot: await_slot.clone(),
                outcome: case.outcome.clone(),
            });
        }
    }

    let declared_outcomes: BTreeSet<&String> =
        awaits.cases.iter().map(|c| &c.outcome).collect();
    let missing: Vec<&String> = target_terminal_states
        .iter()
        .filter(|ts| !declared_outcomes.contains(ts))
        .collect();
    if !missing.is_empty() {
        return Err(ShapeError::AwaitCasesNotExhaustive {
            slot_id: slot_id.to_string(),
            state_id: state_id.to_string(),
            await_slot: await_slot.clone(),
            missing: missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        });
    }

    Ok(())
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
        let is_wildcard = matches!(&t.from, YamlValue::String(s) if s == ANY_NON_TERMINAL);
        let explicit_from_states = if is_wildcard {
            BTreeSet::new()
        } else {
            from_states.clone()
        };
        out.push(ResolvedTransition {
            to: t.to.clone(),
            verb,
            from_states,
            explicit_from_states,
            precondition: t.precondition.clone(),
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
