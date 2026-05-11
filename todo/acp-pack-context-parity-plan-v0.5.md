# ACP Pack Context Parity Plan — v0.5

**Owner:** Adam Cearns
**Status:** Draft for peer review
**Supersedes:** v0.4

## 1. Executive Summary

The current ACP pack projection is materially thinner than the context a repo-aware Codex/Zed session sees. A repo-aware agent can inspect pack YAML, verb schemas, FSMs, macros, workbook plans, domain metadata, service implementations, tests, migrations, and runtime assumptions. Sage operating through ACP sees only the bounded context that ACP projects, and utterance→pack→verb→valid-DSL quality degrades visibly as a result.

The fix is not to grant Sage repo access. The fix is to project a typed, bounded, hashed, **deterministically generated** operational knowledge bundle from SemOS as the single source of truth — including macros and workbook plans as first-class planning surfaces, not just verbs.

This is a measurement-driven capability upgrade with disciplined remediation along the way. The document spine is:

```text
baseline → audits → rip/remediate → enriched envelope → compare → decide whether it earns its keep
```

Five workstreams, with explicit gating between them:

1. **§3 — Pre-remediation baseline.** Measure today's Sage behaviour against repo-aware Codex/Zed before changing anything. Establishes the "earns its keep" benchmark.
2. **§4 — SemOS metadata audit.** What does SemOS contain? What metadata is registry-grade vs. code-grade vs. YAML-grade vs. absent?
3. **§5 — Code hygiene audit.** What utterance-parse, macro-resolution, and verb-dispatch paths exist in code? Where are the ghost routes and bypasses?
4. **§6 — Crate boundary audit.** What `pub` surface exists, and what crate decomposition does the new architecture need?
5. **§7 onwards — Enrichment, envelope, build pipeline, slice 1.**

The baseline (§3) and the three audits (§4, §5, §6) run in parallel. Production envelope schema work, build pipeline work, and Sage runtime wiring do not begin until baseline is captured and all three audits are signed off. Throwaway schema sketches, golden fixture design, and measurement harness baselining are explicitly permitted in parallel, subject to the §3.4 throwaway artefact discipline.

Target outcome: better pack/macro/verb hit rate, fewer invented verbs, more valid DSL drafts, structured pending questions/refusals, no prose-only failure modes, no ghost routes, a tight `pub` surface, no new mutation path, a build pipeline that can be replayed and verified, and an empirical baseline showing whether the envelope materially closed the gap to repo-aware Codex/Zed.

## 2. Problem Statement

### 2.1 Context asymmetry

ACP packs currently expose enough to identify a pack and its high-level verb surface, but not enough operational semantics for Sage to plan as well as Codex with full repo visibility.

Strong today: pack identity, pack hash/version, workspaces, allowed verbs, pack questions, section layout, risk policy, basic selected-verb diagnostics.

Weak or missing today: full verb argument contracts, lookup metadata and binding rules, return shapes, read/write effects, FSM transitions, reducer/source semantics, macro projections, workbook plan projections, workflow data grain, runtime discovery summaries, ambiguous-phrase routing policy, canonical positive/negative examples, cross-pack neighbour hints, route/explainability traces, implementation-derived invariants.

### 2.2 The planning hierarchy

Sage's planning surface has three tiers:

- **Workbook execution plan** — end-to-end arc from entity creation to final end-entity state. May span multiple DAGs.
- **Macro** — multi-entity, multi-state, multi-verb sub-arc within a workbook plan. DAG-confined. The unit at which most utterances should bind. Macros have a `kind` subtype (§7.7).
- **Verb** — single-step DSL primitive. The floor of planning, not the ceiling.

Coarsest match wins:

```text
workbook plan? -> identify which macro within the plan applies to current state
  -> macro? -> bind slots, emit draft with structural guarantees
    -> verb? -> verb-grain draft (current path)
      -> no hit -> structured pending question or refusal
```

### 2.3 Three failure modes the audits address

Three distinct failure modes shaped this plan, all observed in ob-poc to varying degrees:

**Metadata starvation.** SemOS doesn't contain enough structured metadata for the envelope to project from. Codex compensates by reading code; Sage cannot. This is what the §4 metadata audit measures.

**Ghost routes.** Legacy utterance-parse, macro-resolution, and verb-dispatch paths remain reachable in code, including via tests, examples, CLI commands, debug endpoints, fixture loaders, and comments. An LLM hallucinates a call into a deprecated path, or a fallback chain shadows the new architecture, and behaviour becomes non-deterministic. This has bitten ob-poc previously and is a known failure mode. This is what the §5 hygiene audit measures.

**`pub` leakage.** Excessive `pub` visibility turns crates into flat namespaces. The compiler can no longer verify which functions are part of a crate's contract vs. internal helpers. LLMs treat every `pub` symbol as a valid call target — the larger the `pub` surface, the larger the hallucination surface. This is what the §6 crate boundary audit measures.

All three are LLM context contamination problems with the same root cause: insufficient discipline about what is and isn't part of the system's authorised surface area.

### 2.4 SemOS gaps (to be quantified by §4 audit)

Suspected gaps:

- Verb metadata is structurally present but argument-level binding and lookup metadata is incomplete.
- Read/write effects per verb are not consistently declared at the entity grain.
- Macro identity is uneven: some of M1–M18 are likely registry-grade, others are code-grade or constellation-YAML-grade.
- Macro preconditions may be implicit in the first verb of the sequence rather than declared.
- Workbook execution plans may not exist as first-class entities at all.
- Macro/workbook composition may be implicit in code.
- The Active-vs-Draft distinction for macros may not be FSM-enforced.
- Cross-pack collision and neighbour-hint metadata does not exist.
- Cross-DAG composition rules for workbook plans that span DAGs are likely not declared.
- Diagnostic taxonomy is implicit in code rather than declared in registry.

