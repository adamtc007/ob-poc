# Scenario-Based Intent Resolution — Implementation Plan

## Context

The intent pipeline currently resolves all utterances to single DSL verbs. Macros (47 total, 18 multi-verb) only get exact label/FQN matching in Tier 0, while DSL verbs (653) benefit from 8 tiers of search (lexicon, learned phrases, semantic embeddings, phonetic). This asymmetry causes the pipeline to miss composite intents like "Onboard a Luxembourg SICAV" — falling through to a single verb (`fund.create-umbrella` at 0.80) instead of the correct macro (`struct.lux.ucits.sicav` — 13 verbs producing a full runbook).

**Goal:** Add two new intent tiers giving macros search parity and enabling composite intent recognition, while maintaining determinism and zero regression on single-verb hit rates.

**Source spec:** `docs/todo/Scenario-Based Intent Resolution.md` (v0.3)

---

## Phase 0.5: MacroIndex — Macro Search Parity (Tier -2B)

**Highest ROI. Do first. Replaces current primitive Tier 0 `search_macros()` method.**

### New File: `rust/src/mcp/macro_index.rs`

Build a deterministic searchable index over all 47 macros, derived from existing macro metadata at startup:

```rust
pub struct MacroIndex {
    // O(1) fast-path lookups
    fqn_map: HashMap<String, String>,           // normalized_fqn → canonical_fqn
    label_map: HashMap<String, Vec<String>>,     // normalized_label → [fqn]
    alias_map: HashMap<String, Vec<String>>,     // curated alias → [fqn]

    // Derived from macro metadata
    jurisdiction_map: HashMap<String, Vec<String>>,  // "LU" → [struct.lux.ucits.sicav, ...]
    noun_map: HashMap<String, Vec<String>>,          // "sicav" → [struct.lux.ucits.sicav, ...]
    mode_index: HashMap<String, Vec<String>>,        // "onboarding" → [macros with that mode_tag]

    // Full macro metadata for scoring
    entries: HashMap<String, MacroIndexEntry>,
}

pub struct MacroIndexEntry {
    pub fqn: String,
    pub label: String,
    pub description: String,
    pub jurisdiction: Option<String>,       // Extracted from FQN: struct.lux.* → "LU"
    pub structure_type: Option<String>,     // Extracted from FQN: *.sicav → "sicav"
    pub mode_tags: Vec<String>,
    pub operates_on: Option<String>,
    pub produces: Option<String>,
    pub aliases: Vec<String>,              // From curated overrides
    pub noun_tokens: Vec<String>,          // Derived from label + description tokenization
}
```

**MacroIndex scoring (deterministic, from spec §4.4):**

| Signal | Score |
|--------|-------|
| Exact FQN match | +10 |
| Exact label match | +8 |
| Alias/phrase match | +6 |
| Jurisdiction match | +3 |
| Mode match | +2 |
| Noun overlap | +2 |
| Target kind match | +2 |
| Mismatch penalty | −999 (hard exclude) |

**Hard gates:** M1 (mode compatibility), M2 (min score ≥ 6), M3 (disambiguation band Δ ≤ 2 → DecisionPacket)

**Result type:**

```rust
pub struct MacroMatch {
    pub fqn: String,
    pub score: i32,
    pub explain: MacroExplain,
}

pub struct MacroExplain {
    pub matched_signals: Vec<MatchedSignal>,
    pub gates: Vec<GateResult>,
    pub score_total: i32,
    pub resolution_tier: &'static str,  // "Tier2B_MacroIndex"
}
```

**Building the index:** `MacroIndex::from_registry(registry: &MacroRegistry, overrides: Option<&MacroSearchOverrides>)` — derive all search terms from macro metadata, optionally overlay curated phrases from `rust/config/macro_search_overrides.yaml`.

### Modifications to Existing Files

**`rust/src/mcp/verb_search.rs`:**
- Add `macro_index: Option<Arc<MacroIndex>>` field to `HybridVerbSearcher`
- Add `with_macro_index(self, index: Arc<MacroIndex>) -> Self` builder method
- Replace current `search_macros()` call (lines 502-530) with `MacroIndex::resolve()` call
- Position Tier -2B AFTER the ECIR single-verb short-circuit (line 478) but BEFORE Tier 0 lexicon search (line 532)
- When MacroIndex returns a match with score ≥ 6, convert to `VerbSearchResult` with `source: VerbSearchSource::MacroIndex` and score 0.96 (higher than ECIR 0.95, lower than exact 1.0)
- When MacroIndex returns 2-3 candidates within disambiguation band (Δ ≤ 2), return them all for `VerbSearchOutcome::Ambiguous`

