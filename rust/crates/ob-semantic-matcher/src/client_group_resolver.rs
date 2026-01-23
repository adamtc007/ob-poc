//! Two-stage client group resolution:
//! Stage 1: Alias → ClientGroupId (semantic lookup)
//! Stage 2: ClientGroupId → AnchorEntityId (policy-based)
//!
//! This module provides the traits and implementation for resolving
//! client nicknames (like "Allianz", "AGI") to real anchor entities
//! based on verb-specific policies.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Types
// ============================================================================

/// Anchor roles determine which real entity a client group resolves to
/// based on the verb being executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorRole {
    /// UBO top-level parent - used for ownership discovery
    UltimateParent,
    /// Operational/board control entity (ManCo equivalent)
    GovernanceController,
    /// Regional book controller
    BookController,
    /// Day-to-day operations controller
    OperatingController,
    /// Primary regulated entity for compliance
    RegulatoryAnchor,
}

impl AnchorRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UltimateParent => "ultimate_parent",
            Self::GovernanceController => "governance_controller",
            Self::BookController => "book_controller",
            Self::OperatingController => "operating_controller",
            Self::RegulatoryAnchor => "regulatory_anchor",
        }
    }

    /// Get the default anchor role for a verb domain
    pub fn default_for_domain(domain: &str) -> Self {
        match domain {
            "ubo" | "ownership" => Self::UltimateParent,
            "session" | "cbu" | "view" => Self::GovernanceController,
            "kyc" | "screening" | "regulatory" => Self::RegulatoryAnchor,
            "contract" | "service" => Self::OperatingController,
            _ => Self::GovernanceController, // safe default
        }
    }
}

impl FromStr for AnchorRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ultimate_parent" => Ok(Self::UltimateParent),
            "governance_controller" => Ok(Self::GovernanceController),
            "book_controller" => Ok(Self::BookController),
            "operating_controller" => Ok(Self::OperatingController),
            "regulatory_anchor" => Ok(Self::RegulatoryAnchor),
            _ => Err(format!("Unknown anchor role: {}", s)),
        }
    }
}

/// A virtual client group entity (not a legal entity)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClientGroup {
    pub id: Uuid,
    pub canonical_name: String,
    pub short_code: Option<String>,
    pub description: Option<String>,
}

/// An alias for a client group (fuzzy matching target)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClientGroupAlias {
    pub id: Uuid,
    pub group_id: Uuid,
    pub alias: String,
    pub alias_norm: String,
    pub source: String,
    pub confidence: f32,
    pub is_primary: bool,
}

/// Mapping from client group to a real anchor entity
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClientGroupAnchor {
    pub id: Uuid,
    pub group_id: Uuid,
    pub anchor_entity_id: Uuid,
    pub anchor_role: String,
    pub jurisdiction: Option<String>,
    pub confidence: f32,
    pub priority: i32,
    pub valid_from: Option<chrono::NaiveDate>,
    pub valid_to: Option<chrono::NaiveDate>,
}

/// Result of Stage 1 resolution (alias → group)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGroupMatch {
    pub group_id: Uuid,
    pub canonical_name: String,
    pub matched_alias: String,
    pub similarity_score: f32,
}

/// Result of Stage 2 resolution (group → anchor entity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorResolution {
    pub group_id: Uuid,
    pub anchor_entity_id: Uuid,
    pub anchor_role: AnchorRole,
    pub jurisdiction: Option<String>,
    pub confidence: f32,
}

/// Complete resolution result (both stages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGroupResolution {
    pub stage1: ClientGroupMatch,
    pub stage2: AnchorResolution,
}

/// Configuration for client group lookup in verb args (from YAML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGroupLookupConfig {
    /// The anchor role to use for this verb
    pub anchor_role: AnchorRole,
    /// Whether to preserve the group_id (late binding) or resolve to anchor (early binding)
    #[serde(default)]
    pub preserve_group: bool,
    /// Optional jurisdiction filter
    pub jurisdiction: Option<String>,
}

