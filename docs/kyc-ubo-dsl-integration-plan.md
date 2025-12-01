# KYC/UBO DSL Integration Plan

**Document**: `kyc-ubo-dsl-integration-plan.md`  
**Created**: 2025-12-01  
**Status**: READY FOR IMPLEMENTATION  
**Approach**: Integrate existing pieces, add missing verbs, create orchestration layer

---

## Objective

Link KYC/UBO DSL domains to Custody DSL via unified CBU model, enabling:

```
Onboarding Request
        │
        ├──────────────────┬──────────────────┐
        ▼                  ▼                  ▼
   DSL.Custody        DSL.KYC            DSL.KYC.UBO
   (Products)         (Documents)        (Ownership)
        │                  │                  │
        └──────────────────┴──────────────────┘
                           │
                           ▼
                    Same CBU Record
```

---

## Phase 0: Audit Existing Infrastructure

### Task 0.1: Audit Existing Tables

Check what already exists in database schemas:

```sql
-- Check ob-poc schema for entity/ownership tables
\dt "ob-poc".*

-- Look for existing KYC-related tables
\dt kyc.*

-- Check entity_ownership_links or similar
SELECT column_name, data_type 
FROM information_schema.columns 
WHERE table_name = 'entity_ownership_links';

-- Check documents table
SELECT column_name, data_type 
FROM information_schema.columns 
WHERE table_name = 'documents';
```

**Expected existing tables** (verify):
- `"ob-poc".entities` - Core entity table
- `"ob-poc".entity_ownership_links` - Ownership relationships
- `"ob-poc".documents` - Document storage
- `"ob-poc".cbus` - CBU records

### Task 0.2: Audit Existing Verbs

Check `rust/config/verbs.yaml` for existing KYC/UBO verbs:

```bash
grep -A 20 "^kyc:" rust/config/verbs.yaml
grep -A 20 "^ubo:" rust/config/verbs.yaml
grep -A 20 "^entity:" rust/config/verbs.yaml
```

Document what exists vs what's missing.

### Task 0.3: Audit Entity Model

Review entity structure in codebase:

```bash
grep -r "entity_id" rust/src/
grep -r "EntityType" rust/src/
```

**Effort**: 0.5 day

---

## Phase 1: Schema Additions (If Needed)

Based on audit, add missing tables. Create `kyc` schema if not exists.

### File: `migrations/YYYYMMDD_kyc_schema.sql`

```sql
-- Create KYC schema if not exists
CREATE SCHEMA IF NOT EXISTS kyc;

-- Document Requirements (what's needed for KYC)
CREATE TABLE IF NOT EXISTS kyc.document_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    document_type VARCHAR(50) NOT NULL,
    priority VARCHAR(20) DEFAULT 'STANDARD',  -- CRITICAL, HIGH, STANDARD, LOW
    due_date DATE,
    status VARCHAR(20) DEFAULT 'REQUIRED',    -- REQUIRED, SUBMITTED, VERIFIED, REJECTED, WAIVED
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- Submitted Documents
CREATE TABLE IF NOT EXISTS kyc.submitted_documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requirement_id UUID REFERENCES kyc.document_requirements(requirement_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    document_type VARCHAR(50) NOT NULL,
    file_reference TEXT,                       -- S3 key, file path, etc.
    original_filename TEXT,
    submitted_date DATE DEFAULT CURRENT_DATE,
    expiry_date DATE,
    status VARCHAR(20) DEFAULT 'PENDING',      -- PENDING, VERIFIED, REJECTED, EXPIRED
    verified_by TEXT,
    verified_date DATE,
    rejection_reason TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- KYC Status per Entity (rolled up)
CREATE TABLE IF NOT EXISTS kyc.entity_kyc_status (
    status_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    kyc_status VARCHAR(20) DEFAULT 'NOT_STARTED',  -- NOT_STARTED, IN_PROGRESS, PENDING_REVIEW, APPROVED, REJECTED
    risk_rating VARCHAR(20),                        -- LOW, MEDIUM, HIGH, PROHIBITED
    last_review_date DATE,
    next_review_date DATE,
    reviewer TEXT,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(entity_id, cbu_id)
);

-- Ownership Links (if not in ob-poc schema)
CREATE TABLE IF NOT EXISTS kyc.ownership_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    child_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ownership_percentage DECIMAL(5,2),
    ownership_type VARCHAR(20) DEFAULT 'DIRECT',  -- DIRECT, INDIRECT, BENEFICIAL
    effective_date DATE DEFAULT CURRENT_DATE,
    end_date DATE,
    is_active BOOLEAN DEFAULT true,
    verified BOOLEAN DEFAULT false,
    verified_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    CONSTRAINT valid_percentage CHECK (ownership_percentage >= 0 AND ownership_percentage <= 100)
);

-- Control Links (non-ownership control)
CREATE TABLE IF NOT EXISTS kyc.control_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    controller_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    controlled_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    control_type VARCHAR(30) NOT NULL,  -- BOARD_CONTROL, VOTING_RIGHTS, VETO_POWER, MANAGEMENT, OTHER
    description TEXT,
    effective_date DATE DEFAULT CURRENT_DATE,
    end_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- UBO Determinations (computed/confirmed UBOs)
CREATE TABLE IF NOT EXISTS kyc.ubo_determinations (
    determination_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    is_ubo BOOLEAN NOT NULL,
    effective_percentage DECIMAL(5,2),           -- Computed through chain
    determination_basis TEXT,                    -- Why this entity is/isn't a UBO
    threshold_used DECIMAL(5,2) DEFAULT 25.00,   -- Regulatory threshold applied
    determination_date DATE DEFAULT CURRENT_DATE,
    determined_by TEXT,
    status VARCHAR(20) DEFAULT 'PENDING',        -- PENDING, CONFIRMED, DISPUTED
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(cbu_id, entity_id)
);

-- Sanctions Screenings
CREATE TABLE IF NOT EXISTS kyc.sanctions_screenings (
    screening_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    screening_provider VARCHAR(50),              -- REFINITIV, DOW_JONES, LEXISNEXIS, etc.
    screening_type VARCHAR(30),                  -- SANCTIONS, PEP, ADVERSE_MEDIA
    result VARCHAR(20) NOT NULL,                 -- CLEAR, MATCH, POTENTIAL_MATCH, ERROR
    match_details JSONB,
    screening_date DATE DEFAULT CURRENT_DATE,
    reviewed_by TEXT,
    review_date DATE,
    review_notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_doc_req_cbu ON kyc.document_requirements(cbu_id);
CREATE INDEX IF NOT EXISTS idx_doc_req_entity ON kyc.document_requirements(entity_id);
CREATE INDEX IF NOT EXISTS idx_doc_req_status ON kyc.document_requirements(status);
CREATE INDEX IF NOT EXISTS idx_ownership_parent ON kyc.ownership_links(parent_entity_id);
CREATE INDEX IF NOT EXISTS idx_ownership_child ON kyc.ownership_links(child_entity_id);
CREATE INDEX IF NOT EXISTS idx_ubo_cbu ON kyc.ubo_determinations(cbu_id);
CREATE INDEX IF NOT EXISTS idx_screening_entity ON kyc.sanctions_screenings(entity_id);
```

