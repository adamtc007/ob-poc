-- Migration 006: Attribute Extraction Log Table
-- Purpose: Track all document attribute extraction attempts for audit and debugging
-- Date: 2025-11-14

-- Create attribute_extraction_log table for comprehensive extraction tracking
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_extraction_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL,
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE,
    extraction_method TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    extracted_value JSONB,
    confidence_score FLOAT,
    error_message TEXT,
    processing_time_ms INTEGER,
    extracted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    metadata JSONB
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_extraction_log_cbu ON "ob-poc".attribute_extraction_log(cbu_id);
CREATE INDEX IF NOT EXISTS idx_extraction_log_document ON "ob-poc".attribute_extraction_log(document_id);
CREATE INDEX IF NOT EXISTS idx_extraction_log_attribute ON "ob-poc".attribute_extraction_log(attribute_id);
CREATE INDEX IF NOT EXISTS idx_extraction_log_timestamp ON "ob-poc".attribute_extraction_log(extracted_at DESC);
CREATE INDEX IF NOT EXISTS idx_extraction_log_success ON "ob-poc".attribute_extraction_log(success) WHERE success = false;

COMMENT ON TABLE "ob-poc".attribute_extraction_log IS 
'Audit log for all attribute extraction attempts from documents. Tracks success/failure rates and performance metrics.';