### 2.5 Build determinism

Pack construction today is a mix of generated and hand-authored content with no enforced reproducibility. A pack that builds non-deterministically — or that can be modified out-of-band — directly makes Sage non-deterministic. This is treated as a hard constraint, not a quality goal.

## 3. Pre-Remediation Baseline

### 3.1 Why baseline first

Before any rip-first remediation begins, capture today's Sage behaviour against a fixed golden fixture set and compare it with repo-aware Codex/Zed. Without this, the "did the envelope earn its keep" question becomes unanswerable. The empirical case for the entire plan rests on showing that envelope-driven Sage materially closes the gap to repo-aware tooling.

The baseline must be captured before §5 hygiene remediation lands, because remediation may itself shift Sage's behaviour even before the envelope is in place. The baseline reflects the state of the system as it is today.

### 3.2 Baseline scope

Run a fixed golden fixture set against:

- Sage with current ACP pack context.
- Repo-aware Codex/Zed with full repo visibility.

Capture per-fixture:

- Pack hit rate.
- Macro hit rate.
- Verb hit rate.
- First-pass valid DSL draft rate.
- Invented verb count.
- Invented macro count.
- Prose-only failure count.
- Pending-question quality.
- Refusal quality.
- Route chosen / fallback used.
- Wall-clock time to first valid draft.

The fixture set is the same set used in §17 (slice 1) to measure post-envelope quality. The baseline is the "before" half of the before/after comparison.

### 3.3 Baseline deliverables

1. **Fixture set v1.** 30–50 fixtures spanning the slice 1 target packs (`onboarding-request`, `cbu-maintenance`, `product-service-taxonomy`) plus cross-pack collision cases and known ghost-route bait utterances.
2. **Baseline measurement.** Per-fixture results for both Sage and Codex/Zed.
3. **Gap analysis.** Per-metric, the difference between Sage today and Codex/Zed. This is the gap the envelope must close to earn its keep.
4. **Acceptance threshold.** The minimum gap closure that constitutes success. Set during baseline so it cannot be moved retroactively to match whatever slice 1 produces.

### 3.4 Throwaway artefact discipline

Baseline measurement, fixture design, schema sketching, and harness prep run in parallel with the audits. They do not block on audit sign-off. They are subject to the throwaway artefact discipline:

- All throwaway artefacts live in a clearly-named directory or crate (e.g., `experimental/` or `_throwaway_`) that the production build pipeline refuses to package.
- Signing infrastructure refuses to sign any throwaway artefact.
- Sage's pack loader rejects any artefact whose hash is not present in the SemOS registry; throwaway artefacts are not registered.
- Throwaway code cannot be `pub` from a production crate, cannot appear in production lints, and cannot be referenced by name in production code.

"Throwaway" is an enforced property, not a comment on a file. Anything that fails these conditions is not throwaway and is treated as production work, gated by audit sign-off.

## 4. SemOS Metadata Audit

The audit answers, with evidence, the metadata completeness questions for verb registry, macro registry, workbook plan inventory, FSM completeness, lookup surface, phrase-routing surface, and diagnostic taxonomy.

### 4.1 Audit scope

**Verb registry completeness.** For every verb: argument contract, per-argument binding rules, entity-grain read/write effects, FSM transition references, HITL and dry-run flags, diagnostic codes the verb emits. Declared in registry vs. inferred from code.

**Macro registry completeness.** Enumerate every macro in production, including M1–M18. For each: registry-grade / code-grade / YAML-grade. For registry-grade: slots, slot binding rules, preconditions, ordered steps, expected transitions, refusal conditions, dry-run plan shape, HITL gates, `kind` subtype. For each macro: current FSM state, gated by verb or by code reference.

**Workbook plan inventory.** First-class SemOS entities or only documentation/code. Macro composition declared. Cross-DAG span. Workbook-level preconditions, end-state criteria, refusal conditions.

**FSM completeness.** Per entity kind: FSM declared in registry. Static `state_definition` separable from runtime `state_instance`. Transitions verb-gated.

**Lookup surface.** First-class registered surface or scattered across verb definitions.

**Phrase-routing surface.** Registered surface for ambiguous-phrase disambiguation and cross-pack neighbour hints.

**Diagnostic taxonomy.** Registered SemOS entities with stable identifiers vs. string literals at point-of-use. Macro-grain failure classes covered distinctly from verb-grain.

### 4.2 Audit deliverables

1. **Inventory document.** Every verb, macro, workbook plan, FSM, lookup rule, and diagnostic code, tagged by registry-grade / code-grade / YAML-grade / absent.
2. **Gap matrix.** Per registry surface, count of complete / partial / missing entries, with examples.
3. **Macro tier classification.** Every macro classified, with projectability recommendation. **Slice 1 accepts whatever the audit produces** — there is no minimum registry-grade macro count gate. If the audit reveals that most macros are code-grade, slice 1 ships with a thin macro surface and the macro hit rate baseline reflects that. The macro surface grows in subsequent slices as more macros are lifted.
4. **Workbook plan model recommendation.** Whether workbook plans should be lifted to first-class entities. Tightened: first-class only where they affect routing, state interpretation, or macro selection. Pure documentation plans remain out of scope.
5. **Cross-DAG composition recommendation.** Whether and how cross-DAG workbooks should be modelled. Slice 1 is constrained to single-DAG plus explicit handoff references (§7.8); broader cross-DAG composition is out of scope.
6. **Build pipeline determinism report.** Current state of byte-equality on rebuild, hash-pinning, signing, CI enforcement. Byte-equality testing should run early in the audit because it often exposes hidden timestamps and order instability.
7. **Enrichment work plan.** Sequenced enrichment work items, with effort estimates and dependencies.

### 4.3 Audit acceptance criteria

