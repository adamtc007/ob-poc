//! Scope Resolution - Stage 0 of Intent Pipeline
//!
//! This module resolves client group scope BEFORE Candle verb discovery runs.
//! It ensures that entity resolution happens within the correct client context.
//!
//! ## Pipeline Order (Enforced)
//!
//! ```text
//! 1. Scope Resolution (this module) - group alias → group_id
//! 2. Session anchor set / candidates picker
//! 3. Candle verb discovery (with group_id + persona as boosts/filters)
//! 4. DSL assembly (entity args resolved within group scope first)
//! ```
//!
//! ## Key Principle
//!
//! If scope resolution can consume the input (e.g., "I'm working on Allianz"),
//! it returns early and does NOT proceed to verb discovery. The session context
//! is set, and the next user input will benefit from the established scope.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// SCOPE RESOLUTION OUTCOME - Deterministic UX Contract
// =============================================================================

/// Outcome of scope resolution - deterministic UX contract
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScopeResolutionOutcome {
    /// Scope resolved unambiguously - set anchor silently, show "Client: X" chip
    Resolved {
        group_id: Uuid,
        group_name: String,
        entity_count: i64,
    },
    /// Multiple candidates - show compact picker (top 3-5)
    Candidates(Vec<ScopeCandidate>),
    /// No match above threshold - proceed without scope, don't open entity search
    Unresolved,
    /// Input was not a scope-setting phrase - continue to verb discovery
    NotScopePhrase,
}

/// A candidate for scope resolution (when ambiguous)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeCandidate {
    pub group_id: Uuid,
    pub group_name: String,
    pub matched_alias: String,
    pub confidence: f64,
}

// =============================================================================
// SCOPE CONTEXT - Passed through pipeline
// =============================================================================

/// Scope context for entity resolution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopeContext {
    /// Active client group ID (if resolved)
    pub client_group_id: Option<Uuid>,
    /// Client group name (for display)
    pub client_group_name: Option<String>,
    /// Active persona (kyc, trading, ops, onboarding)
    pub persona: Option<String>,
}

impl ScopeContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_client_group(mut self, group_id: Uuid, group_name: String) -> Self {
        self.client_group_id = Some(group_id);
        self.client_group_name = Some(group_name);
        self
    }

    pub fn with_persona(mut self, persona: String) -> Self {
        self.persona = Some(persona);
        self
    }

    /// Check if we have client scope set
    pub fn has_scope(&self) -> bool {
        self.client_group_id.is_some()
    }
}

// =============================================================================
// SCOPE RESOLVER
// =============================================================================

/// Scope resolver - detects and resolves client group context
pub struct ScopeResolver {
    /// Threshold for exact match (alias_norm = query)
    exact_threshold: f64,
    /// Threshold for fuzzy match
    fuzzy_threshold: f64,
    /// Gap required between top and runner-up for unambiguous resolution
    ambiguity_gap: f64,
}

impl Default for ScopeResolver {
    fn default() -> Self {
        Self {
            exact_threshold: 1.0,
            fuzzy_threshold: 0.5,
            ambiguity_gap: 0.10,
        }
    }
}

