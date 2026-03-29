//! Structured verb scoring for the Coder layer.
//!
//! This scorer never reads natural-language utterances directly. It consumes
//! `OutcomeIntent` plus precomputed `VerbMetadataIndex` rows and ranks verbs
//! using deterministic metadata.

use std::collections::HashSet;

use dsl_core::config::types::ActionClass;

use crate::entity_kind::matches as entity_kind_matches;

use super::outcome::OutcomeIntent;
use super::polarity::IntentPolarity;
use super::verb_index::{VerbMeta, VerbMetadataIndex};

/// Ranked candidate returned by the structured scorer.
#[derive(Debug, Clone)]
pub struct ScoredVerbCandidate {
    pub fqn: String,
    pub score: f32,
    pub action_score: f32,
    pub param_overlap_score: f32,
}

/// Candidate counts after each deterministic filter stage.
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterDiagnostics {
    pub base_candidates: usize,
    pub domain_candidates: usize,
    pub phase_candidates: usize,
    pub subject_kind_candidates: usize,
    pub final_candidates: usize,
}

/// Deterministic metadata-based verb scorer.
#[derive(Debug, Clone)]
pub struct StructuredVerbScorer {
    index: VerbMetadataIndex,
}

impl StructuredVerbScorer {
    /// Create a scorer from a metadata index.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    /// use ob_poc::sage::verb_resolve::StructuredVerbScorer;
    ///
    /// let index = VerbMetadataIndex::load()?;
    /// let scorer = StructuredVerbScorer::new(index);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(index: VerbMetadataIndex) -> Self {
        Self { index }
    }

    /// Score verbs for an outcome intent and return the top `limit` candidates.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::{IntentPolarity, ObservationPlane, OutcomeAction, OutcomeIntent, SageConfidence};
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    /// use ob_poc::sage::verb_resolve::StructuredVerbScorer;
    ///
    /// let scorer = StructuredVerbScorer::new(VerbMetadataIndex::load()?);
    /// let intent = OutcomeIntent {
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
    /// let candidates = scorer.score(&intent, 3);
    /// assert!(!candidates.is_empty());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn score(&self, intent: &OutcomeIntent, limit: usize) -> Vec<ScoredVerbCandidate> {
        let requested_action = normalized_action_tags(intent);
        let desired_action_classes = desired_action_classes(intent);
        let requested_params = requested_param_keys(intent);
        let intent_keywords = intent_keywords(intent);
        let (metas, _) = self.candidates_for_intent(intent);

        let mut candidates = metas
            .into_iter()
            .map(|meta| {
                let action_score = action_score(
                    meta,
                    &requested_action,
                    &desired_action_classes,
                    &intent_keywords,
                );
                let param_overlap_score = param_overlap_score(meta, &requested_params);
                let inventory_bias = inventory_read_bias(meta, intent);
                let score = 0.6 * action_score + 0.4 * param_overlap_score + inventory_bias;
                ScoredVerbCandidate {
                    fqn: meta.fqn.clone(),
                    score,
                    action_score,
                    param_overlap_score,
                }
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.fqn.cmp(&b.fqn))
        });
        candidates.truncate(limit);
        candidates
    }

    /// Access the underlying metadata index.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    /// use ob_poc::sage::verb_resolve::StructuredVerbScorer;
    ///
    /// let scorer = StructuredVerbScorer::new(VerbMetadataIndex::load()?);
    /// assert!(scorer.index().len() > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn index(&self) -> &VerbMetadataIndex {
        &self.index
    }

    /// Diagnose how many candidates survive each pre-score filter stage.
    ///
    /// # Examples
    /// ```ignore
    /// use ob_poc::sage::verb_index::VerbMetadataIndex;
    /// use ob_poc::sage::verb_resolve::StructuredVerbScorer;
    ///
    /// let scorer = StructuredVerbScorer::new(VerbMetadataIndex::load()?);
    /// // diagnostics shape depends on the intent provided at runtime
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn diagnose_filters(&self, intent: &OutcomeIntent) -> FilterDiagnostics {
        let (_, diagnostics) = self.candidates_for_intent(intent);
        diagnostics
    }

    fn candidates_for_intent<'a>(
        &'a self,
        intent: &OutcomeIntent,
    ) -> (Vec<&'a VerbMeta>, FilterDiagnostics) {
        let domain_hint =
            (!intent.domain_concept.trim().is_empty()).then_some(intent.domain_concept.as_str());
        let strict_filtered = self.query_by_harm_class(intent, domain_hint, true);
        if !strict_filtered.is_empty() {
            let diagnostics = self.diagnose_filter_chain(intent, domain_hint);
            return (strict_filtered, diagnostics);
        }

        let filtered = self.query_by_harm_class(intent, domain_hint, false);
        if filtered.is_empty() && domain_hint.is_some() {
            let diagnostics = self.diagnose_filter_chain(intent, None);
            (self.query_by_harm_class(intent, None, false), diagnostics)
        } else {
            let diagnostics = self.diagnose_filter_chain(intent, domain_hint);
            (filtered, diagnostics)
        }
    }

    fn query_by_harm_class<'a>(
        &'a self,
        intent: &OutcomeIntent,
        domain_hint: Option<&str>,
        strict_domain: bool,
    ) -> Vec<&'a VerbMeta> {
        let filter_domain =
            |meta: &&VerbMeta| self.matches_domain_filter(intent, meta, domain_hint, strict_domain);

        match intent.polarity {
            IntentPolarity::Read | IntentPolarity::Ambiguous => self
                .index
                .read_only_verbs()
                .filter(|meta| meta.planes.contains(&intent.plane))
                .filter(filter_domain)
                .filter(|meta| candidate_matches_context(meta, intent))
                .collect(),
            IntentPolarity::Write => self
                .index
                .mutating_verbs()
                .filter(|meta| meta.planes.contains(&intent.plane))
                .filter(filter_domain)
                .filter(|meta| candidate_matches_context(meta, intent))
                .collect(),
        }
    }

    fn diagnose_filter_chain(
        &self,
        intent: &OutcomeIntent,
        domain_hint: Option<&str>,
    ) -> FilterDiagnostics {
        let base = self.base_candidates(intent);
        let domain = base
            .iter()
            .copied()
            .filter(|meta| self.matches_domain_filter(intent, meta, domain_hint, true))
            .collect::<Vec<_>>();
        let phase = domain
            .iter()
            .copied()
            .filter(|meta| phase_tags_match(meta, intent))
            .collect::<Vec<_>>();
        let subject_kind = phase
            .iter()
            .copied()
            .filter(|meta| subject_kinds_match(meta, intent))
            .collect::<Vec<_>>();

        FilterDiagnostics {
            base_candidates: base.len(),
            domain_candidates: domain.len(),
            phase_candidates: phase.len(),
            subject_kind_candidates: subject_kind.len(),
            final_candidates: subject_kind.len(),
        }
    }

    fn base_candidates<'a>(&'a self, intent: &OutcomeIntent) -> Vec<&'a VerbMeta> {
        let by_harm: Vec<&VerbMeta> = match intent.polarity {
            IntentPolarity::Read | IntentPolarity::Ambiguous => {
                self.index.read_only_verbs().collect()
            }
            IntentPolarity::Write => self.index.mutating_verbs().collect(),
        };

        by_harm
            .into_iter()
            .filter(|meta| meta.planes.contains(&intent.plane))
            .collect()
    }

    fn matches_domain_filter(
        &self,
        intent: &OutcomeIntent,
        meta: &VerbMeta,
        domain_hint: Option<&str>,
        strict_domain: bool,
    ) -> bool {
        match domain_hint {
            Some(hint) if strict_domain => matches_strict_domain_hint(meta, hint),
            Some(hint) => self
                .index
                .query(intent.plane, intent.polarity, Some(hint))
                .iter()
                .any(|candidate| candidate.fqn == meta.fqn),
            None => true,
        }
    }
}

