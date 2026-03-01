# ECIR Implementation — Claude Code Execution Plan

**Reference:** `docs/ecir-architecture.md`
**Objective:** Entity-Centric Intent Resolution — deterministic Tier -1 verb resolution via SemOS noun→verb cross-linking.
**Target:** First-attempt hit rate ≥80%, deterministic (no-embedding) resolution ≥40%.

---

## Execution Protocol

- Each phase MUST complete fully before the GATE check.
- At each GATE: run the specified command. If it fails, fix before proceeding.
- At each GATE: print `ECIR PHASE N COMPLETE — N% done` and the E-invariant.
- → IMMEDIATELY proceed to the next phase after a passing GATE.
- Do NOT stop between phases. Do NOT ask for confirmation.
- If a phase has sub-steps (A, B, C), complete ALL sub-steps before the GATE.

**E-invariant (must hold at every GATE):**
`cargo test --lib 2>&1 | tail -1` shows `test result: ok` AND
`cargo test --test intent_hit_rate 2>&1 | tail -5` shows no panics.

---

## Phase 0: Subject-Kinds Audit (scanner already derives them)

**Goal:** Verify that the scanner's 3-level fallback produces correct
`subject_kinds` on VerbContractBody, and understand what the NounIndex
needs to map.

### 0A: Audit Scanner Output

Create a diagnostic script that runs the scanner's `scan_verb_contracts()`
over the loaded VerbsConfig and dumps the derived `subject_kinds` for all
653 verbs.

**File:** `rust/src/bin/audit_subject_kinds.rs`

```
Load VerbsConfig from rust/config/verbs/*.yaml
Call scan_verb_contracts() from sem_os_obpoc_adapter::scanner
For each VerbContractBody:
  Print: fqn | metadata.noun | subject_kinds (derived) | source (explicit/produces/lookup/domain)
Sort by source to identify which verbs got heuristic vs explicit kinds.
```

The scanner at `rust/crates/sem_os_obpoc_adapter/src/scanner.rs:110-136`
already has the derivation chain:
1. `metadata.subject_kinds` (explicit — 36 verbs have this)
2. `produces.entity_type` (verb creates something)
3. `args[].lookup.entity_type` (required UUID args with lookup)
4. `domain_to_subject_kind()` (line 408 — domain name heuristic)

**Action:** Run the audit. Review output. Identify verbs where the
heuristic is wrong (e.g., `session.*` verbs getting `subject_kinds: ["session"]`
when they should have no entity filter, or cross-domain verbs getting
the wrong entity type).

### 0B: Fix Heuristic Gaps in Verb YAMLs

For verbs where the scanner heuristic is wrong, add explicit
`metadata.subject_kinds` to the verb YAML. These go in the `metadata:`
block alongside `noun:`, `tier:`, etc.

**Target files:** `rust/config/verbs/*.yaml` (37 domain files)
**Location within YAML:** Under `metadata:` for each verb, add:
```yaml
metadata:
  noun: screening
  subject_kinds: [entity, person, company, trust, partnership]
  phase_tags: [kyc, screening]
```

Priority fixes (likely wrong from heuristic):
- `session.*` verbs — should have empty `subject_kinds` (no entity filter)
- `view.*` verbs — should have empty (applies to all)
- `agent.*` verbs — should have empty (session management)
- `graph.*` verbs — should have empty or very broad
- `batch.*` verbs — should have empty
- `template.*` verbs — should have empty
- Cross-domain verbs where domain default is wrong

**DO NOT** populate every verb manually. Only fix the ones where the
scanner heuristic gets it wrong. The scanner's fallback is correct for
~80% of verbs.

### 0C: Phase_Tags Population

Add `phase_tags` to verb YAMLs using domain→phase mapping:

| Domain Pattern | phase_tags |
|---|---|
| cbu, entity, fund, client-group, legal-entity | [onboarding] |
| screening, ubo, document, control, bods, ownership | [kyc] |
| deal, contract, contract-pack, billing, sla, pricing-config | [deal] |
| trading-profile, investment-manager, matrix-overlay, cash-sweep, booking-* | [trading] |
| lifecycle, temporal, regulatory.* | [monitoring] |
| attribute, attributes, identifier, semantic, graph, rule* | [stewardship] |
| session, view, agent | [navigation] |
| team, user, batch | [administration] |

