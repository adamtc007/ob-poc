# Sage Skeleton — Claude Code Execution Plan (Phase 1)

**Reference:** `docs/UTTERANCE_PIPELINE_ANALYSIS_AND_ARCHITECTURE_v0.3.md` (Part II + Part III Phase 1)  
**Objective:** Build the Sage persona as a new module alongside the existing orchestrator. Sage classifies outcomes using three deterministic pre-classification signals (observation plane, intent polarity, domain hints) and an LLM outcome classifier. Runs in **parallel** with the existing verb-first pipeline — does NOT replace it yet.  
**Target:** Sage outcome classification accuracy ≥ 60% on existing 134-utterance test suite. No changes to existing pipeline behaviour.

---

## Execution Protocol

- Each phase MUST complete fully before the GATE check.
- At each GATE: run the specified command. If it fails, fix before proceeding.
- At each GATE: print `SAGE PHASE N COMPLETE — N% done` and the E-invariant.
- → IMMEDIATELY proceed to the next phase after a passing GATE.
- Do NOT stop between phases. Do NOT ask for confirmation.
- If a phase has sub-steps (A, B, C), complete ALL sub-steps before the GATE.

**E-invariant (must hold at every GATE):**
`cargo check -p ob-poc 2>&1 | tail -1` shows no errors AND
`cargo test --lib -p ob-poc 2>&1 | tail -1` shows `test result: ok`.

The existing pipeline MUST remain fully functional at all times. The Sage is additive.

---

## Phase 1: Core Types (10%)

**Goal:** Define the Sage's type system — ObservationPlane, IntentPolarity, OutcomeIntent, OutcomeStep, SagePreClassification. Pure types, no logic yet.

### 1A: Create module structure

Create `rust/src/sage/` with:

```
rust/src/sage/
  mod.rs              — pub mod declarations + SageEngine trait
  outcome.rs          — OutcomeIntent, OutcomeStep, OutcomeAction, SageConfidence
  plane.rs            — ObservationPlane enum (Instance, Structure, Registry)
  polarity.rs         — IntentPolarity enum (Read, Write, Ambiguous) + clue word lists
  pre_classify.rs     — SagePreClassification struct
  context.rs          — SageContext (conversation history placeholder)
```

### 1B: Define ObservationPlane

**File:** `rust/src/sage/plane.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationPlane {
    /// Operating on entity instances identified by UUID.
    /// Entity linker resolves names → UUIDs. Verbs require *-id args.
    Instance,
    /// Operating on entity types, schemas, taxonomies, data models.
    /// No UUID resolution. Verbs take type names as strings.
    Structure,
    /// Operating on the semantic registry — snapshots, changesets, governance.
    Registry,
}
```

Derive: `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::Display, strum::EnumString, strum::AsRefStr`.

### 1C: Define IntentPolarity with clue word lists

**File:** `rust/src/sage/polarity.rs`

```rust
pub enum IntentPolarity {
    Read,      // show, list, what, who, which, where, how many, find, search,
               // get, describe, display, view, trace, discover, report, summarize,
               // status, count
    Write,     // create, add, new, set up, establish, onboard, open, update, change,
               // modify, assign, set, configure, delete, remove, cancel, end, terminate,
               // close, revoke, upload, import, export, transfer, move, send
    Ambiguous, // check, verify, run, process, handle
}

/// Classify polarity from utterance prefix. O(1) — checks first 1-3 words only.
pub fn classify_polarity(utterance: &str) -> IntentPolarity { ... }
```

The clue word lists are static arrays. `classify_polarity()` lowercases and checks prefix. Must handle multi-word prefixes ("set up", "how many", "who are"). Return `Ambiguous` if no prefix matches.

Include `#[cfg(test)]` unit tests:
- `"show me the CBUs"` → Read
- `"create a new CBU"` → Write
- `"check sanctions"` → Ambiguous
- `"what documents are missing?"` → Read
- `"onboard Allianz"` → Write
- `"run a check"` → Ambiguous
- `"trace ownership chain"` → Read
- `"upload the passport"` → Write
- `""` (empty) → Ambiguous
- `"hello"` (no match) → Ambiguous

### 1D: Define OutcomeIntent and OutcomeStep

**File:** `rust/src/sage/outcome.rs`

