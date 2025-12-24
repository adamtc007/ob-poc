# TODO: KYC Case Builder Extension

## Overview

Extend the KYC subsystem to support intelligent, context-aware case scoping based on:
- **CBU Type** (Fund, Corporate, Individual, etc.)
- **Service Context** (Custody, TA, Broker-Dealer, KYC-as-a-Service)
- **Role Types** (Account Holder, UBO, ManCo, Director, etc.)
- **Entity Regulatory Profile** (Is the entity regulated? By whom?)
- **Product Risk Ratings** (High/Medium/Low risk products)
- **Sponsor Relationships** (For KYC-as-a-Service)

Goal: Agent can inspect a CBU and automatically derive correct KYC scope without accidentally trying to KYC all of Allianz.

---

## Phase 1: Reference Data & Schema

### 1.1 Role Types Configuration

**File:** `config/ontology/role_types.yaml`

```yaml
role_types:
  ACCOUNT_HOLDER:
    name: "Account Holder"
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: true
    check_regulatory_status: false
    cascade_to_entity_ubos: true
    
  UBO:
    name: "Ultimate Beneficial Owner"
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: true
    check_regulatory_status: false
    cascade_to_entity_ubos: false
    
  CONTROLLER:
    name: "Controller"
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: true
    check_regulatory_status: false
    
  MANCO:
    name: "Management Company"
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: false
    check_regulatory_status: true
    if_regulated_obligation: SIMPLIFIED
    
  INVESTMENT_MGR:
    name: "Investment Manager"
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: false
    check_regulatory_status: true
    if_regulated_obligation: SIMPLIFIED
    
  DIRECTOR:
    name: "Director"
    triggers_full_kyc: false
    triggers_screening: true
    triggers_id_verification: true
    
  SIGNATORY:
    name: "Authorized Signatory"
    triggers_full_kyc: false
    triggers_screening: true
    triggers_id_verification: true
    
  DELEGATE:
    name: "Delegate/Service Provider"
    triggers_full_kyc: false
    triggers_screening: true
    triggers_id_verification: false
    check_regulatory_status: true
    if_regulated_obligation: RECORD_ONLY
    
  INVESTOR:
    name: "Fund Investor"
    triggers_full_kyc: true
    triggers_screening: true
    triggers_id_verification: true
    check_regulatory_status: true
    if_regulated_obligation: SIMPLIFIED
    threshold_based: true
```

**Tasks:**
- [ ] Create `role_types.yaml` config file
- [ ] Create `role_types` reference table in database
- [ ] Seed role types from YAML on startup
- [ ] Add role_type validation to `cbu_entity_roles` table

---

### 1.2 Regulators Reference Data

**File:** `config/ontology/regulators.yaml`

```yaml
regulators:
  FCA:
    name: "Financial Conduct Authority"
    jurisdiction: GB
    tier: EQUIVALENT
    registry_url: "https://register.fca.org.uk/s/"
    
  CSSF:
    name: "Commission de Surveillance du Secteur Financier"
    jurisdiction: LU
    tier: EQUIVALENT
    
  CBI:
    name: "Central Bank of Ireland"
    jurisdiction: IE
    tier: EQUIVALENT
    
  SEC:
    name: "Securities and Exchange Commission"
    jurisdiction: US
    tier: EQUIVALENT
    
  BaFin:
    name: "Bundesanstalt für Finanzdienstleistungsaufsicht"
    jurisdiction: DE
    tier: EQUIVALENT
    
  FINMA:
    name: "Swiss Financial Market Supervisory Authority"
    jurisdiction: CH
    tier: EQUIVALENT
    
  MAS:
    name: "Monetary Authority of Singapore"
    jurisdiction: SG
    tier: EQUIVALENT
    
  SFC:
    name: "Securities and Futures Commission"
    jurisdiction: HK
    tier: EQUIVALENT
    
  ASIC:
    name: "Australian Securities and Investments Commission"
    jurisdiction: AU
    tier: EQUIVALENT

regulatory_tiers:
  EQUIVALENT:
    description: "Full reliance permitted"
    allows_simplified_dd: true
    
  ACCEPTABLE:
    description: "Partial reliance, enhanced checks"
    allows_simplified_dd: true
    requires_enhanced_screening: true
    
  NONE:
    description: "No reliance, full KYC required"
    allows_simplified_dd: false
```

