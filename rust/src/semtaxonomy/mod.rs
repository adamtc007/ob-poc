//! SemTaxonomy replacement contracts.
//!
//! These types define the replacement path for utterance grounding, discovery,
//! and runbook composition without depending on the legacy Sage/Coder structs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SageSession {
    pub session_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub cascade_result: Option<Value>,
    pub active_entity: Option<EntityRef>,
    pub entity_candidates: Vec<EntityCandidate>,
    pub domain_scope: Option<String>,
    pub aspect: Option<String>,
    pub verb_surface: Vec<VerbSurfaceEntry>,
    pub entity_state: Option<Value>,
    pub domain_state_summaries: Vec<DomainStateSummary>,
    pub data_snapshots: HashMap<String, Value>,
    pub utterance_history: Vec<String>,
    pub research_cache: HashMap<String, Value>,
    pub likely_intents: Vec<IntentHint>,
    pub grounding_strategy: Option<String>,
    pub grounding_confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidate {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub name: String,
    pub match_score: f64,
    pub match_field: Option<String>,
    pub summary: Option<Value>,
    pub source_kind: Option<String>,
    pub linked_cbu_ids: Vec<Uuid>,
    pub is_onboarding_member: bool,
    pub candidate_for_cbu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfaceEntry {
    pub verb_id: String,
    pub domain: String,
    pub name: String,
    pub description: String,
    pub polarity: String,
    pub phase_tags: Vec<String>,
    pub subject_kinds: Vec<String>,
    pub parameters: Vec<VerbParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbParameter {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentHint {
    pub intent: String,
    pub confidence: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainStateSummary {
    pub domain: String,
    pub active_count: usize,
    pub blocked_count: usize,
    pub notable_gaps: Vec<String>,
    pub next_action_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionRequest {
    pub raw_utterance: String,
    pub context: CompositionContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompositionContext {
    pub entity: Option<EntityRef>,
    pub entity_candidates: Vec<EntityCandidate>,
    pub domain_scope: Option<String>,
    pub aspect: Option<String>,
    pub entity_state: Option<Value>,
    pub domain_state_summaries: Vec<DomainStateSummary>,
    pub verb_surface: Vec<VerbSurfaceEntry>,
    pub session_history: Vec<String>,
    pub intent_hints: Vec<IntentHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposedRunbook {
    pub steps: Vec<DslStatement>,
    pub explanation: String,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslStatement {
    pub verb_id: String,
    pub args: Value,
    pub polarity: String,
}

fn utterance_tokens(input: &str) -> Vec<String> {
    input
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn is_read_only_utterance(input: &str) -> bool {
    let input = input.to_ascii_lowercase();
    [
        "show", "list", "what", "which", "read", "view", "inspect", "describe", "display",
    ]
    .iter()
    .any(|needle| input.contains(needle))
}

fn is_write_utterance(input: &str) -> bool {
    let input = input.to_ascii_lowercase();
    [
        "create", "add", "update", "change", "delete", "remove", "assign", "set", "open",
    ]
    .iter()
    .any(|needle| input.contains(needle))
}

fn is_inventory_utterance(input: &str) -> bool {
    let input = input.to_ascii_lowercase();
    (["show", "list", "what", "which"]
        .iter()
        .any(|needle| input.contains(needle)))
        && (input.contains(" all ")
            || input.contains(" cbus")
            || input.contains(" deals")
            || input.contains(" documents")
            || input.contains(" entities")
            || input.contains(" list"))
}

fn is_options_utterance(input: &str) -> bool {
    let input = input.to_ascii_lowercase();
    [
        "what can i do",
        "what next",
        "next step",
        "next steps",
        "move forward",
        "options",
        "blocked",
        "how do i progress",
        "how can i progress",
    ]
    .iter()
    .any(|needle| input.contains(needle))
}

fn is_state_utterance(input: &str) -> bool {
    let input = input.to_ascii_lowercase();
    is_read_only_utterance(input.as_str())
        || [
            "status",
            "state",
            "missing",
            "progress",
            "who owns",
            "who are the parties",
            "what deals",
            "what documents",
        ]
        .iter()
        .any(|needle| input.contains(needle))
}

fn infer_mode(input: &str) -> &'static str {
    if is_write_utterance(input) {
        "requested_action"
    } else if is_options_utterance(input) {
        "options_forward"
    } else if is_state_utterance(input) {
        "state_now"
    } else {
        "state_now"
    }
}

fn action_bias(input: &str, verb: &VerbSurfaceEntry) -> usize {
    let input = input.to_ascii_lowercase();
    let verb_id = verb.verb_id.to_ascii_lowercase();
    let name = verb.name.to_ascii_lowercase();
    let mut score = 0;

    if ["show", "list", "what", "which", "read", "view", "display"]
        .iter()
        .any(|needle| input.contains(needle))
        && (verb_id.contains("list")
            || verb_id.contains("read")
            || verb_id.contains("inspect")
            || name.contains("list")
            || name.contains("read"))
    {
        score += 4;
    }

    if ["create", "new", "add", "open"]
        .iter()
        .any(|needle| input.contains(needle))
        && (verb_id.contains("create")
            || verb_id.contains("open")
            || verb_id.contains("add")
            || name.contains("create"))
    {
        score += 5;
    }

    if ["update", "change", "edit", "modify", "set"]
        .iter()
        .any(|needle| input.contains(needle))
        && (verb_id.contains("update")
            || verb_id.contains("set")
            || verb_id.contains("edit")
            || name.contains("update"))
    {
        score += 5;
    }

    if ["relationship", "ownership", "owner", "control", "graph"]
        .iter()
        .any(|needle| input.contains(needle))
        && (verb_id.contains("relationship") || verb_id.contains("ownership"))
    {
        score += 6;
    }

    score
}

fn overlaps(tokens: &[String], text: &str) -> usize {
    let haystack = text.to_ascii_lowercase();
    tokens
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count()
}

fn score_surface_verb(request: &CompositionRequest, verb: &VerbSurfaceEntry) -> usize {
    let tokens = utterance_tokens(&request.raw_utterance);
    let mut score = 0;
    score += overlaps(&tokens, &verb.verb_id) * 3;
    score += overlaps(&tokens, &verb.name) * 2;
    score += overlaps(&tokens, &verb.description);
    score += action_bias(&request.raw_utterance, verb);
    if request.context.domain_scope.as_deref() == Some(verb.domain.as_str()) {
        score += 2;
    }
    if let Some(entity) = request.context.entity.as_ref() {
        if verb
            .subject_kinds
            .iter()
            .any(|kind| kind.eq_ignore_ascii_case(&entity.entity_type))
        {
            score += 2;
        }
    }
    if is_read_only_utterance(&request.raw_utterance) && verb.polarity.eq_ignore_ascii_case("read")
    {
        score += 3;
    }
    if is_write_utterance(&request.raw_utterance) && !verb.polarity.eq_ignore_ascii_case("read") {
        score += 4;
    }
    score
}

fn quote_dsl_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn render_arg_value(value: &Value) -> String {
    match value {
        Value::String(value) => quote_dsl_string(value),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(items) => {
            let rendered = items.iter().map(render_arg_value).collect::<Vec<_>>().join(" ");
            format!("(list {})", rendered)
        }
        Value::Null => "nil".to_string(),
        other => quote_dsl_string(&other.to_string()),
    }
}

fn build_step(verb_id: impl Into<String>, args: Value, polarity: impl Into<String>) -> DslStatement {
    DslStatement {
        verb_id: verb_id.into(),
        args,
        polarity: polarity.into(),
    }
}

fn required_param_names(verb: &VerbSurfaceEntry) -> Vec<&str> {
    verb.parameters
        .iter()
        .filter(|parameter| parameter.required)
        .map(|parameter| parameter.name.as_str())
        .collect()
}

fn extract_named_subject(input: &str) -> Option<String> {
    let normalized = input.trim();
    let lower = normalized.to_ascii_lowercase();
    for marker in [" for ", " called ", " named "] {
        if let Some(idx) = lower.find(marker) {
            let value = normalized[idx + marker.len()..].trim();
            if !value.is_empty() {
                return Some(value.trim_matches(|ch| ch == '"' || ch == '\'').to_string());
            }
        }
    }
    None
}

fn synthesize_args(request: &CompositionRequest, verb: &VerbSurfaceEntry) -> serde_json::Map<String, Value> {
    let mut args = serde_json::Map::new();
    if let Some(entity) = request.context.entity.as_ref() {
        let required_names = required_param_names(verb);
        let candidate_arg_names = match entity.entity_type.to_ascii_lowercase().as_str() {
            "client-group" => vec!["group-id", "client-group-id", "client-id", "entity-id"],
            "cbu" => vec!["cbu-id", "entity-id"],
            "deal" => vec!["deal-id", "entity-id"],
            "document" => vec!["document-id", "doc-id", "entity-id"],
            "case" => vec!["case-id", "entity-id"],
            _ => vec!["entity-id", "subject-entity-id", "target-entity-id"],
        };

        for arg_name in candidate_arg_names {
            if required_names.iter().any(|name| *name == arg_name) {
                args.insert(arg_name.to_string(), Value::String(entity.entity_id.to_string()));
                break;
            }
        }
    }

    if let Some(name) = extract_named_subject(&request.raw_utterance) {
        for parameter in &verb.parameters {
            let name_key = parameter.name.to_ascii_lowercase();
            if name_key.contains("name") && !args.contains_key(&parameter.name) {
                args.insert(parameter.name.clone(), Value::String(name.clone()));
                break;
            }
        }
    }

    args
}

fn derive_next_actions(request: &CompositionRequest) -> Vec<String> {
    let mut actions = Vec::new();
    for summary in &request.context.domain_state_summaries {
        for candidate in &summary.next_action_candidates {
            if !actions.iter().any(|existing| existing == candidate) {
                actions.push(candidate.clone());
            }
        }
    }
    if actions.is_empty() {
        for verb in &request.context.verb_surface {
            if !verb.polarity.eq_ignore_ascii_case("read") {
                actions.push(verb.verb_id.clone());
            }
            if actions.len() >= 5 {
                break;
            }
        }
    }
    actions
}

fn build_state_explanation(request: &CompositionRequest) -> String {
    let entity_name = request
        .context
        .entity
        .as_ref()
        .map(|entity| entity.name.clone())
        .unwrap_or_else(|| "this context".to_string());
    let mut gaps = request
        .context
        .domain_state_summaries
        .iter()
        .flat_map(|summary| summary.notable_gaps.iter().cloned())
        .collect::<Vec<_>>();
    gaps.sort();
    gaps.dedup();

    if gaps.is_empty() {
        format!("So you want the current state for {}.", entity_name)
    } else {
        format!(
            "So you want the current state for {}. Current gaps or blockers: {}.",
            entity_name,
            gaps.join(", ")
        )
    }
}

fn build_options_explanation(request: &CompositionRequest, next_actions: &[String]) -> String {
    let entity_name = request
        .context
        .entity
        .as_ref()
        .map(|entity| entity.name.clone())
        .unwrap_or_else(|| "this context".to_string());
    if next_actions.is_empty() {
        format!(
            "So you want the available options to move {} forward, but there are no clear actions from the current grounded state yet.",
            entity_name
        )
    } else {
        format!(
            "So you want the available options to move {} forward. The strongest options are: {}.",
            entity_name,
            next_actions.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

/// Build a composition request from the current unified session and raw utterance.
///
/// # Examples
///
/// ```ignore
/// let session = ob_poc::session::UnifiedSession::new();
/// let req = ob_poc::semtaxonomy::build_composition_request(&session, "show me Allianz");
/// assert_eq!(req.raw_utterance, "show me Allianz");
/// ```
pub fn build_composition_request(
    session: &crate::session::UnifiedSession,
    raw_utterance: impl Into<String>,
) -> CompositionRequest {
    let entity = session
        .semtaxonomy_session
        .as_ref()
        .and_then(|state| state.active_entity.clone())
        .or_else(|| {
            session.context.client_scope.as_ref().and_then(|scope| {
                scope.client_group_id.map(|entity_id| EntityRef {
                    entity_id,
                    entity_type: "client-group".to_string(),
                    name: scope
                        .client_group_name
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                })
            })
        })
        .or_else(|| {
            session.current_case.as_ref().map(|case| EntityRef {
                entity_id: case.case_id,
                entity_type: "case".to_string(),
                name: case.display_name.clone(),
            })
        })
        .or_else(|| {
            session.current_structure.as_ref().map(|structure| EntityRef {
                entity_id: structure.structure_id,
                entity_type: structure.structure_type.to_string(),
                name: structure.display_name.clone(),
            })
        })
        .or_else(|| {
            session.current_mandate.as_ref().map(|mandate| EntityRef {
                entity_id: mandate.mandate_id,
                entity_type: "mandate".to_string(),
                name: mandate.display_name.clone(),
            })
        });
    let domain_scope = session
        .semtaxonomy_session
        .as_ref()
        .and_then(|state| state.domain_scope.clone())
        .or_else(|| session.domain_hint.clone())
        .or_else(|| {
            session
                .context
                .stage_focus
                .as_deref()
                .and_then(|focus| focus.strip_prefix("semos-"))
                .map(ToOwned::to_owned)
        });
    let aspect = session
        .semtaxonomy_session
        .as_ref()
        .and_then(|state| state.aspect.clone())
        .or_else(|| session.context.stage_focus.clone());
    let entity_state = session
        .semtaxonomy_session
        .as_ref()
        .and_then(|state| state.entity_state.clone());
    let entity_candidates = session
        .semtaxonomy_session
        .as_ref()
        .map(|state| state.entity_candidates.clone())
        .unwrap_or_default();
    let domain_state_summaries = session
        .semtaxonomy_session
        .as_ref()
        .map(|state| state.domain_state_summaries.clone())
        .unwrap_or_default();
    let verb_surface = session
        .semtaxonomy_session
        .as_ref()
        .map(|state| state.verb_surface.clone())
        .unwrap_or_default();
    let mut intent_hints = session
        .semtaxonomy_session
        .as_ref()
        .map(|state| state.likely_intents.clone())
        .unwrap_or_default();
    if let Some(stage_focus) = session.context.stage_focus.as_ref() {
        intent_hints.push(IntentHint {
            intent: "stage-focus".to_string(),
            confidence: "high".to_string(),
            reason: format!("Current workflow focus is {}", stage_focus),
        });
    }
    let session_history = session
        .messages
        .iter()
        .rev()
        .take(12)
        .map(|message| message.content.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    CompositionRequest {
        raw_utterance: raw_utterance.into(),
        context: CompositionContext {
            entity,
            entity_candidates,
            domain_scope,
            aspect,
            entity_state,
            domain_state_summaries,
            verb_surface,
            session_history,
            intent_hints,
        },
    }
}

/// Refresh a `SageSession` ledger from discovery outputs.
///
/// # Examples
///
/// ```ignore
/// let mut state = ob_poc::semtaxonomy::SageSession::default();
/// ob_poc::semtaxonomy::hydrate_sage_session(
///     &mut state,
///     Some(serde_json::json!({"query": "Allianz"})),
///     None,
///     Some("deal".to_string()),
///     None,
///     vec![],
///     None,
///     vec![],
/// );
/// assert_eq!(state.domain_scope.as_deref(), Some("deal"));
/// ```
pub fn hydrate_sage_session(
    session: &mut SageSession,
    cascade_result: Option<Value>,
    active_entity: Option<EntityRef>,
    entity_candidates: Vec<EntityCandidate>,
    domain_scope: Option<String>,
    aspect: Option<String>,
    verb_surface: Vec<VerbSurfaceEntry>,
    entity_state: Option<Value>,
    domain_state_summaries: Vec<DomainStateSummary>,
    intent_hints: Vec<IntentHint>,
    grounding_strategy: Option<String>,
    grounding_confidence: Option<String>,
) {
    if let Some(cascade_result) = cascade_result {
        session.cascade_result = Some(cascade_result);
    }
    session.active_entity = active_entity;
    session.entity_candidates = entity_candidates;
    session.domain_scope = domain_scope;
    session.aspect = aspect;
    session.verb_surface = verb_surface;
    session.entity_state = entity_state;
    session.domain_state_summaries = domain_state_summaries;
    session.likely_intents = intent_hints;
    session.grounding_strategy = grounding_strategy;
    session.grounding_confidence = grounding_confidence;
}

/// Compose a deterministic runbook from a grounded request.
///
/// This is the SemTaxonomy replacement fast path. It prefers an explicitly
/// grounded surface verb when one scores clearly, otherwise it falls back to
/// read-only discovery verbs.
///
/// # Examples
///
/// ```ignore
/// let req = ob_poc::semtaxonomy::CompositionRequest {
///     raw_utterance: "show me the context".to_string(),
///     context: Default::default(),
/// };
/// let runbook = ob_poc::semtaxonomy::compose_runbook(&req);
/// assert!(runbook.is_some());
/// ```
pub fn compose_runbook(request: &CompositionRequest) -> Option<ComposedRunbook> {
    let mode = infer_mode(&request.raw_utterance);
    let read_only = is_read_only_utterance(&request.raw_utterance);
    let write_intent = is_write_utterance(&request.raw_utterance);
    let inventory = is_inventory_utterance(&request.raw_utterance);
    let next_actions = derive_next_actions(request);
    let mut scored_candidates = request
        .context
        .verb_surface
        .iter()
        .filter(|verb| !verb.verb_id.starts_with("discovery."))
        .filter(|verb| {
            if write_intent {
                !verb.polarity.eq_ignore_ascii_case("read")
            } else if read_only {
                verb.polarity.eq_ignore_ascii_case("read")
            } else {
                true
            }
        })
        .map(|verb| (score_surface_verb(request, verb), verb))
        .filter(|(score, _)| *score > 0)
        .collect::<Vec<_>>();
    scored_candidates.sort_by(|left, right| right.0.cmp(&left.0));

    if inventory {
        for (_, verb) in &scored_candidates {
            if !verb.verb_id.ends_with(".list") {
                continue;
            }
            let args = synthesize_args(request, verb);
            let missing_required = required_param_names(verb)
                .into_iter()
                .any(|name| !args.contains_key(name));
            if missing_required {
                continue;
            }
            return Some(ComposedRunbook {
                steps: vec![build_step(
                    verb.verb_id.clone(),
                    Value::Object(args),
                    verb.polarity.clone(),
                )],
                explanation: format!(
                    "So you want to list the current {} records for this context.",
                    request
                        .context
                        .domain_scope
                        .clone()
                        .unwrap_or_else(|| "grounded".to_string())
                ),
                requires_confirmation: false,
            });
        }
    }

    if let Some((best_score, _)) = scored_candidates.first() {
        let best_score = *best_score;
        let runner_up = scored_candidates.get(1).map(|item| item.0).unwrap_or(0);
        for (score, verb) in scored_candidates.into_iter() {
            if score < 4 || score + 1 < best_score || score + 1 < runner_up {
                continue;
            }
            let args = synthesize_args(request, verb);
            let missing_required = required_param_names(verb)
                .into_iter()
                .any(|name| !args.contains_key(name));
            if missing_required {
                continue;
            }
            return Some(ComposedRunbook {
                steps: vec![build_step(
                    verb.verb_id.clone(),
                    Value::Object(args),
                    verb.polarity.clone(),
                )],
                explanation: format!(
                    "So you want to use {} to address: {}",
                    verb.verb_id, request.raw_utterance
                ),
                requires_confirmation: !verb.polarity.eq_ignore_ascii_case("read"),
            });
        }
    }

    if mode == "options_forward" {
        return Some(ComposedRunbook {
            steps: Vec::new(),
            explanation: build_options_explanation(request, &next_actions),
            requires_confirmation: false,
        });
    }

    if mode == "state_now" {
        return Some(ComposedRunbook {
            steps: Vec::new(),
            explanation: build_state_explanation(request),
            requires_confirmation: false,
        });
    }

    if write_intent {
        return Some(ComposedRunbook {
            steps: Vec::new(),
            explanation: build_options_explanation(request, &next_actions),
            requires_confirmation: false,
        });
    }

    None
}

/// Render a composed runbook into executable DSL text.
///
/// # Examples
///
/// ```ignore
/// let runbook = ob_poc::semtaxonomy::ComposedRunbook::default();
/// let dsl = ob_poc::semtaxonomy::render_runbook_dsl(&runbook);
/// assert_eq!(dsl, "");
/// ```
pub fn render_runbook_dsl(runbook: &ComposedRunbook) -> String {
    runbook
        .steps
        .iter()
        .map(|statement| {
            let mut parts = vec![format!("({}", statement.verb_id)];
            if let Value::Object(args) = &statement.args {
                let mut keys = args.keys().cloned().collect::<Vec<_>>();
                keys.sort();
                for key in keys {
                    if let Some(value) = args.get(&key) {
                        parts.push(format!(" :{} {}", key, render_arg_value(value)));
                    }
                }
            }
            parts.push(")".to_string());
            parts.join("")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client_group_entity() -> EntityRef {
        EntityRef {
            entity_id: Uuid::nil(),
            entity_type: "client-group".to_string(),
            name: "Allianz".to_string(),
        }
    }

    fn cbu_list_verb() -> VerbSurfaceEntry {
        VerbSurfaceEntry {
            verb_id: "cbu.list".to_string(),
            domain: "cbu".to_string(),
            name: "list".to_string(),
            description: "List CBUs".to_string(),
            polarity: "read".to_string(),
            phase_tags: vec!["onboarding".to_string()],
            subject_kinds: vec!["client-group".to_string()],
            parameters: vec![],
        }
    }

    fn cbu_show_verb() -> VerbSurfaceEntry {
        VerbSurfaceEntry {
            verb_id: "cbu.show".to_string(),
            domain: "cbu".to_string(),
            name: "show".to_string(),
            description: "Show CBU".to_string(),
            polarity: "read".to_string(),
            phase_tags: vec!["onboarding".to_string()],
            subject_kinds: vec!["client-group".to_string()],
            parameters: vec![VerbParameter {
                name: "cbu-id".to_string(),
                required: true,
                description: None,
            }],
        }
    }

    fn cbu_create_verb() -> VerbSurfaceEntry {
        VerbSurfaceEntry {
            verb_id: "cbu.create".to_string(),
            domain: "cbu".to_string(),
            name: "create".to_string(),
            description: "Create CBU".to_string(),
            polarity: "write".to_string(),
            phase_tags: vec!["onboarding".to_string()],
            subject_kinds: vec!["client-group".to_string()],
            parameters: vec![VerbParameter {
                name: "name".to_string(),
                required: true,
                description: None,
            }],
        }
    }

    #[test]
    fn compose_runbook_prefers_inventory_list_for_plural_reads() {
        let request = CompositionRequest {
            raw_utterance: "show me the cbus".to_string(),
            context: CompositionContext {
                entity: Some(client_group_entity()),
                domain_scope: Some("cbu".to_string()),
                aspect: None,
                entity_state: None,
                verb_surface: vec![cbu_show_verb(), cbu_list_verb()],
                session_history: vec![],
                intent_hints: vec![],
            },
        };

        let runbook = compose_runbook(&request).expect("inventory runbook");
        assert_eq!(runbook.steps[0].verb_id, "cbu.list");
    }

    #[test]
    fn compose_runbook_builds_simple_create() {
        let request = CompositionRequest {
            raw_utterance: "create a new CBU for Allianz UK Fund".to_string(),
            context: CompositionContext {
                entity: Some(client_group_entity()),
                domain_scope: Some("cbu".to_string()),
                aspect: None,
                entity_state: None,
                verb_surface: vec![cbu_create_verb()],
                session_history: vec![],
                intent_hints: vec![],
            },
        };

        let runbook = compose_runbook(&request).expect("create runbook");
        assert_eq!(runbook.steps[0].verb_id, "cbu.create");
        assert_eq!(runbook.steps[0].args["name"], Value::String("Allianz UK Fund".to_string()));
    }
}
