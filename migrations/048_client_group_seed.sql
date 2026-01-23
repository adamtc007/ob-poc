-- ============================================================================
-- Client Group Bootstrap Data
-- Seeds: Allianz (full), Aviva (partial), test groups
-- ============================================================================

-- ============================================================================
-- Allianz Group (comprehensive - has entities in DB)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Allianz Global Investors', 'AGI', 'Allianz asset management arm')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- Allianz aliases (common nicknames and variations)
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Allianz Global Investors', 'allianz global investors', true, 'bootstrap', 1.0),
    ('11111111-1111-1111-1111-111111111111', 'Allianz', 'allianz', false, 'bootstrap', 1.0),
    ('11111111-1111-1111-1111-111111111111', 'AGI', 'agi', false, 'bootstrap', 0.95),
    ('11111111-1111-1111-1111-111111111111', 'AllianzGI', 'allianzgi', false, 'bootstrap', 0.98),
    ('11111111-1111-1111-1111-111111111111', 'Allianz GI', 'allianz gi', false, 'bootstrap', 0.98),
    ('11111111-1111-1111-1111-111111111111', 'Allianz Asset Management', 'allianz asset management', false, 'bootstrap', 0.90)
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- Allianz anchor mappings
-- Allianz SE = ultimate parent (7b6942b5-10e9-425f-b8c9-5a674a7d0701)
-- Allianz Global Investors Holdings GmbH = governance controller (084d316f-fa4e-42f0-ac39-1b01a3fbdf27)
INSERT INTO "ob-poc".client_group_anchor (group_id, anchor_entity_id, anchor_role, jurisdiction, priority, confidence, notes) VALUES
    -- Ultimate parent (global)
    ('11111111-1111-1111-1111-111111111111', '7b6942b5-10e9-425f-b8c9-5a674a7d0701', 'ultimate_parent', '', 10, 1.0, 'Allianz SE - Group apex'),
    -- Governance controller (global fallback)
    ('11111111-1111-1111-1111-111111111111', '084d316f-fa4e-42f0-ac39-1b01a3fbdf27', 'governance_controller', '', 10, 1.0, 'AGI Holdings GmbH - Global ManCo'),
    -- Book controller (global - same as governance for now)
    ('11111111-1111-1111-1111-111111111111', '084d316f-fa4e-42f0-ac39-1b01a3fbdf27', 'book_controller', '', 10, 1.0, 'AGI Holdings GmbH')
ON CONFLICT (group_id, anchor_role, anchor_entity_id, jurisdiction) DO NOTHING;

-- ============================================================================
-- Aviva Group (has some entities)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('22222222-2222-2222-2222-222222222222', 'Aviva Investors', 'AVIVA', 'Aviva asset management arm')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- Aviva aliases
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('22222222-2222-2222-2222-222222222222', 'Aviva Investors', 'aviva investors', true, 'bootstrap', 1.0),
    ('22222222-2222-2222-2222-222222222222', 'Aviva', 'aviva', false, 'bootstrap', 1.0),
    ('22222222-2222-2222-2222-222222222222', 'AI', 'ai', false, 'bootstrap', 0.7),  -- Lower confidence - ambiguous
    ('22222222-2222-2222-2222-222222222222', 'Aviva IM', 'aviva im', false, 'bootstrap', 0.95)
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- Aviva anchor mappings
-- Using Aviva Investors (5db4b67a-d500-4093-a3b6-a25c7bc0595a) as governance controller
-- Using Aviva Investors Global (8e2b1b10-a73c-4687-b218-e9283b22f940) as book controller
INSERT INTO "ob-poc".client_group_anchor (group_id, anchor_entity_id, anchor_role, jurisdiction, priority, confidence, notes) VALUES
    -- Ultimate parent (using Aviva Investors for now - no plc in DB)
    ('22222222-2222-2222-2222-222222222222', '5db4b67a-d500-4093-a3b6-a25c7bc0595a', 'ultimate_parent', '', 5, 0.9, 'Aviva Investors - placeholder apex'),
    -- Governance controller
    ('22222222-2222-2222-2222-222222222222', '5db4b67a-d500-4093-a3b6-a25c7bc0595a', 'governance_controller', '', 10, 1.0, 'Aviva Investors'),
    -- Luxembourg-specific controller
    ('22222222-2222-2222-2222-222222222222', 'f1fc872d-1ce2-478c-9a87-c0acf7f22a74', 'governance_controller', 'LU', 20, 1.0, 'Aviva Investors Luxembourg'),
    -- Book controller
    ('22222222-2222-2222-2222-222222222222', '8e2b1b10-a73c-4687-b218-e9283b22f940', 'book_controller', '', 10, 1.0, 'Aviva Investors Global')
