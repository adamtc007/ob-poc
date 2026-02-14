# Agentic Scenario Harness — Comprehensive Script Suite + xtask Runner (Claude Code TODO)

**Repo:** `ob-poc`  
**Goal:** Build a deterministic, repeatable **end-to-end** test harness for the *single* orchestrator pipeline using **scripted multi-turn scenarios** (canned “business” phrases).  
The harness must validate **structured invariants** (outcome kind, chosen verb FQN, SemReg mode, run_sheet deltas, trace flags), not LLM prose.  
It should support **stub**, **record/replay**, and **live** modes.

---

## 0) Principles / invariants

1) **No prose assertions.** Assert only on structured fields:
   - `PipelineOutcome` kind
   - `chosen_verb_fqn`, `forced_verb_fqn`, `selection_source`
   - `semreg_mode` + deny/allow signals
   - run_sheet entry count, runnable count, `dsl_hash`
   - trace flags (`macro_semreg_checked`, etc.)

2) **Multi-turn is state-machine driven.** The harness must “reply” to:
   - ClarifyVerb → select option (by index or verb_fqn)
   - ClarifyArgs → provide missing args
   - Scope selection / decisions → provide the correct response
   - Confirmations → respond yes/no

3) **Single-path enforcement.** All scenario runs must call the orchestrator entrypoint (in-process) unless explicitly running HTTP black-box mode.

4) **Determinism & stability.**
   - CI uses **stub** or **replay** mode (no network).
   - Nightly can run **live** LLM smoke tests with relaxed expectations.

5) **PII safe.** Scripts contain fictional entities only (Acme, Globex, John Example, etc).

---

## 1) File structure to add

```
scenarios/
  README.md
  suites/
    kyc_ubo_core.yaml
    documents_core.yaml
    macro_packs.yaml
    governance_strict.yaml
    regression_traps.yaml
    entity_linking.yaml
    scope_switching.yaml
    clarification_loops.yaml
    execution_runsheet.yaml
    semreg_outage.yaml
  fixtures/
    stub_llm/
      # keyed responses for arg extraction etc.
    replay/
      # recorded sessions / LLM responses (generated)
scripts/
  harness/
    schema.md              # scenario YAML schema docs
    examples/
xtask/
  src/
    harness.rs             # runner
    main.rs                # add command wiring
rust/
  src/
    agent/harness/         # (optional) shared structs if used by tests too
```

---

## 2) Scenario YAML schema (v0.1)

Create `scripts/harness/schema.md` + implement loader.

### 2.1 Top-level
```yaml
name: "Human readable title"
suite: "kyc_ubo_core"
mode_expectations:
  strict_semreg: true
  strict_single_pipeline: true
  allow_direct_dsl: false
  allow_raw_execute: false
session_seed:
  scope: "Allianz GI"
  dominant_entity: null
  actor:
    actor_id: "test.user"
    roles: ["viewer"]
    clearance: null
steps:
  - user: "utterance"
    expect:
      outcome: "Matched|ClarifyVerb|ClarifyArgs|NoAllowedVerbs|ScopeResolved|DirectDslNotAllowed|Error"
      chosen_verb: "bny.kyc.document.request"        # optional
      semreg_mode: "applied|deny_all|unavailable|fail_open"  # optional
      selection_source_in: ["discovery","semreg","user_choice","macro"]  # optional
      run_sheet_delta: 1                             # optional
      runnable_delta: 1                              # optional
      trace:
        macro_semreg_checked: true                   # optional
    on_outcome:
      ClarifyVerb:
        choose_index: 2                              # OR choose_verb_fqn: "..."
      ClarifyArgs:
        reply: "passport for John Example"
      ScopeClarify:
        choose_index: 1
```

### 2.2 Notes
- `on_outcome` can contain multiple handlers. The harness picks the matching handler for the actual outcome.
- If no handler exists for an interactive outcome, scenario fails (to avoid silent hangs).
- `expect` supports partial assertions: only check fields provided.

---

## 3) xtask harness runner