**Tasks:**
- [ ] Create `regulators.yaml` config file
- [ ] Create `regulators` reference table
- [ ] Create `regulatory_tiers` reference table
- [ ] Seed from YAML on startup

---

### 1.3 Entity Regulatory Profile

**Migration:** `V0XX__add_entity_regulatory_profile.sql`

```sql
CREATE TABLE entity_regulatory_profiles (
    entity_id UUID PRIMARY KEY REFERENCES entities(entity_id),
    is_regulated BOOLEAN DEFAULT FALSE,
    regulator_code VARCHAR(50) REFERENCES regulators(regulator_code),
    registration_number VARCHAR(100),
    registration_verified BOOLEAN DEFAULT FALSE,
    verification_date DATE,
    verification_method VARCHAR(50),
    verification_reference VARCHAR(500),
    regulatory_tier VARCHAR(50) DEFAULT 'NONE',
    next_verification_due DATE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_entity_reg_profile_regulated ON entity_regulatory_profiles(is_regulated);
CREATE INDEX idx_entity_reg_profile_regulator ON entity_regulatory_profiles(regulator_code);
```

**Tasks:**
- [ ] Create migration file
- [ ] Run migration
- [ ] Add `regulatory-profile` DSL verbs (set, verify, check)

---

### 1.4 Product Risk Ratings

**File:** `config/ontology/product_kyc_config.yaml`

```yaml
products:
  CUSTODY:
    kyc_risk_rating: HIGH
    requires_kyc: true
    kyc_context: CUSTODY
    
  PRIME_BROKERAGE:
    kyc_risk_rating: HIGH
    requires_kyc: true
    kyc_context: CUSTODY
    
  DERIVATIVES_CLEARING:
    kyc_risk_rating: HIGH
    requires_kyc: true
    kyc_context: CUSTODY
    
  FUND_ACCOUNTING:
    kyc_risk_rating: MEDIUM
    requires_kyc: true
    kyc_context: CUSTODY
    
  TRANSFER_AGENCY:
    kyc_risk_rating: MEDIUM
    requires_kyc: true
    kyc_context: TRANSFER_AGENT
    includes_investor_kyc: true
    
  KYC_AS_A_SERVICE:
    kyc_risk_rating: MEDIUM
    requires_kyc: true
    kyc_context: KYC_AS_A_SERVICE
    requires_sponsor: true
    requires_service_agreement: true
    
  REPORTING_ONLY:
    kyc_risk_rating: LOW
    requires_kyc: false
    
  SECURITIES_LENDING:
    kyc_risk_rating: MEDIUM
    requires_kyc: true
    kyc_context: CUSTODY
```

**Tasks:**
- [ ] Create `product_kyc_config.yaml`
- [ ] Add `kyc_risk_rating` column to `products` table
- [ ] Add `kyc_context` column to `products` table
- [ ] Update product seeding to include KYC config

---

### 1.5 CBU Type Scope Templates

**File:** `config/ontology/kyc_scope_templates.yaml`

```yaml
scope_templates:

  FUND:
    description: "Pooled investment fund"
    account_holder_is: FUND_ENTITY
    ubo_rules:
      applies: conditional
      threshold_pct: 25
      note: "Funds rarely have >25% owners"
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
      DELEGATE: CHECK_REGULATORY

  HEDGE_FUND:
    inherits: FUND
    risk_floor: MEDIUM
    role_obligations:
      CONTROLLER: FULL_KYC_ENHANCED
      INVESTMENT_MGR: FULL_KYC
    additional_checks:
      - STRATEGY_RISK_ASSESSMENT
      - INVESTOR_CONCENTRATION

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
    description: "Natural person - retail client"
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
    description: "Retail investor in fund (TA context)"
    service_context: TRANSFER_AGENT
    kyc_on_behalf_of: FUND
    ubo_rules:
      applies: false
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
    description: "Institutional investor in fund (TA context)"
    service_context: TRANSFER_AGENT
    kyc_on_behalf_of: FUND
    ubo_rules:
      applies: conditional
      threshold_pct: 25
    role_obligations:
      ACCOUNT_HOLDER: CHECK_REGULATORY
      UBO: FULL_KYC
      SIGNATORY: SCREEN_AND_ID
```

