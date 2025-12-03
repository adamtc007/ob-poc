//! Fuzzy search service for CBU and entity name lookups.
//!
//! Two-stage approach:
//! 1. GIN index (pg_trgm) - fast candidate filtering for large datasets
//! 2. strsim (Jaro-Winkler) - accurate ranking on filtered candidates
//!
//! For small datasets (<500 items), skips GIN and uses strsim directly.

use sqlx::PgPool;
use uuid::Uuid;

/// Threshold for "small" dataset - use strsim directly
#[allow(dead_code)]
const SMALL_DATASET_THRESHOLD: usize = 500;

/// Minimum GIN trigram similarity for candidate filtering (lower = more candidates)
const GIN_CANDIDATE_THRESHOLD: f32 = 0.2;

/// Minimum Jaro-Winkler similarity for final results
const JARO_WINKLER_THRESHOLD: f64 = 0.6;

/// Result from fuzzy CBU search
#[derive(Debug, Clone)]
pub struct FuzzyCbuMatch {
    pub cbu_id: Uuid,
    pub name: String,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub similarity_score: f32,
}

/// Result from fuzzy entity search
#[derive(Debug, Clone)]
pub struct FuzzyEntityMatch {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub similarity_score: f32,
}

/// Result from fuzzy person search
#[derive(Debug, Clone)]
pub struct FuzzyPersonMatch {
    pub entity_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub full_name: String,
    pub similarity_score: f32,
}

/// Result from fuzzy company search
#[derive(Debug, Clone)]
pub struct FuzzyCompanyMatch {
    pub entity_id: Uuid,
    pub company_name: String,
    pub jurisdiction: Option<String>,
    pub similarity_score: f32,
}

/// Unified search result
#[derive(Debug, Clone)]
pub struct FuzzySearchResult {
    pub id: Uuid,
    pub name: String,
    pub result_type: String,
    pub subtype: Option<String>,
    pub similarity_score: f32,
}

/// Service for fuzzy text search using two-stage GIN + strsim approach
pub struct FuzzySearchService {
    pool: PgPool,
}

