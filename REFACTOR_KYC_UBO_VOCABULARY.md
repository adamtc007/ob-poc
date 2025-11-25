# Refactor: KYC/UBO Vocabulary + Idempotent Execution

**Created:** 2025-11-25  
**Status:** SPECIFICATION FOR IMPLEMENTATION  
**Priority:** P0 — Core KYC Workflow  
**Scope:** New vocabulary, idempotent semantics, incremental DSL editing  

---

## Executive Summary

The KYC workflow is:
```
Alleged CBU → Collect Documents → Extract Data → Build/Extend UBO Graph → Assess → Decide
```

Each step is an **incremental DSL edit session**. Re-running the full DSL chain must be **idempotent** — same input always produces same DB state.

**Key Changes:**
1. New KYC/UBO vocabulary (~25 new words)
2. UPSERT semantics using natural keys (not blind INSERT)
3. `ensure` verbs for idempotent create-or-update
4. Remove duplicate words

---

## Part 1: Idempotent Execution Model

### 1.1 The Problem

Current:
```clojure
(cbu.create :cbu-name "AcmeFund")  ;; Run twice → TWO CBUs created
```

Needed:
```clojure
(cbu.ensure :cbu-name "AcmeFund")  ;; Run twice → ONE CBU, updated if changed
```

### 1.2 Natural Keys by Entity Type

| Entity Type | Natural Key | Table |
|-------------|-------------|-------|
| CBU | `name` + `jurisdiction` | `cbus` |
| Proper Person | `tax_id` OR `(first_name, last_name, date_of_birth)` | `entity_proper_persons` |
| Limited Company | `jurisdiction` + `company_number` | `entity_limited_companies` |
| Partnership | `jurisdiction` + `registration_number` | `entity_partnerships` |
| Trust | `jurisdiction` + `name` | `entity_trusts` |
| CBU-Entity-Role | `cbu_id` + `entity_id` + `role_id` | `cbu_entity_roles` (already UNIQUE) |
| Ownership Edge | `from_entity_id` + `to_entity_id` + `relationship_type` | `entity_role_connections` |

### 1.3 Verb Naming Convention

| Verb | Semantics | SQL |
|------|-----------|-----|
| `*.create` | INSERT, fail if exists | `INSERT` (error on conflict) |
| `*.ensure` | UPSERT — create if not exists, update if changed | `INSERT ON CONFLICT DO UPDATE` |
| `*.update` | UPDATE existing, fail if not exists | `UPDATE WHERE` |
| `*.delete` | DELETE if exists | `DELETE WHERE` |

**For KYC workflow, prefer `ensure` — idempotent by default.**

### 1.4 Schema Additions for Natural Keys

```sql
-- Add unique constraint for CBU natural key
ALTER TABLE "ob-poc".cbus 
ADD CONSTRAINT cbus_natural_key UNIQUE (name, jurisdiction);

-- Add unique constraint for ownership edges
ALTER TABLE "ob-poc".entity_role_connections 
ADD CONSTRAINT entity_role_connections_natural_key 
UNIQUE (source_entity_id, target_entity_id, relationship_type);

-- Limited companies already have company_number, add constraint
ALTER TABLE "ob-poc".entity_limited_companies
ADD CONSTRAINT limited_companies_natural_key 
UNIQUE (company_number) WHERE company_number IS NOT NULL;
```

---

## Part 2: KYC Case Lifecycle

### 2.1 Investigation Domain

A KYC investigation wraps the entire workflow for one CBU:

```clojure
;; Start investigation
(investigation.create
  :cbu-id @cbu
  :investigation-type "ENHANCED_DUE_DILIGENCE"  ;; or "STANDARD", "SIMPLIFIED"
  :risk-rating "HIGH"
  :regulatory-framework ["EU_5MLD" "US_BSA_AML"]
  :ubo-threshold 10.0          ;; ownership % to track
  :investigation-depth 5       ;; levels to traverse
  :deadline "2024-02-15")

;; Update status as work progresses
(investigation.update-status
  :investigation-id @inv
  :status "COLLECTING_DOCUMENTS")  ;; INITIATED, COLLECTING_DOCUMENTS, ANALYZING, PENDING_REVIEW, COMPLETE

;; Assign to analyst
(investigation.assign
  :investigation-id @inv
  :assignee "analyst@firm.com"
  :role "PRIMARY_ANALYST")

;; Complete
(investigation.complete
  :investigation-id @inv
  :outcome "APPROVED"  ;; APPROVED, REJECTED, CONDITIONAL, ESCALATED
  :notes "All UBOs identified and verified")
```

