-- =============================================================================
-- TRADING PROFILE AST MIGRATION
-- =============================================================================
-- Date: January 6, 2026
-- Description: Migrates existing trading profile documents from the flat
--              structure to the new tree-based AST format (TradingMatrixDocument).
--
-- Old format: Flat arrays (universe, standing_instructions, booking_rules, etc.)
-- New format: Tree with category nodes containing typed child nodes
--
-- This is a DATA MIGRATION - it transforms existing JSONB documents in place.
-- =============================================================================

BEGIN;

-- =============================================================================
-- STEP 1: Create migration function
-- =============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".migrate_trading_profile_to_ast(
    p_profile_id UUID,
    p_old_document JSONB,
    p_cbu_id UUID,
    p_cbu_name TEXT,
    p_version INT
) RETURNS JSONB AS $$
DECLARE
    v_new_doc JSONB;
    v_children JSONB := '[]'::jsonb;
    v_universe_category JSONB;
    v_ssi_category JSONB;
    v_isda_category JSONB;
    v_universe_children JSONB := '[]'::jsonb;
    v_ssi_children JSONB := '[]'::jsonb;
    v_isda_children JSONB := '[]'::jsonb;
    v_instrument_class RECORD;
    v_market RECORD;
    v_ssi RECORD;
    v_booking_rule RECORD;
    v_isda RECORD;
    v_node JSONB;
    v_market_node JSONB;
    v_ssi_node JSONB;
    v_rule_node JSONB;
    v_class_code TEXT;
    v_class_children JSONB;
    v_market_children JSONB;
    v_mic TEXT;