impl Default for ClientGroupLookupConfig {
    fn default() -> Self {
        Self {
            anchor_role: AnchorRole::GovernanceController,
            preserve_group: false,
            jurisdiction: None,
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Errors during client group resolution
#[derive(Debug, thiserror::Error)]
pub enum ClientGroupResolveError {
    #[error("No matching client group found for '{0}'")]
    NoMatch(String),

    #[error("Ambiguous client group match for '{input}': {candidates:?}")]
    Ambiguous {
        input: String,
        candidates: Vec<ClientGroupMatch>,
    },

    #[error("No anchor found for group {group_id} with role {role:?}")]
    NoAnchor { group_id: Uuid, role: AnchorRole },

    #[error("Multiple anchors for group {group_id} with role {role:?} - disambiguation required")]
    MultipleAnchors {
        group_id: Uuid,
        role: AnchorRole,
        anchors: Vec<AnchorResolution>,
    },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Embedding service error: {0}")]
    Embedding(String),
}

impl ClientGroupResolveError {
    /// Check if this error indicates ambiguity (needs user input)
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous { .. } | Self::MultipleAnchors { .. })
    }

    /// Get disambiguation candidates if applicable
    pub fn candidates(&self) -> Option<&[ClientGroupMatch]> {
        match self {
            Self::Ambiguous { candidates, .. } => Some(candidates),
            _ => None,
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for resolution behavior
#[derive(Debug, Clone)]
pub struct ResolutionConfig {
    /// Minimum similarity score to consider a match (0.0-1.0)
    pub min_similarity: f32,
    /// Margin required between top match and second match for auto-resolution
    pub ambiguity_margin: f32,
    /// Maximum candidates to return on ambiguous match
    pub max_candidates: usize,
}

impl Default for ResolutionConfig {
    fn default() -> Self {
        Self {
            min_similarity: 0.75,
            ambiguity_margin: 0.10,
            max_candidates: 5,
        }
    }
}

impl ResolutionConfig {
    /// Stricter config for pre-pass extraction (before LLM)
    pub fn strict() -> Self {
        Self {
            min_similarity: 0.85,
            ambiguity_margin: 0.15,
            max_candidates: 3,
        }
    }

    /// Lenient config for interactive disambiguation
    pub fn lenient() -> Self {
        Self {
            min_similarity: 0.60,
            ambiguity_margin: 0.05,
            max_candidates: 10,
        }
    }
}

// ============================================================================
// Traits
// ============================================================================

/// Stage 1: Resolve alias text to client group
#[async_trait]
pub trait ClientGroupAliasResolver: Send + Sync {
    /// Resolve an alias string to a client group.
    /// Returns Ok(match) if confident, Err(Ambiguous) if multiple candidates.
    async fn resolve_alias(
        &self,
        alias: &str,
        config: &ResolutionConfig,
    ) -> Result<ClientGroupMatch, ClientGroupResolveError>;

    /// Get top-k candidates without confidence filtering (for disambiguation UI)
    async fn search_aliases(
        &self,
        alias: &str,
        limit: usize,
    ) -> Result<Vec<ClientGroupMatch>, ClientGroupResolveError>;

    /// Check if exact alias exists (fast path, no embedding)
    async fn exact_match(
        &self,
        alias_norm: &str,
    ) -> Result<Option<ClientGroupMatch>, ClientGroupResolveError>;
}

/// Stage 2: Resolve client group to anchor entity based on role policy
#[async_trait]
pub trait ClientGroupAnchorResolver: Send + Sync {
    /// Resolve a client group to its anchor entity for the given role.
    /// Applies jurisdiction filtering and temporal validity.
    async fn resolve_anchor(
        &self,
        group_id: Uuid,
        role: AnchorRole,
        jurisdiction: Option<&str>,
    ) -> Result<AnchorResolution, ClientGroupResolveError>;

    /// Get all anchors for a group (for introspection/admin)
    async fn list_anchors(
        &self,
        group_id: Uuid,
    ) -> Result<Vec<AnchorResolution>, ClientGroupResolveError>;
}

/// Combined two-stage resolver (convenience trait)
#[async_trait]
pub trait ClientGroupResolver: ClientGroupAliasResolver + ClientGroupAnchorResolver {
    /// Full resolution: alias → group → anchor entity
    async fn resolve_full(
        &self,
        alias: &str,
        role: AnchorRole,
        jurisdiction: Option<&str>,
        config: &ResolutionConfig,
    ) -> Result<ClientGroupResolution, ClientGroupResolveError> {
        let stage1 = self.resolve_alias(alias, config).await?;
        let stage2 = self
            .resolve_anchor(stage1.group_id, role, jurisdiction)
            .await?;
        Ok(ClientGroupResolution { stage1, stage2 })
    }
}

// Blanket implementation: any type implementing both traits gets ClientGroupResolver
impl<T: ClientGroupAliasResolver + ClientGroupAnchorResolver> ClientGroupResolver for T {}

// ============================================================================
// Embedder Trait (dependency)
// ============================================================================

/// Minimal embedder trait for client group resolution
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Embed text as a query (with BGE query prefix if applicable)
    /// Output MUST be L2-normalized for correct cosine distance
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String>;

    /// Embed text as a target (no prefix, for corpus building)
    /// Output MUST be L2-normalized for correct cosine distance
    async fn embed_target(&self, text: &str) -> Result<Vec<f32>, String>;

    /// Batch embed multiple texts as targets (more efficient for bulk operations)
    /// Output MUST be L2-normalized for correct cosine distance
    async fn embed_batch_targets(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        // Default implementation: embed one by one
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_target(text).await?);
        }
        Ok(results)
    }
}

// ============================================================================
// PostgreSQL Implementation
// ============================================================================

/// PostgreSQL + pgvector implementation of the two-stage resolver
pub struct PgClientGroupResolver<E: Embedder> {
    pool: PgPool,
    embedder: Arc<E>,
    embedder_id: String, // e.g., "bge-small-en-v1.5"
}

impl<E: Embedder> PgClientGroupResolver<E> {
    pub fn new(pool: PgPool, embedder: Arc<E>, embedder_id: String) -> Self {
        Self {
            pool,
            embedder,
            embedder_id,
        }
    }
}

#[async_trait]
impl<E: Embedder + 'static> ClientGroupAliasResolver for PgClientGroupResolver<E> {
    async fn exact_match(
        &self,
        alias_norm: &str,
    ) -> Result<Option<ClientGroupMatch>, ClientGroupResolveError> {
        let row = sqlx::query_as::<_, (Uuid, String, String)>(
            r#"
            SELECT cg.id, cg.canonical_name, cga.alias
            FROM "ob-poc".client_group_alias cga
            JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
            WHERE cga.alias_norm = $1
            LIMIT 1
            "#,
        )
        .bind(alias_norm)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(group_id, canonical_name, matched_alias)| ClientGroupMatch {
                group_id,
                canonical_name,
                matched_alias,
                similarity_score: 1.0,
            },
        ))
    }

    async fn search_aliases(
        &self,
        alias: &str,
        limit: usize,
    ) -> Result<Vec<ClientGroupMatch>, ClientGroupResolveError> {
        // Embed the query with BGE query prefix (must be L2-normalized)
        let embedding = self
            .embedder
            .embed_query(alias)
            .await
            .map_err(ClientGroupResolveError::Embedding)?;

        // Search with embedder_id filter for versioned embeddings
        let rows = sqlx::query_as::<_, (Uuid, String, String, f32)>(
            r#"
            SELECT
                cg.id,
                cg.canonical_name,
                cga.alias,
                (1 - (cgae.embedding <=> $1::vector))::real as similarity
            FROM "ob-poc".client_group_alias_embedding cgae
            JOIN "ob-poc".client_group_alias cga ON cga.id = cgae.alias_id
            JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
            WHERE cgae.embedder_id = $3
            ORDER BY cgae.embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(&embedding)
        .bind(limit as i32)
        .bind(&self.embedder_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(group_id, canonical_name, matched_alias, similarity_score)| ClientGroupMatch {
                    group_id,
                    canonical_name,
                    matched_alias,
                    similarity_score,
                },
            )
            .collect())
    }

    async fn resolve_alias(
        &self,
        alias: &str,
        config: &ResolutionConfig,
    ) -> Result<ClientGroupMatch, ClientGroupResolveError> {
        let alias_norm = alias.to_lowercase().trim().to_string();

        // Fast path: exact match (no embedding needed)
        if let Some(m) = self.exact_match(&alias_norm).await? {
            return Ok(m);
        }

        // Semantic search via embeddings
        let candidates = self.search_aliases(alias, config.max_candidates).await?;

        if candidates.is_empty() {
            return Err(ClientGroupResolveError::NoMatch(alias.to_string()));
        }

        let top = &candidates[0];

        // Check minimum similarity threshold
        if top.similarity_score < config.min_similarity {
            return Err(ClientGroupResolveError::NoMatch(alias.to_string()));
        }

        // Check ambiguity margin (is top result clearly better than second?)
        if candidates.len() > 1 {
            let margin = top.similarity_score - candidates[1].similarity_score;
            if margin < config.ambiguity_margin {
                return Err(ClientGroupResolveError::Ambiguous {
                    input: alias.to_string(),
                    candidates,
                });
            }
        }

        Ok(top.clone())
    }
}

