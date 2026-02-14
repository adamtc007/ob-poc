# Delta TODO — Make Single Pipeline *Correct* (SemReg Pre-Filter + Binding Disambiguation + Telemetry Fixes)

**Repo:** `ob-poc` (post “single pipeline” refactor)  
**Goal:** Close the remaining correctness gaps so Semantic OS (SemReg) truly governs verb selection and the disambiguation loop is binding. Also fix telemetry and remove misleading bypass logic.

---

## 0) Primary defects to fix (summary)

1) **SemReg filter/boost occurs after DSL generation** → governance is cosmetic.  
2) **Disambiguation selection is not enforced** → user choice can be ignored.  
3) **SemReg deny-all and denied `dsl:`** are treated as “fallback to unfiltered/NL” → bypass.  
4) **SubjectRef misuse** (entity id passed as case id) → SemReg context becomes empty/weak.  
5) **Telemetry still checks `starts_with('(')`** even though direct DSL is `dsl:` → incorrect learning signals.  
6) **Misleading bypass trace** (`direct_dsl_legacy`) remains even though legacy bypass removed.

---

## 1) Add pipeline API to generate DSL for a chosen verb (binding selection)

### 1.1 Add a “forced verb” entrypoint to IntentPipeline
File: `rust/src/mcp/intent_pipeline.rs`

- [ ] Add:
  - `pub async fn process_with_forced_verb(&self, req: IntentRequest, forced_verb_fqn: &str) -> Result<IntentResult, IntentError>`
- [ ] Implementation approach:
  1) Run normal stages up to candidate generation (or reuse existing internal functions):
     - normalization, scope, entity linking, candidate discovery
  2) Replace candidate selection with the forced verb:
     - Validate forced verb exists in current candidates OR is resolvable via registry
     - If invalid: return `IntentError::InvalidSelection { forced_verb_fqn }`
  3) Run argument extraction/DSL generation **only for the forced verb**.
- [ ] Ensure `IntentTrace` records:
  - `forced_verb_fqn`
  - `selection_source = "user_choice" | "policy_gate" | "system"`

**Acceptance**
- Given an ambiguous input and a forced verb, the pipeline emits DSL for exactly that verb.

### 1.2 Add “candidates override” optional variant (if needed)
If easier than forced verb:
- [ ] Add:
  - `process_with_candidates_override(req, candidates: Vec<VerbCandidate>, chosen: VerbCandidateId)`  
But forced verb is usually enough.

---

## 2) Move SemReg filtering/boosting to *before* choosing verb + calling LLM

### 2.1 Refactor orchestrator stages to pre-filter candidates
File: `rust/src/agent/orchestrator.rs`

- [ ] Split orchestration into:
  1) **Stage A**: “discover” — produces candidates + linked entities, no DSL
  2) **Stage B**: “select + fill” — chooses verb (post-SemReg), then calls pipeline to fill args + generate DSL

**Minimal change path**
- [ ] Add a new IntentPipeline method (or internal helper) that returns candidates without generating DSL:
  - `discover_candidates(req) -> CandidateBundle { verb_candidates, entity_candidates, scope, trace_partial }`
- [ ] Or: call existing pipeline candidate discovery function directly if it’s already factored (avoid duplication).

### 2.2 Apply SemReg constraints to candidate set
- [ ] In orchestrator, after candidate discovery:
  - Call `sem_reg::context_resolution::resolve_context(...)`
  - Extract allowed/denied/boost info
  - Apply filter/boost to `verb_candidates`
- [ ] Update trace:
  - `verb_candidates_pre_semreg`
  - `verb_candidates_post_semreg`
  - `semreg_applied=true`
  - `semreg_denied_all=true/false`

### 2.3 Decide selection after SemReg
- [ ] If user already selected a verb (see Section 3):
  - Use `process_with_forced_verb(...)`
- [ ] Else:
  - Choose top candidate **from post-SemReg list**
  - Then call `process_with_forced_verb(...)` for that chosen verb (ensures selection is explicit and testable)

**Acceptance**
- SemReg deny/allow rules change the final chosen verb and the generated DSL.

---

## 3) Make verb disambiguation selection binding

### 3.1 Plumb “selected verb” through orchestrator
Files:
- `rust/src/api/agent_service.rs` (or wherever verb selection requests are handled)
- `rust/src/agent/orchestrator.rs`
- `rust/src/mcp/intent_pipeline.rs`