impl ScopeResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect if input is a scope-setting phrase
    ///
    /// Patterns that indicate scope-setting intent:
    /// - "work on X", "working on X"
    /// - "switch to X", "set client to X"
    /// - "I'm working with X", "client is X"
    /// - Just a client name by itself (if it matches a known alias)
    pub fn is_scope_phrase(input: &str) -> bool {
        let lower = input.to_lowercase();
        let prefixes = [
            "work on ",
            "working on ",
            "switch to ",
            "set client to ",
            "set client ",
            "i'm working with ",
            "im working with ",
            "client is ",
            "for client ",
            "load ",
        ];

        for prefix in prefixes {
            if lower.starts_with(prefix) {
                return true;
            }
        }

        // Also check if it's just a potential client name (short input, no verb-like words)
        let words: Vec<&str> = lower.split_whitespace().collect();
        if words.len() <= 3 && !Self::has_verb_indicator(&lower) {
            return true; // Might be just a client name
        }

        false
    }

    /// Check if input has verb-like indicators (suggesting it's a command, not scope)
    fn has_verb_indicator(input: &str) -> bool {
        let verb_indicators = [
            "create", "delete", "update", "show", "list", "add", "remove", "get", "set ", "find",
            "search", "execute", "run", "approve", "reject", "submit",
        ];

        for indicator in verb_indicators {
            if input.contains(indicator) {
                return true;
            }
        }
        false
    }

    /// Extract the potential client name from a scope phrase
    pub fn extract_client_name(input: &str) -> Option<String> {
        let lower = input.to_lowercase();
        let prefixes = [
            "work on ",
            "working on ",
            "switch to ",
            "set client to ",
            "set client ",
            "i'm working with ",
            "im working with ",
            "client is ",
            "for client ",
            "load ",
        ];

        for prefix in prefixes {
            if lower.starts_with(prefix) {
                let remainder = input[prefix.len()..].trim();
                if !remainder.is_empty() {
                    return Some(remainder.to_string());
                }
            }
        }

        // If no prefix matched, the whole input might be a client name
        let trimmed = input.trim();
        if !trimmed.is_empty() && trimmed.split_whitespace().count() <= 3 {
            return Some(trimmed.to_string());
        }

        None
    }

    /// Resolve scope from input
    ///
    /// Returns:
    /// - `Resolved` if unambiguous match found
    /// - `Candidates` if multiple matches need user selection
    /// - `Unresolved` if no match found
    /// - `NotScopePhrase` if input doesn't look like a scope-setting phrase
    #[cfg(feature = "database")]
    pub async fn resolve(&self, input: &str, pool: &PgPool) -> Result<ScopeResolutionOutcome> {
        // Check if this looks like a scope-setting phrase
        if !Self::is_scope_phrase(input) {
            return Ok(ScopeResolutionOutcome::NotScopePhrase);
        }

        // Extract the client name
        let client_name = match Self::extract_client_name(input) {
            Some(name) => name,
            None => return Ok(ScopeResolutionOutcome::NotScopePhrase),
        };

        let client_norm = client_name.to_lowercase();

        // Search for matching client groups via aliases
        let matches = sqlx::query!(
            r#"
            SELECT
                cg.id as group_id,
                cg.canonical_name as "group_name!",
                cga.alias as "matched_alias!",
                cga.confidence as "confidence!",
                (cga.alias_norm = $1) as "exact_match!"
            FROM "ob-poc".client_group_alias cga
            JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
            WHERE cga.alias_norm = $1
               OR cga.alias_norm ILIKE '%' || $1 || '%'
               OR similarity(cga.alias_norm, $1) > $2
            ORDER BY
                (cga.alias_norm = $1) DESC,
                cga.confidence DESC,
                similarity(cga.alias_norm, $1) DESC
            LIMIT 5
            "#,
            client_norm,
            self.fuzzy_threshold as f32
        )
        .fetch_all(pool)
        .await?;

        if matches.is_empty() {
            return Ok(ScopeResolutionOutcome::Unresolved);
        }

        // Check if we have a clear winner
        let top = &matches[0];
        let has_clear_winner = top.exact_match
            || matches.len() == 1
            || (matches.len() > 1 && (top.confidence - matches[1].confidence) > self.ambiguity_gap);

        if has_clear_winner {
            // Get entity count for this group
            let entity_count: i64 = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM "ob-poc".client_group_entity
                WHERE group_id = $1 AND membership_type != 'historical'
                "#,
                top.group_id
            )
            .fetch_one(pool)
            .await?;

            return Ok(ScopeResolutionOutcome::Resolved {
                group_id: top.group_id,
                group_name: top.group_name.clone(),
                entity_count,
            });
        }

        // Ambiguous - return candidates for user selection
        let candidates: Vec<ScopeCandidate> = matches
            .into_iter()
            .take(3)
            .map(|m| ScopeCandidate {
                group_id: m.group_id,
                group_name: m.group_name,
                matched_alias: m.matched_alias,
                confidence: m.confidence,
            })
            .collect();

        Ok(ScopeResolutionOutcome::Candidates(candidates))
    }

    #[cfg(not(feature = "database"))]
    pub async fn resolve(&self, _input: &str) -> Result<ScopeResolutionOutcome> {
        Ok(ScopeResolutionOutcome::NotScopePhrase)
    }

    /// Record a user's scope selection (flywheel)
    ///
    /// When user picks a candidate from the list, reinforce that alias
    #[cfg(feature = "database")]
    pub async fn record_selection(
        pool: &PgPool,
        group_id: Uuid,
        alias_used: &str,
        _session_id: &str,
    ) -> Result<()> {
        let alias_norm = alias_used.to_lowercase().trim().to_string();

        // Reinforce the alias that worked (or create if new)
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".client_group_alias
                (group_id, alias, alias_norm, confidence, source)
            VALUES ($1, $2, $3, 0.9, 'user_confirmed')
            ON CONFLICT (group_id, alias_norm) DO UPDATE SET
                confidence = LEAST(client_group_alias.confidence + 0.05, 1.0),
                source = 'user_confirmed'
            "#,
            group_id,
            alias_used,
            alias_norm
        )
        .execute(pool)
        .await?;

        tracing::info!(
            group_id = %group_id,
            alias = alias_used,
            "Recorded scope selection (flywheel)"
        );

        Ok(())
    }
}

