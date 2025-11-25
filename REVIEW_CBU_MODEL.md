# Review: CBU Model Architecture

**Created:** 2025-11-25  
**Status:** REVIEW COMPLETE  
**Component:** cbu_model_dsl/, forth_engine/cbu_model_parser.rs, forth_engine/env.rs  
**Priority:** Core Business Logic  

---

## Executive Summary

The CBU (Client Business Unit) Model is a **well-designed specification DSL** that defines the business model for client onboarding. It's the central artifact that directs all KYC/onboarding activity.

**What it defines:**
- **Attribute Groups (Chunks)** — Required/optional data organized by category
- **State Machine** — Lifecycle states and valid transitions
- **Roles** — Entity roles with cardinality constraints (min/max)
- **Preconditions** — What attributes must exist before state transitions

**Verdict:** Architecture is sound. A few integration gaps need addressing.

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────────┐
│ CBU Model DSL (cbu_model_dsl/ebnf.rs)                              │
│                                                                    │
│ (cbu-model                                                         │
│   :id "CBU.GENERIC" :version "1.0"                                │
│   (attributes                                                      │
│     (group :name "core"                                           │
│       :required [@attr("LEGAL_NAME"), @attr("JURISDICTION")]      │
│       :optional [@attr("LEI")]))                                  │
│   (states                                                          │
│     :initial "Proposed" :final ["Closed"]                         │
│     (state "Proposed") (state "Active") (state "Closed"))         │
│   (transitions                                                     │
│     (-> "Proposed" "Active"                                       │
│         :verb "cbu.approve"                                       │
│         :chunks ["core"]                                          │
│         :preconditions [@attr("APPROVAL_STATUS")]))               │
│   (roles                                                           │
│     (role "BeneficialOwner" :min 1 :max 10)))                     │
└────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌────────────────────────────────────────────────────────────────────┐
│ CbuModelParser (forth_engine/cbu_model_parser.rs)                  │
│                                                                    │
│ Uses NomDslParser to parse S-expressions → CbuModel AST           │
└────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌────────────────────────────────────────────────────────────────────┐
│ CbuModel AST (cbu_model_dsl/ast.rs)                                │
│                                                                    │
│ struct CbuModel {                                                  │
│   id, version, description, applies_to,                           │
│   attributes: CbuAttributesSpec { groups: [CbuAttributeGroup] },  │
│   states: CbuStateMachine { initial, finals, states, transitions },│
│   roles: Vec<CbuRoleSpec>                                         │
│ }                                                                  │
└────────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ RuntimeEnv       │ │ CbuModelService  │ │ CrudExecutor     │
│                  │ │                  │ │                  │
│ - cbu_model      │ │ - validate_model │ │ - execute_all_   │
│ - cbu_state      │ │ - save_model     │ │   with_env()     │
│ - is_valid_      │ │ - load_model_by_ │ │ - state machine  │
│   transition()   │ │   id()           │ │   validation     │
│ - check_         │ │                  │ │                  │
│   preconditions()│ │                  │ │                  │
└──────────────────┘ └──────────────────┘ └──────────────────┘
```

---

## Strengths

### 1. Clean Separation of Concerns

| Component | Responsibility |
|-----------|----------------|
| `ebnf.rs` | Grammar specification (documentation) |
| `cbu_model_parser.rs` | Parse DSL → AST |
| `ast.rs` | Pure data structures, no DB logic |
| `service.rs` | Validation against dictionary, persistence |
| `env.rs` | Runtime state machine enforcement |

### 2. Rich State Machine Model

```rust
// State machine validation at runtime
impl RuntimeEnv {
    pub fn is_valid_transition(&self, to_state: &str) -> bool {
        match (&self.cbu_model, &self.cbu_state) {
            (Some(model), Some(from_state)) => {
                model.states.is_valid_transition(from_state, to_state)
            }
            _ => true, // No model = no validation (permissive)
        }
    }
    
    pub fn check_transition_preconditions(&self, to_state: &str) -> Vec<String> {
        // Returns missing attributes that block the transition
    }
}
```

### 3. Chunk-Based Attribute Organization

Groups attributes logically for progressive disclosure:
- `core` — Legal name, jurisdiction, entity type
- `contact` — Primary/secondary contact info
- `ubo` — Beneficial ownership data
- `kyc` — KYC verification status

Transitions specify which chunks must be complete:
```clojure
(-> "Proposed" "PendingKYC"
    :verb "cbu.submit"
    :chunks ["core", "contact"]  ; These groups must be filled
    :preconditions [@attr("LEGAL_NAME")])