- Inventory complete (no "unknown" entries).
- Gap matrix reproducible.
- Every production macro has a tier classification.
- Workbook plan model recommendation has defensible rationale.
- Build pipeline determinism report shows whether byte-equality holds today, with evidence.
- Enrichment work plan sequences §7 in dependency order.

## 5. Code Hygiene Audit

The hygiene audit measures executable surface — what code paths can be reached from an utterance, including paths that shouldn't be reachable.

### 5.1 Why this matters

Introducing the envelope alongside legacy utterance-parse paths creates the failure mode ob-poc has been bitten by previously: Sage parses an utterance and hits a registered macro via the envelope, but some legacy path also matches and produces a different REPL. Two paths race, one shadows the other, or Codex/Sage hallucinates a call into a deprecated path because surrounding code "looks like" it should work. Non-determinism enters at the utterance→REPL boundary.

A ghost route is a rogue pack you wrote yourself and forgot about. The §16.6 rogue pack defence addresses external attack surface; the §5 audit addresses internal attack surface.

### 5.2 Audit scope: three layers, six surfaces

The audit covers three layers:

- **Utterance-parse paths** — every code path reachable from an utterance entering the system.
- **Macro-resolution paths** — every code path that can match an utterance to a macro, including legacy macro matchers and code-grade macros.
- **Verb-dispatch paths** — every code path that can dispatch a verb call, including any that bypass macro resolution.

Across these layers, the audit explicitly enumerates ghost-route sources beyond production code:

- **Production code.** The obvious surface.
- **Tests.** Tests against legacy paths keep the paths alive in code search and LLM context. Test names that reference legacy concepts are hallucination beacons even if the test bodies have been refactored.
- **Examples and documentation samples.** Example code in `/examples`, README snippets, and inline doc comments showing legacy call sites.
- **CLI commands and debug endpoints.** Admin or debug surfaces that bypass the production utterance entry point.
- **Fixture loaders.** Test or development fixtures that exercise legacy paths.
- **Comments.** Commented-out code carrying the syntactic shape of working calls. Non-negotiable: removed entirely, not preserved as documentation.

For each path the audit classifies it as:

- **Live and authoritative** — part of the new architecture, kept.
- **Live but legacy** — currently reachable, scheduled for removal, must be ripped out before envelope work begins.
- **Dead but callable** — exists, isn't reached today, but could be reached by an LLM-generated call or a config change. Must be ripped or quarantined (§5.7).
- **Truly dead** — unreachable by any path. Removed for cleanliness.

### 5.3 Connection point analysis

The audit enumerates connection points between utterance parsing and the rest of the system: where utterances enter, what matchers run in what order, what fallback chains exist, which fallbacks lead to legacy macro paths, whether any atomic DSL matchers bypass macro resolution, whether the ACP envelope plugs in at the only utterance entry point.

Multiple utterance entry points is itself a finding. The envelope can only be authoritative if it's the single source of routing, and that requires a single entry point with no bypass.

### 5.4 Feature flag inventory

Every feature flag affecting utterance parsing, macro resolution, or verb dispatch. Recommendation is rip-first regardless of flag state — flag-off is not sufficient because an LLM can flip a flag in a config file.

### 5.5 Test coverage as a finding

Tests against legacy paths classified as refactor / delete:

- **Refactor.** Test exercises functionality that remains relevant under the new architecture. Refactored to exercise the new path. Test name, fixture names, helper names, and comments must not reference legacy code or path names — naming carries hallucination risk independent of code content.
- **Delete.** Test exercises functionality that no longer exists. Deleted with the legacy code.

### 5.6 Rip-first invariant

Rip-first is the agreed strategy. Incremental refactoring of utterance-parse paths under LLM-executed change is exactly where ghost routes survive.

The discipline:

- Decide rip-first vs. same-slice-replacement vs. quarantine (§5.7) per path.
- Rip-first happens *before* the envelope is wired in.
- Same-slice-replacement: replacement in place and legacy removed in the same change. No transitional period.
- No legacy path survives slice 1. Not even commented out. Not even behind a feature flag. Not even as a dead function.

### 5.7 Quarantine as a formal state

Some legacy paths and code-grade/YAML-grade macros carry useful intent that needs harvesting before deletion. Binary rip vs. same-slice-replace is too coarse and risks destroying useful knowledge before it is captured.

Quarantine is a formal third option:

**Definition.** Quarantined code is retained temporarily for audit/reference only, unreachable from runtime, excluded from Sage context, excluded from code search fixtures, scheduled for deletion or registry uplift.

**Strict requirements.** Quarantined code cannot:

- Be callable from any production path.
- Be gated by a feature flag (flags do not satisfy "unreachable").
- Appear in any prompt, test, example, fixture, or LLM context.
- Use names that would suggest reuse to a search or LLM context window.

**Lifecycle.** Quarantined items have an explicit retirement date and a named owner. At retirement, items are either lifted to registry-grade (§7.7) or deleted entirely. Quarantine is not a parking lot.

**Storage.** Quarantined code lives in a clearly-named location (e.g., `_quarantine_` directory or separate workspace-excluded crate) that the build pipeline excludes, the test runner excludes, and code search excludes.

The single-path invariant (§16) is enforced by ensuring quarantine satisfies "unreachable from runtime" with no exceptions.

### 5.8 Audit deliverables

1. **Path inventory.** Every utterance-parse, macro-resolution, and verb-dispatch path, classified.
2. **Ghost-route source enumeration.** Findings broken out by source: production code, tests, examples/docs, CLI/debug, fixture loaders, comments.
3. **Connection point map.** Every entry point and fallback chain.
4. **Bypass inventory.** Any path reaching verb dispatch without going through macro resolution.
5. **Feature flag inventory.** Every flag in scope.
6. **Test rip scope.** Refactor / delete classification per test, with naming guidance for refactored tests.
7. **Rip-first remediation plan.** Sequenced work items with quarantine plan for items not immediately liftable or deletable.

