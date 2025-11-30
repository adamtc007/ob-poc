-- ============================================
-- Fix CBUs unique constraint for cbu.ensure upsert
-- The cbu.ensure verb uses ON CONFLICT (name, jurisdiction)
-- ============================================

BEGIN;

-- Drop the existing unique constraint on name alone
ALTER TABLE "ob-poc".cbus DROP CONSTRAINT IF EXISTS cbus_name_key;

-- Add composite unique constraint on (name, jurisdiction)
-- This allows same-named CBUs in different jurisdictions
ALTER TABLE "ob-poc".cbus ADD CONSTRAINT cbus_name_jurisdiction_key UNIQUE (name, jurisdiction);

COMMIT;

-- Verify
SELECT conname, pg_get_constraintdef(oid)
FROM pg_constraint
WHERE conrelid = '"ob-poc".cbus'::regclass AND contype = 'u';
