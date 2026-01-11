//! Agent Controller
//!
//! Orchestrates the agent loop for research workflows (020 pattern).
//! Connects LLM reasoning to DSL execution via the REPL.
//!
//! # Architecture
//!
//! ```text
//! AgentController
//!     ├── session (RwLock<UnifiedSessionContext>)
//!     ├── pool (database)
//!     ├── llm_client (for orchestration prompts)
//!     └── event_tx (for UI updates)
//!
//! Loop:
//! 1. Check agent status (running/paused/checkpoint)
//! 2. Identify gaps using DSL
//! 3. LLM selects strategy (source, search terms)
//! 4. Execute search, evaluate candidates
//! 5. If confident: auto-import with audit
//! 6. If ambiguous: checkpoint for user
//! 7. Repeat until complete
//! ```

use std::sync::Arc;

use anyhow::Result;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::session::{
    AgentState, AgentStatus, AgentTask, Candidate, Checkpoint, SessionMode, UnifiedSessionContext,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Events emitted by agent for UI updates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent started
    Started {
        agent_session_id: Uuid,
        task: String,
    },

    /// Agent executing DSL
    Executing { dsl: String },

    /// DSL execution completed
    Executed {
        dsl: String,
        success: bool,
        result_summary: Option<String>,
    },

    /// Agent searching external source
    Searching { source: String, query: String },

    /// Search completed with candidates
    SearchComplete {
        source: String,
        candidates_count: usize,
    },

    /// Checkpoint created - awaiting user input
    CheckpointCreated {
        checkpoint_id: Uuid,
        checkpoint_type: String,
        candidates: Vec<Candidate>,
    },

    /// Checkpoint resolved by user
    CheckpointResolved { checkpoint_id: Uuid, action: String },

    /// Agent iteration completed
    IterationComplete {
        iteration: u32,
        gaps_remaining: usize,
    },

    /// Agent paused
    Paused,

    /// Agent resumed
    Resumed,

    /// Agent completed successfully
    Completed {
        decisions_made: usize,
        actions_taken: usize,
    },

    /// Agent failed
    Failed { error: String },

    /// Agent cancelled by user
    Cancelled,

    /// Progress update
    Progress { message: String },
}

/// Confidence thresholds for auto-selection
#[derive(Debug, Clone)]
pub struct ConfidenceConfig {
    /// Score >= this auto-proceeds without user confirmation
    pub auto_proceed_threshold: f64,

    /// Score < this is rejected (try next source)
    pub reject_threshold: f64,

    /// Scores between reject and auto_proceed trigger checkpoint
    pub force_checkpoint_contexts: Vec<String>,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            auto_proceed_threshold: 0.90,
            reject_threshold: 0.70,
            force_checkpoint_contexts: vec![
                "NEW_CLIENT".to_string(),
                "MATERIAL_HOLDING".to_string(),
                "SCREENING_HIT".to_string(),
            ],
        }
    }
}

/// Result of a strategy execution
#[derive(Debug)]
pub enum StrategyResult {
    /// Successfully imported data
    Imported {
        decision_id: Uuid,
        action_id: Uuid,
        entities_created: i32,
    },

    /// Need user confirmation
    NeedsCheckpoint(Checkpoint),

    /// No match found, should try next source or skip
    NoMatch { reason: String },

    /// Error during execution
    Error { message: String },
}

/// Agent controller - orchestrates the research loop
pub struct AgentController {
    /// Shared session state
    session: Arc<RwLock<UnifiedSessionContext>>,

    /// Database pool for DSL execution
    #[cfg(feature = "database")]
    pool: PgPool,

    /// Event channel for UI updates
    event_tx: mpsc::Sender<AgentEvent>,

    /// Confidence configuration
    config: ConfidenceConfig,
}

