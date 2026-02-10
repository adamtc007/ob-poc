# IMPL-PHASE-D: CLI + Publish Lifecycle (v0.1)

**Prerequisites:** Phases A, B, C complete (111 tests, full authoring pipeline + contracts + export)
**Goal:** Production publishing workflow — validate, lint, compile, pin, and register workflow templates.
**Outcome:** CLI commands for the full authoring lifecycle; Postgres-backed template registry; immutable published versions.

---

## A) What This Phase Builds

### A1. Template registry types (bpmn-lite-core)

```rust
// bpmn-lite-core/src/authoring/registry.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub template_key: String,
    pub template_version: u32,
    pub process_key: String,
    pub bytecode_version: [u8; 32],      // content hash, pins the compiled program
    pub dto_snapshot: WorkflowGraphDto,    // frozen DTO at publish time
    pub task_manifest: Vec<String>,        // all task_types referenced
    pub source_format: SourceFormat,
    pub state: TemplateState,
    pub authored_by: String,
    pub created_at: i64,                   // epoch ms
    pub published_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SourceFormat { Yaml, BpmnImport, Agent }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TemplateState { Draft, Published, Retired }
```

### A2. Publish gate (bpmn-lite-core)

```rust
// bpmn-lite-core/src/authoring/publish.rs

#[derive(Debug)]
pub struct PublishResult {
    pub template: WorkflowTemplate,
    pub warnings: Vec<LintDiagnostic>,
    pub bytecode_hash: [u8; 32],
}

#[derive(Debug)]
pub struct PublishError {
    pub stage: PublishStage,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub enum PublishStage {
    YamlParse,
    DtoValidation,
    ContractLint,
    IrConversion,
    IrVerification,
    Lowering,
    BpmnExport,  // optional, only if requested
}

/// Run the full publish pipeline: parse → validate → lint → compile → pin.
/// Returns a PublishResult with the frozen template and any warnings.
pub fn publish_workflow(
    yaml_str: &str,
    registry: &ContractRegistry,
    authored_by: &str,
    strict_contracts: bool,
    export_bpmn: bool,
) -> Result<PublishResult, PublishError> {
    // 1. Parse YAML → DTO
    // 2. Validate DTO (V1-V15)
    // 3. Lint contracts (L1-L5)
    // 4. If strict and lint errors → fail
    // 5. dto_to_ir()
    // 6. IR verifier
    // 7. Lowering → CompiledProgram
    // 8. Compute bytecode_version hash
    // 9. Extract task_manifest from DTO nodes
    // 10. (Optional) dto_to_bpmn_xml() — fail is non-fatal warning
    // 11. Build WorkflowTemplate with state=Draft
    // Return PublishResult
}
```

### A3. Template registry trait (bpmn-lite-core)

```rust
// bpmn-lite-core/src/authoring/registry.rs

#[async_trait]
pub trait TemplateStore: Send + Sync {
    /// Save or update a template.
    async fn save_template(&self, template: &WorkflowTemplate) -> Result<()>;

    /// Load a template by key + version.
    async fn load_template(&self, key: &str, version: u32) -> Result<Option<WorkflowTemplate>>;

    /// Load the latest version of a template.
    async fn load_latest(&self, key: &str) -> Result<Option<WorkflowTemplate>>;

    /// List all templates (optionally filtered by state).
    async fn list_templates(&self, state: Option<TemplateState>) -> Result<Vec<WorkflowTemplate>>;

    /// Transition template state: Draft → Published, Published → Retired.
    async fn set_state(&self, key: &str, version: u32, state: TemplateState) -> Result<()>;
}
```

### A4. Memory template store (bpmn-lite-core, for testing)

```rust
pub struct MemoryTemplateStore {
    templates: RwLock<HashMap<(String, u32), WorkflowTemplate>>,
}
```

### A5. Postgres template store (bpmn-lite-core, feature-gated)

```rust
// bpmn-lite-core/src/authoring/store_postgres_templates.rs
// Behind #[cfg(feature = "postgres")]

pub struct PostgresTemplateStore {
    pool: PgPool,
}
```

### A6. CLI commands (ob-poc xtask)

These are the developer-facing commands. They live in the ob-poc workspace, NOT in bpmn-lite-core.

