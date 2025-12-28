//! Tokenizer that classifies input words against the lexicon and EntityGateway.
//!
//! The tokenizer performs multi-pass classification:
//! 1. Split input into candidate tokens (whitespace, punctuation boundaries)
//! 2. Attempt longest-match phrase lookup in lexicon
//! 3. Fall back to single-word lexicon lookup
//! 4. Attempt entity resolution via EntityResolver trait
//! 5. Mark remaining tokens as Unknown

use std::sync::Arc;

use async_trait::async_trait;

use super::loader::{Lexicon, LifecycleDomain};
use super::tokens::{EntityClass, Token, TokenSource, TokenType};

/// Result of entity resolution.
#[derive(Debug, Clone)]
pub struct ResolvedEntity {
    /// The resolved entity ID (UUID as string).
    pub id: String,

    /// The canonical name of the entity.
    pub name: String,

    /// The entity type (for TokenType classification).
    pub entity_type: String,

    /// Confidence score (0.0-1.0).
    pub confidence: f32,
}

/// Trait for resolving entity names to IDs.
///
/// Implementations can use different backends:
/// - `DatabaseEntityResolver`: Uses EntityGateway gRPC
/// - `MockEntityResolver`: For testing
/// - `SessionEntityResolver`: Resolves from session context (coreference)
#[async_trait]
pub trait EntityResolver: Send + Sync {
    /// Attempt to resolve a text string to an entity.
    ///
    /// Returns None if no match found with sufficient confidence.
    async fn resolve(&self, text: &str, hint: Option<&str>) -> Option<ResolvedEntity>;

    /// Batch resolve multiple texts (for efficiency).
    async fn resolve_batch(
        &self,
        texts: &[&str],
        hint: Option<&str>,
    ) -> Vec<Option<ResolvedEntity>> {
        // Default implementation: sequential resolution
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.resolve(text, hint).await);
        }
        results
    }
}

/// Session context for coreference resolution.
#[derive(Debug, Clone, Default)]
pub struct SessionSalience {
    /// Recently mentioned entities (most recent first).
    pub recent_entities: Vec<SalientEntity>,

    /// Current CBU context (if any).
    pub current_cbu: Option<SalientEntity>,

    /// Current counterparty context (if any).
    pub current_counterparty: Option<SalientEntity>,
}

/// An entity that was recently mentioned in conversation.
#[derive(Debug, Clone)]
pub struct SalientEntity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub mention_count: usize,
}

impl SessionSalience {
    /// Add an entity to the salience tracking.
    pub fn add_entity(&mut self, id: String, name: String, entity_type: String) {
        // Check if already present
        if let Some(existing) = self.recent_entities.iter_mut().find(|e| e.id == id) {
            existing.mention_count += 1;
            return;
        }

        // Add new entity at front
        self.recent_entities.insert(
            0,
            SalientEntity {
                id,
                name,
                entity_type,
                mention_count: 1,
            },
        );

        // Keep only most recent 10
        self.recent_entities.truncate(10);
    }

    /// Get the most salient entity of a given type.
    pub fn get_salient(&self, entity_type: &str) -> Option<&SalientEntity> {
        self.recent_entities
            .iter()
            .find(|e| e.entity_type == entity_type)
    }

    /// Resolve a pronoun to the most likely referent.
    pub fn resolve_pronoun(&self, pronoun: &str) -> Option<&SalientEntity> {
        let pronoun_lower = pronoun.to_lowercase();

        match pronoun_lower.as_str() {
            // "it", "this", "that" → most recent entity
            "it" | "its" | "this" | "that" => self.recent_entities.first(),

            // "them", "they" → could be plural or generic
            "them" | "they" | "their" => self.recent_entities.first(),

            _ => None,
        }
    }
}

/// The main tokenizer.
pub struct Tokenizer {
    /// The lexicon for static lookups.
    lexicon: Arc<Lexicon>,

    /// Entity resolver for dynamic lookups.
    entity_resolver: Option<Arc<dyn EntityResolver>>,

    /// Session context for coreference.
    salience: SessionSalience,

    /// Minimum confidence for entity resolution.
    min_entity_confidence: f32,
}

impl Tokenizer {
    /// Create a new tokenizer with the given lexicon.
    pub fn new(lexicon: Arc<Lexicon>) -> Self {
        Self {
            lexicon,
            entity_resolver: None,
            salience: SessionSalience::default(),
            min_entity_confidence: 0.7,
        }
    }

