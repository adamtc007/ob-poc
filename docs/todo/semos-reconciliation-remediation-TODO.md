# Semantic OS Reconciliation — Implementation Plan

> **Date:** 2026-03-19
> **Source findings:** `docs/todo/semos-reconciliation-findings-2026-03-19.md`
> **Scope:** Implementation plan to remediate reconciliation findings with current-repo alignment
> **Status:** APPROVED FOR IMPLEMENTATION — planning only, no code changes executed

---

## 1. Planning Principles

- This plan is keyed to the findings report, not to the earlier remediation draft where the codebase has already moved.
- Every item below either closes a finding directly or converts a partially-landed fix into a complete, testable remediation.
- If a finding is already partially addressed in code, the task is scoped to the remaining gap rather than repeating landed work.
- No architectural rewrite is proposed; this remains a wiring, validation, coverage, and observability program.

---

## 2. Findings-to-Work Traceability

| Finding | Remediation work |
|--------|------------------|
| **BH-1** Entity kind not filtering verb search | `R1` direct verb-search threading, `R5` SemOS prune visibility and canonical matching completion |
| **BH-2** Discovery bootstrap dead code | `R3` bootstrap audit and completion |
| **BH-3** 68% of verbs accept any entity kind | `R2` subject-kind coverage expansion |
| **BH-4** Trading/billing/deal/contract discovery gap | `R6` discovery family and scenario authoring |
| **BH-5** Missing canonical entity kinds | `R4` canonical kind expansion |
| **BH-6** Entity kind confidence lost between linking and SemOS | `R10` entity confidence threading |
| **BH-7** Macro post-expansion not re-validated | `R8` post-expansion validation gate |
| **BH-8** FailClosed safe-harbor not audited against `harm_class` | `R9` startup safe-harbor audit |
| **3.1 / 3.2** Entity and verb surface mismatch | `R2`, `R4` |
| **3.3** Capabilities without discovery exposure | `R6` |
| **3.4** Discoverable intents without validation | `R7` |
| **3.5** Runtime dependencies not supplied by semantic layers | `R3`, `R9`, `R11` |

---

## 3. Current Repo Status Adjustments

These adjustments are why this plan differs from the earlier TODO draft while still aligning to the findings report.

### Already partially landed

- `resolve_context()` already accepts `entity_kind` and filters `subject_kinds` in `rust/crates/sem_os_core/src/context_resolution.rs`.
- Discovery bootstrap payloads and UI types already exist in:
  - `rust/crates/ob-poc-types/src/chat.rs`
  - `rust/src/api/agent_service.rs`
  - `ob-poc-ui-react/src/types/chat.ts`
  - `ob-poc-ui-react/src/features/chat/components/ChatMessage.tsx`
- `ParkReason` and parked-entry display primitives already exist in the REPL path.

### Still missing or incomplete

- `HybridVerbSearcher.search()` does not yet take `entity_kind`.
- REPL `MatchContext` does not currently carry `entity_kind` into `VerbSearchIntentMatcher`.
- Canonical entity kinds are still sparse.
- `subject_kinds` coverage remains incomplete and heuristic derivation is still shallow.
- Entity-kind mismatch pruning is not surfaced with full fidelity in the SemOS envelope path.
- Discovery families for trading, billing, deal, and contract are still incomplete.
- Safe-harbor validation against `harm_class` is not implemented.
- Post-expansion macro validation is not enforced.

---

## 4. Work Items

## P0 — Core Utterance to Verb Routing

### R1. Thread `entity_kind` into direct verb search

**Addresses:** `BH-1`

**Objective:** Ensure direct search paths use dominant entity kind to filter or deprioritize verbs whose `subject_kinds` do not apply.

**Affected files:**
- `rust/src/mcp/verb_search.rs`
- `rust/src/mcp/intent_pipeline.rs`
- `rust/src/mcp/verb_search_intent_matcher.rs`
- `rust/src/repl/types.rs`
- `rust/src/repl/orchestrator_v2.rs`
- `rust/src/agent/orchestrator.rs`

**Implementation notes:**
- Add `entity_kind: Option<&str>` to `HybridVerbSearcher.search()`.
- Apply filtering after tier candidate retrieval and before ambiguity resolution.
- Preserve current behavior when `entity_kind` is `None`.
- Thread canonicalized entity kind from orchestrator and REPL match context.
- Treat empty `subject_kinds` as globally applicable.