| Command | Purpose |
|---------|---------|
| `cargo xtask validate-yaml <file>` | Parse + validate DTO (V1-V15) |
| `cargo xtask lint-yaml <file> [--contracts-dir <dir>] [--strict]` | Parse + validate + lint contracts |
| `cargo xtask compile-yaml <file>` | Full compile pipeline → print bytecode hash |
| `cargo xtask export-bpmn <file> [--output <out.bpmn>]` | Compile + export BPMN XML |
| `cargo xtask import-bpmn <file.bpmn> [--output <out.yaml>]` | Parse BPMN → IR → DTO → YAML |
| `cargo xtask publish <file> [--db-url <url>] [--strict] [--author <name>]` | Full publish gate + register |
| `cargo xtask list-templates [--db-url <url>] [--state <draft|published|retired>]` | List registered templates |
| `cargo xtask retire-template <key> <version> [--db-url <url>]` | Transition Published → Retired |

---

## B) Postgres Schema

### B1. Migration: create workflow_templates table

```sql
-- migrations/013_create_workflow_templates.sql

CREATE TABLE IF NOT EXISTS workflow_templates (
    template_key     TEXT        NOT NULL,
    template_version INTEGER     NOT NULL,
    process_key      TEXT        NOT NULL,
    bytecode_version BYTEA       NOT NULL,    -- [u8; 32]
    dto_snapshot     JSONB       NOT NULL,
    task_manifest    JSONB       NOT NULL,    -- Vec<String>
    source_format    TEXT        NOT NULL,    -- "yaml" | "bpmn_import" | "agent"
    state            TEXT        NOT NULL DEFAULT 'draft',  -- draft | published | retired
    authored_by      TEXT        NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at     TIMESTAMPTZ,

    PRIMARY KEY (template_key, template_version)
);

-- Index for listing by state
CREATE INDEX idx_templates_state ON workflow_templates (state);

-- Index for latest version lookup
CREATE INDEX idx_templates_key_version ON workflow_templates (template_key, template_version DESC);

-- Enforce immutability: published templates cannot be modified (application-level, but constraint helps)
-- Note: state can only transition draft→published→retired (application enforced)
```

### B2. State transition rules (application-enforced)

| From | To | Allowed |
|------|----|---------|
| Draft | Published | ✅ Sets published_at, pins bytecode_version |
| Draft | Draft | ✅ Re-publish overwrites (version bump recommended) |
| Published | Retired | ✅ No new instances, running continue |
| Published | Draft | ❌ Immutable after publish |
| Retired | Published | ❌ Cannot un-retire |
| Retired | Draft | ❌ Cannot un-retire |

---

## C) Publish Gate — Detailed Pipeline

```
YAML string
  │
  ├─ 1. parse_workflow_yaml() ─────── YamlParse error
  │
  ├─ 2. validate_dto() ───────────── DtoValidation error
  │
  ├─ 3. lint_contracts() ─────────── ContractLint error (strict) or warnings
  │
  ├─ 4. dto_to_ir() ──────────────── IrConversion error
  │
  ├─ 5. verify() ─────────────────── IrVerification error
  │
  ├─ 6. lower() ──────────────────── Lowering error
  │
  ├─ 7. compute bytecode_version ─── SHA-256 of serialized bytecode
  │
  ├─ 8. extract task_manifest ────── Collect unique task_types from DTO
  │
  ├─ 9. (optional) dto_to_bpmn_xml() ── BpmnExport warning if fails
  │
  └─ 10. Build WorkflowTemplate { state: Draft }
```

Bytecode version is computed as:
```rust
use sha2::{Sha256, Digest};
let mut hasher = Sha256::new();
hasher.update(&compiled.bytecode);  // serialized instruction bytes
let bytecode_version: [u8; 32] = hasher.finalize().into();
```

Note: This requires `sha2` crate. Alternative: reuse the existing `compute_hash` if it produces [u8; 32], but sha2 is more standard for content addressing.

---

## D) File Ownership

### bpmn-lite-core (library)

| File | Purpose |
|------|---------|
| `src/authoring/registry.rs` | WorkflowTemplate, TemplateState, SourceFormat, TemplateStore trait, MemoryTemplateStore |
| `src/authoring/publish.rs` | publish_workflow(), PublishResult, PublishError, PublishStage |
| `src/authoring/store_postgres_templates.rs` | PostgresTemplateStore (feature-gated) |
| `src/authoring/mod.rs` | Add `pub mod registry; pub mod publish;` + conditional postgres module |
| `migrations/013_create_workflow_templates.sql` | Postgres schema |

### ob-poc (workspace, xtask binary)

