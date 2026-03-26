//! Session management and entity resolution tool handlers.
//!
//! CBU scope navigation tools now return deprecation errors — scope navigation
//! is handled by the unified REPL V2 pipeline via POST /api/session/:id/input.
//! Entity search, verb surface, and resolution tools are still active.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use super::core::ToolHandlers;

impl ToolHandlers {
    // =========================================================================
    // Session scope tools (deprecated — use REPL V2 unified pipeline)
    // =========================================================================

    pub(super) async fn session_load_cbu(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.load-cbu: Use the unified session pipeline (POST /api/session/:id/input) instead"))
    }

    pub(super) async fn session_load_cluster(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.load-cluster: Use the unified session pipeline instead"))
    }

    pub(super) async fn session_load_jurisdiction(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.load-jurisdiction: Use the unified session pipeline instead"))
    }

    pub(super) async fn session_load_galaxy(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.load-galaxy: Use the unified session pipeline instead"))
    }

    pub(super) async fn session_unload_cbu(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.unload-cbu: Use the unified session pipeline instead"))
    }

    pub(super) async fn session_clear(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.clear: Use the unified session pipeline instead"))
    }

    pub(super) async fn session_undo(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.undo: Use the unified session pipeline instead"))
    }

    pub(super) async fn session_redo(&self, _args: Value) -> Result<Value> {
        Err(anyhow!("session.redo: Use the unified session pipeline instead"))
    }

    pub(super) async fn session_info(&self, args: Value) -> Result<Value> {
        let session_id = args["session_id"].as_str().unwrap_or("unknown");
        Ok(json!({
            "session_id": session_id,
            "status": "active",
            "note": "Session management via unified REPL V2 pipeline."
        }))
    }

    // =========================================================================
    // Active tools (verb surface, entity search, resolution)
    // =========================================================================

    pub(super) async fn session_verb_surface(&self, args: Value) -> Result<Value> {
        use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
        use crate::agent::verb_surface::{
            compute_session_verb_surface, VerbSurfaceContext, VerbSurfaceFailPolicy,
        };

        let include_excluded = args["include_excluded"].as_bool().unwrap_or(false);
        let domain_filter = args["domain_filter"].as_str();

        // Build VerbSurfaceContext from current session state.
        // In the MCP context we use defaults for most fields since there's no
        // active chat session — the tool is introspection-only.
        let agent_mode = self.agent_mode;
        let envelope = SemOsContextEnvelope::unavailable();
        let ctx = VerbSurfaceContext {
            agent_mode,
            stage_focus: None,
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::default(),
            entity_state: None,
            has_group_scope: true,
            composite_state: None,
        };
        let surface = compute_session_verb_surface(&ctx);

        // Filter verbs by domain if requested
        let verbs: Vec<&crate::agent::verb_surface::SurfaceVerb> =
            if let Some(domain) = domain_filter {
                surface.verbs_for_domain(domain)
            } else {
                surface.verbs.iter().collect()
            };

        let verbs_json: Vec<serde_json::Value> = verbs
            .iter()
            .map(|v| {
                json!({
                    "fqn": v.fqn,
                    "domain": v.domain,
                    "action": v.action,
                    "description": v.description,
                    "governance_tier": v.governance_tier,
                    "lifecycle_eligible": v.lifecycle_eligible,
                    "rank_boost": v.rank_boost,
                })
            })
            .collect();

        let mut result = json!({
            "verbs": verbs_json,
            "verb_count": verbs.len(),
            "surface_fingerprint": surface.surface_fingerprint.0,
            "fail_policy": format!("{:?}", surface.fail_policy_applied),
            "computed_at": surface.computed_at.to_rfc3339(),
            "filter_summary": {
                "total_registry": surface.filter_summary.total_registry,
                "after_agent_mode": surface.filter_summary.after_agent_mode,
                "after_workflow": surface.filter_summary.after_workflow,
                "after_semreg": surface.filter_summary.after_semreg,
                "after_lifecycle": surface.filter_summary.after_lifecycle,
                "after_actor": surface.filter_summary.after_actor,
                "final_count": surface.filter_summary.final_count,
            },
        });

        if include_excluded {
            let excluded_json: Vec<serde_json::Value> = surface
                .excluded
                .iter()
                .map(|e| {
                    let reasons: Vec<serde_json::Value> = e
                        .reasons
                        .iter()
                        .map(|r| {
                            json!({
                                "layer": format!("{:?}", r.layer),
                                "reason": r.reason,
                            })
                        })
                        .collect();
                    json!({
                        "fqn": e.fqn,
                        "reasons": reasons,
                    })
                })
                .collect();
            result["excluded"] = json!(excluded_json);
            result["excluded_count"] = json!(surface.excluded.len());
        }

        Ok(result)
    }

