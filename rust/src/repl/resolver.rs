//! Entity Argument Resolver
//!
//! Resolves entity shorthand references to UUIDs using client_group-scoped search.
//! This is where hallucination dies - all entity IDs come from DB searches.
//!
//! # Resolution Flow
//!
//! ```text
//! "Irish funds"
//!      │
//!      ▼
//! ┌─────────────────────────────────────┐
//! │  1. Is it already a UUID?           │
//! │     Yes → DirectUuid                │
//! │     No → continue                   │
//! └─────────────────────────────────────┘
//!      │
//!      ▼
//! ┌─────────────────────────────────────┐
//! │  2. Is it an output ref ($N)?       │
//! │     Yes → OutputRef (resolved later)│
//! │     No → continue                   │
//! └─────────────────────────────────────┘
//!      │
//!      ▼
//! ┌─────────────────────────────────────┐
//! │  3. Search entity tags              │
//! │     - Exact match (confidence=1.0)  │
//! │     - Fuzzy trigram                 │
//! │     - Semantic embedding            │
//! └─────────────────────────────────────┘
//!      │
//!      ▼
//! ┌─────────────────────────────────────┐
//! │  4. Evaluate results                │
//! │     - 0 matches → Failed            │
//! │     - 1 match → Resolved            │
//! │     - Multiple high conf → Resolved │
//! │     - Multiple low conf → Ambiguous │
//! └─────────────────────────────────────┘
//! ```
//!
//! # Anti-Hallucination Guarantees
//!
//! - **No invented UUIDs**: All UUIDs come from `search_entity_tags` or direct UUID parse
//! - **Ambiguous → Picker required**: Multiple low-confidence matches require user selection
//! - **Candidate set stored**: Picker must select from stored candidates only

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Confidence threshold for "high confidence" matches
/// High confidence threshold for entity resolution (f64 to match PostgreSQL FLOAT)
pub const HIGH_CONFIDENCE_THRESHOLD: f64 = 0.7;

/// Minimum confidence to include in results
/// Minimum confidence to include in results (f64 to match PostgreSQL FLOAT)
pub const MIN_CONFIDENCE_THRESHOLD: f64 = 0.3;

/// Match type from entity tag search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    /// Exact tag match
    Exact,
    /// Trigram fuzzy match
    Fuzzy,
    /// Semantic embedding match
    Semantic,
}

impl MatchType {
    pub fn from_db(s: &str) -> Self {
        match s {
            "exact" => Self::Exact,
            "fuzzy" => Self::Fuzzy,
            "semantic" => Self::Semantic,
            _ => Self::Fuzzy,
        }
    }

    pub fn to_db(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Fuzzy => "fuzzy",
            Self::Semantic => "semantic",
        }
    }
}

/// A matched entity from search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub matched_tag: Option<String>,
    /// Confidence score (f64 to match PostgreSQL FLOAT)
    pub confidence: f64,
    pub match_type: MatchType,
}

/// Result of resolving an entity argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionResult {
    /// Resolution status
    pub status: ResolutionOutcome,
    /// Resolved entities (if status is Resolved)
    pub entities: Vec<EntityMatch>,
    /// Error message (if status is Failed)
    pub error: Option<String>,
    /// Search method used
    pub search_method: SearchMethod,
}

/// Resolution outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionOutcome {
    /// All refs resolved to UUIDs
    Resolved,
    /// Needs picker - multiple candidates
    Ambiguous,
    /// No matches found
    Failed,
    /// Output reference ($N) - resolved at execution time
    Deferred,
}

/// Search method used
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMethod {
    /// Direct UUID parse
    DirectUuid,
    /// Output reference
    OutputRef,
    /// Exact tag match only
    ExactTag,
    /// Fuzzy trigram
    FuzzyTag,
    /// Semantic embedding
    SemanticEmbedding,
    /// Combined (tried multiple)
    Combined,
}

/// Entity argument resolver
///
/// Uses client_group-scoped search functions from migration 052.
/// All entity IDs come from database - never invented.
pub struct EntityArgResolver {
    client_group_id: Uuid,
    persona: Option<String>,
    /// Whether to include historical entities
    include_historical: bool,
    /// Maximum results per search
    max_results: i32,
}

impl EntityArgResolver {
    /// Create a new resolver for a client group
    pub fn new(client_group_id: Uuid) -> Self {
        Self {
            client_group_id,
            persona: None,
            include_historical: false,
            max_results: 10,
        }
    }

    /// Set persona for tag filtering
    pub fn with_persona(mut self, persona: impl Into<String>) -> Self {
        self.persona = Some(persona.into());
        self
    }

