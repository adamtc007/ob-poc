//! TaxonomyParser - The core parser trait and combinators
//!
//! This implements a nom-style fluent API for building taxonomies.
//! The key innovation is `.each_is_taxonomy()` which enables fractal navigation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use uuid::Uuid;

use super::grouper::{Grouper, GrouperBox};
use super::source::{DataSource, DataSourceBox, DataSourceError, EmptySource};
use crate::taxonomy::node::{ExpansionRule, TaxonomyNode};
use crate::taxonomy::types::{DimensionValues, NodeType};

/// Generate a deterministic UUID from a string
fn uuid_from_string(s: &str) -> Uuid {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    let hash = hasher.finish();
    let bytes: [u8; 16] = {
        let mut b = [0u8; 16];
        b[0..8].copy_from_slice(&hash.to_le_bytes());
        b[8..16].copy_from_slice(&hash.to_be_bytes());
        b
    };
    Uuid::from_bytes(bytes)
}

// =============================================================================
// CORE PARSER TRAIT
// =============================================================================

/// Core trait for taxonomy parsers.
/// Parsers are composable and can be chained to build complex taxonomies.
#[async_trait]
pub trait TaxonomyParser: Send + Sync + Debug {
    /// Parse the data source into a taxonomy tree
    async fn parse(&self) -> Result<TaxonomyNode, ParserError>;

    /// Parse with a specific root context (for child parsers)
    async fn parse_for(&self, _parent_id: Uuid) -> Result<TaxonomyNode, ParserError> {
        // Default: ignore parent context
        self.parse().await
    }

    /// Get a descriptive name for this parser
    fn name(&self) -> &str;

    /// Clone into Arc for storage in ExpansionRule
    fn clone_arc(&self) -> Arc<dyn TaxonomyParser + Send + Sync>;
}

/// Parser errors
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Data source error: {0}")]
    DataSource(#[from] DataSourceError),

    #[error("Build error: {0}")]
    Build(String),

    #[error("Invalid configuration: {0}")]
    Config(String),
}

// =============================================================================
// PARSER COMBINATOR - Fluent builder for parsers
// =============================================================================

/// A configurable parser built using combinators.
/// This is the main way to construct taxonomy parsers.
/// Type alias for child parser factory function
type ChildParserFn = Arc<dyn Fn(Uuid) -> Arc<dyn TaxonomyParser + Send + Sync> + Send + Sync>;

pub struct ParserCombinator {
    name: String,
    source: DataSourceBox,
    grouper: Option<GrouperBox>,
    child_parser_fn: Option<ChildParserFn>,
    expansion_parser: Option<Arc<dyn TaxonomyParser + Send + Sync>>,
    node_type: NodeType,
    summarize_threshold: Option<usize>,
    is_terminal: bool,
    root_label: String,
}

impl std::fmt::Debug for ParserCombinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParserCombinator")
            .field("name", &self.name)
            .field("node_type", &self.node_type)
            .field("summarize_threshold", &self.summarize_threshold)
            .field("is_terminal", &self.is_terminal)
            .field("root_label", &self.root_label)
            .field("has_grouper", &self.grouper.is_some())
            .field("has_child_parser_fn", &self.child_parser_fn.is_some())
            .field("has_expansion_parser", &self.expansion_parser.is_some())
            .finish()
    }
}

impl Clone for ParserCombinator {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            source: self.source.clone_box(),
            grouper: self.grouper.as_ref().map(|g| g.clone_box()),
            child_parser_fn: self.child_parser_fn.clone(),
            expansion_parser: self.expansion_parser.clone(),
            node_type: self.node_type,
            summarize_threshold: self.summarize_threshold,
            is_terminal: self.is_terminal,
            root_label: self.root_label.clone(),
        }
    }
}

