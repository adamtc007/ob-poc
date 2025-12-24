# KYC Case Builder - Phase 1: Reference Data & Schema

## Overview

This document consolidates Phase 1 implementation for the KYC Case Builder.
It supersedes the fragmented TODO files and provides Claude with a complete,
self-contained implementation guide.

**Goal:** Establish reference data foundation for KYC scoping decisions.

**Key Insight:** Entities can have MULTIPLE regulatory registrations 
(dual-regulation, passporting, multi-jurisdiction).

---

## Deliverables Checklist

### Config Files
- [ ] `config/ontology/role_types.yaml`
- [ ] `config/ontology/regulators.yaml`
- [ ] `config/ontology/product_kyc_config.yaml`
- [ ] `config/ontology/kyc_scope_templates.yaml`
- [ ] `config/verbs/admin/role-types.yaml`
- [ ] `config/verbs/admin/regulators.yaml`
- [ ] `config/verbs/regulatory.yaml`

### Migrations
- [ ] `V0XX__kyc_reference_data.sql` (all reference tables + seeds)
- [ ] `V0XX__entity_regulatory_registrations.sql` (multi-regulator support)

### Rust
- [ ] Load YAML configs on startup
- [ ] Register admin verbs
- [ ] Register regulatory verbs
- [ ] Implement `RegulatoryStatusCheckOp` plugin

---

## 1. Database Schema

### 1.1 Reference Data Schema

**File:** `migrations/V0XX__kyc_reference_data.sql`