### 2.2 New Tables

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_investigations (
    investigation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_type VARCHAR(50) NOT NULL,  -- STANDARD, ENHANCED, SIMPLIFIED
    risk_rating VARCHAR(20),
    regulatory_framework JSONB,  -- ["EU_5MLD", "US_BSA_AML"]
    ubo_threshold NUMERIC(5,2) DEFAULT 10.0,
    investigation_depth INTEGER DEFAULT 5,
    status VARCHAR(50) DEFAULT 'INITIATED',
    deadline DATE,
    outcome VARCHAR(50),  -- NULL until complete
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS "ob-poc".investigation_assignments (
    assignment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID NOT NULL REFERENCES "ob-poc".kyc_investigations(investigation_id),
    assignee VARCHAR(255) NOT NULL,
    role VARCHAR(50),  -- PRIMARY_ANALYST, REVIEWER, APPROVER
    assigned_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(investigation_id, assignee, role)
);
```

---

## Part 3: Document Collection Workflow

### 3.1 Document Request & Acquisition

```clojure
;; Request specific document types
(document.request
  :investigation-id @inv
  :entity-id @company
  :document-type "CERTIFICATE_OF_INCORPORATION"
  :source "REGISTRY"           ;; REGISTRY, CLIENT, THIRD_PARTY
  :priority "HIGH"
  :due-date "2024-01-20")

;; Record document received
(document.receive
  :request-id @req
  :document-id @doc
  :received-from "Luxembourg Company Registry"
  :received-date "2024-01-15")

;; Verify document authenticity
(document.verify
  :document-id @doc
  :verification-method "REGISTRY_CHECK"
  :verification-status "VERIFIED"  ;; VERIFIED, FAILED, PENDING
  :verified-by "analyst@firm.com"
  :notes "Confirmed against LU registry")

;; Extract attributes from document
(document.extract-attributes
  :document-id @doc
  :attributes [
    {:attr-id "CBU.LEGAL_NAME" :value "Meridian Global Fund S.C.A."}
    {:attr-id "CBU.REGISTRATION_NUMBER" :value "LU-B234567"}
    {:attr-id "CBU.INCORPORATION_DATE" :value "2022-08-15"}
  ])
```

### 3.2 New Tables

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".document_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    document_type_code VARCHAR(100) NOT NULL,
    source VARCHAR(50),  -- REGISTRY, CLIENT, THIRD_PARTY
    priority VARCHAR(20) DEFAULT 'NORMAL',
    status VARCHAR(50) DEFAULT 'REQUESTED',  -- REQUESTED, RECEIVED, VERIFIED, REJECTED
    due_date DATE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CHECK ((entity_id IS NOT NULL) OR (cbu_id IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS "ob-poc".document_verifications (
    verification_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(document_id),
    verification_method VARCHAR(100),
    verification_status VARCHAR(50),
    verified_by VARCHAR(255),
    verification_date TIMESTAMPTZ DEFAULT NOW(),
    notes TEXT
);
```

---

## Part 4: UBO Graph Building

### 4.1 Ownership Edge Words

```clojure
;; Create ownership relationship (idempotent)
(entity.ensure-ownership
  :from-entity-id @holding-co
  :to-entity-id @target-co
  :ownership-percent 35.0
  :ownership-type "DIRECT"           ;; DIRECT, INDIRECT, BENEFICIAL
  :control-type "SHAREHOLDING"       ;; SHAREHOLDING, VOTING_RIGHTS, BOARD_CONTROL
  :effective-date "2024-01-01"
  :evidenced-by [@doc-1 @doc-2])

;; Update ownership (e.g., after share transfer)
(entity.update-ownership
  :from-entity-id @holding-co
  :to-entity-id @target-co
  :ownership-percent 45.0
  :effective-date "2024-06-01"
  :reason "Additional share acquisition")

;; Remove ownership (entity sold stake)
(entity.remove-ownership
  :from-entity-id @holding-co
  :to-entity-id @target-co
  :effective-date "2024-12-01")

;; Get full ownership chain to a target
(entity.get-ownership-chain
  :target-entity-id @fund
  :max-depth 7
  :threshold 10.0)
```

### 4.2 Trust Party Words

```clojure
;; Add trust party (settlor, trustee, beneficiary, protector)
(trust.add-party
  :trust-entity-id @trust
  :party-entity-id @person
  :party-role "BENEFICIARY"          ;; SETTLOR, TRUSTEE, BENEFICIARY, PROTECTOR
  :interest-percent 50.0             ;; for beneficiaries
  :is-discretionary true)

;; Add partnership interest
(partnership.add-partner
  :partnership-entity-id @partnership
  :partner-entity-id @company
  :partner-type "LIMITED"            ;; GENERAL, LIMITED
  :interest-percent 30.0
  :capital-contribution 1000000.00
  :effective-date "2024-01-01")
```

### 4.3 UBO Calculation Words

```clojure
;; Calculate UBOs for a CBU (traverses graph)
(ubo.calculate
  :cbu-id @cbu
  :threshold 10.0                    ;; minimum % to be UBO
  :max-depth 7                       ;; levels to traverse
  :algorithm "RECURSIVE_MULTIPLY")   ;; RECURSIVE_MULTIPLY, CUMULATIVE

;; Manually flag a UBO (override calculation)
(ubo.flag
  :cbu-id @cbu
  :entity-id @person
  :ownership-percent 8.0             ;; below threshold but...
  :flag-reason "MATERIAL_INFLUENCE"  ;; MATERIAL_INFLUENCE, CONTROL_WITHOUT_OWNERSHIP, PEP
  :flagged-by "analyst@firm.com")

;; Verify a calculated UBO
(ubo.verify
  :ubo-id @ubo
  :verification-method "DOCUMENT_REVIEW"
  :verified-by "analyst@firm.com"
  :notes "Confirmed via share register")

;; Clear a UBO flag (entity sold stake, etc.)
(ubo.clear
  :ubo-id @ubo
  :reason "OWNERSHIP_BELOW_THRESHOLD"
  :effective-date "2024-12-01")
```

---

## Part 5: Screening Suite

### 5.1 Screening Words

```clojure
;; PEP screening
(screening.pep
  :entity-id @person
  :databases ["WORLD_CHECK" "REFINITIV" "DOW_JONES"]
  :include-rca true)                 ;; Relatives and Close Associates

;; Sanctions screening
(screening.sanctions
  :entity-id @company
  :lists ["OFAC_SDN" "EU_SANCTIONS" "UK_HMT" "UN_SANCTIONS"])

;; Adverse media
(screening.adverse-media
  :entity-id @person
  :depth "DEEP"                      ;; QUICK, STANDARD, DEEP
  :languages ["EN" "DE" "FR"])

;; Record screening result
(screening.record-result
  :screening-id @screening
  :result "MATCH"                    ;; NO_MATCH, POTENTIAL_MATCH, MATCH, FALSE_POSITIVE
  :match-details {:list "OFAC_SDN" :match-score 95}
  :reviewed-by "analyst@firm.com")

;; Resolve match (false positive, true hit, etc.)
(screening.resolve
  :screening-id @screening
  :resolution "FALSE_POSITIVE"       ;; FALSE_POSITIVE, TRUE_HIT, ESCALATE
  :rationale "Different person - DOB mismatch"
  :resolved-by "analyst@firm.com")
```

### 5.2 New Tables

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".screenings (
    screening_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    screening_type VARCHAR(50) NOT NULL,  -- PEP, SANCTIONS, ADVERSE_MEDIA
    databases JSONB,
    status VARCHAR(50) DEFAULT 'PENDING',
    result VARCHAR(50),  -- NO_MATCH, POTENTIAL_MATCH, MATCH
    match_details JSONB,
    resolution VARCHAR(50),
    resolution_rationale TEXT,
    screened_at TIMESTAMPTZ DEFAULT NOW(),
    reviewed_by VARCHAR(255),
    resolved_by VARCHAR(255),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX idx_screenings_entity ON "ob-poc".screenings(entity_id);
CREATE INDEX idx_screenings_investigation ON "ob-poc".screenings(investigation_id);
```

---

## Part 6: Risk Assessment

### 6.1 Risk Words

```clojure
;; Assess entity risk
(risk.assess-entity
  :entity-id @person
  :factors ["PEP_STATUS" "JURISDICTION" "SOURCE_OF_WEALTH" "ADVERSE_MEDIA"])

;; Assess CBU overall risk
(risk.assess-cbu
  :cbu-id @cbu
  :methodology "FACTOR_WEIGHTED")    ;; FACTOR_WEIGHTED, HIGHEST_RISK, CUMULATIVE

;; Set risk rating (manual or calculated)
(risk.set-rating
  :cbu-id @cbu
  :rating "HIGH"                     ;; LOW, MEDIUM, MEDIUM_HIGH, HIGH, PROHIBITED
  :factors [
    {:factor "PEP_EXPOSURE" :rating "HIGH" :weight 0.3}
    {:factor "JURISDICTION" :rating "MEDIUM" :weight 0.2}
    {:factor "OWNERSHIP_COMPLEXITY" :rating "HIGH" :weight 0.3}
    {:factor "BUSINESS_RATIONALE" :rating "LOW" :weight 0.2}
  ]
  :rationale "PEP UBO with complex offshore structure"
  :assessed-by "analyst@firm.com")

;; Add risk note/flag
(risk.add-flag
  :cbu-id @cbu
  :flag-type "RED_FLAG"              ;; RED_FLAG, AMBER_FLAG, NOTE
  :description "Trust beneficiaries not disclosed"
  :flagged-by "analyst@firm.com")
```

### 6.2 New Tables

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".risk_assessments (
    assessment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id),
    assessment_type VARCHAR(50) NOT NULL,  -- CBU, ENTITY
    rating VARCHAR(20),
    factors JSONB,
    methodology VARCHAR(50),
    rationale TEXT,
    assessed_by VARCHAR(255),
    assessed_at TIMESTAMPTZ DEFAULT NOW(),
    CHECK ((cbu_id IS NOT NULL) OR (entity_id IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS "ob-poc".risk_flags (
    flag_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    flag_type VARCHAR(50) NOT NULL,  -- RED_FLAG, AMBER_FLAG, NOTE
    description TEXT,
    status VARCHAR(50) DEFAULT 'ACTIVE',  -- ACTIVE, RESOLVED, SUPERSEDED
    flagged_by VARCHAR(255),
    flagged_at TIMESTAMPTZ DEFAULT NOW(),
    resolved_by VARCHAR(255),
    resolved_at TIMESTAMPTZ,
    resolution_notes TEXT
);
```

---

## Part 7: Decision & Conditions

### 7.1 Decision Words

```clojure
;; Record onboarding decision
(decision.record
  :cbu-id @cbu
  :investigation-id @inv
  :decision "CONDITIONAL_ACCEPTANCE"  ;; ACCEPT, CONDITIONAL_ACCEPTANCE, REJECT, ESCALATE
  :decision-authority "SENIOR_MANAGEMENT"
  :rationale "PEP exposure acceptable with enhanced monitoring"
  :decided-by "senior.manager@firm.com")

;; Add condition to conditional acceptance
(decision.add-condition
  :decision-id @decision
  :condition-type "ENHANCED_MONITORING"
  :description "Quarterly review of all transactions >EUR 1M"
  :frequency "QUARTERLY"
  :due-date "2024-04-15"
  :assigned-to "compliance@firm.com")

(decision.add-condition
  :decision-id @decision
  :condition-type "TRANSACTION_LIMIT"
  :description "Pre-approval required for transactions >EUR 10M"
  :threshold 10000000
  :currency "EUR")

(decision.add-condition
  :decision-id @decision
  :condition-type "DOCUMENT_REQUIRED"
  :description "Obtain trust beneficiary disclosure within 6 months"
  :due-date "2024-07-15")

;; Mark condition as satisfied
(decision.satisfy-condition
  :condition-id @condition
  :satisfied-by "analyst@firm.com"
  :evidence "Trust beneficiaries disclosed - see doc-123"
  :satisfied-date "2024-05-10")
```

### 7.2 New Tables

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id),
    decision VARCHAR(50) NOT NULL,  -- ACCEPT, CONDITIONAL_ACCEPTANCE, REJECT, ESCALATE
    decision_authority VARCHAR(100),
    rationale TEXT,
    decided_by VARCHAR(255),
    decided_at TIMESTAMPTZ DEFAULT NOW(),
    effective_date DATE DEFAULT CURRENT_DATE,
    review_date DATE
);

