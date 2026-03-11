//! Three-step utterance pipeline replacement.
//!
//! This module is the rip-and-replace boundary for utterance understanding.
//! It owns exactly three semantic steps:
//! 1. entity scope
//! 2. entity state
//! 3. verb selection
//!
//! Execution, governance, and discovery I/O remain elsewhere.

use crate::semtaxonomy::{extract_entity_candidates, EntityCandidate};
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Source of the active entity scope.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntitySource {
    SessionCarry,
    SearchHit,
    UserConfirmed,
}

/// Scoped entity selected by Step 1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityScope {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub name: String,
    pub confidence: f64,
    pub source: EntitySource,
}

/// Outcome of entity-scope resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityScopeOutcome {
    Resolved(EntityScope),
    Ambiguous(Vec<EntityScope>),
    Unresolved,
}

/// Current position within a business lane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanePosition {
    pub lane: String,
    pub current_phase: String,
    pub is_active: bool,
    pub is_terminal: bool,
}

/// Valid business verb in the current entity state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidVerb {
    pub verb_id: String,
    pub description: String,
    pub polarity: String,
    pub invocation_phrases: Vec<String>,
    pub parameters: Vec<ValidVerbParameter>,
    pub lane: String,
    pub phase: String,
    pub relevance: f64,
}

/// Parameter contract for a valid verb.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidVerbParameter {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
}

/// Blocked verb with unmet preconditions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockedVerb {
    pub verb_id: String,
    pub description: String,
    pub unmet_preconditions: Vec<String>,
    pub unblocking_actions: Vec<String>,
}

/// Step 2 output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityState {
    pub entity: EntityScope,
    pub lane_positions: Vec<LanePosition>,
    pub valid_verbs: Vec<ValidVerb>,
    pub blocked_verbs: Vec<BlockedVerb>,
}

/// Missing argument that prevents a full proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingArg {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

/// Resolution path used by Step 3.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResolutionMode {
    Deterministic,
    Llm,
    Partial,
    BlockedExplain,
    NoProposal,
}

/// Step 3 output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedVerb {
    pub verb_id: String,
    pub args: Value,
    pub explanation: String,
    pub requires_confirmation: bool,
    pub missing_args: Vec<MissingArg>,
    pub partial: bool,
    pub resolution_mode: ResolutionMode,
}

fn utterance_tokens(input: &str) -> Vec<String> {
    input
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn classify_action(input: &str) -> Option<&'static str> {
    let tokens = utterance_tokens(input);
    if tokens.iter().any(|t| matches!(t.as_str(), "delete" | "remove")) {
        return Some("delete");
    }
    if tokens.iter().any(|t| matches!(t.as_str(), "create" | "add" | "open" | "onboard" | "register")) {
        return Some("create");
    }
    if tokens.iter().any(|t| matches!(t.as_str(), "update" | "change" | "edit" | "set")) {
        return Some("update");
    }
    if tokens.iter().any(|t| matches!(t.as_str(), "show" | "list" | "read" | "view" | "what" | "who")) {
        return Some("read");
    }
    None
}

fn is_inventory_utterance(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    (lower.contains("show me") || lower.contains("list") || lower.contains("what"))
        && (lower.contains(" cbus")
            || lower.contains(" deals")
            || lower.contains(" documents")
            || lower.contains(" parties")
            || lower.contains(" owners")
            || lower.contains(" entities"))
}

fn is_scope_only_utterance(input: &str) -> bool {
    let extracted = extract_entity_candidates(input);
    let token_count = input.split_whitespace().count();
    !extracted.is_empty() && token_count <= extracted[0].split_whitespace().count() + 1
}

