# TODO: Workflow Task Stack & Document Entity

> **Status:** ✅ IMPLEMENTED (Phase 1-5)
> **Date:** 2026-01-23
> **Reviews:** Claude + ChatGPT synthesis (5 rounds)
> **Implemented:** 2026-01-23

### Key Design Decisions (from reviews)

| Decision | Rationale |
|----------|----------|
| `task_id` (UUID) as correlation key | Cleaner than string composition, avoids collisions |
| `idempotency_key` REQUIRED | NULL allows duplicates to slip through |
| Queue is EPHEMERAL, events are PERMANENT | Queue for delivery, `workflow_task_events` for audit |
| Only count `Completed` + `cargo_ref` | Failed results shouldn't consume "expected cargo slots" |
| External creates document FIRST | Then callbacks with URI; avoids race conditions |
| `max_age_days` uses `verified_at` | Not `created_at`; reflects when doc was approved |
| CTE form for queue pop | Planner-independent, safer than nested subquery |
| Composite unique on `(task_id, cargo_ref, status)` | Secondary deduplication for multi-result tasks |
| **Separate requirement from document** | Guards check requirements, not raw document existence |
| **Verification on version, not document** | Each submission verified independently; supports re-uploads |
| **Standardized rejection codes** | Drives client messaging, enables retry automation |
| **Bundle is the norm** | Even single-doc returns use bundle payload; simplifies integrations |
| **idempotency scoped to task** | `UNIQUE(task_id, idempotency_key)` not global; avoids vendor key collisions |
| **`satisfied_at` set on transition** | When requirement reaches `required_state`, set timestamp |

## Problem Statement

The current `ob-workflow` crate can:
- Define workflows in YAML (states, transitions, guards)
- Track workflow instances in PostgreSQL
- Evaluate guards and emit blockers with `resolution_action` (DSL verb to execute)

But it **cannot**:
- Track pending async tasks
- Receive completion signals from external systems
- Auto-advance workflows when tasks complete

The missing piece is the **return path** - when a workflow emits a task (e.g., "solicit passport"), there's no mechanism to receive the result and resume.

---

## Design: Queue-Based Async Return Path

### Core Principles

```
PUSH: Any system can push a TaskResult onto the queue
POP:  Single consumer drains queue, advances workflows

TaskResult is ONE struct - uniform interface
Cargo is always a POINTER (URI) - actual payload stored elsewhere
Routing by task_id (UUID) not string correlation
```

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OUTBOUND: Workflow emits task                                              │
│                                                                             │
│  Blocker detected → resolution DSL emitted:                                │
│  (document.solicit :entity-id <uuid> :doc-type PASSPORT)                   │
│      │                                                                      │
│      ▼                                                                      │
│  DSL Executor:                                                              │
│    1. Generate task_id (UUID) ← this IS the correlation ID                 │
│    2. Insert into workflow_pending_tasks                                   │
│    3. Call external system (Camunda, email, portal, etc.)                  │
│    4. Pass task_id + callback URL to external system                       │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                    External system works...
                              │
┌─────────────────────────────────────────────────────────────────────────────┐
│  INBOUND: Task completion                                                   │
│                                                                             │
│  External system pushes TaskResult to queue:                               │
│  - Via webhook: POST /api/workflow/task-complete                           │
│  - Payload: { task_id, status, cargo_ref, idempotency_key }               │
│  - Note: verb NOT required - looked up from pending_tasks                  │
│                                                                             │
│  Multiple results per task allowed (passport + proof_of_address)           │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────────────────┐
│  LISTENER: Single consumer drains queue                                     │
│                                                                             │
│  loop {                                                                     │
│    result = queue.pop();  // FOR UPDATE SKIP LOCKED                        │
│    pending = lookup_by_task_id(result.task_id);                            │
│    update_pending_status(pending, result);                                 │
│    if all_results_received(pending) {                                      │
│        workflow_engine.try_advance(pending.instance_id);                   │
│    }                                                                        │
│  }                                                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Design: Document as First-Class Entity

Documents are the universal cargo container. A "document" can be:

| Type | Content | Storage |
|------|---------|---------|
| Passport scan | JPEG/PDF | `blob_ref` → S3, `ocr_extracted` → indexed fields |
| Subscription form | JSON | `structured_data` → JSONB |
| LEI record | JSON | `structured_data` → JSONB, `source` = "gleif" |
| Board resolution | PDF | `blob_ref` → S3, `ocr_extracted` → signatories, date |

### Why Documents?

1. **Uniform cargo type** - TaskResult.cargo_ref always points to a document
2. **Queryable** - Guards can ask "does entity X have verified document type Y?"
3. **Auditable** - Full history of what was collected, when, from whom
4. **Flexible** - Structured data OR binary blob OR both

---

## Design: Three-Layer Document Model

Separate **what we need** from **what we have** from **what was submitted**:

| Layer | Table | Purpose | Mutability |
|-------|-------|---------|------------|
| A: Requirement | `document_requirements` | What we need from entity | Status changes |
| B: Document | `documents` | Logical identity | Immutable after creation |
| C: Version | `document_versions` | Each submission attempt | Immutable; verification state changes |

### Why Three Layers?

1. **Requirement exists before upload** - Can answer "what's missing for this entity?"
2. **Rejection with retry** - `attempt_count++`, spawn new outreach task with failure reason
3. **Two-tier guards** - `received` for collection milestones, `verified` for approvals
4. **Version history** - Every submission preserved; supports "show me all attempts"
5. **Clear messaging** - Rejection codes drive client communications

### Requirement State Machine

```
     +----------+     +----------+     +----------+     +----------+
     | missing  |---->| requested|---->| received |---->|  in_qa   |----+
     +----------+     +----------+     +----------+     +----------+    |
          ^                                                  |          |
          |                                                  v          |
          |                                            +----------+     |
          |                                            | verified |     |
          |                                            +----------+     |
          |                                                  |          |
          |          +----------+                           v          |
          +----------| rejected |<--------------------------+          |
                     +----------+                                      |
                          |                                            |
                          |  (attempt_count++, new outreach task)     |
                          +--------------------------------------------+

     +----------+
     |  waived  |  (manual override - skips requirement)
     +----------+

     +----------+
     | expired  |  (document validity lapsed)
     +----------+
```

### Business Rules

| Transition | Trigger | Action |
|------------|---------|--------|
| missing → requested | Outreach task created | Set `current_task_id` |
| requested → received | Document version uploaded | Set `latest_version_id`, clear task |
| received → in_qa | QA pipeline triggered | Auto-transition |
| in_qa → verified | QA passes | Set `requirement.status = verified` |
| in_qa → rejected | QA fails | Record `rejection_code`, increment `attempt_count` |
| rejected → requested | Re-request triggered | New task with rejection context |
| any → waived | Manual override | Clears blocker without document |

---

## Schema

### Rejection Reason Codes (reference data)