fn matches_strict_domain_hint(meta: &VerbMeta, hint: &str) -> bool {
    let hint = hint.trim().to_ascii_lowercase();
    if hint.is_empty() || hint == "unknown" {
        return true;
    }

    let domain = meta.domain.to_ascii_lowercase();
    domain == hint || domain.starts_with(&hint) || hint.starts_with(&domain)
}

fn candidate_matches_context(meta: &VerbMeta, intent: &OutcomeIntent) -> bool {
    phase_tags_match(meta, intent) && subject_kinds_match(meta, intent)
}

fn phase_tags_match(meta: &VerbMeta, intent: &OutcomeIntent) -> bool {
    if meta.phase_tags.is_empty() {
        return true;
    }

    let active_tags = active_phase_tags(intent);
    if active_tags.is_empty() {
        return true;
    }

    meta.phase_tags.iter().any(|tag| {
        let tag = tag.to_ascii_lowercase();
        active_tags.iter().any(|active| active == &tag)
    })
}

fn active_phase_tags(intent: &OutcomeIntent) -> HashSet<String> {
    let mut tags = HashSet::new();
    if let Some(stage_focus) = intent.hints.stage_focus.as_deref() {
        let normalized = stage_focus.to_ascii_lowercase();
        if normalized.contains("kyc") {
            tags.insert("kyc".to_string());
        }
        if normalized.contains("data-management") || normalized.contains("semos-data") {
            tags.insert("data-management".to_string());
            tags.insert("data".to_string());
        }
        if normalized.contains("stewardship") {
            tags.insert("stewardship".to_string());
        }
        if normalized.contains("onboarding") {
            tags.insert("onboarding".to_string());
        }
        if normalized.contains("trading") {
            tags.insert("trading".to_string());
        }
        if normalized.contains("navigation") {
            tags.insert("navigation".to_string());
        }
    }

    for goal in intent
        .coder_handoff
        .constraints
        .iter()
        .chain(intent.hints.explicit_domain_terms.iter())
    {
        let normalized = goal.to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "kyc"
                | "stewardship"
                | "data"
                | "data-management"
                | "onboarding"
                | "trading"
                | "navigation"
        ) {
            tags.insert(normalized);
        }
    }

    tags
}