### 5.9 Audit acceptance criteria

- Path inventory complete.
- Ghost-route sources enumerated across all six categories.
- Connection point map identifies single authoritative entry point or flags multiple as a finding.
- Bypass inventory empty or every bypass has rip-first plan.
- Test rip scope complete with refactor/delete decisions.
- Quarantine plan defines retirement dates and owners.
- Rip-first remediation plan sequenced in dependency order.

## 6. Crate Boundary Audit

The crate boundary audit measures `pub` visibility surface and recommends crate decomposition.

### 6.1 The `pub` leakage anti-pattern

`pub` without a deliberate boundary turns a crate into a flat namespace. The compiler can no longer verify which functions are part of a crate's contract vs. internal helpers. LLMs treat every `pub` symbol as a valid call target — `pub` is hallucination fuel.

The architectural equivalent of leaving commented-out code lying around. Both are LLM context that has no business being LLM context.

### 6.2 Visibility discipline

- **`pub`** — reserved for the crate's external contract.
- **`pub(crate)`** — default for cross-module visibility within a crate.
- **`pub(super)` / `pub(in path)`** — tighter scopes where the compiler should enforce that only specific modules can call something.
- **Module privacy first.** If a function only needs to be visible within its own module, it isn't `pub` anything.

The test for whether something should be `pub`: does an external consumer of this crate need to call this? If no, the answer is `pub(crate)` or tighter.

### 6.3 Decomposition rules (primary), crate sketch (illustrative)

The core architectural rules matter more than the exact crate count:

1. **Envelope generation must not depend on execution.** Envelope contents must not vary based on runtime state.
2. **Utterance parsing must not call mutation paths directly.** It produces REPL drafts; execution dispatches them.
3. **Sage runtime is a consumer, not a dependency target.** Nothing core depends on Sage.
4. **Diagnostics depend on no one downstream.** Used everywhere, depends on nothing.
5. **The cycle test.** No cycle in the workspace dependency graph.

A defensible starting decomposition (illustrative — the audit is invited to push back if better surfaces emerge from the visibility inventory):

- `sem_os_registry` — typed registry of verbs, macros, workbooks, FSMs, lookup, phrase-routing, diagnostic taxonomy. Read-only contract. No execution. No mutation.
- `sem_os_execution` — verb dispatch, macro execution, FSM transition logic. Depends on registry.
- `sem_os_diagnostics` — diagnostic taxonomy and emission. Used by everyone, depends on nothing downstream.
- `sage_utterance` — single utterance entry point and matcher chain. Depends on registry (read) and envelope. Cannot reach into execution directly.
- `acp_context_envelope` — schema, builder, verifier, loader, signing/hash logic in one crate. Depends on registry (read).

Five crates, not seven. Generation and consumption of the envelope live together unless the audit's visibility inventory shows the crate becoming too fat to reason about, in which case the audit recommends a split.

The audit is explicitly invited to challenge this decomposition. The rules are non-negotiable; the crate sketch is a starting point.

### 6.4 Cycles to avoid (non-negotiable)

- Utterance parsing depending on execution (creates the bypass risk).
- Envelope generation depending on execution (creates non-determinism risk).
- Anything depending on Sage runtime.

### 6.5 Audit scope

For each crate currently in the workspace:

- **Visibility inventory.** Every `pub` symbol classified as: genuinely external (contract), should-be-pub-crate, should-be-pub-super, should-be-private, or unused. Total `pub` count vs. recommended count post-audit.
- **Crate decomposition recommendation.** What crates should exist, what each crate's contract is, what depends on what, where cycles are today.
- **Super-crate identification.** Any crate accumulating too much surface area, with splitting recommendation.
- **Workspace dependency graph.** Today vs. target, with diff.
- **Migration plan.** Rip-and-replace per crate.

### 6.6 Migration shape: rip-and-replace

LLM-assisted refactoring of complex Rust codebases with broad `pub` surfaces has consistently performed worse than rip-and-replace: slow, token-furnace, weak outcomes. Rip-and-replace per crate is the agreed migration shape.

- Pull one crate out at a time. Registry first (no upstream dependencies).
- Tighten `pub` surface during the rip. Everything that becomes `pub(crate)` or tighter is changed in the same operation.
- Land the crate with its tightened surface before starting the next.
- Each rip is contained, with clear before/after `pub` count and dependency graph.

Big-bang restructuring rejected. Incremental refactoring within a single super-crate rejected.

### 6.7 Ongoing enforcement

After audit and rip-and-replace land:

- **Lint rule fails CI on unexplained new `pub`.** Not "tracked, increases trigger review" — the lint **fails the build**. Adding a new `pub` requires explicit acknowledgement that it is part of the crate's external contract, in the form of a documented attribute or comment that the lint checks for.
- **`pub` count per crate tracked.** Increases require justification in the PR.
- **Code review checklist.** `pub` is treated as a load-bearing keyword, not a default.

The lint failing the build is the only enforcement that survives LLM pressure. Report-only decays in three sessions.

### 6.8 Audit deliverables

1. Visibility inventory.
2. Crate decomposition recommendation, with push-back commentary if the §6.3 sketch is wrong.
3. Super-crate findings.
4. Dependency graph diff.
5. Rip-and-replace migration plan with before/after `pub` counts.
6. Lint and CI enforcement spec, concrete enough to implement immediately after the migration.

## 7. SemOS Enrichment

The following SemOS work is in-scope and must land before the envelope schema is wired to production. Specific scope and sequencing is set by the §4 audit's enrichment work plan.

### 7.1 Verb metadata enrichment

Per §4.1.

### 7.2 Workflow plans as macro-family entities with `kind` subtype

A workflow plan is not a separate concept from macros, but it is not simply "a macro in Active state" either. Workflow plans carry durable waiting, external correlation, current progress, recovery/resume semantics, HITL gates, cross-DAG composition, and end-state criteria. These are macro-family but with stronger lifecycle semantics.

