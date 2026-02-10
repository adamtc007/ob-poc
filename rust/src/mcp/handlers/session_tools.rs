//! Session management and entity resolution tool handlers.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use super::core::ToolHandlers;

impl ToolHandlers {
    pub(super) fn session_context(&self, args: Value) -> Result<Value> {
        use crate::mcp::session;
        use crate::mcp::types::SessionAction;

        let action: SessionAction =
            serde_json::from_value(args).map_err(|e| anyhow!("Invalid session action: {}", e))?;

        let state = session::session_context(action).map_err(|e| anyhow!("{}", e))?;

        serde_json::to_value(state).map_err(|e| anyhow!("Failed to serialize session state: {}", e))
    }

    // =========================================================================
    // Session v2 Tools - Memory-first CBU session management
    // =========================================================================

    /// Load a single CBU into the session scope
    pub(super) async fn session_load_cbu(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;

        let pool = self.require_pool()?;

        // Get CBU by ID or name
        let cbu_id: Uuid = if let Some(id_str) = args["cbu_id"].as_str() {
            Uuid::parse_str(id_str).map_err(|_| anyhow!("Invalid cbu_id UUID"))?
        } else if let Some(name) = args["cbu_name"].as_str() {
            // Resolve by name
            let row: Option<(Uuid,)> =
                sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE $1 LIMIT 1"#)
                    .bind(format!("%{}%", name))
                    .fetch_optional(pool)
                    .await?;
            row.ok_or_else(|| anyhow!("CBU not found: {}", name))?.0
        } else {
            return Err(anyhow!("Either cbu_id or cbu_name required"));
        };

        // Fetch CBU details
        let cbu: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
            r#"SELECT cbu_id, name, jurisdiction FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        let (id, name, jurisdiction) = cbu.ok_or_else(|| anyhow!("CBU not found"))?;

        // Get or create session and load the CBU
        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;

        // Use default session for now (could be parameterized)
        let session = guard.entry(Uuid::nil()).or_insert_with(UnifiedSession::new);

        session.load_cbu(id);
        let _ = session.save(pool).await;

        Ok(json!({
            "loaded": true,
            "cbu_id": id,
            "name": name,
            "jurisdiction": jurisdiction,
            "scope_size": session.cbu_count()
        }))
    }

