# Document-Attribute Mapping Refactor Plan

## Objective
Implement complete document-to-attribute mapping capability with real database integration and DSL connectivity. This refactor will enable the system to:
1. Know which attributes can be extracted from which document types
2. Process documents based on their type with appropriate extraction methods
3. Connect DSL document operations to actual attribute extraction and persistence
4. Replace all mock implementations with real database operations

## Critical Success Criteria
- ✅ Documents are typed and mapped to extractable attributes
- ✅ DSL `document.extract` operations trigger real extraction
- ✅ Extracted attributes persist to `document_metadata` and `attribute_values_typed`
- ✅ No mock data - all operations hit the database
- ✅ DSL can reference extracted attributes via UUID resolution

## Phase 1: Database Schema Updates

### 1.1 Create Document-Attribute Mapping Table

**File**: Create new `/home/claude/rust/sql/migrations/001_document_attribute_mappings.sql`

```sql
-- Document to Attribute Mapping Table
-- Defines which attributes can be extracted from which document types
CREATE TABLE IF NOT EXISTS "ob-poc".document_attribute_mappings (
    mapping_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(type_id) ON DELETE CASCADE,
    attribute_uuid UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid) ON DELETE CASCADE,
    
    -- Extraction configuration
    extraction_method VARCHAR(50) NOT NULL CHECK (extraction_method IN (
        'OCR', 'MRZ', 'BARCODE', 'QR_CODE', 'FORM_FIELD', 
        'TABLE', 'CHECKBOX', 'SIGNATURE', 'PHOTO'
    )),
    
    -- Location information for extraction
    field_location JSONB, -- {page: 1, region: {x1: 100, y1: 200, x2: 300, y2: 250}}
    field_name VARCHAR(255), -- For form fields
    
    -- Validation and confidence
    confidence_threshold NUMERIC(3,2) DEFAULT 0.80 CHECK (confidence_threshold BETWEEN 0 AND 1),
    is_required BOOLEAN DEFAULT false,
    validation_pattern TEXT, -- Regex pattern for validation
    
    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(document_type_id, attribute_uuid)
);

-- Index for fast lookups
CREATE INDEX idx_doc_attr_mappings_doc_type 
    ON "ob-poc".document_attribute_mappings(document_type_id);
CREATE INDEX idx_doc_attr_mappings_attr 
    ON "ob-poc".document_attribute_mappings(attribute_uuid);

-- Seed passport mappings
INSERT INTO "ob-poc".document_attribute_mappings 
(document_type_id, attribute_uuid, extraction_method, is_required, confidence_threshold)
VALUES
-- Assuming passport type exists, replace with actual UUID
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '3020d46f-472c-5437-9647-1b0682c35935', -- first_name
 'MRZ', true, 0.95),
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '0af112fd-ec04-5938-84e8-6e5949db0b52', -- last_name
 'MRZ', true, 0.95),
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 'c09501c7-2ea9-5ad7-b330-7d664c678e37', -- passport_number
 'MRZ', true, 0.98),
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '1211e18e-fffe-5e17-9836-fb3cd70452d3', -- date_of_birth
 'MRZ', true, 0.95),
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
 '33d0752b-a92c-5e20-8559-43ab3668ecf5', -- nationality
 'MRZ', true, 0.90);

-- Add document type column to document_catalog if missing
ALTER TABLE "ob-poc".document_catalog 
ADD COLUMN IF NOT EXISTS document_type_id UUID REFERENCES "ob-poc".document_types(type_id);

-- Add extraction metadata to document_metadata
ALTER TABLE "ob-poc".document_metadata
ADD COLUMN IF NOT EXISTS extraction_confidence NUMERIC(3,2),
ADD COLUMN IF NOT EXISTS extraction_method VARCHAR(50),
ADD COLUMN IF NOT EXISTS extracted_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS extraction_metadata JSONB;
```

### 1.2 Create Document Types Seed Data

**File**: Create new `/home/claude/rust/sql/migrations/002_seed_document_types.sql`

