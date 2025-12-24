# Async Requests & Fulfillment Pattern

## Overview

Long-running operations (document requests, information requests, external verifications) 
need to be tracked without agent polling. This document defines the pattern:

1. **Fire and Forget**: DSL verb runs, creates request state, returns immediately
2. **State Integration**: Requests visible AS NODES in domain state (not separate list)
3. **Fulfillment Matching**: When response arrives, system matches to pending request
4. **Background Sweep**: Cron handles reminders, escalations, expiry

**Core Principle**: The agent doesn't track requests. The system does. Agent observes state.

**Domain Coherence Principle**: Requests are NOT a separate thing. They are nodes in 
the domain state graph. A KYC case workstream that's blocked shows its outstanding 
requests as child nodes, not as a cross-reference to a "requests list".

---

## Visibility Model

### What User/Agent Sees

```
KYC Case: Alpha Fund
│
├── Alpha Fund (Account Holder)
│   └── Workstream: FULL_KYC ✓ COMPLETE
│
├── Pierre Martin (UBO)
│   └── Workstream: FULL_KYC ◐ BLOCKED
│       │
│       └── 📄 Awaiting: SOURCE_OF_WEALTH      ← CHILD NODE
│           ├── Due: Dec 21 (OVERDUE 10 days)
│           ├── Reminders: 2
│           └── Actions: [remind] [escalate] [waive]
│
└── Jean Dupont (Director)
    └── Workstream: SCREEN_AND_ID ◐ IN_PROGRESS
        └── 📄 Awaiting: ID_DOCUMENT           ← CHILD NODE
            └── Due: Dec 27 (3 days) - On track
```

Requests are embedded in domain state, not a parallel structure.

### NOT This (Anti-Pattern)

```
┌─────────────────────┐     ┌─────────────────────┐
│ KYC Case            │     │ Outstanding Requests │
│ (one view)          │     │ (separate view)      │
│                     │ ──► │                      │
│ WS-003: Blocked     │     │ REQ-456: ...         │
└─────────────────────┘     └─────────────────────┘

User cross-references. Two mental models. Bad UX.
```

---

## Data Model

### Outstanding Requests Table

**Migration:** `V0XX__outstanding_requests.sql`

**Note:** This table is cross-domain infrastructure. Visibility is domain-integrated 
(requests appear as child nodes in domain state queries, not as separate list).

```sql
-- ═══════════════════════════════════════════════════════════════════════════
-- Outstanding Requests: Fire-and-forget operations awaiting response
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE ob_kyc.outstanding_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- ───────────────────────────────────────────────────────────────────────
    -- What is this request attached to?
    -- ───────────────────────────────────────────────────────────────────────
    subject_type VARCHAR(50) NOT NULL,      -- WORKSTREAM, KYC_CASE, ENTITY, CBU
    subject_id UUID NOT NULL,
    
    -- Link to specific workstream/case for easier queries
    workstream_id UUID REFERENCES ob_kyc.workstreams(workstream_id),
    case_id UUID REFERENCES ob_kyc.kyc_cases(case_id),
    cbu_id UUID REFERENCES cbus(cbu_id),
    entity_id UUID REFERENCES entities(entity_id),
    
    -- ───────────────────────────────────────────────────────────────────────
    -- What was requested?
    -- ───────────────────────────────────────────────────────────────────────
    request_type VARCHAR(50) NOT NULL,      -- DOCUMENT, INFORMATION, VERIFICATION, APPROVAL, SIGNATURE
    request_subtype VARCHAR(100) NOT NULL,  -- SOURCE_OF_WEALTH, ID_DOCUMENT, REGISTRY_CHECK, etc.
    request_details JSONB DEFAULT '{}',     -- Flexible payload for request-specific data
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Who is it from/to?
    -- ───────────────────────────────────────────────────────────────────────
    requested_from_type VARCHAR(50),        -- CLIENT, ENTITY, EXTERNAL_PROVIDER, INTERNAL
    requested_from_entity_id UUID,          -- If requesting from specific entity
    requested_from_label VARCHAR(255),      -- Human-readable: "John Smith (UBO)", "FCA Registry"
    requested_by_user_id UUID,              -- User who triggered request
    requested_by_agent BOOLEAN DEFAULT FALSE,
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Timing
    -- ───────────────────────────────────────────────────────────────────────
    requested_at TIMESTAMP DEFAULT NOW(),
    due_date DATE,
    grace_period_days INTEGER DEFAULT 3,    -- Days after due_date before escalation
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Communication tracking
    -- ───────────────────────────────────────────────────────────────────────
    last_reminder_at TIMESTAMP,
    reminder_count INTEGER DEFAULT 0,
    max_reminders INTEGER DEFAULT 3,
    communication_log JSONB DEFAULT '[]',   -- Array of {timestamp, type, channel, reference}
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Status
    -- ───────────────────────────────────────────────────────────────────────
    status VARCHAR(50) DEFAULT 'PENDING',   -- PENDING, FULFILLED, PARTIAL, CANCELLED, ESCALATED, EXPIRED, WAIVED
    status_reason TEXT,
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Fulfillment
    -- ───────────────────────────────────────────────────────────────────────
    fulfilled_at TIMESTAMP,
    fulfilled_by_user_id UUID,
    fulfillment_type VARCHAR(50),           -- DOCUMENT_UPLOAD, MANUAL_ENTRY, API_RESPONSE, WAIVER
    fulfillment_reference_type VARCHAR(50), -- DOCUMENT, VERIFICATION_RESULT, etc.
    fulfillment_reference_id UUID,          -- e.g., document_id that fulfilled this
    fulfillment_notes TEXT,
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Escalation
    -- ───────────────────────────────────────────────────────────────────────
    escalated_at TIMESTAMP,
    escalation_level INTEGER DEFAULT 0,     -- 0=none, 1=first escalation, 2=second, etc.
    escalation_reason VARCHAR(255),
    escalated_to_user_id UUID,
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Blocking behavior
    -- ───────────────────────────────────────────────────────────────────────
    blocks_subject BOOLEAN DEFAULT TRUE,    -- Does this request block the subject?
    blocker_message VARCHAR(500),           -- "Awaiting source of wealth documentation"
    
    -- ───────────────────────────────────────────────────────────────────────
    -- DSL tracking
    -- ───────────────────────────────────────────────────────────────────────
    created_by_verb VARCHAR(100),           -- e.g., "document.request"
    created_by_execution_id UUID,           -- Link to DSL execution log if needed
    
    -- ───────────────────────────────────────────────────────────────────────
    -- Audit
    -- ───────────────────────────────────────────────────────────────────────
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_oreq_subject ON ob_kyc.outstanding_requests(subject_type, subject_id);
CREATE INDEX idx_oreq_workstream ON ob_kyc.outstanding_requests(workstream_id) WHERE workstream_id IS NOT NULL;
CREATE INDEX idx_oreq_case ON ob_kyc.outstanding_requests(case_id) WHERE case_id IS NOT NULL;
CREATE INDEX idx_oreq_cbu ON ob_kyc.outstanding_requests(cbu_id) WHERE cbu_id IS NOT NULL;
CREATE INDEX idx_oreq_status ON ob_kyc.outstanding_requests(status);
CREATE INDEX idx_oreq_status_pending ON ob_kyc.outstanding_requests(due_date) WHERE status = 'PENDING';
CREATE INDEX idx_oreq_type ON ob_kyc.outstanding_requests(request_type, request_subtype);
CREATE INDEX idx_oreq_overdue ON ob_kyc.outstanding_requests(due_date, status) 
    WHERE status = 'PENDING' AND due_date < CURRENT_DATE;

-- Trigger for updated_at
CREATE TRIGGER trg_outstanding_requests_updated
    BEFORE UPDATE ON ob_kyc.outstanding_requests
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();
```