```

### 4. Role Cardinality Constraints

```clojure
(roles
  (role "BeneficialOwner" :min 1 :max 10)  ; At least 1, max 10
  (role "AuthorizedSignatory" :min 1 :max 5)
  (role "PrimaryContact" :min 1 :max 1))   ; Exactly 1
```

```rust
impl CbuRoleSpec {
    pub fn is_satisfied(&self, count: u32) -> bool {
        count >= self.min && self.max.is_none_or(|max| count <= max)
    }
}
```

### 5. Dictionary Validation

CbuModelService validates that all referenced attributes exist in the dictionary and have CBU as a valid sink.

---

## Issues & Gaps

### Issue 1: Model Loading Not Automatic in RuntimeEnv

**Current:** Model must be explicitly loaded by caller:
```rust
// In mod.rs execute_sheet_with_db():
if sheet.domain == "cbu" || sheet.content.contains("cbu.") {
    let model_service = CbuModelService::new(pool.clone());
    match model_service.load_model_by_id("CBU.GENERIC").await {
        Ok(Some(model)) => env.set_cbu_model(model),
        // ...
    }
}
```

**Problem:** Fragile — relies on string matching `"cbu."` in content.

**Solution:** Make model loading automatic based on domain or explicit DSL instruction.

### Issue 2: State Transitions Not Enforced by Words

**Current:** Words emit CRUD without checking state validity:
```rust
// words.rs - cbu_finalize
pub fn cbu_finalize(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    // MISSING: Check if transition to FINALIZED is valid from current state
    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU".to_string(),
        // ...
    }));
    Ok(())
}
```

**Should be:**
```rust
pub fn cbu_finalize(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // Check transition is valid
    if !env.is_valid_transition("Finalized") {
        return Err(EngineError::InvalidTransition {
            from: env.get_cbu_state().unwrap_or("unknown").to_string(),
            to: "Finalized".to_string(),
        });
    }
    
    // Check preconditions
    let missing = env.check_transition_preconditions("Finalized");
    if !missing.is_empty() {
        return Err(EngineError::MissingPreconditions(missing));
    }
    
    // Update state
    env.set_cbu_state("Finalized".to_string());
    
    // Emit CRUD
    env.push_crud(...);
    Ok(())
}
```

### Issue 3: CrudExecutor State Validation is Placeholder

**Current:** `execute_all_with_env` exists but state validation is incomplete:
```rust
// crud_executor.rs
pub async fn execute_all_with_env(
    &self,
    statements: &[CrudStatement],
    env: &mut RuntimeEnv,
) -> Result<Vec<CrudExecutionResult>> {
    // Has some model-aware logic but not comprehensive
}
```

### Issue 4: Chunk Validation Not Integrated

The model defines which chunks must be complete for transitions, but this isn't enforced:
```rust
// RuntimeEnv has:
pub current_chunks: Vec<String>,

// But no method like:
pub fn validate_chunks_complete(&self, chunk_names: &[String]) -> Vec<String> {
    // Check all required attrs in these chunks are present
}
```

### Issue 5: Role Validation Not Implemented

Model defines role cardinality but there's no runtime enforcement:
```rust
// Should have something like:
pub fn validate_role_requirements(&self) -> Vec<RoleViolation> {
    // Check current entity counts against role specs
}
```

### Issue 6: applies_to Field Unused

Model specifies which entity types it applies to:
```clojure
:applies-to ["Fund", "SPV", "Corporation"]
```

But this isn't validated when creating CBUs.

---

## Refactoring Recommendations

### P1: Add State Machine Enforcement to Words

Add validation to transition-triggering words:

```rust
// New helper macro or function
fn validate_transition(env: &RuntimeEnv, target_state: &str) -> Result<(), EngineError> {
    if !env.is_valid_transition(target_state) {
        return Err(EngineError::InvalidTransition {
            from: env.get_cbu_state().map(|s| s.to_string()),
            to: target_state.to_string(),
            verb: env.get_current_transition().map(|s| s.to_string()),
        });
    }
    
    let missing = env.check_transition_preconditions(target_state);
    if !missing.is_empty() {
        return Err(EngineError::MissingPreconditions(missing));
    }
    
    Ok(())
}