```sql
-- ═══════════════════════════════════════════════════════════════════════════
-- Schema for KYC reference data
-- ═══════════════════════════════════════════════════════════════════════════

CREATE SCHEMA IF NOT EXISTS ob_ref;

-- ───────────────────────────────────────────────────────────────────────────
-- Regulatory Tiers: How much can we rely on this regulator?
-- ───────────────────────────────────────────────────────────────────────────

CREATE TABLE ob_ref.regulatory_tiers (
    tier_code VARCHAR(50) PRIMARY KEY,
    description VARCHAR(255) NOT NULL,
    allows_simplified_dd BOOLEAN DEFAULT FALSE,
    requires_enhanced_screening BOOLEAN DEFAULT FALSE
);

INSERT INTO ob_ref.regulatory_tiers (tier_code, description, allows_simplified_dd, requires_enhanced_screening) VALUES
('EQUIVALENT', 'Full reliance permitted - equivalent jurisdiction', TRUE, FALSE),
('ACCEPTABLE', 'Partial reliance - enhanced screening required', TRUE, TRUE),
('NONE', 'No reliance - full KYC required', FALSE, FALSE);

-- ───────────────────────────────────────────────────────────────────────────
-- Regulators: Known financial regulators we recognize
-- ───────────────────────────────────────────────────────────────────────────

CREATE TABLE ob_ref.regulators (
    regulator_code VARCHAR(50) PRIMARY KEY,
    regulator_name VARCHAR(255) NOT NULL,
    jurisdiction VARCHAR(2) NOT NULL,
    regulatory_tier VARCHAR(50) NOT NULL REFERENCES ob_ref.regulatory_tiers(tier_code),
    regulator_type VARCHAR(50) DEFAULT 'GOVERNMENT',
    registry_url VARCHAR(500),
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_regulators_jurisdiction ON ob_ref.regulators(jurisdiction);
CREATE INDEX idx_regulators_tier ON ob_ref.regulators(regulatory_tier);

-- Seed: Major regulators (EQUIVALENT tier)
INSERT INTO ob_ref.regulators (regulator_code, regulator_name, jurisdiction, regulatory_tier, regulator_type, registry_url) VALUES
-- UK
('FCA', 'Financial Conduct Authority', 'GB', 'EQUIVALENT', 'GOVERNMENT', 'https://register.fca.org.uk/s/'),
('PRA', 'Prudential Regulation Authority', 'GB', 'EQUIVALENT', 'GOVERNMENT', NULL),
-- US
('SEC', 'Securities and Exchange Commission', 'US', 'EQUIVALENT', 'GOVERNMENT', 'https://www.sec.gov/cgi-bin/browse-edgar'),
('FINRA', 'Financial Industry Regulatory Authority', 'US', 'EQUIVALENT', 'SRO', 'https://brokercheck.finra.org/'),
('CFTC', 'Commodity Futures Trading Commission', 'US', 'EQUIVALENT', 'GOVERNMENT', NULL),
('OCC', 'Office of the Comptroller of the Currency', 'US', 'EQUIVALENT', 'GOVERNMENT', NULL),
('FDIC', 'Federal Deposit Insurance Corporation', 'US', 'EQUIVALENT', 'GOVERNMENT', NULL),
-- EU
('CSSF', 'Commission de Surveillance du Secteur Financier', 'LU', 'EQUIVALENT', 'GOVERNMENT', 'https://www.cssf.lu/en/entity-search/'),
('CBI', 'Central Bank of Ireland', 'IE', 'EQUIVALENT', 'GOVERNMENT', 'http://registers.centralbank.ie/'),
('BaFin', 'Bundesanstalt für Finanzdienstleistungsaufsicht', 'DE', 'EQUIVALENT', 'GOVERNMENT', 'https://portal.mvp.bafin.de/database/InstInfo/'),
('AMF', 'Autorité des marchés financiers', 'FR', 'EQUIVALENT', 'GOVERNMENT', 'https://www.amf-france.org/en/professionals'),
('AFM', 'Autoriteit Financiële Markten', 'NL', 'EQUIVALENT', 'GOVERNMENT', 'https://www.afm.nl/en/sector/registers'),
('CONSOB', 'Commissione Nazionale per le Società e la Borsa', 'IT', 'EQUIVALENT', 'GOVERNMENT', NULL),
('CNMV', 'Comisión Nacional del Mercado de Valores', 'ES', 'EQUIVALENT', 'GOVERNMENT', NULL),
-- Switzerland
('FINMA', 'Swiss Financial Market Supervisory Authority', 'CH', 'EQUIVALENT', 'GOVERNMENT', 'https://www.finma.ch/en/authorisation/'),
-- Asia Pacific
('MAS', 'Monetary Authority of Singapore', 'SG', 'EQUIVALENT', 'GOVERNMENT', 'https://eservices.mas.gov.sg/fid'),
('SFC', 'Securities and Futures Commission', 'HK', 'EQUIVALENT', 'GOVERNMENT', 'https://www.sfc.hk/publicregWeb/'),
('ASIC', 'Australian Securities and Investments Commission', 'AU', 'EQUIVALENT', 'GOVERNMENT', 'https://connectonline.asic.gov.au/'),
('JFSA', 'Japan Financial Services Agency', 'JP', 'EQUIVALENT', 'GOVERNMENT', NULL),
-- Other
('CIMA', 'Cayman Islands Monetary Authority', 'KY', 'EQUIVALENT', 'GOVERNMENT', NULL),
('BMA', 'Bermuda Monetary Authority', 'BM', 'EQUIVALENT', 'GOVERNMENT', NULL),
('GFSC', 'Guernsey Financial Services Commission', 'GG', 'EQUIVALENT', 'GOVERNMENT', NULL),
('JFSC', 'Jersey Financial Services Commission', 'JE', 'EQUIVALENT', 'GOVERNMENT', NULL);

-- ───────────────────────────────────────────────────────────────────────────
-- Registration Types: How entity relates to regulator
-- ───────────────────────────────────────────────────────────────────────────

CREATE TABLE ob_ref.registration_types (
    registration_type VARCHAR(50) PRIMARY KEY,
    description VARCHAR(255) NOT NULL,
    is_primary BOOLEAN DEFAULT FALSE,
    allows_reliance BOOLEAN DEFAULT TRUE
);

INSERT INTO ob_ref.registration_types (registration_type, description, is_primary, allows_reliance) VALUES
('PRIMARY', 'Primary/home state regulator', TRUE, TRUE),
('DUAL_CONDUCT', 'Dual regulation - conduct authority', FALSE, TRUE),
('DUAL_PRUDENTIAL', 'Dual regulation - prudential authority', FALSE, TRUE),
('PASSPORTED', 'EU/EEA passported registration', FALSE, TRUE),
('BRANCH', 'Branch registration in jurisdiction', FALSE, TRUE),
('SUBSIDIARY', 'Separate subsidiary registration', FALSE, TRUE),
('ADDITIONAL', 'Additional registration (same jurisdiction)', FALSE, TRUE),
('STATE', 'State/provincial registration', FALSE, FALSE),
('SRO', 'Self-regulatory organization membership', FALSE, TRUE);

-- ───────────────────────────────────────────────────────────────────────────
-- Role Types: Entity roles and their KYC implications
-- ───────────────────────────────────────────────────────────────────────────

CREATE TABLE ob_ref.role_types (
    role_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    -- KYC triggers
    triggers_full_kyc BOOLEAN DEFAULT FALSE,
    triggers_screening BOOLEAN DEFAULT FALSE,
    triggers_id_verification BOOLEAN DEFAULT FALSE,
    -- Regulatory check behavior
    check_regulatory_status BOOLEAN DEFAULT FALSE,
    if_regulated_obligation VARCHAR(50),
    -- Cascade behavior
    cascade_to_entity_ubos BOOLEAN DEFAULT FALSE,
    -- Metadata
    category VARCHAR(50),
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_role_types_code ON ob_ref.role_types(code);
CREATE INDEX idx_role_types_category ON ob_ref.role_types(category);

-- Seed: Role types with KYC implications
INSERT INTO ob_ref.role_types (code, name, category, triggers_full_kyc, triggers_screening, triggers_id_verification, check_regulatory_status, if_regulated_obligation, cascade_to_entity_ubos, description) VALUES
-- Always full KYC
('ACCOUNT_HOLDER', 'Account Holder', 'PRINCIPAL', TRUE, TRUE, TRUE, FALSE, NULL, TRUE, 'The primary client entity'),
('UBO', 'Ultimate Beneficial Owner', 'OWNERSHIP', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, 'Person with >25% ownership or control'),
('CONTROLLER', 'Controller', 'OWNERSHIP', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, 'Person controlling entity by other means'),
('JOINT_HOLDER', 'Joint Account Holder', 'PRINCIPAL', TRUE, TRUE, TRUE, FALSE, NULL, FALSE, 'Joint account holder'),

-- Check regulatory status
('MANCO', 'Management Company', 'DELEGATE', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Fund management company'),
('INVESTMENT_MGR', 'Investment Manager', 'DELEGATE', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Discretionary investment manager'),
('INVESTOR', 'Fund Investor', 'INVESTOR', TRUE, TRUE, TRUE, TRUE, 'SIMPLIFIED', FALSE, 'Investor in a fund'),
('PARENT_COMPANY', 'Parent Company', 'OWNERSHIP', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Corporate parent'),
('PRIME_BROKER', 'Prime Broker', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Prime brokerage provider'),

-- Screening + ID only
('DIRECTOR', 'Director', 'GOVERNANCE', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, 'Board director'),
('SIGNATORY', 'Authorized Signatory', 'AUTHORITY', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, 'Person authorized to sign'),
('AUTHORIZED_PERSON', 'Authorized Person', 'AUTHORITY', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, 'Person with power of attorney'),
('CONDUCTING_OFFICER', 'Conducting Officer', 'GOVERNANCE', FALSE, TRUE, TRUE, FALSE, NULL, FALSE, 'Luxembourg conducting officer'),

-- Record only (regulated delegates)
('DELEGATE', 'Delegate/Service Provider', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Generic service provider'),
('CUSTODIAN', 'Custodian', 'DELEGATE', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Assets custodian'),
('DEPOSITARY', 'Depositary', 'DELEGATE', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Fund depositary'),
('ADMINISTRATOR', 'Fund Administrator', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Fund administrator'),
('TRANSFER_AGENT', 'Transfer Agent', 'DELEGATE', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Transfer agent/registrar'),
('AUDITOR', 'Auditor', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'External auditor'),
('LEGAL_COUNSEL', 'Legal Counsel', 'DELEGATE', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE, 'Legal advisor'),

-- Other
('SUBSIDIARY', 'Subsidiary', 'OWNERSHIP', FALSE, TRUE, FALSE, FALSE, NULL, FALSE, 'Corporate subsidiary'),
('NOMINEE', 'Nominee', 'INVESTOR', FALSE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE, 'Nominee/omnibus account');
```

