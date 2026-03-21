//! Shared Phase 2 legality artifacts for runtime gating and trace persistence.

use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
use crate::lookup::LookupResult;
use std::collections::HashSet;

/// Structured blocked action surfaced by Phase 2 legality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase2BlockedAction {
    pub action_id: String,
    pub action_kind: String,
    pub description: String,
    pub reasons: Vec<String>,
}

/// Structured constellation block surfaced by Phase 2 legality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase2ConstellationBlock {
    pub blocked_verb: Option<String>,
    pub blocking_entity: Option<String>,
    pub blocking_state: Option<String>,
    pub required_state: Option<String>,
    pub predicate: String,
    pub resolution_hint: String,
    pub constraint_kind: String,
    pub slot_path: String,
}

/// Shared Phase 2 artifact bundle.
///
/// This is the repo's current concrete handoff between lookup/entity recovery
/// and Sem OS legality. It centralizes:
/// - trace payload shaping
/// - runtime halt reasoning
/// - legality presence checks
#[derive(Debug, Clone, Default)]
pub struct Phase2Artifacts {
    pub lookup: Option<LookupResult>,
    pub envelope: Option<SemOsContextEnvelope>,
}

/// Evaluated Phase 2 result for a single turn.
///
/// This is the service-shaped runtime handoff object that callers should prefer
/// over re-deriving legality metadata from `Phase2Artifacts` repeatedly.
#[derive(Debug, Clone)]
pub struct Phase2Evaluation {
    pub artifacts: Phase2Artifacts,
    pub halt_reason_code: Option<&'static str>,
    pub halt_phase: Option<i16>,
    pub is_available: bool,
    pub is_deny_all: bool,
    pub has_usable_legal_set: bool,
    pub policy_label: &'static str,
    pub legal_verbs_or_empty: HashSet<String>,
    pub legal_verbs_if_usable: Option<HashSet<String>>,
}

impl Phase2Evaluation {
    /// Build a Phase 2 trace payload from the evaluated legality result.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// let payload = evaluation.payload();
    /// assert!(payload.get("status").is_some());
    /// ```
    pub fn payload(&self) -> serde_json::Value {
        self.artifacts.payload()
    }

    /// Build a Phase 2 trace payload or an explicit unavailable placeholder.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// let payload = evaluation.payload_or_unavailable("example");
    /// assert_eq!(payload["status"], "unavailable");
    /// ```
    pub fn payload_or_unavailable(&self, source: &str) -> serde_json::Value {
        self.artifacts.payload_or_unavailable(source)
    }

    /// Return the situation signature hash when Phase 2 computed one.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert_eq!(evaluation.situation_signature_hash(), None);
    /// ```
    pub fn situation_signature_hash(&self) -> Option<i64> {
        self.artifacts.situation_signature_hash()
    }

    /// Return the pinned constellation template version when available.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert_eq!(evaluation.constellation_template_version(), None);
    /// ```
    pub fn constellation_template_version(&self) -> Option<String> {
        self.artifacts.constellation_template_version()
    }

    /// Return the pinned constellation template identifier when available.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert_eq!(evaluation.constellation_template_id(), None);
    /// ```
    pub fn constellation_template_id(&self) -> Option<String> {
        self.artifacts.constellation_template_id()
    }

    /// Return the count of legal verbs exposed by Phase 2.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert_eq!(evaluation.legal_verb_count(), 0);
    /// ```
    pub fn legal_verb_count(&self) -> usize {
        self.artifacts.legal_verb_count()
    }

    /// Return the count of verbs pruned by Phase 2.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert_eq!(evaluation.pruned_verb_count(), 0);
    /// ```
    pub fn pruned_verb_count(&self) -> usize {
        self.artifacts.pruned_verb_count()
    }

    /// Return the legality fingerprint when Phase 2 produced one.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert_eq!(evaluation.fingerprint(), None);
    /// ```
    pub fn fingerprint(&self) -> Option<String> {
        self.artifacts.fingerprint()
    }