impl AgentController {
    /// Create a new agent controller
    #[cfg(feature = "database")]
    pub fn new(
        session: Arc<RwLock<UnifiedSessionContext>>,
        pool: PgPool,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Self {
        Self {
            session,
            pool,
            event_tx,
            config: ConfidenceConfig::default(),
        }
    }

    /// Create with custom confidence config
    #[cfg(feature = "database")]
    pub fn with_config(
        session: Arc<RwLock<UnifiedSessionContext>>,
        pool: PgPool,
        event_tx: mpsc::Sender<AgentEvent>,
        config: ConfidenceConfig,
    ) -> Self {
        Self {
            session,
            pool,
            event_tx,
            config,
        }
    }

    /// Start the agent with a task
    #[cfg(feature = "database")]
    pub async fn start(&self, task: AgentTask, target_entity_id: Option<Uuid>) -> Result<Uuid> {
        let agent_session_id = {
            let mut session = self.session.write().await;

            // Create agent state based on task
            let state = match (&task, target_entity_id) {
                (AgentTask::ResolveGaps, Some(id)) => AgentState::resolve_gaps(id),
                (AgentTask::ChainResearch, Some(id)) => AgentState::chain_research(id),
                (AgentTask::EnrichEntity, Some(id)) => AgentState::enrich_entity(id),
                (AgentTask::EnrichGroup, Some(id)) => AgentState::enrich_group(id),
                _ => AgentState::new(task.clone()),
            };

            let agent_session_id = state.agent_session_id;
            session.agent = Some(state);
            session.mode = SessionMode::Agent;

            agent_session_id
        };

        // Emit start event
        let _ = self
            .event_tx
            .send(AgentEvent::Started {
                agent_session_id,
                task: task.to_string(),
            })
            .await;

        // Spawn the loop
        let controller = AgentControllerHandle {
            session: self.session.clone(),
            pool: self.pool.clone(),
            event_tx: self.event_tx.clone(),
            config: self.config.clone(),
        };

        tokio::spawn(async move {
            if let Err(e) = controller.run_loop().await {
                tracing::error!("Agent loop error: {}", e);
                let _ = controller
                    .event_tx
                    .send(AgentEvent::Failed {
                        error: e.to_string(),
                    })
                    .await;
            }
        });

        Ok(agent_session_id)
    }

    /// Pause the agent
    pub async fn pause(&self) {
        let mut session = self.session.write().await;
        if let Some(agent) = &mut session.agent {
            agent.pause();
        }
        let _ = self.event_tx.send(AgentEvent::Paused).await;
    }

    /// Resume the agent
    pub async fn resume(&self) {
        let mut session = self.session.write().await;
        if let Some(agent) = &mut session.agent {
            agent.resume();
        }
        let _ = self.event_tx.send(AgentEvent::Resumed).await;
    }

    /// Stop the agent
    pub async fn stop(&self) {
        let mut session = self.session.write().await;
        if let Some(agent) = &mut session.agent {
            agent.cancel();
        }
        session.mode = SessionMode::Manual;
        let _ = self.event_tx.send(AgentEvent::Cancelled).await;
    }

    /// Respond to a checkpoint
    #[cfg(feature = "database")]
    pub async fn respond_checkpoint(&self, response: CheckpointResponse) -> Result<()> {
        let checkpoint = {
            let mut session = self.session.write().await;
            session
                .agent
                .as_mut()
                .and_then(|a| a.pending_checkpoint.take())
        };

        if let Some(checkpoint) = checkpoint {
            match response {
                CheckpointResponse::Select { index } => {
                    if index < checkpoint.candidates.len() {
                        let selected = &checkpoint.candidates[index];

                        // Record decision and execute import
                        self.execute_selected_import(
                            &checkpoint.context.source,
                            &selected.key,
                            &selected.key_type,
                            selected.score,
                            checkpoint.context.target_entity_id,
                            "USER_SELECTED",
                        )
                        .await?;

                        let _ = self
                            .event_tx
                            .send(AgentEvent::CheckpointResolved {
                                checkpoint_id: checkpoint.checkpoint_id,
                                action: format!("selected_{}", index),
                            })
                            .await;
                    }
                }
                CheckpointResponse::Reject => {
                    // Record rejection, continue to next source
                    let _ = self
                        .event_tx
                        .send(AgentEvent::CheckpointResolved {
                            checkpoint_id: checkpoint.checkpoint_id,
                            action: "rejected".to_string(),
                        })
                        .await;
                }
                CheckpointResponse::ManualOverride { key, key_type } => {
                    // Use manually provided key
                    self.execute_selected_import(
                        &checkpoint.context.source,
                        &key,
                        &key_type,
                        1.0,
                        checkpoint.context.target_entity_id,
                        "USER_OVERRIDE",
                    )
                    .await?;

                    let _ = self
                        .event_tx
                        .send(AgentEvent::CheckpointResolved {
                            checkpoint_id: checkpoint.checkpoint_id,
                            action: "manual_override".to_string(),
                        })
                        .await;
                }
                CheckpointResponse::Skip => {
                    // Skip this gap entirely
                    let _ = self
                        .event_tx
                        .send(AgentEvent::CheckpointResolved {
                            checkpoint_id: checkpoint.checkpoint_id,
                            action: "skipped".to_string(),
                        })
                        .await;
                }
            }

            // Resume the loop
            let mut session = self.session.write().await;
            if let Some(agent) = &mut session.agent {
                agent.status = AgentStatus::Running;
            }
        }

        Ok(())
    }

    /// Execute import after selection
    #[cfg(feature = "database")]
    async fn execute_selected_import(
        &self,
        source: &str,
        key: &str,
        key_type: &str,
        confidence: f64,
        target_entity_id: Option<Uuid>,
        decision_type: &str,
    ) -> Result<(Uuid, Uuid)> {
        // Record decision
        let decision_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO kyc.research_decisions
               (target_entity_id, search_query, source_provider, candidates_found,
                candidates_count, selected_key, selected_key_type, selection_confidence,
                selection_reasoning, decision_type, auto_selected)
               VALUES ($1, $2, $3, '[]'::jsonb, 1, $4, $5, $6, $7, $8, $9)
               RETURNING decision_id"#,
        )
        .bind(target_entity_id)
        .bind(key) // search query = the key for direct selection
        .bind(source)
        .bind(key)
        .bind(key_type)
        .bind(confidence)
        .bind(format!(
            "{} with confidence {:.2}",
            decision_type, confidence
        ))
        .bind(decision_type)
        .bind(decision_type == "AUTO_SELECTED")
        .fetch_one(&self.pool)
        .await?;

        // Execute import based on source
        let (verb_fqn, entities_created) = match source {
            "gleif" => {
                // Execute GLEIF enrich with decision-id
                let result = self
                    .execute_dsl(&format!(
                        r#"(gleif.enrich :lei "{}" :decision-id {})"#,
                        key, decision_id
                    ))
                    .await?;
                ("gleif:enrich", if result { 1 } else { 0 })
            }
            _ => {
                // Generic import path
                ("research.generic:import-entity", 0)
            }
        };

        // Record action
        let action_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO kyc.research_actions
               (decision_id, verb_fqn, result_summary, entities_created, entities_updated)
               VALUES ($1, $2, $3, $4, 0)
               RETURNING action_id"#,
        )
        .bind(decision_id)
        .bind(verb_fqn)
        .bind(serde_json::json!({"key": key, "source": source}))
        .bind(entities_created)
        .fetch_one(&self.pool)
        .await?;

