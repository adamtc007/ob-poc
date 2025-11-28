//! Generic Entity Search Service
//!
//! Provides fast, fuzzy search across all entity types using PostgreSQL pg_trgm.
//! Supports typeahead with debounce-friendly response times (<10ms typical).

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// =============================================================================
// Types
// =============================================================================

/// Entity types that can be searched
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityType {
    Cbu,
    Person,
    Company,
    Trust,
    Document,
    Product,
    Service,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Cbu => "CBU",
            EntityType::Person => "PERSON",
            EntityType::Company => "COMPANY",
            EntityType::Trust => "TRUST",
            EntityType::Document => "DOCUMENT",
            EntityType::Product => "PRODUCT",
            EntityType::Service => "SERVICE",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CBU" => Some(EntityType::Cbu),
            "PERSON" => Some(EntityType::Person),
            "COMPANY" => Some(EntityType::Company),
            "TRUST" => Some(EntityType::Trust),
            "DOCUMENT" => Some(EntityType::Document),
            "PRODUCT" => Some(EntityType::Product),
            "SERVICE" => Some(EntityType::Service),
            _ => None,
        }
    }
}

/// Search request parameters
#[derive(Debug, Clone, Deserialize)]
pub struct EntitySearchRequest {
    /// Search query (e.g., "john sm")
    pub query: String,

    /// Entity types to search (empty = all types)
    #[serde(default)]
    pub types: Vec<EntityType>,

    /// Maximum results to return
    #[serde(default = "default_limit")]
    pub limit: u32,

    /// Minimum similarity threshold (0.0 - 1.0)
    #[serde(default = "default_threshold")]
    pub threshold: f32,

    /// Optional: limit search to entities related to this CBU
    pub cbu_id: Option<Uuid>,
}

fn default_limit() -> u32 {
    10
}
fn default_threshold() -> f32 {
    0.2
}

/// A single search result
#[derive(Debug, Clone, Serialize)]
pub struct EntityMatch {
    /// Entity UUID
    pub id: Uuid,

    /// Entity type (PERSON, COMPANY, etc.)
    pub entity_type: EntityType,

    /// Primary display name
    pub display_name: String,

    /// Secondary info (nationality, jurisdiction, etc.)
    pub subtitle: Option<String>,

    /// Tertiary info (DOB, reg number, etc.)
    pub detail: Option<String>,

    /// Similarity score (0.0 - 1.0)
    pub score: f32,
}

/// Search response
#[derive(Debug, Clone, Serialize)]
pub struct EntitySearchResponse {
    /// Matching entities
    pub results: Vec<EntityMatch>,

    /// Total matches (before limit)
    pub total: u32,

    /// Whether results were truncated
    pub truncated: bool,

    /// Search time in milliseconds
    pub search_time_ms: u64,
}

// =============================================================================
// Service
// =============================================================================

pub struct EntitySearchService {
    pool: PgPool,
}

