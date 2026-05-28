//! Sage state-machine orchestrator.
//!
//! # State machine
//!
//! ```text
//! Listening ──Utterance──────────────────────────────► Matching
//! Matching  ──SelectPack──────────────────────────────► Confirming
//! Confirming──Confirm(Accept)────────────────────────► Instantiated
//! Confirming──Confirm(EditParameter)─────────────────► Confirming (loop)
//! Confirming──Confirm(RejectPack)────────────────────► Matching
//! Confirming──Confirm(Cancel)────────────────────────► Cancelled
//! Instantiated──Deploy───────────────────────────────► Deployed
//! * ──Cancel──────────────────────────────────────────► Cancelled
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    audit::SageAuditLog,
    confirmation::{ConfirmationSession, ConfirmationState},
    extractor::{extract_parameters, LlmExtractor},
    instantiator::{instantiate, validate_instantiation, InstantiationResult, ValidationSummary},
    matcher::{
        match_packs, match_packs_embedding_only, BagOfWordsEmbedder, LlmClient, PackEmbedder,
    },
    types::{ConfirmationResponse, RankedCandidate, SageContext},
};
use dsl_resolution::PackRegistry;

// ---------------------------------------------------------------------------
// State enum
// ---------------------------------------------------------------------------

/// Current state of the Sage authoring state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SageState {
    /// Waiting for utterance input.
    Listening,
    /// Pack match candidates computed; waiting for user pack selection.
    Matching { candidates: Vec<RankedCandidate> },
    /// Pack selected; parameters proposed; waiting for user confirmation.
    Confirming { session: ConfirmationSession },
    /// Parameters confirmed; DSL instantiated and validated.
    Instantiated {
        result: InstantiationResult,
        validation: ValidationSummary,
    },
    /// Deployment confirmed and recorded.
    Deployed { workflow_id: String },
    /// Terminal error state.
    Failed { reason: String },
    /// User cancelled.
    Cancelled,
}

// ---------------------------------------------------------------------------
// Input enum
// ---------------------------------------------------------------------------

/// An input that can be fed to the Sage state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SageInput {
    /// User provides a natural-language utterance.
    Utterance(String),
    /// User selects a pack from the ranked candidates.
    SelectPack { pack_name: String },
    /// User response to the parameter confirmation dialogue.
    Confirm(ConfirmationResponse),
    /// User approves deployment of the instantiated workflow.
    Deploy { workflow_name: String },
    /// User cancels at any point.
    Cancel,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// A live Sage authoring session that carries the current state and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SageSession {
    /// Stable identifier for this session (e.g., `"sage-a1b2c3d4"`).
    pub session_id: String,
    /// Current state machine state.
    pub state: SageState,
    /// The original utterance, preserved for re-matching after rejection.
    pub utterance: Option<String>,
    /// Contextual signals provided to matchers and extractors.
    pub context: SageContext,
    /// Human-readable log of every state transition.
    pub transition_log: Vec<String>,
}

impl SageSession {
    /// Create a new session in [`SageState::Listening`].
    pub fn new(context: SageContext) -> Self {
        Self {
            session_id: format!("sage-{}", &uuid::Uuid::new_v4().to_string()[..8]),
            state: SageState::Listening,
            utterance: None,
            context,
            transition_log: vec![],
        }
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.transition_log.push(msg.into());
    }
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Drives the Sage authoring state machine.
///
/// Construct with [`SageOrchestrator::new`], then call [`SageOrchestrator::step`]
/// repeatedly to advance the session.
pub struct SageOrchestrator<'a> {
    registry: &'a PackRegistry,
    embedder: Box<dyn PackEmbedder>,
    llm_matcher: Option<Box<dyn LlmClient>>,
    llm_extractor: Option<Box<dyn LlmExtractor>>,
    audit: SageAuditLog,
}

impl<'a> SageOrchestrator<'a> {
    /// Create a new orchestrator backed by the bag-of-words embedder (no LLM).
    pub fn new(registry: &'a PackRegistry) -> Self {
        Self {
            registry,
            embedder: Box::new(BagOfWordsEmbedder),
            llm_matcher: None,
            llm_extractor: None,
            audit: SageAuditLog::new(),
        }
    }

