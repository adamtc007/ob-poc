//! Learning, disambiguation, and decision route handlers for the agent REST API.
//!
//! Includes correction reporting, verb disambiguation selection/abandonment,
//! phrase variant generation, and decision reply handling.

use crate::api::agent_state::AgentState;
use crate::api::agent_types::{ReportCorrectionRequest, ReportCorrectionResponse};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use sqlx::PgPool;
use uuid::Uuid;

// ============================================================================
// Correction Handlers
// ============================================================================

/// POST /api/agent/correction - Report a user correction for learning
///
/// Called by the UI when a user edits agent-generated DSL before executing.
/// This feeds into the continuous improvement loop.
pub(crate) async fn report_correction(
    State(state): State<AgentState>,
    Json(req): Json<ReportCorrectionRequest>,
) -> Json<ReportCorrectionResponse> {
    use crate::agent::learning::{AgentEvent, AgentEventPayload};

    tracing::info!(
        "Recording user correction for session {}: {} chars -> {} chars",
        req.session_id,
        req.generated_dsl.len(),
        req.corrected_dsl.len()
    );

    // Classify the correction type by analyzing the diff
    let correction_type = classify_correction(&req.generated_dsl, &req.corrected_dsl);

    // Build the event
    let event = AgentEvent {
        timestamp: chrono::Utc::now(),
        session_id: Some(req.session_id),
        payload: AgentEventPayload::UserCorrection {
            original_message: req.original_message.unwrap_or_default(),
            generated_dsl: req.generated_dsl,
            corrected_dsl: req.corrected_dsl,
            correction_type,
        },
    };

    // Store directly to database (fire-and-forget style, but we wait for event_id)
    let event_id = match store_correction_event(&state.pool, &event).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::error!("Failed to store correction event: {}", e);
            None
        }
    };

    Json(ReportCorrectionResponse {
        recorded: event_id.is_some(),
        event_id,
    })
}

/// Classify the type of correction by analyzing the diff
fn classify_correction(generated: &str, corrected: &str) -> crate::agent::learning::CorrectionType {
    use crate::agent::learning::CorrectionType;

    // Simple heuristics - can be made more sophisticated
    let gen_lines: Vec<&str> = generated.lines().collect();
    let cor_lines: Vec<&str> = corrected.lines().collect();

    // Check for full rewrite (very different)
    let similarity = compute_line_similarity(&gen_lines, &cor_lines);
    if similarity < 0.3 {
        return CorrectionType::FullRewrite;
    }

    // Check for additions (corrected has more content)
    if cor_lines.len() > gen_lines.len() && corrected.contains(generated.trim()) {
        let added = corrected.replace(generated.trim(), "").trim().to_string();
        if !added.is_empty() {
            return CorrectionType::Addition { added };
        }
    }

    // Check for removals (generated has more content)
    if gen_lines.len() > cor_lines.len() && generated.contains(corrected.trim()) {
        let removed = generated.replace(corrected.trim(), "").trim().to_string();
        if !removed.is_empty() {
            return CorrectionType::Removal { removed };
        }
    }

    // Check for verb changes (look for domain.verb pattern changes)
    let gen_verbs: Vec<&str> = gen_lines.iter().filter_map(|l| extract_verb(l)).collect();
    let cor_verbs: Vec<&str> = cor_lines.iter().filter_map(|l| extract_verb(l)).collect();

    if gen_verbs.len() == 1 && cor_verbs.len() == 1 && gen_verbs[0] != cor_verbs[0] {
        return CorrectionType::VerbChange {
            from_verb: gen_verbs[0].to_string(),
            to_verb: cor_verbs[0].to_string(),
        };
    }

    // Default to full rewrite if we can't classify more specifically
    CorrectionType::FullRewrite
}

/// Extract verb from a DSL line like "(domain.verb ...)"
fn extract_verb(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    // Find the verb: between '(' and first space or ')'
    let start = 1;
    let end = trimmed[start..]
        .find(|c: char| c.is_whitespace() || c == ')')
        .map(|i| i + start)?;
    Some(&trimmed[start..end])
}

