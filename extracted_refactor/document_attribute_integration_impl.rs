// DOCUMENT TO ATTRIBUTE INTEGRATION - FULL IMPLEMENTATION
// Drop this into Zed Claude and say: "Implement all of these components in order"
// This completes the 85% missing functionality from the attribute dictionary

use uuid::Uuid;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use async_trait::async_trait;
use sqlx::{PgPool, FromRow, query_as, query};
use anyhow::{Result, Context};

// ============================================================================
// PHASE 1: TYPE SAFETY - Replace ALL String IDs with AttributeId
// ============================================================================

/// Strongly typed AttributeId - MUST be used everywhere
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct AttributeId(pub Uuid);

impl AttributeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

// ============================================================================
// PHASE 2: DOCUMENT METADATA SCHEMA
// ============================================================================

/// Database migration - RUN FIRST
const MIGRATION: &str = r#"
-- Document metadata table for storing extracted attributes
CREATE TABLE IF NOT EXISTS document_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id),
    attribute_id UUID NOT NULL,
    extracted_value JSONB NOT NULL,
    confidence_score FLOAT DEFAULT 1.0,
    extraction_method TEXT NOT NULL, -- 'ocr', 'nlp', 'regex', 'manual'
    extraction_timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    validation_status TEXT DEFAULT 'pending', -- 'pending', 'validated', 'rejected'
    validated_by UUID REFERENCES users(id),
    validation_timestamp TIMESTAMP WITH TIME ZONE,
    UNIQUE(document_id, attribute_id)
);

-- Index for fast attribute lookups
CREATE INDEX idx_doc_meta_attr ON document_metadata(attribute_id);
CREATE INDEX idx_doc_meta_status ON document_metadata(validation_status);

-- Document catalog for tracking document types and their attributes
CREATE TABLE IF NOT EXISTS document_catalog (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_type TEXT NOT NULL, -- 'passport', 'drivers_license', 'utility_bill'
    issuer TEXT NOT NULL, -- 'US_DMV', 'UK_PASSPORT_OFFICE'
    jurisdiction TEXT NOT NULL, -- 'US-CA', 'UK', 'EU'
    trust_score INTEGER DEFAULT 50, -- 0-100 trust level
    supported_attributes JSONB NOT NULL, -- Array of AttributeIds this doc provides
    validation_rules JSONB, -- Rules for validating this document type
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Attribute extraction logs for audit
CREATE TABLE IF NOT EXISTS attribute_extraction_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    document_id UUID NOT NULL,
    attribute_id UUID NOT NULL,
    extraction_method TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    error_message TEXT,
    processing_time_ms INTEGER,
    extracted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
"#;

// ============================================================================
// PHASE 3: DOCUMENT-ATTRIBUTE LINKAGE
// ============================================================================

/// Document metadata for extracted attributes
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentMetadata {
    pub id: Uuid,
    pub document_id: Uuid,
    pub attribute_id: Uuid,
    pub extracted_value: serde_json::Value,
    pub confidence_score: f32,
    pub extraction_method: String,
    pub extraction_timestamp: DateTime<Utc>,
    pub validation_status: String,
    pub validated_by: Option<Uuid>,
    pub validation_timestamp: Option<DateTime<Utc>>,
}

/// Document catalog entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalogEntry {
    pub id: Uuid,
    pub document_type: String,
    pub issuer: String,
    pub jurisdiction: String,
    pub trust_score: i32,
    pub supported_attributes: serde_json::Value, // Vec<AttributeId>
    pub validation_rules: Option<serde_json::Value>,
}

// ============================================================================
// PHASE 4: DOCUMENT SOURCE IMPLEMENTATION
// ============================================================================

/// Document-based attribute source that extracts from uploaded documents
pub struct DocumentCatalogSource {
    pool: PgPool,
    extraction_service: Box<dyn ExtractionService>,
}

impl DocumentCatalogSource {
    pub fn new(pool: PgPool, extraction_service: Box<dyn ExtractionService>) -> Self {
        Self { pool, extraction_service }
    }
    
    /// Find best document for an attribute
    async fn find_best_document(&self, cbu_id: &Uuid, attr_id: &AttributeId) -> Result<Option<Uuid>> {
        let query = r#"
            SELECT d.id 
            FROM documents d
            JOIN document_catalog dc ON dc.document_type = d.document_type
            WHERE d.cbu_id = $1
            AND dc.supported_attributes @> to_jsonb($2::text)
            ORDER BY dc.trust_score DESC, d.uploaded_at DESC
            LIMIT 1
        "#;
        
        let result: Option<(Uuid,)> = query_as(query)
            .bind(cbu_id)
            .bind(attr_id.0.to_string())
            .fetch_optional(&self.pool)
            .await?;
            
        Ok(result.map(|r| r.0))
    }
    
