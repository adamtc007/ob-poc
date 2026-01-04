# Implementation Plan: Verb Contract Enhancements (Phases 1 & 2)

## Goal

Enhance the existing verb sync infrastructure to provide:
1. **Full compiled verb definitions** stored in database (not just metadata)
2. **Canonical hashing** for reproducible change detection
3. **Execution audit trail** linking executions to specific verb configurations

This builds on existing `VerbSyncService` and `dsl_verbs` rather than creating parallel systems.

---

## Phase 1: Enhance dsl_verbs with Compiled Contract Storage

### 1.1 Database Migration

**File:** `migrations/20260104_verb_contract_columns.sql`

Add columns to existing `dsl_verbs` table:

```sql
-- Add compiled contract storage columns
ALTER TABLE "ob-poc".dsl_verbs
ADD COLUMN IF NOT EXISTS compiled_json JSONB,
ADD COLUMN IF NOT EXISTS effective_config_json JSONB,
ADD COLUMN IF NOT EXISTS diagnostics_json JSONB DEFAULT '{"errors":[],"warnings":[]}',
ADD COLUMN IF NOT EXISTS compiled_hash BYTEA;

-- Add index for compiled_hash lookups
CREATE INDEX IF NOT EXISTS ix_dsl_verbs_compiled_hash 
  ON "ob-poc".dsl_verbs (compiled_hash) 
  WHERE compiled_hash IS NOT NULL;

COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_json IS 
  'Full RuntimeVerb serialized as JSON - the complete compiled contract';
COMMENT ON COLUMN "ob-poc".dsl_verbs.effective_config_json IS 
  'Expanded configuration with all defaults applied';
COMMENT ON COLUMN "ob-poc".dsl_verbs.diagnostics_json IS 
  'Compilation diagnostics (errors, warnings) for this verb';
COMMENT ON COLUMN "ob-poc".dsl_verbs.compiled_hash IS 
  'SHA256 of canonical compiled_json for integrity verification';
```

### 1.2 Rust Types for Contract Storage

**File:** `rust/src/session/verb_contract.rs` (new file)

```rust
use serde::{Deserialize, Serialize};

/// Diagnostics emitted during verb compilation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerbDiagnostics {
    pub errors: Vec<VerbDiagnostic>,
    pub warnings: Vec<VerbDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbDiagnostic {
    pub code: String,           // e.g., "MISSING_LOOKUP_CONFIG"
    pub message: String,
    pub path: Option<String>,   // e.g., "args[2].lookup"
    pub hint: Option<String>,
}

impl VerbDiagnostics {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    
    pub fn add_error(&mut self, code: &str, message: &str) {
        self.errors.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: None,
            hint: None,
        });
    }
    
    pub fn add_warning(&mut self, code: &str, message: &str) {
        self.warnings.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: None,
            hint: None,
        });
    }
}
```

### 1.3 Canonical Hashing Module

**File:** `rust/src/session/canonical_hash.rs` (new file)

```rust
use sha2::{Digest, Sha256};
use serde_json::Value as JsonValue;

/// Compute SHA256 of canonicalized JSON (sorted keys, deterministic)
pub fn canonical_json_hash(value: &JsonValue) -> [u8; 32] {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).expect("json->bytes");
    sha256(&bytes)
}

/// Normalize JSON for deterministic hashing
/// - Object keys sorted alphabetically
/// - Arrays preserve order
/// - Nulls, bools, numbers, strings unchanged
fn canonicalize_json(v: &JsonValue) -> JsonValue {
    match v {
        JsonValue::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            let mut sorted = serde_json::Map::new();
            for k in keys {
                if let Some(child) = map.get(k) {
                    sorted.insert(k.clone(), canonicalize_json(child));
                }
            }
            JsonValue::Object(sorted)
        }
        JsonValue::Array(arr) => {
            JsonValue::Array(arr.iter().map(canonicalize_json).collect())
        }
        other => other.clone(),
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Convert hash bytes to hex string for display/storage
pub fn hash_to_hex(hash: &[u8; 32]) -> String {
    hex::encode(hash)
}

/// Parse hex string back to hash bytes
pub fn hex_to_hash(hex_str: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(hex_str).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_canonical_hash_key_order_independence() {
        let a = json!({"z": 1, "a": 2, "m": 3});
        let b = json!({"a": 2, "m": 3, "z": 1});
        
        assert_eq!(canonical_json_hash(&a), canonical_json_hash(&b));
    }

    #[test]
    fn test_canonical_hash_nested() {
        let a = json!({"outer": {"z": 1, "a": 2}});
        let b = json!({"outer": {"a": 2, "z": 1}});
        
        assert_eq!(canonical_json_hash(&a), canonical_json_hash(&b));
    }
}
```

