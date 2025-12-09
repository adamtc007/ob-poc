# TODO — CBU Decision Verb

> **Goal:**  
> Implement `cbu.decide` verb that transitions CBU collective state (CBU + UBO + entities + docs) at decision points (APPROVED / REJECTED / REFERRED).

This verb execution becomes the searchable decision point in DSL history.

---

## Context

- CBU status represents the **collective state** of the entire CBU graph (entities, UBOs, documents, screenings)
- Decision points (KYC approval, periodic review, material change review) are the meaningful snapshot boundaries
- The DSL execution record IS the snapshot — no separate snapshot mechanism needed
- Decisions are searchable in DSL history by verb name

---

## 1. Add Verb Definition

**File:** `rust/config/verbs/cbu.yaml`

Add to the `cbu` domain verbs:

```yaml
      decide:
        description: "Record KYC/AML decision for CBU collective state (entities, UBOs, documents)"
        behavior: plugin
        plugin:
          handler: CbuDecideOp
        consumes:
          - arg: cbu-id
            type: cbu
            required: true
        lifecycle:
          entity_arg: cbu-id
          requires_states:
            - PENDING_VALIDATION
          # transitions_to determined by decision value
          precondition_checks:
            - check_ubo_completeness
            - check_cbu_evidence_completeness
            - check_kyc_case_ready_for_decision
        args:
          - name: cbu-id
            type: uuid
            required: true
            description: "CBU to decide"
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: decision
            type: string
            required: true
            description: "Decision outcome"
            validation:
              enum:
                - APPROVED
                - REJECTED
                - REFERRED
          - name: decided-by
            type: string
            required: true
            description: "User/agent making the decision"
          - name: rationale
            type: string
            required: true
            description: "Reason for decision"
          - name: case-id
            type: uuid
            required: false
            description: "Associated KYC case (optional, inferred if single active case)"
            lookup:
              table: cases
              entity_type: kyc_case
              schema: kyc
              search_key: case_reference
              primary_key: case_id
          - name: conditions
            type: string
            required: false
            description: "Conditions attached to approval (if any)"
          - name: escalation-reason
            type: string
            required: false
            description: "Reason for referral (required if decision=REFERRED)"
        returns:
          type: record
          description: "Decision record with outcome and resulting CBU status"
```

---

## 2. Implement Plugin Handler

**File:** `rust/src/dsl_v2/custom_ops/cbu_decide.rs` (new file)

