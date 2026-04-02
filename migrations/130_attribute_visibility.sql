-- Two-tier attribute model: add visibility column + system source.
-- Default 'external' ensures all existing rows are backward-compatible.

ALTER TABLE "ob-poc".attribute_registry
    ADD COLUMN IF NOT EXISTS visibility text NOT NULL DEFAULT 'external';

ALTER TABLE "ob-poc".attribute_registry
    ADD CONSTRAINT chk_visibility CHECK (visibility IN ('external', 'internal'));

-- Add 'system' source to cbu_attr_values
ALTER TABLE "ob-poc".cbu_attr_values
    DROP CONSTRAINT IF EXISTS cbu_attr_values_source_check;

ALTER TABLE "ob-poc".cbu_attr_values
    ADD CONSTRAINT cbu_attr_values_source_check
    CHECK (source IN ('derived', 'entity', 'cbu', 'document', 'manual', 'external', 'system'));

-- Add 'system' source to cbu_unified_attr_requirements.preferred_source
ALTER TABLE "ob-poc".cbu_unified_attr_requirements
    DROP CONSTRAINT IF EXISTS cbu_unified_attr_requirements_preferred_source_check;

ALTER TABLE "ob-poc".cbu_unified_attr_requirements
    ADD CONSTRAINT cbu_unified_attr_requirements_preferred_source_check
    CHECK (preferred_source IN ('derived', 'entity', 'cbu', 'document', 'manual', 'external', 'system'));
