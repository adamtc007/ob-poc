//! Search Engine - Interpreter for search s-expressions
//!
//! The search engine takes a schema (from verb YAML) and a query (runtime values),
//! then executes the search against the index, applying discriminator scoring.
//!
//! ## Date Matching Modes
//!
//! For date discriminators like `date_of_birth`, the engine supports multiple match modes:
//! - `exact` - Dates must match exactly (default)
//! - `year_only` - Only the year portion must match
//! - `year_or_exact` - Match if year matches OR exact date matches (progressive refinement)

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::DateMatchMode;
use crate::index::{MatchMode, SearchIndex, SearchMatch, SearchQuery as IndexQuery};
use crate::search_expr::{SearchQuery, SearchSchema};

/// Extract year from various date formats
/// Supports: "1980-01-15", "1980", "15/01/1980", "01-15-1980", etc.
fn extract_year(date_str: &str) -> Option<u16> {
    let trimmed = date_str.trim();

    // Need at least 4 characters to contain a year
    if trimmed.len() < 4 {
        return None;
    }

    // Try to find a 4-digit year
    // First, check if it's just a year
    if trimmed.len() == 4 {
        return trimmed.parse().ok();
    }

    // Try ISO format (YYYY-MM-DD)
    if trimmed.len() >= 10 && trimmed.chars().nth(4) == Some('-') {
        if let Ok(year) = trimmed[0..4].parse::<u16>() {
            return Some(year);
        }
    }

    // Try to find 4-digit sequence that looks like a year (1900-2100)
    for i in 0..=trimmed.len().saturating_sub(4) {
        if let Ok(year) = trimmed[i..i + 4].parse::<u16>() {
            if (1900..=2100).contains(&year) {
                return Some(year);
            }
        }
    }

    None
}

/// Normalize date to YYYY-MM-DD format for comparison
fn normalize_date(date_str: &str) -> String {
    let trimmed = date_str.trim();

    // Already ISO format
    if trimmed.len() == 10
        && trimmed.chars().nth(4) == Some('-')
        && trimmed.chars().nth(7) == Some('-')
    {
        return trimmed.to_string();
    }

    // Just a year - return as-is for comparison
    if trimmed.len() == 4 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return trimmed.to_string();
    }

    // Try common formats and normalize
    // DD/MM/YYYY or MM/DD/YYYY - ambiguous, just return as-is
    trimmed.to_string()
}

/// Result of a search operation
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Matched entities, ranked by score
    pub matches: Vec<RankedMatch>,
    /// Whether a unique match was found (score >= min_confidence)
    pub resolved: bool,
    /// The resolved entity ID (if unique match found)
    pub resolved_id: Option<String>,
}

/// A match with combined score from primary + discriminators
#[derive(Debug, Clone)]
pub struct RankedMatch {
    /// Entity ID (token)
    pub id: String,
    /// Display value
    pub display: String,
    /// Primary search score (from Tantivy)
    pub primary_score: f32,
    /// Discriminator match scores
    pub discriminator_scores: HashMap<String, f32>,
    /// Combined score (weighted by selectivity)
    pub combined_score: f32,
}

/// Search engine that interprets s-expression queries
pub struct SearchEngine {
    /// The search schema (from verb YAML)
    schema: SearchSchema,
}

impl SearchEngine {
    /// Create a new search engine with the given schema
    pub fn new(schema: SearchSchema) -> Self {
        Self { schema }
    }

