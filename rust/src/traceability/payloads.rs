//! Deterministic trace payload builders for early utterance phases.

use serde_json::json;
use sha2::{Digest, Sha256};

/// Builds Phase 0 and Phase 1 trace payload fragments for a raw utterance.
///
/// # Examples
/// ```rust
/// use ob_poc::sage::SageContext;
/// use ob_poc::traceability::build_phase_trace_payload;
///
/// let payload = build_phase_trace_payload("show me all funds", &SageContext::default());
/// assert!(payload.get("phase_0").is_some());
/// assert!(payload.get("phase_1").is_some());
/// ```
pub fn build_phase_trace_payload(
    utterance: &str,
    ctx: &crate::sage::SageContext,
) -> serde_json::Value {
    let pre = crate::sage::pre_classify::pre_classify(utterance, ctx);
    let token_spans = token_spans(utterance);
    let quantifiers = quantifiers(&token_spans);
    let noun_phrases = noun_phrases(&token_spans);
    let verb_phrases = verb_phrases(&token_spans);
    let referential_bindings = referential_bindings(&token_spans);
    let token_map = token_map(&token_spans);

    json!({
        "phase_0": {
            "plane": pre.plane.as_str(),
            "plane_confidence": phase0_plane_confidence(ctx, &pre),
            "polarity": pre.polarity.as_str(),
            "polarity_confidence": phase0_polarity_confidence(&pre),
            "domain_hints": pre.domain_hints,
            "lexical_signals": phase0_lexical_signals(&pre),
        },
        "phase_1": {
            "raw_utterance": utterance,
            "verb_phrases": verb_phrases,
            "noun_phrases": noun_phrases,
            "quantifiers": quantifiers,
            "referential_bindings": referential_bindings,
            "parse_method": "deterministic_trace_builder_v1",
            "token_map": token_map,
        }
    })
}

/// Build the common scaffold payload for an in-progress utterance trace.
///
/// This keeps the initial phase layout consistent across entrypoints while
/// allowing callers to extend the object with entrypoint-specific metadata.
///
/// # Examples
/// ```rust
/// use ob_poc::sage::SageContext;
/// use ob_poc::traceability::{build_phase2_unavailable_payload, build_trace_scaffold_payload};
///
/// let payload = build_trace_scaffold_payload(
///     "open the case",
///     &SageContext::default(),
///     build_phase2_unavailable_payload("example"),
///     "example",
/// );
/// assert_eq!(payload["phase_3"]["status"], "unavailable");
/// ```
#[cfg(feature = "database")]
pub fn build_trace_scaffold_payload(
    utterance: &str,
    ctx: &crate::sage::SageContext,
    phase2_payload: serde_json::Value,
    entrypoint: &str,
) -> serde_json::Value {
    let phase_payload = build_phase_trace_payload(utterance, ctx);
    json!({
        "phase_0": phase_payload["phase_0"].clone(),
        "phase_1": phase_payload["phase_1"].clone(),
        "phase_2": phase2_payload,
        "phase_3": crate::traceability::build_phase3_unavailable_payload(entrypoint),
        "phase_4": crate::traceability::build_phase4_unavailable_payload(entrypoint),
        "phase_5": crate::traceability::build_phase5_unavailable_payload(entrypoint),
        "entrypoint": entrypoint,
    })
}