```sql
-- Standardized rejection reasons - drives client messaging
CREATE TABLE "ob-poc".rejection_reason_codes (
    code TEXT PRIMARY KEY,
    category TEXT NOT NULL,           -- 'quality', 'mismatch', 'validity', 'data', 'format', 'authenticity'
    client_message TEXT NOT NULL,     -- User-facing message
    ops_message TEXT NOT NULL,        -- Internal ops message
    next_action TEXT NOT NULL,        -- What to do next
    is_retryable BOOLEAN DEFAULT true -- Can client retry with different upload?
);

-- Quality issues
INSERT INTO rejection_reason_codes VALUES
('UNREADABLE',      'quality', 'Document image is too blurry to read', 'OCR failed - image quality', 'Please re-upload a clear, high-resolution image', true),
('CUTOFF',          'quality', 'Part of the document is cut off', 'Incomplete capture', 'Ensure all four corners are visible in the image', true),
('GLARE',           'quality', 'Glare obscures important information', 'Light reflection on document', 'Avoid flash and direct lighting when photographing', true),
('LOW_RESOLUTION',  'quality', 'Image resolution too low', 'Below minimum DPI', 'Upload a higher resolution scan (300 DPI minimum)', true),

-- Wrong document
('WRONG_DOC_TYPE',  'mismatch', 'This is not the requested document type', 'Doc type mismatch', 'Please upload the correct document type', true),
('WRONG_PERSON',    'mismatch', 'Document belongs to a different person', 'Name/subject mismatch', 'Upload document for the correct person', true),
('SAMPLE_DOC',      'mismatch', 'This appears to be a sample or specimen', 'Specimen/sample detected', 'Please upload your actual document', true),

-- Validity issues
('EXPIRED',         'validity', 'Document has expired', 'Past expiry date', 'Please provide a current, valid document', true),
('NOT_YET_VALID',   'validity', 'Document is not yet valid', 'Future valid_from date', 'Please provide a currently valid document', true),
('UNDATED',         'validity', 'Document has no issue or expiry date', 'Missing dates', 'Please provide a dated document', true),

-- Data issues
('DOB_MISMATCH',    'data', 'Date of birth does not match our records', 'DOB mismatch vs entity', 'Please verify the correct document or contact support', false),
('NAME_MISMATCH',   'data', 'Name does not match our records', 'Name mismatch vs entity', 'Please verify spelling or provide supporting name change document', false),
('ADDRESS_MISMATCH','data', 'Address does not match declared address', 'Address mismatch', 'Please provide proof of address at declared address', true),

-- Format issues
('UNSUPPORTED_FORMAT', 'format', 'File format not supported', 'Invalid file type', 'Please upload PDF, JPEG, or PNG', true),
('PASSWORD_PROTECTED', 'format', 'Document is password protected', 'Cannot open file', 'Please upload an unprotected version', true),
('CORRUPTED',       'format', 'File appears to be corrupted', 'Cannot read file', 'Please re-upload or try a different file', true),

-- Authenticity (careful with wording - don't accuse)
('SUSPECTED_ALTERATION', 'authenticity', 'Document requires additional verification', 'Possible tampering detected', 'Our team will contact you for verification', false),
('INCONSISTENT_FONTS',   'authenticity', 'Document requires additional verification', 'Font inconsistency detected', 'Our team will contact you for verification', false);
```

### Document Requirements (what we need)

```sql
-- Layer A: What we NEED from entity (exists before any upload)
CREATE TABLE "ob-poc".document_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Scope (workflow or entity level)
    workflow_instance_id UUID REFERENCES workflow_instances(instance_id),
    subject_entity_id UUID REFERENCES entities(entity_id),
    subject_cbu_id UUID REFERENCES cbus(cbu_id),
    
    -- What's required
    doc_type TEXT NOT NULL,           -- 'passport', 'proof_of_address', 'articles_of_incorporation'
    required_state TEXT NOT NULL DEFAULT 'verified'
        CHECK (required_state IN ('received', 'verified')),
    
    -- Current status
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'requested', 'received', 'in_qa', 'verified', 'rejected', 'expired', 'waived')),
    
    -- Retry tracking
    attempt_count INT DEFAULT 0,
    max_attempts INT DEFAULT 3,
    current_task_id UUID REFERENCES workflow_pending_tasks(task_id),
    
    -- Latest document (when received)
    latest_document_id UUID,          -- FK to documents (added after documents table created)
    latest_version_id UUID,           -- FK to document_versions
    
    -- Rejection details (last failure - for messaging)
    last_rejection_code TEXT REFERENCES rejection_reason_codes(code),
    last_rejection_reason TEXT,       -- Optional free-text override
    
    -- Timing
    due_date DATE,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    satisfied_at TIMESTAMPTZ,         -- When status reached required_state
    
    -- Uniqueness: one requirement per doc_type per subject per workflow
    UNIQUE NULLS NOT DISTINCT (workflow_instance_id, subject_entity_id, doc_type)
);

-- Find unsatisfied requirements for a workflow
CREATE INDEX idx_doc_req_workflow_status 
    ON document_requirements(workflow_instance_id, status) 
    WHERE status NOT IN ('verified', 'waived');

-- Find requirements for an entity
CREATE INDEX idx_doc_req_subject 
    ON document_requirements(subject_entity_id, doc_type);

-- Find requirements with active outreach tasks
CREATE INDEX idx_doc_req_task 
    ON document_requirements(current_task_id) 
    WHERE current_task_id IS NOT NULL;
```

### TaskResult Queue (inbound results)

```sql
CREATE TABLE "ob-poc".task_result_queue (
    id BIGSERIAL PRIMARY KEY,
    
    -- Routing (by UUID, not string)
    task_id UUID NOT NULL,
    
    -- Outcome
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'expired')),
    error TEXT,
    
    -- Cargo is always a POINTER (URI)
    cargo_type TEXT,              -- 'document', 'entity', 'screening'
    cargo_ref TEXT,               -- URI: 'document://ob-poc/uuid'
    
    -- Raw payload for audit/debugging (original webhook body)
    payload JSONB,
    
    -- Queue management
    queued_at TIMESTAMPTZ DEFAULT now(),
    processed_at TIMESTAMPTZ,
    
    -- Retry handling
    retry_count INT DEFAULT 0,
    last_error TEXT,
    
    -- Deduplication: idempotency_key scoped to task (not global)
    -- External systems may reuse key formats across vendors
    idempotency_key TEXT NOT NULL
);

-- Primary deduplication: unique per task
CREATE UNIQUE INDEX idx_task_result_queue_idempotency
    ON task_result_queue(task_id, idempotency_key);

-- Secondary dedupe for multi-result safety (backup protection)
CREATE UNIQUE INDEX idx_task_result_queue_dedupe 
    ON task_result_queue(task_id, cargo_ref, status) 
    WHERE cargo_ref IS NOT NULL;

-- Optimized index for queue pop (partial index on unprocessed)
CREATE INDEX idx_task_result_queue_pending 
    ON task_result_queue(processed_at, id) 
    WHERE processed_at IS NULL;

-- Lookup by task
CREATE INDEX idx_task_result_queue_task 
    ON task_result_queue(task_id);
```