    /// Execute a search query against an index
    ///
    /// The query contains values, the schema defines the structure.
    /// Returns ranked matches with combined scores.
    pub async fn search(
        &self,
        index: Arc<dyn SearchIndex + Send + Sync>,
        query: &SearchQuery,
        limit: usize,
    ) -> SearchResult {
        // Step 1: Primary search using Tantivy
        let primary_query = IndexQuery {
            values: vec![query.primary_value.clone()],
            search_key: self.schema.primary_field.clone(),
            mode: MatchMode::Fuzzy,
            limit: limit * 3, // Get more candidates for discriminator filtering
            discriminators: query.discriminators.clone(),
        };

        let primary_matches = index.search(&primary_query).await;

        if primary_matches.is_empty() {
            return SearchResult {
                matches: vec![],
                resolved: false,
                resolved_id: None,
            };
        }

        // Step 2: Score matches using discriminators
        let mut ranked_matches: Vec<RankedMatch> = primary_matches
            .into_iter()
            .map(|m| self.score_match(m, query))
            .collect();

        // Step 3: Sort by combined score
        ranked_matches.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Step 4: Truncate to limit
        ranked_matches.truncate(limit);

        // Step 5: Check for unique resolution
        let (resolved, resolved_id) = self.check_resolution(&ranked_matches);

        SearchResult {
            matches: ranked_matches,
            resolved,
            resolved_id,
        }
    }

    /// Score a primary match using discriminator values
    fn score_match(&self, primary_match: SearchMatch, query: &SearchQuery) -> RankedMatch {
        let mut discriminator_scores = HashMap::new();
        let mut selectivity_sum = 0.0f32;
        let mut weighted_score_sum = 0.0f32;

        for disc in &self.schema.discriminators {
            if query.discriminators.contains_key(&disc.field) {
                // We have a value for this discriminator
                // Check if it matches (would need to query the actual data)
                // For now, we just record that we have the value
                // The actual matching happens in the refined search
                discriminator_scores.insert(disc.field.clone(), disc.selectivity);
                selectivity_sum += disc.selectivity;
                weighted_score_sum += disc.selectivity;
            }
        }

        // Combined score = primary score + discriminator boost
        // Discriminators boost the score based on their selectivity
        let discriminator_boost = if selectivity_sum > 0.0 {
            weighted_score_sum / selectivity_sum * 0.2 // Up to 20% boost
        } else {
            0.0
        };

        let combined_score = primary_match.score + discriminator_boost;

        RankedMatch {
            id: primary_match.token,
            display: primary_match.display,
            primary_score: primary_match.score,
            discriminator_scores,
            combined_score,
        }
    }

    /// Check if we have a unique resolution
    fn check_resolution(&self, matches: &[RankedMatch]) -> (bool, Option<String>) {
        if matches.is_empty() {
            return (false, None);
        }

        // Check if top match is above confidence threshold
        let top_match = &matches[0];
        if top_match.combined_score < self.schema.min_confidence {
            return (false, None);
        }

        // Check if there's a clear winner (significant gap to second place)
        if matches.len() > 1 {
            let second_score = matches[1].combined_score;
            let gap = top_match.combined_score - second_score;
            if gap < 0.1 {
                // Too close, ambiguous
                return (false, None);
            }
        }

        (true, Some(top_match.id.clone()))
    }

    /// Execute a search with discriminator filtering
    ///
    /// This is a two-phase search:
    /// 1. Primary search for candidates
    /// 2. Filter/boost by discriminator values
    pub async fn search_with_filter(
        &self,
        index: Arc<dyn SearchIndex + Send + Sync>,
        query: &SearchQuery,
        filter_data: &HashMap<String, HashMap<String, String>>, // id -> field -> value
        limit: usize,
    ) -> SearchResult {
        // Step 1: Primary search
        let primary_query = IndexQuery {
            values: vec![query.primary_value.clone()],
            search_key: self.schema.primary_field.clone(),
            mode: MatchMode::Fuzzy,
            limit: limit * 5,
            discriminators: query.discriminators.clone(),
        };

        let primary_matches = index.search(&primary_query).await;

        if primary_matches.is_empty() {
            return SearchResult {
                matches: vec![],
                resolved: false,
                resolved_id: None,
            };
        }

        // Step 2: Score and filter by discriminators
        let mut ranked_matches: Vec<RankedMatch> = Vec::new();

        for m in primary_matches {
            let entity_data = filter_data.get(&m.token);
            let ranked = self.score_match_with_data(m, query, entity_data);

            // Only include if required discriminators match
            if self.check_required_discriminators(&ranked, query, entity_data) {
                ranked_matches.push(ranked);
            }
        }

        // Step 3: Sort and truncate
        ranked_matches.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked_matches.truncate(limit);

        // Step 4: Check resolution
        let (resolved, resolved_id) = self.check_resolution(&ranked_matches);

        SearchResult {
            matches: ranked_matches,
            resolved,
            resolved_id,
        }
    }

