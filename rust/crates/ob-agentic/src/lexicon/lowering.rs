//! AST Lowering with Entity Type Inference
//!
//! This module performs type inference on the token stream, similar to
//! Rust's HIR lowering phase. The key operation is inferring concrete
//! EntityClass types for untyped entities based on context.
//!
//! ## Type Inference Rules
//!
//! | Pattern | Inference | Rationale |
//! |---------|-----------|-----------|
//! | `counterparty <name>` | name : Counterparty | Type indicator precedes name |
//! | `<name> as counterparty` | name : Counterparty | Post-position type indicator |
//! | `<name> under <law>` | name : Counterparty | OTC context (law = ISDA domain) |
//! | `<name> for IRS/CDS` | name : Counterparty | OTC instruments imply counterparty |
//! | `assign <name> as <role>` | name : Person | Role assignment implies person |
//! | `<name> to <cbu>` | name : Person | Assignment target implies person |
//!
//! ## Lowering Operations
//!
//! 1. **Type indicator fusion**: `counterparty Barclays` → single typed token
//! 2. **Post-position inference**: `Barclays as counterparty` → typed token
//! 3. **Contextual inference**: `Barclays under NY law` → infer from OTC context

use super::tokens::{EntityClass, PrepType, Token, TokenSource, TokenType, VerbClass};

/// Type inference context accumulated while scanning tokens.
#[derive(Debug, Default)]
struct InferenceContext {
    /// We've seen OTC-related tokens (law, OTC instruments)
    otc_context: bool,
    /// We've seen a role assignment verb (assign, make)
    role_context: bool,
    /// We've seen a CBU reference (implies person assignment)
    cbu_target: bool,
}

/// Lower a token stream with entity type inference.
///
/// This is the main entry point. It:
/// 1. Scans for contextual clues (law, instruments, verbs)
/// 2. Applies type inference rules to untyped entities
/// 3. Fuses type-indicator + name sequences
pub fn lower_tokens(tokens: &[Token]) -> Vec<Token> {
    if tokens.is_empty() {
        return vec![];
    }

    // Phase 1: Scan for contextual type clues
    let ctx = build_inference_context(tokens);

    // Phase 2: Apply type inference and fusion
    lower_with_context(tokens, &ctx)
}

/// Scan tokens to build inference context.
fn build_inference_context(tokens: &[Token]) -> InferenceContext {
    let mut ctx = InferenceContext::default();

    for token in tokens {
        match &token.token_type {
            // OTC context markers
            TokenType::Law => ctx.otc_context = true,
            TokenType::Instrument => {
                // Check if OTC instrument
                let norm = token.normalized.to_uppercase();
                if matches!(
                    norm.as_str(),
                    "IRS" | "CDS" | "FX" | "SWAP" | "SWAPTION" | "FRA"
                ) {
                    ctx.otc_context = true;
                }
            }
            TokenType::Entity(EntityClass::Isda | EntityClass::Csa) => {
                ctx.otc_context = true;
            }

            // Role assignment context
            TokenType::Verb(VerbClass::Link) => ctx.role_context = true,
            TokenType::Role => ctx.role_context = true,

            // CBU target context
            TokenType::Entity(EntityClass::Cbu) => ctx.cbu_target = true,
            TokenType::Prep(PrepType::To) => {
                // "to" often precedes CBU in role assignment
            }

            _ => {}
        }
    }

    ctx
}