---

### 1.2 Entity Regulatory Registrations (Multi-Regulator)

**File:** `migrations/V0XX__entity_regulatory_registrations.sql`

```sql
-- ═══════════════════════════════════════════════════════════════════════════
-- Entity Regulatory Registrations: Multi-regulator support
-- ═══════════════════════════════════════════════════════════════════════════

-- Extend ob_kyc schema
CREATE SCHEMA IF NOT EXISTS ob_kyc;

CREATE TABLE ob_kyc.entity_regulatory_registrations (
    registration_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(entity_id),
    regulator_code VARCHAR(50) NOT NULL REFERENCES ob_ref.regulators(regulator_code),
    
    -- Registration details
    registration_number VARCHAR(100),
    registration_type VARCHAR(50) NOT NULL REFERENCES ob_ref.registration_types(registration_type),
    activity_scope TEXT,
    
    -- For passporting/branch
    home_regulator_code VARCHAR(50) REFERENCES ob_ref.regulators(regulator_code),
    passport_reference VARCHAR(100),
    
    -- Verification
    registration_verified BOOLEAN DEFAULT FALSE,
    verification_date DATE,
    verification_method VARCHAR(50),
    verification_reference VARCHAR(500),
    verification_expires DATE,
    
    -- Status and validity
    status VARCHAR(50) DEFAULT 'ACTIVE',
    effective_date DATE DEFAULT CURRENT_DATE,
    expiry_date DATE,
    
    -- Audit
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    created_by UUID,
    updated_by UUID,
    
    -- Unique constraint: one registration per entity per regulator
    CONSTRAINT uq_entity_regulator UNIQUE (entity_id, regulator_code)
);

CREATE INDEX idx_ereg_entity ON ob_kyc.entity_regulatory_registrations(entity_id);
CREATE INDEX idx_ereg_regulator ON ob_kyc.entity_regulatory_registrations(regulator_code);
CREATE INDEX idx_ereg_status ON ob_kyc.entity_regulatory_registrations(status);
CREATE INDEX idx_ereg_type ON ob_kyc.entity_regulatory_registrations(registration_type);
CREATE INDEX idx_ereg_verified ON ob_kyc.entity_regulatory_registrations(registration_verified);
CREATE INDEX idx_ereg_expires ON ob_kyc.entity_regulatory_registrations(verification_expires);

-- ───────────────────────────────────────────────────────────────────────────
-- View: Entity regulatory summary (for quick status checks)
-- ───────────────────────────────────────────────────────────────────────────

CREATE OR REPLACE VIEW ob_kyc.v_entity_regulatory_summary AS
SELECT 
    e.entity_id,
    e.entity_name,
    e.entity_type,
    COUNT(r.registration_id) AS registration_count,
    COUNT(r.registration_id) FILTER (WHERE r.registration_verified AND r.status = 'ACTIVE') AS verified_count,
    BOOL_OR(r.registration_verified AND r.status = 'ACTIVE' AND rt.allows_simplified_dd) AS allows_simplified_dd,
    ARRAY_AGG(DISTINCT r.regulator_code) FILTER (WHERE r.status = 'ACTIVE') AS active_regulators,
    ARRAY_AGG(DISTINCT r.regulator_code) FILTER (WHERE r.registration_verified AND r.status = 'ACTIVE') AS verified_regulators,
    MAX(r.verification_date) AS last_verified,
    MIN(r.verification_expires) FILTER (WHERE r.verification_expires > CURRENT_DATE) AS next_expiry
FROM entities e
LEFT JOIN ob_kyc.entity_regulatory_registrations r ON e.entity_id = r.entity_id
LEFT JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
LEFT JOIN ob_ref.regulatory_tiers rt ON reg.regulatory_tier = rt.tier_code
GROUP BY e.entity_id, e.entity_name, e.entity_type;

-- ───────────────────────────────────────────────────────────────────────────
-- Function: Check if entity allows simplified due diligence
-- ───────────────────────────────────────────────────────────────────────────

CREATE OR REPLACE FUNCTION ob_kyc.entity_allows_simplified_dd(p_entity_id UUID)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS (
        SELECT 1
        FROM ob_kyc.entity_regulatory_registrations r
        JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
        JOIN ob_ref.regulatory_tiers rt ON reg.regulatory_tier = rt.tier_code
        WHERE r.entity_id = p_entity_id
          AND r.status = 'ACTIVE'
          AND r.registration_verified = TRUE
          AND rt.allows_simplified_dd = TRUE
    );
END;
$$ LANGUAGE plpgsql STABLE;
```

