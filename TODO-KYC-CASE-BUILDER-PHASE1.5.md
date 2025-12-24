## Phase 1.5: Reference Data Admin Verbs

### 1.5.1 Role Types Admin

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
  args: []

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
      column: code

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
```

---

### 1.5.2 Regulators Admin

**File:** `config/verbs/admin/regulators.yaml`

```yaml
domain: admin.regulators

list:
  description: "List all recognized regulators"
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
      description: "ISO 3166-1 alpha-2 country code"
    - name: tier
      type: string
      required: true
      column: regulatory_tier
      enum: [EQUIVALENT, ACCEPTABLE, NONE]
    - name: registry-url
      type: string
      required: false
      column: registry_url

update:
  description: "Update regulator details"
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
  description: "Deactivate regulator (no longer recognized)"
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

### 1.5.3 Regulatory Tiers Admin

**File:** `config/verbs/admin/regulatory-tiers.yaml`

```yaml
domain: admin.regulatory-tiers

list:
  description: "List all regulatory tiers"
  behavior: crud
  crud:
    operation: select
    table: regulatory_tiers
    schema: ob_ref
    multiple: true
  args: []

read:
  description: "Get tier by code"
  behavior: crud
  crud:
    operation: select
    table: regulatory_tiers
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
      column: tier_code

create:
  description: "Create regulatory tier"
  behavior: crud
  crud:
    operation: insert
    table: regulatory_tiers
    schema: ob_ref
  args:
    - name: code
      type: string
      required: true
      column: tier_code
    - name: description
      type: string
      required: true
    - name: allows-simplified-dd
      type: boolean
      required: true
      column: allows_simplified_dd
    - name: requires-enhanced-screening
      type: boolean
      default: false
      column: requires_enhanced_screening
```

---

### 1.5.4 Updated Migration with Seed Support

**Migration:** `V0XX__add_reference_data_tables.sql`