fn subject_kinds_match(meta: &VerbMeta, intent: &OutcomeIntent) -> bool {
    if meta.subject_kinds.is_empty() {
        return true;
    }

    let subject_kind = intent
        .subject
        .as_ref()
        .and_then(|subject| subject.kind_hint.as_deref())
        .or(intent.hints.entity_kind.as_deref());
    let Some(subject_kind) = subject_kind else {
        return true;
    };

    meta.subject_kinds
        .iter()
        .any(|kind| entity_kind_matches(kind, subject_kind))
}

fn normalized_action_tags(intent: &OutcomeIntent) -> HashSet<String> {
    let mut tags = HashSet::new();
    tags.insert(intent.action.as_str().to_string());
    if matches!(
        intent.polarity,
        IntentPolarity::Read | IntentPolarity::Ambiguous
    ) && summary_suggests_collection(intent)
    {
        tags.insert("list".to_string());
    }
    if !intent.domain_concept.is_empty() {
        tags.insert(intent.domain_concept.clone());
    }
    for step in &intent.steps {
        tags.insert(step.action.as_str().to_string());
        if !step.target.is_empty() {
            tags.insert(step.target.clone());
        }
    }
    tags
}

fn desired_action_classes(intent: &OutcomeIntent) -> HashSet<ActionClass> {
    let mut classes = HashSet::from([match intent.action {
        super::OutcomeAction::Read => {
            if summary_suggests_collection(intent) {
                ActionClass::List
            } else {
                ActionClass::Read
            }
        }
        super::OutcomeAction::Create => ActionClass::Create,
        super::OutcomeAction::Update => ActionClass::Update,
        super::OutcomeAction::Delete => ActionClass::Delete,
        super::OutcomeAction::Assign => ActionClass::Assign,
        super::OutcomeAction::Import => ActionClass::Import,
        super::OutcomeAction::Search => ActionClass::Search,
        super::OutcomeAction::Compute => ActionClass::Compute,
        super::OutcomeAction::Publish => ActionClass::Approve,
        super::OutcomeAction::Other(_) => ActionClass::Read,
    }]);

    if matches!(
        intent.polarity,
        IntentPolarity::Read | IntentPolarity::Ambiguous
    ) && summary_suggests_collection(intent)
    {
        classes.insert(ActionClass::List);
    }

    classes
}

fn summary_suggests_collection(intent: &OutcomeIntent) -> bool {
    let normalized = intent.summary.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let domain = intent.domain_concept.trim().to_ascii_lowercase();
    let plural_domain = pluralize_domain(&domain);

    normalized.contains(" all ")
        || normalized.starts_with("all ")
        || normalized.starts_with("list ")
        || normalized.starts_with("show ")
        || normalized.starts_with("show me ")
        || normalized.contains(&format!(" {} ", plural_domain))
        || normalized.ends_with(&format!(" {}", plural_domain))
        || normalized.contains(&plural_domain)
}

