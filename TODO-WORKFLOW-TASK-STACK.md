# TODO: Workflow Task Stack & Document Entity

> **Status:** Draft for peer review
> **Date:** 2026-01-23
> **Context:** ob-workflow crate exists but lacks async task completion pattern

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

## Design: Stack Machine Semantics

Inspired by Forth: single stack, uniform result type, pointer-based cargo.

### Core Principle

```
PUSH: Any system can push a TaskResult onto the stack
POP:  Single listener consumes results, advances workflows

TaskResult is ONE struct - uniform interface
Cargo is always a POINTER - actual payload stored elsewhere
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
│    1. Generate correlation_id                                               │
│    2. Insert into workflow_pending_tasks                                   │
│    3. Call external system (Camunda, email, portal, etc.)                  │
│    4. Pass correlation_id + callback info to external system               │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                    External system works...
                              │
┌─────────────────────────────────────────────────────────────────────────────┐
│  INBOUND: Task completion                                                   │
│                                                                             │
│  External system pushes TaskResult to stack:                               │
│  - Via webhook: POST /api/workflow/task-complete                           │
│  - Via DB insert: INSERT INTO task_result_queue                            │
│  - Via message queue: Kafka topic / Redis stream                           │
│                                                                             │
│  TaskResult {                                                               │
│    correlation_id: "task-abc-123",                                         │
│    verb: "document.solicit",                                                │
│    status: Completed,                                                       │
│    cargo_type: "document",                                                  │
│    cargo_ref: "document://ob-poc/uuid-xyz",  ← POINTER, not payload        │
│  }                                                                          │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────────────────┐
│  LISTENER: Single consumer pops stack                                       │
│                                                                             │
│  loop {                                                                     │
│    result = stack.pop();                                                   │
│    instance = lookup_by_correlation(result.correlation_id);                │
│    instance.store_cargo_ref(result.cargo_ref);                             │
│    workflow_engine.try_advance(instance.id);                               │
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
| KYC questionnaire | YAML/JSON | `structured_data` → JSONB |

### Why Documents?

1. **Uniform cargo type** - TaskResult.cargo_ref always points to a document
2. **Queryable** - Guards can ask "does entity X have verified document type Y?"
3. **Auditable** - Full history of what was collected, when, from whom
4. **Flexible** - Structured data OR binary blob OR both (OCR extracts structure from blob)

---

## Schema

### TaskResult (the ONE return type)

```sql
CREATE TABLE "ob-poc".task_result_queue (
    id BIGSERIAL PRIMARY KEY,
    
    -- Routing
    correlation_id TEXT NOT NULL,
    
    -- What was called
    verb TEXT NOT NULL,
    
    -- Outcome
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'expired')),
    error TEXT,
    
    -- Cargo is always a POINTER
    cargo_type TEXT,              -- 'document', 'entity', 'screening', etc.
    cargo_ref TEXT,               -- URI: 'document://ob-poc/uuid'
    
    -- Queue management
    queued_at TIMESTAMPTZ DEFAULT now(),
    processed_at TIMESTAMPTZ,
    
    -- Deduplication
    idempotency_key TEXT UNIQUE
);

CREATE INDEX idx_task_result_queue_pending 
    ON task_result_queue(id) 
    WHERE processed_at IS NULL;
```

### Pending Tasks (outbound tracking)

```sql
CREATE TABLE "ob-poc".workflow_pending_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    correlation_id TEXT UNIQUE NOT NULL,
    
    -- Links to workflow
    instance_id UUID NOT NULL REFERENCES workflow_instances(instance_id),
    blocker_type TEXT NOT NULL,
    blocker_key TEXT,
    
    -- What was invoked
    verb TEXT NOT NULL,
    args JSONB,
    
    -- State
    status TEXT NOT NULL DEFAULT 'pending' 
        CHECK (status IN ('pending', 'completed', 'failed', 'expired', 'cancelled')),
    
    -- Timing
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    
    -- Result (when complete)
    cargo_ref TEXT,
    error TEXT
);

CREATE INDEX idx_pending_tasks_instance 
    ON workflow_pending_tasks(instance_id);
CREATE INDEX idx_pending_tasks_status 
    ON workflow_pending_tasks(status) 
    WHERE status = 'pending';