```sql
-- Schema for reference data
CREATE SCHEMA IF NOT EXISTS ob_ref;

-- Role types
CREATE TABLE ob_ref.role_types (
    role_type_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    triggers_full_kyc BOOLEAN DEFAULT FALSE,
    triggers_screening BOOLEAN DEFAULT FALSE,
    triggers_id_verification BOOLEAN DEFAULT FALSE,
    check_regulatory_status BOOLEAN DEFAULT FALSE,
    if_regulated_obligation VARCHAR(50),
    cascade_to_entity_ubos BOOLEAN DEFAULT FALSE,
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Regulatory tiers
CREATE TABLE ob_ref.regulatory_tiers (
    tier_code VARCHAR(50) PRIMARY KEY,
    description VARCHAR(255) NOT NULL,
    allows_simplified_dd BOOLEAN DEFAULT FALSE,
    requires_enhanced_screening BOOLEAN DEFAULT FALSE
);

-- Regulators
CREATE TABLE ob_ref.regulators (
    regulator_code VARCHAR(50) PRIMARY KEY,
    regulator_name VARCHAR(255) NOT NULL,
    jurisdiction VARCHAR(2) NOT NULL,
    regulatory_tier VARCHAR(50) REFERENCES ob_ref.regulatory_tiers(tier_code),
    registry_url VARCHAR(500),
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_regulators_jurisdiction ON ob_ref.regulators(jurisdiction);
CREATE INDEX idx_regulators_tier ON ob_ref.regulators(regulatory_tier);

-- Seed regulatory tiers (rarely change)
INSERT INTO ob_ref.regulatory_tiers (tier_code, description, allows_simplified_dd, requires_enhanced_screening) VALUES
('EQUIVALENT', 'Full reliance permitted - equivalent jurisdiction', TRUE, FALSE),
('ACCEPTABLE', 'Partial reliance - enhanced screening required', TRUE, TRUE),
('NONE', 'No reliance - full KYC required', FALSE, FALSE);

-- Seed common regulators
INSERT INTO ob_ref.regulators (regulator_code, regulator_name, jurisdiction, regulatory_tier, registry_url) VALUES
('FCA', 'Financial Conduct Authority', 'GB', 'EQUIVALENT', 'https://register.fca.org.uk/s/'),
('PRA', 'Prudential Regulation Authority', 'GB', 'EQUIVALENT', NULL),
('SEC', 'Securities and Exchange Commission', 'US', 'EQUIVALENT', 'https://www.sec.gov/cgi-bin/browse-edgar'),
('FINRA', 'Financial Industry Regulatory Authority', 'US', 'EQUIVALENT', 'https://brokercheck.finra.org/'),
('CSSF', 'Commission de Surveillance du Secteur Financier', 'LU', 'EQUIVALENT', 'https://www.cssf.lu/en/entity-search/'),
('CBI', 'Central Bank of Ireland', 'IE', 'EQUIVALENT', 'http://registers.centralbank.ie/'),
('BaFin', 'Bundesanstalt für Finanzdienstleistungsaufsicht', 'DE', 'EQUIVALENT', 'https://portal.mvp.bafin.de/database/InstInfo/'),
('AMF', 'Autorité des marchés financiers', 'FR', 'EQUIVALENT', 'https://www.amf-france.org/en/professionals'),
('FINMA', 'Swiss Financial Market Supervisory Authority', 'CH', 'EQUIVALENT', 'https://www.finma.ch/en/authorisation/self-regulatory-organisations-sros/'),
('MAS', 'Monetary Authority of Singapore', 'SG', 'EQUIVALENT', 'https://eservices.mas.gov.sg/fid'),
('SFC', 'Securities and Futures Commission', 'HK', 'EQUIVALENT', 'https://www.sfc.hk/publicregWeb/searchByName'),
('ASIC', 'Australian Securities and Investments Commission', 'AU', 'EQUIVALENT', 'https://connectonline.asic.gov.au/'),
('JFSA', 'Japan Financial Services Agency', 'JP', 'EQUIVALENT', NULL),
('AFM', 'Autoriteit Financiële Markten', 'NL', 'EQUIVALENT', 'https://www.afm.nl/en/sector/registers');

-- Seed role types
INSERT INTO ob_ref.role_types (code, name, triggers_full_kyc, triggers_screening, triggers_id_verification, check_regulatory_status, if_regulated_obligation, cascade_to_entity_ubos) VALUES
('ACCOUNT_HOLDER', 'Account Holder', TRUE, TRUE, TRUE, FALSE, NULL, TRUE),
('UBO', 'Ultimate Beneficial Owner', TRUE, TRUE, TRUE, FALSE, NULL, FALSE),
('CONTROLLER', 'Controller', TRUE, TRUE, TRUE, FALSE, NULL, FALSE),
('MANCO', 'Management Company', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE),
('INVESTMENT_MGR', 'Investment Manager', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE),
('DIRECTOR', 'Director', FALSE, TRUE, TRUE, FALSE, NULL, FALSE),
('SIGNATORY', 'Authorized Signatory', FALSE, TRUE, TRUE, FALSE, NULL, FALSE),
('DELEGATE', 'Delegate/Service Provider', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE),
('INVESTOR', 'Fund Investor', TRUE, TRUE, TRUE, TRUE, 'SIMPLIFIED', FALSE),
('CUSTODIAN', 'Custodian', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE),
('DEPOSITARY', 'Depositary', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE),
('ADMINISTRATOR', 'Fund Administrator', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE),
('TRANSFER_AGENT', 'Transfer Agent', FALSE, FALSE, FALSE, TRUE, 'RECORD_ONLY', FALSE),
('AUDITOR', 'Auditor', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE),
('LEGAL_COUNSEL', 'Legal Counsel', FALSE, TRUE, FALSE, TRUE, 'RECORD_ONLY', FALSE),
('PRIME_BROKER', 'Prime Broker', FALSE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE),
('AUTHORIZED_PERSON', 'Authorized Person (POA)', FALSE, TRUE, TRUE, FALSE, NULL, FALSE),
('JOINT_HOLDER', 'Joint Account Holder', TRUE, TRUE, TRUE, FALSE, NULL, FALSE),
('PARENT_COMPANY', 'Parent Company', TRUE, TRUE, FALSE, TRUE, 'SIMPLIFIED', FALSE),
('SUBSIDIARY', 'Subsidiary', FALSE, TRUE, FALSE, FALSE, NULL, FALSE);
```

---

### 1.5.5 DSL Usage Examples

**Listing regulators:**
```lisp
(admin.regulators.list)
(admin.regulators.list jurisdiction:GB)
(admin.regulators.list tier:EQUIVALENT)
```

**Adding a new regulator:**
```lisp
(admin.regulators.create 
  code:CFTC 
  name:"Commodity Futures Trading Commission" 
  jurisdiction:US 
  tier:EQUIVALENT)
```

**Updating regulator tier:**
```lisp
(admin.regulators.update code:XYZ tier:ACCEPTABLE)
```

**Listing role types:**
```lisp
(admin.role-types.list)
```

**Checking role type config:**
```lisp
(admin.role-types.read code:MANCO)
; Returns: {triggers_full_kyc: true, check_regulatory_status: true, if_regulated_obligation: "SIMPLIFIED"}
```

**Adding custom role type:**
```lisp
(admin.role-types.create
  code:PLACEMENT_AGENT
  name:"Placement Agent"
  triggers-full-kyc:false
  triggers-screening:true
  check-regulatory-status:true
  if-regulated-obligation:RECORD_ONLY)
```

---

### Tasks Added to Phase 1

- [ ] Create `config/verbs/admin/` directory
- [ ] Create `role-types.yaml` verb file
- [ ] Create `regulators.yaml` verb file
- [ ] Create `regulatory-tiers.yaml` verb file
- [ ] Update migration to use `ob_ref` schema
- [ ] Seed role types in migration
- [ ] Seed regulators in migration
- [ ] Register admin verbs in verb loader
- [ ] Test: `(admin.regulators.list)`
- [ ] Test: `(admin.role-types.list)`