---

### Request Types Reference

**Migration:** `V0XX__request_types.sql`

```sql
CREATE TABLE ob_ref.request_types (
    request_type VARCHAR(50) NOT NULL,
    request_subtype VARCHAR(100) NOT NULL,
    
    -- Configuration
    description VARCHAR(255),
    default_due_days INTEGER DEFAULT 7,
    default_grace_days INTEGER DEFAULT 3,
    max_reminders INTEGER DEFAULT 3,
    blocks_by_default BOOLEAN DEFAULT TRUE,
    
    -- Who can fulfill?
    fulfillment_sources VARCHAR(50)[] DEFAULT ARRAY['CLIENT', 'USER'],
    auto_fulfill_on_upload BOOLEAN DEFAULT TRUE,  -- Auto-match document uploads
    
    -- Escalation config
    escalation_enabled BOOLEAN DEFAULT TRUE,
    escalation_after_days INTEGER DEFAULT 10,
    
    PRIMARY KEY (request_type, request_subtype)
);

-- Seed common request types
INSERT INTO ob_ref.request_types (request_type, request_subtype, description, default_due_days, blocks_by_default) VALUES
-- Documents
('DOCUMENT', 'ID_DOCUMENT', 'Identity document (passport, national ID)', 7, TRUE),
('DOCUMENT', 'PROOF_OF_ADDRESS', 'Proof of address document', 7, TRUE),
('DOCUMENT', 'SOURCE_OF_WEALTH', 'Source of wealth documentation', 14, TRUE),
('DOCUMENT', 'SOURCE_OF_FUNDS', 'Source of funds documentation', 14, TRUE),
('DOCUMENT', 'CERTIFICATE_OF_INCORPORATION', 'Company incorporation certificate', 7, TRUE),
('DOCUMENT', 'ARTICLES_OF_ASSOCIATION', 'Articles/memorandum of association', 7, TRUE),
('DOCUMENT', 'REGISTER_OF_MEMBERS', 'Shareholder register', 7, TRUE),
('DOCUMENT', 'REGISTER_OF_DIRECTORS', 'Directors register', 7, TRUE),
('DOCUMENT', 'FINANCIAL_STATEMENTS', 'Audited financial statements', 14, TRUE),
('DOCUMENT', 'OWNERSHIP_STRUCTURE', 'Ownership structure chart', 7, TRUE),
('DOCUMENT', 'BOARD_RESOLUTION', 'Board resolution', 7, TRUE),
('DOCUMENT', 'POWER_OF_ATTORNEY', 'Power of attorney document', 7, TRUE),
('DOCUMENT', 'TAX_FORMS', 'Tax forms (W-8, W-9, CRS)', 7, TRUE),
('DOCUMENT', 'REGULATORY_LICENSE', 'Regulatory license/authorization', 7, TRUE),
('DOCUMENT', 'OTHER', 'Other document', 7, TRUE),

-- Information requests
('INFORMATION', 'UBO_DETAILS', 'Ultimate beneficial owner details', 7, TRUE),
('INFORMATION', 'DIRECTOR_DETAILS', 'Director/officer details', 7, TRUE),
('INFORMATION', 'BUSINESS_DESCRIPTION', 'Business description and activities', 7, TRUE),
('INFORMATION', 'CONTACT_DETAILS', 'Contact information', 5, FALSE),
('INFORMATION', 'INVESTMENT_MANDATE', 'Investment mandate/strategy', 7, FALSE),
('INFORMATION', 'TAX_RESIDENCY', 'Tax residency information', 7, TRUE),

-- Verifications (external)
('VERIFICATION', 'REGISTRY_CHECK', 'Company registry verification', 3, TRUE),
('VERIFICATION', 'REGULATORY_CHECK', 'Regulatory register verification', 3, TRUE),
('VERIFICATION', 'SANCTIONS_SCREENING', 'Sanctions screening', 1, TRUE),
('VERIFICATION', 'PEP_SCREENING', 'PEP screening', 1, TRUE),
('VERIFICATION', 'ADVERSE_MEDIA', 'Adverse media screening', 2, TRUE),
('VERIFICATION', 'ID_VERIFICATION', 'Electronic ID verification', 2, TRUE),

-- Approvals (internal)
('APPROVAL', 'KYC_REVIEW', 'KYC analyst review', 3, TRUE),
('APPROVAL', 'SENIOR_REVIEW', 'Senior/manager review', 2, TRUE),
('APPROVAL', 'WAIVER_APPROVAL', 'Document waiver approval', 1, TRUE),
('APPROVAL', 'RISK_ACCEPTANCE', 'Risk acceptance approval', 2, TRUE),

-- Signatures
('SIGNATURE', 'ACCOUNT_OPENING', 'Account opening documents', 14, TRUE),
('SIGNATURE', 'TAX_CERTIFICATION', 'Tax certification signatures', 14, TRUE),
('SIGNATURE', 'AGREEMENT', 'Agreement/contract signatures', 14, TRUE);
```