### Dead Letter Queue

```sql
CREATE TABLE "ob-poc".task_result_dlq (
    id BIGSERIAL PRIMARY KEY,
    original_id BIGINT NOT NULL,
    task_id UUID NOT NULL,
    status TEXT NOT NULL,
    cargo_type TEXT,
    cargo_ref TEXT,
    error TEXT,
    retry_count INT,
    queued_at TIMESTAMPTZ,
    dead_lettered_at TIMESTAMPTZ DEFAULT now(),
    failure_reason TEXT NOT NULL
);
```

### Pending Tasks (outbound tracking)

```sql
CREATE TABLE "ob-poc".workflow_pending_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Links to workflow
    instance_id UUID NOT NULL REFERENCES workflow_instances(instance_id),
    blocker_type TEXT NOT NULL,
    blocker_key TEXT,
    
    -- What was invoked (source of truth - don't trust external)
    verb TEXT NOT NULL,
    args JSONB,
    
    -- Expected results (multi-result support)
    expected_cargo_count INT DEFAULT 1,  -- How many results expected
    received_cargo_count INT DEFAULT 0,  -- How many completed with cargo
    failed_count INT DEFAULT 0,          -- How many failed/expired
    
    -- State
    status TEXT NOT NULL DEFAULT 'pending' 
        CHECK (status IN ('pending', 'partial', 'completed', 'failed', 'expired', 'cancelled')),
    
    -- Timing
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    
    -- Errors (last error for display)
    last_error TEXT
);

CREATE INDEX idx_pending_tasks_instance 
    ON workflow_pending_tasks(instance_id);
CREATE INDEX idx_pending_tasks_status 
    ON workflow_pending_tasks(status) 
    WHERE status IN ('pending', 'partial');
```

### Task Events History (permanent audit trail)

```sql
-- Permanent record of all task events (queue is ephemeral, this is audit)
-- Queue rows are DELETED after processing; events are kept forever
CREATE TABLE "ob-poc".workflow_task_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES workflow_pending_tasks(task_id),
    
    -- Event type: 'created', 'result_received', 'completed', 'failed', 'expired', 'cancelled'
    event_type TEXT NOT NULL,
    
    -- Result details (for result_received events)
    result_status TEXT,           -- 'completed', 'failed', 'expired'
    cargo_type TEXT,
    cargo_ref TEXT,
    error TEXT,
    
    -- Raw payload for audit (original webhook body)
    payload JSONB,
    
    -- Source tracking
    source TEXT,                  -- 'webhook', 'internal', 'timeout_job'
    idempotency_key TEXT,         -- From the original request
    
    -- Timing
    occurred_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_task_events_task 
    ON workflow_task_events(task_id);
CREATE INDEX idx_task_events_type 
    ON workflow_task_events(event_type, occurred_at);
```

### Documents (Layer B: logical identity)

```sql
-- Layer B: Logical document identity (immutable after creation)
-- Stable identity for "passport for person X" - multiple versions live under this
CREATE TABLE "ob-poc".documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Classification
    document_type TEXT NOT NULL,      -- 'passport', 'subscription_form', 'lei_record'
    
    -- Relationships
    subject_entity_id UUID REFERENCES entities(entity_id),
    subject_cbu_id UUID REFERENCES cbus(cbu_id),
    parent_document_id UUID REFERENCES documents(document_id),
    
    -- Requirement linkage (which requirement this satisfies)
    requirement_id UUID REFERENCES document_requirements(requirement_id),
    
    -- Provenance
    source TEXT NOT NULL,             -- 'upload', 'ocr', 'api', 'gleif', 'workflow'
    source_ref TEXT,                  -- External system ID
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT
);

-- Indexes for lookups
CREATE INDEX idx_documents_subject_type 
    ON documents(subject_entity_id, document_type);
CREATE INDEX idx_documents_requirement 
    ON documents(requirement_id) 
    WHERE requirement_id IS NOT NULL;
```

### Document Versions (Layer C: immutable submissions)

```sql
-- Layer C: Each upload/submission is a new immutable version
-- Never overwrite old versions; only supersede them
CREATE TABLE "ob-poc".document_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(document_id),
    version_no INT NOT NULL DEFAULT 1,
    
    -- Content type
    content_type TEXT NOT NULL,       -- MIME: 'application/json', 'image/jpeg', 'application/pdf'
    
    -- Content (at least one required)
    structured_data JSONB,            -- Parsed JSON/YAML
    blob_ref TEXT,                    -- Pointer to binary: 's3://bucket/key', 'file:///path'
    ocr_extracted JSONB,              -- Indexed fields from OCR/extraction
    
    -- Workflow linkage (which task produced this version)
    task_id UUID REFERENCES workflow_pending_tasks(task_id),
    
    -- Verification status (on VERSION, not document - each submission verified separately)
    verification_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (verification_status IN ('pending', 'in_qa', 'verified', 'rejected')),
    
    -- Rejection details (if rejected)
    rejection_code TEXT REFERENCES rejection_reason_codes(code),
    rejection_reason TEXT,            -- Optional free-text override/detail
    
    -- Verification audit
    verified_by TEXT,
    verified_at TIMESTAMPTZ,
    
    -- Validity period (from document content)
    valid_from DATE,
    valid_to DATE,
    
    -- Quality metrics (from OCR/extraction pipeline)
    quality_score NUMERIC(5,2),       -- 0.00 to 100.00
    extraction_confidence NUMERIC(5,2),
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT,
    
    UNIQUE(document_id, version_no),
    CONSTRAINT version_has_content CHECK (
        structured_data IS NOT NULL OR blob_ref IS NOT NULL
    )
);

-- Find latest version for a document
CREATE INDEX idx_doc_versions_document 
    ON document_versions(document_id, version_no DESC);

-- Find versions by verification status
CREATE INDEX idx_doc_versions_status 
    ON document_versions(verification_status, created_at)
    WHERE verification_status IN ('pending', 'in_qa');

-- Find versions by task
CREATE INDEX idx_doc_versions_task 
    ON document_versions(task_id) 
    WHERE task_id IS NOT NULL;

-- GIN indexes for content search
CREATE INDEX idx_doc_versions_structured 
    ON document_versions USING gin(structured_data jsonb_path_ops)
    WHERE structured_data IS NOT NULL;

CREATE INDEX idx_doc_versions_ocr 
    ON document_versions USING gin(ocr_extracted jsonb_path_ops)
    WHERE ocr_extracted IS NOT NULL;
```

### Document Events (audit trail)

```sql
-- Audit trail for document lifecycle events
CREATE TABLE "ob-poc".document_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(document_id),
    version_id UUID REFERENCES document_versions(version_id),
    
    -- Event type
    event_type TEXT NOT NULL,         -- 'created', 'version_uploaded', 'verified', 'rejected', 'expired'
    
    -- Event details
    old_status TEXT,
    new_status TEXT,
    rejection_code TEXT,
    notes TEXT,
    
    -- Actor
    actor TEXT,                       -- 'system', 'qa_user@example.com', 'api:gleif'
    
    occurred_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_doc_events_document 
    ON document_events(document_id, occurred_at DESC);
```