CREATE TABLE IF NOT EXISTS "ob-poc".decision_conditions (
    condition_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id UUID NOT NULL REFERENCES "ob-poc".kyc_decisions(decision_id),
    condition_type VARCHAR(50) NOT NULL,
    description TEXT,
    frequency VARCHAR(50),  -- ONE_TIME, QUARTERLY, ANNUAL
    due_date DATE,
    threshold NUMERIC(20,2),
    currency VARCHAR(3),
    assigned_to VARCHAR(255),
    status VARCHAR(50) DEFAULT 'PENDING',  -- PENDING, SATISFIED, OVERDUE, WAIVED
    satisfied_by VARCHAR(255),
    satisfied_at TIMESTAMPTZ,
    satisfaction_evidence TEXT
);
```

---

## Part 8: Monitoring Setup

### 8.1 Monitoring Words

```clojure
;; Setup ongoing monitoring
(monitoring.setup
  :cbu-id @cbu
  :monitoring-level "ENHANCED"       ;; STANDARD, ENHANCED
  :components [
    {:type "TRANSACTION_MONITORING" :frequency "REAL_TIME" :threshold-factor 0.5}
    {:type "KYC_REFRESH" :frequency "ANNUAL"}
    {:type "PEP_SCREENING" :frequency "MONTHLY" :databases ["WORLD_CHECK"]}
    {:type "SANCTIONS_SCREENING" :frequency "DAILY"}
  ])

