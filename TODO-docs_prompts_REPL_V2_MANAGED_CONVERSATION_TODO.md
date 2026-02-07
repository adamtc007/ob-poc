# Claude TODO — REPL v2 Managed Conversation + Implied Taxonomy Narrowing + Verb Surface Cleanup

> **Status:** EXECUTABLE — Claude Code work order  
> **Date:** 2026-02-07  
> **Repo:** `ob-poc` (Rust workspace)  
> **Primary goal:** Fewer turns + higher first-hit accuracy by *hard narrowing* + *clean verb surface*.

---

## 0) Ground rules / invariants (do not break)

1. **Runbook is the only durable truth.** Session state is a projection/fold over runbook entries.
2. **Staged ≠ executed.** No external side effects unless the user explicitly runs steps.
3. **Control-plane verbs must not pollute business intent.** (session/runbook/agent/view/nav)
4. **Closed-world resolution wherever possible.** If a slot expects an entity, the resolver must pick from a bounded candidate set.
5. **Ambiguity is surfaced, not guessed.** If top candidates are close, ask one question with 2 options.
6. **Determinism first.** Only use LLM fallback when deterministic slot fill fails.

---

## 1) Why we’re doing this (outcome targets)

### 1.1 Targets (measurable)
- Turns per staged DSL statement reduced (goal: **≤ 1.5** avg in guided packs)
- Verb first-hit accept rate increased (goal: **≥ 95%** for in-pack utterances on test corpus)
- Entity correction rate decreases (goal: **≤ 5%** of staged entries)
- “Impossible verb” proposals eliminated via precondition hard-filter

### 1.2 Key idea: “Implied taxonomy narrowing”
As the user walks down:
**CBU → products/instruments → KYC case → proofs/docs**, the valid verb set collapses.
We exploit that with:
- **Pack allowed_verbs** (soft prior)
- **Focus-mode / context** (strong prior / optional hard filter)
- **DAG precondition eligibility** (hard filter, structural truth)

---

## 2) Repo anchors (where to implement)

### REPL v2
- `rust/src/repl/orchestrator_v2.rs`
- `rust/src/repl/session_v2.rs`
- `rust/src/repl/runbook.rs`
- `rust/src/repl/types.rs` (MatchContext contract)
- `rust/src/repl/intent_service.rs` (+ `intent_matcher.rs`)
- `rust/src/repl/verb_config_index.rs`

### Journey packs
- `rust/src/journey/pack.rs`
- pack YAMLs: `rust/config/packs/*.yaml`

### Existing semantic verb search / RAG
- `rust/src/mcp/verb_search.rs` (HybridVerbSearcher)
- `rust/src/session/verb_sync.rs` (VerbSyncService)
- `rust/xtask/src/verbs.rs` (verb tooling)

### Tests
- `rust/tests/repl_v2_golden_loop.rs`
- `rust/tests/repl_v2_phase*.rs`
- `rust/tests/repl_v2_integration.rs`

---

## 3) Phase A — Generate “Verb Atlas” + add lint gates (bleed detector)

### A1) Add `xtask verbs atlas`
**Goal:** produce a single consolidated table that becomes the source for the review + fixes.

**Implement:**
- New xtask command: `cargo xtask verbs atlas`
- Output files:
  - `docs/generated/verb_atlas.md`
  - `docs/generated/verb_atlas.json` (for tooling)
  - `docs/generated/verb_phrase_collisions.md`

**Atlas columns (minimum):**
- `fqn` (domain.verb)
- `tier` (intent|internal|template_only|…)
- `domain` (schema domain)
- `primary_domain` (coarse: cbu|kyc|proofs|trading|… — computed mapping for now)
- `invocation_phrases_count`
- `produces` / `consumes` summary
- `lifecycle.precondition_checks`
- `execution_mode` / `confirm_policy` (if available)
- `handler/behavior` (if configured)

**Sources:**
- verb YAMLs under `rust/config/verbs/**.yaml`
- lexicon: `rust/config/lexicon/domains.yaml` and `verb_concepts.yaml`

### A2) Add lint: phrase collisions + weak intent patterns
Add an xtask (or CI check) that fails on:
- Exact normalized invocation phrase used by **2+** verbs that are not declared aliases
- `tier: intent` verb with **0** invocation phrases unless `template_only: true`
- Control-plane domain verbs appearing in business packs’ `allowed_verbs`
- Schema domain not mapped into lexicon domain hierarchy (see Phase B)

### A3) Add “alias groups” (minimal)
**Goal:** allow legacy verbs/phrases to exist without semantic duplication.

YAML extension (minimal):
```yaml
metadata:
  canonical_verb: "kyc.create-case"   # if this verb is an alias
  tier: intent
```

Lint rule:
- if two verbs share phrase(s), either:
  - one declares `canonical_verb`, or
  - both declare the same `alias_group` (optional), or
  - collision is an error

