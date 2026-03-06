# Sage/Coder Intent Pipeline — Implementation Plan

## Context

The current utterance→DSL pipeline achieves **38.81% first-attempt accuracy** (52/134 test utterances). Root cause analysis identifies 9 issues:

- **RC-1**: Verb-first error — pipeline searches for verbs before understanding intent
- **RC-2**: Invisible data model — pipeline has no concept of observation planes (instance vs structure vs registry)
- **RC-3**: Embedding overcrowding — 1,263 verbs in 384-dim space creates false proximity
- **RC-5**: No LLM in verb selection — pure embedding search misses semantic nuance
- **RC-7**: No Sage/Coder separation — intent understanding and verb resolution are tangled
- **RC-8**: Entry point fragmentation — 3 `IntentPipeline` instantiations in orchestrator
- **RC-9**: Unexploited intent polarity — Read vs Write distinction not used for narrowing

The `data_management_rewrite()` hack in `orchestrator.rs` (lines 180-319) is a string-level band-aid for the Structure observation plane that should be replaced by a principled mechanism.

**Goal**: Implement Sage/Coder split architecture. Sage understands intent (never sees verb FQNs). Coder resolves to DSL (never interprets NL). Target: ≤2 prompts to execution, 80%+ first-attempt hit rate.

---

## Phase 1: Sage Skeleton (7 sub-phases)

### ✅ Phase 1.1 — Core Types

Create `rust/src/sage/mod.rs` and type modules:

**New files:**
- `rust/src/sage/mod.rs` — module root with re-exports
- `rust/src/sage/outcome.rs` — `OutcomeIntent`, `OutcomeStep`, `OutcomeAction`, `SageConfidence`, `Clarification`, `EntityRef`
- `rust/src/sage/plane.rs` — `ObservationPlane` enum (Instance, Structure, Registry)
- `rust/src/sage/polarity.rs` — `IntentPolarity` enum (Read, Write, Ambiguous) with `CLUE_WORDS` taxonomy

**Modified:**
- `rust/src/lib.rs` — add `pub mod sage;`

Key types from spec:
```
ObservationPlane { Instance, Structure, Registry }
IntentPolarity { Read, Write, Ambiguous }
OutcomeIntent { summary, plane, polarity, domain_concept, action, subject, steps, confidence, pending_clarifications }
OutcomeStep { action, target, params, notes }
SageConfidence { High, Medium, Low }
```

**Gate**: Types compile, `cargo test -p ob-poc --lib sage` passes.

### ✅ Phase 1.2 — Observation Plane Classifier

**New file:**
- `rust/src/sage/pre_classify.rs` — `SagePreClassification` struct, `pre_classify()` function

Deterministic classification rules (no LLM):
- `stage_focus = "semos-data-management"/"semos-data"` + no instance targeting → `Structure`
- `stage_focus = "semos-stewardship"` → `Registry`
- Everything else → `Instance`
- Instance targeting detected by: UUID in utterance, `@`-binding reference, or entity kind in session context

**Reuse**: `NounIndex::extract_nouns()` from `rust/src/mcp/noun_index.rs` for domain hint extraction (already wired via `VerbSearcherFactory`).

**Gate**: Unit tests for all three planes with various session contexts.

### ✅ Phase 1.3 — Domain Hint Extraction

Extend `pre_classify()` to populate `domain_hints: Vec<String>` using:
1. `NounIndex::extract_nouns()` — reuse existing 99-noun taxonomy
2. `ActionCategory::classify()` from `noun_index.rs` — action word detection
3. Session `stage_focus` → domain allowlist mapping (reuse from `verb_surface.rs` workflow phase filter)

> **Note**: Implemented inside `pre_classify.rs` (merged with Phase 1.2).

**Gate**: Domain hints correctly extracted for ≥80% of test utterances.

### ✅ Phase 1.4 — SageEngine Trait + DeterministicSage

**New files:**
- `rust/src/sage/context.rs` — `SageContext` (bridges from `OrchestratorContext`)
- `rust/src/sage/deterministic.rs` — `DeterministicSage` implementing `SageEngine`

