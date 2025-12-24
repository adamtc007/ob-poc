# Agent Integration for Async Requests

## Overview

This document covers the integration points needed for the agent to:
1. **See** outstanding requests embedded in domain state (not separate list)
2. **Know** about fire-and-forget verb patterns
3. **Use** request management verbs appropriately

**Domain Coherence Principle**: Requests are nodes in the domain state graph.
When agent views a KYC case, workstreams show their `awaiting` requests inline.
There is ONE state model, not "case" + "requests".

---

## 1. Session State Injection (Domain-Embedded)

### Principle

Requests appear as child nodes of the domain objects they belong to:

```
workstreams:
  - entity: "Pierre Martin"
    status: BLOCKED
    awaiting:                    ← INLINE, not separate list
      - subtype: SOURCE_OF_WEALTH
        overdue: true
        days_overdue: 10
```

NOT as a parallel structure the agent must cross-reference.

### Session State Template (Domain-Embedded Requests)

**File:** `config/agent/session_state_template.yaml`

```yaml
# Template for what agent sees in session context
# Key: requests are EMBEDDED in workstreams, not separate

session_state:
  active_cbu:
    cbu_id: "{{cbu_id}}"
    name: "{{cbu_name}}"
    client_type: "{{client_type}}"
    stage: "{{stage}}"
    
  kyc_case:
    case_id: "{{case_id}}"
    status: "{{case_status}}"
    risk_rating: "{{case_risk_rating}}"
    
    # Workstreams with EMBEDDED awaiting requests
    workstreams:
      - workstream_id: "{{ws_id}}"
        entity: 
          name: "{{entity_name}}"
          role: "{{role}}"
        type: "{{workstream_type}}"
        status: "{{ws_status}}"
        
        # Requests are CHILD NODES, not separate list
        awaiting:
          - request_id: "{{req_id}}"
            type: "{{request_type}}"
            subtype: "{{subtype}}"
            from: "{{from}}"
            due_date: "{{due_date}}"
            days_overdue: "{{days_overdue}}"
            overdue: "{{is_overdue}}"
            reminder_count: "{{reminder_count}}"
            actions: ["remind", "escalate", "waive"]
            
    summary:
      total_workstreams: "{{total}}"
      complete: "{{complete_count}}"
      blocked: "{{blocked_count}}"
      total_awaiting: "{{awaiting_count}}"
      overdue: "{{overdue_count}}"
      
    # Derived: What needs immediate attention
    attention:
      - workstream: "{{ws_id}}"
        entity: "{{entity_name}}"
        issue: "{{issue_description}}"
        priority: "{{priority}}"
        actions: ["{{action1}}", "{{action2}}"]
```

**Note:** There is no separate `outstanding_requests` section. Requests live inside their workstreams.

---

### Session State Builder (Simplified)

The `SessionStateBuilder` calls `KycCaseStateOp` which returns workstreams with 
embedded `awaiting` arrays. No separate request loading needed.

```rust
impl SessionStateBuilder {
    pub async fn build(&self, pool: &PgPool) -> Result<Value> {
        let mut state = json!({});
        
        if let Some(cbu_id) = self.cbu_id {
            state["active_cbu"] = self.build_cbu_state(cbu_id, pool).await?;
        }
        
        if let Some(case_id) = self.case_id {
            // This returns workstreams with awaiting embedded
            state["kyc_case"] = kyc_case_state_op(case_id, pool).await?;
        }
        
        Ok(state)
    }
}
```

The `kyc_case` already contains:
- `workstreams[].awaiting[]` - requests as child nodes
- `attention[]` - derived issues needing action  
- `summary.overdue` - count of overdue requests

No separate "outstanding_requests" loading. Domain coherence maintained.

---

## 2. Agent Tool Knowledge (RAG/Context)

The agent needs to KNOW about the fire-and-forget pattern and these verbs.

**File:** `config/agent/tool_knowledge/async_requests.yaml`