### 3.1 CLI commands
Implement in `xtask/src/harness.rs` and wire in `xtask/src/main.rs`:

- `cargo xtask harness list`
  - lists suites and scenarios, with counts

- `cargo xtask harness run --suite suites/kyc_ubo_core.yaml --mode stub`
  - runs all scenarios in the suite

- `cargo xtask harness run --scenario <name> --mode stub`
  - runs one scenario by name across all suites

- `cargo xtask harness run --all --mode stub`
  - runs everything

- `cargo xtask harness watch --suite ... --mode stub`
  - watches files and re-runs changed scenarios (use `notify` crate)

- `cargo xtask harness record --suite ...`
  - runs with live LLM, records outputs into `scenarios/fixtures/replay/...`

- `cargo xtask harness replay --suite ...`
  - replays from recordings (CI-friendly)

- `cargo xtask harness dump --scenario ... --out /tmp/run.json`
  - dumps full per-step artifacts (trace, responses, run_sheet snapshot) for debugging

### 3.2 Modes
- **stub**: uses `StubLlmClient` returning deterministic arg JSON from `fixtures/stub_llm/`.
- **replay**: uses `ReplayLlmClient` reading recorded responses; fails if missing recording.
- **live**: uses real LLM client; assertions should be limited to invariants (outcome kind, allowed verbs, semreg mode) to avoid flakiness.

### 3.3 In-process execution (preferred)
Runner should call orchestrator directly, e.g.:
- create a fresh `AgentState` (db pool, semreg store, policy gate)
- create a session (or call session builder)
- for each step:
  - call `AgentService.process_chat(...)` OR directly `orchestrator.handle_utterance(...)`
  - if response is a DecisionPacket requiring reply, feed the reply step as specified
  - track run_sheet state deltas, runnable counts, last dsl_hash

### 3.4 Optional HTTP mode (secondary)
Add `--http http://127.0.0.1:3001` to run black-box over API endpoints.  
This is useful as a smoke test, but in-process is the CI default.

---

## 4) Wire fixtures for LLM stubbing & replay

### 4.1 Define an `LlmClient` trait (if not already)
If already present, add test implementations.

- `StubLlmClient`
  - keyed by `(prompt_version, verb_fqn, utterance_hash)` or `(verb_fqn, step_name)`
  - returns deterministic JSON (args)

- `ReplayLlmClient`
  - loads a recording file per scenario and step
  - returns exactly what was recorded

### 4.2 Recording format
`scenarios/fixtures/replay/<suite>/<scenario>/<step_idx>.json`:
```json
{
  "request": { "verb_fqn": "...", "prompt_version": "...", "utterance_hash": "..." },
  "response": { "args": { "document_type": "passport", "person_id": "..." } }
}
```

---

## 5) Comprehensive scenario suite (30+ scenarios)

Create the following suites (each suite file contains multiple scenarios).  
**Minimum target:** 35 scenarios total.

### 5.1 `suites/scope_switching.yaml` (6 scenarios)
1) “Switch to Allianz GI”
2) “Switch to BlackRock” (scope exists)
3) Ambiguous scope name → ClarifyScope (choose option)
4) Unknown scope → error/clarify
5) Scope persists across steps (verify)
6) Scope reset / change mid-run (verify run_sheet continues)

### 5.2 `suites/entity_linking.yaml` (6 scenarios)
1) “Find UBO of Acme Holdings” (entity found)
2) Ambiguous “Acme” → ClarifyEntity (choose)
3) Typo: “Acmme Holdings” still resolves
4) Person vs company ambiguity (“John Smith”) → ClarifyEntity
5) Jurisdiction hint resolves (“Acme Luxembourg”) improves match
6) Ensure entity linking doesn’t leak denied entities (ABAC scenario)

### 5.3 `suites/kyc_ubo_core.yaml` (6 scenarios)
1) “Start UBO discovery for Acme Holdings”
2) “List known shareholders for Acme”
3) “Check missing UBO links”
4) “Research corporate tree from registry”
5) “Confirm UBO threshold 25%” (ClarifyArgs threshold)
6) “Generate UBO summary” (report-like verb)

