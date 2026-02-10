# CLAUDE CODE SESSION: AUTHORING PHASE D — CLI + PUBLISH LIFECYCLE (v0.1)

**Read IMPL-PHASE-D-CLI-PUBLISH.md first.** It has the full design.

## Prerequisites check

```bash
cargo test -p bpmn-lite-core 2>&1 | tail -5
# Must show 111+ passed (Phases A + B + C)
```

## Non-negotiable constraints

1. **Publish gate is a pure function** in bpmn-lite-core — no CLI dependency. CLI commands call into it.
2. **TemplateStore is a trait** — MemoryTemplateStore for tests, PostgresTemplateStore feature-gated.
3. **State transitions are enforced** — Published cannot go back to Draft. Retired cannot go back.
4. **Bytecode version is SHA-256** of the compiled bytecode bytes. Deterministic.
5. **CLI commands live in ob-poc xtask** — NOT in bpmn-lite-core. But bpmn-lite-core has all the logic.

## This phase has TWO parts

### Part 1: bpmn-lite-core library (registry + publish gate + stores)

Files:
- `src/authoring/registry.rs` — WorkflowTemplate, TemplateState, SourceFormat, TemplateStore trait, MemoryTemplateStore
- `src/authoring/publish.rs` — publish_workflow(), PublishResult, PublishError
- `src/authoring/store_postgres_templates.rs` — PostgresTemplateStore (#[cfg(feature = "postgres")])
- `migrations/013_create_workflow_templates.sql`
- `src/authoring/mod.rs` — add module declarations

### Part 2: ob-poc xtask CLI (if xtask exists in workspace)

Check first:
```bash
ls -la xtask/ 2>/dev/null || echo "No xtask directory"
find . -name "xtask" -type d 2>/dev/null
grep -r "xtask" Cargo.toml 2>/dev/null
```

If xtask doesn't exist yet, **implement Part 1 only**. The CLI can be added later as a separate task. Part 1 is the critical deliverable — it contains all the logic.

## Execution plan

### Step 1: registry.rs

```rust
// WorkflowTemplate struct (all fields from spec)
// SourceFormat enum: Yaml, BpmnImport, Agent
// TemplateState enum: Draft, Published, Retired
// TemplateStore trait (async_trait)
// MemoryTemplateStore implementation
```

Key: MemoryTemplateStore must enforce state transition rules. `set_state()` returns error for invalid transitions.

### Step 2: publish.rs

```rust
// publish_workflow() — the full pipeline
// PublishResult, PublishError, PublishStage
```

This function composes everything from Phases A, B, C:
- parse_workflow_yaml (Phase A)
- validate_dto (Phase A)
- lint_contracts (Phase B) — optional, skip if no registry
- dto_to_ir (Phase A)
- verify (existing)
- lower (existing)
- compute bytecode hash (new — sha2)
- extract task_manifest (new — iterate DTO nodes)
- dto_to_bpmn_xml (Phase C) — optional

### Step 3: Cargo.toml

Add `sha2` dependency for bytecode hashing:
```toml
sha2 = "0.10"
```

### Step 4: Postgres template store

Feature-gated behind `#[cfg(feature = "postgres")]`. Runtime queries only (sqlx::query, not query!).

### Step 5: Migration SQL

`migrations/013_create_workflow_templates.sql` — see IMPL doc for schema.

### Step 6: mod.rs

Add module declarations.

### Step 7: Tests — 10 T-PUB tests

## CRITICAL: publish_workflow signature

```rust
pub fn publish_workflow(
    yaml_str: &str,
    registry: Option<&ContractRegistry>,  // None = skip linting
    authored_by: &str,
    strict_contracts: bool,
    export_bpmn: bool,
) -> Result<PublishResult, PublishError>
```

Note: `registry` is Option — if None, skip the lint step entirely. This preserves backward compatibility and allows publishing without contracts during early development.

## CRITICAL: Bytecode hash computation

```rust
use sha2::{Sha256, Digest};

fn compute_bytecode_hash(program: &CompiledProgram) -> [u8; 32] {
    let mut hasher = Sha256::new();
    // Hash the bytecode instructions
    // Use bincode or a deterministic serialization of the instruction vector
    // The simplest approach: serialize with serde_json (deterministic for our types)
    let bytes = serde_json::to_vec(&program.instructions).expect("instructions serialize");
    hasher.update(&bytes);
    hasher.finalize().into()
}
```

Check if CompiledProgram and Instr already implement Serialize:
```bash
grep -n "Serialize" bpmn-lite-core/src/types.rs | head -20
```

If not, the hash can be computed from the debug representation or a custom serialization. The key requirement is determinism — same instructions always produce the same hash.

## CRITICAL: MemoryTemplateStore state transitions

```rust
impl TemplateStore for MemoryTemplateStore {
    async fn set_state(&self, key: &str, version: u32, new_state: TemplateState) -> Result<()> {
        let mut store = self.templates.write().unwrap();
        let template = store.get_mut(&(key.to_string(), version))
            .ok_or_else(|| anyhow!("Template not found: {}@{}", key, version))?;

        match (template.state, new_state) {
            (TemplateState::Draft, TemplateState::Published) => {
                template.state = TemplateState::Published;
                template.published_at = Some(now_ms());
                Ok(())
            }
            (TemplateState::Published, TemplateState::Retired) => {
                template.state = TemplateState::Retired;
                Ok(())
            }
            (from, to) => Err(anyhow!("Invalid state transition: {:?} → {:?}", from, to)),
        }
    }
}
```

## CRITICAL: Do NOT modify engine.rs

publish_workflow is in authoring/publish.rs. It calls into engine methods (compile_from_yaml or the underlying functions) but does NOT add new methods to BpmnLiteEngine.

Check how to access compile pipeline components:
```bash
grep -n "pub fn compile\|pub fn verify\|pub fn lower\|pub fn dto_to_ir\|pub fn validate_dto" bpmn-lite-core/src/
```

publish_workflow calls these as free functions or through existing engine methods.

## Progress gates

- Step 1 (registry.rs compiles) → 20% → IMMEDIATELY proceed to Step 2
- Step 2 (publish.rs compiles) → 45% → IMMEDIATELY proceed to Step 3
- Step 3 (Cargo.toml deps) → 50% → IMMEDIATELY proceed to Step 4
- Step 4 (postgres store compiles) → 65% → IMMEDIATELY proceed to Step 5
- Step 5 (migration SQL) → 70% → IMMEDIATELY proceed to Step 6
- Step 6 (mod.rs) → 75% → IMMEDIATELY add tests
- Tests added → 90% → Run `cargo test -p bpmn-lite-core`
- All green → 100% → Print DONE signal

## Verification

```bash
cargo test -p bpmn-lite-core 2>&1
```
All existing (111) must pass. Plus:
```bash
cargo test -p bpmn-lite-core -- t_pub 2>&1
```
Expected: `test result: ok. 9 passed, 1 ignored`

```bash
cargo check --features postgres 2>&1
```
Must compile clean (includes PostgresTemplateStore).

## Done signal

```
PHASE D COMPLETE — CLI + publish lifecycle operational.
9/9 T-PUB tests passing (+1 ignored Postgres). Total: 120+ tests.
Authoring pipeline complete (Phases A-D).
```