// =============================================================================
// ENTITY CONTEXT SEARCH - Within scope
// =============================================================================

/// Search for entities within the current client group scope
///
/// This is called during DSL argument extraction when we have entity references
/// that need resolution. By searching within scope first, we avoid ambiguity
/// and the dreaded "entity search modal".
#[cfg(feature = "database")]
pub async fn search_entities_in_scope(
    pool: &PgPool,
    scope: &ScopeContext,
    query: &str,
    limit: usize,
) -> Result<Vec<EntityMatch>> {
    let Some(group_id) = scope.client_group_id else {
        return Ok(vec![]); // No scope, can't search
    };

    // Use the search_entity_tags function for text search (exact + fuzzy)
    let rows = sqlx::query!(
        r#"
        SELECT
            entity_id as "entity_id!",
            entity_name as "entity_name!",
            tag as "tag!",
            confidence as "confidence!",
            match_type as "match_type!"
        FROM "ob-poc".search_entity_tags($1, $2, $3, $4, FALSE)
        "#,
        group_id,
        query,
        scope.persona.as_deref(),
        limit as i32
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EntityMatch {
            entity_id: r.entity_id,
            entity_name: r.entity_name,
            matched_tag: r.tag,
            confidence: r.confidence as f64,
            match_type: r.match_type,
        })
        .collect())
}

/// An entity match from scoped search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub matched_tag: String,
    pub confidence: f64,
    pub match_type: String, // "exact", "fuzzy", "semantic"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_scope_phrase() {
        assert!(ScopeResolver::is_scope_phrase("work on allianz"));
        assert!(ScopeResolver::is_scope_phrase("working on blackrock"));
        assert!(ScopeResolver::is_scope_phrase("switch to aviva"));
        assert!(ScopeResolver::is_scope_phrase("set client to allianz"));
        assert!(ScopeResolver::is_scope_phrase("client is blackrock"));

        // Short inputs might be client names
        assert!(ScopeResolver::is_scope_phrase("allianz"));
        assert!(ScopeResolver::is_scope_phrase("black rock"));

        // Commands should NOT be scope phrases
        assert!(!ScopeResolver::is_scope_phrase(
            "create a new cbu for allianz"
        ));
        assert!(!ScopeResolver::is_scope_phrase("show me the irish funds"));
        assert!(!ScopeResolver::is_scope_phrase(
            "list all entities for allianz"
        ));
    }

    #[test]
    fn test_extract_client_name() {
        assert_eq!(
            ScopeResolver::extract_client_name("work on allianz"),
            Some("allianz".to_string())
        );
        assert_eq!(
            ScopeResolver::extract_client_name("switch to Black Rock"),
            Some("Black Rock".to_string())
        );
        assert_eq!(
            ScopeResolver::extract_client_name("allianz"),
            Some("allianz".to_string())
        );
        assert_eq!(
            ScopeResolver::extract_client_name("client is Aviva Investors"),
            Some("Aviva Investors".to_string())
        );
    }
}
