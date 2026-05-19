# DSL Verb Authoring Pattern (Phase 3 draft)

> **Status:** Draft — produced alongside CR A0. Refined through A-CRs and L-CRs.
> Reviewed by Adam at: post-A0 (initial), post-A6 (after Op removal), post-M-CRs (final).

---

## Purpose

This document defines the standard pattern for authoring DSL verbs in ob-poc. It specifies the bidirectional contract between YAML declaration and Rust implementation. Every verb conforms to this pattern. The wiring check (§4) enforces conformance at startup.

---

## The bidirectional contract

```
YAML declaration (what the verb IS)
    ↓ loaded at startup by ConfigLoader
    ↓ validates structural conformance
    ↓ builds RuntimeVerb in runtime_registry
    ↕ wiring check: every declaration has an impl; every impl has a declaration
Rust implementation (what the verb DOES)
    ↓ registers SemOsVerbOp or CRUD config at startup
    ↓ dispatch: execute_verb() → runtime_registry → impl
    ↓ executes within caller-supplied TransactionScope
```

---

## §1 — YAML declaration

### Required fields

```yaml
verbs:
  <domain>:
    <action>:
      description: "Human-readable description"
      behavior: plugin | crud           # execution strategy
      invocation_phrases:
        - "natural language phrase"     # ≥1 phrase for embedding
      metadata:
        tier: reference | intent | projection | diagnostics | composite | governance
        source_of_truth: <string>       # who owns the canonical state
        scope: cbu | entity | global    # what entity this verb operates on
        noun: <string>                  # display noun (used in narration)
      args:
        - name: <arg-name>
          type: uuid | string | boolean | decimal | string_list
          required: true | false
          description: "arg purpose"
          maps_to: <db_column>          # for crud verbs
```

### Optional fields

```yaml
      flavour: instance_adding | attribute_mutating | instance_removing | read_only | state_transition
      lifecycle:
        writes_tables:
          - schema.table_name
      produces:
        type: <entity_type>
        resolved: true | false
      consumes:
        - arg: <arg-name>
          type: <entity_type>
          required: true | false
      phase_tags: [kyc, trading, custody, onboarding]  # used for DAG phase grouping
      state_effect: transition | preserving             # for catalogue v1.3 gate
      consequence:
        baseline: benign | reviewable | requires_confirmation | requires_explicit_authorisation
```

### Naming conventions

- Domain: lowercase hyphenated (`kyc-case`, `entity-workstream`, `trading-profile`)
- Action: lowercase hyphenated verb (`create`, `update-status`, `list-by-cbu`)
- FQN: `<domain>.<action>` (e.g., `kyc-case.update-status`)
- Args: lowercase hyphenated (`case-id`, `new-status`, `profile-id`)

---

## §2 — Rust implementation

### Plugin verb (behavior: plugin)

```rust
use sem_os_postgres::ops::SemOsVerbOp;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

pub struct MyDomainCreate;

#[async_trait]
impl SemOsVerbOp for MyDomainCreate {
    fn fqn(&self) -> &str {
        "my-domain.create"  // MUST match YAML domain.action exactly
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // 1. Extract args
        let name = json_extract_string(args, ctx, "name")?;

        // 2. FSM precondition check (if state-transition verb)
        // Use conditional UPDATE with AND status = $current to detect concurrent modification.
        // See kyc_case.rs UpdateStatus for the pattern.

        // 3. Execute via scope.executor() (stays within caller's transaction)
        let id: Uuid = sqlx::query_scalar(r#"INSERT INTO ... RETURNING id"#)
            .bind(&name)
            .fetch_one(scope.executor())
            .await?;

        // 4. Return typed outcome
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}
```

**Rules:**
- `fqn()` must match the YAML `<domain>.<action>` exactly
- All DB writes use `scope.executor()` — never open a new transaction inside `execute()`
- Never call `scope.pool()` for writes — that bypasses the active transaction (E8)
- Arg extraction via `json_extract_*` helpers (never raw `.as_str()` without error handling)
- Return `VerbExecutionOutcome::Uuid(id)` for entity-creating verbs; `::Record(json)` for reads; `::Void` for mutations with no return value

### CRUD verb (behavior: crud)

```yaml
      behavior: crud
      crud:
        operation: insert | update | delete | upsert | select | list_by_fk
        table: <table_name>
        schema: <schema_name>
        returning: <primary_key_column>   # for insert/upsert
        conflict_keys:                    # for upsert (ON CONFLICT)
          - <column_name>
        key: <pk_column>                  # for update/delete (WHERE pk = $key_arg)
```

CRUD verbs require no Rust implementation — they dispatch through `GenericCrudExecutor` using the `crud:` config in YAML. Use `behavior: crud` for simple table operations with no business logic.

---

## §3 — Registration

### Plugin verbs: two registration locations

Verbs implemented in `sem_os_postgres::ops::*` register in:
```rust
// sem_os_postgres/src/ops/mod.rs
pub fn build_registry() -> SemOsVerbOpRegistry {
    let mut r = SemOsVerbOpRegistry::new();
    r.register(Box::new(my_domain::MyDomainCreate));
    // ...
    r
}
```

