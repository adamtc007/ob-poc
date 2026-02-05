-- =============================================================================
-- Migration 067: Deal Record & Fee Billing
-- =============================================================================
-- Deal Record is the upstream commercial container that links Sales origination
-- through contracting, onboarding, servicing, and billing in a closed loop.
--
-- Lifecycle: Sales Opportunity → Deal Created → Contracts Negotiated →
--            Contract Signed → Onboarding Requests Spawned → CBU Subscribed →
--            Activity Generated → Fee Billing → Invoice → Client Entity
--
-- The Deal lives as long as the client relationship - parallel tracks run
-- concurrently (contracting, onboarding, servicing, billing).
-- =============================================================================

-- =============================================================================
-- 1. DEAL RECORD - The Hub Entity
-- =============================================================================

CREATE TABLE "ob-poc".deals (
    deal_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Identity
    deal_name       VARCHAR(255) NOT NULL,
    deal_reference  VARCHAR(100) UNIQUE,          -- Internal deal tracking ref

    -- Primary client group (always rolls up to one: Blackrock, Allianz, etc.)
    primary_client_group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id),

    -- Sales ownership
    sales_owner     VARCHAR(255),                 -- Lead sales contact
    sales_team      VARCHAR(255),                 -- Team/desk

    -- Lifecycle - parallel tracks, not linear
    deal_status     VARCHAR(50) NOT NULL DEFAULT 'PROSPECT',
    -- PROSPECT | QUALIFYING | NEGOTIATING | CONTRACTED | ONBOARDING |
    -- ACTIVE | WINDING_DOWN | OFFBOARDED | CANCELLED

    -- Value tracking
    estimated_revenue   NUMERIC(18,2),            -- Estimated annual revenue
    currency_code       VARCHAR(3) DEFAULT 'USD',

    -- Dates
    opened_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    qualified_at    TIMESTAMPTZ,                  -- When opportunity was qualified
    contracted_at   TIMESTAMPTZ,                  -- First contract signed
    active_at       TIMESTAMPTZ,                  -- First CBU onboarded & live
    closed_at       TIMESTAMPTZ,                  -- Offboarded / cancelled

    -- Audit
    notes           TEXT,
    metadata        JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_deals_primary_client ON "ob-poc".deals(primary_client_group_id);
CREATE INDEX idx_deals_status ON "ob-poc".deals(deal_status);
CREATE INDEX idx_deals_client_status ON "ob-poc".deals(primary_client_group_id, deal_status);
CREATE INDEX idx_deals_sales_owner ON "ob-poc".deals(sales_owner);
CREATE INDEX idx_deals_opened_at ON "ob-poc".deals(opened_at);

COMMENT ON TABLE "ob-poc".deals IS 'Hub entity for commercial origination - links sales through contracting, onboarding, and billing';
COMMENT ON COLUMN "ob-poc".deals.deal_status IS 'PROSPECT | QUALIFYING | NEGOTIATING | CONTRACTED | ONBOARDING | ACTIVE | WINDING_DOWN | OFFBOARDED | CANCELLED';

-- =============================================================================
-- 2. DEAL PARTICIPANTS - Regional entities contracting under the deal
-- =============================================================================
-- Under a Blackrock deal, Blackrock UK (separate LEI), Blackrock Luxembourg, etc.
-- each participate as distinct legal entities.

CREATE TABLE "ob-poc".deal_participants (
    deal_participant_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    entity_id       UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Role in the deal
    participant_role VARCHAR(50) NOT NULL DEFAULT 'CONTRACTING_PARTY',
    -- CONTRACTING_PARTY | GUARANTOR | INTRODUCER | INVESTMENT_MANAGER | FUND_ADMIN

    -- This entity's LEI (denormalised for quick reference)
    lei             VARCHAR(20),

    is_primary      BOOLEAN DEFAULT false,        -- The main contracting entity

    created_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(deal_id, entity_id, participant_role)
);

-- Only one primary participant per deal
CREATE UNIQUE INDEX idx_deal_participants_one_primary
    ON "ob-poc".deal_participants(deal_id) WHERE is_primary = true;

CREATE INDEX idx_deal_participants_deal ON "ob-poc".deal_participants(deal_id);
CREATE INDEX idx_deal_participants_entity ON "ob-poc".deal_participants(entity_id);

