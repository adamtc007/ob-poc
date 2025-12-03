# KYC DSL Lifecycle Implementation Plan

## Context

This plan builds on the existing KYC infrastructure:

### Already Implemented
| Component | Location | Status |
|-----------|----------|--------|
| KYC Cases | `kyc.cases`, `kyc.entity_workstreams` | ✓ Full CRUD verbs |
| Red Flags | `kyc.red_flags` | ✓ raise, mitigate, waive, dismiss |
| Doc Requests | `kyc.doc_requests` | ✓ Basic CRUD |
| Screenings | `kyc.screenings` | ✓ run, complete |
| Allegations | `ob-poc.client_allegations` | ✓ record, verify, contradict |
| Observations | `ob-poc.attribute_observations` | ✓ record, reconcile |
| Discrepancies | `ob-poc.observation_discrepancies` | ✓ record, resolve |
| UBO Ownership | `ob-poc.ownership_relationships` | ✓ add, update, end |
| UBO Registry | `ob-poc.ubo_registry` | ✓ register, verify |
| Document Types | `ob-poc.document_types` | ✓ 181 types seeded |
| Document Validity | `ob-poc.document_validity_rules` | ✓ Expiry rules seeded |

### To Be Implemented
| Component | Purpose |
|-----------|---------|
| Threshold Matrix | Compute KYC requirements from risk factors |
| RFI System | Formal document request workflow |
| UBO Chain Analysis | Compute ownership chains, aggregate %, completeness |
| Event Automation | Event-driven evaluation with HITL override |

---

## Phase 1: DSL Argument Type - DocumentTypeCode

### 1.1 Add DocumentTypeCode Validation

The RFI system references document types from `document_types.type_code`. Need compile-time validation.

**Location**: `rust/src/dsl_v2/config/types.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgType {
    // ... existing types ...
    
    /// Document type code - validates against document_types.type_code
    DocumentTypeCode,
    
    /// List of document type codes
    DocumentTypeList,
}
```

**Location**: `rust/src/dsl_v2/csg_linter.rs`

Add validation at lint time:

```rust
impl CsgLinter {
    fn validate_document_type(&self, code: &str, span: Span) -> Result<(), LintError> {
        if !self.document_types.contains(code) {
            let suggestions = self.suggest_similar(&code, &self.document_types);
            return Err(LintError::InvalidDocumentType {
                code: code.to_string(),
                suggestions,
                span,
            });
        }
        Ok(())
    }
}
```

### 1.2 Load Document Types at Startup

**Location**: `rust/src/dsl_v2/runtime_registry.rs`

```rust
impl RuntimeVerbRegistry {
    pub async fn load_document_types(pool: &PgPool) -> Result<HashSet<String>, Error> {
        let types = sqlx::query_scalar!(
            r#"SELECT type_code FROM "ob-poc".document_types WHERE is_active = true"#
        ).fetch_all(pool).await?;
        
        Ok(types.into_iter().collect())
    }
}
```

---

## Phase 2: Threshold Decision Matrix

### 2.1 Database Schema

**File**: `sql/migrations/020_threshold_matrix.sql`

```sql
-- Risk factors for threshold computation
CREATE TABLE "ob-poc".threshold_factors (
    factor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    factor_type VARCHAR(50) NOT NULL,  -- CBU_TYPE, SOURCE_OF_FUNDS, NATURE_PURPOSE, JURISDICTION, PRODUCT_RISK
    factor_code VARCHAR(50) NOT NULL,
    risk_weight INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(factor_type, factor_code)
);

-- Risk bands derived from composite score
CREATE TABLE "ob-poc".risk_bands (
    band_code VARCHAR(20) PRIMARY KEY,
    min_score INTEGER NOT NULL,
    max_score INTEGER NOT NULL,
    description TEXT,
    escalation_required BOOLEAN DEFAULT false
);

-- Requirements per entity role + risk band
CREATE TABLE "ob-poc".threshold_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_role VARCHAR(50) NOT NULL,
    risk_band VARCHAR(20) NOT NULL REFERENCES "ob-poc".risk_bands(band_code),
    attribute_code VARCHAR(50) NOT NULL,
    is_required BOOLEAN NOT NULL DEFAULT true,
    confidence_min NUMERIC(3,2) DEFAULT 0.85,
    max_age_days INTEGER,
    must_be_authoritative BOOLEAN DEFAULT false,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(entity_role, risk_band, attribute_code)
);

-- Acceptable document types per requirement
CREATE TABLE "ob-poc".requirement_acceptable_docs (
    requirement_id UUID REFERENCES "ob-poc".threshold_requirements(requirement_id) ON DELETE CASCADE,
    document_type_code VARCHAR(50) REFERENCES "ob-poc".document_types(type_code),
    priority INTEGER DEFAULT 1,
    PRIMARY KEY (requirement_id, document_type_code)
);

-- Screening requirements per risk band
CREATE TABLE "ob-poc".screening_requirements (
    risk_band VARCHAR(20) REFERENCES "ob-poc".risk_bands(band_code),
    screening_type VARCHAR(50) NOT NULL,
    is_required BOOLEAN NOT NULL DEFAULT true,
    PRIMARY KEY (risk_band, screening_type)
);

-- Indexes
CREATE INDEX idx_threshold_factors_type ON "ob-poc".threshold_factors(factor_type);
CREATE INDEX idx_threshold_requirements_role ON "ob-poc".threshold_requirements(entity_role);
CREATE INDEX idx_threshold_requirements_band ON "ob-poc".threshold_requirements(risk_band);
```

### 2.2 Seed Data

**File**: `sql/seeds/threshold_matrix.sql`

