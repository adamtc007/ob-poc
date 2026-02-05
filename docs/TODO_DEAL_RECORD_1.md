# TODO: Deal Record & Fee Billing - Schema + DSL Implementation

## Overview

Implement the Deal Record domain - the commercial origination hub that sits upstream
of contracting, onboarding, and servicing. Includes the Fee Billing closed-loop that
connects negotiated rate cards through to billable activity on CBU resource instances.

**Key Principle**: Deal Record is a hub entity with FK spokes. It follows the same
container pattern as CBU - a central record with junction tables linking to all
related domains.

**Closed Loop**: Deal → Contract → Rate Card → Fee Billing Profile → Account Targets
(cbu_resource_instances = the running accounts) → Activity → Fee Calculation →
Invoice → Client Entity

---

## Phase 1: Schema Changes

### 1.1 Pre-Flight FK Validation

Before executing DDL, validate these FK targets against the live schema:

- [ ] `deals.primary_client_group_id` → confirm `client_groups(group_id)` exists or if the correct target is `entities(entity_id)` with a group-type filter. Adjust FK accordingly.
- [ ] `deal_documents.document_id` → identify the existing document store table and column. Adjust FK accordingly.
- [ ] `deal_ubo_assessments.kyc_case_id` → check if `kyc_cases` table exists and what the PK column is.
- [ ] `deal_onboarding_requests.kyc_case_id` → same as above.
- [ ] `fee_billing_account_targets.cbu_resource_instance_id` → verify `cbu_resource_instances(instance_id)` is the correct table/column name.
- [ ] `accounting.service_contracts(contract_id)` → verify exists and is referenceable from `ob-poc` schema.

### 1.2 DDL - Create Tables (in dependency order)