impl EntitySearchService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Search entities by name with fuzzy matching
    pub async fn search(
        &self,
        req: &EntitySearchRequest,
    ) -> Result<EntitySearchResponse, sqlx::Error> {
        let start = std::time::Instant::now();

        // Clean and validate query
        let query = req.query.trim();
        if query.is_empty() {
            return Ok(EntitySearchResponse {
                results: vec![],
                total: 0,
                truncated: false,
                search_time_ms: 0,
            });
        }

        // Build type filter
        let type_filter = if req.types.is_empty() {
            None
        } else {
            Some(req.types.iter().map(|t| t.as_str()).collect::<Vec<_>>())
        };

        // Execute search
        let results = self
            .search_internal(query, type_filter.as_deref(), req.limit, req.threshold)
            .await?;

        let search_time_ms = start.elapsed().as_millis() as u64;
        let total = results.len() as u32;
        let truncated = total >= req.limit;

        Ok(EntitySearchResponse {
            results,
            total,
            truncated,
            search_time_ms,
        })
    }

    async fn search_internal(
        &self,
        query: &str,
        types: Option<&[&str]>,
        limit: u32,
        threshold: f32,
    ) -> Result<Vec<EntityMatch>, sqlx::Error> {
        // Set the similarity threshold for the % operator
        sqlx::query(&format!("SET pg_trgm.similarity_threshold = {}", threshold))
            .execute(&self.pool)
            .await
            .ok(); // Ignore errors, use default

        let rows = sqlx::query_as::<_, EntitySearchRow>(
            r#"
            SELECT
                id,
                entity_type,
                display_name,
                subtitle_1,
                subtitle_2,
                similarity(search_text, $1) as score
            FROM "ob-poc".entity_search_view
            WHERE search_text % $1
                AND ($2::text[] IS NULL OR entity_type = ANY($2))
            ORDER BY score DESC, display_name ASC
            LIMIT $3
            "#,
        )
        .bind(query)
        .bind(types)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Search for entities attached to a specific CBU
    pub async fn search_cbu_entities(
        &self,
        cbu_id: Uuid,
        query: Option<&str>,
        _types: Option<&[EntityType]>,
        limit: u32,
    ) -> Result<Vec<EntityMatch>, sqlx::Error> {
        // Search entities that are already attached to this CBU
        let rows = sqlx::query_as::<_, EntitySearchRow>(
            r#"
            WITH cbu_entities AS (
                SELECT
                    ce.entity_id as id,
                    CASE
                        WHEN pp.entity_id IS NOT NULL THEN 'PERSON'
                        WHEN lc.entity_id IS NOT NULL THEN 'COMPANY'
                        ELSE 'UNKNOWN'
                    END as entity_type,
                    COALESCE(
                        pp.given_name || ' ' || pp.family_name,
                        lc.company_name
                    ) as display_name,
                    COALESCE(pp.nationality, lc.jurisdiction) as subtitle_1,
                    ce.role as subtitle_2,
                    COALESCE(
                        pp.given_name || ' ' || pp.family_name,
                        lc.company_name,
                        ''
                    ) as search_text
                FROM "ob-poc".cbu_entities ce
                LEFT JOIN "ob-poc".proper_persons pp ON ce.entity_id = pp.entity_id
                LEFT JOIN "ob-poc".limited_companies lc ON ce.entity_id = lc.entity_id
                WHERE ce.cbu_id = $1
            )
            SELECT
                id,
                entity_type,
                display_name,
                subtitle_1,
                subtitle_2,
                CASE
                    WHEN $2::text IS NULL THEN 1.0
                    ELSE similarity(search_text, $2)
                END as score
            FROM cbu_entities
            WHERE $2::text IS NULL OR search_text % $2
            ORDER BY score DESC, display_name ASC
            LIMIT $3
            "#,
        )
        .bind(cbu_id)
        .bind(query)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get a single entity by ID and type
    pub async fn get_entity(
        &self,
        id: Uuid,
        entity_type: EntityType,
    ) -> Result<Option<EntityMatch>, sqlx::Error> {
        let row = sqlx::query_as::<_, EntitySearchRow>(
            r#"
            SELECT
                id,
                entity_type,
                display_name,
                subtitle_1,
                subtitle_2,
                1.0 as score
            FROM "ob-poc".entity_search_view
            WHERE id = $1 AND entity_type = $2
            "#,
        )
        .bind(id)
        .bind(entity_type.as_str())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }
}

// =============================================================================
// Internal Types
// =============================================================================

#[derive(Debug, sqlx::FromRow)]
struct EntitySearchRow {
    id: Uuid,
    entity_type: String,
    display_name: String,
    subtitle_1: Option<String>,
    subtitle_2: Option<String>,
    score: f32,
}

impl From<EntitySearchRow> for EntityMatch {
    fn from(row: EntitySearchRow) -> Self {
        EntityMatch {
            id: row.id,
            entity_type: EntityType::parse(&row.entity_type).unwrap_or(EntityType::Person),
            display_name: row.display_name,
            subtitle: row.subtitle_1,
            detail: row.subtitle_2,
            score: row.score,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_serde() {
        assert_eq!(EntityType::Person.as_str(), "PERSON");
        assert_eq!(EntityType::parse("PERSON"), Some(EntityType::Person));
        assert_eq!(EntityType::parse("person"), Some(EntityType::Person));
    }

    #[test]
    fn test_search_request_defaults() {
        let json = r#"{"query": "john"}"#;
        let req: EntitySearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.limit, 10);
        assert_eq!(req.threshold, 0.2);
        assert!(req.types.is_empty());
    }
}
