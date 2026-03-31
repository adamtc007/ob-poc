-- Migration 129: Two-stage instrument matrix — group template + CBU instance
--
-- The instrument matrix is a two-stage model:
--   Stage 1: Group-level template (cbu_id=NULL, is_template=true, group_id set)
--   Stage 2: CBU-specific instance cloned from template (cbu_id set, is_template=false)
--
-- Changes:
--   - cbu_id: nullable (was NOT NULL) — templates have no CBU
--   - group_id: new column — links template to client group
--   - is_template: new column — distinguishes templates from instances
--   - template_id: new column — links instance back to source template
--   - Unique constraint relaxed to allow NULL cbu_id for templates
--   - Index on group_id for template lookup

-- Make cbu_id nullable (idempotent — check if already nullable)
DO $$
BEGIN
    -- cbu_id may already be nullable if applied manually
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
          AND table_name = 'cbu_trading_profiles'
          AND column_name = 'cbu_id'
          AND is_nullable = 'NO'
    ) THEN
        ALTER TABLE "ob-poc".cbu_trading_profiles ALTER COLUMN cbu_id DROP NOT NULL;
    END IF;
END $$;

-- Add group_id column (idempotent)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
          AND table_name = 'cbu_trading_profiles'
          AND column_name = 'group_id'
    ) THEN
        ALTER TABLE "ob-poc".cbu_trading_profiles ADD COLUMN group_id uuid;
    END IF;
END $$;

-- Add is_template column (idempotent)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
          AND table_name = 'cbu_trading_profiles'
          AND column_name = 'is_template'
    ) THEN
        ALTER TABLE "ob-poc".cbu_trading_profiles
            ADD COLUMN is_template boolean NOT NULL DEFAULT false;
    END IF;
END $$;

-- Add template_id column (idempotent) — links cloned instance to source template
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'ob-poc'
          AND table_name = 'cbu_trading_profiles'
          AND column_name = 'template_id'
    ) THEN
        ALTER TABLE "ob-poc".cbu_trading_profiles ADD COLUMN template_id uuid;
    END IF;
END $$;

-- Index for template lookup by group
CREATE INDEX IF NOT EXISTS idx_cbu_trading_profiles_group_template
    ON "ob-poc".cbu_trading_profiles (group_id)
    WHERE is_template = true;

-- Index for instance lookup by template source
CREATE INDEX IF NOT EXISTS idx_cbu_trading_profiles_template_id
    ON "ob-poc".cbu_trading_profiles (template_id)
    WHERE template_id IS NOT NULL;

-- Comment
COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.group_id IS 'Client group ID for group-level templates (NULL for CBU instances)';
COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.is_template IS 'True for group-level templates, false for CBU-specific instances';
COMMENT ON COLUMN "ob-poc".cbu_trading_profiles.template_id IS 'Source template profile_id when this instance was cloned from a template';