Verbs that require ob-poc-specific internals register in:
```rust
// rust/src/domain_ops/mod.rs
pub fn extend_registry(r: &mut SemOsVerbOpRegistry) {
    r.register(Box::new(my_internal_op::MyInternalOp));
    // ...
}
```

### CRUD verbs: automatic

CRUD verbs are registered automatically by the YAML loader when `behavior: crud` is declared. No Rust registration needed.

---

## §4 — Wiring check

At startup, after both YAML loading and Rust registration complete, the wiring check verifies:

1. **Declaration without implementation:** every `behavior: plugin` verb in loaded YAMLs has a `SemOsVerbOp` in the registry with matching `fqn()`. Missing implementations fail startup.
2. **Implementation without declaration:** every registered `SemOsVerbOp::fqn()` has a corresponding YAML declaration. Orphan implementations fail startup.
3. **CRUD completeness:** every `behavior: crud` verb has a valid `crud:` block with required fields present.

Mismatch output example:
```
ERROR: Verb registration mismatch — startup aborted
  UNDECLARED IMPLS (registered but no YAML): my-domain.orphan-verb
  UNIMPLEMENTED VERBS (YAML but no impl): new-feature.create
```

The wiring check is the startup safety gate that makes the bidirectional contract mechanically enforced.

**Current status:** The startup check (`cargo test -p ob-poc --lib -- test_plugin_verb_coverage`) tests coverage but is not yet a startup abort. The wiring check as specified above is CR L2 work.

---

## §5 — Testing conventions

### Unit test (no DB)

```rust
#[test]
fn arg_extraction_rejects_missing_required() {
    let args = serde_json::json!({});
    let result = json_extract_uuid(&args, &ExecutionContext::new(), "case-id");
    assert!(result.is_err());
}
```

### Integration test (with DB, via execute_plan)

```rust
#[cfg(feature = "database")]
async fn test_my_verb_creates_entity(pool: &PgPool) {
    let dsl = r#"(my-domain.create :name "Test Entity")"#;
    let ast = parse_program(dsl).unwrap();
    let plan = compile(&ast).unwrap();
    let executor = DslExecutor::new(pool.clone()).with_sem_os_ops(build_registry());
    let mut ctx = ExecutionContext::new();
    executor.execute_plan(&plan, &mut ctx).await.unwrap();
    // assert DB state
}
```

### FSM transition test

For state-transition verbs, test the concurrent-modification path:

```rust
async fn test_concurrent_transition_returns_error() {
    // Start case in INTAKE, transition to UNDER_REVIEW in one task,
    // attempt same transition in a concurrent task — second must fail.
    // Retry with actual state should succeed.
}
```

---

## §6 — Evolution

### Adding an argument

1. Add arg to YAML (mark `required: false` with a `default` if backward-compatible)
2. Update Rust impl to extract the new arg with `json_extract_*_opt`
3. Existing DSL programs remain valid (optional args have no impact)

### Adding a required argument

This breaks existing DSL programs. Treat as a new verb version or negotiate migration:
1. Add a new verb FQN (`my-domain.create-v2`) with the required arg
2. Deprecate the old FQN (document in YAML, emit deprecation diagnostic)
3. Migrate callers, then delete old FQN in a coordinated CR

### FSM evolution

When a state machine gains a new state:
1. Add the state to the DAG taxonomy YAML (`rust/config/sem_os_seeds/dag_taxonomies/`)
2. Update `is_valid_transition` / `is_valid_deal_status_transition` in the plugin op
3. Add the new transition in YAML (`state_effect: transition`, `transition_args`)
4. Verify wiring check still passes

### Deprecating a verb

1. Add `deprecated: true` to YAML (loader emits `DeprecatedVerb` diagnostic on use)
2. Keep the Rust impl until all callers removed
3. After callsite migration: remove YAML declaration + Rust impl in the same PR
4. Wiring check confirms clean state

---

## Appendix: dispatch call chain (post-α)

```
User utterance
  → HybridVerbSearcher → verb FQN
  → execute_verb_in_scope(vc: &VerbCall, ctx, scope: &mut dyn TransactionScope)
      → runtime_registry().get(domain, verb) → RuntimeVerb
      → match RuntimeVerb.behavior {
            Plugin(fqn) → SemOsVerbOpRegistry::execute(fqn, args, ctx, scope)
                         → SemOsVerbOp::execute(args, ctx, scope)  ← your impl
            Crud(config) → GenericCrudExecutor::execute_in_tx(scope.transaction(), verb, args)
        }
  → ExecutionResult
```

The key invariant: **all writes go through `scope.executor()`** (the active transaction's connection). The TransactionScope is owned by the caller (`execute_plan_atomic_in_scope` or the sequencer's runbook scope); the verb implementation is a tenant. Opening a new transaction inside `execute()` is wrong — it would bypass the caller's atomicity boundary.
