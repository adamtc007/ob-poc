# ACP Pack Context Parity Execution Plan - v0.1

**Source:** `acp-pack-context-parity-plan-v0.5.md`  
**Status:** Approved for Gate A execution  
**Purpose:** Turn the v0.5 strategy into an executable audit and implementation plan.  
**Constraint:** No production coding starts until the baseline and all three audits are complete, signed off, and the unified remediation plan is approved.

---

## 1. Review Summary

The v0.5 plan is sound in its main sequencing: measure current behaviour first, audit the source metadata and executable routing surface, tighten crate boundaries, remediate legacy paths, enrich SemOS, then build the ACP Pack Context Envelope v2.

The strongest parts of the plan are:

- It treats context parity as an empirical before/after problem rather than an architectural preference.
- It identifies three concrete sources of LLM context contamination: metadata starvation, ghost routes, and excessive `pub` surface.
- It makes build determinism, single-path routing, pack signing, and online registry verification hard constraints.
- It explicitly prevents production envelope work from starting before the audits are complete.

The main execution risk is scope creep between audit and implementation. This execution plan therefore makes the audit outputs decision artefacts, not background research. Each audit must produce inventories, classifications, counts, and a peer-reviewable remediation plan before implementation begins.

---

## 2. Operating Rules

1. **Baseline and audits before production implementation.** Production envelope schema, build pipeline, and Sage runtime wiring do not start until the baseline plus SemOS, hygiene, and crate-boundary audits are signed off.
2. **Throwaway work is allowed only under enforced quarantine.** Fixture design, measurement harness baselining, schema sketches, and experimental probes may proceed in `_throwaway_` or equivalent locations that cannot be packaged, signed, loaded, or referenced by production code.
3. **Audit outputs must be reproducible.** Every count, inventory, and graph must identify the command, query, script, or source used to produce it.
4. **Implementation follows the audit findings.** Do not assume the five-crate sketch, macro inventory, or workbook plan model is correct until the audits confirm or revise them.
5. **No legacy route survives slice 1.** Legacy utterance parse, macro resolution, or verb dispatch paths are removed, same-slice-replaced, or formally quarantined before envelope wiring.
6. **No code-grade or YAML-grade macro is projected.** Only registry-grade, Active macros enter production envelopes.
7. **Single-path invariant is non-negotiable.** Production utterance-to-REPL routing must trace through a verified envelope or fail with structured refusal.

---

## 3. Workstream Map

The first four workstreams run in parallel:

```text
W1 baseline measurement    \
W2 SemOS metadata audit     \
W3 code hygiene audit        > Gate A: audit sign-off + unified remediation plan
W4 crate boundary audit     /

Gate A -> W5 rip/remediate -> Gate B
Gate B -> W6 SemOS enrichment -> Gate C
Gate C -> W7 envelope v2 + deterministic build -> Gate D
Gate D -> W8 slice 1 static context -> Gate E
Gate E -> W9 slice 2 runtime context planning
```

---

## 4. Gate A - Baseline and Audit Sign-Off

Gate A is passed only when all four pre-implementation workstreams are complete and peer reviewed.

### W1. Pre-Remediation Baseline

**Goal:** Capture current Sage behaviour against repo-aware Codex/Zed before remediation changes behaviour.

**Tasks:**

- Define fixture set v1 with 30-50 utterances across:
  - `onboarding-request`
  - `cbu-maintenance`
  - `product-service-taxonomy`
  - cross-pack collision cases
  - known ghost-route bait utterances
  - refusal-required cases
  - pending-question cases
- Freeze fixture inputs and expected measurement schema.
- Run fixtures against current Sage with current ACP pack context.
- Run equivalent tasks against repo-aware Codex/Zed with full repo visibility.
- Record per-fixture metrics:
  - pack hit
  - workbook hit, where applicable
  - macro hit
  - verb hit
  - first-pass valid DSL draft
  - invented verb count
  - invented macro count
  - prose-only failure
  - pending-question quality
  - refusal quality
  - route/fallback chosen
  - wall-clock time to first valid draft
- Calculate gap between current Sage and repo-aware Codex/Zed.
- Set the post-slice-1 acceptance threshold before implementation starts.

