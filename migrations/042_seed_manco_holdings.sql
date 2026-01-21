-- Migration 042: Seed ManCo Holdings for Governance Controller Testing
--
-- Purpose: Create holdings showing ManCo ownership of SICAV share classes
-- This enables the governance controller to detect board appointment rights via ownership
--
-- Idempotent: Uses ON CONFLICT DO NOTHING

BEGIN;

-- Create holdings showing Allianz Global Investors GmbH owns 100% of each SICAV's default share class
-- This gives the ManCo voting control which triggers board appointment right detection

INSERT INTO kyc.holdings (
    id,
    share_class_id,
    investor_entity_id,
    units,
    status,
    acquisition_date,
    created_at,
    updated_at
)
SELECT
    gen_random_uuid(),
    sc.id AS share_class_id,
    '4f463925-53f4-4a71-aabe-65584074db6b'::uuid AS investor_entity_id,  -- Allianz Global Investors GmbH
    1000000 AS units,  -- Match the supply we created (100% ownership)
    'active' AS status,
    CURRENT_DATE AS acquisition_date,
    NOW() AS created_at,
    NOW() AS updated_at
FROM kyc.share_classes sc
WHERE sc.name LIKE '%Default Class%'
  AND sc.issuer_entity_id IN (
      -- Get SICAV entities from Allianz CBUs
      SELECT DISTINCT cer.entity_id
      FROM "ob-poc".cbu_entity_roles cer
      JOIN "ob-poc".roles r ON r.role_id = cer.role_id AND r.name = 'SICAV'
      JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
      JOIN "ob-poc".entities apex ON apex.entity_id = c.commercial_client_entity_id
      WHERE apex.name ILIKE '%allianz%'
  )
ON CONFLICT (share_class_id, investor_entity_id) DO NOTHING;

COMMIT;

-- Verify holdings were created
-- SELECT COUNT(*) as holdings_count, investor_entity_id
-- FROM kyc.holdings
-- WHERE investor_entity_id = '4f463925-53f4-4a71-aabe-65584074db6b'
-- GROUP BY investor_entity_id;