```sql
-- Risk bands
INSERT INTO "ob-poc".risk_bands VALUES
('LOW', 0, 3, 'Low risk - standard due diligence', false),
('MEDIUM', 4, 6, 'Medium risk - enhanced monitoring', false),
('HIGH', 7, 9, 'High risk - enhanced due diligence', true),
('ENHANCED', 10, 99, 'Enhanced risk - senior approval required', true);

-- CBU type factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('CBU_TYPE', 'LUXSICAV_UCITS', 1, 'Luxembourg SICAV - UCITS regulated'),
('CBU_TYPE', 'LUXSICAV_PART2', 2, 'Luxembourg SICAV - Part II'),
('CBU_TYPE', 'HEDGE_FUND', 3, 'Hedge fund'),
('CBU_TYPE', '40_ACT_FUND', 1, 'US 40 Act fund - SEC regulated'),
('CBU_TYPE', 'FAMILY_TRUST', 2, 'Family trust'),
('CBU_TYPE', 'TRADING_COMPANY', 3, 'Trading company'),
('CBU_TYPE', 'PENSION_FUND', 1, 'Pension fund - regulated'),
('CBU_TYPE', 'CORPORATE', 2, 'Corporate entity');

-- Source of funds factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('SOURCE_OF_FUNDS', 'REGULATED_INSTITUTION', 0, 'Regulated bank/insurer'),
('SOURCE_OF_FUNDS', 'INSTITUTIONAL_INVESTOR', 1, 'Pension, SWF, endowment'),
('SOURCE_OF_FUNDS', 'PRIVATE_WEALTH', 2, 'HNWI, family office'),
('SOURCE_OF_FUNDS', 'CORPORATE', 2, 'Corporate treasury'),
('SOURCE_OF_FUNDS', 'UNKNOWN', 4, 'Not yet determined');

-- Nature/purpose factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('NATURE_PURPOSE', 'LONG_ONLY', 1, 'Long-only investment'),
('NATURE_PURPOSE', 'LEVERAGED_TRADING', 3, 'Leveraged/short strategies'),
('NATURE_PURPOSE', 'REAL_ESTATE', 2, 'Real estate investment'),
('NATURE_PURPOSE', 'PRIVATE_EQUITY', 2, 'PE/VC'),
('NATURE_PURPOSE', 'HOLDING', 2, 'Holding structure');

-- Product risk factors
INSERT INTO "ob-poc".threshold_factors (factor_type, factor_code, risk_weight, description) VALUES
('PRODUCT_RISK', 'CUSTODY', 3, 'Custody - high risk'),
('PRODUCT_RISK', 'FUND_ACCOUNTING', 1, 'Fund accounting - low risk'),
('PRODUCT_RISK', 'TRANSFER_AGENCY', 2, 'Transfer agency - medium risk'),
('PRODUCT_RISK', 'MIDDLE_OFFICE', 2, 'Middle office - medium risk');

-- UBO requirements - LOW risk
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days) VALUES
('UBO', 'LOW', 'identity', true, 0.90, NULL),
('UBO', 'LOW', 'address', true, 0.85, 180),
('UBO', 'LOW', 'date_of_birth', true, 0.90, NULL),
('UBO', 'LOW', 'nationality', true, 0.85, NULL);

-- UBO requirements - HIGH risk
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('UBO', 'HIGH', 'identity', true, 0.95, NULL, true),
('UBO', 'HIGH', 'address', true, 0.90, 90, false),
('UBO', 'HIGH', 'date_of_birth', true, 0.95, NULL, true),
('UBO', 'HIGH', 'nationality', true, 0.90, NULL, false),
('UBO', 'HIGH', 'source_of_wealth', true, 0.85, NULL, false),
('UBO', 'HIGH', 'tax_residence', true, 0.85, 365, false);

-- UBO requirements - ENHANCED risk
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days, must_be_authoritative) VALUES
('UBO', 'ENHANCED', 'identity', true, 0.98, NULL, true),
('UBO', 'ENHANCED', 'address', true, 0.95, 60, true),
('UBO', 'ENHANCED', 'date_of_birth', true, 0.98, NULL, true),
('UBO', 'ENHANCED', 'nationality', true, 0.95, NULL, true),
('UBO', 'ENHANCED', 'source_of_wealth', true, 0.90, NULL, false),
('UBO', 'ENHANCED', 'source_of_funds', true, 0.90, NULL, false),
('UBO', 'ENHANCED', 'tax_residence', true, 0.90, 180, false);

-- Director requirements
INSERT INTO "ob-poc".threshold_requirements (entity_role, risk_band, attribute_code, is_required, confidence_min, max_age_days) VALUES
('DIRECTOR', 'LOW', 'identity', true, 0.85, NULL),
('DIRECTOR', 'LOW', 'address', true, 0.80, 365),
('DIRECTOR', 'HIGH', 'identity', true, 0.90, NULL),
('DIRECTOR', 'HIGH', 'address', true, 0.85, 180);

-- Link requirements to acceptable document types
-- Identity proof
INSERT INTO "ob-poc".requirement_acceptable_docs (requirement_id, document_type_code, priority)
SELECT r.requirement_id, dt.type_code,
    CASE dt.type_code WHEN 'PASSPORT' THEN 1 WHEN 'NATIONAL_ID' THEN 2 ELSE 3 END
FROM "ob-poc".threshold_requirements r
CROSS JOIN "ob-poc".document_types dt
WHERE r.attribute_code = 'identity'
  AND dt.type_code IN ('PASSPORT', 'NATIONAL_ID', 'DRIVERS_LICENSE');

-- Address proof
INSERT INTO "ob-poc".requirement_acceptable_docs (requirement_id, document_type_code, priority)
SELECT r.requirement_id, dt.type_code,
    CASE dt.type_code WHEN 'UTILITY_BILL' THEN 1 WHEN 'BANK_STATEMENT' THEN 2 ELSE 3 END
FROM "ob-poc".threshold_requirements r
CROSS JOIN "ob-poc".document_types dt
WHERE r.attribute_code = 'address'
  AND dt.type_code IN ('UTILITY_BILL', 'BANK_STATEMENT', 'COUNCIL_TAX_BILL', 'TENANCY_AGREEMENT');

-- Screening requirements
INSERT INTO "ob-poc".screening_requirements VALUES
('LOW', 'SANCTIONS', true),
('LOW', 'PEP', true),
('LOW', 'ADVERSE_MEDIA', false),
('MEDIUM', 'SANCTIONS', true),
('MEDIUM', 'PEP', true),
('MEDIUM', 'ADVERSE_MEDIA', true),
('HIGH', 'SANCTIONS', true),
('HIGH', 'PEP', true),
('HIGH', 'ADVERSE_MEDIA', true),
('ENHANCED', 'SANCTIONS', true),
('ENHANCED', 'PEP', true),
('ENHANCED', 'ADVERSE_MEDIA', true);
```

### 2.3 Threshold DSL Verbs

**File**: `rust/config/verbs/kyc/threshold.yaml`