```rust
//! CBU Decision Handler
//!
//! Records KYC/AML decision and transitions CBU collective state.
//! The DSL execution of this verb IS the decision point snapshot.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::{json, Value};

use crate::dsl_v2::custom_ops::{CustomOp, CustomOpContext, CustomOpResult};
use crate::error::AppError;

pub struct CbuDecideOp;

#[async_trait]
impl CustomOp for CbuDecideOp {
    async fn execute(
        &self,
        ctx: &CustomOpContext,
        pool: &PgPool,
    ) -> Result<CustomOpResult, AppError> {
        // Extract args
        let cbu_id: Uuid = ctx.require_arg("cbu-id")?;
        let decision: String = ctx.require_arg("decision")?;
        let decided_by: String = ctx.require_arg("decided-by")?;
        let rationale: String = ctx.require_arg("rationale")?;
        let case_id: Option<Uuid> = ctx.get_arg("case-id")?;
        let conditions: Option<String> = ctx.get_arg("conditions")?;
        let escalation_reason: Option<String> = ctx.get_arg("escalation-reason")?;

        // Validate decision-specific requirements
        if decision == "REFERRED" && escalation_reason.is_none() {
            return Err(AppError::Validation(
                "escalation-reason required when decision is REFERRED".into()
            ));
        }

        // Get current CBU state
        let cbu = sqlx::query!(
            r#"SELECT status, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(pool)
        .await?;

        // Determine new status based on decision
        let new_status = match decision.as_str() {
            "APPROVED" => "VALIDATED",
            "REJECTED" => "TERMINATED",
            "REFERRED" => "PENDING_VALIDATION", // Stays in review, escalated
            _ => return Err(AppError::Validation(format!("Invalid decision: {}", decision))),
        };

        // Find or validate case
        let case_id = match case_id {
            Some(id) => id,
            None => {
                // Find active case for this CBU
                sqlx::query_scalar!(
                    r#"SELECT case_id FROM kyc.cases 
                       WHERE cbu_id = $1 AND status NOT IN ('CLOSED', 'CANCELLED')
                       ORDER BY created_at DESC LIMIT 1"#,
                    cbu_id
                )
                .fetch_optional(pool)
                .await?
                .flatten()
                .ok_or_else(|| AppError::Validation("No active KYC case found for CBU".into()))?
            }
        };

        // Begin transaction
        let mut tx = pool.begin().await?;

        // 1. Update CBU status
        sqlx::query!(
            r#"UPDATE "ob-poc".cbus SET status = $1, updated_at = now() WHERE cbu_id = $2"#,
            new_status,
            cbu_id
        )
        .execute(&mut *tx)
        .await?;

        // 2. Record decision in case
        let case_status = match decision.as_str() {
            "APPROVED" => "APPROVED",
            "REJECTED" => "REJECTED",
            "REFERRED" => "ESCALATED",
            _ => unreachable!(),
        };

        sqlx::query!(
            r#"UPDATE kyc.cases 
               SET status = $1, 
                   decided_at = now(),
                   decided_by = $2,
                   decision_rationale = $3
               WHERE case_id = $4"#,
            case_status,
            decided_by,
            rationale,
            case_id
        )
        .execute(&mut *tx)
        .await?;

        // 3. Create decision snapshot record
        let snapshot_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots 
               (snapshot_id, case_id, decision_made, decision_made_at, decision_made_by, decision_notes)
               VALUES ($1, $2, $3, now(), $4, $5)"#,
            snapshot_id,
            case_id,
            decision,
            decided_by,
            rationale
        )
        .execute(&mut *tx)
        .await?;

        // 4. Log the change
        sqlx::query!(
            r#"INSERT INTO "ob-poc".cbu_change_log 
               (cbu_id, change_type, field_name, old_value, new_value, changed_by, reason, case_id)
               VALUES ($1, 'DECISION', 'status', $2, $3, $4, $5, $6)"#,
            cbu_id,
            json!(cbu.status),
            json!(new_status),
            decided_by,
            rationale,
            case_id
        )
        .execute(&mut *tx)
        .await?;

        // 5. If REFERRED, create escalation record
        if decision == "REFERRED" {
            sqlx::query!(
                r#"INSERT INTO kyc.escalations 
                   (case_id, escalation_reason, escalated_by, escalated_at)
                   VALUES ($1, $2, $3, now())"#,
                case_id,
                escalation_reason,
                decided_by
            )
            .execute(&mut *tx)
            .await
            .ok(); // Table may not exist, don't fail
        }

        tx.commit().await?;

        // Return decision record
        Ok(CustomOpResult::Record(json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu.name,
            "case_id": case_id,
            "decision": decision,
            "previous_status": cbu.status,
            "new_status": new_status,
            "decided_by": decided_by,
            "rationale": rationale,
            "conditions": conditions,
            "snapshot_id": snapshot_id
        })))
    }
}
```

---

## 3. Register the Handler

**File:** `rust/src/dsl_v2/custom_ops/mod.rs`

Add:
```rust
mod cbu_decide;
pub use cbu_decide::CbuDecideOp;

// In the handler registry:
handlers.insert("CbuDecideOp".to_string(), Box::new(CbuDecideOp));
```

---

## 4. Add Precondition Check

**File:** `rust/config/ontology/entity_taxonomy.yaml`

Add to `precondition_checks` section (or create if not exists):

```yaml
precondition_checks:
  check_kyc_case_ready_for_decision:
    description: "Verify KYC case is ready for decision (workstreams complete, red flags addressed)"
    implementation: sql_query
    query: |
      SELECT 
        NOT EXISTS (
          SELECT 1 FROM kyc.entity_workstreams 
          WHERE case_id = $1 AND status NOT IN ('COMPLETE', 'WAIVED')
        )
        AND NOT EXISTS (
          SELECT 1 FROM kyc.red_flags 
          WHERE case_id = $1 AND status IN ('OPEN', 'UNDER_REVIEW')
        )
    entity_arg: case_id
    returns: boolean
```

