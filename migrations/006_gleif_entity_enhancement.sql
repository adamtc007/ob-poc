-- GLEIF Entity Enhancement Migration
-- Adds GLEIF-specific fields and relationship tracking for LEI → UBO pipeline
-- Phase 1: Corporate Layer

BEGIN;

-- ============================================================================
-- 1. Add LEI and GLEIF columns to entity_limited_companies
-- ============================================================================

ALTER TABLE "ob-poc".entity_limited_companies
ADD COLUMN IF NOT EXISTS lei VARCHAR(20) UNIQUE,
ADD COLUMN IF NOT EXISTS gleif_status VARCHAR(20),
ADD COLUMN IF NOT EXISTS gleif_category VARCHAR(50),
ADD COLUMN IF NOT EXISTS gleif_subcategory VARCHAR(50),
ADD COLUMN IF NOT EXISTS legal_form_code VARCHAR(10),
ADD COLUMN IF NOT EXISTS legal_form_text VARCHAR(200),
ADD COLUMN IF NOT EXISTS gleif_validation_level VARCHAR(30),
ADD COLUMN IF NOT EXISTS gleif_last_update TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS gleif_next_renewal DATE,
ADD COLUMN IF NOT EXISTS direct_parent_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS ultimate_parent_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS entity_creation_date DATE,
ADD COLUMN IF NOT EXISTS headquarters_address TEXT,
ADD COLUMN IF NOT EXISTS headquarters_city VARCHAR(200),
ADD COLUMN IF NOT EXISTS headquarters_country VARCHAR(3),
ADD COLUMN IF NOT EXISTS fund_manager_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS umbrella_fund_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS master_fund_lei VARCHAR(20),
ADD COLUMN IF NOT EXISTS is_fund BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS fund_type VARCHAR(30),
ADD COLUMN IF NOT EXISTS gleif_direct_parent_exception VARCHAR(50),
ADD COLUMN IF NOT EXISTS gleif_ultimate_parent_exception VARCHAR(50),
ADD COLUMN IF NOT EXISTS ubo_status VARCHAR(30) DEFAULT 'PENDING';

-- Create index on LEI
CREATE INDEX IF NOT EXISTS idx_limited_companies_lei
ON "ob-poc".entity_limited_companies(lei) WHERE lei IS NOT NULL;

-- Index for parent LEI lookups
CREATE INDEX IF NOT EXISTS idx_limited_companies_direct_parent
ON "ob-poc".entity_limited_companies(direct_parent_lei) WHERE direct_parent_lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_limited_companies_ultimate_parent
ON "ob-poc".entity_limited_companies(ultimate_parent_lei) WHERE ultimate_parent_lei IS NOT NULL;

COMMENT ON COLUMN "ob-poc".entity_limited_companies.gleif_direct_parent_exception IS
'GLEIF Level 2 reporting exception for direct parent: NO_KNOWN_PERSON, NATURAL_PERSONS, NON_CONSOLIDATING, etc.';

COMMENT ON COLUMN "ob-poc".entity_limited_companies.gleif_ultimate_parent_exception IS
'GLEIF Level 2 reporting exception for ultimate parent';

COMMENT ON COLUMN "ob-poc".entity_limited_companies.ubo_status IS
'UBO discovery status: PENDING, DISCOVERED, PUBLIC_FLOAT, EXEMPT, MANUAL_REQUIRED';

