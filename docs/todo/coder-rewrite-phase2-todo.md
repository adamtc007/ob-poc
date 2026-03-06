# Coder Verb Resolution Rewrite — Claude Code Execution Plan (Phase 2)

**Reference:** `docs/UTTERANCE_PIPELINE_ANALYSIS_AND_ARCHITECTURE_v0.3.md` (Part II + Part III Phase 2)  
**Prerequisite:** Phase 1 (Sage Skeleton) GATE 5+ complete — `sage::pre_classify()` and `sage::outcome::OutcomeStep` types exist.  
**Objective:** Build a structured verb resolution path that the Coder uses when receiving an `OutcomeStep` from the Sage. Replaces embedding-based 1,123-verb search with deterministic plane → polarity → domain → action+params lookup. The existing `IntentPipeline` / `HybridVerbSearcher` path remains as fallback — this is a parallel path, not a replacement (yet).  
**Target:** Coder resolves the correct verb from a well-formed `OutcomeStep` ≥ 85% of the time (structured lookup, not fuzzy matching).

---

## Execution Protocol

- Each phase MUST complete fully before the GATE check.
- At each GATE: run the specified command. If it fails, fix before proceeding.
- At each GATE: print `CODER PHASE N COMPLETE — N% done` and the E-invariant.
- → IMMEDIATELY proceed to the next phase after a passing GATE.
- Do NOT stop between phases. Do NOT ask for confirmation.

**E-invariant (must hold at every GATE):**
`cargo check -p ob-poc 2>&1 | tail -1` shows no errors AND
`cargo test --lib -p ob-poc 2>&1 | tail -1` shows `test result: ok`.

The existing pipeline MUST remain fully functional. The Coder resolver is additive.

---

## Phase 1: Verb Metadata Index (15%)

**Goal:** Build a structured index over the verb registry that supports filtering by plane, polarity, domain, and action — the four dimensions the Coder needs.

### 1A: Classify every verb's polarity

**File:** `rust/src/sage/verb_index.rs` (new file, inside sage module)

Build a function that classifies each verb in the registry as Read or Write:

```rust
pub fn classify_verb_polarity(verb_name: &str, verb_def: &RuntimeVerb) -> IntentPolarity
```

Rules (ordered):
1. Verb name starts with `list-`, `show`, `get`, `read`, `search`, `find`, `describe`, `inspect`, `query`, `report`, `analyze`, `trace`, `discover`, `diff`, `for-`, `who-`, `identify`, `missing-`, `check-status`, `count`, `summary`, `impact` → **Read**
2. `metadata.side_effects == "facts_only"` → **Read**
3. Everything else → **Write**

### 1B: Classify every verb's observation plane eligibility

Same file. A verb can be eligible for one or more planes:

```rust
pub fn classify_verb_plane(verb_fqn: &str, verb_def: &RuntimeVerb) -> Vec<ObservationPlane>
```

Rules:
1. FQN starts with `schema.` or `registry.` → `[Structure, Registry]`
2. FQN starts with `changeset.` or `governance.` or `stewardship.` → `[Registry]`
3. All other verbs → `[Instance]`
4. Verbs with `phase_tags` containing `"data-management"` or `"stewardship"` → add `Structure`

### 1C: Build VerbMetadataIndex

```rust
pub struct VerbMetadataIndex {
    /// All indexed verbs: FQN → VerbMeta
    pub entries: HashMap<String, VerbMeta>,
}

pub struct VerbMeta {
    pub fqn: String,
    pub domain: String,       // FQN prefix before first '.'
    pub verb_name: String,    // FQN suffix after first '.'
    pub polarity: IntentPolarity,
    pub planes: Vec<ObservationPlane>,
    pub action_tags: Vec<String>,   // from metadata.tags
    pub param_names: Vec<String>,   // arg names (for overlap scoring)
    pub required_params: Vec<String>, // required arg names only
    pub description: String,
}

impl VerbMetadataIndex {
    /// Build from the runtime registry.
    pub fn from_registry() -> Self { ... }
    
    /// Filter by plane + polarity + domain. Returns candidates.
    pub fn query(
        &self,
        plane: ObservationPlane,
        polarity: IntentPolarity,
        domain: Option<&str>,
    ) -> Vec<&VerbMeta> { ... }
}
```

