# TODO: KYC/UBO Convergence Implementation

## ⛔ MANDATORY FIRST STEP

**Before writing ANY code, read:**
- `/docs/KYC-UBO-SOLUTION-OVERVIEW.md` - The full solution architecture
- `/EGUI-RULES.md` - If any UI work is needed

---

## Objective

Implement the observation-based KYC convergence model where:
1. Client allegations build an ownership graph
2. Proofs are linked to specific edges
3. Observations extracted from proofs are compared to allegations
4. Graph converges when all edges are proven
5. Assertions gate progression to evaluation and decision
6. Full audit trail enables regulator traceability

---

## Current State Assessment

| Component | Status | Location |
|-----------|--------|----------|
| Entity model (nodes) | ✅ Exists | `entities`, `proper_persons`, `limited_companies`, etc. |
| CBU model | ✅ Exists | `cbus`, `cbu_entity_roles` |
| UBO analysis verbs | ⚠️ Partial | `rust/src/dsl_v2/custom_ops/ubo_analysis.rs` |
| Threshold verbs | ⚠️ Partial | `rust/src/dsl_v2/custom_ops/threshold.rs` |
| RFI verbs | ⚠️ Partial | `rust/src/dsl_v2/custom_ops/rfi.rs` |
| Edge model (graph) | ❓ Verify | May need `ubo_edges` table |
| Proof model | ❓ Verify | May need refinement |
| Observation model | ❌ Missing | `ubo_observations` table needed |
| Assertion verbs | ❌ Missing | `ubo.assert` verb |
| Convergence calculation | ❌ Missing | `ubo.status` verb enhancement |
| Decision tracking | ❓ Verify | `kyc_decisions` table |
| Assertion audit log | ❌ Missing | `ubo_assertion_log` table |

---

## Phase 1: Data Model Verification & Gaps

### 1.1 Verify Existing Tables

**Task:** Audit existing schema against required model

```bash
# Check what UBO-related tables exist
psql -c "\dt *ubo*" data_designer
psql -c "\dt *proof*" data_designer  
psql -c "\dt *kyc*" data_designer
```

**Files to check:**
- [ ] `sql/schema/` - Existing migrations
- [ ] `rust/src/dsl_v2/custom_ops/ubo_analysis.rs` - What tables it uses

### 1.2 Create Missing Tables

**File:** `sql/migrations/YYYYMMDD_ubo_convergence_tables.sql`

```sql
-- Only create if not exists - verify first!

-- UBO Edges (ownership graph)
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_edges (
    edge_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Graph edge
    from_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    to_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    edge_type VARCHAR(20) NOT NULL,  -- 'ownership', 'control', 'trust_role'
    
    -- Ownership specifics
    percentage DECIMAL(5,2),
    
    -- Control specifics
    control_role VARCHAR(50),
    
    -- Trust role specifics
    trust_role VARCHAR(50),
    interest_type VARCHAR(20),
    
    -- Allegation
    alleged_percentage DECIMAL(5,2),
    alleged_at TIMESTAMPTZ,
    alleged_by UUID,
    allegation_source VARCHAR(100),
    
    -- Proof linkage
    proof_id UUID REFERENCES "ob-poc".proofs(proof_id),
    proven_percentage DECIMAL(5,2),
    proven_at TIMESTAMPTZ,
    
    -- State
    status VARCHAR(20) NOT NULL DEFAULT 'alleged',
    
    -- Discrepancy
    discrepancy_notes TEXT,
    resolved_at TIMESTAMPTZ,
    resolved_by UUID,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(cbu_id, from_entity_id, to_entity_id, edge_type)
);

-- Observations (what proofs say)
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_observations (
    observation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    proof_id UUID NOT NULL REFERENCES "ob-poc".proofs(proof_id),
    edge_id UUID REFERENCES "ob-poc".ubo_edges(edge_id),
    
    subject_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    attribute_code VARCHAR(50) NOT NULL,
    observed_value JSONB NOT NULL,
    
    extracted_from JSONB,
    extraction_method VARCHAR(50),
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID
);

-- Assertion audit log
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_assertion_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    dsl_execution_id UUID,
    
    assertion_type VARCHAR(50) NOT NULL,
    expected_value BOOLEAN NOT NULL,
    actual_value BOOLEAN NOT NULL,
    passed BOOLEAN NOT NULL,
    
    failure_details JSONB,
    
    asserted_at TIMESTAMPTZ DEFAULT NOW()
);

-- KYC decisions (if not exists)
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    
    status VARCHAR(20) NOT NULL,
    conditions TEXT,
    
    review_interval INTERVAL,
    next_review_date DATE,
    
    evaluation_snapshot JSONB,
    
    decided_by UUID NOT NULL,
    decided_at TIMESTAMPTZ DEFAULT NOW(),
    decision_rationale TEXT,
    
    dsl_execution_id UUID,
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Add dirty flag to proofs if not exists
ALTER TABLE "ob-poc".proofs 
ADD COLUMN IF NOT EXISTS marked_dirty_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS dirty_reason VARCHAR(100);

-- Views
CREATE OR REPLACE VIEW "ob-poc".ubo_convergence_status AS
SELECT 
    cbu_id,
    COUNT(*) AS total_edges,
    COUNT(*) FILTER (WHERE status = 'proven') AS proven_edges,
    COUNT(*) FILTER (WHERE status = 'alleged') AS alleged_edges,
    COUNT(*) FILTER (WHERE status = 'pending') AS pending_edges,
    COUNT(*) FILTER (WHERE status = 'disputed') AS disputed_edges,
    COUNT(*) FILTER (WHERE status = 'proven') = COUNT(*) AS is_converged
FROM "ob-poc".ubo_edges
GROUP BY cbu_id;
```

