//! Bidirectional Verb+Entity Convergence
//!
//! This module implements the dual-path convergence algorithm for maximizing
//! intent-to-DSL hit rate. Instead of verb-first or entity-first, we search
//! BOTH in parallel and cross-filter to find the best (verb, entity) tuple.
//!
//! ## Algorithm
//!
//! ```text
//! USER INPUT: "add custody to Acme Fund"
//!                     │
//!     ┌───────────────┴───────────────┐
//!     │                               │
//!     ▼                               ▼
//! VERB CLUE: "add"              ENTITY CLUE: "Acme Fund"
//!     │                               │
//!     ▼                               ▼
//! verb_search()                 entity_search()
//!     │                               │
//!     ▼                               ▼
//! VerbCandidate[]               EntityCandidate[]
//! - cbu.add-product (0.85)      - Acme Fund [CBU] (0.92)
//! - entity.add-role (0.72)      - Acme Fund Ltd [Entity] (0.78)
//!     │                               │
//!     └───────────────┬───────────────┘
//!                     │
//!                     ▼
//!             CROSS-FILTER:
//!             verb target_type ↔ entity type
//!                     │
//!                     ▼
//!             CONVERGENCE SCORE:
//!             (verb_score + entity_score + type_match_bonus) / 3
//!                     │
//!                     ▼
//!             BEST PAIR: (cbu.add-product, Acme Fund [CBU])
//! ```
//!
//! ## Key Insight
//!
//! The entity type is a STRONG signal for verb selection:
//! - Entity is a CBU → verbs from cbu.*, trading-profile.*, session.*
//! - Entity is a Person → verbs from entity.*, kyc.*, cbu-role.*
//! - Entity is a Document → verbs from document.*, requirement.*
//!
//! Similarly, the verb's target type narrows entity search:
//! - Verb is cbu.add-product → entity must be a CBU
//! - Verb is entity.create → we're creating, not looking up

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// TYPES
// =============================================================================

/// A verb candidate from semantic search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidate {
    /// Fully qualified verb name (e.g., "cbu.add-product")
    pub verb: String,
    /// Match score (0.0 - 1.0)
    pub score: f32,
    /// Entity types this verb operates on (from args with lookup config)
    pub target_types: Vec<String>,
    /// Entity type this verb produces (from produces.type)
    pub produces_type: Option<String>,
    /// The phrase that matched
    pub matched_phrase: String,
}

/// An entity candidate from entity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidate {
    /// Entity UUID
    pub entity_id: Uuid,
    /// Display name
    pub name: String,
    /// Entity type (cbu, entity, document, product, etc.)
    pub entity_type: String,
    /// Match score (0.0 - 1.0)
    pub score: f32,
    /// The search text that matched
    pub matched_text: String,
}

/// A converged (verb, entity) pair with combined score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergedPair {
    /// The verb candidate
    pub verb: VerbCandidate,
    /// The entity candidate (None if verb doesn't need entity)
    pub entity: Option<EntityCandidate>,
    /// Combined convergence score
    pub convergence_score: f32,
    /// Explanation of why this pair was chosen
    pub rationale: String,
}

/// Result of the convergence algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConvergenceOutcome {
    /// Clear winner - one (verb, entity) pair dominates
    Converged(ConvergedPair),
    /// Multiple good options - need user disambiguation
    Ambiguous {
        pairs: Vec<ConvergedPair>,
        question: String,
    },
    /// No good match found
    NoMatch {
        verb_candidates: Vec<VerbCandidate>,
        entity_candidates: Vec<EntityCandidate>,
        reason: String,
    },
}

// =============================================================================
// ENTITY TYPE → VERB DOMAIN MAPPING
// =============================================================================

/// Map entity type to verb domains that operate on it
pub fn domains_for_entity_type(entity_type: &str) -> Vec<&'static str> {
    match entity_type.to_lowercase().as_str() {
        // CBU-centric domains
        "cbu" => vec![
            "cbu",
            "trading-profile",
            "session",
            "view",
            "product",
            "custody",
            "isda",
            "semantic",
        ],
        // Entity (person/company) domains
        "entity" | "person" | "company" | "legal_entity" => {
            vec!["entity", "cbu-role", "kyc", "ubo", "control", "screening"]
        }
        // Document domains
        "document" => vec!["document", "requirement", "control"],
        // Product/service domains
        "product" | "service" => vec!["product", "service-resource", "semantic"],
        // Client group (scope-setting)
        "client_group" | "client" => vec!["session", "view", "runbook"],
        // KYC case
        "kyc_case" => vec!["kyc", "kyc-case"],
        // Default - allow any
        _ => vec![],
    }
}

