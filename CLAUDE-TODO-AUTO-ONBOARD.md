# CLAUDE TODO: Auto-Onboard DSL Verb

## Overview

Create a DSL verb that automatically progresses through onboarding stages by generating and executing DSL for missing entities. This enables rapid test data generation and demonstrates the semantic stage system working end-to-end.

## The Verb

```lisp
(onboarding.auto-complete cbu-id:@alpha-fund max-steps:20 dry-run:false)
```

## Algorithm

```
LOOP (max_steps times):
    1. derive_semantic_state(cbu_id)
    2. IF progress == 100% OR next_actionable.is_empty() → BREAK
    3. stage = next_actionable[0]
    4. missing = missing_entities.find(stage)
    5. dsl = generate_creation_dsl(missing.entity_type, cbu_id, ctx)
    6. IF dry_run → log and continue
       ELSE → execute(dsl)
    7. steps_taken.push(step_info)
    
RETURN { steps_taken, final_progress, entities_created }
```

## Files to Create/Modify

### 1. Verb Definition
**File:** `config/verbs/onboarding.yaml` (new or add to semantic.yaml)

```yaml
auto-complete:
  description: "Automatically progress through onboarding stages by creating missing entities"
  behavior: plugin
  plugin:
    handler: OnboardingAutoCompleteOp
  args:
    - name: cbu-id
      type: uuid
      required: true
      description: "CBU to onboard"
      lookup:
        table: cbus
        schema: ob-poc
        entity_type: cbu
        search_key: name
        primary_key: cbu_id
        resolution_mode: entity
    - name: max-steps
      type: integer
      required: false
      default: 20
      description: "Maximum steps to execute (safety limit)"
    - name: dry-run
      type: boolean
      required: false
      default: false
      description: "Preview what would be created without executing"
    - name: stop-at-stage
      type: string
      required: false
      description: "Stop after completing this stage (e.g., KYC_REVIEW)"
  returns:
    type: onboarding_result
    capture: true
```

### 2. Operation Handler
**File:** `rust/src/dsl_v2/custom_ops/onboarding_ops.rs` (new)

