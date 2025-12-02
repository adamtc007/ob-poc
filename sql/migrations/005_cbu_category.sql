-- Migration: Add cbu_category for template-driven visualization
-- This column determines which layout template to use (LUX_SICAV, CAYMAN_MASTER_FEEDER, FAMILY_TRUST, etc.)

-- Add the column
ALTER TABLE "ob-poc".cbus 
ADD COLUMN IF NOT EXISTS cbu_category VARCHAR(30);

COMMENT ON COLUMN "ob-poc".cbus.cbu_category IS 
'Template discriminator for visualization layout: FUND_MANDATE, CORPORATE_GROUP, INSTITUTIONAL_ACCOUNT, RETAIL_CLIENT, FAMILY_TRUST, CORRESPONDENT_BANK';

-- Backfill based on client_type heuristics
UPDATE "ob-poc".cbus SET cbu_category = 
  CASE 
    WHEN client_type ILIKE '%fund%' OR client_type ILIKE '%sicav%' OR client_type ILIKE '%ucits%' THEN 'FUND_MANDATE'
    WHEN client_type ILIKE '%trust%' OR client_type ILIKE '%family%' THEN 'FAMILY_TRUST'
    WHEN client_type ILIKE '%hedge%' OR client_type ILIKE '%master%' OR client_type ILIKE '%feeder%' THEN 'FUND_MANDATE'
    WHEN client_type ILIKE '%institutional%' THEN 'INSTITUTIONAL_ACCOUNT'
    WHEN client_type ILIKE '%retail%' OR client_type ILIKE '%individual%' THEN 'RETAIL_CLIENT'
    WHEN client_type ILIKE '%bank%' OR client_type ILIKE '%correspondent%' THEN 'CORRESPONDENT_BANK'
    WHEN client_type ILIKE '%corporate%' OR client_type ILIKE '%company%' THEN 'CORPORATE_GROUP'
    ELSE 'CORPORATE_GROUP'
  END
WHERE cbu_category IS NULL;

-- Add check constraint for valid values
ALTER TABLE "ob-poc".cbus
DROP CONSTRAINT IF EXISTS cbus_category_check;

ALTER TABLE "ob-poc".cbus
ADD CONSTRAINT cbus_category_check CHECK (
  cbu_category IS NULL OR cbu_category IN (
    'FUND_MANDATE',
    'CORPORATE_GROUP', 
    'INSTITUTIONAL_ACCOUNT',
    'RETAIL_CLIENT',
    'FAMILY_TRUST',
    'CORRESPONDENT_BANK'
  )
);