### 1.3 Tasks

- [ ] Audit existing UBO-related tables
- [ ] Document what exists vs. what's needed
- [ ] Create migration for missing tables
- [ ] Add indexes for performance
- [ ] Create convergence status view

---

## Phase 2: Core Verbs - Graph Building

### 2.1 `ubo.allege` Verb

**Purpose:** Add an edge to the alleged ownership graph

**DSL:**
```clojure
(ubo.allege :cbu @cbu 
            :from ("fund" "Allianz Dynamic")
            :to ("manco" "Allianz GI GmbH")
            :type "ownership"
            :percentage 100
            :source "client_disclosure")
```

**Implementation:**

**File:** `rust/src/dsl_v2/custom_ops/ubo_graph_ops.rs` (new or extend existing)

```rust
pub struct UboAllegeOp;

#[async_trait]
impl CustomOperation for UboAllegeOp {
    fn domain(&self) -> &'static str { "ubo" }
    fn verb(&self) -> &'static str { "allege" }
    
    async fn execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool) -> Result<ExecutionResult> {
        let cbu_id = get_cbu_id(verb_call, ctx)?;
        let from_entity = resolve_entity_ref(verb_call, "from", pool).await?;
        let to_entity = resolve_entity_ref(verb_call, "to", pool).await?;
        let edge_type = get_string_arg(verb_call, "type")?;
        let percentage = get_decimal_arg(verb_call, "percentage")?;
        let source = get_optional_string_arg(verb_call, "source")?;
        
        // Insert edge with status = 'alleged'
        let edge_id = sqlx::query_scalar(r#"
            INSERT INTO "ob-poc".ubo_edges 
            (cbu_id, from_entity_id, to_entity_id, edge_type, alleged_percentage, 
             allegation_source, alleged_at, alleged_by, status)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, 'alleged')
            ON CONFLICT (cbu_id, from_entity_id, to_entity_id, edge_type) 
            DO UPDATE SET 
                alleged_percentage = EXCLUDED.alleged_percentage,
                alleged_at = NOW(),
                status = CASE WHEN ubo_edges.status = 'proven' THEN 'alleged' ELSE ubo_edges.status END
            RETURNING edge_id
        "#)
        .bind(cbu_id)
        .bind(from_entity)
        .bind(to_entity)
        .bind(&edge_type)
        .bind(percentage)
        .bind(&source)
        .bind(ctx.audit_user)
        .fetch_one(pool)
        .await?;
        
        Ok(ExecutionResult::Created { id: edge_id, entity_type: "ubo_edge".into() })
    }
}
```