---

### Workstream Blocker Extension

**Migration:** `V0XX__extend_workstream_blockers.sql`

```sql
-- Add blocker tracking to workstreams
ALTER TABLE ob_kyc.workstreams ADD COLUMN IF NOT EXISTS blocker_type VARCHAR(50);
ALTER TABLE ob_kyc.workstreams ADD COLUMN IF NOT EXISTS blocker_request_id UUID REFERENCES ob_kyc.outstanding_requests(request_id);
ALTER TABLE ob_kyc.workstreams ADD COLUMN IF NOT EXISTS blocker_message VARCHAR(500);
ALTER TABLE ob_kyc.workstreams ADD COLUMN IF NOT EXISTS blocked_at TIMESTAMP;
ALTER TABLE ob_kyc.workstreams ADD COLUMN IF NOT EXISTS blocked_days_total INTEGER DEFAULT 0;

-- Function to calculate blocked days
CREATE OR REPLACE FUNCTION ob_kyc.update_workstream_blocked_days()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'BLOCKED' AND NEW.status != 'BLOCKED' THEN
        NEW.blocked_days_total = COALESCE(OLD.blocked_days_total, 0) + 
            EXTRACT(DAY FROM NOW() - COALESCE(OLD.blocked_at, NOW()));
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_workstream_blocked_days
    BEFORE UPDATE ON ob_kyc.workstreams
    FOR EACH ROW EXECUTE FUNCTION ob_kyc.update_workstream_blocked_days();
```

---

## DSL Verbs

### Request Verbs (Fire and Forget)

**File:** `config/verbs/request.yaml`

```yaml
domain: request

# ═══════════════════════════════════════════════════════════════════════════
# Core request operations
# ═══════════════════════════════════════════════════════════════════════════

create:
  description: "Create an outstanding request (generic)"
  behavior: plugin
  plugin:
    handler: RequestCreateOp
  args:
    - name: subject-type
      type: string
      required: true
      enum: [WORKSTREAM, KYC_CASE, ENTITY, CBU]
    - name: subject-id
      type: uuid
      required: true
    - name: type
      type: string
      required: true
      column: request_type
      enum: [DOCUMENT, INFORMATION, VERIFICATION, APPROVAL, SIGNATURE]
    - name: subtype
      type: string
      required: true
      column: request_subtype
    - name: from
      type: string
      required: false
      column: requested_from_label
      description: "Who should provide this"
    - name: from-entity
      type: uuid
      required: false
      column: requested_from_entity_id
    - name: due-in-days
      type: integer
      required: false
      default: 7
    - name: due-date
      type: date
      required: false
    - name: blocks
      type: boolean
      required: false
      default: true
      column: blocks_subject
    - name: message
      type: string
      required: false
      column: blocker_message
    - name: details
      type: object
      required: false
      column: request_details

list:
  description: "List outstanding requests"
  behavior: crud
  crud:
    operation: select
    table: outstanding_requests
    schema: ob_kyc
    multiple: true
  args:
    - name: case-id
      type: uuid
      required: false
    - name: workstream-id
      type: uuid
      required: false
    - name: cbu-id
      type: uuid
      required: false
    - name: status
      type: string
      required: false
      default: PENDING

overdue:
  description: "List overdue requests"
  behavior: plugin
  plugin:
    handler: RequestOverdueOp
  args:
    - name: case-id
      type: uuid
      required: false
    - name: cbu-id
      type: uuid
      required: false
    - name: include-grace-period
      type: boolean
      required: false
      default: false

# ═══════════════════════════════════════════════════════════════════════════
# Request lifecycle
# ═══════════════════════════════════════════════════════════════════════════

fulfill:
  description: "Mark request as fulfilled"
  behavior: plugin
  plugin:
    handler: RequestFulfillOp
  args:
    - name: request-id
      type: uuid
      required: true
    - name: fulfillment-type
      type: string
      required: false
      enum: [DOCUMENT_UPLOAD, MANUAL_ENTRY, API_RESPONSE, WAIVER]
    - name: reference-id
      type: uuid
      required: false
      column: fulfillment_reference_id
    - name: reference-type
      type: string
      required: false
      column: fulfillment_reference_type
    - name: notes
      type: string
      required: false

cancel:
  description: "Cancel a pending request"
  behavior: plugin
  plugin:
    handler: RequestCancelOp
  args:
    - name: request-id
      type: uuid
      required: true
    - name: reason
      type: string
      required: true

extend:
  description: "Extend request due date"
  behavior: plugin
  plugin:
    handler: RequestExtendOp
  args:
    - name: request-id
      type: uuid
      required: true
    - name: days
      type: integer
      required: false
    - name: new-due-date
      type: date
      required: false
    - name: reason
      type: string
      required: true

remind:
  description: "Send reminder for pending request"
  behavior: plugin
  plugin:
    handler: RequestRemindOp
  args:
    - name: request-id
      type: uuid
      required: true
    - name: channel
      type: string
      required: false
      enum: [EMAIL, PORTAL, SMS]
      default: EMAIL
    - name: message
      type: string
      required: false

escalate:
  description: "Escalate overdue request"
  behavior: plugin
  plugin:
    handler: RequestEscalateOp
  args:
    - name: request-id
      type: uuid
      required: true
    - name: escalate-to
      type: uuid
      required: false
      column: escalated_to_user_id
    - name: reason
      type: string
      required: false

waive:
  description: "Waive a request requirement"
  behavior: plugin
  plugin:
    handler: RequestWaiveOp
  args:
    - name: request-id
      type: uuid
      required: true
    - name: reason
      type: string
      required: true
    - name: approved-by
      type: uuid
      required: true
      description: "Senior user approving waiver"
```