This is a bulk operation. Write a Python script that reads each YAML,
adds `phase_tags` to `metadata` based on the domain name, and writes back.

**File:** `scripts/populate_phase_tags.py`
**Execution:** `python3 scripts/populate_phase_tags.py rust/config/verbs/`

After running: verify with `cargo test --lib -- test_load_verbs_config`
(if such a test exists, or just `cargo check`).

**GATE 0:** `cargo check` succeeds. Audit script runs and produces
coherent output. No test regressions.
Print: `ECIR PHASE 0 COMPLETE — 15% done`
→ IMMEDIATELY proceed to Phase 1.

---

## Phase 1: NounIndex YAML + Parser

**Goal:** Create the noun taxonomy data file and Rust parser.

### 1A: Create noun_index.yaml

**File:** `rust/config/noun_index.yaml`

Create the file with ~32 nouns. Each noun entry has:
```yaml
nouns:
  share-class:
    aliases: ["share class", "share classes", "ISIN"]
    natural_aliases: ["accumulating", "distributing", "class"]
    entity_type_fqn: "fund.share_classes"
    noun_keys: ["capital", "fund"]
    action_verbs:
      create: [fund.create-share-class, fund.ensure-share-class, capital.share-class.create]
      list:   [fund.list-share-classes, capital.share-class.list]
      read:   [capital.share-class.get-supply]
```

Use the Appendix A table from `docs/ecir-architecture.md` as the
starting template. Cross-reference with the `metadata.noun` dump from
Phase 0A to build the `noun_keys` and `action_verbs` mappings.

**Critical nouns to get right (highest verb count, most common in utterances):**
1. cbu (20 verbs)
2. ubo (25 verbs)
3. entity (21 verbs)
4. fund (20 verbs) — umbrella, subfund, share-class, feeder, master
5. deal (10+42 sub-verbs across deal domain)
6. screening (3 verbs — but very common utterances)
7. lei / gleif (15 verbs)
8. document (13 verbs)
9. ownership (16 verbs)
10. client-group (23 verbs)
11. role (10 verbs)
12. team (22 verbs)

The `action_verbs` mapping is optional. If omitted, ECIR falls back to
matching all verbs whose `metadata.noun` is in the noun's `noun_keys` list.
Only add `action_verbs` where the noun_keys grouping is too coarse (e.g.,
"share-class" needs to distinguish from other "fund" or "capital" nouns).

### 1B: Create NounIndex Rust Module

**File:** `rust/src/mcp/noun_index.rs`
**Add to:** `rust/src/mcp/mod.rs` — add `pub mod noun_index;`

```rust
// Core types:

pub struct NounIndex {
    /// alias (lowercased) → NounEntry
    canonical: HashMap<String, Arc<NounEntry>>,
    /// natural alias (lowercased) → NounEntry  
    natural: HashMap<String, Arc<NounEntry>>,
    /// All aliases sorted by length descending (for longest-match scanning)
    sorted_aliases: Vec<(String, Arc<NounEntry>, bool)>, // (alias, entry, is_canonical)
}

pub struct NounEntry {
    pub key: String,
    pub entity_type_fqn: Option<String>,
    pub noun_keys: Vec<String>,
    pub action_verbs: HashMap<String, Vec<String>>,
}

pub struct NounMatch {
    pub noun: Arc<NounEntry>,
    pub matched_alias: String,
    pub is_canonical: bool,
    pub span: (usize, usize), // character position in utterance
}

pub enum ActionCategory {
    Create, List, Update, Delete, Assign, Compute, Import, Search,
}

impl NounIndex {
    /// Load from YAML file
    pub fn load(path: &Path) -> Result<Self>

    /// Extract nouns from utterance (longest-match-first)
    pub fn extract(&self, utterance: &str) -> Vec<NounMatch>

    /// Classify action from utterance surface patterns
    pub fn classify_action(utterance: &str) -> Option<ActionCategory>

    /// Resolve: given extracted nouns + optional action, return verb FQN candidates
    pub fn resolve(
        &self,
        nouns: &[NounMatch],
        action: Option<ActionCategory>,
        all_verbs: &[VerbContractSummary], // for subject_kinds fallback
    ) -> NounResolution
}

pub struct NounResolution {
    pub candidates: Vec<String>,  // verb FQNs
    pub noun_key: String,         // which noun matched
    pub action: Option<ActionCategory>,
    pub resolution_path: ResolutionPath,
}

pub enum ResolutionPath {
    /// action_verbs explicit mapping — highest confidence
    ExplicitMapping,
    /// noun_keys → metadata.noun match — medium confidence
    NounKeyMatch,
    /// subject_kinds match — lower confidence
    SubjectKindMatch,
    /// No match — fall through
    NoMatch,
}
```