**Deliverables:**

- `baseline-fixtures-v1.md` or equivalent fixture registry
- `baseline-results-current-sage.md`
- `baseline-results-repo-aware.md`
- `baseline-gap-analysis.md`
- `slice-1-acceptance-threshold.md`

**Acceptance criteria:**

- Fixture set covers all target packs and known failure shapes.
- Results are reproducible from documented inputs.
- Acceptance threshold is explicit and cannot be changed post hoc without peer review.

### W2. SemOS Metadata Audit

**Goal:** Determine what SemOS can project today and what must be enriched before envelope generation.

**Tasks:**

- Inventory all production verbs.
- For each verb, classify completeness of:
  - argument contract
  - per-argument binding rules
  - lookup metadata
  - entity-grain read/write effects
  - FSM transition references
  - HITL flags
  - dry-run flags
  - diagnostic codes
- Inventory all production macros, including M1-M18.
- Classify each macro as:
  - registry-grade
  - code-grade
  - YAML-grade
  - absent/unknown
- For registry-grade macros, verify slots, binding rules, preconditions, ordered steps, expected transitions, refusal conditions, dry-run plan shape, HITL gates, and `macro_kind`.
- Inventory workbook execution plans and determine whether each affects routing, state interpretation, or macro selection.
- Inventory FSM definitions and confirm static `state_definition` separation from runtime `state_instance`.
- Inventory lookup surfaces, phrase-routing surfaces, cross-pack neighbour hints, and diagnostic taxonomy.
- Run byte-equality rebuild tests early to expose nondeterministic pack generation.
- Produce enrichment work plan in dependency order.

**Deliverables:**

- `semos-metadata-inventory.md`
- `semos-gap-matrix.md`
- `macro-tier-classification.md`
- `workbook-plan-model-recommendation.md`
- `cross-dag-composition-recommendation.md`
- `build-determinism-audit.md`
- `semos-enrichment-work-plan.md`

**Acceptance criteria:**

- No production verb, macro, workbook plan, FSM, lookup rule, phrase route, or diagnostic code remains `unknown`.
- Every production macro has a tier and recommendation: project, lift, retire, or quarantine.
- Build determinism report includes byte-equality evidence.
- Enrichment plan is sequenced by dependency and implementation risk.

### W3. Code Hygiene Audit

**Goal:** Identify every executable route from utterance input to REPL draft or verb dispatch, including legacy and bypass paths.

**Tasks:**

- Inventory utterance entry points.
- Inventory macro-resolution paths.
- Inventory verb-dispatch paths.
- Search across all six ghost-route sources:
  - production code
  - tests
  - examples and documentation samples
  - CLI commands and debug endpoints
  - fixture loaders
  - comments
- Classify each path as:
  - live and authoritative
  - live but legacy
  - dead but callable
  - truly dead
- Build connection point map from utterance ingress through matcher order, fallback chains, macro resolution, and verb dispatch.
- Identify all paths that dispatch verbs without macro resolution.
- Inventory feature flags affecting utterance parsing, macro resolution, or verb dispatch.
- Classify tests as refactor or delete.
- Define quarantine only where useful intent must be harvested before deletion.

**Deliverables:**

- `utterance-route-path-inventory.md`
- `ghost-route-source-enumeration.md`
- `utterance-connection-point-map.md`
- `verb-dispatch-bypass-inventory.md`
- `routing-feature-flag-inventory.md`
- `legacy-test-rip-scope.md`
- `hygiene-rip-first-remediation-plan.md`
- `quarantine-register.md`

**Acceptance criteria:**

- Single authoritative utterance entry point is identified, or multiple entry points are recorded as remediation findings.
- Every bypass has a rip-first, same-slice-replacement, or quarantine decision.
- Quarantine entries have owner, retirement date, exclusion mechanism, and final disposition.
- No comment, test name, fixture name, helper name, example, or doc sample keeps legacy route vocabulary without a remediation decision.

### W4. Crate Boundary Audit

**Goal:** Reduce hallucination and bypass risk by making crate contracts explicit and shrinking the `pub` surface.

**Tasks:**