```
trait SageEngine: Send + Sync {
    async fn classify(&self, utterance: &str, context: &SageContext) -> Result<OutcomeIntent>;
}
```

`SageContext` fields: `session_id`, `stage_focus`, `goals`, `entity_kind`, `dominant_entity_name`, `last_intents`.

`DeterministicSage` uses ONLY pre-classification (no LLM):
- Plane from session context
- Polarity from clue word prefix scan
- Domain from noun extraction
- Action from first verb-like word
- Confidence always `Low` (deterministic stub)

**Gate**: `DeterministicSage.classify()` returns valid `OutcomeIntent` for all test utterances.

### ✅ Phase 1.5 — Shadow Mode Wiring

**Modified:**
- `rust/src/agent/orchestrator.rs` — insert Sage classification as **Stage 1.5** in `handle_utterance()`, BEFORE entity linking (Stage 3)

#### E-SAGE-1: Sage MUST fire before entity linking (NON-NEGOTIABLE)

This is the architectural invariant that justifies the entire Sage effort. The current `data_management_rewrite()` hack exists precisely because entity linking poisons plane classification — if "deal" resolves to a UUID before the pipeline knows we're in Structure plane, the pipeline treats it as an Instance operation and routes to the wrong verbs.

**Orchestrator stage ordering after Phase 1.5:**
```
Stage 1   — Session load + context extraction
Stage 1.5 — ★ SAGE CLASSIFICATION ★ (raw utterance + session context ONLY)
Stage 2   — ContextEnvelope (SemReg CCIR)
Stage 2.5 — SessionVerbSurface
Stage 3   — Entity linking (LookupService.analyze())
Stage A   — IntentPipeline construction + verb search
Stage B   — DSL generation
```

Shadow mode behavior:
- Gated by env var `SAGE_SHADOW=1` (default: off)
- Runs `DeterministicSage.classify()` synchronously (fast — no LLM, no DB)
- Logs comparison: `tracing::info!(sage_plane=?, sage_polarity=?, sage_domain=?, existing_verb=?)`
- **No production impact** — existing pipeline result always used
- Sage result stored in `IntentTrace` for telemetry (new fields: `sage_plane`, `sage_polarity`, `sage_domain_hints`)

Construction: `DeterministicSage` built in `main.rs` during server startup, injected into orchestrator via `OrchestratorContext` as `Option<Arc<dyn SageEngine>>`.

**Gate**: ✅ `cargo check -p ob-poc` passes.

### Phase 1.6 — Sage Coverage Harness

**New file:**
- `rust/tests/sage_coverage.rs` — coverage harness using extended test fixture

**Modified:**
- `rust/tests/fixtures/intent_test_utterances.toml` — add optional fields: `expected_plane`, `expected_polarity`, `expected_domain_concept`

Harness measures:
- Plane accuracy (% correct observation plane)
- Polarity accuracy (% correct read/write/ambiguous)
- Domain accuracy (% correct domain concept)
- Outputs markdown report to `target/sage-coverage/`

**Gate**: Harness runs, plane accuracy ≥70%, polarity accuracy ≥80%.

### Phase 1.7 — LLM Outcome Classifier

**New file:**
- `rust/src/sage/llm_sage.rs` — `LlmSage` implementing `SageEngine`

LLM-backed Sage with structured output:
- Pre-classification constrains LLM prompt (plane, polarity already determined → LLM fills action, domain_concept, steps)
- System prompt includes ObservationPlane definitions, IntentPolarity definitions
- Response parsed as `OutcomeIntent` via structured JSON output
- Fallback to `DeterministicSage` on LLM failure
- Confidence based on pre-classification agreement with LLM output

Gated by env var `SAGE_LLM=1` (default: use DeterministicSage).

**Gate**: LLM Sage produces valid OutcomeIntent, coverage harness shows improvement over DeterministicSage.

---

## Phase 2: Coder Verb Resolution Rewrite (6 sub-phases)

**Prerequisite**: Phase 1 Gate 5+ complete (shadow mode working, `OutcomeIntent` flowing). ✅

### Phase 2.1 — VerbMetadataIndex

