-- Migration 053: Client Group Entity Context Seed Data
-- Seeds entity membership and shorthand tags for existing client groups

BEGIN;

-- ============================================================================
-- Allianz Entity Membership
-- Add known Allianz entities to group membership
-- ============================================================================

-- Add entities matching Allianz naming patterns
INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by, notes)
SELECT
    '11111111-1111-1111-1111-111111111111'::UUID,  -- Allianz group
    e.entity_id,
    'confirmed',
    'bootstrap',
    'Initial seed from entity naming pattern'
FROM "ob-poc".entities e
WHERE e.name ILIKE '%allianz%'
   OR e.name ILIKE 'agi %'
   OR e.name ILIKE '% agi %'
   OR e.entity_id IN (
       -- Known Allianz entities from anchor table
       SELECT anchor_entity_id FROM "ob-poc".client_group_anchor
       WHERE group_id = '11111111-1111-1111-1111-111111111111'
   )
ON CONFLICT (group_id, entity_id) DO NOTHING;

-- ============================================================================
-- Allianz Shorthand Tags
-- ============================================================================

-- Find the AGI GmbH entity (main ManCo) dynamically
DO $$
DECLARE
    v_agi_gmbh_id UUID;
    v_allianz_group_id UUID := '11111111-1111-1111-1111-111111111111';
BEGIN
    -- Find "Allianz Global Investors GmbH" entity
    SELECT entity_id INTO v_agi_gmbh_id
    FROM "ob-poc".entities
    WHERE name = 'Allianz Global Investors GmbH'
    LIMIT 1;

    IF v_agi_gmbh_id IS NOT NULL THEN
        -- Ensure it's a member
        INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by, notes)
        VALUES (v_allianz_group_id, v_agi_gmbh_id, 'confirmed', 'bootstrap', 'Main ManCo')
        ON CONFLICT (group_id, entity_id) DO NOTHING;

        -- Add shorthand tags for the main ManCo
        INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
        VALUES
            (v_allianz_group_id, v_agi_gmbh_id, 'main manco', 'main manco', NULL, 'bootstrap', 1.0),
            (v_allianz_group_id, v_agi_gmbh_id, 'main lux manco', 'main lux manco', NULL, 'bootstrap', 1.0),
            (v_allianz_group_id, v_agi_gmbh_id, 'agi manco', 'agi manco', NULL, 'bootstrap', 1.0),
            (v_allianz_group_id, v_agi_gmbh_id, 'the manco', 'the manco', NULL, 'bootstrap', 0.95),
            (v_allianz_group_id, v_agi_gmbh_id, 'management company', 'management company', NULL, 'bootstrap', 0.90),
            (v_allianz_group_id, v_agi_gmbh_id, 'agi gmbh', 'agi gmbh', NULL, 'bootstrap', 1.0),
            -- Persona-scoped tags
            (v_allianz_group_id, v_agi_gmbh_id, 'manco for kyc', 'manco for kyc', 'kyc', 'bootstrap', 1.0),
            (v_allianz_group_id, v_agi_gmbh_id, 'kyc manco', 'kyc manco', 'kyc', 'bootstrap', 1.0),
            (v_allianz_group_id, v_agi_gmbh_id, 'trading manco', 'trading manco', 'trading', 'bootstrap', 1.0),
            (v_allianz_group_id, v_agi_gmbh_id, 'book manco', 'book manco', 'trading', 'bootstrap', 1.0)
        ON CONFLICT DO NOTHING;

        RAISE NOTICE 'Tagged Allianz Global Investors GmbH (%)', v_agi_gmbh_id;
    ELSE
        RAISE NOTICE 'Allianz Global Investors GmbH not found in entities table';
    END IF;
END $$;

-- Tag SICAV/RAIF funds with generic fund tags
INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
SELECT
    '11111111-1111-1111-1111-111111111111',
    e.entity_id,
    CASE
        WHEN e.name ILIKE '%sicav%' THEN 'sicav fund'
        WHEN e.name ILIKE '%raif%' THEN 'raif fund'
        ELSE 'allianz fund'
    END,
    CASE
        WHEN e.name ILIKE '%sicav%' THEN 'sicav fund'
        WHEN e.name ILIKE '%raif%' THEN 'raif fund'
        ELSE 'allianz fund'
    END,
    NULL,
    'bootstrap',
    0.85
