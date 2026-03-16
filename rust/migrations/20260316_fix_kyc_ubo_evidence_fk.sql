-- Phase 0.2: Fix stale FK target on kyc_ubo_evidence
-- Source: P3-A CR-2, P4 RISK-2
-- Problem: kyc_ubo_evidence.ubo_registry_id references stale kyc_ubo_registry
-- instead of live ubo_registry table.

-- Drop stale FK
ALTER TABLE "ob-poc".kyc_ubo_evidence
  DROP CONSTRAINT IF EXISTS kyc_ubo_evidence_ubo_registry_id_fkey;

-- Add correct FK to live table
ALTER TABLE "ob-poc".kyc_ubo_evidence
  ADD CONSTRAINT kyc_ubo_evidence_ubo_registry_id_fkey
  FOREIGN KEY (ubo_registry_id) REFERENCES "ob-poc".ubo_registry(ubo_registry_id)
  ON DELETE CASCADE;

-- Fix PK default: gen_random_uuid() → uuidv7()
ALTER TABLE "ob-poc".kyc_ubo_evidence
  ALTER COLUMN evidence_id SET DEFAULT uuidv7();

-- Add missing updated_at column
ALTER TABLE "ob-poc".kyc_ubo_evidence
  ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