```yaml
domains:
  threshold:
    description: KYC threshold computation and evaluation
    verbs:
      derive:
        description: Compute KYC requirements based on CBU risk factors
        behavior: plugin
        plugin:
          handler: threshold_derive
        args:
        - name: cbu-id
          type: uuid
          required: true
        returns:
          type: json
          capture: true
          description: |
            {
              risk_score: integer,
              risk_band: string,
              factors: [{type, code, weight}],
              entity_requirements: [{
                entity_id, entity_role,
                requirements: [{attribute, required, acceptable_docs, confidence_min, max_age_days}]
              }],
              screening_requirements: {sanctions, pep, adverse_media}
            }

      evaluate:
        description: Check if CBU meets threshold requirements
        behavior: plugin
        plugin:
          handler: threshold_evaluate
        args:
        - name: cbu-id
          type: uuid
          required: true
        - name: requirements
          type: json
          required: false
          description: Pre-computed requirements (if not provided, derives them)
        returns:
          type: json
          capture: true
          description: |
            {
              overall_status: COMPLETE | INCOMPLETE | BLOCKED,
              entities: [{
                entity_id, role, status,
                checks: [{attribute, status, observations, proof_document}]
              }],
              gaps: [{type, entity_id, attribute, details}],
              blocking: [{type, entity_id, details}]
            }

      check-entity:
        description: Check single entity against requirements
        behavior: plugin
        plugin:
          handler: threshold_check_entity
        args:
        - name: entity-id
          type: uuid
          required: true
        - name: role
          type: string
          required: true
        - name: risk-band
          type: string
          required: true
        returns:
          type: json
          capture: true
```

### 2.4 Threshold Plugin Implementation

**File**: `rust/src/dsl_v2/custom_ops/threshold.rs`

```rust
use crate::dsl_v2::executor::ExecutionContext;
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ThresholdRequirements {
    pub risk_score: i32,
    pub risk_band: String,
    pub factors: Vec<RiskFactor>,
    pub entity_requirements: Vec<EntityRequirements>,
    pub screening_requirements: ScreeningRequirements,
}

#[derive(Debug, Serialize)]
pub struct RiskFactor {
    pub factor_type: String,
    pub factor_code: String,
    pub weight: i32,
}

#[derive(Debug, Serialize)]
pub struct EntityRequirements {
    pub entity_id: Uuid,
    pub entity_role: String,
    pub requirements: Vec<AttributeRequirement>,
}

#[derive(Debug, Serialize)]
pub struct AttributeRequirement {
    pub attribute: String,
    pub required: bool,
    pub acceptable_docs: Vec<String>,
    pub confidence_min: f64,
    pub max_age_days: Option<i32>,
    pub must_be_authoritative: bool,
}

pub async fn threshold_derive(
    pool: &PgPool,
    cbu_id: Uuid,
) -> Result<ThresholdRequirements, Error> {
    // 1. Load CBU attributes
    let cbu = sqlx::query!(
        r#"SELECT client_type, jurisdiction, nature_purpose, source_of_funds
           FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    ).fetch_one(pool).await?;

    // 2. Load product risk (MAX from service_delivery_map)
    let product_risk = sqlx::query_scalar!(
        r#"SELECT MAX(tf.risk_weight)
           FROM "ob-poc".service_delivery_map sdm
           JOIN "ob-poc".products p ON sdm.product_id = p.product_id
           JOIN "ob-poc".threshold_factors tf ON tf.factor_code = p.product_code
           WHERE sdm.cbu_id = $1 AND tf.factor_type = 'PRODUCT_RISK'"#,
        cbu_id
    ).fetch_optional(pool).await?.flatten().unwrap_or(0);

    // 3. Sum risk weights
    let mut risk_score = 0;
    let mut factors = vec![];

    // CBU type factor
    if let Some(ref client_type) = cbu.client_type {
        if let Some(factor) = load_factor(pool, "CBU_TYPE", client_type).await? {
            risk_score += factor.risk_weight;
            factors.push(factor);
        }
    }

    // Source of funds factor
    if let Some(ref sof) = cbu.source_of_funds {
        if let Some(factor) = load_factor(pool, "SOURCE_OF_FUNDS", sof).await? {
            risk_score += factor.risk_weight;
            factors.push(factor);
        }
    }

    // Nature/purpose factor
    if let Some(ref np) = cbu.nature_purpose {
        if let Some(factor) = load_factor(pool, "NATURE_PURPOSE", np).await? {
            risk_score += factor.risk_weight;
            factors.push(factor);
        }
    }

    // Product risk
    risk_score += product_risk;
    factors.push(RiskFactor {
        factor_type: "PRODUCT_RISK".into(),
        factor_code: "MAX_PRODUCT".into(),
        weight: product_risk,
    });

    // 4. Map to risk band
    let risk_band = sqlx::query_scalar!(
        r#"SELECT band_code FROM "ob-poc".risk_bands
           WHERE $1 >= min_score AND $1 <= max_score"#,
        risk_score
    ).fetch_one(pool).await?;

    // 5. Load entity requirements
    let entities = load_cbu_entities_with_roles(pool, cbu_id).await?;
    let mut entity_requirements = vec![];

    for (entity_id, role) in entities {
        let reqs = load_requirements_for_role(pool, &role, &risk_band).await?;
        entity_requirements.push(EntityRequirements {
            entity_id,
            entity_role: role,
            requirements: reqs,
        });
    }

    // 6. Load screening requirements
    let screening = load_screening_requirements(pool, &risk_band).await?;

    Ok(ThresholdRequirements {
        risk_score,
        risk_band,
        factors,
        entity_requirements,
        screening_requirements: screening,
    })
}

pub async fn threshold_evaluate(
    pool: &PgPool,
    cbu_id: Uuid,
    requirements: Option<ThresholdRequirements>,
) -> Result<ThresholdEvaluation, Error> {
    // Get requirements if not passed
    let reqs = match requirements {
        Some(r) => r,
        None => threshold_derive(pool, cbu_id).await?,
    };

    let mut entities = vec![];
    let mut gaps = vec![];
    let mut blocking = vec![];

    for entity_req in &reqs.entity_requirements {
        let mut checks = vec![];
        let mut entity_status = "COMPLETE";

        for attr_req in &entity_req.requirements {
            // Get observations for this entity+attribute
            let observations = sqlx::query!(
                r#"SELECT observation_id, value_text, confidence, is_authoritative,
                          source_document_id, observed_at
                   FROM "ob-poc".attribute_observations ao
                   JOIN "ob-poc".attribute_registry ar ON ao.attribute_id = ar.attribute_id
                   WHERE ao.entity_id = $1 AND ar.attribute_code = $2
                   AND ao.status = 'ACTIVE'
                   ORDER BY is_authoritative DESC, confidence DESC"#,
                entity_req.entity_id,
                attr_req.attribute
            ).fetch_all(pool).await?;

            let check = evaluate_attribute_requirement(attr_req, &observations);
            
            match check.status.as_str() {
                "MISSING" | "EXPIRED" | "INSUFFICIENT_CONFIDENCE" => {
                    entity_status = "INCOMPLETE";
                    gaps.push(Gap {
                        gap_type: check.status.clone(),
                        entity_id: entity_req.entity_id,
                        attribute: Some(attr_req.attribute.clone()),
                        details: check.notes.clone().unwrap_or_default(),
                    });
                }
                "CONFLICT" => {
                    entity_status = "BLOCKED";
                    blocking.push(Blocker {
                        blocker_type: "UNRESOLVED_CONFLICT".into(),
                        entity_id: entity_req.entity_id,
                        details: format!("Conflicting observations for {}", attr_req.attribute),
                    });
                }
                _ => {}
            }

            checks.push(check);
        }

        entities.push(EntityEvaluation {
            entity_id: entity_req.entity_id,
            role: entity_req.entity_role.clone(),
            status: entity_status.into(),
            checks,
        });
    }

    // Determine overall status
    let overall_status = if !blocking.is_empty() {
        "BLOCKED"
    } else if !gaps.is_empty() {
        "INCOMPLETE"
    } else {
        "COMPLETE"
    };

    Ok(ThresholdEvaluation {
        overall_status: overall_status.into(),
        risk_band: reqs.risk_band,
        entities,
        gaps,
        blocking,
    })
}
```