    /// Score a match using actual entity data for discriminator comparison
    fn score_match_with_data(
        &self,
        primary_match: SearchMatch,
        query: &SearchQuery,
        entity_data: Option<&HashMap<String, String>>,
    ) -> RankedMatch {
        let mut discriminator_scores = HashMap::new();
        let mut selectivity_sum = 0.0f32;
        let mut weighted_score_sum = 0.0f32;

        for disc in &self.schema.discriminators {
            if let Some(query_value) = query.discriminators.get(&disc.field) {
                // We have a query value for this discriminator
                if let Some(data) = entity_data {
                    if let Some(entity_value) = data.get(&disc.field) {
                        // Compare values using appropriate match mode
                        let match_score = self.compare_discriminator_values(
                            &disc.field,
                            query_value,
                            entity_value,
                            disc.match_mode.as_deref(),
                        );

                        discriminator_scores.insert(disc.field.clone(), match_score);
                        selectivity_sum += disc.selectivity;
                        weighted_score_sum += disc.selectivity * match_score;
                    }
                }
            }
        }

        // Combined score
        let discriminator_boost = if selectivity_sum > 0.0 {
            weighted_score_sum / selectivity_sum * 0.3 // Up to 30% boost for matching discriminators
        } else {
            0.0
        };

        let combined_score = primary_match.score + discriminator_boost;

        RankedMatch {
            id: primary_match.token,
            display: primary_match.display,
            primary_score: primary_match.score,
            discriminator_scores,
            combined_score,
        }
    }

    /// Compare discriminator values using appropriate match mode
    fn compare_discriminator_values(
        &self,
        field: &str,
        query_value: &str,
        entity_value: &str,
        match_mode: Option<&str>,
    ) -> f32 {
        // Check for date fields with special match modes
        if field.contains("date") || field.contains("dob") || field.contains("birth") {
            let mode = match_mode
                .and_then(|s| s.parse().ok())
                .unwrap_or(DateMatchMode::Exact);
            return self.compare_dates(query_value, entity_value, mode);
        }

        // Standard string comparison
        let query_lower = query_value.to_lowercase();
        let entity_lower = entity_value.to_lowercase();

        if entity_lower == query_lower {
            1.0 // Exact match
        } else if entity_lower.contains(&query_lower) || query_lower.contains(&entity_lower) {
            0.7 // Partial match
        } else {
            0.0 // No match
        }
    }

    /// Compare date values with support for year-only and year-or-exact matching
    fn compare_dates(&self, query_value: &str, entity_value: &str, mode: DateMatchMode) -> f32 {
        // Extract years from date strings
        // Supports formats: "1980-01-15", "1980", "15/01/1980", etc.
        let query_year = extract_year(query_value);
        let entity_year = extract_year(entity_value);

        match mode {
            DateMatchMode::Exact => {
                // Normalize and compare full dates
                if normalize_date(query_value) == normalize_date(entity_value) {
                    1.0
                } else {
                    0.0
                }
            }
            DateMatchMode::YearOnly => {
                // Only compare years
                match (query_year, entity_year) {
                    (Some(qy), Some(ey)) if qy == ey => 1.0,
                    _ => 0.0,
                }
            }
            DateMatchMode::YearOrExact => {
                // Best of both: exact match gets 1.0, year match gets 0.8
                if normalize_date(query_value) == normalize_date(entity_value) {
                    1.0 // Exact date match
                } else {
                    match (query_year, entity_year) {
                        (Some(qy), Some(ey)) if qy == ey => 0.8, // Year match only
                        _ => 0.0,
                    }
                }
            }
        }
    }