### 1.4 Enhance VerbSyncService

**File:** `rust/src/session/verb_sync.rs` (modify existing)

Add to existing `VerbSyncService`:

```rust
use crate::session::canonical_hash::{canonical_json_hash, hash_to_hex};
use crate::session::verb_contract::VerbDiagnostics;

impl VerbSyncService {
    /// Compile a RuntimeVerb to its full JSON representation with diagnostics
    fn compile_verb_contract(
        &self,
        verb: &RuntimeVerb,
    ) -> (serde_json::Value, serde_json::Value, VerbDiagnostics) {
        let mut diagnostics = VerbDiagnostics::default();
        
        // Serialize full RuntimeVerb as compiled_json
        let compiled_json = serde_json::to_value(verb)
            .unwrap_or_else(|e| {
                diagnostics.add_error("SERIALIZE_FAILED", &e.to_string());
                serde_json::Value::Null
            });
        
        // Build effective_config with expanded defaults
        let effective_config = self.build_effective_config(verb, &mut diagnostics);
        
        // Validate and add warnings
        self.validate_verb_contract(verb, &mut diagnostics);
        
        (compiled_json, effective_config, diagnostics)
    }
    
    /// Build effective configuration with all defaults expanded
    fn build_effective_config(
        &self,
        verb: &RuntimeVerb,
        diagnostics: &mut VerbDiagnostics,
    ) -> serde_json::Value {
        // For now, effective_config mirrors compiled_json
        // Future: expand defaults, resolve references, etc.
        serde_json::to_value(verb).unwrap_or(serde_json::Value::Null)
    }
    
    /// Validate verb contract and add diagnostics
    fn validate_verb_contract(
        &self,
        verb: &RuntimeVerb,
        diagnostics: &mut VerbDiagnostics,
    ) {
        // Check for common issues
        for (i, arg) in verb.args.iter().enumerate() {
            // Warn if lookup configured but entity_type missing
            if let Some(ref lookup) = arg.lookup {
                if lookup.entity_type.is_none() && lookup.table.is_some() {
                    diagnostics.add_warning(
                        "LOOKUP_MISSING_ENTITY_TYPE",
                        &format!("Arg '{}' has lookup table but no entity_type", arg.name),
                    );
                }
            }
            
            // Warn if required arg has default (contradiction)
            if arg.required && arg.default.is_some() {
                diagnostics.add_warning(
                    "REQUIRED_WITH_DEFAULT",
                    &format!("Arg '{}' is required but has default value", arg.name),
                );
            }
        }
        
        // Check lifecycle consistency
        if let Some(ref lifecycle) = verb.lifecycle {
            if lifecycle.transitions_to.is_some() && lifecycle.entity_arg.is_none() {
                diagnostics.add_error(
                    "LIFECYCLE_MISSING_ENTITY_ARG",
                    "Lifecycle has transitions_to but no entity_arg specified",
                );
            }
        }
    }
    
    /// Enhanced upsert that stores compiled contract
    async fn upsert_verb_with_contract(
        &self,
        verb: &RuntimeVerb,
        yaml_hash: &str,
    ) -> Result<(), sqlx::Error> {
        let (compiled_json, effective_config, diagnostics) = 
            self.compile_verb_contract(verb);
        
        let compiled_hash = canonical_json_hash(&compiled_json);
        let diagnostics_json = serde_json::to_value(&diagnostics)
            .unwrap_or(serde_json::json!({"errors":[],"warnings":[]}));
        
        sqlx::query(r#"
            INSERT INTO "ob-poc".dsl_verbs (
                domain, verb_name, description, behavior, category,
                yaml_hash, compiled_json, effective_config_json, 
                diagnostics_json, compiled_hash,
                produces_type, produces_subtype, consumes,
                lifecycle_entity_arg, requires_states, transitions_to
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10,
                $11, $12, $13,
                $14, $15, $16
            )
            ON CONFLICT (domain, verb_name) DO UPDATE SET
                description = EXCLUDED.description,
                behavior = EXCLUDED.behavior,
                category = EXCLUDED.category,
                yaml_hash = EXCLUDED.yaml_hash,
                compiled_json = EXCLUDED.compiled_json,
                effective_config_json = EXCLUDED.effective_config_json,
                diagnostics_json = EXCLUDED.diagnostics_json,
                compiled_hash = EXCLUDED.compiled_hash,
                produces_type = EXCLUDED.produces_type,
                produces_subtype = EXCLUDED.produces_subtype,
                consumes = EXCLUDED.consumes,
                lifecycle_entity_arg = EXCLUDED.lifecycle_entity_arg,
                requires_states = EXCLUDED.requires_states,
                transitions_to = EXCLUDED.transitions_to,
                updated_at = now()
        "#)
        .bind(&verb.domain)
        .bind(&verb.verb)
        .bind(&verb.description)
        .bind(behavior_to_string(&verb.behavior))
        .bind(infer_category(&verb.domain))
        .bind(yaml_hash)
        .bind(&compiled_json)
        .bind(&effective_config)
        .bind(&diagnostics_json)
        .bind(&compiled_hash[..])
        .bind(verb.produces.as_ref().map(|p| &p.produces_type))
        .bind(verb.produces.as_ref().and_then(|p| p.subtype.as_ref()))
        .bind(serde_json::to_value(&verb.consumes).ok())
        .bind(verb.lifecycle.as_ref().and_then(|l| l.entity_arg.as_ref()))
        .bind(verb.lifecycle.as_ref().map(|l| &l.requires_states))
        .bind(verb.lifecycle.as_ref().and_then(|l| l.transitions_to.as_ref()))
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

### 1.5 Add xtask Commands

**File:** `rust/xtask/src/main.rs` (add commands)

```rust
// Add to existing xtask
Commands::Verbs { action } => match action {
    VerbsAction::Compile => verbs_compile().await,
    VerbsAction::Show { verb_name } => verbs_show(&verb_name).await,
    VerbsAction::Diagnostics => verbs_diagnostics().await,
}

