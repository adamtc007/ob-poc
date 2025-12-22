//! Token types for the lexicon-based intent parser.
//!
//! This module defines the token types produced by the tokenizer.
//! Tokens are classified by looking up words against a dictionary
//! rather than using regex pattern matching.

use std::fmt;

/// A token produced by the tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The original text from the input.
    pub text: String,

    /// Normalized/canonical form (lowercase, trimmed).
    pub normalized: String,

    /// The classified token type.
    pub token_type: TokenType,

    /// Byte offset in the original input.
    pub span: (usize, usize),

    /// Where this token classification came from.
    pub source: TokenSource,

    /// Resolved entity ID if this is an entity reference.
    pub resolved_id: Option<String>,

    /// Confidence score (0.0-1.0) for fuzzy matches.
    pub confidence: f32,
}

impl Token {
    /// Create a new token with the given properties.
    pub fn new(
        text: impl Into<String>,
        normalized: impl Into<String>,
        token_type: TokenType,
        span: (usize, usize),
        source: TokenSource,
    ) -> Self {
        Self {
            text: text.into(),
            normalized: normalized.into(),
            token_type,
            span,
            source,
            resolved_id: None,
            confidence: 1.0,
        }
    }

    /// Create an unknown token.
    pub fn unknown(text: impl Into<String>, span: (usize, usize)) -> Self {
        let text = text.into();
        let normalized = text.to_lowercase();
        Self {
            text,
            normalized,
            token_type: TokenType::Unknown,
            span,
            source: TokenSource::Unmatched,
            resolved_id: None,
            confidence: 0.0,
        }
    }

    /// Check if this token is a verb.
    pub fn is_verb(&self) -> bool {
        matches!(self.token_type, TokenType::Verb(_))
    }

    /// Check if this token is an entity reference.
    pub fn is_entity(&self) -> bool {
        matches!(self.token_type, TokenType::Entity(_))
    }

    /// Check if this token is resolved (has an ID).
    pub fn is_resolved(&self) -> bool {
        self.resolved_id.is_some()
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{:?}", self.text, self.token_type)
    }
}

/// Classification of a token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenType {
    /// Action verb (add, create, establish, etc.)
    Verb(VerbClass),

    /// Entity reference (CBU, person, company, etc.)
    Entity(EntityClass),

    /// Product code (CUSTODY, FUND_ACCOUNTING, etc.)
    Product,

    /// Instrument type (IRS, CDS, EQUITY, BOND, etc.)
    Instrument,

    /// Market identifier (XNYS, XLON, etc.)
    Market,

    /// Currency code (USD, EUR, GBP, etc.)
    Currency,

    /// Role identifier (DIRECTOR, UBO, COUNTERPARTY, etc.)
    Role,

    /// CSA type (VM, IM, TWO_WAY, etc.)
    CsaType,

    /// Governing law (NY_LAW, ENGLISH_LAW, etc.)
    Law,

    /// Preposition (to, for, as, with, under, etc.)
    Prep(PrepType),

    /// Conjunction (and, or, but)
    Conj,

    /// Article (a, an, the)
    Article,

    /// Pronoun (it, them, their, this, etc.)
    Pronoun,

    /// Number (literal or word form)
    Number(NumberType),

    /// Modifier/adjective (new, existing, bilateral, etc.)
    Modifier(ModifierType),

    /// Punctuation
    Punct,

    /// Unrecognized token
    Unknown,
}

impl TokenType {
    /// Check if this is a content word (vs function word).
    pub fn is_content(&self) -> bool {
        matches!(
            self,
            TokenType::Verb(_)
                | TokenType::Entity(_)
                | TokenType::Product
                | TokenType::Instrument
                | TokenType::Market
                | TokenType::Currency
                | TokenType::Role
                | TokenType::CsaType
                | TokenType::Law
        )
    }

    /// Check if this is a function word.
    pub fn is_function(&self) -> bool {
        matches!(
            self,
            TokenType::Prep(_) | TokenType::Conj | TokenType::Article | TokenType::Pronoun
        )
    }
}

/// Classification of verbs by their semantic class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerbClass {
    /// Create/add/establish - brings something into existence
    Create,

    /// Update/modify/change - modifies existing entity
    Update,

    /// Delete/remove/revoke - removes entity or relationship
    Delete,

    /// Query/list/show/get - retrieves information
    Query,

    /// Link/assign/associate - creates relationship between entities
    Link,

    /// Unlink/remove relationship
    Unlink,

    /// Provision/activate - enables a service or resource
    Provision,

    /// Trade/execute - transaction-related actions
    Trade,
}

