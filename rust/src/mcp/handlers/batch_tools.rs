//! Batch execution, research macro, and service pipeline tool handlers.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use super::core::ToolHandlers;

impl ToolHandlers {
    // =========================================================================
    // Template Batch Execution Handlers
    // =========================================================================
    //
    // These handlers operate on the UI SessionStore (self.sessions).
    // The SessionStore is the SINGLE SOURCE OF TRUTH for session state.
    // egui and all other consumers access the same store.

    /// Start a template batch execution session
    pub(super) async fn batch_start(&self, args: Value) -> Result<Value> {
        use crate::api::session::{
            SessionMode, TemplateExecutionContext, TemplateParamKeySet, TemplatePhase,
        };
        use crate::templates::TemplateRegistry;
        use std::path::Path;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let template_id = args["template_id"]
            .as_str()
            .ok_or_else(|| anyhow!("template_id required"))?;

        // Load template
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("verbs/templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Extract entity dependencies from template
        let entity_deps = template.entity_dependency_summary();

        // Initialize key sets from template params
        let mut key_sets = std::collections::HashMap::new();

        for param_info in &entity_deps.batch_params {
            key_sets.insert(
                param_info.param_name.clone(),
                TemplateParamKeySet {
                    param_name: param_info.param_name.clone(),
                    entity_type: param_info.entity_type.clone(),
                    cardinality: "batch".to_string(),
                    entities: Vec::new(),
                    is_complete: false,
                    filter_description: String::new(),
                },
            );
        }

        for param_info in &entity_deps.shared_params {
            key_sets.insert(
                param_info.param_name.clone(),
                TemplateParamKeySet {
                    param_name: param_info.param_name.clone(),
                    entity_type: param_info.entity_type.clone(),
                    cardinality: "shared".to_string(),
                    entities: Vec::new(),
                    is_complete: false,
                    filter_description: String::new(),
                },
            );
        }

        // Update UI session state
        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.template_execution = TemplateExecutionContext {
                template_id: Some(template_id.to_string()),
                phase: TemplatePhase::CollectingSharedParams,
                key_sets,
                scalar_params: std::collections::HashMap::new(),
                current_batch_index: 0,
                batch_results: Vec::new(),
                auto_execute: false,
            };
            session.context.mode = SessionMode::TemplateExpansion;
        }

        // Return template info and params to collect
        Ok(json!({
            "success": true,
            "template_id": template_id,
            "template_name": template.metadata.name,
            "summary": template.metadata.summary,
            "phase": "collecting_shared_params",
            "params_to_collect": {
                "batch": entity_deps.batch_params.iter().map(|p| json!({
                    "param_name": p.param_name,
                    "entity_type": p.entity_type,
                    "prompt": p.prompt,
                    "role_hint": p.role_hint
                })).collect::<Vec<_>>(),
                "shared": entity_deps.shared_params.iter().map(|p| json!({
                    "param_name": p.param_name,
                    "entity_type": p.entity_type,
                    "prompt": p.prompt,
                    "role_hint": p.role_hint
                })).collect::<Vec<_>>()
            }
        }))
    }

    /// Add entities to a parameter's key set
    pub(super) async fn batch_add_entities(&self, args: Value) -> Result<Value> {
        use crate::api::session::ResolvedEntityRef;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let param_name = args["param_name"]
            .as_str()
            .ok_or_else(|| anyhow!("param_name required"))?;

        let entities = args["entities"]
            .as_array()
            .ok_or_else(|| anyhow!("entities array required"))?;

        let filter_description = args["filter_description"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Parse entities
        let resolved_entities: Vec<ResolvedEntityRef> = entities
            .iter()
            .filter_map(|e| {
                let entity_id = e["entity_id"]
                    .as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())?;
                let display_name = e["display_name"].as_str()?.to_string();
                let entity_type = e["entity_type"].as_str()?.to_string();
                let metadata = e.get("metadata").cloned().unwrap_or(json!(null));

                Some(ResolvedEntityRef {
                    entity_type,
                    display_name,
                    entity_id,
                    metadata,
                })
            })
            .collect();

        if resolved_entities.is_empty() {
            return Err(anyhow!("No valid entities provided"));
        }

        let added_count = resolved_entities.len();

        // Update UI session
        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let key_set = session
                .context
                .template_execution
                .key_sets
                .get_mut(param_name)
                .ok_or_else(|| anyhow!("Key set not found for param: {}", param_name))?;

            key_set.entities.extend(resolved_entities.clone());
            if !filter_description.is_empty() {
                key_set.filter_description = filter_description;
            }
        }