`from_registry()` iterates `dsl_v2::registry()`, classifies each verb, stores in HashMap.

### 1D: Unit tests for VerbMetadataIndex

- `query(Instance, Read, Some("deal"))` returns `deal.list`, `deal.get`, `deal.list-participants`, etc. — no write verbs.
- `query(Instance, Write, Some("cbu"))` returns `cbu.create`, `cbu.update`, `cbu.delete`, etc. — no read verbs.
- `query(Structure, Read, None)` returns `schema.*` and `registry.*` verbs only.
- `query(Instance, Read, Some("trading-profile"))` returns exactly the 4 read verbs (not 47).
- Total verbs across all `query(Instance, Write, None)` ≈ 500-600.
- Total verbs across all `query(Instance, Read, None)` ≈ 200-300.

**GATE 1:** `cargo test --lib -p ob-poc -- sage::verb_index` passes. Print `CODER PHASE 1 COMPLETE — 15% done`.

→ IMMEDIATELY proceed to Phase 2.

---

## Phase 2: Action + Parameter Overlap Scoring (35%)

**Goal:** Given a filtered set of verb candidates (from Phase 1 query), score each by action match and parameter name overlap with the OutcomeStep.

### 2A: Action matching

**File:** `rust/src/sage/verb_resolve.rs` (new file)

Map `OutcomeAction` to verb name patterns:

```rust
fn action_score(action: &OutcomeAction, verb_meta: &VerbMeta) -> f32
```

| OutcomeAction | Verb name patterns (high score 0.8) | Tag patterns (medium score 0.5) |
|---------------|--------------------------------------|----------------------------------|
| Create | `create`, `ensure`, `add`, `open`, `provision` | `lifecycle`, `write` |
| Investigate | `list`, `get`, `show`, `read`, `describe`, `search`, `find` | `read`, `query` |
| Modify | `update`, `set`, `configure`, `assign`, `change` | `write`, `modify` |
| Assess | `run`, `check`, `validate`, `compute`, `calculate`, `screen` | `assessment`, `screening` |
| Link | `assign`, `add-*`, `link`, `bind`, `associate` | `relationship`, `role` |
| Verify | `verify`, `approve`, `reject`, `confirm`, `mark` | `verification`, `evidence` |
| Remove | `delete`, `remove`, `cancel`, `end`, `terminate`, `revoke`, `deactivate` | `lifecycle`, `cleanup` |
| Transfer | `import`, `export`, `upload`, `transfer`, `move`, `send` | `data-movement` |
| Configure | `set-*`, `configure`, `enable`, `disable` | `config`, `settings` |
| Report | `report`, `summarize`, `compute`, `analyze`, `list-*` | `analytics`, `report` |

Score 0.8 for verb name match, 0.5 for tag match, 0.0 for no match.

### 2B: Parameter overlap scoring

```rust
fn param_overlap_score(step_params: &BTreeMap<String, ResolvedParam>, verb_meta: &VerbMeta) -> f32
```

Score = (number of step param names that match verb param names) / (number of verb required params).

Matching is fuzzy-ish:
- Exact match: `"name"` == `"name"` → 1.0
- Stem match: `"jurisdiction"` matches `"jurisdiction-code"` → 0.8
- Entity type match: step has `"entity"` param, verb has `"entity-id"` (uuid with lookup for entity) → 0.7

### 2C: Composite scoring and selection

```rust
pub fn resolve_verb(
    step: &OutcomeStep,
    index: &VerbMetadataIndex,
) -> Result<CoderResolution>

pub struct CoderResolution {
    pub verb_fqn: String,
    pub score: f32,
    pub candidates_considered: usize,
    pub runner_up: Option<(String, f32)>,
}
```

Flow:
1. `index.query(step.plane, step.polarity, Some(&step.domain_concept))` → candidates
2. If empty, try `index.query(step.plane, step.polarity, None)` (drop domain filter)
3. Score each: `0.6 * action_score + 0.4 * param_overlap_score`
4. Sort descending. Top score ≥ 0.5 → return it. Otherwise → `NeedsDisambiguation` error.