### 2.2 `ubo.link-proof` Verb

**Purpose:** Attach a proof document to an edge

**DSL:**
```clojure
(ubo.link-proof :cbu @cbu 
                :edge [("fund" "Allianz Dynamic") ("manco" "Allianz GI")]
                :proof @shareholder_register
                :proof-type "shareholder_register")
```

**Implementation:** Similar pattern, updates `proof_id` on edge, sets status to 'pending'

### 2.3 Tasks

- [ ] Create `rust/src/dsl_v2/custom_ops/ubo_graph_ops.rs`
- [ ] Implement `UboAllegeOp`
- [ ] Implement `UboLinkProofOp`
- [ ] Implement `UboUpdateAllegationOp`
- [ ] Implement `UboRemoveAllegationOp`
- [ ] Add YAML verb definitions in `config/verbs/ubo.yaml`
- [ ] Register ops in `custom_ops/mod.rs`
- [ ] Unit tests for each verb

---

## Phase 3: Verification & Convergence

### 3.1 `ubo.verify` Verb

**Purpose:** Compare allegations to proof observations, update edge statuses

**DSL:**
```clojure
(ubo.verify :cbu @cbu :as @result)
```

**Logic:**
1. For each edge with status 'pending' (has proof linked):
2. Find observations from that proof
3. Compare `alleged_percentage` to `observed_percentage`
4. If match: status → 'proven', set `proven_percentage`
5. If mismatch: status → 'disputed', record discrepancy
6. Return verification result

**Implementation:**

```rust
pub struct UboVerifyOp;

#[async_trait]
impl CustomOperation for UboVerifyOp {
    async fn execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool) -> Result<ExecutionResult> {
        let cbu_id = get_cbu_id(verb_call, ctx)?;
        
        // Get all pending edges with proofs
        let edges = sqlx::query_as::<_, UboEdge>(r#"
            SELECT e.*, o.observed_value
            FROM "ob-poc".ubo_edges e
            LEFT JOIN "ob-poc".ubo_observations o ON o.edge_id = e.edge_id
            WHERE e.cbu_id = $1 AND e.status = 'pending'
        "#)
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;
        
        let mut proven = 0;
        let mut disputed = 0;
        
        for edge in edges {
            if let Some(observed) = edge.observation {
                let alleged = edge.alleged_percentage;
                let observed_pct = observed.get("percentage").and_then(|v| v.as_f64());
                
                if Some(alleged as f64) == observed_pct {
                    // Match - mark proven
                    update_edge_status(pool, edge.edge_id, "proven", alleged).await?;
                    proven += 1;
                } else {
                    // Mismatch - mark disputed
                    update_edge_status_disputed(pool, edge.edge_id, alleged, observed_pct).await?;
                    disputed += 1;
                }
            }
        }
        
        Ok(ExecutionResult::UboVerification { 
            proven, 
            disputed, 
            cbu_id 
        })
    }
}
```

### 3.2 `ubo.status` Verb

**Purpose:** Return full convergence state

**DSL:**
```clojure
(ubo.status :cbu @cbu :as @status)
```

**Returns:**
```json
{
  "converged": false,
  "total_edges": 5,
  "proven_edges": 3,
  "alleged_edges": 1,
  "pending_edges": 0,
  "disputed_edges": 1,
  "missing_proofs": [
    {"edge_id": "...", "from": "Entity A", "to": "Entity B", "proof_type_needed": "shareholder_register"}
  ],
  "expired_proofs": [],
  "discrepancies": [
    {"edge_id": "...", "alleged": 100, "observed": 70}
  ],
  "ready_for_evaluation": false,
  "blockers": ["disputed_edges", "missing_proofs"]
}
```

### 3.3 Tasks

- [ ] Implement `UboVerifyOp`
- [ ] Implement `UboStatusOp` with full convergence calculation
- [ ] Implement `UboExtractObservationOp` (manual observation entry)
- [ ] Implement `UboResolveDisputeOp`
- [ ] Add result types for verification output
- [ ] Unit tests for convergence calculation
- [ ] Integration test: full verify cycle