    /// Include historical entities in search
    pub fn with_historical(mut self, include: bool) -> Self {
        self.include_historical = include;
        self
    }

    /// Set maximum results
    pub fn with_max_results(mut self, max: i32) -> Self {
        self.max_results = max;
        self
    }

    /// Get client group ID
    pub fn client_group_id(&self) -> Uuid {
        self.client_group_id
    }

    /// Get persona
    pub fn persona(&self) -> Option<&str> {
        self.persona.as_deref()
    }

    /// Check if a string is a valid UUID
    pub fn is_uuid(s: &str) -> bool {
        Uuid::parse_str(s).is_ok()
    }

    /// Check if a string is an output reference ($N or $N.field)
    pub fn is_output_ref(s: &str) -> bool {
        if !s.starts_with('$') {
            return false;
        }
        let rest = &s[1..];
        // $N or $N.something
        let num_part = if let Some(dot_pos) = rest.find('.') {
            &rest[..dot_pos]
        } else {
            rest
        };
        num_part.parse::<i32>().is_ok()
    }

    /// Parse a UUID from string
    pub fn parse_uuid(s: &str) -> Option<Uuid> {
        Uuid::parse_str(s).ok()
    }

    /// Parse output reference number ($1 → 1, $2.result → 2)
    pub fn parse_output_ref(s: &str) -> Option<i32> {
        if !s.starts_with('$') {
            return None;
        }
        let rest = &s[1..];
        let num_part = if let Some(dot_pos) = rest.find('.') {
            &rest[..dot_pos]
        } else {
            rest
        };
        num_part.parse().ok()
    }

    /// Resolve a raw argument value
    ///
    /// This is the main entry point. Returns:
    /// - `Resolved` with entities if unambiguous
    /// - `Ambiguous` with candidates if picker needed
    /// - `Failed` if no matches
    /// - `Deferred` if output reference
    pub fn resolve_sync(&self, raw_value: &str) -> ResolutionResult {
        let trimmed = raw_value.trim();

        // 1. Check if already UUID
        if let Some(uuid) = Self::parse_uuid(trimmed) {
            return ResolutionResult {
                status: ResolutionOutcome::Resolved,
                entities: vec![EntityMatch {
                    entity_id: uuid,
                    entity_name: String::new(), // Will be fetched later
                    matched_tag: None,
                    confidence: 1.0,
                    match_type: MatchType::Exact,
                }],
                error: None,
                search_method: SearchMethod::DirectUuid,
            };
        }

        // 2. Check if output reference ($N)
        if Self::is_output_ref(trimmed) {
            return ResolutionResult {
                status: ResolutionOutcome::Deferred,
                entities: vec![],
                error: None,
                search_method: SearchMethod::OutputRef,
            };
        }

        // 3. This would normally call DB search functions
        // For now, return a result that indicates DB search is needed
        // The actual DB call happens in the async version below
        ResolutionResult {
            status: ResolutionOutcome::Failed,
            entities: vec![],
            error: Some(
                "Synchronous resolution requires database access. Use resolve_async instead."
                    .to_string(),
            ),
            search_method: SearchMethod::Combined,
        }
    }

    /// Evaluate search results and determine resolution outcome
    pub fn evaluate_matches(&self, matches: Vec<EntityMatch>) -> ResolutionResult {
        match matches.len() {
            0 => ResolutionResult {
                status: ResolutionOutcome::Failed,
                entities: vec![],
                error: Some("No entities found".to_string()),
                search_method: SearchMethod::Combined,
            },
            1 => ResolutionResult {
                status: ResolutionOutcome::Resolved,
                entities: matches,
                error: None,
                search_method: SearchMethod::Combined,
            },
            _ => {
                // Multiple matches - check confidence spread
                let high_conf: Vec<_> = matches
                    .iter()
                    .filter(|m| m.confidence >= HIGH_CONFIDENCE_THRESHOLD)
                    .cloned()
                    .collect();

                if !high_conf.is_empty() {
                    // All high-confidence matches are valid
                    ResolutionResult {
                        status: ResolutionOutcome::Resolved,
                        entities: high_conf,
                        error: None,
                        search_method: SearchMethod::Combined,
                    }
                } else {
                    // All low confidence - need picker
                    ResolutionResult {
                        status: ResolutionOutcome::Ambiguous,
                        entities: matches,
                        error: Some("Multiple low-confidence matches - please select".to_string()),
                        search_method: SearchMethod::Combined,
                    }
                }
            }
        }
    }
}

/// Async resolver with database access
#[cfg(feature = "database")]
pub mod db {
    use super::*;
    use crate::agent::learning::SharedEmbedder;
    use anyhow::Result;
    use sqlx::PgPool;