    /// Return the primary constellation block if one is available.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert!(evaluation.primary_constellation_block().is_none());
    /// ```
    pub fn primary_constellation_block(&self) -> Option<Phase2ConstellationBlock> {
        self.artifacts.primary_constellation_block()
    }

    /// Check whether Phase 2 allows a specific verb.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert!(!evaluation.allows_verb("case.open"));
    /// ```
    pub fn allows_verb(&self, verb: &str) -> bool {
        self.artifacts.allows_verb(verb)
    }
}

/// Explicit Phase 2 composition entrypoint for runtime callers.
///
/// This keeps lookup recovery + Sem OS legality assembly behind one named
/// boundary so the rest of the repo does not scatter ad hoc constructors.
#[derive(Debug, Default, Clone, Copy)]
pub struct Phase2Service;

impl Phase2Service {
    /// Compose Phase 2 artifacts from owned lookup and Sem OS inputs.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let artifacts = Phase2Service::compose(None, None);
    /// assert!(artifacts.is_unavailable());
    /// ```
    pub fn compose(
        lookup: Option<LookupResult>,
        envelope: Option<SemOsContextEnvelope>,
    ) -> Phase2Artifacts {
        Phase2Artifacts { lookup, envelope }
    }

    /// Compose Phase 2 artifacts from borrowed lookup and Sem OS inputs.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let artifacts = Phase2Service::compose_from_refs(None, None);
    /// assert!(artifacts.is_unavailable());
    /// ```
    pub fn compose_from_refs(
        lookup: Option<&LookupResult>,
        envelope: Option<&SemOsContextEnvelope>,
    ) -> Phase2Artifacts {
        Phase2Artifacts {
            lookup: lookup.cloned(),
            envelope: envelope.cloned(),
        }
    }

    /// Compose Phase 2 artifacts from Sem OS legality alone.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::agent::sem_os_context_envelope::SemOsContextEnvelope;
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let artifacts = Phase2Service::compose_from_envelope(SemOsContextEnvelope::deny_all());
    /// assert!(artifacts.is_deny_all());
    /// ```
    pub fn compose_from_envelope(envelope: SemOsContextEnvelope) -> Phase2Artifacts {
        Phase2Artifacts {
            lookup: None,
            envelope: Some(envelope),
        }
    }

    /// Evaluate Phase 2 from owned lookup and Sem OS inputs.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate(None, None);
    /// assert!(!evaluation.is_available);
    /// ```
    pub fn evaluate(
        lookup: Option<LookupResult>,
        envelope: Option<SemOsContextEnvelope>,
    ) -> Phase2Evaluation {
        Self::evaluate_artifacts(Self::compose(lookup, envelope))
    }

    /// Evaluate Phase 2 from borrowed lookup and Sem OS inputs.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate_from_refs(None, None);
    /// assert!(!evaluation.is_available);
    /// ```
    pub fn evaluate_from_refs(
        lookup: Option<&LookupResult>,
        envelope: Option<&SemOsContextEnvelope>,
    ) -> Phase2Evaluation {
        Self::evaluate_artifacts(Self::compose_from_refs(lookup, envelope))
    }

    /// Evaluate Phase 2 from Sem OS legality alone.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::agent::sem_os_context_envelope::SemOsContextEnvelope;
    /// use ob_poc::traceability::Phase2Service;
    ///
    /// let evaluation = Phase2Service::evaluate_from_envelope(SemOsContextEnvelope::deny_all());
    /// assert!(evaluation.is_deny_all);
    /// ```
    pub fn evaluate_from_envelope(envelope: SemOsContextEnvelope) -> Phase2Evaluation {
        Self::evaluate_artifacts(Self::compose_from_envelope(envelope))
    }

