-- Migration 055: Client Group Research Discovery Buffer
-- Extends client_group infrastructure for research discovery workflow:
--   1. client_group_entity_roles - Junction table for proper FK roles (replaces TEXT[] tags)
--   2. client_group_relationship - Provisional ownership edges (before promotion)
--   3. client_group_relationship_sources - Multi-source lineage for trust-but-verify
--   4. Extends client_group with discovery status
--   5. Seeds missing structural/fund roles
--   6. Views for intent pipeline resolution and discrepancy detection
--
-- Design: Client group as a scoped working set for entity discovery.
-- Roles provide typed context for intent pipeline resolution.
-- Ownership edges are provisional until promoted to entity_relationships.

BEGIN;

-- ============================================================================
-- 1. Extend client_group_entity with review workflow columns
-- ============================================================================

-- Add review workflow columns to existing table
ALTER TABLE "ob-poc".client_group_entity
ADD COLUMN IF NOT EXISTS source_record_id VARCHAR(255),
ADD COLUMN IF NOT EXISTS review_status VARCHAR(20) NOT NULL DEFAULT 'pending',
ADD COLUMN IF NOT EXISTS reviewed_by VARCHAR(100),
ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS review_notes TEXT;

-- Add check constraint for review_status
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_cge_review_status'
    ) THEN
        ALTER TABLE "ob-poc".client_group_entity
        ADD CONSTRAINT chk_cge_review_status
        CHECK (review_status IN ('pending', 'confirmed', 'rejected', 'needs_update'));
    END IF;
END $$;

-- Add check constraint for membership_type (update to include all values)
ALTER TABLE "ob-poc".client_group_entity DROP CONSTRAINT IF EXISTS chk_cge_membership_type;
ALTER TABLE "ob-poc".client_group_entity
ADD CONSTRAINT chk_cge_membership_type
CHECK (membership_type IN ('in_group', 'confirmed', 'suspected', 'external_partner', 'counterparty', 'service_provider', 'historical'));

-- Index for pending reviews
CREATE INDEX IF NOT EXISTS idx_cge_review
ON "ob-poc".client_group_entity (group_id, review_status)
WHERE review_status IN ('pending', 'needs_update');

COMMENT ON COLUMN "ob-poc".client_group_entity.review_status IS
'Review workflow status: pending (awaiting review), confirmed (approved), rejected (marked for removal), needs_update (data changed)';

-- ============================================================================
-- 2. Create client_group_entity_roles junction table
-- Same pattern as cbu_entity_roles but for client group context
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".client_group_entity_roles (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    cge_id UUID NOT NULL REFERENCES "ob-poc".client_group_entity(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id),

    -- Context: what is this role relative to?
    -- e.g., BlackRock is IM *for* a specific fund within the group
    target_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Effective period
    effective_from DATE,
    effective_to DATE,  -- NULL = current

    -- Discovery metadata
    assigned_by TEXT NOT NULL DEFAULT 'manual',
    source_record_id VARCHAR(255),

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Unique constraint: Same entity can have same role only once per target (or NULL target)
-- Using partial unique index for NULL handling
CREATE UNIQUE INDEX IF NOT EXISTS uq_cger_role_target
ON "ob-poc".client_group_entity_roles (cge_id, role_id, COALESCE(target_entity_id, '00000000-0000-0000-0000-000000000000'));

-- Find all roles for an entity in a group
CREATE INDEX IF NOT EXISTS idx_cger_cge ON "ob-poc".client_group_entity_roles (cge_id);

-- Find all entities with a specific role in a group (HOT PATH for intent resolution)
CREATE INDEX IF NOT EXISTS idx_cger_role ON "ob-poc".client_group_entity_roles (role_id);

-- Find roles by target entity
CREATE INDEX IF NOT EXISTS idx_cger_target ON "ob-poc".client_group_entity_roles (target_entity_id)
WHERE target_entity_id IS NOT NULL;

-- Find current roles (no end date set)
CREATE INDEX IF NOT EXISTS idx_cger_current ON "ob-poc".client_group_entity_roles (cge_id)
WHERE effective_to IS NULL;

COMMENT ON TABLE "ob-poc".client_group_entity_roles IS
'Junction table linking client_group_entity to roles. Same pattern as cbu_entity_roles. Used for intent pipeline scoped search.';

COMMENT ON COLUMN "ob-poc".client_group_entity_roles.target_entity_id IS
'For directed roles (e.g., Investment Manager FOR a specific fund within the group)';

