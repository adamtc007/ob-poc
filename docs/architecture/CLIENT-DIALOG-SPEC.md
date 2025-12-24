# Client Portal - Dialog Specification

## Principle

**Same data model. Different verbs. Read + Respond.**

The client agent reads from the same CBU/KYC/UBO state that internal agents write to.
Client operations create responses (documents, info) that internal workflows process.

---

## Data Model (Already Exists)

The WHY and guidance context is already modeled in `kyc.outstanding_requests`:

```sql
-- Existing table (abbreviated)
CREATE TABLE kyc.outstanding_requests (
    request_id UUID PRIMARY KEY,
    workstream_id UUID REFERENCES kyc.entity_workstreams,
    request_type VARCHAR(100),           -- 'DOCUMENT', 'INFORMATION', 'VERIFICATION'
    request_subtype VARCHAR(100),        -- 'SOURCE_OF_WEALTH', 'IDENTITY', 'TAX_RESIDENCE'
    
    -- THE WHY (already captured)
    reason_for_request TEXT,             -- "Verify source of funds for €50M investment"
    compliance_context TEXT,             -- "FCA SYSC 6.1.1 requires SOF documentation..."
    
    -- WHAT WE ACCEPT (already captured)
    acceptable_document_types TEXT[],    -- ['TAX_RETURN', 'AUDITED_ACCOUNTS', 'ADVISOR_LETTER']
    
    -- STATUS
    status VARCHAR(50),                  -- 'PENDING', 'PARTIALLY_FULFILLED', 'FULFILLED'
    due_date DATE,
    
    -- CLIENT-FACING
    client_visible BOOLEAN DEFAULT true,
    client_notes TEXT,                   -- "Accountant sending by end of month"
    
    -- TRACKING
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
```

**Key insight**: The internal agent already populates `reason_for_request` and `compliance_context` when creating requests. The client agent just surfaces it.

---

## Client Verb Palette

```yaml
# config/verbs/client.yaml

domain: client
description: "Client-facing operations for responding to onboarding requests"

verbs:
  # ─────────────────────────────────────────────────────────────
  # READ OPERATIONS
  # ─────────────────────────────────────────────────────────────
  
  - verb: get-status
    description: "Get current onboarding status for client's CBU(s)"
    args:
      - name: cbu-id
        type: ref
        required: false  # If client has only one CBU, optional
    returns:
      type: onboarding-status
      includes:
        - overall_progress_percent
        - current_stage
        - completed_stages
        - outstanding_count
        - blockers

  - verb: get-outstanding
    description: "Get list of outstanding requests with WHY context"
    args:
      - name: cbu-id
        type: ref
        required: false
      - name: include-completed
        type: boolean
        required: false
        default: false
    returns:
      type: outstanding-list
      includes:
        - request_id
        - request_type
        - reason_for_request      # WHY
        - compliance_context      # Regulatory basis
        - acceptable_documents    # What we accept
        - status
        - due_date
        - client_notes

  - verb: get-request-detail
    description: "Get full detail for a specific request including guidance"
    args:
      - name: request-id
        type: ref
        required: true
    returns:
      type: request-detail
      includes:
        - full_why_explanation
        - regulatory_references
        - acceptable_alternatives
        - common_questions
        - partial_progress

  - verb: get-entity-info
    description: "Get information about an entity (for verification/update)"
    args:
      - name: entity-id
        type: ref
        required: true
    returns:
      type: entity-summary
      includes:
        - name
        - type
        - roles
        - ubo_status
        - outstanding_for_entity

  # ─────────────────────────────────────────────────────────────
  # RESPOND OPERATIONS  
  # ─────────────────────────────────────────────────────────────

  - verb: submit-document
    description: "Submit a document in response to a request"
    args:
      - name: request-id
        type: ref
        required: true
      - name: document-type
        type: code
        ref-type: document-type
        required: true
      - name: file-reference
        type: string
        required: true        # Reference from upload service
      - name: notes
        type: string
        required: false
    effects:
      - creates: document-submission
      - updates: outstanding-request (status may change)
      - triggers: internal-review-workflow

  - verb: provide-info
    description: "Provide structured information in response to a request"
    args:
      - name: request-id
        type: ref
        required: true
      - name: info-type
        type: code
        ref-type: info-type
        required: true
      - name: collected-data
        type: object
        required: true        # Schema depends on info-type
      - name: notes
        type: string
        required: false
    effects:
      - creates: info-submission
      - updates: outstanding-request
      - may-trigger: follow-up-questions

  - verb: add-note
    description: "Add a note to a request (e.g., 'accountant sending next week')"
    args:
      - name: request-id
        type: ref
        required: true
      - name: note
        type: string
        required: true
      - name: expected-date
        type: date
        required: false       # "I expect to have this by..."
    effects:
      - updates: outstanding-request.client_notes
      - may-create: reminder

  - verb: request-clarification
    description: "Ask for clarification about a request"
    args:
      - name: request-id
        type: ref
        required: true
      - name: question
        type: string
        required: true
    effects:
      - creates: clarification-request
      - notifies: assigned-analyst

  # ─────────────────────────────────────────────────────────────
  # GUIDED COLLECTION
  # ─────────────────────────────────────────────────────────────

  - verb: start-collection
    description: "Begin guided collection for a structured info request"
    args:
      - name: request-id
        type: ref
        required: true
    effects:
      - sets: session.collection_mode = true
      - sets: session.active_collection = request-id
      - returns: first-question

  - verb: collection-response
    description: "Provide a response during guided collection"
    args:
      - name: field
        type: string
        required: true
      - name: value
        type: any
        required: true
    effects:
      - validates: value against field schema
      - stores: partial collection progress
      - returns: next-question OR validation-error OR complete

  - verb: collection-confirm
    description: "Confirm collected data before submission"
    args:
      - name: confirmed
        type: boolean
        required: true
    effects:
      - if-true: generates provide-info DSL, submits
      - if-false: returns to editing

  # ─────────────────────────────────────────────────────────────
  # ESCALATION
  # ─────────────────────────────────────────────────────────────

  - verb: escalate
    description: "Request human assistance"
    args:
      - name: reason
        type: string
        required: false
      - name: preferred-contact
        type: code
        required: false       # 'CALL', 'EMAIL', 'VIDEO'
    effects:
      - creates: escalation-request
      - attaches: full-conversation-context
      - notifies: relationship-manager
      - returns: escalation-confirmation
```

