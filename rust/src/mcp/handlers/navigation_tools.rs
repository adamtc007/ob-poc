//! Workflow, template, taxonomy, trading, and feedback tool handlers.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use super::core::ToolHandlers;

impl ToolHandlers {
    // =========================================================================
    // Workflow Tools
    // =========================================================================

    pub(crate) async fn workflow_status(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;
        let workflow_id = args["workflow_id"].as_str().unwrap_or("kyc_onboarding");

        // Load workflow definitions
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Find or get existing workflow
        let instance = engine
            .find_or_start(workflow_id, subject_type, subject_id, None)
            .await
            .map_err(|e| anyhow!("Workflow error: {}", e))?;

        let status = engine
            .get_status(instance.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(serde_json::to_value(status)?)
    }

    /// Try to advance workflow automatically
    pub(crate) async fn workflow_advance(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;
        let workflow_id = args["workflow_id"].as_str().unwrap_or("kyc_onboarding");

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Find existing workflow
        let instance = engine
            .find_or_start(workflow_id, subject_type, subject_id, None)
            .await
            .map_err(|e| anyhow!("Workflow error: {}", e))?;

        // Try to advance
        let advanced = engine
            .try_advance(instance.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to advance: {}", e))?;

        // Get updated status
        let status = engine
            .get_status(advanced.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(json!({
            "advanced": advanced.current_state != instance.current_state,
            "previous_state": instance.current_state,
            "current_state": advanced.current_state,
            "status": status
        }))
    }

    /// Manually transition to a specific state
    pub(crate) async fn workflow_transition(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;
        let workflow_id = args["workflow_id"].as_str().unwrap_or("kyc_onboarding");
        let to_state = args["to_state"]
            .as_str()
            .ok_or_else(|| anyhow!("to_state required"))?;
        let reason = args["reason"].as_str().map(|s| s.to_string());

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Find existing workflow
        let instance = engine
            .find_or_start(workflow_id, subject_type, subject_id, None)
            .await
            .map_err(|e| anyhow!("Workflow error: {}", e))?;

        let previous_state = instance.current_state.clone();

        // Transition
        let transitioned = engine
            .transition(
                instance.instance_id,
                to_state,
                Some("mcp_tool".to_string()),
                reason,
            )
            .await
            .map_err(|e| anyhow!("Transition failed: {}", e))?;

        // Get updated status
        let status = engine
            .get_status(transitioned.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(json!({
            "success": true,
            "previous_state": previous_state,
            "current_state": transitioned.current_state,
            "status": status
        }))
    }

    /// Start a new workflow
    pub(crate) async fn workflow_start(&self, args: Value) -> Result<Value> {
        use crate::workflow::{WorkflowEngine, WorkflowLoader};
        use std::path::Path;

        let workflow_id = args["workflow_id"]
            .as_str()
            .ok_or_else(|| anyhow!("workflow_id required"))?;
        let subject_type = args["subject_type"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_type required"))?;
        let subject_id: Uuid = args["subject_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subject_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subject_id UUID"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let workflows_path = Path::new(&config_dir).join("workflows");
        let definitions = WorkflowLoader::load_from_dir(&workflows_path)
            .map_err(|e| anyhow!("Failed to load workflows: {}", e))?;

        let engine = WorkflowEngine::new(self.pool.clone(), definitions);

        // Start new workflow
        let instance = engine
            .start_workflow(
                workflow_id,
                subject_type,
                subject_id,
                Some("mcp_tool".to_string()),
            )
            .await
            .map_err(|e| anyhow!("Failed to start workflow: {}", e))?;

        // Get status
        let status = engine
            .get_status(instance.instance_id)
            .await
            .map_err(|e| anyhow!("Failed to get status: {}", e))?;

        Ok(json!({
            "instance_id": instance.instance_id,
            "workflow_id": workflow_id,
            "current_state": instance.current_state,
            "status": status
        }))
    }

    /// Get DSL template to resolve a blocker
    pub(crate) fn resolve_blocker(&self, args: Value) -> Result<Value> {
        let blocker_type = args["blocker_type"]
            .as_str()
            .ok_or_else(|| anyhow!("blocker_type required"))?;
        let context = args.get("context").cloned().unwrap_or(json!({}));

        let (verb, template, description) = match blocker_type {
            "missing_role" => {
                let role = context["role"].as_str().unwrap_or("DIRECTOR");
                let cbu_id = context["cbu_id"].as_str().unwrap_or("<cbu-id>");
                let entity_id = context["entity_id"].as_str().unwrap_or("<entity-id>");
                (
                    "cbu.assign-role",
                    format!(
                        "(cbu.assign-role :cbu-id {} :entity-id {} :role \"{}\")",
                        cbu_id, entity_id, role
                    ),
                    format!("Assign {} role to entity", role),
                )
            }
            "missing_document" => {
                let doc_type = context["document_type"].as_str().unwrap_or("PASSPORT");
                let cbu_id = context["cbu_id"].as_str().unwrap_or("<cbu-id>");
                (
                    "document.catalog",
                    format!(
                        "(document.catalog :cbu-id {} :doc-type \"{}\" :title \"<title>\")",
                        cbu_id, doc_type
                    ),
                    format!("Catalog {} document", doc_type),
                )
            }
            "pending_screening" => {
                let entity_id = context["entity_id"].as_str().unwrap_or("<entity-id>");
                let workstream_id = context["workstream_id"]
                    .as_str()
                    .unwrap_or("<workstream-id>");
                (
                    "case-screening.run",
                    format!(
                        "(case-screening.run :workstream-id {} :screening-type \"SANCTIONS\")",
                        workstream_id
                    ),
                    format!("Run screening for entity {}", entity_id),
                )
            }
            "unresolved_alert" => {
                let screening_id = context["screening_id"].as_str().unwrap_or("<screening-id>");
                (
                    "case-screening.review-hit",
                    format!(
                        "(case-screening.review-hit :screening-id {} :disposition \"FALSE_POSITIVE\" :notes \"<reason>\")",
                        screening_id
                    ),
                    "Review and resolve screening alert".to_string(),
                )
            }
            "incomplete_ownership" => {
                let owner_id = context["owner_entity_id"]
                    .as_str()
                    .unwrap_or("<owner-entity-id>");
                let owned_id = context["owned_entity_id"]
                    .as_str()
                    .unwrap_or("<owned-entity-id>");
                (
                    "ubo.add-ownership",
                    format!(
                        "(ubo.add-ownership :owner-entity-id {} :owned-entity-id {} :percentage <pct> :ownership-type \"DIRECT\")",
                        owner_id, owned_id
                    ),
                    "Add ownership relationship".to_string(),
                )
            }
            "unverified_ubo" => {
                let ubo_id = context["ubo_id"].as_str().unwrap_or("<ubo-id>");
                (
                    "ubo.verify-ubo",
                    format!(
                        "(ubo.verify-ubo :ubo-id {} :verification-status \"VERIFIED\" :risk-rating \"LOW\")",
                        ubo_id
                    ),
                    "Verify UBO".to_string(),
                )
            }
            _ => {
                return Err(anyhow!("Unknown blocker type: {}", blocker_type));
            }
        };

        Ok(json!({
            "blocker_type": blocker_type,
            "verb": verb,
            "dsl_template": template,
            "description": description
        }))
    }

    // =========================================================================
    // Template Tools
    // =========================================================================

    /// List available templates with filtering
    pub(crate) fn template_list(&self, args: Value) -> Result<Value> {
        use crate::templates::TemplateRegistry;
        use std::path::Path;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        // Apply filters
        let templates: Vec<_> = if let Some(blocker) = args["blocker"].as_str() {
            registry.find_by_blocker(blocker)
        } else if let Some(tag) = args["tag"].as_str() {
            registry.find_by_tag(tag)
        } else if let (Some(workflow), Some(state)) =
            (args["workflow"].as_str(), args["state"].as_str())
        {
            registry.find_by_workflow_state(workflow, state)
        } else if let Some(search) = args["search"].as_str() {
            registry.search(search)
        } else {
            registry.list()
        };

        let results: Vec<_> = templates
            .iter()
            .map(|t| {
                json!({
                    "template_id": t.template,
                    "name": t.metadata.name,
                    "summary": t.metadata.summary,
                    "tags": t.tags,
                    "resolves_blockers": t.workflow_context.resolves_blockers,
                    "applicable_states": t.workflow_context.applicable_states
                })
            })
            .collect();

        Ok(json!({
            "count": results.len(),
            "templates": results
        }))
    }

    /// Get full template details
    pub(crate) fn template_get(&self, args: Value) -> Result<Value> {
        use crate::templates::TemplateRegistry;
        use std::path::Path;

        let template_id = args["template_id"]
            .as_str()
            .ok_or_else(|| anyhow!("template_id required"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Build parameter info
        let params: Vec<_> = template
            .params
            .iter()
            .map(|(name, def)| {
                json!({
                    "name": name,
                    "type": def.param_type,
                    "required": def.required,
                    "source": def.source,
                    "default": def.default,
                    "prompt": def.prompt,
                    "example": def.example,
                    "validation": def.validation,
                    "enum_values": def.enum_values
                })
            })
            .collect();

        Ok(json!({
            "template_id": template.template,
            "version": template.version,
            "metadata": {
                "name": template.metadata.name,
                "summary": template.metadata.summary,
                "description": template.metadata.description,
                "when_to_use": template.metadata.when_to_use,
                "when_not_to_use": template.metadata.when_not_to_use,
                "effects": template.metadata.effects,
                "next_steps": template.metadata.next_steps
            },
            "tags": template.tags,
            "workflow_context": {
                "applicable_workflows": template.workflow_context.applicable_workflows,
                "applicable_states": template.workflow_context.applicable_states,
                "resolves_blockers": template.workflow_context.resolves_blockers
            },
            "params": params,
            "body": template.body,
            "outputs": template.outputs,
            "related_templates": template.related_templates
        }))
    }

    /// Expand a template to DSL source text
    pub(crate) fn template_expand(&self, args: Value) -> Result<Value> {
        use crate::templates::{ExpansionContext, TemplateExpander, TemplateRegistry};
        use std::collections::HashMap;
        use std::path::Path;

        let template_id = args["template_id"]
            .as_str()
            .ok_or_else(|| anyhow!("template_id required"))?;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let templates_path = Path::new(&config_dir).join("templates");

        let registry = TemplateRegistry::load_from_dir(&templates_path)
            .map_err(|e| anyhow!("Failed to load templates: {}", e))?;

        let template = registry
            .get(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Build explicit params from args
        let explicit_params: HashMap<String, String> = args["params"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Build expansion context from session info
        let mut context = ExpansionContext::new();

        if let Some(cbu_id) = args["cbu_id"].as_str() {
            if let Ok(uuid) = Uuid::parse_str(cbu_id) {
                context.current_cbu = Some(uuid);
            }
        }

        if let Some(case_id) = args["case_id"].as_str() {
            if let Ok(uuid) = Uuid::parse_str(case_id) {
                context.current_case = Some(uuid);
            }
        }

        // Expand template
        let result = TemplateExpander::expand(template, &explicit_params, &context);

        // Format missing params prompt if any
        let prompt = if result.missing_params.is_empty() {
            None
        } else {
            Some(TemplateExpander::format_missing_params_prompt(
                &result.missing_params,
            ))
        };

        Ok(json!({
            "template_id": result.template_id,
            "dsl": result.dsl,
            "complete": result.missing_params.is_empty(),
            "filled_params": result.filled_params,
            "missing_params": result.missing_params.iter().map(|p| json!({
                "name": p.name,
                "type": p.param_type,
                "prompt": p.prompt,
                "example": p.example,
                "required": p.required,
                "validation": p.validation
            })).collect::<Vec<_>>(),
            "prompt": prompt,
            "outputs": result.outputs
        }))
    }

    // =========================================================================
    // Taxonomy Tools
    // =========================================================================

    pub(crate) async fn taxonomy_get(&self, args: Value) -> Result<Value> {
        use crate::taxonomy::{TaxonomyNode, TaxonomyService};

        let pool = self.require_pool()?;
        let include_counts = args["include_counts"].as_bool().unwrap_or(true);

        // Build the taxonomy tree from database
        let service = TaxonomyService::new(pool.clone());
        let tree = service.build_taxonomy_tree(include_counts).await?;

        // Convert to JSON representation
        fn node_to_json(node: &TaxonomyNode) -> serde_json::Value {
            json!({
                "node_id": node.id.to_string(),
                "label": node.label,
                "short_label": node.short_label,
                "node_type": format!("{:?}", node.node_type),
                "entity_count": node.descendant_count,
                "children": node.children.iter().map(node_to_json).collect::<Vec<_>>()
            })
        }

        Ok(json!({
            "success": true,
            "taxonomy": node_to_json(&tree)
        }))
    }

    /// Drill into a taxonomy node
    pub(crate) async fn taxonomy_drill_in(&self, args: Value) -> Result<Value> {
        use crate::taxonomy::{TaxonomyFrame, TaxonomyService};

        let sessions = self.require_sessions()?;
        let pool = self.require_pool()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let node_label = args["node_label"]
            .as_str()
            .ok_or_else(|| anyhow!("node_label required"))?
            .to_uppercase();

        // Get the taxonomy subtree for this node
        let service = TaxonomyService::new(pool.clone());
        let subtree = service.get_subtree(&node_label).await?;

        // Create a new frame for this level using the constructor
        let frame = TaxonomyFrame::from_zoom(
            subtree.id,
            node_label.clone(),
            subtree.clone(),
            None, // No parser needed for type taxonomy
        );

        // Push onto the session's taxonomy stack
        let breadcrumbs = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            let _ = session.context.taxonomy_stack.push(frame);
            session.context.taxonomy_stack.breadcrumbs()
        };

        // Return the children at this level
        let children: Vec<serde_json::Value> = subtree
            .children
            .iter()
            .map(|child| {
                json!({
                    "node_id": child.id.to_string(),
                    "label": child.label,
                    "short_label": child.short_label,
                    "node_type": format!("{:?}", child.node_type),
                    "entity_count": child.descendant_count,
                    "has_children": !child.children.is_empty()
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "current_node": node_label,
            "children": children,
            "breadcrumbs": breadcrumbs
        }))
    }

    /// Zoom out one level in taxonomy
    pub(crate) async fn taxonomy_zoom_out(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let (success, breadcrumbs, current_label) = {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            if !session.context.taxonomy_stack.can_zoom_out() {
                return Ok(json!({
                    "success": false,
                    "error": "Already at root level"
                }));
            }

            session.context.taxonomy_stack.pop();
            let breadcrumbs = session.context.taxonomy_stack.breadcrumbs();
            let current_label = session
                .context
                .taxonomy_stack
                .current()
                .map(|f| f.label.clone())
                .unwrap_or_else(|| "ROOT".to_string());

            (true, breadcrumbs, current_label)
        };

        Ok(json!({
            "success": success,
            "current_node": current_label,
            "breadcrumbs": breadcrumbs
        }))
    }

    /// Reset taxonomy to root level
    pub(crate) async fn taxonomy_reset(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        {
            let mut sessions_guard = sessions.write().await;
            let session = sessions_guard
                .get_mut(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session.context.taxonomy_stack.clear();
        }

        Ok(json!({
            "success": true,
            "message": "Taxonomy reset to root level"
        }))
    }

    /// Get current taxonomy position
    pub(crate) async fn taxonomy_position(&self, args: Value) -> Result<Value> {
        let sessions = self.require_sessions()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let sessions_guard = sessions.read().await;
        let session = sessions_guard
            .get(&session_uuid)
            .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

        let stack = &session.context.taxonomy_stack;

        if stack.is_empty() {
            return Ok(json!({
                "success": true,
                "at_root": true,
                "breadcrumbs": [],
                "depth": 0,
                "can_zoom_out": false,
                "can_drill_in": true
            }));
        }

        let current_frame = stack.current();
        let current_node = current_frame.map(|f| {
            json!({
                "label": f.label,
                "child_count": f.tree.children.len()
            })
        });

        Ok(json!({
            "success": true,
            "at_root": false,
            "breadcrumbs": stack.breadcrumbs(),
            "depth": stack.depth(),
            "current_node": current_node,
            "can_zoom_out": stack.can_zoom_out(),
            "can_drill_in": stack.can_zoom_in()
        }))
    }

    /// List entities of the currently focused type
    pub(crate) async fn taxonomy_entities(&self, args: Value) -> Result<Value> {
        use crate::taxonomy::TaxonomyService;

        let sessions = self.require_sessions()?;
        let pool = self.require_pool()?;

        let session_id = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?;
        let session_uuid =
            Uuid::parse_str(session_id).map_err(|_| anyhow!("Invalid session_id"))?;

        let search = args["search"].as_str().map(|s| s.to_string());
        let limit = args["limit"].as_i64().unwrap_or(20);
        let offset = args["offset"].as_i64().unwrap_or(0);

        // Get current entity type from taxonomy stack
        let entity_type = {
            let sessions_guard = sessions.read().await;
            let session = sessions_guard
                .get(&session_uuid)
                .ok_or_else(|| anyhow!("Session not found: {}", session_uuid))?;

            session
                .context
                .taxonomy_stack
                .current()
                .map(|f| f.label.clone())
                .ok_or_else(|| anyhow!("No taxonomy node selected. Use taxonomy_drill_in first."))?
        };

        // Query entities of this type
        let service = TaxonomyService::new(pool.clone());
        let entities = service
            .list_entities_by_type(&entity_type, search.as_deref(), limit, offset)
            .await?;

        let entity_list: Vec<serde_json::Value> = entities
            .iter()
            .map(|e| {
                json!({
                    "entity_id": e.entity_id,
                    "name": e.name,
                    "entity_type": e.entity_type,
                    "created_at": e.created_at.map(|t| t.to_rfc3339())
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "entity_type": entity_type,
            "entities": entity_list,
            "count": entity_list.len(),
            "limit": limit,
            "offset": offset
        }))
    }

    // =========================================================================
    // Trading Matrix Tools
    // =========================================================================

    /// Get the trading matrix tree for a CBU
    ///
    /// Returns the hierarchical trading configuration:
    /// - Trading Universe (instrument classes -> markets -> currencies)
    /// - Standing Settlement Instructions (SSIs with booking rules)
    /// - Settlement Chains (multi-hop paths)
    /// - Tax Configuration (jurisdictions and statuses)
    /// - ISDA/CSA Agreements (OTC counterparties)
    pub(crate) async fn trading_matrix_get(&self, args: Value) -> Result<Value> {
        let cbu_id_str = args["cbu_id"]
            .as_str()
            .ok_or_else(|| anyhow!("cbu_id required"))?;
        let cbu_id = Uuid::parse_str(cbu_id_str).map_err(|_| anyhow!("Invalid cbu_id"))?;

        // Check if CBU exists
        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?;

        // Get trading profile status
        let profile = sqlx::query!(
            r#"
            SELECT profile_id, status, version, created_at, activated_at
            FROM "ob-poc".cbu_trading_profiles
            WHERE cbu_id = $1 AND status = 'ACTIVE'
            ORDER BY version DESC
            LIMIT 1
            "#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        // Count universe entries
        let universe_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.cbu_instrument_universe WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count SSIs
        let ssi_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.cbu_ssi WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count booking rules
        let rule_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.ssi_booking_rules WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count settlement chains
        let chain_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.cbu_settlement_chains WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Count ISDA agreements
        let isda_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM custody.isda_agreements WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        // Get instrument classes in universe
        let instrument_classes: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT ic.code
            FROM custody.cbu_instrument_universe u
            JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
            WHERE u.cbu_id = $1
            ORDER BY ic.code
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get markets in universe
        let markets: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT m.mic
            FROM custody.cbu_instrument_universe u
            JOIN custody.markets m ON m.market_id = u.market_id
            WHERE u.cbu_id = $1 AND u.market_id IS NOT NULL
            ORDER BY m.mic
            "#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Build summary
        let has_profile = profile.is_some();
        let profile_info = profile.map(|p| {
            json!({
                "profile_id": p.profile_id.to_string(),
                "status": p.status,
                "version": p.version,
                "activated_at": p.activated_at.map(|t| t.to_rfc3339())
            })
        });

        // Determine completeness
        let is_complete = universe_count > 0 && ssi_count > 0 && rule_count > 0;

        Ok(json!({
            "success": true,
            "cbu_id": cbu_id.to_string(),
            "cbu_name": cbu.name,
            "has_trading_profile": has_profile,
            "trading_profile": profile_info,
            "summary": {
                "universe_entries": universe_count,
                "instrument_classes": instrument_classes,
                "markets": markets,
                "ssis": ssi_count,
                "booking_rules": rule_count,
                "settlement_chains": chain_count,
                "isda_agreements": isda_count,
                "is_complete": is_complete
            },
            "endpoints": {
                "full_tree": format!("/api/cbu/{}/trading-matrix", cbu_id),
                "trading_profile_verbs": "Use verbs_list with domain='trading-profile' to see available operations"
            }
        }))
    }

    // =========================================================================
    // Feedback Inspector Handlers
    // =========================================================================

    pub(crate) async fn feedback_analyze(&self, args: Value) -> Result<Value> {
        use crate::feedback::FeedbackInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            since_hours: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;
        let since_hours = args.since_hours.unwrap_or(24);

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(
            pool.clone(),
            Some(std::path::PathBuf::from("/tmp/ob-poc-events.jsonl")),
        );

        let since = chrono::Utc::now() - chrono::Duration::hours(since_hours);
        let report = inspector.analyze(Some(since)).await?;

        Ok(json!({
            "total_failures": report.events_processed,
            "unique_issues": report.failures_created,
            "updated_issues": report.failures_updated,
            "by_error_type": report.by_error_type,
            "by_remediation_path": report.by_remediation_path,
            "analyzed_at": report.analyzed_at.to_rfc3339()
        }))
    }

    pub(crate) async fn feedback_list(&self, args: Value) -> Result<Value> {
        use crate::feedback::{FeedbackInspector, IssueFilter};

        #[derive(serde::Deserialize)]
        struct Args {
            status: Option<String>,
            error_type: Option<String>,
            verb: Option<String>,
            source: Option<String>,
            limit: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);

        let filter = IssueFilter {
            status: args.status.and_then(|s| parse_issue_status(&s)),
            error_type: args.error_type.and_then(|s| parse_error_type(&s)),
            verb: args.verb,
            source: args.source,
            limit: args.limit,
            ..Default::default()
        };

        let issues = inspector.list_issues(filter).await?;

        Ok(json!({
            "count": issues.len(),
            "issues": issues.iter().map(|i| json!({
                "fingerprint": i.fingerprint,
                "error_type": format!("{:?}", i.error_type),
                "status": format!("{:?}", i.status),
                "verb": i.verb,
                "source": i.source,
                "message": i.error_message,
                "occurrence_count": i.occurrence_count,
                "first_seen": i.first_seen_at.to_rfc3339(),
                "last_seen": i.last_seen_at.to_rfc3339(),
                "repro_verified": i.repro_verified
            })).collect::<Vec<_>>()
        }))
    }

    pub(crate) async fn feedback_get(&self, args: Value) -> Result<Value> {
        use crate::feedback::FeedbackInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);

        let issue = inspector.get_issue(&args.fingerprint).await?;

        match issue {
            Some(detail) => Ok(json!({
                "found": true,
                "failure": {
                    "id": detail.failure.id,
                    "fingerprint": detail.failure.fingerprint,
                    "error_type": format!("{:?}", detail.failure.error_type),
                    "remediation_path": format!("{:?}", detail.failure.remediation_path),
                    "status": format!("{:?}", detail.failure.status),
                    "verb": detail.failure.verb,
                    "source": detail.failure.source,
                    "message": detail.failure.error_message,
                    "context": detail.failure.error_context,
                    "user_intent": detail.failure.user_intent,
                    "command_sequence": detail.failure.command_sequence,
                    "repro_type": detail.failure.repro_type,
                    "repro_path": detail.failure.repro_path,
                    "repro_verified": detail.failure.repro_verified,
                    "fix_commit": detail.failure.fix_commit,
                    "fix_notes": detail.failure.fix_notes,
                    "occurrence_count": detail.failure.occurrence_count,
                    "first_seen": detail.failure.first_seen_at.to_rfc3339(),
                    "last_seen": detail.failure.last_seen_at.to_rfc3339()
                },
                "occurrences": detail.occurrences.iter().take(10).map(|o| json!({
                    "id": o.id,
                    "event_timestamp": o.event_timestamp.to_rfc3339(),
                    "session_id": o.session_id,
                    "verb": o.verb,
                    "duration_ms": o.duration_ms,
                    "message": o.error_message
                })).collect::<Vec<_>>(),
                "audit_trail": detail.audit_trail.iter().map(|a| json!({
                    "action": format!("{:?}", a.action),
                    "actor_type": format!("{:?}", a.actor_type),
                    "actor_id": a.actor_id,
                    "details": a.details,
                    "created_at": a.created_at.to_rfc3339()
                })).collect::<Vec<_>>()
            })),
            None => Ok(json!({
                "found": false,
                "fingerprint": args.fingerprint
            })),
        }
    }

    pub(crate) async fn feedback_repro(&self, args: Value) -> Result<Value> {
        use crate::feedback::{FeedbackInspector, ReproGenerator};

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);
        let tests_dir = std::path::PathBuf::from("tests/generated");
        let repro_gen = ReproGenerator::new(tests_dir);

        let result = repro_gen
            .generate_and_verify(&inspector, &args.fingerprint)
            .await?;

        Ok(json!({
            "fingerprint": args.fingerprint,
            "repro_type": format!("{:?}", result.repro_type),
            "path": result.repro_path.to_string_lossy(),
            "verified": result.verified,
            "output": result.output
        }))
    }

    pub(crate) async fn feedback_todo(&self, args: Value) -> Result<Value> {
        use crate::feedback::{FeedbackInspector, TodoGenerator};

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
            todo_number: i32,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);
        let todos_dir = std::path::PathBuf::from("todos/generated");
        let todo_gen = TodoGenerator::new(todos_dir);

        let result = todo_gen
            .generate_todo(&inspector, &args.fingerprint, args.todo_number)
            .await?;

        Ok(json!({
            "fingerprint": args.fingerprint,
            "todo_number": result.todo_number,
            "path": result.todo_path.to_string_lossy(),
            "content": result.content
        }))
    }

    pub(crate) async fn feedback_audit(&self, args: Value) -> Result<Value> {
        use crate::feedback::FeedbackInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;

        let pool = self.require_pool()?;
        let inspector = FeedbackInspector::new(pool.clone(), None);

        let trail = inspector.get_audit_trail(&args.fingerprint).await?;

        Ok(json!({
            "fingerprint": args.fingerprint,
            "count": trail.len(),
            "entries": trail.iter().map(|a| json!({
                "id": a.id,
                "action": format!("{:?}", a.action),
                "actor_type": format!("{:?}", a.actor_type),
                "actor_id": a.actor_id,
                "details": a.details,
                "evidence": a.evidence,
                "previous_status": a.previous_status.map(|s| format!("{:?}", s)),
                "new_status": a.new_status.map(|s| format!("{:?}", s)),
                "created_at": a.created_at.to_rfc3339()
            })).collect::<Vec<_>>()
        }))
    }
}

// =========================================================================
// Helper functions for parsing enum strings
// =========================================================================

pub(crate) fn parse_issue_status(s: &str) -> Option<crate::feedback::IssueStatus> {
    use crate::feedback::IssueStatus;
    match s.to_uppercase().as_str() {
        "NEW" => Some(IssueStatus::New),
        "REPRO_GENERATED" => Some(IssueStatus::ReproGenerated),
        "REPRO_VERIFIED" => Some(IssueStatus::ReproVerified),
        "TODO_CREATED" => Some(IssueStatus::TodoCreated),
        "IN_PROGRESS" => Some(IssueStatus::InProgress),
        "FIX_COMMITTED" => Some(IssueStatus::FixCommitted),
        "RESOLVED" => Some(IssueStatus::Resolved),
        "WONT_FIX" => Some(IssueStatus::WontFix),
        _ => None,
    }
}

pub(crate) fn parse_error_type(s: &str) -> Option<crate::feedback::ErrorType> {
    use crate::feedback::ErrorType;
    match s.to_uppercase().as_str() {
        "TIMEOUT" => Some(ErrorType::Timeout),
        "RATE_LIMITED" => Some(ErrorType::RateLimited),
        "ENUM_DRIFT" => Some(ErrorType::EnumDrift),
        "SCHEMA_DRIFT" => Some(ErrorType::SchemaDrift),
        "HANDLER_PANIC" => Some(ErrorType::HandlerPanic),
        "HANDLER_ERROR" => Some(ErrorType::HandlerError),
        "PARSE_ERROR" => Some(ErrorType::ParseError),
        "DSL_PARSE_ERROR" => Some(ErrorType::DslParseError),
        "VALIDATION_FAILED" => Some(ErrorType::ValidationFailed),
        _ => None,
    }
}
