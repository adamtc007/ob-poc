//! Voice command semantic matching API
//!
//! Provides endpoints for matching voice transcripts to DSL verbs using
//! semantic similarity (Candle ML embeddings) with phonetic fallback.
//!
//! Architecture:
//! ```text
//! Voice Transcript ──► /api/voice/match ──► SemanticMatcher ──► MatchResult
//!                                               │
//!                      ┌────────────────────────┼────────────────────────┐
//!                      ▼                        ▼                        ▼
//!                 Exact Match              Semantic (pgvector)      Phonetic
//!                 (priority 1)             (priority 2)            (priority 3)
//! ```
//!
//! ## Feedback Capture
//!
//! All matches are captured for ML continuous learning:
//! 1. `/api/voice/match` captures input + match result, returns `interaction_id`
//! 2. `/api/voice/outcome` records what user actually did (executed, corrected, etc.)
//! 3. Batch analysis job discovers patterns from accumulated feedback

use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use ob_semantic_matcher::feedback::{FeedbackService, InputSource, Outcome};
use ob_semantic_matcher::SemanticMatcher;

/// Shared semantic matcher state
///
/// The matcher is lazily initialized on first request since it downloads
/// the ML model (~22MB) on first use.
pub struct VoiceMatcherState {
    matcher: RwLock<Option<SemanticMatcher>>,
    feedback: FeedbackService,
    pool: sqlx::PgPool,
}

impl VoiceMatcherState {
    pub fn new(pool: sqlx::PgPool) -> Self {
        let feedback = FeedbackService::new(pool.clone());
        Self {
            matcher: RwLock::new(None),
            feedback,
            pool,
        }
    }

    /// Get or initialize the semantic matcher
    async fn get_matcher(
        &self,
    ) -> Result<tokio::sync::RwLockReadGuard<'_, Option<SemanticMatcher>>, String> {
        // Fast path: check if already initialized
        {
            let guard = self.matcher.read().await;
            if guard.is_some() {
                return Ok(guard);
            }
        }

        // Slow path: initialize
        {
            let mut guard = self.matcher.write().await;
            if guard.is_none() {
                tracing::info!("Initializing SemanticMatcher (downloading model if needed)...");
                match SemanticMatcher::new(self.pool.clone()).await {
                    Ok(m) => {
                        tracing::info!("SemanticMatcher initialized successfully");
                        *guard = Some(m);
                    }
                    Err(e) => {
                        tracing::error!("Failed to initialize SemanticMatcher: {}", e);
                        return Err(format!("Matcher initialization failed: {}", e));
                    }
                }
            }
        }

        Ok(self.matcher.read().await)
    }
}

/// Request body for voice match endpoint
#[derive(Debug, Deserialize)]
pub struct VoiceMatchRequest {
    /// Session ID for feedback tracking
    #[serde(default = "default_session_id")]
    pub session_id: Uuid,
    /// The voice transcript to match
    pub transcript: String,
    /// Confidence from speech recognition (0.0-1.0)
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Voice provider (e.g., "deepgram", "webspeech")
    #[serde(default)]
    pub provider: Option<String>,
    /// Current context for disambiguation
    #[serde(default)]
    pub context: Option<VoiceMatchContext>,
}

fn default_confidence() -> f32 {
    1.0
}

fn default_session_id() -> Uuid {
    Uuid::new_v4()
}

/// Context for better matching
#[derive(Debug, Deserialize, Default)]
pub struct VoiceMatchContext {
    /// Currently focused entity ID
    pub focused_entity_id: Option<String>,
    /// Current CBU ID
    pub current_cbu_id: Option<String>,
    /// Current view mode
    pub view_mode: Option<String>,
}

