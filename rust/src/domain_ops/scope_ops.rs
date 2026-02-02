//! Entity Scope Operations - Pattern B Runtime Scope Resolution
//!
//! This module implements runtime entity scope resolution via plugin verbs.
//! It does NOT modify the parser/AST/compiler - all scope handling is at execute time.
//!
//! # Architecture
//!
//! Pattern B (runtime) scope resolution:
//! 1. `scope.commit` searches entities within client group and creates immutable snapshot
//! 2. Snapshot is bound to `@sX` symbol as a normal UUID (type "entity_scope")
//! 3. Executor rewrites `:scope @sX` args to `:entity-ids [...]` at execution time
//!
//! # Non-Negotiables (from spec)
//!
//! 1. No parser/AST changes - `@s1` is a normal `:as` binding
//! 2. Determinism from snapshot - ordered `selected_entity_ids` (score DESC, uuid ASC)
//! 3. Cross-group protection - every snapshot stores `client_group_id`
//! 4. MVP requires only `scope.commit`
//!
//! # Verbs
//!
//! - `scope.commit` - Search entities and create immutable snapshot (MVP)
//! - `scope.resolve` (future) - Preview search results without committing
//! - `scope.narrow` (future) - Filter an existing scope
//! - `scope.union` (future) - Combine multiple scopes

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::{PgPool, Row};

// =============================================================================
// RESULT TYPES
// =============================================================================

/// Result of scope.commit operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeCommitResult {
    pub snapshot_id: Uuid,
    pub entity_count: usize,
    pub description: String,
    pub resolution_method: String,
}

/// Candidate entity from search (for learning/debug)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeCandidate {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub score: f64,
    pub match_type: String,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

#[cfg(feature = "database")]
fn get_optional_int(verb_call: &VerbCall, key: &str) -> Option<i32> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer().map(|i| i as i32))
}

#[cfg(feature = "database")]
fn get_optional_bool(verb_call: &VerbCall, key: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_boolean())
}

// =============================================================================
// SCOPE.COMMIT OPERATION
// =============================================================================

/// Search entities within client group and create immutable snapshot.
///
/// # Requirements (from spec)
/// 1. Requires `ctx.client_group_id` present (error if not set)
/// 2. Calls existing SQL: `search_entity_tags()` or `search_entity_tags_semantic()`
/// 3. Produces ordered `selected_entity_ids` (score DESC, uuid ASC)
/// 4. Inserts snapshot row with `client_group_id`
/// 5. Returns snapshot UUID, bound via `:as @symbol` typed "entity_scope"
#[register_custom_op]
pub struct ScopeCommitOp;

#[async_trait]
impl CustomOperation for ScopeCommitOp {
    fn domain(&self) -> &'static str {
        "scope"
    }

    fn verb(&self) -> &'static str {
        "commit"
    }

    fn rationale(&self) -> &'static str {
        "Searches entities within client group scope and creates immutable snapshot for \
         deterministic replay. Pattern B runtime scope resolution."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // 1. REQUIRE client_group_id (Non-negotiable)
        let group_id = ctx.client_group_id.ok_or_else(|| {
            anyhow::anyhow!(
                "Set client group first (use 'work on <client>'). \
                 scope.commit requires an active client context."
            )
        })?;

        // 2. Extract arguments
        let desc = get_required_string(verb_call, "desc")?;
        let limit = get_optional_int(verb_call, "limit").unwrap_or(50).min(100);
        let semantic = get_optional_bool(verb_call, "semantic").unwrap_or(false);
        let persona = get_optional_string(verb_call, "persona").or_else(|| ctx.persona.clone());
        let mode = get_optional_string(verb_call, "mode").unwrap_or_else(|| "strict".to_string());

        tracing::info!(
            group_id = %group_id,
            desc = %desc,
            limit = limit,
            semantic = semantic,
            persona = ?persona,
            mode = %mode,
            "scope.commit: searching entities"
        );

        // 3. Call existing search functions
        let (entity_ids, candidates, method) = if semantic {
            search_semantic(pool, group_id, &desc, persona.as_deref(), limit).await?
        } else {
            search_fuzzy(pool, group_id, &desc, persona.as_deref(), limit).await?
        };

        // 4. Handle empty results in strict mode
        if entity_ids.is_empty() && mode == "strict" {
            return Err(anyhow::anyhow!(
                "No entities found matching '{}' in client group. \
                 Try a different search term or use semantic: true for fuzzy matching.",
                desc
            ));
        }

        let entity_count = entity_ids.len();

        // 5. Build descriptor JSONB
        let descriptor = json!({
            "desc": desc,
            "limit": limit,
            "mode": mode,
            "semantic": semantic,
            "persona": persona,
        });

        // 6. Insert snapshot (immutable - trigger prevents updates)
        let snapshot_row = sqlx::query(
            r#"
            INSERT INTO "ob-poc".scope_snapshots
                (group_id, description, filter_applied, limit_requested, mode,
                 selected_entity_ids, top_k_candidates, resolution_method, session_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(group_id)
        .bind(&desc)
        .bind(&descriptor)
        .bind(limit)
        .bind(&mode)
        .bind(&entity_ids)
        .bind(&candidates)
        .bind(&method)
        .bind(ctx.session_id)
        .fetch_one(pool)
        .await?;

        let snapshot_id: Uuid = snapshot_row.get("id");

        tracing::info!(
            snapshot_id = %snapshot_id,
            entity_count = entity_count,
            method = %method,
            "scope.commit: created snapshot"
        );

        // 7. Return UUID - caller binds via :as @symbol with type "entity_scope"
        let result = ScopeCommitResult {
            snapshot_id,
            entity_count,
            description: desc,
            resolution_method: method,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "scope.commit requires database feature to be enabled"
        ))
    }
}