    /// Evaluate a prebuilt Phase 2 artifact bundle.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// let evaluation = Phase2Service::evaluate_artifacts(Phase2Artifacts::new(None, None));
    /// assert_eq!(evaluation.halt_phase, None);
    /// ```
    pub fn evaluate_artifacts(artifacts: Phase2Artifacts) -> Phase2Evaluation {
        let halt_reason_code = Self::halt_reason_code(&artifacts);
        let halt_phase = Self::halt_phase(&artifacts);
        let is_available = Self::is_available(&artifacts);
        let is_deny_all = Self::is_deny_all(&artifacts);
        let has_usable_legal_set = Self::has_usable_legal_set(&artifacts);
        let policy_label = Self::policy_label(&artifacts);
        let legal_verbs_or_empty = Self::legal_verbs_or_empty(&artifacts);
        let legal_verbs_if_usable = Self::legal_verbs_if_usable(&artifacts);

        Phase2Evaluation {
            artifacts,
            halt_reason_code,
            halt_phase,
            is_available,
            is_deny_all,
            has_usable_legal_set,
            policy_label,
            legal_verbs_or_empty,
            legal_verbs_if_usable,
        }
    }

    /// Return the Phase 2 halt reason code for the supplied artifacts.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert_eq!(Phase2Service::halt_reason_code(&Phase2Artifacts::new(None, None)), None);
    /// ```
    pub fn halt_reason_code(artifacts: &Phase2Artifacts) -> Option<&'static str> {
        artifacts.halt_reason_code()
    }

    /// Return the Phase 2 halt phase for the supplied artifacts.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert_eq!(Phase2Service::halt_phase(&Phase2Artifacts::new(None, None)), None);
    /// ```
    pub fn halt_phase(artifacts: &Phase2Artifacts) -> Option<i16> {
        Self::halt_reason_code(artifacts).map(|_| 2)
    }

    /// Classify the current Phase 2 legality status for a specific runtime verb.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert_eq!(
    ///     Phase2Service::runtime_gate_status(&Phase2Artifacts::new(None, None), "case.open"),
    ///     "blocked_unavailable"
    /// );
    /// ```
    pub fn runtime_gate_status(artifacts: &Phase2Artifacts, verb: &str) -> &'static str {
        if artifacts.is_unavailable() {
            "blocked_unavailable"
        } else if artifacts.is_deny_all() {
            "blocked_deny_all"
        } else if !artifacts.allows_verb(verb) {
            "blocked_not_allowed"
        } else {
            "allowed"
        }
    }

    /// Build the standard runtime gate failure message for a blocked verb.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert!(Phase2Service::runtime_gate_failure(&Phase2Artifacts::new(None, None), "case.open")
    ///     .is_some());
    /// ```
    pub fn runtime_gate_failure(artifacts: &Phase2Artifacts, verb: &str) -> Option<String> {
        match Self::runtime_gate_status(artifacts, verb) {
            "blocked_unavailable" => Some(format!(
                "Phase 5 runtime re-check blocked '{verb}' because Sem OS was unavailable."
            )),
            "blocked_deny_all" => {
                if let Some(block) = artifacts.primary_constellation_block() {
                    Some(format!(
                        "Phase 5 runtime re-check blocked '{verb}' because {}. {}.",
                        block.predicate, block.resolution_hint
                    ))
                } else {
                    Some(format!(
                        "Phase 5 runtime re-check blocked '{verb}' because no verbs were legal."
                    ))
                }
            }
            "blocked_not_allowed" => Some(format!(
                "Phase 5 runtime re-check blocked '{verb}' because it is no longer in the Sem OS legal set."
            )),
            "allowed" => None,
            status => Some(format!(
                "Phase 5 runtime re-check blocked '{verb}' due to unexpected Phase 2 status '{status}'."
            )),
        }
    }

    /// Return the current legal verb names when Phase 2 produced a usable legal set.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert_eq!(Phase2Service::legal_verb_names(&Phase2Artifacts::new(None, None)), None);
    /// ```
    pub fn legal_verb_names(artifacts: &Phase2Artifacts) -> Option<Vec<String>> {
        if artifacts.is_unavailable() {
            None
        } else {
            Some(artifacts.legal_verbs().into_iter().collect())
        }
    }

    /// Return true when Phase 2 produced a non-deny-all legality surface.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert!(!Phase2Service::has_usable_legal_set(&Phase2Artifacts::new(None, None)));
    /// ```
    pub fn has_usable_legal_set(artifacts: &Phase2Artifacts) -> bool {
        !artifacts.is_unavailable() && !artifacts.is_deny_all()
    }

    /// Return true when Phase 2 produced any legality surface at all.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert!(!Phase2Service::is_available(&Phase2Artifacts::new(None, None)));
    /// ```
    pub fn is_available(artifacts: &Phase2Artifacts) -> bool {
        !artifacts.is_unavailable()
    }

    /// Return true when Phase 2 produced a deny-all legality surface.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert!(!Phase2Service::is_deny_all(&Phase2Artifacts::new(None, None)));
    /// ```
    pub fn is_deny_all(artifacts: &Phase2Artifacts) -> bool {
        artifacts.is_deny_all()
    }

    /// Return the human-readable Phase 2 policy label.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert_eq!(Phase2Service::policy_label(&Phase2Artifacts::new(None, None)), "unavailable");
    /// ```
    pub fn policy_label(artifacts: &Phase2Artifacts) -> &'static str {
        artifacts.label()
    }

    /// Collect the subset of candidate verbs that are not currently legal in Phase 2.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// let denied = Phase2Service::collect_denied_verbs(
    ///     &Phase2Artifacts::new(None, None),
    ///     ["case.open".to_string()],
    /// );
    /// assert_eq!(denied, vec!["case.open".to_string()]);
    /// ```
    pub fn collect_denied_verbs<I>(artifacts: &Phase2Artifacts, verbs: I) -> Vec<String>
    where
        I: IntoIterator<Item = String>,
    {
        verbs
            .into_iter()
            .filter(|verb| !artifacts.allows_verb(verb))
            .collect()
    }

    /// Return the current legal set, or an empty set when Phase 2 is unavailable or deny-all.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert!(Phase2Service::legal_verbs_or_empty(&Phase2Artifacts::new(None, None)).is_empty());
    /// ```
    pub fn legal_verbs_or_empty(artifacts: &Phase2Artifacts) -> HashSet<String> {
        if Self::has_usable_legal_set(artifacts) {
            artifacts.legal_verbs()
        } else {
            HashSet::new()
        }
    }

    /// Return the legal set only when Phase 2 produced a usable legality surface.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{Phase2Artifacts, Phase2Service};
    ///
    /// assert!(Phase2Service::legal_verbs_if_usable(&Phase2Artifacts::new(None, None)).is_none());
    /// ```
    pub fn legal_verbs_if_usable(artifacts: &Phase2Artifacts) -> Option<HashSet<String>> {
        Self::has_usable_legal_set(artifacts).then(|| artifacts.legal_verbs())
    }
}