**Tasks:**
- [ ] Create `kyc_scope_templates.yaml`
- [ ] Create Rust struct to parse/hold scope templates
- [ ] Load templates on startup
- [ ] Add `kyc_scope_template` column to `cbus` table

---

## Phase 2: Service Context & Sponsor Support

### 2.1 Service Context Schema

**Migration:** `V0XX__add_service_context.sql`

```sql
-- Service contexts for a CBU
CREATE TABLE cbu_service_contexts (
    cbu_id UUID REFERENCES cbus(cbu_id),
    service_context VARCHAR(50) NOT NULL,
    effective_date DATE DEFAULT CURRENT_DATE,
    PRIMARY KEY (cbu_id, service_context)
);

-- KYC service agreements (for KYC-as-a-Service)
CREATE TABLE kyc_service_agreements (
    agreement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sponsor_cbu_id UUID NOT NULL REFERENCES cbus(cbu_id),
    sponsor_entity_id UUID REFERENCES entities(entity_id),
    agreement_reference VARCHAR(100),
    effective_date DATE NOT NULL,
    termination_date DATE,
    kyc_standard VARCHAR(50) NOT NULL,
    auto_accept_threshold VARCHAR(50),
    sponsor_review_required BOOLEAN DEFAULT TRUE,
    target_turnaround_days INTEGER DEFAULT 5,
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_kyc_agreement_sponsor ON kyc_service_agreements(sponsor_cbu_id);

-- Extend KYC cases for sponsor context
ALTER TABLE kyc_cases ADD COLUMN service_context VARCHAR(50);
ALTER TABLE kyc_cases ADD COLUMN sponsor_cbu_id UUID REFERENCES cbus(cbu_id);
ALTER TABLE kyc_cases ADD COLUMN service_agreement_id UUID REFERENCES kyc_service_agreements(agreement_id);
ALTER TABLE kyc_cases ADD COLUMN kyc_standard VARCHAR(50);
ALTER TABLE kyc_cases ADD COLUMN subject_entity_id UUID REFERENCES entities(entity_id);

-- Sponsor decision tracking
CREATE TABLE kyc_case_sponsor_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc_cases(case_id),
    our_recommendation VARCHAR(50),
    our_recommendation_date TIMESTAMP,
    our_recommendation_by UUID,
    our_findings JSONB,
    sponsor_decision VARCHAR(50),
    sponsor_decision_date TIMESTAMP,
    sponsor_decision_by VARCHAR(255),
    sponsor_comments TEXT,
    final_status VARCHAR(50),
    effective_date DATE,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_sponsor_decision_case ON kyc_case_sponsor_decisions(case_id);
```

**Tasks:**
- [ ] Create migration file
- [ ] Run migration
- [ ] Update KYC case Rust structs

---

### 2.2 Fund Investor Table (TA Context)

**Migration:** `V0XX__add_fund_investors.sql`

```sql
CREATE TABLE fund_investors (
    investor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fund_cbu_id UUID NOT NULL REFERENCES cbus(cbu_id),
    investor_entity_id UUID NOT NULL REFERENCES entities(entity_id),
    investor_type VARCHAR(50) NOT NULL,
    investment_amount DECIMAL(20,2),
    currency VARCHAR(3) DEFAULT 'EUR',
    subscription_date DATE,
    kyc_tier VARCHAR(50),
    kyc_status VARCHAR(50) DEFAULT 'PENDING',
    kyc_case_id UUID REFERENCES kyc_cases(case_id),
    last_kyc_date DATE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    CONSTRAINT uq_fund_investor UNIQUE (fund_cbu_id, investor_entity_id)
);

CREATE INDEX idx_fund_investor_fund ON fund_investors(fund_cbu_id);
CREATE INDEX idx_fund_investor_entity ON fund_investors(investor_entity_id);
CREATE INDEX idx_fund_investor_status ON fund_investors(kyc_status);
```

**Tasks:**
- [ ] Create migration file
- [ ] Run migration
- [ ] Add `fund-investor` DSL verbs

---

## Phase 3: DSL Verbs

### 3.1 Regulatory Profile Verbs

**File:** `config/verbs/regulatory.yaml`

