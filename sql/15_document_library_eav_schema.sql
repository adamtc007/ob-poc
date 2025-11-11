-- ============================================================================
-- OB-POC DOCUMENT LIBRARY SCHEMA (EAV/Metadata-Driven)
-- ============================================================================
--
-- This schema implements a flexible, EAV-style document catalog where
-- all indexing metadata (like 'document type', 'issuer', 'domain', etc.)
-- is stored as attributes, linking directly to the master "dictionary" table.
--
-- This design is based on the insight to treat metadata categories as
-- data, not as physical tables.
--
-- ============================================================================

-- ============================================================================
-- TABLE 1: DOCUMENT CATALOG (The "Entity" Table)
-- ============================================================================
-- Stores the central "fact" of each document instance.
--
CREATE TABLE IF NOT EXISTS "ob-poc".document_catalog (
    doc_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- File binary metadata
    file_hash_sha256 TEXT NOT NULL UNIQUE,
    storage_key TEXT NOT NULL, -- e.g., S3 key, internal path
    file_size_bytes BIGINT,
    mime_type VARCHAR(100),

    -- AI extraction results from EBNF (document.catalog :extracted-data {...})
    extracted_data JSONB,

    -- AI processing metadata
    extraction_status VARCHAR(50) DEFAULT 'PENDING',
    extraction_confidence NUMERIC(5,4),
    last_extracted_at TIMESTAMPTZ,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- ============================================================================
-- TABLE 2: DOCUMENT METADATA (The "Attribute-Value" Table)
-- ============================================================================
-- This is the EAV table for all indexing metadata.
-- This table is the "CRITICAL BRIDGE" to the DSL and dictionary.
--
CREATE TABLE IF NOT EXISTS "ob-poc".document_metadata (
    doc_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE,

    -- === THE CRITICAL BRIDGE ===
    -- Foreign key to the master dictionary.
    -- This defines *what* this metadata is (e.g., "Document Type", "Issuer").
    attribute_id UUID NOT NULL REFERENCES "ob-poc".dictionary(attribute_id) ON DELETE CASCADE,

    -- The flexible value for the attribute (e.g., "Passport", "UK Home Office").
    -- Using JSONB to support strings, numbers, booleans, or structured values.
    value JSONB NOT NULL,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    -- Each attribute can only be set once per document.
    PRIMARY KEY (doc_id, attribute_id)
);

-- ============================================================================
-- TABLE 3: DOCUMENT RELATIONSHIPS (The "Link" Table - M:N)
-- ============================================================================
-- Models many-to-many relationships between documents.
-- Implements the `document.link` EBNF verb.
--
CREATE TABLE IF NOT EXISTS "ob-poc".document_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The "source" document of the relationship
    primary_doc_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE,

    -- The "target" document of the relationship
    related_doc_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE,

    -- Type of relationship (e.g., "AMENDS", "SUPERSEDES", "IS_TRANSLATION_OF")
    relationship_type VARCHAR(100) NOT NULL,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    -- Prevent duplicate relationships
    UNIQUE (primary_doc_id, related_doc_id, relationship_type)
);

-- ============================================================================
-- TABLE 4: DOCUMENT USAGE (The "Link" Table - M:N)
-- ============================================================================
-- Models many-to-many relationships between documents and CBUs/Workflows.
-- Implements the `document.use` EBNF verb.
--
CREATE TABLE IF NOT EXISTS "ob-poc".document_usage (
    usage_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The document being used
    doc_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id) ON DELETE CASCADE,

    -- The CBU (onboarding case) that is using the document
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,

    -- Optional: Link to a specific entity this document provides evidence for
    entity_id UUID REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,

    -- Optional: Context for the usage (e.g., "EVIDENCE_OF_ADDRESS")
    usage_context VARCHAR(255),

    -- Timestamps
    used_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    -- Prevent duplicate usage entries
    UNIQUE (doc_id, cbu_id, entity_id, usage_context)
);

-- ============================================================================
-- INDEXES
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_doc_catalog_hash ON "ob-poc".document_catalog (file_hash_sha256);
CREATE INDEX IF NOT EXISTS idx_doc_catalog_status ON "ob-poc".document_catalog (extraction_status);

CREATE INDEX IF NOT EXISTS idx_doc_meta_doc_id ON "ob-poc".document_metadata (doc_id);
CREATE INDEX IF NOT EXISTS idx_doc_meta_attr_id ON "ob-poc".document_metadata (attribute_id);
-- This GIN index allows fast searching on the metadata *values*
CREATE INDEX IF NOT EXISTS idx_doc_meta_value_gin ON "ob-poc".document_metadata USING GIN (value jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_doc_rel_primary ON "ob-poc".document_relationships (primary_doc_id);
CREATE INDEX IF NOT EXISTS idx_doc_rel_related ON "ob-poc".document_relationships (related_doc_id);

CREATE INDEX IF NOT EXISTS idx_doc_usage_doc ON "ob-poc".document_usage (doc_id);
CREATE INDEX IF NOT EXISTS idx_doc_usage_cbu ON "ob-poc".document_usage (cbu_id);
CREATE INDEX IF NOT EXISTS idx_doc_usage_entity ON "ob-poc".document_usage (entity_id);

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================
COMMENT ON TABLE "ob-poc".document_catalog IS 'Central "fact" table for all document instances. Stores file info and AI extraction results.';
COMMENT ON TABLE "ob-poc".document_metadata IS 'EAV table linking documents to their metadata attributes (from the dictionary). This is the critical bridge to the AttributeID-as-Type pattern.';
COMMENT ON TABLE "ob-poc".document_relationships IS 'Models M:N relationships between documents (e.g., amendments, translations).';
COMMENT ON TABLE "ob-poc".document_usage IS 'Tracks M:N linkage of documents to CBUs, workflows, and entities as evidence.';

-- ============================================================================
-- HELPER VIEWS
-- ============================================================================

-- View to easily query documents with their metadata
CREATE OR REPLACE VIEW "ob-poc".document_catalog_with_metadata AS
SELECT
    dc.doc_id,
    dc.file_hash_sha256,
    dc.storage_key,
    dc.file_size_bytes,
    dc.mime_type,
    dc.extracted_data,
    dc.extraction_status,
    dc.extraction_confidence,
    dc.last_extracted_at,
    dc.created_at,
    dc.updated_at,
    -- Aggregate all metadata as JSONB object
    COALESCE(
        jsonb_object_agg(
            d.name,
            dm.value
        ) FILTER (WHERE dm.attribute_id IS NOT NULL),
        '{}'::jsonb
    ) AS metadata
FROM "ob-poc".document_catalog dc
LEFT JOIN "ob-poc".document_metadata dm ON dc.doc_id = dm.doc_id
LEFT JOIN "ob-poc".dictionary d ON dm.attribute_id = d.attribute_id
GROUP BY
    dc.doc_id, dc.file_hash_sha256, dc.storage_key, dc.file_size_bytes,
    dc.mime_type, dc.extracted_data, dc.extraction_status,
    dc.extraction_confidence, dc.last_extracted_at, dc.created_at, dc.updated_at;

COMMENT ON VIEW "ob-poc".document_catalog_with_metadata IS 'Convenient view showing documents with all their metadata aggregated as JSONB';