```sql
-- =============================================================================
-- DEAL RECORD SCHEMA - Commercial Origination & Fee Billing Closed Loop
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
    primary_client_group_id UUID NOT NULL REFERENCES "ob-poc".client_groups(group_id),

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

-- =============================================================================
-- 2. DEAL PARTICIPANTS - Regional entities contracting under the deal
-- =============================================================================
-- Under a Blackrock deal, Blackrock UK (separate LEI), Blackrock Luxembourg, etc.
-- each participate as distinct legal entities.

CREATE TABLE "ob-poc".deal_participants (
    deal_participant_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
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

-- =============================================================================
-- 3. DEAL → CONTRACT LINKS
-- =============================================================================

CREATE TABLE "ob-poc".deal_contracts (
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
    contract_id     UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),

    -- Context
    contract_role   VARCHAR(50) DEFAULT 'PRIMARY',
    -- PRIMARY | ADDENDUM | SCHEDULE | SIDE_LETTER | NDA

    sequence_order  INT NOT NULL DEFAULT 1,       -- Ordering within the deal

    created_at      TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (deal_id, contract_id)
);

-- =============================================================================
-- 4. NEGOTIATED RATE CARDS
-- =============================================================================
-- Product-level pricing negotiated as part of this deal.
-- Links: Deal → Contract → Product → Negotiated Rates

CREATE TABLE "ob-poc".deal_rate_cards (
    rate_card_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
    contract_id     UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),
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

-- Individual fee lines within a rate card
CREATE TABLE "ob-poc".deal_rate_card_lines (
    line_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rate_card_id    UUID NOT NULL REFERENCES "ob-poc".deal_rate_cards(rate_card_id),

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

-- =============================================================================
-- 5. DEAL → SLA LINKS
-- =============================================================================

CREATE TABLE "ob-poc".deal_slas (
    sla_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
    contract_id     UUID REFERENCES accounting.service_contracts(contract_id),
    product_id      UUID REFERENCES "ob-poc".products(product_id),
    service_id      UUID REFERENCES "ob-poc".services(service_id),

    -- SLA details
    sla_name        VARCHAR(255) NOT NULL,
    sla_type        VARCHAR(50),                  -- AVAILABILITY | TURNAROUND | ACCURACY | REPORTING

    -- Metric
    metric_name     VARCHAR(100) NOT NULL,        -- e.g. "NAV Delivery Time"
    target_value    VARCHAR(100) NOT NULL,         -- e.g. "T+1 by 08:00 EST"
    measurement_unit VARCHAR(50),                  -- HOURS | PERCENT | COUNT

    -- Breach handling
    penalty_type    VARCHAR(50),                  -- FEE_REBATE | CREDIT | ESCALATION
    penalty_value   NUMERIC(18,2),

    effective_from  DATE NOT NULL,
    effective_to    DATE,

    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_deal_slas_deal ON "ob-poc".deal_slas(deal_id);

-- =============================================================================
-- 6. DEAL → DOCUMENT LINKS
-- =============================================================================

CREATE TABLE "ob-poc".deal_documents (
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
    document_id     UUID NOT NULL,                -- FK to existing document store

    document_type   VARCHAR(50) NOT NULL,
    -- CONTRACT | TERM_SHEET | SIDE_LETTER | NDA | RATE_SCHEDULE | SLA |
    -- PROPOSAL | RFP_RESPONSE | BOARD_APPROVAL | LEGAL_OPINION

    document_status VARCHAR(50) DEFAULT 'DRAFT',
    -- DRAFT | UNDER_REVIEW | SIGNED | EXECUTED | SUPERSEDED | ARCHIVED

    version         INT DEFAULT 1,

    created_at      TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (deal_id, document_id)
);

-- =============================================================================
-- 7. DEAL → UBO TAXONOMY LINK
-- =============================================================================

CREATE TABLE "ob-poc".deal_ubo_assessments (
    assessment_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
    entity_id       UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Link to KYC case that handles the UBO work
    kyc_case_id     UUID,                         -- FK to kyc_cases if exists

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

-- =============================================================================
-- 8. DEAL → ONBOARDING REQUESTS (the handoff)
-- =============================================================================

CREATE TABLE "ob-poc".deal_onboarding_requests (
    request_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),
    contract_id     UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),

    -- What's being onboarded
    cbu_id          UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    product_id      UUID NOT NULL REFERENCES "ob-poc".products(product_id),

    -- Request lifecycle
    request_status  VARCHAR(50) DEFAULT 'REQUESTED',
    -- REQUESTED | KYC_PENDING | KYC_CLEARED | IN_PROGRESS |
    -- COMPLETED | BLOCKED | CANCELLED

    -- KYC linkage
    requires_kyc    BOOLEAN DEFAULT true,
    kyc_case_id     UUID,                         -- FK to kyc_cases
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
    contract_id     UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),
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

-- =============================================================================
-- 10. FEE BILLING ACCOUNT TARGETS
-- =============================================================================
-- Links fee billing profiles to specific CBU resource instances (accounts,
-- funds, portfolios) that generate billable activity. This is the closed loop.

CREATE TABLE "ob-poc".fee_billing_account_targets (
    target_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id      UUID NOT NULL REFERENCES "ob-poc".fee_billing_profiles(profile_id),

    -- The operational resource generating activity
    cbu_resource_instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),

    -- Which rate card line applies to this resource's activity
    rate_card_line_id UUID REFERENCES "ob-poc".deal_rate_card_lines(line_id),

    -- Resource context (denormalised for billing queries)
    resource_type   VARCHAR(100),                 -- CUSTODY_ACCOUNT | FUND | PORTFOLIO
    resource_ref    VARCHAR(255),                  -- Account number / fund code

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
CREATE INDEX idx_fee_targets_active ON "ob-poc".fee_billing_account_targets(is_active) WHERE is_active = true;

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
    approved_by     VARCHAR(255),
    approved_at     TIMESTAMPTZ,

    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(profile_id, period_start, period_end)
);

CREATE INDEX idx_fee_periods_profile ON "ob-poc".fee_billing_periods(profile_id);
CREATE INDEX idx_fee_periods_status ON "ob-poc".fee_billing_periods(calc_status);

-- Line-level detail per billing period
CREATE TABLE "ob-poc".fee_billing_period_lines (
    period_line_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    period_id       UUID NOT NULL REFERENCES "ob-poc".fee_billing_periods(period_id),
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

-- =============================================================================
-- 12. DEAL ACTIVITY LOG
-- =============================================================================

CREATE TABLE "ob-poc".deal_events (
    event_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id         UUID NOT NULL REFERENCES "ob-poc".deals(deal_id),

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
```

### 1.3 Relationship Summary

```
deals (hub)
  ├── deal_participants          → entities (regional LEIs under the group)
  ├── deal_contracts             → accounting.service_contracts
  ├── deal_rate_cards            → products (negotiated pricing)
  │   └── deal_rate_card_lines   (individual fee lines with CHECK constraints)
  ├── deal_slas                  → products, services
  ├── deal_documents             → document store
  ├── deal_ubo_assessments       → entities, kyc_cases
  ├── deal_onboarding_requests   → cbus, products, kyc_cases
  ├── fee_billing_profiles       → contracts, rate_cards, cbus, products, entities(invoice target)
  │   └── fee_billing_account_targets → cbu_resource_instances, deal_rate_card_lines
  │       └── fee_billing_periods
  │           └── fee_billing_period_lines (the actual calculations)
  └── deal_events                (audit trail)
```

### 1.4 Post-Schema Validation

- [ ] Run `cargo sqlx prepare` to regenerate query metadata
- [ ] Fix any compile errors from new table references
- [ ] Verify all indexes and constraints created successfully

---

## Phase 2: DSL Verb Definitions

Add the following verb definitions to `config/verbs.yaml` under new `deal` and
`billing` domain sections.

### 2.1 DEAL Domain — Core Deal CRUD