**Acceptance criteria:**
- Direct search path excludes verbs with mismatched non-empty `subject_kinds`.
- No regression when `entity_kind` is absent.
- REPL and chat paths both use the same `entity_kind` search contract.

**Validation:**
- Unit tests for filtered and unfiltered search behavior.
- Existing verb search integration tests still pass.
- `cargo check`

---

### R4. Expand canonical entity kinds

**Addresses:** `BH-5`, supports `R1`, `R2`, `R5`, `R10`

**Objective:** Add missing canonical kinds so entity-kind filtering and subject-kind matching use a shared vocabulary.

**Affected files:**
- `rust/config/entity_kind_canonical.yaml`
- `rust/src/entity_kind.rs`

**Target kinds:**
- `fund`
- `document`
- `contract`
- `trading-profile`
- `deal`
- `cbu`

**Implementation notes:**
- Add aliases for each new canonical kind.
- Keep `entity_kind.rs` behavior in sync with YAML-driven vocabulary actually used by the app today.
- Add unit coverage for canonicalization and matching of the new aliases.

**Acceptance criteria:**
- New kinds canonicalize and compare correctly.
- Existing aliases continue to behave unchanged.

**Validation:**
- Entity-kind unit tests.
- `cargo check`

---

## P1 — Entity Surface Quality and SemOS Fidelity

### R2. Expand `subject_kinds` coverage across the verb surface

**Addresses:** `BH-3`, `3.1`, `3.2`

**Objective:** Raise effective `subject_kinds` coverage so entity-kind filtering meaningfully improves routing quality.

**Affected files:**
- `rust/crates/sem_os_obpoc_adapter/src/scanner.rs`
- `rust/src/mcp/noun_index.rs`
- `rust/config/verbs/*.yaml`

**Implementation notes:**
- First improve derivation heuristics in both the SemOS adapter scanner and noun-index fallback logic.
- Extend heuristics beyond current sources to include:
  - `crud.table` to entity-kind mapping
  - stronger domain-to-kind mapping
  - existing `produces.entity_type`
  - required lookup args where entity type is explicit
- After heuristic improvements, manually enrich only the remaining uncovered YAML definitions.
- Keep canonicalization consistent with `R4`.

**Acceptance criteria:**
- Coverage materially increases from the current baseline.
- Heuristic and manual fills do not produce obvious false restrictions.
- Common verbs in deal, contract, trading, document, cbu, and entity domains have non-empty `subject_kinds` where appropriate.

**Validation:**
- Before/after coverage report.
- Focused fixtures for scanner and noun-index derivation.
- `cargo check`

---

### R5. Complete SemOS `entity_kind` remediation by preserving prune visibility

**Addresses:** `BH-1` on the SemOS path

**Objective:** Keep SemOS filtering behavior, but make entity-kind mismatch visible as structured pruning rather than silent disappearance.

**Affected files:**
- `rust/crates/sem_os_core/src/context_resolution.rs`
- `rust/src/agent/sem_os_context_envelope.rs`
- `rust/src/agent/verb_surface.rs`

**Implementation notes:**
- Do not re-implement filtering already present in `context_resolution.rs`.
- Instead, preserve or reconstruct `EntityKindMismatch` as an explicit prune reason.
- Canonicalize compared kinds so aliases do not produce false mismatches.
- Ensure downstream envelope and surface formatting present the reason cleanly.

**Acceptance criteria:**
- Mismatched verbs are excluded and appear as `EntityKindMismatch` in pruned output.
- Empty `subject_kinds` still pass.
- `None` entity kind still yields current unfiltered behavior.

**Validation:**
- SemOS resolution tests for mismatch, match, and `None` cases.
- Envelope serialization test covering `entity_kind_mismatch`.
- `cargo check`

---

### R3. Complete discovery bootstrap delivery and interaction loop

**Addresses:** `BH-2`, partially `3.5`

**Objective:** Finish the remaining bootstrap UX gaps so the discovery surface is not just present in payloads, but operational for ungrounded sessions.

**Affected files:**
- `rust/src/api/agent_service.rs`
- `rust/src/agent/orchestrator.rs`
- `rust/crates/ob-poc-types/src/chat.rs`
- `ob-poc-ui-react/src/types/chat.ts`
- `ob-poc-ui-react/src/features/chat/components/ChatMessage.tsx`
- `ob-poc-ui-react/src/api/chat.ts`