---

## Cargo Reference URI Scheme

```rust
/// URI scheme for cargo references
/// Typed enum prevents string parsing bugs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CargoRef {
    /// Document entity: document://ob-poc/{document_id}
    Document { schema: String, id: Uuid },
    
    /// Document version (preferred for callbacks): version://ob-poc/{version_id}
    Version { schema: String, id: Uuid },
    
    /// Entity (for entity-creation tasks): entity://ob-poc/{entity_id}
    Entity { schema: String, id: Uuid },
    
    /// Screening result: screening://ob-poc/{screening_id}
    Screening { schema: String, id: Uuid },
    
    /// External system passthrough: external://{system}/{external_id}
    External { system: String, id: String },
}

impl CargoRef {
    pub fn version(id: Uuid) -> Self {
        Self::Version { schema: "ob-poc".into(), id }
    }
    
    pub fn document(id: Uuid) -> Self {
        Self::Document { schema: "ob-poc".into(), id }
    }
    
    pub fn parse(s: &str) -> Result<Self, CargoRefParseError> {
        let parts: Vec<&str> = s.splitn(3, "://").collect();
        if parts.len() < 2 {
            return Err(CargoRefParseError::InvalidFormat);
        }
        
        match parts[0] {
            "version" => {
                let (schema, id) = parse_schema_id(parts[1])?;
                Ok(Self::Version { schema, id })
            }
            "document" => {
                let (schema, id) = parse_schema_id(parts[1])?;
                Ok(Self::Document { schema, id })
            }
            "entity" => {
                let (schema, id) = parse_schema_id(parts[1])?;
                Ok(Self::Entity { schema, id })
            }
            "screening" => {
                let (schema, id) = parse_schema_id(parts[1])?;
                Ok(Self::Screening { schema, id })
            }
            "external" => {
                let parts: Vec<&str> = parts[1].splitn(2, '/').collect();
                Ok(Self::External { 
                    system: parts[0].into(), 
                    id: parts.get(1).unwrap_or(&"").to_string() 
                })
            }
            _ => Err(CargoRefParseError::UnknownScheme(parts[0].into())),
        }
    }
    
    pub fn to_uri(&self) -> String {
        match self {
            Self::Version { schema, id } => format!("version://{}/{}", schema, id),
            Self::Document { schema, id } => format!("document://{}/{}", schema, id),
            Self::Entity { schema, id } => format!("entity://{}/{}", schema, id),
            Self::Screening { schema, id } => format!("screening://{}/{}", schema, id),
            Self::External { system, id } => format!("external://{}/{}", system, id),
        }
    }
}
```

---

## Rust Types

```rust
/// Status of a task result
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum TaskStatus {
    Completed,
    Failed,
    Expired,
}

/// Queue row (from database)
#[derive(Debug, Clone, FromRow)]
pub struct TaskResultRow {
    pub id: i64,
    pub task_id: Uuid,
    pub status: TaskStatus,
    pub cargo_type: Option<String>,
    pub cargo_ref: Option<String>,
    pub error: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub queued_at: chrono::DateTime<chrono::Utc>,
    pub retry_count: i32,
    pub idempotency_key: String,
}

/// Inbound task result (parsed from queue row)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: Uuid,
    pub status: TaskStatus,
    pub cargo_type: Option<String>,
    pub cargo_ref: Option<CargoRef>,
    pub error: Option<String>,
    pub idempotency_key: String,              // REQUIRED for deduplication
    pub payload: Option<serde_json::Value>,   // Raw webhook body for audit
}

impl From<&TaskResultRow> for TaskResult {
    fn from(row: &TaskResultRow) -> Self {
        Self {
            task_id: row.task_id,
            status: row.status,
            cargo_type: row.cargo_type.clone(),
            cargo_ref: row.cargo_ref.as_ref().and_then(|s| CargoRef::parse(s).ok()),
            error: row.error.clone(),
            idempotency_key: row.idempotency_key.clone(),
            payload: row.payload.clone(),
        }
    }
}

/// Outbound pending task
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PendingTask {
    pub task_id: Uuid,
    pub instance_id: Uuid,
    pub blocker_type: String,
    pub blocker_key: Option<String>,
    pub verb: String,
    pub args: Option<serde_json::Value>,
    pub expected_cargo_count: i32,
    pub received_cargo_count: i32,
    pub failed_count: Option<i32>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}

impl PendingTask {
    pub fn is_complete(&self) -> bool {
        self.received_cargo_count >= self.expected_cargo_count
    }
    
    pub fn is_terminal(&self) -> bool {
        let total = self.received_cargo_count + self.failed_count.unwrap_or(0);
        total >= self.expected_cargo_count
    }
}

/// Document with current status (joined view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentWithStatus {
    pub document_id: Uuid,
    pub document_type: String,
    pub subject_entity_id: Option<Uuid>,
    pub requirement_id: Option<Uuid>,
    pub source: String,
    pub latest_version_id: Option<Uuid>,
    pub latest_status: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Document version (immutable submission)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub version_id: Uuid,
    pub document_id: Uuid,
    pub version_no: i32,
    pub content_type: String,
    pub blob_ref: Option<String>,
    pub task_id: Option<Uuid>,
    pub verification_status: String,
    pub rejection_code: Option<String>,
    pub rejection_reason: Option<String>,
    pub verified_by: Option<String>,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub valid_from: Option<chrono::NaiveDate>,
    pub valid_to: Option<chrono::NaiveDate>,
    pub quality_score: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Document requirement (what we need from entity)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DocumentRequirement {
    pub requirement_id: Uuid,
    pub workflow_instance_id: Option<Uuid>,
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    pub doc_type: String,
    pub required_state: String,
    pub status: String,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub current_task_id: Option<Uuid>,
    pub latest_document_id: Option<Uuid>,
    pub latest_version_id: Option<Uuid>,
    pub last_rejection_code: Option<String>,
    pub last_rejection_reason: Option<String>,
    pub due_date: Option<chrono::NaiveDate>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub satisfied_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl DocumentRequirement {
    pub fn is_satisfied(&self) -> bool {
        matches!(self.status.as_str(), "verified" | "waived")
    }
    
    pub fn can_retry(&self) -> bool {
        self.status == "rejected" && self.attempt_count < self.max_attempts
    }
}

/// Rejection reason code (from reference table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionCode {
    pub code: String,
    pub category: String,
    pub client_message: String,
    pub ops_message: String,
    pub next_action: String,
    pub is_retryable: bool,
}
```

---

## Listener Implementation