| File | Purpose |
|------|---------|
| `xtask/src/commands/validate_yaml.rs` | validate-yaml command |
| `xtask/src/commands/lint_yaml.rs` | lint-yaml command |
| `xtask/src/commands/compile_yaml.rs` | compile-yaml command |
| `xtask/src/commands/export_bpmn.rs` | export-bpmn command |
| `xtask/src/commands/import_bpmn.rs` | import-bpmn command |
| `xtask/src/commands/publish.rs` | publish command |
| `xtask/src/commands/list_templates.rs` | list-templates command |
| `xtask/src/commands/retire_template.rs` | retire-template command |

---

## E) Tests

### E1. bpmn-lite-core tests (publish gate + registry)

### T-PUB-1: Publish gate — happy path

```
Valid YAML with matching contracts → publish_workflow() succeeds
Assert: PublishResult has bytecode_hash, task_manifest, state=Draft
Assert: warnings list may be non-empty but no errors
```

### T-PUB-2: Publish gate — validation failure

```
Invalid YAML (duplicate node IDs) → publish_workflow() fails
Assert: PublishError.stage == DtoValidation
Assert: error message mentions duplicate IDs
```

### T-PUB-3: Publish gate — lint failure (strict)

```
Valid DTO but missing contracts, strict=true
Assert: PublishError.stage == ContractLint
```

### T-PUB-4: Publish gate — lint warnings (non-strict)

```
Valid DTO but missing contracts, strict=false
Assert: PublishResult succeeds with L4 warnings
```

### T-PUB-5: Bytecode version determinism

```
Same YAML → publish twice → same bytecode_version hash
Different YAML → different bytecode_version hash
```

### T-PUB-6: Task manifest extraction

```
DTO with 3 service tasks (2 unique task_types)
Assert: task_manifest has 2 entries (deduplicated)
```

### T-PUB-7: Template store — memory (CRUD)

```
Save template, load by key+version, load latest, list by state
Assert: all operations work correctly
```

### T-PUB-8: Template state transitions

```
Draft → Published: OK
Published → Retired: OK
Published → Draft: Error
Retired → Published: Error
```

### T-PUB-9: Template immutability

```
Publish a template (Draft → Published)
Attempt to save a modified version with same key+version
Assert: error (cannot modify published template)
```

### T-PUB-10: Postgres template store (ignored, needs DB)

```
Same as T-PUB-7 + T-PUB-8 against Postgres
#[ignore] — requires DATABASE_URL
```

---

## F) Verification Gate

```bash
cargo test -p bpmn-lite-core 2>&1
```

All existing (111 tests after Phases A-C) must pass. Plus:
```bash
cargo test -p bpmn-lite-core -- t_pub 2>&1
```
Expected: `test result: ok. 9 passed, 1 ignored`

```bash
cargo check --features postgres 2>&1
```
Must compile clean.

---

## G) CLI Testing (manual, not automated)

After bpmn-lite-core tests pass, verify CLI commands manually:

```bash
# In ob-poc workspace:
cargo xtask validate-yaml examples/kyc-onboarding.yaml
cargo xtask lint-yaml examples/kyc-onboarding.yaml --contracts-dir contracts/
cargo xtask compile-yaml examples/kyc-onboarding.yaml
cargo xtask export-bpmn examples/kyc-onboarding.yaml --output /tmp/kyc.bpmn
# Open /tmp/kyc.bpmn in Camunda Modeler to verify visual layout
```

---

## H) Done Signal

```
PHASE D COMPLETE — CLI + publish lifecycle operational.
9/9 T-PUB tests passing (+1 ignored Postgres). Total: 120+ tests.
Authoring pipeline complete (Phases A-D).
```

---

## I) Summary — Full Authoring Pipeline After Phase D

```
Author (Zed)                    Agent (LLM)
    │                               │
    ▼                               ▼
  YAML file                    generated YAML
    │                               │
    └──────────┬────────────────────┘
               ▼
    cargo xtask validate-yaml  ← V1-V15
               │
    cargo xtask lint-yaml      ← L1-L5 (contracts)
               │
    cargo xtask compile-yaml   ← IR → verify → lower → bytecode
               │
    cargo xtask export-bpmn    ← Camunda Modeler review
               │
    cargo xtask publish        ← pin bytecode_version, register template
               │
               ▼
    Template Registry (Postgres)
    ┌───────────────────────────────┐
    │ kyc.onboarding-ucits@v1      │
    │ state: Published              │
    │ bytecode_version: a3f2...     │
    │ dto_snapshot: { ... }         │
    │ task_manifest: [kyc.*, ...]   │
    └───────────────────────────────┘
               │
               ▼
    engine.start("kyc-onboarding-ucits", bytecode_version, payload, hash, corr)
```