---

## Phase 3: RFI System

### 3.1 Database Schema

**File**: `sql/migrations/021_rfi_system.sql`

```sql
-- RFI (Request for Information)
CREATE TABLE "ob-poc".rfis (
    rfi_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    rfi_type VARCHAR(30) NOT NULL DEFAULT 'INITIAL',
    status VARCHAR(30) NOT NULL DEFAULT 'DRAFT',
    created_at TIMESTAMPTZ DEFAULT now(),
    sent_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    due_date DATE,
    notes TEXT,
    created_by VARCHAR(100),
    CONSTRAINT valid_rfi_type CHECK (rfi_type IN ('INITIAL', 'SUPPLEMENTARY', 'REFRESH')),
    CONSTRAINT valid_rfi_status CHECK (status IN ('DRAFT', 'SENT', 'PARTIAL', 'COMPLETE', 'CLOSED', 'CANCELLED'))
);

-- RFI Items
CREATE TABLE "ob-poc".rfi_items (
    item_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rfi_id UUID NOT NULL REFERENCES "ob-poc".rfis(rfi_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    proves_attribute VARCHAR(50) NOT NULL,
    request_text TEXT,
    is_required BOOLEAN DEFAULT true,
    max_age_days INTEGER,
    status VARCHAR(30) NOT NULL DEFAULT 'PENDING',
    received_at TIMESTAMPTZ,
    reviewed_at TIMESTAMPTZ,
    reviewed_by VARCHAR(100),
    rejection_reason TEXT,
    notes TEXT,
    CONSTRAINT valid_item_status CHECK (status IN ('PENDING', 'RECEIVED', 'ACCEPTED', 'REJECTED'))
);

-- Acceptable document types per RFI item
CREATE TABLE "ob-poc".rfi_item_acceptable_docs (
    item_id UUID REFERENCES "ob-poc".rfi_items(item_id) ON DELETE CASCADE,
    document_type_code VARCHAR(50) REFERENCES "ob-poc".document_types(type_code),
    priority INTEGER DEFAULT 1,
    PRIMARY KEY (item_id, document_type_code)
);

-- Documents received against RFI items
CREATE TABLE "ob-poc".rfi_item_documents (
    item_id UUID REFERENCES "ob-poc".rfi_items(item_id) ON DELETE CASCADE,
    document_id UUID REFERENCES "ob-poc".document_catalog(document_id),
    received_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (item_id, document_id)
);

-- RFI delivery log
CREATE TABLE "ob-poc".rfi_delivery_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rfi_id UUID REFERENCES "ob-poc".rfis(rfi_id),
    channel VARCHAR(30) NOT NULL,
    recipient VARCHAR(255),
    sent_at TIMESTAMPTZ DEFAULT now(),
    status VARCHAR(30),
    notes TEXT
);

-- Indexes
CREATE INDEX idx_rfis_case ON "ob-poc".rfis(case_id);
CREATE INDEX idx_rfis_cbu ON "ob-poc".rfis(cbu_id);
CREATE INDEX idx_rfis_status ON "ob-poc".rfis(status);
CREATE INDEX idx_rfi_items_rfi ON "ob-poc".rfi_items(rfi_id);
CREATE INDEX idx_rfi_items_entity ON "ob-poc".rfi_items(entity_id);
CREATE INDEX idx_rfi_items_status ON "ob-poc".rfi_items(status);
```

### 3.2 RFI DSL Verbs

**File**: `rust/config/verbs/kyc/rfi.yaml`