// =============================================================================
// SEARCH HELPERS
// =============================================================================

/// Search entities using fuzzy text matching (via search_entity_tags SQL function)
#[cfg(feature = "database")]
async fn search_fuzzy(
    pool: &PgPool,
    group_id: Uuid,
    desc: &str,
    persona: Option<&str>,
    limit: i32,
) -> Result<(Vec<Uuid>, serde_json::Value, String)> {
    // Call existing SQL function from migration 052
    let rows = sqlx::query(
        r#"
        SELECT
            entity_id,
            entity_name,
            tag,
            confidence,
            match_type
        FROM "ob-poc".search_entity_tags($1, $2, $3, $4, FALSE)
        "#,
    )
    .bind(group_id)
    .bind(desc)
    .bind(persona)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    // Build ordered entity_ids (already ordered by search function: priority, confidence DESC)
    // Add secondary sort by UUID for determinism when scores are equal
    let mut scored: Vec<(Uuid, String, f64, String)> = rows
        .iter()
        .map(|r| {
            (
                r.get::<Uuid, _>("entity_id"),
                r.get::<String, _>("entity_name"),
                r.get::<f64, _>("confidence"),
                r.get::<String, _>("match_type"),
            )
        })
        .collect();

    // Sort: confidence DESC, then UUID ASC for determinism
    scored.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let entity_ids: Vec<Uuid> = scored.iter().map(|(id, _, _, _)| *id).collect();

    // Build candidates for learning/debug
    let candidates: Vec<ScopeCandidate> = scored
        .iter()
        .map(|(id, name, score, match_type)| ScopeCandidate {
            entity_id: *id,
            entity_name: name.clone(),
            score: *score,
            match_type: match_type.clone(),
        })
        .collect();

    Ok((
        entity_ids,
        serde_json::to_value(&candidates)?,
        "fuzzy_text".to_string(),
    ))
}

/// Search entities using semantic (Candle) matching
///
/// Uses the `search_entity_tags_semantic` SQL function with pre-computed embeddings
/// from `client_group_entity_tag_embedding` table.
///
/// For now, falls back to enhanced fuzzy search since:
/// 1. Semantic search requires pre-computed tag embeddings in DB
/// 2. The ExecutionContext doesn't have access to the Candle embedder
///
/// The fuzzy search is actually quite effective for entity tags because:
/// - Tags are typically short, descriptive phrases
/// - PostgreSQL trigram matching handles typos and partial matches well
/// - The tag system already includes aliases and normalized forms
///
/// Future enhancement when needed:
/// 1. Add SharedEmbedder to ExecutionContext or DslExecutor
/// 2. Embed the desc using Candle embed_query()
/// 3. Call search_entity_tags_semantic with the embedding vector
#[cfg(feature = "database")]
async fn search_semantic(
    pool: &PgPool,
    group_id: Uuid,
    desc: &str,
    persona: Option<&str>,
    limit: i32,
) -> Result<(Vec<Uuid>, serde_json::Value, String)> {
    // Use enhanced fuzzy search with broader matching
    // The "semantic" flag signals user intent for broader results,
    // so we increase the limit and use similarity threshold
    let enhanced_limit = (limit * 2).min(100); // Search wider, then take top matches

    tracing::info!(
        "scope.commit semantic mode: using enhanced fuzzy search for '{}'",
        desc
    );

    let (entity_ids, candidates, _) =
        search_fuzzy(pool, group_id, desc, persona, enhanced_limit).await?;

    // Take the requested limit from the broader search
    let entity_ids: Vec<Uuid> = entity_ids.into_iter().take(limit as usize).collect();

    Ok((
        entity_ids,
        candidates,
        "semantic_enhanced_fuzzy".to_string(),
    ))
}