Macros have a `macro_kind` field:

```text
macro_kind:
  - atomic_sequence       # short, bounded verb sequence
  - composite_sequence    # multi-step but bounded, no durable waiting
  - workflow_plan         # durable, recoverable, may have external correlation
  - workbook_plan_step    # participates as a step in a workbook plan
```

Workflow plan macros can have additional fields not present on `atomic_sequence` or `composite_sequence` macros (durable waiting state, external correlation keys, recovery semantics).

The §11 envelope schema projects `macro_kind` so Sage can treat workflow plans differently from short verb sequences when planning.

### 7.3 Phrase-routing surface

Per §4.1.

### 7.4 Diagnostic taxonomy

Per §4.1.

### 7.5 Lookup surface

Per §4.1.

### 7.6 Static/runtime separation in state metadata

Per §4.1.

### 7.7 Macro registry hardening

Every production macro becomes a first-class SemOS entity with FSM `Draft → Active → Deprecated → Retired` and hash. State transitions are verb-gated.

A registered macro declares: identity, objective, `macro_kind` (§7.2), entity kinds touched, DAG context, slots with binding rules, preconditions (declared, not inferred), ordered steps, expected FSM transitions per step, entity-grain read/write effects, HITL gates, refusal conditions, pending-question conditions, dry-run plan shape, diagnostic codes, user-facing explanation template.

Macros classified as code-grade or YAML-grade by §4 are either lifted to registry-grade, retired, or quarantined per §5.7. None are projected.

### 7.8 Workbook execution plans as first-class entities (where operationally used)

Workbook plans become registered SemOS entities **only where they affect routing, state interpretation, or macro selection**. Pure documentation plans remain out of scope.

A registered workbook plan declares: identity, objective, initial entity creation step, end-state criteria, macro composition, workbook-level preconditions, refusal conditions, HITL gates at workbook grain, user-facing explanation template.

Cross-DAG composition is constrained: slice 1 supports single-DAG workbook plans plus **explicit cross-DAG handoff references**. The envelope can declare "this macro completes DAG A and creates handoff condition X for DAG B"; Sage cannot freely compose execution across DAGs. Generic cross-DAG planning is out of scope for slice 1.

## 8. ACP Pack Context Envelope v2

Top-level fields:

- `envelope_schema_version`
- `pack_identity`
- `objective`
- `source_refs` (including macro and workbook plan hashes)
- `build_metadata`
- `workspaces`
- `subject_kinds`
- `allowed_verbs`
- `blocked_verbs`
- `verb_contracts` (§9)
- `macro_surfaces` (§10)
- `workbook_plan_surfaces` (§11)
- `state_surfaces` (§12)
- `data_surfaces` (§13, bounded and redacted)
- `lookup_surfaces`
- `example_utterances` (§15, fallback, includes negative examples)
- `diagnostic_policy`
- `known_collision_policy` (§14)
- `pack_neighbours`
- `cross_dag_handoffs` — explicit handoff references per §7.8
- `route_trace_schema` — schema for the route/explainability trace (§17.3)
- `runtime_requirements`
- `context_budget` (§8.1, hard limits)
- `safety_policy`
- `omissions` — including code-grade and YAML-grade macros that were ripped or quarantined

Properties: deterministic, hashable, bounded, dry-run/read-only, safe to expose, explicit about omissions.

### 8.1 Hard envelope budget policy

The envelope has hard budgets, set at design time, enforced at build time:

- **Per-envelope byte limit.** A pack envelope cannot exceed this size.
- **Per-envelope token estimate limit.** Computed against a fixed tokenisation.
- **Per-section budgets.** Each top-level field has a budget. Verb contracts cannot consume the entire envelope.
- **Omission policy.** When a section exceeds its budget, what gets omitted, in what priority order, and how the omission is reported in `omissions`.
- **Summary/detail split.** Where applicable, sections have a summary form (always included) and a detail form (included up to budget).

Without hard budgets, the envelope drifts toward "repo access by serialisation". Budgets are CI-enforced.

## 9. Verb Contract Projection

Per relevant allowed verb: FQN, description, invocation phrases, required arguments, optional arguments, argument types, lookup metadata, binding names and aliases, return shape, side-effect summary, entity-grain read/write effects, FSM transition refs, HITL requirement, dry-run availability, common diagnostic codes.

Generated from the SemOS verb registry. Not hand-authored.

## 10. Macro Surface Projection

Per pack-relevant registered macro: FQN, description, `macro_kind` (§7.2), invocation phrases, slot contract, preconditions, ordered steps, expected FSM transitions, entity-grain read/write effects, HITL gates, refusal conditions, pending-question conditions, dry-run plan shape, diagnostic codes, user-facing explanation template, DAG context.

Active state for production envelopes; Draft macros may appear in development envelopes with a tag.

Generated from the SemOS macro registry. Not hand-authored. Macros are the primary language-acquisition surface.

## 11. Workbook Plan Surface Projection

Per pack-relevant registered workbook plan (where operationally used per §7.8): FQN, objective, initial entity creation step, end-state criteria, macro composition, cross-DAG handoff references where applicable, workbook-level preconditions, refusal conditions, HITL gates, user-facing explanation template.

Generated from the SemOS workbook plan registry. Not hand-authored.

## 12. State Surface Projection (Static)

Per pack-relevant FSM: entity kind, initial state, valid transitions, blocked transitions, transition verbs, expected field/state effects, reducer/source refs at entity grain.

Generated from SemOS `state_definition` registry. Static and hashable. Runtime state belongs to slice 2.

## 13. Data Surface Projection (Bounded and Redacted)

Compact entity/aggregate-grain summaries. **This is the schema-leakage danger zone** and has explicit constraints:

- Entity grain only. No table names, no column names, no SQL-shaped projections.
- Aggregate relationships, not foreign-key chains.
- Bounded by §8.1 budget.
- Redaction policy: any field that names persistence-layer schema details is redacted to its semantic role.
- The audit explicitly reviews `data_surfaces` projections for leakage before they ship.

Critical onboarding grain (illustrative, at correct entity-grain abstraction):

```text
deal onboarding request
  -> CBU + product
  -> active service-resource discoveries for that CBU
  -> one onboarding data request
  -> discoveries -> slices -> attributes
  -> dispatch / provisioning status per resource owner
```

## 14. Known Collision Policy and Neighbour Hints

Projected from the SemOS phrase-routing surface (§7.3).

`pack_neighbours` declares sibling packs Sage should consider redirecting to, with one-line trigger phrases per neighbour.

## 15. Canonical Micro-Patterns (Fallback Surface, with Negative Examples)

5–15 examples per pack as a fallback for cases where no registered macro applies.

Examples include both positive and negative shapes:

**Positive:** utterance, intended pack, intended macro, intended verb, required bindings, valid DSL skeleton, expected pending question or refusal.

**Negative:** explicit "this should not happen" cases that protect against cross-pack collisions and ghost-route reactivation.

```yaml
# Positive
- utterance: "resource dictionary for product onboarding"
  pack: onboarding-request
  macro: M7.compile-onboarding-data-request
  verb: onboarding.compile-data-request
  required_bindings: [onboarding-request-id]
  expected_status: pending_question
  pending_question: "Which onboarding request should I compile?"

# Negative — cross-pack collision
- utterance: "show me the resource dictionary"
  not_pack: onboarding-request
  expected_pack: product-service-taxonomy
  rationale: "browse/read intent, not compile/freeze intent"

# Negative — refusal required
- utterance: "dispatch ready slices"
  pack: onboarding-request
  expected_status: refusal
  rationale: "no dispatchable slices in current state"

# Negative — should not bind to macro
- utterance: "attach service to product"
  not_macro: M11.attach-product-to-CBU
  rationale: "service-to-product binding lives in product-service-taxonomy, not CBU maintenance"
```

Hand-authored, registered in SemOS as governed example fixtures with hashes. Demoted from primary status — macros are the primary language-acquisition surface.

## 16. The Single-Path Invariant

**The envelope is the only utterance→REPL routing surface in production. Any code path that could route an utterance to a REPL by any other means is a defect.**

Hard invariant. Stronger than "no prose-only failure modes". Enforced by:

- §5 hygiene audit identifying every utterance entry point and ripping out non-authoritative ones.
- §6 crate boundary audit ensuring utterance parsing cannot reach into execution directly.
- §16.6 rogue pack defence ensuring no envelope reaches Sage that wasn't authorised by SemOS.
- §16.7 continuous fuzz/property test (new in v0.5).
- Ongoing CI lint preventing reintroduction of bypass paths.

A violation of this invariant is a P1 incident.

### 16.1–16.5 Build pipeline (determinism, reproducibility, audit)

Build inputs: SemOS DSL at pinned hash, governed config artefacts at pinned hash, registered example fixtures at pinned hash, builder version with lockfile. No other inputs.

Determinism: same inputs → byte-identical output. All ordering deterministic. No timestamps, hostnames, build paths, environment values, or randomness in output. No network access during build.

Hash chain: every envelope carries its own content hash, the hash of each input artefact, and a composite input hash sufficient to verify reproducibility.

CI enforcement: every PR rebuilds affected packs and asserts byte-equality. Nightly job rebuilds all packs and reports drift. Pack publication requires signed attestation.

Pack lifecycle: envelope is a registered SemOS entity with FSM `Draft → Active → Deprecated → Retired`. Active packs are immutable.

### 16.6 Rogue pack defence

- Packs signed; Sage refuses to load unsigned packs.
- Pack hash verified at load time against SemOS registry.
- Online registry verification is **always on**, in dev and in production. Dev workflows use real registry entries in a dev SemOS instance, signed properly, loaded through the same path as production.
- Verification failures produce structured refusal.
- Out-of-band modification detected by CI nightly.
- Rogue or mismatched pack treated as P1 incident.

The cost of dev/prod behavioural drift in an LLM-driven system materially exceeds the cost of online verification in dev.

### 16.7 Continuous fuzz/property test

The single-path invariant is only as strong as the audit's coverage. If §5 misses a ghost route, the invariant is silently violated.

Continuous enforcement:

- A property test feeds randomly generated utterances (and a fixed corpus of adversarial utterances) into the production utterance entry point.
- Every REPL emission is traced back to the envelope that produced it. The trace must terminate at a registered envelope with a verified hash.
- Any REPL emission whose trace does not terminate at a verified envelope fails the property test and is treated as a single-path-invariant violation.
- The property test runs on every PR and nightly. Failures block merge.
- The adversarial corpus grows over time as new ghost-route shapes are discovered. Each P1 incident contributes its triggering utterance to the corpus.

Without this, the §5 audit is a one-time measurement and the invariant decays. With it, the invariant is continuously verified.

## 17. Context-Parity Harness

### 17.1 Projection fidelity

Compares envelope output against a deterministic oracle built from the same SemOS sources. Measures whether the envelope correctly projects what SemOS contains.

### 17.2 Sage reasoning quality

Compares envelope-only Sage results against repo-aware Codex/Zed on the §3 fixture set. Metrics: pack hit rate, macro hit rate, workbook hit rate, verb hit rate, first-pass valid draft rate, invented verb count, invented macro count, missing-binding clarity, missing-slot clarity, structured refusal rate, pending-question quality, prose-only failure rate, payload size, route latency.

The §3 baseline is the "before" measurement. Slice 1 ships when the "after" measurement closes the gap by the §3.3 acceptance threshold.

### 17.3 Route trace / explainability

