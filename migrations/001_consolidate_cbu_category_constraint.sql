-- Migration: 001_consolidate_cbu_category_constraint.sql
-- Purpose: Consolidate duplicate CBU category CHECK constraints into a single constraint
--
-- Problem: The cbus table has TWO CHECK constraints on cbu_category with different values:
--   - cbus_category_check includes FAMILY_TRUST but not INTERNAL_TEST
--   - chk_cbu_category includes INTERNAL_TEST but not FAMILY_TRUST
--
-- This migration consolidates them into a single constraint with all valid values.

BEGIN;

-- Drop both existing constraints
ALTER TABLE "ob-poc".cbus DROP CONSTRAINT IF EXISTS cbus_category_check;
ALTER TABLE "ob-poc".cbus DROP CONSTRAINT IF EXISTS chk_cbu_category;

-- Add single consolidated constraint with all valid values
ALTER TABLE "ob-poc".cbus ADD CONSTRAINT chk_cbu_category CHECK (
  cbu_category IS NULL OR cbu_category IN (
    'FUND_MANDATE',
    'CORPORATE_GROUP',
    'INSTITUTIONAL_ACCOUNT',
    'RETAIL_CLIENT',
    'FAMILY_TRUST',
    'INTERNAL_TEST',
    'CORRESPONDENT_BANK'
  )
);

-- Update column comment to reflect all valid values
COMMENT ON COLUMN "ob-poc".cbus.cbu_category IS
  'Template discriminator for visualization layout: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, RETAIL_CLIENT, FAMILY_TRUST, INTERNAL_TEST, CORRESPONDENT_BANK';

COMMIT;