/// Apply type inference and produce lowered token stream.
fn lower_with_context(tokens: &[Token], ctx: &InferenceContext) -> Vec<Token> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        let token = &tokens[i];

        match &token.token_type {
            // Rule 1: Type indicator followed by name → fuse with type
            // e.g., "counterparty Barclays" → Entity(Counterparty, "Barclays")
            // Also handles multi-word: "counterparty Deutsche Bank AG" → "Deutsche Bank AG"
            TokenType::Entity(
                indicator_class
                @ (EntityClass::Counterparty | EntityClass::Isda | EntityClass::Csa),
            ) => {
                if let Some((mut name_token, skip)) = find_following_name(&tokens[i + 1..]) {
                    // Fuse: update the name token with indicator's type
                    name_token.token_type = TokenType::Entity(*indicator_class);
                    result.push(name_token);
                    i += 1 + skip; // skip indicator + noise + name(s)
                    continue;
                }
                // No following name, pass through
                result.push(token.clone());
            }

            // Rule 2 & 3: Untyped entity → infer type from context
            TokenType::Entity(EntityClass::Generic) | TokenType::Unknown => {
                let inferred = infer_entity_type(tokens, i, ctx);

                // Check for "as <type>" pattern (Rule 2)
                if let Some((type_class, skip)) = check_as_type_pattern(&tokens[i..]) {
                    let typed = Token {
                        text: token.text.clone(),
                        normalized: token.normalized.clone(),
                        token_type: TokenType::Entity(type_class),
                        span: token.span,
                        source: TokenSource::Lowering,
                        resolved_id: token.resolved_id.clone(),
                        confidence: token.confidence,
                    };
                    result.push(typed);
                    i += skip; // skip name + as + type-indicator
                    continue;
                }

                // Apply contextual inference if we got one
                if let Some(entity_class) = inferred {
                    let typed = Token {
                        text: token.text.clone(),
                        normalized: token.normalized.clone(),
                        token_type: TokenType::Entity(entity_class),
                        span: token.span,
                        source: TokenSource::Lowering,
                        resolved_id: token.resolved_id.clone(),
                        confidence: token.confidence * 0.9, // slightly lower for inferred
                    };
                    result.push(typed);
                } else {
                    // No inference possible, pass through
                    result.push(token.clone());
                }
            }

            // All other tokens pass through unchanged
            _ => {
                result.push(token.clone());
            }
        }

        i += 1;
    }

    result
}

/// Find a name token (possibly multi-word) following the current position.
/// Returns (fused_name, entity_class, tokens_to_skip) or None.
///
/// For multi-word names like "Deutsche Bank AG", this fuses them into a single string.
fn find_following_name(tokens: &[Token]) -> Option<(Token, usize)> {
    let mut skip = 0;
    let mut name_parts: Vec<&str> = Vec::new();
    let mut first_entity_class: Option<EntityClass> = None;
    let mut name_start_idx = 0;
    let mut in_name = false;

    for (idx, token) in tokens.iter().enumerate().take(8) {
        match &token.token_type {
            // Entity tokens that could be part of a name
            TokenType::Entity(class) => {
                if !in_name {
                    in_name = true;
                    name_start_idx = idx;
                    first_entity_class = Some(*class);
                }
                name_parts.push(&token.text);
                skip = idx + 1;
            }

            // Unknown tokens that look like name parts (capitalized)
            TokenType::Unknown if is_likely_proper_name(&token.text) => {
                if !in_name {
                    in_name = true;
                    name_start_idx = idx;
                    first_entity_class = Some(EntityClass::Generic);
                }
                name_parts.push(&token.text);
                skip = idx + 1;
            }

            // Skip noise before the name starts
            TokenType::Article | TokenType::Punct if !in_name => continue,
            TokenType::Verb(_) if !in_name => continue,
            TokenType::Unknown if !in_name => continue,

            // Noise inside a name - stop collecting (name ended)
            // But allow certain patterns like "Bank of America" where "of" might be Unknown
            TokenType::Unknown if in_name && token.text.to_lowercase() == "of" => {
                name_parts.push(&token.text);
                skip = idx + 1;
            }

            // Stop at structural tokens
            TokenType::Prep(_) | TokenType::Law | TokenType::Instrument => break,

            // Stop collecting if we hit other tokens while in a name
            _ if in_name => break,

            // Stop at other tokens
            _ => break,
        }
    }

    if name_parts.is_empty() {
        return None;
    }

    // Fuse the name parts into a single token
    let fused_name = name_parts.join(" ");
    let entity_class = first_entity_class.unwrap_or(EntityClass::Generic);

    let fused_token = Token {
        text: fused_name.clone(),
        normalized: fused_name.to_lowercase(),
        token_type: TokenType::Entity(entity_class),
        span: tokens.get(name_start_idx).map_or((0, 0), |t| t.span),
        source: TokenSource::Lowering,
        resolved_id: None,
        confidence: 0.95, // slightly lower for fused multi-word
    };

    Some((fused_token, skip))
}

/// Check for "as <type-indicator>" pattern after current token.
/// Returns (EntityClass, tokens_to_skip) if found.
fn check_as_type_pattern(tokens: &[Token]) -> Option<(EntityClass, usize)> {
    if tokens.len() < 3 {
        return None;
    }

    // Look for "as" within next few tokens
    let mut as_idx = None;
    for (i, token) in tokens.iter().enumerate().skip(1).take(3) {
        if matches!(&token.token_type, TokenType::Prep(PrepType::As)) {
            as_idx = Some(i);
            break;
        }
        // Skip only articles/noise
        if !matches!(&token.token_type, TokenType::Article | TokenType::Unknown) {
            break;
        }
    }

    let as_idx = as_idx?;

    // Look for type indicator after "as"
    for (i, token) in tokens.iter().enumerate().skip(as_idx + 1).take(2) {
        match &token.token_type {
            TokenType::Entity(EntityClass::Counterparty) => {
                return Some((EntityClass::Counterparty, i + 1));
            }
            TokenType::Role if token.normalized == "counterparty" => {
                return Some((EntityClass::Counterparty, i + 1));
            }
            TokenType::Article => continue,
            _ => break,
        }
    }

    None
}