    /// Return all audit entries for a session.
    pub fn audit_entries(&self, session_id: &str) -> Vec<crate::audit::SageAuditEntry> {
        self.audit.entries_for_session(session_id)
    }

    /// Process one [`SageInput`], advance the state machine, and return the new state.
    ///
    /// On invalid transitions the state is left unchanged and the invalid input
    /// is noted in the transition log.
    pub async fn step<'b>(
        &self,
        session: &'b mut SageSession,
        input: SageInput,
    ) -> Result<&'b SageState> {
        let from_state = state_name(&session.state);

        match (&session.state, input) {
            // ----------------------------------------------------------------
            // Listening → Matching
            // ----------------------------------------------------------------
            (SageState::Listening, SageInput::Utterance(utterance)) => {
                session.utterance = Some(utterance.clone());
                session.log(format!(
                    "Received utterance: {}",
                    &utterance[..utterance.len().min(60)]
                ));

                let candidates = if let Some(llm) = &self.llm_matcher {
                    match_packs(
                        &utterance,
                        &session.context,
                        self.registry,
                        self.embedder.as_ref(),
                        Some(llm.as_ref()),
                    )
                    .await?
                } else {
                    match_packs_embedding_only(
                        &utterance,
                        &session.context,
                        self.registry,
                        self.embedder.as_ref(),
                    )
                };

                session.log(format!(
                    "Matched {} candidates; top: {}",
                    candidates.len(),
                    candidates
                        .first()
                        .map(|c| c.pack_name.as_str())
                        .unwrap_or("none")
                ));

                self.audit.record(
                    &session.session_id,
                    &format!("{from_state}→Matching"),
                    serde_json::json!({
                        "utterance_len": utterance.len(),
                        "candidate_count": candidates.len(),
                        "top_pack": candidates.first().map(|c| &c.pack_name),
                        "top_confidence": candidates.first().map(|c| c.confidence),
                    }),
                );

                session.state = SageState::Matching { candidates };
            }

            // ----------------------------------------------------------------
            // Matching → Confirming
            // ----------------------------------------------------------------
            (SageState::Matching { .. }, SageInput::SelectPack { pack_name }) => {
                // Look up the pack version from the registry; fall back to "1.0.0".
                let pack_version = self
                    .registry
                    .lookup(&pack_name, "1.0.0")
                    .map(|p| p.version.clone())
                    .unwrap_or_else(|| "1.0.0".to_string());

                let utterance = session.utterance.clone().unwrap_or_default();

                let request = extract_parameters(
                    &utterance,
                    &pack_name,
                    &pack_version,
                    &session.context,
                    self.registry,
                    self.llm_extractor.as_ref().map(|e| e.as_ref()),
                )
                .await?;

                session.log(format!(
                    "Extracted {} parameters for pack {}",
                    request.proposed_parameters.len(),
                    pack_name
                ));

                self.audit.record(
                    &session.session_id,
                    &format!("{from_state}→Confirming"),
                    serde_json::json!({
                        "pack_name": &pack_name,
                        "pack_version": &pack_version,
                        "param_count": request.proposed_parameters.len(),
                    }),
                );

                session.state = SageState::Confirming {
                    session: ConfirmationSession::new(request),
                };
            }

            // ----------------------------------------------------------------
            // Confirming — drives the confirmation sub-state machine
            // ----------------------------------------------------------------
            (
                SageState::Confirming {
                    session: conf_session,
                },
                SageInput::Confirm(response),
            ) => {
                let mut conf_session = conf_session.clone();
                let new_conf_state = conf_session.apply_response(response.clone());

                match new_conf_state {
                    ConfirmationState::Accepted => {
                        let params = conf_session
                            .confirmed_parameters()
                            .expect("confirmed_parameters must be Some when state is Accepted");
                        let pack_name = conf_session.request.pack_name.clone();
                        let pack_version = conf_session.request.pack_version.clone();

                        let result = instantiate(
                            &pack_name,
                            &pack_version,
                            &params,
                            None,
                            &session.context,
                            self.registry,
                        )?;
                        let validation = validate_instantiation(&result.structural_dsl)?;

                        session.log(format!(
                            "Instantiated pack {} → {} atoms",
                            pack_name,
                            result.atom_names.len()
                        ));

                        self.audit.record(
                            &session.session_id,
                            &format!("{from_state}→Instantiated"),
                            serde_json::json!({
                                "pack_name": &pack_name,
                                "atom_count": result.atom_names.len(),
                                "has_errors": validation.has_errors,
                                "node_count": validation.node_count,
                            }),
                        );

                        session.state = SageState::Instantiated { result, validation };
                    }

                    ConfirmationState::Rejected => {
                        session.log("Pack rejected — returning to Matching");

                        self.audit.record(
                            &session.session_id,
                            &format!("{from_state}→Matching"),
                            serde_json::json!({ "reason": "pack_rejected" }),
                        );

                        if let Some(utterance) = &session.utterance {
                            let candidates = match_packs_embedding_only(
                                utterance,
                                &session.context,
                                self.registry,
                                self.embedder.as_ref(),
                            );
                            session.state = SageState::Matching { candidates };
                        } else {
                            session.state = SageState::Listening;
                        }
                    }

                    ConfirmationState::Cancelled => {
                        session.log("Cancelled during confirmation");
                        self.audit.record(
                            &session.session_id,
                            &format!("{from_state}→Cancelled"),
                            serde_json::json!({ "reason": "user_cancel" }),
                        );
                        session.state = SageState::Cancelled;
                    }

                    ConfirmationState::Pending => {
                        // Edit applied — stay in Confirming with the updated session.
                        session.log("Parameter edited — staying in Confirming");
                        self.audit.record(
                            &session.session_id,
                            &format!("{from_state}→Confirming"),
                            serde_json::json!({ "reason": "parameter_edit" }),
                        );
                        session.state = SageState::Confirming {
                            session: conf_session,
                        };
                    }
                }
            }

            // ----------------------------------------------------------------
            // Instantiated → Deployed
            // ----------------------------------------------------------------
            (SageState::Instantiated { .. }, SageInput::Deploy { workflow_name }) => {
                let workflow_id = format!(
                    "{}-{}",
                    workflow_name,
                    &uuid::Uuid::new_v4().to_string()[..8]
                );
                session.log(format!("Deployed workflow: {}", workflow_id));

                self.audit.record(
                    &session.session_id,
                    &format!("{from_state}→Deployed"),
                    serde_json::json!({ "workflow_id": &workflow_id }),
                );

                session.state = SageState::Deployed { workflow_id };
            }

            // ----------------------------------------------------------------
            // Universal cancel
            // ----------------------------------------------------------------
            (_, SageInput::Cancel) => {
                session.log("Cancelled by user");
                self.audit.record(
                    &session.session_id,
                    &format!("{from_state}→Cancelled"),
                    serde_json::json!({ "reason": "explicit_cancel" }),
                );
                session.state = SageState::Cancelled;
            }

            // ----------------------------------------------------------------
            // Invalid transitions — note in log, leave state unchanged
            // ----------------------------------------------------------------
            (state, input) => {
                let msg = format!(
                    "Invalid transition: {:?} in state {}",
                    std::mem::discriminant(&input),
                    state_name(state)
                );
                session.log(msg);
            }
        }

        Ok(&session.state)
    }

    /// Return the top candidate if its confidence meets `threshold`.
    pub fn should_auto_select(candidates: &[RankedCandidate], threshold: f32) -> Option<String> {
        candidates
            .first()
            .filter(|c| c.confidence >= threshold)
            .map(|c| c.pack_name.clone())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn state_name(state: &SageState) -> &'static str {
    match state {
        SageState::Listening => "Listening",
        SageState::Matching { .. } => "Matching",
        SageState::Confirming { .. } => "Confirming",
        SageState::Instantiated { .. } => "Instantiated",
        SageState::Deployed { .. } => "Deployed",
        SageState::Failed { .. } => "Failed",
        SageState::Cancelled => "Cancelled",
    }
}
