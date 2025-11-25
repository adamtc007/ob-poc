# Fix: Context Propagation for Idempotent KYC Sessions

**Created:** 2025-11-25  
**Status:** READY TO IMPLEMENT  
**Priority:** P0 — Blocking test  
**Issue:** `test_kyc_session_idempotent_execution` fails due to missing `cbu_id` context  

---

## Problem Statement

The DSL session:
```clojure
(cbu.ensure :cbu-name "Test Fund" :jurisdiction "LU")
(risk.assess-cbu :methodology "FACTOR_WEIGHTED")  ;; ← No :cbu-id!
```

Fails because:
1. `cbu.ensure` creates the CBU but doesn't capture the returned UUID into `RuntimeEnv.cbu_id`
2. `risk.assess-cbu` doesn't inject `env.cbu_id` into its CRUD values
3. `risk_assessments` table has `CHECK (cbu_id IS NOT NULL OR entity_id IS NOT NULL)`
4. INSERT fails with constraint violation

**Expected behavior:** Once `cbu.ensure` runs, all subsequent words in the session should automatically have access to that CBU's ID without requiring explicit `:cbu-id` in every call.

---

## Solution Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ DSL Source                                                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ (cbu.ensure :cbu-name "Test Fund")                                         │
│      │                                                                      │
│      ▼ Emits DataUpsert with capture_result: Some("cbu_id")                │
│                                                                             │
│ (risk.assess-cbu :methodology "FACTOR_WEIGHTED")                           │
│      │                                                                      │
│      ▼ Word injects env.cbu_id into values before emitting CRUD            │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ CrudExecutor                                                                │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. Execute CBU UPSERT → returns cbu_id UUID                                │
│ 2. See capture_result = "cbu_id"                                           │
│ 3. Set env.cbu_id = Some(returned_uuid)                                    │
│                                                                             │
│ 4. Execute RISK_ASSESSMENT_CBU                                             │
│ 5. values already contains "cbu-id" (injected by word)                     │
│ 6. INSERT succeeds                                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: Extend CrudStatement

### 1.1 Add `capture_result` to DataUpsert

**File:** `rust/src/forth_engine/value.rs`

```rust
#[derive(Debug, Clone)]
pub struct DataUpsert {
    pub asset: String,
    pub values: HashMap<String, Value>,
    pub conflict_keys: Vec<String>,
    /// If set, capture the returned ID into RuntimeEnv under this key
    /// Supported keys: "cbu_id", "entity_id", "investigation_id"
    pub capture_result: Option<String>,
}
```

### 1.2 Add `capture_result` to DataCreate (for non-upsert creates)

```rust
#[derive(Debug, Clone)]
pub struct DataCreate {
    pub asset: String,
    pub values: HashMap<String, Value>,
    /// If set, capture the returned ID into RuntimeEnv under this key
    pub capture_result: Option<String>,
}
```

---

## Part 2: Update Words to Capture Context

### 2.1 `cbu.ensure` — Capture CBU ID

**File:** `rust/src/forth_engine/words.rs`

```rust
pub fn cbu_ensure(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "CBU".to_string(),
        values,
        conflict_keys: vec!["cbu-name".to_string()],
        capture_result: Some("cbu_id".to_string()),  // NEW
    }));
    
    Ok(())
}
```

### 2.2 `cbu.create` — Also Capture CBU ID

```rust
pub fn cbu_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU".to_string(),
        values,
        capture_result: Some("cbu_id".to_string()),  // NEW
    }));
    
    Ok(())
}
```

### 2.3 `investigation.create` — Capture Investigation ID

```rust
pub fn investigation_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    // Inject cbu_id from context if not provided
    inject_cbu_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "INVESTIGATION".to_string(),
        values,
        capture_result: Some("investigation_id".to_string()),  // NEW
    }));
    
    Ok(())
}
```

### 2.4 `entity.create-*` — Capture Entity ID