COMMENT ON COLUMN "ob-poc".client_group_entity_roles.assigned_by IS
'Source: manual, gleif, bods, auto_tag, agent';

-- ============================================================================
-- 3. Create client_group_relationship table (provisional ownership edges)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".client_group_relationship (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id) ON DELETE CASCADE,

    -- The edge: parent → child
    parent_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Relationship classification
    relationship_kind VARCHAR(30) NOT NULL DEFAULT 'ownership',

    -- Effective period (from source documents)
    effective_from DATE,
    effective_to DATE,  -- NULL = current

    -- Review workflow for the edge itself
    review_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    reviewed_by VARCHAR(100),
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,

    -- Promotion tracking (link to formal entity_relationships)
    promoted_to_relationship_id UUID REFERENCES "ob-poc".entity_relationships(relationship_id),
    promoted_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT chk_cgr_relationship_kind CHECK (relationship_kind IN (
        'ownership', 'control', 'beneficial', 'management'
    )),
    CONSTRAINT chk_cgr_review_status CHECK (review_status IN (
        'pending', 'confirmed', 'rejected', 'needs_update'
    )),
    CONSTRAINT chk_cgr_no_self_reference CHECK (parent_entity_id != child_entity_id)
);

-- Same edge can exist with different relationship_kinds
CREATE UNIQUE INDEX IF NOT EXISTS uq_cgr_edge
ON "ob-poc".client_group_relationship (group_id, parent_entity_id, child_entity_id, relationship_kind);

-- Find all relationships for an entity (as parent or child)
CREATE INDEX IF NOT EXISTS idx_cgr_parent ON "ob-poc".client_group_relationship (group_id, parent_entity_id);
CREATE INDEX IF NOT EXISTS idx_cgr_child ON "ob-poc".client_group_relationship (group_id, child_entity_id);

-- Find unpromoted relationships
CREATE INDEX IF NOT EXISTS idx_cgr_unpromoted ON "ob-poc".client_group_relationship (group_id)
WHERE promoted_to_relationship_id IS NULL;

-- Find relationships by kind
CREATE INDEX IF NOT EXISTS idx_cgr_kind ON "ob-poc".client_group_relationship (group_id, relationship_kind);

COMMENT ON TABLE "ob-poc".client_group_relationship IS
'Provisional ownership/control edges within a client group. Staging area before promotion to formal entity_relationships.';

-- ============================================================================
-- 4. Create client_group_relationship_sources table (multi-source lineage)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".client_group_relationship_sources (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    relationship_id UUID NOT NULL REFERENCES "ob-poc".client_group_relationship(id) ON DELETE CASCADE,

    -- Source identification
    source VARCHAR(50) NOT NULL,
    source_type VARCHAR(20) NOT NULL DEFAULT 'discovery',

    -- Ownership values from this source
    ownership_pct NUMERIC(5,2),
    voting_pct NUMERIC(5,2),
    control_pct NUMERIC(5,2),

    -- Provenance / lineage
    source_document_ref VARCHAR(255),
    source_document_type VARCHAR(100),
    source_document_date DATE,
    source_effective_date DATE,
    source_retrieved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_retrieved_by VARCHAR(100),
    raw_payload JSONB,

    -- Allegation → verification linkage
    verifies_source_id UUID REFERENCES "ob-poc".client_group_relationship_sources(id),

    -- Verification outcome
    verification_outcome VARCHAR(20),
    discrepancy_pct NUMERIC(5,2),

    -- Canonical selection (analyst override)
    is_canonical BOOLEAN DEFAULT false,
    canonical_set_by VARCHAR(100),
    canonical_set_at TIMESTAMPTZ,
    canonical_notes TEXT,

    -- KYC workflow status
    verification_status VARCHAR(20) DEFAULT 'unverified',
    verified_by VARCHAR(100),
    verified_at TIMESTAMPTZ,
    verification_notes TEXT,

    -- Confidence / quality
    confidence_score NUMERIC(3,2),
    is_direct_evidence BOOLEAN DEFAULT false,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT chk_cgrs_source CHECK (source IN (
        'client_allegation', 'gleif', 'bods', 'companies_house', 'clearstream',
        'annual_report', 'fund_prospectus', 'kyc_document', 'scraper', 'manual'
    )),
    CONSTRAINT chk_cgrs_source_type CHECK (source_type IN (
        'allegation', 'verification', 'discovery'
    )),
    CONSTRAINT chk_cgrs_verification_outcome CHECK (verification_outcome IS NULL OR verification_outcome IN (
        'confirmed', 'disputed', 'partial', 'superseded'
    )),
    CONSTRAINT chk_cgrs_verification_status CHECK (verification_status IN (
        'unverified', 'verified', 'disputed', 'superseded', 'rejected'
    )),
    -- Verification must reference an allegation
    CONSTRAINT chk_cgrs_verification_needs_target CHECK (
        (source_type = 'verification' AND verifies_source_id IS NOT NULL) OR
        (source_type != 'verification')
    ),
    -- Confidence score range
    CONSTRAINT chk_cgrs_confidence_range CHECK (
        confidence_score IS NULL OR (confidence_score >= 0.00 AND confidence_score <= 1.00)
    )
);

