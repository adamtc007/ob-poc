//! Value Binder
//!
//! Binds attribute UUIDs to their values by coordinating multiple source executors.
//! Sources are tried in priority order until a value is found.

use uuid::Uuid;

use crate::domains::attributes::execution_context::ExecutionContext;
use crate::domains::attributes::sources::{
    DefaultValueSource, DocumentExtractionSource, SourceError, SourceExecutor, UserInputSource,
};

pub struct ValueBinder {
    sources: Vec<Box<dyn SourceExecutor>>,
}

impl ValueBinder {
    pub fn new() -> Self {
        let mut sources: Vec<Box<dyn SourceExecutor>> = vec![
            Box::new(DocumentExtractionSource::new()),
            Box::new(UserInputSource::new()),
            Box::new(DefaultValueSource::new()),
        ];

        // Sort by priority (lower number = higher priority)
        sources.sort_by_key(|s| s.priority());

        Self { sources }
    }

    pub fn with_sources(sources: Vec<Box<dyn SourceExecutor>>) -> Self {
        let mut sources = sources;
        sources.sort_by_key(|s| s.priority());
        Self { sources }
    }

    /// Bind a single attribute by trying sources in priority order
    pub async fn bind_attribute(
        &self,
        attr_uuid: Uuid,
        context: &mut ExecutionContext,
    ) -> Result<(), SourceError> {
        for source in &self.sources {
            if !source.can_handle(&attr_uuid) {
                continue;
            }

            match source.fetch_value(attr_uuid, context).await {
                Ok(value) => {
                    context.bind_value(attr_uuid, value.value, value.source);
                    return Ok(());
                }
                Err(e) => {
                    log::debug!(
                        "Source (priority {}) failed for {}: {}",
                        source.priority(),
                        attr_uuid,
                        e
                    );
                    continue;
                }
            }
        }

        Err(SourceError::NoValidSource(attr_uuid))
    }

    /// Bind multiple attributes in parallel
    pub async fn bind_all(
        &self,
        attr_uuids: Vec<Uuid>,
        context: &mut ExecutionContext,
    ) -> Vec<Result<(), SourceError>> {
        let mut results = Vec::new();

        for uuid in attr_uuids {
            results.push(self.bind_attribute(uuid, context).await);
        }

        results
    }

    /// Get count of available sources
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Check if any source can handle this attribute
    pub fn can_bind(&self, attr_uuid: &Uuid) -> bool {
        self.sources.iter().any(|s| s.can_handle(attr_uuid))
    }
}

impl Default for ValueBinder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::kyc::FirstName;
    use crate::domains::attributes::types::AttributeType;

    #[tokio::test]
    async fn test_value_binder_creation() {
        let binder = ValueBinder::new();
        assert_eq!(binder.source_count(), 3);
    }

    #[tokio::test]
    async fn test_can_bind() {
        let binder = ValueBinder::new();
        let first_name_uuid = FirstName::uuid();

        // DocumentExtractionSource should have this UUID
        assert!(binder.can_bind(&first_name_uuid));

        // Random UUID should not be bindable
        let random_uuid = Uuid::new_v4();
        assert!(!binder.can_bind(&random_uuid));
    }

    #[tokio::test]
    async fn test_bind_attribute() {
        let binder = ValueBinder::new();
        let mut context = ExecutionContext::new();
        let first_name_uuid = FirstName::uuid();

        let result = binder.bind_attribute(first_name_uuid, &mut context).await;
        assert!(result.is_ok());

        // Verify value was bound
        assert!(context.is_bound(&first_name_uuid));
        let value = context.get_value(&first_name_uuid).unwrap();
        assert_eq!(value, &serde_json::json!("John"));
    }

    #[tokio::test]
    async fn test_bind_all() {
        let binder = ValueBinder::new();
        let mut context = ExecutionContext::new();

        use crate::domains::attributes::kyc::{FirstName, LastName, PassportNumber};

        let uuids = vec![FirstName::uuid(), LastName::uuid(), PassportNumber::uuid()];

        let results = binder.bind_all(uuids.clone(), &mut context).await;

        // All should succeed
        assert_eq!(results.len(), 3);
        for result in &results {
            assert!(result.is_ok());
        }

        // Verify all are bound
        for uuid in uuids {
            assert!(context.is_bound(&uuid));
        }
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let binder = ValueBinder::new();

        // Sources should be sorted by priority
        // DocumentExtraction (5) < UserInput (10) < Default (999)
        let priorities: Vec<u32> = binder.sources.iter().map(|s| s.priority()).collect();

        for i in 1..priorities.len() {
            assert!(
                priorities[i - 1] <= priorities[i],
                "Sources not sorted by priority"
            );
        }
    }
}