**Note**: If tables already exist in `"ob-poc"` schema, create views or use existing. Don't duplicate.

**Effort**: 0.5 day (mostly conditional on audit)

---

## Phase 2: KYC Verb Definitions

### File: `rust/config/verbs.yaml` - Add KYC Domain

```yaml
kyc:
  description: "KYC document collection and verification"
  schema: kyc
  
  verbs:
    # Document Requirements
    require-document:
      description: "Add a document requirement for an entity"
      behavior: crud
      crud:
        operation: upsert
        table: document_requirements
        schema: kyc
        conflict_keys: [cbu_id, entity_id, document_type]
        returning: requirement_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: document-type
          type: string
          required: true
          maps_to: document_type
          valid_values: [CERT_OF_INCORPORATION, BOARD_RESOLUTION, AUTHORIZED_SIGNATORIES, 
                         PROOF_OF_ADDRESS, TAX_ID_CERTIFICATE, FINANCIAL_STATEMENTS,
                         PASSPORT, DRIVERS_LICENSE, NATIONAL_ID, UTILITY_BILL,
                         TRUST_DEED, PARTNERSHIP_AGREEMENT, SHAREHOLDER_REGISTER,
                         SOURCE_OF_FUNDS, SOURCE_OF_WEALTH, BANK_REFERENCE]
        - name: priority
          type: string
          required: false
          maps_to: priority
          valid_values: [CRITICAL, HIGH, STANDARD, LOW]
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
        name: requirement_id
        capture: true

    submit-document:
      description: "Record a document submission"
      behavior: crud
      crud:
        operation: insert
        table: submitted_documents
        schema: kyc
        returning: document_id
      args:
        - name: requirement-id
          type: uuid
          required: false
          maps_to: requirement_id
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: document-type
          type: string
          required: true
          maps_to: document_type
        - name: file-reference
          type: string
          required: true
          maps_to: file_reference
        - name: original-filename
          type: string
          required: false
          maps_to: original_filename
        - name: expiry-date
          type: date
          required: false
          maps_to: expiry_date
      returns:
        type: uuid
        name: document_id
        capture: true

    verify-document:
      description: "Mark a document as verified"
      behavior: crud
      crud:
        operation: update
        table: submitted_documents
        schema: kyc
        key: document_id
      args:
        - name: document-id
          type: uuid
          required: true
          maps_to: document_id
        - name: verified-by
          type: string
          required: true
          maps_to: verified_by
        - name: verified-date
          type: date
          required: false
          maps_to: verified_date
        - name: status
          type: string
          required: false
          maps_to: status
          valid_values: [VERIFIED]
      returns:
        type: affected

    reject-document:
      description: "Reject a submitted document"
      behavior: crud
      crud:
        operation: update
        table: submitted_documents
        schema: kyc
        key: document_id
      args:
        - name: document-id
          type: uuid
          required: true
          maps_to: document_id
        - name: reason
          type: string
          required: true
          maps_to: rejection_reason
      returns:
        type: affected

    set-kyc-status:
      description: "Set overall KYC status for an entity within a CBU"
      behavior: crud
      crud:
        operation: upsert
        table: entity_kyc_status
        schema: kyc
        conflict_keys: [entity_id, cbu_id]
        returning: status_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: status
          type: string
          required: true
          maps_to: kyc_status
          valid_values: [NOT_STARTED, IN_PROGRESS, PENDING_REVIEW, APPROVED, REJECTED]
        - name: risk-rating
          type: string
          required: false
          maps_to: risk_rating
          valid_values: [LOW, MEDIUM, HIGH, PROHIBITED]
        - name: reviewer
          type: string
          required: false
          maps_to: reviewer
        - name: notes
          type: string
          required: false
          maps_to: notes
      returns:
        type: uuid
        name: status_id
        capture: false

    list-requirements:
      description: "List document requirements for a CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: document_requirements
        schema: kyc
        fk_col: cbu_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
      returns:
        type: record_set

    list-documents:
      description: "List submitted documents for an entity"
      behavior: crud
      crud:
        operation: list_by_fk
        table: submitted_documents
        schema: kyc
        fk_col: entity_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
      returns:
        type: record_set

    list-outstanding:
      description: "List outstanding (unfulfilled) document requirements for a CBU"
      behavior: plugin
      handler: list_outstanding_requirements
      args:
        - name: cbu-id
          type: uuid
          required: true
      returns:
        type: record_set

    screen-sanctions:
      description: "Record sanctions screening result"
      behavior: crud
      crud:
        operation: insert
        table: sanctions_screenings
        schema: kyc
        returning: screening_id
      args:
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: provider
          type: string
          required: true
          maps_to: screening_provider
          valid_values: [REFINITIV, DOW_JONES, LEXISNEXIS, COMPLY_ADVANTAGE, INTERNAL]
        - name: screening-type
          type: string
          required: false
          maps_to: screening_type
          valid_values: [SANCTIONS, PEP, ADVERSE_MEDIA, FULL]
        - name: result
          type: string
          required: true
          maps_to: result
          valid_values: [CLEAR, MATCH, POTENTIAL_MATCH, ERROR]
        - name: match-details
          type: json
          required: false
          maps_to: match_details
      returns:
        type: uuid
        name: screening_id
        capture: true
```