COMMENT ON TABLE "ob-poc".deal_participants IS 'Regional entities (by LEI) participating in a deal';
COMMENT ON COLUMN "ob-poc".deal_participants.participant_role IS 'CONTRACTING_PARTY | GUARANTOR | INTRODUCER | INVESTMENT_MANAGER | FUND_ADMIN';

-- =============================================================================
-- 3. DEAL → CONTRACT LINKS
-- =============================================================================

CREATE TABLE "ob-poc".deal_contracts (
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    contract_id     UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id),

    -- Context
    contract_role   VARCHAR(50) DEFAULT 'PRIMARY',
    -- PRIMARY | ADDENDUM | SCHEDULE | SIDE_LETTER | NDA

    sequence_order  INT NOT NULL DEFAULT 1,       -- Ordering within the deal

    created_at      TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (deal_id, contract_id)
);

CREATE INDEX idx_deal_contracts_contract ON "ob-poc".deal_contracts(contract_id);

COMMENT ON TABLE "ob-poc".deal_contracts IS 'Links deals to legal contracts';
COMMENT ON COLUMN "ob-poc".deal_contracts.contract_role IS 'PRIMARY | ADDENDUM | SCHEDULE | SIDE_LETTER | NDA';

-- =============================================================================
-- 4. NEGOTIATED RATE CARDS
-- =============================================================================
-- Product-level pricing negotiated as part of this deal.
-- Links: Deal → Contract → Product → Negotiated Rates

CREATE TABLE "ob-poc".deal_rate_cards (
    rate_card_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    contract_id     UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id),
    product_id      UUID NOT NULL REFERENCES "ob-poc".products(product_id),

    -- Rate card identity
    rate_card_name  VARCHAR(255),
    effective_from  DATE NOT NULL,
    effective_to    DATE,                         -- NULL = open-ended

    -- Status
    status          VARCHAR(50) DEFAULT 'DRAFT',
    -- DRAFT | PROPOSED | COUNTER_OFFERED | AGREED | SUPERSEDED | CANCELLED
    negotiation_round INT DEFAULT 1,

    -- Version chain (superseded cards link to replacement)
    superseded_by   UUID REFERENCES "ob-poc".deal_rate_cards(rate_card_id),

    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_deal_rate_cards_deal ON "ob-poc".deal_rate_cards(deal_id);
CREATE INDEX idx_deal_rate_cards_contract ON "ob-poc".deal_rate_cards(contract_id);
CREATE INDEX idx_deal_rate_cards_product ON "ob-poc".deal_rate_cards(product_id);
CREATE INDEX idx_deal_rate_cards_status ON "ob-poc".deal_rate_cards(status);

COMMENT ON TABLE "ob-poc".deal_rate_cards IS 'Negotiated product pricing per deal/contract';
COMMENT ON COLUMN "ob-poc".deal_rate_cards.status IS 'DRAFT | PROPOSED | COUNTER_OFFERED | AGREED | SUPERSEDED | CANCELLED';

-- Individual fee lines within a rate card
CREATE TABLE "ob-poc".deal_rate_card_lines (
    line_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rate_card_id    UUID NOT NULL REFERENCES "ob-poc".deal_rate_cards(rate_card_id) ON DELETE CASCADE,

    -- What's being priced
    fee_type        VARCHAR(100) NOT NULL,        -- CUSTODY, FUND_ACCOUNTING, TA, FX, SETTLEMENT
    fee_subtype     VARCHAR(100) NOT NULL DEFAULT 'DEFAULT', -- Per-market, per-asset-class breakdown

    -- Pricing model
    pricing_model   VARCHAR(50) NOT NULL,
    -- BPS | FLAT | PER_TRANSACTION | TIERED | SPREAD | MINIMUM_FEE

    -- Rate details
    rate_value      NUMERIC(18,6),                -- BPS or per-unit rate
    minimum_fee     NUMERIC(18,2),                -- Floor
    maximum_fee     NUMERIC(18,2),                -- Cap
    currency_code   VARCHAR(3) DEFAULT 'USD',

    -- Tiered pricing (if pricing_model = TIERED)
    tier_brackets   JSONB,                        -- [{from: 0, to: 1000000, rate: 5.0}, ...]

    -- Basis for calculation
    fee_basis       VARCHAR(100),                 -- AUM | NAV | TRADE_COUNT | POSITION_COUNT

    -- Context
    description     TEXT,

    sequence_order  INT,
    created_at      TIMESTAMPTZ DEFAULT NOW(),

    -- Structural invariants: pricing model must have required columns
    CONSTRAINT chk_bps_requires_rate CHECK (
        pricing_model != 'BPS' OR (rate_value IS NOT NULL AND fee_basis IS NOT NULL)
    ),
    CONSTRAINT chk_per_txn_requires_rate CHECK (
        pricing_model != 'PER_TRANSACTION' OR rate_value IS NOT NULL
    ),
    CONSTRAINT chk_tiered_requires_brackets CHECK (
        pricing_model != 'TIERED' OR tier_brackets IS NOT NULL
    ),

    -- One line per fee_type/subtype per rate card
    UNIQUE(rate_card_id, fee_type, fee_subtype)
);