```yaml
# ═══════════════════════════════════════════════════════════════════════════
# Async Request Pattern - Agent Knowledge
# ═══════════════════════════════════════════════════════════════════════════

async_request_pattern:
  summary: |
    Some operations don't complete immediately. Document requests, external
    verifications, and approvals can take days or weeks. These use the
    "fire and forget" pattern:
    
    1. FIRE: Call the request verb → returns immediately with request_id
    2. FORGET: Move on to other work - don't wait or poll
    3. OBSERVE: Check session state later - outstanding requests are visible
    4. ACT: When you see overdue/escalated requests, decide what to do

  key_principle: |
    The verb execution completes immediately (SUCCESS/FAILURE).
    The REQUEST has its own lifecycle (PENDING → FULFILLED/ESCALATED/WAIVED).
    These are different things.
    
    You don't track requests. The system does. When you look at state, truth is there.

  when_to_use_request_verbs: |
    Use request verbs when you need something from outside the system:
    - Documents from client: (document.request ...)
    - Information from entity: (information.request ...)
    - External verification: (verification.request ...)
    - Internal approval: (approval.request ...)
    
    After calling, move on. The request will appear in outstanding_requests.

# ═══════════════════════════════════════════════════════════════════════════
# Document Request Verbs
# ═══════════════════════════════════════════════════════════════════════════

document_request:
  verb: "document.request"
  pattern: "fire-and-forget"
  
  description: |
    Request a document from client/entity. Creates outstanding request,
    blocks workstream (by default), returns immediately.
    
  when_to_use:
    - KYC workstream needs supporting documentation
    - Missing required document identified
    - Enhanced due diligence requires additional evidence
    
  args:
    workstream-id: "UUID of workstream (usually)"
    entity-id: "UUID of entity (alternative to workstream)"
    type: "Document type code (SOURCE_OF_WEALTH, ID_DOCUMENT, etc.)"
    from: "Who should provide it (default: 'client')"
    due-in-days: "Days until due (default: 7)"
    notes: "Additional context for the request"
    
  example: |
    ; Request source of wealth for UBO
    (document.request 
      workstream-id:@ws-003 
      type:SOURCE_OF_WEALTH 
      from:"client"
      due-in-days:14
      notes:"Required for enhanced due diligence - complex wealth structure")
    
    ; Returns immediately:
    ; {request_id: "req-456", status: "PENDING", due_date: "2026-01-07"}
    
    ; Workstream now shows: status: BLOCKED, blocker: "Awaiting SOURCE_OF_WEALTH"
    
  what_happens_next: |
    1. Request appears in session state under outstanding_requests
    2. When client uploads document, system auto-matches and fulfills
    3. Workstream auto-unblocks when all blockers resolved
    4. If overdue, you'll see it flagged - decide to remind/escalate/waive

document_upload:
  verb: "document.upload"
  pattern: "auto-fulfillment"
  
  description: |
    Upload a document. System automatically matches to pending request
    (if any) and unblocks workstream.
    
  when_to_use:
    - Client has provided a document
    - Document received via email/portal needs recording
    
  args:
    workstream-id: "UUID of workstream"
    type: "Document type code"
    file: "The file to upload"
    notes: "Optional notes"
    
  example: |
    (document.upload 
      workstream-id:@ws-003 
      type:SOURCE_OF_WEALTH 
      file:@uploaded-file)
    
    ; Returns:
    ; {
    ;   document_id: "doc-789",
    ;   fulfilled_request_id: "req-456",  ← Auto-matched!
    ;   workstream_unblocked: true        ← Auto-unblocked!
    ; }

document_waive:
  verb: "document.waive"
  pattern: "override"
  
  description: |
    Waive a document requirement. Requires senior approval and reason.
    Use when document cannot be obtained but risk is acceptable.
    
  when_to_use:
    - Client cannot provide document
    - Risk assessment allows proceeding without it
    - Alternative evidence available
    
  args:
    workstream-id: "UUID of workstream"
    type: "Document type to waive"
    reason: "Justification for waiver"
    approved-by: "Senior user approving"
    
  example: |
    (document.waive 
      workstream-id:@ws-003 
      type:SOURCE_OF_WEALTH 
      reason:"Low-value relationship (<€10k), simplified DD applies"
      approved-by:@senior-analyst)

# ═══════════════════════════════════════════════════════════════════════════
# Request Management Verbs
# ═══════════════════════════════════════════════════════════════════════════

request_remind:
  verb: "request.remind"
  
  description: "Send reminder for pending request"
  
  when_to_use:
    - Request approaching due date
    - Request overdue but not yet escalated
    - Client may have forgotten
    
  example: |
    (request.remind 
      request-id:@req-456 
      message:"Gentle reminder - source of wealth documentation still required")

request_escalate:
  verb: "request.escalate"
  
  description: "Escalate overdue request to manager/senior"
  
  when_to_use:
    - Request significantly overdue
    - Multiple reminders sent with no response
    - Need management decision on how to proceed
    
  example: |
    (request.escalate 
      request-id:@req-456 
      reason:"Client unresponsive after 3 reminders over 14 days")

request_extend:
  verb: "request.extend"
  
  description: "Extend request due date"
  
  when_to_use:
    - Client has valid reason for delay
    - Complexity requires more time
    - Partial information received, more coming
    
  example: |
    (request.extend 
      request-id:@req-456 
      days:7 
      reason:"Client traveling, confirmed will provide next week")

request_cancel:
  verb: "request.cancel"
  
  description: "Cancel a pending request"
  
  when_to_use:
    - Request no longer needed
    - Duplicate request created
    - Requirements changed
    
  example: |
    (request.cancel 
      request-id:@req-456 
      reason:"Alternative documentation accepted instead")

# ═══════════════════════════════════════════════════════════════════════════
# Decision Guide: Handling Overdue Requests
# ═══════════════════════════════════════════════════════════════════════════

handling_overdue_requests:
  context: |
    When you see overdue requests in session state, you need to decide
    what to do. Here's the decision framework:
    
  decision_tree: |
    REQUEST OVERDUE
    │
    ├─► How many days overdue?
    │   ├─► 1-3 days: Send reminder (request.remind)
    │   ├─► 4-7 days: Send stronger reminder, note urgency
    │   └─► 7+ days: Consider escalation or waiver
    │
    ├─► How many reminders sent?
    │   ├─► 0-1: Send another reminder
    │   ├─► 2: Send final reminder with deadline
    │   └─► 3+: Escalate or waive
    │
    ├─► Is document critical?
    │   ├─► Yes (e.g., ID for UBO): Must obtain or reject case
    │   └─► No (e.g., nice-to-have): Consider waiver
    │
    └─► Can we proceed without it?
        ├─► Yes: Waive with reason and approval
        └─► No: Escalate for management decision
        
  example_scenario: |
    You see:
    {
      "request_id": "req-456",
      "subtype": "SOURCE_OF_WEALTH",
      "days_overdue": 10,
      "reminder_count": 2,
      "for_entity": "Pierre Martin (UBO)"
    }
    
    Analysis:
    - 10 days overdue is significant
    - 2 reminders already sent
    - SOURCE_OF_WEALTH for UBO is important for high-risk cases
    
    Options:
    1. If relationship is low-value: 
       (document.waive ... reason:"Low-value, simplified DD" approved-by:@senior)
    
    2. If client is responsive but slow:
       (request.extend ... days:7 reason:"Client confirmed will provide")
    
    3. If client unresponsive:
       (request.escalate ... reason:"No response after 2 reminders, 10 days overdue")

# ═══════════════════════════════════════════════════════════════════════════
# Session State: What You'll See (Domain-Embedded)
# ═══════════════════════════════════════════════════════════════════════════

session_state_example:
  description: |
    Requests appear INSIDE workstreams as child nodes, not as separate list.
    
  example: |
    kyc_case:
      status: "IN_PROGRESS"
      
      workstreams:
        - workstream_id: "ws-003"
          entity: 
            name: "Pierre Martin"
            role: "UBO"
          type: "FULL_KYC"
          status: "BLOCKED"
          
          awaiting:                          # ← INLINE
            - request_id: "req-456"
              subtype: "SOURCE_OF_WEALTH"
              from: "client"
              due_date: "2025-12-21"
              days_overdue: 10
              overdue: true
              reminder_count: 2
              actions: ["remind", "escalate", "waive"]
              
        - workstream_id: "ws-004"
          entity:
            name: "Jean Dupont"
            role: "DIRECTOR"
          type: "SCREEN_AND_ID"
          status: "IN_PROGRESS"
          
          awaiting:                          # ← INLINE
            - request_id: "req-457"
              subtype: "ID_DOCUMENT"
              due_date: "2025-12-27"
              days_overdue: 0
              overdue: false
              
      attention:
        - workstream: "ws-003"
          entity: "Pierre Martin"
          issue: "SOURCE_OF_WEALTH overdue 10 days"
          priority: "HIGH"
          actions: ["remind", "escalate", "waive"]
          
      summary:
        total_workstreams: 4
        complete: 2
        blocked: 1
        overdue: 1
        
  note: |
    There is no separate "outstanding_requests" section.
    Agent sees ONE state model. Requests are where they belong - inside their workstreams.
```

