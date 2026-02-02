//! Runbook Service
//!
//! The "default stage mode" enforcement layer.
//!
//! # Critical Invariant
//!
//! **Normal prompts NEVER execute.** Only `runbook_run` with explicit user intent triggers execution.
//! This is the line that impresses risk/compliance folks.
//!
//! # Audit Trail
//!
//! Every operation emits structured events for bank-grade tracing:
//! - scope_resolution: client_group_id/persona + method + confidence
//! - runbook_change: insert/edit/remove + command_id + diff hash
//! - resolver: unresolved/ambiguous + candidate_count
//! - ready_gate: can_run true/false + blockers
//! - execution: runbook_id + exec_id + results per line

use anyhow::{anyhow, Result};
use sqlx::PgPool;
use std::time::Instant;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::dsl_v2::{parse_single_verb, AstNode, Literal, VerbCall};
use crate::session::canonical_hash::sha256;

use super::dag_analyzer::DagAnalyzer;
use super::events::{
    BlockingCommand, CommandResult, EntityFootprintEntry, LearnedTag, PickerCandidate, ReorderDiff,
    RunbookEvent, RunbookSummary, SearchMethod, StagedCommandSummary,
};
use super::repository::StagedRunbookRepository;
use super::resolver::{EntityArgResolver, MatchType, ResolutionOutcome};
use super::staged_runbook::{
    ResolutionSource, ResolutionStatus, ResolvedEntity, RunbookStatus, StagedRunbook,
};