CREATE INDEX idx_deal_rate_card_lines_card ON "ob-poc".deal_rate_card_lines(rate_card_id);

COMMENT ON TABLE "ob-poc".deal_rate_card_lines IS 'Individual fee lines within a negotiated rate card';
COMMENT ON COLUMN "ob-poc".deal_rate_card_lines.pricing_model IS 'BPS | FLAT | PER_TRANSACTION | TIERED | SPREAD | MINIMUM_FEE';
COMMENT ON COLUMN "ob-poc".deal_rate_card_lines.fee_basis IS 'AUM | NAV | TRADE_COUNT | POSITION_COUNT';

-- =============================================================================
-- 5. DEAL → SLA LINKS
-- =============================================================================

CREATE TABLE "ob-poc".deal_slas (
    sla_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    contract_id     UUID REFERENCES "ob-poc".legal_contracts(contract_id),
    product_id      UUID REFERENCES "ob-poc".products(product_id),
    service_id      UUID REFERENCES "ob-poc".services(service_id),

    -- SLA details
    sla_name        VARCHAR(255) NOT NULL,
    sla_type        VARCHAR(50),                  -- AVAILABILITY | TURNAROUND | ACCURACY | REPORTING

    -- Metric
    metric_name     VARCHAR(100) NOT NULL,        -- e.g. "NAV Delivery Time"
    target_value    VARCHAR(100) NOT NULL,        -- e.g. "T+1 by 08:00 EST"
    measurement_unit VARCHAR(50),                 -- HOURS | PERCENT | COUNT

    -- Breach handling
    penalty_type    VARCHAR(50),                  -- FEE_REBATE | CREDIT | ESCALATION
    penalty_value   NUMERIC(18,2),

    effective_from  DATE NOT NULL,
    effective_to    DATE,

    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_deal_slas_deal ON "ob-poc".deal_slas(deal_id);
CREATE INDEX idx_deal_slas_product ON "ob-poc".deal_slas(product_id);

COMMENT ON TABLE "ob-poc".deal_slas IS 'Service level agreements associated with a deal';
COMMENT ON COLUMN "ob-poc".deal_slas.sla_type IS 'AVAILABILITY | TURNAROUND | ACCURACY | REPORTING';
COMMENT ON COLUMN "ob-poc".deal_slas.penalty_type IS 'FEE_REBATE | CREDIT | ESCALATION';

-- =============================================================================
-- 6. DEAL → DOCUMENT LINKS
-- =============================================================================

CREATE TABLE "ob-poc".deal_documents (
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    document_id     UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),

    document_type   VARCHAR(50) NOT NULL,
    -- CONTRACT | TERM_SHEET | SIDE_LETTER | NDA | RATE_SCHEDULE | SLA |
    -- PROPOSAL | RFP_RESPONSE | BOARD_APPROVAL | LEGAL_OPINION

    document_status VARCHAR(50) DEFAULT 'DRAFT',
    -- DRAFT | UNDER_REVIEW | SIGNED | EXECUTED | SUPERSEDED | ARCHIVED

    version         INT DEFAULT 1,

    created_at      TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (deal_id, document_id)
);

CREATE INDEX idx_deal_documents_document ON "ob-poc".deal_documents(document_id);