    /// List saved sessions
    pub(super) async fn session_list(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;

        let pool = self.require_pool()?;
        let limit = args["limit"].as_i64().unwrap_or(20);

        let sessions = UnifiedSession::list_recent(None, limit, pool)
            .await
            .unwrap_or_default();

        Ok(json!({
            "sessions": sessions.iter().map(|s| json!({
                "id": s.id,
                "name": s.name,
                "cbu_count": s.cbu_count,
                "updated_at": s.updated_at.to_rfc3339()
            })).collect::<Vec<_>>()
        }))
    }

    /// Search for entities with fuzzy matching, enrichment, and smart disambiguation
    ///
    /// Returns matches enriched with context (roles, relationships, dates) and
    /// uses resolution strategy to determine whether to auto-resolve, ask user,
    /// or suggest creating a new entity.
    ///
    /// ## Features
    /// - Rich context for disambiguation (nationality, DOB, roles, ownership)
    /// - Context-aware auto-resolution (e.g., "the director" → picks entity with DIRECTOR role)
    /// - Human-readable disambiguation labels
    /// - Confidence scoring with suggested actions
    pub(super) async fn entity_search(&self, args: Value) -> Result<Value> {
        use crate::mcp::enrichment::{EntityEnricher, EntityType as EnrichEntityType};
        use crate::mcp::resolution::{ConversationContext, EnrichedMatch, ResolutionStrategy};

        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow!("query required"))?;
        let entity_type_str = args["entity_type"].as_str();
        let limit = args["limit"].as_i64().unwrap_or(10) as i32;

        // Parse conversation hints for context-aware resolution
        let conversation_hints: Option<ConversationContext> = args
            .get("conversation_hints")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // Map entity_type to EntityGateway nickname
        let nickname = match entity_type_str {
            Some("cbu") => "CBU",
            Some("entity") => "ENTITY",
            Some("person") => "PERSON",
            Some("company") | Some("legal_entity") => "LEGAL_ENTITY",
            Some("document") => "DOCUMENT",
            Some("product") => "PRODUCT",
            Some("service") => "SERVICE",
            None => "ENTITY", // Default to entity search
            Some(t) => {
                return Err(anyhow!(
                    "Unknown entity_type: {}. Valid types: cbu, entity, person, company, document, product, service",
                    t
                ));
            }
        };

        // Step 1: Search via EntityGateway
        let raw_matches = self.gateway_search(nickname, Some(query), limit).await?;

        if raw_matches.is_empty() {
            let result = crate::mcp::resolution::ResolutionResult {
                confidence: crate::mcp::resolution::ResolutionConfidence::None,
                action: crate::mcp::resolution::SuggestedAction::SuggestCreate,
                prompt: Some(format!(
                    "No matches found for '{}'. Would you like to create a new entity?",
                    query
                )),
            };
            return Ok(json!({
                "matches": [],
                "resolution_confidence": result.confidence,
                "suggested_action": result.action,
                "disambiguation_prompt": result.prompt
            }));
        }

        // Step 2: Extract UUIDs for enrichment
        let ids: Vec<Uuid> = raw_matches
            .iter()
            .filter_map(|(id, _, _)| Uuid::parse_str(id).ok())
            .collect();

        // Step 3: Determine entity type for enrichment
        let enrich_type = match entity_type_str {
            Some("person") => EnrichEntityType::ProperPerson,
            Some("company") | Some("legal_entity") => EnrichEntityType::LegalEntity,
            Some("cbu") => EnrichEntityType::Cbu,
            _ => EnrichEntityType::ProperPerson, // Default
        };