```yaml
domain: regulatory

regulatory-profile:
  set:
    description: "Set or update regulatory profile for an entity"
    behavior: crud
    crud:
      operation: upsert
      table: entity_regulatory_profiles
      schema: ob_kyc
    args:
      - name: entity-id
        type: uuid
        required: true
        lookup:
          entity_type: entity
      - name: is-regulated
        type: boolean
        required: true
      - name: regulator
        type: string
        required: false
        lookup:
          table: regulators
          search_key: regulator_code
      - name: registration-number
        type: string
        required: false
      - name: verified
        type: boolean
        required: false
        default: false

  verify:
    description: "Mark regulatory registration as verified"
    behavior: plugin
    plugin:
      handler: RegulatoryVerifyOp
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: verification-method
        type: string
        required: true
        enum: [MANUAL, REGISTRY_API, DOCUMENT]
      - name: reference
        type: string
        required: false
        description: "Evidence reference or URL"

  check:
    description: "Check if entity has valid regulatory status"
    behavior: plugin
    plugin:
      handler: RegulatoryCheckOp
    args:
      - name: entity-id
        type: uuid
        required: true
    returns:
      type: object
      fields:
        is_regulated: boolean
        regulator: string
        registration_verified: boolean
        tier: string
        allows_simplified_dd: boolean
```

**Tasks:**
- [ ] Create `regulatory.yaml` verb file
- [ ] Register in verb loader
- [ ] Implement `RegulatoryVerifyOp` plugin
- [ ] Implement `RegulatoryCheckOp` plugin

---

### 3.2 KYC Scope Preview Verb

**File:** `config/verbs/kyc.yaml` (additions)

```yaml
preview-scope:
  description: "Preview KYC scope for a CBU - shows entities and obligations"
  behavior: plugin
  plugin:
    handler: KycPreviewScopeOp
  args:
    - name: cbu-id
      type: uuid
      required: true
      lookup:
        entity_type: cbu
    - name: service-context
      type: string
      required: false
      enum: [CUSTODY, TRANSFER_AGENT, BROKER_DEALER, KYC_AS_A_SERVICE]
    - name: products
      type: string[]
      required: false
      description: "Products being onboarded (for risk rating)"
  returns:
    type: object
    description: |
      {
        "cbu_type": "FUND",
        "service_context": "CUSTODY",
        "case_risk_rating": "HIGH",
        "risk_drivers": ["CUSTODY is HIGH risk"],
        "scope_template": "FUND",
        "entities": [
          {
            "entity_id": "uuid",
            "entity_name": "Alpha Fund Ltd",
            "entity_type": "FUND",
            "role": "ACCOUNT_HOLDER",
            "is_regulated": false,
            "regulator": null,
            "kyc_obligation": "FULL_KYC",
            "obligation_reason": "Account holder always requires full KYC",
            "existing_kyc_status": "NONE",
            "last_verified": null
          }
        ],
        "summary": {
          "full_kyc_count": 2,
          "simplified_count": 2,
          "screen_and_id_count": 2,
          "record_only_count": 0
        }
      }
```

**Tasks:**
- [ ] Add `preview-scope` to `kyc.yaml`
- [ ] Implement `KycPreviewScopeOp` plugin:
  - [ ] Load CBU and entities with roles
  - [ ] Load scope template for CBU type
  - [ ] Load regulatory profiles for all entities
  - [ ] Apply role × regulatory matrix
  - [ ] Return structured preview

---

### 3.3 Enhanced KYC Case Creation

**File:** `config/verbs/kyc.yaml` (updates)

```yaml
create:
  description: "Create a KYC case"
  behavior: plugin
  plugin:
    handler: KycCaseCreateOp
  args:
    - name: cbu-id
      type: uuid
      required: true
      
    - name: case-type
      type: string
      required: true
      enum: [NEW_CLIENT, PERIODIC_REVIEW, TRIGGER_EVENT, PRODUCT_CHANGE, INVESTOR_SUBSCRIPTION]
      
    - name: service-context
      type: string
      required: false
      enum: [CUSTODY, TRANSFER_AGENT, BROKER_DEALER, KYC_AS_A_SERVICE]
      
    - name: sponsor-cbu-id
      type: uuid
      required: false
      description: "For KYC-aaS: the sponsor with regulatory obligation"
      
    - name: service-agreement-id
      type: uuid
      required: false
      description: "For KYC-aaS: governing service agreement"
      
    - name: subject-entity-id
      type: uuid
      required: false
      description: "For investor KYC: the entity being KYC'd"
      
    - name: risk-rating
      type: string
      required: false
      enum: [LOW, MEDIUM, HIGH]
      description: "Override risk rating"
      
    - name: trigger
      type: string
      required: false
      
    - name: auto-scope
      type: boolean
      required: false
      default: false
      description: "Automatically create workstreams from scope rules"
```