COMMENT ON TABLE "ob-poc".deal_documents IS 'Links deals to documents in the document catalog';
COMMENT ON COLUMN "ob-poc".deal_documents.document_type IS 'CONTRACT | TERM_SHEET | SIDE_LETTER | NDA | RATE_SCHEDULE | SLA | PROPOSAL | RFP_RESPONSE | BOARD_APPROVAL | LEGAL_OPINION';
COMMENT ON COLUMN "ob-poc".deal_documents.document_status IS 'DRAFT | UNDER_REVIEW | SIGNED | EXECUTED | SUPERSEDED | ARCHIVED';

-- =============================================================================
-- 7. DEAL → UBO TAXONOMY LINK
-- =============================================================================

CREATE TABLE "ob-poc".deal_ubo_assessments (
    assessment_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    entity_id       UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Link to KYC case that handles the UBO work
    kyc_case_id     UUID REFERENCES kyc.cases(case_id),

    -- UBO assessment status
    assessment_status VARCHAR(50) DEFAULT 'PENDING',
    -- PENDING | IN_PROGRESS | COMPLETED | REQUIRES_EDD | BLOCKED

    risk_rating     VARCHAR(50),                  -- LOW | MEDIUM | HIGH | PROHIBITED

    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    -- One assessment per entity per deal
    UNIQUE(deal_id, entity_id)
);

CREATE INDEX idx_deal_ubo_deal ON "ob-poc".deal_ubo_assessments(deal_id);
CREATE INDEX idx_deal_ubo_entity ON "ob-poc".deal_ubo_assessments(entity_id);
CREATE INDEX idx_deal_ubo_kyc_case ON "ob-poc".deal_ubo_assessments(kyc_case_id);

COMMENT ON TABLE "ob-poc".deal_ubo_assessments IS 'UBO/KYC assessment tracking per entity in a deal';
COMMENT ON COLUMN "ob-poc".deal_ubo_assessments.assessment_status IS 'PENDING | IN_PROGRESS | COMPLETED | REQUIRES_EDD | BLOCKED';
COMMENT ON COLUMN "ob-poc".deal_ubo_assessments.risk_rating IS 'LOW | MEDIUM | HIGH | PROHIBITED';

-- =============================================================================
-- 8. DEAL → ONBOARDING REQUESTS (the handoff)
-- =============================================================================

CREATE TABLE "ob-poc".deal_onboarding_requests (
    request_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,
    contract_id     UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id),

    -- What's being onboarded
    cbu_id          UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id      UUID NOT NULL REFERENCES "ob-poc".products(product_id),

    -- Request lifecycle
    request_status  VARCHAR(50) DEFAULT 'REQUESTED',
    -- REQUESTED | KYC_PENDING | KYC_CLEARED | IN_PROGRESS |
    -- COMPLETED | BLOCKED | CANCELLED

    -- KYC linkage
    requires_kyc    BOOLEAN DEFAULT true,
    kyc_case_id     UUID REFERENCES kyc.cases(case_id),
    kyc_cleared_at  TIMESTAMPTZ,

    -- Dates
    requested_at    TIMESTAMPTZ DEFAULT NOW(),
    target_live_date DATE,
    completed_at    TIMESTAMPTZ,

    -- Audit
    requested_by    VARCHAR(255),
    notes           TEXT,

    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    -- Prevent duplicate onboarding requests
    UNIQUE(deal_id, contract_id, cbu_id, product_id)
);

CREATE INDEX idx_deal_ob_requests_deal ON "ob-poc".deal_onboarding_requests(deal_id);
CREATE INDEX idx_deal_ob_requests_cbu ON "ob-poc".deal_onboarding_requests(cbu_id);
CREATE INDEX idx_deal_ob_requests_product ON "ob-poc".deal_onboarding_requests(product_id);
CREATE INDEX idx_deal_ob_requests_status ON "ob-poc".deal_onboarding_requests(request_status);
CREATE INDEX idx_deal_ob_requests_deal_status ON "ob-poc".deal_onboarding_requests(deal_id, request_status);

COMMENT ON TABLE "ob-poc".deal_onboarding_requests IS 'Handoff from Sales to Ops - onboarding request per CBU/product';
COMMENT ON COLUMN "ob-poc".deal_onboarding_requests.request_status IS 'REQUESTED | KYC_PENDING | KYC_CLEARED | IN_PROGRESS | COMPLETED | BLOCKED | CANCELLED';