;; Record monitoring event
(monitoring.record-event
  :cbu-id @cbu
  :event-type "TRANSACTION_ALERT"
  :description "Large transaction EUR 15M to offshore account"
  :severity "HIGH"
  :requires-review true)

;; Schedule review
(monitoring.schedule-review
  :cbu-id @cbu
  :review-type "ANNUAL_KYC_REFRESH"
  :due-date "2025-01-15"
  :assigned-to "analyst@firm.com")
```

---

## Part 9: Vocabulary Summary

### 9.1 New Words (35 total)

| Domain | Words |
|--------|-------|
| **investigation** (4) | `investigation.create`, `investigation.update-status`, `investigation.assign`, `investigation.complete` |
| **document** (4 new) | `document.request`, `document.receive`, `document.verify` (enhance), `document.extract-attributes` (enhance) |
| **entity** (4 new) | `entity.ensure-ownership`, `entity.update-ownership`, `entity.remove-ownership`, `entity.get-ownership-chain` |
| **trust** (1) | `trust.add-party` |
| **partnership** (1) | `partnership.add-partner` |
| **ubo** (4) | `ubo.calculate` (implement), `ubo.flag`, `ubo.verify`, `ubo.clear` |
| **screening** (5) | `screening.pep`, `screening.sanctions`, `screening.adverse-media`, `screening.record-result`, `screening.resolve` |
| **risk** (4) | `risk.assess-entity`, `risk.assess-cbu`, `risk.set-rating`, `risk.add-flag` |
| **decision** (4) | `decision.record`, `decision.add-condition`, `decision.satisfy-condition`, `decision.review` |
| **monitoring** (4) | `monitoring.setup`, `monitoring.record-event`, `monitoring.schedule-review`, `monitoring.complete-review` |

### 9.2 Words to Remove (Duplicates)

| Remove | Keep |
|--------|------|
| `set-attribute` | `attr.set` |
| `require-attribute` | `attr.require` |
| `products.add` | `cbu.add-product` (new) |
| `products.configure` | `product.update` |
| `services.discover` | `service.discover` (rename) |
| `services.provision` | `cbu.add-service` (new) |
| `services.activate` | `service.activate` (rename) |

### 9.3 Words to Implement (Stubs → Real)

| Word | Current | Implementation |
|------|---------|----------------|
| `ubo.resolve-ubos` | Stub | Graph traversal, populate `ubo_registry` |
| `ubo.calculate-indirect-ownership` | Stub | Chain multiplication |
| `compliance.screen` | Generic | Route to `screening.*` words |

---

## Part 10: Idempotent Execution in CrudExecutor

### 10.1 UPSERT Pattern

```rust
// For CBU - use natural key (name, jurisdiction)
"CBU" => {
    let name = self.get_string_value(&values, "cbu-name")?;
    let jurisdiction = self.get_string_value(&values, "jurisdiction");
    
    // UPSERT: create if not exists, update if exists
    let cbu_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, nature_purpose, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, NOW(), NOW())
        ON CONFLICT (name, jurisdiction) 
        DO UPDATE SET 
            nature_purpose = COALESCE(EXCLUDED.nature_purpose, cbus.nature_purpose),
            updated_at = NOW()
        RETURNING cbu_id
        "#,
    )
    .bind(&name)
    .bind(&jurisdiction)
    .bind(&nature_purpose)
    .fetch_one(&self.pool)
    .await?;
    
    Ok(CrudExecutionResult { generated_id: Some(cbu_id), ... })
}