---

## 2. Configuration Files

### 2.1 Role Types YAML

**File:** `config/ontology/role_types.yaml`

```yaml
# Role types and their KYC implications
# This file documents the configuration; actual data is in database

role_types:
  # ═══════════════════════════════════════════════════════════════
  # PRINCIPAL: Always requires full KYC
  # ═══════════════════════════════════════════════════════════════
  ACCOUNT_HOLDER:
    name: "Account Holder"
    category: PRINCIPAL
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: true
    cascade_to_entity_ubos: true
    description: |
      The primary client entity. Always requires full KYC regardless
      of regulatory status. UBO discovery cascades from this entity.

  UBO:
    name: "Ultimate Beneficial Owner"
    category: OWNERSHIP
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: true
    description: |
      Person with >25% ownership or significant control.
      Always requires full KYC.

  # ═══════════════════════════════════════════════════════════════
  # DELEGATE: Check regulatory status for simplified DD
  # ═══════════════════════════════════════════════════════════════
  MANCO:
    name: "Management Company"
    category: DELEGATE
    triggers_full_kyc: true
    check_regulatory_status: true
    if_regulated_obligation: SIMPLIFIED
    description: |
      If regulated (CSSF, CBI, FCA, etc.), simplified DD applies.
      Verify registration, screen, but don't chase ManCo's UBOs.

  INVESTMENT_MGR:
    name: "Investment Manager"
    category: DELEGATE
    triggers_full_kyc: true
    check_regulatory_status: true
    if_regulated_obligation: SIMPLIFIED
    description: |
      If regulated, simplified DD. Otherwise full KYC.

  # ═══════════════════════════════════════════════════════════════
  # GOVERNANCE: Screening and ID verification only
  # ═══════════════════════════════════════════════════════════════
  DIRECTOR:
    name: "Director"
    category: GOVERNANCE
    triggers_screening: true
    triggers_id_verification: true
    description: |
      Board directors. PEP/sanctions screening + ID verification.
      No full KYC required.

  SIGNATORY:
    name: "Authorized Signatory"
    category: AUTHORITY
    triggers_screening: true
    triggers_id_verification: true
    description: |
      Persons authorized to sign on behalf of entity.
      Verify identity and screen.
```

---

### 2.2 Regulators YAML

**File:** `config/ontology/regulators.yaml`