As specified in v0.3 §Part II "The Outcome Model". Include:
- `OutcomeIntent` with `plane`, `polarity`, `domain_concept`, `action`, `subject`, `steps`, `confidence`, `pending_clarifications`
- `OutcomeStep` with `plane`, `domain_concept`, `action`, `resolved_params`, `requires_confirmation`, `depends_on`, `execution_mode`
- `OutcomeAction` enum (Create, Investigate, Modify, Assess, Link, Verify, Remove, Transfer, Configure, Report)
- `SageConfidence` enum (High, Medium, Low, Unclear)
- `ExecutionMode` enum (Research, Execute)
- `Clarification` struct (question: String, options: Vec<String>)

All types derive `Debug, Clone, Serialize, Deserialize`.

### 1E: Define SagePreClassification

**File:** `rust/src/sage/pre_classify.rs`

```rust
pub struct SagePreClassification {
    pub plane: ObservationPlane,
    pub polarity: IntentPolarity,
    pub domain_hints: Vec<String>,
    pub clue_word: Option<String>,
    pub sage_only: bool, // True if Read+Structure → skip Coder entirely
}
```

### 1F: Wire module into crate

Add `pub mod sage;` to `rust/src/lib.rs` (or `main.rs` / `mod.rs` depending on crate root).

**GATE 1:** `cargo check -p ob-poc` passes. `cargo test --lib -p ob-poc -- sage` runs and all polarity unit tests pass. Print `SAGE PHASE 1 COMPLETE — 10% done`.

→ IMMEDIATELY proceed to Phase 2.

---

## Phase 2: Observation Plane Classifier (25%)

**Goal:** Classify observation plane from session context. Deterministic — no LLM.

### 2A: Plane classification logic

**File:** `rust/src/sage/plane.rs` — add:

```rust
pub fn classify_plane(
    stage_focus: Option<&str>,
    has_instance_targeting: bool,
) -> ObservationPlane
```

Rules (deterministic, ordered):
1. `stage_focus` starts with `"semos-data-management"` or `"semos-data"` AND no instance targeting → `Structure`
2. `stage_focus` starts with `"semos-stewardship"` → `Registry`
3. Everything else → `Instance`

Instance targeting reuses the existing `has_explicit_instance_targeting()` logic from `orchestrator.rs` line 194 — extract it into `sage::plane` as a shared function. The orchestrator can then call `sage::plane::has_explicit_instance_targeting()` instead of its local copy.

### 2B: Unit tests for plane classification

Test cases:
- `stage_focus=Some("semos-data-management"), no instance targeting` → Structure
- `stage_focus=Some("semos-data"), no instance targeting` → Structure
- `stage_focus=Some("semos-data-management"), utterance contains "deal-id"` → Instance (instance targeting overrides)
- `stage_focus=Some("semos-stewardship")` → Registry
- `stage_focus=Some("semos-kyc")` → Instance
- `stage_focus=Some("semos-onboarding")` → Instance
- `stage_focus=None` → Instance

**GATE 2:** `cargo test --lib -p ob-poc -- sage::plane` passes. Print `SAGE PHASE 2 COMPLETE — 25% done`.

→ IMMEDIATELY proceed to Phase 3.

---

## Phase 3: Domain Hint Extraction (40%)

**Goal:** Extract domain hints from utterance using ECIR noun extraction + entity_kind + stage_focus goals. Deterministic — no LLM.

### 3A: Domain hint function

**File:** `rust/src/sage/pre_classify.rs` — add:

```rust
pub fn extract_domain_hints(
    utterance: &str,
    noun_index: Option<&NounIndex>,
    entity_kind: Option<&str>,
    goals: &[String],
) -> Vec<String>
```

Logic:
1. Run `NounIndex::extract()` on utterance → matched nouns → noun keys → domain hints
2. If `entity_kind` is set (from entity linking), map to domain (e.g., "cbu" → "cbu", "entity" → "entity", "fund" → "fund")
3. Add `goals` as domain hints (e.g., `["kyc"]` → `"screening"`, `"kyc"`, `"document"`)
4. Deduplicate and return

This reuses the existing `NounIndex` from `rust/src/mcp/noun_index.rs`. Import it — do NOT copy it. The Sage calls the noun_index for domain hints; the noun_index does NOT need to know about the Sage.

### 3B: Build SagePreClassification

**File:** `rust/src/sage/pre_classify.rs` — add:

```rust
pub fn pre_classify(
    utterance: &str,
    stage_focus: Option<&str>,
    noun_index: Option<&NounIndex>,
    entity_kind: Option<&str>,
    goals: &[String],
) -> SagePreClassification
```