/// Error types for staging operations
#[derive(Debug)]
pub enum StageError {
    /// DSL parse failed
    ParseFailed { error: String, dsl_raw: String },
    /// No client group context
    NoClientGroup,
    /// Runbook not found or not in building state
    InvalidRunbook {
        runbook_id: Uuid,
        status: RunbookStatus,
    },
    /// Database error
    Database(anyhow::Error),
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseFailed { error, .. } => write!(f, "Parse failed: {}", error),
            Self::NoClientGroup => write!(f, "No client group context for entity resolution"),
            Self::InvalidRunbook { runbook_id, status } => {
                write!(f, "Runbook {} is {:?}, not building", runbook_id, status)
            }
            Self::Database(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for StageError {}

/// Result of staging a command
#[derive(Debug)]
pub struct StageResult {
    pub command_id: Uuid,
    pub resolution_status: ResolutionStatus,
    pub entity_count: usize,
    pub dsl_hash: String,
}

/// Error types for run operations
#[derive(Debug)]
pub enum RunError {
    /// Runbook not ready (has blocking commands)
    NotReady { blockers: Vec<BlockingCommand> },
    /// Runbook not found
    NotFound { runbook_id: Uuid },
    /// Execution failed
    ExecutionFailed { command_id: Uuid, error: String },
    /// Database error
    Database(anyhow::Error),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady { blockers } => {
                write!(f, "Runbook not ready: {} blocking commands", blockers.len())
            }
            Self::NotFound { runbook_id } => write!(f, "Runbook {} not found", runbook_id),
            Self::ExecutionFailed { command_id, error } => {
                write!(f, "Command {} failed: {}", command_id, error)
            }
            Self::Database(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for RunError {}

/// Error types for pick operations
#[derive(Debug)]
pub enum PickError {
    /// No candidates stored for command
    NoCandidates { command_id: Uuid },
    /// Entity ID not in candidate set
    InvalidCandidate { entity_id: Uuid, message: String },
    /// Command not found
    CommandNotFound { command_id: Uuid },
    /// Database error
    Database(anyhow::Error),
}

impl std::fmt::Display for PickError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCandidates { command_id } => {
                write!(f, "No candidates for command {}", command_id)
            }
            Self::InvalidCandidate { entity_id, message } => {
                write!(f, "Invalid candidate {}: {}", entity_id, message)
            }
            Self::CommandNotFound { command_id } => {
                write!(f, "Command {} not found", command_id)
            }
            Self::Database(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for PickError {}

/// Result of picking entities
#[derive(Debug)]
pub struct PickResult {
    pub resolution_status: ResolutionStatus,
    pub selected_count: usize,
}

/// Runbook Service
///
/// Enforces the "default stage mode" - normal prompts stage, never execute.
pub struct RunbookService<'a> {
    pool: &'a PgPool,
    /// Event sink for audit trail
    events: Vec<RunbookEvent>,
}

impl<'a> RunbookService<'a> {
    /// Create a new runbook service
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            events: Vec::new(),
        }
    }

    /// Take accumulated events (for sending to MCP)
    pub fn take_events(&mut self) -> Vec<RunbookEvent> {
        std::mem::take(&mut self.events)
    }

    /// Emit an event (adds to buffer and logs)
    fn emit(&mut self, event: RunbookEvent) {
        info!(
            category = ?event.category(),
            event_type = event.event_type(),
            "Runbook event"
        );
        self.events.push(event);
    }

    // =========================================================================
    // STAGE (the default path - NO EXECUTION)
    // =========================================================================

    /// Stage a DSL command without executing.
    ///
    /// This is the DEFAULT path for all normal prompts.
    /// Parses DSL, resolves entities, stages for later execution.
    ///
    /// **NEVER EXECUTES** - only prepares for execution.
    #[instrument(skip(self), fields(session_id, verb))]
    pub async fn stage(
        &mut self,
        session_id: &str,
        client_group_id: Option<Uuid>,
        persona: Option<&str>,
        dsl_raw: &str,
        description: Option<&str>,
        source_prompt: Option<&str>,
    ) -> Result<StageResult, StageError> {
        let repo = StagedRunbookRepository::new(self.pool);

        // 1. Parse DSL first (fail fast on syntax errors)
        let verb_call = match parse_single_verb(dsl_raw) {
            Ok(call) => call,
            Err(e) => {
                let error = e.to_string();
                // Get or create runbook for error event
                let runbook_id = repo
                    .get_or_create_runbook(session_id, client_group_id, persona)
                    .await
                    .map_err(StageError::Database)?;

                self.emit(RunbookEvent::StageFailed {
                    runbook_id,
                    error_kind: "parse_failed".to_string(),
                    error: error.clone(),
                    dsl_raw: dsl_raw.to_string(),
                });

                return Err(StageError::ParseFailed {
                    error,
                    dsl_raw: dsl_raw.to_string(),
                });
            }
        };

        let verb = format!("{}.{}", verb_call.domain, verb_call.verb);
        tracing::Span::current().record("verb", &verb);

        // 2. Get or create runbook
        let runbook_id = repo
            .get_or_create_runbook(session_id, client_group_id, persona)
            .await
            .map_err(StageError::Database)?;

        // 3. Stage the command
        let command_id = repo
            .stage_command(runbook_id, dsl_raw, &verb, description, source_prompt)
            .await
            .map_err(StageError::Database)?;

        // 4. Resolve entity arguments (if we have client group context)
        let (resolution_status, entity_count) = if let Some(group_id) = client_group_id {
            self.resolve_command_entities(
                &repo, runbook_id, command_id, group_id, persona, &verb_call,
            )
            .await?
        } else {
            // No client group - can't resolve, mark as pending
            (ResolutionStatus::Pending, 0)
        };

        // 5. Compute DSL hash for audit
        let dsl_hash = sha256(dsl_raw.as_bytes());
        let dsl_hash_hex = hex::encode(&dsl_hash[..16]); // First 16 bytes

        // 6. Get runbook summary for event
        let summary = repo
            .get_runbook_summary(runbook_id)
            .await
            .map_err(StageError::Database)?
            .unwrap_or_else(|| RunbookSummary {
                id: runbook_id,
                status: RunbookStatus::Building,
                command_count: 1,
                resolved_count: if resolution_status == ResolutionStatus::Resolved {
                    1
                } else {
                    0
                },
                pending_count: if resolution_status == ResolutionStatus::Pending {
                    1
                } else {
                    0
                },
                ambiguous_count: if resolution_status == ResolutionStatus::Ambiguous {
                    1
                } else {
                    0
                },
                failed_count: if matches!(
                    resolution_status,
                    ResolutionStatus::Failed | ResolutionStatus::ParseFailed
                ) {
                    1
                } else {
                    0
                },
            });

        // 7. Emit staged event
        self.emit(RunbookEvent::CommandStaged {
            runbook_id,
            command: StagedCommandSummary {
                id: command_id,
                source_order: summary.command_count as i32,
                verb: verb.clone(),
                description: description.map(|s| s.to_string()),
                resolution_status,
                entity_count,
            },
            runbook_summary: summary,
            dsl_hash: dsl_hash_hex.clone(),
        });

        Ok(StageResult {
            command_id,
            resolution_status,
            entity_count,
            dsl_hash: dsl_hash_hex,
        })
    }

    /// Resolve entity arguments for a command
    async fn resolve_command_entities(
        &mut self,
        repo: &StagedRunbookRepository<'_>,
        runbook_id: Uuid,
        command_id: Uuid,
        client_group_id: Uuid,
        persona: Option<&str>,
        verb_call: &VerbCall,
    ) -> Result<(ResolutionStatus, usize), StageError> {
        let resolver = EntityArgResolver::new(client_group_id);
        let resolver = if let Some(p) = persona {
            resolver.with_persona(p)
        } else {
            resolver
        };

        let mut overall_status = ResolutionStatus::Resolved;
        let mut entity_count = 0;

        // Look for entity arguments in the verb call
        for arg in &verb_call.arguments {
            // Skip non-entity arguments
            if !Self::is_entity_arg(&arg.key) {
                continue;
            }

            let raw_value = Self::ast_node_to_string(&arg.value);

            // Try to resolve
            let result = resolver.resolve_sync(&raw_value);

            match result.status {
                ResolutionOutcome::Resolved => {
                    // Add resolved entities to footprint
                    for entity in &result.entities {
                        let resolved = ResolvedEntity {
                            entity_id: entity.entity_id,
                            entity_name: entity.entity_name.clone(),
                            arg_name: arg.key.clone(),
                            resolution_source: match entity.match_type {
                                MatchType::Exact => ResolutionSource::TagExact,
                                MatchType::Fuzzy => ResolutionSource::TagFuzzy,
                                MatchType::Semantic => ResolutionSource::TagSemantic,
                            },
                            original_ref: raw_value.clone(),
                            confidence: Some(entity.confidence),
                        };
                        repo.add_resolved_entity(command_id, &resolved)
                            .await
                            .map_err(StageError::Database)?;
                        entity_count += 1;

                        // Emit resolution event
                        self.emit(RunbookEvent::EntityResolved {
                            runbook_id,
                            command_id,
                            arg_name: arg.key.clone(),
                            original_ref: raw_value.clone(),
                            resolved_entity_id: entity.entity_id,
                            resolved_entity_name: entity.entity_name.clone(),
                            resolution_source: resolved.resolution_source.to_db().to_string(),
                            confidence: entity.confidence,
                        });
                    }
                }
                ResolutionOutcome::Ambiguous => {
                    overall_status = ResolutionStatus::Ambiguous;

                    // Store candidates for picker
                    for entity in &result.entities {
                        let candidate = PickerCandidate {
                            entity_id: entity.entity_id,
                            entity_name: entity.entity_name.clone(),
                            matched_tag: entity.matched_tag.clone(),
                            confidence: entity.confidence,
                            match_type: entity.match_type.to_db().to_string(),
                        };
                        repo.add_candidate(command_id, &candidate, &arg.key)
                            .await
                            .map_err(StageError::Database)?;
                    }

                    // Emit ambiguous event
                    self.emit(RunbookEvent::ResolutionAmbiguous {
                        runbook_id,
                        command_id,
                        arg_name: arg.key.clone(),
                        original_ref: raw_value.clone(),
                        candidates: result
                            .entities
                            .iter()
                            .map(|e| PickerCandidate {
                                entity_id: e.entity_id,
                                entity_name: e.entity_name.clone(),
                                matched_tag: e.matched_tag.clone(),
                                confidence: e.confidence,
                                match_type: e.match_type.to_db().to_string(),
                            })
                            .collect(),
                        candidate_count: result.entities.len(),
                    });
                }
                ResolutionOutcome::Failed => {
                    overall_status = ResolutionStatus::Failed;

                    // Emit failure event
                    self.emit(RunbookEvent::ResolutionFailed {
                        runbook_id,
                        command_id,
                        arg_name: arg.key.clone(),
                        original_ref: raw_value.clone(),
                        error: result.error.unwrap_or_default(),
                        search_method: SearchMethod::Combined,
                    });
                }
                ResolutionOutcome::Deferred => {
                    // Output reference - will be resolved at execution time
                    // Keep as resolved for now
                }
            }
        }

        // Update command status
        repo.update_command_resolution(command_id, overall_status, None, None)
            .await
            .map_err(StageError::Database)?;

        Ok((overall_status, entity_count))
    }

    /// Check if an argument name is for entity references
    fn is_entity_arg(name: &str) -> bool {
        let entity_args = [
            "entity-id",
            "entity-ids",
            "cbu-id",
            "cbu-ids",
            "target",
            "targets",
            "subject",
            "subjects",
        ];
        entity_args.contains(&name)
    }

    /// Convert AST node to string for resolution
    fn ast_node_to_string(node: &AstNode) -> String {
        match node {
            AstNode::Literal(lit, _) => Self::literal_to_string(lit),
            AstNode::SymbolRef { name, .. } => format!("@{}", name),
            AstNode::EntityRef { value, .. } => value.clone(),
            AstNode::List { items, .. } => items
                .iter()
                .map(Self::ast_node_to_string)
                .collect::<Vec<_>>()
                .join(", "),
            AstNode::Map { entries, .. } => format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, Self::ast_node_to_string(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            AstNode::Nested(verb_call) => {
                // Nested verb call - format as DSL string
                format!("({}.{} ...)", verb_call.domain, verb_call.verb)
            }
        }
    }

    /// Convert literal to string for resolution
    fn literal_to_string(lit: &Literal) -> String {
        match lit {
            Literal::String(s) => s.clone(),
            Literal::Integer(n) => n.to_string(),
            Literal::Decimal(d) => d.to_string(),
            Literal::Boolean(b) => b.to_string(),
            Literal::Null => "nil".to_string(),
            Literal::Uuid(u) => u.to_string(),
        }
    }

    // =========================================================================
    // PICK (resolve ambiguous)
    // =========================================================================

    /// Pick entities from ambiguous candidates.
    ///
    /// CRITICAL: This validates that entity_ids came from the stored candidate set.
    /// The agent cannot fabricate entity IDs - must use IDs from ResolutionAmbiguous event.
    #[instrument(skip(self))]
    pub async fn pick(
        &mut self,
        command_id: Uuid,
        entity_ids: &[Uuid],
    ) -> Result<PickResult, PickError> {
        let repo = StagedRunbookRepository::new(self.pool);

        // 1. Get command
        let command = repo
            .get_command(command_id)
            .await
            .map_err(PickError::Database)?
            .ok_or(PickError::CommandNotFound { command_id })?;

        // 2. CRITICAL: Validate selection against stored candidates
        let validation = repo
            .validate_picker_selection(command_id, entity_ids)
            .await
            .map_err(PickError::Database)?;

        if !validation.is_valid {
            if let Some(invalid_id) = validation.invalid_entity_id {
                return Err(PickError::InvalidCandidate {
                    entity_id: invalid_id,
                    message: validation
                        .error_message
                        .unwrap_or_else(|| "Entity ID not in stored candidate set".to_string()),
                });
            }
            return Err(PickError::NoCandidates { command_id });
        }

        // 3. Get runbook_id for events
        let runbook = repo
            .get_active_runbook(&command.source_prompt.unwrap_or_default())
            .await
            .map_err(PickError::Database)?;
        let runbook_id = runbook.map(|r| r.id).unwrap_or_default();

        // 4. Move selected candidates to entity footprint
        repo.clear_entity_footprint(command_id)
            .await
            .map_err(PickError::Database)?;

        let candidates = repo
            .get_candidates(command_id)
            .await
            .map_err(PickError::Database)?;

        let mut selected_count = 0;
        for candidate in &candidates {
            if entity_ids.contains(&candidate.entity_id) {
                let resolved = ResolvedEntity {
                    entity_id: candidate.entity_id,
                    entity_name: candidate.entity_name.clone(),
                    arg_name: candidate.arg_name.clone(),
                    resolution_source: ResolutionSource::Picker,
                    original_ref: candidate.matched_tag.clone().unwrap_or_default(),
                    confidence: candidate.confidence,
                };
                repo.add_resolved_entity(command_id, &resolved)
                    .await
                    .map_err(PickError::Database)?;
                selected_count += 1;
            }
        }

        // 5. Clear candidates (picker complete)
        repo.clear_candidates(command_id)
            .await
            .map_err(PickError::Database)?;

        // 6. Update command status
        repo.update_command_resolution(command_id, ResolutionStatus::Resolved, None, None)
            .await
            .map_err(PickError::Database)?;

        // 7. Emit picker applied event
        self.emit(RunbookEvent::PickerApplied {
            runbook_id,
            command_id,
            arg_name: candidates
                .first()
                .map(|c| c.arg_name.clone())
                .unwrap_or_default(),
            selected_entity_ids: entity_ids.to_vec(),
            from_candidate_set: true, // Always true after validation
        });

        Ok(PickResult {
            resolution_status: ResolutionStatus::Resolved,
            selected_count,
        })
    }

    // =========================================================================
    // RUN (explicit execution only)
    // =========================================================================

    /// Execute the runbook.
    ///
    /// **ONLY CALLED ON EXPLICIT USER REQUEST** ("run", "execute", "commit").
    /// This is the sacred boundary - normal prompts NEVER reach this.
    #[instrument(skip(self))]
    pub async fn run(&mut self, runbook_id: Uuid) -> Result<Uuid, RunError> {
        let repo = StagedRunbookRepository::new(self.pool);
        let start = Instant::now();

        // 1. Get runbook
        let runbook = repo
            .get_runbook(runbook_id)
            .await
            .map_err(RunError::Database)?
            .ok_or(RunError::NotFound { runbook_id })?;

        // 2. SERVER-SIDE READY GATE - reject if not ready
        let readiness = repo
            .check_runbook_ready(runbook_id)
            .await
            .map_err(RunError::Database)?;

        if !readiness.is_ready {
            let blockers: Vec<BlockingCommand> = readiness
                .blockers
                .into_iter()
                .map(|b| BlockingCommand {
                    command_id: b.command_id,
                    source_order: b.source_order,
                    status: ResolutionStatus::from_db(&b.status),
                    error: b.error,
                })
                .collect();

            self.emit(RunbookEvent::RunbookNotReady {
                runbook_id,
                blocking_commands: blockers.clone(),
            });

            return Err(RunError::NotReady { blockers });
        }

        // 3. Compute DAG order
        let mut analyzer = DagAnalyzer::new(&runbook.commands);
        if let Err(e) = analyzer.analyze(&runbook.commands) {
            return Err(RunError::ExecutionFailed {
                command_id: Uuid::nil(),
                error: e.to_string(),
            });
        }

        let dag_order = analyzer
            .compute_order()
            .map_err(|e| RunError::ExecutionFailed {
                command_id: Uuid::nil(),
                error: e.to_string(),
            })?;

        let reorder_diff = analyzer.reorder_diff(&runbook.commands);

        // 4. Update DAG orders in DB
        for (idx, &cmd_id) in dag_order.iter().enumerate() {
            repo.update_command_dag_order(cmd_id, idx as i32)
                .await
                .map_err(RunError::Database)?;
        }

        // 5. Emit ready event with entity footprint
        let entity_footprint = self.compute_entity_footprint(&runbook);
        self.emit(RunbookEvent::RunbookReady {
            runbook_id,
            summary: RunbookSummary {
                id: runbook_id,
                status: RunbookStatus::Ready,
                command_count: runbook.commands.len(),
                resolved_count: runbook.commands.len(),
                pending_count: 0,
                ambiguous_count: 0,
                failed_count: 0,
            },
            entity_footprint,
            reorder_diff: reorder_diff.map(|d| ReorderDiff {
                original_order: d.original_order,
                reordered: d.reordered,
                moves: d
                    .moves
                    .into_iter()
                    .map(|m| super::events::ReorderMove {
                        command_id: m.command_id,
                        from_position: m.from_position,
                        to_position: m.to_position,
                        reason: m.reason,
                    })
                    .collect(),
            }),
        });

        // 6. Update runbook status to executing
        repo.update_runbook_status(runbook_id, RunbookStatus::Executing)
            .await
            .map_err(RunError::Database)?;

        let execution_id = Uuid::now_v7();

        // 7. Emit execution started
        self.emit(RunbookEvent::ExecutionStarted {
            runbook_id,
            execution_id,
            total_commands: dag_order.len(),
            dag_order: dag_order.clone(),
        });

        // 8. Execute commands in DAG order
        let mut results = Vec::new();
        let mut learned_tags = Vec::new();

        for (dag_idx, &cmd_id) in dag_order.iter().enumerate() {
            let cmd = runbook
                .commands
                .iter()
                .find(|c| c.id == cmd_id)
                .ok_or_else(|| RunError::ExecutionFailed {
                    command_id: cmd_id,
                    error: "Command not found in runbook".to_string(),
                })?;
            let cmd_start = Instant::now();

            // TODO: Actually execute the DSL command
            // For now, simulate success
            let result = CommandResult {
                command_id: cmd_id,
                success: true,
                error: None,
                output: None,
                duration_ms: cmd_start.elapsed().as_millis() as u64,
            };

            // Emit per-command result
            self.emit(RunbookEvent::CommandExecuted {
                runbook_id,
                execution_id,
                command_id: cmd_id,
                dag_order: dag_idx as i32,
                result: result.clone(),
            });

            results.push(result);

            // Learn tags from successful resolutions
            for entity in &cmd.entity_footprint {
                if matches!(
                    entity.resolution_source,
                    ResolutionSource::TagExact
                        | ResolutionSource::TagFuzzy
                        | ResolutionSource::TagSemantic
                ) {
                    learned_tags.push(LearnedTag {
                        entity_id: entity.entity_id,
                        tag: entity.original_ref.clone(),
                        source: "user_confirmed".to_string(),
                    });
                }
            }
        }

        // 9. Update runbook status to completed
        repo.update_runbook_status(runbook_id, RunbookStatus::Completed)
            .await
            .map_err(RunError::Database)?;

        // 10. Emit execution completed
        self.emit(RunbookEvent::ExecutionCompleted {
            runbook_id,
            execution_id,
            results,
            learned_tags,
            duration_ms: start.elapsed().as_millis() as u64,
        });

        Ok(execution_id)
    }

    /// Compute entity footprint for runbook
    fn compute_entity_footprint(&self, runbook: &StagedRunbook) -> Vec<EntityFootprintEntry> {
        use std::collections::HashMap;

        let mut footprint: HashMap<Uuid, EntityFootprintEntry> = HashMap::new();

        for cmd in &runbook.commands {
            for entity in &cmd.entity_footprint {
                let entry =
                    footprint
                        .entry(entity.entity_id)
                        .or_insert_with(|| EntityFootprintEntry {
                            entity_id: entity.entity_id,
                            entity_name: entity.entity_name.clone(),
                            commands: Vec::new(),
                            operations: Vec::new(),
                        });
                entry.commands.push(cmd.id);
                if !entry.operations.contains(&cmd.verb) {
                    entry.operations.push(cmd.verb.clone());
                }
            }
        }

        footprint.into_values().collect()
    }

    // =========================================================================
    // SHOW / PREVIEW
    // =========================================================================

    /// Get current runbook state
    pub async fn show(&mut self, session_id: &str) -> Result<Option<StagedRunbook>> {
        let repo = StagedRunbookRepository::new(self.pool);
        repo.get_active_runbook(session_id).await
    }

    /// Preview runbook with readiness check
    pub async fn preview(&mut self, runbook_id: Uuid) -> Result<PreviewResult> {
        let repo = StagedRunbookRepository::new(self.pool);

        let runbook = repo
            .get_runbook(runbook_id)
            .await?
            .ok_or_else(|| anyhow!("Runbook not found"))?;

        let readiness = repo.check_runbook_ready(runbook_id).await?;
        let summary = repo
            .get_runbook_summary(runbook_id)
            .await?
            .ok_or_else(|| anyhow!("Runbook summary not found for {}", runbook_id))?;

        // Compute DAG order if ready
        let reorder_diff = if readiness.is_ready {
            let mut analyzer = DagAnalyzer::new(&runbook.commands);
            analyzer.analyze(&runbook.commands).ok();
            analyzer.reorder_diff(&runbook.commands)
        } else {
            None
        };

        let entity_footprint = self.compute_entity_footprint(&runbook);

        Ok(PreviewResult {
            runbook,
            summary,
            is_ready: readiness.is_ready,
            blockers: readiness
                .blockers
                .into_iter()
                .map(|b| BlockingCommand {
                    command_id: b.command_id,
                    source_order: b.source_order,
                    status: ResolutionStatus::from_db(&b.status),
                    error: b.error,
                })
                .collect(),
            entity_footprint,
            reorder_diff,
        })
    }

    // =========================================================================
    // REMOVE / ABORT
    // =========================================================================

    /// Remove a command
    pub async fn remove(&mut self, command_id: Uuid) -> Result<Vec<Uuid>> {
        let repo = StagedRunbookRepository::new(self.pool);

        // Get command first for event
        let command = repo.get_command(command_id).await?;

        let removed = repo.remove_command(command_id).await?;

        if let Some(cmd) = command {
            // Get runbook for event
            if let Ok(Some(runbook)) = repo.get_active_runbook("").await {
                // Summary may not exist if runbook was just created
                let Some(summary) = repo.get_runbook_summary(runbook.id).await? else {
                    return Ok(removed);
                };

                self.emit(RunbookEvent::CommandRemoved {
                    runbook_id: runbook.id,
                    command_id,
                    source_order: cmd.source_order,
                    cascade_removed: removed
                        .iter()
                        .filter(|&id| *id != command_id)
                        .cloned()
                        .collect(),
                    runbook_summary: summary,
                });
            }
        }

        Ok(removed)
    }

    /// Abort runbook
    pub async fn abort(&mut self, runbook_id: Uuid) -> Result<bool> {
        let repo = StagedRunbookRepository::new(self.pool);
        let result = repo.abort_runbook(runbook_id).await?;

        if result {
            self.emit(RunbookEvent::RunbookAborted { runbook_id });
        }

        Ok(result)
    }
}

/// Preview result
pub struct PreviewResult {
    pub runbook: StagedRunbook,
    pub summary: RunbookSummary,
    pub is_ready: bool,
    pub blockers: Vec<BlockingCommand>,
    pub entity_footprint: Vec<EntityFootprintEntry>,
    pub reorder_diff: Option<super::dag_analyzer::ReorderDiff>,
}