- [ ] When user responds with option “2”, convert it into a verb FQN (already available in prior response payload)
- [ ] Call orchestrator with `forced_verb_fqn=...`
- [ ] Or bypass orchestrator Stage A and call:
  - `IntentPipeline.process_with_forced_verb(original_req, forced_verb_fqn)`

**Acceptance**
- User selection cannot be overridden by re-running free-form ranking.

---

## 4) Fix SemReg subject reference (Case vs Entity)

### 4.1 Correct SubjectRef type
File: `rust/src/agent/orchestrator.rs` (and any context builder)

- [ ] Do **not** store dominant_entity_id in `case_id`.
- [ ] Add to OrchestratorContext:
  - `case_id: Uuid` (session/case id)
  - `dominant_entity_id: Option<Uuid>`
- [ ] In SemReg resolution:
  - If dominant entity present: use `SubjectRef::EntityId(dominant_entity_id)`
  - Else: use `SubjectRef::CaseId(case_id)` (real case id)
- [ ] Ensure `SubjectRef` enum supports `EntityId` (add if missing).

**Acceptance**
- SemReg context resolution returns non-empty for entity subjects when available.

---

## 5) Eliminate fail-open governance bypasses

### 5.1 SemReg deny-all handling must be explicit
File: `rust/src/agent/orchestrator.rs`

- [ ] If SemReg returns “deny all” (post-filter list empty):
  - If `PolicyGate.strict_semreg` → return outcome `NoAllowedVerbs` with evidence
  - Else → allow fallback but set trace:
    - `semreg_mode="fail_open"`
    - `semreg_denied_all=true`
- [ ] Do not treat deny-all as “SemReg unavailable”.

### 5.2 Denied `dsl:` must not silently fall back to NL interpretation
- [ ] If input starts with `dsl:` and policy disallows direct DSL:
  - return deterministic error/outcome: `DirectDslNotAllowed`
  - do not continue with NL pipeline

**Acceptance**
- No “silent fallback” that can confuse users or create bypass behavior.

---

## 6) Telemetry + bypass trace fixes

### 6.1 Replace `starts_with('(')` telemetry checks
File: `rust/src/api/agent_routes.rs` (feedback capture / proposed_dsl)

- [ ] Replace with:
  - `starts_with("dsl:")`
- [ ] Prefer using orchestrator trace flags rather than re-deriving from input string.

### 6.2 Remove misleading `direct_dsl_legacy` bypass flag
File: `rust/src/agent/orchestrator.rs`

- [ ] Delete any branch that sets bypass flags based on `starts_with('(')`; legacy bypass is gone.
- [ ] Bypass flags should be set only when direct DSL path is actually taken (`dsl:` + allowed).

### 6.3 Ensure trace reflects real behavior
- [ ] If `PolicyGate` blocks something, trace must say so:
  - `blocked_reason`
  - `policy_snapshot`

---

## 7) Tests (add these — they’ll catch regressions forever)

### 7.1 SemReg truly controls verb selection (killer test)
- [ ] Seed SemReg context such that:
  - candidate list contains Verb A and Verb B
  - SemReg denies A, allows B
- [ ] Run orchestrator on utterance that would normally pick A
- [ ] Assert final chosen verb == B and DSL verb == B

### 7.2 Disambiguation binding test
- [ ] Trigger ambiguous result (returns N options)
- [ ] Submit selection “2”
- [ ] Assert DSL verb equals option #2 verb FQN

### 7.3 Denied `dsl:` is explicit (no fallback)
- [ ] With direct DSL disabled:
  - input `dsl:(cbu.list)`
  - assert outcome `DirectDslNotAllowed`

### 7.4 Strict SemReg deny-all fails closed
- [ ] With `strict_semreg=true` and SemReg denies all:
  - assert outcome `NoAllowedVerbs`
  - no DSL generated

---

## 8) “Done” checklist

- [ ] SemReg filters/boosts candidates **before** verb selection + LLM arg fill
- [ ] Disambiguation selection is binding via forced-verb pipeline call
- [ ] SubjectRef is correct (EntityId vs CaseId)
- [ ] Deny-all and denied `dsl:` are explicit; no silent fallback
- [ ] Telemetry uses `dsl:` and/or trace flags
- [ ] New killer tests pass