ON CONFLICT (group_id, anchor_role, anchor_entity_id, jurisdiction) DO NOTHING;

-- ============================================================================
-- BlackRock Group (minimal - only one entity in DB)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('33333333-3333-3333-3333-333333333333', 'BlackRock', 'BLK', 'BlackRock asset management')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- BlackRock aliases
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('33333333-3333-3333-3333-333333333333', 'BlackRock', 'blackrock', true, 'bootstrap', 1.0),
    ('33333333-3333-3333-3333-333333333333', 'BLK', 'blk', false, 'bootstrap', 0.95),
    ('33333333-3333-3333-3333-333333333333', 'Black Rock', 'black rock', false, 'bootstrap', 0.90)
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- BlackRock anchor mapping (only has Transition Management entity)
INSERT INTO "ob-poc".client_group_anchor (group_id, anchor_entity_id, anchor_role, jurisdiction, priority, confidence, notes) VALUES
    ('33333333-3333-3333-3333-333333333333', '5598c9bf-8508-4f07-b484-aa78f296a09a', 'governance_controller', '', 5, 0.7, 'BlackRock Transition Management - placeholder')
ON CONFLICT (group_id, anchor_role, anchor_entity_id, jurisdiction) DO NOTHING;

-- ============================================================================
-- Test Group (for disambiguation testing)
-- ============================================================================
INSERT INTO "ob-poc".client_group (id, canonical_name, short_code, description) VALUES
    ('44444444-4444-4444-4444-444444444444', 'Aberdeen Global Infrastructure', 'AGI-ABER', 'Aberdeen infrastructure fund')
ON CONFLICT (id) DO UPDATE SET
    canonical_name = EXCLUDED.canonical_name,
    short_code = EXCLUDED.short_code,
    description = EXCLUDED.description;

-- Aberdeen aliases (shares "AGI" with Allianz for disambiguation testing)
INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, is_primary, source, confidence) VALUES
    ('44444444-4444-4444-4444-444444444444', 'Aberdeen Global Infrastructure', 'aberdeen global infrastructure', true, 'bootstrap', 1.0),
    ('44444444-4444-4444-4444-444444444444', 'Aberdeen', 'aberdeen', false, 'bootstrap', 0.95),
    ('44444444-4444-4444-4444-444444444444', 'AGI', 'agi', false, 'bootstrap', 0.80)  -- Same as Allianz - tests disambiguation
ON CONFLICT (group_id, alias_norm) DO NOTHING;

-- ============================================================================
-- Verify data
-- ============================================================================
DO $$
DECLARE
    group_count INT;
    alias_count INT;
    anchor_count INT;
BEGIN
    SELECT COUNT(*) INTO group_count FROM "ob-poc".client_group;
    SELECT COUNT(*) INTO alias_count FROM "ob-poc".client_group_alias;
    SELECT COUNT(*) INTO anchor_count FROM "ob-poc".client_group_anchor;

    RAISE NOTICE 'Client group seed complete: % groups, % aliases, % anchors',
        group_count, alias_count, anchor_count;
END $$;
