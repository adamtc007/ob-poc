-- Migration: Add compiled verb contract storage to dsl_verbs
-- Purpose: Store full compiled verb definitions for reproducibility and auditability
-- Date: 2026-01-04

-- Add compiled contract storage columns to existing dsl_verbs table
ALTER TABLE "ob-poc".dsl_verbs
ADD COLUMN IF NOT EXISTS compiled_json JSONB,
ADD COLUMN IF NOT EXISTS effective_config_json JSONB,
ADD COLUMN IF NOT EXISTS diagnostics_json JSONB DEFAULT '{"errors":[],"warnings":[]}',
ADD COLUMN IF NOT EXISTS compiled_hash BYTEA;

-- Add index for compiled_hash lookups (for finding executions by verb config)
CREATE INDEX IF NOT EXISTS ix_dsl_verbs_compiled_hash
  ON "ob-poc".dsl_verbs (compiled_hash)
  WHERE compiled_hash IS NOT NULL;

-- Add comments for documentation
COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_json IS
  'Full RuntimeVerb serialized as JSON - the complete compiled contract';
COMMENT ON COLUMN "ob-poc".dsl_verbs.effective_config_json IS
  'Expanded configuration with all defaults applied (for debugging)';
COMMENT ON COLUMN "ob-poc".dsl_verbs.diagnostics_json IS
  'Compilation diagnostics: {"errors": [...], "warnings": [...]}';
COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_hash IS
  'SHA256 of canonical compiled_json for integrity verification';