        // Update agent state
        {
            let mut session = self.session.write().await;
            if let Some(agent) = &mut session.agent {
                agent.record_decision(decision_id, decision_type, source);
                agent.record_action(action_id, verb_fqn, true);
            }
        }

        Ok((decision_id, action_id))
    }

    /// Execute DSL via the session executor
    #[cfg(feature = "database")]
    async fn execute_dsl(&self, dsl: &str) -> Result<bool> {
        let _ = self
            .event_tx
            .send(AgentEvent::Executing {
                dsl: dsl.to_string(),
            })
            .await;

        // Parse and execute DSL
        let executor = crate::dsl_v2::DslExecutor::new(self.pool.clone());
        let mut ctx = crate::dsl_v2::ExecutionContext::new();

        match executor.execute_dsl(dsl, &mut ctx).await {
            Ok(_) => {
                let _ = self
                    .event_tx
                    .send(AgentEvent::Executed {
                        dsl: dsl.to_string(),
                        success: true,
                        result_summary: None,
                    })
                    .await;
                Ok(true)
            }
            Err(e) => {
                let _ = self
                    .event_tx
                    .send(AgentEvent::Executed {
                        dsl: dsl.to_string(),
                        success: false,
                        result_summary: Some(e.to_string()),
                    })
                    .await;
                Ok(false)
            }
        }
    }
}