---

### Document Request Verbs (Convenience Wrappers)

**File:** `config/verbs/document.yaml`

```yaml
domain: document

request:
  description: "Request a document (creates outstanding request)"
  behavior: plugin
  plugin:
    handler: DocumentRequestOp
  args:
    - name: workstream-id
      type: uuid
      required: false
    - name: entity-id
      type: uuid
      required: false
    - name: case-id
      type: uuid
      required: false
    - name: type
      type: string
      required: true
      description: "Document type code"
      enum: [ID_DOCUMENT, PROOF_OF_ADDRESS, SOURCE_OF_WEALTH, SOURCE_OF_FUNDS, 
             CERTIFICATE_OF_INCORPORATION, ARTICLES_OF_ASSOCIATION, REGISTER_OF_MEMBERS,
             REGISTER_OF_DIRECTORS, FINANCIAL_STATEMENTS, OWNERSHIP_STRUCTURE,
             BOARD_RESOLUTION, POWER_OF_ATTORNEY, TAX_FORMS, REGULATORY_LICENSE, OTHER]
    - name: from
      type: string
      required: false
      default: "client"
    - name: due-in-days
      type: integer
      required: false
    - name: notes
      type: string
      required: false
  example: |
    (document.request workstream-id:@ws-003 type:SOURCE_OF_WEALTH from:"client" due-in-days:14)

upload:
  description: "Upload a document (auto-fulfills matching request)"
  behavior: plugin
  plugin:
    handler: DocumentUploadOp
  args:
    - name: workstream-id
      type: uuid
      required: false
    - name: entity-id
      type: uuid
      required: false
    - name: case-id
      type: uuid
      required: false
    - name: type
      type: string
      required: true
    - name: file
      type: file
      required: true
    - name: notes
      type: string
      required: false
  returns:
    type: object
    description: |
      {
        "document_id": "uuid",
        "fulfilled_request_id": "uuid or null",
        "workstream_unblocked": true/false
      }

waive:
  description: "Waive document requirement"
  behavior: plugin
  plugin:
    handler: DocumentWaiveOp
  args:
    - name: workstream-id
      type: uuid
      required: true
    - name: type
      type: string
      required: true
    - name: reason
      type: string
      required: true
    - name: approved-by
      type: uuid
      required: true
  example: |
    (document.waive workstream-id:@ws-003 type:SOURCE_OF_WEALTH 
       reason:"Low value account, low risk" approved-by:@senior-analyst)
```

---

### Information Request Verbs

**File:** `config/verbs/information.yaml`

```yaml
domain: information

request:
  description: "Request information from entity/client"
  behavior: plugin
  plugin:
    handler: InformationRequestOp
  args:
    - name: workstream-id
      type: uuid
      required: false
    - name: entity-id
      type: uuid
      required: false
    - name: type
      type: string
      required: true
      enum: [UBO_DETAILS, DIRECTOR_DETAILS, BUSINESS_DESCRIPTION, 
             CONTACT_DETAILS, INVESTMENT_MANDATE, TAX_RESIDENCY]
    - name: from
      type: string
      required: false
    - name: questions
      type: string[]
      required: false
      description: "Specific questions to answer"
    - name: due-in-days
      type: integer
      required: false

provide:
  description: "Provide requested information (fulfills request)"
  behavior: plugin
  plugin:
    handler: InformationProvideOp
  args:
    - name: request-id
      type: uuid
      required: false
      description: "Specific request to fulfill"
    - name: workstream-id
      type: uuid
      required: false
    - name: type
      type: string
      required: true
    - name: data
      type: object
      required: true
      description: "The information being provided"
    - name: notes
      type: string
      required: false
```

---

### Verification Request Verbs

**File:** `config/verbs/verification.yaml`

```yaml
domain: verification

request:
  description: "Request external verification"
  behavior: plugin
  plugin:
    handler: VerificationRequestOp
  args:
    - name: entity-id
      type: uuid
      required: true
    - name: type
      type: string
      required: true
      enum: [REGISTRY_CHECK, REGULATORY_CHECK, SANCTIONS_SCREENING, 
             PEP_SCREENING, ADVERSE_MEDIA, ID_VERIFICATION]
    - name: provider
      type: string
      required: false
      description: "Specific provider to use"
    - name: workstream-id
      type: uuid
      required: false
    - name: auto-process
      type: boolean
      required: false
      default: true
      description: "Automatically process result when received"

# Note: Fulfillment happens via:
# - Webhook from provider → system calls (request.fulfill)
# - Manual entry → user calls (verification.record)

record:
  description: "Record verification result"
  behavior: plugin
  plugin:
    handler: VerificationRecordOp
  args:
    - name: request-id
      type: uuid
      required: false
    - name: entity-id
      type: uuid
      required: true
    - name: type
      type: string
      required: true
    - name: result
      type: string
      required: true
      enum: [CLEAR, HIT, INCONCLUSIVE, FAILED]
    - name: details
      type: object
      required: false
    - name: provider
      type: string
      required: false
    - name: reference
      type: string
      required: false
```