/// Build the common finalized trace payload for an utterance trace.
///
/// This keeps the terminal phase layout consistent across entrypoints while
/// allowing callers to extend the object with entrypoint-specific metadata.
///
/// # Examples
/// ```rust
/// use ob_poc::sage::SageContext;
/// use ob_poc::traceability::{
///     build_phase2_unavailable_payload, build_phase3_unavailable_payload,
///     build_phase4_unavailable_payload, build_phase5_unavailable_payload,
///     build_final_trace_payload,
/// };
///
/// let payload = build_final_trace_payload(
///     "open the case",
///     &SageContext::default(),
///     build_phase2_unavailable_payload("example"),
///     build_phase3_unavailable_payload("example"),
///     build_phase4_unavailable_payload("example"),
///     build_phase5_unavailable_payload("example"),
///     "example",
/// );
/// assert_eq!(payload["phase_4"]["status"], "unavailable");
/// ```
#[cfg(feature = "database")]
pub fn build_final_trace_payload(
    utterance: &str,
    ctx: &crate::sage::SageContext,
    phase2_payload: serde_json::Value,
    phase3_payload: serde_json::Value,
    phase4_payload: serde_json::Value,
    phase5_payload: serde_json::Value,
    entrypoint: &str,
) -> serde_json::Value {
    let phase_payload = build_phase_trace_payload(utterance, ctx);
    json!({
        "phase_0": phase_payload["phase_0"].clone(),
        "phase_1": phase_payload["phase_1"].clone(),
        "phase_2": phase2_payload,
        "phase_3": phase3_payload,
        "phase_4": phase4_payload,
        "phase_5": phase5_payload,
        "entrypoint": entrypoint,
    })
}