// For Limited Company - use natural key (company_number or jurisdiction+name)
"LIMITED_COMPANY" => {
    let company_number = self.get_string_value(&values, "company-number");
    let jurisdiction = self.get_string_value(&values, "jurisdiction");
    let name = self.get_string_value(&values, "name")?;
    
    // First ensure base entity exists
    let entity_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, jurisdiction, created_at)
        VALUES (gen_random_uuid(), $1, $2, $3, NOW())
        ON CONFLICT (name, jurisdiction) WHERE entity_type_id = $1
        DO UPDATE SET updated_at = NOW()
        RETURNING entity_id
        "#,
    )
    .bind(limited_company_type_id)
    .bind(&name)
    .bind(&jurisdiction)
    .fetch_one(&self.pool)
    .await?;
    
    // Then ensure extension exists
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".entity_limited_companies 
            (company_id, entity_id, company_number, incorporation_date)
        VALUES (gen_random_uuid(), $1, $2, $3)
        ON CONFLICT (entity_id) 
        DO UPDATE SET 
            company_number = COALESCE(EXCLUDED.company_number, entity_limited_companies.company_number)
        "#,
    )
    .bind(entity_id)
    .bind(&company_number)
    .bind(incorporation_date)
    .execute(&self.pool)
    .await?;
}