// =============================================================================
// SCOPE.RESOLVE OPERATION (Preview without committing)
// =============================================================================

/// Preview entity scope resolution without committing (dry-run).
/// Returns matching entities without creating a snapshot.
#[register_custom_op]
pub struct ScopeResolveOp;

#[async_trait]
impl CustomOperation for ScopeResolveOp {
    fn domain(&self) -> &'static str {
        "scope"
    }

    fn verb(&self) -> &'static str {
        "resolve"
    }

    fn rationale(&self) -> &'static str {
        "Preview entity scope resolution without creating an immutable snapshot. \
         Useful for checking what entities would be selected before committing."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Require client_group_id
        let group_id = ctx.client_group_id.ok_or_else(|| {
            anyhow::anyhow!(
                "Set client group first (use 'work on <client>'). \
                 scope.resolve requires an active client context."
            )
        })?;

        // Extract arguments
        let desc = get_required_string(verb_call, "desc")?;
        let limit = get_optional_int(verb_call, "limit").unwrap_or(50).min(100);
        let semantic = get_optional_bool(verb_call, "semantic").unwrap_or(false);
        let persona = get_optional_string(verb_call, "persona").or_else(|| ctx.persona.clone());

        tracing::info!(
            group_id = %group_id,
            desc = %desc,
            limit = limit,
            semantic = semantic,
            "scope.resolve: previewing entity search"
        );

        // Call search (same as commit but don't persist)
        let (entity_ids, candidates_json, method) = if semantic {
            search_semantic(pool, group_id, &desc, persona.as_deref(), limit).await?
        } else {
            search_fuzzy(pool, group_id, &desc, persona.as_deref(), limit).await?
        };

        // Parse candidates back to get details
        let candidates: Vec<ScopeCandidate> =
            serde_json::from_value(candidates_json).unwrap_or_default();

        // Build preview result
        let preview: Vec<serde_json::Value> = candidates
            .iter()
            .map(|c| {
                json!({
                    "entity_id": c.entity_id,
                    "entity_name": c.entity_name,
                    "score": c.score,
                    "match_type": c.match_type
                })
            })
            .collect();

        tracing::info!(
            entity_count = entity_ids.len(),
            method = %method,
            "scope.resolve: preview complete"
        );

        Ok(ExecutionResult::RecordSet(preview))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "scope.resolve requires database feature to be enabled"
        ))
    }
}

// =============================================================================
// SCOPE.NARROW OPERATION (Filter existing scope)
// =============================================================================

/// Create new scope by filtering an existing scope with additional criteria.
#[register_custom_op]
pub struct ScopeNarrowOp;