```yaml
# Recognized regulators and their equivalence tiers
# This file documents the configuration; actual data is in database

regulatory_tiers:
  EQUIVALENT:
    description: "Full reliance permitted"
    allows_simplified_dd: true
    requires_enhanced_screening: false
    jurisdictions: |
      UK, US, EU/EEA, Switzerland, Singapore, Hong Kong,
      Australia, Japan, Cayman, Bermuda, Guernsey, Jersey

  ACCEPTABLE:
    description: "Partial reliance with enhanced checks"
    allows_simplified_dd: true
    requires_enhanced_screening: true
    
  NONE:
    description: "No reliance - full KYC required"
    allows_simplified_dd: false

regulators:
  # UK - Dual regulation
  FCA:
    name: "Financial Conduct Authority"
    jurisdiction: GB
    tier: EQUIVALENT
    type: GOVERNMENT
    notes: "Conduct regulator. Dual-regulated firms also have PRA."
    
  PRA:
    name: "Prudential Regulation Authority"
    jurisdiction: GB
    tier: EQUIVALENT
    type: GOVERNMENT
    notes: "Prudential regulator for banks, insurers, major investment firms."

  # US - Multiple regulators
  SEC:
    name: "Securities and Exchange Commission"
    jurisdiction: US
    tier: EQUIVALENT
    type: GOVERNMENT
    
  FINRA:
    name: "Financial Industry Regulatory Authority"
    jurisdiction: US
    tier: EQUIVALENT
    type: SRO
    notes: "Self-regulatory organization for broker-dealers."
    
  CFTC:
    name: "Commodity Futures Trading Commission"
    jurisdiction: US
    tier: EQUIVALENT
    type: GOVERNMENT
    notes: "Derivatives/futures regulation."

  # EU - Major jurisdictions
  CSSF:
    name: "Commission de Surveillance du Secteur Financier"
    jurisdiction: LU
    tier: EQUIVALENT
    type: GOVERNMENT
    notes: "Luxembourg. Major fund domicile."
    
  CBI:
    name: "Central Bank of Ireland"
    jurisdiction: IE
    tier: EQUIVALENT
    type: GOVERNMENT
    notes: "Ireland. Major fund domicile."
    
  BaFin:
    name: "Bundesanstalt für Finanzdienstleistungsaufsicht"
    jurisdiction: DE
    tier: EQUIVALENT
    type: GOVERNMENT
```

---

### 2.3 Product KYC Configuration

**File:** `config/ontology/product_kyc_config.yaml`

```yaml
# Product risk ratings and KYC contexts

products:
  # ═══════════════════════════════════════════════════════════════
  # HIGH RISK PRODUCTS
  # ═══════════════════════════════════════════════════════════════
  CUSTODY:
    kyc_risk_rating: HIGH
    requires_kyc: true
    kyc_context: CUSTODY
    description: "Asset safekeeping - high risk due to asset control"
    
  PRIME_BROKERAGE:
    kyc_risk_rating: HIGH
    requires_kyc: true
    kyc_context: CUSTODY
    
  DERIVATIVES_CLEARING:
    kyc_risk_rating: HIGH
    requires_kyc: true
    kyc_context: CUSTODY
    
  SECURITIES_LENDING:
    kyc_risk_rating: HIGH
    requires_kyc: true
    kyc_context: CUSTODY

  # ═══════════════════════════════════════════════════════════════
  # MEDIUM RISK PRODUCTS
  # ═══════════════════════════════════════════════════════════════
  FUND_ACCOUNTING:
    kyc_risk_rating: MEDIUM
    requires_kyc: true
    kyc_context: CUSTODY
    
  TRANSFER_AGENCY:
    kyc_risk_rating: MEDIUM
    requires_kyc: true
    kyc_context: TRANSFER_AGENT
    includes_investor_kyc: true
    description: "Includes KYC of fund investors"
    
  KYC_AS_A_SERVICE:
    kyc_risk_rating: MEDIUM
    requires_kyc: true
    kyc_context: KYC_AS_A_SERVICE
    requires_sponsor: true
    requires_service_agreement: true
    description: "Outsourced KYC on behalf of sponsor"

  # ═══════════════════════════════════════════════════════════════
  # LOW RISK PRODUCTS
  # ═══════════════════════════════════════════════════════════════
  REPORTING_ONLY:
    kyc_risk_rating: LOW
    requires_kyc: false
    description: "Reporting services only - no asset handling"

# Risk rating determines case-level risk when multiple products
risk_rating_precedence:
  - HIGH
  - MEDIUM
  - LOW
```

---

### 2.4 KYC Scope Templates

**File:** `config/ontology/kyc_scope_templates.yaml`