**`rust/src/mcp/verb_search_factory.rs`:**
- Add `macro_index: Option<Arc<MacroIndex>>` parameter to `build()`
- Wire `with_macro_index()` on the searcher

**`rust/crates/ob-poc-web/src/main.rs`:**
- After loading `MacroRegistry`, build `MacroIndex::from_registry(&registry, overrides)`
- Pass `Arc<MacroIndex>` into `VerbSearcherFactory::build()`

**`rust/src/mcp/verb_search.rs` (VerbSearchSource enum):**
- Add variant `MacroIndex` (distinct from existing `Macro` which becomes unused)

### Files to Create
- `rust/src/mcp/macro_index.rs` — MacroIndex struct, scoring, resolve
- `rust/config/macro_search_overrides.yaml` — initially empty, optional curated aliases

### Files to Modify
- `rust/src/mcp/verb_search.rs` — replace Tier 0 search_macros with MacroIndex integration
- `rust/src/mcp/verb_search_factory.rs` — add macro_index parameter
- `rust/src/mcp/mod.rs` — add `pub mod macro_index;`
- `rust/crates/ob-poc-web/src/main.rs` — build and wire MacroIndex at startup

### Gate
MacroIndex resolves utterances like "set up Lux SICAV" to `struct.lux.ucits.sicav`. `cargo test --lib` passes. No regression in verb-level hit rate (existing intent_hit_rate harness).

---

## Phase 1: ScenarioIndex + Scoring Ledger (Tier -2A)

### New File: `rust/src/mcp/scenario_index.rs`

Load scenario definitions from YAML, evaluate deterministic scoring ledger with hard gates.

```rust
pub struct ScenarioIndex {
    scenarios: Vec<ScenarioDef>,
}

pub struct ScenarioDef {
    pub id: String,
    pub title: String,
    pub modes: Vec<String>,
    pub requires: RequiresGate,             // any_of / all_of compound signal gates
    pub signals: SignalConfig,              // actions, jurisdictions, nouns_any, phrases_any
    pub routes: ScenarioRoute,             // macro | macro_sequence | macro_selector
    pub explain: ExplainConfig,
}

pub enum ScenarioRoute {
    Macro { macro_fqn: String },
    MacroSequence { macros: Vec<String> },
    MacroSelector { select: SelectorConfig, then: Vec<String> },
}
```

**Scoring ledger (from spec §4.2):**

| Signal Bucket | Score |
|---------------|-------|
| Compound outcome verb (onboard, set up, establish) | +4 |
| Jurisdiction found | +4 |
| Structure noun (sicav, icav, LP) | +3 |
| Phase noun (KYC, screening, mandate) | +2 |
| Quantifier ("three sub-funds") | +2 |
| Macro metadata match | +3 |
| Negative: single-verb cue | −6 |

**Hard gates:** G1 (compound signal required), G2 (mode compatibility), G3 (min score ≥ 8)

### New File: `rust/src/mcp/compound_intent.rs`

Shared feature extraction used by both ScenarioIndex and MacroIndex:

```rust
pub struct CompoundSignals {
    pub has_compound_action: bool,       // "onboard", "set up", "establish"
    pub compound_action: Option<String>,
    pub jurisdiction: Option<String>,     // "LU", "IE", "UK", "US"
    pub structure_nouns: Vec<String>,     // "sicav", "icav", "LP"
    pub phase_nouns: Vec<String>,         // "kyc", "screening"
    pub has_quantifier: bool,            // "three sub-funds", "all roles"
    pub has_jurisdiction_structure_pair: bool,
    pub has_multi_noun_workflow: bool,
}

pub fn extract_compound_signals(utterance: &str) -> CompoundSignals;
pub fn extract_jurisdiction(utterance: &str) -> Option<String>;
```

**Jurisdiction extraction:** Static map of aliases → ISO codes (Luxembourg/Lux → LU, Ireland/Irish → IE, etc.)

**Compound action detection:** Static list of outcome verbs: onboard, "set up", establish, "spin up", configure, "do the", "run the", "complete the".

### New File: `rust/config/scenario_index.yaml`

Start with ~5-10 scenarios covering existing jurisdiction macros:
- `lux-sicav-setup` → `struct.lux.ucits.sicav`
- `ie-icav-setup` → `struct.ie.ucits.icav`
- `full-screening` → `[case.open, screening.full]` (macro_sequence)
- `full-onboarding-journey` → macro_selector by jurisdiction + then macros