```yaml
deal:
  verbs:
    deal.create:
      description: "Create a new deal record for a sales opportunity"
      inputs:
        - name: deal-name
          type: string
          required: true
        - name: primary-client-group-id
          type: uuid
          required: true
        - name: deal-reference
          type: string
          required: false
        - name: sales-owner
          type: string
          required: false
        - name: sales-team
          type: string
          required: false
        - name: estimated-revenue
          type: decimal
          required: false
        - name: currency-code
          type: string
          required: false
          default: "USD"
        - name: notes
          type: string
          required: false
      outputs:
        - name: deal-id
          type: uuid
      operations:
        - INSERT into deals
        - Record deal_events DEAL_CREATED
      assignable: true

    deal.get:
      description: "Retrieve deal record by ID"
      inputs:
        - name: deal-id
          type: uuid
          required: true
      outputs:
        - name: deal
          type: object

    deal.list:
      description: "List deals with optional filters"
      inputs:
        - name: client-group-id
          type: uuid
          required: false
        - name: status
          type: string
          required: false
        - name: sales-owner
          type: string
          required: false
      outputs:
        - name: deals
          type: array

    deal.search:
      description: "Search deals by name or reference"
      inputs:
        - name: query
          type: string
          required: true
      outputs:
        - name: deals
          type: array

    deal.update:
      description: "Update deal record fields"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: deal-name
          type: string
          required: false
        - name: sales-owner
          type: string
          required: false
        - name: estimated-revenue
          type: decimal
          required: false
        - name: notes
          type: string
          required: false
      operations:
        - UPDATE deals
        - Record deal_events

    deal.update-status:
      description: "Transition deal status with lifecycle validation"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: new-status
          type: string
          required: true
          enum: [PROSPECT, QUALIFYING, NEGOTIATING, CONTRACTED, ONBOARDING, ACTIVE, WINDING_DOWN, OFFBOARDED, CANCELLED]
      operations:
        - Validate status transition (see state machine below)
        - UPDATE deals.deal_status + appropriate timestamp
        - Record deal_events STATUS_CHANGED
      validation:
        state_machine:
          PROSPECT: [QUALIFYING, CANCELLED]
          QUALIFYING: [NEGOTIATING, CANCELLED]
          NEGOTIATING: [CONTRACTED, QUALIFYING, CANCELLED]
          CONTRACTED: [ONBOARDING, CANCELLED]
          ONBOARDING: [ACTIVE, CANCELLED]
          ACTIVE: [WINDING_DOWN]
          WINDING_DOWN: [OFFBOARDED]
          # OFFBOARDED and CANCELLED are terminal

    deal.cancel:
      description: "Cancel a deal (soft delete - sets status to CANCELLED)"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: reason
          type: string
          required: true
      operations:
        - Validate deal is not ACTIVE/WINDING_DOWN/OFFBOARDED
        - UPDATE deals SET deal_status = 'CANCELLED', closed_at = NOW()
        - Record deal_events STATUS_CHANGED
```

### 2.2 DEAL Domain — Participants

```yaml
    deal.add-participant:
      description: "Add a participating entity (regional LEI) to a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: entity-id
          type: uuid
          required: true
        - name: participant-role
          type: string
          required: false
          default: "CONTRACTING_PARTY"
          enum: [CONTRACTING_PARTY, GUARANTOR, INTRODUCER, INVESTMENT_MANAGER, FUND_ADMIN]
        - name: lei
          type: string
          required: false
        - name: is-primary
          type: boolean
          required: false
          default: false
      operations:
        - INSERT into deal_participants (UPSERT on unique constraint)
        - If is-primary=true, DB partial unique index enforces one primary per deal
        - Record deal_events

    deal.remove-participant:
      description: "Remove a participant from a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: entity-id
          type: uuid
          required: true
        - name: participant-role
          type: string
          required: false
      operations:
        - DELETE from deal_participants
        - Validate no orphaned contracts reference this entity
        - Record deal_events

    deal.list-participants:
      description: "List all participants in a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
      outputs:
        - name: participants
          type: array
```

### 2.3 DEAL Domain — Contracts

```yaml
    deal.add-contract:
      description: "Link a service contract to a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: contract-id
          type: uuid
          required: true
        - name: contract-role
          type: string
          required: false
          default: "PRIMARY"
          enum: [PRIMARY, ADDENDUM, SCHEDULE, SIDE_LETTER, NDA]
        - name: sequence-order
          type: integer
          required: false
      operations:
        - INSERT into deal_contracts
        - Record deal_events CONTRACT_ADDED

    deal.remove-contract:
      description: "Unlink a contract from a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: contract-id
          type: uuid
          required: true
      operations:
        - Validate no rate cards or billing profiles reference this contract
        - DELETE from deal_contracts
        - Record deal_events

    deal.list-contracts:
      description: "List all contracts linked to a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
      outputs:
        - name: contracts
          type: array
```

### 2.4 DEAL Domain — Rate Cards