impl VerbClass {
    /// Get the DSL verb prefix for this class.
    pub fn verb_prefix(&self) -> &'static str {
        match self {
            VerbClass::Create => "create",
            VerbClass::Update => "update",
            VerbClass::Delete => "delete",
            VerbClass::Query => "list",
            VerbClass::Link => "assign",
            VerbClass::Unlink => "remove",
            VerbClass::Provision => "provision",
            VerbClass::Trade => "execute",
        }
    }
}

/// Classification of entity references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityClass {
    /// Client Business Unit
    Cbu,

    /// Natural person
    Person,

    /// Legal entity (company, fund, etc.)
    LegalEntity,

    /// Counterparty (OTC derivatives)
    Counterparty,

    /// ISDA master agreement
    Isda,

    /// CSA (Credit Support Annex)
    Csa,

    /// Product reference
    Product,

    /// Service reference
    Service,

    /// Generic entity (resolved from context)
    Generic,
}

impl EntityClass {
    /// Get the entity type code for this class.
    pub fn type_code(&self) -> &'static str {
        match self {
            EntityClass::Cbu => "cbu",
            EntityClass::Person => "proper_person",
            EntityClass::LegalEntity => "limited_company",
            EntityClass::Counterparty => "counterparty",
            EntityClass::Isda => "isda",
            EntityClass::Csa => "csa",
            EntityClass::Product => "product",
            EntityClass::Service => "service",
            EntityClass::Generic => "entity",
        }
    }
}

/// Types of prepositions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrepType {
    /// "to" - direction, recipient
    To,

    /// "for" - beneficiary, purpose
    For,

    /// "as" - role assignment
    As,

    /// "with" - accompaniment, attribute
    With,

    /// "under" - governance (under NY law)
    Under,

    /// "from" - source
    From,

    /// "in" - location, containment
    In,

    /// "on" - temporal, surface
    On,

    /// "by" - agent, means
    By,

    /// "of" - possession, partitive
    Of,

    /// "at" - location, rate
    At,
}

/// Types of number tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberType {
    /// Integer value
    Integer(i64),

    /// Decimal value
    Decimal(f64),

    /// Percentage
    Percentage(f64),

    /// Currency amount
    Amount { value: f64, currency: String },
}

impl Eq for NumberType {}

impl std::hash::Hash for NumberType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            NumberType::Integer(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            NumberType::Decimal(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
            NumberType::Percentage(v) => {
                2u8.hash(state);
                v.to_bits().hash(state);
            }
            NumberType::Amount { value, currency } => {
                3u8.hash(state);
                value.to_bits().hash(state);
                currency.hash(state);
            }
        }
    }
}

/// Types of modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierType {
    /// Temporal (new, existing, current)
    Temporal,

    /// Bilateral/multilateral
    Lateral,

    /// OTC-specific (collateralized, margined)
    OtcQualifier,

    /// Risk-related (high, low, enhanced)
    Risk,

    /// Status (active, pending, suspended)
    Status,
}

/// Where a token classification came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenSource {
    /// Matched in static lexicon YAML.
    Lexicon,

    /// Resolved via EntityGateway (database lookup).
    EntityGateway,

    /// Resolved via session context (coreference).
    SessionContext,

    /// Inferred from structure/position.
    Inferred,

    /// No match found.
    Unmatched,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_creation() {
        let token = Token::new(
            "Goldman Sachs",
            "goldman sachs",
            TokenType::Entity(EntityClass::Counterparty),
            (0, 13),
            TokenSource::EntityGateway,
        );

        assert!(token.is_entity());
        assert!(!token.is_verb());
        assert_eq!(token.confidence, 1.0);
    }

    #[test]
    fn test_unknown_token() {
        let token = Token::unknown("xyzzy", (0, 5));

        assert_eq!(token.token_type, TokenType::Unknown);
        assert_eq!(token.confidence, 0.0);
        assert_eq!(token.source, TokenSource::Unmatched);
    }

    #[test]
    fn test_token_type_classification() {
        assert!(TokenType::Verb(VerbClass::Create).is_content());
        assert!(TokenType::Entity(EntityClass::Cbu).is_content());
        assert!(TokenType::Prep(PrepType::To).is_function());
        assert!(TokenType::Article.is_function());
        assert!(!TokenType::Unknown.is_content());
    }
}