```rust
const MAX_RETRIES: i32 = 3;

async fn task_result_listener(pool: PgPool, engine: Arc<WorkflowEngine>) {
    loop {
        // Atomic pop with CTE form (safer than subquery, planner-independent)
        let result = sqlx::query_as!(TaskResultRow, r#"
            WITH next AS (
                SELECT id
                FROM "ob-poc".task_result_queue
                WHERE processed_at IS NULL
                ORDER BY id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            UPDATE "ob-poc".task_result_queue q
            SET processed_at = now()
            FROM next
            WHERE q.id = next.id
            RETURNING 
                q.id,
                q.task_id,
                q.status as "status: TaskStatus",
                q.cargo_type,
                q.cargo_ref,
                q.error,
                q.payload,
                q.retry_count,
                q.queued_at,
                q.idempotency_key
        "#)
        .fetch_optional(&pool)
        .await;
        
        match result {
            Ok(Some(row)) => {
                let task_result = TaskResult::from(&row);
                match handle_task_result(&pool, &engine, &task_result).await {
                    Ok(_) => {
                        // Success - delete from queue (events table has permanent record)
                        delete_queue_row(&pool, row.id).await;
                    }
                    Err(e) if row.retry_count < MAX_RETRIES => {
                        // Requeue with incremented retry
                        requeue_with_retry(&pool, row.id, &e.to_string()).await;
                    }
                    Err(e) => {
                        // Move to DLQ (preserves row for investigation)
                        move_to_dlq(&pool, &row, &e.to_string()).await;
                        tracing::error!(?e, task_id = %task_result.task_id, "Task moved to DLQ");
                    }
                }
            }
            Ok(None) => {
                // Queue empty, wait
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                tracing::error!(?e, "Failed to pop from queue");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_task_result(
    pool: &PgPool, 
    engine: &WorkflowEngine, 
    result: &TaskResult
) -> Result<()> {
    // 1. Find the pending task (source of truth for verb, args, etc.)
    let pending = sqlx::query_as!(PendingTask, r#"
        SELECT * FROM workflow_pending_tasks WHERE task_id = $1
    "#, result.task_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Unknown task_id: {}", result.task_id))?;
    
    // 2. Record event in permanent history table (audit trail)
    //    Queue is ephemeral and will be deleted; events are kept forever
    sqlx::query!(r#"
        INSERT INTO workflow_task_events 
            (task_id, event_type, result_status, cargo_type, cargo_ref, error, 
             payload, source, idempotency_key, occurred_at)
        VALUES ($1, 'result_received', $2, $3, $4, $5, $6, 'webhook', $7, now())
    "#, 
        result.task_id,
        result.status.as_str(),
        result.cargo_type,
        result.cargo_ref.as_ref().map(|c| c.to_uri()),
        result.error,
        result.payload,           // Raw webhook body for audit
        result.idempotency_key
    )
    .execute(pool)
    .await?;
    
    // 3. Link version to task if it's a completed document cargo
    //    NOTE: Version must already exist (created by external via POST /api/documents/{id}/versions)
    //    cargo_ref now points to version://ob-poc/{version_id}
    if result.status == TaskStatus::Completed {
        if let Some(CargoRef::Version { id, .. }) = &result.cargo_ref {
            let updated = sqlx::query!(r#"
                UPDATE document_versions SET task_id = $1 
                WHERE version_id = $2 AND task_id IS NULL
            "#, result.task_id, id)
            .execute(pool)
            .await?;
            
            if updated.rows_affected() == 0 {
                tracing::warn!(
                    task_id = %result.task_id,
                    version_id = %id,
                    "Version not found or already linked to another task"
                );
            }
            
            // Update requirement status if version is linked to a document with a requirement
            sqlx::query!(r#"
                UPDATE document_requirements dr
                SET status = 'received',
                    latest_version_id = $2,
                    updated_at = now()
                FROM document_versions dv
                JOIN documents d ON d.document_id = dv.document_id
                WHERE dv.version_id = $2
                  AND d.requirement_id = dr.requirement_id
                  AND dr.status IN ('missing', 'requested', 'rejected')
            "#, result.task_id, id)
            .execute(pool)
            .await?;
        }
    }
    
    // 4. Update pending task counters
    //    IMPORTANT: Only increment received_cargo_count for Completed + cargo_ref
    //    Failed/expired results are tracked separately to avoid prematurely "completing" task
    let (new_received, new_failed) = match result.status {
        TaskStatus::Completed if result.cargo_ref.is_some() => (1, 0),
        TaskStatus::Failed | TaskStatus::Expired => (0, 1),
        _ => (0, 0),  // Completed without cargo - unusual, don't count
    };
    
    let updated_pending = sqlx::query_as!(PendingTask, r#"
        UPDATE workflow_pending_tasks 
        SET received_cargo_count = received_cargo_count + $2,
            failed_count = COALESCE(failed_count, 0) + $3,
            last_error = COALESCE($4, last_error)
        WHERE task_id = $1
        RETURNING *
    "#, result.task_id, new_received, new_failed, result.error)
    .fetch_one(pool)
    .await?;
    
    // 5. Determine new status based on updated counts
    let new_status = if updated_pending.received_cargo_count >= updated_pending.expected_cargo_count {
        "completed"
    } else if updated_pending.failed_count.unwrap_or(0) > 0 
           && updated_pending.received_cargo_count + updated_pending.failed_count.unwrap_or(0) 
              >= updated_pending.expected_cargo_count {
        // All expected results received, but some failed
        "failed"
    } else if updated_pending.received_cargo_count > 0 {
        "partial"
    } else {
        "pending"
    };
    
    sqlx::query!(r#"
        UPDATE workflow_pending_tasks 
        SET status = $2,
            completed_at = CASE WHEN $2 IN ('completed', 'failed') THEN now() ELSE NULL END
        WHERE task_id = $1
    "#, result.task_id, new_status)
    .execute(pool)
    .await?;
    
    // 6. Try to advance workflow if task is complete (success or all-failed)
    if new_status == "completed" || new_status == "failed" {
        // Record terminal event
        sqlx::query!(r#"
            INSERT INTO workflow_task_events 
                (task_id, event_type, occurred_at)
            VALUES ($1, $2, now())
        "#, result.task_id, new_status)
        .execute(pool)
        .await?;
        
        engine.try_advance(updated_pending.instance_id).await?;
    }
    
    Ok(())
}

/// Delete processed queue row (queue is ephemeral)
/// Called after handle_task_result succeeds
async fn delete_queue_row(pool: &PgPool, queue_id: i64) {
    sqlx::query!("DELETE FROM task_result_queue WHERE id = $1", queue_id)
        .execute(pool)
        .await
        .ok();
}

async fn requeue_with_retry(pool: &PgPool, id: i64, error: &str) {
    sqlx::query!(r#"
        UPDATE task_result_queue 
        SET processed_at = NULL,
            retry_count = retry_count + 1,
            last_error = $2
        WHERE id = $1
    "#, id, error)
    .execute(pool)
    .await
    .ok();
}

async fn move_to_dlq(pool: &PgPool, row: &TaskResultRow, reason: &str) {
    sqlx::query!(r#"
        INSERT INTO task_result_dlq 
            (original_id, task_id, status, cargo_type, cargo_ref, error, retry_count, queued_at, failure_reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
    "#, 
        row.id, row.task_id, row.status, row.cargo_type, row.cargo_ref, 
        row.error, row.retry_count, row.queued_at, reason
    )
    .execute(pool)
    .await
    .ok();
    
    sqlx::query!("DELETE FROM task_result_queue WHERE id = $1", row.id)
        .execute(pool)
        .await
        .ok();
}
```