**Tasks:**
- [ ] Update `KycCaseCreateOp` to handle new fields
- [ ] Implement auto-scope logic using preview-scope internally
- [ ] Validate sponsor/agreement for KYC-aaS context

---

### 3.4 Sponsor Decision Verbs

**File:** `config/verbs/kyc.yaml` (additions)

```yaml
recommend:
  description: "Record our KYC recommendation (for KYC-aaS cases)"
  behavior: plugin
  plugin:
    handler: KycRecommendOp
  args:
    - name: case-id
      type: uuid
      required: true
    - name: recommendation
      type: string
      required: true
      enum: [APPROVE, REJECT, REFER]
    - name: risk-rating
      type: string
      required: true
      enum: [LOW, MEDIUM, HIGH]
    - name: findings
      type: object
      required: false
    - name: notes
      type: string
      required: false

sponsor-decision:
  description: "Record sponsor's decision on KYC case"
  behavior: plugin
  plugin:
    handler: KycSponsorDecisionOp
  args:
    - name: case-id
      type: uuid
      required: true
    - name: decision
      type: string
      required: true
      enum: [ACCEPTED, REJECTED, ESCALATED, DEFERRED]
    - name: decided-by
      type: string
      required: true
    - name: comments
      type: string
      required: false
```

**Tasks:**
- [ ] Add verbs to `kyc.yaml`
- [ ] Implement `KycRecommendOp`:
  - [ ] Validate case is KYC-aaS context
  - [ ] Record recommendation
  - [ ] Check auto-accept threshold
  - [ ] Auto-close if within threshold
- [ ] Implement `KycSponsorDecisionOp`:
  - [ ] Record sponsor decision
  - [ ] Update case status
  - [ ] Trigger downstream (investor cleared, etc.)

---

### 3.5 Service Agreement Verbs

**File:** `config/verbs/kyc-agreement.yaml`

```yaml
domain: kyc-agreement

create:
  description: "Create KYC service agreement with sponsor"
  behavior: crud
  crud:
    operation: insert
    table: kyc_service_agreements
    schema: ob_kyc
  args:
    - name: sponsor-cbu-id
      type: uuid
      required: true
    - name: agreement-reference
      type: string
      required: true
    - name: kyc-standard
      type: string
      required: true
      enum: [SPONSOR_STANDARD, BNY_STANDARD, REGULATORY_MINIMUM, ENHANCED]
    - name: auto-accept-threshold
      type: string
      required: false
      enum: [LOW, MEDIUM, null]
    - name: sponsor-review-required
      type: boolean
      default: true
    - name: effective-date
      type: date
      required: true

read:
  description: "Get service agreement"
  behavior: crud
  crud:
    operation: select
    table: kyc_service_agreements
  args:
    - name: agreement-id
      type: uuid
      required: false
    - name: sponsor-cbu-id
      type: uuid
      required: false
```

**Tasks:**
- [ ] Create `kyc-agreement.yaml` verb file
- [ ] Register in verb loader

---

### 3.6 Fund Investor Verbs

**File:** `config/verbs/fund-investor.yaml`

```yaml
domain: fund-investor

create:
  description: "Register investor in fund (TA context)"
  behavior: plugin
  plugin:
    handler: FundInvestorCreateOp
  args:
    - name: fund-cbu-id
      type: uuid
      required: true
    - name: investor-entity-id
      type: uuid
      required: true
    - name: investor-type
      type: string
      required: true
      enum: [RETAIL, INSTITUTIONAL, NOMINEE]
    - name: investment-amount
      type: decimal
      required: true
    - name: currency
      type: string
      default: EUR
    - name: subscription-date
      type: date
      required: false

# Plugin auto-calculates KYC tier from amount

list:
  description: "List investors for a fund"
  behavior: crud
  crud:
    operation: select
    table: fund_investors
    multiple: true
  args:
    - name: fund-cbu-id
      type: uuid
      required: true
    - name: kyc-status
      type: string
      required: false
      enum: [PENDING, IN_PROGRESS, CLEARED, REJECTED]
```