---

## 3. Verb Registry Update

The agent needs these verbs in its available verb list.

**File:** `config/verbs/_registry.yaml` (update)

```yaml
# Add to registry
verb_domains:
  # ... existing domains ...
  
  request:
    description: "Outstanding request management"
    verbs:
      - create
      - list
      - overdue
      - fulfill
      - cancel
      - extend
      - remind
      - escalate
      - waive
    file: request.yaml
    
  document:
    description: "Document operations"
    verbs:
      - request
      - upload
      - waive
      - list
    file: document.yaml
    
  information:
    description: "Information request operations"
    verbs:
      - request
      - provide
    file: information.yaml
    
  verification:
    description: "External verification operations"
    verbs:
      - request
      - record
    file: verification.yaml
```

---

## 4. Agent Prompt Injection

When agent starts session with a case, inject the pattern knowledge.

**File:** `config/agent/prompts/kyc_session.yaml`

```yaml
# Injected when agent is working on KYC case

kyc_session_context: |
  ## Outstanding Requests
  
  Some operations create "outstanding requests" that take time to fulfill
  (documents from clients, external verifications, approvals).
  
  These requests are visible in your session state under `outstanding_requests`.
  
  ### Pattern
  1. Call request verb (e.g., `document.request`) → returns immediately
  2. Request appears in state as PENDING
  3. When fulfilled (upload, response), auto-matches and unblocks
  4. If overdue, decide: remind, escalate, extend, or waive
  
  ### Your Current Outstanding Requests
  {{outstanding_requests_summary}}
  
  ### Attention Needed
  {{attention_needed}}
  
  ### Suggested Actions
  {{suggested_actions}}
```