```yaml
# KYC scoping rules by CBU type and service context

scope_templates:

  FUND:
    description: "Pooled investment fund - custody context"
    account_holder_is: FUND_ENTITY
    
    ubo_rules:
      applies: conditional
      threshold_pct: 25
      note: "Funds rarely have >25% individual owners"
      
    cascade_rules:
      chase_manco_ubos: false
      chase_im_ubos: false
      chase_fund_investors: false
      
    role_obligations:
      ACCOUNT_HOLDER: FULL_KYC
      UBO: FULL_KYC
      CONTROLLER: FULL_KYC
      MANCO: CHECK_REGULATORY
      INVESTMENT_MGR: CHECK_REGULATORY
      DIRECTOR: SCREEN_AND_ID
      SIGNATORY: SCREEN_AND_ID
      CONDUCTING_OFFICER: SCREEN_AND_ID
      DELEGATE: CHECK_REGULATORY
      DEPOSITARY: RECORD_ONLY
      CUSTODIAN: RECORD_ONLY
      ADMINISTRATOR: RECORD_ONLY
      AUDITOR: RECORD_ONLY

  HEDGE_FUND:
    inherits: FUND
    description: "Alternative investment fund - enhanced scrutiny"
    risk_floor: MEDIUM
    
    role_obligations:
      CONTROLLER: FULL_KYC_ENHANCED
      INVESTMENT_MGR: FULL_KYC
      PRIME_BROKER: SIMPLIFIED
      
    additional_checks:
      - STRATEGY_RISK_ASSESSMENT
      - INVESTOR_CONCENTRATION
      - LEVERAGE_REVIEW

  CORPORATE:
    description: "Corporate entity as direct client"
    account_holder_is: COMPANY_ENTITY
    
    ubo_rules:
      applies: always
      threshold_pct: 25
      max_chain_depth: 4
      
    cascade_rules:
      chase_parent_ubos: true
      stop_at_regulated: true
      stop_at_listed: true
      
    role_obligations:
      ACCOUNT_HOLDER: FULL_KYC
      UBO: FULL_KYC
      CONTROLLER: FULL_KYC
      DIRECTOR: SCREEN_AND_ID
      SIGNATORY: SCREEN_AND_ID
      PARENT_COMPANY: CHECK_REGULATORY_OR_LISTED

  INDIVIDUAL:
    description: "Natural person - retail/HNW client"
    account_holder_is: PERSON_ENTITY
    
    ubo_rules:
      applies: false
      
    role_obligations:
      ACCOUNT_HOLDER: FULL_KYC
      AUTHORIZED_PERSON: SCREEN_AND_ID
      JOINT_HOLDER: FULL_KYC
      
    required_checks:
      - ID_VERIFICATION
      - ADDRESS_VERIFICATION
      - SOURCE_OF_FUNDS
      - PEP_SCREENING
      - SANCTIONS_SCREENING

  FUND_INVESTOR_RETAIL:
    description: "Retail investor in fund - TA context"
    service_context: TRANSFER_AGENT
    kyc_on_behalf_of: FUND
    
    threshold_based_kyc:
      - threshold: 15000
        currency: EUR
        obligation: SIMPLIFIED_KYC
      - threshold: 150000
        currency: EUR
        obligation: STANDARD_KYC
      - threshold: 1000000
        currency: EUR
        obligation: ENHANCED_KYC

  FUND_INVESTOR_INSTITUTIONAL:
    description: "Institutional investor in fund"
    service_context: TRANSFER_AGENT
    kyc_on_behalf_of: FUND
    
    role_obligations:
      ACCOUNT_HOLDER: CHECK_REGULATORY
      UBO: FULL_KYC
      SIGNATORY: SCREEN_AND_ID
```

---

## 3. DSL Verbs

### 3.1 Admin Verbs - Role Types

**File:** `config/verbs/admin/role-types.yaml`

```yaml
domain: admin.role-types

list:
  description: "List all role types"
  behavior: crud
  crud:
    operation: select
    table: role_types
    schema: ob_ref
    multiple: true
  args:
    - name: category
      type: string
      required: false
    - name: active-only
      type: boolean
      required: false
      default: true

read:
  description: "Get role type by code"
  behavior: crud
  crud:
    operation: select
    table: role_types
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true

create:
  description: "Create new role type"
  behavior: crud
  crud:
    operation: insert
    table: role_types
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
    - name: name
      type: string
      required: true
    - name: category
      type: string
      required: true
      enum: [PRINCIPAL, OWNERSHIP, DELEGATE, GOVERNANCE, AUTHORITY, INVESTOR]
    - name: triggers-full-kyc
      type: boolean
      default: false
      column: triggers_full_kyc
    - name: triggers-screening
      type: boolean
      default: false
      column: triggers_screening
    - name: triggers-id-verification
      type: boolean
      default: false
      column: triggers_id_verification
    - name: check-regulatory-status
      type: boolean
      default: false
      column: check_regulatory_status
    - name: if-regulated-obligation
      type: string
      required: false
      column: if_regulated_obligation
      enum: [SIMPLIFIED, SCREEN_ONLY, RECORD_ONLY]
    - name: cascade-to-entity-ubos
      type: boolean
      default: false
      column: cascade_to_entity_ubos
    - name: description
      type: string
      required: false

update:
  description: "Update role type"
  behavior: crud
  crud:
    operation: update
    table: role_types
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
      key: true
    - name: triggers-full-kyc
      type: boolean
      required: false
    - name: triggers-screening
      type: boolean
      required: false
    - name: check-regulatory-status
      type: boolean
      required: false
    - name: if-regulated-obligation
      type: string
      required: false
    - name: active
      type: boolean
      required: false
```

---

### 3.2 Admin Verbs - Regulators

**File:** `config/verbs/admin/regulators.yaml`

