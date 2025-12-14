# TODO: Workflow Requirements Evaluation (Gap Fill)

**Purpose**: Make guards data-driven from YAML requirements, not hardcoded  
**Priority**: HIGH - Core to config-driven ethos  
**Effort**: ~6-8 hours

---

## Current State

**What exists and works:**
- `rust/src/workflow/` - Full engine, repository, state management
- `rust/config/workflows/kyc_onboarding.yaml` - Complete workflow definition
- MCP tools wired up (workflow_status, workflow_advance, etc.)
- Named guards in `guards.rs` (entities_complete, screening_complete, etc.)

**The Problem:**

YAML defines requirements declaratively:
```yaml
requirements:
  ENTITY_COLLECTION:
    - type: role_count
      role: DIRECTOR
      min: 1
```

But guards.rs ignores this and has hardcoded logic:
```rust
match guard_name {
    "entities_complete" => self.check_entities_complete(subject_id).await,
    // hardcoded SQL queries inside
}
```

**What we want:** Guards evaluate requirements from YAML, not hardcoded. Add new requirements = edit YAML, not Rust code.

---

## Solution Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        YAML WORKFLOW DEFINITION                         │
│                                                                         │
│  requirements:                                                          │
│    ENTITY_COLLECTION:                                                   │
│      - type: role_count                                                 │
│        role: DIRECTOR                                                   │
│        min: 1                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      GUARD EVALUATOR (Updated)                          │
│                                                                         │
│  1. Get requirements for current state from definition                  │
│  2. Evaluate each requirement using RequirementEvaluator                │
│  3. Collect blockers from any failed requirements                       │
│  4. Also run named custom guard if specified on transition              │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation

### 1. Create `rust/src/workflow/requirements.rs`

Generic evaluator for `RequirementDef` from YAML:

```rust
//! Requirement Evaluation
//!
//! Evaluates requirements defined in workflow YAML.
//! Each requirement type maps to a database check.

use sqlx::PgPool;
use uuid::Uuid;

use super::definition::RequirementDef;
use super::state::{Blocker, BlockerType};

/// Evaluates workflow requirements from YAML definitions
pub struct RequirementEvaluator {
    pool: PgPool,
}

impl RequirementEvaluator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Evaluate a single requirement, returning blockers if not met
    pub async fn evaluate(
        &self,
        req: &RequirementDef,
        subject_id: Uuid,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        match req {
            RequirementDef::RoleCount { role, min, description } => {
                self.check_role_count(subject_id, role, *min, description).await
            }
            RequirementDef::AllEntitiesScreened { description } => {
                self.check_all_screened(subject_id, description).await
            }
            RequirementDef::DocumentSet { documents, description } => {
                self.check_document_set(subject_id, documents, description).await
            }
            RequirementDef::PerEntityDocument { entity_type, documents, description } => {
                self.check_per_entity_docs(subject_id, entity_type, documents, description).await
            }
            RequirementDef::OwnershipComplete { threshold, description } => {
                self.check_ownership(subject_id, *threshold, description).await
            }
            RequirementDef::AllUbosVerified { description } => {
                self.check_ubos_verified(subject_id, description).await
            }
            RequirementDef::NoOpenAlerts { description } => {
                self.check_no_alerts(subject_id, description).await
            }
            RequirementDef::CaseChecklistComplete { description } => {
                self.check_checklist(subject_id, description).await
            }
        }
    }
    
    /// Evaluate all requirements for a state
    pub async fn evaluate_all(
        &self,
        requirements: &[RequirementDef],
        subject_id: Uuid,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        let mut all_blockers = Vec::new();
        
        for req in requirements {
            let blockers = self.evaluate(req, subject_id).await?;
            all_blockers.extend(blockers);
        }
        
        Ok(all_blockers)
    }
    
    // --- Individual requirement checks ---
    
    async fn check_role_count(
        &self,
        cbu_id: Uuid,
        role: &str,
        min: u32,
        description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            AND r.name = $2
            "#,
        )
        .bind(cbu_id)
        .bind(role)
        .fetch_one(&self.pool)
        .await?;
        
        if (count as u32) < min {
            Ok(vec![
                Blocker::new(
                    BlockerType::MissingRole {
                        role: role.to_string(),
                        required: min,
                        current: count as u32,
                    },
                    if description.is_empty() {
                        format!("At least {} {} required", min, role.to_lowercase())
                    } else {
                        description.to_string()
                    },
                )
                .with_resolution("cbu.assign-role")
                .with_detail("role", serde_json::json!(role))
            ])
        } else {
            Ok(vec![])
        }
    }
    
    async fn check_all_screened(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        let unscreened: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".screenings s
                WHERE s.entity_id = e.entity_id
                AND s.screened_at > NOW() - INTERVAL '90 days'
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(unscreened.iter().map(|(id, name)| {
            Blocker::new(
                BlockerType::PendingScreening { entity_id: *id },
                format!("Screening required for {}", name),
            )
            .with_resolution("case-screening.run")
            .with_detail("entity_id", serde_json::json!(id))
        }).collect())
    }
    
    async fn check_document_set(
        &self,
        cbu_id: Uuid,
        documents: &[String],
        _description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        let mut blockers = Vec::new();
        
        for doc_type in documents {
            let exists: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".document_catalog d
                    WHERE d.cbu_id = $1
                    AND d.document_type_code = $2
                    AND d.status = 'active'
                )
                "#,
            )
            .bind(cbu_id)
            .bind(doc_type)
            .fetch_one(&self.pool)
            .await?;
            
            if !exists {
                blockers.push(
                    Blocker::new(
                        BlockerType::MissingDocument {
                            document_type: doc_type.clone(),
                            for_entity: None,
                        },
                        format!("{} required", doc_type.replace('_', " ").to_lowercase()),
                    )
                    .with_resolution("document.catalog")
                    .with_detail("document_type", serde_json::json!(doc_type))
                );
            }
        }
        
        Ok(blockers)
    }
    
    async fn check_per_entity_docs(
        &self,
        cbu_id: Uuid,
        entity_type: &str,
        documents: &[String],
        _description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        // Get entities of this type (e.g., DIRECTOR role)
        let entities: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_entity_roles cer ON e.entity_id = cer.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.cbu_id = $1
            AND r.name = $2
            "#,
        )
        .bind(cbu_id)
        .bind(entity_type)
        .fetch_all(&self.pool)
        .await?;
        
        let mut blockers = Vec::new();
        
        for (entity_id, entity_name) in entities {
            for doc_type in documents {
                let has_doc: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM "ob-poc".document_catalog d
                        WHERE d.entity_id = $1
                        AND d.document_type_code = $2
                        AND d.status = 'active'
                    )
                    "#,
                )
                .bind(entity_id)
                .bind(doc_type)
                .fetch_one(&self.pool)
                .await?;
                
                if !has_doc {
                    blockers.push(
                        Blocker::new(
                            BlockerType::MissingDocument {
                                document_type: doc_type.clone(),
                                for_entity: Some(entity_id),
                            },
                            format!("{} required for {}", doc_type.replace('_', " "), entity_name),
                        )
                        .with_resolution("document.catalog")
                        .with_detail("entity_id", serde_json::json!(entity_id))
                        .with_detail("document_type", serde_json::json!(doc_type))
                    );
                }
            }
        }
        
        Ok(blockers)
    }
    
    async fn check_ownership(
        &self,
        cbu_id: Uuid,
        threshold: f64,
        _description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        let total: Option<rust_decimal::Decimal> = sqlx::query_scalar(
            r#"
            SELECT SUM(ownership_percent) FROM "ob-poc".ownership_relationships o
            JOIN "ob-poc".cbu_entity_roles cer ON o.owned_entity_id = cer.entity_id
            WHERE cer.cbu_id = $1
            AND (o.effective_to IS NULL OR o.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await?;
        
        let total_f64: f64 = total
            .map(|d| d.to_string().parse().unwrap_or(0.0))
            .unwrap_or(0.0);
        
        if (total_f64 - threshold).abs() > 0.01 && total_f64 < threshold {
            Ok(vec![
                Blocker::new(
                    BlockerType::IncompleteOwnership {
                        current_total: total_f64,
                        required: threshold,
                    },
                    format!("Ownership {:.1}% of {:.0}% documented", total_f64, threshold),
                )
                .with_resolution("ubo.add-ownership")
            ])
        } else {
            Ok(vec![])
        }
    }
    
    async fn check_ubos_verified(
        &self,
        cbu_id: Uuid,
        _description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        let unverified: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT u.ubo_id, e.name
            FROM "ob-poc".ubo_registry u
            JOIN "ob-poc".entities e ON u.ubo_person_id = e.entity_id
            WHERE u.cbu_id = $1
            AND u.verification_status NOT IN ('VERIFIED', 'PROVEN')
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(unverified.iter().map(|(ubo_id, name)| {
            Blocker::new(
                BlockerType::UnverifiedUbo {
                    ubo_id: *ubo_id,
                    person_name: name.clone(),
                },
                format!("UBO verification required for {}", name),
            )
            .with_resolution("ubo.verify-ubo")
            .with_detail("ubo_id", serde_json::json!(ubo_id))
        }).collect())
    }
    
    async fn check_no_alerts(
        &self,
        cbu_id: Uuid,
        _description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        let alerts: Vec<(Uuid, Uuid)> = sqlx::query_as(
            r#"
            SELECT s.screening_id, ew.entity_id
            FROM kyc.screenings s
            JOIN kyc.entity_workstreams ew ON s.workstream_id = ew.workstream_id
            JOIN kyc.cases c ON ew.case_id = c.case_id
            WHERE c.cbu_id = $1
            AND s.status = 'HIT_PENDING_REVIEW'
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(alerts.iter().map(|(alert_id, entity_id)| {
            Blocker::new(
                BlockerType::UnresolvedAlert {
                    alert_id: *alert_id,
                    entity_id: *entity_id,
                },
                "Unresolved screening alert",
            )
            .with_resolution("case-screening.review-hit")
            .with_detail("screening_id", serde_json::json!(alert_id))
        }).collect())
    }
    
    async fn check_checklist(
        &self,
        cbu_id: Uuid,
        description: &str,
    ) -> Result<Vec<Blocker>, sqlx::Error> {
        // Check if case checklist is complete
        let incomplete: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM kyc.case_checklist_items ci
                JOIN kyc.cases c ON ci.case_id = c.case_id
                WHERE c.cbu_id = $1
                AND ci.completed_at IS NULL
            )
            "#,
        )
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await?;
        
        if incomplete {
            Ok(vec![
                Blocker::new(
                    BlockerType::Custom { code: "CHECKLIST_INCOMPLETE".to_string() },
                    if description.is_empty() {
                        "Case checklist items not complete".to_string()
                    } else {
                        description.to_string()
                    },
                )
            ])
        } else {
            Ok(vec![])
        }
    }
}
```