**Tasks:**
- [ ] Create `fund-investor.yaml` verb file
- [ ] Implement `FundInvestorCreateOp`:
  - [ ] Calculate KYC tier from amount + thresholds
  - [ ] Check regulatory status for institutional
  - [ ] Set appropriate kyc_tier

---

## Phase 4: Plugin Implementations

### 4.1 KycPreviewScopeOp

**File:** `rust/src/dsl_v2/custom_ops/kyc_scope.rs`

```rust
pub struct KycPreviewScopeOp;

impl KycPreviewScopeOp {
    pub async fn execute(&self, args: &Args, pool: &PgPool) -> Result<Value> {
        // 1. Load CBU
        let cbu = load_cbu(args.get_uuid("cbu-id")?, pool).await?;
        
        // 2. Determine service context
        let service_context = args.get_string("service-context")
            .unwrap_or_else(|| derive_service_context(&cbu, pool));
        
        // 3. Load scope template for CBU type
        let template = load_scope_template(&cbu.client_type)?;
        
        // 4. Load all entities with roles in CBU
        let entity_roles = load_cbu_entity_roles(cbu.cbu_id, pool).await?;
        
        // 5. Load regulatory profiles for all entities
        let reg_profiles = load_regulatory_profiles(&entity_roles, pool).await?;
        
        // 6. Calculate risk rating from products
        let products = args.get_string_array("products").unwrap_or_default();
        let risk_rating = calculate_risk_rating(&products)?;
        
        // 7. Apply decision matrix: role × regulatory status
        let mut scoped_entities = Vec::new();
        for (entity, role) in entity_roles {
            let reg_profile = reg_profiles.get(&entity.entity_id);
            let obligation = determine_obligation(&role, reg_profile, &template)?;
            
            scoped_entities.push(ScopedEntity {
                entity_id: entity.entity_id,
                entity_name: entity.name,
                entity_type: entity.entity_type,
                role: role.role_type,
                is_regulated: reg_profile.map(|r| r.is_regulated).unwrap_or(false),
                regulator: reg_profile.and_then(|r| r.regulator_code.clone()),
                kyc_obligation: obligation,
                obligation_reason: explain_obligation(&role, reg_profile, &obligation),
                existing_kyc_status: entity.kyc_status,
                last_verified: entity.last_kyc_date,
            });
        }
        
        // 8. Build summary
        let summary = build_summary(&scoped_entities);
        
        Ok(json!({
            "cbu_type": cbu.client_type,
            "service_context": service_context,
            "case_risk_rating": risk_rating,
            "scope_template": template.name,
            "entities": scoped_entities,
            "summary": summary
        }))
    }
}

fn determine_obligation(
    role: &RoleType,
    reg_profile: Option<&RegulatoryProfile>,
    template: &ScopeTemplate
) -> KycObligation {
    // Check template override for this role
    if let Some(override_obligation) = template.role_obligations.get(&role.code) {
        if *override_obligation == "CHECK_REGULATORY" {
            // Check if entity is regulated
            if let Some(profile) = reg_profile {
                if profile.is_regulated && profile.registration_verified {
                    if profile.regulatory_tier == "EQUIVALENT" {
                        return role.if_regulated_obligation.clone()
                            .unwrap_or(KycObligation::Simplified);
                    }
                }
            }
            // Not regulated or not verified - full KYC
            return KycObligation::FullKyc;
        }
        return parse_obligation(override_obligation);
    }
    
    // Fall back to role type defaults
    if role.triggers_full_kyc {
        if role.check_regulatory_status {
            if let Some(profile) = reg_profile {
                if profile.is_regulated && profile.registration_verified {
                    return role.if_regulated_obligation.clone()
                        .unwrap_or(KycObligation::Simplified);
                }
            }
        }
        return KycObligation::FullKyc;
    }
    
    if role.triggers_screening && role.triggers_id_verification {
        return KycObligation::ScreenAndId;
    }
    
    if role.triggers_screening {
        return KycObligation::ScreenOnly;
    }
    
    KycObligation::RecordOnly
}
```