- Build workspace dependency graph.
- Count every `pub` symbol by crate.
- Classify each `pub` as:
  - external contract
  - should be `pub(crate)`
  - should be `pub(super)` or `pub(in path)`
  - should be private
  - unused
- Identify super-crates and cyclic dependencies.
- Test the v0.5 five-crate sketch against actual code:
  - `sem_os_registry`
  - `sem_os_execution`
  - `sem_os_diagnostics`
  - `sage_utterance`
  - `acp_context_envelope`
- Recommend revised crate decomposition if the inventory disproves the sketch.
- Define lint and CI enforcement for new `pub` symbols.

**Deliverables:**

- `visibility-inventory.md`
- `workspace-dependency-graph-current.md`
- `workspace-dependency-graph-target.md`
- `crate-decomposition-recommendation.md`
- `super-crate-findings.md`
- `crate-rip-and-replace-migration-plan.md`
- `pub-lint-ci-enforcement-spec.md`

**Acceptance criteria:**

- Before/after `pub` counts exist per crate.
- Target dependency graph has no cycles and prevents utterance parsing from depending on execution.
- Lint spec is concrete enough to implement immediately after migration.
- Crate migration plan is sequenced one crate at a time, registry first unless audit findings justify otherwise.

---

## 5. Gate B - Unified Rip/Remediation Approval

Gate B is passed only after peer review approves a single remediation plan built from W2-W4 findings.

**Required decisions:**

- Which metadata gaps block slice 1 and which are deferred.
- Which macros are projected, lifted, retired, or quarantined.
- Which workbook plans become first-class SemOS entities.
- Which route paths are ripped, same-slice-replaced, or quarantined.
- Which tests are refactored or deleted.
- Which crate boundaries are changed in slice 1.
- Which `pub` symbols remain part of external crate contracts.

**Implementation order after Gate B:**

1. Rip or quarantine non-authoritative utterance routes.
2. Remove or refactor tests, fixtures, docs, examples, comments, CLI/debug routes, and feature flags that preserve ghost-route vocabulary.
3. Migrate crate boundaries one crate at a time.
4. Add `pub` lint enforcement after the relevant crate migration lands.
5. Re-run route inventory and dependency graph checks.

**Gate B exit criteria:**

- Hygiene remediation has a sequence that preserves testability.
- Quarantine register is finite and enforceable.
- Crate migration sequence is bounded and reviewable.
- Peer reviewers agree no production envelope work starts until the rip-first items blocking single-path routing are complete.

---

## 6. Gate C - SemOS Enrichment Complete

Gate C is passed when the SemOS registry can generate the slice 1 static envelope fields without hand-authored production data.

**Implementation scope:**

- Verb metadata enrichment.
- Macro registry hardening.
- `macro_kind` support:
  - `atomic_sequence`
  - `composite_sequence`
  - `workflow_plan`
  - `workbook_plan_step`
- Phrase-routing surface.
- Diagnostic taxonomy.
- Lookup surface.
- Static/runtime state separation.
- Workbook execution plans as first-class entities only where operationally used.
- Cross-DAG handoff references for slice 1; no generic cross-DAG planning.

**Gate C exit criteria:**

- All projected slice 1 fields have SemOS registry sources.
- Registry records carry stable identifiers and hashes.
- Active production macros are first-class SemOS entities.
- Code-grade and YAML-grade macros are absent from projections and listed in omissions/quarantine/deletion records.

---

## 7. Gate D - Envelope v2 and Deterministic Build

Gate D is passed when the envelope can be built, verified, budget-checked, signed, and reproduced deterministically.

**Implementation scope:**

- `AcpPackContextEnvelopeV2` schema.
- Top-level envelope fields from v0.5 section 8.
- Hard context budget policy:
  - per-envelope byte limit
  - fixed-tokenizer token estimate
  - per-section budgets
  - summary/detail split
  - deterministic omission policy
- Deterministic builder:
  - pinned SemOS DSL hash
  - pinned governed config artefact hash
  - pinned registered fixture hash
  - builder version and lockfile
  - no timestamps, hostnames, random ordering, build paths, env leakage, or network access
- Content hash chain.
- Pack signing.
- Pack lifecycle FSM: `Draft -> Active -> Deprecated -> Retired`.
- Online registry verification always on in dev and production.
- Structured refusal on verification failure.