### 2. Update `guards.rs` to Use Requirements

Update GuardEvaluator to read requirements from workflow definition:

```rust
//! Guard Evaluation
//!
//! Guards now evaluate requirements from YAML definitions.
//! Custom named guards still supported for complex logic.

use sqlx::PgPool;
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;

use super::definition::WorkflowDefinition;
use super::requirements::RequirementEvaluator;
use super::state::{Blocker, BlockerType};

pub struct GuardEvaluator {
    pool: PgPool,
    requirement_evaluator: RequirementEvaluator,
    definitions: Arc<HashMap<String, WorkflowDefinition>>,
}

impl GuardEvaluator {
    pub fn new(pool: PgPool, definitions: Arc<HashMap<String, WorkflowDefinition>>) -> Self {
        Self {
            requirement_evaluator: RequirementEvaluator::new(pool.clone()),
            pool,
            definitions,
        }
    }
    
    /// Evaluate guard for a transition
    /// 
    /// Two-phase evaluation:
    /// 1. Evaluate requirements for the TARGET state from YAML
    /// 2. If transition has a named custom guard, also run that
    pub async fn evaluate_for_transition(
        &self,
        workflow_id: &str,
        from_state: &str,
        to_state: &str,
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<GuardResult, sqlx::Error> {
        let definition = match self.definitions.get(workflow_id) {
            Some(d) => d,
            None => return Ok(GuardResult::failed(format!("Unknown workflow: {}", workflow_id))),
        };
        
        let mut all_blockers = Vec::new();
        
        // 1. Evaluate requirements for TARGET state
        if let Some(requirements) = definition.requirements.get(to_state) {
            let blockers = self.requirement_evaluator
                .evaluate_all(requirements, subject_id)
                .await?;
            all_blockers.extend(blockers);
        }
        
        // 2. If transition has a named guard, also run that
        if let Some(transition) = definition.get_transition(from_state, to_state) {
            if let Some(guard_name) = &transition.guard {
                let custom_result = self.evaluate_custom_guard(guard_name, subject_id, subject_type).await?;
                all_blockers.extend(custom_result.blockers);
            }
        }
        
        if all_blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(all_blockers))
        }
    }
    
    /// Evaluate a custom named guard (for complex logic)
    async fn evaluate_custom_guard(
        &self,
        guard_name: &str,
        subject_id: Uuid,
        _subject_type: &str,
    ) -> Result<GuardResult, sqlx::Error> {
        match guard_name {
            // Case status checks (can't be expressed as requirements)
            "review_approved" => self.check_case_status(subject_id, "APPROVED").await,
            "review_rejected" => self.check_case_status(subject_id, "REJECTED").await,
            
            // Add other custom guards here as needed
            _ => {
                // Unknown guard - log warning but don't block
                tracing::warn!("Unknown custom guard: {}", guard_name);
                Ok(GuardResult::passed())
            }
        }
    }
    
    async fn check_case_status(
        &self,
        cbu_id: Uuid,
        required_status: &str,
    ) -> Result<GuardResult, sqlx::Error> {
        let case_status: Option<String> = sqlx::query_scalar(
            r#"
            SELECT status FROM kyc.cases
            WHERE cbu_id = $1
            ORDER BY opened_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if case_status.as_deref() == Some(required_status) {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(vec![
                Blocker::new(
                    BlockerType::ManualApprovalRequired,
                    format!("Case must be {}", required_status),
                )
                .with_resolution("kyc-case.update-status")
            ]))
        }
    }
}

// ... GuardResult remains the same
```