```yaml
    deal.create-rate-card:
      description: "Create a negotiated rate card for a product within a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: contract-id
          type: uuid
          required: true
        - name: product-id
          type: uuid
          required: true
        - name: rate-card-name
          type: string
          required: false
        - name: effective-from
          type: date
          required: true
        - name: effective-to
          type: date
          required: false
      outputs:
        - name: rate-card-id
          type: uuid
      operations:
        - Validate contract-id is linked to this deal
        - INSERT into deal_rate_cards (status=DRAFT)
        - Record deal_events RATE_CARD_CREATED
      assignable: true

    deal.add-rate-card-line:
      description: "Add a fee line to a rate card"
      inputs:
        - name: rate-card-id
          type: uuid
          required: true
        - name: fee-type
          type: string
          required: true
        - name: fee-subtype
          type: string
          required: false
          default: "DEFAULT"
        - name: pricing-model
          type: string
          required: true
          enum: [BPS, FLAT, PER_TRANSACTION, TIERED, SPREAD, MINIMUM_FEE]
        - name: rate-value
          type: decimal
          required: false
        - name: minimum-fee
          type: decimal
          required: false
        - name: maximum-fee
          type: decimal
          required: false
        - name: currency-code
          type: string
          required: false
          default: "USD"
        - name: tier-brackets
          type: json
          required: false
        - name: fee-basis
          type: string
          required: false
          enum: [AUM, NAV, TRADE_COUNT, POSITION_COUNT]
        - name: description
          type: string
          required: false
      outputs:
        - name: line-id
          type: uuid
      operations:
        - Validate rate card is in DRAFT or PROPOSED status
        - INSERT into deal_rate_card_lines
        - DB CHECK constraints enforce pricing_model ↔ required columns
      assignable: true

    deal.update-rate-card-line:
      description: "Modify an existing rate card line"
      inputs:
        - name: line-id
          type: uuid
          required: true
        - name: rate-value
          type: decimal
          required: false
        - name: minimum-fee
          type: decimal
          required: false
        - name: maximum-fee
          type: decimal
          required: false
        - name: tier-brackets
          type: json
          required: false
      operations:
        - Validate parent rate card is still negotiable (not AGREED)
        - UPDATE deal_rate_card_lines

    deal.remove-rate-card-line:
      description: "Remove a fee line from a rate card"
      inputs:
        - name: line-id
          type: uuid
          required: true
      operations:
        - Validate no billing targets reference this line
        - DELETE from deal_rate_card_lines

    deal.list-rate-card-lines:
      description: "List all fee lines for a rate card"
      inputs:
        - name: rate-card-id
          type: uuid
          required: true
      outputs:
        - name: lines
          type: array
```

### 2.5 DEAL Domain — Rate Card Negotiation (Complex Ops)

```yaml
    deal.propose-rate-card:
      description: "Submit rate card for client review"
      inputs:
        - name: rate-card-id
          type: uuid
          required: true
      operations:
        - Validate at least one line exists
        - UPDATE deal_rate_cards SET status = 'PROPOSED', negotiation_round += 1
        - Record deal_events RATE_CARD_PROPOSED

    deal.counter-rate-card:
      description: "Client counter-offer - creates new version via clone"
      inputs:
        - name: rate-card-id
          type: uuid
          required: true
        - name: counter-lines
          type: array
          required: true
          description: "Array of {line_id, proposed_rate, proposed_minimum, proposed_maximum}"
      outputs:
        - name: new-rate-card-id
          type: uuid
      operations:
        - Clone rate card with status = COUNTER_OFFERED
        - Apply counter values to cloned lines
        - Set original.superseded_by = new card
        - UPDATE original SET status = 'SUPERSEDED'
        - Record deal_events

    deal.agree-rate-card:
      description: "Finalise rate card - both parties agree. Lines become immutable."
      inputs:
        - name: rate-card-id
          type: uuid
          required: true
      operations:
        - Validate rate card is PROPOSED or COUNTER_OFFERED
        - UPDATE deal_rate_cards SET status = 'AGREED'
        - Record deal_events RATE_CARD_AGREED
```

### 2.6 DEAL Domain — SLAs

```yaml
    deal.add-sla:
      description: "Add a service level agreement to a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: contract-id
          type: uuid
          required: false
        - name: product-id
          type: uuid
          required: false
        - name: service-id
          type: uuid
          required: false
        - name: sla-name
          type: string
          required: true
        - name: sla-type
          type: string
          required: false
          enum: [AVAILABILITY, TURNAROUND, ACCURACY, REPORTING]
        - name: metric-name
          type: string
          required: true
        - name: target-value
          type: string
          required: true
        - name: measurement-unit
          type: string
          required: false
        - name: penalty-type
          type: string
          required: false
          enum: [FEE_REBATE, CREDIT, ESCALATION]
        - name: penalty-value
          type: decimal
          required: false
        - name: effective-from
          type: date
          required: true
      outputs:
        - name: sla-id
          type: uuid
      assignable: true

    deal.remove-sla:
      description: "Remove an SLA from a deal"
      inputs:
        - name: sla-id
          type: uuid
          required: true

    deal.list-slas:
      description: "List SLAs for a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
      outputs:
        - name: slas
          type: array
```

### 2.7 DEAL Domain — Documents