impl ParserCombinator {
    /// Create a new parser combinator with a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: Box::new(EmptySource),
            grouper: None,
            child_parser_fn: None,
            expansion_parser: None,
            node_type: NodeType::Entity,
            summarize_threshold: None,
            is_terminal: false,
            root_label: "Root".to_string(),
        }
    }

    /// Set the data source
    pub fn from(mut self, source: impl DataSource + 'static) -> Self {
        self.source = Box::new(source);
        self
    }

    /// Set grouper to cluster items
    pub fn grouped_by(mut self, grouper: impl Grouper + 'static) -> Self {
        self.grouper = Some(Box::new(grouper));
        self
    }

    /// Set the node type for items
    pub fn as_type(mut self, node_type: NodeType) -> Self {
        self.node_type = node_type;
        self
    }

    /// Set root label
    pub fn with_root_label(mut self, label: impl Into<String>) -> Self {
        self.root_label = label.into();
        self
    }

    /// Add child parser per node
    /// The closure receives the parent node's ID and returns a parser for its children
    pub fn with_children<F>(mut self, f: F) -> Self
    where
        F: Fn(Uuid) -> Arc<dyn TaxonomyParser + Send + Sync> + Send + Sync + 'static,
    {
        self.child_parser_fn = Some(Arc::new(f));
        self
    }

    /// **KEY COMBINATOR**: Mark children as expandable taxonomies
    /// This sets ExpansionRule::Parser on each child node, enabling fractal navigation.
    /// When a user zooms into a child, this parser is used to build its taxonomy.
    pub fn each_is_taxonomy(mut self, parser: impl TaxonomyParser + 'static) -> Self {
        self.expansion_parser = Some(Arc::new(parser) as Arc<dyn TaxonomyParser + Send + Sync>);
        self
    }

    /// Add summary nodes when groups exceed threshold
    pub fn summarized(mut self, threshold: usize) -> Self {
        self.summarize_threshold = Some(threshold);
        self
    }

    /// Mark nodes as expandable (has_more_children = true)
    pub fn expandable(self) -> Self {
        // This is handled during build
        self
    }

    /// Mark nodes as terminal (cannot be expanded)
    pub fn terminal(mut self) -> Self {
        self.is_terminal = true;
        self
    }

    /// Build into a completed parser
    pub fn build(self) -> TaxonomyParserBuilder {
        TaxonomyParserBuilder { combinator: self }
    }
}

// =============================================================================
// TAXONOMY PARSER BUILDER - The actual parser implementation
// =============================================================================

/// The built parser that implements TaxonomyParser trait
#[derive(Debug, Clone)]
pub struct TaxonomyParserBuilder {
    combinator: ParserCombinator,
}

#[async_trait]
impl TaxonomyParser for TaxonomyParserBuilder {
    async fn parse(&self) -> Result<TaxonomyNode, ParserError> {
        let c = &self.combinator;

        // Fetch data from source
        let items = c.source.fetch().await?;

        // Create root node
        let mut root = TaxonomyNode::root(&c.root_label);

        // Apply grouping if configured
        if let Some(ref grouper) = c.grouper {
            let groups = grouper.group(items);

            for group in groups {
                let mut group_node = TaxonomyNode::new(
                    group.group_id,
                    group.node_type,
                    &group.label,
                    group.dimensions,
                );

                // Add member nodes under the group
                for item in group.members {
                    let mut node =
                        TaxonomyNode::new(item.id, c.node_type, &item.label, item.dimensions);
                    node.short_label = item.short_label;

                    // Set expansion rule
                    node.expansion = self.determine_expansion_rule(&node);

                    group_node.add_child(node);
                }

                // Apply summarization if needed
                if let Some(threshold) = c.summarize_threshold {
                    if group_node.children.len() > threshold {
                        group_node = self.summarize_group(group_node, threshold);
                    }
                }

                root.add_child(group_node);
            }
        } else {
            // No grouping - items become direct children
            for item in items {
                let mut node =
                    TaxonomyNode::new(item.id, c.node_type, &item.label, item.dimensions);
                node.short_label = item.short_label;
                node.expansion = self.determine_expansion_rule(&node);
                root.add_child(node);
            }
        }

        // Compute metrics
        root.compute_metrics();

        Ok(root)
    }

    async fn parse_for(&self, parent_id: Uuid) -> Result<TaxonomyNode, ParserError> {
        // Fetch children for the specific parent
        let items = self.combinator.source.fetch_children(parent_id).await?;

        let mut root = TaxonomyNode::root(&self.combinator.root_label);

        for item in items {
            let mut node = TaxonomyNode::new(
                item.id,
                self.combinator.node_type,
                &item.label,
                item.dimensions,
            );
            node.short_label = item.short_label;
            node.expansion = self.determine_expansion_rule(&node);
            root.add_child(node);
        }

        root.compute_metrics();
        Ok(root)
    }

    fn name(&self) -> &str {
        &self.combinator.name
    }

    fn clone_arc(&self) -> Arc<dyn TaxonomyParser + Send + Sync> {
        Arc::new(self.clone())
    }
}