```yaml
domains:
  rfi:
    description: Request for Information management
    verbs:
      generate:
        description: Auto-generate RFI from threshold evaluation gaps
        behavior: plugin
        plugin:
          handler: rfi_generate
        args:
        - name: case-id
          type: uuid
          required: true
        - name: gaps
          type: json
          required: true
          description: Gaps from threshold.evaluate
        - name: type
          type: string
          required: false
          default: INITIAL
          valid_values: [INITIAL, SUPPLEMENTARY, REFRESH]
        - name: due-days
          type: integer
          required: false
          default: 14
        returns:
          type: uuid
          name: rfi_id
          capture: true

      create:
        description: Create empty RFI for manual population
        behavior: crud
        crud:
          operation: insert
          table: rfis
          schema: ob-poc
          returning: rfi_id
        args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: type
          type: string
          required: false
          default: INITIAL
          maps_to: rfi_type
        - name: due-date
          type: date
          required: false
          maps_to: due_date
        - name: notes
          type: string
          required: false
          maps_to: notes
        returns:
          type: uuid
          name: rfi_id
          capture: true

      request-document:
        description: Add document request to RFI
        behavior: plugin
        plugin:
          handler: rfi_request_document
        args:
        - name: rfi-id
          type: uuid
          required: true
        - name: entity-id
          type: uuid
          required: true
        - name: proves
          type: string
          required: true
          description: Attribute this document proves (identity, address, etc.)
        - name: acceptable-docs
          type: document_type_list
          required: true
          description: List of acceptable document type codes
        - name: required
          type: boolean
          required: false
          default: true
        - name: max-age-days
          type: integer
          required: false
        - name: notes
          type: string
          required: false
        returns:
          type: uuid
          name: item_id
          capture: true

      finalize:
        description: Finalize RFI (lock for sending)
        behavior: crud
        crud:
          operation: update
          table: rfis
          schema: ob-poc
          key: rfi_id
          set_values:
            status: SENT
        args:
        - name: rfi-id
          type: uuid
          required: true
          maps_to: rfi_id
        returns:
          type: affected

      send:
        description: Send RFI to recipient
        behavior: plugin
        plugin:
          handler: rfi_send
        args:
        - name: rfi-id
          type: uuid
          required: true
        - name: channel
          type: string
          required: true
          valid_values: [EMAIL, PORTAL, API]
        - name: recipient
          type: string
          required: true
        returns:
          type: affected

      receive:
        description: Record document received for RFI item
        behavior: plugin
        plugin:
          handler: rfi_receive
        args:
        - name: rfi-id
          type: uuid
          required: true
        - name: item-id
          type: uuid
          required: true
        - name: document-id
          type: uuid
          required: true
        returns:
          type: affected

      close:
        description: Close RFI
        behavior: crud
        crud:
          operation: update
          table: rfis
          schema: ob-poc
          key: rfi_id
        args:
        - name: rfi-id
          type: uuid
          required: true
          maps_to: rfi_id
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [COMPLETE, CANCELLED]
        - name: notes
          type: string
          required: false
          maps_to: notes
        returns:
          type: affected

      list-by-case:
        description: List RFIs for a case
        behavior: crud
        crud:
          operation: list_by_fk
          table: rfis
          schema: ob-poc
          fk_col: case_id
        args:
        - name: case-id
          type: uuid
          required: true
        returns:
          type: record_set

      get-items:
        description: Get items for an RFI
        behavior: crud
        crud:
          operation: list_by_fk
          table: rfi_items
          schema: ob-poc
          fk_col: rfi_id
        args:
        - name: rfi-id
          type: uuid
          required: true
        returns:
          type: record_set
```

### 3.3 RFI Plugin Implementation

**File**: `rust/src/dsl_v2/custom_ops/rfi.rs`

```rust
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{NaiveDate, Utc, Duration};

pub async fn rfi_generate(
    pool: &PgPool,
    case_id: Uuid,
    gaps: Vec<Gap>,
    rfi_type: &str,
    due_days: i32,
) -> Result<Uuid, Error> {
    // Get CBU from case
    let cbu_id = sqlx::query_scalar!(
        "SELECT cbu_id FROM kyc.cases WHERE case_id = $1",
        case_id
    ).fetch_one(pool).await?;

    // Create RFI
    let due_date = Utc::now().date_naive() + Duration::days(due_days as i64);
    let rfi_id = sqlx::query_scalar!(
        r#"INSERT INTO "ob-poc".rfis (case_id, cbu_id, rfi_type, due_date)
           VALUES ($1, $2, $3, $4) RETURNING rfi_id"#,
        case_id, cbu_id, rfi_type, due_date
    ).fetch_one(pool).await?;

    // Group gaps by entity
    let mut by_entity: HashMap<Uuid, Vec<&Gap>> = HashMap::new();
    for gap in &gaps {
        by_entity.entry(gap.entity_id).or_default().push(gap);
    }

    // Create items for each gap
    for (entity_id, entity_gaps) in by_entity {
        for gap in entity_gaps {
            if let Some(ref attribute) = gap.attribute {
                // Get acceptable docs from requirements
                let acceptable_docs = get_acceptable_docs_for_attribute(pool, attribute).await?;
                
                // Generate request text
                let entity_name = get_entity_name(pool, entity_id).await?;
                let request_text = format!(
                    "Please provide {} for {}",
                    attribute.replace('_', " "),
                    entity_name
                );

                // Create item
                let item_id = sqlx::query_scalar!(
                    r#"INSERT INTO "ob-poc".rfi_items 
                       (rfi_id, entity_id, proves_attribute, request_text, is_required)
                       VALUES ($1, $2, $3, $4, true) RETURNING item_id"#,
                    rfi_id, entity_id, attribute, request_text
                ).fetch_one(pool).await?;

                // Link acceptable docs
                for (i, doc_code) in acceptable_docs.iter().enumerate() {
                    sqlx::query!(
                        r#"INSERT INTO "ob-poc".rfi_item_acceptable_docs 
                           (item_id, document_type_code, priority)
                           VALUES ($1, $2, $3)"#,
                        item_id, doc_code, (i + 1) as i32
                    ).execute(pool).await?;
                }
            }
        }
    }

    Ok(rfi_id)
}

pub async fn rfi_request_document(
    pool: &PgPool,
    rfi_id: Uuid,
    entity_id: Uuid,
    proves: &str,
    acceptable_docs: Vec<String>,
    required: bool,
    max_age_days: Option<i32>,
    notes: Option<String>,
) -> Result<Uuid, Error> {
    // Validate RFI is in DRAFT status
    let status: String = sqlx::query_scalar!(
        r#"SELECT status FROM "ob-poc".rfis WHERE rfi_id = $1"#,
        rfi_id
    ).fetch_one(pool).await?;

    if status != "DRAFT" {
        return Err(Error::InvalidState("RFI must be in DRAFT status to add items"));
    }

    // Validate document type codes exist
    for doc_code in &acceptable_docs {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".document_types WHERE type_code = $1)"#,
            doc_code
        ).fetch_one(pool).await?.unwrap_or(false);

        if !exists {
            return Err(Error::InvalidDocumentType(doc_code.clone()));
        }
    }

    // Create item
    let item_id = sqlx::query_scalar!(
        r#"INSERT INTO "ob-poc".rfi_items 
           (rfi_id, entity_id, proves_attribute, is_required, max_age_days, notes)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING item_id"#,
        rfi_id, entity_id, proves, required, max_age_days, notes
    ).fetch_one(pool).await?;

    // Link acceptable docs
    for (i, doc_code) in acceptable_docs.iter().enumerate() {
        sqlx::query!(
            r#"INSERT INTO "ob-poc".rfi_item_acceptable_docs 
               (item_id, document_type_code, priority)
               VALUES ($1, $2, $3)"#,
            item_id, doc_code, (i + 1) as i32
        ).execute(pool).await?;
    }

    Ok(item_id)
}

pub async fn rfi_receive(
    pool: &PgPool,
    rfi_id: Uuid,
    item_id: Uuid,
    document_id: Uuid,
) -> Result<(), Error> {
    // Link document to item
    sqlx::query!(
        r#"INSERT INTO "ob-poc".rfi_item_documents (item_id, document_id)
           VALUES ($1, $2)"#,
        item_id, document_id
    ).execute(pool).await?;

    // Update item status
    sqlx::query!(
        r#"UPDATE "ob-poc".rfi_items SET status = 'RECEIVED', received_at = now()
           WHERE item_id = $1"#,
        item_id
    ).execute(pool).await?;

    // Check if all required items received
    let all_received = sqlx::query_scalar!(
        r#"SELECT NOT EXISTS(
            SELECT 1 FROM "ob-poc".rfi_items
            WHERE rfi_id = $1 AND is_required = true AND status = 'PENDING'
        )"#,
        rfi_id
    ).fetch_one(pool).await?.unwrap_or(false);

    if all_received {
        sqlx::query!(
            r#"UPDATE "ob-poc".rfis SET status = 'COMPLETE', completed_at = now()
               WHERE rfi_id = $1"#,
            rfi_id
        ).execute(pool).await?;
    } else {
        sqlx::query!(
            r#"UPDATE "ob-poc".rfis SET status = 'PARTIAL' WHERE rfi_id = $1 AND status = 'SENT'"#,
            rfi_id
        ).execute(pool).await?;
    }

    Ok(())
}
```

