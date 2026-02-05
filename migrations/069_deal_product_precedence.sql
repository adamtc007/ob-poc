-- Migration 069: Deal → Product Junction Table & Rate Card Precedence
-- =============================================================================
-- Addresses structural gaps in the Deal Record model:
--
-- 1. DEAL_PRODUCTS - Direct junction table showing which products a deal covers
--    (Deal links to Products, which then link down to Rate Cards)
--
-- 2. RATE CARD PRECEDENCE - Enforce that only ONE rate card per deal/contract/product
--    can be in AGREED status at any time (the "active" rate card)
--
-- 3. SUPERSESSION CHAIN - When a rate card is superseded, enforce integrity
-- =============================================================================

-- =============================================================================
-- 1. DEAL → PRODUCTS JUNCTION TABLE
-- =============================================================================
-- A deal covers multiple products. This is the commercial scope.
-- Rate cards are then negotiated per product under the deal.

CREATE TABLE "ob-poc".deal_products (
    deal_product_id UUID PRIMARY KEY DEFAULT uuidv7(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    product_id      UUID NOT NULL REFERENCES "ob-poc".products(product_id),

    -- Status of this product in the deal
    product_status  VARCHAR(50) NOT NULL DEFAULT 'PROPOSED',
    -- PROPOSED | NEGOTIATING | AGREED | DECLINED | REMOVED

    -- Indicative values (pre-rate card)
    indicative_revenue  NUMERIC(18,2),            -- Estimated annual revenue for this product
    currency_code       VARCHAR(3) DEFAULT 'USD',

    -- Notes
    notes           TEXT,

    -- Audit
    added_at        TIMESTAMPTZ DEFAULT NOW(),
    agreed_at       TIMESTAMPTZ,                  -- When product was agreed
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    -- One entry per deal/product
    UNIQUE(deal_id, product_id)
);

-- Status CHECK constraint
ALTER TABLE "ob-poc".deal_products
ADD CONSTRAINT deal_products_status_check CHECK (product_status IN (
    'PROPOSED', 'NEGOTIATING', 'AGREED', 'DECLINED', 'REMOVED'
));

CREATE INDEX idx_deal_products_deal ON "ob-poc".deal_products(deal_id);
CREATE INDEX idx_deal_products_product ON "ob-poc".deal_products(product_id);
CREATE INDEX idx_deal_products_status ON "ob-poc".deal_products(product_status);

COMMENT ON TABLE "ob-poc".deal_products IS 'Products covered by a deal - the commercial scope before rate card negotiation';
COMMENT ON COLUMN "ob-poc".deal_products.product_status IS 'PROPOSED | NEGOTIATING | AGREED | DECLINED | REMOVED';

-- =============================================================================
-- 2. RATE CARD PRECEDENCE - Only ONE AGREED rate card per deal/contract/product
-- =============================================================================
-- Business rule: At any given time, there can only be ONE active (AGREED) rate
-- card for a specific deal + contract + product combination. Old rate cards
-- must be SUPERSEDED before a new one can be AGREED.

-- Partial unique index: only one AGREED rate card per deal/contract/product
CREATE UNIQUE INDEX idx_deal_rate_cards_one_agreed
ON "ob-poc".deal_rate_cards(deal_id, contract_id, product_id)
WHERE status = 'AGREED';

COMMENT ON INDEX "ob-poc".idx_deal_rate_cards_one_agreed IS 'Enforces only ONE AGREED rate card per deal/contract/product';

-- =============================================================================
-- 3. SUPERSESSION CHAIN INTEGRITY
-- =============================================================================
-- When a rate card is superseded:
-- - Its status MUST be 'SUPERSEDED'
-- - superseded_by MUST point to a valid rate card for the SAME deal/contract/product
-- - The superseding card MUST have status 'AGREED' or 'PROPOSED' (or newer state)

-- Trigger function to validate supersession chain
CREATE OR REPLACE FUNCTION "ob-poc".validate_rate_card_supersession()
RETURNS TRIGGER AS $$
DECLARE
    superseding_card RECORD;
    old_card RECORD;
BEGIN
    -- Only validate when superseded_by is being set
    IF NEW.superseded_by IS NOT NULL THEN
        -- The card being superseded must have status SUPERSEDED
        IF NEW.status != 'SUPERSEDED' THEN
            RAISE EXCEPTION 'Rate card with superseded_by set must have status SUPERSEDED, got: %', NEW.status;
        END IF;

        -- Fetch the superseding card
        SELECT * INTO superseding_card
        FROM "ob-poc".deal_rate_cards
        WHERE rate_card_id = NEW.superseded_by;

        IF NOT FOUND THEN
            RAISE EXCEPTION 'superseded_by references non-existent rate card: %', NEW.superseded_by;
        END IF;

        -- The superseding card must be for the same deal/contract/product
        IF superseding_card.deal_id != NEW.deal_id
           OR superseding_card.contract_id != NEW.contract_id
           OR superseding_card.product_id != NEW.product_id THEN
            RAISE EXCEPTION 'superseded_by must reference a rate card for the same deal/contract/product';
        END IF;

        -- The superseding card should not be CANCELLED or SUPERSEDED itself
        IF superseding_card.status IN ('CANCELLED', 'SUPERSEDED') THEN
            RAISE EXCEPTION 'Cannot supersede to a CANCELLED or SUPERSEDED rate card';
        END IF;
    END IF;

    -- When status changes TO 'AGREED', check no other AGREED exists
    -- (The unique index handles this, but we provide a better error message)
    IF NEW.status = 'AGREED' AND (TG_OP = 'INSERT' OR OLD.status != 'AGREED') THEN
        IF EXISTS (
            SELECT 1 FROM "ob-poc".deal_rate_cards
            WHERE deal_id = NEW.deal_id
            AND contract_id = NEW.contract_id
            AND product_id = NEW.product_id
            AND status = 'AGREED'
            AND rate_card_id != NEW.rate_card_id
        ) THEN
            RAISE EXCEPTION 'Cannot set status to AGREED: another AGREED rate card exists for this deal/contract/product. Supersede the existing card first.';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_validate_rate_card_supersession
BEFORE INSERT OR UPDATE ON "ob-poc".deal_rate_cards
FOR EACH ROW
EXECUTE FUNCTION "ob-poc".validate_rate_card_supersession();

COMMENT ON FUNCTION "ob-poc".validate_rate_card_supersession() IS 'Validates rate card supersession chain integrity';

-- =============================================================================
-- 4. HELPER FUNCTION: Supersede Rate Card
-- =============================================================================
-- Atomically supersedes an existing rate card and sets the new one to AGREED

CREATE OR REPLACE FUNCTION "ob-poc".supersede_rate_card(
    p_old_rate_card_id UUID,
    p_new_rate_card_id UUID
) RETURNS VOID AS $$
DECLARE
    old_card RECORD;
    new_card RECORD;
BEGIN
    -- Lock both cards
    SELECT * INTO old_card FROM "ob-poc".deal_rate_cards
    WHERE rate_card_id = p_old_rate_card_id FOR UPDATE;

    SELECT * INTO new_card FROM "ob-poc".deal_rate_cards
    WHERE rate_card_id = p_new_rate_card_id FOR UPDATE;

    IF old_card IS NULL THEN
        RAISE EXCEPTION 'Old rate card not found: %', p_old_rate_card_id;
    END IF;

    IF new_card IS NULL THEN
        RAISE EXCEPTION 'New rate card not found: %', p_new_rate_card_id;
    END IF;

    -- Validate same deal/contract/product
    IF old_card.deal_id != new_card.deal_id
       OR old_card.contract_id != new_card.contract_id
       OR old_card.product_id != new_card.product_id THEN
        RAISE EXCEPTION 'Rate cards must be for the same deal/contract/product';
    END IF;

    -- Old card must be AGREED to be superseded
    IF old_card.status != 'AGREED' THEN
        RAISE EXCEPTION 'Can only supersede an AGREED rate card, current status: %', old_card.status;
    END IF;

    -- New card must be in a state ready to become AGREED
    IF new_card.status NOT IN ('DRAFT', 'PROPOSED', 'COUNTER_PROPOSED') THEN
        RAISE EXCEPTION 'New rate card must be in DRAFT, PROPOSED, or COUNTER_PROPOSED status, got: %', new_card.status;
    END IF;

    -- Perform the supersession atomically
    -- 1. Mark old card as SUPERSEDED (trigger validates this)
    UPDATE "ob-poc".deal_rate_cards
    SET status = 'SUPERSEDED',
        superseded_by = p_new_rate_card_id,
        updated_at = NOW()
    WHERE rate_card_id = p_old_rate_card_id;

    -- 2. Mark new card as AGREED
    UPDATE "ob-poc".deal_rate_cards
    SET status = 'AGREED',
        updated_at = NOW()
    WHERE rate_card_id = p_new_rate_card_id;

END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".supersede_rate_card(UUID, UUID) IS 'Atomically supersedes an existing AGREED rate card with a new one';

-- =============================================================================
-- 5. VIEW: Active Rate Cards per Deal/Product
-- =============================================================================
-- Shows the currently active (AGREED) rate card for each deal/contract/product

CREATE OR REPLACE VIEW "ob-poc".v_active_rate_cards AS
SELECT
    drc.rate_card_id,
    drc.deal_id,
    drc.contract_id,
    drc.product_id,
    drc.rate_card_name,
    drc.effective_from,
    drc.effective_to,
    drc.negotiation_round,
    d.deal_name,
    lc.client_label as contract_client,
    p.name as product_name,
    (
        SELECT COUNT(*)
        FROM "ob-poc".deal_rate_card_lines drcl
        WHERE drcl.rate_card_id = drc.rate_card_id
    ) as line_count,
    (
        SELECT COUNT(*)
        FROM "ob-poc".deal_rate_cards prev
        WHERE prev.superseded_by = drc.rate_card_id
    ) as superseded_count
FROM "ob-poc".deal_rate_cards drc
JOIN "ob-poc".deals d ON drc.deal_id = d.deal_id
JOIN "ob-poc".legal_contracts lc ON drc.contract_id = lc.contract_id
JOIN "ob-poc".products p ON drc.product_id = p.product_id
WHERE drc.status = 'AGREED';

COMMENT ON VIEW "ob-poc".v_active_rate_cards IS 'Currently active (AGREED) rate cards per deal/contract/product';

-- =============================================================================
-- 6. VIEW: Rate Card History (Supersession Chain)
-- =============================================================================
-- Shows the full history of rate cards for a deal/product including supersession

CREATE OR REPLACE VIEW "ob-poc".v_rate_card_history AS
WITH RECURSIVE chain AS (
    -- Start with active (AGREED) cards
    SELECT
        rate_card_id,
        deal_id,
        contract_id,
        product_id,
        rate_card_name,
        status,
        negotiation_round,
        effective_from,
        superseded_by,
        created_at,
        0 as chain_depth,
        ARRAY[rate_card_id] as chain_path
    FROM "ob-poc".deal_rate_cards
    WHERE status = 'AGREED'

    UNION ALL

    -- Walk backwards through superseded cards
    SELECT
        drc.rate_card_id,
        drc.deal_id,
        drc.contract_id,
        drc.product_id,
        drc.rate_card_name,
        drc.status,
        drc.negotiation_round,
        drc.effective_from,
        drc.superseded_by,
        drc.created_at,
        c.chain_depth + 1,
        c.chain_path || drc.rate_card_id
    FROM "ob-poc".deal_rate_cards drc
    JOIN chain c ON drc.superseded_by = c.rate_card_id
    WHERE drc.rate_card_id != ALL(c.chain_path)  -- Prevent cycles
    AND c.chain_depth < 100  -- Safety limit
)
SELECT
    rate_card_id,
    deal_id,
    contract_id,
    product_id,
    rate_card_name,
    status,
    negotiation_round,
    effective_from,
    superseded_by,
    created_at,
    chain_depth,
    CASE WHEN chain_depth = 0 THEN 'CURRENT' ELSE 'SUPERSEDED' END as chain_status
FROM chain
ORDER BY deal_id, contract_id, product_id, chain_depth;

COMMENT ON VIEW "ob-poc".v_rate_card_history IS 'Full rate card history with supersession chain';

-- =============================================================================
-- 7. UPDATE DEAL SUMMARY VIEW
-- =============================================================================
-- Add product count to deal summary

DROP VIEW IF EXISTS "ob-poc".v_deal_summary;

CREATE OR REPLACE VIEW "ob-poc".v_deal_summary AS
SELECT
    d.deal_id,
    d.deal_name,
    d.deal_reference,
    d.deal_status,
    d.sales_owner,
    d.estimated_revenue,
    d.currency_code,
    d.opened_at,
    cg.canonical_name as client_group_name,
    COUNT(DISTINCT dprod.product_id) as product_count,
    COUNT(DISTINCT dprod.product_id) FILTER (WHERE dprod.product_status = 'AGREED') as agreed_product_count,
    COUNT(DISTINCT dp.entity_id) as participant_count,
    COUNT(DISTINCT dc.contract_id) as contract_count,
    COUNT(DISTINCT dr.rate_card_id) as rate_card_count,
    COUNT(DISTINCT dr.rate_card_id) FILTER (WHERE dr.status = 'AGREED') as agreed_rate_card_count,
    COUNT(DISTINCT dor.request_id) as onboarding_request_count,
    COUNT(DISTINCT dor.request_id) FILTER (WHERE dor.request_status = 'COMPLETED') as completed_onboarding_count,
    COUNT(DISTINCT fb.profile_id) as billing_profile_count
FROM "ob-poc".deals d
LEFT JOIN "ob-poc".client_group cg ON d.primary_client_group_id = cg.id
LEFT JOIN "ob-poc".deal_products dprod ON d.deal_id = dprod.deal_id
LEFT JOIN "ob-poc".deal_participants dp ON d.deal_id = dp.deal_id
LEFT JOIN "ob-poc".deal_contracts dc ON d.deal_id = dc.deal_id
LEFT JOIN "ob-poc".deal_rate_cards dr ON d.deal_id = dr.deal_id
LEFT JOIN "ob-poc".deal_onboarding_requests dor ON d.deal_id = dor.deal_id
LEFT JOIN "ob-poc".fee_billing_profiles fb ON d.deal_id = fb.deal_id
GROUP BY d.deal_id, d.deal_name, d.deal_reference, d.deal_status, d.sales_owner,
         d.estimated_revenue, d.currency_code, d.opened_at, cg.canonical_name;

COMMENT ON VIEW "ob-poc".v_deal_summary IS 'Summary view of deals with related entity counts including products';

-- =============================================================================
-- 8. VERIFICATION
-- =============================================================================

DO $$
DECLARE
    table_count INTEGER;
    index_count INTEGER;
    trigger_count INTEGER;
BEGIN
    -- Check deal_products table exists
    SELECT COUNT(*) INTO table_count
    FROM information_schema.tables
    WHERE table_schema = 'ob-poc' AND table_name = 'deal_products';

    IF table_count != 1 THEN
        RAISE EXCEPTION 'deal_products table not created';
    END IF;

    -- Check unique index for AGREED rate cards
    SELECT COUNT(*) INTO index_count
    FROM pg_indexes
    WHERE schemaname = 'ob-poc'
    AND indexname = 'idx_deal_rate_cards_one_agreed';

    IF index_count != 1 THEN
        RAISE EXCEPTION 'idx_deal_rate_cards_one_agreed index not created';
    END IF;

    -- Check trigger exists (information_schema.triggers uses event_object_schema, not trigger_schema)
    SELECT COUNT(*) INTO trigger_count
    FROM information_schema.triggers
    WHERE event_object_schema = 'ob-poc'
    AND event_object_table = 'deal_rate_cards'
    AND trigger_name = 'trg_validate_rate_card_supersession';

    IF trigger_count < 1 THEN
        RAISE EXCEPTION 'trg_validate_rate_card_supersession trigger not created';
    END IF;

    RAISE NOTICE '✓ Migration 069 verified: deal_products table, precedence index, supersession trigger';
END $$;
