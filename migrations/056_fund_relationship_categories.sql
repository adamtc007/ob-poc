-- ============================================================================
-- Migration 056: Fund Relationship Categories
--
-- Adds relationship_category to client_group_entity to distinguish ownership
-- hierarchy from investment management relationships. This enables loading
-- 1000+ managed funds during GLEIF import while keeping UBO analysis clean.
-- ============================================================================

-- ============================================================================
-- 1. Extend client_group_entity with relationship classification
-- ============================================================================

-- Add relationship category to distinguish ownership vs IM
ALTER TABLE "ob-poc".client_group_entity
ADD COLUMN IF NOT EXISTS relationship_category VARCHAR(30) DEFAULT 'OWNERSHIP';

-- Add flag for quick fund filtering
ALTER TABLE "ob-poc".client_group_entity
ADD COLUMN IF NOT EXISTS is_fund BOOLEAN DEFAULT FALSE;

-- Add linking LEI (the ManCo that brings this fund into the group)
ALTER TABLE "ob-poc".client_group_entity
ADD COLUMN IF NOT EXISTS related_via_lei VARCHAR(20);

-- Add comment for clarity
COMMENT ON COLUMN "ob-poc".client_group_entity.relationship_category IS
    'OWNERSHIP = consolidation hierarchy, INVESTMENT_MANAGEMENT = ManCo manages fund, FUND_STRUCTURE = umbrella/subfund/master/feeder';

COMMENT ON COLUMN "ob-poc".client_group_entity.is_fund IS
    'True if this entity is a fund (GLEIF category=FUND). Enables fast filtering.';

COMMENT ON COLUMN "ob-poc".client_group_entity.related_via_lei IS
    'For IM relationships, the LEI of the ManCo that manages this fund.';

-- ============================================================================
-- 2. Create indexes for efficient filtering
-- ============================================================================

-- Index for filtering by relationship type
CREATE INDEX IF NOT EXISTS idx_cge_rel_category
    ON "ob-poc".client_group_entity(group_id, relationship_category);

-- Partial index for fund queries (only index fund rows)
CREATE INDEX IF NOT EXISTS idx_cge_funds
    ON "ob-poc".client_group_entity(group_id)
    WHERE is_fund = TRUE;

-- Index for finding funds by their ManCo
CREATE INDEX IF NOT EXISTS idx_cge_related_via_lei
    ON "ob-poc".client_group_entity(related_via_lei)
    WHERE related_via_lei IS NOT NULL;

-- ============================================================================
-- 3. Create fund_metadata table for fund-specific attributes
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".fund_metadata (
    entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id),
    lei VARCHAR(20) NOT NULL,

    -- Fund classification (may be inferred from name, jurisdiction, or external sources)
    fund_structure_type VARCHAR(50),   -- SICAV, ICAV, OEIC, FCP, etc.
    fund_type VARCHAR(50),             -- UCITS, AIF, ETF, etc.

    -- Umbrella relationship
    umbrella_lei VARCHAR(20),
    is_umbrella BOOLEAN DEFAULT FALSE,
    subfund_count INTEGER DEFAULT 0,

    -- Master-feeder
    master_fund_lei VARCHAR(20),
    is_feeder BOOLEAN DEFAULT FALSE,
    is_master BOOLEAN DEFAULT FALSE,
    feeder_count INTEGER DEFAULT 0,

    -- ManCo relationship (denormalized for query speed)
    manco_lei VARCHAR(20),
    manco_name TEXT,
    manco_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Source tracking
    source VARCHAR(50) DEFAULT 'gleif',
    confidence_score DECIMAL(3,2),

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for fund_metadata
CREATE INDEX IF NOT EXISTS idx_fund_metadata_manco ON "ob-poc".fund_metadata(manco_lei);
CREATE INDEX IF NOT EXISTS idx_fund_metadata_umbrella ON "ob-poc".fund_metadata(umbrella_lei);
CREATE INDEX IF NOT EXISTS idx_fund_metadata_master ON "ob-poc".fund_metadata(master_fund_lei);
CREATE INDEX IF NOT EXISTS idx_fund_metadata_type ON "ob-poc".fund_metadata(fund_type);
CREATE INDEX IF NOT EXISTS idx_fund_metadata_manco_entity ON "ob-poc".fund_metadata(manco_entity_id);

COMMENT ON TABLE "ob-poc".fund_metadata IS
    'Fund-specific metadata discovered from GLEIF. Tracks fund type, structure, ManCo, and umbrella/feeder relationships.';

-- ============================================================================
-- 4. Create views for fund analysis
-- ============================================================================

-- View: Funds managed by each ManCo with type breakdown
CREATE OR REPLACE VIEW "ob-poc".v_manco_funds AS
SELECT
    m.manco_lei,
    m.manco_name,
    m.manco_entity_id,
    COUNT(*) as fund_count,
    COUNT(*) FILTER (WHERE m.fund_type = 'UCITS') as ucits_count,
    COUNT(*) FILTER (WHERE m.fund_type = 'AIF') as aif_count,
    COUNT(*) FILTER (WHERE m.fund_type = 'ETF') as etf_count,
    COUNT(*) FILTER (WHERE m.is_umbrella) as umbrella_count,
    COUNT(*) FILTER (WHERE m.is_feeder) as feeder_count,
    COUNT(*) FILTER (WHERE m.is_master) as master_count,
    array_agg(DISTINCT m.fund_type) FILTER (WHERE m.fund_type IS NOT NULL) as fund_types