```yaml
    deal.add-document:
      description: "Link a document to a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: document-id
          type: uuid
          required: true
        - name: document-type
          type: string
          required: true
          enum: [CONTRACT, TERM_SHEET, SIDE_LETTER, NDA, RATE_SCHEDULE, SLA, PROPOSAL, RFP_RESPONSE, BOARD_APPROVAL, LEGAL_OPINION]
        - name: document-status
          type: string
          required: false
          default: "DRAFT"

    deal.update-document-status:
      description: "Update document status (e.g. DRAFT → SIGNED → EXECUTED)"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: document-id
          type: uuid
          required: true
        - name: document-status
          type: string
          required: true
          enum: [DRAFT, UNDER_REVIEW, SIGNED, EXECUTED, SUPERSEDED, ARCHIVED]

    deal.list-documents:
      description: "List all documents linked to a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
      outputs:
        - name: documents
          type: array
```

### 2.8 DEAL Domain — UBO Assessments

```yaml
    deal.add-ubo-assessment:
      description: "Link an entity's UBO assessment to a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: entity-id
          type: uuid
          required: true
        - name: kyc-case-id
          type: uuid
          required: false
      outputs:
        - name: assessment-id
          type: uuid
      assignable: true

    deal.update-ubo-assessment:
      description: "Update UBO assessment status and risk rating"
      inputs:
        - name: assessment-id
          type: uuid
          required: true
        - name: assessment-status
          type: string
          required: false
          enum: [PENDING, IN_PROGRESS, COMPLETED, REQUIRES_EDD, BLOCKED]
        - name: risk-rating
          type: string
          required: false
          enum: [LOW, MEDIUM, HIGH, PROHIBITED]
      operations:
        - UPDATE deal_ubo_assessments
        - If PROHIBITED → check if deal should be blocked
        - Record deal_events
```

### 2.9 DEAL Domain — Onboarding Handoff (Complex Ops)

```yaml
    deal.request-onboarding:
      description: "Create onboarding request - handoff from Sales to Ops"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: contract-id
          type: uuid
          required: true
        - name: cbu-id
          type: uuid
          required: true
        - name: product-id
          type: uuid
          required: true
        - name: requires-kyc
          type: boolean
          required: false
          default: true
        - name: target-live-date
          type: date
          required: false
        - name: requested-by
          type: string
          required: false
        - name: notes
          type: string
          required: false
      outputs:
        - name: request-id
          type: uuid
      operations:
        - Validate deal is CONTRACTED or ONBOARDING
        - Validate contract-id is linked to this deal
        - Validate cbu-id belongs to the deal's client group
        - INSERT into deal_onboarding_requests
        - If deal_status = CONTRACTED → transition to ONBOARDING
        - Record deal_events ONBOARDING_REQUESTED
      assignable: true

    deal.request-onboarding-batch:
      description: "Batch onboarding request - multiple CBUs to multiple products"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: contract-id
          type: uuid
          required: true
        - name: requests
          type: array
          required: true
          description: "Array of {cbu-id, product-id, target-live-date}"
        - name: requires-kyc
          type: boolean
          required: false
          default: true
        - name: requested-by
          type: string
          required: false
      outputs:
        - name: request-ids
          type: array
      operations:
        - Validate all in single transaction
        - INSERT batch into deal_onboarding_requests
        - Transition deal status if needed
        - Record deal_events for each

    deal.update-onboarding-status:
      description: "Update onboarding request status"
      inputs:
        - name: request-id
          type: uuid
          required: true
        - name: request-status
          type: string
          required: true
          enum: [REQUESTED, KYC_PENDING, KYC_CLEARED, IN_PROGRESS, COMPLETED, BLOCKED, CANCELLED]
        - name: kyc-case-id
          type: uuid
          required: false
      operations:
        - UPDATE deal_onboarding_requests
        - If KYC_CLEARED → set kyc_cleared_at
        - If COMPLETED → set completed_at, check if all requests complete → deal.ACTIVE
        - Record deal_events

    deal.list-onboarding-requests:
      description: "List onboarding requests for a deal"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: status
          type: string
          required: false
      outputs:
        - name: requests
          type: array
```

### 2.10 DEAL Domain — Summary & Reporting

```yaml
    deal.summary:
      description: "Full deal summary - all linked entities, status, progress"
      inputs:
        - name: deal-id
          type: uuid
          required: true
      outputs:
        - name: deal
          type: object
          description: "Deal with nested participants, contracts, rate_cards, slas, onboarding_requests, billing_profiles, events"
      operations:
        - JOIN across all deal_* tables
        - Calculate progress metrics (% onboarding complete, % KYC cleared)
        - Return denormalised view

    deal.timeline:
      description: "Deal event timeline for audit trail"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: event-type
          type: string
          required: false
        - name: from-date
          type: timestamp
          required: false
        - name: to-date
          type: timestamp
          required: false
      outputs:
        - name: events
          type: array
```

### 2.11 BILLING Domain — Fee Billing Profile CRUD

