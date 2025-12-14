//! Resolution Strategy for Entity Disambiguation
//!
//! Determines whether to auto-resolve, ask user, or suggest creating new entity
//! based on match quality and conversation context.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enrichment::EntityContext;

/// Confidence level for entity resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionConfidence {
    /// Single exact match or very high score - auto-resolve
    High,
    /// Good match but may want to confirm
    Medium,
    /// Multiple similar matches - must ask user
    Low,
    /// No good matches at all
    None,
}

/// Suggested action based on analysis
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SuggestedAction {
    /// Use this match automatically
    AutoResolve { match_id: String },
    /// Ask user to choose between matches
    AskUser,
    /// Suggest creating a new entity
    SuggestCreate,
    /// Need more information to search effectively
    NeedMoreInfo { missing: Vec<String> },
}

/// Result of resolution analysis
#[derive(Debug, Clone, Serialize)]
pub struct ResolutionResult {
    pub confidence: ResolutionConfidence,
    pub action: SuggestedAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// An enriched match with context for resolution
#[derive(Debug, Clone, Serialize)]
pub struct EnrichedMatch {
    pub id: String,
    pub display: String,
    pub score: f32,
    pub entity_type: String,
    pub context: EntityContext,
    pub disambiguation_label: String,
}

/// Context from the conversation that can help resolution
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ConversationContext {
    /// Roles mentioned in conversation (e.g., ["DIRECTOR", "UBO"])
    #[serde(default)]
    pub mentioned_roles: Vec<String>,
    /// CBU name mentioned
    #[serde(default)]
    pub mentioned_cbu: Option<String>,
    /// Nationality mentioned (e.g., "US", "GB")
    #[serde(default)]
    pub mentioned_nationality: Option<String>,
    /// Jurisdiction mentioned
    #[serde(default)]
    pub mentioned_jurisdiction: Option<String>,
    /// Currently active CBU in the session
    #[serde(default)]
    pub current_cbu_id: Option<Uuid>,
}

/// Resolution strategy analyzer
pub struct ResolutionStrategy;

impl ResolutionStrategy {
    /// Analyze matches and determine resolution strategy
    ///
    /// Logic:
    /// 1. No matches → suggest create
    /// 2. Single high-score match (>0.90) → auto-resolve
    /// 3. Clear winner (gap >0.15 to second) → auto-resolve
    /// 4. Context-based resolution (role/nationality match) → auto-resolve with medium confidence
    /// 5. Multiple close matches → ask user
    /// 6. Low scores (<0.50) → suggest create
    pub fn analyze(
        matches: &[EnrichedMatch],
        conversation_context: Option<&ConversationContext>,
    ) -> ResolutionResult {
        // Case 1: No matches at all
        if matches.is_empty() {
            return ResolutionResult {
                confidence: ResolutionConfidence::None,
                action: SuggestedAction::SuggestCreate,
                prompt: Some(
                    "No matching entities found. Would you like to create a new one?".to_string(),
                ),
            };
        }

        let top = &matches[0];

        // Case 2: Single high-confidence match
        if matches.len() == 1 && top.score > 0.90 {
            return ResolutionResult {
                confidence: ResolutionConfidence::High,
                action: SuggestedAction::AutoResolve {
                    match_id: top.id.clone(),
                },
                prompt: None,
            };
        }

        // Case 3: Clear winner (big gap to second place)
        if matches.len() > 1 {
            let gap = top.score - matches[1].score;
            if gap > 0.15 && top.score > 0.85 {
                return ResolutionResult {
                    confidence: ResolutionConfidence::High,
                    action: SuggestedAction::AutoResolve {
                        match_id: top.id.clone(),
                    },
                    prompt: None,
                };
            }
        }

        // Case 4: Context-based resolution
        if let Some(ctx) = conversation_context {
            if let Some(resolved_id) = Self::resolve_from_context(matches, ctx) {
                return ResolutionResult {
                    confidence: ResolutionConfidence::Medium,
                    action: SuggestedAction::AutoResolve {
                        match_id: resolved_id,
                    },
                    prompt: None,
                };
            }
        }

        // Case 5: All matches have low scores - suggest create
        if top.score < 0.50 {
            return ResolutionResult {
                confidence: ResolutionConfidence::None,
                action: SuggestedAction::SuggestCreate,
                prompt: Some(format!(
                    "No good match found (best score: {:.0}%). Would you like to create a new entity?",
                    top.score * 100.0
                )),
            };
        }

        // Case 6: Multiple similar matches - ask user
        if matches.len() > 1 && (top.score - matches[1].score).abs() < 0.10 {
            return ResolutionResult {
                confidence: ResolutionConfidence::Low,
                action: SuggestedAction::AskUser,
                prompt: Some(Self::build_disambiguation_prompt(matches)),
            };
        }

        // Case 7: Single match with medium score
        if matches.len() == 1 && top.score >= 0.50 && top.score <= 0.90 {
            return ResolutionResult {
                confidence: ResolutionConfidence::Medium,
                action: SuggestedAction::AskUser,
                prompt: Some(format!(
                    "Found one match: {}. Is this correct?",
                    top.disambiguation_label
                )),
            };
        }

        // Default: ask user
        ResolutionResult {
            confidence: ResolutionConfidence::Low,
            action: SuggestedAction::AskUser,
            prompt: Some(Self::build_disambiguation_prompt(matches)),
        }
    }

