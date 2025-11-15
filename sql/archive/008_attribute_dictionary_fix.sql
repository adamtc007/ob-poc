-- Migration 008: Attribute Dictionary Fix
-- Adds proper typed attribute storage and extends existing document_catalog

-- 1. Create proper attribute_values_typed table
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_values_typed (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attribute_id UUID NOT NULL,
    cbu_id UUID NOT NULL,
    value_type VARCHAR(50) NOT NULL,
    string_value TEXT,
    numeric_value DECIMAL(20, 6),
    boolean_value BOOLEAN,
    date_value DATE,
    json_value JSONB,
    source_doc_id UUID,
    confidence_score FLOAT DEFAULT 1.0,
    extracted_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_attribute FOREIGN KEY (attribute_id) 
        REFERENCES "ob-poc".dictionary(attribute_id),
    CONSTRAINT fk_cbu FOREIGN KEY (cbu_id) 
        REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT fk_source_doc FOREIGN KEY (source_doc_id)
        REFERENCES "ob-poc".document_catalog(document_id) ON DELETE SET NULL,
    CONSTRAINT check_value_type CHECK (value_type IN ('string', 'numeric', 'boolean', 'date', 'json'))
);

-- 2. Extend document_catalog with cbu_id (only if column doesn't exist)
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'document_catalog' 
        AND column_name = 'cbu_id'
    ) THEN
        ALTER TABLE "ob-poc".document_catalog 
        ADD COLUMN cbu_id UUID;
        
        ALTER TABLE "ob-poc".document_catalog 
        ADD CONSTRAINT fk_document_catalog_cbu FOREIGN KEY (cbu_id) 
            REFERENCES "ob-poc".cbus(cbu_id);
            
        CREATE INDEX idx_document_catalog_cbu_new ON "ob-poc".document_catalog(cbu_id);
    END IF;
END $$;

-- 3. Extend document_catalog with extraction_status (only if column doesn't exist)
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'document_catalog' 
        AND column_name = 'extraction_status'
    ) THEN
        ALTER TABLE "ob-poc".document_catalog 
        ADD COLUMN extraction_status VARCHAR(50) DEFAULT 'pending';
    END IF;
END $$;

-- 4. Extend document_catalog with extraction_confidence (only if column doesn't exist)
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'document_catalog' 
        AND column_name = 'extraction_confidence'
    ) THEN
        ALTER TABLE "ob-poc".document_catalog 
        ADD COLUMN extraction_confidence FLOAT;
    END IF;
END $$;

-- 5. Create document_metadata for extraction tracking
CREATE TABLE IF NOT EXISTS "ob-poc".document_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL,
    attribute_id UUID NOT NULL,
    extracted_value TEXT,
    confidence FLOAT DEFAULT 0.0,
    page_number INTEGER,
    bounding_box JSONB,
    extraction_method VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_document FOREIGN KEY (document_id) 
        REFERENCES "ob-poc".document_catalog(document_id) ON DELETE CASCADE,
    CONSTRAINT fk_attribute FOREIGN KEY (attribute_id) 
        REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE
);

-- 6. Create attribute_sources and attribute_sinks tables
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attribute_id UUID NOT NULL,
    source_type VARCHAR(50) NOT NULL,
    source_config JSONB NOT NULL,
    priority INTEGER DEFAULT 0,
    CONSTRAINT fk_attribute FOREIGN KEY (attribute_id) 
        REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS "ob-poc".attribute_sinks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    attribute_id UUID NOT NULL,
    sink_type VARCHAR(50) NOT NULL,
    sink_config JSONB NOT NULL,
    CONSTRAINT fk_attribute FOREIGN KEY (attribute_id) 
        REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE
);

-- 7. Add indexes for new tables
CREATE INDEX IF NOT EXISTS idx_attribute_values_cbu ON "ob-poc".attribute_values_typed(cbu_id);
CREATE INDEX IF NOT EXISTS idx_attribute_values_attr ON "ob-poc".attribute_values_typed(attribute_id);
CREATE INDEX IF NOT EXISTS idx_attribute_values_source_doc ON "ob-poc".attribute_values_typed(source_doc_id);
CREATE INDEX IF NOT EXISTS idx_document_metadata_doc ON "ob-poc".document_metadata(document_id);
CREATE INDEX IF NOT EXISTS idx_document_metadata_attr ON "ob-poc".document_metadata(attribute_id);
CREATE INDEX IF NOT EXISTS idx_attribute_sources_attr ON "ob-poc".attribute_sources(attribute_id);
CREATE INDEX IF NOT EXISTS idx_attribute_sinks_attr ON "ob-poc".attribute_sinks(attribute_id);