```rust
pub fn entity_create_limited_company(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let values = args_to_crud_values(args);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "LIMITED_COMPANY".to_string(),
        values,
        capture_result: Some("entity_id".to_string()),  // NEW
    }));
    
    Ok(())
}

// Same for entity_create_proper_person, entity_create_partnership, entity_create_trust
```

---

## Part 3: Helper Function for Context Injection

### 3.1 Add Helper Functions

**File:** `rust/src/forth_engine/words.rs`

```rust
/// Inject cbu_id from RuntimeEnv into values if not already present
fn inject_cbu_id_if_missing(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    if !values.contains_key("cbu-id") {
        if let Some(cbu_id) = &env.cbu_id {
            values.insert("cbu-id".to_string(), Value::Str(cbu_id.to_string()));
        }
    }
}

/// Inject entity_id from RuntimeEnv into values if not already present
fn inject_entity_id_if_missing(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    if !values.contains_key("entity-id") {
        if let Some(entity_id) = &env.entity_id {
            values.insert("entity-id".to_string(), Value::Str(entity_id.to_string()));
        }
    }
}

/// Inject investigation_id from RuntimeEnv into values if not already present  
fn inject_investigation_id_if_missing(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    if !values.contains_key("investigation-id") {
        if let Some(inv_id) = &env.investigation_id {
            values.insert("investigation-id".to_string(), Value::Str(inv_id.to_string()));
        }
    }
}

/// Inject all relevant context IDs
fn inject_context_ids(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    inject_cbu_id_if_missing(values, env);
    inject_entity_id_if_missing(values, env);
    inject_investigation_id_if_missing(values, env);
}
```

---

## Part 4: Update Context-Dependent Words

### 4.1 Words That Need CBU Context

| Word | Inject |
|------|--------|
| `investigation.create` | `cbu_id` |
| `investigation.update-status` | `cbu_id`, `investigation_id` |
| `risk.assess-cbu` | `cbu_id` |
| `risk.set-rating` | `cbu_id` |
| `risk.add-flag` | `cbu_id` |
| `decision.record` | `cbu_id`, `investigation_id` |
| `decision.add-condition` | (needs decision_id — different pattern) |
| `monitoring.setup` | `cbu_id` |
| `monitoring.schedule-review` | `cbu_id` |
| `cbu.attach-entity` | `cbu_id` |

### 4.2 Update Each Word

```rust
pub fn risk_assess_cbu(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    // Inject context
    inject_cbu_id_if_missing(&mut values, env);
    inject_investigation_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "RISK_ASSESSMENT_CBU".to_string(),
        values,
        capture_result: None,
    }));
    
    Ok(())
}

pub fn risk_set_rating(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    inject_cbu_id_if_missing(&mut values, env);
    inject_investigation_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "RISK_RATING".to_string(),
        values,
        capture_result: None,
    }));
    
    Ok(())
}

pub fn risk_add_flag(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    inject_cbu_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "RISK_FLAG".to_string(),
        values,
        capture_result: None,
    }));
    
    Ok(())
}

pub fn decision_record(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    inject_cbu_id_if_missing(&mut values, env);
    inject_investigation_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DECISION".to_string(),
        values,
        capture_result: Some("decision_id".to_string()),  // Capture for conditions
    }));
    
    Ok(())
}

pub fn monitoring_setup(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    inject_cbu_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "MONITORING_SETUP".to_string(),
        values,
        conflict_keys: vec!["cbu-id".to_string()],  // One monitoring per CBU
        capture_result: None,
    }));
    
    Ok(())
}

pub fn monitoring_schedule_review(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    inject_cbu_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "SCHEDULED_REVIEW".to_string(),
        values,
        capture_result: None,
    }));
    
    Ok(())
}

pub fn investigation_update_status(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    let mut values = args_to_crud_values(args);
    
    inject_cbu_id_if_missing(&mut values, env);
    inject_investigation_id_if_missing(&mut values, env);
    
    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "INVESTIGATION".to_string(),
        values,
        where_clause: HashMap::new(),  // Will use investigation_id
    }));
    
    Ok(())
}
```

