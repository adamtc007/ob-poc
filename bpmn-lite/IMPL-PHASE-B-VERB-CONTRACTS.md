# IMPL-PHASE-B: Verb Contracts + Lints (v0.1)

**Prerequisites:** Phase A complete (91 tests, authoring pipeline live)
**Goal:** Add verb contract definitions and compile-time lints that validate workflow DTO against verb capabilities.
**Outcome:** Workflows can only reference verbs that exist, use flags those verbs actually write, and route errors those verbs actually raise.

---

## A) What This Phase Builds

### A1. VerbContract struct + registry

A verb contract declares what a verb reads, writes, raises, and correlates. The contract is loaded from YAML files and stored in a registry keyed by `task_type`.

```rust
// bpmn-lite-core/src/authoring/contracts.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbContract {
    pub task_type: String,
    pub description: String,
    #[serde(default)]
    pub reads_flags: Vec<String>,
    #[serde(default)]
    pub writes_flags: Vec<String>,
    #[serde(default)]
    pub may_raise_errors: Vec<String>,
    #[serde(default)]
    pub correlation: CorrelationContract,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorrelationContract {
    #[serde(default)]
    pub produces: Vec<String>,
    #[serde(default)]
    pub consumes: Vec<String>,
}

/// Registry of verb contracts, keyed by task_type.
#[derive(Debug, Clone, Default)]
pub struct ContractRegistry {
    contracts: HashMap<String, VerbContract>,
}

impl ContractRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, contract: VerbContract) {
        self.contracts.insert(contract.task_type.clone(), contract);
    }

    pub fn get(&self, task_type: &str) -> Option<&VerbContract> {
        self.contracts.get(task_type)
    }

    /// Load all .yaml files from a directory, extracting the `contract:` section.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        // reads each .yaml, deserializes VerbContract from `contract:` key
        // skips files without a contract section (incremental adoption)
    }

    /// Load from a list of inline YAML strings (for testing).
    pub fn load_from_strings(yamls: &[&str]) -> Result<Self> { ... }
}
```

### A2. Lint engine

Lints run AFTER `validate_dto()` succeeds and BEFORE `dto_to_ir()`. They take the validated DTO + a ContractRegistry and produce warnings and errors.

```rust
// bpmn-lite-core/src/authoring/lints.rs

#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub level: LintLevel,
    pub rule: String,        // e.g., "L1-flag-provenance"
    pub message: String,
    pub node_id: Option<String>,
    pub edge_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LintLevel { Warning, Error }

/// Run all contract lints against a validated DTO.
pub fn lint_contracts(
    dto: &WorkflowGraphDto,
    registry: &ContractRegistry,
    strict: bool,  // if true, missing contracts are errors not warnings
) -> Vec<LintDiagnostic> { ... }
```

### A3. Lint rules

| Rule | ID | Level | Check |
|------|-----|-------|-------|
| **Flag provenance** | L1 | Error | Every `condition.flag` on an edge must appear in `writes_flags` of a ServiceTask that is *upstream* of the gateway node (reachable via BFS from Start without crossing the gateway). |
| **Error code validity** | L2 | Error | Every `on_error.code` (except `"*"`) on an edge must appear in `may_raise_errors` of the ServiceTask that the edge's `from` references. |
| **Correlation provenance** | L3 | Warning | Every `corr_key_source` on a MessageWait/HumanWait/RaceArm should match `correlation.produces` from an upstream ServiceTask (or be a known workflow input). Warning because correlation provenance is hard to prove statically. |
| **Missing contract** | L4 | Warning/Error | Every ServiceTask's `task_type` should have a registered contract. Warning in normal mode, Error in strict mode. |
| **Unused writes** | L5 | Warning | A verb declares `writes_flags: [orch_x]` but no downstream gateway condition references `orch_x`. Informational — helps catch dead flags. |

### A4. Upstream reachability for L1

Flag provenance requires knowing which tasks are upstream of a gateway. Algorithm:

1. Build adjacency from edges (forward direction: from → to)
2. Build reverse adjacency (to → from)
3. For each gateway with conditional edges, BFS backward from the gateway node
4. Collect all ServiceTask nodes reachable in reverse
5. Union their `writes_flags` from the registry
6. Check that each condition flag is in this union

This is a DTO-level graph traversal, NOT IR-level. Works on node IDs and edges.

---

## B) Contract YAML Format

Verb contract files can be standalone or embedded in existing verb definition files:

### B1. Standalone contract file

```yaml
# contracts/kyc.screen-sanctions.yaml
task_type: kyc.screen-sanctions
description: "Screen entity against sanctions lists"

reads_flags: []
writes_flags:
  - orch_sanctions_clear
may_raise_errors:
  - SANCTIONS_HIT
  - SANCTIONS_TIMEOUT
correlation:
  produces: [screening_id]
  consumes: [case_id]
```