-- Only one canonical source per relationship (partial unique index)
CREATE UNIQUE INDEX IF NOT EXISTS uq_cgrs_canonical
ON "ob-poc".client_group_relationship_sources (relationship_id)
WHERE is_canonical = true;

-- Find all sources for a relationship
CREATE INDEX IF NOT EXISTS idx_cgrs_relationship ON "ob-poc".client_group_relationship_sources (relationship_id);

-- Find unverified allegations
CREATE INDEX IF NOT EXISTS idx_cgrs_unverified ON "ob-poc".client_group_relationship_sources (source_type, verification_status)
WHERE source_type = 'allegation' AND verification_status = 'unverified';

-- Find canonical sources
CREATE INDEX IF NOT EXISTS idx_cgrs_canonical ON "ob-poc".client_group_relationship_sources (relationship_id)
WHERE is_canonical = true;

-- Lineage traversal
CREATE INDEX IF NOT EXISTS idx_cgrs_verifies ON "ob-poc".client_group_relationship_sources (verifies_source_id)
WHERE verifies_source_id IS NOT NULL;

COMMENT ON TABLE "ob-poc".client_group_relationship_sources IS
'Multi-source lineage for ownership edges. Supports trust-but-verify workflow: client alleges → we verify against authoritative sources.';

COMMENT ON COLUMN "ob-poc".client_group_relationship_sources.verifies_source_id IS
'If this is a verification, which allegation does it verify?';

COMMENT ON COLUMN "ob-poc".client_group_relationship_sources.is_canonical IS
'Analyst override: mark this source as the canonical value for reporting.';

-- ============================================================================
-- 5. Extend client_group with discovery status columns
-- ============================================================================

ALTER TABLE "ob-poc".client_group
ADD COLUMN IF NOT EXISTS discovery_status VARCHAR(20) NOT NULL DEFAULT 'not_started',
ADD COLUMN IF NOT EXISTS discovery_started_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS discovery_completed_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS discovery_source VARCHAR(50),
ADD COLUMN IF NOT EXISTS discovery_root_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS entity_count INTEGER NOT NULL DEFAULT 0,
ADD COLUMN IF NOT EXISTS pending_review_count INTEGER NOT NULL DEFAULT 0;

-- Add check constraint for discovery_status
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'chk_cg_discovery_status'
    ) THEN
        ALTER TABLE "ob-poc".client_group
        ADD CONSTRAINT chk_cg_discovery_status
        CHECK (discovery_status IN ('not_started', 'in_progress', 'complete', 'stale', 'failed'));
    END IF;
END $$;

COMMENT ON COLUMN "ob-poc".client_group.discovery_status IS
'Research discovery status: not_started, in_progress, complete, stale, failed';

COMMENT ON COLUMN "ob-poc".client_group.discovery_root_lei IS
'Starting LEI for GLEIF crawl';

-- ============================================================================
-- 6. Seed missing roles for client group context
-- ============================================================================

INSERT INTO "ob-poc".roles (role_id, name, description, role_category, is_active)
SELECT gen_random_uuid(), v.name, v.description, v.role_category, true
FROM (VALUES
    ('ULTIMATE_PARENT', 'Top of corporate ownership chain', 'OWNERSHIP_CHAIN'),
    ('SPV', 'Special purpose vehicle', 'OWNERSHIP_CHAIN'),
    ('UCITS', 'EU regulated retail fund (UCITS directive)', 'FUND_STRUCTURE'),
    ('AIF', 'Alternative investment fund (AIFMD)', 'FUND_STRUCTURE'),
    ('UMBRELLA', 'Umbrella fund structure with sub-funds', 'FUND_STRUCTURE'),
    ('FUND', 'Generic fund entity', 'FUND_STRUCTURE')
) AS v(name, description, role_category)
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".roles r WHERE r.name = v.name
);