    /// Extract attribute value from document
    async fn extract_from_document(
        &self, 
        doc_id: &Uuid, 
        attr_id: &AttributeId
    ) -> Result<serde_json::Value> {
        // Check if already extracted
        let existing = self.get_existing_extraction(doc_id, attr_id).await?;
        if let Some(metadata) = existing {
            return Ok(metadata.extracted_value);
        }
        
        // Perform extraction
        let value = self.extraction_service.extract(doc_id, attr_id).await?;
        
        // Store in metadata
        self.store_extraction(doc_id, attr_id, &value).await?;
        
        Ok(value)
    }
    
    async fn get_existing_extraction(
        &self,
        doc_id: &Uuid,
        attr_id: &AttributeId
    ) -> Result<Option<DocumentMetadata>> {
        let query = r#"
            SELECT * FROM document_metadata 
            WHERE document_id = $1 AND attribute_id = $2
            AND validation_status != 'rejected'
        "#;
        
        query_as::<_, DocumentMetadata>(query)
            .bind(doc_id)
            .bind(attr_id.0)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to fetch existing extraction")
    }
    
    async fn store_extraction(
        &self,
        doc_id: &Uuid,
        attr_id: &AttributeId,
        value: &serde_json::Value
    ) -> Result<()> {
        let query = r#"
            INSERT INTO document_metadata 
            (document_id, attribute_id, extracted_value, extraction_method, confidence_score)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (document_id, attribute_id) 
            DO UPDATE SET 
                extracted_value = EXCLUDED.extracted_value,
                extraction_timestamp = NOW()
        "#;
        
        sqlx::query(query)
            .bind(doc_id)
            .bind(attr_id.0)
            .bind(value)
            .bind("ocr") // Or get from extraction service
            .bind(0.95f32)
            .execute(&self.pool)
            .await?;
            
        Ok(())
    }
}

// ============================================================================
// PHASE 5: ATTRIBUTE SOURCE TRAIT IMPLEMENTATION
// ============================================================================

#[async_trait]
impl AttributeSource for DocumentCatalogSource {
    async fn get_value(&self, attr_id: &AttributeId, context: &ExecutionContext) -> Result<Option<serde_json::Value>> {
        // Find best document for this attribute
        let doc_id = match self.find_best_document(&context.cbu_id, attr_id).await? {
            Some(id) => id,
            None => return Ok(None),
        };
        
        // Extract value from document
        let value = self.extract_from_document(&doc_id, attr_id).await?;
        
        // Log extraction
        self.log_extraction(&context.cbu_id, &doc_id, attr_id, true, None).await?;
        
        Ok(Some(value))
    }
    
    fn priority(&self) -> i32 {
        100 // High priority for document sources
    }
}

// ============================================================================
// PHASE 6: EXTRACTION SERVICE
// ============================================================================

#[async_trait]
pub trait ExtractionService: Send + Sync {
    async fn extract(&self, doc_id: &Uuid, attr_id: &AttributeId) -> Result<serde_json::Value>;
}

/// OCR-based extraction service
pub struct OcrExtractionService {
    pool: PgPool,
}

#[async_trait]
impl ExtractionService for OcrExtractionService {
    async fn extract(&self, doc_id: &Uuid, attr_id: &AttributeId) -> Result<serde_json::Value> {
        // Get document content
        let doc = self.get_document(doc_id).await?;
        
        // Get attribute definition
        let attr_def = self.get_attribute_definition(attr_id).await?;
        
        // Apply extraction based on attribute type
        match attr_def.data_type.as_str() {
            "date" => self.extract_date(&doc.content, &attr_def.extraction_rules),
            "text" => self.extract_text(&doc.content, &attr_def.extraction_rules),
            "number" => self.extract_number(&doc.content, &attr_def.extraction_rules),
            _ => Ok(serde_json::Value::Null),
        }
    }
}

// ============================================================================
// PHASE 7: DSL INTEGRATION
// ============================================================================

/// DSL Parser extension for attribute references
pub mod dsl_parser {
    use nom::{
        IResult,
        bytes::complete::{tag, take_while1},
        character::complete::{char, multispace0},
        combinator::{map, opt},
        sequence::{delimited, preceded, tuple},
    };
    use super::AttributeId;
    
    /// Parse @attr{uuid} syntax in DSL
    pub fn parse_attribute_reference(input: &str) -> IResult<&str, AttributeId> {
        map(
            delimited(
                tag("@attr{"),
                take_while1(|c: char| c.is_alphanumeric() || c == '-'),
                char('}')
            ),
            |uuid_str: &str| AttributeId::from_string(uuid_str).unwrap()
        )(input)
    }
    
    /// Parse attribute with source hint: @attr{uuid}:doc
    pub fn parse_attribute_with_source(input: &str) -> IResult<&str, (AttributeId, Option<String>)> {
        map(
            tuple((
                parse_attribute_reference,
                opt(preceded(char(':'), take_while1(|c: char| c.is_alphabetic())))
            )),
            |(attr_id, source)| (attr_id, source.map(String::from))
        )(input)
    }
}

// ============================================================================
// PHASE 8: ATTRIBUTE EXECUTION ENGINE
// ============================================================================