FROM "ob-poc".entities e
WHERE e.name ILIKE '%allianz%'
  AND (e.name ILIKE '%sicav%' OR e.name ILIKE '%raif%' OR e.name ILIKE '%fund%')
ON CONFLICT DO NOTHING;

-- Tag Luxembourg entities
INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
SELECT
    '11111111-1111-1111-1111-111111111111',
    e.entity_id,
    'lux entity',
    'lux entity',
    NULL,
    'bootstrap',
    0.80
FROM "ob-poc".entities e
WHERE e.name ILIKE '%allianz%'
  AND (e.name ILIKE '%s.a.%' OR e.name ILIKE '%sarl%' OR e.name ILIKE '%s.à r.l.%' OR e.name ILIKE '%luxembourg%')
ON CONFLICT DO NOTHING;

-- ============================================================================
-- Aviva Entity Membership
-- ============================================================================

INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by, notes)
SELECT
    '22222222-2222-2222-2222-222222222222'::UUID,  -- Aviva group
    e.entity_id,
    'confirmed',
    'bootstrap',
    'Initial seed from entity naming pattern'
FROM "ob-poc".entities e
WHERE e.name ILIKE '%aviva%'
ON CONFLICT (group_id, entity_id) DO NOTHING;

-- Aviva shorthand tags for anchor entities
INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
SELECT
    '22222222-2222-2222-2222-222222222222',
    cga.anchor_entity_id,
    t.tag,
    "ob-poc".normalize_tag(t.tag),
    NULL,
    'bootstrap',
    1.0
FROM "ob-poc".client_group_anchor cga
CROSS JOIN (VALUES
    ('main manco'),
    ('aviva manco'),
    ('the manco'),
    ('governance controller')
) AS t(tag)
WHERE cga.group_id = '22222222-2222-2222-2222-222222222222'
  AND cga.anchor_role = 'governance_controller'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- BlackRock Entity Membership
-- ============================================================================

INSERT INTO "ob-poc".client_group_entity (group_id, entity_id, membership_type, added_by, notes)
SELECT
    '33333333-3333-3333-3333-333333333333'::UUID,  -- BlackRock group
    e.entity_id,
    'confirmed',
    'bootstrap',
    'Initial seed from entity naming pattern'
FROM "ob-poc".entities e
WHERE e.name ILIKE '%blackrock%'
   OR e.name ILIKE '%black rock%'
ON CONFLICT (group_id, entity_id) DO NOTHING;

-- BlackRock shorthand tags
INSERT INTO "ob-poc".client_group_entity_tag (group_id, entity_id, tag, tag_norm, persona, source, confidence)
SELECT
    '33333333-3333-3333-3333-333333333333',
    cga.anchor_entity_id,
    t.tag,
    "ob-poc".normalize_tag(t.tag),
    NULL,
    'bootstrap',
    0.85
FROM "ob-poc".client_group_anchor cga
CROSS JOIN (VALUES
    ('blackrock manco'),
    ('blk manco'),
    ('transition management')
) AS t(tag)
WHERE cga.group_id = '33333333-3333-3333-3333-333333333333'
  AND cga.anchor_role = 'governance_controller'
ON CONFLICT DO NOTHING;

-- ============================================================================
-- Verify seed
-- ============================================================================
DO $$
DECLARE
    entity_count INT;
    tag_count INT;
    allianz_entities INT;
    aviva_entities INT;
BEGIN
    SELECT COUNT(*) INTO entity_count FROM "ob-poc".client_group_entity;
    SELECT COUNT(*) INTO tag_count FROM "ob-poc".client_group_entity_tag;

    SELECT COUNT(*) INTO allianz_entities
    FROM "ob-poc".client_group_entity
    WHERE group_id = '11111111-1111-1111-1111-111111111111';

    SELECT COUNT(*) INTO aviva_entities
    FROM "ob-poc".client_group_entity
    WHERE group_id = '22222222-2222-2222-2222-222222222222';

    RAISE NOTICE 'Entity context seed complete:';
    RAISE NOTICE '  Total memberships: %', entity_count;
    RAISE NOTICE '  Total tags: %', tag_count;
    RAISE NOTICE '  Allianz entities: %', allianz_entities;
    RAISE NOTICE '  Aviva entities: %', aviva_entities;
END $$;

COMMIT;