/// Compute simple line-based similarity (0.0 to 1.0)
fn compute_line_similarity(a: &[&str], b: &[&str]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_set: std::collections::HashSet<&str> = a.iter().map(|s| s.trim()).collect();
    let b_set: std::collections::HashSet<&str> = b.iter().map(|s| s.trim()).collect();

    let intersection = a_set.intersection(&b_set).count();
    let union = a_set.union(&b_set).count();

    if union == 0 {
        return 1.0;
    }

    intersection as f32 / union as f32
}

/// Store correction event directly to database
async fn store_correction_event(
    pool: &PgPool,
    event: &crate::agent::learning::AgentEvent,
) -> Result<i64, sqlx::Error> {
    use crate::agent::learning::AgentEventPayload;

    let event_type = event.payload.event_type_str();

    // Extract fields from the UserCorrection payload
    let (user_message, generated_dsl, corrected_dsl, correction_type) =
        if let AgentEventPayload::UserCorrection {
            ref original_message,
            ref generated_dsl,
            ref corrected_dsl,
            ref correction_type,
        } = event.payload
        {
            (
                Some(original_message.clone()),
                Some(generated_dsl.clone()),
                Some(corrected_dsl.clone()),
                Some(format!("{:?}", correction_type)),
            )
        } else {
            (None, None, None, None)
        };

    let event_id = sqlx::query_scalar!(
        r#"
        INSERT INTO agent.events (
            session_id, event_type, user_message, generated_dsl,
            corrected_dsl, correction_type, was_corrected
        )
        VALUES ($1, $2, $3, $4, $5, $6, true)
        RETURNING id
        "#,
        event.session_id,
        event_type,
        user_message,
        generated_dsl,
        corrected_dsl,
        correction_type,
    )
    .fetch_one(pool)
    .await?;

    tracing::debug!("Stored correction event with ID {}", event_id);

    Ok(event_id)
}

// ============================================================================
// Verb Disambiguation
// ============================================================================

/// POST /api/session/:id/select-verb
///
/// RETIRED: This endpoint bypassed orchestrator SemReg + PolicyGate.
/// All verb selection now flows through `/decision/reply` -> orchestrator forced-verb.
/// Returns 410 Gone.
pub(crate) async fn select_verb_disambiguation(
    State(_state): State<AgentState>,
    Path(_session_id): Path<Uuid>,
    Json(_req): Json<ob_poc_types::VerbSelectionRequest>,
) -> Result<Json<ob_poc_types::VerbSelectionResponse>, StatusCode> {
    tracing::warn!("RETIRED endpoint /select-verb called -- returning 410 Gone");
    Err(StatusCode::GONE)
}