-- ============================================================================
-- 7. Trigger to maintain entity/review counts on client_group
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".update_client_group_counts()
RETURNS TRIGGER AS $$
DECLARE
    v_group_id UUID;
BEGIN
    -- Get the affected group_id
    v_group_id := COALESCE(NEW.group_id, OLD.group_id);

    -- Update counts
    UPDATE "ob-poc".client_group SET
        entity_count = (
            SELECT COUNT(*) FROM "ob-poc".client_group_entity
            WHERE group_id = v_group_id
            AND membership_type NOT IN ('historical')
        ),
        pending_review_count = (
            SELECT COUNT(*) FROM "ob-poc".client_group_entity
            WHERE group_id = v_group_id
            AND review_status IN ('pending', 'needs_update')
        ),
        updated_at = NOW()
    WHERE id = v_group_id;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_cge_counts ON "ob-poc".client_group_entity;
CREATE TRIGGER trg_cge_counts
AFTER INSERT OR UPDATE OR DELETE ON "ob-poc".client_group_entity
FOR EACH ROW EXECUTE FUNCTION "ob-poc".update_client_group_counts();

-- ============================================================================
-- 8. View: Agent scoped entity search (for intent pipeline)
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_client_group_entity_search AS
SELECT
    cge.id as cge_id,
    cge.group_id,
    cge.entity_id,
    cge.membership_type,
    cge.review_status,
    cge.added_by,

    -- Entity display fields
    e.name as entity_name,
    elc.lei,
    elc.jurisdiction,
    et.type_code as entity_type,

    -- Roles as array (for filtering/display)
    COALESCE(
        (SELECT array_agg(DISTINCT r.name ORDER BY r.name)
         FROM "ob-poc".client_group_entity_roles cer
         JOIN "ob-poc".roles r ON r.role_id = cer.role_id
         WHERE cer.cge_id = cge.id
           AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)),
        '{}'::VARCHAR[]
    ) as role_names,

    -- Role IDs for joins
    COALESCE(
        (SELECT array_agg(DISTINCT cer.role_id)
         FROM "ob-poc".client_group_entity_roles cer
         WHERE cer.cge_id = cge.id
           AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)),
        '{}'::UUID[]
    ) as role_ids,

    -- Group context
    cg.canonical_name as group_name,

    -- Is external?
    (cge.membership_type IN ('external_partner', 'counterparty', 'service_provider')) as is_external,

    -- Timestamps
    cge.created_at,
    cge.updated_at

FROM "ob-poc".client_group_entity cge
JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
LEFT JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
JOIN "ob-poc".client_group cg ON cg.id = cge.group_id
WHERE cge.membership_type != 'historical';

COMMENT ON VIEW "ob-poc".v_client_group_entity_search IS
'Denormalized view for intent pipeline scoped search. Hot path for "the IM", "the custodian" resolution.';

-- ============================================================================
-- 9. View: Canonical relationship ownership
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cgr_canonical AS
SELECT DISTINCT ON (r.id)
    r.id as relationship_id,
    r.group_id,
    r.parent_entity_id,
    r.child_entity_id,
    r.relationship_kind,
    r.review_status as relationship_review_status,
    r.effective_from,
    r.effective_to,
    pe.name as parent_name,
    ce.name as child_name,
    s.id as source_id,
    s.ownership_pct,
    s.voting_pct,
    s.control_pct,
    s.source as canonical_source,
    s.source_type,
    s.verification_status,
    s.is_canonical,
    s.confidence_score,
    s.source_document_ref,
    s.source_document_date,
    cg.canonical_name as group_name
FROM "ob-poc".client_group_relationship r
JOIN "ob-poc".client_group cg ON cg.id = r.group_id
JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
LEFT JOIN "ob-poc".client_group_relationship_sources s ON s.relationship_id = r.id
    AND s.verification_status != 'rejected'
ORDER BY r.id,
    -- Explicit canonical wins
    s.is_canonical DESC NULLS LAST,
    -- Then verified
    CASE s.verification_status WHEN 'verified' THEN 0 ELSE 1 END,
    -- Then by source authority (companies_house > clearstream > bods > gleif)
    CASE s.source
        WHEN 'companies_house' THEN 1
        WHEN 'clearstream' THEN 2
        WHEN 'bods' THEN 3
        WHEN 'gleif' THEN 4
        WHEN 'annual_report' THEN 5
        WHEN 'fund_prospectus' THEN 6
        WHEN 'kyc_document' THEN 7
        WHEN 'client_allegation' THEN 8
        WHEN 'manual' THEN 9
        ELSE 10
    END,
    -- Then by confidence
    s.confidence_score DESC NULLS LAST,
    -- Then by recency
    s.source_document_date DESC NULLS LAST;

