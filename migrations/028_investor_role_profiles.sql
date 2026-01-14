-- Migration 028: Investor Role Profiles
--
-- Purpose: Add issuer-scoped holder role metadata to:
-- 1. Prevent pooled vehicles (FoF, master pools, nominees) from being misclassified as UBO
-- 2. Control look-through policy per holder-issuer relationship
-- 3. Support temporal versioning for point-in-time queries
--
-- Design: "Same entity, different treatment" - AllianzLife can be an end-investor in Fund A
-- but a master pool operator for Fund B, with different UBO eligibility and look-through rules.

-- =============================================================================
-- INVESTOR ROLE PROFILES TABLE
-- =============================================================================

CREATE TABLE IF NOT EXISTS kyc.investor_role_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Relationship scope
    issuer_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    share_class_id UUID NULL REFERENCES kyc.share_classes(id) ON DELETE SET NULL,

    -- Role classification
    role_type VARCHAR(50) NOT NULL,

    -- Look-through policy
    lookthrough_policy VARCHAR(30) NOT NULL DEFAULT 'NONE',

    -- Holder affiliation (intra-group vs external)
    holder_affiliation VARCHAR(20) NOT NULL DEFAULT 'UNKNOWN',

    -- BO data availability flag
    beneficial_owner_data_available BOOLEAN NOT NULL DEFAULT false,

    -- UBO eligibility (false = never create UBO edges for this holder)
    is_ubo_eligible BOOLEAN NOT NULL DEFAULT true,

    -- Optional group container (for intra-group holders)
    group_container_entity_id UUID NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE SET NULL,
    group_label TEXT NULL,

    -- Temporal versioning (effective_from/effective_to pattern)
    effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to DATE NULL,  -- NULL = current/active version

    -- Audit
    source VARCHAR(50) DEFAULT 'MANUAL',
    source_reference TEXT NULL,
    notes TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by VARCHAR(100) NULL,

    -- Role type enum
    CONSTRAINT chk_role_type CHECK (role_type IN (
        'END_INVESTOR',         -- Ultimate beneficial owner candidate
        'NOMINEE',              -- Holding on behalf of others
        'OMNIBUS',              -- Omnibus account (multiple underlying)
        'INTERMEDIARY_FOF',     -- Fund-of-funds intermediary
        'MASTER_POOL',          -- Master pooling vehicle
        'INTRA_GROUP_POOL',     -- Intra-group pooling (same corporate group)
        'TREASURY',             -- Group treasury function
        'CUSTODIAN',            -- Custodial holding
        'OTHER'
    )),

    -- Lookthrough policy enum
    CONSTRAINT chk_lookthrough CHECK (lookthrough_policy IN (
        'NONE',                 -- Do not look through (treat as leaf)
        'ON_DEMAND',            -- Look through only when explicitly requested
        'AUTO_IF_DATA',         -- Automatic look-through if BO data available
        'ALWAYS'                -- Always look through regardless of data
    )),

    -- Holder affiliation enum
    CONSTRAINT chk_holder_affiliation CHECK (holder_affiliation IN (
        'INTRA_GROUP',          -- Same corporate group as issuer
        'EXTERNAL',             -- External third-party investor
        'MIXED',                -- Hybrid (both intra-group and external)
        'UNKNOWN'               -- Not yet classified
    ))
);

-- Comments
COMMENT ON TABLE kyc.investor_role_profiles IS
'Issuer-scoped holder role metadata. Controls UBO eligibility and look-through policy per holder-issuer relationship.';

COMMENT ON COLUMN kyc.investor_role_profiles.role_type IS
'END_INVESTOR (UBO candidate), NOMINEE, OMNIBUS, INTERMEDIARY_FOF (fund-of-funds), MASTER_POOL, INTRA_GROUP_POOL, TREASURY, CUSTODIAN, OTHER';

COMMENT ON COLUMN kyc.investor_role_profiles.lookthrough_policy IS
'NONE (treat as leaf), ON_DEMAND (explicit request), AUTO_IF_DATA (if BO data available), ALWAYS (regardless of data)';

COMMENT ON COLUMN kyc.investor_role_profiles.holder_affiliation IS
'INTRA_GROUP (same corporate group), EXTERNAL (third-party), MIXED (hybrid), UNKNOWN';

COMMENT ON COLUMN kyc.investor_role_profiles.is_ubo_eligible IS
'If false, UBO sync trigger will never create ownership edges for this holder, regardless of percentage';

COMMENT ON COLUMN kyc.investor_role_profiles.effective_from IS
'Start date for this role profile version. Enables point-in-time queries for mid-year reclassifications.';

COMMENT ON COLUMN kyc.investor_role_profiles.effective_to IS
'End date for this role profile version. NULL means current/active version.';

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Lookup by issuer (most common query pattern)
CREATE INDEX IF NOT EXISTS idx_role_profiles_issuer
    ON kyc.investor_role_profiles(issuer_entity_id);

-- Lookup by holder
CREATE INDEX IF NOT EXISTS idx_role_profiles_holder
    ON kyc.investor_role_profiles(holder_entity_id);

-- Lookup by group container
CREATE INDEX IF NOT EXISTS idx_role_profiles_group
    ON kyc.investor_role_profiles(group_container_entity_id)
    WHERE group_container_entity_id IS NOT NULL;

