-- ============================================================================
-- Document-Attribute Mapping Table
-- ============================================================================
-- Defines which attributes can be extracted from which document types
-- This enables type-aware document processing and attribute extraction

SET search_path TO "ob-poc";

-- Document to Attribute Mapping Table
CREATE TABLE IF NOT EXISTS "ob-poc".document_attribute_mappings (
    mapping_id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    document_type_id UUID NOT NULL REFERENCES "ob-poc".document_types(type_id) ON DELETE CASCADE,
    attribute_uuid UUID NOT NULL REFERENCES "ob-poc".attribute_registry(uuid) ON DELETE CASCADE,

    -- Extraction configuration
    extraction_method VARCHAR(50) NOT NULL CHECK (extraction_method IN (
        'OCR', 'MRZ', 'BARCODE', 'QR_CODE', 'FORM_FIELD',
        'TABLE', 'CHECKBOX', 'SIGNATURE', 'PHOTO', 'NLP', 'AI'
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

-- Indexes for fast lookups
CREATE INDEX IF NOT EXISTS idx_doc_attr_mappings_doc_type
    ON "ob-poc".document_attribute_mappings(document_type_id);
CREATE INDEX IF NOT EXISTS idx_doc_attr_mappings_attr
    ON "ob-poc".document_attribute_mappings(attribute_uuid);

-- Add document type column to document_catalog if missing
ALTER TABLE "ob-poc".document_catalog
ADD COLUMN IF NOT EXISTS document_type_id UUID REFERENCES "ob-poc".document_types(type_id);

-- Add extraction metadata to document_metadata
ALTER TABLE "ob-poc".document_metadata
ADD COLUMN IF NOT EXISTS extraction_confidence NUMERIC(3,2),
ADD COLUMN IF NOT EXISTS extraction_method VARCHAR(50),
ADD COLUMN IF NOT EXISTS extracted_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS extraction_metadata JSONB;

COMMENT ON TABLE "ob-poc".document_attribute_mappings IS 'Maps document types to extractable attributes with extraction methods';
COMMENT ON COLUMN "ob-poc".document_attribute_mappings.extraction_method IS 'Method used to extract the attribute: OCR, MRZ, BARCODE, FORM_FIELD, etc.';
COMMENT ON COLUMN "ob-poc".document_attribute_mappings.confidence_threshold IS 'Minimum confidence score (0.0-1.0) required for extraction';
