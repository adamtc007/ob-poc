//! Search Engine - Interpreter for search s-expressions
//!
//! The search engine takes a schema (from verb YAML) and a query (runtime values),
//! then executes the search against the index, applying discriminator scoring.

use std::collections::HashMap;
use std::sync::Arc;

use crate::index::{MatchMode, SearchIndex, SearchMatch, SearchQuery as IndexQuery};
use crate::search_expr::{SearchQuery, SearchSchema};

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
                        // Compare values
                        let match_score =
                            if entity_value.to_lowercase() == query_value.to_lowercase() {
                                1.0 // Exact match
                            } else if entity_value
                                .to_lowercase()
                                .contains(&query_value.to_lowercase())
                            {
                                0.7 // Partial match
                            } else {
                                0.0 // No match
                            };

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
}