---

## Bundle Payload Format

All task callbacks use a **bundle payload** format, even for single-item returns. This simplifies external integrations and avoids special-casing.

### Standard Bundle Structure

```json
{
  "task_id": "uuid",
  "status": "completed",
  "idempotency_key": "vendor-event-12345",
  "items": [
    {
      "cargo_ref": "version://ob-poc/version-uuid-1",
      "doc_type": "passport",
      "status": "completed"
    },
    {
      "cargo_ref": "version://ob-poc/version-uuid-2",
      "doc_type": "proof_of_address",
      "status": "completed"
    }
  ]
}
```

### Handling Rules

| Scenario | Handling |
|----------|----------|
| All items completed | Task `completed`, update all requirements to `received` |
| Some items completed | Task `partial`, update completed requirements only |
| All items failed | Task `failed`, keep requirement status, record errors |
| Mixed results | Task `partial`, update per-item status |

### Partial Bundle + Re-request Flow

```
1. Task expects 2 docs: passport + proof_of_address
2. External returns bundle with only passport (proof_of_address missing)
3. Listener:
   - Updates passport requirement to 'received'
   - Task status → 'partial'
   - proof_of_address requirement stays 'requested'
4. Re-request job (scheduled):
   - Finds incomplete requirements where attempt_count < max_attempts
   - Spawns new outreach task for missing doc_types only
   - Includes rejection_reason context if any prior attempts failed
```

### Single-Item Bundle (same format)

```json
{
  "task_id": "uuid",
  "status": "completed",
  "idempotency_key": "vendor-event-12346",
  "items": [
    {
      "cargo_ref": "version://ob-poc/version-uuid-3",
      "doc_type": "passport",
      "status": "completed"
    }
  ]
}
```

---

## Webhook Endpoint

```rust
/// POST /api/workflow/task-complete
/// 
/// External systems call this to report task completion with bundle payload.
/// 
/// Rules:
/// - All callbacks use bundle format (items array), even for single docs
/// - idempotency_key IS required and scoped to task_id
/// - Version must already exist (external creates via POST /api/documents/{id}/versions)
/// - cargo_ref uses version:// scheme (not document://)

/// Single item in a bundle
#[derive(Debug, Deserialize)]
pub struct BundleItem {
    pub cargo_ref: String,           // version://ob-poc/{version_id}
    pub doc_type: String,            // 'passport', 'proof_of_address'
    pub status: TaskStatus,          // 'completed', 'failed', 'expired'
    #[serde(default)]
    pub error: Option<String>,       // Error message if failed
}

#[derive(Debug, Deserialize)]
pub struct TaskCompleteRequest {
    pub task_id: Uuid,
    pub status: TaskStatus,          // Overall bundle status
    /// REQUIRED: unique key for deduplication (scoped to task_id)
    pub idempotency_key: String,
    /// Bundle items - always present, even for single-doc returns
    pub items: Vec<BundleItem>,
    #[serde(default)]
    pub error: Option<String>,       // Overall error if all failed
}

async fn handle_task_complete(
    State(pool): State<PgPool>,
    Json(req): Json<TaskCompleteRequest>,
) -> Result<StatusCode, AppError> {
    // Validate task exists and is not already terminal
    let pending = sqlx::query_scalar!(r#"
        SELECT status FROM workflow_pending_tasks WHERE task_id = $1
    "#, req.task_id)
    .fetch_optional(&pool)
    .await?;
    
    match pending {
        None => return Err(AppError::NotFound(format!("Task {} not found", req.task_id))),
        Some(status) if status == "completed" || status == "failed" || status == "cancelled" => {
            // Task already terminal - accept idempotently but don't queue
            return Ok(StatusCode::OK);
        }
        _ => {}
    }
    
    // Validate all cargo_refs are version:// scheme and exist
    for item in &req.items {
        let cargo_ref = CargoRef::parse(&item.cargo_ref)?;
        if let CargoRef::Version { id, .. } = &cargo_ref {
            let exists = sqlx::query_scalar!(r#"
                SELECT EXISTS(SELECT 1 FROM document_versions WHERE version_id = $1) as "exists!"
            "#, id)
            .fetch_one(&pool)
            .await?;
            
            if !exists {
                return Err(AppError::BadRequest(format!(
                    "Version {} not found. Create version first via POST /api/documents/{{doc_id}}/versions",
                    id
                )));
            }
        } else {
            return Err(AppError::BadRequest(
                "cargo_ref must use version:// scheme".to_string()
            ));
        }
    }
    
    // Store raw payload for audit
    let payload = serde_json::to_value(&req).ok();
    
    // Insert into queue (listener will process bundle)
    // ON CONFLICT handles duplicate (task_id, idempotency_key)
    let result = sqlx::query!(r#"
        INSERT INTO task_result_queue 
            (task_id, status, cargo_type, payload, idempotency_key)
        VALUES ($1, $2, 'bundle', $3, $4)
        ON CONFLICT ON CONSTRAINT idx_task_result_queue_idempotency DO NOTHING
    "#, 
        req.task_id, 
        req.status.as_str(),
        payload,
        req.idempotency_key
    )
    .execute(&pool)
    .await?;
    
    if result.rows_affected() == 0 {
        // Duplicate (task_id, idempotency_key) - already processed
        Ok(StatusCode::OK)
    } else {
        Ok(StatusCode::ACCEPTED)
    }
}
```

---

## Guard Integration

Guards check **requirement status**, not raw document existence. This enables two-tier progression:
- `received` = evidence collected (allegation)
- `verified` = QA passed (confirmed)

```yaml
# In workflow YAML
states:
  awaiting_identity:
    requirements:
      # Guards check requirement status, NOT document existence
      - all:
          - type: requirement_satisfied
            doc_type: passport
            min_state: verified       # Final approval requires verified
            subject: $entity_id
          - type: requirement_satisfied
            doc_type: proof_of_address
            min_state: verified
            subject: $entity_id
            max_age_days: 90          # Must be recently verified
    
    on_fail:
      blocker:
        type: MissingDocuments
        resolution: "(document.solicit-set :subject-entity-id $entity_id :doc-types [passport proof_of_address])"
        expected_count: 2  # Task expects 2 results

  evidence_collected:
    # Lower bar - just need allegation (received)
    requirements:
      - all:
          - type: requirement_satisfied
            doc_type: passport
            min_state: received       # Allegation is enough to proceed
            subject: $entity_id
```