---

## Phase 4: Assertion Verbs (Declarative Gates)

### 4.1 `ubo.assert` Verb

**Purpose:** Declarative gate - pass if condition true, fail with details if false

**DSL:**
```clojure
(ubo.assert :cbu @cbu :converged true)
(ubo.assert :cbu @cbu :no-expired-proofs true)
(ubo.assert :cbu @cbu :thresholds-pass true)
(ubo.assert :cbu @cbu :no-blocking-flags true)
```

**Implementation:**

**File:** `rust/src/dsl_v2/custom_ops/ubo_assert_ops.rs` (new)

```rust
pub struct UboAssertOp;

#[derive(Debug, Clone, Serialize)]
pub struct AssertionResult {
    pub assertion_type: String,
    pub passed: bool,
    pub expected: bool,
    pub actual: bool,
    pub failure_details: Option<serde_json::Value>,
}

#[async_trait]
impl CustomOperation for UboAssertOp {
    fn domain(&self) -> &'static str { "ubo" }
    fn verb(&self) -> &'static str { "assert" }
    
    async fn execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool) -> Result<ExecutionResult> {
        let cbu_id = get_cbu_id(verb_call, ctx)?;
        
        // Determine which assertion to check
        let (assertion_type, expected) = extract_assertion_condition(verb_call)?;
        
        // Evaluate the condition
        let (actual, details) = match assertion_type.as_str() {
            "converged" => evaluate_converged(pool, cbu_id).await?,
            "no-expired-proofs" => evaluate_no_expired_proofs(pool, cbu_id).await?,
            "thresholds-pass" => evaluate_thresholds_pass(pool, cbu_id).await?,
            "no-blocking-flags" => evaluate_no_blocking_flags(pool, cbu_id).await?,
            _ => return Err(anyhow!("Unknown assertion type: {}", assertion_type)),
        };
        
        let passed = actual == expected;
        
        // Log assertion to audit trail
        log_assertion(pool, cbu_id, ctx.execution_id, &assertion_type, expected, actual, passed, &details).await?;
        
        if passed {
            Ok(ExecutionResult::AssertionPassed { 
                assertion_type,
                cbu_id,
            })
        } else {
            Err(anyhow!(AssertionFailedError {
                assertion_type,
                expected,
                actual,
                details,
            }))
        }
    }
}

async fn evaluate_converged(pool: &PgPool, cbu_id: Uuid) -> Result<(bool, Option<serde_json::Value>)> {
    let status = sqlx::query_as::<_, ConvergenceStatus>(r#"
        SELECT * FROM "ob-poc".ubo_convergence_status WHERE cbu_id = $1
    "#)
    .bind(cbu_id)
    .fetch_optional(pool)
    .await?;
    
    match status {
        Some(s) if s.is_converged => Ok((true, None)),
        Some(s) => {
            let details = json!({
                "total_edges": s.total_edges,
                "proven_edges": s.proven_edges,
                "alleged_edges": s.alleged_edges,
                "disputed_edges": s.disputed_edges,
                "blocking": get_blocking_edges(pool, cbu_id).await?,
            });
            Ok((false, Some(details)))
        },
        None => Ok((true, None)), // No edges = converged (vacuously true)
    }
}

async fn log_assertion(
    pool: &PgPool,
    cbu_id: Uuid,
    execution_id: Option<Uuid>,
    assertion_type: &str,
    expected: bool,
    actual: bool,
    passed: bool,
    details: &Option<serde_json::Value>,
) -> Result<()> {
    sqlx::query(r#"
        INSERT INTO "ob-poc".ubo_assertion_log 
        (cbu_id, dsl_execution_id, assertion_type, expected_value, actual_value, passed, failure_details)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
    "#)
    .bind(cbu_id)
    .bind(execution_id)
    .bind(assertion_type)
    .bind(expected)
    .bind(actual)
    .bind(passed)
    .bind(details)
    .execute(pool)
    .await?;
    
    Ok(())
}
```

### 4.2 Tasks