```yaml
domain: admin.regulators

list:
  description: "List recognized regulators"
  behavior: crud
  crud:
    operation: select
    table: regulators
    schema: ob_ref
    multiple: true
  args:
    - name: jurisdiction
      type: string
      required: false
    - name: tier
      type: string
      required: false
      column: regulatory_tier
    - name: active-only
      type: boolean
      required: false
      default: true

read:
  description: "Get regulator by code"
  behavior: crud
  crud:
    operation: select
    table: regulators
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
      column: regulator_code

create:
  description: "Add new recognized regulator"
  behavior: crud
  crud:
    operation: insert
    table: regulators
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
      column: regulator_code
    - name: name
      type: string
      required: true
      column: regulator_name
    - name: jurisdiction
      type: string
      required: true
    - name: tier
      type: string
      required: true
      column: regulatory_tier
      enum: [EQUIVALENT, ACCEPTABLE, NONE]
    - name: type
      type: string
      required: false
      column: regulator_type
      default: GOVERNMENT
      enum: [GOVERNMENT, SRO, CENTRAL_BANK]
    - name: registry-url
      type: string
      required: false
      column: registry_url

update:
  description: "Update regulator"
  behavior: crud
  crud:
    operation: update
    table: regulators
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
      column: regulator_code
      key: true
    - name: tier
      type: string
      required: false
      column: regulatory_tier
    - name: registry-url
      type: string
      required: false
      column: registry_url
    - name: active
      type: boolean
      required: false

deactivate:
  description: "Deactivate regulator"
  behavior: crud
  crud:
    operation: update
    table: regulators
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
      column: regulator_code
      key: true
  set:
    active: false
```

---

### 3.3 Regulatory Registration Verbs

**File:** `config/verbs/regulatory.yaml`

```yaml
domain: regulatory

# ═══════════════════════════════════════════════════════════════════════════
# Registration management (multi-regulator)
# ═══════════════════════════════════════════════════════════════════════════

registration:
  add:
    description: "Add regulatory registration for an entity"
    behavior: crud
    crud:
      operation: insert
      table: entity_regulatory_registrations
      schema: ob_kyc
    args:
      - name: entity-id
        type: uuid
        required: true
        lookup:
          entity_type: entity
      - name: regulator
        type: string
        required: true
        column: regulator_code
        lookup:
          table: regulators
          schema: ob_ref
          search_key: regulator_code
      - name: registration-number
        type: string
        required: false
      - name: registration-type
        type: string
        required: true
        column: registration_type
        enum: [PRIMARY, DUAL_CONDUCT, DUAL_PRUDENTIAL, PASSPORTED, BRANCH, SUBSIDIARY, ADDITIONAL, STATE, SRO]
      - name: activity-scope
        type: string
        required: false
        column: activity_scope
      - name: home-regulator
        type: string
        required: false
        column: home_regulator_code
      - name: effective-date
        type: date
        required: false
        column: effective_date

  list:
    description: "List regulatory registrations for an entity"
    behavior: crud
    crud:
      operation: select
      table: entity_regulatory_registrations
      schema: ob_kyc
      multiple: true
    args:
      - name: entity-id
        type: uuid
        required: true
        column: entity_id
      - name: status
        type: string
        required: false
        default: ACTIVE

  verify:
    description: "Verify a regulatory registration"
    behavior: plugin
    plugin:
      handler: RegistrationVerifyOp
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: regulator
        type: string
        required: true
      - name: method
        type: string
        required: true
        column: verification_method
        enum: [MANUAL, REGISTRY_API, DOCUMENT]
      - name: reference
        type: string
        required: false
        column: verification_reference
      - name: expires
        type: date
        required: false
        column: verification_expires

  remove:
    description: "Remove/withdraw registration"
    behavior: crud
    crud:
      operation: update
      table: entity_regulatory_registrations
      schema: ob_kyc
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: regulator
        type: string
        required: true
        column: regulator_code
    set:
      status: WITHDRAWN
      expiry_date: CURRENT_DATE

# ═══════════════════════════════════════════════════════════════════════════
# Status checks (computed from registrations)
# ═══════════════════════════════════════════════════════════════════════════

status:
  check:
    description: "Check entity's overall regulatory status"
    behavior: plugin
    plugin:
      handler: RegulatoryStatusCheckOp
    args:
      - name: entity-id
        type: uuid
        required: true
    returns:
      type: object
      fields:
        is_regulated: boolean
        registration_count: integer
        verified_count: integer
        allows_simplified_dd: boolean
        registrations: array
        next_verification_due: date
```

---

## 4. Rust Implementation

### 4.1 Config Loader