**New file:**
- `rust/src/sage/verb_index.rs` — `VerbMetadataIndex`, `VerbMeta`

Precomputed index over all 1,263 verbs from `RuntimeVerbRegistry`:

```
VerbMeta {
    fqn, domain, verb_name,
    polarity: IntentPolarity,     // Derived from CRUD operation + action keywords
    planes: Vec<ObservationPlane>, // Derived from domain prefix + metadata.tier
    action_tags: Vec<String>,      // Normalized action synonyms
    param_names: Vec<String>,      // All arg names
    required_params: Vec<String>,  // Required arg names
    description: String,
}
```

Classification rules:
- **Polarity**: `list-*/show*/get*/read*/search*` → Read, everything else → Write
- **Plane**: `schema.*/registry.*` → [Structure, Registry]; `changeset.*/governance.*` → [Registry]; others → [Instance]
- **Action tags**: Generated from verb name + CRUD operation + description keywords

Built once at startup from `RuntimeVerbRegistry` + verb YAML config.

**Reuse**: `VerbConfig` and `VerbMetadata` from `rust/crates/dsl-core/src/config/types.rs`.

**Gate**: Index builds for all 1,263 verbs, plane/polarity classification unit tests pass.

### Phase 2.2 — StructuredVerbScorer

**New file:**
- `rust/src/sage/verb_resolve.rs` — `StructuredVerbScorer`, scoring functions

Metadata-driven verb ranking (no embedding search):

1. **Filter** by plane + polarity from `OutcomeIntent`
2. **Action score** (0.0-1.0): exact verb name match (0.8), action tag match (0.5), description keyword match (0.3)
3. **Param overlap score** (0.0-1.0): Jaccard similarity between `OutcomeStep.params` keys and `VerbMeta.param_names`
4. **Composite**: `0.6 * action_score + 0.4 * param_overlap_score`
5. **Rank** and return top-N candidates with scores

Key insight: Plane+polarity filtering eliminates ~80% of verbs before scoring even begins.

**Gate**: Scorer returns correct verb in top-3 for ≥60% of test utterances (baseline, without LLM).

### Phase 2.3 — Argument Assembly from OutcomeStep

**New file:**
- `rust/src/sage/arg_assembly.rs` — `assemble_args_from_step()`, DSL string generation

Maps `OutcomeStep.params` to verb args:
1. Load verb's arg definitions from `VerbConfig`
2. Match step param names to arg names (exact + fuzzy)
3. Type coerce values (string → uuid lookup, string → enum validation)
4. Generate DSL string

**Reuse**: Extract `assemble_dsl_string()` from `rust/src/mcp/intent_pipeline.rs:945-960` as a shared pub free function. Reuse `IntentArgValue` enum from same file (line 67-89).

**Modified:**
- `rust/src/mcp/intent_pipeline.rs` — extract `assemble_dsl_string()` to `pub` visibility (or move to shared module)

**Gate**: Arg assembly produces valid DSL for test cases with known params.

### Phase 2.4 — CoderEngine + Shadow Comparison

**New file:**
- `rust/src/sage/coder.rs` — `CoderEngine`, `CoderResult`

```
CoderEngine {
    verb_index: VerbMetadataIndex,
}

CoderResult {
    verb_fqn: String,
    dsl: String,
    resolution: CoderResolution, // Confident | Proposed | NeedsInput
    missing_args: Vec<String>,
    unresolved_refs: Vec<String>,
}
```

End-to-end: `OutcomeIntent` → filter verbs → score → pick top → assemble args → DSL string.

**Modified:**
- `rust/src/agent/orchestrator.rs` — extend shadow mode to run full Sage→Coder pipeline alongside existing pipeline, log comparison

Shadow comparison logs:
- `sage_coder_verb` vs `existing_verb` — agreement rate
- `sage_coder_dsl` vs `existing_dsl` — structural similarity
- Time comparison (Sage+Coder vs existing pipeline)

**Gate**: Shadow comparison running, Sage+Coder agreement with existing pipeline ≥50%.

### Phase 2.5 — Comparative Coverage Harness

