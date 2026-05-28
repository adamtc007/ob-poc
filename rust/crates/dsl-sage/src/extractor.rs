//! Parameter extractor — given a matched pack and an utterance, propose values
//! for the pack's declared parameters.
//!
//! # Two strategies
//!
//! 1. **LLM-based** (production): delegates to the [`LlmExtractor`] trait.
//!    Tranche 4 will wire in the real Anthropic client.
//!
//! 2. **Heuristic-based** (fallback / testing): [`HeuristicExtractor`] applies
//!    lightweight pattern matching per parameter type.  Used when no
//!    `llm_client` is provided.
//!
//! # Entry point
//!
//! [`extract_parameters`] is the single public async function.  It selects the
//! active strategy, runs extraction, and wraps the result in a
//! [`ConfirmationRequest`] that includes a placeholder preview DSL string
//! (Tranche 3 will replace the placeholder with real DSL emission).

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use dsl_resolution::{DecisionPack, PackParam, PackRegistry};

use crate::types::{ConfirmationRequest, ParameterProposal, SageContext};

// ---------------------------------------------------------------------------
// LlmExtractor trait
// ---------------------------------------------------------------------------

/// Summary of a pack (name, description, parameters) passed to the LLM prompt.
#[derive(Debug, Clone)]
pub struct PackSummaryWithParams {
    pub name: String,
    pub description: String,
    pub parameters: Vec<PackParam>,
}

/// Async trait for the LLM-based parameter extraction back-end.
///
/// The real implementation (Tranche 4) will POST to the Anthropic API using
/// the following prompt template:
///
/// ```text
/// You are extracting parameters for a workflow decision pack.
///
/// Pack: {name}
/// Description: {description}
/// Parameters:
///   - {name} ({type}, required={required}): {description}
///   ...
///
/// User utterance: "{utterance}"
///
/// Extract values for each parameter from the utterance.
/// Return JSON: {"parameter_name": {"value": ..., "confidence": 0.0-1.0,
///               "rationale": "...", "source_phrase": "..."}}
/// For list parameters, return an array.
/// For node-ref parameters, suggest a kebab-case identifier.
/// For parameters you cannot extract, return
///   {"value": null, "confidence": 0.1, "rationale": "Not mentioned in utterance"}.
/// ```
#[async_trait]
pub trait LlmExtractor: Send + Sync {
    /// Extract parameter proposals from `utterance` for the given `pack`.
    async fn extract_parameters(
        &self,
        utterance: &str,
        pack: &PackSummaryWithParams,
    ) -> Result<Vec<ParameterProposal>>;
}

// ---------------------------------------------------------------------------
// HeuristicExtractor — pure-Rust fallback
// ---------------------------------------------------------------------------

/// Lightweight heuristic extractor; no ML/LLM required.
///
/// Handles the four most common parameter types that appear in the 12 seed
/// packs.  Confidence scores are intentionally low (0.2–0.5) to signal that
/// the proposals are starting points for human review.
pub struct HeuristicExtractor;

impl HeuristicExtractor {
    /// Extract parameter proposals from `utterance` for every parameter in
    /// `pack`.  Always returns exactly `pack.parameters.len()` proposals.
    pub fn extract(utterance: &str, pack: &DecisionPack) -> Vec<ParameterProposal> {
        let lower = utterance.to_lowercase();
        pack.parameters
            .iter()
            .map(|param| Self::extract_one(&lower, utterance, param))
            .collect()
    }