**Tasks:**
- [ ] Create `kyc_scope.rs` file
- [ ] Implement `KycPreviewScopeOp`
- [ ] Implement helper functions
- [ ] Add tests
- [ ] Register in custom ops

---

### 4.2 Auto-Scope in Case Creation

**File:** `rust/src/dsl_v2/custom_ops/kyc.rs` (update)

```rust
impl KycCaseCreateOp {
    pub async fn execute(&self, args: &Args, ctx: &mut ExecutionContext, pool: &PgPool) -> Result<Value> {
        // ... existing case creation ...
        
        // If auto-scope enabled, create workstreams
        if args.get_bool("auto-scope").unwrap_or(false) {
            let scope = KycPreviewScopeOp::execute_internal(
                args.get_uuid("cbu-id")?,
                args.get_string("service-context"),
                args.get_string_array("products"),
                pool
            ).await?;
            
            for entity in scope.entities {
                if entity.kyc_obligation != KycObligation::RecordOnly {
                    create_workstream(
                        case_id,
                        entity.entity_id,
                        entity.kyc_obligation,
                        entity.role,
                        pool
                    ).await?;
                }
            }
        }
        
        // ... rest of implementation ...
    }
}
```

**Tasks:**
- [ ] Update `KycCaseCreateOp` with auto-scope
- [ ] Extract scope logic to reusable function
- [ ] Add tests for auto-scope

---

## Phase 5: Agent Integration

### 5.1 Agent Context Injection

**File:** `config/agent/kyc_context.yaml`

```yaml
kyc_scoping_rules:
  description: |
    KYC scope depends on:
    1. CBU TYPE - What kind of client?
    2. SERVICE CONTEXT - What service are we providing?
    3. ROLE TYPE - What's the entity's relationship to CBU?
    4. REGULATORY STATUS - Is the entity regulated?
    
  decision_flow: |
    STEP 1: Check CBU type and service context
      (kyc.preview-scope cbu-id:@client)
      
    STEP 2: Review derived obligations for each entity
      - FULL_KYC: Full due diligence required
      - SIMPLIFIED: Verify regulatory registration + screening
      - SCREEN_AND_ID: ID verification + PEP/sanctions
      - SCREEN_ONLY: PEP/sanctions only
      - RECORD_ONLY: Just record the relationship
      
    STEP 3: Create case with auto-scope OR manually create workstreams
      (kyc-case.create cbu-id:@client case-type:NEW_CLIENT auto-scope:true)
      
  key_rules: |
    FUNDS (Custody context):
    - Fund itself: FULL_KYC
    - ManCo/IM: SIMPLIFIED if regulated, else FULL_KYC
    - Directors/Signatories: SCREEN_AND_ID
    - DO NOT chase ManCo's UBOs
    - DO NOT chase fund's investors (not our job as custodian)
    
    FUNDS (TA context):
    - Fund: Light KYC
    - Investors: Threshold-based (€15k simplified, €150k+ enhanced)
    - Institutional investors: Check if regulated
    
    RETAIL (Broker-dealer):
    - Individual IS the account holder IS the UBO
    - Full KYC directly on them
    
    KYC-as-a-Service:
    - We PERFORM, sponsor DECIDES
    - Use (kyc-case.recommend) then (kyc-case.sponsor-decision)
    
  product_risk_ratings:
    HIGH: CUSTODY, PRIME_BROKERAGE, DERIVATIVES_CLEARING
    MEDIUM: FUND_ACCOUNTING, TRANSFER_AGENCY, KYC_AS_A_SERVICE
    LOW: REPORTING_ONLY

  available_verbs:
    - kyc.preview-scope: "See derived KYC obligations"
    - kyc-case.create: "Create case (use auto-scope:true)"
    - entity-workstream.create: "Create individual workstream"
    - regulatory-profile.set: "Set entity regulatory status"
    - regulatory-profile.verify: "Verify registration"
    - kyc-case.recommend: "Our recommendation (KYC-aaS)"
    - kyc-case.sponsor-decision: "Record sponsor decision"
```

