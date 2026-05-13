//! Unified Taxonomy Module
//!
//! This module implements the unified session + taxonomy architecture where:
//! - **Session = Intent Scope = Visual State = Operation Target**
//! - Shape determines metaphor (not prescribed)
//! - AstroLevel derives from descendant count and depth
//!
//! # Key Types
//!
//! - [`TaxonomyNode`] - Universal tree node with computed metrics
//! - [`TaxonomyContext`] - Defines WHAT taxonomy to build (Universe, Book, CbuTrading, etc.)
//! - [`MembershipRules`] - Defines HOW to build (edges, grouping, terminus)
//! - [`TaxonomyBuilder`] - Constructs trees from rules and data
//! - [`TaxonomyParser`] - Nom-style parser trait with combinators
//! - [`TaxonomyStack`] - Stack-based fractal navigation
//!
//! # Derivation Philosophy
//!
//! ```text
//! Tree Shape → Metrics → Metaphor
//!           → Metrics → AstroLevel
//! ```
//!
//! The taxonomy doesn't prescribe metaphors or levels - they emerge from structure.
//!
//! # Fractal Navigation
//!
//! Every node can expand into its own taxonomy via `ExpansionRule::Parser`.
//! The `.each_is_taxonomy()` combinator sets this on child nodes.

mod builder;
pub mod combinators;
mod node;
mod rules;
#[cfg(feature = "database")]
mod service;
mod stack;
mod types;

pub use types::{AstroLevel, DimensionValues, EntitySummary, Filter, Metaphor, NodeType, Status};

pub use node::{ExpansionRule, TaxonomyNode};

pub use rules::{
    Dimension, EdgeType, EntityFilter, GroupingStrategy, MembershipRules, RootFilter,
    TaxonomyContext, TerminusCondition, TraversalDirection,
};

pub use builder::TaxonomyBuilder;

pub use combinators::{
    DataSource, DataSourceBox, EmptySource, Grouper, GrouperBox, ParserCombinator, TaxonomyParser,
    TaxonomyParserBuilder,
};

pub use stack::{TaxonomyFrame, TaxonomyStack};

#[cfg(feature = "database")]
pub use service::{ChildNode, EntityListItem, TaxonomyPosition, TaxonomyService};