/// Builds a Phase 2 trace payload from current lookup and Sem OS legality data.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::build_phase2_trace_payload;
///
/// let payload = build_phase2_trace_payload(None, None);
/// assert_eq!(payload["authoritative_source"], "sem_os_context_envelope");
/// ```
#[cfg(feature = "database")]
pub fn build_phase2_trace_payload(
    lookup: Option<&crate::lookup::LookupResult>,
    envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> serde_json::Value {
    let resolution_mode = phase2_resolution_mode(lookup);
    let resolved_entities = phase2_resolved_entities(lookup);
    let ambiguous_entities = phase2_ambiguous_entities(lookup);
    let unresolved_nouns = phase2_unresolved_nouns(lookup);
    let dominant_entity = phase2_dominant_entity(lookup);
    let legal_verb_set = phase2_legal_verb_set(envelope);
    let legality = phase2_legality(envelope);
    let situation_signature = phase2_situation_signature(lookup, envelope);
    let constellation_snapshot = phase2_constellation_snapshot(lookup, envelope);
    let dag_provenance = phase2_dag_provenance(envelope);

    json!({
        "authoritative_source": "sem_os_context_envelope",
        "status": if envelope.is_some() { "available" } else { "unavailable" },
        "resolution_mode": resolution_mode,
        "resolved_entities": resolved_entities,
        "ambiguous_entities": ambiguous_entities,
        "unresolved_nouns": unresolved_nouns,
        "constellation_snapshot": constellation_snapshot,
        "constellation_recovery_time_ms": serde_json::Value::Null,
        "situation_signature": situation_signature,
        "dag_provenance": dag_provenance,
        "verb_taxonomy": serde_json::Value::Null,
        "legality": legality,
        "legal_verb_set": legal_verb_set,
        "expected_kinds": lookup.map(|value| value.expected_kinds.clone()).unwrap_or_default(),
        "dominant_entity": dominant_entity,
        "entities_resolved": lookup.map(|value| value.entities_resolved).unwrap_or(false),
        "verb_matched": lookup.map(|value| value.verb_matched).unwrap_or(false),
    })
}

/// Builds a minimal unavailable Phase 2 placeholder for paths without Sem OS data.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::build_phase2_unavailable_payload;
///
/// let payload = build_phase2_unavailable_payload("agent_service_direct");
/// assert_eq!(payload["status"], "unavailable");
/// ```
pub fn build_phase2_unavailable_payload(entrypoint: &str) -> serde_json::Value {
    json!({
        "authoritative_source": "sem_os_context_envelope",
        "status": "unavailable",
        "entrypoint": entrypoint,
        "resolution_mode": "unavailable",
        "resolved_entities": [],
        "ambiguous_entities": [],
        "unresolved_nouns": [],
        "constellation_snapshot": serde_json::Value::Null,
        "constellation_recovery_time_ms": serde_json::Value::Null,
        "situation_signature": serde_json::Value::Null,
        "dag_provenance": serde_json::Value::Null,
        "verb_taxonomy": serde_json::Value::Null,
        "legality": serde_json::Value::Null,
        "legal_verb_set": [],
    })
}

/// Compute a stable situation-signature hash from the current Phase 2 inputs.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::compute_phase2_situation_signature_hash;
///
/// let hash = compute_phase2_situation_signature_hash(None, None);
/// assert!(hash.is_some());
/// ```
#[cfg(feature = "database")]
pub fn compute_phase2_situation_signature_hash(
    lookup: Option<&crate::lookup::LookupResult>,
    envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> Option<i64> {
    let canonical_form = phase2_canonical_form(lookup, envelope);
    Some(stable_signature_hash(&canonical_form))
}

#[cfg(feature = "database")]
fn stable_signature_hash(value: &str) -> i64 {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    i64::from_be_bytes(bytes)
}

#[cfg(feature = "database")]
fn phase2_resolution_mode(lookup: Option<&crate::lookup::LookupResult>) -> &'static str {
    let Some(lookup) = lookup else {
        return "unavailable";
    };

    let resolved_count = lookup
        .entities
        .iter()
        .filter(|entity| entity.selected.is_some())
        .count();
    let ambiguous_count = lookup
        .entities
        .iter()
        .filter(|entity| entity.selected.is_none() && entity.candidates.len() > 1)
        .count();

    if resolved_count > 1 {
        "cross_entity"
    } else if resolved_count == 1 {
        "single_direct"
    } else if ambiguous_count > 0 {
        "disambiguated"
    } else if !lookup.expected_kinds.is_empty() {
        "filtered_set"
    } else {
        "unresolved"
    }
}

#[cfg(feature = "database")]
fn phase2_resolved_entities(
    lookup: Option<&crate::lookup::LookupResult>,
) -> Vec<serde_json::Value> {
    lookup
        .map(|value| {
            value
                .entities
                .iter()
                .filter_map(|entity| {
                    let selected = entity.selected?;
                    let selected_candidate = entity
                        .candidates
                        .iter()
                        .find(|candidate| candidate.entity_id == selected)
                        .or_else(|| entity.candidates.first())?;

                    Some(json!({
                        "entity_id": selected,
                        "entity_kind": selected_candidate.entity_kind,
                        "canonical_name": selected_candidate.canonical_name,
                        "mention_text": entity.mention_text,
                        "mention_span": [entity.mention_span.0, entity.mention_span.1],
                        "confidence": entity.confidence,
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(feature = "database")]
fn phase2_ambiguous_entities(
    lookup: Option<&crate::lookup::LookupResult>,
) -> Vec<serde_json::Value> {
    lookup
        .map(|value| {
            value
                .entities
                .iter()
                .filter(|entity| entity.selected.is_none() && entity.candidates.len() > 1)
                .map(|entity| {
                    let candidates = entity
                        .candidates
                        .iter()
                        .take(5)
                        .map(|candidate| {
                            json!({
                                "entity_id": candidate.entity_id,
                                "entity_kind": candidate.entity_kind,
                                "canonical_name": candidate.canonical_name,
                                "score": candidate.score,
                            })
                        })
                        .collect::<Vec<_>>();

                    json!({
                        "noun_phrase": entity.mention_text,
                        "mention_span": [entity.mention_span.0, entity.mention_span.1],
                        "candidates": candidates,
                        "disambiguation_method": "entity_linking_score_margin",
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(feature = "database")]
fn phase2_unresolved_nouns(lookup: Option<&crate::lookup::LookupResult>) -> Vec<serde_json::Value> {
    lookup
        .map(|value| {
            value
                .entities
                .iter()
                .filter_map(|entity| {
                    if entity.selected.is_some() {
                        return None;
                    }

                    let failure_reason = if entity.candidates.is_empty() {
                        "no_candidates"
                    } else if entity.candidates.len() == 1 {
                        "below_selection_threshold"
                    } else {
                        "ambiguous_candidates"
                    };

                    Some(json!({
                        "noun_phrase": entity.mention_text,
                        "binding_type": "entity_mention",
                        "resolution_attempts": entity.candidates.len(),
                        "failure_reason": failure_reason,
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(feature = "database")]
fn phase2_dominant_entity(
    lookup: Option<&crate::lookup::LookupResult>,
) -> Option<serde_json::Value> {
    lookup.and_then(|value| {
        value.dominant_entity.as_ref().map(|entity| {
            json!({
                "entity_id": entity.entity_id,
                "entity_kind": entity.entity_kind,
                "canonical_name": entity.canonical_name,
                "confidence": entity.confidence,
                "mention_span": [entity.mention_span.0, entity.mention_span.1],
            })
        })
    })
}

#[cfg(feature = "database")]
fn phase2_legal_verb_set(
    envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> Vec<String> {
    let Some(envelope) = envelope else {
        return Vec::new();
    };

    let mut verbs: Vec<String> = envelope.allowed_verbs.iter().cloned().collect();
    verbs.sort();
    verbs
}

#[cfg(feature = "database")]
fn phase2_legality(
    envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> serde_json::Value {
    match envelope {
        Some(value) => json!({
            "status": value.label(),
            "resolution_stage": format!("{:?}", value.resolution_stage),
            "legal_verb_count": value.allowed_verbs.len(),
            "pruned_verb_count": value.pruned_count(),
            "fingerprint": value.fingerprint_str(),
            "deny_all": value.is_deny_all(),
            "unavailable": value.is_unavailable(),
            "discovery_stage": value.is_discovery_stage(),
            "evidence_gap_count": value.evidence_gaps.len(),
            "governance_signal_count": value.governance_signals.len(),
            "constraint_signal_count": value
                .grounded_action_surface
                .as_ref()
                .map(|surface| surface.constraint_signals.len())
                .unwrap_or(0),
            "blocked_action_count": value
                .grounded_action_surface
                .as_ref()
                .map(|surface| surface.blocked_actions.len())
                .unwrap_or(0),
            "blocked_actions": phase2_blocked_actions(value),
            "constellation_blocks": phase2_constellation_blocks(value),
            "snapshot_set_id": value.snapshot_set_id,
        }),
        None => serde_json::Value::Null,
    }
}

#[cfg(feature = "database")]
fn phase2_blocked_actions(
    envelope: &crate::agent::sem_os_context_envelope::SemOsContextEnvelope,
) -> Vec<serde_json::Value> {
    envelope
        .grounded_action_surface
        .as_ref()
        .map(|surface| {
            surface
                .blocked_actions
                .iter()
                .map(|blocked| {
                    json!({
                        "action_id": blocked.action_id,
                        "action_kind": blocked.action_kind,
                        "description": blocked.description,
                        "reasons": blocked.reasons,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(feature = "database")]
fn phase2_constellation_blocks(
    envelope: &crate::agent::sem_os_context_envelope::SemOsContextEnvelope,
) -> Vec<serde_json::Value> {
    let Some(surface) = envelope.grounded_action_surface.as_ref() else {
        return Vec::new();
    };

    surface
        .constraint_signals
        .iter()
        .map(|signal| {
            json!({
                "blocked_verb": blocked_verb_for_signal(surface, signal),
                "blocking_entity": signal.related_slot,
                "blocking_entity_type": serde_json::Value::Null,
                "blocking_state": signal.actual_state,
                "predicate": signal.message,
                "resolution_hint": constellation_block_resolution_hint(signal),
                "constraint_kind": signal.kind,
                "required_state": signal.required_state,
                "slot_path": signal.slot_path,
            })
        })
        .collect()
}

#[cfg(feature = "database")]
fn blocked_verb_for_signal(
    surface: &sem_os_core::context_resolution::GroundedActionSurface,
    signal: &sem_os_core::context_resolution::GroundedConstraintSignal,
) -> serde_json::Value {
    surface
        .blocked_actions
        .iter()
        .find(|blocked| {
            blocked
                .reasons
                .iter()
                .any(|reason| reason == &signal.message)
        })
        .map(|action| serde_json::Value::String(action.action_id.clone()))
        .unwrap_or(serde_json::Value::Null)
}

#[cfg(feature = "database")]
fn constellation_block_resolution_hint(
    signal: &sem_os_core::context_resolution::GroundedConstraintSignal,
) -> String {
    match (
        &signal.related_slot,
        &signal.required_state,
        &signal.actual_state,
    ) {
        (Some(slot), Some(required), Some(actual)) => {
            format!("move '{slot}' from '{actual}' to at least '{required}'")
        }
        (Some(slot), Some(required), None) => {
            format!("materialize '{slot}' and reach at least '{required}'")
        }
        (Some(slot), None, _) => format!("satisfy constraints on '{slot}'"),
        _ => "satisfy blocking constellation constraints".to_string(),
    }
}

#[cfg(feature = "database")]
fn phase2_situation_signature(
    lookup: Option<&crate::lookup::LookupResult>,
    envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> serde_json::Value {
    let canonical_form = phase2_canonical_form(lookup, envelope);
    let entity_types_present = lookup
        .map(|value| {
            let mut kinds = value
                .entities
                .iter()
                .filter_map(|entity| {
                    entity.selected.and_then(|selected| {
                        entity
                            .candidates
                            .iter()
                            .find(|candidate| candidate.entity_id == selected)
                            .or_else(|| entity.candidates.first())
                            .map(|candidate| candidate.entity_kind.clone())
                    })
                })
                .collect::<Vec<_>>();
            kinds.sort();
            kinds.dedup();
            kinds
        })
        .unwrap_or_default();

    let operational_phase = envelope.map_or("unknown", |value| {
        if value.is_discovery_stage() {
            "Discovery"
        } else if value.is_deny_all() {
            "Blocked"
        } else if value.is_unavailable() {
            "Unavailable"
        } else {
            "Grounded"
        }
    });

    json!({
        "canonical_form": canonical_form,
        "signature_hash": compute_phase2_situation_signature_hash(lookup, envelope),
        "situation_label": "phase2_placeholder",
        "entity_types_present": entity_types_present,
        "entity_types_missing": lookup.map(|value| value.expected_kinds.clone()).unwrap_or_default(),
        "operational_phase": operational_phase,
    })
}

#[cfg(feature = "database")]
fn phase2_canonical_form(
    lookup: Option<&crate::lookup::LookupResult>,
    _envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> String {
    let entity_types_present = lookup
        .map(|value| {
            let mut kinds = value
                .entities
                .iter()
                .filter_map(|entity| {
                    entity.selected.and_then(|selected| {
                        entity
                            .candidates
                            .iter()
                            .find(|candidate| candidate.entity_id == selected)
                            .or_else(|| entity.candidates.first())
                            .map(|candidate| candidate.entity_kind.clone())
                    })
                })
                .collect::<Vec<_>>();
            kinds.sort();
            kinds.dedup();
            kinds
        })
        .unwrap_or_default();

    if entity_types_present.is_empty() {
        "no_entities".to_string()
    } else {
        entity_types_present.join("+")
    }
}

#[cfg(feature = "database")]
fn phase2_constellation_snapshot(
    lookup: Option<&crate::lookup::LookupResult>,
    envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> serde_json::Value {
    let root_entity = phase2_dominant_entity(lookup);
    let linked_entities = phase2_resolved_entities(lookup)
        .into_iter()
        .map(|entity| {
            json!({
                "entity_id": entity["entity_id"].clone(),
                "entity_type": entity["entity_kind"].clone(),
                "current_state": serde_json::Value::Null,
                "relationship": "resolved_from_utterance",
                "link_status": "resolved",
            })
        })
        .collect::<Vec<_>>();

    json!({
        "root_entity": root_entity,
        "root_state": serde_json::Value::Null,
        "linked_entities": linked_entities,
        "structure_links": [],
        "snapshot_ts": envelope.map(|value| value.computed_at.to_rfc3339()),
        "constellation_version": envelope.and_then(|value| value.snapshot_set_id.clone()),
    })
}

#[cfg(feature = "database")]
fn phase2_dag_provenance(
    envelope: Option<&crate::agent::sem_os_context_envelope::SemOsContextEnvelope>,
) -> serde_json::Value {
    match envelope {
        Some(value) => json!({
            "template_id": value
                .grounded_action_surface
                .as_ref()
                .and_then(|surface| surface.resolved_constellation.clone())
                .unwrap_or_else(|| "sem_os_context_envelope".to_string()),
            "template_version": value.snapshot_set_id,
            "fingerprint": value.fingerprint_str(),
            "resolved_subject": value
                .grounded_action_surface
                .as_ref()
                .map(|surface| &surface.resolved_subject),
            "resolved_slot_path": value
                .grounded_action_surface
                .as_ref()
                .and_then(|surface| surface.resolved_slot_path.clone()),
            "resolved_node_id": value
                .grounded_action_surface
                .as_ref()
                .and_then(|surface| surface.resolved_node_id.clone()),
            "resolved_state_machine": value
                .grounded_action_surface
                .as_ref()
                .and_then(|surface| surface.resolved_state_machine.clone()),
            "current_state": value
                .grounded_action_surface
                .as_ref()
                .and_then(|surface| surface.current_state.clone()),
            "traversed_edges": value
                .grounded_action_surface
                .as_ref()
                .map(|surface| {
                    surface
                        .traversed_edges
                        .iter()
                        .map(|edge| json!({
                            "from_type": edge.from_type,
                            "to_type": edge.to_type,
                            "relationship": edge.relationship,
                            "direction": edge.direction,
                            "from_instance": edge.from_instance,
                            "to_instance": edge.to_instance,
                        }))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            "propagation_rules_fired": value
                .grounded_action_surface
                .as_ref()
                .map(|surface| {
                    surface
                        .constraint_signals
                        .iter()
                        .map(|signal| json!({
                            "kind": signal.kind,
                            "slot_path": signal.slot_path,
                            "related_slot": signal.related_slot,
                            "required_state": signal.required_state,
                            "actual_state": signal.actual_state,
                            "message": signal.message,
                        }))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        }),
        None => serde_json::Value::Null,
    }
}

fn phase0_plane_confidence(
    ctx: &crate::sage::SageContext,
    pre: &crate::sage::pre_classify::SagePreClassification,
) -> f64 {
    if pre.sage_only {
        0.95
    } else if ctx.stage_focus.is_some() {
        0.85
    } else {
        0.7
    }
}

fn phase0_polarity_confidence(pre: &crate::sage::pre_classify::SagePreClassification) -> f64 {
    if pre.polarity_clue.is_some() {
        0.9
    } else {
        0.6
    }
}

fn phase0_lexical_signals(pre: &crate::sage::pre_classify::SagePreClassification) -> Vec<String> {
    let mut signals = pre.domain_hints.clone();
    if let Some(clue) = &pre.polarity_clue {
        signals.push(clue.clone());
    }
    signals
}

#[derive(Clone)]
struct TokenSpan {
    start: usize,
    end: usize,
    text: String,
    lower: String,
}

fn token_spans(utterance: &str) -> Vec<TokenSpan> {
    let mut spans = Vec::new();
    let mut start = None;

    for (idx, ch) in utterance.char_indices() {
        let is_token_char = ch.is_alphanumeric() || ch == '@' || ch == '_' || ch == '-';
        match (start, is_token_char) {
            (None, true) => start = Some(idx),
            (Some(token_start), false) => {
                spans.push(make_token_span(utterance, token_start, idx));
                start = None;
            }
            _ => {}
        }
    }

    if let Some(token_start) = start {
        spans.push(make_token_span(utterance, token_start, utterance.len()));
    }

    spans
}

fn make_token_span(utterance: &str, start: usize, end: usize) -> TokenSpan {
    let text = utterance[start..end].to_string();
    TokenSpan {
        start,
        end,
        lower: text.to_ascii_lowercase(),
        text,
    }
}

fn quantifiers(tokens: &[TokenSpan]) -> Vec<serde_json::Value> {
    tokens
        .iter()
        .filter(|token| {
            matches!(
                token.lower.as_str(),
                "all" | "each" | "every" | "both" | "three" | "four" | "five" | "many" | "multiple"
            ) || token.lower.chars().all(|ch| ch.is_ascii_digit())
        })
        .map(|token| {
            json!({
                "phrase": token.text,
                "span": [token.start, token.end]
            })
        })
        .collect()
}

fn noun_phrases(tokens: &[TokenSpan]) -> Vec<serde_json::Value> {
    let mut phrases = Vec::new();
    let mut current: Vec<&TokenSpan> = Vec::new();

    for token in tokens {
        if is_noun_like(&token.lower) {
            current.push(token);
        } else if !current.is_empty() {
            phrases.push(render_phrase(&current));
            current.clear();
        }
    }

    if !current.is_empty() {
        phrases.push(render_phrase(&current));
    }

    phrases
}

fn verb_phrases(tokens: &[TokenSpan]) -> Vec<serde_json::Value> {
    tokens
        .iter()
        .filter(|token| is_verb_like(&token.lower))
        .map(|token| {
            json!({
                "phrase": token.text,
                "span": [token.start, token.end]
            })
        })
        .collect()
}

fn referential_bindings(tokens: &[TokenSpan]) -> Vec<serde_json::Value> {
    tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| {
            matches!(
                token.lower.as_str(),
                "it" | "this" | "that" | "them" | "those" | "these" | "they"
            )
        })
        .map(|(idx, token)| {
            json!({
                "noun_phrase_index": idx,
                "binding_type": "anaphora",
                "resolved_antecedent": serde_json::Value::Null,
                "scope_expression": token.text,
                "confidence": 0.5
            })
        })
        .collect()
}

fn token_map(tokens: &[TokenSpan]) -> Vec<serde_json::Value> {
    tokens
        .iter()
        .map(|token| {
            json!({
                "source_span": [token.start, token.end],
                "target": token_target(&token.lower),
                "confidence": token_confidence(&token.lower)
            })
        })
        .collect()
}

fn render_phrase(tokens: &[&TokenSpan]) -> serde_json::Value {
    let phrase = tokens
        .iter()
        .map(|token| token.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    json!({
        "phrase": phrase,
        "span": [tokens.first().map(|t| t.start).unwrap_or_default(), tokens.last().map(|t| t.end).unwrap_or_default()]
    })
}

fn is_verb_like(token: &str) -> bool {
    matches!(
        token,
        "show"
            | "list"
            | "get"
            | "fetch"
            | "describe"
            | "view"
            | "create"
            | "add"
            | "make"
            | "update"
            | "change"
            | "edit"
            | "delete"
            | "remove"
            | "assign"
            | "link"
            | "approve"
            | "reject"
            | "run"
            | "execute"
            | "switch"
            | "open"
            | "collect"
            | "verify"
    )
}

fn is_noun_like(token: &str) -> bool {
    !matches!(
        token,
        "the"
            | "a"
            | "an"
            | "to"
            | "for"
            | "of"
            | "and"
            | "or"
            | "with"
            | "on"
            | "in"
            | "me"
            | "please"
            | "all"
            | "each"
            | "every"
    ) && !is_verb_like(token)
}

fn token_target(token: &str) -> &'static str {
    if is_verb_like(token) {
        "verb_phrase"
    } else if matches!(
        token,
        "all" | "each" | "every" | "both" | "three" | "four" | "five" | "many" | "multiple"
    ) || token.chars().all(|ch| ch.is_ascii_digit())
    {
        "quantifier"
    } else if matches!(
        token,
        "it" | "this" | "that" | "them" | "those" | "these" | "they"
    ) {
        "referential_binding"
    } else {
        "noun_phrase"
    }
}

fn token_confidence(token: &str) -> f64 {
    if is_verb_like(token) {
        0.85
    } else if matches!(token_target(token), "quantifier" | "referential_binding") {
        0.75
    } else {
        0.65
    }
}

#[cfg(test)]
mod tests {
    use super::build_phase2_trace_payload;
    use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
    use sem_os_core::context_resolution::{
        BlockedActionOption, GroundedActionSurface, GroundedConstraintSignal,
        GroundedTraversalEdge, SubjectRef,
    };
    use uuid::Uuid;

    #[test]
    fn test_phase2_payload_includes_dag_provenance_from_grounded_surface() {
        let mut envelope = SemOsContextEnvelope::test_with_verbs(&["case.open"]);
        envelope.grounded_action_surface = Some(GroundedActionSurface {
            resolved_subject: SubjectRef::TaskId(Uuid::nil()),
            resolved_constellation: Some("constellation.kyc".into()),
            resolved_slot_path: Some("case".into()),
            resolved_node_id: Some("node-1".into()),
            resolved_state_machine: Some("case_machine".into()),
            current_state: Some("intake".into()),
            traversed_edges: vec![GroundedTraversalEdge {
                from_type: "constellation.kyc".into(),
                to_type: "case".into(),
                relationship: "resolved_slot_path".into(),
                direction: "parent_to_child".into(),
                from_instance: Some(Uuid::nil().to_string()),
                to_instance: Some("node-1".into()),
            }],
            constraint_signals: vec![GroundedConstraintSignal {
                kind: "dependency_block".into(),
                slot_path: "case".into(),
                related_slot: Some("cbu".into()),
                required_state: Some("filled".into()),
                actual_state: Some("empty".into()),
                message: "dependency 'cbu' is in state 'empty' but requires 'filled'".into(),
            }],
            valid_actions: vec![],
            blocked_actions: vec![BlockedActionOption {
                action_id: "case.open".into(),
                action_kind: "primitive".into(),
                description: "Blocked action for slot 'case'".into(),
                reasons: vec!["dependency 'cbu' is in state 'empty' but requires 'filled'".into()],
            }],
            dsl_candidates: vec![],
        });

        let payload = build_phase2_trace_payload(None, Some(&envelope));
        assert_eq!(
            payload["dag_provenance"]["template_id"],
            "constellation.kyc"
        );
        assert_eq!(payload["dag_provenance"]["resolved_slot_path"], "case");
        assert_eq!(
            payload["dag_provenance"]["traversed_edges"][0]["to_type"],
            "case"
        );
        assert_eq!(
            payload["dag_provenance"]["propagation_rules_fired"][0]["kind"],
            "dependency_block"
        );
        assert_eq!(payload["legality"]["blocked_action_count"], 1);
        assert_eq!(
            payload["legality"]["blocked_actions"][0]["action_id"],
            "case.open"
        );
        assert_eq!(
            payload["legality"]["constellation_blocks"][0]["blocking_entity"],
            "cbu"
        );
        assert_eq!(
            payload["legality"]["constellation_blocks"][0]["blocked_verb"],
            "case.open"
        );
        assert_eq!(
            payload["legality"]["constellation_blocks"][0]["resolution_hint"],
            "move 'cbu' from 'empty' to at least 'filled'"
        );
    }
}