```rust
/// Requirement status levels
/// Note: rejected/expired are NOT in the ordered progression - they're failure states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementState {
    Missing,
    Requested,
    Received,     // Allegation received
    InQa,
    Verified,     // QA passed
    Waived,       // Manual override
    Rejected,     // QA failed - needs re-upload
    Expired,      // Validity lapsed
}

impl RequirementState {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "missing" => Ok(Self::Missing),
            "requested" => Ok(Self::Requested),
            "received" => Ok(Self::Received),
            "in_qa" => Ok(Self::InQa),
            "verified" => Ok(Self::Verified),
            "waived" => Ok(Self::Waived),
            "rejected" => Ok(Self::Rejected),
            "expired" => Ok(Self::Expired),
            _ => Err(anyhow!("Unknown requirement state: {}", s)),
        }
    }
    
    /// Check if this state satisfies a minimum threshold
    /// rejected/expired do NOT satisfy any threshold (need re-upload)
    pub fn satisfies(&self, min_state: RequirementState) -> bool {
        use RequirementState::*;
        match (self, min_state) {
            // Failure states never satisfy any requirement
            (Rejected, _) | (Expired, _) => false,
            // Waived satisfies anything
            (Waived, _) => true,
            // Verified satisfies anything
            (Verified, _) => true,
            // Check ordered progression: missing < requested < received < in_qa < verified
            (current, threshold) => {
                let order = |s: &RequirementState| -> u8 {
                    match s {
                        Missing => 0,
                        Requested => 1,
                        Received => 2,
                        InQa => 3,
                        Verified => 4,
                        Waived => 5,
                        Rejected | Expired => 0, // Never reached in this branch
                    }
                };
                order(current) >= order(&threshold)
            }
        }
    }
    
    /// Is this a failure state that needs re-upload?
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Rejected | Self::Expired)
    }
    
    /// Is this a terminal success state?
    pub fn is_satisfied(&self) -> bool {
        matches!(self, Self::Verified | Self::Waived)
    }
}

/// Guard requirement types
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Requirement {
    /// Check if a document requirement is satisfied to at least min_state
    RequirementSatisfied {
        doc_type: String,
        min_state: String,            // 'received', 'verified'
        subject: String,              // Variable reference like "$entity_id"
        #[serde(default)]
        max_age_days: Option<i32>,    // For recency check on verified_at
    },
    /// Legacy: check document existence directly (deprecated, use RequirementSatisfied)
    DocumentExists {
        document_type: String,
        status: String,
        subject: String,
        #[serde(default)]
        max_age_days: Option<i32>,
    },
    All { all: Vec<Requirement> },
    Any { any: Vec<Requirement> },
}

impl RequirementEvaluator {
    pub async fn evaluate(
        &self,
        req: &Requirement,
        context: &WorkflowContext,
    ) -> Result<bool> {
        match req {
            Requirement::RequirementSatisfied { doc_type, min_state, subject, max_age_days } => {
                let entity_id = context.resolve_variable(subject)?;
                let min = RequirementState::from_str(min_state)?;
                self.check_requirement_satisfied(entity_id, doc_type, min, *max_age_days).await
            }
            // Legacy support - maps to requirement check
            Requirement::DocumentExists { document_type, status, subject, max_age_days } => {
                let entity_id = context.resolve_variable(subject)?;
                let min = RequirementState::from_str(status)?;
                self.check_requirement_satisfied(entity_id, document_type, min, *max_age_days).await
            }
            Requirement::All { all } => {
                for r in all {
                    if !self.evaluate(r, context).await? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Requirement::Any { any } => {
                for r in any {
                    if self.evaluate(r, context).await? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
    
    /// Check if requirement for doc_type is satisfied to at least min_state
    async fn check_requirement_satisfied(
        &self,
        entity_id: Uuid,
        doc_type: &str,
        min_state: RequirementState,
        max_age_days: Option<i32>,
    ) -> Result<bool> {
        // Query requirement status directly
        // max_age_days applies to satisfied_at (when requirement reached required state)
        let row = sqlx::query!(
            r#"
            SELECT 
                status,
                satisfied_at
            FROM document_requirements
            WHERE subject_entity_id = $1
              AND doc_type = $2
            "#,
            entity_id,
            doc_type
        )
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            None => Ok(false),  // No requirement exists = not satisfied
            Some(r) => {
                let current = RequirementState::from_str(&r.status)?;
                
                // Check if current state satisfies threshold
                // Note: rejected/expired states will return false (need re-upload)
                if !current.satisfies(min_state) {
                    return Ok(false);
                }
                
                // Check recency if specified
                if let (Some(days), Some(satisfied)) = (max_age_days, r.satisfied_at) {
                    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                    if satisfied < cutoff {
                        return Ok(false);  // Too old
                    }
                }
                
                Ok(true)
            }
        }
    }
    
    /// Find all unsatisfied requirements for a workflow
    pub async fn find_unsatisfied(
        &self,
        workflow_instance_id: Uuid,
    ) -> Result<Vec<UnsatisfiedRequirement>> {
        let rows = sqlx::query_as!(
            UnsatisfiedRequirement,
            r#"
            SELECT 
                requirement_id,
                doc_type,
                subject_entity_id,
                status,
                required_state,
                attempt_count,
                last_rejection_code,
                last_rejection_reason
            FROM document_requirements
            WHERE workflow_instance_id = $1
              AND status NOT IN ('verified', 'waived')
            "#,
            workflow_instance_id
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows)
    }
}

/// Unsatisfied requirement (for blocker generation)
#[derive(Debug, Clone)]
pub struct UnsatisfiedRequirement {
    pub requirement_id: Uuid,
    pub doc_type: String,
    pub subject_entity_id: Uuid,
    pub status: String,
    pub required_state: String,
    pub attempt_count: i32,
    pub last_rejection_code: Option<String>,
    pub last_rejection_reason: Option<String>,
}

impl UnsatisfiedRequirement {
    /// Generate client-facing message for re-request
    pub fn rejection_message(&self, codes: &RejectionCodeLookup) -> Option<String> {
        self.last_rejection_code.as_ref().and_then(|code| {
            codes.get(code).map(|rc| format!(
                "{} {}",
                rc.client_message,
                rc.next_action
            ))
        })
    }
}
```

---

## Blob Storage Abstraction