### 2D: Unit tests

- `OutcomeStep { plane: Instance, polarity: Write, domain: "cbu", action: Create, params: {name: _, jurisdiction: _} }` → resolves to `cbu.create`
- `OutcomeStep { plane: Instance, polarity: Read, domain: "deal", action: Investigate, params: {} }` → resolves to `deal.list` (no params → list not get)
- `OutcomeStep { plane: Instance, polarity: Read, domain: "ubo", action: Investigate, params: {entity-id: _} }` → resolves to `ubo.list-by-subject` or `ubo.list-ubos`
- `OutcomeStep { plane: Structure, polarity: Read, domain: "document", action: Investigate, params: {entity-type: "document"} }` → resolves to `schema.entity.describe`
- `OutcomeStep { plane: Instance, polarity: Write, domain: "screening", action: Assess, params: {entity-id: _} }` → resolves to `screening.sanctions`
- `OutcomeStep { plane: Instance, polarity: Write, domain: "fund", action: Create, params: {name: _, structure-type: "umbrella"} }` → resolves to `fund.create-umbrella`

At least 20 test cases covering each OutcomeAction.

**GATE 2:** `cargo test --lib -p ob-poc -- sage::verb_resolve` passes. Print `CODER PHASE 2 COMPLETE — 35% done`.

→ IMMEDIATELY proceed to Phase 3.

---

## Phase 3: Argument Assembly from OutcomeStep (55%)

**Goal:** Given a resolved verb and the OutcomeStep's resolved_params, produce the argument list that the existing DSL assembler needs. This bridges the Sage's domain-level params to the Coder's verb-level args.

### 3A: Parameter mapping

**File:** `rust/src/sage/arg_assembly.rs` (new file)

```rust
pub fn assemble_args(
    step: &OutcomeStep,
    verb_fqn: &str,
    verb_def: &RuntimeVerb,
) -> Result<Vec<IntentArgument>>
```

For each verb arg:
1. Look for exact name match in `step.resolved_params` → use it
2. Look for stem/alias match (e.g., step has `"entity"`, verb wants `"entity-id"`) → use it, mark as `Unresolved` if it's a UUID lookup
3. If verb arg is required and not found → `IntentArgValue::Missing`
4. If verb arg has a default → skip (let DSL assembler use default)

This reuses the existing `IntentArgument` / `IntentArgValue` types from `mcp/intent_pipeline.rs`. Import them — do NOT duplicate.

### 3B: DSL generation from assembled args

```rust
pub fn generate_dsl(
    verb_fqn: &str,
    args: &[IntentArgument],
) -> Result<String>
```

Reuses the existing `assemble_dsl_string()` logic from `IntentPipeline`. Extract it into a shared function that both the existing pipeline and the Coder can call. The Coder calls it with args from `assemble_args()`. The existing pipeline calls it with args from LLM extraction.

**File to modify:** `rust/src/mcp/intent_pipeline.rs` — extract `assemble_dsl_string` body into a pub free function `pub fn assemble_dsl(verb: &str, args: &[IntentArgument]) -> Result<String>`. The existing method delegates to it. The Coder imports it.

### 3C: End-to-end Coder path

**File:** `rust/src/sage/coder.rs` (new file)

```rust
pub struct CoderEngine {
    pub verb_index: VerbMetadataIndex,
}

impl CoderEngine {
    /// Full Coder path: OutcomeStep → verb resolution → arg assembly → DSL string
    pub fn generate_dsl_from_step(
        &self,
        step: &OutcomeStep,
    ) -> Result<CoderResult>
}

pub struct CoderResult {
    pub verb_fqn: String,
    pub dsl: String,
    pub resolution: CoderResolution,
    pub missing_args: Vec<String>,
    pub unresolved_refs: Vec<String>,
}
```

### 3D: Unit tests for end-to-end Coder