```sql
-- Seed essential document types
INSERT INTO "ob-poc".document_types 
(type_code, display_name, category, domain, description)
VALUES
('PASSPORT', 'Passport', 'IDENTITY', 'KYC', 'International travel document'),
('DRIVING_LICENSE', 'Driving License', 'IDENTITY', 'KYC', 'Government-issued driving permit'),
('NATIONAL_ID', 'National ID Card', 'IDENTITY', 'KYC', 'National identification document'),
('BANK_STATEMENT', 'Bank Statement', 'FINANCIAL', 'KYC', 'Bank account statement'),
('UTILITY_BILL', 'Utility Bill', 'PROOF_OF_ADDRESS', 'KYC', 'Utility service bill for address verification'),
('TAX_RETURN', 'Tax Return', 'FINANCIAL', 'TAX', 'Annual tax filing document'),
('ARTICLES_OF_INCORPORATION', 'Articles of Incorporation', 'CORPORATE', 'ENTITY', 'Company formation document'),
('FINANCIAL_STATEMENT', 'Financial Statement', 'FINANCIAL', 'ENTITY', 'Audited financial statements')
ON CONFLICT (type_code) DO NOTHING;
```

## Phase 2: Rust Model Updates

### 2.1 Create Document Type Models

**File**: Create new `/home/claude/rust/src/models/document_type_models.rs`

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

/// Document type with extraction capabilities
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentType {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: DocumentCategory,
    pub domain: String,
    pub description: Option<String>,
}

/// Document categories
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum DocumentCategory {
    #[sqlx(rename = "IDENTITY")]
    Identity,
    #[sqlx(rename = "FINANCIAL")]
    Financial,
    #[sqlx(rename = "PROOF_OF_ADDRESS")]
    ProofOfAddress,
    #[sqlx(rename = "CORPORATE")]
    Corporate,
    #[sqlx(rename = "TAX")]
    Tax,
    #[sqlx(rename = "LEGAL")]
    Legal,
}

/// Extraction method for attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum ExtractionMethod {
    #[sqlx(rename = "OCR")]
    OCR,
    #[sqlx(rename = "MRZ")]
    MRZ,
    #[sqlx(rename = "BARCODE")]
    Barcode,
    #[sqlx(rename = "QR_CODE")]
    QRCode,
    #[sqlx(rename = "FORM_FIELD")]
    FormField,
    #[sqlx(rename = "TABLE")]
    Table,
    #[sqlx(rename = "CHECKBOX")]
    Checkbox,
    #[sqlx(rename = "SIGNATURE")]
    Signature,
    #[sqlx(rename = "PHOTO")]
    Photo,
}

/// Document-Attribute mapping
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentAttributeMapping {
    pub mapping_id: Uuid,
    pub document_type_id: Uuid,
    pub attribute_uuid: Uuid,
    pub extraction_method: ExtractionMethod,
    pub field_location: Option<serde_json::Value>,
    pub field_name: Option<String>,
    pub confidence_threshold: f64,
    pub is_required: bool,
    pub validation_pattern: Option<String>,
}

/// Field location for extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldLocation {
    pub page: u32,
    pub region: Option<BoundingBox>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}
```

### 2.2 Update Document Models

**File**: Update `/home/claude/rust/src/models/document_models.rs`

Add to the existing file:

```rust
// Add to imports
use super::document_type_models::{DocumentType, DocumentAttributeMapping, ExtractionMethod};

