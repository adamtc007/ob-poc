-- Fund Structure Ontology Migration
-- Creates tables for umbrella funds, sub-funds, share classes, control relationships, and delegations

-- =============================================================================
-- 1. ENTITY EXTENSION TABLES
-- =============================================================================

-- Fund entities (umbrella, subfund, standalone, master, feeder)
CREATE TABLE IF NOT EXISTS "ob-poc".entity_funds (
  entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

  -- Fund identifiers
  lei VARCHAR(20),
  isin_base VARCHAR(12),
  registration_number VARCHAR(100),

  -- Fund classification
  fund_structure_type TEXT,           -- 'SICAV', 'ICAV', 'OEIC', 'VCC', 'UNIT_TRUST', 'FCP'
  fund_type TEXT,                     -- 'UCITS', 'AIF', 'HEDGE_FUND', 'PRIVATE_EQUITY', etc.
  regulatory_status TEXT,             -- 'UCITS', 'AIF', 'RAIF', 'PART_II', 'UNREGULATED'

  -- Hierarchy
  parent_fund_id UUID REFERENCES "ob-poc".entities(entity_id),  -- umbrella for subfund, master for feeder
  master_fund_id UUID REFERENCES "ob-poc".entities(entity_id),  -- master fund for feeders

  -- Regulatory
  jurisdiction VARCHAR(10),
  regulator VARCHAR(100),
  authorization_date DATE,

  -- Investment profile
  investment_objective TEXT,
  base_currency VARCHAR(3),

  -- Dates
  incorporation_date DATE,
  launch_date DATE,
  financial_year_end VARCHAR(5),      -- 'MM-DD' format

  -- Investor targeting (for feeders)
  investor_type TEXT,                 -- 'US_TAXABLE', 'US_TAX_EXEMPT', 'NON_US', etc.

  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_entity_funds_parent ON "ob-poc".entity_funds(parent_fund_id);
CREATE INDEX IF NOT EXISTS idx_entity_funds_master ON "ob-poc".entity_funds(master_fund_id);
CREATE INDEX IF NOT EXISTS idx_entity_funds_jurisdiction ON "ob-poc".entity_funds(jurisdiction);

-- Share class entities
CREATE TABLE IF NOT EXISTS "ob-poc".entity_share_classes (
  entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

  -- Link to parent sub-fund
  parent_fund_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

  -- Share class identifiers
  isin VARCHAR(12) UNIQUE,
  share_class_code VARCHAR(20),

  -- Classification
  share_class_type TEXT NOT NULL,     -- 'INSTITUTIONAL', 'RETAIL', 'SEED', 'FOUNDER', 'CLEAN'
  distribution_type TEXT NOT NULL,    -- 'ACC', 'DIST', 'FLEX'
  currency VARCHAR(3) NOT NULL,
  is_hedged BOOLEAN DEFAULT FALSE,

  -- Fees
  management_fee_bps INTEGER,
  performance_fee_pct DECIMAL(5,2),

  -- Investment
  minimum_investment DECIMAL(18,2),

  -- Status dates
  launch_date DATE,
  soft_close_date DATE,
  hard_close_date DATE,

  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_share_classes_parent ON "ob-poc".entity_share_classes(parent_fund_id);

-- Management company entities
CREATE TABLE IF NOT EXISTS "ob-poc".entity_manco (
  entity_id UUID PRIMARY KEY REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,

  -- Identifiers
  lei VARCHAR(20),
  regulatory_reference VARCHAR(100),

  -- Authorization
  manco_type TEXT NOT NULL,           -- 'UCITS_MANCO', 'AIFM', 'DUAL_AUTHORIZED'
  authorized_jurisdiction VARCHAR(10) NOT NULL,
  regulator VARCHAR(100),
  authorization_date DATE,

  -- Capabilities
  can_manage_ucits BOOLEAN DEFAULT FALSE,
  can_manage_aif BOOLEAN DEFAULT FALSE,
  passported_jurisdictions TEXT[],

  -- Capital
  regulatory_capital_eur DECIMAL(15,2),

  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- =============================================================================
-- 2. FUND STRUCTURE RELATIONSHIPS
-- =============================================================================

-- Structural containment (umbrella→subfund, subfund→shareclass, master→feeder)
CREATE TABLE IF NOT EXISTS "ob-poc".fund_structure (
  structure_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  parent_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

  relationship_type TEXT NOT NULL DEFAULT 'CONTAINS',
  -- 'CONTAINS' (umbrella→subfund, subfund→shareclass)
  -- 'MASTER_FEEDER' (master→feeder)

  effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
  effective_to DATE,

  created_at TIMESTAMPTZ DEFAULT NOW(),
  created_by VARCHAR(100),

  UNIQUE(parent_entity_id, child_entity_id, relationship_type, effective_from)
);

CREATE INDEX IF NOT EXISTS idx_fund_structure_parent ON "ob-poc".fund_structure(parent_entity_id);
CREATE INDEX IF NOT EXISTS idx_fund_structure_child ON "ob-poc".fund_structure(child_entity_id);

-- Fund-to-fund investments (FoF)
CREATE TABLE IF NOT EXISTS "ob-poc".fund_investments (
  investment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  investor_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  investee_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

  -- Allocation
  percentage_of_investor_nav DECIMAL(5,2) NOT NULL,
  percentage_of_investee_aum DECIMAL(5,2),

  investment_type TEXT DEFAULT 'DIRECT',  -- 'DIRECT', 'VIA_SHARE_CLASS', 'SIDE_POCKET'

  -- Dates
  investment_date DATE,
  redemption_date DATE,
  valuation_date DATE,

  created_at TIMESTAMPTZ DEFAULT NOW(),

  UNIQUE(investor_entity_id, investee_entity_id, investment_date)
);

CREATE INDEX IF NOT EXISTS idx_fund_investments_investor ON "ob-poc".fund_investments(investor_entity_id);
CREATE INDEX IF NOT EXISTS idx_fund_investments_investee ON "ob-poc".fund_investments(investee_entity_id);

-- =============================================================================
-- 3. CONTROL RELATIONSHIPS (separate from ownership)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".control_relationships (
  control_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  controller_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  controlled_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

  control_type TEXT NOT NULL,
  -- 'VOTING_RIGHTS' - voting control > ownership
  -- 'BOARD_APPOINTMENT' - right to appoint directors
  -- 'VETO_POWER' - negative control
  -- 'MANAGEMENT_CONTROL' - operational control
  -- 'RESERVED_MATTERS' - approval rights on specific decisions

  control_percentage DECIMAL(5,2),
  control_description TEXT,

  -- Evidence
  evidence_doc_id UUID REFERENCES "ob-poc".document_catalog(doc_id),

  -- Dates
  effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
  effective_to DATE,
  is_active BOOLEAN DEFAULT TRUE,

  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW(),

  UNIQUE(controller_entity_id, controlled_entity_id, control_type, effective_from)
);

CREATE INDEX IF NOT EXISTS idx_control_controller ON "ob-poc".control_relationships(controller_entity_id);
CREATE INDEX IF NOT EXISTS idx_control_controlled ON "ob-poc".control_relationships(controlled_entity_id);

-- =============================================================================
-- 4. DELEGATION RELATIONSHIPS
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".delegation_relationships (
  delegation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  delegator_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  delegate_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

  delegation_scope TEXT NOT NULL,
  -- 'INVESTMENT_MANAGEMENT'
  -- 'RISK_MANAGEMENT'
  -- 'PORTFOLIO_ADMINISTRATION'
  -- 'DISTRIBUTION'
  -- 'TRANSFER_AGENCY'

  delegation_description TEXT,

  -- Which fund/CBU this applies to (optional - may be firm-wide)
  applies_to_cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),

  -- Regulatory
  regulatory_notification_date DATE,
  regulatory_approval_required BOOLEAN DEFAULT FALSE,
  regulatory_approval_date DATE,

  -- Contract
  contract_doc_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  effective_from DATE NOT NULL,
  effective_to DATE,

  created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_delegation_delegator ON "ob-poc".delegation_relationships(delegator_entity_id);
CREATE INDEX IF NOT EXISTS idx_delegation_delegate ON "ob-poc".delegation_relationships(delegate_entity_id);
CREATE INDEX IF NOT EXISTS idx_delegation_cbu ON "ob-poc".delegation_relationships(applies_to_cbu_id);

-- =============================================================================
-- 5. SEED DATA - Entity Types
-- =============================================================================

INSERT INTO "ob-poc".entity_types (type_code, name, entity_category, table_name, description) VALUES
  ('fund_umbrella', 'Umbrella Fund', 'SHELL', 'entity_funds', 'SICAV, ICAV, OEIC - single legal entity with multiple compartments'),
  ('fund_subfund', 'Sub-fund/Compartment', 'SHELL', 'entity_funds', 'Segregated compartment within umbrella fund'),
  ('fund_share_class', 'Share Class', 'SHELL', 'entity_share_classes', 'Share class within a sub-fund'),
  ('fund_standalone', 'Standalone Fund', 'SHELL', 'entity_funds', 'Non-umbrella single fund'),
  ('fund_master', 'Master Fund', 'SHELL', 'entity_funds', 'Master in master-feeder structure'),
  ('fund_feeder', 'Feeder Fund', 'SHELL', 'entity_funds', 'Feeder in master-feeder structure'),
  ('management_company', 'Management Company', 'SHELL', 'entity_manco', 'ManCo or AIFM - regulated fund manager'),
  ('depositary', 'Depositary', 'SHELL', 'entity_limited_companies', 'Depositary/custodian with fiduciary duty'),
  ('fund_administrator', 'Fund Administrator', 'SHELL', 'entity_limited_companies', 'NAV calculation, TA services')
ON CONFLICT (type_code) DO NOTHING;

-- =============================================================================
-- 6. SEED DATA - Fund-Specific Roles
-- =============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category) VALUES
  -- Fund governance
  ('MANAGEMENT_COMPANY', 'Appointed ManCo/AIFM', 'FUND_GOVERNANCE'),
  ('DEPOSITARY', 'Depositary bank with fiduciary duties', 'FUND_GOVERNANCE'),
  ('AUDITOR', 'Statutory auditor', 'FUND_GOVERNANCE'),
  ('LEGAL_COUNSEL', 'Legal advisor to the fund', 'FUND_GOVERNANCE'),

  -- Fund operations
  ('FUND_ADMINISTRATOR', 'NAV calculation, accounting', 'FUND_OPERATIONS'),
  ('TRANSFER_AGENT', 'Shareholder servicing, registry', 'FUND_OPERATIONS'),
  ('PAYING_AGENT', 'Distribution payments', 'FUND_OPERATIONS'),
  ('REGISTRAR', 'Share register maintenance', 'FUND_OPERATIONS'),

  -- Investment
  ('SUB_ADVISOR', 'Delegated investment management', 'INVESTMENT'),
  ('INVESTMENT_ADVISOR', 'Non-discretionary advice', 'INVESTMENT'),

  -- Distribution
  ('GLOBAL_DISTRIBUTOR', 'Primary distribution responsibility', 'DISTRIBUTION'),
  ('LOCAL_DISTRIBUTOR', 'Jurisdiction-specific distribution', 'DISTRIBUTION'),
  ('PLACEMENT_AGENT', 'Private placement', 'DISTRIBUTION'),

  -- Lending/Collateral
  ('PRIME_BROKER', 'Prime brokerage services', 'FINANCING'),
  ('SECURITIES_LENDING_AGENT', 'Securities lending program', 'FINANCING'),
  ('COLLATERAL_MANAGER', 'Collateral management', 'FINANCING')
ON CONFLICT (name) DO NOTHING;