    /// Try to resolve using conversation context clues
    fn resolve_from_context(
        matches: &[EnrichedMatch],
        ctx: &ConversationContext,
    ) -> Option<String> {
        // Priority 1: If user mentioned a specific role, prefer entity with that role
        for role in &ctx.mentioned_roles {
            let role_upper = role.to_uppercase();
            for m in matches {
                if m.context
                    .roles
                    .iter()
                    .any(|r| r.role.to_uppercase() == role_upper)
                {
                    return Some(m.id.clone());
                }
            }
        }

        // Priority 2: If user mentioned a specific CBU, prefer entity linked to it
        if let Some(cbu_name) = &ctx.mentioned_cbu {
            let cbu_lower = cbu_name.to_lowercase();
            for m in matches {
                if m.context
                    .roles
                    .iter()
                    .any(|r| r.cbu_name.to_lowercase().contains(&cbu_lower))
                {
                    return Some(m.id.clone());
                }
            }
        }

        // Priority 3: If user mentioned nationality, prefer matching entity
        if let Some(nat) = &ctx.mentioned_nationality {
            let nat_upper = nat.to_uppercase();
            for m in matches {
                if m.context
                    .nationality
                    .as_ref()
                    .map(|n| n.to_uppercase() == nat_upper)
                    .unwrap_or(false)
                {
                    return Some(m.id.clone());
                }
            }
        }

        // Priority 4: If user mentioned jurisdiction, prefer matching entity
        if let Some(jur) = &ctx.mentioned_jurisdiction {
            let jur_upper = jur.to_uppercase();
            for m in matches {
                if m.context
                    .jurisdiction
                    .as_ref()
                    .map(|j| j.to_uppercase() == jur_upper)
                    .unwrap_or(false)
                {
                    return Some(m.id.clone());
                }
            }
        }

        None
    }

    /// Build user-friendly disambiguation prompt
    fn build_disambiguation_prompt(matches: &[EnrichedMatch]) -> String {
        let options: Vec<String> = matches
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, m)| format!("{}. {}", i + 1, m.disambiguation_label))
            .collect();