---

## 5. DSL Usage Examples

```lisp
;; Approve after successful KYC
(cbu.decide 
  :cbu-id @fund 
  :decision "APPROVED"
  :decided-by "analyst@custody.com"
  :rationale "UBO chain verified to natural persons. No PEP/sanctions hits. Source of funds documented.")

;; Approve with conditions
(cbu.decide 
  :cbu-id @fund 
  :decision "APPROVED"
  :decided-by "compliance@custody.com"
  :rationale "Approved subject to conditions"
  :conditions "Enhanced monitoring for 12 months. Quarterly transaction review.")

;; Reject
(cbu.decide 
  :cbu-id @fund 
  :decision "REJECTED"
  :decided-by "mlro@custody.com"
  :rationale "Unable to verify UBO chain. Beneficial owner in high-risk jurisdiction with no mitigating factors.")

;; Refer for escalation
(cbu.decide 
  :cbu-id @fund 
  :decision "REFERRED"
  :decided-by "analyst@custody.com"
  :rationale "Complex ownership structure requires senior review"
  :escalation-reason "PEP identified in ownership chain - requires MLRO sign-off")
```

---

## 6. Querying Decision Points

Decision points are searchable in DSL execution history:

```sql
-- All decisions
SELECT * FROM dsl_executions 
WHERE dsl_source LIKE '%(cbu.decide%'
ORDER BY executed_at DESC;

-- Decisions for specific CBU
SELECT * FROM dsl_executions 
WHERE cbu_id = $1 
AND dsl_source LIKE '%(cbu.decide%'
ORDER BY executed_at DESC;

-- All approvals
SELECT * FROM dsl_executions 
WHERE dsl_source LIKE '%:decision "APPROVED"%';

-- All referrals
SELECT * FROM dsl_executions 
WHERE dsl_source LIKE '%:decision "REFERRED"%';

-- Decisions by user
SELECT * FROM dsl_executions 
WHERE dsl_source LIKE '%:decided-by "analyst@custody.com"%';
```

---

## 7. Tests

**File:** `rust/src/dsl_v2/custom_ops/cbu_decide_test.rs`

```rust
#[tokio::test]
async fn test_cbu_decide_approved() {
    // Setup: CBU in PENDING_VALIDATION with complete KYC
    // Execute: cbu.decide with APPROVED
    // Assert: CBU status = VALIDATED, case status = APPROVED
}

#[tokio::test]
async fn test_cbu_decide_rejected() {
    // Setup: CBU in PENDING_VALIDATION
    // Execute: cbu.decide with REJECTED
    // Assert: CBU status = TERMINATED, case status = REJECTED
}

#[tokio::test]
async fn test_cbu_decide_referred() {
    // Setup: CBU in PENDING_VALIDATION
    // Execute: cbu.decide with REFERRED
    // Assert: CBU status stays PENDING_VALIDATION, case status = ESCALATED
}

#[tokio::test]
async fn test_cbu_decide_requires_escalation_reason_for_referred() {
    // Execute: cbu.decide with REFERRED but no escalation-reason
    // Assert: Validation error
}

#[tokio::test]
async fn test_cbu_decide_blocked_in_wrong_state() {
    // Setup: CBU in DRAFT (not PENDING_VALIDATION)
    // Execute: cbu.decide
    // Assert: Planner blocks with LifecycleViolation
}
```

---

## 8. Acceptance Criteria

- [ ] `cbu.decide` verb defined in YAML with APPROVED/REJECTED/REFERRED options
- [ ] Plugin handler implements state transitions
- [ ] CBU status transitions correctly based on decision
- [ ] KYC case status updated in parallel
- [ ] Decision recorded in `case_evaluation_snapshots`
- [ ] Change logged in `cbu_change_log`
- [ ] Planner enforces `requires_states: [PENDING_VALIDATION]`
- [ ] Precondition checks validate readiness
- [ ] REFERRED requires `escalation-reason`
- [ ] DSL execution searchable by verb and decision value
- [ ] Tests passing

---

## Summary

The `cbu.decide` verb is the decision point for CBU collective state. Its execution in DSL history IS the searchable snapshot boundary. No separate snapshot mechanism needed — the DSL model captures everything.