#[async_trait]
impl CustomOperation for ScopeNarrowOp {
    fn domain(&self) -> &'static str {
        "scope"
    }

    fn verb(&self) -> &'static str {
        "narrow"
    }

    fn rationale(&self) -> &'static str {
        "Creates a new scope by filtering an existing scope. Enables progressive \
         refinement of entity sets without losing the original scope."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Require client_group_id
        let group_id = ctx.client_group_id.ok_or_else(|| {
            anyhow::anyhow!(
                "Set client group first. scope.narrow requires an active client context."
            )
        })?;

        // Get source scope
        let source_scope_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "source-scope")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing required argument :source-scope"))?;

        // Load source snapshot
        let source_row = sqlx::query(
            r#"SELECT group_id, selected_entity_ids FROM "ob-poc".scope_snapshots WHERE id = $1"#,
        )
        .bind(source_scope_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Source scope snapshot not found: {}", source_scope_id))?;

        // Cross-group protection
        let source_group_id: Uuid = source_row.get("group_id");
        if source_group_id != group_id {
            return Err(anyhow::anyhow!(
                "Source scope belongs to different client group. Cannot narrow across groups."
            ));
        }

        let source_entity_ids: Vec<Uuid> = source_row.get("selected_entity_ids");

        if source_entity_ids.is_empty() {
            return Err(anyhow::anyhow!("Source scope is empty. Nothing to narrow."));
        }

        // Get filter criteria
        let filter_desc = get_optional_string(verb_call, "filter-desc");
        let entity_type = get_optional_string(verb_call, "entity-type");
        let jurisdiction = get_optional_string(verb_call, "jurisdiction");
        let limit = get_optional_int(verb_call, "limit").unwrap_or(50).min(100);

        // Build filtered entity list
        let mut filtered_ids = source_entity_ids.clone();

        // Apply entity type filter
        if let Some(ref etype) = entity_type {
            let type_filtered = sqlx::query(
                r#"SELECT e.entity_id FROM "ob-poc".entities e
                   JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                   WHERE e.entity_id = ANY($1) AND et.type_code = $2"#,
            )
            .bind(&filtered_ids)
            .bind(etype)
            .fetch_all(pool)
            .await?;

            filtered_ids = type_filtered.iter().map(|r| r.get("entity_id")).collect();
        }

        // Apply jurisdiction filter
        if let Some(ref jur) = jurisdiction {
            let jur_filtered = sqlx::query(
                r#"SELECT entity_id FROM "ob-poc".entities
                   WHERE entity_id = ANY($1) AND jurisdiction = $2"#,
            )
            .bind(&filtered_ids)
            .bind(jur)
            .fetch_all(pool)
            .await?;

            filtered_ids = jur_filtered.iter().map(|r| r.get("entity_id")).collect();
        }

        // Apply text filter description (if provided)
        if let Some(ref desc) = filter_desc {
            let text_filtered = sqlx::query(
                r#"SELECT entity_id FROM "ob-poc".entities
                   WHERE entity_id = ANY($1)
                   AND (name ILIKE '%' || $2 || '%' OR search_name ILIKE '%' || $2 || '%')"#,
            )
            .bind(&filtered_ids)
            .bind(desc)
            .fetch_all(pool)
            .await?;

            filtered_ids = text_filtered.iter().map(|r| r.get("entity_id")).collect();
        }

        // Apply limit
        filtered_ids.truncate(limit as usize);

        if filtered_ids.is_empty() {
            return Err(anyhow::anyhow!(
                "No entities remain after applying filters. \
                 Original scope had {} entities.",
                source_entity_ids.len()
            ));
        }

        // Build descriptor
        let descriptor = json!({
            "source_scope_id": source_scope_id,
            "filter_desc": filter_desc,
            "entity_type": entity_type,
            "jurisdiction": jurisdiction,
            "limit": limit,
        });

        let description = format!(
            "Narrowed from {} ({} -> {} entities)",
            source_scope_id,
            source_entity_ids.len(),
            filtered_ids.len()
        );

        // Insert new snapshot
        let snapshot_row = sqlx::query(
            r#"
            INSERT INTO "ob-poc".scope_snapshots
                (group_id, description, filter_applied, limit_requested, mode,
                 selected_entity_ids, resolution_method, session_id)
            VALUES ($1, $2, $3, $4, 'strict', $5, 'narrowed', $6)
            RETURNING id
            "#,
        )
        .bind(group_id)
        .bind(&description)
        .bind(&descriptor)
        .bind(limit)
        .bind(&filtered_ids)
        .bind(ctx.session_id)
        .fetch_one(pool)
        .await?;

        let snapshot_id: Uuid = snapshot_row.get("id");

        tracing::info!(
            snapshot_id = %snapshot_id,
            source_count = source_entity_ids.len(),
            filtered_count = filtered_ids.len(),
            "scope.narrow: created narrowed snapshot"
        );

        let result = ScopeCommitResult {
            snapshot_id,
            entity_count: filtered_ids.len(),
            description,
            resolution_method: "narrowed".to_string(),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "scope.narrow requires database feature to be enabled"
        ))
    }
}

// =============================================================================
// SCOPE.UNION OPERATION (Combine multiple scopes)
// =============================================================================

/// Create new scope by combining multiple existing scopes.
#[register_custom_op]
pub struct ScopeUnionOp;