**Effort**: 0.5 day

---

## Phase 3: UBO Verb Definitions

### File: `rust/config/verbs.yaml` - Add UBO Domain

```yaml
ubo:
  description: "UBO ownership and control mapping"
  schema: kyc
  
  verbs:
    add-ownership:
      description: "Add ownership link between entities"
      behavior: crud
      crud:
        operation: insert
        table: ownership_links
        schema: kyc
        returning: link_id
      args:
        - name: parent-entity-id
          type: uuid
          required: true
          maps_to: parent_entity_id
        - name: child-entity-id
          type: uuid
          required: true
          maps_to: child_entity_id
        - name: percentage
          type: decimal
          required: true
          maps_to: ownership_percentage
        - name: ownership-type
          type: string
          required: false
          maps_to: ownership_type
          valid_values: [DIRECT, INDIRECT, BENEFICIAL]
        - name: effective-date
          type: date
          required: false
          maps_to: effective_date
      returns:
        type: uuid
        name: link_id
        capture: true

    update-ownership:
      description: "Update ownership percentage or type"
      behavior: crud
      crud:
        operation: update
        table: ownership_links
        schema: kyc
        key: link_id
      args:
        - name: link-id
          type: uuid
          required: true
          maps_to: link_id
        - name: percentage
          type: decimal
          required: false
          maps_to: ownership_percentage
        - name: ownership-type
          type: string
          required: false
          maps_to: ownership_type
          valid_values: [DIRECT, INDIRECT, BENEFICIAL]
        - name: end-date
          type: date
          required: false
          maps_to: end_date
      returns:
        type: affected

    remove-ownership:
      description: "End an ownership link"
      behavior: crud
      crud:
        operation: update
        table: ownership_links
        schema: kyc
        key: link_id
        set_values:
          is_active: false
      args:
        - name: link-id
          type: uuid
          required: true
          maps_to: link_id
        - name: end-date
          type: date
          required: false
          maps_to: end_date
      returns:
        type: affected

    add-control:
      description: "Add control relationship (non-ownership)"
      behavior: crud
      crud:
        operation: insert
        table: control_links
        schema: kyc
        returning: link_id
      args:
        - name: controller-entity-id
          type: uuid
          required: true
          maps_to: controller_entity_id
        - name: controlled-entity-id
          type: uuid
          required: true
          maps_to: controlled_entity_id
        - name: control-type
          type: string
          required: true
          maps_to: control_type
          valid_values: [BOARD_CONTROL, VOTING_RIGHTS, VETO_POWER, MANAGEMENT, TRUSTEE, OTHER]
        - name: description
          type: string
          required: false
          maps_to: description
        - name: effective-date
          type: date
          required: false
          maps_to: effective_date
      returns:
        type: uuid
        name: link_id
        capture: true

    remove-control:
      description: "End a control relationship"
      behavior: crud
      crud:
        operation: update
        table: control_links
        schema: kyc
        key: link_id
        set_values:
          is_active: false
      args:
        - name: link-id
          type: uuid
          required: true
          maps_to: link_id
        - name: end-date
          type: date
          required: false
          maps_to: end_date
      returns:
        type: affected

    determine-ubo:
      description: "Record UBO determination for an entity"
      behavior: crud
      crud:
        operation: upsert
        table: ubo_determinations
        schema: kyc
        conflict_keys: [cbu_id, entity_id]
        returning: determination_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: is-ubo
          type: boolean
          required: true
          maps_to: is_ubo
        - name: effective-percentage
          type: decimal
          required: false
          maps_to: effective_percentage
        - name: basis
          type: string
          required: false
          maps_to: determination_basis
        - name: threshold
          type: decimal
          required: false
          maps_to: threshold_used
        - name: determined-by
          type: string
          required: false
          maps_to: determined_by
      returns:
        type: uuid
        name: determination_id
        capture: true

    calculate-ownership-chain:
      description: "Calculate effective ownership through chain (plugin - recursive)"
      behavior: plugin
      handler: calculate_ownership_chain
      args:
        - name: cbu-id
          type: uuid
          required: true
        - name: threshold
          type: decimal
          required: false
          description: "UBO threshold percentage (default 25)"
      returns:
        type: record_set
        description: "List of entities with effective ownership >= threshold"

    list-ownership-chain:
      description: "List all ownership links for an entity (up and down)"
      behavior: plugin
      handler: list_ownership_chain
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: direction
          type: string
          required: false
          valid_values: [UP, DOWN, BOTH]
      returns:
        type: record_set

    list-ubos:
      description: "List confirmed UBOs for a CBU"
      behavior: crud
      crud:
        operation: list_by_fk
        table: ubo_determinations
        schema: kyc
        fk_col: cbu_id
        filter:
          is_ubo: true
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
      returns:
        type: record_set

    verify-identity:
      description: "Record identity verification for an individual"
      behavior: plugin
      handler: verify_entity_identity
      args:
        - name: entity-id
          type: uuid
          required: true
        - name: id-type
          type: string
          required: true
          valid_values: [PASSPORT, DRIVERS_LICENSE, NATIONAL_ID, TAX_ID]
        - name: id-number
          type: string
          required: true
        - name: id-country
          type: string
          required: true
        - name: expiry-date
          type: date
          required: false
        - name: verified-date
          type: date
          required: false
      returns:
        type: affected
```