    /// Set the entity resolver.
    pub fn with_entity_resolver(mut self, resolver: Arc<dyn EntityResolver>) -> Self {
        self.entity_resolver = Some(resolver);
        self
    }

    /// Set session salience for coreference resolution.
    pub fn with_salience(mut self, salience: SessionSalience) -> Self {
        self.salience = salience;
        self
    }

    /// Set minimum confidence for entity resolution.
    pub fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_entity_confidence = confidence;
        self
    }

    /// Tokenize input text into a stream of classified tokens.
    pub async fn tokenize(&self, input: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut remaining = input;
        let mut offset = 0usize;

        while !remaining.is_empty() {
            // Skip leading whitespace
            let trimmed = remaining.trim_start();
            let ws_len = remaining.len() - trimmed.len();
            offset += ws_len;
            remaining = trimmed;

            if remaining.is_empty() {
                break;
            }

            // Try longest match first (multi-word phrases)
            if let Some((token, consumed)) = self.try_phrase_match(remaining, offset).await {
                tokens.push(token);
                remaining = &remaining[consumed..];
                offset += consumed;
                continue;
            }

            // Extract next word
            let (word, rest) = self.extract_word(remaining);
            let word_len = word.len();

            if word.is_empty() {
                // Handle punctuation
                if let Some(c) = remaining.chars().next() {
                    let c_len = c.len_utf8();
                    tokens.push(Token::new(
                        c.to_string(),
                        c.to_string(),
                        TokenType::Punct,
                        (offset, offset + c_len),
                        TokenSource::Lexicon,
                    ));
                    remaining = &remaining[c_len..];
                    offset += c_len;
                }
                continue;
            }

            // Try lexicon lookup
            if let Some(entry) = self.lexicon.lookup_word(word) {
                tokens.push(Token {
                    text: word.to_string(),
                    normalized: word.to_lowercase(),
                    token_type: entry.token_type.clone(),
                    span: (offset, offset + word_len),
                    source: TokenSource::Lexicon,
                    resolved_id: None,
                    confidence: 1.0,
                });
            }
            // Try pronoun resolution
            else if let Some(token) = self.try_pronoun_resolution(word, offset) {
                tokens.push(token);
            }
            // Try entity resolution
            else if let Some(token) = self.try_entity_resolution(word, offset).await {
                tokens.push(token);
            }
            // Unknown token
            else {
                tokens.push(Token::unknown(
                    word.to_string(),
                    (offset, offset + word_len),
                ));
            }

            remaining = rest;
            offset += word_len;
        }

        tokens
    }

    /// Try to match a multi-word phrase from the lexicon.
    async fn try_phrase_match(&self, input: &str, offset: usize) -> Option<(Token, usize)> {
        // Try progressively shorter phrases (longest match first)
        let words: Vec<&str> = input.split_whitespace().take(5).collect();

        for len in (2..=words.len()).rev() {
            let phrase = words[..len].join(" ");
            let phrase_len = phrase.len();

            // Check if input actually starts with this phrase
            if !input.to_lowercase().starts_with(&phrase.to_lowercase()) {
                continue;
            }

            if let Some(entry) = self.lexicon.lookup_phrase(&phrase) {
                return Some((
                    Token {
                        text: phrase.clone(),
                        normalized: phrase.to_lowercase(),
                        token_type: entry.token_type.clone(),
                        span: (offset, offset + phrase_len),
                        source: TokenSource::Lexicon,
                        resolved_id: None,
                        confidence: 1.0,
                    },
                    phrase_len,
                ));
            }
        }

        None
    }

    /// Extract the next word from input (stops at whitespace or punctuation).
    fn extract_word<'a>(&self, input: &'a str) -> (&'a str, &'a str) {
        let mut end = 0;

        for (i, c) in input.char_indices() {
            if c.is_whitespace() || is_punctuation(c) {
                if i == 0 {
                    // Input starts with punctuation
                    return ("", input);
                }
                return (&input[..i], &input[i..]);
            }
            end = i + c.len_utf8();
        }

        (&input[..end], "")
    }

    /// Try to resolve a pronoun using session salience.
    fn try_pronoun_resolution(&self, word: &str, offset: usize) -> Option<Token> {
        let word_lower = word.to_lowercase();

        // Check if it's a known pronoun
        if !matches!(
            word_lower.as_str(),
            "it" | "its" | "this" | "that" | "them" | "they" | "their"
        ) {
            return None;
        }

        // Try to resolve from salience
        if let Some(referent) = self.salience.resolve_pronoun(&word_lower) {
            let entity_class = match referent.entity_type.as_str() {
                "cbu" => EntityClass::Cbu,
                "proper_person" => EntityClass::Person,
                "limited_company" => EntityClass::LegalEntity,
                "counterparty" => EntityClass::Counterparty,
                _ => EntityClass::Generic,
            };

            return Some(Token {
                text: word.to_string(),
                normalized: word_lower,
                token_type: TokenType::Entity(entity_class),
                span: (offset, offset + word.len()),
                source: TokenSource::SessionContext,
                resolved_id: Some(referent.id.clone()),
                confidence: 0.8, // Lower confidence for coreference
            });
        }

        // Return as unresolved pronoun
        Some(Token {
            text: word.to_string(),
            normalized: word_lower,
            token_type: TokenType::Pronoun,
            span: (offset, offset + word.len()),
            source: TokenSource::Lexicon,
            resolved_id: None,
            confidence: 1.0,
        })
    }

    /// Try to resolve a word as an entity via EntityGateway.
    async fn try_entity_resolution(&self, word: &str, offset: usize) -> Option<Token> {
        let resolver = self.entity_resolver.as_ref()?;

        let resolved = resolver.resolve(word, None).await?;

        if resolved.confidence < self.min_entity_confidence {
            return None;
        }

        let entity_class = match resolved.entity_type.as_str() {
            "cbu" => EntityClass::Cbu,
            "proper_person" => EntityClass::Person,
            "limited_company" | "legal_entity" => EntityClass::LegalEntity,
            "counterparty" => EntityClass::Counterparty,
            "isda" => EntityClass::Isda,
            "csa" => EntityClass::Csa,
            "product" => EntityClass::Product,
            "service" => EntityClass::Service,
            _ => EntityClass::Generic,
        };

        Some(Token {
            text: word.to_string(),
            normalized: word.to_lowercase(),
            token_type: TokenType::Entity(entity_class),
            span: (offset, offset + word.len()),
            source: TokenSource::EntityGateway,
            resolved_id: Some(resolved.id),
            confidence: resolved.confidence,
        })
    }

    /// Detect the lifecycle domain from tokens.
    pub fn detect_domain(&self, tokens: &[Token]) -> Option<LifecycleDomain> {
        let mut otc_score = 0;
        let mut exchange_score = 0;

        for token in tokens {
            match &token.token_type {
                TokenType::Entity(EntityClass::Counterparty)
                | TokenType::Entity(EntityClass::Isda)
                | TokenType::Entity(EntityClass::Csa)
                | TokenType::CsaType
                | TokenType::Law => {
                    otc_score += 2;
                }
                TokenType::Instrument => {
                    // Check domain hint from lexicon
                    if let Some(entry) = self.lexicon.lookup_word(&token.normalized) {
                        match entry.domain_hint {
                            Some(LifecycleDomain::Otc) => otc_score += 2,
                            Some(LifecycleDomain::ExchangeTraded) => exchange_score += 2,
                            None => {}
                        }
                    }
                }
                TokenType::Market => {
                    exchange_score += 2;
                }
                _ => {}
            }
        }

        if otc_score > exchange_score && otc_score > 0 {
            Some(LifecycleDomain::Otc)
        } else if exchange_score > otc_score && exchange_score > 0 {
            Some(LifecycleDomain::ExchangeTraded)
        } else {
            None
        }
    }

    /// Get a reference to the lexicon.
    pub fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    /// Get mutable access to salience (for updates after processing).
    pub fn salience_mut(&mut self) -> &mut SessionSalience {
        &mut self.salience
    }
}