---

## Plugin Implementations

### DocumentRequestOp (Fire and Forget)

**File:** `rust/src/dsl_v2/custom_ops/document.rs`

```rust
pub struct DocumentRequestOp;

impl DocumentRequestOp {
    pub async fn execute(&self, args: &Args, ctx: &ExecutionContext, pool: &PgPool) -> Result<Value> {
        // Resolve subject
        let (subject_type, subject_id, workstream_id, case_id, cbu_id, entity_id) = 
            resolve_request_subject(args, pool).await?;
        
        let doc_type = args.get_string("type")?;
        let requested_from = args.get_string("from").unwrap_or("client".to_string());
        
        // Get defaults from request_types config
        let config = sqlx::query!(
            r#"
            SELECT default_due_days, default_grace_days, blocks_by_default
            FROM ob_ref.request_types
            WHERE request_type = 'DOCUMENT' AND request_subtype = $1
            "#,
            doc_type
        ).fetch_optional(pool).await?.unwrap_or_default();
        
        let due_days = args.get_i64("due-in-days").unwrap_or(config.default_due_days as i64);
        let due_date = Utc::now().date_naive() + Duration::days(due_days);
        let blocks = args.get_bool("blocks").unwrap_or(config.blocks_by_default);
        
        let blocker_message = format!("Awaiting {} from {}", 
            humanize_doc_type(&doc_type), requested_from);
        
        // Create outstanding request
        let request_id = sqlx::query_scalar!(
            r#"
            INSERT INTO ob_kyc.outstanding_requests (
                subject_type, subject_id, workstream_id, case_id, cbu_id, entity_id,
                request_type, request_subtype, 
                requested_from_type, requested_from_label,
                requested_by_agent, due_date, grace_period_days,
                blocks_subject, blocker_message,
                request_details, created_by_verb
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                'DOCUMENT', $7,
                'CLIENT', $8,
                $9, $10, $11,
                $12, $13,
                $14, 'document.request'
            )
            RETURNING request_id
            "#,
            subject_type, subject_id, workstream_id, case_id, cbu_id, entity_id,
            doc_type,
            requested_from,
            ctx.is_agent, due_date, config.default_grace_days,
            blocks, blocker_message,
            json!({"notes": args.get_string("notes")})
        ).fetch_one(pool).await?;
        
        // If blocking and attached to workstream, update workstream status
        if blocks && workstream_id.is_some() {
            sqlx::query!(
                r#"
                UPDATE ob_kyc.workstreams
                SET status = 'BLOCKED',
                    blocker_type = 'AWAITING_DOCUMENT',
                    blocker_request_id = $2,
                    blocker_message = $3,
                    blocked_at = NOW()
                WHERE workstream_id = $1
                "#,
                workstream_id.unwrap(), request_id, blocker_message
            ).execute(pool).await?;
        }
        
        Ok(json!({
            "request_id": request_id,
            "request_type": "DOCUMENT",
            "request_subtype": doc_type,
            "status": "PENDING",
            "due_date": due_date,
            "blocks_subject": blocks,
            "subject": {
                "type": subject_type,
                "id": subject_id
            }
        }))
    }
}
```

---

### DocumentUploadOp (Auto-Fulfillment)

**File:** `rust/src/dsl_v2/custom_ops/document.rs`

```rust
pub struct DocumentUploadOp;

impl DocumentUploadOp {
    pub async fn execute(&self, args: &Args, ctx: &ExecutionContext, pool: &PgPool) -> Result<Value> {
        let doc_type = args.get_string("type")?;
        let file = args.get_file("file")?;
        
        // Resolve subject
        let (subject_type, subject_id, workstream_id, case_id, cbu_id, entity_id) = 
            resolve_request_subject(args, pool).await?;
        
        // Store document
        let document_id = store_document(file, doc_type.clone(), entity_id, pool).await?;
        
        // Try to find and fulfill matching pending request
        let fulfilled_request = sqlx::query!(
            r#"
            UPDATE ob_kyc.outstanding_requests
            SET status = 'FULFILLED',
                fulfilled_at = NOW(),
                fulfilled_by_user_id = $4,
                fulfillment_type = 'DOCUMENT_UPLOAD',
                fulfillment_reference_type = 'DOCUMENT',
                fulfillment_reference_id = $3
            WHERE request_id = (
                SELECT request_id
                FROM ob_kyc.outstanding_requests
                WHERE request_type = 'DOCUMENT'
                  AND request_subtype = $2
                  AND status = 'PENDING'
                  AND (
                    (workstream_id = $1 AND $1 IS NOT NULL)
                    OR (entity_id = $5 AND $5 IS NOT NULL AND workstream_id IS NULL)
                    OR (case_id = $6 AND $6 IS NOT NULL AND workstream_id IS NULL AND entity_id IS NULL)
                  )
                ORDER BY requested_at ASC
                LIMIT 1
                FOR UPDATE
            )
            RETURNING request_id, workstream_id, blocker_request_id
            "#,
            workstream_id, doc_type, document_id, ctx.user_id, entity_id, case_id
        ).fetch_optional(pool).await?;
        
        let mut workstream_unblocked = false;
        
        // If we fulfilled a request that was blocking a workstream, try to unblock
        if let Some(req) = &fulfilled_request {
            if let Some(ws_id) = req.workstream_id {
                workstream_unblocked = try_unblock_workstream(ws_id, pool).await?;
            }
        }
        
        Ok(json!({
            "document_id": document_id,
            "document_type": doc_type,
            "fulfilled_request_id": fulfilled_request.as_ref().map(|r| r.request_id),
            "workstream_unblocked": workstream_unblocked,
            "subject": {
                "type": subject_type,
                "id": subject_id
            }
        }))
    }
}

async fn try_unblock_workstream(workstream_id: Uuid, pool: &PgPool) -> Result<bool> {
    // Check if there are any remaining blocking requests
    let remaining_blockers = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) 
        FROM ob_kyc.outstanding_requests
        WHERE workstream_id = $1
          AND status = 'PENDING'
          AND blocks_subject = TRUE
        "#,
        workstream_id
    ).fetch_one(pool).await?;
    
    if remaining_blockers.unwrap_or(0) == 0 {
        // No more blockers, unblock workstream
        sqlx::query!(
            r#"
            UPDATE ob_kyc.workstreams
            SET status = CASE 
                    WHEN status = 'BLOCKED' THEN 'IN_PROGRESS'
                    ELSE status
                END,
                blocker_type = NULL,
                blocker_request_id = NULL,
                blocker_message = NULL
            WHERE workstream_id = $1
            "#,
            workstream_id
        ).execute(pool).await?;
        
        return Ok(true);
    }
    
    Ok(false)
}
```