**Gate D exit criteria:**

- Same inputs produce byte-identical envelopes.
- CI can rebuild affected packs and assert byte equality.
- Unsigned or hash-mismatched packs fail to load with structured refusal.
- Active packs are immutable.
- Envelope omissions are explicit and deterministic.

---

## 8. Gate E - Slice 1 Static Context Acceptance

Gate E is passed when envelope-driven Sage closes the baseline gap by the pre-agreed threshold.

**Slice 1 implementation scope:**

- Verb contract projections for:
  - `onboarding-request`
  - `cbu-maintenance`
  - `product-service-taxonomy`
- Macro surface projections for registry-grade macros in those packs.
- Workbook plan surface projections where the audit approved first-class workbook plans.
- Static state surface projections.
- Bounded and redacted data surfaces.
- Collision policy and `pack_neighbours`.
- Canonical micro-patterns with negative examples.
- Cross-DAG handoff references.
- Route trace schema projected and emitted by Sage.
- Projection fidelity harness.
- Sage reasoning harness.
- Continuous fuzz/property test for the single-path invariant.
- CI-enforced `pub` lint.
- Read-only/dry-run posture preserved end to end.

**Validation:**

- Re-run W1 fixture set against envelope-driven Sage.
- Compare after metrics with baseline gap.
- Verify every REPL emission traces to a registered envelope with verified hash.
- Verify no prose-only failure modes remain for covered cases.
- Verify no direct utterance-to-execution bypass remains.

**Gate E exit criteria:**

- Gap closure meets the W1 threshold.
- Projection fidelity harness passes.
- Sage reasoning harness passes or documents accepted residual gaps.
- Continuous fuzz/property test runs in CI and nightly.
- Single-path invariant is demonstrably enforced.

---

## 9. Slice 2 Runtime Context Planning

Slice 2 completed its separate peer review cycle after Gate E.

Accepted scope: session-derived runtime context only. Runtime projection is transport-neutral, deny-by-default, freshness-scoped to the request/session snapshot, and wired into the deterministic ACP session-input path.

Deferred scope: direct database-backed runtime source adapters. Those require a separate Slice 3/runtime-source-adapter plan with source inventory, authorization model, snapshot model, redaction fixtures, and review gate.

Slice 3 planning is recorded in `todo/acp-pack-context-parity-gate-a/slice-3-dag-session-adapter-plan.md`. It treats the existing Sage entity-linking service and hydrated DAG session instance as the adapter boundary; ACP must not introduce a second resolver or query raw database rows directly.

Candidate runtime fields:

- existing onboarding request summary
- CBU/product binding summary
- active SRDEF discovery count
- expected slice count
- expected attribute count
- owner principal coverage
- L4 binding blockers
- existing compiled data request status
- current FSM instance state
- current macro/workbook plan progress

Slice 2 must address redaction, freshness, snapshot consistency, and runtime budget separately.

---

## 10. Peer Review Checklist

Reviewers should approve or challenge:

- Whether the four pre-implementation workstreams are sufficient.
- Whether any production coding should be allowed before Gate A. The default answer is no.
- Whether the baseline fixture set covers enough cross-pack and ghost-route bait.
- Whether the acceptance threshold methodology prevents post hoc success criteria.
- Whether quarantine is strict enough and finite enough.
- Whether the crate decomposition should remain five crates or split envelope generation/consumption.
- Whether `workbook_plan_step` belongs in `macro_kind` or as a separate dimension.
- Whether online registry verification always-on is acceptable for dev workflows.
- Whether `pub` lint failure is too strict, too weak, or correctly calibrated.
- Whether slice 1 should accept whatever registry-grade macro count the audit finds.
- Whether the envelope budget policy needs concrete byte/token numbers before implementation starts.

---

## 11. Immediate Next Actions

1. Peer review this execution plan.
2. Name owners for W1-W4.
3. Choose the artefact directory layout for audit outputs.
4. Define the fixture schema and measurement schema.
5. Start W1-W4 in parallel.
6. Do not begin production envelope, build pipeline, or Sage runtime wiring work until Gate A is complete.