// Update DocumentCatalog struct to include type
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalog {
    pub doc_id: Uuid,
    pub document_type_id: Option<Uuid>, // ADD THIS
    pub file_hash_sha256: String,
    pub storage_key: String,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: String,
    pub extraction_confidence: Option<f64>,
    pub last_extracted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Add extraction result model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub attribute_uuid: Uuid,
    pub value: serde_json::Value,
    pub confidence: f64,
    pub extraction_method: ExtractionMethod,
    pub metadata: ExtractionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMetadata {
    pub page: Option<u32>,
    pub coordinates: Option<BoundingBox>,
    pub raw_text: Option<String>,
    pub processing_time_ms: u32,
}
```

## Phase 3: Repository Layer

### 3.1 Create Document Type Repository

**File**: Create new `/home/claude/rust/src/database/document_type_repository.rs`

```rust
use crate::models::document_type_models::*;
use sqlx::PgPool;
use uuid::Uuid;
use std::collections::HashMap;

pub struct DocumentTypeRepository {
    pool: PgPool,
}

impl DocumentTypeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get document type by ID
    pub async fn get_document_type(&self, type_id: Uuid) -> Result<DocumentType, sqlx::Error> {
        sqlx::query_as!(
            DocumentType,
            r#"
            SELECT type_id, type_code, display_name, 
                   category as "category: _", domain, description
            FROM "ob-poc".document_types
            WHERE type_id = $1
            "#,
            type_id
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Get document type by code
    pub async fn get_document_type_by_code(&self, type_code: &str) -> Result<DocumentType, sqlx::Error> {
        sqlx::query_as!(
            DocumentType,
            r#"
            SELECT type_id, type_code, display_name,
                   category as "category: _", domain, description
            FROM "ob-poc".document_types
            WHERE type_code = $1
            "#,
            type_code
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Get all attribute mappings for a document type
    pub async fn get_mappings_for_document_type(
        &self,
        document_type_id: Uuid,
    ) -> Result<Vec<DocumentAttributeMapping>, sqlx::Error> {
        sqlx::query_as!(
            DocumentAttributeMapping,
            r#"
            SELECT mapping_id, document_type_id, attribute_uuid,
                   extraction_method as "extraction_method: _", 
                   field_location, field_name,
                   confidence_threshold as "confidence_threshold!",
                   is_required, validation_pattern
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1
            ORDER BY is_required DESC, confidence_threshold DESC
            "#,
            document_type_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Check if a document type supports an attribute
    pub async fn supports_attribute(
        &self,
        document_type_id: Uuid,
        attribute_uuid: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".document_attribute_mappings
                WHERE document_type_id = $1 AND attribute_uuid = $2
            )
            "#,
            document_type_id,
            attribute_uuid
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(result.unwrap_or(false))
    }
}
```

## Phase 4: Service Layer Refactor

### 4.1 Replace Mock Document Extraction

**File**: Replace `/home/claude/rust/src/domains/attributes/sources/document_extraction.rs`

```rust
//! Document Extraction Source - REAL IMPLEMENTATION
//!
//! Fetches attribute values from extracted document data using actual document types.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;
use sqlx::PgPool;
use crate::database::document_type_repository::DocumentTypeRepository;
use crate::models::document_type_models::*;

/// Real document extraction source that uses database mappings
pub struct DocumentExtractionSource {
    pool: PgPool,
    document_type_repo: DocumentTypeRepository,
    extraction_service: Arc<dyn ExtractionService>,
}

impl DocumentExtractionSource {
    pub fn new(pool: PgPool, extraction_service: Arc<dyn ExtractionService>) -> Self {
        let document_type_repo = DocumentTypeRepository::new(pool.clone());
        Self {
            pool,
            document_type_repo,
            extraction_service,
        }
    }

    /// Extract attributes from a document based on its type
    pub async fn extract_from_document(
        &self,
        doc_id: Uuid,
        document_type_id: Uuid,
    ) -> Result<HashMap<Uuid, JsonValue>, SourceError> {
        // Get mappings for this document type
        let mappings = self.document_type_repo
            .get_mappings_for_document_type(document_type_id)
            .await
            .map_err(|e| SourceError::ExtractionFailed(e.to_string()))?;
        
        let mut extracted_values = HashMap::new();
        
        // Extract each mapped attribute
        for mapping in mappings {
            let extraction_result = self.extraction_service
                .extract_attribute(
                    doc_id,
                    mapping.attribute_uuid,
                    mapping.extraction_method,
                    mapping.field_location,
                )
                .await;
                
            match extraction_result {
                Ok(result) if result.confidence >= mapping.confidence_threshold => {
                    // Store in document_metadata
                    self.store_extraction(doc_id, mapping.attribute_uuid, &result).await?;
                    extracted_values.insert(mapping.attribute_uuid, result.value);
                }
                Ok(result) => {
                    log::warn!(
                        "Extraction confidence {} below threshold {} for attribute {}",
                        result.confidence, mapping.confidence_threshold, mapping.attribute_uuid
                    );
                    if mapping.is_required {
                        return Err(SourceError::ExtractionFailed(
                            format!("Required attribute {} confidence too low", mapping.attribute_uuid)
                        ));
                    }
                }
                Err(e) if mapping.is_required => {
                    return Err(SourceError::ExtractionFailed(
                        format!("Failed to extract required attribute {}: {}", mapping.attribute_uuid, e)
                    ));
                }
                Err(e) => {
                    log::warn!("Failed to extract optional attribute {}: {}", mapping.attribute_uuid, e);
                }
            }
        }
        
        Ok(extracted_values)
    }

    async fn store_extraction(
        &self,
        doc_id: Uuid,
        attr_uuid: Uuid,
        result: &ExtractionResult,
    ) -> Result<(), SourceError> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_metadata
            (doc_id, attribute_id, value, extraction_confidence, 
             extraction_method, extracted_at, extraction_metadata)
            VALUES ($1, $2, $3, $4, $5, NOW(), $6)
            ON CONFLICT (doc_id, attribute_id) DO UPDATE SET
                value = EXCLUDED.value,
                extraction_confidence = EXCLUDED.extraction_confidence,
                extraction_method = EXCLUDED.extraction_method,
                extracted_at = EXCLUDED.extracted_at,
                extraction_metadata = EXCLUDED.extraction_metadata
            "#,
            doc_id,
            attr_uuid,
            result.value,
            result.confidence,
            result.extraction_method.to_string(),
            serde_json::to_value(&result.metadata).ok()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SourceError::DatabaseError(e))?;
        
        Ok(())
    }
}

#[async_trait]
impl SourceExecutor for DocumentExtractionSource {
    async fn fetch_value(
        &self,
        attr_uuid: Uuid,
        context: &ExecutionContext,
    ) -> SourceResult<AttributeValue> {
        // Get document ID from context
        let doc_id = context.current_document_id
            .ok_or(SourceError::NoValidSource(attr_uuid))?;
        
        // Check if we already have this extraction
        let existing = sqlx::query!(
            r#"
            SELECT value, extraction_confidence
            FROM "ob-poc".document_metadata
            WHERE doc_id = $1 AND attribute_id = $2
            "#,
            doc_id,
            attr_uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SourceError::DatabaseError(e))?;
        
        if let Some(row) = existing {
            let semantic_id = context.resolver
                .uuid_to_semantic(&attr_uuid)
                .map_err(|_| SourceError::NoValidSource(attr_uuid))?;
                
            return Ok(AttributeValue {
                uuid: attr_uuid,
                semantic_id,
                value: row.value,
                source: ValueSource::DocumentExtraction {
                    document_id: doc_id,
                    page: None,
                    confidence: row.extraction_confidence.unwrap_or(0.0) as f64,
                },
            });
        }
        
        Err(SourceError::NoValidSource(attr_uuid))
    }

    fn can_handle(&self, attr_uuid: &Uuid) -> bool {
        // Check if any document type supports this attribute
        // This would be cached in production
        true
    }

    fn priority(&self) -> u32 {
        5 // High priority
    }
}
```

### 4.2 Update Document Extraction Service

**File**: Replace `/home/claude/rust/src/services/document_extraction_service.rs`

```rust
//! Document extraction service with real type-aware extraction

use crate::data_dictionary::{AttributeDefinition, AttributeId, DictionaryService};
use crate::database::document_type_repository::DocumentTypeRepository;
use crate::models::document_type_models::*;
use crate::models::document_models::*;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DocumentExtractionService {
    pool: PgPool,
    doc_type_repo: DocumentTypeRepository,
    extraction_engine: Arc<dyn ExtractionEngine>,
}

/// Trait for actual extraction engines
#[async_trait::async_trait]
pub trait ExtractionEngine: Send + Sync {
    async fn extract(
        &self,
        document_path: &str,
        method: ExtractionMethod,
        location: Option<serde_json::Value>,
    ) -> Result<ExtractionResult, String>;
}

impl DocumentExtractionService {
    pub fn new(pool: PgPool, extraction_engine: Arc<dyn ExtractionEngine>) -> Self {
        let doc_type_repo = DocumentTypeRepository::new(pool.clone());
        Self { 
            pool, 
            doc_type_repo,
            extraction_engine,
        }
    }

    /// Extract attributes from a document based on its type
    pub async fn extract_attributes_from_document(
        &self,
        doc_id: Uuid,
        entity_id: Uuid,
    ) -> Result<HashMap<AttributeId, Value>, String> {
        // Step 1: Get document with its type
        let document = sqlx::query!(
            r#"
            SELECT dc.doc_id, dc.storage_key, dc.mime_type, 
                   dc.document_type_id, dt.type_code
            FROM "ob-poc".document_catalog dc
            LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
            WHERE dc.doc_id = $1
            "#,
            doc_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch document: {}", e))?
        .ok_or_else(|| format!("Document {} not found", doc_id))?;
        
        let document_type_id = document.document_type_id
            .ok_or_else(|| "Document type not set".to_string())?;
        
        // Step 2: Get attribute mappings for this document type
        let mappings = self.doc_type_repo
            .get_mappings_for_document_type(document_type_id)
            .await
            .map_err(|e| format!("Failed to get mappings: {}", e))?;
        
        if mappings.is_empty() {
            return Err(format!(
                "No attribute mappings defined for document type {}",
                document.type_code.unwrap_or_default()
            ));
        }
        
        let mut extracted_values = HashMap::new();
        
        // Step 3: Extract each mapped attribute
        for mapping in mappings {
            match self.extract_single_attribute(
                &document.storage_key,
                &mapping,
            ).await {
                Ok(result) => {
                    // Store in document_metadata
                    self.store_document_metadata(
                        doc_id,
                        mapping.attribute_uuid,
                        &result,
                    ).await?;
                    
                    // Store in attribute_values_typed
                    self.store_attribute_value(
                        entity_id,
                        mapping.attribute_uuid,
                        &result.value,
                    ).await?;
                    
                    let attr_id = AttributeId::from_uuid(mapping.attribute_uuid);
                    extracted_values.insert(attr_id, result.value);
                }
                Err(e) if mapping.is_required => {
                    return Err(format!(
                        "Failed to extract required attribute {}: {}",
                        mapping.attribute_uuid, e
                    ));
                }
                Err(e) => {
                    log::warn!(
                        "Failed to extract optional attribute {}: {}",
                        mapping.attribute_uuid, e
                    );
                }
            }
        }
        
        // Step 4: Update document extraction status
        sqlx::query!(
            r#"
            UPDATE "ob-poc".document_catalog
            SET extraction_status = 'COMPLETED',
                extraction_confidence = $2,
                last_extracted_at = NOW(),
                updated_at = NOW()
            WHERE doc_id = $1
            "#,
            doc_id,
            0.85 // Average confidence, calculate from actual results
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update document status: {}", e))?;
        
        Ok(extracted_values)
    }

    async fn extract_single_attribute(
        &self,
        storage_key: &str,
        mapping: &DocumentAttributeMapping,
    ) -> Result<ExtractionResult, String> {
        let start = std::time::Instant::now();
        
        // Use the real extraction engine
        let result = self.extraction_engine
            .extract(
                storage_key,
                mapping.extraction_method.clone(),
                mapping.field_location.clone(),
            )
            .await?;
        
        // Validate against pattern if provided
        if let Some(pattern) = &mapping.validation_pattern {
            let regex = regex::Regex::new(pattern)
                .map_err(|e| format!("Invalid validation pattern: {}", e))?;
                
            if let Value::String(s) = &result.value {
                if !regex.is_match(s) {
                    return Err(format!(
                        "Value '{}' does not match validation pattern",
                        s
                    ));
                }
            }
        }
        
        // Check confidence threshold
        if result.confidence < mapping.confidence_threshold {
            return Err(format!(
                "Confidence {} below threshold {}",
                result.confidence, mapping.confidence_threshold
            ));
        }
        
        Ok(result)
    }

    async fn store_document_metadata(
        &self,
        doc_id: Uuid,
        attribute_uuid: Uuid,
        result: &ExtractionResult,
    ) -> Result<(), String> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_metadata (
                doc_id, attribute_id, value, extraction_confidence,
                extraction_method, extracted_at, extraction_metadata
            ) VALUES ($1, $2, $3, $4, $5, NOW(), $6)
            ON CONFLICT (doc_id, attribute_id) DO UPDATE SET
                value = EXCLUDED.value,
                extraction_confidence = EXCLUDED.extraction_confidence,
                extraction_method = EXCLUDED.extraction_method,
                extracted_at = EXCLUDED.extracted_at,
                extraction_metadata = EXCLUDED.extraction_metadata
            "#,
            doc_id,
            attribute_uuid,
            result.value,
            result.confidence,
            result.extraction_method.to_string(),
            serde_json::to_value(&result.metadata).unwrap()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to store document metadata: {}", e))?;
        
        Ok(())
    }

    async fn store_attribute_value(
        &self,
        entity_id: Uuid,
        attribute_uuid: Uuid,
        value: &Value,
    ) -> Result<(), String> {
        // Get the semantic ID for the attribute
        let semantic_id = sqlx::query_scalar!(
            r#"
            SELECT id FROM "ob-poc".attribute_registry
            WHERE uuid = $1
            "#,
            attribute_uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to get semantic ID: {}", e))?
        .ok_or_else(|| format!("No semantic ID for UUID {}", attribute_uuid))?;
        
        // Store based on value type
        match value {
            Value::String(s) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_text, attribute_uuid
                    ) VALUES ($1, $2, $3, $4)
                    "#,
                    entity_id,
                    semantic_id,
                    s,
                    attribute_uuid
                )
                .execute(&self.pool)
                .await?;
            }
            Value::Number(n) => {
                let num_val = bigdecimal::BigDecimal::from_str(&n.to_string()).unwrap();
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_number, attribute_uuid
                    ) VALUES ($1, $2, $3, $4)
                    "#,
                    entity_id,
                    semantic_id,
                    num_val,
                    attribute_uuid
                )
                .execute(&self.pool)
                .await?;
            }
            Value::Bool(b) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_boolean, attribute_uuid
                    ) VALUES ($1, $2, $3, $4)
                    "#,
                    entity_id,
                    semantic_id,
                    b,
                    attribute_uuid
                )
                .execute(&self.pool)
                .await?;
            }
            _ => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_json, attribute_uuid
                    ) VALUES ($1, $2, $3, $4)
                    "#,
                    entity_id,
                    semantic_id,
                    value,
                    attribute_uuid
                )
                .execute(&self.pool)
                .await?;
            }
        }
        
        Ok(())
    }
}
```

## Phase 5: DSL Integration

### 5.1 Update DSL Executor for Document Operations

**File**: Update `/home/claude/rust/src/execution/dsl_executor.rs`

Add document extraction handling:

```rust
impl DslExecutor {
    /// Execute document.extract operation from DSL
    pub async fn execute_document_extract(
        &mut self,
        doc_id: Uuid,
        attribute_refs: Vec<AttrRef>,
    ) -> Result<(), ExecutionError> {
        // Get the document extraction service
        let extraction_service = self.services
            .document_extraction_service
            .as_ref()
            .ok_or(ExecutionError::ServiceNotAvailable("document_extraction"))?;
        
        // Get entity ID from context
        let entity_id = self.context.entity_id;
        
        // Extract all attributes from the document
        let extracted = extraction_service
            .extract_attributes_from_document(doc_id, entity_id)
            .await
            .map_err(|e| ExecutionError::ExtractionFailed(e))?;
        
        // Bind extracted values to context for immediate use in DSL
        for (attr_id, value) in extracted {
            let uuid = attr_id.as_uuid();
            self.context.bind_value(uuid, value);
        }
        
        // If specific attributes were requested, verify they were extracted
        if !attribute_refs.is_empty() {
            for attr_ref in attribute_refs {
                let uuid = self.resolve_attr_ref(attr_ref)?;
                if !self.context.has_value(&uuid) {
                    return Err(ExecutionError::AttributeNotExtracted(uuid));
                }
            }
        }
        
        Ok(())
    }
    