---

## Phase 4: UBO Chain Analysis

### 4.1 Database Function

**File**: `sql/migrations/022_ubo_chain_function.sql`

```sql
-- Compute ownership chains for a CBU
CREATE OR REPLACE FUNCTION "ob-poc".compute_ownership_chains(target_cbu_id UUID)
RETURNS TABLE (
    chain_id INTEGER,
    depth INTEGER,
    entity_id UUID,
    entity_name TEXT,
    entity_type VARCHAR(50),
    parent_entity_id UUID,
    ownership_pct NUMERIC(5,2),
    aggregate_pct NUMERIC(5,2),
    terminates_at VARCHAR(50),
    is_ubo BOOLEAN,
    chain_path UUID[]
) AS $$
WITH RECURSIVE ownership_tree AS (
    -- Base: start from CBU commercial client entity
    SELECT 
        1 as chain_id,
        0 as depth,
        c.commercial_client_entity_id as entity_id,
        e.name as entity_name,
        et.type_code as entity_type,
        NULL::UUID as parent_entity_id,
        100.00::NUMERIC(5,2) as ownership_pct,
        100.00::NUMERIC(5,2) as aggregate_pct,
        ARRAY[c.commercial_client_entity_id] as chain_path
    FROM "ob-poc".cbus c
    JOIN "ob-poc".entities e ON c.commercial_client_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE c.cbu_id = target_cbu_id
    
    UNION ALL
    
    -- Recursive: follow ownership links upward
    SELECT 
        ot.chain_id + CASE WHEN ol.owner_entity_id = ANY(ot.chain_path) THEN 1000 ELSE 0 END,
        ot.depth + 1,
        ol.owner_entity_id,
        e.name,
        et.type_code,
        ot.entity_id,
        ol.ownership_percent,
        (ot.aggregate_pct * ol.ownership_percent / 100)::NUMERIC(5,2),
        ot.chain_path || ol.owner_entity_id
    FROM ownership_tree ot
    JOIN "ob-poc".ownership_relationships ol ON ol.owned_entity_id = ot.entity_id
        AND (ol.effective_to IS NULL OR ol.effective_to > CURRENT_DATE)
    JOIN "ob-poc".entities e ON ol.owner_entity_id = e.entity_id
    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
    WHERE ot.depth < 10  -- Max depth
      AND NOT (ol.owner_entity_id = ANY(ot.chain_path))  -- Cycle detection
)
SELECT 
    chain_id,
    depth,
    entity_id,
    entity_name,
    entity_type,
    parent_entity_id,
    ownership_pct,
    aggregate_pct,
    CASE 
        WHEN entity_type = 'proper_person' THEN 'NATURAL_PERSON'
        WHEN entity_type = 'listed_company' THEN 'LISTED_COMPANY'
        WHEN entity_type = 'government_body' THEN 'GOVERNMENT'
        WHEN entity_type = 'regulated_fund' THEN 'REGULATED_FUND'
        ELSE NULL
    END as terminates_at,
    entity_type = 'proper_person' AND aggregate_pct >= 25.00 as is_ubo,
    chain_path
FROM ownership_tree
ORDER BY chain_id, depth;
$$ LANGUAGE SQL STABLE;

-- UBO completeness check view
CREATE OR REPLACE VIEW "ob-poc".v_cbu_ubo_completeness AS
WITH chains AS (
    SELECT c.cbu_id, oc.*
    FROM "ob-poc".cbus c
    CROSS JOIN LATERAL "ob-poc".compute_ownership_chains(c.cbu_id) oc
),
chain_terminals AS (
    SELECT cbu_id, chain_id, MAX(depth) as max_depth
    FROM chains
    GROUP BY cbu_id, chain_id
),
terminal_entities AS (
    SELECT c.cbu_id, c.chain_id, c.entity_id, c.entity_name, c.entity_type,
           c.aggregate_pct, c.terminates_at
    FROM chains c
    JOIN chain_terminals ct ON c.cbu_id = ct.cbu_id 
        AND c.chain_id = ct.chain_id 
        AND c.depth = ct.max_depth
)
SELECT 
    cbu_id,
    COUNT(DISTINCT chain_id) as total_chains,
    COUNT(DISTINCT chain_id) FILTER (WHERE terminates_at IS NOT NULL) as terminated_chains,
    COUNT(DISTINCT chain_id) FILTER (WHERE terminates_at IS NULL) as unterminated_chains,
    SUM(aggregate_pct) FILTER (WHERE terminates_at = 'NATURAL_PERSON') as identified_ownership_pct,
    COUNT(DISTINCT entity_id) FILTER (WHERE terminates_at = 'NATURAL_PERSON' AND aggregate_pct >= 25) as ubo_count
FROM terminal_entities
GROUP BY cbu_id;
```

### 4.2 UBO Plugin Verbs

**File**: Update `rust/config/verbs/ubo.yaml` to add:

```yaml
      trace-chains:
        description: Compute all ownership chains for CBU
        behavior: plugin
        plugin:
          handler: ubo_trace_chains
        args:
        - name: cbu-id
          type: uuid
          required: true
        returns:
          type: json
          capture: true
          description: |
            {
              chains: [{chain_id, path: [{entity_id, name, type, ownership_pct, aggregate_pct}], terminates_at, aggregate_pct}],
              ubos: [{entity_id, name, aggregate_pct, chain_ids}]
            }

      check-completeness:
        description: Check if UBO structure is complete
        behavior: plugin
        plugin:
          handler: ubo_check_completeness
        args:
        - name: cbu-id
          type: uuid
          required: true
        returns:
          type: json
          capture: true
          description: |
            {
              complete: boolean,
              total_chains: integer,
              terminated_chains: integer,
              unterminated_chains: integer,
              identified_ownership_pct: float,
              ubo_count: integer,
              gaps: [{chain_id, last_entity_id, last_entity_name, aggregate_pct}]
            }

      insert-placeholder:
        description: Insert placeholder entity for unknown UBO
        behavior: plugin
        plugin:
          handler: ubo_insert_placeholder
        args:
        - name: cbu-id
          type: uuid
          required: true
        - name: role
          type: string
          required: true
        - name: parent-entity-id
          type: uuid
          required: true
        - name: ownership-pct
          type: decimal
          required: false
        - name: notes
          type: string
          required: false
        returns:
          type: uuid
          name: entity_id
          capture: true
```

---

## Phase 5: Event-Driven Automation

### 5.1 Database Schema

**File**: `sql/migrations/023_event_system.sql`

```sql
-- Event types
CREATE TABLE "ob-poc".event_types (
    event_type VARCHAR(50) PRIMARY KEY,
    description TEXT,
    payload_schema JSONB,
    is_active BOOLEAN DEFAULT true
);

-- Event handlers
CREATE TABLE "ob-poc".event_handlers (
    handler_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) REFERENCES "ob-poc".event_types(event_type),
    handler_name VARCHAR(100) NOT NULL,
    conditions JSONB,
    dsl_source TEXT NOT NULL,
    priority INTEGER DEFAULT 100,
    is_active BOOLEAN DEFAULT true,
    automation_mode VARCHAR(20) DEFAULT 'SEMI_AUTO',
    created_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT valid_automation_mode CHECK (automation_mode IN ('FULL_AUTO', 'SEMI_AUTO', 'MANUAL'))
);

-- Event log
CREATE TABLE "ob-poc".event_log (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) NOT NULL,
    payload JSONB NOT NULL,
    cbu_id UUID,
    case_id UUID,
    entity_id UUID,
    occurred_at TIMESTAMPTZ DEFAULT now(),
    processed_at TIMESTAMPTZ,
    status VARCHAR(20) DEFAULT 'PENDING',
    error_message TEXT,
    CONSTRAINT valid_event_status CHECK (status IN ('PENDING', 'PROCESSING', 'COMPLETED', 'FAILED'))
);

-- Action queue (SEMI_AUTO mode)
CREATE TABLE "ob-poc".action_queue (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES "ob-poc".event_log(event_id),
    action_type VARCHAR(50) NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(20) DEFAULT 'PENDING',
    assigned_to VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT now(),
    reviewed_at TIMESTAMPTZ,
    reviewed_by VARCHAR(100),
    notes TEXT,
    CONSTRAINT valid_action_status CHECK (status IN ('PENDING', 'APPROVED', 'REJECTED', 'EXECUTED'))
);

-- Seed event types
INSERT INTO "ob-poc".event_types (event_type, description) VALUES
('DOCUMENT_UPLOADED', 'Document uploaded'),
('RFI_SENT', 'RFI sent to client'),
('RFI_ITEM_RECEIVED', 'Document received for RFI item'),
('RFI_COMPLETE', 'All required RFI items received'),
('OBSERVATION_CREATED', 'Observation extracted from document'),
('OBSERVATION_CONFLICT', 'Conflicting observations detected'),
('OWNERSHIP_CHANGED', 'CBU ownership structure modified'),
('ENTITY_ADDED', 'New entity added to CBU'),
('THRESHOLD_EVALUATED', 'Threshold evaluation completed'),
('SCREENING_COMPLETE', 'Screening completed'),
('CASE_STATE_CHANGED', 'KYC case state transition');

-- Indexes
CREATE INDEX idx_event_log_type ON "ob-poc".event_log(event_type);
CREATE INDEX idx_event_log_status ON "ob-poc".event_log(status);
CREATE INDEX idx_event_log_cbu ON "ob-poc".event_log(cbu_id);
CREATE INDEX idx_action_queue_status ON "ob-poc".action_queue(status);
```

### 5.2 Event DSL Verbs

**File**: `rust/config/verbs/kyc/automation.yaml`

```yaml
domains:
  automation:
    description: Event-driven automation
    verbs:
      emit-event:
        description: Emit event to event log
        behavior: crud
        crud:
          operation: insert
          table: event_log
          schema: ob-poc
          returning: event_id
        args:
        - name: event-type
          type: string
          required: true
          maps_to: event_type
        - name: payload
          type: json
          required: true
          maps_to: payload
        - name: cbu-id
          type: uuid
          required: false
          maps_to: cbu_id
        - name: case-id
          type: uuid
          required: false
          maps_to: case_id
        - name: entity-id
          type: uuid
          required: false
          maps_to: entity_id
        returns:
          type: uuid
          name: event_id
          capture: true

      queue-action:
        description: Queue action for human review
        behavior: crud
        crud:
          operation: insert
          table: action_queue
          schema: ob-poc
          returning: action_id
        args:
        - name: event-id
          type: uuid
          required: false
          maps_to: event_id
        - name: action-type
          type: string
          required: true
          maps_to: action_type
        - name: payload
          type: json
          required: true
          maps_to: payload
        - name: assigned-to
          type: string
          required: false
          maps_to: assigned_to
        - name: notes
          type: string
          required: false
          maps_to: notes
        returns:
          type: uuid
          name: action_id
          capture: true

      approve-action:
        description: Approve queued action
        behavior: crud
        crud:
          operation: update
          table: action_queue
          schema: ob-poc
          key: action_id
          set_values:
            status: APPROVED
            reviewed_at: now()
        args:
        - name: action-id
          type: uuid
          required: true
          maps_to: action_id
        - name: reviewed-by
          type: string
          required: true
          maps_to: reviewed_by
        - name: notes
          type: string
          required: false
          maps_to: notes
        returns:
          type: affected

      reject-action:
        description: Reject queued action
        behavior: crud
        crud:
          operation: update
          table: action_queue
          schema: ob-poc
          key: action_id
          set_values:
            status: REJECTED
            reviewed_at: now()
        args:
        - name: action-id
          type: uuid
          required: true
          maps_to: action_id
        - name: reviewed-by
          type: string
          required: true
          maps_to: reviewed_by
        - name: notes
          type: string
          required: true
          maps_to: notes
        returns:
          type: affected

      list-pending-actions:
        description: List pending actions for review
        behavior: crud
        crud:
          operation: select
          table: action_queue
          schema: ob-poc
        args:
        - name: assigned-to
          type: string
          required: false
          maps_to: assigned_to
        returns:
          type: record_set
```

