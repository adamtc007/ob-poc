# Patch TODO v2 — Close Remaining SemReg Governance Holes (Matched-path + Strict Fail-Closed)

**Repo:** `ob-poc` (post delta correctness)  
**Purpose:** Fix the last two major leaks:
1) SemReg filtering happens too late for **Matched** outcomes (DSL already generated).  
2) SemReg **deny-all** and **resolve errors** still fail-open even in strict mode.

This patch is intentionally small and surgical.

---

## 1) Make SemReg constraints binding for *all* outcomes, including Matched

### 1.1 Do not “accept pre-SemReg DSL” when SemReg is enabled
File: `rust/src/agent/orchestrator.rs`

Current behavior:
- Orchestrator calls `IntentPipeline.process_with_scope(...)` early.
- If `discovery_result.dsl` is non-empty, orchestrator returns it as-is (even if SemReg would deny/alter the top verb).

**Patch**
- [ ] Introduce a helper in orchestrator:
  - `fn is_early_exit(outcome: &IntentResult) -> bool` (scope resolution, direct DSL accepted, etc.)
- [ ] If SemReg context is available (resolve succeeded):
  - For non-early-exit outcomes, **do not** return `discovery_result.dsl` directly.
  - Instead, pick a verb after SemReg filtering and generate DSL via forced-verb call:
    - `IntentPipeline.process_with_forced_verb(req, chosen_verb_fqn)`
- [ ] Special case: if `discovery_result.intent.verb_fqn` exists and is still allowed and remains top after SemReg:
  - You MAY reuse the already-generated DSL (optional optimization), but only if you can prove it matches the same verb.
  - Otherwise regenerate to ensure consistency.

**Acceptance**
- If SemReg denies the verb that `process_with_scope()` would have matched, orchestrator generates DSL for an allowed verb instead (or fails closed).

---

## 2) Represent SemReg outcomes correctly: distinguish “unavailable” vs “deny-all”

### 2.1 Return explicit deny-all instead of collapsing to None
File: `rust/src/agent/orchestrator.rs` (`resolve_sem_reg_verbs`)

Current:
- if `allowed.is_empty()` → `(None, None)` which means “SemReg unavailable; don’t filter”.

**Patch**
- [ ] Change return type to a structured enum:
  - `enum SemRegVerbPolicy { Unavailable, AllowedSet(HashSet<String>), DenyAll }`
- [ ] Map outcomes:
  - Resolve success + allowed non-empty → `AllowedSet`
  - Resolve success + allowed empty → `DenyAll`
  - Resolve error → `Unavailable`

**Acceptance**
- Deny-all is distinguishable and can be fail-closed in strict mode.

---

## 3) Enforce strict SemReg behavior (fail-closed)

### 3.1 Deny-all must fail closed in strict mode
File: `rust/src/agent/orchestrator.rs`

- [ ] If `policy.strict_semreg == true` and SemReg policy is `DenyAll`:
  - Return outcome `NoAllowedVerbs` (or new `SemRegDeniedAll`) with trace evidence.
  - Ensure no DSL is produced.

### 3.2 Resolve error must fail closed in strict mode
- [ ] If `policy.strict_semreg == true` and SemReg policy is `Unavailable` because resolve errored:
  - Return outcome `SemRegUnavailable` (or `NoAllowedVerbs`) with error details redacted.
  - Ensure no DSL is produced.
- [ ] If strict_semreg is false:
  - Allow fail-open fallback, but set trace flags:
    - `semreg_mode="fail_open"`
    - `semreg_unavailable=true`

**Acceptance**
- In strict mode, SemReg cannot be bypassed by deny-all or outage.

---

## 4) Make post-SemReg verb selection authoritative

### 4.1 Ensure chosen verb drives DSL generation
File: `rust/src/agent/orchestrator.rs`

- [ ] After SemReg filtering/boosting:
  - Select top candidate (or user-chosen forced verb)
  - Call `process_with_forced_verb` to generate DSL for that verb
- [ ] Ensure `IntentTrace` records:
  - `chosen_verb_pre_semreg` (if any)
  - `chosen_verb_post_semreg`
  - `dsl_verb_fqn` (parsed from generated DSL if possible, or from pipeline result)
  - `semreg_policy = AllowedSet|DenyAll|Unavailable`

**Acceptance**
- Trace proves DSL verb == chosen post-SemReg verb.

---

## 5) Tests (these are “killer” regression tests)

### 5.1 Matched-path SemReg override test
- [ ] Seed verb candidates A and B where lexical ranks A first.
- [ ] Configure SemReg to deny A and allow B.
- [ ] Run orchestrator with utterance that would normally match A.
- [ ] Assert:
  - final chosen verb == B
  - generated DSL uses verb == B

### 5.2 Strict deny-all fails closed
- [ ] Configure SemReg to return deny-all for subject.
- [ ] Enable `strict_semreg=true`.
- [ ] Assert outcome is `NoAllowedVerbs` (or `SemRegDeniedAll`), and DSL is empty.

### 5.3 Strict SemReg outage fails closed
- [ ] Force SemReg resolve to error (mock store or temporarily break DB connection in test harness).
- [ ] Enable `strict_semreg=true`.
- [ ] Assert outcome is `SemRegUnavailable` (or `NoAllowedVerbs`), and DSL is empty.

### 5.4 Non-strict outage is explicit fail-open
- [ ] Same as 5.3, but `strict_semreg=false`.
- [ ] Assert DSL is produced, and trace includes `semreg_mode="fail_open"`.

---

## 6) “Done” checklist

- [ ] Orchestrator no longer returns pre-SemReg Matched DSL unless it matches the post-SemReg chosen verb
- [ ] Deny-all is not treated as “SemReg unavailable”
- [ ] strict_semreg enforces fail-closed on deny-all and on resolve errors
- [ ] Tests 5.1–5.4 pass
- [ ] Trace clearly shows SemReg policy mode and final DSL verb