-- =============================================================================
-- 9. FEE BILLING PROFILES - The Closed Loop
-- =============================================================================
-- Bridges commercial (deal/contract/rate_card) to operational (cbu/product).
-- The fee billing profile is the bridge between "what was commercially agreed"
-- and "what is operationally running and generating billable activity"

CREATE TABLE "ob-poc".fee_billing_profiles (
    profile_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Commercial side (what was sold)
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
    contract_id     UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id),
    rate_card_id    UUID NOT NULL REFERENCES "ob-poc".deal_rate_cards(rate_card_id),

    -- Operational side (what's running)
    cbu_id          UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id      UUID NOT NULL REFERENCES "ob-poc".products(product_id),

    -- Profile identity
    profile_name    VARCHAR(255),
    billing_frequency VARCHAR(50) NOT NULL DEFAULT 'MONTHLY',
    -- DAILY | WEEKLY | MONTHLY | QUARTERLY | ANNUALLY

    -- Status
    status          VARCHAR(50) DEFAULT 'PENDING',
    -- PENDING | ACTIVE | SUSPENDED | CLOSED

    -- Invoice target - which client entity receives the invoice
    invoice_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    invoice_currency  VARCHAR(3) DEFAULT 'USD',

    -- Payment
    payment_method  VARCHAR(50),                  -- ACH | WIRE | DEBIT_FROM_ACCOUNT
    payment_account_ref VARCHAR(255),             -- Account reference for auto-debit

    effective_from  DATE NOT NULL,
    effective_to    DATE,

    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    -- Prevent duplicate billing profiles for same CBU/product/rate card
    UNIQUE(cbu_id, product_id, rate_card_id)
);

CREATE INDEX idx_fee_billing_deal ON "ob-poc".fee_billing_profiles(deal_id);
CREATE INDEX idx_fee_billing_cbu ON "ob-poc".fee_billing_profiles(cbu_id);
CREATE INDEX idx_fee_billing_invoice_entity ON "ob-poc".fee_billing_profiles(invoice_entity_id);
CREATE INDEX idx_fee_billing_status ON "ob-poc".fee_billing_profiles(status);
CREATE INDEX idx_fee_billing_rate_card ON "ob-poc".fee_billing_profiles(rate_card_id);

COMMENT ON TABLE "ob-poc".fee_billing_profiles IS 'Bridges commercial (deal/contract/rate_card) to operational (cbu/product) for billing';
COMMENT ON COLUMN "ob-poc".fee_billing_profiles.billing_frequency IS 'DAILY | WEEKLY | MONTHLY | QUARTERLY | ANNUALLY';
COMMENT ON COLUMN "ob-poc".fee_billing_profiles.status IS 'PENDING | ACTIVE | SUSPENDED | CLOSED';
COMMENT ON COLUMN "ob-poc".fee_billing_profiles.payment_method IS 'ACH | WIRE | DEBIT_FROM_ACCOUNT';

-- =============================================================================
-- 10. FEE BILLING ACCOUNT TARGETS
-- =============================================================================
-- Links fee billing profiles to specific CBU resource instances (accounts,
-- funds, portfolios) that generate billable activity. This is the closed loop.

