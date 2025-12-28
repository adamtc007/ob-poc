//! Lexicon-based tokenizer and intent parser.
//!
//! This module provides a formal grammar approach to intent classification,
//! replacing regex-based pattern matching with:
//!
//! 1. **Lexicon**: YAML-driven dictionary of known terms (verbs, entities, instruments, etc.)
//! 2. **Tokenizer**: Classifies input words against the lexicon and EntityGateway
//! 3. **Parser**: Nom-based grammar parser that builds IntentAst from tokens
//!
//! ## Architecture
//!
//! ```text
//! User Input: "Add Goldman Sachs as counterparty for IRS trades under NY law"
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Tokenizer                                   │
//! │  - Lexicon lookup (verbs, roles, instruments, prepositions)     │
//! │  - EntityGateway lookup (counterparties, CBUs, persons)         │
//! │  - Session context (coreference resolution)                     │
//! └─────────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! Token Stream: [VERB:add, ENTITY:goldman_sachs, PREP:as, ROLE:counterparty,
//!                PREP:for, INSTRUMENT:irs, NOUN:trades, PREP:under, LAW:ny]
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Nom Grammar Parser                            │
//! │  - Matches token patterns to intent structures                  │
//! │  - Builds typed IntentAst nodes                                 │
//! │  - Handles domain detection (OTC vs Exchange-Traded)            │
//! └─────────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! IntentAst::CounterpartyCreate {
//!     counterparty: ResolvedEntity { id: "...", name: "Goldman Sachs" },
//!     instruments: [InstrumentCode::IRS],
//!     governing_law: Some(GoverningLaw::NewYork),
//! }
//! ```

#[cfg(feature = "gateway")]
mod db_resolver;
mod intent_ast;
mod intent_parser;
mod loader;
mod pipeline;
mod tokenizer;
mod tokens;

#[cfg(feature = "gateway")]
pub use db_resolver::{CompositeEntityResolver, DatabaseEntityResolver};
pub use intent_ast::{
    CsaType, CurrencyCode, EntityRef, GoverningLaw, InstrumentCode, IntentAst, MarketCode, RoleCode,
};
pub use intent_parser::parse_tokens;
pub use loader::{
    EntitiesConfig, InstrumentsConfig, Lexicon, LexiconConfig, LexiconEntry, LifecycleDomain,
    PrepositionsConfig, VerbsConfig,
};
pub use pipeline::{LexiconPipeline, LexiconPipelineResult};
pub use tokenizer::{
    EntityResolver, MockEntityResolver, ResolvedEntity, SalientEntity, SessionSalience, Tokenizer,
};
pub use tokens::{
    EntityClass, ModifierType, NumberType, PrepType, Token, TokenSource, TokenType, VerbClass,
};