        format!(
            "Multiple matches found. Which did you mean?\n{}",
            options.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::enrichment::RoleContext;

    fn make_match(id: &str, display: &str, score: f32) -> EnrichedMatch {
        EnrichedMatch {
            id: id.to_string(),
            display: display.to_string(),
            score,
            entity_type: "person".to_string(),
            context: EntityContext::default(),
            disambiguation_label: display.to_string(),
        }
    }

    fn make_match_with_role(
        id: &str,
        display: &str,
        score: f32,
        role: &str,
        cbu: &str,
    ) -> EnrichedMatch {
        EnrichedMatch {
            id: id.to_string(),
            display: display.to_string(),
            score,
            entity_type: "person".to_string(),
            context: EntityContext {
                roles: vec![RoleContext {
                    role: role.to_string(),
                    cbu_name: cbu.to_string(),
                    since: None,
                }],
                ..Default::default()
            },
            disambiguation_label: format!("{} - {} at {}", display, role, cbu),
        }
    }

    fn make_match_with_nationality(
        id: &str,
        display: &str,
        score: f32,
        nat: &str,
    ) -> EnrichedMatch {
        EnrichedMatch {
            id: id.to_string(),
            display: display.to_string(),
            score,
            entity_type: "person".to_string(),
            context: EntityContext {
                nationality: Some(nat.to_string()),
                ..Default::default()
            },
            disambiguation_label: format!("{} ({})", display, nat),
        }
    }

    #[test]
    fn test_auto_resolve_single_high_score() {
        let matches = vec![make_match("uuid-1", "John Smith", 0.95)];
        let result = ResolutionStrategy::analyze(&matches, None);

        assert_eq!(result.confidence, ResolutionConfidence::High);
        assert!(matches!(result.action, SuggestedAction::AutoResolve { .. }));
    }

    #[test]
    fn test_auto_resolve_clear_winner() {
        let matches = vec![
            make_match("uuid-1", "John Smith", 0.92),
            make_match("uuid-2", "John Smithson", 0.70),
        ];
        let result = ResolutionStrategy::analyze(&matches, None);

        assert_eq!(result.confidence, ResolutionConfidence::High);
        if let SuggestedAction::AutoResolve { match_id } = result.action {
            assert_eq!(match_id, "uuid-1");
        } else {
            panic!("Expected AutoResolve");
        }
    }

    #[test]
    fn test_context_resolution_by_role() {
        let matches = vec![
            make_match_with_role("uuid-1", "John Smith", 0.90, "Shareholder", "Fund A"),
            make_match_with_role("uuid-2", "John Smith", 0.88, "Director", "Fund B"),
        ];

        let ctx = ConversationContext {
            mentioned_roles: vec!["DIRECTOR".to_string()],
            ..Default::default()
        };

        let result = ResolutionStrategy::analyze(&matches, Some(&ctx));

        assert_eq!(result.confidence, ResolutionConfidence::Medium);
        if let SuggestedAction::AutoResolve { match_id } = result.action {
            assert_eq!(match_id, "uuid-2"); // The director
        } else {
            panic!("Expected AutoResolve");
        }
    }

    #[test]
    fn test_context_resolution_by_nationality() {
        let matches = vec![
            make_match_with_nationality("uuid-1", "John Smith", 0.90, "US"),
            make_match_with_nationality("uuid-2", "John Smith", 0.88, "GB"),
        ];

        let ctx = ConversationContext {
            mentioned_nationality: Some("GB".to_string()),
            ..Default::default()
        };

        let result = ResolutionStrategy::analyze(&matches, Some(&ctx));

        assert_eq!(result.confidence, ResolutionConfidence::Medium);
        if let SuggestedAction::AutoResolve { match_id } = result.action {
            assert_eq!(match_id, "uuid-2"); // The British one
        } else {
            panic!("Expected AutoResolve");
        }
    }

    #[test]
    fn test_disambiguation_close_scores() {
        let matches = vec![
            make_match("uuid-1", "John Smith", 0.85),
            make_match("uuid-2", "John Smith", 0.83),
        ];
        let result = ResolutionStrategy::analyze(&matches, None);

        assert_eq!(result.confidence, ResolutionConfidence::Low);
        assert!(matches!(result.action, SuggestedAction::AskUser));
        assert!(result.prompt.is_some());
    }

    #[test]
    fn test_suggest_create_no_matches() {
        let matches: Vec<EnrichedMatch> = vec![];
        let result = ResolutionStrategy::analyze(&matches, None);

        assert_eq!(result.confidence, ResolutionConfidence::None);
        assert!(matches!(result.action, SuggestedAction::SuggestCreate));
    }

    #[test]
    fn test_suggest_create_low_scores() {
        let matches = vec![
            make_match("uuid-1", "Jon Smythe", 0.45),
            make_match("uuid-2", "Johan Schmidt", 0.40),
        ];
        let result = ResolutionStrategy::analyze(&matches, None);

        assert_eq!(result.confidence, ResolutionConfidence::None);
        assert!(matches!(result.action, SuggestedAction::SuggestCreate));
    }

    #[test]
    fn test_context_resolution_by_cbu() {
        let matches = vec![
            make_match_with_role("uuid-1", "John Smith", 0.90, "Director", "Apex Fund"),
            make_match_with_role("uuid-2", "John Smith", 0.88, "Director", "Beta Holdings"),
        ];

        let ctx = ConversationContext {
            mentioned_cbu: Some("Beta".to_string()),
            ..Default::default()
        };

        let result = ResolutionStrategy::analyze(&matches, Some(&ctx));

        assert_eq!(result.confidence, ResolutionConfidence::Medium);
        if let SuggestedAction::AutoResolve { match_id } = result.action {
            assert_eq!(match_id, "uuid-2"); // The one at Beta
        } else {
            panic!("Expected AutoResolve");
        }
    }
}