fn summary_is_inventory_question(intent: &OutcomeIntent) -> bool {
    let normalized = intent.summary.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    summary_suggests_collection(intent)
        && (normalized.starts_with("what ")
            || normalized.starts_with("show ")
            || normalized.starts_with("show me ")
            || normalized.starts_with("list ")
            || normalized.contains(" do ")
            || normalized.contains(" have"))
}

fn pluralize_domain(domain: &str) -> String {
    if domain.ends_with('y') && domain.len() > 1 {
        format!("{}ies", &domain[..domain.len() - 1])
    } else if domain.ends_with('s') {
        domain.to_string()
    } else {
        format!("{domain}s")
    }
}

fn requested_param_keys(intent: &OutcomeIntent) -> HashSet<String> {
    let mut keys = HashSet::new();
    for step in &intent.steps {
        keys.extend(step.params.keys().cloned());
    }
    keys
}

fn intent_keywords(intent: &OutcomeIntent) -> HashSet<String> {
    let mut keywords = HashSet::new();
    for token in intent.summary.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        let token = token.trim().to_ascii_lowercase();
        if token.len() >= 3 {
            keywords.insert(token);
        }
    }
    if !intent.domain_concept.is_empty() {
        keywords.insert(intent.domain_concept.to_ascii_lowercase());
    }
    keywords
}