FROM "ob-poc".fund_metadata m
WHERE m.manco_lei IS NOT NULL
GROUP BY m.manco_lei, m.manco_name, m.manco_entity_id;

-- View: Client group entities with relationship categories
CREATE OR REPLACE VIEW "ob-poc".v_client_group_entities_categorized AS
SELECT
    cge.id,
    cge.group_id,
    cge.entity_id,
    cge.membership_type,
    cge.added_by,
    cge.relationship_category,
    cge.is_fund,
    cge.related_via_lei,
    cge.source_record_id,
    e.name as entity_name,
    e.entity_category,
    COALESCE(elc.lei, ef.lei) as lei,
    COALESCE(elc.jurisdiction, ef.jurisdiction) as jurisdiction,
    CASE
        WHEN cge.relationship_category = 'OWNERSHIP' THEN 'Corporate Structure'
        WHEN cge.relationship_category = 'INVESTMENT_MANAGEMENT' THEN 'Managed Fund'
        WHEN cge.relationship_category = 'FUND_STRUCTURE' THEN 'Fund Structure'
        ELSE 'Other'
    END as display_category,
    cge.created_at,
    cge.updated_at
FROM "ob-poc".client_group_entity cge
JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id;

-- View: Summary of relationship categories per client group
CREATE OR REPLACE VIEW "ob-poc".v_client_group_category_summary AS
SELECT
    cge.group_id,
    cg.canonical_name as group_name,
    COUNT(*) as total_entities,
    COUNT(*) FILTER (WHERE cge.relationship_category = 'OWNERSHIP') as ownership_count,
    COUNT(*) FILTER (WHERE cge.relationship_category = 'INVESTMENT_MANAGEMENT') as im_count,
    COUNT(*) FILTER (WHERE cge.relationship_category = 'FUND_STRUCTURE') as fund_structure_count,
    COUNT(*) FILTER (WHERE cge.is_fund = TRUE) as fund_count,
    COUNT(DISTINCT cge.related_via_lei) FILTER (WHERE cge.related_via_lei IS NOT NULL) as manco_count
FROM "ob-poc".client_group_entity cge
JOIN "ob-poc".client_group cg ON cg.id = cge.group_id
WHERE cge.membership_type != 'historical'
GROUP BY cge.group_id, cg.canonical_name;

-- ============================================================================
-- 5. Helper functions
-- ============================================================================

-- Get all entities in ownership hierarchy (excluding IM relationships)
CREATE OR REPLACE FUNCTION "ob-poc".get_ownership_tree(p_group_id UUID)
RETURNS TABLE (
    entity_id UUID,
    lei VARCHAR,
    name TEXT,
    relationship_category VARCHAR
) AS $$
    SELECT
        cge.entity_id,
        COALESCE(elc.lei, ef.lei) as lei,
        e.name,
        cge.relationship_category
    FROM "ob-poc".client_group_entity cge
    JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
    LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
    LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
    WHERE cge.group_id = p_group_id
      AND cge.relationship_category = 'OWNERSHIP'
      AND cge.membership_type != 'historical';
$$ LANGUAGE SQL STABLE;

-- Get all funds managed by entities in a client group
CREATE OR REPLACE FUNCTION "ob-poc".get_managed_funds_for_group(p_group_id UUID)
RETURNS TABLE (
    fund_entity_id UUID,
    fund_lei VARCHAR,
    fund_name TEXT,
    manco_lei VARCHAR,
    manco_name TEXT,
    fund_type VARCHAR
) AS $$
    SELECT
        cge.entity_id as fund_entity_id,
        COALESCE(elc.lei, ef.lei) as fund_lei,
        e.name as fund_name,
        cge.related_via_lei as manco_lei,
        manco_e.name as manco_name,
        fm.fund_type
    FROM "ob-poc".client_group_entity cge
    JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
    LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
    LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
    LEFT JOIN "ob-poc".fund_metadata fm ON fm.entity_id = cge.entity_id
    LEFT JOIN "ob-poc".entities manco_e ON manco_e.entity_id = (
        SELECT entity_id FROM "ob-poc".entity_limited_companies WHERE lei = cge.related_via_lei
        LIMIT 1
    )
    WHERE cge.group_id = p_group_id
      AND cge.relationship_category = 'INVESTMENT_MANAGEMENT'
      AND cge.membership_type != 'historical';
$$ LANGUAGE SQL STABLE;

-- ============================================================================
-- 6. Backfill existing data
-- ============================================================================

-- Set existing entries to OWNERSHIP (they were imported before this migration)
UPDATE "ob-poc".client_group_entity
SET relationship_category = 'OWNERSHIP',
    is_fund = FALSE
WHERE relationship_category IS NULL;

-- Update is_fund flag based on entity category
UPDATE "ob-poc".client_group_entity cge
SET is_fund = TRUE
FROM "ob-poc".entities e
LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
WHERE cge.entity_id = e.entity_id
  AND (
    COALESCE(elc.gleif_category, ef.gleif_category) = 'FUND'
    OR e.entity_category = 'FUND'
  );

-- ============================================================================
-- 7. Update added_by allowed values comment
-- ============================================================================

COMMENT ON COLUMN "ob-poc".client_group_entity.added_by IS
    'Source of this entity: manual, discovery, gleif (ownership), gleif_im (managed funds), ownership_trace, user_confirmed';