async fn verbs_compile() -> Result<()> {
    // Load registry, sync to DB, print summary
    let registry = RuntimeVerbRegistry::load()?;
    let sync_service = VerbSyncService::new(pool);
    let result = sync_service.sync_all(&registry).await?;
    
    println!("Verb compilation complete:");
    println!("  Added:     {}", result.added);
    println!("  Updated:   {}", result.updated);
    println!("  Unchanged: {}", result.unchanged);
    println!("  Errors:    {}", result.error_count);
    println!("  Warnings:  {}", result.warning_count);
    
    Ok(())
}

async fn verbs_show(verb_name: &str) -> Result<()> {
    // Query dsl_verbs for specific verb, print compiled_json + diagnostics
    let row = sqlx::query(r#"
        SELECT compiled_json, effective_config_json, diagnostics_json
        FROM "ob-poc".dsl_verbs
        WHERE full_name = $1
    "#)
    .bind(verb_name)
    .fetch_optional(&pool)
    .await?;
    
    if let Some(row) = row {
        println!("=== {} ===", verb_name);
        println!("\n--- Compiled Contract ---");
        println!("{}", serde_json::to_string_pretty(&row.compiled_json)?);
        println!("\n--- Diagnostics ---");
        println!("{}", serde_json::to_string_pretty(&row.diagnostics_json)?);
    } else {
        println!("Verb not found: {}", verb_name);
    }
    
    Ok(())
}

async fn verbs_diagnostics() -> Result<()> {
    // Query all verbs with errors/warnings, summarize
    let rows = sqlx::query(r#"
        SELECT full_name, diagnostics_json
        FROM "ob-poc".dsl_verbs
        WHERE diagnostics_json->'errors' != '[]'::jsonb
           OR diagnostics_json->'warnings' != '[]'::jsonb
        ORDER BY full_name
    "#)
    .fetch_all(&pool)
    .await?;
    
    for row in rows {
        let diag: VerbDiagnostics = serde_json::from_value(row.diagnostics_json)?;
        println!("{}: {} errors, {} warnings", 
            row.full_name, diag.errors.len(), diag.warnings.len());
        for e in &diag.errors {
            println!("  ERROR: [{}] {}", e.code, e.message);
        }
        for w in &diag.warnings {
            println!("  WARN:  [{}] {}", w.code, w.message);
        }
    }
    
    Ok(())
}
```

---

## Phase 2: Execution Audit Trail with Verb Hash

### 2.1 Database Migration

**File:** `migrations/20260104_execution_verb_hash.sql`

```sql
-- Add verb configuration tracking to execution log
ALTER TABLE "ob-poc".dsl_execution_log
ADD COLUMN IF NOT EXISTS verb_hashes JSONB DEFAULT '{}';

COMMENT ON COLUMN "ob-poc".dsl_execution_log.verb_hashes IS 
  'Map of verb_name -> yaml_hash for all verbs used in this execution';

-- Index for finding executions by verb hash
CREATE INDEX IF NOT EXISTS ix_dsl_execution_log_verb_hashes 
  ON "ob-poc".dsl_execution_log 
  USING GIN (verb_hashes);
```

### 2.2 Enhance Execution Logging

**File:** `rust/src/dsl_v2/executor.rs` (modify)

Add verb hash collection during execution:

```rust
impl DslExecutor {
    /// Collect verb hashes for all verbs used in a plan
    fn collect_verb_hashes(&self, plan: &ExecutionPlan) -> HashMap<String, String> {
        let mut hashes = HashMap::new();
        
        for step in &plan.steps {
            let full_name = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
            if !hashes.contains_key(&full_name) {
                if let Some(verb) = self.registry.get(&step.verb_call.domain, &step.verb_call.verb) {
                    // Get hash from registry or compute on the fly
                    let hash = self.compute_verb_hash(&verb);
                    hashes.insert(full_name, hash);
                }
            }
        }
        
        hashes
    }
    
    /// Execute plan with verb hash tracking
    pub async fn execute_plan_with_audit(
        &self,
        plan: &ExecutionPlan,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResults, ExecutionError> {
        let verb_hashes = self.collect_verb_hashes(plan);
        
        // Execute as normal
        let results = self.execute_plan(plan, ctx).await?;
        
        // Log with verb hashes
        self.log_execution_with_verb_hashes(
            ctx.execution_id,
            &verb_hashes,
            &results,
        ).await?;
        
        Ok(results)
    }
    
    async fn log_execution_with_verb_hashes(
        &self,
        execution_id: Uuid,
        verb_hashes: &HashMap<String, String>,
        results: &ExecutionResults,
    ) -> Result<(), sqlx::Error> {
        let verb_hashes_json = serde_json::to_value(verb_hashes)?;
        
        sqlx::query(r#"
            UPDATE "ob-poc".dsl_execution_log
            SET verb_hashes = $2
            WHERE execution_id = $1
        "#)
        .bind(execution_id)
        .bind(&verb_hashes_json)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

### 2.3 Query Helper for Audit

**File:** `rust/src/database/execution_repository.rs` (add method)

```rust
impl ExecutionRepository {
    /// Find all executions that used a specific verb configuration
    pub async fn find_executions_by_verb_hash(
        &self,
        verb_name: &str,
        yaml_hash: &str,
    ) -> Result<Vec<ExecutionLogEntry>, sqlx::Error> {
        sqlx::query_as(r#"
            SELECT *
            FROM "ob-poc".dsl_execution_log
            WHERE verb_hashes->>$1 = $2
            ORDER BY started_at DESC
        "#)
        .bind(verb_name)
        .bind(yaml_hash)
        .fetch_all(&self.pool)
        .await
    }
    
    /// Get verb configuration used in a specific execution
    pub async fn get_execution_verb_config(
        &self,
        execution_id: Uuid,
        verb_name: &str,
    ) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let row = sqlx::query(r#"
            SELECT dv.compiled_json
            FROM "ob-poc".dsl_execution_log el
            JOIN "ob-poc".dsl_verbs dv ON dv.yaml_hash = el.verb_hashes->>$2
            WHERE el.execution_id = $1
        "#)
        .bind(execution_id)
        .bind(verb_name)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|r| r.get("compiled_json")))
    }
}
```

---

## File Summary

### New Files
| File | Purpose |
|------|---------|
| `migrations/20260104_verb_contract_columns.sql` | Add compiled_json, effective_config, diagnostics columns |
| `migrations/20260104_execution_verb_hash.sql` | Add verb_hashes to execution log |
| `rust/src/session/verb_contract.rs` | VerbDiagnostics types |
| `rust/src/session/canonical_hash.rs` | Canonical JSON hashing |

### Modified Files
| File | Changes |
|------|---------|
| `rust/src/session/mod.rs` | Export new modules |
| `rust/src/session/verb_sync.rs` | Add compile_verb_contract, upsert_verb_with_contract |
| `rust/src/dsl_v2/executor.rs` | Add verb hash collection, execute_plan_with_audit |
| `rust/src/database/execution_repository.rs` | Add find_executions_by_verb_hash |
| `rust/xtask/src/main.rs` | Add verbs compile/show/diagnostics commands |

---

## Testing Checklist

### Phase 1
- [ ] Migration applies cleanly to existing database
- [ ] `VerbSyncService.sync_all()` populates compiled_json for all verbs
- [ ] Canonical hash is stable (same YAML content = same hash regardless of key order)
- [ ] Diagnostics capture validation warnings/errors
- [ ] `cargo x verbs compile` shows summary
- [ ] `cargo x verbs show cbu.ensure` displays compiled contract

### Phase 2
- [ ] Migration adds verb_hashes column
- [ ] Execution logs include verb_hashes for all used verbs
- [ ] Can query "which executions used this verb config?"
- [ ] Can retrieve "what verb config was used in this execution?"

---

## Rollback Plan

Both migrations are additive (new columns, not modifications). Rollback:

```sql
-- Phase 1 rollback
ALTER TABLE "ob-poc".dsl_verbs 
DROP COLUMN IF EXISTS compiled_json,
DROP COLUMN IF EXISTS effective_config_json,
DROP COLUMN IF EXISTS diagnostics_json,
DROP COLUMN IF EXISTS compiled_hash;

-- Phase 2 rollback
ALTER TABLE "ob-poc".dsl_execution_log
DROP COLUMN IF EXISTS verb_hashes;
```

---

## Success Criteria

After implementation:

1. **Reproducibility**: Given an execution_id, can retrieve exact verb configurations used
2. **Auditability**: Can answer "which executions used verb config X?"
3. **Diagnostics**: Verb compilation issues surfaced via `cargo x verbs diagnostics`
4. **Minimal disruption**: Existing execution path unchanged, new columns are additive

---

## Notes for Implementation

1. **Start with migrations** - get schema in place first
2. **Canonical hashing module** - small, testable, foundational
3. **Enhance VerbSyncService incrementally** - don't break existing sync
4. **Phase 2 is independent** - can be done after Phase 1 is stable
5. **xtask commands are nice-to-have** - core value is in data storage