Test the full chain OutcomeStep → DSL string:
- `{ domain: "cbu", action: Create, params: {name: "Allianz", jurisdiction: "LU"} }` → `(cbu.create :name "Allianz" :jurisdiction "LU")`
- `{ domain: "deal", action: Investigate, params: {} }` → `(deal.list)`
- `{ domain: "schema", action: Investigate, params: {entity-type: "document"} }` → `(schema.entity.describe :entity-type "document")`
- `{ domain: "screening", action: Assess, params: {entity-id: <unresolved "HSBC">} }` → `(screening.sanctions :entity-id @"HSBC")` with unresolved ref

**GATE 3:** `cargo test --lib -p ob-poc -- sage::coder` passes. `cargo test --lib -p ob-poc -- sage::arg_assembly` passes. Print `CODER PHASE 3 COMPLETE — 55% done`.

→ IMMEDIATELY proceed to Phase 4.

---

## Phase 4: Shadow Comparison — Sage+Coder vs Existing Pipeline (75%)

**Goal:** For every utterance, run BOTH the existing IntentPipeline path AND the Sage→Coder path. Compare results. Log discrepancies.

### 4A: Add Coder to shadow mode

**File:** `rust/src/agent/orchestrator.rs`

Extend the shadow classification from Phase 1 (Sage GATE 5). After the Sage classifies the outcome, also run the Coder:

```rust
// After sage_outcome is computed (Phase 1 step 5C):
let coder_result = if let Some(ref outcome) = sage_outcome {
    if let Some(ref step) = outcome.steps.first() {
        match coder_engine.generate_dsl_from_step(step) {
            Ok(result) => {
                tracing::info!(
                    sage_verb = %result.verb_fqn,
                    pipeline_verb = %chosen_verb_pre_semreg.as_deref().unwrap_or("none"),
                    match_ = result.verb_fqn == chosen_verb_pre_semreg.as_deref().unwrap_or(""),
                    "Sage+Coder shadow: verb comparison"
                );
                Some(result)
            }
            Err(e) => {
                tracing::debug!(error = %e, "Sage+Coder shadow: Coder resolution failed");
                None
            }
        }
    } else { None }
} else { None };
```

### 4B: Add CoderEngine to OrchestratorContext

Add `pub coder_engine: Option<sage::coder::CoderEngine>` to `OrchestratorContext`.

Build it in `agent_service.rs::build_orchestrator_context()`:
```rust
coder_engine: Some(sage::coder::CoderEngine {
    verb_index: sage::verb_index::VerbMetadataIndex::from_registry(),
}),
```

### 4C: Add coder_result to IntentTrace and OrchestratorOutcome

New fields:
- `IntentTrace::sage_verb: Option<String>` — the verb the Sage+Coder picked
- `IntentTrace::sage_coder_match: Option<bool>` — did Sage+Coder agree with the pipeline?
- `OrchestratorOutcome::coder_result: Option<sage::coder::CoderResult>`

**GATE 4:** `cargo check -p ob-poc` passes. Server starts and logs `Sage+Coder shadow: verb comparison` lines. Print `CODER PHASE 4 COMPLETE — 75% done`.

→ IMMEDIATELY proceed to Phase 5.

---

## Phase 5: Comparative Coverage Harness (90%)

**Goal:** Extend the utterance coverage harness to report Sage+Coder accuracy alongside existing pipeline accuracy.

### 5A: Extend utterance_api_coverage.rs

**File:** `rust/tests/utterance_api_coverage.rs`

For each test case, extract from the API response:
- Existing pipeline verb (already tracked as `predicted_top_verb`)
- Sage+Coder verb (from `sage_verb` field in response, if present)

Add columns to coverage report:
```
| Utterance | Expected | Pipeline Verb | Sage+Coder Verb | Pipeline Pass | Sage Pass |
```

Add summary:
```
Pipeline accuracy: 38.81% (52/134)
Sage+Coder accuracy: XX.XX% (YY/134)
Sage wins (Sage right, pipeline wrong): N
Pipeline wins (pipeline right, Sage wrong): N
Both right: N
Both wrong: N
```

### 5B: Run and document comparison

Run `cargo test --test utterance_api_coverage -- --ignored --nocapture`.