#[async_trait]
impl CustomOperation for ScopeUnionOp {
    fn domain(&self) -> &'static str {
        "scope"
    }

    fn verb(&self) -> &'static str {
        "union"
    }

    fn rationale(&self) -> &'static str {
        "Creates a new scope by combining entity sets from multiple existing scopes. \
         Useful for building comprehensive scopes from multiple search criteria."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Require client_group_id
        let group_id = ctx.client_group_id.ok_or_else(|| {
            anyhow::anyhow!(
                "Set client group first. scope.union requires an active client context."
            )
        })?;

        // Get list of scope IDs
        let scope_ids: Vec<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scopes")
            .and_then(|a| {
                a.value.as_list().map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            if let Some(name) = item.as_symbol() {
                                ctx.resolve(name)
                            } else {
                                item.as_uuid()
                            }
                        })
                        .collect()
                })
            })
            .ok_or_else(|| {
                anyhow::anyhow!("Missing required argument :scopes (list of scope IDs)")
            })?;

        if scope_ids.is_empty() {
            return Err(anyhow::anyhow!(":scopes list cannot be empty"));
        }

        if scope_ids.len() < 2 {
            return Err(anyhow::anyhow!(
                ":scopes requires at least 2 scopes to union. Use scope.commit for single searches."
            ));
        }

        let dedupe = get_optional_bool(verb_call, "dedupe").unwrap_or(true);
        let limit = get_optional_int(verb_call, "limit").unwrap_or(100).min(200);

        // Load all source snapshots and validate group
        let mut all_entity_ids: Vec<Uuid> = Vec::new();

        for scope_id in &scope_ids {
            let row = sqlx::query(
                r#"SELECT group_id, selected_entity_ids FROM "ob-poc".scope_snapshots WHERE id = $1"#,
            )
            .bind(scope_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Scope snapshot not found: {}", scope_id))?;

            // Cross-group protection
            let source_group_id: Uuid = row.get("group_id");
            if source_group_id != group_id {
                return Err(anyhow::anyhow!(
                    "Scope {} belongs to different client group. Cannot union across groups.",
                    scope_id
                ));
            }

            let entity_ids: Vec<Uuid> = row.get("selected_entity_ids");
            all_entity_ids.extend(entity_ids);
        }

        // Deduplicate if requested (preserves first occurrence order)
        let combined_ids: Vec<Uuid> = if dedupe {
            let mut seen = std::collections::HashSet::new();
            all_entity_ids
                .into_iter()
                .filter(|id| seen.insert(*id))
                .take(limit as usize)
                .collect()
        } else {
            all_entity_ids.into_iter().take(limit as usize).collect()
        };

        if combined_ids.is_empty() {
            return Err(anyhow::anyhow!(
                "All source scopes are empty. Nothing to union."
            ));
        }

        // Build descriptor
        let descriptor = json!({
            "source_scope_ids": scope_ids,
            "dedupe": dedupe,
            "limit": limit,
        });

        let description = format!(
            "Union of {} scopes ({} entities{})",
            scope_ids.len(),
            combined_ids.len(),
            if dedupe { ", deduplicated" } else { "" }
        );

        // Insert new snapshot
        let snapshot_row = sqlx::query(
            r#"
            INSERT INTO "ob-poc".scope_snapshots
                (group_id, description, filter_applied, limit_requested, mode,
                 selected_entity_ids, resolution_method, session_id)
            VALUES ($1, $2, $3, $4, 'strict', $5, 'union', $6)
            RETURNING id
            "#,
        )
        .bind(group_id)
        .bind(&description)
        .bind(&descriptor)
        .bind(limit)
        .bind(&combined_ids)
        .bind(ctx.session_id)
        .fetch_one(pool)
        .await?;

        let snapshot_id: Uuid = snapshot_row.get("id");

        tracing::info!(
            snapshot_id = %snapshot_id,
            source_count = scope_ids.len(),
            combined_count = combined_ids.len(),
            dedupe = dedupe,
            "scope.union: created combined snapshot"
        );

        let result = ScopeCommitResult {
            snapshot_id,
            entity_count: combined_ids.len(),
            description,
            resolution_method: "union".to_string(),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "scope.union requires database feature to be enabled"
        ))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_commit_op_metadata() {
        let op = ScopeCommitOp;
        assert_eq!(op.domain(), "scope");
        assert_eq!(op.verb(), "commit");
        assert!(op.rationale().contains("Pattern B"));
    }

    #[test]
    fn test_scope_resolve_op_metadata() {
        let op = ScopeResolveOp;
        assert_eq!(op.domain(), "scope");
        assert_eq!(op.verb(), "resolve");
        assert!(op.rationale().contains("Preview"));
    }

    #[test]
    fn test_scope_narrow_op_metadata() {
        let op = ScopeNarrowOp;
        assert_eq!(op.domain(), "scope");
        assert_eq!(op.verb(), "narrow");
        assert!(op.rationale().contains("filtering"));
    }

    #[test]
    fn test_scope_union_op_metadata() {
        let op = ScopeUnionOp;
        assert_eq!(op.domain(), "scope");
        assert_eq!(op.verb(), "union");
        assert!(op.rationale().contains("combining"));
    }
}