// For CBU-Entity-Role - already has UNIQUE constraint
"CBU_ENTITY_ROLE" => {
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id)
        VALUES (gen_random_uuid(), $1, $2, $3)
        ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
        "#,
    )
    .bind(cbu_id)
    .bind(entity_id)
    .bind(role_id)
    .execute(&self.pool)
    .await?;
}
```

### 10.2 Ensure Verbs in Words

```rust
// words.rs - add ensure variants

pub fn cbu_ensure(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "CBU".to_string(),
        values,
        conflict_keys: vec!["cbu-name".to_string(), "jurisdiction".to_string()],
    }));
    
    Ok(())
}

pub fn entity_ensure_limited_company(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "LIMITED_COMPANY".to_string(),
        values,
        conflict_keys: vec!["company-number".to_string()],
    }));
    
    Ok(())
}
```

### 10.3 New CrudStatement Variant

```rust
// value.rs - add DataUpsert

#[derive(Debug, Clone)]
pub enum CrudStatement {
    DataCreate(DataCreate),
    DataRead(DataRead),
    DataUpdate(DataUpdate),
    DataDelete(DataDelete),
    DataUpsert(DataUpsert),  // NEW
}

#[derive(Debug, Clone)]
pub struct DataUpsert {
    pub asset: String,
    pub values: HashMap<String, Value>,
    pub conflict_keys: Vec<String>,  // Natural key fields
}
```

---

## Part 11: Example Incremental KYC Session

```clojure
;; =============================================================================
;; SESSION 1: Initial CBU structure (alleged by client)
;; =============================================================================