```rust
//! Onboarding Automation Operations
//!
//! Provides automated onboarding progression using the semantic stage system.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use crate::database::derive_semantic_state;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::custom_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::ontology::SemanticStageRegistry;

pub struct OnboardingAutoCompleteOp;

#[async_trait]
impl CustomOperation for OnboardingAutoCompleteOp {
    fn domain(&self) -> &'static str { "onboarding" }
    fn verb(&self) -> &'static str { "auto-complete" }
    fn rationale(&self) -> &'static str {
        "Orchestrates multiple DSL executions based on semantic state - requires loop control"
    }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        // Extract args
        let cbu_id = get_cbu_id(verb_call, ctx)?;
        let max_steps = get_int_arg(verb_call, "max-steps").unwrap_or(20) as usize;
        let dry_run = get_bool_arg(verb_call, "dry-run").unwrap_or(false);
        let stop_at_stage = get_string_arg(verb_call, "stop-at-stage");
        
        let registry = SemanticStageRegistry::load_default()?;
        
        let mut steps: Vec<serde_json::Value> = vec![];
        let mut entities_created: Vec<Uuid> = vec![];
        
        for step_num in 0..max_steps {
            // 1. Derive current state
            let state = derive_semantic_state(pool, &registry, cbu_id).await?;
            
            // 2. Check completion
            if state.overall_progress.percentage >= 100.0 {
                tracing::info!("Auto-onboard complete: 100% progress");
                break;
            }
            
            // 3. Check stop condition
            if let Some(ref stop_stage) = stop_at_stage {
                if state.required_stages.iter().any(|s| 
                    &s.code == stop_stage && s.status == StageStatus::Complete
                ) {
                    tracing::info!("Auto-onboard stopped at stage: {}", stop_stage);
                    break;
                }
            }
            
            // 4. Get next actionable stage
            let Some(next_stage) = state.next_actionable.first() else {
                tracing::info!("Auto-onboard blocked: no actionable stages");
                break;
            };
            
            // 5. Get missing entity for this stage
            let Some(missing) = state.missing_entities.iter()
                .find(|e| &e.stage == next_stage) else {
                continue; // Stage has no missing entities, weird but skip
            };
            
            // 6. Generate creation DSL
            let (dsl, description) = generate_entity_dsl(
                &missing.entity_type, 
                cbu_id, 
                ctx,
                step_num
            )?;
            
            let step_info = json!({
                "step": step_num + 1,
                "stage": next_stage,
                "entity_type": missing.entity_type,
                "dsl": dsl,
                "description": description
            });
            
            // 7. Execute or preview
            if dry_run {
                tracing::info!("[DRY RUN] Would execute: {}", dsl);
                steps.push(step_info);
            } else {
                // Parse and execute the generated DSL
                match execute_generated_dsl(pool, &dsl, ctx).await {
                    Ok(entity_id) => {
                        let mut info = step_info;
                        info["entity_id"] = json!(entity_id.to_string());
                        info["success"] = json!(true);
                        steps.push(info);
                        entities_created.push(entity_id);
                    }
                    Err(e) => {
                        let mut info = step_info;
                        info["success"] = json!(false);
                        info["error"] = json!(e.to_string());
                        steps.push(info);
                        // Continue trying other entities
                    }
                }
            }
        }
        
        // Final state
        let final_state = derive_semantic_state(pool, &registry, cbu_id).await?;
        
        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "dry_run": dry_run,
            "steps_executed": steps.len(),
            "entities_created": entities_created.len(),
            "final_progress": {
                "complete": final_state.overall_progress.stages_complete,
                "total": final_state.overall_progress.stages_total,
                "percentage": final_state.overall_progress.percentage
            },
            "steps": steps
        })))
    }
}

/// Generate DSL to create a missing entity
fn generate_entity_dsl(
    entity_type: &str,
    cbu_id: Uuid,
    ctx: &ExecutionContext,
    step_num: usize,
) -> Result<(String, String)> {
    // Map entity types to creation DSL
    // Uses reasonable defaults for test data
    let (dsl, desc) = match entity_type {
        "kyc_case" => (
            format!("(kyc-case.create cbu-id:{} case-type:INITIAL)", cbu_id),
            "Create initial KYC case".to_string()
        ),
        
        "entity_workstream" => {
            // Need a case_id - check context for @kyc_case or query
            let case_ref = ctx.resolve("kyc_case")
                .map(|id| id.to_string())
                .unwrap_or_else(|| format!("(query-latest-case {})", cbu_id));
            (
                format!("(entity-workstream.create case-id:{} entity-id:{} workstream-type:PRIMARY)", 
                    case_ref, cbu_id),
                "Create entity workstream for CBU".to_string()
            )
        },
        
        "trading_profile" => (
            format!("(trading-profile.import cbu-id:{} profile-name:\"Auto Profile {}\")", 
                cbu_id, step_num),
            "Create trading profile".to_string()
        ),
        
        "cbu_instrument_universe" => (
            format!("(cbu-custody.add-universe cbu-id:{} instrument-class:EQUITY market:US)", cbu_id),
            "Add instrument universe entry".to_string()
        ),
        
        "cbu_ssi" => (
            format!("(cbu-custody.create-ssi cbu-id:{} ssi-type:STANDARD currency:USD)", cbu_id),
            "Create standing settlement instruction".to_string()
        ),
        
        "ssi_booking_rule" => {
            let ssi_ref = ctx.resolve("cbu_ssi")
                .map(|id| id.to_string())
                .unwrap_or_else(|| "?".to_string());
            (
                format!("(cbu-custody.add-booking-rule ssi-id:{} rule-type:DEFAULT)", ssi_ref),
                "Add SSI booking rule".to_string()
            )
        },
        
        "cbu_lifecycle_instance" | "cbu_resource_instance" => (
            format!("(service-resource.provision cbu-id:{} resource-type:LIFECYCLE)", cbu_id),
            "Provision lifecycle resource".to_string()
        ),
        
        "isda_agreement" => (
            format!("(isda.create cbu-id:{} agreement-type:2002 governing-law:NY)", cbu_id),
            "Create ISDA master agreement".to_string()
        ),
        
        "csa_agreement" => {
            let isda_ref = ctx.resolve("isda_agreement")
                .map(|id| id.to_string())
                .unwrap_or_else(|| "?".to_string());
            (
                format!("(isda.add-csa isda-id:{} csa-type:VM)", isda_ref),
                "Add CSA to ISDA".to_string()
            )
        },
        
        "cbu_pricing_config" => (
            format!("(pricing-config.set cbu-id:{} source:BLOOMBERG)", cbu_id),
            "Configure pricing source".to_string()
        ),
        
        "share_class" => (
            format!("(share-class.create cbu-id:{} class-name:\"Class A\" currency:USD)", cbu_id),
            "Create share class".to_string()
        ),
        
        "holding" => {
            let class_ref = ctx.resolve("share_class")
                .map(|id| id.to_string())
                .unwrap_or_else(|| "?".to_string());
            (
                format!("(holding.create share-class-id:{} investor-name:\"Test Investor\" units:1000)", class_ref),
                "Create investor holding".to_string()
            )
        },
        
        _ => {
            return Err(anyhow::anyhow!(
                "No auto-generation template for entity type: {}", entity_type
            ));
        }
    };
    
    Ok((dsl, desc))
}

/// Execute generated DSL and return the created entity ID
async fn execute_generated_dsl(
    pool: &sqlx::PgPool,
    dsl: &str,
    ctx: &mut ExecutionContext,
) -> Result<Uuid> {
    use crate::dsl_v2::{compile, parse_program, DslExecutor};
    
    let program = parse_program(dsl)?;
    let plan = compile(&program)?;
    let executor = DslExecutor::new(pool.clone());
    
    let results = executor.execute_plan(&plan, ctx).await?;
    
    // Extract UUID from first result
    for result in results {
        if let crate::dsl_v2::ExecutionResult::Uuid(id) = result {
            return Ok(id);
        }
    }
    
    Err(anyhow::anyhow!("No entity ID returned from DSL execution"))
}
```