---

## Dialog State Schema

```rust
/// Client dialog state - persisted across sessions
pub struct ClientDialogState {
    /// Client identity
    pub client_id: Uuid,
    pub client_name: String,
    pub client_email: String,
    
    /// Scoped access
    pub accessible_cbus: Vec<Uuid>,  // CBUs this client can see
    pub active_cbu: Option<Uuid>,    // Currently focused CBU
    
    /// Collection mode state
    pub collection: Option<ActiveCollection>,
    
    /// Conversation context (for LLM)
    pub conversation_history: Vec<DialogTurn>,
    
    /// Client commitments (for reminders)
    pub pending_commitments: Vec<ClientCommitment>,
    
    /// Session metadata
    pub last_active: DateTime<Utc>,
    pub session_count: u32,
}

/// Active guided collection session
pub struct ActiveCollection {
    pub request_id: Uuid,
    pub info_type: String,
    pub schema: CollectionSchema,
    pub collected_fields: HashMap<String, ValidatedValue>,
    pub pending_fields: Vec<FieldDefinition>,
    pub current_field: Option<String>,
    pub validation_errors: Vec<ValidationError>,
}

/// A turn in the dialog
pub struct DialogTurn {
    pub timestamp: DateTime<Utc>,
    pub role: DialogRole,  // Client, Agent, System
    pub content: String,
    pub metadata: Option<TurnMetadata>,
}

pub struct TurnMetadata {
    pub intent_detected: Option<String>,
    pub entities_mentioned: Vec<EntityRef>,
    pub requests_referenced: Vec<Uuid>,
    pub action_taken: Option<String>,  // "Submitted document", "Started collection"
}

/// Client commitment for follow-up
pub struct ClientCommitment {
    pub request_id: Uuid,
    pub commitment: String,          // "Sending tax returns"
    pub expected_date: Option<Date>,
    pub reminder_date: Option<Date>,
    pub status: CommitmentStatus,    // Pending, Fulfilled, Overdue
}
```

---

## Context Injection (Leveraging Existing Agent Context)

The client agent uses the SAME semantic state derivation, just rendered differently:

```rust
impl AgentService {
    /// Derive client-facing context from existing CBU/KYC state
    async fn derive_client_context(&self, client: &ClientDialogState) -> ClientContext {
        let cbu_id = client.active_cbu.unwrap_or(client.accessible_cbus[0]);
        
        // REUSE existing semantic state derivation
        let semantic_state = derive_semantic_state(&self.pool, &self.registry, cbu_id).await?;
        
        // REUSE existing KYC case context
        let kyc_context = self.derive_kyc_case_context(kyc_case_id).await?;
        
        // Transform for client view
        ClientContext {
            // Progress (client-friendly)
            onboarding_progress: semantic_state.to_client_progress(),
            
            // Outstanding with WHY
            outstanding_requests: self.load_client_outstanding(cbu_id).await?,
            
            // What's been done
            completed_items: self.load_completed_items(cbu_id).await?,
            
            // Their commitments
            pending_commitments: client.pending_commitments.clone(),
            
            // Entity summary (who we're onboarding)
            entities: self.load_client_entity_summary(cbu_id).await?,
        }
    }
    
    /// Load outstanding requests with full WHY context
    async fn load_client_outstanding(&self, cbu_id: Uuid) -> Vec<ClientOutstandingRequest> {
        sqlx::query_as!(ClientOutstandingRequest, r#"
            SELECT 
                r.request_id,
                r.request_type,
                r.request_subtype,
                e.name as entity_name,
                
                -- THE WHY (already in database)
                r.reason_for_request,
                r.compliance_context,
                r.acceptable_document_types,
                
                -- Status
                r.status,
                r.due_date,
                r.client_notes,
                
                -- Partial progress
                (SELECT COUNT(*) FROM kyc.request_submissions 
                 WHERE request_id = r.request_id) as submissions_count
                
            FROM kyc.outstanding_requests r
            JOIN kyc.entity_workstreams w ON r.workstream_id = w.workstream_id
            JOIN "ob-poc".entities e ON w.entity_id = e.entity_id
            JOIN kyc.cases c ON w.case_id = c.case_id
            WHERE c.cbu_id = $1
              AND r.client_visible = true
              AND r.status != 'FULFILLED'
            ORDER BY r.due_date NULLS LAST, r.created_at
        "#, cbu_id)
        .fetch_all(&self.pool)
        .await?
    }
}
```

---

## Prompt Layer Differences

```rust
impl AgentService {
    fn build_client_system_prompt(&self, ctx: &ClientContext) -> String {
        format!(r#"
You are a client-facing onboarding assistant for {company_name}.

## Your Role
- Help the client understand what's needed and why
- Guide them through providing documents and information
- Be clear, patient, and professional
- Explain regulatory requirements in plain English
- Never make them feel stupid for asking questions

## You Are NOT
- A generic chatbot - you know their specific situation
- Bureaucratic - explain the WHY, not just the WHAT
- Inflexible - offer alternatives when possible
- A barrier - your job is to help them complete onboarding

## Current Context

### Onboarding Progress
{progress_summary}

### Outstanding Items
{outstanding_with_why}

### Recently Completed
{completed_items}

## Response Style
- Lead with what they need to know
- Explain WHY before asking for things
- Acknowledge frustration if expressed
- Offer to escalate to human if stuck
- Confirm understanding before proceeding

## Available Actions
- Show status and progress
- Explain any outstanding request in detail
- Accept document uploads
- Guide through structured information collection
- Add notes about expected delivery
- Escalate to relationship manager

## Constraints
- You can only see CBUs this client has access to
- You cannot modify their data directly (only submit responses)
- You cannot waive regulatory requirements
- You CAN explain alternatives and workarounds
"#,
            company_name = "BNY Mellon",
            progress_summary = ctx.format_progress(),
            outstanding_with_why = ctx.format_outstanding_with_why(),
            completed_items = ctx.format_completed(),
        )
    }
}
```

---

## Example Flow: Full Conversation