#[async_trait]
impl<E: Embedder + 'static> ClientGroupAnchorResolver for PgClientGroupResolver<E> {
    async fn resolve_anchor(
        &self,
        group_id: Uuid,
        role: AnchorRole,
        jurisdiction: Option<&str>,
    ) -> Result<AnchorResolution, ClientGroupResolveError> {
        // Jurisdiction uses empty string for "global" (no jurisdiction filter)
        let jurisdiction_param = jurisdiction.unwrap_or("");

        // Deterministic ordering:
        // 1. Exact jurisdiction match first
        // 2. Then global (empty string) as fallback
        // 3. Then by priority DESC
        // 4. Then by confidence DESC
        // 5. Then by anchor_entity_id for stable tie-breaking
        let row = sqlx::query_as::<_, (Uuid, String, f32)>(
            r#"
            SELECT anchor_entity_id, jurisdiction, confidence::real
            FROM "ob-poc".client_group_anchor
            WHERE group_id = $1
              AND anchor_role = $2
              AND (valid_from IS NULL OR valid_from <= CURRENT_DATE)
              AND (valid_to IS NULL OR valid_to >= CURRENT_DATE)
              AND (
                  jurisdiction = $3              -- exact match
                  OR jurisdiction = ''           -- global fallback
              )
            ORDER BY
                CASE WHEN jurisdiction = $3 AND $3 != '' THEN 0  -- exact jurisdiction first
                     WHEN jurisdiction = '' THEN 1               -- global fallback second
                     ELSE 2 END,
                priority DESC,
                confidence DESC,
                anchor_entity_id                                 -- stable tie-breaker
            LIMIT 1
            "#,
        )
        .bind(group_id)
        .bind(role.as_str())
        .bind(jurisdiction_param)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((anchor_entity_id, anchor_jurisdiction, confidence)) => Ok(AnchorResolution {
                group_id,
                anchor_entity_id,
                anchor_role: role,
                jurisdiction: if anchor_jurisdiction.is_empty() {
                    None
                } else {
                    Some(anchor_jurisdiction)
                },
                confidence,
            }),
            None => Err(ClientGroupResolveError::NoAnchor { group_id, role }),
        }
    }

    async fn list_anchors(
        &self,
        group_id: Uuid,
    ) -> Result<Vec<AnchorResolution>, ClientGroupResolveError> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, f32)>(
            r#"
            SELECT anchor_entity_id, anchor_role, jurisdiction, confidence::real
            FROM "ob-poc".client_group_anchor
            WHERE group_id = $1
              AND (valid_from IS NULL OR valid_from <= CURRENT_DATE)
              AND (valid_to IS NULL OR valid_to >= CURRENT_DATE)
            ORDER BY anchor_role, priority DESC
            "#,
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(anchor_entity_id, role_str, jurisdiction, confidence)| {
                AnchorRole::from_str(&role_str)
                    .ok()
                    .map(|anchor_role| AnchorResolution {
                        group_id,
                        anchor_entity_id,
                        anchor_role,
                        jurisdiction: if jurisdiction.is_empty() {
                            None
                        } else {
                            Some(jurisdiction)
                        },
                        confidence,
                    })
            })
            .collect())
    }
}