```

### Documents (universal container)

```sql
CREATE TABLE "ob-poc".documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Classification
    document_type TEXT NOT NULL,      -- 'passport', 'subscription_form', 'lei_record'
    content_type TEXT NOT NULL,       -- MIME: 'application/json', 'image/jpeg', 'application/pdf'
    
    -- Content (at least one required)
    structured_data JSONB,            -- Parsed JSON/YAML
    blob_ref TEXT,                    -- Pointer to binary: 's3://bucket/key'
    ocr_extracted JSONB,              -- Indexed fields from OCR
    
    -- Relationships
    subject_entity_id UUID REFERENCES entities(entity_id),
    subject_cbu_id UUID REFERENCES cbus(cbu_id),
    parent_document_id UUID REFERENCES documents(document_id),
    
    -- Provenance
    source TEXT NOT NULL,             -- 'upload', 'ocr', 'api', 'gleif', 'workflow'
    source_ref TEXT,                  -- External system ID
    correlation_id TEXT,              -- Links to workflow task
    
    -- Lifecycle
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'verified', 'rejected', 'expired', 'superseded')),
    verified_by TEXT,
    verified_at TIMESTAMPTZ,
    rejection_reason TEXT,
    
    -- Validity
    valid_from DATE,
    valid_to DATE,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT,
    updated_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT document_has_content CHECK (
        structured_data IS NOT NULL OR blob_ref IS NOT NULL
    )
);

-- Workflow guard queries
CREATE INDEX idx_documents_subject_type_status 
    ON documents(subject_entity_id, document_type, status);

-- Full-text search on content
CREATE INDEX idx_documents_content_gin 
    ON documents USING gin(
        COALESCE(structured_data, '{}') || COALESCE(ocr_extracted, '{}')
    );

-- Correlation lookup (from task results)
CREATE INDEX idx_documents_correlation 
    ON documents(correlation_id) 
    WHERE correlation_id IS NOT NULL;
```

---

## Rust Types

### TaskResult (uniform return)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub correlation_id: String,
    pub verb: String,
    pub status: TaskStatus,
    
    // Cargo is always a POINTER
    pub cargo_type: Option<String>,   // "document", "entity", etc.
    pub cargo_ref: Option<String>,    // URI: "document://ob-poc/uuid"
    
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Completed,
    Failed,
    Expired,
}
```

### PendingTask (outbound)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTask {
    pub task_id: Uuid,
    pub correlation_id: String,
    pub instance_id: Uuid,
    pub blocker_type: String,
    pub blocker_key: Option<String>,
    pub verb: String,
    pub args: serde_json::Value,
    pub status: PendingTaskStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Blocker {
    /// Convert blocker to pending task for tracking
    pub fn to_pending_task(&self, instance_id: Uuid) -> PendingTask {
        PendingTask {
            task_id: Uuid::new_v4(),
            correlation_id: format!("{}:{}:{}", 
                instance_id, 
                self.blocker_type, 
                self.key().unwrap_or_default()
            ),
            instance_id,
            blocker_type: self.blocker_type.to_string(),
            blocker_key: self.key(),
            verb: self.resolution_action.clone().unwrap_or_default(),
            args: self.to_dsl_args(),
            status: PendingTaskStatus::Pending,
            created_at: Utc::now(),
            expires_at: self.deadline,
        }
    }
}
```

### Document

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub document_id: Uuid,
    pub document_type: String,
    pub content_type: String,
    
    // Content (polymorphic)
    pub structured_data: Option<serde_json::Value>,
    pub blob_ref: Option<String>,
    pub ocr_extracted: Option<serde_json::Value>,
    
    // Relationships
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    
    // Provenance
    pub source: String,
    pub source_ref: Option<String>,
    pub correlation_id: Option<String>,
    
    // Lifecycle
    pub status: DocumentStatus,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentStatus {
    Pending,
    Verified,
    Rejected,
    Expired,
    Superseded,
}
```

---

## DSL Verbs

### document.*