**Effort**: 0.5 day

---

## Phase 4: Plugin Handlers for Complex Operations

### File: `rust/src/dsl_v2/custom_ops/kyc_ops.rs`

```rust
use anyhow::Result;
use sqlx::PgPool;
use serde_json::{json, Value};
use uuid::Uuid;

/// List outstanding document requirements (not yet fulfilled)
pub async fn list_outstanding_requirements(
    pool: &PgPool,
    args: &std::collections::HashMap<String, Value>,
) -> Result<Value> {
    let cbu_id: Uuid = args.get("cbu-id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing cbu-id"))?;
    
    let rows = sqlx::query!(
        r#"
        SELECT 
            dr.requirement_id,
            dr.entity_id,
            e.legal_name as entity_name,
            dr.document_type,
            dr.priority,
            dr.due_date,
            dr.status,
            dr.notes
        FROM kyc.document_requirements dr
        JOIN "ob-poc".entities e ON e.entity_id = dr.entity_id
        WHERE dr.cbu_id = $1 
          AND dr.status IN ('REQUIRED', 'REJECTED')
        ORDER BY 
            CASE dr.priority 
                WHEN 'CRITICAL' THEN 1 
                WHEN 'HIGH' THEN 2 
                WHEN 'STANDARD' THEN 3 
                ELSE 4 
            END,
            dr.due_date NULLS LAST
        "#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    
    let results: Vec<Value> = rows.iter().map(|r| json!({
        "requirement_id": r.requirement_id,
        "entity_id": r.entity_id,
        "entity_name": r.entity_name,
        "document_type": r.document_type,
        "priority": r.priority,
        "due_date": r.due_date,
        "status": r.status,
        "notes": r.notes
    })).collect();
    
    Ok(json!(results))
}

/// Calculate effective ownership through the chain using recursive CTE
pub async fn calculate_ownership_chain(
    pool: &PgPool,
    args: &std::collections::HashMap<String, Value>,
) -> Result<Value> {
    let cbu_id: Uuid = args.get("cbu-id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing cbu-id"))?;
    
    let threshold: f64 = args.get("threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(25.0);
    
    // Get the root entity for this CBU
    let cbu = sqlx::query!(
        r#"SELECT entity_id FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_one(pool)
    .await?;
    
    let root_entity_id = cbu.entity_id;
    
    // Recursive CTE to walk ownership chain upward
    let rows = sqlx::query!(
        r#"
        WITH RECURSIVE ownership_chain AS (
            -- Base case: direct owners of the root entity
            SELECT 
                ol.parent_entity_id as entity_id,
                ol.ownership_percentage as effective_pct,
                1 as depth,
                ARRAY[ol.child_entity_id, ol.parent_entity_id] as path
            FROM kyc.ownership_links ol
            WHERE ol.child_entity_id = $1 
              AND ol.is_active = true
            
            UNION ALL
            
            -- Recursive case: owners of owners
            SELECT 
                ol.parent_entity_id as entity_id,
                (oc.effective_pct * ol.ownership_percentage / 100.0)::DECIMAL(5,2) as effective_pct,
                oc.depth + 1,
                oc.path || ol.parent_entity_id
            FROM kyc.ownership_links ol
            JOIN ownership_chain oc ON ol.child_entity_id = oc.entity_id
            WHERE ol.is_active = true
              AND oc.depth < 10  -- Prevent infinite loops
              AND NOT ol.parent_entity_id = ANY(oc.path)  -- Prevent cycles
        )
        SELECT DISTINCT ON (entity_id)
            oc.entity_id,
            e.legal_name,
            e.entity_type,
            e.jurisdiction,
            SUM(oc.effective_pct) OVER (PARTITION BY oc.entity_id) as total_effective_pct,
            MIN(oc.depth) OVER (PARTITION BY oc.entity_id) as min_depth
        FROM ownership_chain oc
        JOIN "ob-poc".entities e ON e.entity_id = oc.entity_id
        ORDER BY entity_id, depth
        "#,
        root_entity_id
    )
    .fetch_all(pool)
    .await?;
    
    // Filter by threshold and identify UBOs
    let ubos: Vec<Value> = rows.iter()
        .filter(|r| r.total_effective_pct.map(|p| f64::from(p) >= threshold).unwrap_or(false))
        .map(|r| json!({
            "entity_id": r.entity_id,
            "legal_name": r.legal_name,
            "entity_type": r.entity_type,
            "jurisdiction": r.jurisdiction,
            "effective_percentage": r.total_effective_pct,
            "depth": r.min_depth,
            "is_ubo": true
        }))
        .collect();
    
    Ok(json!({
        "cbu_id": cbu_id,
        "root_entity_id": root_entity_id,
        "threshold": threshold,
        "ubos": ubos
    }))
}

/// List ownership chain for an entity (up/down/both)
pub async fn list_ownership_chain(
    pool: &PgPool,
    args: &std::collections::HashMap<String, Value>,
) -> Result<Value> {
    let entity_id: Uuid = args.get("entity-id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing entity-id"))?;
    
    let direction = args.get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("BOTH");
    
    let mut result = json!({
        "entity_id": entity_id,
        "owners": [],
        "owned": []
    });
    
    // Get owners (UP)
    if direction == "UP" || direction == "BOTH" {
        let owners = sqlx::query!(
            r#"
            SELECT 
                ol.link_id,
                ol.parent_entity_id,
                e.legal_name,
                e.entity_type,
                ol.ownership_percentage,
                ol.ownership_type
            FROM kyc.ownership_links ol
            JOIN "ob-poc".entities e ON e.entity_id = ol.parent_entity_id
            WHERE ol.child_entity_id = $1 AND ol.is_active = true
            "#,
            entity_id
        )
        .fetch_all(pool)
        .await?;
        
        result["owners"] = json!(owners.iter().map(|r| json!({
            "link_id": r.link_id,
            "entity_id": r.parent_entity_id,
            "legal_name": r.legal_name,
            "entity_type": r.entity_type,
            "percentage": r.ownership_percentage,
            "ownership_type": r.ownership_type
        })).collect::<Vec<_>>());
    }
    
    // Get owned entities (DOWN)
    if direction == "DOWN" || direction == "BOTH" {
        let owned = sqlx::query!(
            r#"
            SELECT 
                ol.link_id,
                ol.child_entity_id,
                e.legal_name,
                e.entity_type,
                ol.ownership_percentage,
                ol.ownership_type
            FROM kyc.ownership_links ol
            JOIN "ob-poc".entities e ON e.entity_id = ol.child_entity_id
            WHERE ol.parent_entity_id = $1 AND ol.is_active = true
            "#,
            entity_id
        )
        .fetch_all(pool)
        .await?;
        
        result["owned"] = json!(owned.iter().map(|r| json!({
            "link_id": r.link_id,
            "entity_id": r.child_entity_id,
            "legal_name": r.legal_name,
            "entity_type": r.entity_type,
            "percentage": r.ownership_percentage,
            "ownership_type": r.ownership_type
        })).collect::<Vec<_>>());
    }
    
    Ok(result)
}

/// Verify entity identity (updates entity record)
pub async fn verify_entity_identity(
    pool: &PgPool,
    args: &std::collections::HashMap<String, Value>,
) -> Result<Value> {
    let entity_id: Uuid = args.get("entity-id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing entity-id"))?;
    
    let id_type = args.get("id-type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing id-type"))?;
    
    let id_number = args.get("id-number")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing id-number"))?;
    
    let id_country = args.get("id-country")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing id-country"))?;
    
    // Store in entity's identity_documents JSONB or separate table
    // This is simplified - might want a dedicated table
    let result = sqlx::query!(
        r#"
        UPDATE "ob-poc".entities 
        SET 
            tax_id = $2,
            tax_country = $3,
            updated_at = now()
        WHERE entity_id = $1
        RETURNING entity_id
        "#,
        entity_id,
        id_number,
        id_country
    )
    .fetch_one(pool)
    .await?;
    
    Ok(json!({
        "entity_id": result.entity_id,
        "id_type": id_type,
        "verified": true
    }))
}
```