pub struct AttributeExecutor {
    sources: Vec<Box<dyn AttributeSource>>,
    sinks: Vec<Box<dyn AttributeSink>>,
    dictionary: AttributeDictionary,
}

impl AttributeExecutor {
    /// Execute attribute resolution with fallback chain
    pub async fn resolve_attribute(
        &self,
        attr_id: &AttributeId,
        context: &ExecutionContext
    ) -> Result<serde_json::Value> {
        // Get attribute definition
        let def = self.dictionary.get_attribute(attr_id).await?;
        
        // Try sources in priority order
        let mut value = None;
        for source in &self.sources {
            if let Some(v) = source.get_value(attr_id, context).await? {
                value = Some(v);
                break;
            }
        }
        
        let value = value.ok_or_else(|| anyhow::anyhow!("No value found for attribute"))?;
        
        // Validate
        self.dictionary.validate_attribute_value(attr_id, &value).await?;
        
        // Persist to sinks
        for sink in &self.sinks {
            sink.write_value(attr_id, &value, context).await?;
        }
        
        Ok(value)
    }
}

// ============================================================================
// PHASE 9: INTEGRATION TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_document_extraction_flow() {
        // Setup
        let pool = setup_test_db().await;
        let extraction = Box::new(OcrExtractionService { pool: pool.clone() });
        let source = DocumentCatalogSource::new(pool.clone(), extraction);
        
        // Create test document
        let doc_id = create_test_document(&pool, "passport").await;
        let attr_id = AttributeId::new();
        let cbu_id = Uuid::new_v4();
        
        // Test extraction
        let context = ExecutionContext { cbu_id, ..Default::default() };
        let value = source.get_value(&attr_id, &context).await.unwrap();
        
        assert!(value.is_some());
    }
    
    #[test]
    fn test_dsl_attribute_parsing() {
        let input = "@attr{550e8400-e29b-41d4-a716-446655440000}:doc";
        let (remaining, (attr_id, source)) = dsl_parser::parse_attribute_with_source(input).unwrap();
        
        assert_eq!(remaining, "");
        assert_eq!(source, Some("doc".to_string()));
    }
    
    #[tokio::test]
    async fn test_attribute_resolution_with_fallback() {
        let executor = setup_test_executor().await;
        let attr_id = AttributeId::new();
        let context = ExecutionContext::default();
        
        // Should try document source first, then fall back to form
        let value = executor.resolve_attribute(&attr_id, &context).await;
        assert!(value.is_ok());
    }
}

// ============================================================================
// PHASE 10: IMPLEMENTATION CHECKLIST
// ============================================================================

/* 
IMPLEMENTATION STEPS FOR CLAUDE IN ZED:

1. [ ] Run database migration
   - Execute the MIGRATION SQL above
   - Verify all tables created

2. [ ] Replace ALL String attribute IDs
   - Global search/replace: String -> AttributeId
   - Update all function signatures
   - Fix compilation errors

3. [ ] Implement DocumentCatalogSource
   - Wire up to existing document storage
   - Connect extraction service
   - Test with real documents

4. [ ] Update DSL Parser
   - Add attribute_reference parser
   - Integrate with existing nom combinators
   - Update DSL execution to resolve attributes

5. [ ] Create AttributeExecutor
   - Initialize with all sources/sinks
   - Wire into DSL execution context
   - Add to dependency injection

6. [ ] Implement ExtractionService
   - OCR integration for PDFs
   - NLP for text documents  
   - Regex patterns for structured data

7. [ ] Add Validation Layer
   - Type checking
   - Format validation
   - Business rules

8. [ ] Create Audit Logging
   - Track all extractions
   - Log validation decisions
   - Performance metrics

9. [ ] Integration Testing
   - End-to-end document upload → attribute extraction
   - DSL compilation with attributes
   - Multi-source fallback scenarios

10. [ ] Performance Optimization
    - Add caching layer
    - Batch extractions
    - Background processing queue
*/

// ============================================================================
// CRITICAL FIXES NEEDED IN EXISTING CODE
// ============================================================================

/*
FILES TO UPDATE:

1. src/models/attribute.rs
   - Change: pub id: String → pub id: AttributeId
   - Add: impl From<Uuid> for AttributeId

2. src/services/dictionary_service.rs
   - Update all HashMap<String, _> to HashMap<AttributeId, _>
   - Fix get_attribute() to use AttributeId

3. src/dsl/parser.rs
   - Add: parse_attribute_reference function
   - Update: expression parser to handle @attr{} syntax

4. src/db/schema.sql
   - Add: document_metadata table
   - Add: document_catalog table
   - Add: attribute_extraction_log table

5. src/api/handlers/document_handler.rs
   - Add: trigger extraction on upload
   - Add: validation endpoint

6. migrations/
   - Create: 004_document_metadata.sql
   - Run: sqlx migrate run

COMMAND TO RUN IN ZED:
"Implement all components in this file systematically, starting with the database migration, 
then type safety changes, then document source implementation. Test each phase before moving on."
*/