/// Infer entity type from surrounding context.
fn infer_entity_type(tokens: &[Token], pos: usize, ctx: &InferenceContext) -> Option<EntityClass> {
    let token = &tokens[pos];

    // Only infer for untyped tokens that look like names
    if !matches!(
        &token.token_type,
        TokenType::Entity(EntityClass::Generic) | TokenType::Unknown
    ) {
        return None;
    }

    if !is_likely_proper_name(&token.text) {
        return None;
    }

    // Rule 3a: OTC context → Counterparty
    // If we've seen law, ISDA, CSA, or OTC instruments, untyped names are likely counterparties
    if ctx.otc_context {
        return Some(EntityClass::Counterparty);
    }

    // Rule 3b: Check local context (nearby tokens)
    // Look ahead for law or OTC instruments
    for following in tokens.iter().skip(pos + 1).take(5) {
        match &following.token_type {
            TokenType::Law => return Some(EntityClass::Counterparty),
            TokenType::Prep(PrepType::Under) => continue, // "under" often precedes law
            TokenType::Instrument => {
                let norm = following.normalized.to_uppercase();
                if matches!(norm.as_str(), "IRS" | "CDS" | "FX" | "SWAP") {
                    return Some(EntityClass::Counterparty);
                }
            }
            _ => {}
        }
    }

    // Rule 4: Role assignment context → Person
    if ctx.role_context && !ctx.otc_context {
        // Check if this name is being assigned a role
        for following in tokens.iter().skip(pos + 1).take(4) {
            if matches!(&following.token_type, TokenType::Role) {
                return Some(EntityClass::Person);
            }
            if matches!(&following.token_type, TokenType::Prep(PrepType::As)) {
                continue;
            }
        }
    }

    None
}

/// Check if text looks like a proper name (capitalized, not a common word).
fn is_likely_proper_name(text: &str) -> bool {
    // Must start with uppercase
    let first_char = text.chars().next();
    if !first_char.is_some_and(|c| c.is_uppercase()) {
        return false;
    }

    // Must not be a common word
    !is_common_word(text)
}