-- ============================================================================
-- 2. Create entity_names table (alternative names, trading names, etc.)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_names (
    name_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    name_type VARCHAR(50) NOT NULL,  -- LEGAL, TRADING, TRANSLITERATED, HISTORICAL
    name TEXT NOT NULL,
    language VARCHAR(10),
    is_primary BOOLEAN DEFAULT FALSE,
    effective_from DATE,
    effective_to DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_name_type CHECK (
        name_type IN ('LEGAL', 'TRADING', 'TRANSLITERATED', 'HISTORICAL', 'ALTERNATIVE', 'SHORT')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_names_entity
ON "ob-poc".entity_names(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_names_search
ON "ob-poc".entity_names USING gin(to_tsvector('english', name));

COMMENT ON TABLE "ob-poc".entity_names IS
'Alternative names for entities from GLEIF otherNames and transliteratedOtherNames fields';

-- ============================================================================
-- 3. Create entity_addresses table (structured address data)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_addresses (
    address_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    address_type VARCHAR(50) NOT NULL,  -- LEGAL, HEADQUARTERS, BRANCH, ALTERNATIVE
    language VARCHAR(10),
    address_lines TEXT[],
    city VARCHAR(200),
    region VARCHAR(50),           -- ISO 3166-2
    country VARCHAR(3) NOT NULL,  -- ISO 3166-1 alpha-2
    postal_code VARCHAR(50),
    is_primary BOOLEAN DEFAULT FALSE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_address_type CHECK (
        address_type IN ('LEGAL', 'HEADQUARTERS', 'BRANCH', 'ALTERNATIVE', 'TRANSLITERATED')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_addresses_entity
ON "ob-poc".entity_addresses(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_addresses_country
ON "ob-poc".entity_addresses(country);

COMMENT ON TABLE "ob-poc".entity_addresses IS
'Structured address data from GLEIF legalAddress, headquartersAddress, otherAddresses';

-- ============================================================================
-- 4. Create entity_identifiers table (LEI, BIC, ISIN, etc.)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_identifiers (
    identifier_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    identifier_type VARCHAR(30) NOT NULL,  -- LEI, BIC, ISIN, CIK, REG_NUM, MIC
    identifier_value VARCHAR(100) NOT NULL,
    issuing_authority VARCHAR(100),
    is_primary BOOLEAN DEFAULT FALSE,
    valid_from DATE,
    valid_until DATE,
    source VARCHAR(50),
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_identifier_type CHECK (
        identifier_type IN ('LEI', 'BIC', 'ISIN', 'CIK', 'MIC', 'REG_NUM', 'FIGI', 'CUSIP', 'SEDOL')
    ),
    UNIQUE(entity_id, identifier_type, identifier_value)
);

CREATE INDEX IF NOT EXISTS idx_entity_identifiers_lookup
ON "ob-poc".entity_identifiers(identifier_type, identifier_value);

COMMENT ON TABLE "ob-poc".entity_identifiers IS
'Cross-reference identifiers from GLEIF (LEI, BIC mappings, etc.) and other sources';

-- ============================================================================
-- 5. Create entity_parent_relationships table (ownership chains)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_parent_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    parent_entity_id UUID REFERENCES "ob-poc".entities(entity_id),  -- NULL if parent not in our system
    parent_lei VARCHAR(20),  -- Store even if parent not in our system
    parent_name TEXT,        -- Denormalized for display
    relationship_type VARCHAR(50) NOT NULL,  -- DIRECT_PARENT, ULTIMATE_PARENT
    accounting_standard VARCHAR(20),  -- IFRS, US_GAAP, etc.
    relationship_start DATE,
    relationship_end DATE,
    relationship_status VARCHAR(30) DEFAULT 'ACTIVE',
    validation_source VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_relationship_type CHECK (
        relationship_type IN ('DIRECT_PARENT', 'ULTIMATE_PARENT', 'FUND_MANAGER',
                              'UMBRELLA_FUND', 'MASTER_FUND', 'BRANCH_OF')
    ),
    UNIQUE(child_entity_id, parent_lei, relationship_type)
);

CREATE INDEX IF NOT EXISTS idx_entity_parents_child
ON "ob-poc".entity_parent_relationships(child_entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_parents_parent
ON "ob-poc".entity_parent_relationships(parent_entity_id) WHERE parent_entity_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_entity_parents_parent_lei
ON "ob-poc".entity_parent_relationships(parent_lei) WHERE parent_lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_entity_parents_type
ON "ob-poc".entity_parent_relationships(relationship_type);

COMMENT ON TABLE "ob-poc".entity_parent_relationships IS
'Corporate ownership relationships from GLEIF Level 2 data - direct and ultimate parents';

-- ============================================================================
-- 6. Create entity_lifecycle_events table (corporate actions)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_lifecycle_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL,  -- CHANGE_LEGAL_NAME, MERGER, DISSOLUTION, etc.
    event_status VARCHAR(30),         -- PENDING, COMPLETED
    effective_date DATE,
    recorded_date DATE,
    affected_fields JSONB,            -- What changed
    old_values JSONB,
    new_values JSONB,
    successor_lei VARCHAR(20),
    successor_name TEXT,
    validation_documents VARCHAR(50),
    validation_reference TEXT,
    source VARCHAR(50) DEFAULT 'GLEIF',
    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_event_type CHECK (
        event_type IN ('CHANGE_LEGAL_NAME', 'CHANGE_LEGAL_ADDRESS', 'CHANGE_HQ_ADDRESS',
                       'CHANGE_LEGAL_FORM', 'MERGER', 'SPIN_OFF', 'ACQUISITION',
                       'DISSOLUTION', 'BANKRUPTCY', 'DEREGISTRATION', 'RELOCATION')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_events_entity
ON "ob-poc".entity_lifecycle_events(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_events_type
ON "ob-poc".entity_lifecycle_events(event_type);

CREATE INDEX IF NOT EXISTS idx_entity_events_date
ON "ob-poc".entity_lifecycle_events(effective_date DESC);

COMMENT ON TABLE "ob-poc".entity_lifecycle_events IS
'Corporate lifecycle events from GLEIF eventGroups - name changes, mergers, etc.';

-- ============================================================================
-- 7. Create gleif_sync_log table (track GLEIF data freshness)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".gleif_sync_log (
    sync_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    lei VARCHAR(20),
    sync_type VARCHAR(30) NOT NULL,  -- FULL, INCREMENTAL, RELATIONSHIP
    sync_status VARCHAR(30) NOT NULL,  -- SUCCESS, FAILED, PARTIAL
    records_fetched INTEGER DEFAULT 0,
    records_updated INTEGER DEFAULT 0,
    records_created INTEGER DEFAULT 0,
    error_message TEXT,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    CONSTRAINT valid_sync_status CHECK (
        sync_status IN ('SUCCESS', 'FAILED', 'PARTIAL', 'IN_PROGRESS')
    )
);

CREATE INDEX IF NOT EXISTS idx_gleif_sync_entity
ON "ob-poc".gleif_sync_log(entity_id) WHERE entity_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_gleif_sync_lei
ON "ob-poc".gleif_sync_log(lei) WHERE lei IS NOT NULL;

COMMENT ON TABLE "ob-poc".gleif_sync_log IS
'Audit log for GLEIF data synchronization operations';

COMMIT;