```yaml
billing:
  verbs:
    billing.create-profile:
      description: "Create a fee billing profile - bridges commercial to operational"
      inputs:
        - name: deal-id
          type: uuid
          required: true
        - name: contract-id
          type: uuid
          required: true
        - name: rate-card-id
          type: uuid
          required: true
        - name: cbu-id
          type: uuid
          required: true
        - name: product-id
          type: uuid
          required: true
        - name: invoice-entity-id
          type: uuid
          required: true
        - name: profile-name
          type: string
          required: false
        - name: billing-frequency
          type: string
          required: false
          default: "MONTHLY"
          enum: [DAILY, WEEKLY, MONTHLY, QUARTERLY, ANNUALLY]
        - name: invoice-currency
          type: string
          required: false
          default: "USD"
        - name: payment-method
          type: string
          required: false
          enum: [ACH, WIRE, DEBIT_FROM_ACCOUNT]
        - name: payment-account-ref
          type: string
          required: false
        - name: effective-from
          type: date
          required: true
      outputs:
        - name: profile-id
          type: uuid
      operations:
        - Validate rate-card is AGREED
        - Validate contract, cbu, product all linked to this deal
        - INSERT into fee_billing_profiles (status=PENDING)
        - Record deal_events BILLING_PROFILE_CREATED
      assignable: true

    billing.activate-profile:
      description: "Activate a billing profile (CBU is live and generating activity)"
      inputs:
        - name: profile-id
          type: uuid
          required: true
      operations:
        - Validate at least one account target exists
        - UPDATE fee_billing_profiles SET status = 'ACTIVE'
        - Record deal_events BILLING_ACTIVATED

    billing.suspend-profile:
      description: "Suspend billing (e.g. dispute, investigation)"
      inputs:
        - name: profile-id
          type: uuid
          required: true
        - name: reason
          type: string
          required: true

    billing.close-profile:
      description: "Close billing profile (offboarding)"
      inputs:
        - name: profile-id
          type: uuid
          required: true
        - name: effective-to
          type: date
          required: true

    billing.get-profile:
      description: "Retrieve billing profile with account targets"
      inputs:
        - name: profile-id
          type: uuid
          required: true
      outputs:
        - name: profile
          type: object

    billing.list-profiles:
      description: "List billing profiles for a deal or CBU"
      inputs:
        - name: deal-id
          type: uuid
          required: false
        - name: cbu-id
          type: uuid
          required: false
        - name: status
          type: string
          required: false
      outputs:
        - name: profiles
          type: array
```

### 2.12 BILLING Domain — Account Targets (Closed Loop)

```yaml
    billing.add-account-target:
      description: "Link a CBU resource instance (account) to a billing profile"
      inputs:
        - name: profile-id
          type: uuid
          required: true
        - name: cbu-resource-instance-id
          type: uuid
          required: true
        - name: rate-card-line-id
          type: uuid
          required: false
        - name: resource-type
          type: string
          required: false
        - name: resource-ref
          type: string
          required: false
        - name: activity-type
          type: string
          required: false
          enum: [TRANSACTIONS, AUM, NAV, POSITIONS]
        - name: has-override
          type: boolean
          required: false
          default: false
        - name: override-rate
          type: decimal
          required: false
        - name: override-model
          type: string
          required: false
      outputs:
        - name: target-id
          type: uuid
      operations:
        - Validate resource instance belongs to the profile's CBU
        - INSERT into fee_billing_account_targets
      assignable: true

    billing.remove-account-target:
      description: "Soft-remove an account target from billing"
      inputs:
        - name: target-id
          type: uuid
          required: true
      operations:
        - Validate no open billing periods reference this target
        - UPDATE fee_billing_account_targets SET is_active = false

    billing.list-account-targets:
      description: "List account targets for a billing profile"
      inputs:
        - name: profile-id
          type: uuid
          required: true
      outputs:
        - name: targets
          type: array
```

### 2.13 BILLING Domain — Fee Calculation Runs