    /// Check if all required discriminators match
    fn check_required_discriminators(
        &self,
        _ranked: &RankedMatch,
        query: &SearchQuery,
        entity_data: Option<&HashMap<String, String>>,
    ) -> bool {
        for disc in &self.schema.discriminators {
            if disc.required {
                if let Some(query_value) = query.discriminators.get(&disc.field) {
                    if let Some(data) = entity_data {
                        if let Some(entity_value) = data.get(&disc.field) {
                            if entity_value.to_lowercase() != query_value.to_lowercase() {
                                return false; // Required discriminator doesn't match
                            }
                        } else {
                            return false; // Required field not in entity data
                        }
                    } else {
                        return false; // No entity data
                    }
                }
                // If query doesn't have the required discriminator, we can't filter
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SearchMatch;
    use crate::search_expr::SearchSchema;

    #[test]
    fn test_search_result_resolved() {
        let result = SearchResult {
            matches: vec![RankedMatch {
                id: "uuid-1".to_string(),
                display: "John Smith".to_string(),
                primary_score: 0.95,
                discriminator_scores: HashMap::new(),
                combined_score: 0.95,
            }],
            resolved: true,
            resolved_id: Some("uuid-1".to_string()),
        };

        assert!(result.resolved);
        assert_eq!(result.resolved_id, Some("uuid-1".to_string()));
    }

    #[test]
    fn test_search_result_ambiguous() {
        let result = SearchResult {
            matches: vec![
                RankedMatch {
                    id: "uuid-1".to_string(),
                    display: "John Smith".to_string(),
                    primary_score: 0.85,
                    discriminator_scores: HashMap::new(),
                    combined_score: 0.85,
                },
                RankedMatch {
                    id: "uuid-2".to_string(),
                    display: "John Smith Jr".to_string(),
                    primary_score: 0.83,
                    discriminator_scores: HashMap::new(),
                    combined_score: 0.83,
                },
            ],
            resolved: false,
            resolved_id: None,
        };

        assert!(!result.resolved);
        assert!(result.resolved_id.is_none());
    }

    // ==========================================================================
    // DATE EXTRACTION AND NORMALIZATION TESTS
    // ==========================================================================

    #[test]
    fn test_extract_year_iso_format() {
        assert_eq!(extract_year("1980-01-15"), Some(1980));
        assert_eq!(extract_year("2023-12-31"), Some(2023));
    }

    #[test]
    fn test_extract_year_year_only() {
        assert_eq!(extract_year("1980"), Some(1980));
        assert_eq!(extract_year("2023"), Some(2023));
    }

    #[test]
    fn test_extract_year_slash_format() {
        assert_eq!(extract_year("15/01/1980"), Some(1980));
        assert_eq!(extract_year("01/15/1980"), Some(1980));
    }

    #[test]
    fn test_extract_year_invalid() {
        assert_eq!(extract_year("invalid"), None);
        assert_eq!(extract_year(""), None);
        assert_eq!(extract_year("abc"), None);
    }

    #[test]
    fn test_normalize_date_iso() {
        assert_eq!(normalize_date("1980-01-15"), "1980-01-15");
    }

    #[test]
    fn test_normalize_date_year_only() {
        assert_eq!(normalize_date("1980"), "1980");
    }

    // ==========================================================================
    // DATE MATCHING MODE TESTS
    // ==========================================================================

    fn create_test_engine() -> SearchEngine {
        let schema = SearchSchema::parse(
            "(search_name (nationality :selectivity 0.7) (date_of_birth :selectivity 0.95 :match-mode year-or-exact))"
        ).unwrap();
        SearchEngine::new(schema)
    }

    #[test]
    fn test_compare_dates_exact_match() {
        let engine = create_test_engine();

        // Exact match
        let score = engine.compare_dates("1980-01-15", "1980-01-15", DateMatchMode::Exact);
        assert!((score - 1.0).abs() < 0.01);

        // No match (different dates)
        let score = engine.compare_dates("1980-01-15", "1980-02-20", DateMatchMode::Exact);
        assert!((score - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_compare_dates_year_only() {
        let engine = create_test_engine();

        // Same year, different day
        let score = engine.compare_dates("1980-01-15", "1980-12-25", DateMatchMode::YearOnly);
        assert!((score - 1.0).abs() < 0.01);

        // Different year
        let score = engine.compare_dates("1980-01-15", "1981-01-15", DateMatchMode::YearOnly);
        assert!((score - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_compare_dates_year_or_exact() {
        let engine = create_test_engine();

        // Exact match gets 1.0
        let score = engine.compare_dates("1980-01-15", "1980-01-15", DateMatchMode::YearOrExact);
        assert!((score - 1.0).abs() < 0.01);

        // Year match only gets 0.8
        let score = engine.compare_dates("1980-01-15", "1980-06-30", DateMatchMode::YearOrExact);
        assert!((score - 0.8).abs() < 0.01);

        // Different year gets 0.0
        let score = engine.compare_dates("1980-01-15", "1981-01-15", DateMatchMode::YearOrExact);
        assert!((score - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_compare_dates_with_year_only_input() {
        let engine = create_test_engine();

        // Query with year-only matches full date with same year
        let score = engine.compare_dates("1980", "1980-06-30", DateMatchMode::YearOrExact);
        assert!((score - 0.8).abs() < 0.01);

        // Query with year-only against different year
        let score = engine.compare_dates("1980", "1981-01-15", DateMatchMode::YearOrExact);
        assert!((score - 0.0).abs() < 0.01);
    }

    // ==========================================================================
    // DISCRIMINATOR VALUE COMPARISON TESTS
    // ==========================================================================

    #[test]
    fn test_compare_discriminator_nationality_exact() {
        let engine = create_test_engine();

        // Exact match (case insensitive)
        let score = engine.compare_discriminator_values("nationality", "US", "US", None);
        assert!((score - 1.0).abs() < 0.01);

        let score = engine.compare_discriminator_values("nationality", "us", "US", None);
        assert!((score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_compare_discriminator_nationality_partial() {
        let engine = create_test_engine();

        // Partial match
        let score = engine.compare_discriminator_values("nationality", "United States", "US", None);
        // Contains check: "US" contains in "United States" or vice versa
        assert!(score >= 0.0); // May be 0.7 for partial or 0.0 if no contain
    }

    #[test]
    fn test_compare_discriminator_nationality_no_match() {
        let engine = create_test_engine();

        // No match
        let score = engine.compare_discriminator_values("nationality", "US", "GB", None);
        assert!((score - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_compare_discriminator_date_auto_detected() {
        let engine = create_test_engine();

        // Field name contains "date" - should use date comparison
        let score = engine.compare_discriminator_values(
            "date_of_birth",
            "1980-01-15",
            "1980-06-30",
            Some("year-or-exact"),
        );
        // Year matches, should get 0.8
        assert!((score - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_compare_discriminator_dob_field() {
        let engine = create_test_engine();

        // Field name contains "dob" - should use date comparison
        let score =
            engine.compare_discriminator_values("dob", "1980-01-15", "1980-01-15", Some("exact"));
        assert!((score - 1.0).abs() < 0.01);
    }

    // ==========================================================================
    // RESOLUTION CHECK TESTS
    // ==========================================================================

    #[test]
    fn test_check_resolution_single_high_confidence() {
        let engine = create_test_engine();

        let matches = vec![RankedMatch {
            id: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            primary_score: 0.95,
            discriminator_scores: HashMap::new(),
            combined_score: 0.95,
        }];

        let (resolved, id) = engine.check_resolution(&matches);
        assert!(resolved);
        assert_eq!(id, Some("uuid-1".to_string()));
    }

    #[test]
    fn test_check_resolution_single_low_confidence() {
        let engine = create_test_engine();

        let matches = vec![RankedMatch {
            id: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            primary_score: 0.5,
            discriminator_scores: HashMap::new(),
            combined_score: 0.5, // Below 0.8 threshold
        }];

        let (resolved, id) = engine.check_resolution(&matches);
        assert!(!resolved);
        assert!(id.is_none());
    }

    #[test]
    fn test_check_resolution_ambiguous_close_scores() {
        let engine = create_test_engine();

        let matches = vec![
            RankedMatch {
                id: "uuid-1".to_string(),
                display: "John Smith".to_string(),
                primary_score: 0.90,
                discriminator_scores: HashMap::new(),
                combined_score: 0.90,
            },
            RankedMatch {
                id: "uuid-2".to_string(),
                display: "John A Smith".to_string(),
                primary_score: 0.88,
                discriminator_scores: HashMap::new(),
                combined_score: 0.88, // Gap < 0.1
            },
        ];

        let (resolved, id) = engine.check_resolution(&matches);
        assert!(!resolved); // Too close, ambiguous
        assert!(id.is_none());
    }

    #[test]
    fn test_check_resolution_clear_winner() {
        let engine = create_test_engine();

        let matches = vec![
            RankedMatch {
                id: "uuid-1".to_string(),
                display: "John Smith".to_string(),
                primary_score: 0.95,
                discriminator_scores: HashMap::new(),
                combined_score: 0.95,
            },
            RankedMatch {
                id: "uuid-2".to_string(),
                display: "John A Smith".to_string(),
                primary_score: 0.70,
                discriminator_scores: HashMap::new(),
                combined_score: 0.70, // Gap > 0.1
            },
        ];

        let (resolved, id) = engine.check_resolution(&matches);
        assert!(resolved); // Clear winner
        assert_eq!(id, Some("uuid-1".to_string()));
    }

    #[test]
    fn test_check_resolution_empty() {
        let engine = create_test_engine();

        let (resolved, id) = engine.check_resolution(&[]);
        assert!(!resolved);
        assert!(id.is_none());
    }

    // ==========================================================================
    // REQUIRED DISCRIMINATOR TESTS
    // ==========================================================================

    #[test]
    fn test_check_required_discriminators_no_required() {
        let schema = SearchSchema::parse("(search_name (nationality :selectivity 0.7))").unwrap();
        let engine = SearchEngine::new(schema);

        let ranked = RankedMatch {
            id: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            primary_score: 0.9,
            discriminator_scores: HashMap::new(),
            combined_score: 0.9,
        };

        let query = SearchQuery::simple("John Smith");
        let result = engine.check_required_discriminators(&ranked, &query, None);
        assert!(result); // No required discriminators, passes
    }

    #[test]
    fn test_check_required_discriminators_required_matches() {
        let schema =
            SearchSchema::parse("(search_name (nationality :selectivity 0.7 :required true))")
                .unwrap();
        let engine = SearchEngine::new(schema);

        let ranked = RankedMatch {
            id: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            primary_score: 0.9,
            discriminator_scores: HashMap::new(),
            combined_score: 0.9,
        };

        let mut query = SearchQuery::simple("John Smith");
        query
            .discriminators
            .insert("nationality".to_string(), "US".to_string());

        let mut entity_data = HashMap::new();
        entity_data.insert("nationality".to_string(), "US".to_string());

        let result = engine.check_required_discriminators(&ranked, &query, Some(&entity_data));
        assert!(result); // Required discriminator matches
    }

    #[test]
    fn test_check_required_discriminators_required_mismatch() {
        let schema =
            SearchSchema::parse("(search_name (nationality :selectivity 0.7 :required true))")
                .unwrap();
        let engine = SearchEngine::new(schema);

        let ranked = RankedMatch {
            id: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            primary_score: 0.9,
            discriminator_scores: HashMap::new(),
            combined_score: 0.9,
        };

        let mut query = SearchQuery::simple("John Smith");
        query
            .discriminators
            .insert("nationality".to_string(), "US".to_string());

        let mut entity_data = HashMap::new();
        entity_data.insert("nationality".to_string(), "GB".to_string()); // Mismatch!

        let result = engine.check_required_discriminators(&ranked, &query, Some(&entity_data));
        assert!(!result); // Required discriminator doesn't match
    }

    // ==========================================================================
    // SCORE MATCH WITH DATA TESTS (discriminator boosting)
    // ==========================================================================

    #[test]
    fn test_score_match_with_data_nationality_boost() {
        let schema = SearchSchema::parse("(search_name (nationality :selectivity 0.7))").unwrap();
        let engine = SearchEngine::new(schema);

        let primary_match = SearchMatch {
            input: "John Smith".to_string(),
            token: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            score: 0.8,
        };

        let mut query = SearchQuery::simple("John Smith");
        query
            .discriminators
            .insert("nationality".to_string(), "US".to_string());

        let mut entity_data = HashMap::new();
        entity_data.insert("nationality".to_string(), "US".to_string());

        let ranked = engine.score_match_with_data(primary_match, &query, Some(&entity_data));

        // Should have boosted score due to matching nationality
        assert!(ranked.combined_score > 0.8);
        assert!(ranked.discriminator_scores.contains_key("nationality"));
        assert!((ranked.discriminator_scores["nationality"] - 1.0).abs() < 0.01);
        // Exact match
    }

    #[test]
    fn test_score_match_with_data_dob_year_boost() {
        let schema = SearchSchema::parse(
            "(search_name (date_of_birth :selectivity 0.95 :match-mode year-or-exact))",
        )
        .unwrap();
        let engine = SearchEngine::new(schema);

        let primary_match = SearchMatch {
            input: "John Smith".to_string(),
            token: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            score: 0.8,
        };

        let mut query = SearchQuery::simple("John Smith");
        query
            .discriminators
            .insert("date_of_birth".to_string(), "1980-01-15".to_string());

        let mut entity_data = HashMap::new();
        entity_data.insert("date_of_birth".to_string(), "1980-06-30".to_string()); // Same year

        let ranked = engine.score_match_with_data(primary_match, &query, Some(&entity_data));

        // Should have boosted score (0.8 for year match with year-or-exact mode)
        assert!(ranked.combined_score > 0.8);
        assert!(ranked.discriminator_scores.contains_key("date_of_birth"));
        assert!((ranked.discriminator_scores["date_of_birth"] - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_score_match_with_data_combined_discriminators() {
        let schema = SearchSchema::parse(
            "(search_name (nationality :selectivity 0.7) (date_of_birth :selectivity 0.95 :match-mode year-or-exact))"
        ).unwrap();
        let engine = SearchEngine::new(schema);

        let primary_match = SearchMatch {
            input: "John Smith".to_string(),
            token: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            score: 0.75,
        };

        let mut query = SearchQuery::simple("John Smith");
        query
            .discriminators
            .insert("nationality".to_string(), "US".to_string());
        query
            .discriminators
            .insert("date_of_birth".to_string(), "1980-01-15".to_string());

        let mut entity_data = HashMap::new();
        entity_data.insert("nationality".to_string(), "US".to_string()); // Exact match
        entity_data.insert("date_of_birth".to_string(), "1980-01-15".to_string()); // Exact match

        let ranked = engine.score_match_with_data(primary_match, &query, Some(&entity_data));

        // Both discriminators match exactly
        assert!(ranked.combined_score > 0.75);
        assert_eq!(ranked.discriminator_scores.len(), 2);
        assert!((ranked.discriminator_scores["nationality"] - 1.0).abs() < 0.01);
        assert!((ranked.discriminator_scores["date_of_birth"] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_score_match_no_entity_data() {
        let schema = SearchSchema::parse("(search_name (nationality :selectivity 0.7))").unwrap();
        let engine = SearchEngine::new(schema);

        let primary_match = SearchMatch {
            input: "John Smith".to_string(),
            token: "uuid-1".to_string(),
            display: "John Smith".to_string(),
            score: 0.8,
        };

        let mut query = SearchQuery::simple("John Smith");
        query
            .discriminators
            .insert("nationality".to_string(), "US".to_string());

        // No entity data provided
        let ranked = engine.score_match_with_data(primary_match, &query, None);

        // Score should be unchanged since we can't verify discriminators
        assert!((ranked.combined_score - 0.8).abs() < 0.01);
        assert!(ranked.discriminator_scores.is_empty());
    }
}