/// Record verb selection as gold-standard learning signal
///
/// This is HIGH CONFIDENCE data (confidence=0.95) because:
/// - User was shown multiple options
/// - User explicitly clicked one
/// - This is an active correction, not passive acceptance
///
/// Uses agent.user_learned_phrases table for immediate effect on verb search.
/// Uses a "global" user_id (all zeros) since this is system-wide learning.
///
/// Also generates and stores phrase variants (confidence=0.85) to make learning
/// more robust to phrasings like "show me the cbus" vs "list all cbus".
pub async fn record_verb_selection_signal(
    pool: &PgPool,
    original_input: &str,
    selected_verb: &str,
    all_candidates: &[String],
) -> Result<(), sqlx::Error> {
    // Use a "global" user_id for system-wide disambiguation learning
    // This allows the learning to benefit all users immediately
    let global_user_id = Uuid::nil(); // 00000000-0000-0000-0000-000000000000

    // Insert primary phrase with gold-standard confidence (0.95)
    sqlx::query!(
        r#"
        INSERT INTO agent.user_learned_phrases (
            user_id,
            phrase,
            verb,
            occurrence_count,
            confidence,
            source,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, 1, 0.95, 'user_disambiguation', NOW(), NOW())
        ON CONFLICT (user_id, phrase)
        DO UPDATE SET
            occurrence_count = agent.user_learned_phrases.occurrence_count + 1,
            confidence = GREATEST(agent.user_learned_phrases.confidence, 0.95),
            verb = EXCLUDED.verb,
            updated_at = NOW()
        "#,
        global_user_id,
        original_input,
        selected_verb,
    )
    .execute(pool)
    .await?;

    // Generate and store phrase variants with slightly lower confidence (0.85)
    // This addresses the "too literal" learning failure case
    let variants = generate_phrase_variants(original_input);
    let mut variants_stored = 0;
    for variant in &variants {
        if variant != original_input {
            sqlx::query!(
                r#"
                INSERT INTO agent.user_learned_phrases (
                    user_id,
                    phrase,
                    verb,
                    occurrence_count,
                    confidence,
                    source,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, 1, 0.85, 'generated_variant', NOW(), NOW())
                ON CONFLICT (user_id, phrase)
                DO UPDATE SET
                    occurrence_count = agent.user_learned_phrases.occurrence_count + 1,
                    confidence = GREATEST(agent.user_learned_phrases.confidence, 0.85),
                    updated_at = NOW()
                "#,
                global_user_id,
                variant,
                selected_verb,
            )
            .execute(pool)
            .await?;
            variants_stored += 1;
        }
    }

    // Record to phrase_blocklist for rejected alternatives
    // This prevents the same phrase from matching wrong verbs in future
    for candidate in all_candidates {
        if candidate != selected_verb {
            // Add to blocklist with reason
            // Schema: phrase, blocked_verb, user_id, reason, embedding, embedding_model, expires_at, created_at
            sqlx::query!(
                r#"
                INSERT INTO agent.phrase_blocklist (
                    phrase,
                    blocked_verb,
                    reason,
                    created_at
                )
                VALUES ($1, $2, 'user_disambiguation_rejected', NOW())
                ON CONFLICT (phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid)) DO NOTHING
                "#,
                original_input,
                candidate,
            )
            .execute(pool)
            .await?;
        }
    }

    tracing::info!(
        "Recorded disambiguation learning: '{}' -> '{}' ({} variants, blocked {} alternatives)",
        original_input,
        selected_verb,
        variants_stored,
        all_candidates.len() - 1
    );

    Ok(())
}

