# Fund Structure Ontology Implementation

## Overview

Extend the CBU/entity model to support complex fund structures including:
- Umbrella funds (SICAV, ICAV, OEIC) with sub-funds and share classes
- Fund of Funds with look-through ownership
- Master-Feeder structures
- Management company and service provider relationships
- Control vs ownership distinction
- Delegation chains

This is required before loading complex fund manager data (e.g., Allianz).

---

## Phase 1: Database Schema Extensions

### 1.1 New Entity Types (seed data)

Add to `entity_types` table:

```sql
-- Fund structure entity types
INSERT INTO entity_types (type_code, name, category, table_name, description) VALUES
  ('fund_umbrella', 'Umbrella Fund', 'fund', 'entity_funds', 'SICAV, ICAV, OEIC - single legal entity with multiple compartments'),
  ('fund_subfund', 'Sub-fund/Compartment', 'fund', 'entity_funds', 'Segregated compartment within umbrella fund'),
  ('fund_share_class', 'Share Class', 'fund', 'entity_share_classes', 'Share class within a sub-fund (Inst, Retail, hedged)'),
  ('fund_standalone', 'Standalone Fund', 'fund', 'entity_funds', 'Non-umbrella single fund'),
  ('fund_master', 'Master Fund', 'fund', 'entity_funds', 'Master in master-feeder structure'),
  ('fund_feeder', 'Feeder Fund', 'fund', 'entity_funds', 'Feeder in master-feeder structure');

-- Service provider entity types (these are entities with regulatory roles)
INSERT INTO entity_types (type_code, name, category, table_name, description) VALUES
  ('management_company', 'Management Company', 'service_provider', 'entity_manco', 'ManCo or AIFM - regulated fund manager'),
  ('depositary', 'Depositary', 'service_provider', 'entity_limited_companies', 'Depositary/custodian with fiduciary duty'),
  ('fund_administrator', 'Fund Administrator', 'service_provider', 'entity_limited_companies', 'NAV calculation, TA services');
```

### 1.2 Fund Extension Table

```sql
-- Extension table for fund entities
CREATE TABLE entity_funds (
  entity_id UUID PRIMARY KEY REFERENCES entities(entity_id) ON DELETE CASCADE,
  
  -- Fund identifiers
  lei VARCHAR(20),                    -- Legal Entity Identifier
  isin_base VARCHAR(12),              -- Base ISIN (sub-funds/classes have own ISINs)
  fund_registration_number VARCHAR(100),
  
  -- Fund classification
  fund_structure TEXT NOT NULL,       -- 'UMBRELLA', 'STANDALONE', 'MASTER', 'FEEDER'
  fund_type TEXT,                     -- 'UCITS', 'AIF', 'PRIVATE', 'ETF'
  legal_form TEXT,                    -- 'SICAV', 'ICAV', 'FCP', 'OEIC', 'LP', 'LLC'
  
  -- Regulatory
  domicile_jurisdiction VARCHAR(10) NOT NULL,
  regulatory_status TEXT,             -- 'AUTHORIZED', 'REGISTERED', 'EXEMPT'
  regulator VARCHAR(100),             -- 'CSSF', 'CBI', 'FCA', 'SEC', 'BaFin'
  authorization_date DATE,
  
  -- For sub-funds: link to umbrella
  umbrella_entity_id UUID REFERENCES entities(entity_id),
  compartment_number INTEGER,         -- Legal compartment number within umbrella
  
  -- For feeders: link to master
  master_entity_id UUID REFERENCES entities(entity_id),
  
  -- Investment profile
  investment_objective TEXT,
  base_currency VARCHAR(3),
  
  -- Dates
  inception_date DATE,
  financial_year_end VARCHAR(5),      -- 'MM-DD' format
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_entity_funds_umbrella ON entity_funds(umbrella_entity_id);
CREATE INDEX idx_entity_funds_master ON entity_funds(master_entity_id);
CREATE INDEX idx_entity_funds_structure ON entity_funds(fund_structure);
```

### 1.3 Share Class Extension Table

```sql
-- Extension table for share class entities
CREATE TABLE entity_share_classes (
  entity_id UUID PRIMARY KEY REFERENCES entities(entity_id) ON DELETE CASCADE,
  
  -- Link to parent sub-fund
  subfund_entity_id UUID NOT NULL REFERENCES entities(entity_id),
  
  -- Share class identifiers
  isin VARCHAR(12) UNIQUE,
  share_class_code VARCHAR(20),       -- Internal code (e.g., 'A', 'I', 'R')
  
  -- Classification
  investor_type TEXT NOT NULL,        -- 'INSTITUTIONAL', 'RETAIL', 'PRIVATE'
  currency VARCHAR(3) NOT NULL,
  hedged BOOLEAN DEFAULT FALSE,
  distributing BOOLEAN DEFAULT FALSE, -- vs accumulating
  
  -- Fees
  management_fee_bps INTEGER,
  performance_fee_pct DECIMAL(5,2),
  
  -- Status
  launch_date DATE,
  soft_close_date DATE,
  hard_close_date DATE,
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_share_classes_subfund ON entity_share_classes(subfund_entity_id);
```