Composes the three signals:
1. `classify_plane(stage_focus, has_explicit_instance_targeting(utterance))`
2. `classify_polarity(utterance)`
3. `extract_domain_hints(utterance, noun_index, entity_kind, goals)`
4. Compute `sage_only`: true if `plane == Structure && polarity == Read`

### 3C: Unit tests for pre_classify

Test the composition:
- `"show me deal record"` + semos-data-management → `{ plane: Structure, polarity: Read, domain: ["deal"], sage_only: true }`
- `"create a CBU for Allianz"` + semos-onboarding → `{ plane: Instance, polarity: Write, domain: ["cbu"], sage_only: false }`
- `"who are the beneficial owners?"` + semos-kyc → `{ plane: Instance, polarity: Read, domain: ["ubo"], sage_only: false }`
- `"check sanctions"` + semos-kyc → `{ plane: Instance, polarity: Ambiguous, domain: ["screening"], sage_only: false }`
- `"describe entity schema for document"` + semos-data → `{ plane: Structure, polarity: Read, domain: ["document"], sage_only: true }`

**GATE 3:** `cargo test --lib -p ob-poc -- sage::pre_classify` passes. Print `SAGE PHASE 3 COMPLETE — 40% done`.

→ IMMEDIATELY proceed to Phase 4.

---

## Phase 4: SageEngine Trait + Stub Implementation (55%)

**Goal:** Define the SageEngine trait and a stub implementation that uses pre-classification only (no LLM). This is the minimum viable Sage.

### 4A: SageEngine trait

**File:** `rust/src/sage/mod.rs`

```rust
#[async_trait::async_trait]
pub trait SageEngine: Send + Sync {
    /// Classify an utterance into an OutcomeIntent.
    async fn classify(
        &self,
        utterance: &str,
        context: &SageContext,
    ) -> anyhow::Result<OutcomeIntent>;
}
```

### 4B: SageContext

**File:** `rust/src/sage/context.rs`

```rust
pub struct SageContext {
    pub session_id: Option<Uuid>,
    pub stage_focus: Option<String>,
    pub goals: Vec<String>,
    pub entity_kind: Option<String>,
    pub dominant_entity_name: Option<String>,
    pub last_intents: Vec<(String, String)>, // (utterance, outcome_summary) — last 3
}
```

Build `SageContext::from_orchestrator_context()` that extracts relevant fields from `OrchestratorContext`. This is the bridge — the Sage reads from the same session state the orchestrator already has.

### 4C: DeterministicSage (stub — pre-classification only, no LLM)

**File:** `rust/src/sage/deterministic.rs`

Implements `SageEngine`. Uses `pre_classify()` to build a basic `OutcomeIntent`:

- Plane from pre-classification
- Polarity from pre-classification  
- Domain from first domain hint (or "unknown")
- Action: map from polarity (Read → Investigate, Write + domain_has_create_context → Create, Write → Modify, Ambiguous → Assess)
- Confidence: High if `sage_only` (Read+Structure), Medium if single domain hint, Low if no domain hint, Unclear if no signals
- Steps: single OutcomeStep mirroring the OutcomeIntent (no decomposition in stub)
- No LLM call

This stub is enough to measure the pre-classification accuracy against the test suite.

### 4D: Unit tests for DeterministicSage

Test at least 10 utterances from the existing test fixture, verifying the OutcomeIntent fields match expected domain + action + plane.

**GATE 4:** `cargo test --lib -p ob-poc -- sage` all pass. Print `SAGE PHASE 4 COMPLETE — 55% done`.

→ IMMEDIATELY proceed to Phase 5.

---

## Phase 5: Shadow Mode — Sage Alongside Orchestrator (70%)

**Goal:** Wire the Sage into the orchestrator in shadow mode — it classifies every utterance but the result is logged, not acted upon. The existing pipeline continues to execute. This gives us comparison data.

### 5A: Add Sage to OrchestratorContext

**File:** `rust/src/agent/orchestrator.rs`

Add to `OrchestratorContext`:
```rust
/// Sage engine for outcome classification (shadow mode).
/// When present, classify() runs in parallel with existing pipeline.
pub sage: Option<Arc<dyn sage::SageEngine>>,
```

### 5B: Build and inject Sage in agent_service

**File:** `rust/src/api/agent_service.rs`

In `build_orchestrator_context()`, construct a `DeterministicSage` and inject it:

```rust
sage: Some(Arc::new(sage::deterministic::DeterministicSage::new(
    self.noun_index.clone(),
))),
```

### 5C: Shadow classification in handle_utterance

**File:** `rust/src/agent/orchestrator.rs`

At the TOP of `handle_utterance()`, BEFORE entity linking (this is critical — the Sage pre-classifies before entity linking can pollute the plane):

```rust
// -- Step 0: Sage shadow classification (before entity linking) --
let sage_outcome = if let Some(ref sage) = ctx.sage {
    let sage_ctx = sage::context::SageContext::from_orchestrator(ctx, utterance);
    match sage.classify(utterance, &sage_ctx).await {
        Ok(outcome) => {
            tracing::info!(
                plane = %outcome.plane,
                polarity = %outcome.polarity,
                domain = %outcome.domain_concept,
                action = %outcome.action,
                confidence = %outcome.confidence,
                sage_only = outcome.pre_classification.sage_only,
                "Sage shadow classification"
            );
            Some(outcome)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Sage shadow classification failed");
            None
        }
    }
} else {
    None
};
```

Add `sage_outcome` to `IntentTrace` as a new optional field so it appears in telemetry.

### 5D: Add sage_outcome to OrchestratorOutcome

Add `pub sage_outcome: Option<sage::outcome::OutcomeIntent>` to `OrchestratorOutcome`. This makes the Sage's classification visible in API responses and test harnesses without changing any existing behaviour.

**GATE 5:** `cargo check -p ob-poc` passes. `cargo test --lib -p ob-poc` all pass. The existing 134-utterance harness still runs unchanged. Sage classification appears in server logs when the server is running. Print `SAGE PHASE 5 COMPLETE — 70% done`.

→ IMMEDIATELY proceed to Phase 6.

---

## Phase 6: Sage Coverage Harness (85%)

**Goal:** Measure the Sage's pre-classification accuracy against the existing test utterances. Build a harness that compares Sage domain + action vs expected verb domain + action.

### 6A: Extend utterance test fixtures

**File:** `docs/todo/intent_test_utterances.toml`

Add fields to each `[[test]]` entry (do NOT remove existing fields):
```toml
expected_plane = "instance"          # instance | structure | registry
expected_polarity = "write"          # read | write | ambiguous
expected_domain_concept = "cbu"      # domain concept (not verb FQN prefix)
```

Populate for all 134 entries. Use the existing `expected_verb` to derive:
- `expected_plane`: "instance" for most, "structure" for schema.* verbs, "registry" for registry.* verbs
- `expected_polarity`: "read" if expected_verb starts with list/get/show/find/search/read/trace/discover/for-/who-/identify/missing/report/analyze/compute/calculate, else "write"
- `expected_domain_concept`: verb FQN prefix (e.g., `cbu.create` → `"cbu"`, `screening.sanctions` → `"screening"`)

### 6B: Build Sage accuracy measurement

**File:** `rust/tests/sage_coverage.rs`

New test (similar structure to `utterance_api_coverage.rs`) that:
1. For each test case, builds a `SageContext` with appropriate `stage_focus` and `goals`
2. Calls `DeterministicSage::classify()`
3. Compares `outcome.plane` vs `expected_plane`, `outcome.polarity` vs `expected_polarity`, `outcome.domain_concept` vs `expected_domain_concept`
4. Reports accuracy per signal: plane accuracy %, polarity accuracy %, domain accuracy %
5. Writes results to `target/sage-coverage/sage_accuracy.md`

### 6C: Run and document baseline

Run the sage coverage harness. Document the baseline accuracy. Expected:
- Polarity accuracy: ~90%+ (prefix-based, should be very reliable)
- Plane accuracy: ~95%+ (session context based, should be near-perfect)
- Domain accuracy: ~50-70% (depends on ECIR noun_index coverage)

**GATE 6:** `cargo test --test sage_coverage -- --ignored --nocapture` runs and produces coverage report. Print `SAGE PHASE 6 COMPLETE — 85% done`.

→ IMMEDIATELY proceed to Phase 7.

---

## Phase 7: LLM Outcome Classifier (100%)

**Goal:** Add an LLM-backed SageEngine implementation that uses the pre-classification signals to constrain a structured output LLM call. This is the full Sage.

### 7A: LlmSage implementation

**File:** `rust/src/sage/llm_sage.rs`