### B2. Embedded in verb definition (future ob-poc integration)

```yaml
# verbs/kyc.screen-sanctions.yaml
task_type: kyc.screen-sanctions
description: "Screen entity against sanctions lists"
tier: 1

# ... other verb metadata ...

contract:
  reads_flags: []
  writes_flags:
    - orch_sanctions_clear
  may_raise_errors:
    - SANCTIONS_HIT
    - SANCTIONS_TIMEOUT
  correlation:
    produces: [screening_id]
    consumes: [case_id]
```

Phase B supports both formats. The loader extracts the contract regardless of whether it's top-level or nested under `contract:`.

---

## C) File Ownership

| File | Purpose |
|------|---------|
| `bpmn-lite-core/src/authoring/contracts.rs` | VerbContract, CorrelationContract, ContractRegistry |
| `bpmn-lite-core/src/authoring/lints.rs` | lint_contracts(), LintDiagnostic, LintLevel, rules L1–L5 |
| `bpmn-lite-core/src/authoring/mod.rs` | Add `pub mod contracts; pub mod lints;` |

No changes to engine.rs, dto.rs, validate.rs, dto_to_ir.rs, or yaml.rs.

---

## D) Integration with compile pipeline

```rust
// Usage pattern (not a new engine method — caller composes):
let dto = parse_workflow_yaml(yaml_str)?;
let validation_errors = validate_dto(&dto);
if !validation_errors.is_empty() { return Err(...); }

// NEW: contract linting
let registry = ContractRegistry::load_from_dir(Path::new("contracts/"))?;
let lint_results = lint_contracts(&dto, &registry, strict);
let errors: Vec<_> = lint_results.iter().filter(|d| d.level == LintLevel::Error).collect();
if !errors.is_empty() { return Err(...); }
// warnings are reported but don't block compilation

let ir = dto_to_ir(&dto)?;
// ... existing verifier → lowering → bytecode
```

The lint step is **optional** — `compile_from_yaml()` and `compile_from_dto()` continue to work without a registry. Lints are an additional safety layer for production publishing.

---

## E) Tests

### T-LINT-1: Flag provenance — valid

```yaml
# Workflow: task_a writes orch_high_risk → XOR checks orch_high_risk
# Contract: task_a writes_flags: [orch_high_risk]
# Expected: no L1 errors
```

### T-LINT-2: Flag provenance — violation

```yaml
# Workflow: XOR checks orch_high_risk but NO upstream task writes it
# Expected: L1 error on the gateway edge condition
```

### T-LINT-3: Error code validity — valid

```yaml
# Workflow: screen_sanctions has on_error SANCTIONS_HIT
# Contract: screen_sanctions may_raise_errors: [SANCTIONS_HIT]
# Expected: no L2 errors
```

### T-LINT-4: Error code validity — unknown code

```yaml
# Workflow: screen_sanctions has on_error UNKNOWN_CODE
# Contract: screen_sanctions may_raise_errors: [SANCTIONS_HIT]
# Expected: L2 error
```

### T-LINT-5: Error code catch-all — always valid

```yaml
# Workflow: screen_sanctions has on_error "*"
# Expected: no L2 error (catch-all is always valid)
```

### T-LINT-6: Missing contract — warning mode

```yaml
# Workflow: task_type "unknown.verb" with no registered contract
# strict=false
# Expected: L4 warning (not error)
```

### T-LINT-7: Missing contract — strict mode

```yaml
# Same as T-LINT-6 but strict=true
# Expected: L4 error
```

### T-LINT-8: Correlation provenance — warning

```yaml
# Workflow: MessageWait with corr_key_source "screening_id"
# No upstream task produces screening_id
# Expected: L3 warning
```

### T-LINT-9: Unused writes — warning

```yaml
# Contract: task_a writes orch_x but no condition anywhere references orch_x
# Expected: L5 warning
```

### T-LINT-10: Multiple lints in one workflow

```yaml
# Workflow with both L1 and L2 violations + one L5 warning
# Expected: all three reported, correct rule IDs and node references
```

---

## F) Verification Gate

```bash
cargo test -p bpmn-lite-core 2>&1
```

All existing (91 tests) must pass. Plus:
```bash
cargo test -p bpmn-lite-core -- t_lint 2>&1
```
Expected: `test result: ok. 10 passed`

```bash
cargo check --features postgres 2>&1
```
Must compile clean.

---

## G) Done Signal

```
PHASE B COMPLETE — Verb contracts + lints operational.
10/10 T-LINT tests passing. Total: 101 tests.
Next: Phase C (BPMN export + ir_to_dto).
```