---

## 4) Phase B — Reconcile the two intent surfaces (weakest link fix)

**Problem:** You currently have two “intent pattern surfaces”:
1) verb schema YAML `invocation_phrases` (`rust/config/verbs/**`)
2) lexicon `rust/config/lexicon/verb_concepts.yaml`

### B1) Decide canonical source of intent patterns
Pick one (recommendation: schema YAML `invocation_phrases` because it’s co-located with verb config).

Then:
- Generate the other surface mechanically OR
- Treat `verb_concepts.yaml` as “curated public surface” that *must* map onto schema verbs.

### B2) Add a reconciliation check
New xtask: `cargo xtask lexicon reconcile`
- Report:
  - concepts that map to missing schema verbs
  - schema verbs marked `tier=intent` not present in concepts (if you require curation)
  - domains in schema not represented in lexicon domain inference

### B3) Wire VerbSyncService on startup (if not already)
Confirm `VerbSyncService.sync_all_with_phrases()` runs in the server startup path.
If not, add it in `rust/crates/ob-poc-web/src/main.rs` (or the central API bootstrap).

---

## 5) Phase C — Extend MatchContext to carry pack + focus + planning state

### C1) Extend `MatchContext` contract
File: `rust/src/repl/types.rs`

Add fields (minimal set):
- `pack_id: Option<String>`
- `allowed_verbs: Vec<String>`
- `forbidden_verbs: Vec<String>`
- `template_expected_verb: Option<String>`
- `focus_mode: Option<String>` (e.g. "cbu"|"products"|"instrument"|"kyc_case"|"proofs")
- `eligibility_mode: EligibilityMode` (`Executable`|`Plan`)
- `exclusions: Vec<Exclusion>` (short-lived negative constraints)
- (keep existing: scope, dominant_entity_id, bindings, domain_hint…)

### C2) Populate MatchContext in OrchestratorV2
File: `rust/src/repl/orchestrator_v2.rs`
- Update `build_match_context()` to:
  - set `pack_id`, `allowed_verbs`, `forbidden_verbs` from `session.journey_context.pack`
  - set `template_expected_verb` from `session.journey_context.template_id` + template progress (see Phase F)
  - set `eligibility_mode` based on current REPL state (in pack staging → Plan)
  - set `focus_mode` from session-derived focus (see Phase F)

---

## 6) Phase D — Create a production `IntentMatcher` implementation for REPL v2

**Today:** REPL v2 has the `IntentMatcher` trait but production wiring is missing (tests only).
We need a real implementation that calls existing semantic search.

### D1) Add `HybridIntentMatcherV2`
New file: `rust/src/repl/hybrid_intent_matcher_v2.rs`

Implement `IntentMatcher` by wrapping:
- `crate::mcp::verb_search::HybridVerbSearcher` (preferred reuse)
- or DB-based verb discovery if that is already stable

Pipeline inside matcher:
1) Get top-k candidates from semantic search (k=20)
2) Apply **forbidden_verbs hard filter**
3) Apply **DAG eligibility hard filter** (Phase E)
4) Apply **pack priors** (boost allowed, penalize out-of-pack)
5) Apply **focus-mode priors** (boost matching contexts)
6) Apply **template expected boost** (if available)
7) Enforce ambiguity policy (margin check) and return:
   - Matched
   - Ambiguous (top 2)
   - NoMatch (below threshold)

### D2) Wire into IntentService and OrchestratorV2 (production)
- Construct `IntentService` with the new matcher + `VerbConfigIndex`
- Ensure `ReplOrchestratorV2` is built with:
  - `.with_verb_config_index(...)`
  - `.with_intent_service(...)`
  - (optional) `.with_proposal_engine(...)`

**Where to wire:** `rust/crates/ob-poc-web/src/main.rs` (see Phase H)

---

## 7) Phase E — DAG Precondition Filter (hard eligibility narrowing)

**Addendum exists** (DAG precondition filter work order). Implement it directly in the v2 matcher.

### E1) Extend VerbConfigIndex to expose lifecycle/provides/consumes
File: `rust/src/repl/verb_config_index.rs`

Extend `VerbIndexEntry` to include:
- `produces: Option<Produces>`
- `consumes: Vec<Consumes>`
- `lifecycle: Option<Lifecycle>` (specifically `precondition_checks`)

### E2) Add `EligibilityEngine`
New file: `rust/src/repl/eligibility.rs`

Responsibilities:
- Build a **fact set** from the runbook fold:
  - `EligibilityMode::Executable`: executed-only
  - `EligibilityMode::Plan`: executed + staged (apply produces/consumes as “planned facts” without side effects)

Implement:
- `fn eligible_verbs(ctx: &MatchContext, runbook: &Runbook, index: &VerbConfigIndex) -> HashSet<String>`
- `fn missing_preconditions(verb: &str, ...) -> Vec<MissingPrereq>`