/// Decide whether an utterance explicitly introduces a fresh entity reference.
///
/// This is stricter than plain extraction. Generic inventory or action utterances
/// must not trigger a new entity search just because a residual token survived
/// stop-word stripping.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::semtaxonomy_v2::introduces_entity_reference;
///
/// assert!(introduces_entity_reference("show me the deals for Allianz"));
/// assert!(!introduces_entity_reference("show me the cbus"));
/// ```
pub fn introduces_entity_reference(utterance: &str) -> bool {
    let extracted = extract_entity_candidates(utterance);
    if extracted.is_empty() {
        return false;
    }

    let utterance_trimmed = utterance.trim();
    let utterance_lower = utterance_trimmed.to_ascii_lowercase();
    let explicit_markers = [
        " for ",
        " called ",
        " named ",
        " on ",
        " about ",
        " regarding ",
        " between ",
        " and ",
        " owns ",
        " controls ",
    ];
    if explicit_markers
        .iter()
        .any(|marker| utterance_lower.contains(marker))
    {
        return true;
    }

    extracted.iter().any(|candidate| {
        candidate.split_whitespace().count() > 1
            || candidate.chars().any(|ch| ch.is_uppercase())
            || candidate.chars().any(|ch| ch.is_numeric())
            || utterance_trimmed == candidate
    })
}

fn semantic_similarity_score(utterance: &str, verb: &ValidVerb) -> i64 {
    let utterance_lower = utterance.to_ascii_lowercase();
    let mut score = 0i64;
    if utterance_lower.contains(&verb.verb_id.to_ascii_lowercase()) {
        score += 10;
    }
    for token in verb
        .verb_id
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        if utterance_lower.contains(token) {
            score += 2;
        }
    }
    for phrase in &verb.invocation_phrases {
        let phrase_lower = phrase.to_ascii_lowercase();
        if utterance_lower.contains(&phrase_lower) {
            score += 6;
        } else {
            for token in phrase_lower.split_whitespace() {
                if token.len() > 2 && utterance_lower.contains(token) {
                    score += 1;
                }
            }
        }
    }
    if utterance_lower.contains("beneficial owner") && verb.verb_id.starts_with("ubo.") {
        score += 6;
    }
    if utterance_lower.contains("missing") && verb.verb_id.contains("missing") {
        score += 5;
    }
    score
}

fn synthesize_args(utterance: &str, entity: &EntityScope, verb: &ValidVerb) -> BTreeMap<String, Value> {
    let lower = utterance.to_ascii_lowercase();
    let mut args = BTreeMap::new();
    for parameter in &verb.parameters {
        match parameter.name.as_str() {
            "entity-id" => {
                args.insert(parameter.name.clone(), Value::String(entity.entity_id.to_string()));
            }
            "group-id" if entity.entity_type.eq_ignore_ascii_case("client-group") => {
                args.insert(parameter.name.clone(), Value::String(entity.entity_id.to_string()));
            }
            "cbu-id" if entity.entity_type.eq_ignore_ascii_case("cbu") => {
                args.insert(parameter.name.clone(), Value::String(entity.entity_id.to_string()));
            }
            "deal-id" if entity.entity_type.eq_ignore_ascii_case("deal") => {
                args.insert(parameter.name.clone(), Value::String(entity.entity_id.to_string()));
            }
            "name" => {
                let extracted = extract_entity_candidates(utterance);
                if let Some(name) = extracted.first() {
                    if !name.eq_ignore_ascii_case(&entity.name) || lower.contains("called") || lower.contains("named") || lower.contains("for ") {
                        args.insert(parameter.name.clone(), Value::String(name.clone()));
                    }
                }
            }
            _ => {}
        }
    }
    args
}

fn find_missing_required(verb: &ValidVerb, args: &BTreeMap<String, Value>) -> Vec<MissingArg> {
    verb.parameters
        .iter()
        .filter(|parameter| parameter.required && !args.contains_key(&parameter.name))
        .map(|parameter| MissingArg {
            name: parameter.name.clone(),
            description: parameter.description.clone(),
            required: parameter.required,
        })
        .collect()
}