### ECIR Single-Verb Blocker Integration

In `HybridVerbSearcher::search()`, after the existing ECIR Tier -1 code (line 433-500), add the single-verb blocker check:

```
If ECIR produced exactly 1 candidate (would short-circuit)
AND compound_signals shows:
  - no quantifier
  - no jurisdiction+structure pair
  - no multi-noun workflow signals
→ Skip Tier -2 entirely, resolve at ECIR (existing short-circuit path)
```

This means: extract compound signals BEFORE the ECIR check, and gate the ECIR short-circuit to also fire when compound signals are absent. When compound signals ARE present, suppress the ECIR short-circuit and let the utterance flow to Tier -2A/B.

### Pipeline Order in `search()`

```
1. Extract features ONCE: nouns, action, compound_signals, jurisdiction
2. ECIR Tier -1 probe (existing code lines 433-500)
   - If 1 candidate AND no compound signals → short-circuit (existing behavior)
   - If 1 candidate AND compound signals present → save for post-boost, continue to Tier -2
3. Tier -2A: ScenarioIndex.resolve(nouns, action, jurisdiction, compound_signals, mode)
   - If matched → return macro/sequence/selector result
   - If ambiguous → return DecisionPacket with explain payloads
4. Tier -2B: MacroIndex.resolve(nouns, action, jurisdiction, mode) [from Phase 0.5]
5. ECIR Tier -1 noun→verb (existing, for multi-candidate case)
6. Tiers 1+: Existing verb pipeline
```

### New File: `rust/src/mcp/sequence_validator.rs`

Three-valued prereq validation for `macro_sequence` routes:

```rust
pub enum PrereqCheck {
    Pass,
    Fail { missing: String, satisfied_by: Vec<String> },
    Deferred { requires_args: Vec<String> }
}

pub fn validate_macro_sequence(
    macros: &[String],
    registry: &MacroRegistry,
    current_state: &HashSet<String>,
) -> SequenceValidationResult;
```

### Files to Create
- `rust/src/mcp/scenario_index.rs` — ScenarioIndex, scoring ledger, resolver
- `rust/src/mcp/compound_intent.rs` — CompoundSignals extraction, jurisdiction detection
- `rust/src/mcp/sequence_validator.rs` — macro sequence prereq validation
- `rust/config/scenario_index.yaml` — ~5-10 journey scenarios

### Files to Modify
- `rust/src/mcp/verb_search.rs` — insert Tier -2A before Tier -2B; modify ECIR short-circuit to check compound signals
- `rust/src/mcp/verb_search_factory.rs` — add `scenario_index` parameter
- `rust/src/mcp/mod.rs` — add `pub mod scenario_index; pub mod compound_intent; pub mod sequence_validator;`
- `rust/crates/ob-poc-web/src/main.rs` — load scenario_index.yaml, build ScenarioIndex, wire into factory

### Gate
Compound utterances ("Onboard a Luxembourg SICAV") match the correct scenario. Single-verb utterances ("create umbrella fund") are NOT intercepted by Tier -2. `cargo test --lib` passes.

---

## Phase 2: Guided Runbook Building

### Provenance Metadata

When macro expansion creates RunbookEntries (existing `expand_macro()` in `rust/src/dsl_v2/macros/expander.rs`), tag them using the existing `labels: HashMap<String, String>` field:

```rust
entry.labels.insert("origin_kind".into(), "macro".into());
entry.labels.insert("origin_macro_fqn".into(), macro_fqn.into());
// Only if triggered by scenario:
entry.labels.insert("origin_scenario_id".into(), scenario_id.into());
```

No new fields, no new structs — uses existing RunbookEntry labels.

### Orchestrator Integration

In the orchestrator pipeline (`rust/src/agent/orchestrator.rs`), when Tier -2 matches:
1. Expand matched macro(s) via existing `expand_macro()` → RunbookEntries
2. Add provenance labels to each entry
3. Insert entries into runbook
4. Let existing `derive_pending_questions()` drive conversational arg collection
5. Enhance progress narration: "Step 4 of 13: Lux UCITS SICAV Setup"

### Files to Modify
- `rust/src/dsl_v2/macros/expander.rs` — add provenance labels to expanded entries
- `rust/src/agent/orchestrator.rs` — handle Tier -2 results by expanding macro → runbook
- `rust/src/repl/runbook.rs` — progress narration using `origin_scenario_id` label

### Gate
Macro expansion produces runbook entries with provenance labels. PendingQuestions drive arg collection. Progress shows scenario title + step count.

---

## Phase 3: Macro Sequence Orchestration