impl Phase2Artifacts {
    /// Build a Phase 2 artifact bundle from optional lookup and Sem OS inputs.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let artifacts = Phase2Artifacts::new(None, None);
    /// assert!(artifacts.is_unavailable());
    /// ```
    pub fn new(lookup: Option<LookupResult>, envelope: Option<SemOsContextEnvelope>) -> Self {
        Phase2Service::compose(lookup, envelope)
    }

    /// Clone from borrowed lookup and Sem OS inputs.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let artifacts = Phase2Artifacts::from_refs(None, None);
    /// assert!(artifacts.is_unavailable());
    /// ```
    pub fn from_refs(
        lookup: Option<&LookupResult>,
        envelope: Option<&SemOsContextEnvelope>,
    ) -> Self {
        Phase2Service::compose_from_refs(lookup, envelope)
    }

    /// Render the persisted Phase 2 payload.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let payload = Phase2Artifacts::new(None, None).payload();
    /// assert_eq!(payload["status"], "unavailable");
    /// ```
    pub fn payload(&self) -> serde_json::Value {
        if self.lookup.is_none() && self.envelope.is_none() {
            crate::traceability::build_phase2_unavailable_payload("phase2_artifacts")
        } else {
            crate::traceability::build_phase2_trace_payload(
                self.lookup.as_ref(),
                self.envelope.as_ref(),
            )
        }
    }

