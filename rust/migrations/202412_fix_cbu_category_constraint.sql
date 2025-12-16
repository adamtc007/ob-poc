-- Fix conflicting CHECK constraints on cbus.cbu_category
-- There are two constraints with different allowed values:
-- - cbus_category_check: allows FAMILY_TRUST but not INTERNAL_TEST
-- - chk_cbu_category: allows INTERNAL_TEST but not FAMILY_TRUST
--
-- This migration drops both and creates a single unified constraint
-- that allows all values from both original constraints.

BEGIN;

-- Drop the conflicting constraints
ALTER TABLE "ob-poc".cbus DROP CONSTRAINT IF EXISTS cbus_category_check;
ALTER TABLE "ob-poc".cbus DROP CONSTRAINT IF EXISTS chk_cbu_category;

-- Create unified constraint with all valid values
ALTER TABLE "ob-poc".cbus ADD CONSTRAINT chk_cbu_category CHECK (
    cbu_category IS NULL OR cbu_category IN (
        'FUND_MANDATE',
        'CORPORATE_GROUP',
        'INSTITUTIONAL_ACCOUNT',
        'RETAIL_CLIENT',
        'FAMILY_TRUST',
        'CORRESPONDENT_BANK',
        'INTERNAL_TEST'
    )
);

-- Update column comment to reflect all valid values
COMMENT ON COLUMN "ob-poc".cbus.cbu_category IS
    'Template discriminator for visualization layout: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, RETAIL_CLIENT, FAMILY_TRUST, CORRESPONDENT_BANK, INTERNAL_TEST';

COMMIT;