```rust
// src/config/kyc_config.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct RoleTypeConfig {
    pub name: String,
    pub category: String,
    pub triggers_full_kyc: bool,
    pub triggers_screening: bool,
    pub triggers_id_verification: bool,
    pub check_regulatory_status: bool,
    pub if_regulated_obligation: Option<String>,
    pub cascade_to_entity_ubos: bool,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScopeTemplate {
    pub description: String,
    pub account_holder_is: String,
    pub ubo_rules: Option<UboRules>,
    pub cascade_rules: Option<CascadeRules>,
    pub role_obligations: HashMap<String, String>,
    pub required_checks: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UboRules {
    pub applies: String,  // "always", "conditional", "false"
    pub threshold_pct: Option<u8>,
    pub max_chain_depth: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct CascadeRules {
    pub chase_manco_ubos: bool,
    pub chase_im_ubos: bool,
    pub chase_fund_investors: bool,
    pub stop_at_regulated: Option<bool>,
    pub stop_at_listed: Option<bool>,
}

pub fn load_scope_templates() -> Result<HashMap<String, ScopeTemplate>> {
    let yaml = std::fs::read_to_string("config/ontology/kyc_scope_templates.yaml")?;
    let config: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
    let templates = config["scope_templates"].as_mapping()
        .ok_or_else(|| anyhow!("Missing scope_templates"))?;
    
    let mut result = HashMap::new();
    for (key, value) in templates {
        let name = key.as_str().unwrap().to_string();
        let template: ScopeTemplate = serde_yaml::from_value(value.clone())?;
        result.insert(name, template);
    }
    Ok(result)
}
```

---

### 4.2 Regulatory Status Check Plugin

```rust
// src/dsl_v2/custom_ops/regulatory.rs

use sqlx::PgPool;
use serde_json::{json, Value};

pub struct RegulatoryStatusCheckOp;

impl RegulatoryStatusCheckOp {
    pub async fn execute(&self, args: &Args, pool: &PgPool) -> Result<Value> {
        let entity_id = args.get_uuid("entity-id")?;
        
        // Query the summary view
        let summary = sqlx::query_as!(
            RegulatorySummary,
            r#"
            SELECT 
                entity_id,
                entity_name,
                registration_count,
                verified_count,
                allows_simplified_dd,
                active_regulators,
                verified_regulators,
                last_verified,
                next_expiry
            FROM ob_kyc.v_entity_regulatory_summary
            WHERE entity_id = $1
            "#,
            entity_id
        )
        .fetch_optional(pool)
        .await?;
        
        let summary = summary.unwrap_or_default();
        
        // Get detailed registrations
        let registrations = sqlx::query!(
            r#"
            SELECT 
                r.regulator_code,
                r.registration_type,
                r.registration_verified,
                r.verification_expires,
                r.status,
                reg.regulator_name,
                rt.tier_code as regulatory_tier
            FROM ob_kyc.entity_regulatory_registrations r
            JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
            JOIN ob_ref.regulatory_tiers rt ON reg.regulatory_tier = rt.tier_code
            WHERE r.entity_id = $1 AND r.status = 'ACTIVE'
            ORDER BY r.registration_type
            "#,
            entity_id
        )
        .fetch_all(pool)
        .await?;
        
        Ok(json!({
            "entity_id": entity_id,
            "is_regulated": summary.registration_count > 0,
            "registration_count": summary.registration_count,
            "verified_count": summary.verified_count,
            "allows_simplified_dd": summary.allows_simplified_dd,
            "active_regulators": summary.active_regulators,
            "verified_regulators": summary.verified_regulators,
            "last_verified": summary.last_verified,
            "next_verification_due": summary.next_expiry,
            "registrations": registrations.iter().map(|r| json!({
                "regulator": r.regulator_code,
                "regulator_name": r.regulator_name,
                "type": r.registration_type,
                "verified": r.registration_verified,
                "tier": r.regulatory_tier,
                "expires": r.verification_expires
            })).collect::<Vec<_>>()
        }))
    }
}
```

---

## 5. Verification Tests

After implementation, verify:

```lisp
; List role types
(admin.role-types.list)
; → Returns 20+ role types with KYC triggers

; Check specific role
(admin.role-types.read code:MANCO)
; → {triggers_full_kyc: true, check_regulatory_status: true, if_regulated_obligation: "SIMPLIFIED"}

; List regulators
(admin.regulators.list jurisdiction:GB)
; → [{FCA, ...}, {PRA, ...}]

; Add registration
(regulatory.registration.add 
  entity-id:@test-entity 
  regulator:FCA 
  registration-type:PRIMARY 
  registration-number:"123456")

; Add second registration (dual regulation)
(regulatory.registration.add 
  entity-id:@test-entity 
  regulator:PRA 
  registration-type:DUAL_PRUDENTIAL)

; List registrations
(regulatory.registration.list entity-id:@test-entity)
; → [{FCA, PRIMARY}, {PRA, DUAL_PRUDENTIAL}]

; Check status
(regulatory.status.check entity-id:@test-entity)
; → {is_regulated: true, registration_count: 2, allows_simplified_dd: true, ...}
```

---

## Summary

**Phase 1 establishes:**

1. **Reference tables** - role_types, regulators, regulatory_tiers, registration_types
2. **Multi-regulator support** - entity_regulatory_registrations (many-to-many)
3. **Admin DSL** - Manage reference data at runtime
4. **Regulatory DSL** - Add/verify/check registrations
5. **Status check** - Determine if simplified DD allowed

**This enables Phase 2+:**
- KYC scope preview uses role types + regulatory status
- Case builder applies correct obligations
- Agent can query regulatory status before scoping