/// Response from voice match endpoint
#[derive(Debug, Serialize)]
pub struct VoiceMatchResponse {
    /// Interaction ID for feedback tracking (use with /api/voice/outcome)
    pub interaction_id: Uuid,
    /// Whether a match was found
    pub matched: bool,
    /// The matched verb name (e.g., "ui.zoom-in", "ubo.list-owners")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb_name: Option<String>,
    /// The pattern that matched (for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_phrase: Option<String>,
    /// Similarity score (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
    /// How the match was made (exact, semantic, phonetic, cached)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_method: Option<String>,
    /// Category (navigation, investigation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Whether this verb requires agent processing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_agent_bound: Option<bool>,
    /// Suggested action for the UI
    pub action: VoiceMatchAction,
    /// Alternative matches (for selection feedback)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<AlternativeMatch>,
    /// Error message if matching failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Alternative match for user selection
#[derive(Debug, Serialize)]
pub struct AlternativeMatch {
    pub verb_name: String,
    pub pattern_phrase: String,
    pub similarity: f32,
}

/// Suggested action for the UI based on match result
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceMatchAction {
    /// Execute navigation command locally
    ExecuteNavigation,
    /// Send to agent for processing
    SendToAgent,
    /// No action - unrecognized command
    None,
    /// Ask user for clarification (low confidence)
    Clarify,
}

/// Match a voice transcript to a verb
///
/// POST /api/voice/match
///
/// Captures feedback for ML learning. Returns `interaction_id` that should be
/// passed to `/api/voice/outcome` when the user takes action.
pub async fn match_voice_command(
    State(state): State<Arc<VoiceMatcherState>>,
    Json(request): Json<VoiceMatchRequest>,
) -> impl IntoResponse {
    let interaction_id = Uuid::new_v4();

    // Reject low-confidence transcripts
    if request.confidence < 0.5 {
        return Json(VoiceMatchResponse {
            interaction_id,
            matched: false,
            verb_name: None,
            pattern_phrase: None,
            similarity: None,
            match_method: None,
            category: None,
            is_agent_bound: None,
            action: VoiceMatchAction::None,
            alternatives: vec![],
            error: Some("Confidence too low".to_string()),
        });
    }

    // Get or initialize matcher
    let matcher_guard = match state.get_matcher().await {
        Ok(g) => g,
        Err(e) => {
            return Json(VoiceMatchResponse {
                interaction_id,
                matched: false,
                verb_name: None,
                pattern_phrase: None,
                similarity: None,
                match_method: None,
                category: None,
                is_agent_bound: None,
                action: VoiceMatchAction::None,
                alternatives: vec![],
                error: Some(e),
            });
        }
    };

    let matcher = match matcher_guard.as_ref() {
        Some(m) => m,
        None => {
            return Json(VoiceMatchResponse {
                interaction_id,
                matched: false,
                verb_name: None,
                pattern_phrase: None,
                similarity: None,
                match_method: None,
                category: None,
                is_agent_bound: None,
                action: VoiceMatchAction::None,
                alternatives: vec![],
                error: Some("Matcher not initialized".to_string()),
            });
        }
    };

    // Get context for feedback
    let graph_context = request
        .context
        .as_ref()
        .and_then(|c| c.current_cbu_id.clone());
    let workflow_phase = request.context.as_ref().and_then(|c| c.view_mode.clone());

    // Perform semantic matching with alternatives
    let (primary_result, alternatives) = match matcher
        .find_match_with_alternatives(&request.transcript, 3)
        .await
    {
        Ok((primary, alts)) => (Some(primary), alts),
        Err(_) => (None, vec![]),
    };

    // Convert alternatives for response
    let alt_matches: Vec<AlternativeMatch> = alternatives
        .iter()
        .map(|a| AlternativeMatch {
            verb_name: a.verb_name.clone(),
            pattern_phrase: a.pattern_phrase.clone(),
            similarity: a.similarity,
        })
        .collect();

    // Capture feedback (fire-and-forget, don't block response)
    let feedback_result = state
        .feedback
        .capture_match(
            request.session_id,
            &request.transcript,
            InputSource::Voice,
            primary_result.as_ref(),
            &alternatives,
            graph_context.as_deref(),
            workflow_phase.as_deref(),
        )
        .await;

    // Use captured interaction_id if available, otherwise use generated one
    let final_interaction_id = match feedback_result {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Failed to capture feedback: {}", e);
            interaction_id
        }
    };