// Use in words:
pub fn cbu_submit(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    validate_transition(env, "PendingKYC")?;
    env.set_cbu_state("PendingKYC".to_string());
    // ... emit CRUD
}
```

### P1: Add Chunk Validation to RuntimeEnv

```rust
impl RuntimeEnv {
    /// Check if all required attributes in chunks are present
    pub fn validate_chunks(&self, chunk_names: &[String]) -> Result<(), Vec<String>> {
        let model = self.cbu_model.as_ref().ok_or_else(|| vec!["No model loaded".to_string()])?;
        
        let mut missing = Vec::new();
        for chunk_name in chunk_names {
            if let Some(chunk) = model.attributes.get_group(chunk_name) {
                for required in &chunk.required {
                    if !self.attribute_cache.contains_key(&AttributeId(required.clone())) {
                        missing.push(format!("{}:{}", chunk_name, required));
                    }
                }
            }
        }
        
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}
```

### P2: Auto-Load Model Based on Domain

```rust
impl RuntimeEnv {
    pub async fn load_model_for_domain(&mut self, domain: &str, pool: &PgPool) -> Result<()> {
        let model_id = match domain {
            "cbu" => "CBU.GENERIC",
            "kyc" => "CBU.GENERIC", // KYC uses same CBU model
            _ => return Ok(()), // No model for other domains
        };
        
        let service = CbuModelService::new(pool.clone());
        if let Some(model) = service.load_model_by_id(model_id).await? {
            self.set_cbu_model(model);
        }
        
        Ok(())
    }
}
```

### P2: Add Role Validation

```rust
impl RuntimeEnv {
    pub fn validate_roles(&self) -> Vec<RoleViolation> {
        let model = match &self.cbu_model {
            Some(m) => m,
            None => return vec![],
        };
        
        let mut violations = Vec::new();
        
        // Would need to query entity counts from DB or cache
        // For now, placeholder
        for role in &model.roles {
            let count = self.get_role_count(&role.name);
            if !role.is_satisfied(count) {
                violations.push(RoleViolation {
                    role: role.name.clone(),
                    required_min: role.min,
                    required_max: role.max,
                    actual: count,
                });
            }
        }
        
        violations
    }
}
```

### P3: Add Error Variants

```rust
// errors.rs - add:
#[error("Invalid state transition from '{from}' to '{to}'")]
InvalidTransition {
    from: Option<String>,
    to: String,
    verb: Option<String>,
},

#[error("Missing preconditions for transition: {0:?}")]
MissingPreconditions(Vec<String>),

#[error("Incomplete chunks: {0:?}")]
IncompleteChunks(Vec<String>),

#[error("Role constraint violation: {0:?}")]
RoleViolation(Vec<RoleViolation>),
```

### P3: Validate applies_to

```rust
// In CbuModelService or CrudExecutor
fn validate_entity_type_allowed(&self, entity_type: &str, model: &CbuModel) -> Result<()> {
    if model.applies_to.is_empty() {
        return Ok(()); // No restriction
    }
    
    if !model.applies_to.iter().any(|t| t.eq_ignore_ascii_case(entity_type)) {
        return Err(anyhow!(
            "Entity type '{}' not allowed by model. Allowed: {:?}",
            entity_type, model.applies_to
        ));
    }
    
    Ok(())
}
```

---

## Summary

| Aspect | Status |
|--------|--------|
| DSL Grammar | ✅ Well-defined in EBNF |
| Parser | ✅ Clean NOM-based |
| AST | ✅ Rich, type-safe structures |
| State Machine Definition | ✅ Comprehensive |
| Attribute Chunks | ✅ Well-organized |
| Role Constraints | ✅ Defined in model |
| Dictionary Validation | ✅ Implemented in service |
| **Runtime State Enforcement** | ⚠️ Partial — not in words |
| **Chunk Validation** | ⚠️ Defined but not enforced |
| **Role Validation** | ❌ Not implemented |
| **applies_to Validation** | ❌ Not implemented |

**Overall:** The CBU Model is architecturally sound. Main work is **connecting the validation logic to the execution path** — making sure words check the model before emitting CRUD.

---

## Claude Code Instructions

Priority order:

1. **Add error variants** to `forth_engine/errors.rs`
2. **Add `validate_transition()` helper** in a new `forth_engine/validation.rs`
3. **Update transition words** (`cbu_submit`, `cbu_approve`, `kyc_complete`, etc.) to call validation
4. **Add `validate_chunks()` to RuntimeEnv**
5. **Integrate chunk validation** into transition words

This makes the CBU Model a **live enforcement mechanism**, not just documentation.