---

## 5. Integration Checklist

### Session State Integration

- [ ] Update `SessionStateBuilder` to include `outstanding_requests`
- [ ] Add `attention_needed` derivation logic
- [ ] Add `suggested_actions` derivation logic
- [ ] Inject into agent context on session start
- [ ] Refresh on state change (document upload, etc.)

### Tool Knowledge Integration

- [ ] Create `config/agent/tool_knowledge/async_requests.yaml`
- [ ] Add to agent RAG/context loading
- [ ] Include decision guide for overdue handling
- [ ] Add examples for each verb

### Verb Registry Integration

- [ ] Add `request` domain to registry
- [ ] Add `document` domain to registry
- [ ] Add `information` domain to registry
- [ ] Add `verification` domain to registry
- [ ] Verify verbs appear in agent's available verb list

### Prompt Integration

- [ ] Create KYC session prompt template
- [ ] Include outstanding requests summary
- [ ] Include attention items
- [ ] Include suggested actions
- [ ] Inject on session start

### Testing

- [ ] Test: Agent sees outstanding requests in state
- [ ] Test: Agent uses `document.request` correctly
- [ ] Test: Agent recognizes overdue requests
- [ ] Test: Agent suggests appropriate actions
- [ ] Test: Upload auto-fulfills and agent sees unblock

---

## Summary

| Integration Point | What | Why |
|-------------------|------|-----|
| Session State | Add `outstanding_requests` to context | Agent must SEE requests |
| Tool Knowledge | Add async request pattern docs | Agent must KNOW the pattern |
| Verb Registry | Register request/document verbs | Agent must have VERBS available |
| Prompt Injection | Include summary + suggestions | Agent must be GUIDED to act |

Without all four, the agent is blind to async operations.