Implements `SageEngine`. Flow:
1. Call `pre_classify()` for the three deterministic signals
2. If `sage_only` (Read+Structure) AND domain hint is specific → build OutcomeIntent directly, NO LLM call (fast path)
3. Otherwise, build LLM prompt pre-constrained by the signals (see v0.3 §Phase 1 for prompt template)
4. Parse JSON response into OutcomeIntent
5. Apply asymmetric confidence thresholds (v0.3 §"Asymmetric Risk"):
   - Read + any domain match → bump confidence to at least Medium
   - Write + no domain match → cap confidence at Low

**LLM prompt template:**
```
You are a custody banking operations specialist. Given the user's 
request and context, identify the outcome.

PRE-CLASSIFICATION (already determined):
  Observation plane: {plane}
  Intent polarity: {polarity}
  Domain hints: {domain_hints}

Given these constraints, identify:
1. OUTCOME: What does the user want to achieve? (one sentence)
2. DOMAIN: Confirm or refine the domain. ({domain_list_filtered_by_polarity})
3. ACTION: What type? ({action_list_filtered_by_polarity})
4. PARAMETERS: What specific values did the user provide? (JSON object)
5. CONFIDENCE: How certain? (high/medium/low/unclear)

Respond in JSON only. No explanation.
```

Note: the domain list and action list in the prompt are FILTERED by polarity (v0.3 §"Polarity as Early Outcome Narrowing"). Read polarity gets ~4 actions + read-relevant domains. Write polarity gets ~6 actions + write-relevant domains.

### 7B: Feature-flag Sage implementation

Use a runtime flag to select `DeterministicSage` vs `LlmSage`:

```rust
// In agent_service build_orchestrator_context:
let sage: Arc<dyn SageEngine> = if self.sage_llm_enabled {
    Arc::new(LlmSage::new(self.llm_client.clone(), self.noun_index.clone()))
} else {
    Arc::new(DeterministicSage::new(self.noun_index.clone()))
};
```

Default: `DeterministicSage` (zero cost, no LLM). `LlmSage` activated by env var `SAGE_LLM=1`.

### 7C: Integration test with LLM

**File:** `rust/tests/sage_llm_integration.rs`

Ignored test (requires running LLM) that sends 10 representative utterances through `LlmSage` and validates OutcomeIntent fields.

### 7D: Re-run sage coverage harness with LlmSage

Run `SAGE_LLM=1 cargo test --test sage_coverage -- --ignored --nocapture`.
Compare domain accuracy improvement over DeterministicSage.

**GATE 7:** `cargo check -p ob-poc` passes. `cargo test --lib -p ob-poc` all pass (no LLM tests in lib). LlmSage integration test passes when run manually with `SAGE_LLM=1`. Coverage report shows domain accuracy ≥ 60%. Print `SAGE PHASE 7 COMPLETE — 100% done`.

---

## Files Created/Modified Summary

| File | Action | Phase |
|------|--------|-------|
| `rust/src/sage/mod.rs` | Create | 1A, 4A |
| `rust/src/sage/outcome.rs` | Create | 1D |
| `rust/src/sage/plane.rs` | Create | 1B, 2A |
| `rust/src/sage/polarity.rs` | Create | 1C |
| `rust/src/sage/pre_classify.rs` | Create | 1E, 3A, 3B |
| `rust/src/sage/context.rs` | Create | 1F, 4B |
| `rust/src/sage/deterministic.rs` | Create | 4C |
| `rust/src/sage/llm_sage.rs` | Create | 7A |
| `rust/src/lib.rs` (or crate root) | Modify — add `pub mod sage` | 1F |
| `rust/src/agent/orchestrator.rs` | Modify — add sage shadow, sage field on context/outcome | 5A, 5C, 5D |
| `rust/src/api/agent_service.rs` | Modify — build + inject Sage | 5B, 7B |
| `docs/todo/intent_test_utterances.toml` | Modify — add expected_plane/polarity/domain fields | 6A |
| `rust/tests/sage_coverage.rs` | Create | 6B |
| `rust/tests/sage_llm_integration.rs` | Create | 7C |

## Touchpoints in Existing Code (Read-Only References)

These files are READ by the Sage but NOT modified:
- `rust/src/mcp/noun_index.rs` — NounIndex for domain hint extraction
- `rust/src/agent/orchestrator.rs::OrchestratorContext` — session state source
- `rust/src/agent/orchestrator.rs::IntentTrace` — augmented with sage_outcome field
- `rust/config/noun_index.yaml` — noun definitions (Sage reuses, does not modify)
