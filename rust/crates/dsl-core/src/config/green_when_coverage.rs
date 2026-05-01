//! Coverage accounting for DAG `green_when` predicates.
//!
//! Phase 8 needs a machine-readable sweep over authored DAG taxonomies before
//! predicates are backfilled workspace by workspace. This module deliberately
//! reports coverage; it does not invent predicates.

use crate::config::dag::{Dag, SlotStateMachine, StateDef, TransitionDef};
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreenWhenCoverageRow {
    pub workspace: String,
    pub dag_id: String,
    pub slot_id: String,
    pub state_id: String,
    pub candidate: bool,
    pub has_green_when: bool,
    pub inbound_verbs: Vec<String>,
    pub exclusion_reason: Option<GreenWhenExclusionReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GreenWhenExclusionReason {
    EntryState,
    SourceOnlyState,
    DiscretionaryDestination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreenWhenCoverageSummary {
    pub total_states: usize,
    pub candidate_states: usize,
    pub covered_candidate_states: usize,
    pub missing_candidate_states: usize,
}

impl GreenWhenCoverageSummary {
    pub fn coverage_percent(&self) -> f64 {
        if self.candidate_states == 0 {
            100.0
        } else {
            (self.covered_candidate_states as f64 / self.candidate_states as f64) * 100.0
        }
    }
}

pub fn green_when_coverage_for_dags(
    dags: &BTreeMap<String, Dag>,
    discretionary_verbs: &HashSet<String>,
) -> Vec<GreenWhenCoverageRow> {
    let mut rows = Vec::new();
    for (workspace, dag) in dags {
        rows.extend(green_when_coverage_for_dag(
            workspace,
            dag,
            discretionary_verbs,
        ));
    }
    rows
}

pub fn green_when_coverage_for_dag(
    workspace: &str,
    dag: &Dag,
    discretionary_verbs: &HashSet<String>,
) -> Vec<GreenWhenCoverageRow> {
    let mut rows = Vec::new();
    for slot in &dag.slots {
        let Some(SlotStateMachine::Structured(machine)) = &slot.state_machine else {
            continue;
        };
        let inbound = inbound_verbs_by_destination(&machine.transitions);
        for state in &machine.states {
            rows.push(row_for_state(
                workspace,
                dag,
                &slot.id,
                state,
                inbound.get(&state.id).cloned().unwrap_or_default(),
                discretionary_verbs,
            ));
        }
    }
    rows.sort_by(|a, b| {
        (&a.workspace, &a.slot_id, &a.state_id).cmp(&(&b.workspace, &b.slot_id, &b.state_id))
    });
    rows
}

pub fn green_when_coverage_summary(rows: &[GreenWhenCoverageRow]) -> GreenWhenCoverageSummary {
    let total_states = rows.len();
    let candidate_states = rows.iter().filter(|row| row.candidate).count();
    let covered_candidate_states = rows
        .iter()
        .filter(|row| row.candidate && row.has_green_when)
        .count();
    GreenWhenCoverageSummary {
        total_states,
        candidate_states,
        covered_candidate_states,
        missing_candidate_states: candidate_states - covered_candidate_states,
    }
}

fn row_for_state(
    workspace: &str,
    dag: &Dag,
    slot_id: &str,
    state: &StateDef,
    inbound_verbs: Vec<String>,
    discretionary_verbs: &HashSet<String>,
) -> GreenWhenCoverageRow {
    let has_green_when = state
        .green_when
        .as_deref()
        .is_some_and(|predicate| !predicate.trim().is_empty());
    let exclusion_reason = if state.entry {
        Some(GreenWhenExclusionReason::EntryState)
    } else if inbound_verbs.is_empty() {
        Some(GreenWhenExclusionReason::SourceOnlyState)
    } else if !inbound_verbs.is_empty()
        && inbound_verbs
            .iter()
            .all(|verb| discretionary_verbs.contains(verb))
    {
        Some(GreenWhenExclusionReason::DiscretionaryDestination)
    } else {
        None
    };

    GreenWhenCoverageRow {
        workspace: workspace.to_string(),
        dag_id: dag.dag_id.clone(),
        slot_id: slot_id.to_string(),
        state_id: state.id.clone(),
        candidate: exclusion_reason.is_none(),
        has_green_when,
        inbound_verbs,
        exclusion_reason,
    }
}

fn inbound_verbs_by_destination(transitions: &[TransitionDef]) -> BTreeMap<String, Vec<String>> {
    let mut inbound: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for transition in transitions {
        let verbs = verbs_from_transition(transition);
        let entry = inbound.entry(transition.to.clone()).or_default();
        for verb in verbs {
            entry.insert(verb);
        }
    }
    inbound
        .into_iter()
        .map(|(state, verbs)| (state, verbs.into_iter().collect()))
        .collect()
}

fn verbs_from_transition(transition: &TransitionDef) -> Vec<String> {
    transition
        .via
        .as_ref()
        .map(states_from_yaml_value)
        .unwrap_or_default()
}

fn states_from_yaml_value(value: &YamlValue) -> Vec<String> {
    match value {
        YamlValue::String(s) => split_tupleish(s),
        YamlValue::Sequence(items) => items
            .iter()
            .flat_map(states_from_yaml_value)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    }
}

fn split_tupleish(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let inner = trimmed
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
