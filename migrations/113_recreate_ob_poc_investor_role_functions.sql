-- Migration 113: Recreate investor role helper functions in "ob-poc"
--
-- Purpose:
-- - Restore function surface after legacy schema cutover removed `kyc.*` routines.
-- - Keep runtime/domain ops and integration tests working against "ob-poc".

CREATE OR REPLACE FUNCTION "ob-poc".get_current_role_profile(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_share_class_id UUID DEFAULT NULL
) RETURNS "ob-poc".investor_role_profiles AS $$
    SELECT *
    FROM "ob-poc".investor_role_profiles
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_to IS NULL
    LIMIT 1;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION "ob-poc".get_current_role_profile IS
'Get the current (active) role profile for a holder-issuer relationship.';

CREATE OR REPLACE FUNCTION "ob-poc".get_role_profile_as_of(
    p_issuer_entity_id UUID,
    p_holder_entity_id UUID,
    p_as_of_date DATE,
    p_share_class_id UUID DEFAULT NULL
) RETURNS "ob-poc".investor_role_profiles AS $$
    SELECT *
    FROM "ob-poc".investor_role_profiles
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_from <= p_as_of_date
      AND (effective_to IS NULL OR effective_to > p_as_of_date)
    ORDER BY effective_from DESC
    LIMIT 1;
$$ LANGUAGE SQL STABLE;

COMMENT ON FUNCTION "ob-poc".get_role_profile_as_of IS
'Get the role profile active as of a specific date (point-in-time query).';

CREATE OR REPLACE FUNCTION "ob-poc".upsert_role_profile(
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
    UPDATE "ob-poc".investor_role_profiles
    SET effective_to = p_effective_from,
        updated_at = now()
    WHERE issuer_entity_id = p_issuer_entity_id
      AND holder_entity_id = p_holder_entity_id
      AND (share_class_id = p_share_class_id OR (share_class_id IS NULL AND p_share_class_id IS NULL))
      AND effective_to IS NULL;

    INSERT INTO "ob-poc".investor_role_profiles (
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

COMMENT ON FUNCTION "ob-poc".upsert_role_profile IS
'Create or update a role profile with temporal versioning by closing active version and inserting a new one.';