### Register Plugins

Update plugin registry to include KYC handlers:

```rust
// In rust/src/dsl_v2/custom_ops/mod.rs
pub mod kyc_ops;

// In plugin registration
plugins.insert("list_outstanding_requirements", kyc_ops::list_outstanding_requirements);
plugins.insert("calculate_ownership_chain", kyc_ops::calculate_ownership_chain);
plugins.insert("list_ownership_chain", kyc_ops::list_ownership_chain);
plugins.insert("verify_entity_identity", kyc_ops::verify_entity_identity);
```

**Effort**: 1 day

---

## Phase 5: Onboarding Orchestration

### Concept: Onboarding Request Coordinates Multiple DSL Domains

```
OnboardingRequest {
    cbu_name: "Global Asset Management",
    products: [Custody, PrimeBrokerage],
    markets: [XNYS, XLON],
    
    // Triggers:
    // 1. Create CBU (entity domain)
    // 2. DSL.Custody execution (if Custody in products)
    // 3. DSL.KYC kickoff (document requirements)
    // 4. DSL.UBO mapping (if entity type requires)
}
```

### File: `rust/src/orchestration/onboarding.rs`

```rust
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct OnboardingOrchestrator {
    pool: PgPool,
    dsl_executor: DslExecutor,
}

#[derive(Debug, Clone)]
pub struct OnboardingRequest {
    pub client_name: String,
    pub entity_type: EntityType,
    pub jurisdiction: String,
    pub products: Vec<Product>,
    pub markets: Vec<String>,
    pub otc_counterparties: Vec<OtcCounterparty>,
}

#[derive(Debug, Clone)]
pub enum Product {
    Custody,
    PrimeBrokerage,
    FundAdmin,
    Clearing,
}

#[derive(Debug, Clone)]
pub struct OnboardingResult {
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub custody_setup: Option<CustodySetupResult>,
    pub kyc_requirements: Vec<DocumentRequirement>,
    pub ubo_status: UboStatus,
}

impl OnboardingOrchestrator {
    pub async fn onboard(&self, request: OnboardingRequest) -> Result<OnboardingResult> {
        // Step 1: Create or lookup entity
        let entity_id = self.ensure_entity(&request).await?;
        
        // Step 2: Create CBU
        let cbu_id = self.create_cbu(&request, entity_id).await?;
        
        // Step 3: Execute product-specific DSL (parallel where possible)
        let custody_result = if request.products.contains(&Product::Custody) {
            Some(self.setup_custody(&request, cbu_id).await?)
        } else {
            None
        };
        
        // Step 4: Determine KYC requirements based on entity type + jurisdiction + products
        let kyc_requirements = self.determine_kyc_requirements(&request, cbu_id, entity_id).await?;
        
        // Step 5: Initialize UBO mapping if required
        let ubo_status = self.initialize_ubo(&request, cbu_id, entity_id).await?;
        
        Ok(OnboardingResult {
            cbu_id,
            entity_id,
            custody_setup: custody_result,
            kyc_requirements,
            ubo_status,
        })
    }
    
    async fn determine_kyc_requirements(
        &self,
        request: &OnboardingRequest,
        cbu_id: Uuid,
        entity_id: Uuid,
    ) -> Result<Vec<DocumentRequirement>> {
        // Rule-based requirement determination
        let mut requirements = Vec::new();
        
        // Base requirements for all entities
        requirements.push(DocumentRequirement {
            document_type: "CERT_OF_INCORPORATION".to_string(),
            priority: "CRITICAL".to_string(),
        });
        
        requirements.push(DocumentRequirement {
            document_type: "BOARD_RESOLUTION".to_string(),
            priority: "CRITICAL".to_string(),
        });
        
        requirements.push(DocumentRequirement {
            document_type: "AUTHORIZED_SIGNATORIES".to_string(),
            priority: "HIGH".to_string(),
        });
        
        // Jurisdiction-specific
        match request.jurisdiction.as_str() {
            "US" => {
                requirements.push(DocumentRequirement {
                    document_type: "TAX_ID_CERTIFICATE".to_string(), // W-9
                    priority: "CRITICAL".to_string(),
                });
            }
            "UK" | "GB" => {
                requirements.push(DocumentRequirement {
                    document_type: "PROOF_OF_ADDRESS".to_string(),
                    priority: "HIGH".to_string(),
                });
            }
            _ => {}
        }
        
        // Entity type specific
        match request.entity_type {
            EntityType::Trust => {
                requirements.push(DocumentRequirement {
                    document_type: "TRUST_DEED".to_string(),
                    priority: "CRITICAL".to_string(),
                });
            }
            EntityType::Partnership => {
                requirements.push(DocumentRequirement {
                    document_type: "PARTNERSHIP_AGREEMENT".to_string(),
                    priority: "CRITICAL".to_string(),
                });
            }
            _ => {}
        }
        
        // Product-specific
        if request.products.contains(&Product::PrimeBrokerage) {
            requirements.push(DocumentRequirement {
                document_type: "FINANCIAL_STATEMENTS".to_string(),
                priority: "HIGH".to_string(),
            });
        }
        
        // Generate DSL to create requirements
        let dsl = self.generate_kyc_requirements_dsl(cbu_id, entity_id, &requirements);
        self.dsl_executor.execute(&dsl).await?;
        
        Ok(requirements)
    }
    
    fn generate_kyc_requirements_dsl(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        requirements: &[DocumentRequirement],
    ) -> String {
        let mut dsl = String::new();
        
        for req in requirements {
            dsl.push_str(&format!(
                r#"kyc.require-document cbu-id: "{}" entity-id: "{}" document-type: {} priority: {}
"#,
                cbu_id, entity_id, req.document_type, req.priority
            ));
        }
        
        dsl
    }
}
```