```
┌─────────────────────────────────────────────────────────────────┐
│ CLIENT PORTAL - Allianz Reinsurance AG                          │
├─────────────────────────────────────────────────────────────────┤

Client: "Hi, what do you still need from us?"

[Agent internally]:
  - derive_client_context() 
  - Finds 3 outstanding requests
  - Loads WHY from database

Agent: "Hello! Here's where we are with Allianz Reinsurance:

**Overall Progress: 87% complete**

Outstanding items:

1. **Source of Wealth - Pierre Dupont** (due Jan 15)
   We have your bank statements ✓
   Still need: Tax returns for 2021-2023
   
   WHY: Pierre is investing €50M. FCA regulations require us to 
   verify how he accumulated this wealth - not just the source of 
   this specific investment, but his overall financial history.
   
   ACCEPTS: Tax returns, audited accounts, or letter from tax advisor

2. **Identity Document - Thomas Müller** (due Jan 10)
   WHY: UK Money Laundering Regulations require ID verification 
   for all persons with significant control.
   
   ACCEPTS: Passport (preferred) or national ID card

3. **Maria Schmidt - Date of Birth** (quick one!)
   WHY: Required field for PSC register filing
   
That's everything! The tax returns are the main one."

Client: "Maria was born March 15, 1972. I have Thomas's passport here."

[Agent internally]:
  - Extracts DOB: "March 15, 1972" → 1972-03-15
  - Validates: age 52, reasonable ✓
  - Detects document upload intent

Agent: "Got it - I've recorded Maria Schmidt's DOB as 15 March 1972. ✓

For Thomas's passport, go ahead and drop the file here or click 
to upload."

[Client uploads: Thomas_Mueller_Passport.pdf]

[Agent internally]:
  - client.submit-document 
  - :request-id @thomas_muller_id_request
  - :document-type PASSPORT
  - :file-reference "upload://abc123"

Agent: "Received Thomas Müller's passport. ✓

I've submitted it for review - you'll see it marked complete 
within 24 hours once verified.

**Updated status:**
- Source of Wealth (Pierre): Still need tax returns
- Thomas Müller ID: Submitted, pending review ✓
- Maria Schmidt DOB: Complete ✓

Just the tax returns remaining! Any update on those?"

Client: "Still waiting on our accountant. Should have them next week."

[Agent internally]:
  - client.add-note
  - :request-id @pierre_sow_request  
  - :note "Waiting on accountant"
  - :expected-date 2025-01-03

Agent: "No problem - I've noted that you're expecting them next week 
from your accountant.

I'll check back on Friday January 3rd if we haven't received them 
by then. Does that work?"

Client: "Perfect"

Agent: "Great! You're 94% done now. I'll be here when you have 
those tax returns. Anything else I can help with?"

└─────────────────────────────────────────────────────────────────┘
```

---

## Test Scenarios

```rust
#[cfg(test)]
mod client_dialog_tests {
    
    #[tokio::test]
    async fn test_get_outstanding_includes_why() {
        // Setup: Create request with reason_for_request populated
        // Action: client.get-outstanding
        // Assert: Response includes WHY context
    }
    
    #[tokio::test]
    async fn test_partial_collection_persists() {
        // Setup: Start collection, provide 2 of 5 fields
        // Action: Session ends, new session starts
        // Assert: 2 fields still collected, continues from field 3
    }
    
    #[tokio::test]
    async fn test_document_submission_updates_request() {
        // Setup: Outstanding document request
        // Action: client.submit-document
        // Assert: Request status = PENDING_REVIEW, submission recorded
    }
    
    #[tokio::test]
    async fn test_client_note_creates_reminder() {
        // Setup: Outstanding request
        // Action: client.add-note with expected-date
        // Assert: Reminder scheduled for expected-date + 1
    }
    
    #[tokio::test]
    async fn test_escalation_transfers_context() {
        // Setup: Conversation with 10 turns
        // Action: client.escalate
        // Assert: Escalation includes full transcript + outstanding summary
    }
    
    #[tokio::test]
    async fn test_cbu_scoping_enforced() {
        // Setup: Client has access to CBU-A, not CBU-B
        // Action: Try to query CBU-B
        // Assert: Access denied / not visible
    }
    
    #[tokio::test]
    async fn test_inline_validation_during_collection() {
        // Setup: Collecting beneficial owner, DOB field
        // Action: Provide "1850-01-01"
        // Assert: Validation error, not persisted, helpful message
    }
}
```

---

## Implementation Priority

| Phase | Deliverable | Dependencies |
|-------|-------------|--------------|
| 1 | `client.get-outstanding` with WHY | Existing DB schema |
| 2 | `client.submit-document` | Upload service integration |
| 3 | `client.provide-info` (simple) | Existing request types |
| 4 | Dialog state persistence | New table |
| 5 | Guided collection mode | Schema per info-type |
| 6 | Reminders & commitments | Scheduler integration |
| 7 | Escalation with context | RM notification |

Phase 1-3 are testable with existing infrastructure.