### E3) Integrate eligibility into verb matching
In `HybridIntentMatcherV2.match_intent()`:
- Prefer pre-filter (best) if semantic search supports candidate restrictions.
- Otherwise post-filter after search but before ambiguity checks.

### E4) “Why not?” UX: suggest the prerequisite verb
If user requests an ineligible verb:
- reply with a *single* explanation (“blocked: missing X”)
- propose staging the prerequisite step
- ask for exactly one missing slot needed to stage it

---

## 8) Phase F — Focus-mode / implied taxonomy gating (CBU → products → instruments → KYC → proofs)

### F1) Add focus derivation from runbook
Add a lightweight derived focus object in `ReplSessionV2`:
- active_cbu_id / name
- active_case_id
- active_instrument_matrix_id (if any)
- active_proof_request_id (if any)
- `focus_mode` computed from these + pack id + pack progress

### F2) Add verb “contexts” metadata (soft to start)
Option A (preferred): add to verb YAML schema:
```yaml
metadata:
  focus_contexts: ["kyc_case", "proofs"]
  primary_domain: "kyc"
```

Option B (fallback): infer from `domain` prefix mapping until YAML is updated.

### F3) Apply focus-mode narrowing in matcher
- strong boost to verbs that match focus_mode
- optional hard filter in guided/template mode only

### F4) Use pack templates/progress to compute `template_expected_verb`
If `session.journey_context.template_id` is active:
- locate current step (pack progress)
- set `template_expected_verb` in MatchContext
- apply boost

---

## 9) Phase G — Conversation discipline affordances (reduce back-and-forth)

### G1) Enforce “ask only one missing slot”
When proposing a step:
- if required args missing → ask for exactly one
- do not ask forms / multiple slots in one turn

### G2) Two-option ambiguity prompt
If ambiguity:
- show exactly top 2 options with discriminators
- single “which one?” question

### G3) Power-user commands
Add REPL commands (if not already):
- `options`, `why`, `undo`, `focus <entity>`, `run step N`

---

## 10) Phase H — Wire REPL v2 into the server (if not already)

File: `rust/crates/ob-poc-web/src/main.rs`

Tasks:
1) Load packs from `rust/config/packs/*.yaml` into `PackRouter`
2) Build `VerbConfigIndex::from_verbs_config(...)`
3) Construct `HybridVerbSearcher` (existing) + new `HybridIntentMatcherV2`
4) Build `IntentService`
5) Build `ReplOrchestratorV2` and inject:
   - verb_config_index
   - intent_service
   - (proposal_engine optional)
6) Merge v2 router into axum routes:
   - `crate::api::create_repl_v2_router()` with `ReplV2RouteState { orchestrator }`

Acceptance:
- `/api/repl/v2/session` works and returns Journey options after scope gate
- `/api/repl/v2/session/{id}/input` uses the new matcher

---

## 11) Phase I — Tests (must update / add)

### I1) Add tests for pack priors + forbidden hard filter
- utterance that matches forbidden verb should never be proposed

### I2) Add tests for DAG precondition filter
- request verb that is impossible → should not appear in candidates
- “why not” suggests prerequisite

### I3) Add tests for planning-mode eligibility
- multi-step utterance can stage follow-on steps even before `run` (Plan mode)

### I4) Add tests for focus-mode narrowing
- in “proofs” focus, “assign product” is downweighted / filtered in guided mode

---

## 12) Phase J — Telemetry + replay harness (tuning)

Add structured logs (redacted) for each match:
- utterance hash (not raw text by default)
- pack_id, focus_mode, eligibility_mode
- top-k before/after filters (verb IDs + scores)
- reason for rejection (forbidden / ineligible / below threshold / ambiguous)
- user accept/reject outcome

Optional:
- `cargo xtask replay-intent` that replays a captured log and reports metrics.

---

## 13) Definition of Done

- REPL v2 uses a production IntentMatcher implementation (not test-only).
- Pack allowed_verbs influences scoring; forbidden_verbs is a hard filter.
- DAG eligibility is enforced as a hard filter.
- Focus-mode narrowing is implemented (soft boost + optional guided hard filter).
- Verb Atlas + lint exist and run in CI.
- New tests pass and cover:
  - forbidden
  - preconditions
  - planning-mode staging
  - ambiguity two-option prompts

---

## Appendix: Quick “where to start” ordering

1) Phase C (MatchContext extensions)
2) Phase D (HybridIntentMatcherV2 + wiring in tests)
3) Phase E (EligibilityEngine + precondition filter)
4) Phase H (server wiring)
5) Phase A/B (atlas + lint + reconciliation)
6) Phase F/G (focus-mode + convo discipline)
7) Phase I (test hardening) + Phase J (telemetry)