Both harnesses and the production runtime support a route trace per utterance:

```text
utterance
  -> candidate pack (with confidence)
  -> candidate workbook plan (with confidence)
  -> candidate macro (with confidence)
  -> slot bindings (per slot, with source)
  -> rejected candidates (with diagnostic codes per rejection)
  -> final REPL draft OR pending question OR refusal
```

The trace is structured (not prose), schema'd, and projected as a top-level envelope field (`route_trace_schema`).

The trace serves three purposes:

- **Debugging.** Why did Sage pick X when Y was expected?
- **Quality measurement.** Hit-rate metrics decompose by where in the trace the routing succeeded or failed.
- **HITL transparency.** Reviewers can see exactly how Sage reached a draft.

## 18. Safety and Non-Goals

Non-goals: no raw repository exposure, no mutation through ACP, no second execution path, no full generic simulator, no unbounded envelope, no prose-only failure modes, no hand-authored production fields outside registered fixtures, no projection of code-grade or YAML-grade macros, no surviving legacy paths post-slice-1, no `pub` symbols outside crate contracts, no dev/prod online-verification asymmetry.

Safety requirements: read-only/dry-run unless existing runbook/HITL gates approve execution; all envelope source refs hashable and auditable; sensitive runtime values redacted; payload size capped per §8.1; envelope states what context was omitted; pack signature verified at load; build pipeline deterministic and CI-enforced; single-path invariant (§16) enforced including continuous fuzz (§16.7).

## 19. Sequencing

```text
§3 baseline measurement   ┐
§4 SemOS metadata audit   ├─→ all four sign off → unified rip-and-replace remediation
§5 Code hygiene audit     │                          (rips landed before envelope work)
§6 Crate boundary audit   ┘                                          ↓
                                                              §7 SemOS enrichment
                                                                     ↓
                                                              §8 envelope schema
                                                                     ↓
                                                              §16 build pipeline
                                                                     ↓
                                                              §20 first slice
                                                                     ↓
                                                              §21 second slice
```

Baseline and the three audits run in parallel. Throwaway schema sketches, fixture design, and harness baselining run in parallel too, subject to §3.4 throwaway discipline. Production envelope schema work, build pipeline work, and Sage runtime wiring do not begin until baseline is captured and all three audits sign off, and the unified remediation plan is agreed.

## 20. First Implementation Slice (Static Context Only)

After baseline, audits, rip-and-replace remediation, and SemOS enrichment have landed:

1. `AcpPackContextEnvelopeV2` schema with envelope versioning.
2. Hard envelope budget policy (§8.1) wired into build pipeline.
3. Deterministic build pipeline (§16) including CI byte-equality enforcement and signing.
4. Pack lifecycle as SemOS entity.
5. Verb contract projections for `onboarding-request`, `cbu-maintenance`, `product-service-taxonomy`.
6. Macro surface projections for whatever registry-grade macros the §4 audit produced for those packs (no minimum count gate).
7. Workbook plan surface projections where §7.8 ships.
8. Static state surface projections for the same packs.
9. Data surface projections at entity grain, redacted per §13.
10. Collision policy and `pack_neighbours` projection.
11. Canonical micro-patterns including negative examples (§15).
12. Cross-DAG handoff references where applicable (§7.8).
13. Route trace schema (§17.3) projected and emitted by Sage.
14. The §3 fixture set re-run against envelope-driven Sage. Acceptance: gap closure meets §3.3 threshold.
15. Projection fidelity harness (§17.1).
16. Sage reasoning harness (§17.2).
17. Continuous fuzz/property test (§16.7) running in CI.
18. `pub` lint failing CI on unexplained new `pub` (§6.7).
19. Read-only/dry-run posture preserved end-to-end.
20. Single-path invariant verified by CI.

Code-grade and YAML-grade macros identified by audit and ripped or quarantined per §5.7 are not in scope. They appear in `omissions`.

## 21. Second Implementation Slice (Runtime Context)

A separate envelope or envelope extension carries read-only live state and resource discovery summaries: existing onboarding request summary, CBU/product binding summary, active SRDEF discovery count, expected slice count, expected attribute count, owner principal coverage, L4 binding blockers, existing compiled data request status, current FSM instance state, current macro/workbook plan progress.

Slice 2 has its own peer review cycle. Cost/risk profile (redaction, freshness, snapshot consistency) materially higher than slice 1.

## 22. Open Risks

- §3 baseline reveals the gap is smaller than expected, weakening the case for the entire plan. **Mitigation:** baseline produces an honest measurement; if the gap is small, the plan is rescoped.
- Audits reveal scope larger than expected. **Mitigation:** sequence enrichment by impact; ship envelope with whatever registry-grade macros the audit produces.
- Hygiene audit reveals more ghost routes than expected. **Mitigation:** rip-first per finding; quarantine for items needing harvest; envelope work waits.
- Crate boundary audit reveals super-crates requiring substantial restructuring. **Mitigation:** rip-and-replace per crate, sequenced; envelope work waits.
- Workbook plans not first-class entities for some operationally important plans. **Mitigation:** §7.8 narrows the lifting criterion; documentation plans stay out.
- Cross-DAG composition pressure pushes scope. **Mitigation:** slice 1 supports explicit handoffs only; broader composition is post-slice-1.
- Envelope grows beyond budget. **Mitigation:** §8.1 hard budgets, CI-enforced.
- Generated contracts leak schema. **Mitigation:** §13 entity-grain, redacted, audit-reviewed.
- Macros drift from implementation. **Mitigation:** first-class SemOS entities with FSM, hash, verb-gated transitions, CI drift detection.
- Build pipeline non-determinism re-introduced. **Mitigation:** lockfile, CI byte-equality on every PR, nightly drift job, byte-equality tested early in §4 audit.
- Rogue pack inserted out-of-band. **Mitigation:** signing, online registry verification always-on (dev and prod), P1 treatment.
- `pub` discipline decays after audit. **Mitigation:** §6.7 lint **fails** the build (not "tracks").
- LLMs reintroduce ghost routes. **Mitigation:** §16.7 continuous fuzz/property test feeds adversarial corpus and verifies every REPL trace terminates at registered envelope.
- Hand-authored fixtures drift. **Mitigation:** registered as SemOS governed entities with hashes.
- Quarantine becomes a parking lot. **Mitigation:** §5.7 retirement dates and named owners; quarantined items are lifted or deleted, never indefinite.
- Throwaway artefacts leak into production. **Mitigation:** §3.4 build pipeline rejects, signing infrastructure rejects, Sage loader rejects.