**Implementation notes:**
- Treat this as an audit-and-complete task, not a greenfield feature.
- Confirm `discovery_bootstrap` is emitted only when the session is still discovery-stage and there is meaningful guidance to show.
- Upgrade entry-question rendering from passive display to actionable selection if current UX does not fully close the loop.
- Ensure selections post back through the normal chat path and update discovery context.

**Acceptance criteria:**
- Ungrounded session receives bootstrap prompts.
- User can act on bootstrap prompts from the UI.
- Grounded sessions remain unaffected.

**Validation:**
- Manual flow check from new session to answered bootstrap prompt.
- `cargo check`

---

## P2 — Discovery Coverage and Validation

### R6. Author discovery families for trading, billing, deal, and contract

**Addresses:** `BH-4`, `3.3`

**Objective:** Close the largest discovery coverage gaps called out in the findings report.

**Affected files:**
- `rust/config/sem_os_seeds/constellation_families/trading_mandate.yaml`
- `rust/config/sem_os_seeds/constellation_families/deal_lifecycle.yaml`
- `rust/config/sem_os_seeds/constellation_families/billing_operations.yaml` or equivalent new billing family file
- `rust/config/sem_os_seeds/constellation_families/contract_lifecycle.yaml` or equivalent new contract family file
- `rust/config/sem_os_seeds/universes/trading_operations.yaml`
- `rust/config/sem_os_seeds/universes/deal_execution.yaml`
- Additional universe files if billing and contract are modeled separately
- `rust/config/scenario_index.yaml`

**Implementation notes:**
- The findings explicitly include billing, so billing must be first-class in this task.
- Prefer reusing existing macro chains and verb routes instead of inventing new execution flows.
- Add trigger phrases, selection rules, and scenario entries for compound discovery.

**Acceptance criteria:**
- Trading, billing, deal, and contract domains become discoverable from utterance signals.
- Families route to existing valid verbs/macros rather than dead references.

**Validation:**
- Seed scan and scenario routing checks.
- Manual phrase probes for each domain.
- `cargo check`

---

### R7. Validate constellation map verb references against the runtime registry

**Addresses:** `3.4`

**Objective:** Ensure discoverable constellation-map actions only point to verbs that actually exist.

**Affected files:**
- `rust/src/constellation/validate.rs`
- `rust/src/constellation/map_loader.rs`
- `rust/crates/ob-poc-web/src/main.rs` or other startup path that loads constellation assets

**Implementation notes:**
- Extend existing constellation validation rather than adding a separate ad hoc checker in unrelated modules.
- Verify all `verbs:` references in constellation maps against the runtime registry.
- Warn or fail based on startup mode, but default to non-fatal diagnostics unless there is already a strict-validation convention.

**Acceptance criteria:**
- Broken constellation verb references are surfaced at startup or load time.
- Existing valid maps continue to load cleanly.

**Validation:**
- Add a test fixture with an intentionally broken verb reference.
- `cargo check`

---

## P3 — Safety, Confidence, and Execution Refinements

### R8. Re-validate macro-expanded DSL before execution planning

**Addresses:** `BH-7`

**Objective:** Catch invalid macro substitutions after expansion and before DAG assembly or execution.

**Affected files:**
- `rust/src/dsl_v2/macros/expander.rs`
- `rust/src/runbook/compiler.rs`

**Implementation notes:**
- Validate expanded statements against runtime verb contracts.
- Check required args, enum validity, and entity-kind compatibility where contract metadata supports it.
- Fail before execution planning when expansion yields an invalid verb call.

**Acceptance criteria:**
- Invalid macro substitutions are rejected after expansion.
- Valid expansions continue unchanged.

**Validation:**
- Add regression tests for invalid expanded DSL.
- `cargo check`

---

### R9. Audit FailClosed safe-harbor verbs against `harm_class`

**Addresses:** `BH-8`, partially `3.5`

**Objective:** Ensure fail-closed fallback never exposes verbs whose harm profile exceeds read-like safety.

**Affected files:**
- `rust/src/agent/verb_surface.rs`
- `rust/crates/ob-poc-web/src/main.rs`

