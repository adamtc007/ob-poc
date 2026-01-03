//! Data Sources for Taxonomy Parsing
//!
//! A DataSource provides the raw data that gets parsed into taxonomy nodes.
//! Sources are async and can fetch from database, API, or in-memory collections.

use async_trait::async_trait;
use std::fmt::Debug;
use uuid::Uuid;

use crate::taxonomy::types::DimensionValues;

/// A single item from a data source
#[derive(Debug, Clone)]
pub struct SourceItem {
    /// Unique identifier
    pub id: Uuid,
    /// Display label
    pub label: String,
    /// Short label for compact display
    pub short_label: Option<String>,
    /// Dimension values for filtering/grouping
    pub dimensions: DimensionValues,
    /// Optional parent ID for hierarchical sources
    pub parent_id: Option<Uuid>,
    /// Arbitrary metadata
    pub metadata: serde_json::Value,
}

impl SourceItem {
    /// Create a new source item
    pub fn new(id: Uuid, label: impl Into<String>, dimensions: DimensionValues) -> Self {
        Self {
            id,
            label: label.into(),
            short_label: None,
            dimensions,
            parent_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Set short label
    pub fn with_short_label(mut self, short: impl Into<String>) -> Self {
        self.short_label = Some(short.into());
        self
    }

    /// Set parent ID
    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Async data source trait
#[async_trait]
pub trait DataSource: Send + Sync + Debug {
    /// Fetch all items from this source
    async fn fetch(&self) -> Result<Vec<SourceItem>, DataSourceError>;

    /// Fetch items for a specific parent (for hierarchical sources)
    async fn fetch_children(&self, parent_id: Uuid) -> Result<Vec<SourceItem>, DataSourceError> {
        let all = self.fetch().await?;
        Ok(all
            .into_iter()
            .filter(|item| item.parent_id == Some(parent_id))
            .collect())
    }

    /// Get a single item by ID
    async fn get(&self, id: Uuid) -> Result<Option<SourceItem>, DataSourceError> {
        let all = self.fetch().await?;
        Ok(all.into_iter().find(|item| item.id == id))
    }

    /// Clone this source into a boxed trait object
    fn clone_box(&self) -> Box<dyn DataSource>;
}

/// Boxed data source for ergonomics
pub type DataSourceBox = Box<dyn DataSource>;

/// Data source errors
#[derive(Debug, thiserror::Error)]
pub enum DataSourceError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Empty data source - returns no items
#[derive(Debug, Clone, Default)]
pub struct EmptySource;

#[async_trait]
impl DataSource for EmptySource {
    async fn fetch(&self) -> Result<Vec<SourceItem>, DataSourceError> {
        Ok(Vec::new())
    }

    fn clone_box(&self) -> Box<dyn DataSource> {
        Box::new(self.clone())
    }
}

/// In-memory data source from a vector
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct VecSource {
    items: Vec<SourceItem>,
}

impl VecSource {
    #[allow(dead_code)]
    pub fn new(items: Vec<SourceItem>) -> Self {
        Self { items }
    }
}

#[async_trait]
impl DataSource for VecSource {
    async fn fetch(&self) -> Result<Vec<SourceItem>, DataSourceError> {
        Ok(self.items.clone())
    }

    fn clone_box(&self) -> Box<dyn DataSource> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_source() {
        let source = EmptySource;
        let items = source.fetch().await.unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_vec_source() {
        let items = vec![
            SourceItem::new(Uuid::new_v4(), "Item 1", DimensionValues::default()),
            SourceItem::new(Uuid::new_v4(), "Item 2", DimensionValues::default()),
        ];
        let source = VecSource::new(items.clone());
        let fetched = source.fetch().await.unwrap();
        assert_eq!(fetched.len(), 2);
    }
}
