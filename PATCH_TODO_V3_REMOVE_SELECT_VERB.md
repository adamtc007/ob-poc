# Patch TODO v3 — Retire `/select-verb` Side Door + Make ClarifyVerb Single-Path + Enforce SemReg on Macro Expansion

**Repo:** `ob-poc`  
**Motivation:** `select-verb` is legacy and unreliable; it currently acts as an alternate DSL generation path that can bypass SemReg + PolicyGate. This patch removes that drift by routing all “verb choice” flows through the orchestrator and ensuring macro-expanded DSL is governed.

---

## 1) Remove/retire the legacy `/select-verb` endpoint (or hard-disable in strict mode)

### 1.1 Identify route + handler and lock it down
File: `rust/src/api/agent_learning_routes.rs`

- [ ] Deprecate `POST /api/session/:id/select-verb`:
  - Option A (recommended): remove route entirely, return 404
  - Option B: gate behind `PolicyGate.allow_legacy_generate` + operator role, and emit `X-OBPOC-Deprecated: true`

**Acceptance**
- There is no endpoint that can “force a verb” and generate DSL outside orchestrator.

### 1.2 Remove the alternate generator
- [ ] Delete or quarantine `generate_dsl_for_selected_verb(...)`
- [ ] Ensure no other production code calls it.

**Acceptance**
- Only orchestrator + IntentPipeline forced-verb path can generate DSL.

---

## 2) Make ClarifyVerb reply path authoritative (single pipeline)

### 2.1 Ensure the system stores offered verb candidates for a session turn
- [ ] When orchestrator returns `PipelineOutcome::ClarifyVerb`:
  - persist a `pending_choice` record in session state:
    - `choice_kind=ClarifyVerb`
    - `options=[{index, verb_fqn, label}]`
    - `original_utterance`
    - `scope_snapshot`
    - `trace_id` (optional)

Where to store:
- simplest: `SessionState` in memory if you have it
- better: DB table (if sessions persist)

**Acceptance**
- The server can validate that a later “choice reply” matches options that were actually offered.

### 2.2 Wire `decision/reply` for ClarifyVerb to call orchestrator with forced verb
File: `rust/src/api/agent_learning_routes.rs` (`handle_decision_reply`)

- [ ] On `DecisionKind::ClarifyVerb`:
  1) Load pending choice options for that session
  2) Validate selected index is in range
  3) Extract `forced_verb_fqn`
  4) Call `AgentService.handle_utterance_with_forced_verb(session_id, original_utterance, forced_verb_fqn)`
     - This must route to orchestrator and ultimately `IntentPipeline.process_with_forced_verb`
  5) Replace pending choice with final outcome, update staged DSL/run_sheet

**Acceptance**
- Selecting option “2” always generates DSL for option #2, under SemReg policy.

### 2.3 Remove any “rerun free-form ranking” on ClarifyVerb replies
- [ ] Ensure the code never calls `handle_utterance(original_utterance)` when a forced verb exists.

---

## 3) Enforce SemReg governance on MacroExpanded outcomes (close bypass)

### 3.1 Remove `MacroExpanded` from `is_early_exit` or apply SemReg check post-expansion
File: `rust/src/agent/orchestrator.rs`

- [ ] If outcome is `MacroExpanded { dsl, ... }`:
  - Parse DSL to extract verb FQNs (lightweight parse):
    - best: reuse existing DSL parser/AST
    - fallback: regex for leading `(domain.verb` tokens (only if safe)
  - Apply SemReg allowed set to every verb:
    - If strict_semreg and any verb denied → fail closed `NoAllowedVerbs`
    - If non-strict and denied verbs present → return error outcome rather than silently accepting
  - If allowed, proceed to stage/execute as normal.

**Acceptance**
- Macros cannot inject disallowed verbs.

---

## 4) Observability (make bypass impossible to miss)

### 4.1 Trace flags for “forced verb” and “macro governed”
- [ ] When forced verb selection is used:
  - set `trace.forced_verb_fqn`
  - set `trace.selection_source="user_choice"`
- [ ] When macro-expanded DSL is governed:
  - set `trace.macro_semreg_checked=true`
  - record denied list if any (redacted if needed)

---

## 5) Tests (keep this from regressing)

### 5.1 `/select-verb` no longer available
- [ ] In strict mode:
  - calling it returns 404/403

### 5.2 ClarifyVerb decision reply generates correct DSL verb
- [ ] Trigger ambiguous match → ClarifyVerb with options
- [ ] Reply selection “2”
- [ ] Assert generated DSL verb == option #2 verb_fqn
- [ ] Assert SemReg denies option #2 → strict mode fails closed

### 5.3 Macro expansion governed by SemReg
- [ ] Create macro that expands into a denied verb
- [ ] strict_semreg=true
- [ ] Assert outcome is NoAllowedVerbs and DSL not staged

---

## “Done” checklist

- [ ] `/select-verb` removed or hard-disabled; no legacy generator remains
- [ ] ClarifyVerb replies are validated against stored options and route through orchestrator forced-verb path
- [ ] MacroExpanded outcomes are SemReg-checked (no bypass)
- [ ] Trace records forced selection + macro governance
- [ ] Tests pass

