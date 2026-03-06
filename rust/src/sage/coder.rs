//! Coder engine — deterministic OutcomeIntent -> verb + DSL resolution.

use anyhow::{anyhow, Result};
use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{VerbConfig, VerbsConfig};

use crate::mcp::intent_pipeline::IntentArgValue;

use super::arg_assembly::structured_intent_from_step;
use super::outcome::{OutcomeIntent, OutcomeStep};
use super::verb_index::VerbMetadataIndex;
use super::verb_resolve::{ScoredVerbCandidate, StructuredVerbScorer};

/// Resolution state for the Coder output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoderResolution {
    Confident,
    Proposed,
    NeedsInput,
}

/// End-to-end Coder output.
#[derive(Debug, Clone)]
pub struct CoderResult {
    pub verb_fqn: String,
    pub dsl: String,
    pub resolution: CoderResolution,
    pub missing_args: Vec<String>,
    pub unresolved_refs: Vec<String>,
}

/// Deterministic Coder engine over verb metadata and config.
#[derive(Debug, Clone)]
pub struct CoderEngine {
    verb_index: VerbMetadataIndex,
    scorer: StructuredVerbScorer,
    config: VerbsConfig,
}

impl CoderEngine {
    /// Build a coder engine from loaded verb config.
    ///
    /// # Examples
    /// ```ignore
    /// use dsl_core::config::loader::ConfigLoader;
    /// use ob_poc::sage::coder::CoderEngine;
    ///
    /// let config = ConfigLoader::from_env().load_verbs()?;
    /// let engine = CoderEngine::from_config(config);
    /// assert!(engine.verb_index().len() > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn from_config(config: VerbsConfig) -> Self {
        let verb_index = VerbMetadataIndex::from_config(&config);
        let scorer = StructuredVerbScorer::new(verb_index.clone());
        Self {
            verb_index,
            scorer,
            config,
        }
    }

    /// Load the coder engine from the default config loader.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::coder::CoderEngine;
    ///
    /// let engine = CoderEngine::load()?;
    /// assert!(engine.verb_index().len() > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load() -> Result<Self> {
        let config = ConfigLoader::from_env().load_verbs()?;
        Ok(Self::from_config(config))
    }

    /// Resolve an `OutcomeIntent` into a verb and DSL.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::{IntentPolarity, ObservationPlane, OutcomeAction, OutcomeIntent, SageConfidence};
    /// use ob_poc::sage::coder::CoderEngine;
    ///
    /// let engine = CoderEngine::load()?;
    /// let outcome = OutcomeIntent {
    ///     summary: "List CBUs".to_string(),
    ///     plane: ObservationPlane::Instance,
    ///     polarity: IntentPolarity::Read,
    ///     domain_concept: "cbu".to_string(),
    ///     action: OutcomeAction::Read,
    ///     subject: None,
    ///     steps: vec![],
    ///     confidence: SageConfidence::Low,
    ///     pending_clarifications: vec![],
    /// };
    /// let result = engine.resolve(&outcome)?;
    /// assert!(!result.verb_fqn.is_empty());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn resolve(&self, outcome: &OutcomeIntent) -> Result<CoderResult> {
        if let Some(result) = self.try_structure_read_describe(outcome)? {
            return Ok(result);
        }

        let candidates = self.scorer.score(outcome, 5);
        let top = candidates
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("no coder candidates for outcome"))?;
        self.resolve_candidate(outcome, &top)
    }

    /// Access the underlying verb index.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::coder::CoderEngine;
    ///
    /// let engine = CoderEngine::load()?;
    /// assert!(engine.verb_index().len() > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn verb_index(&self) -> &VerbMetadataIndex {
        &self.verb_index
    }

    fn try_structure_read_describe(&self, outcome: &OutcomeIntent) -> Result<Option<CoderResult>> {
        if outcome.plane != super::ObservationPlane::Structure
            || outcome.polarity != super::IntentPolarity::Read
            || outcome.domain_concept.is_empty()
        {
            return Ok(None);
        }

        if self.verb_index.get("schema.entity.describe").is_none() {
            return Ok(None);
        }

        let candidate = ScoredVerbCandidate {
            fqn: "schema.entity.describe".to_string(),
            score: 1.0,
            action_score: 1.0,
            param_overlap_score: 1.0,
        };
        self.resolve_candidate(outcome, &candidate).map(Some)
    }

    fn resolve_candidate(
        &self,
        outcome: &OutcomeIntent,
        candidate: &ScoredVerbCandidate,
    ) -> Result<CoderResult> {
        let step = primary_step(outcome);
        let config = self.verb_config(&candidate.fqn)?;
        let structured = structured_intent_from_step(&candidate.fqn, &step, config)?;

        let missing_args = structured
            .arguments
            .iter()
            .filter_map(|arg| match &arg.value {
                IntentArgValue::Missing { arg_name } => Some(arg_name.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let unresolved_refs = structured
            .arguments
            .iter()
            .filter_map(|arg| match &arg.value {
                IntentArgValue::Unresolved { value, .. } => Some(format!("{}={}", arg.name, value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let dsl = crate::mcp::intent_pipeline::assemble_dsl_string(&structured)?;

        let resolution =
            if candidate.score >= 0.75 && missing_args.is_empty() && unresolved_refs.is_empty() {
                CoderResolution::Confident
            } else if !candidate.fqn.is_empty() {
                CoderResolution::Proposed
            } else {
                CoderResolution::NeedsInput
            };

        Ok(CoderResult {
            verb_fqn: candidate.fqn.clone(),
            dsl,
            resolution,
            missing_args,
            unresolved_refs,
        })
    }

    fn verb_config(&self, fqn: &str) -> Result<&VerbConfig> {
        let (domain, verb_name) = fqn
            .split_once('.')
            .ok_or_else(|| anyhow!("invalid verb fqn '{fqn}'"))?;
        self.config
            .domains
            .get(domain)
            .and_then(|domain_cfg| domain_cfg.verbs.get(verb_name))
            .ok_or_else(|| anyhow!("missing verb config for '{fqn}'"))
    }
}

fn primary_step(outcome: &OutcomeIntent) -> OutcomeStep {
    outcome
        .steps
        .first()
        .cloned()
        .unwrap_or_else(|| OutcomeStep {
            action: outcome.action.clone(),
            target: if outcome.domain_concept.is_empty() {
                String::new()
            } else {
                outcome.domain_concept.clone()
            },
            params: Default::default(),
            notes: None,
        })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dsl_core::config::types::{ArgConfig, ArgType, DomainConfig, VerbBehavior};

    use super::*;
    use crate::sage::{IntentPolarity, ObservationPlane, OutcomeAction, SageConfidence};

    fn sample_config() -> VerbsConfig {
        let mut domains = HashMap::new();
        let mut verbs = HashMap::new();
        verbs.insert(
            "create".to_string(),
            VerbConfig {
                description: "Create a CBU".to_string(),
                behavior: VerbBehavior::Plugin,
                crud: None,
                handler: Some("cbu.create".to_string()),
                graph_query: None,
                durable: None,
                args: vec![ArgConfig {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                    maps_to: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    description: None,
                    validation: None,
                    fuzzy_check: None,
                    slot_type: None,
                    preferred_roles: vec![],
                }],
                returns: None,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                metadata: None,
                invocation_phrases: vec![],
                policy: None,
                sentences: None,
                confirm_policy: None,
            },
        );
        domains.insert(
            "cbu".to_string(),
            DomainConfig {
                description: "CBU".to_string(),
                verbs,
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );
        VerbsConfig {
            version: "1.0".to_string(),
            domains,
        }
    }

    fn sample_outcome() -> OutcomeIntent {
        OutcomeIntent {
            summary: "Create a CBU".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Write,
            domain_concept: "cbu".to_string(),
            action: OutcomeAction::Create,
            subject: None,
            steps: vec![OutcomeStep {
                action: OutcomeAction::Create,
                target: "cbu".to_string(),
                params: HashMap::from([(String::from("name"), String::from("Apex Fund"))]),
                notes: None,
            }],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
        }
    }

    #[test]
    fn structure_read_prefers_schema_entity_describe() {
        let engine = CoderEngine::load().unwrap();
        let outcome = OutcomeIntent {
            summary: "Describe entity schema for document with fields relationships and verbs"
                .to_string(),
            plane: ObservationPlane::Structure,
            polarity: IntentPolarity::Read,
            domain_concept: "document".to_string(),
            action: OutcomeAction::Read,
            subject: None,
            steps: vec![OutcomeStep {
                action: OutcomeAction::Read,
                target: "document".to_string(),
                params: HashMap::from([("entity-type".to_string(), "document".to_string())]),
                notes: None,
            }],
            confidence: SageConfidence::High,
            pending_clarifications: vec![],
        };

        let result = engine.resolve(&outcome).unwrap();
        assert_eq!(result.verb_fqn, "schema.entity.describe");
        assert_eq!(
            result.dsl,
            "(schema.entity.describe :entity-type \"document\")"
        );
    }

    #[test]
    fn coder_resolves_confident_result() {
        let engine = CoderEngine::from_config(sample_config());
        let result = engine.resolve(&sample_outcome()).unwrap();
        assert_eq!(result.verb_fqn, "cbu.create");
        assert_eq!(result.resolution, CoderResolution::Confident);
        assert_eq!(result.dsl, "(cbu.create :name \"Apex Fund\")");
    }

    #[test]
    fn coder_marks_missing_args_as_proposed() {
        let mut outcome = sample_outcome();
        outcome.steps[0].params.clear();
        let engine = CoderEngine::from_config(sample_config());
        let result = engine.resolve(&outcome).unwrap();
        assert_eq!(result.resolution, CoderResolution::Proposed);
        assert_eq!(result.missing_args, vec!["name".to_string()]);
    }
}