/// Check if a character is punctuation.
fn is_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '-'
    )
}

/// A mock entity resolver for testing.
#[derive(Debug, Default)]
pub struct MockEntityResolver {
    entities: std::collections::HashMap<String, ResolvedEntity>,
}

impl MockEntityResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entity(
        mut self,
        search_key: impl Into<String>,
        id: impl Into<String>,
        name: impl Into<String>,
        entity_type: impl Into<String>,
    ) -> Self {
        self.entities.insert(
            search_key.into().to_lowercase(),
            ResolvedEntity {
                id: id.into(),
                name: name.into(),
                entity_type: entity_type.into(),
                confidence: 0.95,
            },
        );
        self
    }
}

#[async_trait]
impl EntityResolver for MockEntityResolver {
    async fn resolve(&self, text: &str, _hint: Option<&str>) -> Option<ResolvedEntity> {
        self.entities.get(&text.to_lowercase()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lexicon() -> Lexicon {
        use super::super::loader::LexiconConfig;

        let config = LexiconConfig {
            verbs: super::super::loader::VerbsConfig {
                create: vec!["add".to_string(), "create".to_string()],
                link: vec!["assign".to_string()],
                ..Default::default()
            },
            entities: super::super::loader::EntitiesConfig {
                counterparty: vec!["counterparty".to_string()],
                ..Default::default()
            },
            instruments: super::super::loader::InstrumentsConfig {
                otc: vec!["irs".to_string()],
                exchange_traded: vec!["equity".to_string()],
            },
            prepositions: super::super::loader::PrepositionsConfig {
                as_: vec!["as".to_string()],
                for_: vec!["for".to_string()],
                under: vec!["under".to_string()],
                ..Default::default()
            },
            laws: vec![super::super::loader::LawEntry {
                code: "NY_LAW".to_string(),
                aliases: vec!["ny law".to_string()],
            }],
            articles: vec!["a".to_string(), "an".to_string(), "the".to_string()],
            ..Default::default()
        };

        Lexicon::from_config(config).unwrap()
    }

    #[tokio::test]
    async fn test_basic_tokenization() {
        let lexicon = Arc::new(test_lexicon());
        let tokenizer = Tokenizer::new(lexicon);

        let tokens = tokenizer.tokenize("add counterparty").await;

        assert_eq!(tokens.len(), 2);
        assert!(matches!(
            tokens[0].token_type,
            TokenType::Verb(super::super::tokens::VerbClass::Create)
        ));
        assert!(matches!(
            tokens[1].token_type,
            TokenType::Entity(EntityClass::Counterparty)
        ));
    }

    #[tokio::test]
    async fn test_tokenization_with_articles() {
        let lexicon = Arc::new(test_lexicon());
        let tokenizer = Tokenizer::new(lexicon);

        let tokens = tokenizer.tokenize("add a counterparty").await;

        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[1].token_type, TokenType::Article));
    }