fn action_score(
    meta: &VerbMeta,
    requested_action: &HashSet<String>,
    desired_action_classes: &HashSet<ActionClass>,
    intent_keywords: &HashSet<String>,
) -> f32 {
    if desired_action_classes.contains(&meta.action_class) {
        return 0.95;
    }

    let verb_name = meta.verb_name.to_ascii_lowercase();
    if requested_action.contains(&verb_name) {
        return 0.8;
    }

    let action_tags = meta
        .action_tags
        .iter()
        .map(|tag| tag.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    if requested_action.iter().any(|tag| action_tags.contains(tag)) {
        return 0.5;
    }

    let description_words = meta
        .description
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(|word| word.trim().to_ascii_lowercase())
        .filter(|word| word.len() >= 3)
        .collect::<HashSet<_>>();
    if intent_keywords
        .iter()
        .any(|word| description_words.contains(word))
    {
        return 0.3;
    }

    0.0
}

fn inventory_read_bias(meta: &VerbMeta, intent: &OutcomeIntent) -> f32 {
    if !matches!(
        intent.polarity,
        IntentPolarity::Read | IntentPolarity::Ambiguous
    ) || !summary_is_inventory_question(intent)
    {
        return 0.0;
    }

    let verb_name = meta.verb_name.to_ascii_lowercase();
    if verb_name.starts_with("list") {
        return 0.2;
    }
    if verb_name.starts_with("search") {
        return -0.1;
    }

    0.0
}

fn param_overlap_score(meta: &VerbMeta, requested_params: &HashSet<String>) -> f32 {
    if requested_params.is_empty() {
        return if meta.required_params.is_empty() {
            0.2
        } else {
            0.0
        };
    }

    if meta.param_names.is_empty() {
        return 0.0;
    }

    let candidate_params = meta
        .param_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<HashSet<_>>();

    let intersection = requested_params
        .iter()
        .filter(|key| candidate_params.contains(&key.to_ascii_lowercase()))
        .count();
    let union = requested_params.len() + candidate_params.len() - intersection;

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use dsl_core::config::types::{ActionClass, HarmClass};

    use crate::sage::verb_index::VerbMeta;
    use crate::sage::{
        Clarification, CoderHandoff, EntityRef, IntentPolarity, ObservationPlane, OutcomeAction,
        OutcomeStep, SageConfidence, SageExplain, UtteranceHints,
    };

    fn index_with(entries: Vec<VerbMeta>) -> VerbMetadataIndex {
        let mut by_fqn = HashMap::new();
        for entry in entries {
            by_fqn.insert(entry.fqn.clone(), entry);
        }
        // same module can't access private field across module boundaries, so use from test helper below
        VerbMetadataIndex::from_test_map(by_fqn)
    }

    fn sample_meta(
        fqn: &str,
        polarity: IntentPolarity,
        planes: Vec<ObservationPlane>,
        action_tags: &[&str],
        params: &[&str],
        description: &str,
    ) -> VerbMeta {
        let (domain, verb_name) = fqn.split_once('.').unwrap();
        VerbMeta {
            fqn: fqn.to_string(),
            domain: domain.to_string(),
            verb_name: verb_name.to_string(),
            polarity,
            side_effects: Some(
                match polarity {
                    IntentPolarity::Read | IntentPolarity::Ambiguous => "facts_only",
                    IntentPolarity::Write => "state_write",
                }
                .to_string(),
            ),
            harm_class: match polarity {
                IntentPolarity::Read | IntentPolarity::Ambiguous => HarmClass::ReadOnly,
                IntentPolarity::Write => HarmClass::Reversible,
            },
            action_class: match polarity {
                IntentPolarity::Read | IntentPolarity::Ambiguous => {
                    if verb_name == "list" || verb_name.starts_with("list-") {
                        ActionClass::List
                    } else {
                        ActionClass::Read
                    }
                }
                IntentPolarity::Write => ActionClass::Create,
            },
            subject_kinds: vec![],
            phase_tags: vec![],
            requires_subject: true,
            planes,
            action_tags: action_tags.iter().map(|s| s.to_string()).collect(),
            param_names: params.iter().map(|s| s.to_string()).collect(),
            required_params: vec![],
            description: description.to_string(),
        }
    }

    fn sample_intent() -> OutcomeIntent {
        OutcomeIntent {
            summary: "Create a deal for this client".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Write,
            domain_concept: "deal".to_string(),
            action: OutcomeAction::Create,
            subject: Some(EntityRef {
                mention: "this client".to_string(),
                kind_hint: Some("entity".to_string()),
                uuid: None,
            }),
            steps: vec![OutcomeStep {
                action: OutcomeAction::Create,
                target: "deal".to_string(),
                params: HashMap::from([(String::from("client-id"), String::from("123"))]),
                notes: None,
            }],
            confidence: SageConfidence::Medium,
            pending_clarifications: Vec::<Clarification>::new(),
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        }
    }

    #[test]
    fn scorer_filters_by_plane_and_polarity() {
        let scorer = StructuredVerbScorer::new(index_with(vec![
            sample_meta(
                "deal.create",
                IntentPolarity::Write,
                vec![ObservationPlane::Instance],
                &["create", "deal"],
                &["client-id"],
                "Create a deal",
            ),
            sample_meta(
                "registry.list-entities",
                IntentPolarity::Read,
                vec![ObservationPlane::Structure, ObservationPlane::Registry],
                &["list", "registry"],
                &[],
                "List registry entities",
            ),
        ]));

        let candidates = scorer.score(&sample_intent(), 5);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].fqn, "deal.create");
    }

    #[test]
    fn scorer_never_offers_write_verbs_for_read_intents() {
        let scorer = StructuredVerbScorer::new(index_with(vec![
            sample_meta(
                "deal.list",
                IntentPolarity::Read,
                vec![ObservationPlane::Instance],
                &["list", "deal"],
                &[],
                "List deals",
            ),
            sample_meta(
                "deal.create",
                IntentPolarity::Write,
                vec![ObservationPlane::Instance],
                &["create", "deal"],
                &["client-id"],
                "Create a deal",
            ),
        ]));

        let intent = OutcomeIntent {
            summary: "show me the deals".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Read,
            domain_concept: "deal".to_string(),
            action: OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };

        let candidates = scorer.score(&intent, 5);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].fqn, "deal.list");
    }

    #[test]
    fn scorer_excludes_destructive_candidates_for_ambiguous_reads() {
        let mut list = sample_meta(
            "deal.list",
            IntentPolarity::Read,
            vec![ObservationPlane::Instance],
            &["list", "deal"],
            &[],
            "List deals",
        );
        list.harm_class = HarmClass::ReadOnly;

        let mut delete = sample_meta(
            "deal.delete",
            IntentPolarity::Write,
            vec![ObservationPlane::Instance],
            &["delete", "deal"],
            &["deal-id"],
            "Delete a deal",
        );
        delete.harm_class = HarmClass::Destructive;

        let scorer = StructuredVerbScorer::new(index_with(vec![list, delete]));
        let intent = OutcomeIntent {
            summary: "show me the deals".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Ambiguous,
            domain_concept: "deal".to_string(),
            action: OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };

        let candidates = scorer.score(&intent, 5);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].fqn, "deal.list");
    }

    #[test]
    fn scorer_filters_by_phase_tags_when_stage_focus_is_known() {
        let mut kyc = sample_meta(
            "deal.list",
            IntentPolarity::Read,
            vec![ObservationPlane::Instance],
            &["list", "deal"],
            &[],
            "List deals",
        );
        kyc.phase_tags = vec!["kyc".to_string()];

        let mut onboarding = sample_meta(
            "deal.list-onboarding",
            IntentPolarity::Read,
            vec![ObservationPlane::Instance],
            &["list", "deal"],
            &[],
            "List onboarding deals",
        );
        onboarding.phase_tags = vec!["onboarding".to_string()];

        let scorer = StructuredVerbScorer::new(index_with(vec![kyc, onboarding]));
        let mut intent = OutcomeIntent {
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
                stage_focus: Some("semos-kyc".to_string()),
                ..UtteranceHints::default()
            },
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };

        let candidates = scorer.score(&intent, 5);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].fqn, "deal.list");

        intent.hints.stage_focus = Some("onboarding".to_string());
        let candidates = scorer.score(&intent, 5);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].fqn, "deal.list-onboarding");
    }

    #[test]
    fn scorer_prefers_exact_action_and_param_overlap() {
        let scorer = StructuredVerbScorer::new(index_with(vec![
            sample_meta(
                "deal.create",
                IntentPolarity::Write,
                vec![ObservationPlane::Instance],
                &["create", "deal"],
                &["client-id"],
                "Create a deal",
            ),
            sample_meta(
                "deal.assign-owner",
                IntentPolarity::Write,
                vec![ObservationPlane::Instance],
                &["assign", "deal"],
                &["role-id"],
                "Assign owner to a deal",
            ),
        ]));

        let candidates = scorer.score(&sample_intent(), 5);
        assert_eq!(candidates[0].fqn, "deal.create");
        assert!(candidates[0].score > candidates[1].score);
    }

    #[test]
    fn scorer_prefers_deal_list_for_inventory_question() {
        let scorer = StructuredVerbScorer::new(index_with(vec![
            sample_meta(
                "deal.list",
                IntentPolarity::Read,
                vec![ObservationPlane::Instance],
                &["list", "deal"],
                &[],
                "List deals with optional filters",
            ),
            sample_meta(
                "deal.search",
                IntentPolarity::Read,
                vec![ObservationPlane::Instance],
                &["search", "deal"],
                &["query"],
                "Search deals by name or reference",
            ),
        ]));

        let intent = OutcomeIntent {
            summary: "what deals does Allianz have?".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Read,
            domain_concept: "deal".to_string(),
            action: OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };

        let candidates = scorer.score(&intent, 5);
        assert_eq!(candidates[0].fqn, "deal.list");
    }

    #[test]
    fn scorer_uses_description_keywords_as_fallback() {
        let intent = OutcomeIntent {
            summary: "timeline for this deal".to_string(),
            plane: ObservationPlane::Instance,
            polarity: IntentPolarity::Read,
            domain_concept: "status-history".to_string(),
            action: OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::Low,
            pending_clarifications: vec![],
            hints: UtteranceHints::default(),
            explain: SageExplain::default(),
            coder_handoff: CoderHandoff::default(),
        };
        let scorer = StructuredVerbScorer::new(index_with(vec![sample_meta(
            "deal.read-timeline",
            IntentPolarity::Read,
            vec![ObservationPlane::Instance],
            &["deal"],
            &[],
            "Show the deal timeline and status history",
        )]));

        let candidates = scorer.score(&intent, 3);
        assert_eq!(candidates[0].action_score, 0.95);
    }

    #[test]
    fn subject_kind_filter_accepts_canonical_aliases() {
        let mut meta = sample_meta(
            "kyc-case.create",
            IntentPolarity::Write,
            vec![ObservationPlane::Instance],
            &["create", "kyc-case"],
            &["cbu-id"],
            "Create a case",
        );
        meta.subject_kinds = vec!["kyc-case".to_string()];

        let mut intent = sample_intent();
        intent.subject = Some(EntityRef {
            mention: "this case".to_string(),
            kind_hint: Some("kyc_case".to_string()),
            uuid: None,
        });

        assert!(subject_kinds_match(&meta, &intent));
    }
}