### 5.3 KYC Case Re-evaluate Verb

**File**: Update `rust/config/verbs/kyc/kyc-case.yaml` to add:

```yaml
      reevaluate:
        description: Force full evaluation cycle
        behavior: plugin
        plugin:
          handler: kyc_case_reevaluate
        args:
        - name: case-id
          type: uuid
          required: true
        - name: reason
          type: string
          required: false
        returns:
          type: json
          capture: true
          description: |
            {
              threshold_result: ThresholdEvaluation,
              ubo_completeness: UboCompleteness,
              gaps: [Gap],
              recommended_actions: [string]
            }
```

---

## Phase 6: Go Test Harness

### 6.1 Purpose

Go service providing:
- Web UI for running DSL test scenarios
- DSL execution result display
- Database state inspection
- Manual event triggering

### 6.2 Project Structure

```
go-harness/
├── cmd/
│   └── harness/
│       └── main.go
├── internal/
│   ├── server/
│   │   ├── server.go
│   │   └── handlers.go
│   ├── dslclient/
│   │   └── client.go
│   ├── dbviewer/
│   │   └── queries.go
│   └── templates/
│       ├── layout.html
│       ├── scenarios.html
│       ├── execution.html
│       └── dbstate.html
├── test_scenarios/
│   ├── initial_kyc.yaml
│   ├── complex_ubo.yaml
│   └── rfi_loop.yaml
├── go.mod
└── go.sum
```

### 6.3 Test Scenario Format

```yaml
name: "Initial KYC for LuxSICAV"
description: "Complete initial KYC flow"

setup:
  - sql: |
      INSERT INTO "ob-poc".cbus (cbu_id, name, client_type, jurisdiction)
      VALUES ('{{cbu_id}}', 'Test Fund', 'LUXSICAV_UCITS', 'LU')

steps:
  - name: "Create KYC Case"
    dsl: |
      (kyc-case.create :cbu-id {{cbu_id}} :case-type "NEW_CLIENT" :as @case)
    expect:
      bindings:
        case: uuid

  - name: "Derive Thresholds"
    dsl: |
      (threshold.derive :cbu-id {{cbu_id}} :as @reqs)
    expect:
      result.risk_band: "LOW"

  - name: "Evaluate (should have gaps)"
    dsl: |
      (threshold.evaluate :cbu-id {{cbu_id}} :as @eval)
    expect:
      result.overall_status: "INCOMPLETE"
      result.gaps.length: "> 0"

  - name: "Generate RFI"
    dsl: |
      (rfi.generate :case-id @case :gaps @eval.gaps :as @rfi)
    expect:
      bindings:
        rfi: uuid

cleanup:
  - sql: |
      DELETE FROM "ob-poc".cbus WHERE cbu_id = '{{cbu_id}}'
```

---

## Implementation Order

### Week 1: Foundation
1. Phase 1 - DocumentTypeCode argument type
2. Phase 2 - Threshold schema + seeds
3. Phase 2 - threshold.derive plugin

### Week 2: Evaluation + RFI
4. Phase 2 - threshold.evaluate plugin
5. Phase 3 - RFI schema
6. Phase 3 - RFI verbs (CRUD)
7. Phase 3 - rfi.generate plugin

### Week 3: UBO Analysis
8. Phase 4 - compute_ownership_chains SQL function
9. Phase 4 - ubo.trace-chains plugin
10. Phase 4 - ubo.check-completeness plugin

### Week 4: Automation + Integration
11. Phase 5 - Event system schema
12. Phase 5 - Event/automation verbs
13. Phase 5 - kyc-case.reevaluate plugin

### Week 5: Go Harness
14. Phase 6 - Go project setup
15. Phase 6 - DSL client (HTTP to Rust server)
16. Phase 6 - Web UI + scenario runner

### Week 6: Testing
17. End-to-end: Initial KYC scenario
18. End-to-end: Complex UBO discovery
19. End-to-end: RFI loop with document upload

---

## File Checklist

### SQL Migrations
- [ ] `sql/migrations/020_threshold_matrix.sql`
- [ ] `sql/migrations/021_rfi_system.sql`
- [ ] `sql/migrations/022_ubo_chain_function.sql`
- [ ] `sql/migrations/023_event_system.sql`

### SQL Seeds
- [ ] `sql/seeds/threshold_matrix.sql`

### Verb YAML
- [ ] `rust/config/verbs/kyc/threshold.yaml`
- [ ] `rust/config/verbs/kyc/rfi.yaml`
- [ ] `rust/config/verbs/kyc/automation.yaml`
- [ ] Update `rust/config/verbs/ubo.yaml` (add trace-chains, check-completeness)
- [ ] Update `rust/config/verbs/kyc/kyc-case.yaml` (add reevaluate)

### Rust Plugins
- [ ] `rust/src/dsl_v2/custom_ops/threshold.rs`
- [ ] `rust/src/dsl_v2/custom_ops/rfi.rs`
- [ ] `rust/src/dsl_v2/custom_ops/ubo_analysis.rs`
- [ ] `rust/src/dsl_v2/custom_ops/kyc_reevaluate.rs`
- [ ] Update `rust/src/dsl_v2/custom_ops/mod.rs`

### Go Harness
- [ ] `go-harness/` project structure
- [ ] Test scenarios

---

## Testing Checklist

- [ ] Document type codes validate at DSL parse time
- [ ] threshold.derive computes correct risk band
- [ ] threshold.evaluate identifies all gap types
- [ ] rfi.generate creates items with correct acceptable docs
- [ ] rfi.request-document validates document type codes
- [ ] UBO chains compute correctly with recursive CTE
- [ ] Circular ownership detected (max depth 10)
- [ ] Event emission works
- [ ] Action queue workflow (SEMI_AUTO)
- [ ] kyc-case.reevaluate runs full cycle
- [ ] Go harness executes scenarios
