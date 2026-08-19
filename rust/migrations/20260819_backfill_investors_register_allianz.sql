-- EOP-PLAN-GAMEBOARD-001 R2 follow-on — provision a real "ob-poc".investors
-- register row for the one genuine live TA-usage investor found behind
-- holdings.investor_entity_id: "Allianz Global Investors GmbH" (45 of the
-- 49 usage_type='TA' holdings, 45,000,000 units). The other 4 distinct
-- investor entities behind the remaining 4 TA holdings are named
-- captest_<hash>_majority_holder (1 holding each, exactly 600 units
-- apiece) — scripted test fixture data, not real investors; deliberately
-- NOT backfilled (would misrepresent test data as a verified production
-- register entry).
--
-- Provisioned as booked/verified, not alleged: this is an established,
-- already-operational position (45 real holdings), not a new allegation
-- needing a fresh KYC pass. `holding.create-for-investor` already models
-- this exact duality (provider set + holding-status ACTIVE = booked;
-- absent provider + holding-status PENDING = alleged) — this backfill
-- provisions the investors-side half of that already-designed shape.
--
-- Idempotent + environment-safe: no-ops if the entity doesn't exist in
-- this database (a fresh bootstrap or a different dev DB) or if a
-- matching investors row already exists.

DO $$
DECLARE
    v_entity_id uuid := '4f463925-53f4-4a71-aabe-65584074db6b';
    v_investor_id uuid;
BEGIN
    IF NOT EXISTS (SELECT 1 FROM "ob-poc".entities WHERE entity_id = v_entity_id) THEN
        RETURN;
    END IF;

    IF EXISTS (SELECT 1 FROM "ob-poc".investors WHERE entity_id = v_entity_id) THEN
        RETURN;
    END IF;

    INSERT INTO "ob-poc".investors (entity_id, investor_type, lifecycle_state, kyc_status, provider)
    VALUES (v_entity_id, 'INSTITUTIONAL', 'ACTIVE', 'APPROVED', 'MANUAL')
    RETURNING investor_id INTO v_investor_id;

    UPDATE "ob-poc".holdings
    SET investor_id = v_investor_id
    WHERE investor_entity_id = v_entity_id
      AND usage_type = 'TA'
      AND investor_id IS NULL;
END $$;
