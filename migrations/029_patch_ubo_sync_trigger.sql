-- Migration 029: Patch UBO sync trigger to respect usage_type and role profiles
--
-- Fixes: The original trigger in migration 011 creates UBO edges for ALL holdings ≥25%,
-- but pooled vehicles (FoF, master pools, nominees) should NOT create UBO edges.
--
-- This patch adds:
-- 1. Check usage_type = 'UBO' (skip TA holdings)
-- 2. Check investor_role_profiles.is_ubo_eligible (skip ineligible holders)
-- 3. Default-deny for known pooled vehicle role types

-- =============================================================================
-- PATCHED UBO SYNC TRIGGER
-- =============================================================================

CREATE OR REPLACE FUNCTION kyc.sync_holding_to_ubo_relationship()
RETURNS TRIGGER AS $$
DECLARE
    v_total_units NUMERIC;
    v_ownership_pct NUMERIC;
    v_fund_entity_id UUID;
    v_is_ubo_eligible BOOLEAN;
    v_role_type VARCHAR(50);
BEGIN
    -- NEW CHECK 1: Only sync UBO holdings, skip TA holdings
    -- TA (Transfer Agency) holdings are for client KYC, not UBO tracking
    IF COALESCE(NEW.usage_type, 'TA') != 'UBO' THEN
        RETURN NEW;
    END IF;

    -- Get total units for percentage calculation
    SELECT COALESCE(SUM(units), 0) INTO v_total_units
    FROM kyc.holdings
    WHERE share_class_id = NEW.share_class_id
      AND COALESCE(holding_status, status) = 'active';

    -- Calculate ownership percentage
    IF v_total_units > 0 THEN
        v_ownership_pct := (NEW.units / v_total_units) * 100;
    ELSE
        v_ownership_pct := 0;
    END IF;

    -- Get fund entity ID from share class
    SELECT entity_id INTO v_fund_entity_id
    FROM kyc.share_classes WHERE id = NEW.share_class_id;

    -- NEW CHECK 2: Check investor role profile for UBO eligibility
    SELECT is_ubo_eligible, role_type
    INTO v_is_ubo_eligible, v_role_type
    FROM kyc.investor_role_profiles
    WHERE holder_entity_id = NEW.investor_entity_id
      AND issuer_entity_id = v_fund_entity_id
      AND effective_to IS NULL  -- Current version only
    LIMIT 1;

    -- If role profile exists and is_ubo_eligible = false, skip
    IF v_is_ubo_eligible = false THEN
        RETURN NEW;
    END IF;

    -- NEW CHECK 3: Default-deny for pooled vehicle role types even without explicit profile
    -- These role types typically should not create UBO edges
    IF v_role_type IN ('NOMINEE', 'OMNIBUS', 'INTERMEDIARY_FOF', 'MASTER_POOL', 'INTRA_GROUP_POOL', 'CUSTODIAN') THEN
        -- Only create UBO edge if explicitly marked as eligible (handled above)
        -- Since we got here, is_ubo_eligible is either NULL or TRUE
        -- For pooled vehicles, require explicit TRUE, not just NULL
        IF v_is_ubo_eligible IS NULL THEN
            RETURN NEW;  -- Skip if no explicit eligibility set for pooled vehicles
        END IF;
    END IF;

    -- Create/update ownership relationship if ≥25% and fund entity exists
    IF v_ownership_pct >= 25 AND v_fund_entity_id IS NOT NULL THEN
        INSERT INTO "ob-poc".entity_relationships (
            from_entity_id, to_entity_id, relationship_type,
            percentage, ownership_type, interest_type, direct_or_indirect,
            effective_from, source, notes
        ) VALUES (
            NEW.investor_entity_id, v_fund_entity_id, 'ownership',
            v_ownership_pct, 'DIRECT', 'shareholding', 'direct',
            COALESCE(NEW.acquisition_date, CURRENT_DATE),
            'INVESTOR_REGISTER',
            'Synced from UBO holding ' || NEW.id::text
        )
        ON CONFLICT (from_entity_id, to_entity_id, relationship_type)
        WHERE effective_to IS NULL
        DO UPDATE SET
            percentage = EXCLUDED.percentage,
            updated_at = NOW(),
            notes = EXCLUDED.notes;
    ELSE
        -- Remove relationship if dropped below 25%
        UPDATE "ob-poc".entity_relationships
        SET effective_to = CURRENT_DATE,
            updated_at = NOW()
        WHERE from_entity_id = NEW.investor_entity_id
          AND to_entity_id = v_fund_entity_id
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
          AND effective_to IS NULL;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Comment explaining the changes
COMMENT ON FUNCTION kyc.sync_holding_to_ubo_relationship() IS
'Sync holdings to UBO ownership edges. PATCHED in migration 029 to:
1. Only sync usage_type=UBO (skip TA holdings)
2. Respect investor_role_profiles.is_ubo_eligible
3. Default-deny for pooled vehicle role types (NOMINEE, FOF, MASTER_POOL, etc.)';

-- =============================================================================
-- NOTES FOR FUTURE REFERENCE
-- =============================================================================
-- The trigger is already attached to kyc.holdings from migration 011:
--   CREATE TRIGGER trg_sync_holding_to_ubo
--   AFTER INSERT OR UPDATE OF units, holding_status, status ON kyc.holdings
--   FOR EACH ROW EXECUTE FUNCTION kyc.sync_holding_to_ubo_relationship();
--
-- This migration only updates the function body, not the trigger itself.
-- No need to drop/recreate the trigger.
