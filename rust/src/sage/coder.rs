//! Coder engine — deterministic OutcomeIntent -> verb + DSL resolution.

use anyhow::{anyhow, Result};
use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{HarmClass, VerbConfig, VerbsConfig};
use serde::{Deserialize, Serialize};

use crate::mcp::intent_pipeline::IntentArgValue;

use super::arg_assembly::structured_intent_from_step;
use super::outcome::{OutcomeIntent, OutcomeStep};
use super::verb_index::VerbMetadataIndex;
use super::verb_resolve::{FilterDiagnostics, ScoredVerbCandidate, StructuredVerbScorer};

/// Resolution state for the Coder output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoderResolution {
    Confident,
    Proposed,
    NeedsInput,
}

/// Explicit failure reason when deterministic Coder resolution cannot proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoderFailureKind {
    NoCandidateAfterFilters,
    DomainConflict,
    PhaseConflict,
    SubjectKindConflict,
    ActionConflict,
    BelowThreshold,
    PolicyConflict,
}

/// Diagnostics explaining how Coder resolution succeeded or failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderDiagnostics {
    pub failure_kind: Option<CoderFailureKind>,
    pub filter_diagnostics: CoderFilterDiagnostics,
    pub top_candidate: Option<String>,
    pub top_score: Option<f32>,
    pub threshold: Option<f32>,
}

/// Serializable copy of scorer filter counts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CoderFilterDiagnostics {
    pub base_candidates: usize,
    pub domain_candidates: usize,
    pub phase_candidates: usize,
    pub subject_kind_candidates: usize,
    pub final_candidates: usize,
}

impl From<FilterDiagnostics> for CoderFilterDiagnostics {
    fn from(value: FilterDiagnostics) -> Self {
        Self {
            base_candidates: value.base_candidates,
            domain_candidates: value.domain_candidates,
            phase_candidates: value.phase_candidates,
            subject_kind_candidates: value.subject_kind_candidates,
            final_candidates: value.final_candidates,
        }
    }
}