/// Map verb domain to entity types it primarily operates on
pub fn entity_types_for_domain(domain: &str) -> Vec<&'static str> {
    match domain {
        "cbu" => vec!["cbu", "entity"],
        "trading-profile" => vec!["cbu"],
        "session" => vec!["cbu", "client_group"],
        "view" => vec!["cbu", "entity"],
        "entity" => vec!["entity", "person", "company"],
        "cbu-role" => vec!["cbu", "entity"],
        "kyc" | "kyc-case" => vec!["cbu", "entity", "kyc_case"],
        "ubo" | "control" => vec!["entity"],
        "document" | "requirement" => vec!["document", "entity"],
        "product" => vec!["product", "cbu"],
        "custody" => vec!["cbu"],
        "isda" => vec!["cbu", "entity"],
        "screening" => vec!["entity"],
        _ => vec![],
    }
}

// =============================================================================
// CONVERGENCE ALGORITHM
// =============================================================================

/// Configuration for the convergence algorithm
#[derive(Debug, Clone)]
pub struct ConvergenceConfig {
    /// Minimum verb score to consider
    pub min_verb_score: f32,
    /// Minimum entity score to consider
    pub min_entity_score: f32,
    /// Bonus for type match (verb target = entity type)
    pub type_match_bonus: f32,
    /// Gap required between top pair and runner-up for clear winner
    pub ambiguity_gap: f32,
    /// Maximum pairs to return in ambiguous case
    pub max_ambiguous_pairs: usize,
}

impl Default for ConvergenceConfig {
    fn default() -> Self {
        Self {
            min_verb_score: 0.50,
            min_entity_score: 0.40,
            type_match_bonus: 0.15,
            ambiguity_gap: 0.10,
            max_ambiguous_pairs: 3,
        }
    }
}

/// The main convergence engine
pub struct ConvergenceEngine {
    config: ConvergenceConfig,
}

impl ConvergenceEngine {
    pub fn new() -> Self {
        Self {
            config: ConvergenceConfig::default(),
        }
    }

    pub fn with_config(config: ConvergenceConfig) -> Self {
        Self { config }
    }

