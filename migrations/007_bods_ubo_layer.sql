-- ============================================================================
-- Migration 007: BODS UBO Layer
-- Beneficial Ownership Data Standard tables for UBO discovery
-- ============================================================================

-- ============================================================================
-- BODS Entity Statements (companies/trusts from beneficial ownership registers)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_entity_statements (
    statement_id VARCHAR(100) PRIMARY KEY,  -- BODS statement ID
    entity_type VARCHAR(50),                -- registeredEntity, legalEntity, arrangement
    name TEXT,
    jurisdiction VARCHAR(10),

    -- Identifiers (denormalized for query performance)
    lei VARCHAR(20),                        -- LEI if present
    company_number VARCHAR(100),
    opencorporates_id VARCHAR(200),

    -- Full identifiers array
    identifiers JSONB,

    -- Metadata
    source_register VARCHAR(100),           -- UK_PSC, DENMARK_CVR, etc.
    statement_date DATE,
    source_url TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_entity_type CHECK (
        entity_type IN ('registeredEntity', 'legalEntity', 'arrangement',
                        'anonymousEntity', 'unknownEntity', 'state', 'stateBody')
    )
);

CREATE INDEX IF NOT EXISTS idx_bods_entity_lei
ON "ob-poc".bods_entity_statements(lei) WHERE lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bods_entity_company_num
ON "ob-poc".bods_entity_statements(company_number) WHERE company_number IS NOT NULL;

-- ============================================================================
-- BODS Person Statements (natural persons - the actual UBOs)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_person_statements (
    statement_id VARCHAR(100) PRIMARY KEY,  -- BODS statement ID
    person_type VARCHAR(50),                -- knownPerson, anonymousPerson, unknownPerson

    -- Name (primary)
    full_name TEXT,
    given_name VARCHAR(200),
    family_name VARCHAR(200),

    -- All names (JSONB for aliases, maiden names, etc.)
    names JSONB,

    -- Demographics
    birth_date DATE,
    birth_date_precision VARCHAR(20),       -- exact, month, year
    death_date DATE,

    -- Location
    nationalities VARCHAR(10)[],            -- ISO country codes
    country_of_residence VARCHAR(10),
    addresses JSONB,

    -- Tax identifiers (if disclosed)
    tax_residencies VARCHAR(10)[],

    -- Metadata
    source_register VARCHAR(100),
    statement_date DATE,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_person_type CHECK (
        person_type IN ('knownPerson', 'anonymousPerson', 'unknownPerson')
    )
);

CREATE INDEX IF NOT EXISTS idx_bods_person_name
ON "ob-poc".bods_person_statements USING gin(to_tsvector('english', full_name));

-- ============================================================================
-- BODS Ownership/Control Statements (the relationships)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".bods_ownership_statements (
    statement_id VARCHAR(100) PRIMARY KEY,

    -- Subject (the entity being owned/controlled)
    subject_entity_statement_id VARCHAR(100),
    subject_lei VARCHAR(20),
    subject_name TEXT,

    -- Interested Party (the owner - can be person or entity)
    interested_party_type VARCHAR(20),      -- person, entity
    interested_party_statement_id VARCHAR(100),
    interested_party_name TEXT,

    -- Ownership details
    ownership_type VARCHAR(50),             -- shareholding, votingRights, appointmentOfBoard, etc.
    share_min DECIMAL,
    share_max DECIMAL,
    share_exact DECIMAL,
    is_direct BOOLEAN,

    -- Control details
    control_types VARCHAR(50)[],

    -- Validity
    start_date DATE,
    end_date DATE,

    -- Metadata
    source_register VARCHAR(100),
    statement_date DATE,
    source_description TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bods_ownership_subject
ON "ob-poc".bods_ownership_statements(subject_entity_statement_id);

CREATE INDEX IF NOT EXISTS idx_bods_ownership_subject_lei
ON "ob-poc".bods_ownership_statements(subject_lei) WHERE subject_lei IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bods_ownership_interested
ON "ob-poc".bods_ownership_statements(interested_party_statement_id);

-- ============================================================================
-- Link table: Connect our entities to BODS statements
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_bods_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    bods_entity_statement_id VARCHAR(100) REFERENCES "ob-poc".bods_entity_statements(statement_id),
    match_method VARCHAR(50),               -- LEI, COMPANY_NUMBER, NAME_MATCH
    match_confidence DECIMAL,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(entity_id, bods_entity_statement_id)
);

-- ============================================================================
-- UBO Summary View (denormalized for quick access)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".entity_ubos (
    ubo_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

    -- UBO details
    person_statement_id VARCHAR(100),
    person_name TEXT,
    nationalities VARCHAR(10)[],
    country_of_residence VARCHAR(10),

    -- Ownership chain
    ownership_chain JSONB,                  -- Array of intermediate entities
    chain_depth INTEGER,

    -- Ownership percentage (aggregated)
    ownership_min DECIMAL,
    ownership_max DECIMAL,
    ownership_exact DECIMAL,

    -- Control types
    control_types VARCHAR(50)[],
    is_direct BOOLEAN,

    -- Status
    ubo_type VARCHAR(30),                   -- NATURAL_PERSON, PUBLIC_FLOAT, UNKNOWN
    confidence_level VARCHAR(20),

    -- Source tracking
    source VARCHAR(50),                     -- BODS, GLEIF, MANUAL
    source_register VARCHAR(100),
    discovered_at TIMESTAMPTZ DEFAULT NOW(),
    verified_at TIMESTAMPTZ,
    verified_by VARCHAR(255),

    CONSTRAINT valid_ubo_type CHECK (
        ubo_type IN ('NATURAL_PERSON', 'PUBLIC_FLOAT', 'STATE_OWNED',
                     'WIDELY_HELD', 'UNKNOWN', 'EXEMPT')
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_ubos_entity
ON "ob-poc".entity_ubos(entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_ubos_person
ON "ob-poc".entity_ubos(person_statement_id) WHERE person_statement_id IS NOT NULL;

-- ============================================================================
-- Comments for documentation
-- ============================================================================

COMMENT ON TABLE "ob-poc".bods_entity_statements IS 'BODS entity statements from beneficial ownership registers (UK PSC, etc.)';
COMMENT ON TABLE "ob-poc".bods_person_statements IS 'BODS person statements - natural persons who are UBOs';
COMMENT ON TABLE "ob-poc".bods_ownership_statements IS 'BODS ownership/control statements linking persons to entities';
COMMENT ON TABLE "ob-poc".entity_bods_links IS 'Links our entities to BODS entity statements';
COMMENT ON TABLE "ob-poc".entity_ubos IS 'Denormalized UBO summary for quick access';

COMMENT ON COLUMN "ob-poc".bods_entity_statements.lei IS 'LEI identifier if present - join key to GLEIF data';
COMMENT ON COLUMN "ob-poc".bods_person_statements.birth_date_precision IS 'Precision of birth date: exact, month, or year';
COMMENT ON COLUMN "ob-poc".entity_ubos.ownership_chain IS 'JSON array of intermediate entities in ownership chain';
COMMENT ON COLUMN "ob-poc".entity_ubos.ubo_type IS 'Type: NATURAL_PERSON, PUBLIC_FLOAT, STATE_OWNED, WIDELY_HELD, UNKNOWN, EXEMPT';