**Modified:**
- `rust/tests/utterance_api_coverage.rs` — add Sage+Coder column to coverage report

Extended coverage report adds columns:
- `sage_verb`: verb resolved by Sage+Coder path
- `sage_dsl`: DSL generated by Sage+Coder path
- `sage_match`: whether Sage+Coder matched expected verb
- Side-by-side accuracy comparison

**Gate**: Comparative harness runs, report shows Sage+Coder accuracy alongside existing pipeline.

### Phase 2.6 — Sage-Only Fast Path

For `Read + Structure` utterances (safe, no side effects):
- Skip entire existing pipeline (no `IntentPipeline`, no `HybridVerbSearcher`)
- Use Sage→Coder result directly
- Gated by env var `SAGE_FAST_PATH=1` AND `sage_only=true` from pre-classification AND `confidence >= Medium`

**Modified:**
- `rust/src/agent/orchestrator.rs` — add fast-path branch before IntentPipeline construction

This is the first production use of the Sage/Coder pipeline (not shadow mode).

**Gate**: Fast path active for qualifying utterances, no regressions, accuracy ≥ existing pipeline for Read+Structure queries.

---

## File Manifest

### New Files (Phase 1)
| File | Status | Purpose |
|------|--------|---------|
| `rust/src/sage/mod.rs` | ✅ Done | Module root, re-exports |
| `rust/src/sage/outcome.rs` | ✅ Done | OutcomeIntent, OutcomeStep, OutcomeAction, SageConfidence |
| `rust/src/sage/plane.rs` | ✅ Done | ObservationPlane enum |
| `rust/src/sage/polarity.rs` | ✅ Done | IntentPolarity enum + CLUE_WORDS |
| `rust/src/sage/pre_classify.rs` | ✅ Done | SagePreClassification, pre_classify(), domain_hints |
| `rust/src/sage/context.rs` | ✅ Done | SageContext (orchestrator bridge) |
| `rust/src/sage/deterministic.rs` | ✅ Done | DeterministicSage (no-LLM stub) |
| `rust/src/sage/llm_sage.rs` | Pending | LlmSage (LLM-backed, Phase 1.7) |
| `rust/tests/sage_coverage.rs` | Pending | Coverage harness |

### New Files (Phase 2)
| File | Status | Purpose |
|------|--------|---------|
| `rust/src/sage/verb_index.rs` | Pending | VerbMetadataIndex (plane/polarity/domain classification) |
| `rust/src/sage/verb_resolve.rs` | Pending | StructuredVerbScorer (action + param overlap) |
| `rust/src/sage/arg_assembly.rs` | Pending | OutcomeStep → DSL arg assembly |
| `rust/src/sage/coder.rs` | Pending | CoderEngine (end-to-end Coder path) |

### Modified Files
| File | Status | Change |
|------|--------|--------|
| `rust/src/lib.rs` | ✅ Done | Add `pub mod sage;` |
| `rust/src/agent/orchestrator.rs` | ✅ 1.5 done | Shadow mode insertion (Phase 1.5); Coder comparison (2.4) and fast path (2.6) pending |
| `rust/src/mcp/intent_pipeline.rs` | Pending | Extract `assemble_dsl_string()` as pub (Phase 2.3) |
| `rust/tests/fixtures/intent_test_utterances.toml` | Pending | Add expected_plane, expected_polarity, expected_domain_concept fields |
| `rust/tests/utterance_api_coverage.rs` | Pending | Sage+Coder comparative column (Phase 2.5) |
| `rust/crates/ob-poc-web/src/main.rs` | Pending | Build and inject SageEngine into orchestrator |

### Reused Components (no changes needed)
| Component | File | Usage |
|-----------|------|-------|
| `NounIndex::extract_nouns()` | `rust/src/mcp/noun_index.rs` | Domain hint extraction |
| `ActionCategory::classify()` | `rust/src/mcp/noun_index.rs` | Action word classification |
| `IntentArgValue` | `rust/src/mcp/intent_pipeline.rs` | Arg type representation |
| `RuntimeVerbRegistry` | `rust/src/dsl_v2/runtime_registry.rs` | Source for VerbMetadataIndex |
| `VerbConfig` / `VerbMetadata` | `rust/crates/dsl-core/src/config/types.rs` | Verb classification input |
| `SessionVerbSurface` | `rust/src/agent/verb_surface.rs` | Allowed verb pre-constraint |