/// Generate phrase variants for more robust learning
///
/// Addresses the failure case where "list all cbus" was learned
/// but "show me the cbus" wasn't recognized.
///
/// One disambiguation teaches multiple phrasings:
/// - "list all cbus" -> cbu.list (0.95 confidence)
/// - "list cbu"      -> cbu.list (0.85 confidence)  // generated
/// - "show all cbus" -> cbu.list (0.85 confidence)  // generated
/// - "show cbus"     -> cbu.list (0.85 confidence)  // generated
fn generate_phrase_variants(phrase: &str) -> Vec<String> {
    // MAX 5 VARIANTS (prevent pollution per TODO spec)
    const MAX_VARIANTS: usize = 5;
    // MIN 2 tokens (quality filter per TODO spec)
    const MIN_TOKENS: usize = 2;

    let mut variants = vec![phrase.to_string()];
    let lower = phrase.to_lowercase();

    // Plural normalization (cbus -> cbu, entities -> entity)
    if lower.contains("cbus") {
        variants.push(lower.replace("cbus", "cbu"));
    }
    if lower.contains("entities") {
        variants.push(lower.replace("entities", "entity"));
    }

    // Common verb swaps
    let verb_swaps = [
        ("list", "show"),
        ("show", "list"),
        ("display", "show"),
        ("get", "list"),
        ("view", "show"),
        ("find", "search"),
        ("search", "find"),
    ];
    for (from, to) in verb_swaps {
        if lower.starts_with(from) || lower.contains(&format!(" {}", from)) {
            let swapped = lower.replace(from, to);
            if !variants.contains(&swapped) {
                variants.push(swapped);
            }
        }
    }

    // Article/quantifier removal
    let stripped = lower
        .replace(" the ", " ")
        .replace(" all ", " ")
        .replace(" my ", " ")
        .replace("  ", " ")
        .trim()
        .to_string();
    if stripped != lower && !variants.contains(&stripped) {
        variants.push(stripped);
    }

    // Also try with articles removed at start
    let prefixes_to_strip = ["show me ", "list all ", "get all ", "display all "];
    for prefix in prefixes_to_strip {
        if lower.starts_with(prefix) {
            let without_prefix = lower.strip_prefix(prefix).unwrap_or(&lower).to_string();
            if !without_prefix.is_empty() && !variants.contains(&without_prefix) {
                variants.push(without_prefix);
            }
        }
    }

    // Dedupe and sort
    variants.sort();
    variants.dedup();

    // Quality filter: Min 2 tokens, not generic alone
    let filtered: Vec<String> = variants
        .into_iter()
        .filter(|v| {
            let tokens: Vec<&str> = v.split_whitespace().collect();
            // Must have at least MIN_TOKENS words
            if tokens.len() < MIN_TOKENS {
                return false;
            }
            // Not just generic stopwords
            let generic_only = tokens.iter().all(|t| {
                matches!(
                    *t,
                    "the"
                        | "a"
                        | "an"
                        | "all"
                        | "my"
                        | "this"
                        | "that"
                        | "please"
                        | "can"
                        | "you"
                        | "i"
                        | "me"
                        | "show"
                        | "list"
                        | "get"
                )
            });
            !generic_only
        })
        .collect();

    // Apply MAX_VARIANTS limit - always include original if it passed filter
    let result: Vec<String> = filtered.into_iter().take(MAX_VARIANTS).collect();

    // If original passed filter, ensure it's first
    if result.contains(&phrase.to_string()) {
        let mut final_result = vec![phrase.to_string()];
        for v in result {
            if v != phrase && final_result.len() < MAX_VARIANTS {
                final_result.push(v);
            }
        }
        final_result
    } else if result.is_empty() {
        // Fallback: return original even if short
        vec![phrase.to_string()]
    } else {
        result
    }
}

// ============================================================================
// Decision Reply
// ============================================================================