**Implementation notes:**
- Validate the current safe-harbor set derived from fail-closed logic.
- For every included verb, inspect contract metadata or runtime metadata for `harm_class`.
- Allow only read-safe classes such as informational or read-adjacent equivalents used in this codebase.
- Warn on missing `harm_class`; error or panic on clearly unsafe values based on configured strictness.

**Acceptance criteria:**
- Safe-harbor set is startup-audited.
- Unsafe verbs cannot silently remain in the fail-closed set.

**Validation:**
- Startup validation test.
- `cargo check`

---

### R10. Thread entity confidence into SemOS

**Addresses:** `BH-6`

**Objective:** Preserve entity-linking confidence so SemOS can make better narrowing decisions when entity-kind inference is weak.

**Affected files:**
- `rust/src/agent/orchestrator.rs`
- `rust/crates/sem_os_core/src/context_resolution.rs`

**Implementation notes:**
- Add `entity_confidence: Option<f64>` to `ContextResolutionRequest`.
- Pass dominant-entity score from lookup/orchestrator into SemOS.
- Use confidence as a ranking or widening signal rather than as a brittle hard gate.

**Acceptance criteria:**
- Confidence is present end-to-end when dominant entity exists.
- Low-confidence entity matches do not over-constrain candidate verbs.

**Validation:**
- Resolution tests covering high- and low-confidence inputs.
- `cargo check`

---

### R11. Expose park reason in chat responses

**Addresses:** `3.5`

**Objective:** Surface why work is parked so users can tell whether they need to act or wait.

**Affected files:**
- `rust/crates/ob-poc-types/src/chat.rs`
- `rust/src/api/agent_service.rs`
- `rust/src/api/agent_routes.rs`
- `ob-poc-ui-react/src/types/chat.ts`
- `ob-poc-ui-react/src/features/chat/`

**Implementation notes:**
- Reuse existing runbook `ParkReason` semantics rather than inventing a duplicate model.
- Translate parked state into a chat-friendly payload.
- Distinguish human-gate, callback, resource-unavailable, and user-paused cases.

**Acceptance criteria:**
- Chat clients can render why a runbook is parked.
- The payload is consistent with existing runbook semantics.

**Validation:**
- Serialization tests.
- UI rendering check.
- `cargo check`

---

## 5. Delivery Slices

### Slice A

- `R1` Thread `entity_kind` into direct verb search
- `R4` Expand canonical entity kinds

**Reason:** Enables the core routing fix with minimal blast radius.

### Slice B

- `R2` Expand `subject_kinds` coverage
- `R5` Surface SemOS entity-kind prune reasons

**Reason:** Makes entity-kind routing effective and observable.

### Slice C

- `R3` Complete discovery bootstrap interaction loop

**Reason:** Keeps discovery UX isolated from verb-surface correctness work.

### Slice D

- `R6` Discovery families for trading, billing, deal, contract
- `R7` Constellation-map verb validation

**Reason:** Expands discoverability and hardens discovery correctness together.

### Slice E

- `R8` Macro post-expansion validation
- `R9` Safe-harbor harm audit
- `R10` Entity confidence threading
- `R11` Park reason exposure

**Reason:** Safety and refinement work can land after the main routing and discovery gaps are closed.

---

## 6. Verification Gate Per Slice

Before marking any slice complete:

- Run `cargo fmt`
- Run `cargo check`
- Run targeted tests for touched modules
- Run `cargo clippy -- -D warnings` when the slice is stable enough to absorb repo-local warnings

If a slice materially changes chat payloads or discovery behavior:

- Verify the React UI still renders the changed payloads
- Verify no older path silently drops the new fields

---

## 7. Review Checklist

- [ ] Every finding in `semos-reconciliation-findings-2026-03-19.md` has a mapped remediation task
- [ ] Already-landed code paths are treated as audit/completion work, not duplicated
- [ ] Affected file lists match the current repo structure
- [ ] Billing is explicitly included in discovery-gap remediation
- [ ] Constellation validation extends existing validators rather than adding a disconnected startup hack
- [ ] Entity-kind filtering is implemented in both direct search and SemOS paths
- [ ] Every slice ends with at least `cargo check`
- [ ] No unrelated dirty-worktree changes are overwritten during implementation

---

## 8. Final Implementation Order

1. `R1 + R4`
2. `R2 + R5`
3. `R3`
4. `R6 + R7`
5. `R8 + R9 + R10 + R11`

This order is the recommended execution path for implementation.
