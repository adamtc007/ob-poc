-- Pilot P.2 pre-requisite (2026-04-23): extend cbus.status CHECK
-- constraint with SUSPENDED and ARCHIVED per A-1 v3 Q7 domain answer.
--
-- Parent docs:
--   docs/todo/instrument-matrix-slot-inventory-2026-04-23.md §3.1
--   docs/todo/instrument-matrix-dag-dsl-break-remediation-2026-04-23.md
--
-- Context.
--   A-1 v3 Q7: Adam confirmed that CBU has suspended and archived states
--   missing from the current CHECK list. These are real operational
--   states; the schema needs to admit them before pilot P.3 declares
--   any CBU transition verb targeting them.
--
-- Rollout policy.
--   * Forward-only. Once deployed, consumers may write the new values.
--   * Additive. All existing rows stay valid (their `status` values
--     remain in the old set, which is a subset of the new).
--   * No data migration required. No backfill. No trigger changes.
--
-- Before this migration:
--   CHECK (status IN ('DISCOVERED', 'VALIDATION_PENDING', 'VALIDATED',
--                     'UPDATE_PENDING_PROOF', 'VALIDATION_FAILED'))
--
-- After this migration:
--   CHECK (status IN ('DISCOVERED', 'VALIDATION_PENDING', 'VALIDATED',
--                     'UPDATE_PENDING_PROOF', 'VALIDATION_FAILED',
--                     'SUSPENDED', 'ARCHIVED'))
--
-- Note on ordering: CHECK constraints in Postgres don't have a natural
-- order for IN-sets, but keeping the source list stable aids diff
-- review against master-schema.sql.

BEGIN;

-- Drop the existing constraint by name. This is safe because:
--   1. No existing row has a status outside the old set (the old CHECK
--      was enforcing exactly that), so a brief constraint-less window
--      exposes no new semantic.
--   2. The ADD below restores constraint coverage immediately within
--      the same transaction.
ALTER TABLE "ob-poc".cbus
    DROP CONSTRAINT IF EXISTS chk_cbu_status;

ALTER TABLE "ob-poc".cbus
    ADD CONSTRAINT chk_cbu_status CHECK (
        (status)::text = ANY (ARRAY[
            'DISCOVERED'::character varying,
            'VALIDATION_PENDING'::character varying,
            'VALIDATED'::character varying,
            'UPDATE_PENDING_PROOF'::character varying,
            'VALIDATION_FAILED'::character varying,
            'SUSPENDED'::character varying,
            'ARCHIVED'::character varying
        ]::text[])
    );

COMMIT;

-- Verification (run manually after migration):
--   SELECT conname, pg_get_constraintdef(oid)
--     FROM pg_constraint
--     WHERE conname = 'chk_cbu_status';
--
-- Expected output includes SUSPENDED and ARCHIVED in the IN list.