/// Check if word is a common English word (not a proper name).
fn is_common_word(text: &str) -> bool {
    matches!(
        text.to_lowercase().as_str(),
        "the"
            | "a"
            | "an"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "have"
            | "has"
            | "had"
            | "do"
            | "does"
            | "did"
            | "will"
            | "would"
            | "could"
            | "should"
            | "may"
            | "might"
            | "must"
            | "shall"
            | "this"
            | "that"
            | "these"
            | "those"
            | "it"
            | "its"
            | "what"
            | "which"
            | "who"
            | "whom"
            | "whose"
            | "where"
            | "when"
            | "why"
            | "how"
            | "all"
            | "each"
            | "every"
            | "both"
            | "few"
            | "more"
            | "most"
            | "other"
            | "some"
            | "such"
            | "no"
            | "nor"
            | "not"
            | "only"
            | "same"
            | "so"
            | "than"
            | "too"
            | "very"
            | "just"
            | "also"
            | "now"
            | "here"
            | "there"
            | "new"
            | "old"
            | "first"
            | "last"
            | "long"
            | "great"
            | "little"
            | "own"
            | "right"
            | "big"
            | "high"
            | "different"
            | "small"
            | "large"
            | "next"
            | "early"
            | "young"
            | "important"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_token(text: &str, token_type: TokenType) -> Token {
        Token {
            text: text.to_string(),
            normalized: text.to_lowercase(),
            token_type,
            span: (0, text.len()),
            source: TokenSource::Lexicon,
            resolved_id: None,
            confidence: 1.0,
        }
    }

    #[test]
    fn test_type_indicator_fusion() {
        // "counterparty Barclays" → Entity(Counterparty, "Barclays")
        let tokens = vec![
            make_token("counterparty", TokenType::Entity(EntityClass::Counterparty)),
            make_token("Barclays", TokenType::Entity(EntityClass::Generic)),
        ];

        let lowered = lower_tokens(&tokens);

        assert_eq!(lowered.len(), 1);
        assert_eq!(lowered[0].text, "Barclays");
        assert!(matches!(
            lowered[0].token_type,
            TokenType::Entity(EntityClass::Counterparty)
        ));
    }

    #[test]
    fn test_as_type_pattern() {
        // "Barclays as counterparty" → Entity(Counterparty, "Barclays")
        let tokens = vec![
            make_token("Barclays", TokenType::Entity(EntityClass::Generic)),
            make_token("as", TokenType::Prep(PrepType::As)),
            make_token("counterparty", TokenType::Entity(EntityClass::Counterparty)),
        ];

        let lowered = lower_tokens(&tokens);

        assert_eq!(lowered.len(), 1);
        assert_eq!(lowered[0].text, "Barclays");
        assert!(matches!(
            lowered[0].token_type,
            TokenType::Entity(EntityClass::Counterparty)
        ));
    }

    #[test]
    fn test_contextual_inference_from_law() {
        // "Barclays under NY law" → infer Counterparty from OTC context
        let tokens = vec![
            make_token("Barclays", TokenType::Entity(EntityClass::Generic)),
            make_token("under", TokenType::Prep(PrepType::Under)),
            make_token("NY law", TokenType::Law),
        ];

        let lowered = lower_tokens(&tokens);

        assert_eq!(lowered.len(), 3);
        assert_eq!(lowered[0].text, "Barclays");
        assert!(
            matches!(
                lowered[0].token_type,
                TokenType::Entity(EntityClass::Counterparty)
            ),
            "Expected Counterparty, got {:?}",
            lowered[0].token_type
        );
    }

    #[test]
    fn test_full_create_counterparty_under_law() {
        // "create counterparty Barclays under NY law"
        let tokens = vec![
            make_token("create", TokenType::Verb(VerbClass::Create)),
            make_token("counterparty", TokenType::Entity(EntityClass::Counterparty)),
            make_token("Barclays", TokenType::Entity(EntityClass::Generic)),
            make_token("under", TokenType::Prep(PrepType::Under)),
            make_token("NY law", TokenType::Law),
        ];

        let lowered = lower_tokens(&tokens);

        // Should be: [create, Barclays(Counterparty), under, NY law]
        assert_eq!(lowered.len(), 4, "Expected 4 tokens, got {:?}", lowered);

        assert!(matches!(
            lowered[0].token_type,
            TokenType::Verb(VerbClass::Create)
        ));

        assert_eq!(lowered[1].text, "Barclays");
        assert!(
            matches!(
                lowered[1].token_type,
                TokenType::Entity(EntityClass::Counterparty)
            ),
            "Expected Barclays as Counterparty, got {:?}",
            lowered[1].token_type
        );

        assert!(matches!(
            lowered[2].token_type,
            TokenType::Prep(PrepType::Under)
        ));
        assert!(matches!(lowered[3].token_type, TokenType::Law));
    }

    #[test]
    fn test_preserves_already_typed_entities() {
        // Already-typed entities should pass through unchanged
        let tokens = vec![
            make_token("add", TokenType::Verb(VerbClass::Create)),
            make_token(
                "Goldman Sachs",
                TokenType::Entity(EntityClass::Counterparty),
            ),
        ];

        let lowered = lower_tokens(&tokens);

        assert_eq!(lowered.len(), 2);
        assert!(matches!(
            lowered[0].token_type,
            TokenType::Verb(VerbClass::Create)
        ));
        assert_eq!(lowered[1].text, "Goldman Sachs");
        assert!(matches!(
            lowered[1].token_type,
            TokenType::Entity(EntityClass::Counterparty)
        ));
    }

    #[test]
    fn test_role_context_infers_person() {
        // "assign Smith as director" → Smith : Person
        let tokens = vec![
            make_token("assign", TokenType::Verb(VerbClass::Link)),
            make_token("Smith", TokenType::Entity(EntityClass::Generic)),
            make_token("as", TokenType::Prep(PrepType::As)),
            make_token("director", TokenType::Role),
        ];

        let lowered = lower_tokens(&tokens);

        // Smith should be inferred as Person
        let smith = lowered.iter().find(|t| t.text == "Smith");
        assert!(smith.is_some());
        assert!(
            matches!(
                smith.unwrap().token_type,
                TokenType::Entity(EntityClass::Person)
            ),
            "Expected Person, got {:?}",
            smith.unwrap().token_type
        );
    }
}
