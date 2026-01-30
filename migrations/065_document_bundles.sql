-- Migration 065: Document Bundles System
-- Implements versioned document requirement bundles for structure macros
-- Design doc: TODO-cbu-structure-macros-v3.md

-- ============================================================================
-- SECTION 1: Document Bundle Registry
-- ============================================================================

-- Master table for document bundles (e.g., 'docs.bundle.ucits-baseline')
CREATE TABLE IF NOT EXISTS "ob-poc".document_bundles (
    bundle_id VARCHAR(100) PRIMARY KEY,  -- e.g., 'docs.bundle.ucits-baseline'
    display_name VARCHAR(200) NOT NULL,
    description TEXT,
    version VARCHAR(20) NOT NULL,        -- Semver-style: '2024-03'
    effective_from DATE NOT NULL,
    effective_to DATE,                   -- NULL = currently active
    extends VARCHAR(100) REFERENCES "ob-poc".document_bundles(bundle_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure we don't have overlapping effective dates for same bundle
    CONSTRAINT valid_dates CHECK (effective_to IS NULL OR effective_to > effective_from)
);

COMMENT ON TABLE "ob-poc".document_bundles IS
'Registry of versioned document bundles that define required documents for structure types';

COMMENT ON COLUMN "ob-poc".document_bundles.extends IS
'Parent bundle ID - child inherits all documents from parent, can override';

-- ============================================================================
-- SECTION 2: Bundle Document Requirements
-- ============================================================================

-- Documents required by each bundle
CREATE TABLE IF NOT EXISTS "ob-poc".bundle_documents (
    bundle_id VARCHAR(100) NOT NULL REFERENCES "ob-poc".document_bundles(bundle_id) ON DELETE CASCADE,
    document_id VARCHAR(50) NOT NULL,        -- e.g., 'prospectus', 'kiid', 'lpa'
    document_name VARCHAR(200) NOT NULL,     -- Human-readable name
    document_description TEXT,
    required BOOLEAN NOT NULL DEFAULT true,
    required_if VARCHAR(200),                -- Condition expression (e.g., 'has-prime-broker')
    template_ref VARCHAR(200),               -- Optional reference to document template
    sort_order INT NOT NULL DEFAULT 0,       -- Display order within bundle

    PRIMARY KEY (bundle_id, document_id)
);

COMMENT ON TABLE "ob-poc".bundle_documents IS
'Documents required within each bundle. required_if allows conditional requirements.';

COMMENT ON COLUMN "ob-poc".bundle_documents.required_if IS
'Condition expression for conditional requirements (e.g., "has-prime-broker", "umbrella == true")';

-- Index for lookups
CREATE INDEX IF NOT EXISTS idx_bundle_documents_bundle
    ON "ob-poc".bundle_documents(bundle_id);

-- ============================================================================
-- SECTION 3: Applied Bundles (Audit Trail)
-- ============================================================================

-- Track when bundles are applied to CBUs (audit trail)
CREATE TABLE IF NOT EXISTS "ob-poc".applied_bundles (
    applied_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    bundle_id VARCHAR(100) NOT NULL REFERENCES "ob-poc".document_bundles(bundle_id),
    bundle_version VARCHAR(20) NOT NULL,     -- Version at time of application
    macro_id VARCHAR(100),                   -- Which macro applied this bundle
    macro_invocation_id UUID,                -- FK to macro_invocations if exists
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    applied_by VARCHAR(100),                 -- User or system that applied

    -- One bundle per CBU (can be re-applied/upgraded)
    CONSTRAINT unique_bundle_per_cbu UNIQUE (cbu_id, bundle_id)
);

COMMENT ON TABLE "ob-poc".applied_bundles IS
'Audit trail of document bundles applied to CBUs. Links macro invocations to their bundle application.';

-- Index for lookups
CREATE INDEX IF NOT EXISTS idx_applied_bundles_cbu
    ON "ob-poc".applied_bundles(cbu_id);

CREATE INDEX IF NOT EXISTS idx_applied_bundles_bundle
    ON "ob-poc".applied_bundles(bundle_id);

-- ============================================================================
-- SECTION 3.5: Add unique constraint for CBU-level document requirements
-- ============================================================================

-- Add partial unique index for CBU-level requirements (not covered by original migration)
-- This allows one requirement per doc_type per CBU when there's no workflow/entity context
CREATE UNIQUE INDEX IF NOT EXISTS idx_doc_req_cbu_doctype_unique
    ON "ob-poc".document_requirements(subject_cbu_id, doc_type)
    WHERE workflow_instance_id IS NULL AND subject_entity_id IS NULL;

-- ============================================================================
-- SECTION 4: Resolved Bundle View (with inheritance)
-- ============================================================================

-- View that resolves bundle inheritance for complete document list
CREATE OR REPLACE VIEW "ob-poc".v_resolved_bundle_documents AS
WITH RECURSIVE bundle_chain AS (
    -- Base: start with the bundle
    SELECT
        b.bundle_id,
        b.bundle_id AS root_bundle_id,
        b.extends,
        1 AS depth
    FROM "ob-poc".document_bundles b

    UNION ALL

    -- Recursive: walk up inheritance chain
    SELECT
        parent.bundle_id,
        bc.root_bundle_id,
        parent.extends,
        bc.depth + 1
    FROM bundle_chain bc
    JOIN "ob-poc".document_bundles parent ON parent.bundle_id = bc.extends
    WHERE bc.depth < 10  -- Safety limit
),
-- Get all documents from the bundle and its parents
all_docs AS (
    SELECT
        bc.root_bundle_id,
        bd.document_id,
        bd.document_name,
        bd.document_description,
        bd.required,
        bd.required_if,
        bd.template_ref,
        bd.sort_order,
        bc.depth,
        bc.bundle_id AS source_bundle_id
    FROM bundle_chain bc
    JOIN "ob-poc".bundle_documents bd ON bd.bundle_id = bc.bundle_id
)
-- Child bundle documents override parent (lower depth wins)
SELECT DISTINCT ON (root_bundle_id, document_id)
    root_bundle_id AS bundle_id,
    document_id,
    document_name,
    document_description,
    required,
    required_if,
    template_ref,
    sort_order,
    source_bundle_id
FROM all_docs
ORDER BY root_bundle_id, document_id, depth ASC;

COMMENT ON VIEW "ob-poc".v_resolved_bundle_documents IS
'Resolves bundle inheritance - returns complete document list including inherited documents. Child overrides parent.';

-- ============================================================================
-- SECTION 5: Functions for Bundle Operations
-- ============================================================================

-- Function to apply a bundle to a CBU (creates document_requirements)
CREATE OR REPLACE FUNCTION "ob-poc".apply_document_bundle(
    p_cbu_id UUID,
    p_bundle_id VARCHAR(100),
    p_macro_id VARCHAR(100) DEFAULT NULL,
    p_applied_by VARCHAR(100) DEFAULT 'system',
    p_context JSONB DEFAULT '{}'::JSONB  -- For evaluating required_if conditions
)
RETURNS TABLE (
    requirement_id UUID,
    document_id VARCHAR(50),
    document_name VARCHAR(200),
    required BOOLEAN,
    status TEXT
) AS $$
DECLARE
    v_bundle_version VARCHAR(20);
    v_applied_id UUID;
    v_doc RECORD;
    v_req_id UUID;
    v_should_create BOOLEAN;
BEGIN
    -- Get current bundle version
    SELECT version INTO v_bundle_version
    FROM "ob-poc".document_bundles
    WHERE bundle_id = p_bundle_id
      AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
      AND effective_from <= CURRENT_DATE;

    IF v_bundle_version IS NULL THEN
        RAISE EXCEPTION 'Bundle % not found or not effective', p_bundle_id;
    END IF;

    -- Record bundle application (upsert)
    INSERT INTO "ob-poc".applied_bundles (
        cbu_id, bundle_id, bundle_version, macro_id, applied_by
    ) VALUES (
        p_cbu_id, p_bundle_id, v_bundle_version, p_macro_id, p_applied_by
    )
    ON CONFLICT (cbu_id, bundle_id) DO UPDATE
        SET bundle_version = EXCLUDED.bundle_version,
            macro_id = COALESCE(EXCLUDED.macro_id, applied_bundles.macro_id),
            applied_at = NOW(),
            applied_by = EXCLUDED.applied_by
    RETURNING applied_id INTO v_applied_id;

    -- Create document requirements for each document in resolved bundle
    FOR v_doc IN
        SELECT * FROM "ob-poc".v_resolved_bundle_documents
        WHERE bundle_id = p_bundle_id
        ORDER BY sort_order, document_id
    LOOP
        -- Evaluate required_if condition if present
        v_should_create := TRUE;
        IF v_doc.required_if IS NOT NULL THEN
            -- Simple key lookup in context (e.g., "has-prime-broker")
            -- More complex expressions could be added later
            IF p_context ? v_doc.required_if THEN
                v_should_create := (p_context->>v_doc.required_if)::BOOLEAN;
            ELSE
                -- Default: required_if key not in context = skip this doc
                v_should_create := FALSE;
            END IF;
        END IF;

        IF NOT v_should_create THEN
            CONTINUE;
        END IF;

        -- Create or update document requirement
        INSERT INTO "ob-poc".document_requirements (
            subject_cbu_id,
            doc_type,
            required_state,
            status
        ) VALUES (
            p_cbu_id,
            v_doc.document_id,
            CASE WHEN v_doc.required THEN 'verified' ELSE 'received' END,
            'missing'
        )
        ON CONFLICT (workflow_instance_id, subject_entity_id, doc_type)
        WHERE workflow_instance_id IS NULL AND subject_entity_id IS NULL
        DO UPDATE SET updated_at = NOW()
        RETURNING "ob-poc".document_requirements.requirement_id INTO v_req_id;

        -- Return result row
        requirement_id := v_req_id;
        document_id := v_doc.document_id;
        document_name := v_doc.document_name;
        required := v_doc.required;
        status := 'missing';
        RETURN NEXT;
    END LOOP;

    RETURN;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".apply_document_bundle IS
'Applies a document bundle to a CBU, creating document_requirements for each document. Handles inheritance and required_if conditions.';

-- Function to get effective bundle for a date (for audits/replay)
CREATE OR REPLACE FUNCTION "ob-poc".get_effective_bundle(
    p_bundle_id VARCHAR(100),
    p_effective_date DATE DEFAULT CURRENT_DATE
)
RETURNS "ob-poc".document_bundles AS $$
    SELECT *
    FROM "ob-poc".document_bundles
    WHERE bundle_id = p_bundle_id
      AND effective_from <= p_effective_date
      AND (effective_to IS NULL OR effective_to > p_effective_date)
    ORDER BY effective_from DESC
    LIMIT 1;
$$ LANGUAGE SQL STABLE;

-- ============================================================================
-- SECTION 6: Stats View
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_bundle_stats AS
SELECT
    b.bundle_id,
    b.display_name,
    b.version,
    b.effective_from,
    b.effective_to,
    b.extends,
    (SELECT COUNT(*) FROM "ob-poc".bundle_documents bd WHERE bd.bundle_id = b.bundle_id) AS direct_document_count,
    (SELECT COUNT(*) FROM "ob-poc".v_resolved_bundle_documents vr WHERE vr.bundle_id = b.bundle_id) AS total_document_count,
    (SELECT COUNT(*) FROM "ob-poc".applied_bundles ab WHERE ab.bundle_id = b.bundle_id) AS cbu_count
FROM "ob-poc".document_bundles b;

COMMENT ON VIEW "ob-poc".v_bundle_stats IS
'Statistics for each document bundle including document counts and CBU application count';