```yaml
    billing.create-period:
      description: "Create a billing period for calculation"
      inputs:
        - name: profile-id
          type: uuid
          required: true
        - name: period-start
          type: date
          required: true
        - name: period-end
          type: date
          required: true
      outputs:
        - name: period-id
          type: uuid
      operations:
        - Validate no overlapping period exists
        - Validate profile is ACTIVE
        - INSERT into fee_billing_periods (calc_status=PENDING)
      assignable: true

    billing.calculate-period:
      description: "Run fee calculation for a billing period"
      inputs:
        - name: period-id
          type: uuid
          required: true
      operations:
        - For each active account target in this profile:
          - Query activity volume from resource instance for period window
          - Look up applicable rate card line
          - Apply pricing model (BPS, tiered, flat, etc.)
          - INSERT fee_billing_period_lines with calculation detail
        - SUM all lines → UPDATE fee_billing_periods totals
        - SET calc_status = 'CALCULATED'
        - Record deal_events

    billing.review-period:
      description: "Mark billing period as reviewed, optionally apply adjustments"
      inputs:
        - name: period-id
          type: uuid
          required: true
        - name: reviewed-by
          type: string
          required: true
        - name: adjustments
          type: array
          required: false
          description: "Array of {period_line_id, adjustment_amount, reason}"
      operations:
        - Apply any adjustments to period lines
        - Recalculate net totals
        - SET calc_status = 'REVIEWED'

    billing.approve-period:
      description: "Approve billing period for invoicing"
      inputs:
        - name: period-id
          type: uuid
          required: true
        - name: approved-by
          type: string
          required: true
      operations:
        - SET calc_status = 'APPROVED', approved_at = NOW()

    billing.generate-invoice:
      description: "Generate invoice from approved billing period"
      inputs:
        - name: period-id
          type: uuid
          required: true
      outputs:
        - name: invoice-id
          type: uuid
      operations:
        - Validate calc_status = 'APPROVED'
        - Create invoice record (may need new table or link to existing)
        - SET calc_status = 'INVOICED', invoice_id, invoiced_at
        - Record deal_events INVOICE_GENERATED

    billing.dispute-period:
      description: "Client disputes a billing period"
      inputs:
        - name: period-id
          type: uuid
          required: true
        - name: dispute-reason
          type: string
          required: true
        - name: disputed-lines
          type: array
          required: false
          description: "Array of period_line_ids being disputed"
      operations:
        - SET calc_status = 'DISPUTED'
        - Record dispute details
        - Record deal_events

    billing.period-summary:
      description: "Get billing period with line-level detail"
      inputs:
        - name: period-id
          type: uuid
          required: true
      outputs:
        - name: period
          type: object

    billing.revenue-summary:
      description: "Revenue summary across deals, periods, products"
      inputs:
        - name: deal-id
          type: uuid
          required: false
        - name: cbu-id
          type: uuid
          required: false
        - name: from-date
          type: date
          required: false
        - name: to-date
          type: date
          required: false
      outputs:
        - name: summary
          type: object
          description: "Aggregated revenue by product, period, entity"
```

---

## Phase 3: Rust Implementation

### 3.1 Struct Definitions

- [ ] Create `src/models/deal.rs` with structs for all 14 tables
- [ ] Derive `sqlx::FromRow`, `Serialize`, `Deserialize` on all structs
- [ ] Add to `src/models/mod.rs`

### 3.2 Custom Ops Implementation

- [ ] Create `src/dsl_v2/custom_ops/deal_ops.rs`
- [ ] Implement handlers for each `deal.*` verb following existing pattern
- [ ] Create `src/dsl_v2/custom_ops/billing_ops.rs`
- [ ] Implement handlers for each `billing.*` verb
- [ ] Register in `src/dsl_v2/custom_ops/mod.rs`

### 3.3 State Machines

Implement as separate validator functions, not inline:

- [ ] Deal status transitions (see 2.1 state_machine)
- [ ] Rate card status: DRAFT → PROPOSED → COUNTER_OFFERED → AGREED (or SUPERSEDED/CANCELLED)
- [ ] Onboarding request status: REQUESTED → KYC_PENDING → KYC_CLEARED → IN_PROGRESS → COMPLETED (or BLOCKED/CANCELLED)
- [ ] Billing period status: PENDING → CALCULATING → CALCULATED → REVIEWED → APPROVED → INVOICED (with DISPUTED branch from CALCULATED/REVIEWED/APPROVED)

### 3.4 Event Recording

- [ ] Implement `record_deal_event()` helper that all deal/billing ops call
- [ ] Ensure every mutating operation records to `deal_events`

---

## Phase 4: Integration Testing

### 4.1 End-to-End Scenarios

- [ ] **Happy path**: Create deal → add participants → add contract → create rate card with lines → agree rate card → request onboarding → create billing profile → add account targets → activate billing → create period → calculate → review → approve → generate invoice
- [ ] **Negotiation round-trip**: Create rate card → propose → counter → counter again → agree
- [ ] **Batch onboarding**: Single deal spawning 5 CBU×product onboarding requests
- [ ] **Closed loop validation**: Verify the chain from deal_rate_card_line through billing_account_target through period_line uses consistent rates
- [ ] **Status transition failures**: Attempt invalid transitions, verify rejections
- [ ] **CHECK constraint validation**: Attempt BPS line without rate_value, TIERED without brackets — verify DB rejects

### 4.2 Referential Integrity

- [ ] Cannot delete deal with active contracts
- [ ] Cannot modify AGREED rate card lines
- [ ] Cannot create billing profile with non-AGREED rate card
- [ ] Billing account targets must reference correct CBU's resource instances (not another CBU's)
- [ ] Cannot create duplicate onboarding request (deal + contract + cbu + product)
- [ ] Cannot create duplicate billing profile (cbu + product + rate_card)
- [ ] Cannot set two participants as primary on same deal

---

## Phase 5: DSL Script Examples

Create example scripts in `examples/` exercising the full lifecycle:

```clojure
;; Example: Blackrock Prime Brokerage Deal
(deal.create
  :deal-name "Blackrock Prime Brokerage 2026"
  :primary-client-group-id @blackrock-group
  :sales-owner "Jane Smith"
  :estimated-revenue 2500000.00
  :as @br-deal)

(deal.add-participant
  :deal-id @br-deal
  :entity-id @blackrock-uk
  :participant-role "CONTRACTING_PARTY"
  :lei "549300LKFJ4HHDQ1C531"
  :is-primary true)

(deal.add-participant
  :deal-id @br-deal
  :entity-id @blackrock-lux
  :participant-role "CONTRACTING_PARTY"
  :lei "549300XYZ...")

(deal.add-contract
  :deal-id @br-deal
  :contract-id @br-custody-contract
  :contract-role "PRIMARY")

(deal.create-rate-card
  :deal-id @br-deal
  :contract-id @br-custody-contract
  :product-id @custody-product
  :rate-card-name "Blackrock Custody Fees 2026"
  :effective-from "2026-01-01"
  :as @br-rate-card)

(deal.add-rate-card-line
  :rate-card-id @br-rate-card
  :fee-type "CUSTODY"
  :pricing-model "BPS"
  :rate-value 1.5
  :fee-basis "AUM"
  :minimum-fee 50000.00
  :currency-code "USD")

(deal.add-rate-card-line
  :rate-card-id @br-rate-card
  :fee-type "SETTLEMENT"
  :pricing-model "PER_TRANSACTION"
  :rate-value 15.00
  :currency-code "USD")

(deal.propose-rate-card :rate-card-id @br-rate-card)
(deal.agree-rate-card :rate-card-id @br-rate-card)

(deal.update-status :deal-id @br-deal :new-status "CONTRACTED")

;; Handoff to Ops
(deal.request-onboarding-batch
  :deal-id @br-deal
  :contract-id @br-custody-contract
  :requires-kyc true
  :requested-by "Jane Smith"
  :requests [
    {:cbu-id @br-uk-fund-1 :product-id @custody-product :target-live-date "2026-03-01"}
    {:cbu-id @br-uk-fund-2 :product-id @custody-product :target-live-date "2026-03-01"}
    {:cbu-id @br-lux-sicav :product-id @custody-product :target-live-date "2026-04-01"}
  ])

;; After onboarding completes - set up billing
(billing.create-profile
  :deal-id @br-deal
  :contract-id @br-custody-contract
  :rate-card-id @br-rate-card
  :cbu-id @br-uk-fund-1
  :product-id @custody-product
  :invoice-entity-id @blackrock-uk
  :billing-frequency "MONTHLY"
  :effective-from "2026-03-01"
  :as @br-billing)

(billing.add-account-target
  :profile-id @br-billing
  :cbu-resource-instance-id @br-fund-1-custody-acct
  :rate-card-line-id @custody-bps-line
  :resource-type "CUSTODY_ACCOUNT"
  :resource-ref "CUST-BR-001"
  :activity-type "AUM")

(billing.activate-profile :profile-id @br-billing)

;; Monthly billing run
(billing.create-period
  :profile-id @br-billing
  :period-start "2026-03-01"
  :period-end "2026-03-31"
  :as @mar-period)

(billing.calculate-period :period-id @mar-period)
(billing.review-period :period-id @mar-period :reviewed-by "Ops Team")
(billing.approve-period :period-id @mar-period :approved-by "Finance Lead")
(billing.generate-invoice :period-id @mar-period)
```

---

## Notes for Claude Code

1. **Follow existing patterns** — look at how CBU CRUD and KYC verbs are implemented in `custom_ops/mod.rs` for the handler pattern
2. **Generic CRUD executor** — deal CRUD ops should use the existing generic executor where possible
3. **Event recording** — every mutating operation must write to `deal_events`. Use correct event names: RATE_CARD_CREATED (not PROPOSED) on create, BILLING_PROFILE_CREATED (not ACTIVATED) on create
4. **State machine validation** — implement as a separate validator function, not inline
5. **Rate card immutability** — once status = AGREED, lines are frozen. Enforce at handler level (DB CHECK constraints handle structural validity, Rust handlers enforce workflow)
6. **Closed loop validation** — when creating billing account targets, validate the resource instance belongs to the billing profile's CBU (not just any CBU)
7. **Batch operations** — `deal.request-onboarding-batch` must be transactional (all-or-nothing)
8. **Fee calculation** — `billing.calculate-period` is the most complex op. Iterate account targets, query activity data, apply pricing model per line. Start with BPS and FLAT, add TIERED later
9. **Schema doc** — after applying DDL, update `database_schema.md` and `claude.md` with the new tables
10. **DB constraints do the structural work** — CHECK constraints on deal_rate_card_lines, partial unique on deal_participants, UNIQUE on onboarding_requests and fee_billing_profiles. Don't duplicate these in handler code — let them fail at the DB level and surface meaningful errors