COMMENT ON VIEW "ob-poc".v_cgr_canonical IS
'Returns best-available (canonical) ownership value for each relationship edge. Priority: explicit canonical > verified > source authority > confidence > recency.';

-- ============================================================================
-- 10. View: Unverified allegations
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cgr_unverified_allegations AS
SELECT
    r.group_id,
    cg.canonical_name as group_name,
    r.id as relationship_id,
    pe.name as parent_name,
    ce.name as child_name,
    r.relationship_kind,
    s.id as source_id,
    s.ownership_pct as alleged_pct,
    s.source_document_ref,
    s.source_document_date,
    s.created_at as alleged_at,
    -- How many verifications exist?
    (SELECT COUNT(*) FROM "ob-poc".client_group_relationship_sources v
     WHERE v.verifies_source_id = s.id) as verification_count
FROM "ob-poc".client_group_relationship_sources s
JOIN "ob-poc".client_group_relationship r ON r.id = s.relationship_id
JOIN "ob-poc".client_group cg ON cg.id = r.group_id
JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
WHERE s.source_type = 'allegation'
  AND s.verification_status = 'unverified';

COMMENT ON VIEW "ob-poc".v_cgr_unverified_allegations IS
'Lists all client allegations awaiting verification. Use for KYC review queue.';

-- ============================================================================
-- 11. View: Relationship discrepancies (multi-source conflicts)
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_cgr_discrepancies AS
SELECT
    r.group_id,
    cg.canonical_name as group_name,
    r.id as relationship_id,
    r.parent_entity_id,
    r.child_entity_id,
    r.relationship_kind,
    pe.name as parent_name,
    ce.name as child_name,
    array_agg(DISTINCT s.source ORDER BY s.source) as sources,
    array_agg(s.ownership_pct ORDER BY s.confidence_score DESC NULLS LAST) as ownership_values,
    MAX(s.ownership_pct) - MIN(s.ownership_pct) as ownership_spread,
    MAX(s.ownership_pct) FILTER (WHERE s.source_type = 'allegation') as alleged_pct,
    MAX(s.ownership_pct) FILTER (WHERE s.source_type = 'verification') as verified_pct,
    COUNT(DISTINCT s.source) as source_count,
    COUNT(DISTINCT s.ownership_pct) as distinct_value_count
FROM "ob-poc".client_group_relationship r
JOIN "ob-poc".client_group cg ON cg.id = r.group_id
JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
JOIN "ob-poc".client_group_relationship_sources s ON s.relationship_id = r.id
WHERE s.ownership_pct IS NOT NULL
  AND s.verification_status != 'rejected'
GROUP BY r.group_id, cg.canonical_name, r.id, r.parent_entity_id, r.child_entity_id,
         r.relationship_kind, pe.name, ce.name
HAVING COUNT(DISTINCT s.ownership_pct) > 1;

COMMENT ON VIEW "ob-poc".v_cgr_discrepancies IS
'Identifies relationships where sources disagree on ownership percentage. Shows spread and value counts for reconciliation.';

-- ============================================================================
-- 12. Helper function: Get default confidence score by source
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".get_source_confidence(p_source VARCHAR(50))
RETURNS NUMERIC(3,2) AS $$
BEGIN
    RETURN CASE p_source
        WHEN 'companies_house' THEN 0.95
        WHEN 'clearstream' THEN 0.90
        WHEN 'bods' THEN 0.85
        WHEN 'gleif' THEN 0.80
        WHEN 'annual_report' THEN 0.75
        WHEN 'fund_prospectus' THEN 0.70
        WHEN 'kyc_document' THEN 0.65
        WHEN 'client_allegation' THEN 0.50
        WHEN 'manual' THEN 0.40
        WHEN 'scraper' THEN 0.35
        ELSE 0.30
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION "ob-poc".get_source_confidence IS
'Returns default confidence score for ownership sources. Priority: regulatory filings > settlement systems > registries > client-provided.';

-- ============================================================================
-- 13. Ensure trigram extension and index for name search
-- ============================================================================

-- pg_trgm should already exist from migration 052, but ensure it
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Add trigram index on entities.name if not exists
CREATE INDEX IF NOT EXISTS idx_entities_name_trgm
ON "ob-poc".entities USING GIN (name gin_trgm_ops);

COMMIT;