    /// Async entity argument resolver with database access
    pub struct AsyncEntityArgResolver<'a> {
        pool: &'a PgPool,
        resolver: EntityArgResolver,
        #[allow(dead_code)]
        embedder: Option<&'a SharedEmbedder>,
    }

    impl<'a> AsyncEntityArgResolver<'a> {
        /// Create a new async resolver
        pub fn new(pool: &'a PgPool, client_group_id: Uuid) -> Self {
            Self {
                pool,
                resolver: EntityArgResolver::new(client_group_id),
                embedder: None,
            }
        }

        /// Set persona
        pub fn with_persona(mut self, persona: impl Into<String>) -> Self {
            self.resolver = self.resolver.with_persona(persona);
            self
        }

        /// Set embedder for semantic search
        pub fn with_embedder(mut self, embedder: &'a SharedEmbedder) -> Self {
            self.embedder = Some(embedder);
            self
        }

        /// Resolve a raw argument value using database
        pub async fn resolve(&self, raw_value: &str) -> Result<ResolutionResult> {
            let trimmed = raw_value.trim();

            // 1. Check if already UUID
            if let Some(uuid) = EntityArgResolver::parse_uuid(trimmed) {
                // Fetch entity name
                let name = self.fetch_entity_name(uuid).await.unwrap_or_default();
                return Ok(ResolutionResult {
                    status: ResolutionOutcome::Resolved,
                    entities: vec![EntityMatch {
                        entity_id: uuid,
                        entity_name: name,
                        matched_tag: None,
                        confidence: 1.0,
                        match_type: MatchType::Exact,
                    }],
                    error: None,
                    search_method: SearchMethod::DirectUuid,
                });
            }

            // 2. Check if output reference
            if EntityArgResolver::is_output_ref(trimmed) {
                return Ok(ResolutionResult {
                    status: ResolutionOutcome::Deferred,
                    entities: vec![],
                    error: None,
                    search_method: SearchMethod::OutputRef,
                });
            }

            // 3. Search entity tags (exact + fuzzy)
            let matches = self.search_entity_tags(trimmed).await?;

            if !matches.is_empty() {
                return Ok(self.resolver.evaluate_matches(matches));
            }

            // 4. Try semantic search if no text matches and embedder available
            if self.embedder.is_some() {
                let semantic_matches = self.search_semantic(trimmed).await?;
                if !semantic_matches.is_empty() {
                    return Ok(self.resolver.evaluate_matches(semantic_matches));
                }
            }

            // 5. No matches found
            Ok(ResolutionResult {
                status: ResolutionOutcome::Failed,
                entities: vec![],
                error: Some(format!("No entities found for '{}'", raw_value)),
                search_method: SearchMethod::Combined,
            })
        }

        /// Search entity tags using migration 052 function
        async fn search_entity_tags(&self, query: &str) -> Result<Vec<EntityMatch>> {
            let rows = sqlx::query!(
                r#"
                SELECT
                    entity_id,
                    entity_name,
                    tag,
                    confidence as "confidence!: f64",
                    match_type
                FROM "ob-poc".search_entity_tags($1, $2, $3, $4, $5)
                "#,
                self.resolver.client_group_id,
                query,
                self.resolver.persona.as_deref(),
                self.resolver.max_results,
                self.resolver.include_historical
            )
            .fetch_all(self.pool)
            .await?;

            Ok(rows
                .into_iter()
                .filter_map(|r| {
                    Some(EntityMatch {
                        entity_id: r.entity_id?,
                        entity_name: r.entity_name?,
                        matched_tag: r.tag,
                        confidence: r.confidence,
                        match_type: MatchType::from_db(r.match_type.as_deref().unwrap_or("fuzzy")),
                    })
                })
                .collect())
        }

        /// Search using semantic embeddings
        async fn search_semantic(&self, _query: &str) -> Result<Vec<EntityMatch>> {
            // TODO: Implement semantic search with Candle embeddings
            // This requires embedding the query and calling search_entity_tags_semantic
            Ok(vec![])
        }

        /// Fetch entity name by ID
        async fn fetch_entity_name(&self, entity_id: Uuid) -> Result<String> {
            let row = sqlx::query_scalar!(
                r#"SELECT name::TEXT as "name!" FROM "ob-poc".entities WHERE entity_id = $1"#,
                entity_id
            )
            .fetch_optional(self.pool)
            .await?;

            Ok(row.unwrap_or_default())
        }

        /// Validate picker selection against stored candidates
        ///
        /// CRITICAL: This prevents the agent from fabricating entity IDs.
        /// Only entity_ids that were returned in a ResolutionAmbiguous event
        /// can be selected via runbook_pick.
        pub async fn validate_picker_selection(
            &self,
            command_id: Uuid,
            entity_ids: &[Uuid],
        ) -> Result<PickerValidation> {
            let result = sqlx::query!(
                r#"
                SELECT is_valid, invalid_entity_id, error_message
                FROM "ob-poc".validate_picker_selection($1, $2)
                "#,
                command_id,
                entity_ids
            )
            .fetch_one(self.pool)
            .await?;

            Ok(PickerValidation {
                is_valid: result.is_valid.unwrap_or(false),
                invalid_entity_id: result.invalid_entity_id,
                error_message: result.error_message,
            })
        }
    }

    /// Result of picker validation
    #[derive(Debug, Clone)]
    pub struct PickerValidation {
        pub is_valid: bool,
        pub invalid_entity_id: Option<Uuid>,
        pub error_message: Option<String>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_uuid() {
        assert!(EntityArgResolver::is_uuid(
            "550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(!EntityArgResolver::is_uuid("not-a-uuid"));
        assert!(!EntityArgResolver::is_uuid("Irish funds"));
    }

    #[test]
    fn test_is_output_ref() {
        assert!(EntityArgResolver::is_output_ref("$1"));
        assert!(EntityArgResolver::is_output_ref("$2"));
        assert!(EntityArgResolver::is_output_ref("$1.result"));
        assert!(EntityArgResolver::is_output_ref("$3.entity_ids"));
        assert!(!EntityArgResolver::is_output_ref("$abc"));
        assert!(!EntityArgResolver::is_output_ref("Irish funds"));
        assert!(!EntityArgResolver::is_output_ref("$"));
    }

    #[test]
    fn test_parse_output_ref() {
        assert_eq!(EntityArgResolver::parse_output_ref("$1"), Some(1));
        assert_eq!(EntityArgResolver::parse_output_ref("$2.result"), Some(2));
        assert_eq!(EntityArgResolver::parse_output_ref("$10"), Some(10));
        assert_eq!(EntityArgResolver::parse_output_ref("Irish"), None);
    }

    #[test]
    fn test_evaluate_single_match() {
        let resolver = EntityArgResolver::new(Uuid::now_v7());
        let matches = vec![EntityMatch {
            entity_id: Uuid::now_v7(),
            entity_name: "Test Entity".to_string(),
            matched_tag: Some("test".to_string()),
            confidence: 0.9,
            match_type: MatchType::Exact,
        }];

        let result = resolver.evaluate_matches(matches);
        assert_eq!(result.status, ResolutionOutcome::Resolved);
        assert_eq!(result.entities.len(), 1);
    }

    #[test]
    fn test_evaluate_high_confidence_matches() {
        let resolver = EntityArgResolver::new(Uuid::now_v7());
        let matches = vec![
            EntityMatch {
                entity_id: Uuid::now_v7(),
                entity_name: "Entity A".to_string(),
                matched_tag: Some("tag a".to_string()),
                confidence: 0.9,
                match_type: MatchType::Exact,
            },
            EntityMatch {
                entity_id: Uuid::now_v7(),
                entity_name: "Entity B".to_string(),
                matched_tag: Some("tag b".to_string()),
                confidence: 0.8,
                match_type: MatchType::Fuzzy,
            },
        ];

        let result = resolver.evaluate_matches(matches);
        assert_eq!(result.status, ResolutionOutcome::Resolved);
        assert_eq!(result.entities.len(), 2);
    }

    #[test]
    fn test_evaluate_low_confidence_matches_ambiguous() {
        let resolver = EntityArgResolver::new(Uuid::now_v7());
        let matches = vec![
            EntityMatch {
                entity_id: Uuid::now_v7(),
                entity_name: "Entity A".to_string(),
                matched_tag: Some("manco".to_string()),
                confidence: 0.5,
                match_type: MatchType::Fuzzy,
            },
            EntityMatch {
                entity_id: Uuid::now_v7(),
                entity_name: "Entity B".to_string(),
                matched_tag: Some("manco".to_string()),
                confidence: 0.4,
                match_type: MatchType::Fuzzy,
            },
        ];

        let result = resolver.evaluate_matches(matches);
        assert_eq!(result.status, ResolutionOutcome::Ambiguous);
        assert_eq!(result.entities.len(), 2);
    }

    #[test]
    fn test_evaluate_no_matches() {
        let resolver = EntityArgResolver::new(Uuid::now_v7());
        let result = resolver.evaluate_matches(vec![]);
        assert_eq!(result.status, ResolutionOutcome::Failed);
    }
}
