-- Migration 114: Restore UBO sync trigger on "ob-poc".holdings
--
-- Purpose:
-- - Recreate post-insert/update synchronization from UBO holdings to entity_relationships
--   after legacy schema cutover removed `kyc.*` trigger wiring.

CREATE OR REPLACE FUNCTION "ob-poc".sync_holding_to_ubo_relationship()
RETURNS TRIGGER AS $$
DECLARE
    v_total_units NUMERIC;
    v_ownership_pct NUMERIC;
    v_fund_entity_id UUID;
    v_is_ubo_eligible BOOLEAN;
    v_role_type VARCHAR(50);
    v_has_profile BOOLEAN;
    v_allow_edge BOOLEAN := true;
BEGIN
    -- Only UBO-mode holdings participate in UBO relationship sync.
    IF COALESCE(NEW.usage_type, 'TA') <> 'UBO' THEN
        RETURN NEW;
    END IF;

    SELECT COALESCE(SUM(units), 0)
    INTO v_total_units
    FROM "ob-poc".holdings
    WHERE share_class_id = NEW.share_class_id
      AND UPPER(COALESCE(holding_status, status)) = 'ACTIVE';

    IF v_total_units > 0 THEN
        v_ownership_pct := (NEW.units / v_total_units) * 100;
    ELSE
        v_ownership_pct := 0;
    END IF;

    SELECT COALESCE(issuer_entity_id, entity_id)
    INTO v_fund_entity_id
    FROM "ob-poc".share_classes
    WHERE id = NEW.share_class_id;

    SELECT
        true,
        rp.is_ubo_eligible,
        rp.role_type
    INTO
        v_has_profile,
        v_is_ubo_eligible,
        v_role_type
    FROM "ob-poc".investor_role_profiles rp
    WHERE rp.holder_entity_id = NEW.investor_entity_id
      AND rp.issuer_entity_id = v_fund_entity_id
      AND rp.effective_to IS NULL
    ORDER BY rp.effective_from DESC
    LIMIT 1;

    IF v_has_profile THEN
        IF v_is_ubo_eligible = false THEN
            v_allow_edge := false;
        ELSIF v_role_type IN (
            'NOMINEE',
            'OMNIBUS',
            'INTERMEDIARY_FOF',
            'MASTER_POOL',
            'INTRA_GROUP_POOL',
            'CUSTODIAN'
        ) THEN
            -- Default-deny pooled/intermediary holder classes.
            v_allow_edge := false;
        END IF;
    END IF;

    IF v_ownership_pct >= 25
        AND v_fund_entity_id IS NOT NULL
        AND v_allow_edge
    THEN
        UPDATE "ob-poc".entity_relationships
        SET percentage = v_ownership_pct,
            updated_at = NOW(),
            notes = 'Synced from UBO holding ' || NEW.id::text
        WHERE from_entity_id = NEW.investor_entity_id
          AND to_entity_id = v_fund_entity_id
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
          AND effective_to IS NULL;

        IF NOT FOUND THEN
            INSERT INTO "ob-poc".entity_relationships (
                from_entity_id,
                to_entity_id,
                relationship_type,
                percentage,
                ownership_type,
                interest_type,
                direct_or_indirect,
                effective_from,
                source,
                confidence,
                notes
            ) VALUES (
                NEW.investor_entity_id,
                v_fund_entity_id,
                'ownership',
                v_ownership_pct,
                'DIRECT',
                'shareholding',
                'direct',
                COALESCE(NEW.acquisition_date, CURRENT_DATE),
                'INVESTOR_REGISTER',
                'MEDIUM',
                'Synced from UBO holding ' || NEW.id::text
            );
        END IF;
    ELSE
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

COMMENT ON FUNCTION "ob-poc".sync_holding_to_ubo_relationship() IS
'Sync UBO holdings to ownership edges in entity_relationships with usage_type and role-profile gating.';

DROP TRIGGER IF EXISTS trg_sync_holding_to_ubo ON "ob-poc".holdings;

CREATE TRIGGER trg_sync_holding_to_ubo
AFTER INSERT OR UPDATE OF units, holding_status, status, usage_type ON "ob-poc".holdings
FOR EACH ROW
EXECUTE FUNCTION "ob-poc".sync_holding_to_ubo_relationship();