    #[tokio::test]
    async fn test_entity_resolution() {
        let lexicon = Arc::new(test_lexicon());
        let resolver = MockEntityResolver::new().with_entity(
            "goldman sachs",
            "uuid-123",
            "Goldman Sachs",
            "counterparty",
        );

        let tokenizer = Tokenizer::new(lexicon).with_entity_resolver(Arc::new(resolver));

        // Note: This won't match "Goldman Sachs" as two separate words
        // Real implementation would need smarter word grouping
        let tokens = tokenizer.tokenize("add Goldman").await;

        // "Goldman" alone won't match, so it's Unknown
        assert_eq!(tokens.len(), 2);
    }

    #[tokio::test]
    async fn test_domain_detection() {
        let lexicon = Arc::new(test_lexicon());
        let tokenizer = Tokenizer::new(lexicon);

        let tokens = tokenizer.tokenize("add counterparty for irs").await;
        let domain = tokenizer.detect_domain(&tokens);

        assert_eq!(domain, Some(LifecycleDomain::Otc));
    }

    #[tokio::test]
    async fn test_phrase_matching() {
        let lexicon = Arc::new(test_lexicon());
        let tokenizer = Tokenizer::new(lexicon);

        let tokens = tokenizer.tokenize("under ny law").await;

        // "ny law" should match as a phrase
        assert!(tokens
            .iter()
            .any(|t| matches!(t.token_type, TokenType::Law)));
    }

    #[test]
    fn test_session_salience() {
        let mut salience = SessionSalience::default();

        salience.add_entity(
            "uuid-1".to_string(),
            "Goldman Sachs".to_string(),
            "counterparty".to_string(),
        );
        salience.add_entity(
            "uuid-2".to_string(),
            "Apex Fund".to_string(),
            "cbu".to_string(),
        );

        // Most recent is Apex Fund
        let referent = salience.resolve_pronoun("it").unwrap();
        assert_eq!(referent.name, "Apex Fund");

        // Get salient by type
        let cp = salience.get_salient("counterparty").unwrap();
        assert_eq!(cp.name, "Goldman Sachs");
    }
}