    match primary_result {
        Some(result) => {
            let action = if result.is_agent_bound {
                VoiceMatchAction::SendToAgent
            } else if result.similarity < 0.7 {
                VoiceMatchAction::Clarify
            } else {
                VoiceMatchAction::ExecuteNavigation
            };

            Json(VoiceMatchResponse {
                interaction_id: final_interaction_id,
                matched: true,
                verb_name: Some(result.verb_name),
                pattern_phrase: Some(result.pattern_phrase),
                similarity: Some(result.similarity),
                match_method: Some(result.match_method.to_string()),
                category: Some(result.category),
                is_agent_bound: Some(result.is_agent_bound),
                action,
                alternatives: alt_matches,
                error: None,
            })
        }
        None => {
            // No match found - suggest sending to agent as fallback
            Json(VoiceMatchResponse {
                interaction_id: final_interaction_id,
                matched: false,
                verb_name: None,
                pattern_phrase: None,
                similarity: None,
                match_method: None,
                category: None,
                is_agent_bound: None,
                action: VoiceMatchAction::SendToAgent, // Fallback to agent
                alternatives: alt_matches,
                error: Some("No match found".to_string()),
            })
        }
    }
}

/// Batch match multiple transcripts
///
/// POST /api/voice/match/batch
#[derive(Debug, Deserialize)]
pub struct BatchMatchRequest {
    pub transcripts: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchMatchResponse {
    pub results: Vec<VoiceMatchResponse>,
}

pub async fn batch_match_voice_commands(
    State(state): State<Arc<VoiceMatcherState>>,
    Json(request): Json<BatchMatchRequest>,
) -> impl IntoResponse {
    let matcher_guard = match state.get_matcher().await {
        Ok(g) => g,
        Err(e) => {
            return Json(BatchMatchResponse {
                results: request
                    .transcripts
                    .iter()
                    .map(|_| VoiceMatchResponse {
                        interaction_id: Uuid::new_v4(),
                        matched: false,
                        verb_name: None,
                        pattern_phrase: None,
                        similarity: None,
                        match_method: None,
                        category: None,
                        is_agent_bound: None,
                        action: VoiceMatchAction::None,
                        alternatives: vec![],
                        error: Some(e.clone()),
                    })
                    .collect(),
            });
        }
    };

    let matcher = match matcher_guard.as_ref() {
        Some(m) => m,
        None => {
            return Json(BatchMatchResponse {
                results: request
                    .transcripts
                    .iter()
                    .map(|_| VoiceMatchResponse {
                        interaction_id: Uuid::new_v4(),
                        matched: false,
                        verb_name: None,
                        pattern_phrase: None,
                        similarity: None,
                        match_method: None,
                        category: None,
                        is_agent_bound: None,
                        action: VoiceMatchAction::None,
                        alternatives: vec![],
                        error: Some("Matcher not initialized".to_string()),
                    })
                    .collect(),
            });
        }
    };

    let mut results = Vec::with_capacity(request.transcripts.len());
    for transcript in &request.transcripts {
        let response = match matcher.find_match(transcript).await {
            Ok(result) => {
                let action = if result.is_agent_bound {
                    VoiceMatchAction::SendToAgent
                } else if result.similarity < 0.7 {
                    VoiceMatchAction::Clarify
                } else {
                    VoiceMatchAction::ExecuteNavigation
                };

                VoiceMatchResponse {
                    interaction_id: Uuid::new_v4(), // Batch doesn't capture feedback
                    matched: true,
                    verb_name: Some(result.verb_name),
                    pattern_phrase: Some(result.pattern_phrase),
                    similarity: Some(result.similarity),
                    match_method: Some(result.match_method.to_string()),
                    category: Some(result.category),
                    is_agent_bound: Some(result.is_agent_bound),
                    action,
                    alternatives: vec![],
                    error: None,
                }
            }
            Err(e) => VoiceMatchResponse {
                interaction_id: Uuid::new_v4(),
                matched: false,
                verb_name: None,
                pattern_phrase: None,
                similarity: None,
                match_method: None,
                category: None,
                is_agent_bound: None,
                action: VoiceMatchAction::SendToAgent,
                alternatives: vec![],
                error: Some(format!("No match: {}", e)),
            },
        };
        results.push(response);
    }

    Json(BatchMatchResponse { results })
}

/// Request to record outcome of a voice match
#[derive(Debug, Deserialize)]
pub struct VoiceOutcomeRequest {
    /// The interaction_id from the match response
    pub interaction_id: Uuid,
    /// What the user did
    pub outcome: String,
    /// If user selected alternative or corrected, which verb
    #[serde(default)]
    pub outcome_verb: Option<String>,
    /// If user corrected by rephrasing, the correction text
    #[serde(default)]
    pub correction_input: Option<String>,
    /// Time from match to outcome in milliseconds
    #[serde(default)]
    pub time_to_outcome_ms: Option<i32>,
}

/// Response from outcome recording
#[derive(Debug, Serialize)]
pub struct VoiceOutcomeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Record the outcome of a voice match
///
/// POST /api/voice/outcome
///
/// Call this after the user takes action on a voice match to complete the
/// feedback loop for ML learning.
pub async fn record_voice_outcome(
    State(state): State<Arc<VoiceMatcherState>>,
    Json(request): Json<VoiceOutcomeRequest>,
) -> impl IntoResponse {
    // Parse outcome string to enum
    let outcome = match request.outcome.as_str() {
        "executed" => Outcome::Executed,
        "selected_alt" => Outcome::SelectedAlt,
        "corrected" => Outcome::Corrected,
        "rephrased" => Outcome::Rephrased,
        "abandoned" => Outcome::Abandoned,
        _ => {
            return Json(VoiceOutcomeResponse {
                success: false,
                error: Some(format!("Invalid outcome: {}", request.outcome)),
            });
        }
    };

    match state
        .feedback
        .record_outcome(
            request.interaction_id,
            outcome,
            request.outcome_verb,
            request.correction_input,
            request.time_to_outcome_ms,
        )
        .await
    {
        Ok(found) => {
            if found {
                Json(VoiceOutcomeResponse {
                    success: true,
                    error: None,
                })
            } else {
                Json(VoiceOutcomeResponse {
                    success: false,
                    error: Some("Interaction not found".to_string()),
                })
            }
        }
        Err(e) => {
            tracing::error!("Failed to record outcome: {}", e);
            Json(VoiceOutcomeResponse {
                success: false,
                error: Some(format!("Database error: {}", e)),
            })
        }
    }
}

/// Health check for voice matching service
///
/// GET /api/voice/health
pub async fn voice_health(State(state): State<Arc<VoiceMatcherState>>) -> impl IntoResponse {
    #[derive(Serialize)]
    struct HealthResponse {
        status: String,
        matcher_initialized: bool,
    }

    let initialized = state.matcher.read().await.is_some();

    Json(HealthResponse {
        status: if initialized { "ready" } else { "initializing" }.to_string(),
        matcher_initialized: initialized,
    })
}

/// Create the voice matching router
///
/// Returns a `Router<()>` that can be merged with other routers.
/// The voice matcher state is encapsulated internally.
///
/// ## Endpoints
///
/// - `POST /api/voice/match` - Match a voice transcript, returns `interaction_id`
/// - `POST /api/voice/outcome` - Record outcome for ML learning
/// - `POST /api/voice/match/batch` - Batch match multiple transcripts
/// - `GET /api/voice/health` - Health check
pub fn create_voice_router(pool: sqlx::PgPool) -> axum::Router<()> {
    use axum::routing::{get, post};

    let state = Arc::new(VoiceMatcherState::new(pool));

    axum::Router::new()
        .route("/api/voice/match", post(match_voice_command))
        .route("/api/voice/outcome", post(record_voice_outcome))
        .route("/api/voice/match/batch", post(batch_match_voice_commands))
        .route("/api/voice/health", get(voice_health))
        .with_state(state)
}