### 5.4 `suites/documents_core.yaml` (6 scenarios)
1) “Request passport for UBO director” (ClarifyArgs for person)
2) “Request proof of address for John Example”
3) “Upload received passport” (should stage doc attach DSL)
4) “Validate passport document” (review verb)
5) “Mark document as verified” (status change)
6) “Request corporate registry extract for Acme” (doc type clarifies)

### 5.5 `suites/clarification_loops.yaml` (5 scenarios)
1) Ambiguous utterance → ClarifyVerb → choose #2
2) ClarifyArgs → provide missing arg, then Matched
3) Two clarifications in sequence (Verb then Args)
4) ClarifyVerb selection is binding (assert forced_verb_fqn)
5) ClarifyArgs where user gives irrelevant answer → clarify again

### 5.6 `suites/macro_packs.yaml` (4 scenarios)
1) “Standard UBO discovery pack for Acme” → MacroExpanded → stages multiple entries
2) “Corporate documents pack for Acme” → stages doc requests
3) Macro expands to 2+ verbs, verify each verb is SemReg allowed
4) Macro expansion formatting stress (multi-line, comments) still governed

### 5.7 `suites/governance_strict.yaml` (4 scenarios)
1) Strict SemReg denies verb → NoAllowedVerbs
2) Strict SemReg deny-all → NoAllowedVerbs
3) Strict SemReg unavailable/outage simulated → fail-closed
4) Non-strict mode → fail-open flagged in trace (semreg_mode=fail_open)

### 5.8 `suites/regression_traps.yaml` (4 scenarios)
1) `dsl:(...)` denied when allow_direct_dsl=false → DirectDslNotAllowed
2) Raw execute denied without privilege
3) Macro denied verb blocked in strict mode
4) DecisionPacket tamper attempt (if applicable): invalid selection rejected

### 5.9 `suites/execution_runsheet.yaml` (4 scenarios)
1) Stage 2 entries, execute → marks only runnable as executed
2) Stage entry A, execute; stage entry B; execute → ensures A not re-run
3) Mark failed entry doesn’t block runnable execution (if supported)
4) Runnable_dsl ordering stable across runs

---

## 6) Harness assertions (what to check per step)

Implement a small assertion DSL in Rust:

- Outcome kind matches (exact or in a set)
- If `chosen_verb` expected → matches
- If `selection_source_in` expected → contains actual
- If `semreg_mode` expected → matches
- run_sheet delta, runnable delta
- trace flags expected (macro_semreg_checked, etc.)
- If strict mode: ensure no outcome returns DSL when denied (NoAllowedVerbs implies empty DSL)

Also implement “global invariants” enforced for every step:
- In strict_single_pipeline mode, no legacy endpoints invoked (in-process mode ensures this)
- If `dsl_hash` produced → run_sheet entry exists
- If `MacroExpanded` → macro_semreg_checked must be true

---

## 7) Reporting & developer ergonomics

- Print a per-suite summary:
  - passed / failed
  - top failure reasons (mismatch fields)
- On failure:
  - dump artifacts to `target/harness_failures/<scenario>/<step>.json`
    - include request, response, trace, run_sheet snapshot
- Add `--diff` option to compare current run outputs vs recorded baseline (for replay mode)

---

## 8) CI integration

- Add `cargo xtask harness run --all --mode stub` to CI (fast, deterministic)
- Add nightly job (optional):
  - `cargo xtask harness run --suite governance_strict.yaml --mode live --relaxed`
  - relaxed asserts only on invariants, not specific args

---

## 9) “Done” checklist

- [ ] xtask harness runner supports list/run/watch/record/replay/dump
- [ ] Scenario YAML schema documented + validated
- [ ] 35+ scenarios implemented across suites above
- [ ] Stub and replay modes work in CI (no external calls)
- [ ] Failures produce artifact dumps for debugging
- [ ] Harness only drives orchestrator (single pipeline), no side paths