---

## Status Integration (Domain-Embedded)

### Principle: Requests as Child Nodes

Requests are NOT returned as a separate `outstanding_requests` array.
They are EMBEDDED in the domain objects they belong to (workstreams, entities).

### Workstream State Query

**Verb:** `(workstream.state workstream-id:@ws-003)`

**Returns requests as child nodes:**

```json
{
  "workstream_id": "ws-003",
  "entity": {
    "entity_id": "...",
    "name": "Pierre Martin",
    "role": "UBO"
  },
  "type": "FULL_KYC",
  "status": "BLOCKED",
  
  "awaiting": [
    {
      "request_id": "req-456",
      "type": "DOCUMENT",
      "subtype": "SOURCE_OF_WEALTH",
      "from": "client",
      "requested_at": "2025-12-14",
      "due_date": "2025-12-21",
      "days_overdue": 10,
      "overdue": true,
      "reminder_count": 2,
      "actions": ["remind", "escalate", "extend", "waive"]
    }
  ],
  
  "checks_complete": [
    {"type": "SANCTIONS", "result": "CLEAR", "date": "2025-12-15"},
    {"type": "PEP", "result": "CLEAR", "date": "2025-12-15"}
  ],
  
  "documents_received": [
    {"type": "ID_DOCUMENT", "document_id": "...", "date": "2025-12-16"},
    {"type": "PROOF_OF_ADDRESS", "document_id": "...", "date": "2025-12-16"}
  ]
}
```

### Case State Query (Workstreams with Embedded Requests)

**Verb:** `(kyc-case.state case-id:@case-123)`

**Returns:**

```json
{
  "case_id": "...",
  "cbu": {"name": "Alpha Fund", "type": "FUND"},
  "status": "IN_PROGRESS",
  "risk_rating": "HIGH",
  
  "workstreams": [
    {
      "workstream_id": "ws-001",
      "entity": {"name": "Alpha Fund", "role": "ACCOUNT_HOLDER"},
      "type": "FULL_KYC",
      "status": "COMPLETE",
      "awaiting": []
    },
    {
      "workstream_id": "ws-002",
      "entity": {"name": "Lux ManCo", "role": "MANCO"},
      "type": "SIMPLIFIED",
      "status": "COMPLETE",
      "awaiting": []
    },
    {
      "workstream_id": "ws-003",
      "entity": {"name": "Pierre Martin", "role": "UBO"},
      "type": "FULL_KYC",
      "status": "BLOCKED",
      "awaiting": [
        {
          "request_id": "req-456",
          "subtype": "SOURCE_OF_WEALTH",
          "days_overdue": 10,
          "overdue": true,
          "actions": ["remind", "escalate", "waive"]
        }
      ]
    },
    {
      "workstream_id": "ws-004",
      "entity": {"name": "Jean Dupont", "role": "DIRECTOR"},
      "type": "SCREEN_AND_ID",
      "status": "IN_PROGRESS",
      "awaiting": [
        {
          "request_id": "req-457",
          "subtype": "ID_DOCUMENT",
          "due_date": "2025-12-27",
          "days_overdue": 0,
          "overdue": false
        }
      ]
    }
  ],
  
  "summary": {
    "total_workstreams": 4,
    "complete": 2,
    "in_progress": 1,
    "blocked": 1,
    "total_awaiting": 2,
    "overdue": 1
  },
  
  "attention": [
    {
      "workstream": "ws-003",
      "entity": "Pierre Martin",
      "issue": "SOURCE_OF_WEALTH overdue 10 days",
      "priority": "HIGH",
      "actions": ["remind", "escalate", "waive"]
    }
  ]
}
```

### Implementation

**Update `KycCaseStateOp`:**