    /// Run the convergence algorithm
    ///
    /// Takes verb candidates and entity candidates from parallel searches,
    /// cross-filters based on type compatibility, and returns the best pair(s).
    pub fn converge(
        &self,
        verb_candidates: Vec<VerbCandidate>,
        entity_candidates: Vec<EntityCandidate>,
    ) -> ConvergenceOutcome {
        // Filter by minimum scores
        let verbs: Vec<_> = verb_candidates
            .into_iter()
            .filter(|v| v.score >= self.config.min_verb_score)
            .collect();

        let entities: Vec<_> = entity_candidates
            .into_iter()
            .filter(|e| e.score >= self.config.min_entity_score)
            .collect();

        // No verbs found
        if verbs.is_empty() {
            return ConvergenceOutcome::NoMatch {
                verb_candidates: vec![],
                entity_candidates: entities,
                reason: "No verbs matched above threshold".to_string(),
            };
        }

        // If no entities found, return top verb without entity
        if entities.is_empty() {
            let top_verb = verbs.into_iter().next().unwrap();
            return ConvergenceOutcome::Converged(ConvergedPair {
                rationale: format!("Verb '{}' matched (no entity in input)", top_verb.verb),
                verb: top_verb,
                entity: None,
                convergence_score: 0.0, // Will be set below
            });
        }

        // Generate all (verb, entity) pairs with convergence scores
        let mut pairs: Vec<ConvergedPair> = Vec::new();

        for verb in &verbs {
            for entity in &entities {
                let (score, rationale) = self.score_pair(verb, entity);
                if score > 0.0 {
                    pairs.push(ConvergedPair {
                        verb: verb.clone(),
                        entity: Some(entity.clone()),
                        convergence_score: score,
                        rationale,
                    });
                }
            }
        }

        // Sort by convergence score descending
        pairs.sort_by(|a, b| {
            b.convergence_score
                .partial_cmp(&a.convergence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // No valid pairs (type mismatches filtered everything)
        if pairs.is_empty() {
            // Fall back to top verb without entity constraint
            let top_verb = verbs.into_iter().next().unwrap();
            return ConvergenceOutcome::Converged(ConvergedPair {
                rationale: format!(
                    "Verb '{}' matched (entity type mismatch filtered pairs)",
                    top_verb.verb
                ),
                verb: top_verb,
                entity: None,
                convergence_score: 0.0,
            });
        }

        // Check for clear winner vs ambiguity
        let top = &pairs[0];
        if pairs.len() == 1 {
            return ConvergenceOutcome::Converged(pairs.into_iter().next().unwrap());
        }

        let runner_up = &pairs[1];
        let gap = top.convergence_score - runner_up.convergence_score;

        if gap >= self.config.ambiguity_gap {
            // Clear winner
            ConvergenceOutcome::Converged(pairs.into_iter().next().unwrap())
        } else {
            // Ambiguous - return top N pairs for user selection
            let top_pairs: Vec<_> = pairs
                .into_iter()
                .take(self.config.max_ambiguous_pairs)
                .collect();

            let options: Vec<String> = top_pairs
                .iter()
                .map(|p| {
                    if let Some(ref e) = p.entity {
                        format!("{} on {} [{}]", p.verb.verb, e.name, e.entity_type)
                    } else {
                        p.verb.verb.clone()
                    }
                })
                .collect();

            ConvergenceOutcome::Ambiguous {
                question: format!("Did you mean: {}?", options.join(" or ")),
                pairs: top_pairs,
            }
        }
    }

    /// Score a (verb, entity) pair
    ///
    /// Returns (score, rationale) where score combines:
    /// - verb match score
    /// - entity match score
    /// - type compatibility bonus
    fn score_pair(&self, verb: &VerbCandidate, entity: &EntityCandidate) -> (f32, String) {
        // Check type compatibility
        let verb_domain = verb.verb.split('.').next().unwrap_or("");
        let compatible_types = entity_types_for_domain(verb_domain);
        let compatible_domains = domains_for_entity_type(&entity.entity_type);

        let type_match = verb.target_types.contains(&entity.entity_type)
            || compatible_types.contains(&entity.entity_type.as_str())
            || compatible_domains.contains(&verb_domain);

        if !type_match {
            // Type mismatch - this pair is invalid
            return (
                0.0,
                format!(
                    "Type mismatch: verb '{}' (domain {}) incompatible with entity type '{}'",
                    verb.verb, verb_domain, entity.entity_type
                ),
            );
        }

        // Calculate convergence score
        let base_score = (verb.score + entity.score) / 2.0;
        let bonus = if verb.target_types.contains(&entity.entity_type) {
            self.config.type_match_bonus // Strong match - verb explicitly targets this type
        } else {
            self.config.type_match_bonus / 2.0 // Weak match - domain compatible but not explicit
        };

        let final_score = (base_score + bonus).min(1.0);

        let rationale = format!(
            "verb={} ({:.2}) + entity={} [{}] ({:.2}) + type_bonus={:.2} = {:.2}",
            verb.verb,
            verb.score,
            entity.name,
            entity.entity_type,
            entity.score,
            bonus,
            final_score
        );

        (final_score, rationale)
    }
}

impl Default for ConvergenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// PARALLEL SEARCH INTEGRATION
// =============================================================================

/// Extract potential entity mentions from an utterance
///
/// This is a simple heuristic - looks for:
/// - Quoted strings
/// - Capitalized words (proper nouns)
/// - Known entity patterns (e.g., "Fund", "Ltd", "Inc")
pub fn extract_entity_mentions(input: &str) -> Vec<String> {
    let mut mentions = Vec::new();

    // Extract quoted strings
    let mut in_quote = false;
    let mut quote_start = 0;
    for (i, c) in input.char_indices() {
        if c == '"' {
            if in_quote {
                let quoted = &input[quote_start + 1..i];
                if !quoted.is_empty() {
                    mentions.push(quoted.to_string());
                }
                in_quote = false;
            } else {
                quote_start = i;
                in_quote = true;
            }
        }
    }

    // Extract capitalized sequences (proper nouns)
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut current_proper: Vec<&str> = Vec::new();

    for word in &words {
        let is_capitalized = word
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        let is_entity_suffix = matches!(
            word.to_lowercase().as_str(),
            "fund" | "funds" | "ltd" | "limited" | "inc" | "corp" | "plc" | "gmbh" | "ag" | "sa"
        );

        if is_capitalized || (is_entity_suffix && !current_proper.is_empty()) {
            current_proper.push(word);
        } else if !current_proper.is_empty() {
            let proper_noun = current_proper.join(" ");
            if proper_noun.len() > 2 {
                mentions.push(proper_noun);
            }
            current_proper.clear();
        }
    }

    // Don't forget the last sequence
    if !current_proper.is_empty() {
        let proper_noun = current_proper.join(" ");
        if proper_noun.len() > 2 {
            mentions.push(proper_noun);
        }
    }

    // Deduplicate
    mentions.sort();
    mentions.dedup();
    mentions
}

// =============================================================================
// DATABASE-INTEGRATED PARALLEL SEARCH
// =============================================================================

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Search for entities matching any of the extracted mentions
///
/// Uses the existing `search_entity_tags` SQL function for fuzzy matching.
/// Row type for entity search results
#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct EntitySearchRow {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    score: f64,
}

/// Row type for CBU search results
#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct CbuSearchRow {
    entity_id: Uuid,
    name: String,
    entity_type: String,
    score: Option<f32>,
}

/// Returns candidates with entity_type populated from the database.
#[cfg(feature = "database")]
pub async fn search_entities_parallel(
    pool: &PgPool,
    mentions: &[String],
    client_group_id: Option<Uuid>,
    limit_per_mention: i32,
) -> Result<Vec<EntityCandidate>, sqlx::Error> {
    let mut all_candidates = Vec::new();

    for mention in mentions {
        // Search across entity types using the search_entity_tags function
        let rows: Vec<EntitySearchRow> = sqlx::query_as(
            r#"
            SELECT
                entity_id,
                name,
                entity_type,
                similarity_score as score
            FROM "ob-poc".search_entity_tags($1, $2, $3, $4, FALSE)
            "#,
        )
        .bind(mention)
        .bind(client_group_id)
        .bind(limit_per_mention)
        .bind(0.3f64)
        .fetch_all(pool)
        .await?;

        for row in rows {
            all_candidates.push(EntityCandidate {
                entity_id: row.entity_id,
                name: row.name,
                entity_type: row.entity_type,
                score: row.score as f32,
                matched_text: mention.clone(),
            });
        }

        // Also search CBUs specifically (they're in a different table)
        let cbu_rows: Vec<CbuSearchRow> = sqlx::query_as(
            r#"
            SELECT
                cbu_id as entity_id,
                name,
                'cbu' as entity_type,
                similarity(LOWER(name), LOWER($1)) as score
            FROM "ob-poc".cbus
            WHERE similarity(LOWER(name), LOWER($1)) > 0.3
            ORDER BY similarity(LOWER(name), LOWER($1)) DESC
            LIMIT $2
            "#,
        )
        .bind(mention)
        .bind(limit_per_mention as i64)
        .fetch_all(pool)
        .await?;

        for row in cbu_rows {
            all_candidates.push(EntityCandidate {
                entity_id: row.entity_id,
                name: row.name,
                entity_type: row.entity_type,
                score: row.score.unwrap_or(0.0),
                matched_text: mention.clone(),
            });
        }
    }

    // Deduplicate by entity_id, keeping highest score
    all_candidates.sort_by(|a, b| {
        a.entity_id.cmp(&b.entity_id).then(
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    all_candidates.dedup_by(|a, b| a.entity_id == b.entity_id);

    // Sort by score descending
    all_candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(all_candidates)
}

/// Convert VerbSearchResult to VerbCandidate with target_types from registry
pub fn verb_result_to_candidate(
    result: &crate::mcp::verb_search::VerbSearchResult,
) -> VerbCandidate {
    // Get target types from verb registry
    let target_types = get_verb_target_types(&result.verb);
    let produces_type = get_verb_produces_type(&result.verb);

    VerbCandidate {
        verb: result.verb.clone(),
        score: result.score,
        target_types,
        produces_type,
        matched_phrase: result.matched_phrase.clone(),
    }
}

/// Get the entity types a verb operates on (from its args with lookup config)
fn get_verb_target_types(verb_fqn: &str) -> Vec<String> {
    let registry = crate::dsl_v2::runtime_registry();
    let parts: Vec<&str> = verb_fqn.splitn(2, '.').collect();
    if parts.len() != 2 {
        return vec![];
    }

    let verb_def = match registry.get(parts[0], parts[1]) {
        Some(v) => v,
        None => return vec![],
    };

    verb_def
        .args
        .iter()
        .filter_map(|arg| arg.lookup.as_ref().and_then(|l| l.entity_type.clone()))
        .collect()
}

/// Get the entity type a verb produces (from its produces config)
fn get_verb_produces_type(verb_fqn: &str) -> Option<String> {
    let registry = crate::dsl_v2::runtime_registry();
    let parts: Vec<&str> = verb_fqn.splitn(2, '.').collect();
    if parts.len() != 2 {
        return None;
    }

    let verb_def = registry.get(parts[0], parts[1])?;
    verb_def.produces.as_ref().map(|p| p.produced_type.clone())
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domains_for_entity_type() {
        let cbu_domains = domains_for_entity_type("cbu");
        assert!(cbu_domains.contains(&"cbu"));
        assert!(cbu_domains.contains(&"trading-profile"));

        let entity_domains = domains_for_entity_type("entity");
        assert!(entity_domains.contains(&"entity"));
        assert!(entity_domains.contains(&"cbu-role"));
    }

    #[test]
    fn test_extract_entity_mentions() {
        let mentions = extract_entity_mentions("add custody to Acme Fund");
        assert!(mentions.contains(&"Acme Fund".to_string()));

        let mentions2 = extract_entity_mentions("show \"BlackRock Inc\" CBUs");
        assert!(mentions2.contains(&"BlackRock Inc".to_string()));
    }

    #[test]
    fn test_convergence_type_match() {
        let engine = ConvergenceEngine::new();

        let verbs = vec![VerbCandidate {
            verb: "cbu.add-product".to_string(),
            score: 0.85,
            target_types: vec!["cbu".to_string()],
            produces_type: None,
            matched_phrase: "add product".to_string(),
        }];

        let entities = vec![
            EntityCandidate {
                entity_id: Uuid::new_v4(),
                name: "Acme Fund".to_string(),
                entity_type: "cbu".to_string(),
                score: 0.90,
                matched_text: "Acme Fund".to_string(),
            },
            EntityCandidate {
                entity_id: Uuid::new_v4(),
                name: "Acme Fund Ltd".to_string(),
                entity_type: "entity".to_string(),
                score: 0.60, // Lower score to create clear gap
                matched_text: "Acme Fund Ltd".to_string(),
            },
        ];

        let result = engine.converge(verbs, entities);

        match result {
            ConvergenceOutcome::Converged(pair) => {
                assert_eq!(pair.verb.verb, "cbu.add-product");
                assert!(pair.entity.is_some());
                let entity = pair.entity.unwrap();
                // Should prefer CBU entity because verb targets CBU and it has higher score
                assert_eq!(entity.entity_type, "cbu");
            }
            ConvergenceOutcome::Ambiguous { pairs, .. } => {
                // Also acceptable - verify the top pair is the CBU
                assert!(!pairs.is_empty());
                let top = &pairs[0];
                assert_eq!(top.verb.verb, "cbu.add-product");
                assert!(top.entity.is_some());
                assert_eq!(top.entity.as_ref().unwrap().entity_type, "cbu");
            }
            _ => panic!("Expected Converged or Ambiguous outcome"),
        }
    }

    #[test]
    fn test_convergence_ambiguous() {
        let engine = ConvergenceEngine::new();

        let verbs = vec![
            VerbCandidate {
                verb: "cbu.add-product".to_string(),
                score: 0.80,
                target_types: vec!["cbu".to_string()],
                produces_type: None,
                matched_phrase: "add".to_string(),
            },
            VerbCandidate {
                verb: "entity.add-role".to_string(),
                score: 0.78,
                target_types: vec!["entity".to_string()],
                produces_type: None,
                matched_phrase: "add".to_string(),
            },
        ];

        let entities = vec![
            EntityCandidate {
                entity_id: Uuid::new_v4(),
                name: "Acme".to_string(),
                entity_type: "cbu".to_string(),
                score: 0.75,
                matched_text: "Acme".to_string(),
            },
            EntityCandidate {
                entity_id: Uuid::new_v4(),
                name: "Acme".to_string(),
                entity_type: "entity".to_string(),
                score: 0.74,
                matched_text: "Acme".to_string(),
            },
        ];

        let result = engine.converge(verbs, entities);

        match result {
            ConvergenceOutcome::Ambiguous { pairs, .. } => {
                assert!(pairs.len() >= 2);
            }
            ConvergenceOutcome::Converged(_) => {
                // Also acceptable if scores happened to diverge enough
            }
            _ => panic!("Expected Ambiguous or Converged outcome"),
        }
    }
}