// ============================================================================
// Chat Disambiguation Response
// ============================================================================

/// Response format for chat when disambiguation is needed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationResponse {
    pub message: String,
    pub candidates: Vec<DisambiguationCandidate>,
    pub original_input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationCandidate {
    pub index: usize,
    pub group_id: Uuid,
    pub display_name: String,
    pub matched_alias: String,
    pub similarity_score: f32,
}

impl DisambiguationResponse {
    /// Create from an Ambiguous error for chat display
    pub fn from_ambiguous_error(err: &ClientGroupResolveError) -> Option<Self> {
        match err {
            ClientGroupResolveError::Ambiguous { input, candidates } => {
                let candidates: Vec<_> = candidates
                    .iter()
                    .enumerate()
                    .map(|(i, c)| DisambiguationCandidate {
                        index: i + 1,
                        group_id: c.group_id,
                        display_name: c.canonical_name.clone(),
                        matched_alias: c.matched_alias.clone(),
                        similarity_score: c.similarity_score,
                    })
                    .collect();

                let options: Vec<_> = candidates
                    .iter()
                    .map(|c| {
                        format!(
                            "{}. {} (matched '{}')",
                            c.index, c.display_name, c.matched_alias
                        )
                    })
                    .collect();

                Some(Self {
                    message: format!(
                        "I found multiple matches for '{}'. Which did you mean?\n{}",
                        input,
                        options.join("\n")
                    ),
                    candidates,
                    original_input: input.clone(),
                })
            }
            _ => None,
        }
    }