```rust
pub async fn execute(&self, args: &Args, pool: &PgPool) -> Result<Value> {
    let case_id = args.get_uuid("case-id")?;
    
    // Load case
    let case_info = load_case(case_id, pool).await?;
    
    // Load workstreams WITH their requests embedded
    let workstreams = sqlx::query!(
        r#"
        SELECT 
            w.workstream_id,
            w.entity_id,
            w.workstream_type,
            w.status,
            e.entity_name,
            r.role_type as entity_role
        FROM ob_kyc.workstreams w
        JOIN entities e ON w.entity_id = e.entity_id
        LEFT JOIN cbu_entity_roles r ON w.entity_id = r.entity_id AND w.case_id = r.cbu_id
        WHERE w.case_id = $1
        ORDER BY w.created_at
        "#,
        case_id
    ).fetch_all(pool).await?;
    
    // For each workstream, load its awaiting requests
    let mut workstream_states = Vec::new();
    
    for ws in workstreams {
        let awaiting = sqlx::query!(
            r#"
            SELECT 
                request_id, request_type, request_subtype,
                requested_from_label, requested_at, due_date,
                reminder_count, escalation_level, status,
                CURRENT_DATE - due_date as days_overdue
            FROM ob_kyc.outstanding_requests
            WHERE workstream_id = $1 AND status IN ('PENDING', 'ESCALATED')
            ORDER BY due_date ASC
            "#,
            ws.workstream_id
        ).fetch_all(pool).await?;
        
        let awaiting_nodes: Vec<Value> = awaiting.iter().map(|r| {
            let overdue = r.days_overdue.unwrap_or(0) > 0;
            let mut actions = vec!["remind", "extend"];
            if overdue {
                actions.push("escalate");
            }
            actions.push("waive");
            
            json!({
                "request_id": r.request_id,
                "type": r.request_type,
                "subtype": r.request_subtype,
                "from": r.requested_from_label,
                "requested_at": r.requested_at,
                "due_date": r.due_date,
                "days_overdue": r.days_overdue.unwrap_or(0).max(0),
                "overdue": overdue,
                "reminder_count": r.reminder_count,
                "escalated": r.escalation_level > 0,
                "actions": actions
            })
        }).collect();
        
        workstream_states.push(json!({
            "workstream_id": ws.workstream_id,
            "entity": {
                "entity_id": ws.entity_id,
                "name": ws.entity_name,
                "role": ws.entity_role
            },
            "type": ws.workstream_type,
            "status": ws.status,
            "awaiting": awaiting_nodes  // ← EMBEDDED, not separate
        }));
    }
    
    // Build summary and attention items
    let blocked_count = workstream_states.iter()
        .filter(|w| w["status"] == "BLOCKED").count();
    let overdue_count = workstream_states.iter()
        .flat_map(|w| w["awaiting"].as_array())
        .flatten()
        .filter(|r| r["overdue"].as_bool().unwrap_or(false))
        .count();
    
    // Build attention items from overdue requests
    let attention: Vec<Value> = workstream_states.iter()
        .flat_map(|ws| {
            ws["awaiting"].as_array().unwrap_or(&vec![]).iter()
                .filter(|r| r["overdue"].as_bool().unwrap_or(false))
                .map(|r| json!({
                    "workstream": ws["workstream_id"],
                    "entity": ws["entity"]["name"],
                    "issue": format!("{} overdue {} days", 
                        r["subtype"].as_str().unwrap_or(""),
                        r["days_overdue"].as_i64().unwrap_or(0)),
                    "priority": if r["days_overdue"].as_i64().unwrap_or(0) > 7 { "HIGH" } else { "MEDIUM" },
                    "actions": r["actions"]
                }))
                .collect::<Vec<_>>()
        })
        .collect();
    
    Ok(json!({
        "case_id": case_id,
        "cbu": case_info.cbu,
        "status": case_info.status,
        "risk_rating": case_info.risk_rating,
        "workstreams": workstream_states,
        "summary": {
            "total_workstreams": workstream_states.len(),
            "complete": workstream_states.iter().filter(|w| w["status"] == "COMPLETE").count(),
            "in_progress": workstream_states.iter().filter(|w| w["status"] == "IN_PROGRESS").count(),
            "blocked": blocked_count,
            "total_awaiting": workstream_states.iter()
                .flat_map(|w| w["awaiting"].as_array())
                .map(|a| a.len())
                .sum::<usize>(),
            "overdue": overdue_count
        },
        "attention": attention
    }))
}
```

---

## Background Sweep (Cron Job)

**File:** `rust/src/jobs/request_sweep.rs`

