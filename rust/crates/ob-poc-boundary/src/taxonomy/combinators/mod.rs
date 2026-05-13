//! Nom-style Parser Combinators for Taxonomy Generation
//!
//! This module provides a fluent, composable API for building taxonomies from data sources.
//! The key innovation is `.each_is_taxonomy()` which marks child nodes as expandable
//! with their own parser - enabling fractal navigation.
//!
//! # Example
//!
//! ```ignore
//! let tree = parser()
//!     .from(CbuDataSource::new(pool))
//!     .grouped_by(jurisdiction_grouper())
//!     .with_children(|cbu| entity_parser().for_cbu(cbu.id))
//!     .each_is_taxonomy(trading_matrix_parser)  // Children can zoom into trading view
//!     .summarized()
//!     .build()
//!     .await?;
//! ```
//!
//! # Combinator Methods
//!
//! | Method | Purpose |
//! |--------|---------|
//! | `.from(source)` | Set data source |
//! | `.grouped_by(grouper)` | Add intermediate grouping nodes |
//! | `.with_children(f)` | Add child parser per node |
//! | `.each_is_taxonomy(parser_fn)` | Set ExpansionRule::Parser on children |
//! | `.summarized()` | Add summary nodes for large groups |
//! | `.expandable()` | Mark as lazy-loadable |
//! | `.terminal()` | Mark as non-expandable leaf |

mod grouper;
mod parser;
mod source;

pub use grouper::{EntityTypeGrouper, FieldGrouper, Grouper, GrouperBox, JurisdictionGrouper};
pub use parser::{ParserCombinator, TaxonomyParser, TaxonomyParserBuilder};
pub use source::{DataSource, DataSourceBox, EmptySource};
