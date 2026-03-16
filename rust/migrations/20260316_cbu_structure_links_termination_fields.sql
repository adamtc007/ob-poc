BEGIN;

ALTER TABLE "ob-poc".cbu_structure_links
    ADD COLUMN IF NOT EXISTS terminated_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS terminated_reason TEXT;

COMMIT;