/// POST /api/session/:id/decision/reply
///
/// Unified endpoint for all decision packet responses.
/// Handles: Select (A/B/C), Confirm (token), Type (free text), Narrow (filter), Cancel
///
/// This is the NEW unified path that will eventually replace:
/// - /api/session/:id/select-verb
/// - /api/session/:id/abandon-disambiguation
/// - /api/session/:id/select-intent-tier
pub(crate) async fn handle_decision_reply(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ob_poc_types::DecisionReplyRequest>,
) -> Result<Json<ob_poc_types::DecisionReplyResponse>, StatusCode> {
    use crate::clarify::{validate_confirm_token, ConfirmTokenError};
    use ob_poc_types::{DecisionKind, DecisionReplyResponse, UserReply};

    tracing::info!(
        session_id = %session_id,
        packet_id = %req.packet_id,
        "Handling decision reply"
    );

    // Get session
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Take the pending decision packet (moves ownership to avoid borrow issues)
    let packet = session.pending_decision.take().ok_or_else(|| {
        tracing::warn!(session_id = %session_id, "No pending decision");
        StatusCode::BAD_REQUEST
    })?;

    // Verify packet_id matches
    if packet.packet_id != req.packet_id {
        tracing::warn!(
            expected = %packet.packet_id,
            received = %req.packet_id,
            "Packet ID mismatch"
        );
        // Put it back since we're rejecting
        session.pending_decision = Some(packet);
        return Err(StatusCode::CONFLICT);
    }

    // Handle based on reply type (using ob_poc_types::UserReply)
    let response = match &req.reply {
        UserReply::Select { index } => {
            // User selected an option (A/B/C)
            let choice = packet.choices.get(*index).ok_or_else(|| {
                tracing::warn!(
                    index = index,
                    max = packet.choices.len(),
                    "Invalid selection"
                );
                StatusCode::BAD_REQUEST
            })?;

            tracing::info!(
                choice_id = %choice.id,
                choice_label = %choice.label,
                "User selected option"
            );

            // Route based on decision kind
            let message = match &packet.kind {
                DecisionKind::ClarifyVerb => {
                    // Extract verb_fqn from the VerbPayload
                    let verb_fqn =
                        if let ob_poc_types::ClarificationPayload::Verb(ref vp) = packet.payload {
                            // choice.id is the index (1-based string); map to verb option
                            choice
                                .id
                                .parse::<usize>()
                                .ok()
                                .and_then(|idx| vp.options.get(idx.saturating_sub(1)))
                                .map(|opt| opt.verb_fqn.clone())
                        } else {
                            None
                        };

                    if let Some(fqn) = verb_fqn {
                        let original_utterance = packet.utterance.clone();
                        let actor = crate::policy::ActorResolver::from_headers(&headers);

                        // Route through orchestrator forced-verb path
                        match state
                            .agent_service
                            .process_forced_verb_selection(
                                session,
                                &original_utterance,
                                &fqn,
                                actor,
                            )
                            .await
                        {
                            Ok(resp) => {
                                tracing::info!(
                                    verb = %fqn,
                                    dsl = ?resp.dsl_source,
                                    "ClarifyVerb: forced-verb selection through orchestrator"
                                );
                                resp.message.clone()
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "ClarifyVerb forced-verb failed");
                                format!("Failed to generate DSL for {}: {}", fqn, e)
                            }
                        }
                    } else {
                        tracing::warn!("ClarifyVerb: could not extract verb_fqn from payload");
                        format!("Selected verb option: {}", choice.label)
                    }
                }
                DecisionKind::ClarifyGroup => {
                    // Handle client group selection
                    if let ob_poc_types::ClarificationPayload::Group(group_payload) =
                        &packet.payload
                    {
                        // Find the selected group by index
                        if let Ok(idx) = choice.id.parse::<usize>() {
                            if let Some(group) = group_payload.options.get(idx.saturating_sub(1)) {
                                // Set client group context in session
                                if let Ok(group_uuid) = uuid::Uuid::parse_str(&group.id) {
                                    let scope = crate::mcp::scope_resolution::ScopeContext::new()
                                        .with_client_group(group_uuid, group.alias.clone());
                                    session.context.set_client_scope(scope);
                                    format!("Now working with client: {}", group.alias)
                                } else {
                                    "Invalid group ID".to_string()
                                }
                            } else {
                                format!("Selected client: {}", choice.label)
                            }
                        } else {
                            format!("Selected client: {}", choice.label)
                        }
                    } else {
                        format!("Selected client: {}", choice.label)
                    }
                }
                DecisionKind::ClarifyScope => {
                    // Check if this is a Semantic OS workflow selection
                    let is_semos = packet.trace.decision_reason == "semos_workflow_selection";
                    if is_semos {
                        // Map choice → stage_focus for verb phase_tag filtering
                        let stage_focus = match choice.id.as_str() {
                            "1" => "semos-onboarding",
                            "2" => "semos-kyc",
                            "3" => "semos-data-management",
                            "4" => "semos-stewardship",
                            _ => "semos-data-management", // safe default
                        };
                        session.context.stage_focus = Some(stage_focus.to_string());
                        tracing::info!(
                            session_id = %session_id,
                            stage_focus = %stage_focus,
                            workflow = %choice.label,
                            "Semantic OS workflow selected — stage_focus set"
                        );
                        format!(
                            "Great, let's work on **{}**. I'll focus on {} verbs and tools. How can I help?",
                            choice.label,
                            choice.label.to_lowercase()
                        )
                    } else {
                        format!("Selected scope: {}", choice.label)
                    }
                }
                DecisionKind::ClarifyDeal => {
                    // Handle deal selection
                    if choice.id == "NEW" {
                        // User wants to create a new deal
                        "Let's create a new deal. What would you like to name it?".to_string()
                    } else if choice.id == "SKIP" {
                        // User wants to skip deal context
                        session.context.deal_id = None;
                        session.context.deal_name = None;
                        "Continuing without deal context. You can set one later with 'load deal'."
                            .to_string()
                    } else {
                        // User selected an existing deal - extract from payload
                        if let ob_poc_types::ClarificationPayload::Deal(deal_payload) =
                            &packet.payload
                        {
                            if let Ok(idx) = choice.id.parse::<usize>() {
                                if let Some(deal) = deal_payload.deals.get(idx.saturating_sub(1)) {
                                    // Set deal context in session
                                    if let Ok(deal_uuid) = uuid::Uuid::parse_str(&deal.deal_id) {
                                        session.context.deal_id = Some(deal_uuid);
                                        session.context.deal_name = Some(deal.deal_name.clone());
                                        format!("Now working on deal: {}", deal.deal_name)
                                    } else {
                                        "Invalid deal ID".to_string()
                                    }
                                } else {
                                    format!("Selected deal: {}", choice.label)
                                }
                            } else {
                                format!("Selected deal: {}", choice.label)
                            }
                        } else {
                            format!("Selected deal: {}", choice.label)
                        }
                    }
                }
                _ => format!("Selected: {}", choice.label),
            };

            DecisionReplyResponse {
                next_packet: None,
                execution_result: None,
                message,
                complete: true,
            }
        }

        UserReply::Confirm { token } => {
            // User confirmed execution
            if let Some(expected_token) = packet.confirm_token.as_ref() {
                // Validate token if provided
                if let Some(user_token) = token {
                    validate_confirm_token(user_token, expected_token, None).map_err(|e| {
                        match e {
                            ConfirmTokenError::Expired => {
                                tracing::warn!("Confirm token expired");
                                StatusCode::GONE // 410 Gone - token expired
                            }
                            ConfirmTokenError::Mismatch => {
                                tracing::warn!("Confirm token mismatch");
                                StatusCode::UNAUTHORIZED
                            }
                            _ => StatusCode::BAD_REQUEST,
                        }
                    })?;
                }
            }

            tracing::info!("Execution confirmed");

            // Packet already taken at start of handler
            // TODO: Execute the DSL and return result
            DecisionReplyResponse {
                next_packet: None,
                execution_result: None,
                message: "Execution confirmed".to_string(),
                complete: true,
            }
        }

        UserReply::TypeExact { text } => {
            // User typed exact text - treat as new input
            tracing::info!(text = %text, "User typed exact text");

            // Packet already taken at start of handler
            DecisionReplyResponse {
                next_packet: None,
                execution_result: None,
                message: format!("Processing: {}", text),
                complete: true,
            }
        }

        UserReply::Narrow { term } => {
            // User wants to narrow/filter
            tracing::info!(term = %term, "User wants to narrow search");

            // Put packet back since we're continuing the flow
            // (Could filter options here and return modified packet)
            DecisionReplyResponse {
                next_packet: Some(Box::new(packet.clone())),
                execution_result: None,
                message: format!("Narrowing by: {}", term),
                complete: false,
            }
        }

        UserReply::More { kind } => {
            // User wants more options
            tracing::info!(kind = ?kind, "User wants more options");

            // Put packet back since we're continuing the flow
            DecisionReplyResponse {
                next_packet: Some(Box::new(packet.clone())),
                execution_result: None,
                message: "Showing more options".to_string(),
                complete: false,
            }
        }

        UserReply::Cancel => {
            // User cancelled
            tracing::info!("User cancelled decision");

            // Packet already taken at start of handler
            DecisionReplyResponse {
                next_packet: None,
                execution_result: None,
                message: "Cancelled".to_string(),
                complete: true,
            }
        }
    };

    Ok(Json(response))
}