**Tasks:**
- [ ] Create `kyc_context.yaml`
- [ ] Inject into agent system prompt when KYC stage active
- [ ] Add examples for agent

---

### 5.2 Agent Tool Knowledge

**File:** Update `tool_knowledge` in agent config

```yaml
tool_knowledge:
  kyc_preview_scope:
    description: "Always call this FIRST before creating KYC case"
    when_to_use:
      - "User mentions onboarding and products"
      - "User asks about KYC requirements"
      - "Before creating any KYC case"
    example: "(kyc.preview-scope cbu-id:@client products:[\"CUSTODY\"])"
    
  kyc_case_create_auto:
    description: "Create case with automatic workstream creation"
    when_to_use:
      - "After preview-scope confirms requirements"
      - "User approves suggested scope"
    example: "(kyc-case.create cbu-id:@client case-type:NEW_CLIENT auto-scope:true)"
    
  regulatory_profile:
    description: "Check/set regulatory status before KYC scoping"
    when_to_use:
      - "Adding new ManCo, IM, or institutional entity"
      - "preview-scope shows entity with CHECK_REGULATORY"
    example: "(regulatory-profile.set entity-id:@manco is-regulated:true regulator:CSSF)"
```

**Tasks:**
- [ ] Update agent tool knowledge
- [ ] Add KYC-specific examples to agent training

---

## Phase 6: Testing

### 6.1 Unit Tests

- [ ] Test role type × regulatory status matrix
- [ ] Test scope template loading
- [ ] Test obligation determination logic
- [ ] Test KYC tier calculation from amounts
- [ ] Test auto-accept threshold logic

### 6.2 Integration Tests

- [ ] Test: Fund onboarding (Custody) - no investor KYC
- [ ] Test: Fund onboarding (TA) - investor KYC required
- [ ] Test: Corporate with UBO chain
- [ ] Test: Retail individual
- [ ] Test: KYC-aaS with auto-accept
- [ ] Test: KYC-aaS with sponsor review

### 6.3 Agent Tests

- [ ] Test: Agent preview-scope flow
- [ ] Test: Agent auto-scope creation
- [ ] Test: Agent handles regulated vs unregulated entities
- [ ] Test: Agent KYC-aaS recommendation flow

---

## Phase 7: Documentation

- [ ] Update `OPERATIONS-RUN-BOOK.md` with KYC scoping examples
- [ ] Add KYC scope decision flowchart
- [ ] Document role type configuration
- [ ] Document regulatory profile management
- [ ] Document KYC-aaS sponsor workflow
- [ ] Add agent prompt examples

---

## Dependencies

```
Phase 1 (Schema & Config)
    │
    ├──► Phase 2 (Service Context)
    │       │
    │       └──► Phase 3 (DSL Verbs)
    │               │
    │               └──► Phase 4 (Plugins)
    │                       │
    │                       └──► Phase 5 (Agent)
    │                               │
    │                               └──► Phase 6 (Testing)
    │                                       │
    │                                       └──► Phase 7 (Docs)
```

---

## Estimated Effort

| Phase | Effort | Notes |
|-------|--------|-------|
| Phase 1: Schema & Config | 2 days | YAML configs, migrations |
| Phase 2: Service Context | 1 day | Sponsor tables |
| Phase 3: DSL Verbs | 2 days | Verb definitions |
| Phase 4: Plugins | 3 days | Core logic implementation |
| Phase 5: Agent | 1 day | Context injection |
| Phase 6: Testing | 2 days | All levels |
| Phase 7: Docs | 1 day | Runbooks, examples |
| **Total** | **12 days** | ~2.5 weeks |

---

## Success Criteria

1. **Agent can preview scope**: `(kyc.preview-scope cbu-id:@fund)` returns correct obligations
2. **No Allianz problem**: ManCo UBOs not in scope for fund onboarding
3. **Regulatory reliance works**: Regulated ManCo/IM gets SIMPLIFIED not FULL_KYC
4. **TA context different**: Same fund, different scope when TA vs Custody
5. **KYC-aaS flow**: Recommend → auto-accept or sponsor review
6. **Agent autonomous**: Can create correctly scoped case from "onboard X for Y"