;; Create investigation
(investigation.create
  :cbu-id nil                        ;; CBU doesn't exist yet
  :investigation-type "ENHANCED_DUE_DILIGENCE"
  :risk-rating "HIGH"
  :ubo-threshold 10.0
  :deadline "2024-02-15")

;; Create alleged CBU (idempotent)
(cbu.ensure
  :cbu-name "Meridian Global Fund"
  :jurisdiction "LU"
  :nature-purpose "Alternative investment fund"
  :client-type "SICAV")

;; Create alleged entities (idempotent)
(entity.ensure-limited-company
  :name "Meridian GP S.à r.l."
  :jurisdiction "LU"
  :company-number "LU-B234568"
  :role-in-structure "GENERAL_PARTNER")

(entity.ensure-proper-person
  :first-name "Chen"
  :last-name "Wei"
  :nationality "SG"
  :tax-id "SG-123456789")

;; Create alleged ownership structure (idempotent)
(entity.ensure-ownership
  :from-entity-id @gp
  :to-entity-id @fund
  :ownership-percent 0.1
  :control-type "GENERAL_PARTNER_CONTROL"
  :voting-rights 100.0)

;; Attach to CBU (idempotent)
(cbu.attach-entity :cbu-id @cbu :entity-id @gp :role "GeneralPartner")
(cbu.attach-entity :cbu-id @cbu :entity-id @chen-wei :role "BeneficialOwner" :ownership-percent 18.0)

;; =============================================================================
;; SESSION 2: Document collection
;; =============================================================================

;; Request documents
(document.request
  :investigation-id @inv
  :entity-id @fund
  :document-type "CERTIFICATE_OF_INCORPORATION"
  :source "LUXEMBOURG_REGISTRY"
  :priority "HIGH")

(document.request
  :investigation-id @inv
  :entity-id @chen-wei
  :document-type "PASSPORT"
  :source "CLIENT"
  :priority "HIGH")

;; Receive and verify (later)
(document.receive
  :request-id @req-1
  :document-id "doc-incorp-001")

(document.verify
  :document-id "doc-incorp-001"
  :verification-status "VERIFIED"
  :verified-by "analyst@firm.com")

;; =============================================================================
;; SESSION 3: Data extraction - extend CBU model
;; =============================================================================

(document.extract-attributes
  :document-id "doc-incorp-001"
  :cbu-id @cbu
  :attributes [
    {:attr-id "CBU.LEGAL_NAME" :value "Meridian Global Investment Fund S.C.A."}
    {:attr-id "CBU.REGISTRATION_NUMBER" :value "LU-B234567"}
    {:attr-id "CBU.INCORPORATION_DATE" :value "2022-08-15"}
  ])

;; Update CBU with verified info (idempotent - same natural key)
(cbu.ensure
  :cbu-name "Meridian Global Investment Fund S.C.A."  ;; Corrected name
  :jurisdiction "LU"
  :nature-purpose "Alternative investment fund"
  :registration-number "LU-B234567")

;; =============================================================================
;; SESSION 4: Screening
;; =============================================================================

(screening.pep
  :entity-id @chen-wei
  :databases ["WORLD_CHECK" "REFINITIV"])

(screening.record-result
  :screening-id @pep-screening
  :result "MATCH"
  :match-details {:pep-type "FOREIGN_PROMINENT_PUBLIC_OFFICIAL" 
                  :position "Former Minister of Trade, Singapore"})

(screening.sanctions
  :entity-id @chen-wei
  :lists ["OFAC_SDN" "EU_SANCTIONS" "UN_SANCTIONS"])