- [ ] Create `rust/src/dsl_v2/custom_ops/ubo_assert_ops.rs`
- [ ] Implement `UboAssertOp` with condition dispatch
- [ ] Implement `evaluate_converged()`
- [ ] Implement `evaluate_no_expired_proofs()`
- [ ] Implement `evaluate_thresholds_pass()`
- [ ] Implement `evaluate_no_blocking_flags()`
- [ ] Create `AssertionFailedError` type with structured details
- [ ] Implement assertion logging
- [ ] Add YAML verb definition
- [ ] Unit tests for each assertion type
- [ ] Integration test: assertion blocks workflow when false

---

## Phase 5: Evaluation Verbs

### 5.1 `ubo.evaluate` Verb

**Purpose:** Run threshold calculation and red flag checks on converged graph

**DSL:**
```clojure
(ubo.evaluate :cbu @cbu :as @evaluation)
```

**Returns:**
```json
{
  "thresholds": {
    "jurisdiction": "LU",
    "threshold_percentage": 25,
    "passed": true,
    "beneficial_owners": [
      {"entity_id": "...", "name": "Person X", "effective_percentage": 100, "path": [...]}
    ],
    "control_persons": [
      {"entity_id": "...", "name": "CEO Y", "control_role": "ceo"}
    ]
  },
  "red_flags": {
    "blocking": [],
    "warning": ["pep_relative"],
    "info": []
  }
}
```

### 5.2 `ubo.traverse` Verb

**Purpose:** Walk ownership chain, calculate effective percentage

**DSL:**
```clojure
(ubo.traverse :cbu @cbu :from @fund :as @chain)
```

**Returns:**
```json
{
  "chain": [
    {"entity": "Fund ABC", "ownership": 100},
    {"entity": "ManCo GmbH", "ownership": 100},
    {"entity": "HoldCo SE", "ownership": 100},
    {"entity": "Person X", "ownership": 100}
  ],
  "ultimate_owners": [
    {"entity_id": "...", "name": "Person X", "effective_percentage": 100}
  ]
}
```

### 5.3 Tasks

- [ ] Implement `UboEvaluateOp` (or enhance existing)
- [ ] Implement `UboTraverseOp` for chain walking
- [ ] Implement threshold calculation per jurisdiction
- [ ] Integrate with existing `threshold.rs` if applicable
- [ ] Integrate with existing red flag checks
- [ ] Unit tests for chain calculation
- [ ] Unit tests for jurisdiction-specific thresholds

---

## Phase 6: Decision & Review Verbs

### 6.1 `kyc.decision` Verb

**Purpose:** Record final KYC decision

**DSL:**
```clojure
(kyc.decision :cbu @cbu :status "CLEARED" :review-in "12m" :as @decision)
```

### 6.2 `ubo.mark-dirty` Verb

**Purpose:** Flag proofs for re-verification (triggers review)

**DSL:**
```clojure
(ubo.mark-dirty :cbu @cbu :reason "periodic_review")
```

### 6.3 `kyc.trigger-review` Verb

**Purpose:** Initiate ad-hoc review

**DSL:**
```clojure
(kyc.trigger-review :cbu @cbu :reason "ownership_change" :source "corporate_registry")
```

### 6.4 Tasks

- [ ] Implement `KycDecisionOp`
- [ ] Implement `UboMarkDirtyOp`
- [ ] Implement `KycTriggerReviewOp`
- [ ] Implement `KycScheduleReviewOp`
- [ ] Add decision snapshot (evaluation state at decision time)
- [ ] Unit tests

---

## Phase 7: Templates (Run Books)

### 7.1 Standard KYC Template

**File:** `rust/config/verbs/templates/kyc/standard-kyc.yaml`

