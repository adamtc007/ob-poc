//! Document Extraction Source
//!
//! Fetches attribute values from extracted document data.
//! In production, this would integrate with OCR/NLP services.
//! For now, uses mock extracted data.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;

/// Simulated document extraction - replace with real OCR/NLP later
pub struct DocumentExtractionSource {
    extracted_data: HashMap<Uuid, JsonValue>,
}

impl DocumentExtractionSource {
    pub fn new() -> Self {
        let mut extracted_data = HashMap::new();

        // Mock passport extraction data using real UUIDs from kyc.rs

        // First name UUID: attr.identity.first_name
        if let Ok(uuid) = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935") {
            extracted_data.insert(uuid, JsonValue::String("John".to_string()));
        }

        // Last name UUID: attr.identity.last_name
        if let Ok(uuid) = Uuid::parse_str("0af112fd-ec04-5938-84e8-6e5949db0b52") {
            extracted_data.insert(uuid, JsonValue::String("Smith".to_string()));
        }

        // Passport number UUID: attr.identity.passport_number
        if let Ok(uuid) = Uuid::parse_str("c09501c7-2ea9-5ad7-b330-7d664c678e37") {
            extracted_data.insert(uuid, JsonValue::String("AB123456".to_string()));
        }

        // Nationality UUID: attr.identity.nationality
        if let Ok(uuid) = Uuid::parse_str("33d0752b-a92c-5e20-8559-43ab3668ecf5") {
            extracted_data.insert(uuid, JsonValue::String("US".to_string()));
        }

        Self { extracted_data }
    }

    pub fn with_extractions(extracted_data: HashMap<Uuid, JsonValue>) -> Self {
        Self { extracted_data }
    }

    pub fn add_extraction(&mut self, uuid: Uuid, value: JsonValue) {
        self.extracted_data.insert(uuid, value);
    }
}

impl Default for DocumentExtractionSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceExecutor for DocumentExtractionSource {
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue> {
        let value = self
            .extracted_data
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
            source: ValueSource::DocumentExtraction {
                document_id: Uuid::new_v4(),
                page: Some(1),
                confidence: 0.95,
            },
        })
    }

    fn can_handle(&self, attr_uuid: &Uuid) -> bool {
        self.extracted_data.contains_key(attr_uuid)
    }

    fn priority(&self) -> u32 {
        5 // High priority - document extraction is preferred when available
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::execution_context::ExecutionContext;
    use crate::domains::attributes::kyc::FirstName;
    use crate::domains::attributes::types::AttributeType;

    #[tokio::test]
    async fn test_document_extraction_creation() {
        let source = DocumentExtractionSource::new();
        assert_eq!(source.priority(), 5);
    }

    #[tokio::test]
    async fn test_can_handle_real_uuid() {
        let source = DocumentExtractionSource::new();
        let first_name_uuid = FirstName::uuid();

        assert!(source.can_handle(&first_name_uuid));
    }

    #[tokio::test]
    async fn test_fetch_extracted_value() {
        let source = DocumentExtractionSource::new();
        let context = ExecutionContext::new();
        let first_name_uuid = FirstName::uuid();

        let result = source.fetch_value(first_name_uuid, &context).await;
        assert!(result.is_ok());

        let attr_value = result.unwrap();
        assert_eq!(attr_value.uuid, first_name_uuid);
        assert_eq!(attr_value.semantic_id, "attr.identity.first_name");
        assert_eq!(attr_value.value, JsonValue::String("John".to_string()));

        // Verify source type
        match attr_value.source {
            ValueSource::DocumentExtraction { confidence, .. } => {
                assert_eq!(confidence, 0.95);
            }
            _ => panic!("Expected DocumentExtraction source"),
        }
    }
}