        Ok(json!({
            "success": true,
            "param_name": param_name,
            "added_count": added_count,
            "entities": resolved_entities.iter().map(|e| json!({
                "entity_id": e.entity_id.to_string(),
                "display_name": e.display_name,
                "entity_type": e.entity_type
            })).collect::<Vec<_>>()
        }))
    }

    /// Mark a key set as complete
    pub(super) async fn batch_confirm_keyset(&self, args: Value) -> Result<Value> {
        use crate::api::session::TemplatePhase;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let param_name = args["param_name"]
            .as_str()
            .ok_or_else(|| anyhow!("param_name required"))?;

        let (all_complete, phase) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &mut session.context.template_execution;

            let key_set = ctx
                .key_sets
                .get_mut(param_name)
                .ok_or_else(|| anyhow!("Key set not found for param: {}", param_name))?;

            if key_set.entities.is_empty() {
                return Err(anyhow!("Cannot confirm empty key set: {}", param_name));
            }

            key_set.is_complete = true;

            // Check if all key sets are complete
            let all_complete = ctx.key_sets.values().all(|ks| ks.is_complete);

            // Auto-advance phase if all complete
            if all_complete {
                ctx.phase = TemplatePhase::ReviewingKeySets;
            }

            (all_complete, ctx.phase.clone())
        };

        Ok(json!({
            "success": true,
            "param_name": param_name,
            "all_key_sets_complete": all_complete,
            "phase": format!("{:?}", phase).to_lowercase()
        }))
    }

    /// Set a scalar parameter value
    pub(super) async fn batch_set_scalar(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let param_name = args["param_name"]
            .as_str()
            .ok_or_else(|| anyhow!("param_name required"))?;

        let value = args["value"]
            .as_str()
            .ok_or_else(|| anyhow!("value required"))?;

        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session
                .context
                .template_execution
                .scalar_params
                .insert(param_name.to_string(), value.to_string());
        }

        Ok(json!({
            "success": true,
            "param_name": param_name,
            "value": value
        }))
    }

    /// Get current template execution state
    pub(super) async fn batch_get_state(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let context = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.template_execution.clone()
        };

        Ok(json!({
            "template_id": context.template_id,
            "phase": format!("{:?}", context.phase).to_lowercase(),
            "key_sets": context.key_sets.iter().map(|(name, ks)| {
                json!({
                    "param_name": name,
                    "entity_type": ks.entity_type,
                    "cardinality": ks.cardinality,
                    "entity_count": ks.entities.len(),
                    "is_complete": ks.is_complete,
                    "entities": ks.entities.iter().map(|e| json!({
                        "entity_id": e.entity_id.to_string(),
                        "display_name": e.display_name
                    })).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>(),
            "scalar_params": context.scalar_params,
            "current_batch_index": context.current_batch_index,
            "batch_size": context.batch_size(),
            "progress": context.progress_string(),
            "batch_results": context.batch_results.iter().map(|r| json!({
                "index": r.index,
                "source_entity": r.source_entity.display_name,
                "success": r.success,
                "created_id": r.created_id.map(|id| id.to_string()),
                "error": r.error
            })).collect::<Vec<_>>(),
            "is_active": context.is_active()
        }))
    }

    /// Expand template for current batch item
    pub(super) async fn batch_expand_current(&self, args: Value) -> Result<Value> {
        use crate::templates::{ExpansionContext, TemplateExpander, TemplateRegistry};
        use std::path::Path;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        // Get template context from UI session
        let context = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.template_execution.clone()
        };

        let template_id = context
            .template_id
            .as_ref()
            .ok_or_else(|| anyhow!("No template set"))?;

        // Load template
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("verbs/templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Build params from context
        let mut params = std::collections::HashMap::new();

        // Add current batch entity
        if let Some(batch_entity) = context.current_batch_entity() {
            // Find the batch param name
            if let Some((param_name, _)) = context
                .key_sets
                .iter()
                .find(|(_, ks)| ks.cardinality == "batch")
            {
                params.insert(param_name.clone(), batch_entity.entity_id.to_string());
                // Also add .name for display
                params.insert(
                    format!("{}.name", param_name),
                    batch_entity.display_name.clone(),
                );
            }
        }

        // Add shared entities
        for (param_name, entity) in context.shared_entities() {
            params.insert(param_name.to_string(), entity.entity_id.to_string());
            params.insert(format!("{}.name", param_name), entity.display_name.clone());
        }

        // Add scalar params
        for (name, value) in &context.scalar_params {
            params.insert(name.clone(), value.clone());
        }

        // Expand template
        let expansion_ctx = ExpansionContext::new();
        let result = TemplateExpander::expand(template, &params, &expansion_ctx);

        Ok(json!({
            "dsl": result.dsl,
            "complete": result.missing_params.is_empty(),
            "batch_index": context.current_batch_index,
            "batch_size": context.batch_size(),
            "current_entity": context.current_batch_entity().map(|e| json!({
                "entity_id": e.entity_id.to_string(),
                "display_name": e.display_name,
                "entity_type": e.entity_type
            })),
            "missing_params": result.missing_params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
        }))
    }

    /// Record result from executing current batch item
    pub(super) async fn batch_record_result(&self, args: Value) -> Result<Value> {
        use crate::api::session::{BatchItemResult, TemplatePhase};

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let success = args["success"]
            .as_bool()
            .ok_or_else(|| anyhow!("success required"))?;

        let created_id = args["created_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok());

        let error = args["error"].as_str().map(|s| s.to_string());

        let executed_dsl = args["executed_dsl"].as_str().map(|s| s.to_string());

        let (has_more, new_index, phase) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &mut session.context.template_execution;
            let index = ctx.current_batch_index;

            // Get the current batch entity
            let source_entity = ctx
                .current_batch_entity()
                .cloned()
                .ok_or_else(|| anyhow!("No current batch entity"))?;

            // Record result
            ctx.batch_results.push(BatchItemResult {
                index,
                source_entity,
                success,
                created_id,
                error: error.clone(),
                executed_dsl,
            });

            // Advance to next item
            let has_more = ctx.advance();

            // Update phase if complete
            if !has_more {
                ctx.phase = TemplatePhase::Complete;
            }

            (has_more, ctx.current_batch_index, ctx.phase.clone())
        };

        Ok(json!({
            "success": true,
            "recorded_success": success,
            "has_more_items": has_more,
            "next_index": new_index,
            "phase": format!("{:?}", phase).to_lowercase(),
            "created_id": created_id.map(|id| id.to_string()),
            "error": error
        }))
    }

    /// Skip current batch item
    pub(super) async fn batch_skip_current(&self, args: Value) -> Result<Value> {
        use crate::api::session::{BatchItemResult, TemplatePhase};

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let reason = args["reason"].as_str().unwrap_or("User skipped");

        let (has_more, new_index, phase) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &mut session.context.template_execution;
            let index = ctx.current_batch_index;

            // Get the current batch entity
            let source_entity = ctx
                .current_batch_entity()
                .cloned()
                .ok_or_else(|| anyhow!("No current batch entity"))?;

            // Record skip as failed result
            ctx.batch_results.push(BatchItemResult {
                index,
                source_entity,
                success: false,
                created_id: None,
                error: Some(format!("Skipped: {}", reason)),
                executed_dsl: None,
            });

            // Advance to next item
            let has_more = ctx.advance();

            // Update phase if complete
            if !has_more {
                ctx.phase = TemplatePhase::Complete;
            }

            (has_more, ctx.current_batch_index, ctx.phase.clone())
        };

        Ok(json!({
            "success": true,
            "skipped": true,
            "has_more_items": has_more,
            "next_index": new_index,
            "phase": format!("{:?}", phase).to_lowercase()
        }))
    }

    /// Cancel batch operation
    pub(super) async fn batch_cancel(&self, args: Value) -> Result<Value> {
        use crate::api::session::SessionMode;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let (completed_count, failed_count, pending_count) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let ctx = &session.context.template_execution;
            let completed = ctx.batch_results.iter().filter(|r| r.success).count();
            let failed = ctx.batch_results.iter().filter(|r| !r.success).count();
            let total = ctx.batch_size();
            let pending = total.saturating_sub(completed + failed);

            // Reset template execution state
            session.context.template_execution.reset();
            session.context.mode = SessionMode::Chat;

            (completed, failed, pending)
        };

        Ok(json!({
            "success": true,
            "cancelled": true,
            "completed_count": completed_count,
            "skipped_count": failed_count,
            "abandoned_count": pending_count
        }))
    }

    // ========================================================================
    // Research Macro Handlers
    // ========================================================================

    /// List available research macros with optional filtering
    pub(super) async fn research_list(&self, args: Value) -> Result<Value> {
        use crate::research::{ResearchMacroRegistry, ReviewRequirement};

        let search = args["search"].as_str();
        let tag = args["tag"].as_str();

        // Load registry from config directory
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let macros_dir = std::path::Path::new(&config_dir).join("macros/research");

        let registry = ResearchMacroRegistry::load_from_dir(&macros_dir)
            .map_err(|e| anyhow!("Failed to load research macros: {}", e))?;

        let macros: Vec<Value> = registry
            .list(search)
            .iter()
            .filter(|m| {
                // Apply tag filter
                if let Some(tag_filter) = tag {
                    if !m.tags.iter().any(|t| t.eq_ignore_ascii_case(tag_filter)) {
                        return false;
                    }
                }
                true
            })
            .map(|m| {
                json!({
                    "name": m.name,
                    "description": m.description,
                    "tags": m.tags,
                    "review_required": matches!(m.output.review, ReviewRequirement::Required),
                    "param_count": m.parameters.len()
                })
            })
            .collect();

        Ok(json!({
            "macros": macros,
            "count": macros.len()
        }))
    }

    /// Get full details of a specific research macro
    pub(super) async fn research_get(&self, args: Value) -> Result<Value> {
        use crate::research::ResearchMacroRegistry;

        let macro_name = args["macro_name"]
            .as_str()
            .ok_or_else(|| anyhow!("macro_name required"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let macros_dir = std::path::Path::new(&config_dir).join("macros/research");

        let registry = ResearchMacroRegistry::load_from_dir(&macros_dir)
            .map_err(|e| anyhow!("Failed to load research macros: {}", e))?;

        let macro_def = registry
            .get(macro_name)
            .ok_or_else(|| anyhow!("Research macro not found: {}", macro_name))?;

        // Build parameter descriptions
        let params: Vec<Value> = macro_def
            .parameters
            .iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "param_type": &p.param_type,
                    "required": p.required,
                    "description": p.description,
                    "default": p.default,
                    "enum_values": p.enum_values
                })
            })
            .collect();

        Ok(json!({
            "name": macro_def.name,
            "description": macro_def.description,
            "params": params,
            "output_schema": macro_def.output.schema,
            "review_requirement": format!("{:?}", macro_def.output.review),
            "suggested_verbs_template": macro_def.suggested_verbs,
            "tags": macro_def.tags
        }))
    }

    /// Execute a research macro with LLM + web search
    pub(super) async fn research_execute(&self, args: Value) -> Result<Value> {
        use crate::research::{ClaudeResearchClient, ResearchExecutor, ResearchMacroRegistry};

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let macro_name = args["macro_name"]
            .as_str()
            .ok_or_else(|| anyhow!("macro_name required"))?;

        let params = args["params"].as_object().cloned().unwrap_or_default();

        // Load registry
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let macros_dir = std::path::Path::new(&config_dir).join("macros/research");

        let registry = ResearchMacroRegistry::load_from_dir(&macros_dir)
            .map_err(|e| anyhow!("Failed to load research macros: {}", e))?;

        // Convert params to HashMap
        let params_map: std::collections::HashMap<String, serde_json::Value> =
            params.into_iter().collect();

        // Create LLM client and executor
        let llm_client = ClaudeResearchClient::from_env()
            .map_err(|e| anyhow!("Failed to create LLM client: {}", e))?;
        let executor = ResearchExecutor::new(registry, llm_client);
        let result = executor
            .execute(macro_name, params_map)
            .await
            .map_err(|e| anyhow!("Research execution failed: {}", e))?;

        // Store result in session for review workflow using ResearchContext API
        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            // Use the ResearchContext.set_pending() method
            session.context.research.set_pending(result.clone());
        }

        Ok(json!({
            "success": true,
            "result_id": result.result_id,
            "macro_name": result.macro_name,
            "data": result.data,
            "schema_valid": result.schema_valid,
            "validation_errors": result.validation_errors,
            "review_required": result.review_required,
            "suggested_verbs": result.suggested_verbs,
            "search_quality": result.search_quality
        }))
    }

    /// Approve research results and get generated DSL verbs
    pub(super) async fn research_approve(&self, args: Value) -> Result<Value> {
        use crate::session::ResearchState;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        // Optional edits to the research data before approval
        let edits: Option<Value> = args.get("edits").cloned();

        let (verbs, macro_name, result_id) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let research = &mut session.context.research;

            // Verify we're in the right state
            if research.state != ResearchState::PendingReview {
                return Err(anyhow!(
                    "Cannot approve: research is not pending review (state: {})",
                    research.state
                ));
            }

            // Get macro name before approval
            let macro_name = research
                .pending_macro_name()
                .unwrap_or("unknown")
                .to_string();
            let result_id = research.pending.as_ref().map(|r| r.result_id);

            // Use the ResearchContext.approve() method
            let approved = research
                .approve(edits)
                .map_err(|e| anyhow!("Approval failed: {}", e))?;

            let verbs = Some(approved.generated_verbs.clone());

            (verbs, macro_name, result_id)
        };

        Ok(json!({
            "success": true,
            "approved": true,
            "result_id": result_id,
            "macro_name": macro_name,
            "suggested_verbs": verbs,
            "message": "Research approved. Use the suggested_verbs DSL to create entities."
        }))
    }

    /// Reject research results
    pub(super) async fn research_reject(&self, args: Value) -> Result<Value> {
        use crate::session::ResearchState;

        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let reason = args["reason"].as_str().unwrap_or("No reason provided");

        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let research = &mut session.context.research;

            // Verify we're in the right state
            if research.state != ResearchState::PendingReview {
                return Err(anyhow!(
                    "Cannot reject: research is not pending review (state: {})",
                    research.state
                ));
            }

            // Use the ResearchContext.reject() method
            research.reject();
        }

        Ok(json!({
            "success": true,
            "rejected": true,
            "reason": reason,
            "message": "Research rejected. You can re-execute with different parameters."
        }))
    }

    /// Get current research status for a session
    pub(super) async fn research_status(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let status = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let research = &session.context.research;

            json!({
                "state": research.state.to_string(),
                "current_macro": research.pending_macro_name(),
                "has_pending_result": research.has_pending(),
                "has_pending_verbs": research.has_verbs_ready(),
                "approved_count": research.approved_count(),
                "recent_approvals": research.approved.values()
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                    .take(5)
                    .map(|a| {
                        json!({
                            "result_id": a.result_id,
                            "approved_at": a.approved_at.to_rfc3339(),
                            "edits_made": a.edits_made
                        })
                    })
                    .collect::<Vec<_>>()
            })
        };

        Ok(json!({
            "success": true,
            "status": status
        }))
    }

    // =========================================================================
    // Service Resource Pipeline Tools
    // =========================================================================

    pub(super) async fn service_intent_create(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{NewServiceIntent, ServiceResourcePipelineService};

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
            product_id: String,
            service_id: String,
            options: Option<Value>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        // Resolve CBU ID (UUID or name)
        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        // Resolve product ID
        let product_id = self.resolve_product_id(&args.product_id).await?;

        // Resolve service ID
        let service_id = self.resolve_service_id(&args.service_id).await?;

        let service = ServiceResourcePipelineService::new(pool.clone());
        let input = NewServiceIntent {
            cbu_id,
            product_id,
            service_id,
            options: args.options,
            created_by: None,
        };

        let intent_id = service.create_service_intent(&input).await?;

        Ok(json!({
            "success": true,
            "intent_id": intent_id,
            "cbu_id": cbu_id,
            "product_id": product_id,
            "service_id": service_id
        }))
    }

    pub(super) async fn service_intent_list(&self, args: Value) -> Result<Value> {
        use crate::service_resources::ServiceResourcePipelineService;

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let service = ServiceResourcePipelineService::new(pool.clone());
        let intents = service.get_service_intents(cbu_id).await?;

        Ok(json!({
            "success": true,
            "count": intents.len(),
            "intents": intents.iter().map(|i| json!({
                "intent_id": i.intent_id,
                "cbu_id": i.cbu_id,
                "product_id": i.product_id,
                "service_id": i.service_id,
                "options": i.options,
                "status": i.status,
                "created_at": i.created_at.map(|t| t.to_rfc3339())
            })).collect::<Vec<_>>()
        }))
    }

    pub(super) async fn service_discovery_run(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{load_srdefs_from_config, run_discovery_pipeline};

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let registry = load_srdefs_from_config().unwrap_or_default();
        let result = run_discovery_pipeline(pool, &registry, cbu_id).await?;

        Ok(json!({
            "success": true,
            "cbu_id": result.cbu_id,
            "srdefs_discovered": result.srdefs_discovered,
            "attrs_rolled_up": result.attrs_rolled_up,
            "attrs_populated": result.attrs_populated,
            "attrs_missing": result.attrs_missing
        }))
    }

    pub(super) async fn service_attributes_gaps(&self, args: Value) -> Result<Value> {
        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        // Query the gap view directly
        let gaps: Vec<AttrGapRow> = sqlx::query_as(
            r#"
            SELECT attr_id, attr_code, attr_name, attr_category, has_value
            FROM "ob-poc".v_cbu_attr_gaps
            WHERE cbu_id = $1 AND NOT has_value
            ORDER BY attr_category, attr_name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "gap_count": gaps.len(),
            "gaps": gaps.iter().map(|g| json!({
                "attr_id": g.attr_id,
                "attr_code": g.attr_code,
                "attr_name": g.attr_name,
                "attr_category": g.attr_category
            })).collect::<Vec<_>>()
        }))
    }

    pub(super) async fn service_attributes_set(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{
            AttributeSource, ServiceResourcePipelineService, SetCbuAttrValue,
        };

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
            attr_id: Uuid,
            value: Value,
            source: Option<String>,
            evidence_refs: Option<Vec<String>>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let source = match args.source.as_deref() {
            Some("derived") => AttributeSource::Derived,
            Some("entity") => AttributeSource::Entity,
            Some("cbu") => AttributeSource::Cbu,
            Some("document") => AttributeSource::Document,
            Some("external") => AttributeSource::External,
            _ => AttributeSource::Manual,
        };

        // Convert evidence_refs strings to EvidenceRef structs
        let evidence_refs = args.evidence_refs.map(|refs| {
            refs.into_iter()
                .map(|r| crate::service_resources::EvidenceRef {
                    ref_type: "document".to_string(),
                    id: Uuid::parse_str(&r).ok().map(|u| u.to_string()),
                    path: None,
                    details: Some(serde_json::json!({ "description": r })),
                })
                .collect()
        });

        let service = ServiceResourcePipelineService::new(pool.clone());
        let input = SetCbuAttrValue {
            cbu_id,
            attr_id: args.attr_id,
            value: args.value.clone(),
            source,
            evidence_refs,
            explain_refs: None,
        };

        service.set_cbu_attr_value(&input).await?;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "attr_id": args.attr_id,
            "value": args.value
        }))
    }

    pub(super) async fn service_readiness_get(&self, args: Value) -> Result<Value> {
        use crate::service_resources::ServiceResourcePipelineService;

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let service = ServiceResourcePipelineService::new(pool.clone());
        let readiness = service.get_service_readiness(cbu_id).await?;

        let ready = readiness.iter().filter(|r| r.status == "ready").count();
        let partial = readiness.iter().filter(|r| r.status == "partial").count();
        let blocked = readiness.iter().filter(|r| r.status == "blocked").count();

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "summary": {
                "total": readiness.len(),
                "ready": ready,
                "partial": partial,
                "blocked": blocked
            },
            "services": readiness.iter().map(|r| json!({
                "service_id": r.service_id,
                "product_id": r.product_id,
                "status": r.status,
                "blocking_reasons": r.blocking_reasons
            })).collect::<Vec<_>>()
        }))
    }

    pub(super) async fn service_readiness_recompute(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{load_srdefs_from_config, ReadinessEngine};

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let registry = load_srdefs_from_config().unwrap_or_default();
        let engine = ReadinessEngine::new(pool, &registry);
        let result = engine.compute_for_cbu(cbu_id).await?;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "recomputed": true,
            "total_services": result.total_services,
            "ready": result.ready,
            "partial": result.partial,
            "blocked": result.blocked
        }))
    }

    pub(super) async fn service_pipeline_run(&self, args: Value) -> Result<Value> {
        use crate::service_resources::{
            load_srdefs_from_config, run_discovery_pipeline, run_provisioning_pipeline,
        };

        #[derive(serde::Deserialize)]
        struct Args {
            cbu_id: String,
            dry_run: Option<bool>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;
        let _dry_run = args.dry_run.unwrap_or(false);

        let cbu_id = self.resolve_cbu_id(&args.cbu_id).await?;

        let registry = load_srdefs_from_config().unwrap_or_default();

        // Run discovery + rollup + populate
        let discovery = run_discovery_pipeline(pool, &registry, cbu_id).await?;

        // Run provisioning + readiness
        let provisioning = run_provisioning_pipeline(pool, &registry, cbu_id).await?;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id,
            "discovery": {
                "srdefs_discovered": discovery.srdefs_discovered,
                "attrs_rolled_up": discovery.attrs_rolled_up,
                "attrs_populated": discovery.attrs_populated,
                "attrs_missing": discovery.attrs_missing
            },
            "provisioning": {
                "requests_created": provisioning.requests_created,
                "already_active": provisioning.already_active,
                "not_ready": provisioning.not_ready
            },
            "readiness": {
                "services_ready": provisioning.services_ready,
                "services_partial": provisioning.services_partial,
                "services_blocked": provisioning.services_blocked
            }
        }))
    }

    pub(super) async fn srdef_list(&self, args: Value) -> Result<Value> {
        use crate::service_resources::load_srdefs_from_config;

        #[derive(serde::Deserialize, Default)]
        struct Args {
            domain: Option<String>,
            resource_type: Option<String>,
        }

        let args: Args = serde_json::from_value(args).unwrap_or_default();

        let registry = load_srdefs_from_config().unwrap_or_default();

        let srdefs: Vec<_> = registry
            .srdefs
            .values()
            .filter(|s| {
                args.domain
                    .as_ref()
                    .is_none_or(|d| s.code.starts_with(&format!("{}:", d)))
            })
            .filter(|s| {
                args.resource_type
                    .as_ref()
                    .is_none_or(|rt| s.resource_type.eq_ignore_ascii_case(rt))
            })
            .map(|s| {
                json!({
                    "srdef_id": s.srdef_id,
                    "code": s.code,
                    "name": s.name,
                    "resource_type": s.resource_type,
                    "owner": s.owner,
                    "provisioning_strategy": s.provisioning_strategy,
                    "triggered_by_services": s.triggered_by_services,
                    "attribute_count": s.attributes.len(),
                    "depends_on": s.depends_on
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "count": srdefs.len(),
            "srdefs": srdefs
        }))
    }

    pub(super) async fn srdef_get(&self, args: Value) -> Result<Value> {
        use crate::service_resources::load_srdefs_from_config;

        #[derive(serde::Deserialize)]
        struct Args {
            srdef_id: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let registry = load_srdefs_from_config().unwrap_or_default();

        // Try direct lookup, then with decoded colons
        let srdef_id = args.srdef_id.replace("%3A", ":").replace("%3a", ":");

        match registry.get(&srdef_id) {
            Some(srdef) => Ok(json!({
                "success": true,
                "srdef": {
                    "srdef_id": srdef.srdef_id,
                    "code": srdef.code,
                    "name": srdef.name,
                    "resource_type": srdef.resource_type,
                    "purpose": srdef.purpose,
                    "owner": srdef.owner,
                    "provisioning_strategy": srdef.provisioning_strategy,
                    "triggered_by_services": srdef.triggered_by_services,
                    "attributes": srdef.attributes.iter().map(|a| json!({
                        "attr_id": a.attr_id,
                        "requirement": a.requirement,
                        "source_policy": a.source_policy,
                        "constraints": a.constraints,
                        "description": a.description
                    })).collect::<Vec<_>>(),
                    "depends_on": srdef.depends_on,
                    "per_market": srdef.per_market,
                    "per_currency": srdef.per_currency,
                    "per_counterparty": srdef.per_counterparty
                }
            })),
            None => Err(anyhow!("SRDEF not found: {}", srdef_id)),
        }
    }
}

// Helper struct for attribute gap query
#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct AttrGapRow {
    attr_id: Uuid,
    attr_code: String,
    attr_name: String,
    attr_category: String,
    has_value: bool,
}
