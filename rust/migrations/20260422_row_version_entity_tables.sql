-- Phase D.1 (F9 follow-on, 2026-04-22): row_version columns on entity
-- tables used by the gate surface.
--
-- The three-plane v0.3 spec §10.5 defines `StateGateHash` as a BLAKE3
-- hash over (entity_id, row_version) pairs of the resolved entities at
-- gate time. The runtime re-checks this hash inside the dispatch
-- transaction after acquiring row locks (Phase D.3) and aborts on
-- mismatch — closing the TOCTOU window between gate decision and write.
--
-- Without a monotonic row_version column per entity, the hash cannot
-- be computed deterministically (using `updated_at` would hash the
-- wall clock; using `xmin` is backend-specific and tricky to export).
--
-- This migration adds `row_version bigint NOT NULL DEFAULT 1` to the
-- core entity tables plus a trigger that increments it on every UPDATE.
-- The column is nullable-safe on insert via the DEFAULT; existing rows
-- get row_version=1 at migration time.
--
-- Rollout policy:
--  * Forward-only. Once deployed, consumers assume the column exists.
--  * Additive only. No existing queries break (column is unreferenced
--    until Phase D.3 wires the recheck).
--  * The trigger uses `pg_trigger_depth() = 0` so cascading updates
--    from other triggers don't double-increment.
--
-- Tables covered (gate-surface entities per v0.3 R13 audit):
--   "ob-poc".cbus
--   "ob-poc".entities
--   "ob-poc".kyc_cases
--   "ob-poc".deals
--   "ob-poc".client_groups
--
-- Tables NOT covered by this migration (deliberate — not on the gate
-- surface): taxonomy / registry catalog tables, audit trails, research
-- action logs, session traces. Those are append-only or versioned by
-- other means.

BEGIN;

-- ---------------------------------------------------------------------------
-- Shared bump function. Runs on BEFORE UPDATE, increments row_version by 1.
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION "ob-poc".bump_row_version()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    -- Only bump on the top-level update — nested trigger-driven updates
    -- share the same row_version as their parent so a "single logical
    -- commit" has a single version step.
    IF pg_trigger_depth() > 1 THEN
        RETURN NEW;
    END IF;
    NEW.row_version := COALESCE(OLD.row_version, 0) + 1;
    RETURN NEW;
END;
$$;

-- ---------------------------------------------------------------------------
-- cbus
-- ---------------------------------------------------------------------------
ALTER TABLE "ob-poc".cbus
    ADD COLUMN IF NOT EXISTS row_version bigint NOT NULL DEFAULT 1;

DROP TRIGGER IF EXISTS trg_cbus_bump_row_version ON "ob-poc".cbus;
CREATE TRIGGER trg_cbus_bump_row_version
    BEFORE UPDATE ON "ob-poc".cbus
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".bump_row_version();

-- ---------------------------------------------------------------------------
-- entities
-- ---------------------------------------------------------------------------
ALTER TABLE "ob-poc".entities
    ADD COLUMN IF NOT EXISTS row_version bigint NOT NULL DEFAULT 1;

DROP TRIGGER IF EXISTS trg_entities_bump_row_version ON "ob-poc".entities;
CREATE TRIGGER trg_entities_bump_row_version
    BEFORE UPDATE ON "ob-poc".entities
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".bump_row_version();

-- ---------------------------------------------------------------------------
-- kyc_cases
-- ---------------------------------------------------------------------------
ALTER TABLE "ob-poc".kyc_cases
    ADD COLUMN IF NOT EXISTS row_version bigint NOT NULL DEFAULT 1;

DROP TRIGGER IF EXISTS trg_kyc_cases_bump_row_version ON "ob-poc".kyc_cases;
CREATE TRIGGER trg_kyc_cases_bump_row_version
    BEFORE UPDATE ON "ob-poc".kyc_cases
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".bump_row_version();

-- ---------------------------------------------------------------------------
-- deals
-- ---------------------------------------------------------------------------
ALTER TABLE "ob-poc".deals
    ADD COLUMN IF NOT EXISTS row_version bigint NOT NULL DEFAULT 1;

DROP TRIGGER IF EXISTS trg_deals_bump_row_version ON "ob-poc".deals;
CREATE TRIGGER trg_deals_bump_row_version
    BEFORE UPDATE ON "ob-poc".deals
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".bump_row_version();

-- ---------------------------------------------------------------------------
-- client_groups
-- ---------------------------------------------------------------------------
ALTER TABLE "ob-poc".client_groups
    ADD COLUMN IF NOT EXISTS row_version bigint NOT NULL DEFAULT 1;

DROP TRIGGER IF EXISTS trg_client_groups_bump_row_version ON "ob-poc".client_groups;
CREATE TRIGGER trg_client_groups_bump_row_version
    BEFORE UPDATE ON "ob-poc".client_groups
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".bump_row_version();

COMMIT;

-- ---------------------------------------------------------------------------
-- Verification queries (run manually after migration):
--
--   SELECT table_name, column_name
--     FROM information_schema.columns
--     WHERE table_schema = 'ob-poc'
--       AND column_name = 'row_version'
--     ORDER BY table_name;
--
--   SELECT tgname, tgrelid::regclass
--     FROM pg_trigger
--     WHERE tgname LIKE 'trg_%_bump_row_version'
--     ORDER BY tgname;
-- ---------------------------------------------------------------------------