    /// Resolve user's selection (by index or name)
    pub fn resolve_selection(&self, selection: &str) -> Option<Uuid> {
        // Try numeric index first
        if let Ok(idx) = selection.trim().parse::<usize>() {
            return self.candidates.get(idx - 1).map(|c| c.group_id);
        }

        // Try matching by name (case-insensitive)
        let selection_lower = selection.to_lowercase();
        self.candidates
            .iter()
            .find(|c| {
                c.display_name.to_lowercase().contains(&selection_lower)
                    || c.matched_alias.to_lowercase().contains(&selection_lower)
            })
            .map(|c| c.group_id)
    }
}

// ============================================================================
// Pre-Pass Extraction (Optional, runs before LLM)
// ============================================================================

/// Pre-pass: Extract client alias from user text before LLM arg extraction.
/// This makes client identification LLM-independent.
pub async fn pre_extract_client_alias<R: ClientGroupAliasResolver>(
    user_text: &str,
    resolver: &R,
) -> Option<ClientGroupMatch> {
    let config = ResolutionConfig::strict();

    // Try resolution - if confident, return match; otherwise None
    resolver.resolve_alias(user_text, &config).await.ok()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_role_roundtrip() {
        for role in [
            AnchorRole::UltimateParent,
            AnchorRole::GovernanceController,
            AnchorRole::BookController,
            AnchorRole::OperatingController,
            AnchorRole::RegulatoryAnchor,
        ] {
            let s = role.as_str();
            let parsed = AnchorRole::from_str(s).expect("should parse");
            assert_eq!(role, parsed);
        }
    }

    #[test]
    fn test_default_for_domain() {
        assert_eq!(
            AnchorRole::default_for_domain("ubo"),
            AnchorRole::UltimateParent
        );
        assert_eq!(
            AnchorRole::default_for_domain("session"),
            AnchorRole::GovernanceController
        );
        assert_eq!(
            AnchorRole::default_for_domain("kyc"),
            AnchorRole::RegulatoryAnchor
        );
    }

    #[test]
    fn test_error_is_ambiguous() {
        let err = ClientGroupResolveError::NoMatch("test".into());
        assert!(!err.is_ambiguous());

        let err = ClientGroupResolveError::Ambiguous {
            input: "test".into(),
            candidates: vec![],
        };
        assert!(err.is_ambiguous());
    }
}