/// Handle for the spawned agent loop
#[cfg(feature = "database")]
struct AgentControllerHandle {
    session: Arc<RwLock<UnifiedSessionContext>>,
    pool: PgPool,
    event_tx: mpsc::Sender<AgentEvent>,
    config: ConfidenceConfig,
}

#[cfg(feature = "database")]
impl AgentControllerHandle {
    /// Main agent loop
    async fn run_loop(&self) -> Result<()> {
        loop {
            // Check status
            let (status, iteration, max_iterations, target_entity_id) = {
                let session = self.session.read().await;
                match &session.agent {
                    Some(agent) => (
                        agent.status,
                        agent.loop_iteration,
                        agent.max_iterations,
                        agent.target_entity_id,
                    ),
                    None => break,
                }
            };

            match status {
                AgentStatus::Running => {
                    // Continue with loop
                }
                AgentStatus::Paused => {
                    // Wait for resume
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                AgentStatus::Checkpoint => {
                    // Wait for user response
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                AgentStatus::Complete | AgentStatus::Failed | AgentStatus::Cancelled => {
                    break;
                }
            }

            // Check iteration limit
            if iteration >= max_iterations {
                self.complete_with_limit_reached().await;
                break;
            }

            // Increment iteration
            {
                let mut session = self.session.write().await;
                if let Some(agent) = &mut session.agent {
                    agent.increment_iteration();
                }
            }

            // Step 1: Identify gaps
            let gaps = self.identify_gaps(target_entity_id).await?;

            if gaps.is_empty() {
                // No more gaps - we're done!
                self.complete_successfully().await;
                break;
            }

            let _ = self
                .event_tx
                .send(AgentEvent::Progress {
                    message: format!("Found {} gaps to resolve", gaps.len()),
                })
                .await;

            // Step 2: Process first gap
            let gap = &gaps[0];
            let result = self.resolve_gap(gap).await?;

            match result {
                StrategyResult::Imported { .. } => {
                    // Success - continue to next iteration
                    let _ = self
                        .event_tx
                        .send(AgentEvent::IterationComplete {
                            iteration: iteration + 1,
                            gaps_remaining: gaps.len() - 1,
                        })
                        .await;
                }
                StrategyResult::NeedsCheckpoint(checkpoint) => {
                    // Set checkpoint and wait
                    let checkpoint_id = checkpoint.checkpoint_id;
                    let candidates = checkpoint.candidates.clone();
                    let checkpoint_type = format!("{:?}", checkpoint.checkpoint_type);

                    {
                        let mut session = self.session.write().await;
                        if let Some(agent) = &mut session.agent {
                            agent.set_checkpoint(checkpoint);
                        }
                    }

                    let _ = self
                        .event_tx
                        .send(AgentEvent::CheckpointCreated {
                            checkpoint_id,
                            checkpoint_type,
                            candidates,
                        })
                        .await;

                    // Loop will wait at Checkpoint status
                }
                StrategyResult::NoMatch { reason } => {
                    // Log and continue
                    let _ = self
                        .event_tx
                        .send(AgentEvent::Progress {
                            message: format!("No match for gap: {}", reason),
                        })
                        .await;
                }
                StrategyResult::Error { message } => {
                    // Log error but continue
                    tracing::warn!("Gap resolution error: {}", message);
                }
            }
        }

        Ok(())
    }

    /// Identify ownership gaps for target entity
    async fn identify_gaps(&self, target_entity_id: Option<Uuid>) -> Result<Vec<OwnershipGap>> {
        let Some(entity_id) = target_entity_id else {
            return Ok(vec![]);
        };

        // Query for entities with missing parent relationships
        let gaps: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
            r#"SELECT e.entity_id, e.name, lc.lei
               FROM "ob-poc".entities e
               LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
               LEFT JOIN "ob-poc".entity_relationships r
                   ON r.target_entity_id = e.entity_id
                   AND r.relationship_type = 'OWNERSHIP'
               WHERE e.entity_id = $1 OR e.entity_id IN (
                   SELECT target_entity_id FROM "ob-poc".entity_relationships
                   WHERE source_entity_id = $1
               )
               AND r.relationship_id IS NULL
               AND lc.lei IS NULL
               LIMIT 10"#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(gaps
            .into_iter()
            .map(|(id, name, lei)| OwnershipGap {
                entity_id: id,
                entity_name: name,
                existing_lei: lei,
                gap_type: "MISSING_PARENT".to_string(),
            })
            .collect())
    }

    /// Resolve a single ownership gap
    async fn resolve_gap(&self, gap: &OwnershipGap) -> Result<StrategyResult> {
        let _ = self
            .event_tx
            .send(AgentEvent::Searching {
                source: "gleif".to_string(),
                query: gap.entity_name.clone(),
            })
            .await;

        // Search GLEIF for the entity
        let candidates = self.search_gleif(&gap.entity_name).await?;

        let _ = self
            .event_tx
            .send(AgentEvent::SearchComplete {
                source: "gleif".to_string(),
                candidates_count: candidates.len(),
            })
            .await;

        if candidates.is_empty() {
            return Ok(StrategyResult::NoMatch {
                reason: "No GLEIF matches found".to_string(),
            });
        }

        // Evaluate best candidate
        let best = &candidates[0];

        if best.score >= self.config.auto_proceed_threshold {
            // Auto-proceed
            let (decision_id, action_id) = self
                .execute_import(
                    "gleif",
                    &best.key,
                    &best.key_type,
                    best.score,
                    Some(gap.entity_id),
                    "AUTO_SELECTED",
                )
                .await?;

            Ok(StrategyResult::Imported {
                decision_id,
                action_id,
                entities_created: 1,
            })
        } else if best.score >= self.config.reject_threshold {
            // Ambiguous - need checkpoint
            Ok(StrategyResult::NeedsCheckpoint(
                Checkpoint::ambiguous_match(
                    gap.entity_name.clone(),
                    "gleif".to_string(),
                    candidates,
                    Some(gap.entity_id),
                ),
            ))
        } else {
            Ok(StrategyResult::NoMatch {
                reason: format!("Best match score {} below threshold", best.score),
            })
        }
    }

    /// Search GLEIF for candidates
    async fn search_gleif(&self, name: &str) -> Result<Vec<Candidate>> {
        // Use the GLEIF client
        let client = crate::gleif::GleifClient::new()?;
        let results = client.search_by_name(name, 5).await?;

        Ok(results
            .iter()
            .enumerate()
            .map(|(i, record)| {
                let lei = record.lei();
                let record_name = &record.attributes.entity.legal_name.name;

                // Simple fuzzy score based on name similarity
                let score = calculate_name_similarity(name, record_name);

                Candidate {
                    key: lei.to_string(),
                    key_type: "LEI".to_string(),
                    name: record_name.clone(),
                    jurisdiction: record.attributes.entity.jurisdiction.clone(),
                    score,
                    details: Some(serde_json::json!({
                        "rank": i + 1,
                        "status": record.attributes.entity.status,
                        "category": record.attributes.entity.category,
                    })),
                }
            })
            .collect())
    }

    /// Execute import with audit trail
    async fn execute_import(
        &self,
        source: &str,
        key: &str,
        key_type: &str,
        confidence: f64,
        target_entity_id: Option<Uuid>,
        decision_type: &str,
    ) -> Result<(Uuid, Uuid)> {
        // Record decision
        let decision_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO kyc.research_decisions
               (target_entity_id, search_query, source_provider, candidates_found,
                candidates_count, selected_key, selected_key_type, selection_confidence,
                selection_reasoning, decision_type, auto_selected)
               VALUES ($1, $2, $3, '[]'::jsonb, 1, $4, $5, $6, $7, $8, $9)
               RETURNING decision_id"#,
        )
        .bind(target_entity_id)
        .bind(key)
        .bind(source)
        .bind(key)
        .bind(key_type)
        .bind(confidence)
        .bind(format!(
            "{} with confidence {:.2}",
            decision_type, confidence
        ))
        .bind(decision_type)
        .bind(decision_type == "AUTO_SELECTED")
        .fetch_one(&self.pool)
        .await?;

        // Execute GLEIF enrich
        let executor = crate::dsl_v2::DslExecutor::new(self.pool.clone());
        let mut ctx = crate::dsl_v2::ExecutionContext::new();

        let dsl = format!(
            r#"(gleif.enrich :lei "{}" :decision-id {})"#,
            key, decision_id
        );

        let _ = self
            .event_tx
            .send(AgentEvent::Executing { dsl: dsl.clone() })
            .await;

        let success = executor.execute_dsl(&dsl, &mut ctx).await.is_ok();

        let _ = self
            .event_tx
            .send(AgentEvent::Executed {
                dsl,
                success,
                result_summary: None,
            })
            .await;

        // Record action
        let action_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO kyc.research_actions
               (decision_id, verb_fqn, result_summary, entities_created, entities_updated)
               VALUES ($1, $2, $3, $4, 0)
               RETURNING action_id"#,
        )
        .bind(decision_id)
        .bind("gleif:enrich")
        .bind(serde_json::json!({"lei": key}))
        .bind(if success { 1 } else { 0 })
        .fetch_one(&self.pool)
        .await?;