impl TaxonomyParserBuilder {
    /// Determine the expansion rule for a node based on combinator config
    fn determine_expansion_rule(&self, _node: &TaxonomyNode) -> ExpansionRule {
        let c = &self.combinator;

        if c.is_terminal {
            ExpansionRule::Terminal
        } else if let Some(ref parser) = c.expansion_parser {
            ExpansionRule::Parser(parser.clone())
        } else {
            ExpansionRule::Complete
        }
    }

    /// Create summary nodes when a group is too large
    fn summarize_group(&self, mut group: TaxonomyNode, threshold: usize) -> TaxonomyNode {
        if group.children.len() <= threshold {
            return group;
        }

        // Split children into chunks
        let chunk_size = group.children.len().div_ceil(threshold);
        let children = std::mem::take(&mut group.children);
        let chunks: Vec<_> = children.chunks(chunk_size).collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let first = &chunk[0];
            let last = chunk.last().unwrap();

            let summary_label = if chunk.len() == 1 {
                first.label.clone()
            } else {
                format!("{} - {} ({} items)", first.label, last.label, chunk.len())
            };

            let mut summary_node = TaxonomyNode::new(
                uuid_from_string(&format!("summary-{}", i)),
                NodeType::Cluster,
                summary_label,
                DimensionValues::default(),
            );

            for child in chunk.iter() {
                summary_node.add_child(child.clone());
            }

            group.add_child(summary_node);
        }

        group
    }
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Create a new parser combinator
#[allow(dead_code)]
pub fn parser(name: impl Into<String>) -> ParserCombinator {
    ParserCombinator::new(name)
}

/// Create a terminal parser (for leaf nodes)
#[allow(dead_code)]
pub fn terminal_parser(name: impl Into<String>) -> ParserCombinator {
    ParserCombinator::new(name).terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::combinators::grouper::JurisdictionGrouper;
    use crate::taxonomy::combinators::source::{SourceItem, VecSource};

    fn make_test_items() -> Vec<SourceItem> {
        vec![
            {
                let mut dims = DimensionValues::default();
                dims.set("jurisdiction", "LU");
                SourceItem::new(Uuid::new_v4(), "Fund Alpha", dims)
            },
            {
                let mut dims = DimensionValues::default();
                dims.set("jurisdiction", "LU");
                SourceItem::new(Uuid::new_v4(), "Fund Beta", dims)
            },
            {
                let mut dims = DimensionValues::default();
                dims.set("jurisdiction", "IE");
                SourceItem::new(Uuid::new_v4(), "Fund Gamma", dims)
            },
        ]
    }

    #[tokio::test]
    async fn test_simple_parser() {
        let items = make_test_items();
        let source = VecSource::new(items);

        let parser = parser("test")
            .from(source)
            .as_type(NodeType::Cbu)
            .with_root_label("Funds")
            .build();

        let tree = parser.parse().await.unwrap();

        assert_eq!(tree.label, "Funds");
        assert_eq!(tree.children.len(), 3);
    }

    #[tokio::test]
    async fn test_grouped_parser() {
        let items = make_test_items();
        let source = VecSource::new(items);

        let parser = parser("test")
            .from(source)
            .grouped_by(JurisdictionGrouper::new())
            .as_type(NodeType::Cbu)
            .with_root_label("Universe")
            .build();

        let tree = parser.parse().await.unwrap();

        assert_eq!(tree.label, "Universe");
        assert_eq!(tree.children.len(), 2); // LU and IE groups

        let lu_group = tree.children.iter().find(|n| n.label == "LU").unwrap();
        assert_eq!(lu_group.children.len(), 2); // Fund Alpha and Beta
    }

    #[tokio::test]
    async fn test_each_is_taxonomy() {
        let items = make_test_items();
        let source = VecSource::new(items);

        // Create a child parser that will be attached to each node
        let child_parser = parser("child").terminal().build();

        let parser = parser("test")
            .from(source)
            .as_type(NodeType::Cbu)
            .each_is_taxonomy(child_parser)
            .build();

        let tree = parser.parse().await.unwrap();

        // Each child should have ExpansionRule::Parser
        for child in &tree.children {
            match &child.expansion {
                ExpansionRule::Parser(_) => {} // Expected
                other => panic!("Expected Parser expansion rule, got {:?}", other),
            }
        }
    }
}