Document: which utterances does the Sage+Coder get right that the pipeline gets wrong? Which does it get wrong? This guides Phase 2 TODO (Coder) tuning.

**GATE 5:** Coverage harness runs and produces comparative report. Print `CODER PHASE 5 COMPLETE — 90% done`.

→ IMMEDIATELY proceed to Phase 6.

---

## Phase 6: Sage-Only Fast Path for Read+Structure (100%)

**Goal:** For Read+Structure utterances where the Sage determines `sage_only = true` AND has a specific domain hint, bypass the existing pipeline entirely and return the Sage+Coder result as the primary response. This is the first real traffic flowing through the new path.

### 6A: Add sage_only execution path

**File:** `rust/src/agent/orchestrator.rs`

After Sage shadow classification, BEFORE the existing pipeline runs:

```rust
// -- Step 0.5: Sage-only fast path (Read+Structure) --
if let Some(ref outcome) = sage_outcome {
    if outcome.pre_classification.sage_only && outcome.confidence >= SageConfidence::Medium {
        if let Some(ref coder_result) = coder_result {
            if coder_result.missing_args.is_empty() {
                tracing::info!(
                    verb = %coder_result.verb_fqn,
                    "Sage-only fast path: Read+Structure — bypassing pipeline"
                );
                // Build PipelineResult from coder_result and return early
                // ... (construct PipelineResult with coder_result.dsl, etc.)
            }
        }
    }
}
```

This is gated by TWO conditions: `sage_only` (Read+Structure) AND `confidence >= Medium`. Safe because reads are low-risk (v0.3 §"Asymmetric Risk").

### 6B: Feature flag

Gate behind env var `SAGE_FAST_PATH=1`. Default: off. This allows gradual rollout.

### 6C: Test the fast path

Manually test with the server running:
- `stage_focus=semos-data-management` + `"show me deal record"` → should take Sage fast path → `schema.entity.describe :entity-type "deal"`
- `stage_focus=semos-data-management` + `"show me documents"` → Sage fast path → `schema.entity.describe :entity-type "document"`
- `stage_focus=semos-kyc` + `"show me documents"` → should NOT take fast path (Instance plane) → existing pipeline
- `stage_focus=semos-data-management` + `"create a new CBU"` → should NOT take fast path (Write polarity) → existing pipeline

**GATE 6:** `cargo check -p ob-poc` passes. `cargo test --lib -p ob-poc` all pass. Fast path activates correctly for Read+Structure utterances when `SAGE_FAST_PATH=1`. Existing pipeline handles everything else unchanged. Print `CODER PHASE 6 COMPLETE — 100% done`.

---

## Files Created/Modified Summary

| File | Action | Phase |
|------|--------|-------|
| `rust/src/sage/verb_index.rs` | Create | 1 |
| `rust/src/sage/verb_resolve.rs` | Create | 2 |
| `rust/src/sage/arg_assembly.rs` | Create | 3A |
| `rust/src/sage/coder.rs` | Create | 3C |
| `rust/src/sage/mod.rs` | Modify — add new submodule declarations | 1-3 |
| `rust/src/mcp/intent_pipeline.rs` | Modify — extract `assemble_dsl_string` into pub free fn | 3B |
| `rust/src/agent/orchestrator.rs` | Modify — add Coder shadow comparison + sage_only fast path | 4A, 6A |
| `rust/src/api/agent_service.rs` | Modify — build + inject CoderEngine | 4B |
| `rust/tests/utterance_api_coverage.rs` | Modify — add Sage+Coder comparison columns | 5A |

## Dependencies on Phase 1 (Sage Skeleton)

Phase 2 requires these Phase 1 artifacts to exist:
- `sage::outcome::OutcomeStep` (Phase 1, step 1D)
- `sage::outcome::OutcomeAction` (Phase 1, step 1D)
- `sage::plane::ObservationPlane` (Phase 1, step 1B)
- `sage::polarity::IntentPolarity` (Phase 1, step 1C)
- `sage::pre_classify::SagePreClassification` (Phase 1, step 1E)
- Sage shadow mode wired into orchestrator (Phase 1, GATE 5)

Phase 2 can begin as soon as Phase 1 GATE 5 passes.
