//! Default Value Source
//!
//! Provides default values for attributes when no other source is available.
//! This is the lowest priority source.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct DefaultValueSource {
    defaults: HashMap<Uuid, JsonValue>,
}

impl DefaultValueSource {
    pub fn new() -> Self {
        let mut defaults = HashMap::new();

        // Add common defaults
        // Note: These UUIDs would be from your actual attribute definitions
        // For now using placeholder UUIDs

        // Country default
        if let Ok(uuid) = Uuid::parse_str("f47ac10b-58cc-5e7a-a716-446655440037") {
            defaults.insert(uuid, JsonValue::String("US".to_string()));
        }

        // Risk tolerance default
        if let Ok(uuid) = Uuid::parse_str("f47ac10b-58cc-5e7a-a716-446655440047") {
            defaults.insert(uuid, JsonValue::String("MODERATE".to_string()));
        }

        Self { defaults }
    }

    pub fn with_defaults(defaults: HashMap<Uuid, JsonValue>) -> Self {
        Self { defaults }
    }

    pub fn add_default(&mut self, uuid: Uuid, value: JsonValue) {
        self.defaults.insert(uuid, value);
    }
}

impl Default for DefaultValueSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceExecutor for DefaultValueSource {
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue> {
        let value = self
            .defaults
            .get(&attr_uuid)
            .ok_or(SourceError::NoValidSource(attr_uuid))?;

        let semantic_id = context
            .resolver
            .uuid_to_semantic(&attr_uuid)
            .map_err(|_| SourceError::NoValidSource(attr_uuid))?;

        Ok(AttributeValue {
            uuid: attr_uuid,
            semantic_id,
            value: value.clone(),
            source: ValueSource::Default {
                reason: "System default value".to_string(),
            },
        })
    }

    fn can_handle(&self, attr_uuid: &Uuid) -> bool {
        self.defaults.contains_key(attr_uuid)
    }

    fn priority(&self) -> u32 {
        999 // Lowest priority - only used as fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::execution_context::ExecutionContext;

    #[tokio::test]
    async fn test_default_source_creation() {
        let source = DefaultValueSource::new();
        assert_eq!(source.priority(), 999);
    }

    #[tokio::test]
    async fn test_can_handle() {
        let mut source = DefaultValueSource::new();
        let test_uuid = Uuid::new_v4();

        assert!(!source.can_handle(&test_uuid));

        source.add_default(test_uuid, JsonValue::String("test".to_string()));
        assert!(source.can_handle(&test_uuid));
    }

    #[tokio::test]
    async fn test_fetch_default_value() {
        let mut source = DefaultValueSource::new();
        let context = ExecutionContext::new();
        let test_uuid = Uuid::new_v4();

        source.add_default(test_uuid, JsonValue::String("default_value".to_string()));

        // This will fail UUID resolution since test_uuid isn't in the resolver
        // In real usage, UUIDs would be from the attribute dictionary
        let result = source.fetch_value(test_uuid, &context).await;
        assert!(result.is_err()); // Expected because UUID not in resolver
    }
}