### 1.4 Management Company Extension Table

```sql
-- Extension table for ManCo entities
CREATE TABLE entity_manco (
  entity_id UUID PRIMARY KEY REFERENCES entities(entity_id) ON DELETE CASCADE,
  
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
  passported_jurisdictions TEXT[],    -- Array of jurisdiction codes
  
  -- Capital
  regulatory_capital_eur DECIMAL(15,2),
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

### 1.5 Fund Structure Relationships Table

```sql
-- Structural containment relationships (NOT ownership)
-- Umbrella→Subfund, Subfund→ShareClass, Master→Feeder
CREATE TABLE fund_structure_relationships (
  relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  
  parent_entity_id UUID NOT NULL REFERENCES entities(entity_id),
  child_entity_id UUID NOT NULL REFERENCES entities(entity_id),
  
  relationship_type TEXT NOT NULL,    
  -- 'UMBRELLA_CONTAINS_SUBFUND'
  -- 'SUBFUND_CONTAINS_SHARECLASS'
  -- 'MASTER_HAS_FEEDER'
  
  effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
  effective_to DATE,                  -- NULL = current
  
  -- Audit
  created_at TIMESTAMPTZ DEFAULT NOW(),
  created_by VARCHAR(100),
  
  UNIQUE(parent_entity_id, child_entity_id, relationship_type, effective_from)
);

CREATE INDEX idx_fund_structure_parent ON fund_structure_relationships(parent_entity_id);
CREATE INDEX idx_fund_structure_child ON fund_structure_relationships(child_entity_id);
CREATE INDEX idx_fund_structure_type ON fund_structure_relationships(relationship_type);
```

### 1.6 Control Relationships Table

```sql
-- Control relationships SEPARATE from ownership
-- Control may differ from ownership (voting rights, board seats, veto powers)
CREATE TABLE control_relationships (
  control_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  
  controller_entity_id UUID NOT NULL REFERENCES entities(entity_id),
  controlled_entity_id UUID NOT NULL REFERENCES entities(entity_id),
  
  control_type TEXT NOT NULL,
  -- 'VOTING_RIGHTS' - controls voting > ownership
  -- 'BOARD_APPOINTMENT' - right to appoint directors
  -- 'VETO_POWER' - negative control
  -- 'MANAGEMENT_CONTROL' - operational control
  -- 'RESERVED_MATTERS' - approval rights on specific decisions
  
  control_percentage DECIMAL(5,2),    -- May differ from ownership %
  control_description TEXT,           -- Narrative description
  
  -- Evidence
  evidence_doc_id UUID REFERENCES document_catalog(doc_id),
  
  -- Dates
  effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
  effective_to DATE,
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  
  UNIQUE(controller_entity_id, controlled_entity_id, control_type, effective_from)
);

CREATE INDEX idx_control_controller ON control_relationships(controller_entity_id);
CREATE INDEX idx_control_controlled ON control_relationships(controlled_entity_id);
```

### 1.7 Investment Relationships Table (FoF)

```sql
-- Fund-to-Fund investment relationships
-- For Fund of Funds look-through calculations
CREATE TABLE fund_investment_relationships (
  investment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  
  investor_entity_id UUID NOT NULL REFERENCES entities(entity_id),  -- The FoF
  investee_entity_id UUID NOT NULL REFERENCES entities(entity_id),  -- Underlying fund
  
  -- Investment details
  investment_percentage DECIMAL(5,2) NOT NULL,  -- % of FoF NAV
  investment_type TEXT NOT NULL,      -- 'DIRECT', 'INDIRECT'
  
  -- Valuation
  invested_amount DECIMAL(18,2),
  invested_currency VARCHAR(3),
  valuation_date DATE,
  
  -- Dates
  investment_date DATE,
  redemption_date DATE,               -- NULL = still invested
  
  created_at TIMESTAMPTZ DEFAULT NOW(),
  
  UNIQUE(investor_entity_id, investee_entity_id, investment_date)
);

