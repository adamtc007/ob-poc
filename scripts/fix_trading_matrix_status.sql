-- Fix trading matrix status to ACTIVE
UPDATE "ob-poc".cbu_trading_profiles
SET status = 'ACTIVE'
WHERE cbu_id = 'daa59bd3-a6a1-4030-a181-d0c256ee3e86';

-- Verify
SELECT c.name, tp.status, tp.document IS NOT NULL as has_doc
FROM "ob-poc".cbus c
JOIN "ob-poc".cbu_trading_profiles tp ON c.cbu_id = tp.cbu_id
WHERE c.cbu_id = 'daa59bd3-a6a1-4030-a181-d0c256ee3e86';