/// End-to-end Coder output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderResult {
    pub verb_fqn: String,
    pub dsl: String,
    pub resolution: CoderResolution,
    pub missing_args: Vec<String>,
    pub unresolved_refs: Vec<String>,
    pub diagnostics: Option<CoderDiagnostics>,
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

        let filter_diagnostics = self.scorer.diagnose_filters(outcome);
        let candidates = self.scorer.score(outcome, 5);
        let step = primary_step(outcome);
        let threshold = acceptance_threshold(&step);
        let top = candidates.first().cloned().ok_or_else(|| {
            self.failure_error(outcome, None, threshold, filter_diagnostics, None)
        })?;
        let verb_required = self
            .verb_index
            .get(&top.fqn)
            .map(|meta| meta.required_params.len())
            .unwrap_or_default();

        tracing::debug!(
            domain_concept = %outcome.domain_concept,
            action = ?outcome.action,
            plane = ?outcome.plane,
            polarity = ?outcome.polarity,
            candidate_count = candidates.len(),
            top_candidate = %top.fqn,
            top_score = top.score,
            param_overlap_score = top.param_overlap_score,
            step_param_count = step.params.len(),
            verb_required_count = verb_required,
            "Coder resolve"
        );

        if top.score < threshold {
            return Err(self.failure_error(
                outcome,
                Some(&top),
                threshold,
                filter_diagnostics,
                Some(CoderFailureKind::BelowThreshold),
            ));
        }

        if !self.is_candidate_allowed(outcome, &top.fqn) {
            return Err(self.failure_error(
                outcome,
                Some(&top),
                threshold,
                filter_diagnostics,
                Some(CoderFailureKind::PolicyConflict),
            ));
        }
        self.resolve_candidate(
            outcome,
            &top,
            Some(CoderDiagnostics {
                failure_kind: None,
                filter_diagnostics: filter_diagnostics.into(),
                top_candidate: Some(top.fqn.clone()),
                top_score: Some(top.score),
                threshold: Some(threshold),
            }),
        )
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
        self.resolve_candidate(outcome, &candidate, None).map(Some)
    }

    fn resolve_candidate(
        &self,
        outcome: &OutcomeIntent,
        candidate: &ScoredVerbCandidate,
        diagnostics: Option<CoderDiagnostics>,
    ) -> Result<CoderResult> {
        let step = primary_step(outcome);
        let config = self.verb_config(&candidate.fqn)?;
        let structured = structured_intent_from_step(&candidate.fqn, &step, config)?;

        let required_args = config
            .args
            .iter()
            .filter(|arg| arg.required)
            .map(|arg| arg.name.as_str())
            .collect::<std::collections::HashSet<_>>();
        let missing_args = structured
            .arguments
            .iter()
            .filter_map(|arg| match &arg.value {
                IntentArgValue::Missing { arg_name }
                    if required_args.contains(arg_name.as_str()) =>
                {
                    Some(arg_name.clone())
                }
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
            diagnostics,
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

    fn is_candidate_allowed(&self, outcome: &OutcomeIntent, fqn: &str) -> bool {
        let Some(meta) = self.verb_index.get(fqn) else {
            return false;
        };
        match outcome.polarity {
            super::IntentPolarity::Read | super::IntentPolarity::Ambiguous => {
                meta.harm_class == HarmClass::ReadOnly
            }
            super::IntentPolarity::Write => meta.harm_class != HarmClass::ReadOnly,
        }
    }

    fn failure_error(
        &self,
        _outcome: &OutcomeIntent,
        top: Option<&ScoredVerbCandidate>,
        threshold: f32,
        filter_diagnostics: FilterDiagnostics,
        override_kind: Option<CoderFailureKind>,
    ) -> anyhow::Error {
        let failure_kind = override_kind
            .unwrap_or_else(|| classify_failure_kind(filter_diagnostics, top, threshold));
        let diagnostics = CoderDiagnostics {
            failure_kind: Some(failure_kind),
            filter_diagnostics: filter_diagnostics.into(),
            top_candidate: top.map(|candidate| candidate.fqn.clone()),
            top_score: top.map(|candidate| candidate.score),
            threshold: Some(threshold),
        };
        anyhow!(
            "coder resolution failed: kind={:?}, top_candidate={}, top_score={}, threshold={:.3}, filters=base:{} domain:{} phase:{} subject:{} final:{}",
            failure_kind,
            diagnostics.top_candidate.as_deref().unwrap_or("<none>"),
            diagnostics
                .top_score
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "<none>".to_string()),
            threshold,
            diagnostics.filter_diagnostics.base_candidates,
            diagnostics.filter_diagnostics.domain_candidates,
            diagnostics.filter_diagnostics.phase_candidates,
            diagnostics.filter_diagnostics.subject_kind_candidates,
            diagnostics.filter_diagnostics.final_candidates,
        )
    }
}

fn classify_failure_kind(
    filter_diagnostics: FilterDiagnostics,
    top: Option<&ScoredVerbCandidate>,
    threshold: f32,
) -> CoderFailureKind {
    if filter_diagnostics.base_candidates == 0 || filter_diagnostics.final_candidates == 0 {
        if filter_diagnostics.domain_candidates == 0 && filter_diagnostics.base_candidates > 0 {
            return CoderFailureKind::DomainConflict;
        }
        if filter_diagnostics.phase_candidates == 0 && filter_diagnostics.domain_candidates > 0 {
            return CoderFailureKind::PhaseConflict;
        }
        if filter_diagnostics.subject_kind_candidates == 0
            && filter_diagnostics.phase_candidates > 0
        {
            return CoderFailureKind::SubjectKindConflict;
        }
        return CoderFailureKind::NoCandidateAfterFilters;
    }

    if let Some(top) = top {
        if top.score < threshold {
            if top.action_score < 0.5 {
                return CoderFailureKind::ActionConflict;
            }
            return CoderFailureKind::BelowThreshold;
        }
    }

    CoderFailureKind::NoCandidateAfterFilters
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

fn acceptance_threshold(step: &OutcomeStep) -> f32 {
    if step.params.is_empty() {
        0.25
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dsl_core::config::types::{
        ActionClass, ArgConfig, ArgType, DomainConfig, HarmClass, VerbBehavior, VerbMetadata,
    };

    use super::*;
    use crate::sage::{
        CoderHandoff, IntentPolarity, ObservationPlane, OutcomeAction, SageConfidence, SageExplain,
        UtteranceHints,
    };

    fn sample_config() -> VerbsConfig {
        let mut domains = HashMap::new();
        let mut verbs = HashMap::new();
        verbs.insert(
            "create".to_string(),
            VerbConfig {
                description: "Create a deal".to_string(),
                behavior: VerbBehavior::Plugin,
                crud: None,
                handler: Some("deal.create".to_string()),
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
                outputs: vec![],
                three_axis: None,
                transition_args: None,
            },
        );
        domains.insert(
            "deal".to_string(),
            DomainConfig {
                description: "Deal".to_string(),
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
            summary: "Create a deal".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Write,
            domain_concept: "deal".to_string(),
            action: OutcomeAction::Create,
            subject: None,
            steps: vec![OutcomeStep {
                action: OutcomeAction::Create,
                target: "deal".to_string(),
                params: HashMap::from([(String::from("name"), String::from("Apex Deal"))]),
                notes: None,
            }],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        }
    }

    fn sample_read_config_with_metadata(metadata: VerbMetadata) -> VerbsConfig {
        sample_read_config_named("list", "List deals", metadata)
    }

    fn sample_read_config_named(
        verb_name: &str,
        description: &str,
        metadata: VerbMetadata,
    ) -> VerbsConfig {
        let mut domains = HashMap::new();
        let mut verbs = HashMap::new();
        verbs.insert(
            verb_name.to_string(),
            VerbConfig {
                description: description.to_string(),
                behavior: VerbBehavior::Plugin,
                crud: None,
                handler: Some(format!("deal.{verb_name}")),
                graph_query: None,
                durable: None,
                args: vec![],
                returns: None,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                metadata: Some(metadata),
                invocation_phrases: vec![],
                policy: None,
                sentences: None,
                confirm_policy: None,
                outputs: vec![],
                three_axis: None,
                transition_args: None,
            },
        );
        domains.insert(
            "deal".to_string(),
            DomainConfig {
                description: "Deal".to_string(),
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
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
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
        assert_eq!(result.verb_fqn, "deal.create");
        assert_eq!(result.resolution, CoderResolution::Confident);
        assert_eq!(result.dsl, "(deal.create :name \"Apex Deal\")");
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

    #[test]
    fn coder_reports_phase_conflict_when_candidates_are_filtered_by_stage() {
        let engine = CoderEngine::from_config(sample_read_config_with_metadata(VerbMetadata {
            side_effects: Some("facts_only".to_string()),
            harm_class: Some(HarmClass::ReadOnly),
            action_class: Some(ActionClass::List),
            phase_tags: vec!["kyc".to_string()],
            ..VerbMetadata::default()
        }));
        let outcome = OutcomeIntent {
            summary: "show me the deals".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Read,
            domain_concept: "deal".to_string(),
            action: OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: UtteranceHints {
                stage_focus: Some("onboarding".to_string()),
                ..UtteranceHints::default()
            },
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };

        let error = engine.resolve(&outcome).unwrap_err().to_string();
        assert!(error.contains("kind=PhaseConflict"));
    }

    #[test]
    fn coder_reports_subject_kind_conflict_when_candidates_are_filtered_by_kind() {
        let engine = CoderEngine::from_config(sample_read_config_with_metadata(VerbMetadata {
            side_effects: Some("facts_only".to_string()),
            harm_class: Some(HarmClass::ReadOnly),
            action_class: Some(ActionClass::List),
            subject_kinds: vec!["fund".to_string()],
            ..VerbMetadata::default()
        }));
        let outcome = OutcomeIntent {
            summary: "show me the deals".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Read,
            domain_concept: "deal".to_string(),
            action: OutcomeAction::Read,
            subject: Some(crate::sage::EntityRef {
                mention: "this cbu".to_string(),
                kind_hint: Some("cbu".to_string()),
                uuid: None,
            }),
            steps: vec![],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };

        let error = engine.resolve(&outcome).unwrap_err().to_string();
        assert!(error.contains("kind=SubjectKindConflict"));
    }

    #[test]
    fn coder_reports_action_conflict_when_top_candidate_is_below_threshold() {
        let engine = CoderEngine::from_config(sample_read_config_named(
            "status",
            "Read current status",
            VerbMetadata {
                side_effects: Some("facts_only".to_string()),
                harm_class: Some(HarmClass::ReadOnly),
                action_class: Some(ActionClass::Read),
                ..VerbMetadata::default()
            },
        ));
        let outcome = OutcomeIntent {
            summary: "publish the portfolio".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Read,
            domain_concept: String::new(),
            action: OutcomeAction::Publish,
            subject: None,
            steps: vec![OutcomeStep {
                action: OutcomeAction::Publish,
                target: "portfolio".to_string(),
                params: HashMap::from([("target-id".to_string(), "123".to_string())]),
                notes: None,
            }],
            confidence: SageConfidence::Low,
            pending_clarifications: vec![],
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };

        let error = engine.resolve(&outcome).unwrap_err().to_string();
        assert!(error.contains("kind=ActionConflict") || error.contains("kind=BelowThreshold"));
    }
}