**Resolution chain inside `resolve()`:**
1. If noun has `action_verbs[action]` → return those (ExplicitMapping)
2. If noun has `action_verbs["_default"]` → return those (ExplicitMapping)
3. If noun has `noun_keys` → find all verbs whose `metadata.noun` is in noun_keys (NounKeyMatch)
4. If noun has `entity_type_fqn` → find all verbs whose `subject_kinds` contains it (SubjectKindMatch)
5. No match → return empty (NoMatch)

**The `all_verbs` parameter:** This is a lightweight summary of all registered
verbs with their metadata.noun and subject_kinds. It's built once at startup
from the VerbsConfig or from a SemOS query. It does NOT require a DB call
at resolution time.

### 1C: Action Classifier

Include in `noun_index.rs` (no separate module needed — it's ~40 lines).

```rust
impl NounIndex {
    pub fn classify_action(utterance: &str) -> Option<ActionCategory> {
        let lower = utterance.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let first = words.first().map(|w| *w).unwrap_or("");
        
        // Imperative verb at start
        match first {
            "create" | "add" | "new" | "register" | "establish" | "onboard" | "set" => {
                if first == "set" && words.get(1).map(|w| *w) == Some("up") {
                    return Some(ActionCategory::Create);
                }
                return Some(ActionCategory::Create);
            }
            "show" | "list" | "display" | "get" | "view" => return Some(ActionCategory::List),
            "update" | "change" | "modify" | "edit" | "rename" => return Some(ActionCategory::Update),
            "delete" | "remove" | "drop" | "cancel" | "revoke" | "archive" => return Some(ActionCategory::Delete),
            "assign" | "link" | "connect" | "attach" | "map" | "bind" => return Some(ActionCategory::Assign),
            "compute" | "calculate" | "run" | "check" | "screen" | "verify" | "validate" => return Some(ActionCategory::Compute),
            "import" | "pull" | "fetch" | "sync" | "enrich" | "load" | "ingest" => return Some(ActionCategory::Import),
            "trace" | "find" | "search" | "discover" => return Some(ActionCategory::Search),
            _ => {}
        }
        
        // Question patterns
        match first {
            "who" | "where" => return Some(ActionCategory::Search),
            "what" | "how" => {
                if words.get(1).map(|w| *w) == Some("many") {
                    return Some(ActionCategory::List);
                }
                return Some(ActionCategory::List);
            }
            _ => {}
        }
        
        None // Unknown — ECIR still works without action, just broader candidate set
    }
}
```

### 1D: Unit Tests

**File:** `rust/src/mcp/noun_index.rs` (inline `#[cfg(test)] mod tests`)

Test cases:
1. `NounIndex::load()` — parses noun_index.yaml without error
2. `extract("create a new share class")` → NounMatch { key: "share-class", matched_alias: "share class" }
3. `extract("check for OFAC hits")` → NounMatch { key: "sanctions", matched_alias: "OFAC" }
4. `extract("who are the beneficial owners?")` → NounMatch { key: "ubo", matched_alias: "beneficial owner" }
5. `classify_action("create a new CBU")` → Some(Create)
6. `classify_action("who controls this?")` → Some(Search)
7. `classify_action("what documents are missing?")` → Some(List)
8. `resolve()` with single-verb mapping → 1 candidate
9. `resolve()` with multi-verb mapping → 2-5 candidates
10. `resolve()` with no noun → empty candidates

**GATE 1:** `cargo test --lib -- noun_index` passes all 10+ tests.
`cargo check` succeeds. E-invariant holds.
Print: `ECIR PHASE 1 COMPLETE — 45% done`
→ IMMEDIATELY proceed to Phase 2.

---

## Phase 2: Verb Contract Summary Cache

**Goal:** Build the startup-time cache that maps metadata.noun and
subject_kinds to verb FQNs, so NounIndex.resolve() has O(1) lookup
without DB queries at resolution time.

### 2A: VerbContractSummary

**File:** `rust/src/mcp/noun_index.rs` (add to existing module)

```rust
/// Lightweight verb metadata for noun→verb resolution.
/// Built once at startup from VerbsConfig.
pub struct VerbContractSummary {
    pub fqn: String,
    pub domain: String,
    pub action: String,
    pub description: String,
    pub noun: Option<String>,        // metadata.noun
    pub subject_kinds: Vec<String>,  // derived or explicit
    pub phase_tags: Vec<String>,
}

/// Index for fast noun→verb lookup at resolution time.
pub struct VerbContractIndex {
    /// metadata.noun → vec of verb FQNs
    by_noun: HashMap<String, Vec<String>>,
    /// subject_kind → vec of verb FQNs
    by_subject_kind: HashMap<String, Vec<String>>,
    /// All summaries by FQN
    by_fqn: HashMap<String, VerbContractSummary>,
}

impl VerbContractIndex {
    /// Build from loaded VerbsConfig using scanner's derivation logic.
    pub fn from_verbs_config(config: &VerbsConfig) -> Self {
        // Use sem_os_obpoc_adapter::scanner::scan_verb_contracts() to get
        // VerbContractBody with derived subject_kinds, then extract summaries.
        ...
    }
}
```

### 2B: Wire VerbContractIndex into NounIndex

Modify `NounIndex` to hold `Arc<VerbContractIndex>`:

```rust
pub struct NounIndex {
    canonical: HashMap<String, Arc<NounEntry>>,
    natural: HashMap<String, Arc<NounEntry>>,
    sorted_aliases: Vec<(String, Arc<NounEntry>, bool)>,
    verb_index: Arc<VerbContractIndex>,  // ← NEW
}
```

Update `resolve()` to use `verb_index` for the NounKeyMatch and
SubjectKindMatch resolution paths.

### 2C: Integration into Startup

Find where `HybridVerbSearcher` is constructed. This is in
`rust/src/mcp/verb_search_factory.rs` (or wherever the builder is called).

Load the NounIndex at the same point where VerbsConfig is loaded:
```rust
let noun_index = NounIndex::load(
    &config_dir.join("noun_index.yaml"),
    VerbContractIndex::from_verbs_config(&verbs_config),
)?;
```

Thread `noun_index` into `HybridVerbSearcher` as a new field:
```rust
// In verb_search.rs, HybridVerbSearcher struct (line 232):
pub struct HybridVerbSearcher {
    // ... existing fields ...
    noun_index: Option<Arc<NounIndex>>,  // ← NEW
}
```

Add `with_noun_index()` builder method matching the pattern of
`with_macro_registry()`, `with_lexicon()`, etc.

**GATE 2:** `cargo check` succeeds. NounIndex loads at startup.
`cargo test --lib -- noun_index` still passes.
Print: `ECIR PHASE 2 COMPLETE — 60% done`
→ IMMEDIATELY proceed to Phase 3.

---

## Phase 3: Tier -1 Integration in HybridVerbSearcher

**Goal:** Wire ECIR as the first resolution tier in `search()`.

### 3A: Add VerbSearchSource::NounTaxonomy

**File:** `rust/src/mcp/verb_search.rs`
**Location:** `VerbSearchSource` enum (line 69)

Add:
```rust
/// Noun taxonomy deterministic match (Tier -1, highest priority)
NounTaxonomy,
```

### 3B: Add Tier -1 Block in search()

**File:** `rust/src/mcp/verb_search.rs`
**Location:** Inside `search()` (line 370), BEFORE the Tier 0 macro block (line 420).

Insert after the embedding computation and before the macro search:

```rust
// ── Tier -1: ECIR Noun Taxonomy (deterministic, before all embedding tiers) ──
if let Some(ref noun_index) = self.noun_index {
    let nouns = noun_index.extract(&normalized);
    if !nouns.is_empty() {
        let action = NounIndex::classify_action(&normalized);
        let resolution = noun_index.resolve(&nouns, action);

        match resolution.candidates.len() {
            0 => {
                // No verb candidates from noun — fall through to embedding
                tracing::debug!(
                    noun = %resolution.noun_key,
                    "ECIR: noun matched but no verb candidates, falling through"
                );
            }
            1 => {
                // Deterministic single-verb resolution — skip ALL embedding tiers
                let fqn = &resolution.candidates[0];
                if self.matches_domain(fqn, domain_filter)
                    && allowed_verbs.map_or(true, |av| av.contains(fqn))
                    && !seen_verbs.contains(fqn)
                {
                    tracing::info!(
                        verb = %fqn,
                        noun = %resolution.noun_key,
                        path = ?resolution.resolution_path,
                        "ECIR: deterministic single-verb resolution"
                    );
                    seen_verbs.insert(fqn.clone());
                    results.push(VerbSearchResult {
                        verb: fqn.clone(),
                        score: 0.95,
                        source: VerbSearchSource::NounTaxonomy,
                        matched_phrase: format!("noun:{}", resolution.noun_key),
                        description: noun_index.verb_index
                            .by_fqn.get(fqn)
                            .map(|s| s.description.clone()),
                    });
                    // SHORT-CIRCUIT: return immediately, skip all embedding tiers
                    return self.normalize_candidates(results, query, allowed_verbs);
                }
            }
            n if n <= 5 => {
                // Narrow candidate set — run embedding ONLY within these candidates
                tracing::info!(
                    candidates = n,
                    noun = %resolution.noun_key,
                    action = ?resolution.action,
                    "ECIR: narrow candidate set, constraining embedding search"
                );
                // Add all candidates as baseline results with medium score
                for fqn in &resolution.candidates {
                    if self.matches_domain(fqn, domain_filter)
                        && allowed_verbs.map_or(true, |av| av.contains(fqn))
                        && !seen_verbs.contains(fqn)
                    {
                        seen_verbs.insert(fqn.clone());
                        results.push(VerbSearchResult {
                            verb: fqn.clone(),
                            score: 0.80, // base score, embedding can boost
                            source: VerbSearchSource::NounTaxonomy,
                            matched_phrase: format!("noun:{}", resolution.noun_key),
                            description: noun_index.verb_index
                                .by_fqn.get(fqn)
                                .map(|s| s.description.clone()),
                        });
                    }
                }
                // If embedding is available, re-rank within narrow set
                if let Some(ref query_embedding) = query_embedding {
                    // Compute similarity only against narrow candidate phrases
                    // (not full 653 verbs). This is Phase 5 optimisation —
                    // for now, the base scores + existing ambiguity gate suffice.
                }
                // DON'T short-circuit — let existing tiers run but narrow set
                // is already populated. Existing tiers may find a better match
                // outside the noun set (safety net).
            }
            _ => {
                // 6+ candidates — too many for deterministic resolution.
                // Add as low-priority context boost, let embedding resolve.
                tracing::debug!(
                    candidates = resolution.candidates.len(),
                    noun = %resolution.noun_key,
                    "ECIR: large candidate set, using as context boost"
                );
                // Don't add to results — just use as domain_filter hint
                // for downstream tiers (future enhancement)
            }
        }
    }
}
```

### 3C: Governance Safety Check

ECIR must respect the `allowed_verbs` parameter (which comes from
SessionVerbSurface governance). The code above already does this:
```rust
allowed_verbs.map_or(true, |av| av.contains(fqn))
```

**Verify:** No ECIR path returns a verb that isn't in `allowed_verbs`.
This is Safety Invariant SI-1 from the architecture doc.

### 3D: Test ECIR in Hit Rate Harness

Add ECIR annotation fields to test fixtures.

**File:** `rust/tests/fixtures/intent_test_utterances.toml`

For each existing test case, add optional fields:
```toml
[[test]]
utterance = "create a new umbrella fund"
expected_verb = "fund.create-umbrella"
category = "direct"
difficulty = "easy"
expected_noun = "umbrella"         # NEW
expected_action = "create"         # NEW
ecir_path = "deterministic"        # NEW: deterministic | narrow | fallthrough
```

Don't annotate all 120 — annotate at least the 30 "easy" cases to
validate ECIR is working.

### 3E: ECIR Metrics in Hit Rate Report

**File:** `rust/tests/intent_hit_rate.rs`

Add ECIR-specific counters to the report output:
```
ECIR Resolution:
  Deterministic (1 candidate):  28/120 (23%)
  Narrow (2-5 candidates):      22/120 (18%)
  Fallthrough (0 or 6+):        70/120 (58%)
  
  Noun extraction recall:        52/120 (43%)
  Action classification:         45/52  (87%)
```

**GATE 3:**
- `cargo test --lib` passes (E-invariant)
- `cargo test --test intent_hit_rate -- --nocapture` runs without panics
- ECIR produces at least SOME deterministic resolutions (non-zero)
- No verb outside `allowed_verbs` is returned

Print: `ECIR PHASE 3 COMPLETE — 80% done`
→ IMMEDIATELY proceed to Phase 4.

---

## Phase 4: Tuning + Observability

**Goal:** Tune the NounIndex aliases using hit rate failures. Add
structured logging for production observability.

### 4A: Hit Rate Gap Analysis

Run: `INTENT_VERBOSE=1 cargo test --test intent_hit_rate -- --nocapture 2>&1 | tee /tmp/ecir-report.txt`

For each MISS in the report:
1. Check if the utterance contains a noun that ECIR should have caught
2. If yes → add the missing alias to `noun_index.yaml`
3. If the noun was caught but wrong verb → fix `action_verbs` mapping
4. If no identifiable noun → mark as `ecir_path = "fallthrough"` (not ECIR's job)

Iterate until ECIR deterministic + narrow rate ≥ 65% on the test corpus.

### 4B: Structured Logging

Ensure all ECIR paths have structured tracing:

```rust
tracing::info!(
    noun = %noun_key,
    action = ?action,
    candidates = candidates.len(),
    resolution_path = ?path,
    selected_verb = %verb,
    ecir_score = score,
    "ecir.resolve"
);
```

This enables:
- `grep "ecir.resolve" | jq .resolution_path` → resolution path distribution
- `grep "ecir.resolve" | jq 'select(.candidates == 0)'` → fallthrough analysis
- `grep "ecir.resolve" | jq 'select(.resolution_path == "ExplicitMapping")'` → deterministic hits

### 4C: Threshold Tuning

The key thresholds:
- ECIR deterministic score: `0.95` (single verb match)
- ECIR narrow base score: `0.80` (2-5 candidates)
- ECIR minimum candidates for short-circuit: `1`
- ECIR maximum candidates for narrow path: `5`

These are reasonable defaults. Tune only if hit rate data shows issues:
- If deterministic resolution is correct but ambiguity gate rejects → raise to 0.98
- If narrow set misses the correct verb → check noun_keys coverage
- If narrow set too often includes wrong verbs → tighten action_verbs mappings

**GATE 4:**
- First-attempt hit rate ≥ 70% (stretch: 80%)
- ECIR deterministic resolution ≥ 30% (stretch: 40%)
- No test regressions (`cargo test --lib`)
- Structured logging present on all ECIR paths

Print: `ECIR PHASE 4 COMPLETE — 95% done`
→ IMMEDIATELY proceed to Phase 5.

---

## Phase 5: Cleanup + Documentation

### 5A: Code Cleanup

- Remove any `#[allow(dead_code)]` added during development
- Run `cargo clippy` and fix warnings in new code
- Ensure all public types have doc comments

### 5B: Update Architecture Doc

**File:** `docs/ecir-architecture.md`

Update Section 9 (Measurement) with actual hit rate numbers.
Note any deviations from the original architecture.

### 5C: Update Hit Rate Strategy Doc

**File:** `docs/intent-hit-rate-strategy.md`

Add note that ECIR (Phase 0 in strategy doc, now implemented) subsumes
Phase 3 (contextual scoring) and reduces scope of Phase 2 (LLM classifier).

**GATE 5:**
- `cargo clippy --all-targets` — zero warnings in `noun_index.rs`
- All docs updated
- `cargo test --lib && cargo test --test intent_hit_rate` both pass

Print: `ECIR IMPLEMENTATION COMPLETE — 100%`

---

## File Manifest

| Phase | File | Action |
|---|---|---|
| 0A | `rust/src/bin/audit_subject_kinds.rs` | CREATE — diagnostic script |
| 0B | `rust/config/verbs/*.yaml` (~10 files) | EDIT — add explicit subject_kinds where heuristic is wrong |
| 0C | `scripts/populate_phase_tags.py` | CREATE — bulk add phase_tags |
| 0C | `rust/config/verbs/*.yaml` (all 37 files) | EDIT — add phase_tags via script |
| 1A | `rust/config/noun_index.yaml` | CREATE — 32 nouns, ~160 aliases |
| 1B | `rust/src/mcp/noun_index.rs` | CREATE — NounIndex, NounEntry, VerbContractIndex, ActionClassifier |
| 1B | `rust/src/mcp/mod.rs` (line 56) | EDIT — add `pub mod noun_index;` |
| 2C | `rust/src/mcp/verb_search.rs` (line 232) | EDIT — add `noun_index: Option<Arc<NounIndex>>` field |
| 2C | `rust/src/mcp/verb_search.rs` | EDIT — add `with_noun_index()` builder |
| 2C | `rust/src/mcp/verb_search_factory.rs` | EDIT — load NounIndex, pass to builder |
| 3A | `rust/src/mcp/verb_search.rs` (line 69) | EDIT — add `NounTaxonomy` variant to enum |
| 3B | `rust/src/mcp/verb_search.rs` (line ~420) | EDIT — insert Tier -1 block before Tier 0 |
| 3D | `rust/tests/fixtures/intent_test_utterances.toml` | EDIT — add ECIR annotations |
| 3E | `rust/tests/intent_hit_rate.rs` | EDIT — add ECIR metrics to report |
| 5B | `docs/ecir-architecture.md` | EDIT — update with actual metrics |
| 5C | `docs/intent-hit-rate-strategy.md` | EDIT — note ECIR subsumption |

---

## Critical Integration Points (exact locations)

| What | File | Line | Notes |
|---|---|---|---|
| VerbSearchSource enum | `rust/src/mcp/verb_search.rs` | 69 | Add `NounTaxonomy` variant |
| VerbSearchResult struct | `rust/src/mcp/verb_search.rs` | 58 | No change needed |
| HybridVerbSearcher struct | `rust/src/mcp/verb_search.rs` | 232 | Add `noun_index` field |
| search() entry point | `rust/src/mcp/verb_search.rs` | 370 | Insert Tier -1 before line ~420 |
| Tier 0 macro search | `rust/src/mcp/verb_search.rs` | ~420 | ECIR goes BEFORE this |
| VerbMetadata.subject_kinds | `rust/crates/dsl-core/src/config/types.rs` | 348 | Already exists, no change |
| VerbMetadata.phase_tags | `rust/crates/dsl-core/src/config/types.rs` | 353 | Already exists, no change |
| Scanner subject_kinds derivation | `rust/crates/sem_os_obpoc_adapter/src/scanner.rs` | 110-136 | Already has 3-level fallback |
| Scanner domain_to_subject_kind | `rust/crates/sem_os_obpoc_adapter/src/scanner.rs` | 408 | May need fixes for session/view/agent |
| mcp/mod.rs | `rust/src/mcp/mod.rs` | 56 | Add `pub mod noun_index;` |
| VerbContractBody.metadata.noun | `rust/src/sem_reg/verb_contract.rs` | ~170 | Already populated on 648/653 verbs |
| intent_hit_rate test harness | `rust/tests/intent_hit_rate.rs` | 1 | Add ECIR metrics |
| Test fixtures | `rust/tests/fixtures/intent_test_utterances.toml` | 1 | Add ECIR annotations |

---

## Dependencies

| Crate | Purpose | Already in Cargo.toml? |
|---|---|---|
| serde + serde_yaml | Parse noun_index.yaml | Yes |
| tracing | Structured logging | Yes |
| Arc, HashMap | In-memory index | std |

No new external dependencies required. NounIndex uses only types already
in the dependency graph.

---

## Risk Mitigations

1. **If NounIndex YAML parse fails at startup:** `noun_index` field is
   `Option<Arc<NounIndex>>`. If loading fails, log warning and continue —
   all tiers below Tier -1 work exactly as before. Zero regression risk.

2. **If ECIR resolves to wrong verb:** The `normalize_candidates()` call
   at the end of search() applies the ambiguity gate. If a single wrong
   verb is returned with score 0.95 but the user utterance doesn't match
   any phrase for that verb, subsequent tiers may override. BUT the
   short-circuit path skips subsequent tiers — this is the one risk.
   **Mitigation:** Only short-circuit on ExplicitMapping path (hand-curated
   action_verbs), not on NounKeyMatch or SubjectKindMatch.

3. **If subject_kinds heuristic is wrong for some verbs:** The ECIR
   SubjectKindMatch path is lowest priority. ExplicitMapping and
   NounKeyMatch are preferred. Wrong subject_kinds just means a slightly
   wider candidate set, not wrong resolution.