    /// Render the persisted Phase 2 payload with a caller-specific unavailable source label.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let payload = Phase2Artifacts::new(None, None).payload_or_unavailable("example");
    /// assert_eq!(payload["source"], "example");
    /// ```
    pub fn payload_or_unavailable(&self, source: &str) -> serde_json::Value {
        if self.is_unavailable() {
            crate::traceability::build_phase2_unavailable_payload(source)
        } else {
            self.payload()
        }
    }

    /// Return the stable situation-signature hash for the current Phase 2 inputs.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let hash = Phase2Artifacts::new(None, None).situation_signature_hash();
    /// assert!(hash.is_none());
    /// ```
    pub fn situation_signature_hash(&self) -> Option<i64> {
        if self.is_unavailable() {
            return None;
        }
        crate::traceability::compute_phase2_situation_signature_hash(
            self.lookup.as_ref(),
            self.envelope.as_ref(),
        )
    }

    /// Return the Sem OS snapshot-set pin currently acting as constellation version.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert_eq!(Phase2Artifacts::new(None, None).constellation_template_version(), None);
    /// ```
    pub fn constellation_template_version(&self) -> Option<String> {
        self.envelope
            .as_ref()
            .and_then(|envelope| envelope.snapshot_set_id.clone())
    }

    /// Return the Sem OS constellation/template identifier when grounding resolved one.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert_eq!(Phase2Artifacts::new(None, None).constellation_template_id(), None);
    /// ```
    pub fn constellation_template_id(&self) -> Option<String> {
        self.envelope.as_ref().and_then(|envelope| {
            envelope
                .grounded_action_surface
                .as_ref()
                .and_then(|surface| surface.resolved_constellation.clone())
        })
    }

    /// Return the current Phase 2 legal verb set.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert!(Phase2Artifacts::new(None, None).legal_verbs().is_empty());
    /// ```
    pub fn legal_verbs(&self) -> std::collections::HashSet<String> {
        self.envelope
            .as_ref()
            .map(|envelope| envelope.allowed_verbs.clone())
            .unwrap_or_default()
    }

    /// Return the legal verb count currently exposed by Phase 2.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert_eq!(Phase2Artifacts::new(None, None).legal_verb_count(), 0);
    /// ```
    pub fn legal_verb_count(&self) -> usize {
        self.envelope
            .as_ref()
            .map(|envelope| envelope.allowed_verbs.len())
            .unwrap_or(0)
    }

    /// Return the number of verbs pruned by the current Phase 2 legality filter.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert_eq!(Phase2Artifacts::new(None, None).pruned_verb_count(), 0);
    /// ```
    pub fn pruned_verb_count(&self) -> usize {
        self.envelope
            .as_ref()
            .map(|envelope| envelope.pruned_count())
            .unwrap_or(0)
    }

    /// Return the Sem OS legality fingerprint for the current Phase 2 view.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert_eq!(Phase2Artifacts::new(None, None).fingerprint(), None);
    /// ```
    pub fn fingerprint(&self) -> Option<String> {
        self.envelope
            .as_ref()
            .map(|envelope| envelope.fingerprint_str().to_string())
    }