---

## Part 5: Update RuntimeEnv

### 5.1 Add `investigation_id` and `decision_id` Fields

**File:** `rust/src/forth_engine/env.rs`

```rust
pub struct RuntimeEnv {
    // ... existing fields ...
    
    /// Current CBU ID for this execution context
    pub cbu_id: Option<Uuid>,

    /// Current entity ID for this execution context
    pub entity_id: Option<Uuid>,
    
    /// Current investigation ID for this execution context (NEW)
    pub investigation_id: Option<Uuid>,
    
    /// Current decision ID for this execution context (NEW)
    pub decision_id: Option<Uuid>,
    
    // ... rest of fields ...
}
```

### 5.2 Update Constructor

```rust
impl RuntimeEnv {
    pub fn new(request_id: OnboardingRequestId) -> Self {
        Self {
            request_id,
            #[cfg(feature = "database")]
            pool: None,
            cbu_id: None,
            entity_id: None,
            investigation_id: None,  // NEW
            decision_id: None,       // NEW
            // ... rest ...
        }
    }
}
```

---

## Part 6: Update CrudExecutor to Capture Results

### 6.1 Modify Execute Method Signature

**File:** `rust/src/database/crud_executor.rs`

The executor needs access to `RuntimeEnv` to set captured IDs:

```rust
impl CrudExecutor {
    /// Execute a CRUD statement and optionally capture result into env
    pub async fn execute_with_context(
        &self,
        stmt: &CrudStatement,
        env: &mut RuntimeEnv,
    ) -> Result<CrudExecutionResult> {
        let result = self.execute(stmt).await?;
        
        // Capture result if requested
        self.capture_result_if_needed(stmt, &result, env);
        
        Ok(result)
    }
    
    fn capture_result_if_needed(
        &self,
        stmt: &CrudStatement,
        result: &CrudExecutionResult,
        env: &mut RuntimeEnv,
    ) {
        let capture_key = match stmt {
            CrudStatement::DataCreate(c) => c.capture_result.as_ref(),
            CrudStatement::DataUpsert(u) => u.capture_result.as_ref(),
            _ => None,
        };
        
        if let Some(key) = capture_key {
            if let Some(id) = result.generated_id {
                match key.as_str() {
                    "cbu_id" => {
                        env.cbu_id = Some(id);
                        info!("Captured cbu_id into context: {}", id);
                    }
                    "entity_id" => {
                        env.entity_id = Some(id);
                        info!("Captured entity_id into context: {}", id);
                    }
                    "investigation_id" => {
                        env.investigation_id = Some(id);
                        info!("Captured investigation_id into context: {}", id);
                    }
                    "decision_id" => {
                        env.decision_id = Some(id);
                        info!("Captured decision_id into context: {}", id);
                    }
                    _ => {
                        warn!("Unknown capture key: {}", key);
                    }
                }
            }
        }
    }
}
```

### 6.2 Update Orchestrator to Use New Method

**File:** `rust/src/dsl_source/orchestrator.rs` (or wherever CRUD execution happens)

```rust
// Change from:
for stmt in env.pending_crud.drain(..) {
    let result = executor.execute(&stmt).await?;
    results.push(result);
}

// To:
for stmt in env.pending_crud.drain(..) {
    let result = executor.execute_with_context(&stmt, &mut env).await?;
    results.push(result);
}
```

---

## Part 7: Default capture_result for Existing Code

To avoid breaking existing code, set default `capture_result: None` in constructors:

```rust
impl DataCreate {
    pub fn new(asset: String, values: HashMap<String, Value>) -> Self {
        Self {
            asset,
            values,
            capture_result: None,
        }
    }
}

impl DataUpsert {
    pub fn new(asset: String, values: HashMap<String, Value>, conflict_keys: Vec<String>) -> Self {
        Self {
            asset,
            values,
            conflict_keys,
            capture_result: None,
        }
    }
}
```