```yaml
document:
  create:
    description: "Create a document record"
    behavior: crud
    args:
      - name: document-type
        type: string
        required: true
      - name: content-type
        type: string
        required: true
      - name: subject-entity-id
        type: uuid
        required: false
      - name: structured-data
        type: json
        required: false
      - name: blob-ref
        type: string
        required: false
        
  solicit:
    description: "Request document from external system (async)"
    behavior: plugin
    metadata:
      creates_pending_task: true
    args:
      - name: subject-entity-id
        type: uuid
        required: true
      - name: document-type
        type: string
        required: true
      - name: callback-url
        type: string
        required: false
        
  verify:
    description: "Mark document as verified"
    behavior: crud
    args:
      - name: document-id
        type: uuid
        required: true
      - name: verified-by
        type: string
        required: true
        
  find:
    description: "Find documents for entity"
    behavior: crud
    args:
      - name: subject-entity-id
        type: uuid
        required: true
      - name: document-type
        type: string
        required: false
      - name: status
        type: string
        required: false
```

### workflow.*

```yaml
workflow:
  complete-task:
    description: "Signal task completion (called by external systems or webhooks)"
    behavior: plugin
    args:
      - name: correlation-id
        type: string
        required: true
      - name: status
        type: string
        required: true
        valid_values: [completed, failed, expired]
      - name: cargo-type
        type: string
        required: false
      - name: cargo-ref
        type: string
        required: false
      - name: error
        type: string
        required: false
```

---

## API Endpoints

### Webhook for external systems

```
POST /api/workflow/task-complete
Content-Type: application/json

{
  "correlation_id": "task-abc-123",
  "status": "completed",
  "cargo_type": "document",
  "cargo_ref": "document://ob-poc/uuid-xyz",
  "idempotency_key": "ext-system-ref-456"
}

Response: 202 Accepted
```

### Query pending tasks

```
GET /api/workflow/{instance_id}/pending-tasks

Response:
{
  "tasks": [
    {
      "correlation_id": "...",
      "verb": "document.solicit",
      "status": "pending",
      "created_at": "...",
      "expires_at": "..."
    }
  ]
}
```

---

## Listener Implementation

### Option A: PostgreSQL Queue (simple, good for POC)

```rust
async fn task_result_listener(pool: PgPool, engine: Arc<WorkflowEngine>) {
    loop {
        // Atomic pop with FOR UPDATE SKIP LOCKED
        let result = sqlx::query_as!(TaskResult, r#"
            UPDATE task_result_queue 
            SET processed_at = now()
            WHERE id = (
                SELECT id FROM task_result_queue 
                WHERE processed_at IS NULL 
                ORDER BY id 
                LIMIT 1 
                FOR UPDATE SKIP LOCKED
            )
            RETURNING 
                correlation_id,
                verb,
                status as "status: TaskStatus",
                cargo_type,
                cargo_ref,
                error,
                queued_at as timestamp
        "#)
        .fetch_optional(&pool)
        .await;
        
        match result {
            Ok(Some(task_result)) => {
                if let Err(e) = handle_task_result(&engine, task_result).await {
                    tracing::error!(?e, "Failed to handle task result");
                }
            }
            Ok(None) => {
                // Queue empty, wait for notification or poll
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                tracing::error!(?e, "Failed to pop from queue");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_task_result(engine: &WorkflowEngine, result: TaskResult) -> Result<()> {
    // 1. Find the pending task
    let pending = engine.repo
        .find_pending_task_by_correlation(&result.correlation_id)
        .await?
        .ok_or_else(|| anyhow!("Unknown correlation_id"))?;
    
    // 2. Update pending task status
    engine.repo
        .complete_pending_task(
            pending.task_id,
            &result.status,
            result.cargo_ref.as_deref(),
            result.error.as_deref(),
        )
        .await?;
    
    // 3. Store cargo ref in workflow context if needed
    if let Some(ref cargo_ref) = result.cargo_ref {
        engine.repo
            .store_context_value(
                pending.instance_id,
                &format!("task_result:{}", pending.blocker_key.unwrap_or_default()),
                serde_json::json!({ "cargo_ref": cargo_ref }),
            )
            .await?;
    }
    
    // 4. Try to advance workflow
    engine.try_advance(pending.instance_id).await?;
    
    Ok(())
}
```