        // Step 4: Enrich with context (roles, nationality, etc.)
        let enricher = EntityEnricher::new(self.pool.clone());
        let contexts = enricher.enrich(enrich_type, &ids).await.unwrap_or_default();

        // Step 5: Build enriched matches with disambiguation labels
        let enriched_matches: Vec<EnrichedMatch> = raw_matches
            .iter()
            .map(|(id, display, score)| {
                let uuid = Uuid::parse_str(id).ok();
                let context = uuid
                    .and_then(|u| contexts.get(&u).cloned())
                    .unwrap_or_default();
                let disambiguation_label = context.disambiguation_label(display, enrich_type);

                EnrichedMatch {
                    id: id.clone(),
                    display: display.clone(),
                    score: *score,
                    entity_type: entity_type_str.unwrap_or("entity").to_string(),
                    context,
                    disambiguation_label,
                }
            })
            .collect();

        // Step 6: Analyze and determine resolution strategy
        let resolution =
            ResolutionStrategy::analyze(&enriched_matches, conversation_hints.as_ref());

        // Step 7: Build response
        Ok(json!({
            "matches": enriched_matches,
            "resolution_confidence": resolution.confidence,
            "suggested_action": resolution.action,
            "disambiguation_prompt": resolution.prompt
        }))
    }

    // ==================== Resolution Sub-Session Tools ====================

    /// Start a resolution sub-session
    pub(super) async fn resolution_start(&self, args: Value) -> Result<Value> {
        use crate::session::{
            EntityMatchInfo, ResolutionSubSession, SubSessionType, UnifiedSession,
            UnresolvedRefInfo,
        };

        let sessions = self.require_sessions()?;

        let parent_id: Uuid = args["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("session_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid session_id UUID"))?;

        let parent_dsl_index = args["parent_dsl_index"].as_u64().unwrap_or(0) as usize;

        // Parse unresolved refs
        let unresolved_refs_json = args["unresolved_refs"]
            .as_array()
            .ok_or_else(|| anyhow!("unresolved_refs array required"))?;

        let unresolved_refs: Vec<UnresolvedRefInfo> = unresolved_refs_json
            .iter()
            .map(|r| {
                let initial_matches = r["initial_matches"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|m| EntityMatchInfo {
                                value: m["value"].as_str().unwrap_or("").to_string(),
                                display: m["display"].as_str().unwrap_or("").to_string(),
                                score_pct: m["score_pct"].as_u64().unwrap_or(0) as u8,
                                detail: m["detail"].as_str().map(|s| s.to_string()),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                UnresolvedRefInfo {
                    ref_id: r["ref_id"].as_str().unwrap_or("").to_string(),
                    search_value: r["search_value"].as_str().unwrap_or("").to_string(),
                    entity_type: r["entity_type"].as_str().unwrap_or("entity").to_string(),
                    context_line: r["context_line"].as_str().unwrap_or("").to_string(),
                    initial_matches,
                    resolved_key: None,
                    resolved_display: None,
                }
            })
            .collect();

        // Get parent session
        let parent = {
            let sessions_guard = sessions.read().await;
            sessions_guard.get(&parent_id).cloned()
        }
        .ok_or_else(|| anyhow!("Parent session {} not found", parent_id))?;

        // Create resolution sub-session
        let resolution_state = ResolutionSubSession {
            unresolved_refs: unresolved_refs.clone(),
            parent_dsl_index,
            current_ref_index: 0,
            resolutions: std::collections::HashMap::new(),
        };

        let child =
            UnifiedSession::new_subsession(&parent, SubSessionType::Resolution(resolution_state));
        let child_id = child.id;

        // Store child session
        {
            let mut sessions_guard = sessions.write().await;
            sessions_guard.insert(child_id, child);
        }

        // Return sub-session info
        Ok(json!({
            "subsession_id": child_id.to_string(),
            "parent_id": parent_id.to_string(),
            "unresolved_count": unresolved_refs.len(),
            "current_ref": unresolved_refs.first().map(|r| json!({
                "ref_id": r.ref_id,
                "search_value": r.search_value,
                "entity_type": r.entity_type,
                "matches": r.initial_matches.iter().map(|m| json!({
                    "value": m.value,
                    "display": m.display,
                    "score_pct": m.score_pct,
                    "detail": m.detail
                })).collect::<Vec<_>>()
            }))
        }))
    }

    /// Refine search using discriminators
    pub(super) async fn resolution_search(&self, args: Value) -> Result<Value> {
        use crate::session::SubSessionType;

        let sessions = self.require_sessions()?;

        let subsession_id: Uuid = args["subsession_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subsession_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subsession_id UUID"))?;

        // Get sub-session
        let session = {
            let sessions_guard = sessions.read().await;
            sessions_guard.get(&subsession_id).cloned()
        }
        .ok_or_else(|| anyhow!("Sub-session {} not found", subsession_id))?;

        let SubSessionType::Resolution(resolution) = &session.sub_session_type else {
            return Err(anyhow!(
                "Session {} is not a resolution sub-session",
                subsession_id
            ));
        };

        let current_ref = resolution
            .unresolved_refs
            .get(resolution.current_ref_index)
            .ok_or_else(|| anyhow!("No current reference to resolve"))?;

        // Parse discriminators
        let discriminators = args.get("discriminators");
        let natural_language = args["natural_language"].as_str();

        // Build search query with discriminators
        let base_query = &current_ref.search_value;
        let entity_type = &current_ref.entity_type;

        // For now, re-search with the base query
        // TODO: Apply discriminators to filter results
        let nickname = match entity_type.as_str() {
            "person" => "PERSON",
            "company" | "legal_entity" => "LEGAL_ENTITY",
            "cbu" => "CBU",
            _ => "ENTITY",
        };

        let raw_matches = self.gateway_search(nickname, Some(base_query), 10).await?;

        // Apply discriminator filtering (basic implementation)
        let filtered_matches = raw_matches;

        if let Some(disc) = discriminators {
            // TODO: Implement proper discriminator filtering via EntityEnricher
            // For now, log that we received discriminators
            tracing::debug!(
                "Resolution search with discriminators: {:?}, natural_language: {:?}",
                disc,
                natural_language
            );
        }

        Ok(json!({
            "ref_id": current_ref.ref_id,
            "search_value": current_ref.search_value,
            "matches": filtered_matches.iter().map(|(id, display, score)| json!({
                "value": id,
                "display": display,
                "score_pct": (score * 100.0) as u8
            })).collect::<Vec<_>>(),
            "discriminators_applied": discriminators.is_some(),
            "natural_language_parsed": natural_language.is_some()
        }))
    }

    /// Select a match to resolve current reference
    pub(super) async fn resolution_select(&self, args: Value) -> Result<Value> {
        use crate::session::SubSessionType;

        let sessions = self.require_sessions()?;

        let subsession_id: Uuid = args["subsession_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subsession_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subsession_id UUID"))?;

        let selection = args["selection"].as_u64();
        let entity_id = args["entity_id"].as_str();

        if selection.is_none() && entity_id.is_none() {
            return Err(anyhow!("Either selection index or entity_id required"));
        }

        // Get and update sub-session
        let mut session = {
            let sessions_guard = sessions.read().await;
            sessions_guard.get(&subsession_id).cloned()
        }
        .ok_or_else(|| anyhow!("Sub-session {} not found", subsession_id))?;

        // Extract values and update resolution in a scope that ends before we move session
        let (
            ref_id,
            selected_value,
            is_complete,
            resolutions_count,
            remaining_count,
            next_ref_json,
        ) = {
            let SubSessionType::Resolution(resolution) = &mut session.sub_session_type else {
                return Err(anyhow!(
                    "Session {} is not a resolution sub-session",
                    subsession_id
                ));
            };

            let current_ref = resolution
                .unresolved_refs
                .get(resolution.current_ref_index)
                .ok_or_else(|| anyhow!("No current reference to resolve"))?;

            // Determine the selected value
            let selected_value = if let Some(idx) = selection {
                let match_info = current_ref
                    .initial_matches
                    .get(idx as usize)
                    .ok_or_else(|| anyhow!("Selection index {} out of range", idx))?;
                match_info.value.clone()
            } else if let Some(eid) = entity_id {
                eid.to_string()
            } else {
                return Err(anyhow!("No selection provided"));
            };

            // Record resolution
            let ref_id = current_ref.ref_id.clone();
            resolution
                .resolutions
                .insert(ref_id.clone(), selected_value.clone());

            // Move to next
            resolution.current_ref_index += 1;

            let is_complete = resolution.current_ref_index >= resolution.unresolved_refs.len();
            let next_ref_json = if !is_complete {
                resolution
                    .unresolved_refs
                    .get(resolution.current_ref_index)
                    .map(|r| {
                        json!({
                            "ref_id": r.ref_id,
                            "search_value": r.search_value,
                            "entity_type": r.entity_type,
                            "context_line": r.context_line,
                            "initial_matches": r.initial_matches
                        })
                    })
            } else {
                None
            };

            let resolutions_count = resolution.current_ref_index;
            let remaining_count = resolution.unresolved_refs.len() - resolution.current_ref_index;

            (
                ref_id,
                selected_value,
                is_complete,
                resolutions_count,
                remaining_count,
                next_ref_json,
            )
        };

        // Store updated session
        {
            let mut sessions_guard = sessions.write().await;
            sessions_guard.insert(subsession_id, session);
        }

        Ok(json!({
            "resolved": {
                "ref_id": ref_id,
                "value": selected_value
            },
            "is_complete": is_complete,
            "resolutions_count": resolutions_count,
            "remaining_count": remaining_count,
            "next_ref": next_ref_json
        }))
    }

    /// Complete resolution sub-session
    pub(super) async fn resolution_complete(&self, args: Value) -> Result<Value> {
        use crate::api::session::BoundEntity;
        use crate::session::SubSessionType;

        let sessions = self.require_sessions()?;

        let subsession_id: Uuid = args["subsession_id"]
            .as_str()
            .ok_or_else(|| anyhow!("subsession_id required"))?
            .parse()
            .map_err(|_| anyhow!("Invalid subsession_id UUID"))?;

        let apply = args["apply"].as_bool().unwrap_or(true);

        // Remove child session
        let child = {
            let mut sessions_guard = sessions.write().await;
            sessions_guard.remove(&subsession_id)
        }
        .ok_or_else(|| anyhow!("Sub-session {} not found", subsession_id))?;

        let parent_id = child
            .parent_session_id
            .ok_or_else(|| anyhow!("Session {} has no parent", subsession_id))?;

        let SubSessionType::Resolution(resolution) = &child.sub_session_type else {
            return Err(anyhow!(
                "Session {} is not a resolution sub-session",
                subsession_id
            ));
        };

        let resolutions_count = resolution.resolutions.len();

        if apply && resolutions_count > 0 {
            // Build bound entities from resolutions
            let mut bound_entities = Vec::new();
            for unresolved in &resolution.unresolved_refs {
                if let Some(resolved_value) = resolution.resolutions.get(&unresolved.ref_id) {
                    // Find match info
                    let match_info = unresolved
                        .initial_matches
                        .iter()
                        .find(|m| &m.value == resolved_value);

                    if let Some(info) = match_info {
                        if let Ok(uuid) = Uuid::parse_str(resolved_value) {
                            bound_entities.push((
                                unresolved.ref_id.clone(),
                                BoundEntity {
                                    id: uuid,
                                    entity_type: unresolved.entity_type.clone(),
                                    display_name: info.display.clone(),
                                },
                            ));
                        }
                    }
                }
            }

            // Apply to parent session
            {
                let mut sessions_guard = sessions.write().await;
                if let Some(parent) = sessions_guard.get_mut(&parent_id) {
                    for (ref_id, bound_entity) in &bound_entities {
                        parent
                            .context
                            .bindings
                            .insert(ref_id.clone(), bound_entity.clone());
                        tracing::info!(
                            "Applied resolution: {} -> {} ({})",
                            ref_id,
                            bound_entity.id,
                            bound_entity.display_name
                        );
                    }
                }
            }
        }

        Ok(json!({
            "success": true,
            "parent_id": parent_id.to_string(),
            "resolutions_applied": if apply { resolutions_count } else { 0 },
            "message": format!(
                "Resolution complete. {} bindings {}.",
                resolutions_count,
                if apply { "applied to parent" } else { "discarded" }
            )
        }))
    }
}