```yaml
template: standard-kyc
metadata:
  name: Standard KYC Review
  summary: Standard KYC convergence and evaluation flow
  category: kyc

params:
  cbu:
    type: cbu_ref
    cardinality: single
    required: true

primary_entity:
  entity_type: cbu
  param: cbu

body: |
  ;; Check convergence state
  (ubo.status :cbu "$cbu" :as @status)
  
  ;; Assertions - declarative gates
  (ubo.assert :cbu "$cbu" :converged true)
  (ubo.assert :cbu "$cbu" :no-expired-proofs true)
  
  ;; Evaluate
  (ubo.evaluate :cbu "$cbu" :as @eval)
  
  ;; More assertions
  (ubo.assert :cbu "$cbu" :thresholds-pass true)
  (ubo.assert :cbu "$cbu" :no-blocking-flags true)
  
  ;; Decision
  (kyc.decision :cbu "$cbu" :status "CLEARED" :review-in "12m")

outputs:
  - status
  - eval
```

### 7.2 Enhanced DD Template

**File:** `rust/config/verbs/templates/kyc/enhanced-dd.yaml`

Similar but with additional checks and shorter review cycle.

### 7.3 Tasks

- [ ] Create `rust/config/verbs/templates/kyc/` directory
- [ ] Create `standard-kyc.yaml` template
- [ ] Create `enhanced-dd.yaml` template
- [ ] Test template invocation
- [ ] Integration test: full KYC flow via template

---

## Phase 8: Agent Integration

### 8.1 Agent Prompts

**File:** `rust/src/dsl_v2/prompts/kyc_prompts.rs` (or extend existing)

Add KYC-specific prompt guidance:

```rust
pub const KYC_AGENT_GUIDANCE: &str = r#"
When helping with KYC/UBO tasks:

1. Always check current status first:
   (ubo.status :cbu @cbu :as @status)

2. If not converged, identify what's blocking:
   - Missing proofs → help user upload documents
   - Disputed edges → explain discrepancy, ask for clarification
   - Alleged edges → prompt for proof linkage

3. Only attempt evaluation after convergence:
   (ubo.assert :cbu @cbu :converged true)
   (ubo.evaluate :cbu @cbu :as @eval)

4. Present blocking items clearly:
   "The ownership model is X% converged. Blocking items:
    - Edge A→B: needs shareholder register
    - Edge C→D: disputed (alleged 100%, document shows 70%)"

5. Guide through resolution:
   "To resolve the dispute for Edge C→D, we can either:
    1. Update the allegation to match the document (70%)
    2. Request clarification from the client"
"#;
```

### 8.2 Tasks

- [ ] Add KYC guidance to agent system prompt
- [ ] Add KYC intent detection
- [ ] Test agent-driven KYC flow
- [ ] Integration test: chat-based KYC completion

---

## Phase 9: Trigger Infrastructure

### 9.1 Periodic Review Scheduler

**Concept:** Background job that queries `kyc_decisions.next_review_date` and triggers reviews

```rust
// Pseudocode for scheduler job
async fn check_reviews_due(pool: &PgPool) {
    let due = sqlx::query_as::<_, DueReview>(r#"
        SELECT cbu_id FROM "ob-poc".kyc_decisions 
        WHERE next_review_date <= CURRENT_DATE
          AND status = 'CLEARED'
    "#)
    .fetch_all(pool)
    .await?;
    
    for review in due {
        // Execute DSL
        execute_dsl(&format!(
            r#"(ubo.mark-dirty :cbu "{}" :reason "periodic_review")"#,
            review.cbu_id
        ), pool).await?;
        
        // Notify relevant parties
        notify_review_due(review.cbu_id).await?;
    }
}
```

### 9.2 Event Hooks

**Concept:** External events (corporate registry changes, screening hits) trigger reviews

```rust
// API endpoint for external triggers
async fn handle_external_event(event: ExternalEvent, pool: &PgPool) {
    match event.event_type.as_str() {
        "ownership_change" => {
            execute_dsl(&format!(
                r#"(kyc.trigger-review :cbu "{}" :reason "ownership_change" :source "{}")"#,
                event.cbu_id, event.source
            ), pool).await?;
        },
        "sanctions_hit" => {
            // Immediate flag
        },
        _ => {}
    }
}
```

### 9.3 Tasks

- [ ] Design scheduler approach (cron job, background worker, etc.)
- [ ] Implement periodic review check
- [ ] Implement event hook endpoint
- [ ] Test periodic trigger
- [ ] Test event-driven trigger

---

## File Summary