**Effort**: 1 day

---

## Phase 6: Agentic Integration for KYC/UBO

### File: `rust/src/agentic/prompts/kyc_intent_extraction_system.md`

```markdown
# KYC Intent Extraction

You are extracting structured KYC/compliance intent from natural language requests.

## Document Types
- CERT_OF_INCORPORATION - Certificate of incorporation/formation
- BOARD_RESOLUTION - Board resolution authorizing account
- AUTHORIZED_SIGNATORIES - List of authorized signers
- PROOF_OF_ADDRESS - Address verification document
- TAX_ID_CERTIFICATE - Tax ID (W-9, W-8BEN, etc.)
- FINANCIAL_STATEMENTS - Audited financials
- PASSPORT - Individual passport
- DRIVERS_LICENSE - Driver's license
- TRUST_DEED - Trust agreement
- PARTNERSHIP_AGREEMENT - Partnership agreement
- SHAREHOLDER_REGISTER - Register of shareholders
- SOURCE_OF_FUNDS - Source of funds documentation
- SOURCE_OF_WEALTH - Source of wealth documentation

## Output Format
{
  "cbu_id": "uuid if known",
  "entity_id": "uuid if known",
  "action": "require_documents | submit_document | verify_document | check_status",
  "documents": [
    {"type": "CERT_OF_INCORPORATION", "priority": "CRITICAL"},
    ...
  ],
  "notes": "any special instructions"
}
```