### Sequence Expansion

For `routes.kind: macro_sequence`, expand macros in order:
1. Validate sequence using `sequence_validator.rs` (Pass/Fail/Deferred)
2. For each macro in sequence: expand → runbook entries (with provenance)
3. For `routes.kind: macro_selector`: resolve jurisdiction → pick macro FQN, then expand `then` macros in order

### Disambiguation

For `macro_selector` without enough context (no jurisdiction in scope):
- Emit `DecisionPacket` with jurisdiction choices (LU, IE, UK, US)
- Each choice includes the macro it would select and its explain payload
- User selects → continue expansion

### Files to Modify
- `rust/src/mcp/scenario_index.rs` — handle macro_sequence and macro_selector route types
- `rust/src/agent/orchestrator.rs` — sequence expansion + DecisionPacket for selector
- `rust/src/mcp/sequence_validator.rs` — full implementation with session state

### Gate
Multi-macro sequences expand correctly. Prerequisite failures produce deterministic error messages. Deferred prereqs don't block sequence acceptance.

---

## Phase 4: Macro Gap Filling

Audit verb surface for common multi-verb patterns lacking a wrapping macro. Create missing macros:
- `screening.full` (wrapping screening.sanctions + pep + adverse-media)
- `kyc-review.complete` (if needed)
- Others identified during Phase 3 gap detection

Then add scenario entries routing to new macros.

### Files to Create/Modify
- New macro YAML files in `rust/config/verb_schemas/macros/`
- `rust/config/scenario_index.yaml` — entries for new macros

---

## Phase 5: Measurement + Tuning

### Test Harness Extension

Add scenario/macro test cases to existing TOML test fixture (`rust/tests/fixtures/intent_test_utterances.toml`):

```toml
[[test]]
utterance = "Onboard this Luxembourg SICAV with three sub-funds"
category = "scenario"
expected_tier = "scenario"
expected_scenario_id = "lux-sicav-setup"
expected_route_target = "struct.lux.ucits.sicav"

[[test]]
utterance = "Set up structure for Lux SICAV"
category = "macro_match"
expected_tier = "macro_index"
expected_route_target = "struct.lux.ucits.sicav"

[[test]]
utterance = "Create an umbrella fund"
expected_verb = "fund.create-umbrella"
category = "direct"
expected_tier = "ecir"
```

Extend the harness to report:
- Scenario match rate (compound utterances correctly routed)
- Macro match rate (single-macro utterances correctly routed)
- False positive rate (single-verb utterances incorrectly intercepted at Tier -2)
- Resolution tier distribution

### Success Criteria (from spec §11)

| Metric | Target |
|--------|--------|
| Compound intent → correct scenario | ≥80% |
| Scenario → correct macro/sequence | ≥90% |
| MacroIndex → correct macro | ≥75% |
| Single-verb NOT intercepted by Tier -2 | ≥95% (<5% FP) |
| No regression in single-verb hit rate | ≥ baseline |
| Explain payload on all Tier -2 resolutions | 100% |

### Files to Modify
- `rust/tests/fixtures/intent_test_utterances.toml` — add scenario/macro test cases
- `rust/src/mcp/noun_index.rs` — extend intent_hit_rate test to cover Tier -2

---

## Key Reuse Points (Existing Code)

| Existing | Reused For |
|----------|-----------|
| `NounIndex::extract()` / `classify_action()` | Feature extraction shared across all tiers |
| `MacroRegistry` (47 macros, 9 YAML files) | Source data for MacroIndex derivation |
| `expand_macro()` (expander.rs) | Macro → RunbookEntry expansion (unchanged) |
| `RunbookEntry.labels` HashMap | Provenance metadata (no new fields) |
| `derive_pending_questions()` | Conversational arg collection |
| `prereqs` / `sets-state` on macros | Sequence validation |
| `DecisionPacket` / `ClarifyVerb` | Disambiguation for ambiguous matches |
| `VerbSearchResult` / `VerbSearchSource` | Return type for MacroIndex results |

## Verification

1. **Unit tests:** Each new module (macro_index, scenario_index, compound_intent, sequence_validator) gets comprehensive unit tests
2. **Integration:** `cargo x pre-commit` passes (format + clippy + unit tests)
3. **Hit rate:** Extend intent_hit_rate TOML corpus with ~20 scenario/macro/blocker test cases
4. **No regression:** Existing 133 ECIR test cases maintain ≥36.8% first-attempt rate
5. **Manual:** "Set up a Lux SICAV" → `struct.lux.ucits.sicav` (not `fund.create-umbrella`)