fn build_partial_explanation(utterance: &str, verb: &ValidVerb, missing: &[MissingArg]) -> String {
    format!(
        "So you want to use {} for '{}', but I still need: {}.",
        verb.verb_id,
        utterance,
        missing
            .iter()
            .map(|arg| arg.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn parse_lane_positions(entity_state: Option<&Value>, transitions: Option<&Value>) -> Vec<LanePosition> {
    let active_lanes = entity_state
        .and_then(|state| state.get("activities"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|activity| {
            let lane = activity.get("domain").and_then(Value::as_str)?.to_string();
            let current_phase = activity
                .get("phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let status = activity
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some(LanePosition {
                lane,
                current_phase,
                is_active: matches!(status, "in_progress" | "blocked" | "pending_review"),
                is_terminal: matches!(status, "completed" | "closed" | "cancelled"),
            })
        })
        .collect::<Vec<_>>();

    let mut positions = active_lanes;
    if let Some(lanes) = transitions.and_then(|value| value.get("lanes")).and_then(Value::as_array) {
        for lane in lanes {
            let lane_name = lane
                .get("lane")
                .and_then(Value::as_str)
                .unwrap_or("general")
                .to_string();
            if positions.iter().any(|position| position.lane == lane_name) {
                continue;
            }
            positions.push(LanePosition {
                lane: lane_name,
                current_phase: lane
                    .get("current_phase")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                is_active: lane
                    .get("valid")
                    .and_then(Value::as_array)
                    .map(|verbs| !verbs.is_empty())
                    .unwrap_or(false),
                is_terminal: false,
            });
        }
    }
    positions
}

fn parse_valid_verbs(transitions: Option<&Value>) -> Vec<ValidVerb> {
    transitions
        .and_then(|value| value.get("lanes"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .flat_map(|lane| {
            let lane_name = lane
                .get("lane")
                .and_then(Value::as_str)
                .unwrap_or("general")
                .to_string();
            let phase = lane
                .get("current_phase")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            lane.get("valid")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(move |verb| ValidVerb {
                    verb_id: verb
                        .get("verb_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    description: verb
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    polarity: verb
                        .get("polarity")
                        .and_then(Value::as_str)
                        .unwrap_or("read")
                        .to_string(),
                    invocation_phrases: verb
                        .get("invocation_phrases")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect(),
                    parameters: verb
                        .get("parameters")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|parameter| ValidVerbParameter {
                            name: parameter
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            required: parameter
                                .get("required")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            description: parameter
                                .get("description")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        })
                        .collect(),
                    lane: lane_name.clone(),
                    phase: phase.clone(),
                    relevance: verb.get("relevance").and_then(Value::as_f64).unwrap_or_default(),
                })
        })
        .collect()
}

fn parse_blocked_verbs(transitions: Option<&Value>) -> Vec<BlockedVerb> {
    transitions
        .and_then(|value| value.get("blocked"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|verb| BlockedVerb {
            verb_id: verb
                .get("verb_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            description: verb
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            unmet_preconditions: verb
                .get("unmet_preconditions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect(),
            unblocking_actions: verb
                .get("unblocking_actions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect(),
        })
        .collect()
}

fn request_explicitly_names_candidate(utterance_lower: &str, candidate: &EntityCandidate) -> bool {
    utterance_lower.contains(&candidate.name.to_ascii_lowercase())
}

fn candidate_rank_key(candidate: &EntityCandidate) -> (bool, bool, usize, i64) {
    (
        candidate.has_active_workflow,
        candidate.lifecycle_populated,
        candidate.linked_entity_count,
        (candidate.match_score * 1000.0) as i64,
    )
}

fn structurally_better(left: &EntityCandidate, right: &EntityCandidate) -> bool {
    (
        left.has_active_workflow,
        left.lifecycle_populated,
        left.linked_entity_count,
    ) > (
        right.has_active_workflow,
        right.lifecycle_populated,
        right.linked_entity_count,
    )
}

fn candidate_to_scope(candidate: &EntityCandidate, source: EntitySource, confidence: f64) -> EntityScope {
    EntityScope {
        entity_id: candidate.entity_id,
        entity_type: candidate.entity_type.clone(),
        name: candidate.name.clone(),
        confidence,
        source,
    }
}

fn merge_entity_candidates(mut candidates: Vec<EntityCandidate>) -> Vec<EntityCandidate> {
    candidates.sort_by(|left, right| {
        candidate_rank_key(right)
            .cmp(&candidate_rank_key(left))
            .then_with(|| left.name.cmp(&right.name))
    });
    candidates.dedup_by(|left, right| left.entity_id == right.entity_id);
    candidates
}

/// Resolve the active entity scope for an utterance.
///
/// This function handles only Step 1 of the three-step pipeline. It may carry
/// session scope when the utterance does not introduce a new entity, or it may
/// return ambiguity when multiple search hits remain plausible.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::semtaxonomy_v2::{step1_entity_scope, EntityScopeOutcome};
///
/// let outcome = step1_entity_scope("show me the cbus", None, &[]);
/// assert!(matches!(outcome, EntityScopeOutcome::Unresolved));
/// ```
pub fn step1_entity_scope(
    utterance: &str,
    previous_scope: Option<&EntityScope>,
    search_candidates: &[EntityCandidate],
) -> EntityScopeOutcome {
    if !introduces_entity_reference(utterance) {
        if let Some(previous_scope) = previous_scope {
            return EntityScopeOutcome::Resolved(EntityScope {
                entity_id: previous_scope.entity_id,
                entity_type: previous_scope.entity_type.clone(),
                name: previous_scope.name.clone(),
                confidence: previous_scope.confidence,
                source: EntitySource::SessionCarry,
            });
        }
    }

    let candidates = merge_entity_candidates(search_candidates.to_vec());
    if candidates.is_empty() {
        return EntityScopeOutcome::Unresolved;
    }

    if candidates.len() == 1 {
        let candidate = &candidates[0];
        return EntityScopeOutcome::Resolved(candidate_to_scope(
            candidate,
            EntitySource::SearchHit,
            candidate.match_score,
        ));
    }

    let utterance_lower = utterance.to_ascii_lowercase();
    let top = &candidates[0];
    let runner_up = &candidates[1];
    let top_is_named = request_explicitly_names_candidate(&utterance_lower, top);
    let dominant = top.match_score >= runner_up.match_score + 0.12
        || structurally_better(top, runner_up);

    if dominant || top_is_named {
        let confidence = if top_is_named {
            top.match_score.max(0.9)
        } else {
            top.match_score
        };
        return EntityScopeOutcome::Resolved(candidate_to_scope(
            top,
            EntitySource::SearchHit,
            confidence,
        ));
    }

    EntityScopeOutcome::Ambiguous(
        candidates
            .into_iter()
            .take(5)
            .map(|candidate| {
                candidate_to_scope(&candidate, EntitySource::SearchHit, candidate.match_score)
            })
            .collect(),
    )
}

/// Normalize grounded state and valid transitions into Step 2 output.
///
/// This function converts the live `entity-context` and `valid-transitions`
/// payloads into the typed `EntityState` contract used by the replacement
/// pipeline.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::semtaxonomy_v2::{step2_entity_state, EntityScope, EntitySource};
/// use serde_json::json;
/// use uuid::Uuid;
///
/// let entity = EntityScope {
///     entity_id: Uuid::new_v4(),
///     entity_type: "client-group".to_string(),
///     name: "Allianz".to_string(),
///     confidence: 1.0,
///     source: EntitySource::SearchHit,
/// };
/// let state = step2_entity_state(entity, None, None);
/// assert!(state.valid_verbs.is_empty());
/// ```
pub fn step2_entity_state(
    entity: EntityScope,
    entity_context: Option<&Value>,
    valid_transitions: Option<&Value>,
) -> EntityState {
    EntityState {
        entity,
        lane_positions: parse_lane_positions(entity_context, valid_transitions),
        valid_verbs: parse_valid_verbs(valid_transitions),
        blocked_verbs: parse_blocked_verbs(valid_transitions),
    }
}

/// Select a business verb from the valid transition set.
///
/// This is Step 3 of the replacement pipeline. It selects only from the
/// already-valid verbs in `EntityState`; it does not perform new grounding.
///
/// # Examples
///
/// ```ignore
/// use ob_poc::semtaxonomy_v2::{step3_select_verb, step2_entity_state, EntityScope, EntitySource};
/// use serde_json::json;
/// use uuid::Uuid;
///
/// let entity = EntityScope {
///     entity_id: Uuid::new_v4(),
///     entity_type: "client-group".to_string(),
///     name: "Allianz".to_string(),
///     confidence: 1.0,
///     source: EntitySource::SearchHit,
/// };
/// let state = step2_entity_state(entity, None, None);
/// let selected = step3_select_verb("what can I do next?", &state);
/// assert!(selected.is_none());
/// ```
pub fn step3_select_verb(utterance: &str, state: &EntityState) -> Option<SelectedVerb> {
    if is_scope_only_utterance(utterance) {
        return None;
    }
    if state.valid_verbs.is_empty() {
        if !state.blocked_verbs.is_empty() {
            let blocked = &state.blocked_verbs[0];
            return Some(SelectedVerb {
                verb_id: blocked.verb_id.clone(),
                args: Value::Object(Default::default()),
                explanation: format!(
                    "This action is currently blocked by: {}",
                    blocked.unmet_preconditions.join(", ")
                ),
                requires_confirmation: false,
                missing_args: Vec::new(),
                partial: true,
                resolution_mode: ResolutionMode::BlockedExplain,
            });
        }
        return None;
    }

    let action = classify_action(utterance);
    let inventory = is_inventory_utterance(utterance);
    let mut candidates = state
        .valid_verbs
        .iter()
        .filter(|verb| !verb.verb_id.starts_with("discovery."))
        .filter(|verb| match action {
            Some("read") => verb.polarity.eq_ignore_ascii_case("read"),
            Some("create") | Some("update") | Some("delete") => !verb.polarity.eq_ignore_ascii_case("read"),
            None => verb.polarity.eq_ignore_ascii_case("read"),
            _ => true,
        })
        .map(|verb| {
            let mut score = semantic_similarity_score(utterance, verb);
            score += (verb.relevance * 10.0) as i64;
            if inventory && verb.verb_id.ends_with(".list") {
                score += 8;
            }
            if matches!(action, Some("create")) && verb.verb_id.contains(".create") {
                score += 8;
            }
            if matches!(action, Some("update")) && verb.verb_id.contains(".update") {
                score += 8;
            }
            if matches!(action, Some("delete")) && (verb.verb_id.contains(".delete") || verb.verb_id.contains(".remove")) {
                score += 8;
            }
            (score, verb)
        })
        .filter(|(score, _)| *score > 0)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.0.cmp(&left.0));

    let (score, verb) = candidates.first()?;
    if *score <= 0 {
        return None;
    }

    let args = synthesize_args(utterance, &state.entity, verb);
    let missing_args = find_missing_required(verb, &args);
    let partial = !missing_args.is_empty();
    Some(SelectedVerb {
        verb_id: verb.verb_id.clone(),
        args: Value::Object(args.into_iter().collect()),
        explanation: if partial {
            build_partial_explanation(utterance, verb, &missing_args)
        } else {
            format!("So you want to use {} for '{}'.", verb.verb_id, utterance)
        },
        requires_confirmation: !partial && !verb.polarity.eq_ignore_ascii_case("read"),
        missing_args,
        partial,
        resolution_mode: if partial {
            ResolutionMode::Partial
        } else {
            ResolutionMode::Deterministic
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{
        introduces_entity_reference, step1_entity_scope, step2_entity_state, step3_select_verb,
        EntityScope, EntityScopeOutcome, EntitySource,
    };
    use crate::semtaxonomy::EntityCandidate;
    use serde_json::json;
    use uuid::Uuid;

    fn candidate(name: &str, score: f64) -> EntityCandidate {
        EntityCandidate {
            entity_id: Uuid::new_v4(),
            entity_type: "client-group".to_string(),
            name: name.to_string(),
            match_score: score,
            match_field: Some("name".to_string()),
            summary: None,
            source_kind: Some("search".to_string()),
            linked_cbu_ids: Vec::new(),
            is_onboarding_member: false,
            candidate_for_cbu: true,
            lifecycle_populated: false,
            linked_entity_count: 0,
            has_active_workflow: false,
        }
    }

    #[test]
    fn step1_uses_session_carry_when_no_new_entity_is_introduced() {
        let previous = EntityScope {
            entity_id: Uuid::new_v4(),
            entity_type: "client-group".to_string(),
            name: "Allianz".to_string(),
            confidence: 1.0,
            source: EntitySource::UserConfirmed,
        };

        let outcome = step1_entity_scope("show me the cbus", Some(&previous), &[]);
        assert_eq!(
            outcome,
            EntityScopeOutcome::Resolved(EntityScope {
                source: EntitySource::SessionCarry,
                ..previous
            })
        );
    }

    #[test]
    fn generic_inventory_utterance_does_not_introduce_entity_reference() {
        assert!(!introduces_entity_reference("show me the cbus"));
        assert!(!introduces_entity_reference("what deals do we have?"));
    }

    #[test]
    fn explicit_named_entity_does_introduce_entity_reference() {
        assert!(introduces_entity_reference("show me the deals for Allianz Global Investors"));
        assert!(introduces_entity_reference("who owns BNP Paribas"));
    }

    #[test]
    fn step1_resolves_a_single_search_hit() {
        let outcomes = vec![candidate("Allianz Global Investors", 0.91)];
        let outcome = step1_entity_scope("allianz", None, &outcomes);
        match outcome {
            EntityScopeOutcome::Resolved(scope) => {
                assert_eq!(scope.name, "Allianz Global Investors");
                assert_eq!(scope.source, EntitySource::SearchHit);
            }
            other => panic!("expected resolved outcome, got {other:?}"),
        }
    }

    #[test]
    fn step1_returns_ambiguity_for_close_candidates() {
        let candidates = vec![candidate("Allianz Global Investors", 0.71), candidate("Allianz SE", 0.69)];
        let outcome = step1_entity_scope("allianz", None, &candidates);
        match outcome {
            EntityScopeOutcome::Ambiguous(scopes) => assert_eq!(scopes.len(), 2),
            other => panic!("expected ambiguous outcome, got {other:?}"),
        }
    }

    #[test]
    fn step1_prefers_explicitly_named_candidate() {
        let candidates = vec![
            candidate("Allianz Global Investors", 0.71),
            candidate("Allianz SE", 0.69),
        ];
        let outcome = step1_entity_scope("show me Allianz Global Investors", None, &candidates);
        match outcome {
            EntityScopeOutcome::Resolved(scope) => {
                assert_eq!(scope.name, "Allianz Global Investors");
            }
            other => panic!("expected resolved outcome, got {other:?}"),
        }
    }

    #[test]
    fn step2_parses_valid_and_blocked_transitions() {
        let entity = EntityScope {
            entity_id: Uuid::new_v4(),
            entity_type: "client-group".to_string(),
            name: "Allianz".to_string(),
            confidence: 0.95,
            source: EntitySource::SearchHit,
        };
        let context = json!({
            "activities": [
                {"domain": "deal", "phase": "pricing", "status": "in_progress"}
            ]
        });
        let transitions = json!({
            "lanes": [
                {
                    "lane": "deal",
                    "current_phase": "pricing",
                    "valid": [
                        {
                            "verb_id": "deal.read-record",
                            "description": "Read the deal record",
                            "polarity": "read",
                            "invocation_phrases": ["show the deal"],
                            "parameters": [{"name": "deal-id", "required": true, "description": "Deal id"}],
                            "relevance": 0.9
                        }
                    ]
                }
            ],
            "blocked": [
                {
                    "verb_id": "deal.propose-rate-card",
                    "description": "Propose a rate card",
                    "unmet_preconditions": ["existing_pricing_context"],
                    "unblocking_actions": ["deal.read-record"]
                }
            ]
        });

        let state = step2_entity_state(entity, Some(&context), Some(&transitions));
        assert_eq!(state.lane_positions.len(), 1);
        assert_eq!(state.valid_verbs.len(), 1);
        assert_eq!(state.blocked_verbs.len(), 1);
        assert_eq!(state.valid_verbs[0].verb_id, "deal.read-record");
        assert_eq!(state.blocked_verbs[0].verb_id, "deal.propose-rate-card");
    }

    #[test]
    fn step3_prefers_inventory_list_for_plural_reads() {
        let entity = EntityScope {
            entity_id: Uuid::new_v4(),
            entity_type: "client-group".to_string(),
            name: "Allianz".to_string(),
            confidence: 0.95,
            source: EntitySource::SearchHit,
        };
        let transitions = json!({
            "lanes": [
                {
                    "lane": "cbu",
                    "current_phase": "active",
                    "valid": [
                        {
                            "verb_id": "cbu.list",
                            "description": "List CBUs",
                            "polarity": "read",
                            "invocation_phrases": ["show me the cbus"],
                            "parameters": [],
                            "relevance": 0.8
                        },
                        {
                            "verb_id": "cbu.read",
                            "description": "Read a CBU",
                            "polarity": "read",
                            "invocation_phrases": ["show me the cbu"],
                            "parameters": [{"name": "cbu-id", "required": true, "description": "CBU id"}],
                            "relevance": 0.6
                        }
                    ]
                }
            ],
            "blocked": []
        });
        let state = step2_entity_state(entity, None, Some(&transitions));
        let selected = step3_select_verb("show me the cbus", &state).expect("selected verb");
        assert_eq!(selected.verb_id, "cbu.list");
        assert!(!selected.partial);
    }

    #[test]
    fn step3_returns_partial_when_required_args_are_missing() {
        let entity = EntityScope {
            entity_id: Uuid::new_v4(),
            entity_type: "client-group".to_string(),
            name: "Allianz".to_string(),
            confidence: 0.95,
            source: EntitySource::SearchHit,
        };
        let transitions = json!({
            "lanes": [
                {
                    "lane": "cbu",
                    "current_phase": "draft",
                    "valid": [
                        {
                            "verb_id": "cbu.create",
                            "description": "Create a CBU",
                            "polarity": "write",
                            "invocation_phrases": ["create a new cbu"],
                            "parameters": [{"name": "name", "required": true, "description": "CBU name"}],
                            "relevance": 0.9
                        }
                    ]
                }
            ],
            "blocked": []
        });
        let state = step2_entity_state(entity, None, Some(&transitions));
        let selected = step3_select_verb("create a new CBU", &state).expect("selected verb");
        assert_eq!(selected.verb_id, "cbu.create");
        assert!(selected.partial);
        assert_eq!(selected.missing_args[0].name, "name");
    }

    #[test]
    fn step3_does_not_propose_for_scope_only_utterance() {
        let entity = EntityScope {
            entity_id: Uuid::new_v4(),
            entity_type: "client-group".to_string(),
            name: "Allianz".to_string(),
            confidence: 0.95,
            source: EntitySource::SearchHit,
        };
        let transitions = json!({
            "lanes": [
                {
                    "lane": "ownership",
                    "current_phase": "active",
                    "valid": [
                        {
                            "verb_id": "ownership.refresh",
                            "description": "Refresh ownership state",
                            "polarity": "write",
                            "invocation_phrases": ["refresh ownership"],
                            "parameters": [],
                            "relevance": 0.9
                        }
                    ]
                }
            ],
            "blocked": []
        });
        let state = step2_entity_state(entity, None, Some(&transitions));
        assert!(step3_select_verb("allianz", &state).is_none());
    }
}