        // Update agent state
        {
            let mut session = self.session.write().await;
            if let Some(agent) = &mut session.agent {
                agent.record_decision(decision_id, decision_type, source);
                agent.record_action(action_id, "gleif:enrich", success);
            }
        }

        Ok((decision_id, action_id))
    }

    /// Complete successfully
    async fn complete_successfully(&self) {
        let (decisions, actions) = {
            let mut session = self.session.write().await;
            if let Some(agent) = &mut session.agent {
                agent.complete();
                (agent.decisions.len(), agent.actions.len())
            } else {
                (0, 0)
            }
        };

        let _ = self
            .event_tx
            .send(AgentEvent::Completed {
                decisions_made: decisions,
                actions_taken: actions,
            })
            .await;
    }

    /// Complete with iteration limit reached
    async fn complete_with_limit_reached(&self) {
        {
            let mut session = self.session.write().await;
            if let Some(agent) = &mut session.agent {
                agent.complete();
            }
        }

        let _ = self
            .event_tx
            .send(AgentEvent::Progress {
                message: "Iteration limit reached".to_string(),
            })
            .await;

        self.complete_successfully().await;
    }
}

/// Response to a checkpoint
#[derive(Debug, Clone)]
pub enum CheckpointResponse {
    /// Select candidate by index
    Select { index: usize },