```rust
/// Run daily to handle overdue requests
pub async fn run_request_sweep(pool: &PgPool) -> Result<SweepResult> {
    let mut result = SweepResult::default();
    
    // 1. Send reminders for approaching due dates (due in 2 days, not reminded recently)
    result.reminders_sent = send_due_reminders(pool).await?;
    
    // 2. Escalate overdue requests past grace period
    result.escalated = escalate_overdue_requests(pool).await?;
    
    // 3. Expire very old requests (configurable, e.g., 90 days)
    result.expired = expire_stale_requests(pool).await?;
    
    // 4. Generate daily summary for case managers
    generate_outstanding_requests_report(pool).await?;
    
    Ok(result)
}

async fn send_due_reminders(pool: &PgPool) -> Result<i32> {
    let due_soon = sqlx::query!(
        r#"
        SELECT request_id, workstream_id, case_id, requested_from_label, 
               request_subtype, due_date, reminder_count, max_reminders
        FROM ob_kyc.outstanding_requests
        WHERE status = 'PENDING'
          AND due_date BETWEEN CURRENT_DATE AND CURRENT_DATE + INTERVAL '2 days'
          AND (last_reminder_at IS NULL OR last_reminder_at < CURRENT_DATE - INTERVAL '1 day')
          AND reminder_count < max_reminders
        "#
    ).fetch_all(pool).await?;
    
    let mut sent = 0;
    for req in due_soon {
        // Send reminder (email, notification, etc.)
        if send_reminder(&req, pool).await.is_ok() {
            sqlx::query!(
                r#"
                UPDATE ob_kyc.outstanding_requests
                SET last_reminder_at = NOW(),
                    reminder_count = reminder_count + 1,
                    communication_log = communication_log || $2::jsonb
                WHERE request_id = $1
                "#,
                req.request_id,
                json!({
                    "timestamp": Utc::now(),
                    "type": "REMINDER",
                    "channel": "EMAIL",
                    "triggered_by": "SWEEP"
                })
            ).execute(pool).await?;
            sent += 1;
        }
    }
    
    Ok(sent)
}

async fn escalate_overdue_requests(pool: &PgPool) -> Result<i32> {
    // Find requests past due_date + grace_period that aren't already escalated
    let overdue = sqlx::query!(
        r#"
        SELECT request_id, workstream_id, case_id, request_subtype,
               due_date, grace_period_days, escalation_level
        FROM ob_kyc.outstanding_requests
        WHERE status = 'PENDING'
          AND due_date + (grace_period_days || ' days')::interval < CURRENT_DATE
          AND escalation_level = 0
        "#
    ).fetch_all(pool).await?;
    
    let mut escalated = 0;
    for req in overdue {
        // Update to escalated status
        sqlx::query!(
            r#"
            UPDATE ob_kyc.outstanding_requests
            SET status = 'ESCALATED',
                escalated_at = NOW(),
                escalation_level = 1,
                escalation_reason = 'Overdue past grace period'
            WHERE request_id = $1
            "#,
            req.request_id
        ).execute(pool).await?;
        
        // Create escalation alert/task for case manager
        create_escalation_alert(&req, pool).await?;
        
        escalated += 1;
    }
    
    Ok(escalated)
}

async fn expire_stale_requests(pool: &PgPool) -> Result<i32> {
    // Expire requests older than 90 days that are still pending
    let expired = sqlx::query!(
        r#"
        UPDATE ob_kyc.outstanding_requests
        SET status = 'EXPIRED',
            status_reason = 'Auto-expired after 90 days'
        WHERE status IN ('PENDING', 'ESCALATED')
          AND requested_at < CURRENT_DATE - INTERVAL '90 days'
        RETURNING request_id
        "#
    ).fetch_all(pool).await?;
    
    Ok(expired.len() as i32)
}
```

**Cron configuration:**

```yaml
# config/jobs.yaml
jobs:
  request_sweep:
    schedule: "0 6 * * *"  # Daily at 6 AM
    handler: request_sweep::run_request_sweep
    enabled: true
    
  request_reminder_check:
    schedule: "0 */4 * * *"  # Every 4 hours
    handler: request_sweep::send_due_reminders
    enabled: true
```

---

## DSL Usage Examples

### Request Document (Fire and Forget)

```lisp
; Request source of wealth document
(document.request 
  workstream-id:@ws-003 
  type:SOURCE_OF_WEALTH 
  from:"client" 
  due-in-days:14
  notes:"Required for enhanced due diligence - UBO with complex wealth structure")

; Returns immediately:
; {
;   "request_id": "req-456",
;   "status": "PENDING",
;   "due_date": "2026-01-07",
;   "blocks_subject": true
; }

; Agent moves on - doesn't wait
```

### Check Case Status (See Outstanding Requests)

```lisp
(case.full-status case-id:@case-123)

; Returns:
; {
;   "workstreams": [...],
;   "outstanding_requests": [
;     {
;       "request_id": "req-456",
;       "type": "DOCUMENT",
;       "subtype": "SOURCE_OF_WEALTH",
;       "for_workstream": "ws-003",
;       "from": "client",
;       "days_outstanding": 10,
;       "days_overdue": 3,
;       "overdue": true,
;       "reminder_count": 1
;     }
;   ],
;   "blockers": ["SOURCE_OF_WEALTH: Awaiting document (overdue 3 days)"],
;   "whats_next": ["1 overdue request - send reminder, escalate, or waive"]
; }
```

### Handle Overdue Request (Agent Decides)

```lisp
; Option 1: Send another reminder
(request.remind request-id:@req-456 message:"Gentle reminder - document still required")

; Option 2: Escalate
(request.escalate request-id:@req-456 reason:"Client unresponsive after 3 reminders")

; Option 3: Extend deadline
(request.extend request-id:@req-456 days:7 reason:"Client traveling, will provide next week")

; Option 4: Waive requirement (with senior approval)
(document.waive 
  workstream-id:@ws-003 
  type:SOURCE_OF_WEALTH 
  reason:"Low-value relationship, simplified due diligence applies"
  approved-by:@senior-analyst)
```

### Upload Document (Auto-Fulfills)

```lisp
; When client uploads document (via UI or API)
(document.upload 
  workstream-id:@ws-003 
  type:SOURCE_OF_WEALTH 
  file:@uploaded-file)

; Returns:
; {
;   "document_id": "doc-789",
;   "fulfilled_request_id": "req-456",  ← Auto-matched!
;   "workstream_unblocked": true        ← Auto-unblocked!
; }

; Next time agent checks status:
; - Request shows as FULFILLED
; - Workstream shows as IN_PROGRESS (not BLOCKED)
; - No action needed
```

---

## Summary

| Component | Purpose |
|-----------|---------|
| `outstanding_requests` table | Track all fire-and-forget requests |
| Request verbs | Create requests, manage lifecycle |
| Document/Information verbs | Convenience wrappers |
| Auto-fulfillment | Upload → matches pending request → unblocks |
| Status integration | Requests visible in case.full-status |
| Background sweep | Reminders, escalations, expiry (no polling) |

**Agent behavior:**
1. Fire request → verb returns immediately
2. Check status later → sees outstanding requests
3. If overdue → decide: remind, escalate, extend, or waive
4. When fulfilled → automatically reflected in status

**No polling. No tracking. State is truth. Agent observes and acts.**