---

## Invariants (must hold at every gate)

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| **E-SAGE-1** | Sage classification fires BEFORE entity linking in orchestrator pipeline | `#[cfg(test)]` call-order assertion in orchestrator; harness test verifying Sage receives raw utterance |
| **E-SAGE-2** | Sage never sees verb FQNs — only utterance + SageContext | `SageContext` struct has no verb/fqn fields; `SageEngine` trait signature enforces this |
| **E-SAGE-3** | Coder never interprets NL — only `OutcomeIntent` + `VerbMetadataIndex` | `CoderEngine` takes `OutcomeIntent`, not `&str` utterance |
| **E-SAGE-4** | Shadow mode has zero production impact — existing pipeline result always returned | Shadow branch gated by `SAGE_SHADOW` env var; result only written to trace/logs |
| **E-SAGE-5** | `cargo check -p ob-poc` passes after every sub-phase | All new code compiles alongside existing pipeline; no existing signatures changed |
| **E-SAGE-6** | `data_management_rewrite()` unchanged until Sage accuracy exceeds it | Hack deletion gated behind `cfg(not(feature = "sage"))`, not removed in Phase 1 or 2 |

---

## Transition Plan for data_management_rewrite()

The `data_management_rewrite()` hack in `orchestrator.rs` (lines 180-319) will be replaced incrementally:

1. **Phase 1.5** (shadow): ✅ Sage plane classification runs alongside existing hack, logs comparison
2. **Phase 2.4** (shadow comparison): Sage+Coder results compared with hack-modified results
3. **Phase 2.6** (fast path): Read+Structure bypasses hack entirely for qualifying utterances
4. **Post Phase 2**: Once Sage+Coder accuracy exceeds hack accuracy, gate `data_management_rewrite()` behind `cfg(not(feature = "sage"))` and eventually delete

Functions to eventually delete (~140 lines): `data_management_rewrite()`, `is_data_management_focus()`, `has_explicit_instance_targeting()`, `infer_data_management_domain()`, `should_use_structure_first_prompt()`, `apply_data_management_candidate_policy()`, `is_structure_semantics_verb()`, `is_instance_bound_content_verb()`, `DataManagementRewrite` struct.

---

## Verification

### Unit Tests
```bash
# Phase 1 types + pre-classification
cargo test -p ob-poc --lib sage

# Phase 2 verb index + scoring
cargo test -p ob-poc --lib sage::verb_index
cargo test -p ob-poc --lib sage::verb_resolve
```

### Coverage Harness
```bash
# Sage coverage (plane/polarity/domain accuracy)
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sage_coverage -- --ignored --nocapture

# Comparative coverage (Sage+Coder vs existing pipeline)
# Requires running server:
# DATABASE_URL="postgresql:///data_designer" SAGE_SHADOW=1 cargo run -p ob-poc-web
RUSTC_WRAPPER= \
  cargo test --test utterance_api_coverage -- --ignored --nocapture
```

### Shadow Mode Validation
```bash
# Start server with shadow mode
DATABASE_URL="postgresql:///data_designer" SAGE_SHADOW=1 cargo run -p ob-poc-web

# Watch shadow logs
# Look for: sage_plane, sage_polarity, sage_domain, sage_coder_verb
```

### Pre-commit (no regressions)
```bash
cargo x pre-commit
```

### Success Criteria
| Metric | Target | Measurement |
|--------|--------|-------------|
| Plane accuracy | ≥70% | sage_coverage harness |
| Polarity accuracy | ≥80% | sage_coverage harness |
| Domain accuracy | ≥60% | sage_coverage harness |
| Sage+Coder verb match | ≥50% agreement | shadow comparison logs |
| No regressions | 0 failing tests | cargo x pre-commit |
| Fast path accuracy | ≥ existing | utterance_api_coverage comparative |