### 3. Register the Operation
**File:** `rust/src/dsl_v2/custom_ops/mod.rs`

```rust
mod onboarding_ops;
pub use onboarding_ops::OnboardingAutoCompleteOp;

// In register_all():
registry.register(Box::new(OnboardingAutoCompleteOp));
```

## Usage Examples

### Basic Auto-Complete
```lisp
; Complete as much as possible
(onboarding.auto-complete cbu-id:@alpha-fund)
```

### Dry Run Preview
```lisp
; See what would be created without executing
(onboarding.auto-complete cbu-id:@alpha-fund dry-run:true)
```

### Stop at Specific Stage
```lisp
; Complete up to KYC, then stop
(onboarding.auto-complete cbu-id:@alpha-fund stop-at-stage:KYC_REVIEW)
```

### Limited Steps
```lisp
; Only do 5 steps (for testing)
(onboarding.auto-complete cbu-id:@alpha-fund max-steps:5)
```

## Expected Output

```json
{
  "cbu_id": "uuid...",
  "dry_run": false,
  "steps_executed": 8,
  "entities_created": 8,
  "final_progress": {
    "complete": 5,
    "total": 6,
    "percentage": 83.3
  },
  "steps": [
    {
      "step": 1,
      "stage": "KYC_REVIEW",
      "entity_type": "kyc_case",
      "dsl": "(kyc-case.create cbu-id:... case-type:INITIAL)",
      "entity_id": "uuid...",
      "success": true
    },
    ...
  ]
}
```

## Test Scenario

```lisp
; 1. Create a new CBU with a product
(cbu.ensure name:"Auto Test Fund" jurisdiction:US) -> @fund
(cbu.add-product cbu-id:@fund product:CUSTODY)

; 2. Check initial state
(semantic.get-state cbu-id:@fund)
; Shows: 1/6 stages complete, KYC_REVIEW next

; 3. Preview auto-complete
(onboarding.auto-complete cbu-id:@fund dry-run:true)
; Shows: Would create kyc_case, entity_workstream, trading_profile, etc.

; 4. Run auto-complete
(onboarding.auto-complete cbu-id:@fund)
; Creates all entities, returns progress

; 5. Verify completion
(semantic.get-state cbu-id:@fund)
; Shows: 6/6 stages complete (100%)
```

## Edge Cases to Handle

1. **Circular dependencies** - Some entities need others first (e.g., workstream needs case)
   - Solution: Context bindings track created entities, retry on failure

2. **Missing lookup data** - Entity creation needs reference data (currencies, markets)
   - Solution: Use common defaults (USD, US, EQUITY)

3. **Validation failures** - Generated DSL might fail validation
   - Solution: Log error, continue with next entity, report at end

4. **Conditional stages** - Some stages only apply with certain products
   - Solution: Already handled by semantic state derivation

5. **Blocking stages** - KYC might block downstream until approved
   - Solution: Auto-approve for test data, or stop and report

## Future Enhancements

1. **UI Integration** - "Auto-Pilot" button in journey panel
2. **Streaming** - Return steps as they happen for real-time UI updates
3. **Rollback** - Undo all created entities on failure
4. **Templates** - Use configurable templates instead of hardcoded DSL
5. **AI Generation** - Use LLM to generate more realistic test data

## Success Criteria

1. `(onboarding.auto-complete cbu-id:@fund)` takes a CBU from 0% to 80%+ progress
2. Each step is logged with DSL and result
3. Dry-run mode shows what would happen without side effects
4. Handles errors gracefully, continues where possible
5. Works with all product types (CUSTODY, FUND_ACCOUNTING, etc.)