-- Fast lookup for current/active profiles
CREATE INDEX IF NOT EXISTS idx_role_profiles_active
    ON kyc.investor_role_profiles(issuer_entity_id, holder_entity_id)
    WHERE effective_to IS NULL;

-- Point-in-time queries
CREATE INDEX IF NOT EXISTS idx_role_profiles_temporal
    ON kyc.investor_role_profiles(issuer_entity_id, holder_entity_id, effective_from, effective_to);

-- Unique constraint: only one active (effective_to IS NULL) profile per issuer+holder+share_class
-- Using partial unique index since PostgreSQL doesn't allow COALESCE in unique constraints
CREATE UNIQUE INDEX IF NOT EXISTS idx_role_profiles_unique_active
    ON kyc.investor_role_profiles(issuer_entity_id, holder_entity_id, COALESCE(share_class_id, '00000000-0000-0000-0000-000000000000'::uuid))
    WHERE effective_to IS NULL;

-- =============================================================================
-- HELPER FUNCTION: Get current role profile
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.get_current_role_profile(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_share_class_id UUID DEFAULT NULL
) RETURNS kyc.investor_role_profiles AS $$
    SELECT *
    FROM kyc.investor_role_profiles
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_to IS NULL
    LIMIT 1;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.get_current_role_profile IS
'Get the current (active) role profile for a holder-issuer relationship';

-- =============================================================================
-- HELPER FUNCTION: Get role profile as of date
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.get_role_profile_as_of(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_as_of_date DATE,
    p_share_class_id UUID DEFAULT NULL
) RETURNS kyc.investor_role_profiles AS $$
    SELECT *
    FROM kyc.investor_role_profiles
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_from <= p_as_of_date
      AND (effective_to IS NULL OR effective_to > p_as_of_date)
    ORDER BY effective_from DESC
    LIMIT 1;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION kyc.get_role_profile_as_of IS
'Get the role profile that was active as of a specific date (point-in-time query)';

-- =============================================================================
-- HELPER FUNCTION: Close current profile and create new version
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.upsert_role_profile(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_role_type VARCHAR(50),
    p_lookthrough_policy VARCHAR(30) DEFAULT 'NONE',
    p_holder_affiliation VARCHAR(20) DEFAULT 'UNKNOWN',
    p_beneficial_owner_data_available BOOLEAN DEFAULT false,
    p_is_ubo_eligible BOOLEAN DEFAULT true,
    p_share_class_id UUID DEFAULT NULL,
    p_group_container_entity_id UUID DEFAULT NULL,
    p_group_label TEXT DEFAULT NULL,
    p_effective_from DATE DEFAULT CURRENT_DATE,
    p_source VARCHAR(50) DEFAULT 'MANUAL',
    p_source_reference TEXT DEFAULT NULL,
    p_notes TEXT DEFAULT NULL,
    p_created_by VARCHAR(100) DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_new_id UUID;
BEGIN
    -- Close any existing active profile
    UPDATE kyc.investor_role_profiles
    SET effective_to = p_effective_from,
        updated_at = now()
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_to IS NULL;

    -- Insert new version
    INSERT INTO kyc.investor_role_profiles (
        issuer_entity_id,
        holder_entity_id,
        share_class_id,
        role_type,
        lookthrough_policy,
        holder_affiliation,
        beneficial_owner_data_available,
        is_ubo_eligible,
        group_container_entity_id,
        group_label,
        effective_from,
        source,
        source_reference,
        notes,
        created_by
    ) VALUES (
        p_issuer_entity_id,
        p_holder_entity_id,
        p_share_class_id,
        p_role_type,
        p_lookthrough_policy,
        p_holder_affiliation,
        p_beneficial_owner_data_available,
        p_is_ubo_eligible,
        p_group_container_entity_id,
        p_group_label,
        p_effective_from,
        p_source,
        p_source_reference,
        p_notes,
        p_created_by
    )
    RETURNING id INTO v_new_id;

    RETURN v_new_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION kyc.upsert_role_profile IS
'Create or update a role profile with temporal versioning. Closes existing active profile and creates new version.';

-- =============================================================================
-- VIEW: Current role profiles (convenience view)
-- =============================================================================

CREATE OR REPLACE VIEW kyc.v_current_role_profiles AS
SELECT
    rp.*,
    issuer.name AS issuer_name,
    holder.name AS holder_name,
    gc.name AS group_container_name
FROM kyc.investor_role_profiles rp
JOIN "ob-poc".entities issuer ON rp.issuer_entity_id = issuer.entity_id
JOIN "ob-poc".entities holder ON rp.holder_entity_id = holder.entity_id
LEFT JOIN "ob-poc".entities gc ON rp.group_container_entity_id = gc.entity_id
WHERE rp.effective_to IS NULL;

COMMENT ON VIEW kyc.v_current_role_profiles IS
'Current (active) role profiles with entity names resolved';

-- =============================================================================
-- UPDATE TRIGGER for updated_at
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.update_role_profile_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_role_profile_updated ON kyc.investor_role_profiles;
CREATE TRIGGER trg_role_profile_updated
    BEFORE UPDATE ON kyc.investor_role_profiles
    FOR EACH ROW
    EXECUTE FUNCTION kyc.update_role_profile_timestamp();