---

## Part 8: Full List of Words to Update

### Words That Capture Results

| Word | Captures | Into |
|------|----------|------|
| `cbu.create` | Generated UUID | `env.cbu_id` |
| `cbu.ensure` | Generated/Found UUID | `env.cbu_id` |
| `entity.create-proper-person` | Generated UUID | `env.entity_id` |
| `entity.create-limited-company` | Generated UUID | `env.entity_id` |
| `entity.create-partnership` | Generated UUID | `env.entity_id` |
| `entity.create-trust` | Generated UUID | `env.entity_id` |
| `investigation.create` | Generated UUID | `env.investigation_id` |
| `decision.record` | Generated UUID | `env.decision_id` |

### Words That Inject Context

| Word | Injects |
|------|---------|
| `investigation.create` | `cbu_id` |
| `investigation.update-status` | `cbu_id`, `investigation_id` |
| `investigation.assign` | `investigation_id` |
| `investigation.complete` | `investigation_id` |
| `risk.assess-cbu` | `cbu_id`, `investigation_id` |
| `risk.assess-entity` | `entity_id`, `investigation_id` |
| `risk.set-rating` | `cbu_id`, `investigation_id` |
| `risk.add-flag` | `cbu_id` |
| `decision.record` | `cbu_id`, `investigation_id` |
| `decision.add-condition` | `decision_id` |
| `monitoring.setup` | `cbu_id` |
| `monitoring.schedule-review` | `cbu_id` |
| `monitoring.record-event` | `cbu_id` |
| `cbu.attach-entity` | `cbu_id` |
| `cbu.detach-entity` | `cbu_id` |
| `cbu.list-entities` | `cbu_id` |
| `screening.pep` | `entity_id`, `investigation_id` |
| `screening.sanctions` | `entity_id`, `investigation_id` |
| `screening.adverse-media` | `entity_id`, `investigation_id` |

---

## Part 9: Verification

After implementation:

```bash
# Run the idempotency test
cd rust
cargo test --features database test_kyc_session_idempotent_execution -- --nocapture
```

Expected output:
```
Run 1 counts: (1, 1, 1, 1, 1, 1)  # CBU, Investigation, Risk, Decision, Monitoring, Review
Run 2 counts: (1, 1, 1, 1, 1, 1)  # Same counts - idempotent!
test test_kyc_session_idempotent_execution ... ok
```

---

## Part 10: Files to Modify

| File | Changes |
|------|---------|
| `rust/src/forth_engine/value.rs` | Add `capture_result` to `DataCreate`, `DataUpsert` |
| `rust/src/forth_engine/env.rs` | Add `investigation_id`, `decision_id` fields |
| `rust/src/forth_engine/words.rs` | Add helper functions, update ~20 words |
| `rust/src/database/crud_executor.rs` | Add `execute_with_context`, `capture_result_if_needed` |
| `rust/src/dsl_source/orchestrator.rs` | Use `execute_with_context` |

---

## Summary

| Problem | Solution |
|---------|----------|
| `cbu.ensure` doesn't capture returned ID | Add `capture_result: Some("cbu_id")` |
| Subsequent words don't have `cbu_id` | Inject from `env.cbu_id` if not in args |
| `RuntimeEnv` missing investigation/decision IDs | Add new fields |
| `CrudExecutor` doesn't set context | Add `capture_result_if_needed` |

This fix enables the expected flow:
```clojure
(cbu.ensure :cbu-name "Test")       ;; → env.cbu_id = "abc-123"
(investigation.create ...)          ;; → uses env.cbu_id, sets env.investigation_id
(risk.assess-cbu :methodology ...) ;; → uses env.cbu_id, env.investigation_id
(decision.record :decision ...)    ;; → uses env.cbu_id, sets env.decision_id
(monitoring.setup ...)             ;; → uses env.cbu_id
```

All without explicit `:cbu-id` in every word call.