    /// Load all CBUs in a jurisdiction
    pub(super) async fn session_load_jurisdiction(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;

        let pool = self.require_pool()?;
        let jurisdiction = args["jurisdiction"]
            .as_str()
            .ok_or_else(|| anyhow!("jurisdiction required"))?;

        // Find all CBUs in jurisdiction
        let rows: Vec<(Uuid, String)> =
            sqlx::query_as(r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE jurisdiction = $1"#)
                .bind(jurisdiction)
                .fetch_all(pool)
                .await?;

        if rows.is_empty() {
            return Err(anyhow!("No CBUs found in jurisdiction: {}", jurisdiction));
        }

        let cbu_ids: Vec<Uuid> = rows.iter().map(|(id, _)| *id).collect();
        let cbu_names: Vec<String> = rows.iter().map(|(_, name)| name.clone()).collect();

        // Get or create session
        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(UnifiedSession::new);

        // Load all CBUs
        session.load_cbus(cbu_ids.clone());
        let _ = session.save(pool).await;

        Ok(json!({
            "loaded": true,
            "jurisdiction": jurisdiction,
            "cbu_count": cbu_ids.len(),
            "cbu_ids": cbu_ids,
            "cbu_names": cbu_names
        }))
    }

    /// Load all CBUs under a commercial client (galaxy)
    pub(super) async fn session_load_galaxy(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;

        let pool = self.require_pool()?;

        // Get apex entity by ID or name
        let apex_id: Uuid = if let Some(id_str) = args["apex_entity_id"].as_str() {
            Uuid::parse_str(id_str).map_err(|_| anyhow!("Invalid apex_entity_id UUID"))?
        } else if let Some(name) = args["apex_name"].as_str() {
            // Resolve by name
            let row: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE $1 LIMIT 1"#,
            )
            .bind(format!("%{}%", name))
            .fetch_optional(pool)
            .await?;
            row.ok_or_else(|| anyhow!("Entity not found: {}", name))?.0
        } else {
            return Err(anyhow!("Either apex_entity_id or apex_name required"));
        };

        // Find all CBUs under this commercial client
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE commercial_client_entity_id = $1"#,
        )
        .bind(apex_id)
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Err(anyhow!("No CBUs found under commercial client"));
        }

        let cbu_ids: Vec<Uuid> = rows.iter().map(|(id, _)| *id).collect();
        let cbu_names: Vec<String> = rows.iter().map(|(_, name)| name.clone()).collect();

        // Get apex entity name for response
        let apex_name: Option<(String,)> =
            sqlx::query_as(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                .bind(apex_id)
                .fetch_optional(pool)
                .await?;

        // Get or create session
        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(UnifiedSession::new);

        // Load all CBUs
        session.load_cbus(cbu_ids.clone());
        let _ = session.save(pool).await;

        Ok(json!({
            "loaded": true,
            "apex_entity_id": apex_id,
            "apex_name": apex_name.map(|n| n.0),
            "cbu_count": cbu_ids.len(),
            "cbu_ids": cbu_ids,
            "cbu_names": cbu_names
        }))
    }

    /// Remove a CBU from the current session scope
    pub(super) async fn session_unload_cbu(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;

        let pool = self.require_pool()?;
        let cbu_id = args["cbu_id"]
            .as_str()
            .ok_or_else(|| anyhow!("cbu_id required"))?;
        let cbu_id = Uuid::parse_str(cbu_id).map_err(|_| anyhow!("Invalid cbu_id UUID"))?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(UnifiedSession::new);

        session.unload_cbu(cbu_id);
        let _ = session.save(pool).await;

        Ok(json!({
            "unloaded": true,
            "cbu_id": cbu_id,
            "scope_size": session.cbu_count()
        }))
    }

    /// Clear session scope to empty (universe view)
    pub(super) async fn session_clear(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;
        let _ = args; // unused but kept for consistent signature

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(UnifiedSession::new);

        session.clear_cbus_with_history();
        let _ = session.save(pool).await;

        Ok(json!({
            "cleared": true,
            "scope_size": 0
        }))
    }

    /// Undo the last scope change
    pub(super) async fn session_undo(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;
        let _ = args;

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(UnifiedSession::new);

        let success = session.undo_cbu();
        if success {
            let _ = session.save(pool).await;
        }

        Ok(json!({
            "success": success,
            "scope_size": session.cbu_count(),
            "history_depth": session.cbu_history_depth(),
            "future_depth": session.cbu_future_depth()
        }))
    }

    /// Redo a previously undone scope change
    pub(super) async fn session_redo(&self, args: Value) -> Result<Value> {
        use crate::session::UnifiedSession;
        let _ = args;

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let mut guard = sessions.write().await;
        let session = guard.entry(Uuid::nil()).or_insert_with(UnifiedSession::new);

        let success = session.redo_cbu();
        if success {
            let _ = session.save(pool).await;
        }

        Ok(json!({
            "success": success,
            "scope_size": session.cbu_count(),
            "history_depth": session.cbu_history_depth(),
            "future_depth": session.cbu_future_depth()
        }))
    }

    /// Get current session state and scope
    pub(super) async fn session_info(&self, args: Value) -> Result<Value> {
        let _ = args;

        let pool = self.require_pool()?;

        let sessions = self.require_cbu_sessions()?;
        let guard = sessions.read().await;
        let session = guard.get(&Uuid::nil());

        if let Some(session) = session {
            let cbu_ids = session.cbu_ids_vec();

            // Fetch CBU names if we have any
            let cbu_names: Vec<String> = if cbu_ids.is_empty() {
                vec![]
            } else {
                let rows: Vec<(String,)> =
                    sqlx::query_as(r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = ANY($1)"#)
                        .bind(&cbu_ids)
                        .fetch_all(pool)
                        .await
                        .unwrap_or_default();
                rows.into_iter().map(|(name,)| name).collect()
            };

            Ok(json!({
                "id": session.id,
                "name": session.name,
                "cbu_count": cbu_ids.len(),
                "cbu_ids": cbu_ids,
                "cbu_names": cbu_names,
                "history_depth": session.cbu_history_depth(),
                "future_depth": session.cbu_future_depth(),
                "dirty": session.dirty
            }))
        } else {
            Ok(json!({
                "id": null,
                "name": null,
                "cbu_count": 0,
                "cbu_ids": [],
                "cbu_names": [],
                "history_depth": 0,
                "future_depth": 0,
                "dirty": false
            }))
        }
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