### 3. Update `engine.rs` to Use New Guard Evaluation

Change the engine to call `evaluate_for_transition` instead of just evaluating named guards:

```rust
// In engine.rs, update try_advance and evaluate_blockers:

async fn try_advance(&self, instance_id: Uuid) -> Result<WorkflowInstance, WorkflowError> {
    let mut instance = self.repo.load(instance_id).await?;
    let definition = self.definitions.get(&instance.workflow_id)
        .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;
    
    // Find auto transitions from current state
    for transition in definition.transitions_from(&instance.current_state) {
        if !transition.auto {
            continue;
        }
        
        // Evaluate guard using requirements + custom guard
        let result = self.guard_evaluator.evaluate_for_transition(
            &instance.workflow_id,
            &instance.current_state,
            &transition.to,
            instance.subject_id,
            &instance.subject_type,
        ).await?;
        
        if result.passed {
            // Execute transition
            instance.transition(transition.to.clone(), None, Some("Auto-advanced".to_string()));
            self.repo.save(&instance).await?;
            
            // Recursively try to advance again
            return self.try_advance(instance.instance_id).await;
        }
    }
    
    // Update blockers for all outgoing transitions
    instance.blockers = self.evaluate_all_blockers(&instance, definition).await?;
    self.repo.save(&instance).await?;
    
    Ok(instance)
}

async fn evaluate_all_blockers(
    &self,
    instance: &WorkflowInstance,
    definition: &WorkflowDefinition,
) -> Result<Vec<Blocker>, WorkflowError> {
    let mut all_blockers = Vec::new();
    
    // Check each possible outgoing transition
    for transition in definition.transitions_from(&instance.current_state) {
        let result = self.guard_evaluator.evaluate_for_transition(
            &instance.workflow_id,
            &instance.current_state,
            &transition.to,
            instance.subject_id,
            &instance.subject_type,
        ).await?;
        
        all_blockers.extend(result.blockers);
    }
    
    // Deduplicate
    all_blockers.sort_by(|a, b| a.description.cmp(&b.description));
    all_blockers.dedup_by(|a, b| a.description == b.description);
    
    Ok(all_blockers)
}
```

### 4. Update `mod.rs` Exports

```rust
mod requirements;

pub use requirements::RequirementEvaluator;
```

---

## Summary of Changes

| File | Change |
|------|--------|
| `requirements.rs` | **NEW** - Generic evaluator for RequirementDef |
| `guards.rs` | **UPDATE** - Use RequirementEvaluator, keep minimal custom guards |
| `engine.rs` | **UPDATE** - Call evaluate_for_transition |
| `mod.rs` | **UPDATE** - Export RequirementEvaluator |

---

## Benefits

1. **Config-driven** - Add new requirements to YAML, no code changes
2. **Consistent** - All requirement types evaluated the same way
3. **Custom guards for complex logic** - Still supported when needed
4. **Backward compatible** - Existing workflow YAML works unchanged

---

## Example: Adding New Requirement

**Before (code change required):**
```rust
// Edit guards.rs, add new match arm, write SQL
```

**After (YAML only):**
```yaml
requirements:
  ENTITY_COLLECTION:
    - type: role_count
      role: COMPLIANCE_OFFICER  # New requirement
      min: 1
      description: Compliance officer required
```

---

## Optional: JSON Schema for IDE

Create `rust/config/workflows/schema/workflow.schema.json` for IDE auto-completion.

Low priority - YAML is rarely edited, and the definition.rs already validates on load.

---

## Implementation Checklist

- [ ] Create `requirements.rs` with RequirementEvaluator
- [ ] Implement all RequirementDef type evaluations
- [ ] Update `guards.rs` to use RequirementEvaluator
- [ ] Update `engine.rs` to call evaluate_for_transition
- [ ] Update `mod.rs` exports
- [ ] Test: Add requirement to YAML, verify it generates blockers
- [ ] Test: Remove requirement from YAML, verify no blockers
- [ ] Test: Custom guard still works for review_approved/rejected

---

## Effort Estimate

| Task | Hours |
|------|-------|
| Create requirements.rs | 3 |
| Update guards.rs | 1 |
| Update engine.rs | 1 |
| Testing | 2 |
| **Total** | **~7 hours** |
