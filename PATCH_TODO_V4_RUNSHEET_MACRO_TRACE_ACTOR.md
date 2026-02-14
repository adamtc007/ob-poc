# Patch TODO v4 — RunSheet Correctness + Macro Governance via AST + Trace/Actor Consistency

**Repo:** `ob-poc` (post “single pipeline” + SemReg binding fixes)  
**Objective:** Close remaining high-risk holes that still cause “pipeline feels broken” behavior:
1) Prevent **re-running executed DSL** when new draft entries are staged.  
2) Make macro SemReg governance **sound** by extracting verbs via the DSL AST (no line-parsing).  
3) Ensure trace fields are **truthful** and actor identity is **consistent** across `/chat` and `/decision/reply`.

---

## 1) RunSheet execution must run *only runnable entries* (no replays)

### 1.1 Add runnable-only DSL builder
File: `rust/src/repl/run_sheet.rs` (or wherever RunSheet lives)

- [ ] Add:
  - `pub fn runnable_entries(&self) -> impl Iterator<Item=&RunEntry>`
    - Includes only statuses: `Draft`, `Ready` (define exact set)
    - Excludes `Executed`, `Skipped`, `Failed`, etc.
  - `pub fn runnable_dsl(&self) -> String`
    - Concatenate DSL for runnable entries only
    - Preserve a stable ordering (e.g., insertion order or sequence number)

### 1.2 Update execution paths to use runnable_dsl()
Files:
- `rust/src/api/agent_service.rs` (execute_runbook)
- `rust/src/api/agent_routes.rs` (`/api/session/:id/execute`)
- Anywhere else that currently uses `combined_dsl()` for execution

- [ ] Replace `combined_dsl()` with `runnable_dsl()` (or an executor that takes entries)
- [ ] After successful execution:
  - Mark only the executed runnable entries as `Executed`
  - Preserve previously executed entries unchanged

**Acceptance**
- Staging a new entry does **not** cause previously executed DSL to re-run.

### 1.3 Add a regression test (killer)
- [ ] Create RunSheet with:
  - Entry A = Executed, DSL=(a)
  - Entry B = Draft, DSL=(b)
- [ ] Execute runbook
- [ ] Assert only (b) is executed (you can assert executor received runnable_dsl)
- [ ] Assert A remains Executed and not re-executed

---

## 2) Macro SemReg governance must parse DSL via AST (no line-token parsing)

### 2.1 Implement verb extraction from DSL AST
Files:
- Prefer: existing DSL parser module (e.g., `rust/src/dsl/*` or `rust/src/repl/*`)
- Or add helper: `rust/src/agent/dsl_introspection.rs`

- [ ] Implement:
  - `pub fn extract_verb_fqns(dsl: &str) -> Result<Vec<String>, DslParseError>`
- [ ] Use the real parser to build an AST and traverse:
  - For each top-level s-expression, collect head symbol `domain.verb`
  - Handle multi-line forms, whitespace, comments, nested forms, etc.

**Acceptance**
- Verb extraction is correct regardless of formatting.

### 2.2 Apply AST-based extraction in macro governance block
File: `rust/src/agent/orchestrator.rs`

- [ ] Replace current `.lines()` extraction with `extract_verb_fqns(&expanded_dsl)`
- [ ] Enforce SemReg:
  - Strict: any denied verb → outcome `NoAllowedVerbs`, empty DSL
  - Non-strict: keep current permissive behavior if desired, but must record trace flags and denied list

### 2.3 Add a regression test (killer)
- [ ] Create a macro expansion that yields DSL with:
  - multi-line s-expression
  - leading comments/blank lines
  - two statements on one line (if supported)
- [ ] Include a denied verb
- [ ] Assert strict mode blocks it (NoAllowedVerbs)
- [ ] Assert extraction finds denied verb reliably

---

## 3) Trace fields must reflect actual behavior

### 3.1 Fix macro trace fields
File: `rust/src/agent/orchestrator.rs`

- [ ] When macro governance runs:
  - set `trace.macro_semreg_checked = true`
  - set `trace.macro_denied_verbs = <list>`
  - set `trace.selection_source = "macro"` (or similar)
- [ ] Avoid leaving `build_trace()` defaults (macro_semreg_checked=false) when macro governance is executed.
  - Either update `build_trace()` to accept optional overrides,
  - or mutate trace after calling build_trace.

**Acceptance**
- Logs prove macro governance actually executed.

### 3.2 Ensure forced-verb regeneration sets trace.forced_verb_fqn
File: `rust/src/agent/orchestrator.rs`

- [ ] When SemReg changes winner and orchestrator regenerates DSL via `process_with_forced_verb`:
  - set `trace.selection_source = "policy_gate"` (or "semreg")
  - set `trace.forced_verb_fqn = chosen_verb_fqn`

**Acceptance**
- Trace shows when SemReg forced a different verb than discovery.

---

## 4) Actor identity must be consistent across `/chat` and `/decision/reply`

### 4.1 Use header-derived ActorContext for decision replies
File: `rust/src/api/agent_learning_routes.rs`

- [ ] Change `handle_decision_reply` signature to accept request headers (or read from axum extractors)
- [ ] Use `ActorResolver::from_headers(&headers)` (same as chat path)
- [ ] Remove/avoid `from_session_id(session_id)` as the primary actor source
  - If needed, use session actor only as fallback.

**Acceptance**
- Policy/ABAC outcomes are consistent between initial ClarifyVerb and the selection reply.

### 4.2 Add test
- [ ] Create two actors:
  - Actor A triggers ClarifyVerb
  - Actor B attempts to reply/select
- [ ] Expect: reply is denied (or treated as different actor) according to policy

---

## 5) “Done” checklist

- [ ] Execution uses `runnable_dsl()` and does not replay Executed entries
- [ ] Macro SemReg governance uses AST-based verb extraction
- [ ] Trace fields (`macro_semreg_checked`, `macro_denied_verbs`, `selection_source`, `forced_verb_fqn`) are accurate
- [ ] Decision replies use the same ActorContext resolution as chat
- [ ] Killer regression tests for (1) run replay and (2) macro parsing pass