impl FuzzySearchService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find CBUs by fuzzy name match
    /// Uses two-stage approach: GIN filtering → strsim ranking
    pub async fn find_cbu(
        &self,
        search_term: &str,
        max_results: usize,
    ) -> Result<Vec<FuzzyCbuMatch>, sqlx::Error> {
        // Stage 1: Get candidates using GIN index (or all if small dataset)
        let candidates: Vec<(Uuid, String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT cbu_id, name, client_type, jurisdiction
            FROM "ob-poc".cbus
            WHERE name % $1 OR similarity(name, $1) >= $2
            ORDER BY similarity(name, $1) DESC
            LIMIT 100
            "#,
        )
        .bind(search_term)
        .bind(GIN_CANDIDATE_THRESHOLD)
        .fetch_all(&self.pool)
        .await?;

        // Stage 2: Re-rank with Jaro-Winkler for accurate scoring
        let mut results: Vec<FuzzyCbuMatch> = candidates
            .into_iter()
            .map(|(cbu_id, name, client_type, jurisdiction)| {
                let score = strsim::jaro_winkler(&search_term.to_lowercase(), &name.to_lowercase());
                FuzzyCbuMatch {
                    cbu_id,
                    name,
                    client_type,
                    jurisdiction,
                    similarity_score: score as f32,
                }
            })
            .filter(|m| m.similarity_score >= JARO_WINKLER_THRESHOLD as f32)
            .collect();

        // Sort by Jaro-Winkler score (best first)
        results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        results.truncate(max_results);

        Ok(results)
    }

    /// Find entities by fuzzy name match
    pub async fn find_entity(
        &self,
        search_term: &str,
        max_results: usize,
    ) -> Result<Vec<FuzzyEntityMatch>, sqlx::Error> {
        // Stage 1: GIN filtering
        let candidates: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name, et.name as entity_type
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE e.name % $1 OR similarity(e.name, $1) >= $2
            ORDER BY similarity(e.name, $1) DESC
            LIMIT 100
            "#,
        )
        .bind(search_term)
        .bind(GIN_CANDIDATE_THRESHOLD)
        .fetch_all(&self.pool)
        .await?;

        // Stage 2: Jaro-Winkler ranking
        let mut results: Vec<FuzzyEntityMatch> = candidates
            .into_iter()
            .map(|(entity_id, name, entity_type)| {
                let score = strsim::jaro_winkler(&search_term.to_lowercase(), &name.to_lowercase());
                FuzzyEntityMatch {
                    entity_id,
                    name,
                    entity_type,
                    similarity_score: score as f32,
                }
            })
            .filter(|m| m.similarity_score >= JARO_WINKLER_THRESHOLD as f32)
            .collect();

        results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        results.truncate(max_results);

        Ok(results)
    }

    /// Find persons by fuzzy name match (searches first, last, and full name)
    pub async fn find_person(
        &self,
        search_term: &str,
        max_results: usize,
    ) -> Result<Vec<FuzzyPersonMatch>, sqlx::Error> {
        // Stage 1: GIN filtering on first_name, last_name, or combined
        let candidates: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT entity_id, first_name, last_name
            FROM "ob-poc".entity_proper_persons
            WHERE first_name % $1 
               OR last_name % $1 
               OR (first_name || ' ' || last_name) % $1
               OR similarity(first_name || ' ' || last_name, $1) >= $2
            ORDER BY similarity(first_name || ' ' || last_name, $1) DESC
            LIMIT 100
            "#,
        )
        .bind(search_term)
        .bind(GIN_CANDIDATE_THRESHOLD)
        .fetch_all(&self.pool)
        .await?;

        // Stage 2: Jaro-Winkler on full name
        let search_lower = search_term.to_lowercase();
        let mut results: Vec<FuzzyPersonMatch> = candidates
            .into_iter()
            .map(|(entity_id, first_name, last_name)| {
                let full_name = format!("{} {}", first_name, last_name);
                let score = strsim::jaro_winkler(&search_lower, &full_name.to_lowercase())
                    .max(strsim::jaro_winkler(&search_lower, &first_name.to_lowercase()))
                    .max(strsim::jaro_winkler(&search_lower, &last_name.to_lowercase()));
                FuzzyPersonMatch {
                    entity_id,
                    first_name,
                    last_name,
                    full_name,
                    similarity_score: score as f32,
                }
            })
            .filter(|m| m.similarity_score >= JARO_WINKLER_THRESHOLD as f32)
            .collect();

        results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        results.truncate(max_results);

        Ok(results)
    }

    /// Find companies by fuzzy name match
    pub async fn find_company(
        &self,
        search_term: &str,
        max_results: usize,
    ) -> Result<Vec<FuzzyCompanyMatch>, sqlx::Error> {
        // Stage 1: GIN filtering
        let candidates: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT entity_id, company_name, jurisdiction
            FROM "ob-poc".entity_limited_companies
            WHERE company_name % $1 OR similarity(company_name, $1) >= $2
            ORDER BY similarity(company_name, $1) DESC
            LIMIT 100
            "#,
        )
        .bind(search_term)
        .bind(GIN_CANDIDATE_THRESHOLD)
        .fetch_all(&self.pool)
        .await?;

        // Stage 2: Jaro-Winkler ranking
        let mut results: Vec<FuzzyCompanyMatch> = candidates
            .into_iter()
            .map(|(entity_id, company_name, jurisdiction)| {
                let score = strsim::jaro_winkler(&search_term.to_lowercase(), &company_name.to_lowercase());
                FuzzyCompanyMatch {
                    entity_id,
                    company_name,
                    jurisdiction,
                    similarity_score: score as f32,
                }
            })
            .filter(|m| m.similarity_score >= JARO_WINKLER_THRESHOLD as f32)
            .collect();

        results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        results.truncate(max_results);

        Ok(results)
    }

    /// Unified search across CBUs and all entity types
    pub async fn search_all(
        &self,
        search_term: &str,
        max_results: usize,
    ) -> Result<Vec<FuzzySearchResult>, sqlx::Error> {
        // Run searches in parallel would be better, but for simplicity:
        let cbus = self.find_cbu(search_term, max_results).await?;
        let entities = self.find_entity(search_term, max_results).await?;

        let mut results: Vec<FuzzySearchResult> = Vec::new();

        for cbu in cbus {
            results.push(FuzzySearchResult {
                id: cbu.cbu_id,
                name: cbu.name,
                result_type: "cbu".to_string(),
                subtype: cbu.client_type,
                similarity_score: cbu.similarity_score,
            });
        }

        for entity in entities {
            results.push(FuzzySearchResult {
                id: entity.entity_id,
                name: entity.name,
                result_type: "entity".to_string(),
                subtype: Some(entity.entity_type),
                similarity_score: entity.similarity_score,
            });
        }

        // Sort all results by score
        results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        results.truncate(max_results);

        Ok(results)
    }

    /// Quick resolve: find best matching CBU by name
    /// Returns None if no match above threshold
    pub async fn resolve_cbu_name(
        &self,
        name: &str,
    ) -> Result<Option<FuzzyCbuMatch>, sqlx::Error> {
        let results = self.find_cbu(name, 1).await?;
        Ok(results.into_iter().next())
    }

    /// Quick resolve: find best matching entity by name
    pub async fn resolve_entity_name(
        &self,
        name: &str,
    ) -> Result<Option<FuzzyEntityMatch>, sqlx::Error> {
        let results = self.find_entity(name, 1).await?;
        Ok(results.into_iter().next())
    }
}

// =============================================================================
// Small dataset fuzzy matching (for products, services, roles, etc.)
// Uses strsim directly without GIN - suitable for <500 items
// =============================================================================

/// Find best matches in a small list using Jaro-Winkler
/// Use this for lookup tables like products, services, roles
pub fn fuzzy_match_small_list<'a>(
    search_term: &str,
    candidates: &'a [String],
    max_results: usize,
) -> Vec<(&'a str, f32)> {
    if candidates.is_empty() {
        return vec![];
    }

    let search_lower = search_term.to_lowercase();
    
    let mut scored: Vec<(&str, f64)> = candidates
        .iter()
        .map(|c| {
            let score = strsim::jaro_winkler(&search_lower, &c.to_lowercase());
            (c.as_str(), score)
        })
        .filter(|(_, score)| *score >= JARO_WINKLER_THRESHOLD)
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    
    scored
        .into_iter()
        .take(max_results)
        .map(|(s, score)| (s, score as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_small_list() {
        let candidates = vec![
            "custody".to_string(),
            "fund-admin".to_string(),
            "prime-brokerage".to_string(),
            "trade-settlement".to_string(),
        ];

        // Exact match
        let results = fuzzy_match_small_list("custody", &candidates, 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "custody");
        assert!(results[0].1 > 0.99);

        // Typo
        let results = fuzzy_match_small_list("custdy", &candidates, 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "custody");

        // No good match
        let results = fuzzy_match_small_list("xyz123", &candidates, 3);
        assert!(results.is_empty());
    }
}