| File | Action | Purpose |
|------|--------|---------|
| `sql/migrations/YYYYMMDD_ubo_convergence.sql` | Create | Schema for edges, observations, assertions |
| `rust/src/dsl_v2/custom_ops/ubo_graph_ops.rs` | Create | allege, link-proof, update, remove |
| `rust/src/dsl_v2/custom_ops/ubo_verify_ops.rs` | Create | verify, status, extract-observation |
| `rust/src/dsl_v2/custom_ops/ubo_assert_ops.rs` | Create | assert verb with conditions |
| `rust/src/dsl_v2/custom_ops/kyc_decision_ops.rs` | Create | decision, schedule-review, trigger-review |
| `rust/config/verbs/ubo.yaml` | Modify | Add new verb definitions |
| `rust/config/verbs/kyc.yaml` | Create | KYC verb definitions |
| `rust/config/verbs/templates/kyc/` | Create | KYC templates directory |
| `rust/tests/ubo_convergence_test.rs` | Create | Integration tests |

---

## Testing Plan

### Unit Tests

```rust
#[test]
fn test_convergence_calculation() {
    // Given: CBU with 3 edges (2 proven, 1 alleged)
    // When: Calculate convergence
    // Then: converged = false, blockers = [alleged edge]
}

#[test]
fn test_assertion_passes_when_converged() {
    // Given: CBU with all edges proven
    // When: (ubo.assert :cbu @cbu :converged true)
    // Then: Passes silently
}

#[test]
fn test_assertion_fails_with_details() {
    // Given: CBU with disputed edge
    // When: (ubo.assert :cbu @cbu :converged true)
    // Then: Fails with structured error including disputed edge details
}

#[test]
fn test_threshold_calculation() {
    // Given: Ownership chain with 60% at one level, 50% at next
    // When: Calculate effective ownership
    // Then: 30% effective (0.6 * 0.5)
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_kyc_flow() {
    // 1. Create CBU
    // 2. Allege ownership structure
    // 3. Link proofs
    // 4. Verify
    // 5. Assert converged
    // 6. Evaluate
    // 7. Assert thresholds pass
    // 8. Decision
    // 9. Verify audit trail
}

#[tokio::test]
async fn test_discrepancy_flow() {
    // 1. Allege 100% ownership
    // 2. Link proof showing 70%
    // 3. Verify → disputed
    // 4. Assert converged → fails
    // 5. Resolve dispute
    // 6. Assert converged → passes
}
```

---

## Success Criteria

### Functional

- [ ] `ubo.allege` creates edges with status='alleged'
- [ ] `ubo.link-proof` attaches proof, sets status='pending'
- [ ] `ubo.verify` compares and updates to proven/disputed
- [ ] `ubo.status` returns accurate convergence state
- [ ] `ubo.assert :converged` passes only when all proven
- [ ] `ubo.assert :converged` fails with blocking details
- [ ] `ubo.evaluate` calculates thresholds and checks flags
- [ ] `kyc.decision` records decision with snapshot
- [ ] Assertions logged to audit table
- [ ] Templates execute full flow

### Audit

- [ ] Every edge state change traceable to DSL verb
- [ ] Assertion pass/fail logged with reasons
- [ ] Decision traceable to evaluation snapshot
- [ ] Regulator can follow: decision ← assertions ← proofs ← allegations

### Agent

- [ ] Agent can query convergence status
- [ ] Agent explains blocking items clearly
- [ ] Agent guides through proof collection
- [ ] Agent can execute full KYC flow via chat

---

## References

- Solution overview: `/docs/KYC-UBO-SOLUTION-OVERVIEW.md`
- Existing UBO ops: `rust/src/dsl_v2/custom_ops/ubo_analysis.rs`
- Existing threshold ops: `rust/src/dsl_v2/custom_ops/threshold.rs`
- Existing RFI ops: `rust/src/dsl_v2/custom_ops/rfi.rs`
- Template system: `rust/src/templates/`
- Batch execution: `rust/src/dsl_v2/batch_executor.rs`

---

*Implementation plan for KYC/UBO convergence model. See KYC-UBO-SOLUTION-OVERVIEW.md for full architecture.*