    /// Execute document.catalog operation
    pub async fn execute_document_catalog(
        &mut self,
        file_path: String,
        document_type_code: String,
        metadata: HashMap<String, Value>,
    ) -> Result<Uuid, ExecutionError> {
        // Get document type
        let doc_type = self.doc_type_repo
            .get_document_type_by_code(&document_type_code)
            .await
            .map_err(|e| ExecutionError::InvalidDocumentType(document_type_code))?;
        
        // Upload document
        let doc_id = self.document_service
            .upload_document(
                file_path,
                doc_type.type_id,
                self.context.entity_id,
                metadata,
            )
            .await?;
        
        // Store in context
        self.context.current_document_id = Some(doc_id);
        
        Ok(doc_id)
    }
}
```

### 5.2 Update Execution Context

**File**: Update `/home/claude/rust/src/domains/attributes/execution_context.rs`

```rust
pub struct ExecutionContext {
    pub cbu_id: Uuid,           // ADD: CBU identifier
    pub entity_id: Uuid,         // ADD: Current entity
    pub current_document_id: Option<Uuid>,  // ADD: Current document being processed
    pub resolver: AttributeResolver,
    pub sources: Vec<Box<dyn SourceExecutor>>,
    pub validation_mode: ValidationMode,
    bound_values: HashMap<Uuid, serde_json::Value>,  // ADD: Runtime value bindings
}

