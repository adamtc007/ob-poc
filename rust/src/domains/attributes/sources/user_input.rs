//! User Input Source
//!
//! Fetches attribute values from user-provided form input.
//! Medium priority source.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct UserInputSource {
    form_data: HashMap<Uuid, (JsonValue, String)>, // (value, form_id)
}

impl UserInputSource {
    pub fn new() -> Self {
        Self {
            form_data: HashMap::new(),
        }
    }

    pub fn with_form_data(form_data: HashMap<Uuid, (JsonValue, String)>) -> Self {
        Self { form_data }
    }

    pub fn add_input(&mut self, uuid: Uuid, value: JsonValue, form_id: String) {
        self.form_data.insert(uuid, (value, form_id));
    }
}

impl Default for UserInputSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceExecutor for UserInputSource {
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue> {
        let (value, form_id) = self
            .form_data
            .get(&attr_uuid)
            .ok_or(SourceError::NoValidSource(attr_uuid))?;

        let semantic_id = context
            .resolver
            .uuid_to_semantic(&attr_uuid)
            .map_err(|_| SourceError::NoValidSource(attr_uuid))?;

        Ok(AttributeValue {
            uuid: attr_uuid,
            semantic_id: semantic_id.clone(),
            value: value.clone(),
            source: ValueSource::UserInput {
                form_id: form_id.clone(),
                field: semantic_id,
            },
        })
    }

    fn can_handle(&self, attr_uuid: &Uuid) -> bool {
        self.form_data.contains_key(attr_uuid)
    }

    fn priority(&self) -> u32 {
        10 // Medium priority
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::execution_context::ExecutionContext;
    use crate::domains::attributes::kyc::FirstName;
    use crate::domains::attributes::types::AttributeType;

    #[tokio::test]
    async fn test_user_input_creation() {
        let source = UserInputSource::new();
        assert_eq!(source.priority(), 10);
    }

    #[tokio::test]
    async fn test_add_and_fetch_input() {
        let mut source = UserInputSource::new();
        let context = ExecutionContext::new();
        let first_name_uuid = FirstName::uuid();

        source.add_input(
            first_name_uuid,
            JsonValue::String("Alice".to_string()),
            "onboarding-form".to_string(),
        );

        assert!(source.can_handle(&first_name_uuid));

        let result = source.fetch_value(first_name_uuid, &context).await;
        assert!(result.is_ok());

        let attr_value = result.unwrap();
        assert_eq!(attr_value.value, JsonValue::String("Alice".to_string()));

        match attr_value.source {
            ValueSource::UserInput { form_id, .. } => {
                assert_eq!(form_id, "onboarding-form");
            }
            _ => panic!("Expected UserInput source"),
        }
    }
}