(screening.record-result
  :screening-id @sanctions-screening
  :result "NO_MATCH")

;; =============================================================================
;; SESSION 5: UBO calculation & risk assessment
;; =============================================================================

;; Calculate UBOs (idempotent - recalculates and updates ubo_registry)
(ubo.calculate
  :cbu-id @cbu
  :threshold 10.0
  :max-depth 5)

;; Risk assessment
(risk.assess-cbu
  :cbu-id @cbu
  :methodology "FACTOR_WEIGHTED")

(risk.set-rating
  :cbu-id @cbu
  :rating "HIGH"
  :factors [
    {:factor "PEP_EXPOSURE" :rating "HIGH"}
    {:factor "JURISDICTION" :rating "MEDIUM"}
  ]
  :rationale "UBO is former PEP")

;; =============================================================================
;; SESSION 6: Decision
;; =============================================================================

(decision.record
  :cbu-id @cbu
  :investigation-id @inv
  :decision "CONDITIONAL_ACCEPTANCE"
  :decision-authority "SENIOR_MANAGEMENT"
  :rationale "PEP risk acceptable with enhanced monitoring")

(decision.add-condition
  :decision-id @decision
  :condition-type "ENHANCED_MONITORING"
  :frequency "QUARTERLY")

(decision.add-condition
  :decision-id @decision
  :condition-type "TRANSACTION_APPROVAL"
  :threshold 10000000
  :currency "EUR")

;; Complete investigation
(investigation.complete
  :investigation-id @inv
  :outcome "CONDITIONAL_ACCEPTANCE")

;; Setup monitoring
(monitoring.setup
  :cbu-id @cbu
  :monitoring-level "ENHANCED")

;; =============================================================================
;; RE-RUN ENTIRE DSL: Same DB state (idempotent)
;; =============================================================================
```

---

## Part 12: Files to Create/Modify

### Create

| File | Purpose |
|------|---------|
| `sql/migrations/20251125_kyc_investigation_tables.sql` | New KYC tables |
| `rust/src/database/investigation_service.rs` | Investigation CRUD |
| `rust/src/database/screening_service.rs` | Screening operations |
| `rust/src/database/risk_service.rs` | Risk assessment operations |
| `rust/src/database/decision_service.rs` | Decision & conditions |
| `rust/src/database/monitoring_service.rs` | Monitoring setup |

### Modify

| File | Changes |
|------|---------|
| `rust/src/forth_engine/value.rs` | Add `DataUpsert` variant |
| `rust/src/forth_engine/words.rs` | Add ~35 new word implementations |
| `rust/src/forth_engine/vocab_registry.rs` | Register new words, remove duplicates |
| `rust/src/database/crud_executor.rs` | Add UPSERT handling, route new asset types |
| `rust/src/database/entity_service.rs` | Add ownership edge methods |
| `rust/src/database/mod.rs` | Export new services |
| `sql/00_MASTER_SCHEMA_CONSOLIDATED.sql` | Add unique constraints for idempotency |

---

## Part 13: Implementation Order

1. **Schema changes** — Add unique constraints, new tables
2. **DataUpsert in value.rs** — New CrudStatement variant
3. **Services** — Investigation, Screening, Risk, Decision, Monitoring
4. **CrudExecutor** — UPSERT routing, new asset types
5. **Words** — Implement all 35 new words
6. **Vocab registry** — Register new words, remove duplicates
7. **Integration test** — Run full KYC session twice, verify idempotent

---

## Summary

| Aspect | Before | After |
|--------|--------|-------|
| KYC Words | 8 (thin, stubs) | 43 (full workflow) |
| Idempotency | None (duplicates on re-run) | Full (UPSERT on natural keys) |
| UBO Graph | Schema only | Words + calculation |
| Screening | Generic | PEP, Sanctions, Adverse Media |
| Risk | None | Assessment + rating |
| Decision | Binary finalize | Conditional + conditions tracking |
| Monitoring | None | Setup + events |

This delivers a complete KYC/UBO workflow that supports incremental DSL editing with idempotent execution.