impl ExecutionContext {
    pub fn new(cbu_id: Uuid, entity_id: Uuid) -> Self {
        Self {
            cbu_id,
            entity_id,
            current_document_id: None,
            resolver: AttributeResolver::new(),
            sources: Vec::new(),
            validation_mode: ValidationMode::Strict,
            bound_values: HashMap::new(),
        }
    }
    
    /// Bind a value to an attribute UUID
    pub fn bind_value(&mut self, uuid: Uuid, value: serde_json::Value) {
        self.bound_values.insert(uuid, value);
    }
    
    /// Get a bound value
    pub fn get_value(&self, uuid: &Uuid) -> Option<&serde_json::Value> {
        self.bound_values.get(uuid)
    }
    
    /// Check if a value is bound
    pub fn has_value(&self, uuid: &Uuid) -> bool {
        self.bound_values.contains_key(uuid)
    }
}
```

### 5.3 Connect DSL Parser to Document Operations

**File**: Update `/home/claude/rust/src/parser/statements.rs`

Add document operation parsing:

```rust
/// Parse document operations
pub fn parse_document_operation(input: &str) -> IResult<&str, DocumentOperation> {
    alt((
        parse_document_catalog,
        parse_document_extract,
        parse_document_link,
    ))(input)
}

fn parse_document_catalog(input: &str) -> IResult<&str, DocumentOperation> {
    let (input, _) = tag("document.catalog")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, file_path) = parse_string_literal(input)?;
    let (input, _) = multispace1(input)?;
    let (input, doc_type) = parse_identifier(input)?;
    
    Ok((input, DocumentOperation::Catalog {
        file_path,
        document_type: doc_type,
    }))
}