    fn extract_one(lower: &str, original: &str, param: &PackParam) -> ParameterProposal {
        let (value, confidence, rationale, source_phrase) = match param.param_type.as_str() {
            "node-ref" | "symbol" => {
                // Generate a kebab-case identifier from the parameter name.
                // Real LLM would infer a meaningful name from context.
                let default = format!("{}-node", param.name.replace('_', "-"));
                (
                    serde_json::Value::String(default),
                    0.3,
                    "Generated from parameter name (heuristic fallback)".to_string(),
                    None,
                )
            }
            "list-of-condition-expr" => {
                // Extract non-trivial words from the utterance as condition stubs.
                let stop_words = [
                    "must", "should", "when", "then", "and", "all", "the", "for", "are", "is",
                    "to", "be", "in", "of", "a", "an", "this", "that",
                ];
                let terms: Vec<String> = lower
                    .split_whitespace()
                    .filter(|w| w.len() > 4 && !stop_words.contains(w))
                    .take(3)
                    .map(|w| format!("(= {} approved)", w))
                    .collect();
                let source = original[..original.len().min(50)].to_string();
                let val = serde_json::Value::Array(
                    terms
                        .iter()
                        .map(|t| serde_json::Value::String(t.clone()))
                        .collect(),
                );
                (
                    val,
                    0.4,
                    "Extracted keywords from utterance as condition stubs".to_string(),
                    Some(source),
                )
            }
            "list-of-band-spec" | "list-of-map" => {
                // Placeholder — variable-arity list types need human input.
                (
                    serde_json::Value::Array(vec![serde_json::json!({"path": "default-path"})]),
                    0.2,
                    "Placeholder — requires human specification".to_string(),
                    None,
                )
            }
            "string" => {
                let snippet = lower[..lower.len().min(40)].to_string();
                (
                    serde_json::Value::String(snippet),
                    0.3,
                    "Extracted first 40 characters of utterance".to_string(),
                    None,
                )
            }
            "integer" => {
                // Look for the first integer token in the utterance.
                let num = lower
                    .split_whitespace()
                    .find_map(|w| {
                        w.trim_matches(|c: char| !c.is_ascii_digit())
                            .parse::<i64>()
                            .ok()
                    })
                    .unwrap_or(12);
                (
                    serde_json::Value::Number(serde_json::Number::from(num)),
                    0.5,
                    "Extracted first integer found in utterance".to_string(),
                    None,
                )
            }
            "boolean" => {
                // Positive phrasing → true; negative → false; default true.
                let negative = ["none", "no", "false", "never", "not"];
                let val = !negative.iter().any(|w| lower.contains(w));
                (
                    serde_json::Value::Bool(val),
                    0.4,
                    "Inferred from positive/negative phrasing in utterance".to_string(),
                    None,
                )
            }
            _ => {
                // Unknown type — return null with minimal confidence.
                (
                    serde_json::Value::Null,
                    0.1,
                    format!(
                        "Unable to extract unknown parameter type '{}'",
                        param.param_type
                    ),
                    None,
                )
            }
        };

        ParameterProposal {
            parameter_name: param.name.clone(),
            proposed_value: value,
            confidence,
            rationale,
            source_phrase,
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a placeholder preview DSL string showing the pack and parameter names.
///
/// Tranche 3 will replace this with real DSL emission via the instantiator.
fn build_preview_dsl(pack_name: &str, proposals: &[ParameterProposal]) -> String {
    let bindings: Vec<String> = proposals
        .iter()
        .map(|p| {
            let v = match &p.proposed_value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => "null".to_string(),
                other => serde_json::to_string(other).unwrap_or_else(|_| "?".to_string()),
            };
            format!("  :{} {}", p.parameter_name, v)
        })
        .collect();
    format!("(instantiate-pack {}\n{})", pack_name, bindings.join("\n"))
}

/// Extract parameter values from `utterance` for a specific pack.
///
/// Returns a [`ConfirmationRequest`] with proposed values and a preview DSL
/// string.  When `llm_client` is `None` the heuristic extractor is used.
///
/// # Errors
///
/// Returns an error if the pack cannot be found in the registry.
pub async fn extract_parameters(
    utterance: &str,
    pack_name: &str,
    pack_version: &str,
    _context: &SageContext,
    registry: &PackRegistry,
    llm_client: Option<&dyn LlmExtractor>,
) -> Result<ConfirmationRequest> {
    let pack = registry.lookup(pack_name, pack_version).ok_or_else(|| {
        anyhow!(
            "Pack '{}@{}' not found in registry",
            pack_name,
            pack_version
        )
    })?;

    let proposals = if let Some(client) = llm_client {
        let summary = PackSummaryWithParams {
            name: pack.name.clone(),
            description: pack.description.clone(),
            parameters: pack.parameters.clone(),
        };
        client.extract_parameters(utterance, &summary).await?
    } else {
        HeuristicExtractor::extract(utterance, pack)
    };

    let preview_dsl = build_preview_dsl(pack_name, &proposals);

    Ok(ConfirmationRequest {
        pack_name: pack_name.to_string(),
        pack_version: pack_version.to_string(),
        proposed_parameters: proposals,
        preview_dsl,
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_pack(params: Vec<(&str, &str)>) -> DecisionPack {
        DecisionPack {
            name: "test-pack".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            domain_scope: vec![],
            parameters: params
                .into_iter()
                .map(|(name, typ)| PackParam {
                    name: name.to_string(),
                    param_type: typ.to_string(),
                    required: true,
                    description: None,
                    default_value: None,
                })
                .collect(),
            example_utterances: vec![],
            structural_signature: None,
            governance_ref: None,
            template_raw: String::new(),
        }
    }

    #[test]
    fn heuristic_extracts_integer() {
        let pack = mock_pack(vec![("threshold", "integer")]);
        let proposals = HeuristicExtractor::extract("older than 24 months", &pack);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].proposed_value, serde_json::json!(24));
    }

    #[test]
    fn heuristic_extracts_node_ref() {
        let pack = mock_pack(vec![("gate-name", "node-ref")]);
        let proposals = HeuristicExtractor::extract("route to approver", &pack);
        assert_eq!(proposals.len(), 1);
        assert!(proposals[0].proposed_value.is_string());
    }

    #[test]
    fn heuristic_extracts_boolean_positive() {
        let pack = mock_pack(vec![("all-required", "boolean")]);
        let proposals = HeuristicExtractor::extract("all conditions must be met", &pack);
        assert_eq!(proposals[0].proposed_value, serde_json::Value::Bool(true));
    }

    #[test]
    fn heuristic_null_for_unknown_type() {
        let pack = mock_pack(vec![("x", "exotic-type")]);
        let proposals = HeuristicExtractor::extract("anything", &pack);
        assert_eq!(proposals[0].proposed_value, serde_json::Value::Null);
        assert!(proposals[0].confidence < 0.2);
    }

    #[test]
    fn build_preview_dsl_round_trip() {
        let proposals = vec![ParameterProposal {
            parameter_name: "gate-name".to_string(),
            proposed_value: serde_json::Value::String("kyc-gate".to_string()),
            confidence: 0.9,
            rationale: "test".to_string(),
            source_phrase: None,
        }];
        let preview = build_preview_dsl("conjunctive-gate", &proposals);
        assert!(preview.contains("conjunctive-gate"));
        assert!(preview.contains(":gate-name"));
        assert!(preview.contains("kyc-gate"));
    }
}