## 23. Peer Review Questions

1. Is the §3 baseline scope sufficient, and is the fixture set appropriately covering known failure shapes?
2. Is the "all four (baseline + three audits) gating production work" rule the right discipline?
3. Is the §3.4 throwaway artefact discipline enforceable as stated?
4. Is the §5 hygiene audit scope (utterance-parse, macro-resolution, verb-dispatch — across production, tests, examples, CLI/debug, fixtures, comments) sufficient?
5. Is the §5.7 quarantine discipline strict enough to preserve the single-path invariant?
6. Is the §6.3 rule-first / sketch-second decomposition right? Should the crate sketch be five crates or seven?
7. Is the §7.2 `macro_kind` enum complete? Is `workbook_plan_step` a kind or a separate dimension?
8. Should workbook execution plans be lifted to first-class SemOS entities under the §7.8 narrowed criterion?
9. Is single-DAG-plus-explicit-handoff the right slice 1 boundary for cross-DAG composition?
10. Is the §8.1 hard budget policy implementable, and are the per-section budgets the right granularity?
11. Are the §15 negative example shapes (cross-pack collision, refusal-required, should-not-bind) the right four?
12. Is online registry verification always-on (dev and prod) the right call?
13. Is the §16.7 continuous fuzz/property test sufficient to keep the single-path invariant alive?
14. Does the §17.3 route trace schema give enough debugging/measurement signal?
15. Is the §6.7 "lint fails the build, not just reports" the right enforcement strength for `pub`?
16. Are the §17 metrics the right set, and is the §3.3 acceptance threshold methodology sound?
17. Is rip-and-replace the right migration shape for both §5 hygiene and §6 crate decomposition?
18. Should slice 1 ship with a minimum registry-grade macro count gate, or accept whatever the audit produces? **(v0.5 takes "accept whatever the audit produces"; reviewers may push back.)**

## 24. Recommended Decision

Proceed with §3 baseline measurement and §4, §5, §6 audits in parallel. Throwaway schema sketches, fixture design, and harness baselining may proceed in parallel under §3.4 discipline. Hold all production envelope schema work, build pipeline work, and Sage runtime wiring until baseline is captured, all three audits sign off, and the unified rip-and-replace remediation plan is agreed.

After audit sign-off, peer-review the remediation plan, §7 enrichment scope, envelope schema (§8), build pipeline determinism and signing (§16), and acceptance metrics.

The envelope cannot be sound if its source of truth is incomplete, if ghost routes can race it, if the crate boundaries leak, or if there is no baseline against which to judge whether it earned its keep. The four parallel workstreams are the only way to know whether any of these conditions hold, and the rip-and-replace remediation is the only way to ensure they don't hold by the time the envelope ships.

---

## Changes from v0.4

- §1 restructured around the measurement-driven spine: baseline → audits → rip/remediate → enriched envelope → compare → decide.
- §3 added: pre-remediation baseline measurement workstream, fourth parallel item alongside the three audits.
- §3.4 added: throwaway artefact discipline allowing parallel non-production work, with enforced "throwaway" properties (build/sign/load rejection).
- §4 (was §3): macro tier classification updated — slice 1 accepts whatever the audit produces, no minimum count gate.
- §4.2.6: byte-equality testing called out as early audit task (often exposes hidden timestamps/order instability).
- §5.2 (was §4.2): ghost-route source enumeration expanded across six explicit categories (production code, tests, examples/docs, CLI/debug, fixture loaders, comments).
- §5.7 added: quarantine as a formal third option alongside rip and same-slice-replace, with strict requirements to preserve single-path invariant.
- §6.3 (was §5.3): decomposition rules elevated above crate sketch; sketch reduced to five crates (envelope generation/consumption merged unless audit recommends split).
- §6.7 (was §5.7): `pub` enforcement strengthened — lint **fails** the build, not just tracks/reports.
- §7.2 (was §6.2): workflow plans reframed as macro-family with `macro_kind` enum subtype, not "macro plus Active".
- §7.8 (was §6.8): workbook plan first-class lifting tightened — only where they affect routing, state interpretation, or macro selection. Cross-DAG constrained to explicit handoffs in slice 1.
- §8 envelope schema additions: `cross_dag_handoffs`, `route_trace_schema`, `context_budget`.
- §8.1 added: hard envelope budget policy in main design (was peer review question in v0.4).
- §13 (was §12): data surface projection explicitly bounded and redacted, called out as schema-leakage danger zone.
- §15 (was §14): canonical micro-patterns include negative examples (cross-pack collision, refusal-required, should-not-bind).
- §16.6: online registry verification always-on (dev and prod) explicitly stated as non-negotiable; dev/prod asymmetry rejected.
- §16.7 added: continuous fuzz/property test for single-path invariant, with adversarial corpus growing from P1 incidents.
- §17.3 added: route trace / explainability schema as harness and production requirement.
- §22 risk register expanded for baseline weakness, quarantine drift, throwaway leakage.
- §23 peer review questions expanded for baseline scope, throwaway discipline, quarantine, `macro_kind`, budget policy, negative examples, online verification, fuzz coverage, route trace, lint strength, audit-floor decision.
