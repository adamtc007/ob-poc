-- EOP-PLAN-GAMEBOARD-001 follow-on (2026-08-19): close the holding.create
-- bypass that let all 49 historical TA-usage holdings skip the investors
-- register entirely.
--
-- Root cause: "ob-poc".holdings.usage_type defaulted to 'TA', and
-- holding.create (the generic/legacy verb, keyed by investor_entity_id ->
-- entities, with no investor-id or usage-type arg at all) never set
-- usage_type or investor_id explicitly -- every row it created silently
-- picked up the TA default with investor_id left NULL. holding.create is
-- structurally incapable of the TA/investor-register path (it has no way
-- to reference "ob-poc".investors), so TA was never a correct default for
-- it in the first place; UBO is. Real TA holdings must go through
-- holding.create-for-investor (requires investor-id, required usage-type
-- enum) or holding.ensure (optional investor-id/usage-type, but at least
-- capable of setting them).
--
-- Two changes:
--   1. Flip the column default TA -> UBO. holding.create's upsert never
--      lists usage_type in its args, so this alone forces every future
--      holding.create call to land as UBO, not TA.
--   2. Add a CHECK constraint as defense in depth, independent of which
--      verb (or future verb, or direct SQL) performs the write: a TA-usage
--      holding must carry a real investor_id. Added NOT VALID because 4
--      live rows predate this fix and violate it -- all 4 are pre-existing
--      captest_*_majority_holder test fixture rows (not real production
--      data; the 49-row backfill in 20260819_backfill_investors_register_
--      allianz.sql was deliberately scoped to the one real Allianz
--      investor, per Adam's explicit "Allianz only" ruling, and did not
--      touch these). NOT VALID skips validating existing rows so this
--      migration doesn't touch/fix that pre-existing test data; new/updated
--      rows are checked immediately. Validate later once/if those 4 rows
--      are resolved.

ALTER TABLE "ob-poc".holdings
    ALTER COLUMN usage_type SET DEFAULT 'UBO'::character varying;

ALTER TABLE "ob-poc".holdings
    ADD CONSTRAINT chk_holdings_ta_requires_investor_id
    CHECK (usage_type IS DISTINCT FROM 'TA' OR investor_id IS NOT NULL)
    NOT VALID;

COMMENT ON CONSTRAINT chk_holdings_ta_requires_investor_id ON "ob-poc".holdings IS
    'A usage_type=TA holding must be linked to a real "ob-poc".investors row via investor_id -- TA is the Transfer Agency / KYC-as-a-Service register, not a free-text label. Added NOT VALID 2026-08-19; 4 pre-existing captest_*_majority_holder test rows are grandfathered exceptions, not validated.';