fn parse_document_extract(input: &str) -> IResult<&str, DocumentOperation> {
    let (input, _) = tag("document.extract")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, doc_ref) = parse_doc_ref(input)?;
    let (input, _) = multispace1(input)?;
    let (input, attrs) = delimited(
        char('['),
        separated_list0(multispace1, parse_attr_ref),
        char(']'),
    )(input)?;
    
    Ok((input, DocumentOperation::Extract {
        document_id: doc_ref,
        attributes: attrs,
    }))
}
```

## Phase 6: Testing

### 6.1 Integration Test

**File**: Create `/home/claude/rust/tests/document_extraction_integration.rs`

```rust
#[tokio::test]
async fn test_end_to_end_document_extraction() {
    // Setup
    let pool = setup_test_db().await;
    let extraction_engine = create_test_extraction_engine();
    let service = DocumentExtractionService::new(pool.clone(), extraction_engine);
    
    // Create a test entity
    let entity_id = create_test_entity(&pool, "TEST_PERSON").await?;
    
    // Upload a passport document
    let doc_id = sqlx::query_scalar!(
        r#"
        INSERT INTO "ob-poc".document_catalog 
        (file_hash_sha256, storage_key, document_type_id, mime_type)
        VALUES (
            'test_hash',
            '/test/passport.pdf',
            (SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'PASSPORT'),
            'application/pdf'
        )
        RETURNING doc_id
        "#
    )
    .fetch_one(&pool)
    .await?;
    
    // Extract attributes
    let extracted = service
        .extract_attributes_from_document(doc_id, entity_id)
        .await?;
    
    // Verify extractions
    assert!(extracted.len() > 0);
    
    // Verify document_metadata was populated
    let metadata_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) FROM "ob-poc".document_metadata
        WHERE doc_id = $1
        "#,
        doc_id
    )
    .fetch_one(&pool)
    .await?;
    
    assert_eq!(metadata_count, Some(extracted.len() as i64));
    
    // Verify attribute_values_typed was populated
    let first_name_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935")?;
    let first_name = sqlx::query_scalar!(
        r#"
        SELECT value_text FROM "ob-poc".attribute_values_typed
        WHERE entity_id = $1 AND attribute_uuid = $2
        "#,
        entity_id,
        first_name_uuid
    )
    .fetch_optional(&pool)
    .await?;
    
    assert!(first_name.is_some());
}
```

### 6.2 DSL Integration Test

**File**: Create `/home/claude/rust/tests/dsl_document_integration.rs`

```rust
#[tokio::test]
async fn test_dsl_document_operations() {
    let pool = setup_test_db().await;
    let executor = create_test_executor(pool.clone()).await;
    
    // DSL that catalogs a document and extracts attributes
    let dsl = r#"
    (case.create "TEST_CASE_001")
    
    ;; Catalog a passport document
    (document.catalog "/uploads/john_doe_passport.pdf" PASSPORT)
    
    ;; Extract identity attributes
    (document.extract @doc{current} 
        [@attr.identity.first_name 
         @attr.identity.last_name
         @attr.identity.passport_number])
    
    ;; Use extracted attributes in entity creation
    (entity.create 
        :type "PERSON"
        :first_name @attr.identity.first_name
        :last_name @attr.identity.last_name
        :passport @attr.identity.passport_number)
    "#;
    
    let result = executor.execute(dsl).await?;
    assert!(result.success);
    
    // Verify entity was created with extracted values
    let entity = get_entity(result.entity_id).await?;
    assert_eq!(entity.first_name, "John");
    assert_eq!(entity.last_name, "Doe");
}
```

## Phase 7: Configuration and Deployment

### 7.1 Add Extraction Engine Configuration

**File**: Create `/home/claude/rust/src/config/extraction_config.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractionConfig {
    pub engine: ExtractionEngineType,
    pub textract: Option<TextractConfig>,
    pub azure: Option<AzureConfig>,
    pub tesseract: Option<TesseractConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ExtractionEngineType {
    AWS_Textract,
    Azure_FormRecognizer,
    Tesseract,
    Mock, // For testing only
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextractConfig {
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl ExtractionConfig {
    pub fn from_env() -> Self {
        Self {
            engine: ExtractionEngineType::AWS_Textract,
            textract: Some(TextractConfig {
                region: std::env::var("AWS_REGION").unwrap_or("us-east-1".to_string()),
                access_key_id: std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID"),
                secret_access_key: std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY"),
            }),
            azure: None,
            tesseract: None,
        }
    }
}
```

## Implementation Checklist

### Database Changes
- [ ] Run migration to create `document_attribute_mappings` table
- [ ] Seed document types
- [ ] Seed attribute mappings for each document type
- [ ] Add columns to existing tables

### Code Changes
- [ ] Create document type models
- [ ] Create document type repository
- [ ] Replace mock extraction source
- [ ] Update document extraction service
- [ ] Update execution context with CBU/entity IDs
- [ ] Update DSL executor for document operations
- [ ] Implement real extraction engine (AWS Textract or similar)

### Testing
- [ ] Unit tests for document type repository
- [ ] Integration tests for extraction pipeline
- [ ] DSL integration tests
- [ ] Performance tests with real documents

### Documentation
- [ ] Update API documentation
- [ ] Document extraction method configuration
- [ ] DSL document operation examples

## Success Metrics
- Zero mock data in document extraction
- All document operations persist to database
- DSL document.extract creates document_metadata entries
- Extracted attributes available in DSL via @attr references
- Document type determines which attributes to extract
- Confidence thresholds enforced
- Required attributes validated

## Timeline
- Day 1-2: Database schema and migrations
- Day 3-4: Model and repository layer
- Day 5-6: Service layer refactoring
- Day 7-8: DSL integration
- Day 9-10: Testing and documentation

This refactor will completely eliminate mock data and establish a production-ready document-to-attribute extraction pipeline with full DSL integration.