### Option B: PostgreSQL LISTEN/NOTIFY (avoid polling)

```rust
async fn task_result_listener_notify(pool: PgPool, engine: Arc<WorkflowEngine>) {
    let mut listener = PgListener::connect_with(&pool).await.unwrap();
    listener.listen("task_result_ready").await.unwrap();
    
    loop {
        // Wait for notification (no busy polling)
        let notification = listener.recv().await;
        
        match notification {
            Ok(_) => {
                // Notification received, drain the queue
                while let Some(result) = pop_task_result(&pool).await {
                    if let Err(e) = handle_task_result(&engine, result).await {
                        tracing::error!(?e, "Failed to handle task result");
                    }
                }
            }
            Err(e) => {
                tracing::error!(?e, "Listener error, reconnecting...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

// Trigger on insert to notify listener
// CREATE FUNCTION notify_task_result() RETURNS TRIGGER AS $$
// BEGIN
//     PERFORM pg_notify('task_result_ready', NEW.correlation_id);
//     RETURN NEW;
// END;
// $$ LANGUAGE plpgsql;
```

### Option C: Kafka/Redis (production scale)

For high-volume production, replace PG queue with:
- Kafka topic: `workflow.task-results`
- Redis Stream: `task_result_stream`

Same consumer logic, different transport.

---

## Workflow Guard Integration

Guards can now query documents:

```yaml
# In workflow YAML
states:
  awaiting_passport:
    requirements:
      - type: document_exists
        document_type: passport
        status: verified
        subject: $entity_id
    
    on_fail:
      blocker:
        type: MissingDocument
        resolution: "(document.solicit :subject-entity-id $entity_id :document-type passport)"
```

```rust
// Guard evaluator
impl RequirementEvaluator {
    async fn check_document_exists(
        &self,
        entity_id: Uuid,
        doc_type: &str,
        required_status: &str,
    ) -> Result<bool> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM "ob-poc".documents
                WHERE subject_entity_id = $1
                  AND document_type = $2
                  AND status = $3
                  AND (valid_to IS NULL OR valid_to > CURRENT_DATE)
            ) as "exists!"
            "#,
            entity_id,
            doc_type,
            required_status
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(exists)
    }
}
```

---

## Implementation Plan

### Phase 1: Schema & Types
- [ ] Add `task_result_queue` table
- [ ] Add `workflow_pending_tasks` table  
- [ ] Add `documents` table
- [ ] Define Rust types: `TaskResult`, `PendingTask`, `Document`

### Phase 2: Outbound (emit tasks)
- [ ] Extend `Blocker` with `to_pending_task()` method
- [ ] Create `document.solicit` verb (plugin handler)
- [ ] Record pending tasks when resolution DSL executes
- [ ] Pass correlation_id to external systems

### Phase 3: Inbound (receive results)
- [ ] Create webhook endpoint: `POST /api/workflow/task-complete`
- [ ] Create `workflow.complete-task` DSL verb
- [ ] Implement queue listener (PG or Kafka)
- [ ] Wire listener to workflow engine `try_advance()`

### Phase 4: Document entity
- [ ] Create `document.*` DSL verbs
- [ ] Integrate with blob storage (S3/local)
- [ ] Add OCR extraction hook (future: call external OCR service)
- [ ] Update workflow guards to query documents

### Phase 5: Testing & Integration
- [ ] Unit tests for TaskResult handling
- [ ] Integration test: full round-trip (emit → external mock → callback → advance)
- [ ] Update existing workflow YAMLs to use document requirements

---

## Open Questions

1. **Blob storage**: S3? Local filesystem for POC? Abstract behind trait?

2. **OCR integration**: Separate service? Or just store pre-extracted fields?

3. **Expiry/timeout handling**: Background job to expire pending tasks? Or lazy check on query?

4. **Retry semantics**: If external system fails, who retries? Workflow or external?

5. **Multi-tenancy**: Does correlation_id need tenant prefix?

---

## References

- `rust/crates/ob-workflow/` - Existing workflow engine
- `rust/config/workflows/` - YAML workflow definitions
- `rust/src/mcp/handlers/core.rs` - MCP tools (workflow_status, etc.)
- Camunda 8 external task pattern (inspiration, not direct integration)