    /// Reject all candidates
    Reject,

    /// Provide manual key
    ManualOverride { key: String, key_type: String },

    /// Skip this gap
    Skip,
}

/// An ownership gap that needs resolution
#[allow(dead_code)] // Fields used for debugging/future expansion
#[derive(Debug, Clone)]
struct OwnershipGap {
    entity_id: Uuid,
    entity_name: String,
    existing_lei: Option<String>,
    gap_type: String,
}

/// Calculate simple name similarity score
fn calculate_name_similarity(query: &str, candidate: &str) -> f64 {
    let query_lower = query.to_lowercase();
    let candidate_lower = candidate.to_lowercase();

    // Exact match
    if query_lower == candidate_lower {
        return 1.0;
    }

    // Contains match
    if candidate_lower.contains(&query_lower) || query_lower.contains(&candidate_lower) {
        return 0.85;
    }

    // Word overlap score (Jaccard similarity)
    let query_words: std::collections::HashSet<&str> = query_lower.split_whitespace().collect();
    let candidate_words: std::collections::HashSet<&str> =
        candidate_lower.split_whitespace().collect();

    let intersection = query_words.intersection(&candidate_words).count();
    let union = query_words.union(&candidate_words).count();

    if union > 0 {
        // Scale Jaccard similarity: 0 overlap = 0.0, full overlap = 0.9
        0.9 * (intersection as f64 / union as f64)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_similarity() {
        assert!((calculate_name_similarity("Test Corp", "Test Corp") - 1.0).abs() < 0.01);
        assert!(calculate_name_similarity("Test", "Test Corporation") > 0.8);
        assert!(calculate_name_similarity("Allianz", "Allianz Global Investors") > 0.5);
        assert!(calculate_name_similarity("ABC", "XYZ") < 0.5);
    }

    #[test]
    fn test_confidence_config_default() {
        let config = ConfidenceConfig::default();
        assert!((config.auto_proceed_threshold - 0.90).abs() < 0.01);
        assert!((config.reject_threshold - 0.70).abs() < 0.01);
    }
}
