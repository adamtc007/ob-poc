-- Trading profile templates: group-level instrument matrix that exists
-- independently of CBUs. Attaching to a CBU clones the template.
--
-- Two-state model:
--   Template (group_id set, cbu_id NULL, is_template=true)
--   Instance (group_id set, cbu_id set, template_id points to source)

-- Make cbu_id nullable (templates don't have a CBU)
ALTER TABLE "ob-poc".cbu_trading_profiles
  ALTER COLUMN cbu_id DROP NOT NULL;

-- Add template columns
ALTER TABLE "ob-poc".cbu_trading_profiles
  ADD COLUMN IF NOT EXISTS group_id uuid REFERENCES "ob-poc".client_group(id),
  ADD COLUMN IF NOT EXISTS is_template boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS template_id uuid REFERENCES "ob-poc".cbu_trading_profiles(profile_id);

-- Index for group-level template lookup
CREATE INDEX IF NOT EXISTS idx_trading_profiles_group_template
  ON "ob-poc".cbu_trading_profiles (group_id)
  WHERE is_template = true;

-- Index for finding instances cloned from a template
CREATE INDEX IF NOT EXISTS idx_trading_profiles_template_source
  ON "ob-poc".cbu_trading_profiles (template_id)
  WHERE template_id IS NOT NULL;

-- Backfill: existing profiles get is_template=false (they're all CBU instances)
-- group_id can be populated later from client_group_entity linkage