BEGIN
    -- ==========================================================================
    -- Build Trading Universe Category
    -- ==========================================================================

    -- Process instrument classes
    IF p_old_document->'universe'->'instrument_classes' IS NOT NULL THEN
        FOR v_instrument_class IN
            SELECT * FROM jsonb_array_elements(p_old_document->'universe'->'instrument_classes')
        LOOP
            v_class_code := v_instrument_class.value->>'class_code';
            v_class_children := '[]'::jsonb;

            -- Find markets for this instrument class
            IF p_old_document->'universe'->'allowed_markets' IS NOT NULL THEN
                FOR v_market IN
                    SELECT * FROM jsonb_array_elements(p_old_document->'universe'->'allowed_markets')
                LOOP
                    v_mic := v_market.value->>'mic';
                    v_market_children := '[]'::jsonb;

                    -- Create universe entry node for this market
                    v_node := jsonb_build_object(
                        'id', jsonb_build_array('_UNIVERSE', v_class_code, v_mic, gen_random_uuid()::text),
                        'node_type', jsonb_build_object(
                            'type', 'universe_entry',
                            'universe_id', gen_random_uuid()::text,
                            'currencies', COALESCE(v_market.value->'currencies', '["USD"]'::jsonb),
                            'settlement_types', COALESCE(v_market.value->'settlement_types', '["DVP"]'::jsonb),
                            'is_held', COALESCE((v_instrument_class.value->>'is_held')::boolean, true),
                            'is_traded', COALESCE((v_instrument_class.value->>'is_traded')::boolean, true)
                        ),
                        'label', 'Universe Entry',
                        'sublabel', array_to_string(ARRAY(SELECT jsonb_array_elements_text(COALESCE(v_market.value->'currencies', '["USD"]'::jsonb))), ', '),
                        'children', '[]'::jsonb,
                        'status_color', 'green',
                        'is_loaded', true,
                        'leaf_count', 1
                    );
                    v_market_children := v_market_children || v_node;

                    -- Create market node
                    v_market_node := jsonb_build_object(
                        'id', jsonb_build_array('_UNIVERSE', v_class_code, v_mic),
                        'node_type', jsonb_build_object(
                            'type', 'market',
                            'mic', v_mic,
                            'market_name', v_mic,
                            'country_code', CASE
                                WHEN v_mic LIKE 'X%' THEN
                                    CASE substring(v_mic from 2 for 2)
                                        WHEN 'NY' THEN 'US'
                                        WHEN 'NA' THEN 'US'
                                        WHEN 'LO' THEN 'GB'
                                        WHEN 'ET' THEN 'DE'
                                        WHEN 'PA' THEN 'FR'
                                        WHEN 'SW' THEN 'CH'
                                        WHEN 'HK' THEN 'HK'
                                        WHEN 'TK' THEN 'JP'
                                        ELSE 'XX'
                                    END
                                ELSE 'XX'
                            END
                        ),
                        'label', v_mic,
                        'children', v_market_children,
                        'status_color', 'green',
                        'is_loaded', true,
                        'leaf_count', jsonb_array_length(v_market_children)
                    );
                    v_class_children := v_class_children || v_market_node;
                END LOOP;
            END IF;

            -- Create instrument class node
            v_node := jsonb_build_object(
                'id', jsonb_build_array('_UNIVERSE', v_class_code),
                'node_type', jsonb_build_object(
                    'type', 'instrument_class',
                    'class_code', v_class_code,
                    'cfi_prefix', NULL,
                    'is_otc', v_class_code LIKE 'OTC%'
                ),
                'label', v_class_code,
                'children', v_class_children,
                'status_color', 'green',
                'is_loaded', true,
                'leaf_count', GREATEST(jsonb_array_length(v_class_children), 1)
            );
            v_universe_children := v_universe_children || v_node;
        END LOOP;
    END IF;

    -- Create Trading Universe category
    v_universe_category := jsonb_build_object(
        'id', jsonb_build_array('_UNIVERSE'),
        'node_type', jsonb_build_object('type', 'category', 'name', 'Trading Universe'),
        'label', 'Trading Universe',
        'children', v_universe_children,
        'status_color', CASE WHEN jsonb_array_length(v_universe_children) > 0 THEN 'green' ELSE 'gray' END,
        'is_loaded', true,
        'leaf_count', COALESCE((SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_universe_children) c), 0)
    );
    v_children := v_children || v_universe_category;

    -- ==========================================================================
    -- Build Settlement Instructions Category
    -- ==========================================================================

    -- Process SSIs from standing_instructions.SECURITIES
    IF p_old_document->'standing_instructions'->'SECURITIES' IS NOT NULL THEN
        FOR v_ssi IN
            SELECT * FROM jsonb_array_elements(p_old_document->'standing_instructions'->'SECURITIES')
        LOOP
            v_market_children := '[]'::jsonb;

            -- Find booking rules that reference this SSI
            IF p_old_document->'booking_rules' IS NOT NULL THEN
                FOR v_booking_rule IN
                    SELECT * FROM jsonb_array_elements(p_old_document->'booking_rules')
                    WHERE (value->>'ssi_ref') = (v_ssi.value->>'name')
                LOOP
                    v_rule_node := jsonb_build_object(
                        'id', jsonb_build_array('_SSI', v_ssi.value->>'name', 'rule_' || (v_booking_rule.value->>'name')),
                        'node_type', jsonb_build_object(
                            'type', 'booking_rule',
                            'rule_id', gen_random_uuid()::text,
                            'rule_name', v_booking_rule.value->>'name',
                            'priority', COALESCE((v_booking_rule.value->>'priority')::int, 50),
                            'specificity_score', CASE
                                WHEN v_booking_rule.value->'match'->>'instrument_class' IS NOT NULL THEN 1 ELSE 0
                            END + CASE
                                WHEN v_booking_rule.value->'match'->>'mic' IS NOT NULL THEN 1 ELSE 0
                            END + CASE
                                WHEN v_booking_rule.value->'match'->>'currency' IS NOT NULL THEN 1 ELSE 0
                            END + CASE
                                WHEN v_booking_rule.value->'match'->>'settlement_type' IS NOT NULL THEN 1 ELSE 0
                            END,
                            'is_active', true,
                            'match_criteria', jsonb_build_object(
                                'instrument_class', v_booking_rule.value->'match'->>'instrument_class',
                                'mic', v_booking_rule.value->'match'->>'mic',
                                'currency', v_booking_rule.value->'match'->>'currency',
                                'settlement_type', v_booking_rule.value->'match'->>'settlement_type',
                                'security_type', v_booking_rule.value->'match'->>'security_type',
                                'counterparty_entity_id', v_booking_rule.value->'match'->>'counterparty'
                            )
                        ),
                        'label', v_booking_rule.value->>'name',
                        'sublabel', 'Priority ' || COALESCE(v_booking_rule.value->>'priority', '50'),
                        'children', '[]'::jsonb,
                        'status_color', 'green',
                        'is_loaded', true,
                        'leaf_count', 1
                    );
                    v_market_children := v_market_children || v_rule_node;
                END LOOP;
            END IF;

            -- Create SSI node
            v_ssi_node := jsonb_build_object(
                'id', jsonb_build_array('_SSI', v_ssi.value->>'name'),
                'node_type', jsonb_build_object(
                    'type', 'ssi',
                    'ssi_id', gen_random_uuid()::text,
                    'ssi_name', v_ssi.value->>'name',
                    'ssi_type', 'SECURITIES',
                    'status', 'ACTIVE',
                    'safekeeping_account', v_ssi.value->>'custody_account',
                    'safekeeping_bic', v_ssi.value->>'custody_bic',
                    'cash_account', v_ssi.value->>'cash_account',
                    'cash_bic', v_ssi.value->>'cash_bic',
                    'pset_bic', NULL,
                    'cash_currency', v_ssi.value->>'currency'
                ),
                'label', v_ssi.value->>'name',
                'sublabel', COALESCE(v_ssi.value->>'custody_bic', '') || ' / ' || COALESCE(v_ssi.value->>'currency', ''),
                'children', v_market_children,
                'status_color', 'green',
                'is_loaded', true,
                'leaf_count', GREATEST(jsonb_array_length(v_market_children), 1)
            );
            v_ssi_children := v_ssi_children || v_ssi_node;
        END LOOP;
    END IF;

    -- Create Settlement Instructions category
    v_ssi_category := jsonb_build_object(
        'id', jsonb_build_array('_SSI'),
        'node_type', jsonb_build_object('type', 'category', 'name', 'Settlement Instructions'),
        'label', 'Settlement Instructions',
        'children', v_ssi_children,
        'status_color', CASE WHEN jsonb_array_length(v_ssi_children) > 0 THEN 'green' ELSE 'gray' END,
        'is_loaded', true,
        'leaf_count', COALESCE((SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_ssi_children) c), 0)
    );
    v_children := v_children || v_ssi_category;

    -- ==========================================================================
    -- Build ISDA Agreements Category
    -- ==========================================================================

    IF p_old_document->'isda_agreements' IS NOT NULL AND
       jsonb_array_length(p_old_document->'isda_agreements') > 0 THEN
        FOR v_isda IN
            SELECT * FROM jsonb_array_elements(p_old_document->'isda_agreements')
        LOOP
            v_node := jsonb_build_object(
                'id', jsonb_build_array('_ISDA', COALESCE(v_isda.value->>'counterparty_name', 'Unknown')),
                'node_type', jsonb_build_object(
                    'type', 'isda_agreement',
                    'isda_id', gen_random_uuid()::text,
                    'counterparty_name', COALESCE(v_isda.value->>'counterparty_name', v_isda.value->'counterparty'->>'name'),
                    'governing_law', v_isda.value->>'governing_law',
                    'agreement_date', v_isda.value->>'agreement_date',
                    'counterparty_entity_id', v_isda.value->'counterparty'->>'entity_id',
                    'counterparty_lei', v_isda.value->'counterparty'->>'lei'
                ),
                'label', COALESCE(v_isda.value->>'counterparty_name', v_isda.value->'counterparty'->>'name', 'Unknown'),
                'sublabel', COALESCE(v_isda.value->>'governing_law', 'NY') || ' Law',
                'children', '[]'::jsonb,  -- CSAs would be children here
                'status_color', 'green',
                'is_loaded', true,
                'leaf_count', 1
            );
            v_isda_children := v_isda_children || v_node;
        END LOOP;
    END IF;

    -- Create ISDA Agreements category
    v_isda_category := jsonb_build_object(
        'id', jsonb_build_array('_ISDA'),
        'node_type', jsonb_build_object('type', 'category', 'name', 'ISDA Agreements'),
        'label', 'ISDA Agreements',
        'children', v_isda_children,
        'status_color', CASE WHEN jsonb_array_length(v_isda_children) > 0 THEN 'green' ELSE 'gray' END,
        'is_loaded', true,
        'leaf_count', COALESCE((SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_isda_children) c), 0)
    );
    v_children := v_children || v_isda_category;

    -- ==========================================================================
    -- Build final document
    -- ==========================================================================

    v_new_doc := jsonb_build_object(
        'cbu_id', p_cbu_id::text,
        'cbu_name', p_cbu_name,
        'version', p_version,
        'status', 'DRAFT',
        'children', v_children,
        'total_leaf_count', COALESCE(
            (SELECT SUM((c->>'leaf_count')::int) FROM jsonb_array_elements(v_children) c),
            0
        ),
        'metadata', jsonb_build_object(
            'source', 'migration',
            'source_ref', 'migration_20260106',
            'notes', 'Migrated from flat structure to AST format',
            'regulatory_framework', p_old_document->'metadata'->>'regulatory_framework'
        ),
        'created_at', to_char(now(), 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
        'updated_at', to_char(now(), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
    );

    RETURN v_new_doc;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- STEP 2: Check if migration is needed (old format detection)
-- =============================================================================

-- Old format has 'universe' as an object with 'instrument_classes' array
-- New format has 'children' array with category nodes

CREATE OR REPLACE FUNCTION "ob-poc".needs_ast_migration(p_document JSONB) RETURNS BOOLEAN AS $$
BEGIN
    -- If document has 'universe' as an object (not in children), it's old format
    IF p_document ? 'universe' AND
       jsonb_typeof(p_document->'universe') = 'object' AND
       NOT (p_document ? 'children') THEN
        RETURN TRUE;
    END IF;

    -- If document has 'standing_instructions' at top level, it's old format
    IF p_document ? 'standing_instructions' AND
       NOT (p_document ? 'children') THEN
        RETURN TRUE;
    END IF;

    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- STEP 3: Create backup table
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".trading_profile_migration_backup (
    backup_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL,
    original_document JSONB NOT NULL,
    migrated_at TIMESTAMPTZ DEFAULT NOW()
);

-- =============================================================================
-- STEP 4: Perform migration with backup
-- =============================================================================

DO $$
DECLARE
    v_profile RECORD;
    v_new_doc JSONB;
    v_count INT := 0;
    v_cbu_name TEXT;
BEGIN
    RAISE NOTICE 'Starting Trading Profile AST migration...';

    FOR v_profile IN
        SELECT
            tp.profile_id,
            tp.cbu_id,
            tp.version,
            tp.document,
            c.name as cbu_name
        FROM "ob-poc".cbu_trading_profiles tp
        JOIN "ob-poc".cbus c ON c.cbu_id = tp.cbu_id
        WHERE "ob-poc".needs_ast_migration(tp.document)
    LOOP
        v_cbu_name := COALESCE(v_profile.cbu_name, 'Unknown CBU');

        -- Backup original document
        INSERT INTO "ob-poc".trading_profile_migration_backup (profile_id, original_document)
        VALUES (v_profile.profile_id, v_profile.document);

        -- Migrate to new format
        v_new_doc := "ob-poc".migrate_trading_profile_to_ast(
            v_profile.profile_id,
            v_profile.document,
            v_profile.cbu_id,
            v_cbu_name,
            v_profile.version
        );

        -- Update profile with new document
        UPDATE "ob-poc".cbu_trading_profiles
        SET document = v_new_doc,
            document_hash = md5(v_new_doc::text)
        WHERE profile_id = v_profile.profile_id;

        v_count := v_count + 1;
        RAISE NOTICE 'Migrated profile % for CBU %', v_profile.profile_id, v_cbu_name;
    END LOOP;

    RAISE NOTICE 'Migration complete. Migrated % profiles.', v_count;
END $$;

-- =============================================================================
-- STEP 5: Verification query
-- =============================================================================

-- Check migration results
SELECT
    tp.profile_id,
    c.name as cbu_name,
    tp.version,
    tp.status,
    jsonb_array_length(tp.document->'children') as category_count,
    (tp.document->>'total_leaf_count')::int as leaf_count,
    tp.document->'metadata'->>'source' as migration_source
FROM "ob-poc".cbu_trading_profiles tp
JOIN "ob-poc".cbus c ON c.cbu_id = tp.cbu_id
WHERE tp.document ? 'children'
ORDER BY c.name, tp.version;

-- Show backup count
SELECT COUNT(*) as backed_up_profiles
FROM "ob-poc".trading_profile_migration_backup;

COMMIT;

-- =============================================================================
-- ROLLBACK INSTRUCTIONS (if needed)
-- =============================================================================
-- To rollback the migration:
--
-- BEGIN;
-- UPDATE "ob-poc".cbu_trading_profiles tp
-- SET document = b.original_document,
--     document_hash = md5(b.original_document::text)
-- FROM "ob-poc".trading_profile_migration_backup b
-- WHERE tp.profile_id = b.profile_id;
-- COMMIT;
--
-- Then optionally drop the backup table:
-- DROP TABLE "ob-poc".trading_profile_migration_backup;
-- =============================================================================