### File: `rust/src/agentic/prompts/ubo_intent_extraction_system.md`

```markdown
# UBO Intent Extraction

You are extracting ownership structure from natural language descriptions.

## Entity Types
- INDIVIDUAL - Natural person
- CORPORATION - Corporation/company
- LLC - Limited liability company
- PARTNERSHIP - General or limited partnership
- TRUST - Trust structure
- FOUNDATION - Foundation
- FUND - Investment fund

## Ownership Types
- DIRECT - Direct shareholding
- INDIRECT - Through intermediate entity
- BENEFICIAL - Beneficial ownership

## Control Types
- BOARD_CONTROL - Board seat / director
- VOTING_RIGHTS - Voting control
- VETO_POWER - Veto rights
- MANAGEMENT - Management control
- TRUSTEE - Trustee of trust

## Output Format
{
  "root_entity": {
    "name": "Target Company LLC",
    "type": "LLC",
    "jurisdiction": "US-DE"
  },
  "ownership_chain": [
    {
      "owner": {"name": "Holding Corp", "type": "CORPORATION", "jurisdiction": "US-DE"},
      "owned": "Target Company LLC",
      "percentage": 60,
      "type": "DIRECT"
    },
    {
      "owner": {"name": "John Smith", "type": "INDIVIDUAL", "jurisdiction": "US"},
      "owned": "Holding Corp",
      "percentage": 100,
      "type": "DIRECT"
    }
  ],
  "control_relationships": [
    {
      "controller": "John Smith",
      "controlled": "Target Company LLC",
      "type": "BOARD_CONTROL"
    }
  ],
  "identified_ubos": [
    {"name": "John Smith", "effective_percentage": 60, "basis": "60% through Holding Corp"}
  ]
}
```

### Add to CLI

```rust
// In dsl_cli
Subcommand::Kyc { input, plan_only, execute, ... } => {
    // Similar to custody command but for KYC domain
}

Subcommand::Ubo { input, plan_only, execute, ... } => {
    // Similar to custody command but for UBO domain
}
```

**Effort**: 1 day

---

## Phase 7: Update Graph Builder for KYC/UBO

### Update: `rust/src/graph/builder.rs`

Add KYC and UBO layer loading:

```rust
async fn load_kyc_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
    // Load document requirements
    let requirements = sqlx::query!(
        r#"SELECT dr.*, e.legal_name as entity_name
           FROM kyc.document_requirements dr
           JOIN "ob-poc".entities e ON e.entity_id = dr.entity_id
           WHERE dr.cbu_id = $1"#,
        self.cbu_id
    )
    .fetch_all(pool)
    .await?;
    
    for req in requirements {
        graph.add_node(GraphNode {
            id: req.requirement_id.to_string(),
            node_type: NodeType::Document,
            layer: LayerType::Kyc,
            label: req.document_type,
            sublabel: Some(format!("{} - {}", req.entity_name, req.status)),
            status: match req.status.as_str() {
                "VERIFIED" => NodeStatus::Active,
                "SUBMITTED" => NodeStatus::Pending,
                "REJECTED" => NodeStatus::Expired,
                _ => NodeStatus::Draft,
            },
            data: serde_json::to_value(&req)?,
        });
        
        // Edge: Entity → Document requirement
        graph.add_edge(GraphEdge {
            id: format!("entity->{}", req.requirement_id),
            source: req.entity_id.to_string(),
            target: req.requirement_id.to_string(),
            edge_type: EdgeType::Requires,
            label: None,
        });
    }
    
    Ok(())
}

async fn load_ubo_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
    // Get CBU's root entity
    let cbu = sqlx::query!(
        r#"SELECT entity_id FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        self.cbu_id
    )
    .fetch_one(pool)
    .await?;
    
    // Load ownership chain using recursive CTE
    let links = sqlx::query!(
        r#"
        WITH RECURSIVE chain AS (
            SELECT ol.*, 0 as depth
            FROM kyc.ownership_links ol
            WHERE ol.child_entity_id = $1 AND ol.is_active = true
            
            UNION ALL
            
            SELECT ol.*, c.depth + 1
            FROM kyc.ownership_links ol
            JOIN chain c ON ol.child_entity_id = c.parent_entity_id
            WHERE ol.is_active = true AND c.depth < 5
        )
        SELECT DISTINCT c.*, 
               pe.legal_name as parent_name, pe.entity_type as parent_type,
               ce.legal_name as child_name, ce.entity_type as child_type
        FROM chain c
        JOIN "ob-poc".entities pe ON pe.entity_id = c.parent_entity_id
        JOIN "ob-poc".entities ce ON ce.entity_id = c.child_entity_id
        "#,
        cbu.entity_id
    )
    .fetch_all(pool)
    .await?;
    
    let mut seen_entities = std::collections::HashSet::new();
    
    // Add root entity first
    if seen_entities.insert(cbu.entity_id) {
        let root = sqlx::query!(
            r#"SELECT * FROM "ob-poc".entities WHERE entity_id = $1"#,
            cbu.entity_id
        )
        .fetch_one(pool)
        .await?;
        
        graph.add_node(GraphNode {
            id: cbu.entity_id.to_string(),
            node_type: NodeType::Entity,
            layer: LayerType::Ubo,
            label: root.legal_name,
            sublabel: Some(format!("{} ({})", root.entity_type, root.jurisdiction)),
            status: NodeStatus::Active,
            data: serde_json::json!({"is_root": true}),
        });
    }
    
    for link in links {
        // Add parent entity node
        if seen_entities.insert(link.parent_entity_id) {
            graph.add_node(GraphNode {
                id: link.parent_entity_id.to_string(),
                node_type: NodeType::Entity,
                layer: LayerType::Ubo,
                label: link.parent_name.unwrap_or_default(),
                sublabel: link.parent_type,
                status: NodeStatus::Active,
                data: serde_json::json!({}),
            });
        }
        
        // Add ownership edge
        graph.add_edge(GraphEdge {
            id: link.link_id.to_string(),
            source: link.parent_entity_id.to_string(),
            target: link.child_entity_id.to_string(),
            edge_type: EdgeType::Owns,
            label: link.ownership_percentage.map(|p| format!("{}%", p)),
        });
    }
    
    // Load control links
    let controls = sqlx::query!(
        r#"
        SELECT cl.*, 
               ce.legal_name as controller_name,
               de.legal_name as controlled_name
        FROM kyc.control_links cl
        JOIN "ob-poc".entities ce ON ce.entity_id = cl.controller_entity_id
        JOIN "ob-poc".entities de ON de.entity_id = cl.controlled_entity_id
        WHERE cl.controlled_entity_id = $1 AND cl.is_active = true
        "#,
        cbu.entity_id
    )
    .fetch_all(pool)
    .await?;
    
    for ctrl in controls {
        graph.add_edge(GraphEdge {
            id: ctrl.link_id.to_string(),
            source: ctrl.controller_entity_id.to_string(),
            target: ctrl.controlled_entity_id.to_string(),
            edge_type: EdgeType::Controls,
            label: Some(ctrl.control_type),
        });
    }
    
    Ok(())
}
```

**Effort**: 0.5 day

---

## Summary

| Phase | Description | Effort |
|-------|-------------|--------|
| 0 | Audit existing tables/verbs | 0.5 day |
| 1 | Schema additions (if needed) | 0.5 day |
| 2 | KYC verb definitions | 0.5 day |
| 3 | UBO verb definitions | 0.5 day |
| 4 | Plugin handlers | 1 day |
| 5 | Onboarding orchestration | 1 day |
| 6 | Agentic KYC/UBO | 1 day |
| 7 | Graph builder updates | 0.5 day |
| **Total** | | **5.5 days** |

---

## Demo Flow After Implementation

```bash
# Step 1: Onboard with custody products
dsl_cli custody -i "Onboard Global Asset Management for US and UK equities" --execute

# Step 2: Kick off KYC (attaches to existing CBU)
dsl_cli kyc -i "Set up KYC requirements for Global Asset Management: need incorporation docs, board resolution, authorized signatories, and W-9" --execute

# Step 3: Map UBO structure
dsl_cli ubo -i "Map ownership for Global Asset Management: 60% owned by Apex Holdings (Delaware), which is 100% owned by John Smith (US citizen). 40% owned by Pacific Trust with Sarah Jones as trustee." --execute

# Step 4: Visualize - same CBU, toggle views
# Browser: http://localhost:8080/
# Select "Global Asset Management"
# Toggle [Custody] [KYC] [UBO] views
```

---

## Key Principle

**Same CBU ID** threads through everything:

```
custody.cbu_instrument_universe.cbu_id ──┐
custody.cbu_ssi.cbu_id ──────────────────┤
custody.ssi_booking_rules.cbu_id ────────┤
                                         ├──► "ob-poc".cbus.cbu_id
kyc.document_requirements.cbu_id ────────┤
kyc.entity_kyc_status.cbu_id ────────────┤
kyc.ubo_determinations.cbu_id ───────────┘
```

One client. Full picture. Multiple DSL domains. Common model.

---

*End of Integration Plan*