CREATE INDEX idx_fund_investment_investor ON fund_investment_relationships(investor_entity_id);
CREATE INDEX idx_fund_investment_investee ON fund_investment_relationships(investee_entity_id);
```

### 1.8 Delegation Relationships Table

```sql
-- Service provider delegation chains
-- ManCo → Sub-advisor, Administrator → Sub-contractor
CREATE TABLE delegation_relationships (
  delegation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  
  delegator_entity_id UUID NOT NULL REFERENCES entities(entity_id),
  delegate_entity_id UUID NOT NULL REFERENCES entities(entity_id),
  
  -- Scope
  delegation_scope TEXT NOT NULL,
  -- 'INVESTMENT_MANAGEMENT'
  -- 'RISK_MANAGEMENT'
  -- 'PORTFOLIO_ADMINISTRATION'
  -- 'DISTRIBUTION'
  -- 'TRANSFER_AGENCY'
  
  delegation_description TEXT,
  
  -- Which fund/CBU this delegation applies to (optional - may be firm-wide)
  applies_to_cbu_id UUID REFERENCES cbus(cbu_id),
  
  -- Regulatory
  regulatory_notification_date DATE,
  regulatory_approval_required BOOLEAN DEFAULT FALSE,
  regulatory_approval_date DATE,
  
  -- Contract
  contract_doc_id UUID REFERENCES document_catalog(doc_id),
  effective_from DATE NOT NULL,
  effective_to DATE,
  
  created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_delegation_delegator ON delegation_relationships(delegator_entity_id);
CREATE INDEX idx_delegation_delegate ON delegation_relationships(delegate_entity_id);
```

### 1.9 Extended Role Types (seed data)

```sql
-- Add fund-specific roles to roles table
INSERT INTO roles (name, category, description) VALUES
  -- Fund governance
  ('MANAGEMENT_COMPANY', 'fund_governance', 'Appointed ManCo/AIFM'),
  ('DEPOSITARY', 'fund_governance', 'Depositary bank with fiduciary duties'),
  ('AUDITOR', 'fund_governance', 'Statutory auditor'),
  ('LEGAL_COUNSEL', 'fund_governance', 'Legal advisor to the fund'),
  
  -- Fund operations  
  ('FUND_ADMINISTRATOR', 'fund_operations', 'NAV calculation, accounting'),
  ('TRANSFER_AGENT', 'fund_operations', 'Shareholder servicing, registry'),
  ('PAYING_AGENT', 'fund_operations', 'Distribution payments'),
  ('REGISTRAR', 'fund_operations', 'Share register maintenance'),
  
  -- Investment
  ('INVESTMENT_MANAGER', 'investment', 'Discretionary investment management'),
  ('SUB_ADVISOR', 'investment', 'Delegated investment management'),
  ('INVESTMENT_ADVISOR', 'investment', 'Non-discretionary advice'),
  
  -- Distribution
  ('GLOBAL_DISTRIBUTOR', 'distribution', 'Primary distribution responsibility'),
  ('LOCAL_DISTRIBUTOR', 'distribution', 'Jurisdiction-specific distribution'),
  ('PLACEMENT_AGENT', 'distribution', 'Private placement'),
  
  -- Lending/Collateral
  ('PRIME_BROKER', 'financing', 'Prime brokerage services'),
  ('SECURITIES_LENDING_AGENT', 'financing', 'Securities lending program'),
  ('COLLATERAL_MANAGER', 'financing', 'Collateral management');
```

---

## Phase 2: Entity Taxonomy Updates

Add to `rust/config/ontology/entity_taxonomy.yaml`:

```yaml
  # ===========================================================================
  # Fund - Umbrella (SICAV, ICAV, OEIC)
  # ===========================================================================
  fund_umbrella:
    description: "Umbrella fund structure - single legal entity with segregated compartments"
    category: fund
    parent_type: entity
    
    db:
      schema: ob-poc
      table: entities
      pk: entity_id
      extension_table: entity_funds
      extension_fk: entity_id
      type_code: fund_umbrella
      
    search_keys:
      - column: name
        unique: false
      - column: lei
        unique: true
      - columns: [name, domicile_jurisdiction]
        unique: true
        
    lifecycle:
      status_column: status
      states:
        - DRAFT
        - AUTHORIZED
        - ACTIVE
        - SUSPENDED
        - LIQUIDATING
        - TERMINATED
      transitions:
        - from: DRAFT
          to: [AUTHORIZED]
        - from: AUTHORIZED
          to: [ACTIVE]
        - from: ACTIVE
          to: [SUSPENDED, LIQUIDATING]
        - from: SUSPENDED
          to: [ACTIVE, LIQUIDATING]
        - from: LIQUIDATING
          to: [TERMINATED]
      initial_state: DRAFT
      
    implicit_create:
      allowed: true
      canonical_verb: entity.create-fund-umbrella
      required_args: [name, jurisdiction, legal-form]

  # ===========================================================================
  # Fund - Sub-fund / Compartment
  # ===========================================================================
  fund_subfund:
    description: "Sub-fund or compartment within an umbrella fund"
    category: fund
    parent_type: entity
    
    db:
      schema: ob-poc
      table: entities
      pk: entity_id
      extension_table: entity_funds
      extension_fk: entity_id
      type_code: fund_subfund
      
    search_keys:
      - column: name
        unique: false
      - column: isin_base
        unique: true
      - columns: [umbrella_entity_id, name]
        unique: true
        
    containment:
      parent_type: fund_umbrella
      parent_fk: umbrella_entity_id
      
    lifecycle:
      status_column: status
      states:
        - DRAFT
        - LAUNCHED
        - ACTIVE
        - SOFT_CLOSED
        - HARD_CLOSED
        - LIQUIDATING
        - MERGED
        - TERMINATED
      transitions:
        - from: DRAFT
          to: [LAUNCHED]
        - from: LAUNCHED
          to: [ACTIVE]
        - from: ACTIVE
          to: [SOFT_CLOSED, LIQUIDATING, MERGED]
        - from: SOFT_CLOSED
          to: [ACTIVE, HARD_CLOSED, LIQUIDATING]
        - from: HARD_CLOSED
          to: [LIQUIDATING, MERGED]
        - from: LIQUIDATING
          to: [TERMINATED]
        - from: MERGED
          to: [TERMINATED]
      initial_state: DRAFT
      
    implicit_create:
      allowed: true
      canonical_verb: entity.create-fund-subfund
      required_args: [name, umbrella-id, base-currency]

  # ===========================================================================
  # Fund - Share Class
  # ===========================================================================
  fund_share_class:
    description: "Share class within a sub-fund"
    category: fund
    parent_type: entity
    
    db:
      schema: ob-poc
      table: entities
      pk: entity_id
      extension_table: entity_share_classes
      extension_fk: entity_id
      type_code: fund_share_class
      
    search_keys:
      - column: isin
        unique: true
      - columns: [subfund_entity_id, share_class_code]
        unique: true
        
    containment:
      parent_type: fund_subfund
      parent_fk: subfund_entity_id
      
    lifecycle:
      status_column: status
      states:
        - DRAFT
        - LAUNCHED
        - ACTIVE
        - SOFT_CLOSED
        - HARD_CLOSED
        - TERMINATED
      initial_state: DRAFT
      
    implicit_create:
      allowed: true
      canonical_verb: entity.create-share-class
      required_args: [subfund-id, share-class-code, currency, investor-type]

  # ===========================================================================
  # Management Company
  # ===========================================================================
  management_company:
    description: "Management Company or AIFM"
    category: service_provider
    parent_type: entity
    
    db:
      schema: ob-poc
      table: entities
      pk: entity_id
      extension_table: entity_manco
      extension_fk: entity_id
      type_code: management_company
      
    search_keys:
      - column: name
        unique: false
      - column: lei
        unique: true
      - column: regulatory_reference
        unique: true
        
    lifecycle:
      status_column: status
      states:
        - DRAFT
        - AUTHORIZED
        - ACTIVE
        - SUSPENDED
        - WITHDRAWN
      initial_state: DRAFT
      
    implicit_create:
      allowed: true
      canonical_verb: entity.create-manco
      required_args: [name, jurisdiction, manco-type]
```

---

## Phase 3: Verb YAML Definitions

### 3.1 Create `rust/config/verbs/fund.yaml`

```yaml
domains:
  fund:
    description: Fund structure management operations
    verbs:
    
      create-umbrella:
        description: Create an umbrella fund (SICAV, ICAV, OEIC)
        behavior: crud
        produces:
          type: entity
          subtype: fund_umbrella
          resolved: false
        crud:
          operation: entity_create
          base_table: entities
          schema: ob-poc
          extension_table: entity_funds
          type_code: fund_umbrella
          returning: entity_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: jurisdiction
            type: string
            required: true
            maps_to: domicile_jurisdiction
            lookup:
              table: master_jurisdictions
              entity_type: jurisdiction
              schema: ob-poc
              search_key: jurisdiction_code
              primary_key: jurisdiction_code
          - name: legal-form
            type: string
            required: true
            maps_to: legal_form
            validation:
              enum: [SICAV, ICAV, FCP, OEIC, VCC]
          - name: fund-type
            type: string
            required: false
            maps_to: fund_type
            validation:
              enum: [UCITS, AIF, PRIVATE]
          - name: lei
            type: string
            required: false
            maps_to: lei
          - name: regulator
            type: string
            required: false
            maps_to: regulator
          - name: authorization-date
            type: date
            required: false
            maps_to: authorization_date
        returns:
          type: uuid
          name: entity_id
          capture: true

      create-subfund:
        description: Create a sub-fund within an umbrella
        behavior: crud
        produces:
          type: entity
          subtype: fund_subfund
          resolved: false
        consumes:
          - arg: umbrella-id
            type: entity
            subtype: fund_umbrella
            required: true
        crud:
          operation: entity_create
          base_table: entities
          schema: ob-poc
          extension_table: entity_funds
          type_code: fund_subfund
          returning: entity_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: umbrella-id
            type: uuid
            required: true
            maps_to: umbrella_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
              filter:
                type_code: fund_umbrella
          - name: base-currency
            type: string
            required: true
            maps_to: base_currency
          - name: isin-base
            type: string
            required: false
            maps_to: isin_base
          - name: compartment-number
            type: integer
            required: false
            maps_to: compartment_number
          - name: investment-objective
            type: string
            required: false
            maps_to: investment_objective
          - name: inception-date
            type: date
            required: false
            maps_to: inception_date
        returns:
          type: uuid
          name: entity_id
          capture: true

      create-share-class:
        description: Create a share class within a sub-fund
        behavior: crud
        produces:
          type: entity
          subtype: fund_share_class
          resolved: false
        consumes:
          - arg: subfund-id
            type: entity
            subtype: fund_subfund
            required: true
        crud:
          operation: entity_create
          base_table: entities
          schema: ob-poc
          extension_table: entity_share_classes
          type_code: fund_share_class
          returning: entity_id
        args:
          - name: subfund-id
            type: uuid
            required: true
            maps_to: subfund_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
              filter:
                type_code: fund_subfund
          - name: share-class-code
            type: string
            required: true
            maps_to: share_class_code
          - name: currency
            type: string
            required: true
            maps_to: currency
          - name: investor-type
            type: string
            required: true
            maps_to: investor_type
            validation:
              enum: [INSTITUTIONAL, RETAIL, PRIVATE]
          - name: isin
            type: string
            required: false
            maps_to: isin
          - name: hedged
            type: boolean
            required: false
            maps_to: hedged
            default: false
          - name: distributing
            type: boolean
            required: false
            maps_to: distributing
            default: false
          - name: management-fee-bps
            type: integer
            required: false
            maps_to: management_fee_bps
        returns:
          type: uuid
          name: entity_id
          capture: true

      link-feeder-to-master:
        description: Establish master-feeder relationship
        behavior: crud
        crud:
          operation: insert
          table: fund_structure_relationships
          schema: ob-poc
          returning: relationship_id
        args:
          - name: feeder-id
            type: uuid
            required: true
            maps_to: child_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: master-id
            type: uuid
            required: true
            maps_to: parent_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: effective-from
            type: date
            required: false
            maps_to: effective_from
        static_values:
          relationship_type: MASTER_HAS_FEEDER
        returns:
          type: uuid
          name: relationship_id

      list-subfunds:
        description: List all sub-funds for an umbrella
        behavior: crud
        crud:
          operation: list_by_fk
          table: entity_funds
          schema: ob-poc
          fk_col: umbrella_entity_id
        args:
          - name: umbrella-id
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
              filter:
                type_code: fund_umbrella
        returns:
          type: record_set

      list-share-classes:
        description: List all share classes for a sub-fund
        behavior: crud
        crud:
          operation: list_by_fk
          table: entity_share_classes
          schema: ob-poc
          fk_col: subfund_entity_id
        args:
          - name: subfund-id
            type: uuid
            required: true
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record_set

      assign-manco:
        description: Assign management company to a fund
        behavior: crud
        crud:
          operation: role_link
          junction: cbu_entity_roles
          schema: ob-poc
          from_col: cbu_id
          to_col: entity_id
          role_table: roles
          returning: cbu_entity_role_id
        args:
          - name: fund-cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: manco-entity-id
            type: uuid
            required: true
            maps_to: entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: effective-from
            type: date
            required: false
            maps_to: effective_from
        static_values:
          role: MANAGEMENT_COMPANY
        returns:
          type: uuid
          name: cbu_entity_role_id

      assign-depositary:
        description: Assign depositary to a fund
        behavior: crud
        crud:
          operation: role_link
          junction: cbu_entity_roles
          schema: ob-poc
          from_col: cbu_id
          to_col: entity_id
          role_table: roles
          returning: cbu_entity_role_id
        args:
          - name: fund-cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: depositary-entity-id
            type: uuid
            required: true
            maps_to: entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        static_values:
          role: DEPOSITARY
        returns:
          type: uuid
          name: cbu_entity_role_id
```

### 3.2 Create `rust/config/verbs/control.yaml`

```yaml
domains:
  control:
    description: Control relationship management (distinct from ownership)
    verbs:
    
      add:
        description: Add a control relationship between entities
        behavior: crud
        crud:
          operation: insert
          table: control_relationships
          schema: ob-poc
          returning: control_id
        args:
          - name: controller-entity-id
            type: uuid
            required: true
            maps_to: controller_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: controlled-entity-id
            type: uuid
            required: true
            maps_to: controlled_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: control-type
            type: string
            required: true
            maps_to: control_type
            validation:
              enum:
                - VOTING_RIGHTS
                - BOARD_APPOINTMENT
                - VETO_POWER
                - MANAGEMENT_CONTROL
                - RESERVED_MATTERS
          - name: control-percentage
            type: decimal
            required: false
            maps_to: control_percentage
          - name: description
            type: string
            required: false
            maps_to: control_description
          - name: effective-from
            type: date
            required: false
            maps_to: effective_from
          - name: evidence-doc-id
            type: uuid
            required: false
            maps_to: evidence_doc_id
            lookup:
              table: document_catalog
              entity_type: document
              schema: ob-poc
              search_key: document_name
              primary_key: doc_id
        returns:
          type: uuid
          name: control_id
          capture: true

      end:
        description: End a control relationship
        behavior: crud
        crud:
          operation: update
          table: control_relationships
          schema: ob-poc
          key: control_id
        args:
          - name: control-id
            type: uuid
            required: true
            maps_to: control_id
          - name: effective-to
            type: date
            required: true
            maps_to: effective_to
        returns:
          type: affected

      list-controllers:
        description: List who controls an entity
        behavior: crud
        crud:
          operation: list_by_fk
          table: control_relationships
          schema: ob-poc
          fk_col: controlled_entity_id
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: controlled_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record_set

      list-controlled:
        description: List what an entity controls
        behavior: crud
        crud:
          operation: list_by_fk
          table: control_relationships
          schema: ob-poc
          fk_col: controller_entity_id
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: controller_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record_set
```

### 3.3 Create `rust/config/verbs/delegation.yaml`

```yaml
domains:
  delegation:
    description: Service provider delegation chain management
    verbs:
    
      add:
        description: Add a delegation relationship
        behavior: crud
        crud:
          operation: insert
          table: delegation_relationships
          schema: ob-poc
          returning: delegation_id
        args:
          - name: delegator-entity-id
            type: uuid
            required: true
            maps_to: delegator_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: delegate-entity-id
            type: uuid
            required: true
            maps_to: delegate_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: scope
            type: string
            required: true
            maps_to: delegation_scope
            validation:
              enum:
                - INVESTMENT_MANAGEMENT
                - RISK_MANAGEMENT
                - PORTFOLIO_ADMINISTRATION
                - DISTRIBUTION
                - TRANSFER_AGENCY
          - name: description
            type: string
            required: false
            maps_to: delegation_description
          - name: applies-to-cbu-id
            type: uuid
            required: false
            maps_to: applies_to_cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: regulatory-notification-date
            type: date
            required: false
            maps_to: regulatory_notification_date
          - name: effective-from
            type: date
            required: true
            maps_to: effective_from
          - name: contract-doc-id
            type: uuid
            required: false
            maps_to: contract_doc_id
            lookup:
              table: document_catalog
              entity_type: document
              schema: ob-poc
              search_key: document_name
              primary_key: doc_id
        returns:
          type: uuid
          name: delegation_id
          capture: true

      end:
        description: End a delegation
        behavior: crud
        crud:
          operation: update
          table: delegation_relationships
          schema: ob-poc
          key: delegation_id
        args:
          - name: delegation-id
            type: uuid
            required: true
            maps_to: delegation_id
          - name: effective-to
            type: date
            required: true
            maps_to: effective_to
        returns:
          type: affected

      list-delegates:
        description: List who an entity delegates to
        behavior: crud
        crud:
          operation: list_by_fk
          table: delegation_relationships
          schema: ob-poc
          fk_col: delegator_entity_id
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: delegator_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record_set

      list-delegations-received:
        description: List delegations an entity has received
        behavior: crud
        crud:
          operation: list_by_fk
          table: delegation_relationships
          schema: ob-poc
          fk_col: delegate_entity_id
        args:
          - name: entity-id
            type: uuid
            required: true
            maps_to: delegate_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record_set
```

### 3.4 Create `rust/config/verbs/investment.yaml`

```yaml
domains:
  investment:
    description: Fund-to-fund investment relationships (for FoF look-through)
    verbs:
    
      add:
        description: Record fund investment in another fund
        behavior: crud
        crud:
          operation: insert
          table: fund_investment_relationships
          schema: ob-poc
          returning: investment_id
        args:
          - name: investor-fund-id
            type: uuid
            required: true
            maps_to: investor_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: investee-fund-id
            type: uuid
            required: true
            maps_to: investee_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: percentage
            type: decimal
            required: true
            maps_to: investment_percentage
            description: Percentage of investor fund NAV
          - name: investment-type
            type: string
            required: true
            maps_to: investment_type
            validation:
              enum: [DIRECT, INDIRECT]
          - name: invested-amount
            type: decimal
            required: false
            maps_to: invested_amount
          - name: currency
            type: string
            required: false
            maps_to: invested_currency
          - name: investment-date
            type: date
            required: false
            maps_to: investment_date
        returns:
          type: uuid
          name: investment_id
          capture: true

      update:
        description: Update investment percentage (rebalancing)
        behavior: crud
        crud:
          operation: update
          table: fund_investment_relationships
          schema: ob-poc
          key: investment_id
        args:
          - name: investment-id
            type: uuid
            required: true
            maps_to: investment_id
          - name: percentage
            type: decimal
            required: false
            maps_to: investment_percentage
          - name: invested-amount
            type: decimal
            required: false
            maps_to: invested_amount
          - name: valuation-date
            type: date
            required: false
            maps_to: valuation_date
        returns:
          type: affected

      redeem:
        description: Record full redemption from underlying fund
        behavior: crud
        crud:
          operation: update
          table: fund_investment_relationships
          schema: ob-poc
          key: investment_id
        args:
          - name: investment-id
            type: uuid
            required: true
            maps_to: investment_id
          - name: redemption-date
            type: date
            required: true
            maps_to: redemption_date
        returns:
          type: affected

      list-holdings:
        description: List what funds an investor fund holds
        behavior: crud
        crud:
          operation: list_by_fk
          table: fund_investment_relationships
          schema: ob-poc
          fk_col: investor_entity_id
        args:
          - name: investor-fund-id
            type: uuid
            required: true
            maps_to: investor_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record_set

      list-investors:
        description: List fund-of-funds that invest in this fund
        behavior: crud
        crud:
          operation: list_by_fk
          table: fund_investment_relationships
          schema: ob-poc
          fk_col: investee_entity_id
        args:
          - name: investee-fund-id
            type: uuid
            required: true
            maps_to: investee_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record_set

      calculate-look-through:
        description: Calculate look-through ownership for FoF to underlying assets
        behavior: plugin
        plugin:
          handler: InvestmentLookThroughOp
        args:
          - name: investor-fund-id
            type: uuid
            required: true
            description: The Fund-of-Funds to calculate look-through for
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: target-entity-id
            type: uuid
            required: false
            description: Specific underlying entity (if null, calculates all)
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: threshold
            type: decimal
            required: false
            default: 0.0
            description: Minimum ownership % to include in results
        returns:
          type: record_set
          description: List of effective ownership percentages through all paths
```

---

## Phase 4: Plugin Implementations

### 4.1 Look-Through Calculator

Create `rust/src/dsl_v2/custom_ops/investment_look_through.rs`:

```rust
//! Investment Look-Through Calculator
//! 
//! Calculates effective ownership through Fund-of-Funds chains.
//! Handles:
//! - Multi-path aggregation (FoF owns Fund A and Fund B, both own Corp X)
//! - Circular detection (rare but legal in some jurisdictions)
//! - Threshold filtering (>10%, >25%, >50%)

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct LookThroughResult {
    pub target_entity_id: Uuid,
    pub target_name: String,
    pub effective_percentage: f64,
    pub paths: Vec<OwnershipPath>,
}

pub struct OwnershipPath {
    pub entities: Vec<Uuid>,
    pub percentage: f64,
}

/// Calculate look-through ownership from investor to all underlying entities
pub async fn calculate_look_through(
    pool: &PgPool,
    investor_fund_id: Uuid,
    target_entity_id: Option<Uuid>,
    threshold: f64,
) -> Result<Vec<LookThroughResult>, Error> {
    // BFS/DFS through investment + ownership relationships
    // Aggregate percentages by target entity
    // Handle cycles with visited set
    todo!()
}
```

---

## Phase 5: Graph Visualization Updates

### 5.1 New Node Types for egui Graph

Add to graph rendering logic:

```rust
pub enum FundNodeType {
    Umbrella,      // Large container node
    SubFund,       // Medium node inside umbrella
    ShareClass,    // Small node inside subfund
    Master,        // Diamond shape
    Feeder,        // Arrow pointing to master
    ManCo,         // Hexagon (service provider)
}

impl FundNodeType {
    pub fn color(&self) -> Color32 {
        match self {
            Self::Umbrella => Color32::from_rgb(70, 130, 180),    // Steel blue
            Self::SubFund => Color32::from_rgb(100, 149, 237),   // Cornflower
            Self::ShareClass => Color32::from_rgb(176, 196, 222), // Light steel
            Self::Master => Color32::from_rgb(255, 215, 0),      // Gold
            Self::Feeder => Color32::from_rgb(255, 165, 0),      // Orange
            Self::ManCo => Color32::from_rgb(147, 112, 219),     // Purple
        }
    }
}
```

### 5.2 New Edge Types

```rust
pub enum FundEdgeType {
    Ownership(f64),        // Solid, labeled with %
    Control,               // Dashed, different color
    Containment,           // Dotted (umbrella→subfund)
    MasterFeeder,          // Thick arrow
    Delegation,            // Thin dashed
    Investment(f64),       // Labeled with %
}
```

### 5.3 Layout Hints

```rust
pub enum LayoutHint {
    // Containment creates visual grouping
    ContainedBy(Uuid),     // Subfund contained by umbrella
    
    // Hierarchy flows top-down
    OwnerAbove,            // Owners above owned entities
    
    // Service providers on periphery
    ServiceProvider,       // Place on edge of graph
    
    // Master-feeder alignment
    MasterCenter,          // Master in center
    FeedersRadial,         // Feeders arranged around master
}
```

---

## Implementation Order

1. **Database migrations first** (Phase 1)
   - Run schema creation SQL
   - Add seed data for entity_types and roles
   
2. **Entity taxonomy updates** (Phase 2)
   - Update entity_taxonomy.yaml
   - Verify loader parses correctly
   
3. **Verb YAML files** (Phase 3)
   - Create fund.yaml, control.yaml, delegation.yaml, investment.yaml
   - Verify verbs appear in registry
   
4. **Basic CRUD operations** (test)
   - Create umbrella, subfund, share class
   - Test containment relationships
   
5. **Plugin implementations** (Phase 4)
   - Look-through calculator
   
6. **Graph visualization** (Phase 5)
   - After loading real Allianz data

---

## Test Cases

### Fund Structure Creation

```dsl
# Create umbrella
fund.create-umbrella name="Allianz Global Investors Fund" 
                     jurisdiction="LU" 
                     legal-form="SICAV"
                     fund-type="UCITS"
  -> $umbrella

# Create sub-fund
fund.create-subfund name="AGI Dynamic Multi Asset Plus"
                    umbrella-id=$umbrella
                    base-currency="EUR"
                    compartment-number=1
  -> $subfund

# Create share classes
fund.create-share-class subfund-id=$subfund
                        share-class-code="I"
                        currency="EUR"
                        investor-type="INSTITUTIONAL"
                        isin="LU1234567890"
  -> $class_i

fund.create-share-class subfund-id=$subfund
                        share-class-code="A"
                        currency="EUR"  
                        investor-type="RETAIL"
                        distributing=true
  -> $class_a
```

### Service Provider Assignment

```dsl
# Assign ManCo
fund.assign-manco fund-cbu-id=$fund_cbu
                  manco-entity-id=$agi_manco
                  effective-from="2020-01-01"

# Assign Depositary
fund.assign-depositary fund-cbu-id=$fund_cbu
                       depositary-entity-id=$state_street
```

### Control vs Ownership

```dsl
# 30% ownership but 51% voting control
ubo.add-ownership owner-entity-id=$holding_co
                  owned-entity-id=$subsidiary
                  percentage=30.0
                  ownership-type="DIRECT"

control.add controller-entity-id=$holding_co
            controlled-entity-id=$subsidiary
            control-type="VOTING_RIGHTS"
            control-percentage=51.0
            description="Dual class share structure"
```

### Fund-of-Funds Look-Through

```dsl
# FoF invests in underlying funds
investment.add investor-fund-id=$fof
               investee-fund-id=$underlying_a
               percentage=15.0
               investment-type="DIRECT"

investment.add investor-fund-id=$fof
               investee-fund-id=$underlying_b
               percentage=25.0
               investment-type="DIRECT"

# Calculate look-through to Corp X
investment.calculate-look-through investor-fund-id=$fof
                                   target-entity-id=$corp_x
                                   threshold=1.0
```