```rust
/// Abstract blob storage for document binaries
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Store binary content, return reference URI
    async fn store(&self, key: &str, content: &[u8], content_type: &str) -> Result<String>;
    
    /// Fetch binary content by reference
    async fn fetch(&self, blob_ref: &str) -> Result<Vec<u8>>;
    
    /// Delete binary content
    async fn delete(&self, blob_ref: &str) -> Result<()>;
    
    /// Generate presigned URL for direct access (optional)
    async fn presigned_url(&self, blob_ref: &str, expires_secs: u64) -> Result<Option<String>> {
        Ok(None)  // Default: not supported
    }
}

/// Local filesystem implementation (for POC)
pub struct LocalBlobStore {
    base_path: PathBuf,
}

impl LocalBlobStore {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self { base_path: base_path.into() }
    }
}

#[async_trait]
impl BlobStore for LocalBlobStore {
    async fn store(&self, key: &str, content: &[u8], _content_type: &str) -> Result<String> {
        let path = self.base_path.join(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, content).await?;
        Ok(format!("file://{}", path.display()))
    }
    
    async fn fetch(&self, blob_ref: &str) -> Result<Vec<u8>> {
        let path = blob_ref.strip_prefix("file://").ok_or_else(|| anyhow!("Invalid local blob ref"))?;
        Ok(tokio::fs::read(path).await?)
    }
    
    async fn delete(&self, blob_ref: &str) -> Result<()> {
        let path = blob_ref.strip_prefix("file://").ok_or_else(|| anyhow!("Invalid local blob ref"))?;
        tokio::fs::remove_file(path).await?;
        Ok(())
    }
}

/// S3-compatible implementation (for production)
pub struct S3BlobStore {
    client: aws_sdk_s3::Client,
    bucket: String,
}

// ... S3 implementation
```

---

## Implementation Plan

### Phase 1: Schema & Types ✅ COMPLETE
- [x] Create migration: `rejection_reason_codes` (reference data)
- [x] Create migration: `document_requirements`
- [x] Create migration: `documents`, `document_versions`, `document_events`
- [x] Create migration: `task_result_queue`, `task_result_dlq`, `workflow_task_events`
- [x] Create migration: `workflow_pending_tasks`
- [x] Define Rust types: `TaskResultRow`, `TaskResult`, `TaskStatus`, `CargoRef`, `PendingTask`
- [x] Define Rust types: `Document`, `DocumentVersion`, `DocumentRequirement`
- [x] Define Rust types: `RequirementState`, `UnsatisfiedRequirement`, `RejectionCode`

**Files created:**
- `migrations/049_workflow_task_queue_documents.sql` - Comprehensive migration with all tables
- `rust/crates/ob-workflow/src/cargo_ref.rs` - CargoRef enum and parsing
- `rust/crates/ob-workflow/src/task_queue.rs` - TaskResult, TaskStatus, TaskResultRow, PendingTask
- `rust/crates/ob-workflow/src/document.rs` - Document, DocumentVersion, DocumentRequirement, RequirementState

### Phase 2: Outbound (emit tasks) ✅ COMPLETE (via verbs)
- [x] Create `document.solicit` verb (plugin handler)
- [x] Create `document.solicit-set` verb (multi-doc)
- [x] Create `requirement.create` verb (initialize requirements)
- [x] Link requirement to task on outreach

**Files created:**
- `rust/config/verbs/document.yaml` - 7 document verbs
- `rust/config/verbs/requirement.yaml` - 5 requirement verbs

### Phase 3: Inbound (receive results) ✅ COMPLETE
- [x] Create webhook endpoint: `POST /api/workflow/task-complete`
- [x] Create webhook endpoint: `POST /api/documents` (external creates doc first)
- [x] Create webhook endpoint: `POST /api/documents/{id}/versions` (new version upload)
- [x] Implement queue listener with retry + DLQ
- [x] Wire listener to workflow engine `try_advance()`
- [x] Handle multiple results per task
- [x] Update requirement status on document receipt

**Files created:**
- `rust/src/api/workflow_routes.rs` - All HTTP endpoints
- `rust/crates/ob-workflow/src/listener.rs` - Queue listener with retry and DLQ handling

### Phase 4: QA Pipeline Integration ✅ COMPLETE (via verbs & endpoints)
- [x] Create `document.verify` verb (QA approves)
- [x] Create `document.reject` verb (QA rejects with reason code)
- [x] POST /api/documents/:doc_id/versions/:version_id/verify endpoint
- [x] POST /api/documents/:doc_id/versions/:version_id/reject endpoint
- [x] Update requirement status on verification result

### Phase 5: Document Entity & Guards ✅ COMPLETE
- [x] Create `document.*` DSL verbs (create, list, get-versions)
- [x] Implement `BlobStore` trait + local filesystem impl
- [x] Update workflow guards to use `requirement_satisfied` type
- [x] Add `RequirementSatisfied` and `DocumentExists` guard variants

**Files created:**
- `rust/crates/ob-workflow/src/blob_store.rs` - BlobStore trait, LocalBlobStore, InMemoryBlobStore

### Phase 6: Testing
- [ ] Unit tests for `CargoRef` parsing
- [ ] Unit tests for `RequirementState` ordering
- [ ] Unit tests for `Requirement` evaluation
- [ ] Integration test: emit → mock external → callback → advance
- [ ] Integration test: upload → QA reject → re-request → re-upload → verify
- [ ] DLQ test: verify failed tasks land in DLQ after retries
- [ ] Multi-result test: task with expected_count=2

**Note:** Phase 6 testing is deferred - implementation compiles but needs integration tests after migration is applied.

---

## Files Created/Modified

| Status | File | Description |
|--------|------|-------------|
| ✅ CREATED | `migrations/049_workflow_task_queue_documents.sql` | All tables in one migration |
| ✅ CREATED | `rust/crates/ob-workflow/src/task_queue.rs` | TaskResultRow, TaskResult, TaskStatus, PendingTask |
| ✅ CREATED | `rust/crates/ob-workflow/src/cargo_ref.rs` | CargoRef enum + parsing |
| ✅ CREATED | `rust/crates/ob-workflow/src/blob_store.rs` | BlobStore trait + LocalBlobStore + InMemoryBlobStore |
| ✅ CREATED | `rust/crates/ob-workflow/src/listener.rs` | Queue listener with retry + DLQ handling |
| ✅ CREATED | `rust/crates/ob-workflow/src/document.rs` | Document, DocumentVersion, DocumentRequirement, RequirementState |
| ✅ EDITED | `rust/crates/ob-workflow/src/definition.rs` | Added RequirementSatisfied, DocumentExists guard types |
| ✅ EDITED | `rust/crates/ob-workflow/src/requirements.rs` | Added check_requirement_satisfied method |
| ✅ EDITED | `rust/crates/ob-workflow/src/lib.rs` | Export new modules |
| ✅ EDITED | `rust/crates/ob-workflow/Cargo.toml` | Added tokio macros feature |
| ✅ CREATED | `rust/config/verbs/document.yaml` | 7 document verbs (solicit, verify, reject, etc.) |
| ✅ CREATED | `rust/config/verbs/requirement.yaml` | 5 requirement verbs (create, waive, etc.) |
| ✅ CREATED | `rust/src/api/workflow_routes.rs` | Webhook + document API endpoints |
| ✅ EDITED | `rust/src/api/mod.rs` | Export workflow_routes module |

### Notes on Implementation

1. **Single Migration**: All tables consolidated into `049_workflow_task_queue_documents.sql` for FK ordering
2. **Runtime Queries**: Used `sqlx::query()` instead of `sqlx::query!()` for tables that don't exist at compile time
3. **Feature Gates**: Database-dependent code gated behind `#[cfg(feature = "database")]`
4. **Option<T>**: Used consistently for nullable fields per design spec
5. **Cargo Check**: Full workspace compiles successfully