    /// Return the human-readable Phase 2 legality label.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert_eq!(Phase2Artifacts::new(None, None).label(), "unavailable");
    /// ```
    pub fn label(&self) -> &'static str {
        self.envelope
            .as_ref()
            .map(|envelope| envelope.label())
            .unwrap_or("unavailable")
    }

    /// True when Phase 2 is currently in deny-all mode.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert!(!Phase2Artifacts::new(None, None).is_deny_all());
    /// ```
    pub fn is_deny_all(&self) -> bool {
        self.envelope
            .as_ref()
            .map(|envelope| envelope.is_deny_all())
            .unwrap_or(false)
    }

    /// True when a verb is contained in the current Phase 2 legal set.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert!(!Phase2Artifacts::new(None, None).allows_verb("case.open"));
    /// ```
    pub fn allows_verb(&self, verb: &str) -> bool {
        self.envelope
            .as_ref()
            .map(|envelope| envelope.is_allowed(verb))
            .unwrap_or(false)
    }

    /// Return structured blocked actions from the grounded Sem OS legality surface.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let blocked = Phase2Artifacts::new(None, None).blocked_actions();
    /// assert!(blocked.is_empty());
    /// ```
    pub fn blocked_actions(&self) -> Vec<Phase2BlockedAction> {
        self.envelope
            .as_ref()
            .and_then(|envelope| envelope.grounded_action_surface.as_ref())
            .map(|surface| {
                surface
                    .blocked_actions
                    .iter()
                    .map(|blocked| Phase2BlockedAction {
                        action_id: blocked.action_id.clone(),
                        action_kind: blocked.action_kind.clone(),
                        description: blocked.description.clone(),
                        reasons: blocked.reasons.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return structured constellation blocks derived from Sem OS constraint signals.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let blocks = Phase2Artifacts::new(None, None).constellation_blocks();
    /// assert!(blocks.is_empty());
    /// ```
    pub fn constellation_blocks(&self) -> Vec<Phase2ConstellationBlock> {
        let Some(surface) = self
            .envelope
            .as_ref()
            .and_then(|envelope| envelope.grounded_action_surface.as_ref())
        else {
            return Vec::new();
        };

        surface
            .constraint_signals
            .iter()
            .map(|signal| {
                let blocked_verb = surface
                    .blocked_actions
                    .iter()
                    .find(|blocked| {
                        blocked
                            .reasons
                            .iter()
                            .any(|reason| reason == &signal.message)
                    })
                    .map(|action| action.action_id.clone());

                let resolution_hint = match (
                    signal.related_slot.as_deref(),
                    signal.required_state.as_deref(),
                    signal.actual_state.as_deref(),
                ) {
                    (Some(slot), Some(required), Some(actual)) => {
                        format!("move '{slot}' from '{actual}' to at least '{required}'")
                    }
                    (Some(slot), Some(required), None) => {
                        format!("materialize '{slot}' and reach at least '{required}'")
                    }
                    (Some(slot), None, _) => format!("satisfy constraints on '{slot}'"),
                    _ => "satisfy blocking constellation constraints".to_string(),
                };

                Phase2ConstellationBlock {
                    blocked_verb,
                    blocking_entity: signal.related_slot.clone(),
                    blocking_state: signal.actual_state.clone(),
                    required_state: signal.required_state.clone(),
                    predicate: signal.message.clone(),
                    resolution_hint,
                    constraint_kind: signal.kind.clone(),
                    slot_path: signal.slot_path.clone(),
                }
            })
            .collect()
    }

    /// Return the primary constellation block for user-facing runtime gating.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert!(Phase2Artifacts::new(None, None).primary_constellation_block().is_none());
    /// ```
    pub fn primary_constellation_block(&self) -> Option<Phase2ConstellationBlock> {
        self.constellation_blocks().into_iter().next()
    }

    /// Return the current Phase 2 halt reason, if Phase 2 should stop the turn.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// let artifacts = Phase2Artifacts::new(None, None);
    /// assert_eq!(artifacts.halt_reason_code(), None);
    /// ```
    pub fn halt_reason_code(&self) -> Option<&'static str> {
        if let Some(envelope) = self.envelope.as_ref() {
            if envelope.is_deny_all() {
                return Some("no_allowed_verbs");
            }
            if envelope.is_unavailable() {
                return Some("sem_os_unavailable");
            }
        }

        let lookup = self.lookup.as_ref()?;
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
        let unresolved_count = lookup
            .entities
            .iter()
            .filter(|entity| entity.selected.is_none() && entity.candidates.is_empty())
            .count();

        if ambiguous_count > 0 {
            Some("ambiguous_entity")
        } else if unresolved_count > 0 && resolved_count == 0 {
            Some("no_entity_found")
        } else {
            None
        }
    }

    /// True when neither lookup nor Sem OS legality are available.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase2Artifacts;
    ///
    /// assert!(Phase2Artifacts::new(None, None).is_unavailable());
    /// ```
    pub fn is_unavailable(&self) -> bool {
        let sem_os_unavailable = self
            .envelope
            .as_ref()
            .is_none_or(SemOsContextEnvelope::is_unavailable);
        self.lookup.is_none() && sem_os_unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::{Phase2Artifacts, Phase2Service};
    use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
    use crate::entity_linking::{EntityCandidate, EntityResolution};
    use crate::lookup::LookupResult;
    use sem_os_core::context_resolution::{
        BlockedActionOption, GroundedActionSurface, GroundedConstraintSignal, SubjectRef,
    };
    use uuid::Uuid;

    #[test]
    fn test_phase2_artifacts_detect_deny_all() {
        let artifacts = Phase2Artifacts::new(None, Some(SemOsContextEnvelope::deny_all()));
        assert_eq!(artifacts.halt_reason_code(), Some("no_allowed_verbs"));
        assert_eq!(Phase2Service::halt_phase(&artifacts), Some(2));
    }

    #[test]
    fn test_phase2_artifacts_unavailable_has_no_signature_hash() {
        let artifacts = Phase2Artifacts::new(None, None);
        assert_eq!(artifacts.situation_signature_hash(), None);
    }

    #[test]
    fn test_phase2_artifacts_detect_ambiguous_entity() {
        let artifacts = Phase2Artifacts::new(
            Some(LookupResult {
                verbs: vec![],
                entities: vec![EntityResolution {
                    mention_span: (0, 7),
                    mention_text: "Allianz".to_string(),
                    candidates: vec![
                        EntityCandidate {
                            entity_id: Uuid::new_v4(),
                            entity_kind: "company".to_string(),
                            canonical_name: "Allianz SE".to_string(),
                            score: 0.81,
                            evidence: vec![],
                        },
                        EntityCandidate {
                            entity_id: Uuid::new_v4(),
                            entity_kind: "fund".to_string(),
                            canonical_name: "Allianz Fund".to_string(),
                            score: 0.79,
                            evidence: vec![],
                        },
                    ],
                    selected: None,
                    confidence: 0.0,
                    evidence: vec![],
                }],
                dominant_entity: None,
                expected_kinds: vec!["company".to_string()],
                concepts: vec![],
                verb_matched: false,
                entities_resolved: false,
            }),
            None,
        );

        assert_eq!(artifacts.halt_reason_code(), Some("ambiguous_entity"));
        assert_eq!(artifacts.payload()["resolution_mode"], "disambiguated");
    }

    #[test]
    fn test_phase2_artifacts_surface_constellation_blocks() {
        let mut envelope = SemOsContextEnvelope::test_with_verbs(&[]);
        envelope.grounded_action_surface = Some(GroundedActionSurface {
            resolved_subject: SubjectRef::TaskId(Uuid::nil()),
            resolved_constellation: Some("constellation.kyc".to_string()),
            resolved_slot_path: Some("case".to_string()),
            resolved_node_id: Some("node-1".to_string()),
            resolved_state_machine: Some("case_machine".to_string()),
            current_state: Some("intake".to_string()),
            traversed_edges: vec![],
            constraint_signals: vec![GroundedConstraintSignal {
                kind: "dependency_block".to_string(),
                slot_path: "case".to_string(),
                related_slot: Some("cbu".to_string()),
                required_state: Some("filled".to_string()),
                actual_state: Some("empty".to_string()),
                message: "dependency 'cbu' is in state 'empty' but requires 'filled'".to_string(),
            }],
            valid_actions: vec![],
            blocked_actions: vec![BlockedActionOption {
                action_id: "case.open".to_string(),
                action_kind: "primitive".to_string(),
                description: "Blocked action for slot 'case'".to_string(),
                reasons: vec![
                    "dependency 'cbu' is in state 'empty' but requires 'filled'".to_string()
                ],
            }],
            dsl_candidates: vec![],
        });

        let artifacts = Phase2Artifacts::new(None, Some(envelope));
        let block = artifacts.primary_constellation_block().expect("block");
        assert_eq!(block.blocked_verb.as_deref(), Some("case.open"));
        assert_eq!(block.blocking_entity.as_deref(), Some("cbu"));
        assert_eq!(
            block.resolution_hint,
            "move 'cbu' from 'empty' to at least 'filled'"
        );
        assert_eq!(artifacts.blocked_actions().len(), 1);
    }

    #[test]
    fn test_phase2_artifacts_surface_legal_set_accessors() {
        let envelope = SemOsContextEnvelope::test_with_verbs(&["case.open", "case.submit"]);
        let artifacts = Phase2Artifacts::new(None, Some(envelope));

        assert_eq!(artifacts.legal_verb_count(), 2);
        assert!(Phase2Service::has_usable_legal_set(&artifacts));
        assert!(artifacts.allows_verb("case.open"));
        assert!(!artifacts.allows_verb("case.reject"));
        assert_eq!(
            Phase2Service::runtime_gate_status(&artifacts, "case.open"),
            "allowed"
        );
        assert_eq!(
            Phase2Service::runtime_gate_status(&artifacts, "case.reject"),
            "blocked_not_allowed"
        );
        assert_eq!(artifacts.label(), "allowed_set");
        assert!(artifacts.fingerprint().is_some());
        assert_eq!(
            Phase2Service::legal_verb_names(&artifacts).map(|mut verbs| {
                verbs.sort();
                verbs
            }),
            Some(vec!["case.open".to_string(), "case.submit".to_string()])
        );
        assert_eq!(
            Phase2Service::collect_denied_verbs(
                &artifacts,
                ["case.open".to_string(), "case.reject".to_string()]
            ),
            vec!["case.reject".to_string()]
        );
        assert_eq!(
            Phase2Service::legal_verbs_or_empty(&artifacts),
            artifacts.legal_verbs()
        );
        assert_eq!(
            Phase2Service::legal_verbs_if_usable(&artifacts),
            Some(artifacts.legal_verbs())
        );
        assert!(Phase2Service::is_available(&artifacts));
        assert!(!Phase2Service::is_deny_all(&artifacts));
        assert_eq!(Phase2Service::policy_label(&artifacts), "allowed_set");
    }

    #[test]
    fn test_phase2_service_runtime_gate_failure_uses_constellation_block() {
        let mut envelope = SemOsContextEnvelope::deny_all();
        envelope.grounded_action_surface = Some(GroundedActionSurface {
            resolved_subject: SubjectRef::TaskId(Uuid::nil()),
            resolved_constellation: Some("constellation.kyc".to_string()),
            resolved_slot_path: Some("case".to_string()),
            resolved_node_id: Some("node-1".to_string()),
            resolved_state_machine: Some("case_machine".to_string()),
            current_state: Some("draft".to_string()),
            traversed_edges: vec![],
            constraint_signals: vec![GroundedConstraintSignal {
                kind: "dependency_block".to_string(),
                slot_path: "case".to_string(),
                related_slot: Some("case".to_string()),
                required_state: Some("approved".to_string()),
                actual_state: Some("draft".to_string()),
                message: "case must reach approved".to_string(),
            }],
            valid_actions: vec![],
            blocked_actions: vec![BlockedActionOption {
                action_id: "case.approve".to_string(),
                action_kind: "primitive".to_string(),
                description: "Blocked action for slot 'case'".to_string(),
                reasons: vec!["case must reach approved".to_string()],
            }],
            dsl_candidates: vec![],
        });
        let artifacts = Phase2Artifacts::new(None, Some(envelope));

        let failure = Phase2Service::runtime_gate_failure(&artifacts, "case.approve")
            .expect("deny-all should produce a failure");
        assert!(failure.contains("case must reach approved"));
        assert!(failure.contains("move 'case' from 'draft' to at least 'approved'"));
    }
}