CREATE TABLE "ob-poc".fee_billing_account_targets (
    target_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id      UUID NOT NULL REFERENCES "ob-poc".fee_billing_profiles(profile_id) ON DELETE CASCADE,

    -- The operational resource generating activity
    cbu_resource_instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    -- Which rate card line applies to this resource's activity
    rate_card_line_id UUID REFERENCES "ob-poc".deal_rate_card_lines(line_id),

    -- Resource context (denormalised for billing queries)
    resource_type   VARCHAR(100),                 -- CUSTODY_ACCOUNT | FUND | PORTFOLIO
    resource_ref    VARCHAR(255),                 -- Account number / fund code

    -- Activity tracking
    activity_type   VARCHAR(100),                 -- TRANSACTIONS | AUM | NAV | POSITIONS

    -- Override pricing (if this specific account has special terms)
    has_override    BOOLEAN DEFAULT false,
    override_rate   NUMERIC(18,6),
    override_model  VARCHAR(50),

    -- Status
    is_active       BOOLEAN DEFAULT true,

    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_fee_targets_profile ON "ob-poc".fee_billing_account_targets(profile_id);
CREATE INDEX idx_fee_targets_resource ON "ob-poc".fee_billing_account_targets(cbu_resource_instance_id);
CREATE INDEX idx_fee_targets_active ON "ob-poc".fee_billing_account_targets(profile_id) WHERE is_active = true;
CREATE INDEX idx_fee_targets_rate_line ON "ob-poc".fee_billing_account_targets(rate_card_line_id);

COMMENT ON TABLE "ob-poc".fee_billing_account_targets IS 'Links billing profiles to CBU resource instances that generate billable activity';
COMMENT ON COLUMN "ob-poc".fee_billing_account_targets.activity_type IS 'TRANSACTIONS | AUM | NAV | POSITIONS';

-- =============================================================================
-- 11. FEE BILLING PERIODS & CALCULATIONS
-- =============================================================================

CREATE TABLE "ob-poc".fee_billing_periods (
    period_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id      UUID NOT NULL REFERENCES "ob-poc".fee_billing_profiles(profile_id),

    -- Billing window
    period_start    DATE NOT NULL,
    period_end      DATE NOT NULL,

    -- Calculation status
    calc_status     VARCHAR(50) DEFAULT 'PENDING',
    -- PENDING | CALCULATING | CALCULATED | REVIEWED | APPROVED | INVOICED | DISPUTED

    -- Totals
    gross_amount    NUMERIC(18,2),
    adjustments     NUMERIC(18,2) DEFAULT 0,      -- Credits, rebates, SLA penalties
    net_amount      NUMERIC(18,2),
    currency_code   VARCHAR(3),

    -- Invoice linkage
    invoice_id      UUID,                         -- FK to invoice when generated
    invoiced_at     TIMESTAMPTZ,

    -- Audit
    calculated_at   TIMESTAMPTZ,
    reviewed_by     VARCHAR(255),
    reviewed_at     TIMESTAMPTZ,
    approved_by     VARCHAR(255),
    approved_at     TIMESTAMPTZ,

    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(profile_id, period_start, period_end)
);

CREATE INDEX idx_fee_periods_profile ON "ob-poc".fee_billing_periods(profile_id);
CREATE INDEX idx_fee_periods_status ON "ob-poc".fee_billing_periods(calc_status);
CREATE INDEX idx_fee_periods_dates ON "ob-poc".fee_billing_periods(period_start, period_end);

COMMENT ON TABLE "ob-poc".fee_billing_periods IS 'Billing period windows with calculation status and totals';
COMMENT ON COLUMN "ob-poc".fee_billing_periods.calc_status IS 'PENDING | CALCULATING | CALCULATED | REVIEWED | APPROVED | INVOICED | DISPUTED';

-- Line-level detail per billing period
CREATE TABLE "ob-poc".fee_billing_period_lines (
    period_line_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    period_id       UUID NOT NULL REFERENCES "ob-poc".fee_billing_periods(period_id) ON DELETE CASCADE,
    target_id       UUID NOT NULL REFERENCES "ob-poc".fee_billing_account_targets(target_id),
    rate_card_line_id UUID REFERENCES "ob-poc".deal_rate_card_lines(line_id),

    -- Activity metrics for this period
    activity_volume NUMERIC(18,4),                -- Trade count, AUM, etc.
    activity_unit   VARCHAR(50),                  -- TRADES | USD_AUM | POSITIONS

    -- Fee calculation
    applied_rate    NUMERIC(18,6),
    calculated_fee  NUMERIC(18,2),
    adjustment      NUMERIC(18,2) DEFAULT 0,
    net_fee         NUMERIC(18,2),

    -- Breakdown
    calculation_detail JSONB,                     -- Full calc audit trail

    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_fee_period_lines_period ON "ob-poc".fee_billing_period_lines(period_id);
CREATE INDEX idx_fee_period_lines_target ON "ob-poc".fee_billing_period_lines(target_id);

COMMENT ON TABLE "ob-poc".fee_billing_period_lines IS 'Line-level fee calculations per billing period per account target';

-- =============================================================================
-- 12. DEAL ACTIVITY LOG
-- =============================================================================

CREATE TABLE "ob-poc".deal_events (
    event_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id) ON DELETE CASCADE,

    event_type      VARCHAR(100) NOT NULL,
    -- DEAL_CREATED | STATUS_CHANGED | CONTRACT_ADDED | RATE_CARD_CREATED |
    -- RATE_CARD_PROPOSED | RATE_CARD_AGREED | SLA_AGREED |
    -- ONBOARDING_REQUESTED | KYC_CLEARED | CBU_ONBOARDED |
    -- BILLING_PROFILE_CREATED | BILLING_ACTIVATED | INVOICE_GENERATED |
    -- NOTE_ADDED

    -- What changed
    subject_type    VARCHAR(50),                  -- DEAL | CONTRACT | RATE_CARD | SLA | CBU | etc.
    subject_id      UUID,

    -- Details
    old_value       VARCHAR(255),
    new_value       VARCHAR(255),
    description     TEXT,

    -- Who
    actor           VARCHAR(255),

    occurred_at     TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_deal_events_deal ON "ob-poc".deal_events(deal_id);
CREATE INDEX idx_deal_events_type ON "ob-poc".deal_events(event_type);
CREATE INDEX idx_deal_events_deal_occurred ON "ob-poc".deal_events(deal_id, occurred_at);
CREATE INDEX idx_deal_events_subject ON "ob-poc".deal_events(subject_type, subject_id);

COMMENT ON TABLE "ob-poc".deal_events IS 'Audit trail for all deal-related activity';

-- =============================================================================
-- 13. SUMMARY VIEWS
-- =============================================================================

-- Deal summary with counts
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
    COUNT(DISTINCT dp.entity_id) as participant_count,
    COUNT(DISTINCT dc.contract_id) as contract_count,
    COUNT(DISTINCT dr.rate_card_id) as rate_card_count,
    COUNT(DISTINCT dor.request_id) as onboarding_request_count,
    COUNT(DISTINCT dor.request_id) FILTER (WHERE dor.request_status = 'COMPLETED') as completed_onboarding_count,
    COUNT(DISTINCT fb.profile_id) as billing_profile_count
FROM "ob-poc".deals d
LEFT JOIN "ob-poc".client_group cg ON d.primary_client_group_id = cg.id
LEFT JOIN "ob-poc".deal_participants dp ON d.deal_id = dp.deal_id
LEFT JOIN "ob-poc".deal_contracts dc ON d.deal_id = dc.deal_id
LEFT JOIN "ob-poc".deal_rate_cards dr ON d.deal_id = dr.deal_id
LEFT JOIN "ob-poc".deal_onboarding_requests dor ON d.deal_id = dor.deal_id
LEFT JOIN "ob-poc".fee_billing_profiles fb ON d.deal_id = fb.deal_id
GROUP BY d.deal_id, d.deal_name, d.deal_reference, d.deal_status, d.sales_owner,
         d.estimated_revenue, d.currency_code, d.opened_at, cg.canonical_name;

COMMENT ON VIEW "ob-poc".v_deal_summary IS 'Summary view of deals with related entity counts';

-- Billing profile with revenue
CREATE OR REPLACE VIEW "ob-poc".v_billing_profile_summary AS
SELECT
    fb.profile_id,
    fb.profile_name,
    fb.status,
    fb.billing_frequency,
    fb.effective_from,
    d.deal_name,
    c.name as cbu_name,
    p.name as product_name,
    e.search_name as invoice_entity_name,
    COUNT(DISTINCT bt.target_id) as account_target_count,
    COUNT(DISTINCT bp.period_id) as period_count,
    COALESCE(SUM(bp.net_amount), 0) as total_billed_amount,
    fb.invoice_currency
FROM "ob-poc".fee_billing_profiles fb
JOIN "ob-poc".deals d ON fb.deal_id = d.deal_id
JOIN "ob-poc".cbus c ON fb.cbu_id = c.cbu_id
JOIN "ob-poc".products p ON fb.product_id = p.product_id
JOIN "ob-poc".entities e ON fb.invoice_entity_id = e.entity_id
LEFT JOIN "ob-poc".fee_billing_account_targets bt ON fb.profile_id = bt.profile_id
LEFT JOIN "ob-poc".fee_billing_periods bp ON fb.profile_id = bp.profile_id
GROUP BY fb.profile_id, fb.profile_name, fb.status, fb.billing_frequency, fb.effective_from,
         d.deal_name, c.name, p.name, e.search_name, fb.invoice_currency;

COMMENT ON VIEW "ob-poc".v_billing_profile_summary IS 'Summary view of billing profiles with related entities and totals';
