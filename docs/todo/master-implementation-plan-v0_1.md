# ob-poc v0.1 Implementation — Master Plan

**Status**: Draft v1.0 — generated from v0.1 design document set
**Scope**: Full implementation of v0.1 design (Sessions 1/2/3) — unified DSL atom model, two-frontend compiler, journey-persisted runtime, 12 decision packs, 12 worked examples
**Workflow**: Adam architects and reviews; Sonnet (in Zed) implements. Tranche-level STOP gates; no per-phase STOP gates within tranches.
**Estimated wall-clock**: 25–35 of Adam's working sessions, distributed by day-job availability.

---

## How to use this plan

Each tranche has:

- **Exit criterion** — concrete, testable condition that closes the tranche.
- **STOP gate** — Adam reviews diff and behaviour at tranche close before proceeding to the next tranche.
- **Sub-phases** — work units within the tranche. Sonnet completes them sequentially without intervening STOPs. Adam may interject mid-tranche if obviously off-track, but the design intent is that Sonnet runs straight through a tranche to its exit criterion.
- **Inputs** — what Sonnet needs in context to start the tranche.
- **Outputs** — what the tranche produces.
- **Risk notes** — where Sonnet is most likely to need correction or where surprises are likely.

The plan does not estimate per-phase time. Time is dominated by Adam's review at the STOP gate, which is the only meaningful coordination point.

The phrase **"Do not commit"** appears at the end of every tranche. Adam reviews the diff before any git commit.

---

## Reference documents

All tranches assume these are loaded as context:

- `unified-dsl-design-v0_1-session1-regen.md` — language layer (atom model, verb catalogues)
- `unified-dsl-design-v0_1-session2.md` — compiler architecture, runtime design
- `unified-dsl-design-v0_1-session3.md` — regression strategy, decision packs (§7, §9 ex 1–11, §10–12, App A–D)
- `unified-dsl-design-v0_1-session3-patch.md` — patched §8, §9 Ex 12, App E (decision packs in Session 1 syntax)

Cross-references in this plan use the form **S1 §3.3.2**, **S2 §6.2**, **S3 §7.1**, **S3-patch §8.3 Pack 4**, etc.

---

## Tranche 0 — Document consolidation

**Goal**: Seal the v0.1 design document set as a single internally consistent reference. Produce derived artifacts (rendered Word, README index).

**Exit criterion**:
- Session 3 patched in place — patched §8, §9 Example 12, and Appendix E spliced into Session 3; rest unchanged.
- README index at the top of the v0.1 set, describing reading order and listing the 17 architectural commitments with cross-references.
- Optional: Word/PDF rendering of the full set via the docx skill (only if BNY peer review is imminent).
- Tag `design/v0.1` applied in git.

**Sub-phases**:

**0.1 Splice the Session 3 patch**

Open the existing Session 3 markdown. Remove sections §8 (entire), §9 Example 12, and Appendix E. Paste in the corresponding sections from the patch file. Verify section numbering is unbroken. Re-link table of contents.

**0.2 README index**

Create `unified-dsl-design-v0_1-README.md` at the top of the document set. Contents:

- Reading order: Session 1 (language) → Session 2 (implementation) → Session 3 (validation + catalogue).
- One-paragraph summary of each session.
- Table listing the 17 architectural commitments with source document + section reference.
- Index of `[GAP: ...]` markers for v0.2 backlog reference.
- Index of where each of the 12 decision packs is defined (S3-patch §8.3 + Appendix E).
- Index of where each of the 12 worked examples is defined (S3 §9).

**0.3 Word rendering (optional, gated on need)**

If BNY peer review imminent, run docx skill against each session document. Concatenate into single Word document with table of contents. If not imminent, defer.

**0.4 Internal review pass**

Adam reads end-to-end with fresh eyes. Flag any v0.1 gaps not already marked. Add `[GAP: ...]` annotations inline where new gaps surface. These feed v0.2 backlog.

**0.5 Git tagging**

Commit all consolidated documents. Tag `design/v0.1`.

**Inputs**: All four v0.1 documents (Session 1 regen, Session 2, Session 3, Session 3 patch).

**Outputs**: Consolidated v0.1 design document set; README index; optional Word rendering.

**Risk notes**: Mechanical work. Only risk is splice errors — verify section numbering after splice.

**STOP gate — Tranche 0 review**:
Adam confirms the v0.1 document set is sealed and consistent. Tag `design/v0.1` applied. Proceed to Tranche 1.

**Do not commit until Adam reviews the splice diff.**

---

## Tranche 1 — Pre-refactor SemOS regression baseline

**Goal**: Build the regression infrastructure that proves Tranche 3 (SemOS reshape) preserves behaviour. Without this baseline, the reshape is unverifiable.

**Exit criterion**:
- All test types from S3 §7.1 implemented and green against the current ob-poc codebase.
- CI integration: regression suite runs on every push.
- Any pre-existing bugs surfaced by Phase 1E (effect declaration verification) logged in `issues/` with classification.

**Sub-phases**:

**1.1 Test infrastructure setup**

Add `insta` crate dependency to relevant test crates. Create `tests/snapshots/` directory hierarchy mirroring the test-target structure. Configure CI to fail on snapshot diff unless explicitly accepted via `cargo insta accept`.

**1.2 AST golden shapes (Phase 1A from S3 §7.1)**

Select 50 representative verb-call utterances spanning the major domain categories (CBU, KYC, deal, IM, screening, plus utility verbs). For each, write a snapshot test that parses the source through the current parser and asserts the serialised AST against a golden snapshot. Run, accept initial snapshots, commit.

**1.3 DagGraph golden shapes (Phase 1B)**

Select 20 representative runbooks spanning major workflow categories. For each, write a snapshot test that processes the runbook through the current assembly pipeline and asserts the DagGraph structure (nodes, edges, dependency relationships) against a golden snapshot.

**1.4 @-slot binding assertions (Phase 1C)**

For each `@`-slot variant in the current SemOS verb registry, write a unit test that constructs an invocation context and asserts the `@`-slot resolves to the expected value. Cover `@cbu`, `@entity`, `@workspace`, `@deal`, and any other context-injection mechanisms currently in use.

**1.5 ExecutionPlan golden shapes (Phase 1D)**

Select 10 representative multi-step plans (typically utterances that resolve to >3 verb invocations). For each, snapshot the lowered ExecutionPlan structure.

**1.6 Effect declaration verification (Phase 1E)**

For each verb in the current registry (~1,098 total), generate a unit test that compares the verb's declared `:effects` against its handler's actual database operations. This is mechanically generated — Sonnet writes a test-generator that introspects the handler code and produces effect assertions.

Surfaced mismatches are bugs in the current codebase. Categorise them:

- **Type A**: handler does more than declared (extra writes/reads). Fix by extending declarations or scoping handler.
- **Type B**: handler does less than declared (declared writes never happen). Fix by trimming declarations.
- **Type C**: type mismatch (declared `reads X` but handler `writes X` or vice versa). Likely real bugs — fix the handler.

Do not attempt to fix all of these in this tranche. Log them with classification, fix only blocking issues (Type C with safety impact).

**1.7 Dependency injection ordering tests (Phase 1F)**

Property tests asserting that for any plan, no step executes before its declared inputs are available. Use `proptest` or similar.

**1.8 CI integration**

Add `regression-baseline` job to CI. Job runs the full Tranche 1 test suite on every push. Failure blocks merge.

**1.9 Pre-Tranche-3 readiness assertion**

Document that the regression baseline is in place. Capture in `docs/v0_1-implementation-status.md`. Note any Type C effect bugs that remain unresolved — these are pre-existing issues, not caused by the upcoming refactor.

**Inputs**: Current ob-poc codebase. S3 §7.1 (regression strategy specification).

**Outputs**:
- ~100 snapshot tests across AST, DagGraph, ExecutionPlan categories.
- Per-verb effect verification tests for ~1,098 verbs.
- CI integration.
- Bug log for Type A/B/C effect declarations.

**Risk notes**:
- Existing test coverage may be thinner than S3 §7.1 estimated. If snapshot generation reveals the current pipeline is non-deterministic in some areas (e.g., HashMap iteration order surfacing in serialised output), those need to be fixed before snapshots are stable. Use `BTreeMap` everywhere relevant in snapshot serialisation paths.
- Type C bugs from 1.6 may be more numerous than expected. Resist the urge to fix all of them in this tranche; the goal is the regression baseline, not a code-quality pass.

**STOP gate — Tranche 1 review**:
Adam confirms regression baseline is green, CI integrated, bug log captured. Proceed to Tranche 2.

**Do not commit until Adam reviews the diff.**

---

## Tranche 2 — Atom model and parser foundation

**Goal**: Build the shared language infrastructure. S-expression parser producing typed AST. No SemOS or bpmn-lite specifics yet — pure language layer.

**Exit criterion**:
- All worked example DSL fragments from S1 §3.11 (Examples 1–6) parse to typed AST without panics.
- Template substitution forms (`,name`, `,@name`) and insertion markers (`$pre-node`, `$post-node`) parse correctly.
- Diagnostic surface produces source-attributed error messages.
- Crate scaffolding established for the multi-frontend compiler.

**Sub-phases**:

**2.1 Workspace scaffolding**

Create the multi-crate workspace under `dsl/` (or wherever fits ob-poc's existing layout). Crates:

- `dsl-core` — shared types, atom kind taxonomy enum.
- `dsl-parser` — lexer, S-expression parser, atom dispatch.
- `dsl-ast` — typed AST nodes, atom-bag container.
- `dsl-diagnostics` — error types, source attribution, pretty-printing.
- `dsl-semos-frontend` — empty placeholder; populated in Tranche 3.
- `dsl-bpmn-frontend` — empty placeholder; populated in Tranche 4.
- `dsl-resolution` — empty placeholder; populated in Tranche 5.
- `dsl-lowering` — empty placeholder; populated in Tranches 3 and 4.

Each crate has skeleton `lib.rs` and `Cargo.toml` with inter-crate dependencies declared per S2 §5.1.

**2.2 Lexer**

In `dsl-parser`, implement the S-expression lexer. Tokens: open-paren, close-paren, open-bracket, close-bracket, open-brace, close-brace, symbol, keyword (colon-prefixed), string-literal, number-literal, comma (template-subst), comma-at (template-splice), dollar-prefix (insertion-marker), comment-to-eol.

EBNF reference: S1 §3.1.

**2.3 Untyped parse tree**

In `dsl-parser`, produce an untyped parse tree from the token stream. Atoms are `RawAtom { kind: Symbol, name: Option<Symbol>, body: Vec<RawValue> }`. RawValue is a sum type covering atoms, lists, maps, literals, name-refs, template-subst forms, and insertion markers.

**2.4 Atom kind taxonomy**

In `dsl-core`, define the full taxonomy of atom kinds from S1 Appendix B. Two main enums:

```rust
pub enum StructuralKind {
    Verb, Invoke, Node, Gateway, Flow, BoundaryAttachment, ParallelJoin,
    Entity, Relationship, Predicate, Decision, DataType,
    MessageDefinition, TimerDefinition, ErrorDefinition,
    GraphPack, UtteranceBinding, ConstellationRoot, WorkspaceConstraint,
    DecisionPack,
}

pub enum DeclarativeKind {
    Provenance, GovernanceStatus, ReviewAnnotation, JurisdictionTag,
}
```

Plus a classifier function mapping atom-kind-symbol → (StructuralKind | DeclarativeKind | UnknownDeclarative | UnknownStructural).

Per S1 §3.2: unknown declarative kinds are warnings; unknown structural kinds are errors.

**2.5 Typed AST**

In `dsl-ast`, define typed AST node structures for each atom kind. Use trait-bound generic for the per-kind slot-bag, or per-kind structs with `#[derive(Deserialize)]`-shaped slot extraction.

The atom-bag `AtomBag { atoms: Vec<TypedAtom>, by_name: HashMap<Symbol, AtomIndex> }`. Forward-reference-safe — names index into the bag; resolution happens later passes.

**2.6 Atom dispatch**

In `dsl-parser`, implement the dispatch from `RawAtom` → `TypedAtom` for each structural and declarative kind. Slot extraction enforces required slots; missing required slots become diagnostics.

**2.7 Template substitution form parsing**

The forms `,name` and `,@name` parse to a `TemplateSubstNode { kind: Scalar | Splice, name: Symbol }` AST node. Scope enforcement ("only valid inside `(decision-pack :template ...)`") is **not** in the parser — the parser accepts them anywhere; the assembly pass enforces scope. The parser's job is to produce well-typed forms.

Insertion markers `$pre-node`, `$post-node`: parse as `InsertionMarkerNode { name: Symbol }`. Similarly accepted anywhere by the parser; resolution is author-time (Sage), not compiler-time.

**2.8 Diagnostic types**

In `dsl-diagnostics`, define the diagnostic enum hierarchy. Each variant carries source location (file, line, column, span). Pretty-print uses the `codespan-reporting` crate or similar for nice terminal output.

Diagnostic severity: Error, Warning, Note.

**2.9 Parse-only smoke tests**

Write a test suite that parses each of the S1 §3.11 worked examples and asserts the resulting AtomBag has expected structure (atom counts per kind, name-refs present, declarative atoms classified correctly).

**Inputs**: S1 §3.1 (EBNF), §3.2 (dichotomy), §3.3 (atom kinds), §3.5 (reference model), Appendix B (atom kind reference). S2 §5.1 (crate decomposition), §5.2 (parser), §5.3 (AST representation).

**Outputs**:
- `dsl-core`, `dsl-parser`, `dsl-ast`, `dsl-diagnostics` crates functional.
- Parser handles every atom kind in the v0.1 taxonomy.
- Smoke tests green against S1 worked examples.

**Risk notes**:
- The dispatch from RawAtom to TypedAtom is mechanical but bulky (24 atom kinds). Sonnet should batch this work — produce dispatch + types for ~5 kinds at a time, run tests, move on. If Sonnet tries to do all 24 in one pass, the diff is unreviewable; tell it to batch.
- Template-subst scope is a frequent confusion point. Reinforce in the prompt that the parser is uniform — scope enforcement is Tranche 5 (resolution).

**STOP gate — Tranche 2 review**:
Adam runs the smoke test suite. All S1 worked examples parse cleanly. Diagnostic output is human-readable. Proceed to Tranche 3.

**Do not commit until Adam reviews the diff.**

---

## Tranche 3 — SemOS frontend (reshape)

**Goal**: Reshape the existing SemOS verb model against the unified atom model. Preserve behaviour: regression baseline from Tranche 1 must remain green throughout.

**Exit criterion**:
- All ~1,098 SemOS verbs declared as `(verb ...)` atoms per S1 §3.3.1.
- SemOS assembly pass produces identical DagGraph shapes to the current pipeline (snapshot tests from Tranche 1 pass).
- SemOS lowering produces ExecutionPlan compatible with the current runtime (which is unchanged in this tranche).
- All 149 REPL V2 tests pass. 353-utterance hit rate maintained or improved. 44 scenario suite paths unchanged.

**Sub-phases**:

**3.1 SemOS atom parsing**

In `dsl-parser` (already from Tranche 2), confirm parsing of SemOS-specific atom kinds: `verb`, `invoke`, `graph-pack`, `utterance-binding`, `constellation-root`, `workspace-constraint`. Add any missing dispatch from Tranche 2 if surfaced.

**3.2 Verb reshape — mechanical batches**

Reshape the ~1,098 verbs from `VerbConfig` (current) to `(verb ...)` atom form (new) per S1 §4.1.

Process in batches of 50–100 verbs by domain category:

- Batch 1: `cbu.*` verbs
- Batch 2: `entity.*` verbs
- Batch 3: `kyc.*` verbs
- Batch 4: `deal.*` verbs
- Batch 5: `im.*` verbs
- Batch 6: `screening.*` verbs
- Batch 7: `workflow.*` verbs
- ... continue per domain

For each batch:
- Sonnet identifies the pattern category per S1 §4.1 (Pattern A: simple CRUD; Pattern B: composite; Pattern C: governance-tagged; Pattern D: edge cases).
- Sonnet generates the `(verb ...)` atom source file per S1 §4.1's worked examples.
- Sonnet creates the `Cargo` or `serde` integration so the new atom-source file replaces the corresponding `VerbConfig` entries.
- The compatibility shim continues to expose the same verb registry interface to existing code.

Adam spot-checks 5–10% of each batch.

**3.3 Pattern D verb redesign**

For the ~5% of verbs that don't fit Patterns A/B/C (per S1 §4.1's identified blockers), redesign individually. Each such verb gets a small design note in `docs/verb-redesigns/<verb-name>.md` documenting the reshape decision.

**3.4 SemOS assembly pass**

In `dsl-semos-frontend`, implement the utterance expansion to dependency DAG. Algorithm: walk utterance-binding atoms, expand to invoke atoms, build dependency edges from effect declarations and explicit `:after` references.

This pass produces the same DagGraph shape as the current pipeline. Tranche 1's snapshot tests (1.3) are the regression check.

**3.5 SemOS lowering**

In `dsl-lowering/src/semos.rs`, produce the ExecutionPlan format the current runtime consumes. No runtime changes in this tranche — only the path from source to ExecutionPlan changes.

Per S2 §5.6.1.

**3.6 Integration and regression validation**

Wire the new pipeline as the default path. Run the full Tranche 1 regression suite. All snapshots green; all REPL V2 tests pass; utterance hit rate maintained.

If any regression — pause, diagnose, fix. Do not proceed to Tranche 4 with a regression unresolved.

**3.7 Old VerbConfig retirement**

Once regression is fully green, remove the old `VerbConfig` struct and its associated code paths. The reshape is permanent.

**Inputs**: S1 §3.3 (atom kinds, especially §3.3.1 verb), §3.6 (verb signature surface), §3.7 (effect model), §4.1 (SemOS reshape). S2 §5.4.1 (SemOS assembly pass), §5.6 (lowering). Tranche 1 regression baseline.

**Outputs**:
- ~1,098 verbs reshaped to `(verb ...)` atoms across ~20 source files.
- SemOS assembly pass produces identical DagGraphs.
- Old `VerbConfig` removed.
- Per-verb redesign notes for Pattern D cases.

**Risk notes**:
- **Highest-risk tranche of the entire plan.** If the regression baseline from Tranche 1 is incomplete or has gaps, behavioural drift in the reshape will not be caught. The exit criterion is "regression green" — do not relax it.
- Pattern D verbs (the ~5% that don't fit standard patterns) are time sinks. Adam should look at the Pattern D list early; if any of them are truly hard, consider deferring them to v0.2 rather than blocking the tranche.
- The `:effects` reshape interacts with Tranche 1's effect verification tests. If Tranche 1 surfaced Type C effect bugs that were not fixed, those will likely surface again here. Decide upfront whether to fix Type C bugs as part of the reshape or defer.

**STOP gate — Tranche 3 review**:
Adam runs the full regression suite. All green. Old `VerbConfig` removed. Pattern D redesign notes reviewed. Proceed to Tranche 4.

**Do not commit until Adam reviews the diff.** The Tranche 3 diff is large (~1,098 verbs); review by category may need to be batched across multiple sessions.

---

## Tranche 4 — bpmn-lite frontend (compile-only)

**Goal**: Build the bpmn-lite compiler frontend. Greenfield — no compatibility surface. By end of tranche, all 12 worked examples (S3 §9) compile to railway graphs.

**Exit criterion**:
- bpmn-lite atom parsing (node, gateway, flow, boundary-attachment, parallel-join, message/timer/error-definition) works.
- Railway assembly produces typed process graphs per S2 §5.4.2.
- All 12 worked examples (S3 §9 Examples 1–11 + S3-patch §9 Example 12) compile cleanly.
- Validation errors (S2 §5.4.2 structural rules) are caught with source attribution.
- Worked Example 6 (undeclared write conflict) produces detect-and-fail diagnostic at compile time per S2 §5.4.2.

**Sub-phases**:

**4.1 bpmn-lite atom parsing**

Already from Tranche 2 — atom kinds are in the taxonomy. Confirm parsing works against all bpmn-lite atoms in S3 §9 worked examples.

**4.2 Railway assembly algorithm**

In `dsl-bpmn-frontend`, implement the assembly:

- Index node atoms by name.
- Walk flow atoms, resolving source and target by name. Build edge list.
- Validate structural rules from S2 §5.4.2:
  - Every flow has source and target that resolve.
  - Every node reachable from a start event.
  - Every path terminates in an end event or marked terminal.
  - Gateway fan-in/fan-out rules per gateway kind.
  - Boundary event attachment rules.
  - Cycle rules (cycles only through loop markers or repeated subprocess).
  - Duplicate name detection.

Produce typed `RailwayGraph { nodes, edges, boundary_attachments, parallel_joins }`.

**4.3 Inclusive gateway expected-set computation**

Per S2 §6.7: inclusive gateways have dynamic fan-out. At assembly time, the static expected-set for each inclusive join is the set of branches downstream of the fork. Compute and store on the join atom.

**4.4 Parallel-join merge protocol validation**

Per S2 §6.8: parallel-join `:merge` clauses are validated at compile time:

- Every operator is one of the known operators (max, min, union, concat, sum, latest, earliest, custom).
- Every location referenced is a valid data location.
- Custom operators reference a known verb.
- Detect-and-fail: if multiple branches write the same location and the join does not declare a merge, the assembly pass emits `UndeclaredMergeConflict` warning at compile time. This is a warning (not error) because the runtime may still tolerate it if no actual conflict occurs.

Worked Example 6 exercises this — should produce the warning.

**4.5 bpmn-lite lowering**

In `dsl-lowering/src/bpmn.rs`, produce the `JourneySpec` format the runtime will consume in Tranche 6. Per S2 §5.6.2.

Note: no runtime exists yet. Lowering produces a serialised JourneySpec that the runtime will consume. Output to JSON for now; can switch to a more compact format later.

**4.6 Worked example compilation tests**

For each of the 12 worked examples (S3 §9), write a compilation test:

- Parse the example's DSL.
- Assemble to RailwayGraph.
- Validate structural correctness.
- Lower to JourneySpec.
- Assert JourneySpec matches an expected snapshot.

Run all 12 examples. Adam reviews any failures.

**Inputs**: S1 §3.3 (atom kinds, bpmn-lite-specific), §4.2 (bpmn-lite verb catalogue). S2 §5.4.2 (bpmn-lite assembly pass), §5.6 (lowering), §6.7 (multi-token semantics), §6.8 (merge protocol). S3 §9 worked examples 1–11. S3-patch §9 Example 12.

**Outputs**:
- `dsl-bpmn-frontend` populated.
- `dsl-lowering/src/bpmn.rs` produces JourneySpec.
- 12 worked examples compile cleanly.
- Snapshot tests for each example.

**Risk notes**:
- Inclusive gateway expected-set computation has subtle edge cases. If a downstream branch contains a sub-gateway that re-joins, the expected-set computation must handle nesting correctly. Test against Example 4 thoroughly.
- Cycle rules need careful interpretation. S1 §5.4.2 says cycles allowed only through loop markers or repeated subprocess. Worked Example 11 has a loop-marked sign-off — verify cycle detection passes.

**STOP gate — Tranche 4 review**:
Adam runs the 12-example compilation suite. All compile cleanly. Diagnostic surface produces useful errors for malformed DSL. Proceed to Tranche 5.

**Do not commit until Adam reviews the diff.**

---

## Tranche 5 — Resolution pass and decision-pack support

**Goal**: Complete the compiler pipeline with the unifying resolution pass. Add decision-pack atom support. Integrate with REPL.

**Exit criterion**:
- Resolution pass binds `@`-slots, resolves verb signatures, resolves decision/entity/atom refs.
- Decision-pack atoms parse and validate per S1 §3.3.2.
- Provenance atoms validate against pack registry; unknown packs are warnings.
- REPL endpoints `validate()`, `compile()`, `deploy()` return structured responses per S2 §5.8.
- All 12 worked examples compile end-to-end through the full pipeline (parse → assemble → resolve → lower).
- Pack-authored worked example (S3-patch §9 Example 12) validates cleanly with provenance.

**Sub-phases**:

**5.1 Shared resolution pass**

In `dsl-resolution`, implement the resolution pass per S2 §5.5:

- Verb signature lookup against registry (the registry from Tranche 3).
- `@`-slot binding from authoring context. Assembly-resolved slots (`@node`, `@decision`, `@subprocess`) get bound here. Runtime-resolved slots (`@process`, `@token`, `@parent`) marked `RuntimeBound`.
- Type compatibility checks against verb signatures.
- Cross-artifact reference resolution (qualified names `pack-name/atom-name`).
- Diagnostic emission for unresolved refs, missing slots, type mismatches.

**5.2 Decision-pack atom parsing**

The atom kind is already in the taxonomy. Confirm parsing of S1 §3.3.2 worked example and S3-patch §8.3 packs 1–12.

**5.3 Decision-pack validation**

In `dsl-resolution`, validate `(decision-pack ...)` atoms per S1 §3.3.2:

- Parameter declarations are well-formed.
- Template body contains valid atoms.
- Template substitution forms (`,name`, `,@name`) reference declared parameters.
- Splice forms (`,@name`) reference list-typed parameters.
- Scalar forms (`,name`) used in single-value positions.
- Templates contain insertion markers (`$pre-node`, `$post-node`) where the surrounding process attachment is expected.
- Index validated packs into the pack registry.

**5.4 Pack registry**

In `dsl-core`, implement the pack registry interface:

```rust
pub struct PackRegistry {
    packs: HashMap<(PackName, Version), DecisionPack>,
}

impl PackRegistry {
    pub fn register(&mut self, pack: DecisionPack) -> Result<(), PackRegistryError>;
    pub fn lookup(&self, name: &PackName, version: &Version) -> Option<&DecisionPack>;
    pub fn lookup_latest(&self, name: &PackName) -> Option<&DecisionPack>;
    pub fn list_active(&self) -> Vec<&DecisionPack>;
}
```

The registry is populated at compile time from `(decision-pack ...)` atoms in loaded source. Persistence is Tranche 6's problem (Postgres-backed pack table).

**5.5 Provenance validation**

In `dsl-resolution`, validate `(provenance ...)` atoms per S1 §3.10:

- `:covers` refs resolve to structural atoms in the same source.
- `:source-id` and `:version` reference a known pack in the registry. Unknown pack → `UnknownPackReference` warning.
- Pack version FSM state (from `(governance-status ...)` declarative atom):
  - `active` → OK.
  - `deprecated` → `DeprecatedPackVersion` warning.
  - `retired` → `RetiredPackVersion` error.

**5.6 REPL integration**

In the REPL crate, wire up:

- `validate(source) → ValidateResponse { graph, diagnostics, provenance_summary }`
- `compile(source) → CompileResponse { journey_spec | execution_plan, diagnostics, provenance_summary }`
- `deploy(name, source) → DeployResponse { workflow_id, name, version, bytecode_hash }`

Response schemas per S2 §5.8.

**5.7 End-to-end pipeline integration**

Run all 12 worked examples (S3 §9 + S3-patch §9 Example 12) through the full pipeline: parse → assemble → resolve → lower. Snapshot the full ValidateResponse for each.

Verify pack-authored Example 12 produces:

- Zero errors, zero warnings.
- `graph.nodes` contains the expected gateway.
- `provenance_summary.instantiations[0].pack_id = "conjunctive-gate"`.

**Inputs**: S1 §3.3.2 (decision-pack atom), §3.5 (reference model, @-slots), §3.10 (provenance). S2 §5.5 (resolution pass), §5.8 (REPL contract). S3-patch §8.1, §8.2, §9 Example 12.

**Outputs**:
- `dsl-resolution` populated.
- Pack registry interface.
- REPL endpoints functional.
- All 12 examples validate end-to-end.

**Risk notes**:
- The `@`-slot binding has subtle scoping rules. Verbs invoked inside subprocesses see `@subprocess` but not the outer process's `@process` unless explicitly imported. Test against Example 7 (subprocess invocation) thoroughly.
- Pack registry persistence is deferred to Tranche 6. In Tranche 5, the registry is in-memory only, populated from source files at REPL session start.

**STOP gate — Tranche 5 review**:
Adam runs the 12-example validate suite. All examples produce expected ValidateResponse. Pack-authored example validates with provenance. Proceed to Tranche 6.

**Do not commit until Adam reviews the diff.**

---

## Tranche 6 — Runtime persistence schema and event loop

**Goal**: Build the journey-persisted runtime. By end of tranche, single-token bpmn-lite instances run end-to-end through the harness with persisted state.

**Exit criterion**:
- Postgres schema migrations applied per S2 §6.2.
- Event loop processes events, hydrates instances, dehydrates after transitions.
- Verb invocation interface works for service tasks.
- Switch adaptor protocol works for exclusive gateways.
- Single-token worked examples (S3 §9 Examples 1, 2, 7, 8) run end-to-end.
- Long-lived waits (timer, message, human task) work; Examples 9 and 10 run end-to-end.
- Crash recovery validated: kill the runtime mid-execution; restart; instance resumes from persisted state.

**Sub-phases**:

**6.1 Schema migration**

Apply S2 §6.2's full Postgres schema as a migration. Tables: `bpmn_instance`, `journey_log`, `active_token`, `instance_data`, `pending_wait`, `pending_timer`, `switch_decision_request`, `event_queue`, `bpmn_audit`. Plus the pack registry table (`decision_pack_registry`) and pack provenance table (`pack_provenance`) if not already present.

**Adam stops and reviews schema before next phase.** This is the only mid-tranche review point in the entire plan, because schema is the load-bearing element.

**6.2 Event queue**

In a new `bpmn-runtime` crate, implement the event queue interface:

```rust
pub trait EventQueue {
    fn enqueue(&self, event: Event) -> Result<EventId>;
    fn dequeue(&self, max: usize) -> Result<Vec<EventEnvelope>>;
    fn ack(&self, event_id: EventId) -> Result<()>;
    fn nack(&self, event_id: EventId, reason: &str) -> Result<()>;
}
```

Implementation backed by the `event_queue` Postgres table with `FOR UPDATE SKIP LOCKED` for concurrent workers.

**6.3 Main event loop**

Per S2 §6.4:

```
loop {
    let events = queue.dequeue(batch_size)?;
    for event in events {
        let instance = hydrate(event.instance_id)?;
        process_event(instance, event)?;
        persist(instance)?;
        queue.ack(event.id)?;
        // dehydrate happens implicitly when instance goes out of scope
    }
}
```

`process_event` dispatches to event-kind-specific handlers (instance_start, verb_completion, timer_fired, message_arrived, switch_decision_reply, human_task_complete, sub_process_complete, error_raised, cancellation).

**6.4 Verb invocation interface**

In `bpmn-runtime`, implement the `VerbContext` trait and the in-process verb invocation pattern per S2 §6.5:

```rust
pub trait VerbHandler: Send + Sync {
    fn invoke(&self, ctx: &mut VerbContext) -> Result<VerbOutput, VerbError>;
}

pub struct VerbContext {
    pub at_slots: AtSlotBindings,
    pub inputs: BTreeMap<String, Value>,
    pub effect_emitter: EffectEmitter,
}
```

Effect emission is the API by which verbs initiate waits, write data, send messages, schedule timers, raise errors.

**6.5 Switch adaptor protocol**

In `bpmn-runtime`, implement the switch adaptor trait and request/reply protocol per S2 §6.6:

```rust
pub trait SwitchAdaptor: Send + Sync {
    fn handle(&self, request: SwitchRequest) -> Result<SwitchReply, SwitchError>;
}
```

In-process trait-based registration. Adaptors are registered against gateway names or kinds. Test harness adaptor implementation provides scripted replies.

**6.6 Test harness scaffolding**

In a new `bpmn-test-harness` crate, implement the scenario DSL per S3 §9 examples:

```rust
scenario("name")
  .start(json!({...}))
  .expect_at("node-name")
  .complete_verb("node-name", json!({...}))
  .adaptor_reply("gateway-name", vec!["branch-name"])
  .expect_status(Completed)
```

Backed by direct invocation of the event loop with synthesised events.

**6.7 Single-token worked examples**

Run S3 §9 Examples 1, 2, 7, 8 through the harness:

- Example 1 (linear sequence): user task wait, complete; service task → service task → end.
- Example 2 (exclusive gateway, Pattern A): single decision; adaptor reply routes correctly.
- Example 7 (subprocess invocation): subprocess scope; completion; parent resumes.
- Example 8 (interrupting error boundary): verb raises error; boundary catches; error path taken.

**6.8 Long-lived waits**

Implement timer service per S2 §6.10:

- `pending_timer` table populated when timer events scheduled.
- Timer worker polls (`FOR UPDATE SKIP LOCKED`); fires expired timers; enqueues `TimerFired` events.

Implement message correlation per S2 §6.10: arriving messages find target instance via correlation keys in `pending_wait` table.

Implement human task completion: external API endpoint that ingests completion → enqueues `HumanTaskComplete` event.

**6.9 Long-lived-wait examples**

Run S3 §9 Examples 9, 10:

- Example 9 (non-interrupting timer boundary): host node continues; timer fires; escalation path runs in parallel (single-token doesn't fully exercise this — defer the parallel aspect to Tranche 7).
- Example 10 (event-based gateway): three catching events registered; first fires; others cancelled.

**6.10 Crash recovery validation**

Manually validate: start an instance; kill the runtime mid-event-processing; restart; verify the instance resumes from persisted state. Per S2 §6.4's idempotency guarantees.

**Inputs**: S2 §6 (entire — overview, schema, events, event loop, verb invocation, switch adaptor, long-lived waits, observability). S3 §9 Examples 1, 2, 7, 8, 9, 10.

**Outputs**:
- `bpmn-runtime` crate populated.
- `bpmn-test-harness` crate populated.
- Postgres schema migrated.
- Six single-token examples run end-to-end.

**Risk notes**:
- **The biggest single tranche.** The runtime is the most novel architectural piece. Sonnet may default to conventional in-memory engine patterns; Adam must push back if so. The journey-persisted hydrate/dehydrate model is the commitment.
- Schema is load-bearing. The mid-tranche STOP at 6.1 is deliberate — get the schema right before any code uses it.
- Crash recovery testing is mostly manual at this stage. Automation comes in Tranche 8.

**STOP gate — Tranche 6 review**:
Adam runs the six single-token examples through the harness. All pass with expected final journey state. Crash recovery scenario validated. Proceed to Tranche 7.

**Do not commit until Adam reviews the diff.** This diff will be substantial. Review in stages: schema first (already gated at 6.1), then event loop, then verb invocation, then switch adaptor, then long-lived waits.

---

## Tranche 7 — Multi-token, parallel-join, merge protocol

**Goal**: Complete the runtime by adding parallel-fork, parallel-join, declared merge protocol, inclusive gateway dynamic semantics, and token death short-circuit.

**Exit criterion**:
- S3 §9 Examples 4, 5, 6 run correctly:
  - Example 4 (inclusive gateway): dynamic fan-out at fork, dynamic fan-in at join.
  - Example 5 (parallel fork/join with declared merge): three tokens fork, merge protocol applies, merged data in instance_data.
  - Example 6 (undeclared write conflict): detect-and-fail diagnostic at runtime; instance status `Failed` with precise diagnostic message naming conflicting branches.
- Multi-token persistence is consistent across crash recovery scenarios.

**Sub-phases**:

**7.1 Parallel-fork**

In `bpmn-runtime`, implement parallel-fork semantics per S2 §6.7:

- When a token arrives at a parallel-fork node, emit N tokens (one per outgoing edge).
- Each child token gets a clean write log.
- All children share the same instance but have distinct token IDs.
- Persist N rows in `active_token` table.

**7.2 Parallel-join arrival accumulation**

When a token arrives at a parallel-join:

- Check the join's expected-arrival set (computed at compile time, stored on the join atom).
- If arrival count < expected, persist arrival, dehydrate. Wait for more arrivals.
- If arrival count == expected, all tokens arrived. Hydrate the join. Proceed to merge resolution.

**7.3 Merge resolution algorithm**

Per S2 §6.8:

```
for each location written by any branch:
    if only one branch wrote it: apply the write
    if multiple branches wrote identical values: apply once
    if multiple branches wrote differing values:
        if join declares merge for location: apply merge operator
        else: fail the instance with diagnostic
```

Merge operators: `max`, `min`, `union`, `concat`, `sum`, `latest`, `earliest`, `custom <verb-ref>`.

**7.4 Detect-and-fail backstop**

When no merge declared and branches conflict, the instance enters status `Failed` with a `JourneyError` event in the audit log. Error message names the conflicting branches and the location.

This is Example 6's exit condition — verify by running.

**7.5 Inclusive gateway dynamic fan-out**

When a token arrives at an inclusive gateway:

- Evaluate each outgoing edge's condition (via switch adaptor).
- For each true edge, emit a child token.
- Record the dynamic fan-out set on the gateway's matching inclusive-join.

**7.6 Inclusive gateway dynamic fan-in**

The inclusive-join's expected-arrival set is the dynamic set recorded at the inclusive-fork (not the compile-time static set). When arrivals match, the join fires.

**7.7 Token death short-circuit**

When a branch terminates (reaches its own end event) without reaching the parallel-join, reduce the join's expected-arrival count by one. If the new count == arrivals, fire the join immediately.

**7.8 Token excess**

If a join's arrival count exceeds expected (more tokens arrive than expected), raise a runtime error. This indicates a compiler bug — should not happen in correctly-compiled bpmn-lite.

**7.9 Multi-token worked examples**

Run S3 §9 Examples 4, 5, 6 through the harness:

- Example 4 (inclusive): adaptor selects 2 of 3 branches; join fires after both arrive; third branch not taken.
- Example 5 (parallel with merge): three tokens fork; each branch writes data; merge applies; instance_data reflects merged state.
- Example 6 (undeclared conflict): two branches write same location with different values; runtime fails the instance with diagnostic.

**7.10 Multi-token crash recovery**

Validate: start an instance with parallel branches; kill the runtime when only some children have completed; restart; verify the runtime correctly identifies the in-flight tokens and resumes.

**Inputs**: S2 §6.7 (multi-token), §6.8 (merge protocol). S3 §9 Examples 4, 5, 6.

**Outputs**:
- Multi-token semantics in the runtime.
- Three multi-token examples pass.
- Crash recovery validated for multi-token instances.

**Risk notes**:
- Inclusive gateway's dynamic expected-set is the most subtle piece. Test exhaustively against Example 4. Edge cases: zero branches selected (inclusive-fork with no true conditions), all branches selected (degenerate to parallel-fork), one branch selected (degenerate to exclusive).
- Token death and token excess are dual edge cases — handle both, test both.

**STOP gate — Tranche 7 review**:
Adam runs the three multi-token examples plus the existing six single-token examples. All pass. Crash recovery scenarios validated. Proceed to Tranche 8.

**Do not commit until Adam reviews the diff.**

---

## Tranche 8 — Integration validation

**Goal**: Validate the full v0.1 implementation against the most complex worked example and the pack-authored example. Confirm composition works.

**Exit criterion**:
- S3 §9 Example 11 (complex KYC onboarding) runs end-to-end correctly. Includes: jurisdictional routing, parallel KYC + deal + IM workstreams, error boundaries, multi-state sign-off loop, timer-based SLA escalation.
- S3-patch §9 Example 12 (pack-authored) runs end-to-end correctly via the Sage-stub script and the runtime.
- All 12 worked examples (1–12) run cleanly in a single test run.
- Crash recovery validated across the full example suite.

**Sub-phases**:

**8.1 Example 11 wiring**

The complex KYC onboarding example combines many features:

- Exclusive gateway (jurisdictional routing).
- Three subprocesses (UK/EU/standard KYC).
- Parallel fork (main work after KYC).
- Three service tasks (KYC, deal, IM).
- Parallel join with merge.
- User task with loop marker (sign-off up to 3 times).
- Error boundary on sign-off (max loop exceeded).
- Non-interrupting timer boundary on intake (SLA reminder).

Wire up the harness scenario and run.

**8.2 Sage-stub script**

For Example 12 pack-authored validation, write a thin Sage-stub script that:

- Takes a natural-language utterance as input.
- Looks up packs in the registry.
- For demonstration, hard-code the match to `conjunctive-gate` for the Example 12 utterance.
- Performs template substitution per S1 §3.5.x.
- Emits the DSL (expanded atoms + provenance atom).
- Submits to `compile()` and then `deploy()`.

This is not production Sage — it's a validation stub. Production Sage matching is v0.2 work.

**8.3 Example 12 end-to-end**

Run Example 12 through the Sage-stub, the compile path, and the runtime. Validate:

- Pack expansion produces expected structural atoms.
- Provenance atom recorded.
- Compilation succeeds.
- Runtime executes the expanded gateway correctly.
- Audit log records the pack provenance.

**8.4 Full suite run**

Single test run executing all 12 worked examples. Assertion: all pass; no flaky behaviour; instance final states match expected.

**8.5 Hardening**

- Observability event emission per S2 §6.12: structured events for instance lifecycle, verb invocation, gateway decisions.
- Audit log completeness: every state transition appears in `bpmn_audit`.
- Error path testing: verify error events properly recorded, error paths properly executed.
- Performance smoke test: run 100 instances in parallel through the harness; verify completion and consistency.

**8.6 v0.1 implementation status document**

Update `docs/v0_1-implementation-status.md`:

- Tranches completed.
- All 12 worked examples pass.
- Known limitations (from `[GAP: ...]` markers).
- Pre-existing bugs surfaced and their resolution status.

**Inputs**: S3 §9 Examples 11, 12. All preceding tranche outputs.

**Outputs**:
- Complex example running.
- Sage-stub script.
- Full 12-example suite green.
- v0.1 implementation status document.

**Risk notes**:
- Example 11 has subtle wiring around KYC subprocess outputs being available to main-fork via the kyc-outcome write. The DSL in S3 §9 Example 11 has a comment noting this; verify the runtime handles cross-section data flow correctly.
- Sage-stub is intentionally minimal. Resist scope creep — production Sage matching is v0.2.

**STOP gate — Tranche 8 review**:
Adam runs the full 12-example suite. All green. Implementation status document accurate. Proceed to Tranche 9 (parallel-runnable).

**Do not commit until Adam reviews the diff.**

---

## Tranche 9 — Decision pack catalogue authoring

**Goal**: Author the 12 seed decision packs as DSL source files in the pack registry. Apply governance metadata. Validate.

**Note**: This tranche can be run in parallel with Tranches 4–8 because packs are independent of compiler/runtime work. The exit criterion just requires Tranche 5 (resolution and pack registry) to be complete.

**Exit criterion**:
- All 12 packs from S3-patch §8.3 and Appendix E loaded as DSL source files.
- Each pack parses cleanly through Tranche 2's parser and validates through Tranche 5's resolution pass.
- Governance status atoms applied per S3-patch §8.5.
- Pack registry contains all 12 packs in `active` state.
- Pack registry queries work (lookup by name, list active, filter by domain scope).

**Sub-phases**:

**9.1 Pack source files**

Create `dsl-source/packs/` directory. One file per pack:

- `dsl-source/packs/conjunctive-gate.dsl`
- `dsl-source/packs/disjunctive-gate.dsl`
- ... etc., 12 files total.

Each file contains the `(decision-pack ...)` atom from S3-patch Appendix E plus the corresponding `(governance-status ...)` atom.

**9.2 Pack registry loader**

In `dsl-core`, add a function to load all pack files from a directory into the pack registry at startup:

```rust
pub fn load_packs_from_dir(dir: &Path, registry: &mut PackRegistry) -> Result<()>;
```

**9.3 Pack validation tests**

Test each pack:

- Parses cleanly.
- Resolves cleanly (template substitution forms valid, parameter types valid).
- Indexed correctly in registry.
- Queryable by name and version.

**9.4 Sage-stub integration**

The Sage-stub from Tranche 8 already uses the pack registry. Verify all 12 packs can be matched by their example utterances and instantiated. Write a script that instantiates each pack with synthetic parameters and verifies the resulting DSL compiles cleanly.

This validates the catalogue, not Sage's matching quality. Production matching is v0.2.

**9.5 Pack catalogue documentation**

Generate `docs/decision-pack-catalogue.md` from the pack source files. One section per pack: name, description, parameters, example utterances, governance status. This is human-readable reference documentation; technical reference remains in S3-patch §8.3 and Appendix E.

**Inputs**: S3-patch §8.3 Packs 1–12. S3-patch Appendix E. S3-patch §8.5 (governance lifecycle).

**Outputs**:
- 12 pack source files.
- Pack registry loader.
- Pack catalogue documentation.

**Risk notes**:
- Several packs (3, 4, 5, 6, 7, 8, 10) have `[GAP: variable-arity atom generation deferred to v0.2]` markers per S3-patch §8.1.3. These pack templates are valid v0.1 templates for fixed N (typically N=2 or N=3); the variable-arity case requires a v0.2 `for-each` template combinator. Load them as-is; defer the v0.2 work.

**STOP gate — Tranche 9 review**:
Adam confirms all 12 packs loaded, validated, and documented. Proceed to Tranche 10.

**Do not commit until Adam reviews the diff.**

---

## Tranche 10 — Documentation and handoff

**Goal**: Finalise v0.1 documentation. Capture v0.2 backlog from accumulated `[GAP: ...]` markers and surfaced issues. Tag the implementation.

**Exit criterion**:
- README documents reflect v0.1 implementation state.
- Operational runbook covers deployment, monitoring, common failure modes.
- v0.2 backlog captured with priorities.
- Tag `impl/v0.1` applied in git.

**Sub-phases**:

**10.1 README updates**

Update top-level README to reflect:

- Implementation state of v0.1.
- How to run the system locally.
- How to run the test suite.
- How to add new verbs, packs, processes.

**10.2 Operational runbook**

Create `docs/operations/v0_1-runbook.md`:

- Deployment procedure (schema migrations, runtime startup).
- Monitoring (metrics, journey log queries, audit log queries).
- Common failure modes and responses (instance stuck in Waiting, journey log inconsistency, timer worker crashed).
- Backup and recovery procedures.

**10.3 v0.2 backlog**

Aggregate all `[GAP: ...]` markers from the v0.1 design and implementation. Categorise:

- **Language extensions**: variable-arity template combinator (`for-each`), conditional events, parallel multi-instance dynamic expected count, additional `@`-slots.
- **Runtime extensions**: full BPMN compensation beyond transaction-subprocess scope, timer cycle support, async verb invocation (cross-process), advanced observability.
- **Catalogue extensions**: additional decision packs (the seed 12 are a starting point; common patterns will surface in practice).
- **Tooling**: production Sage pack-matching (currently a stub), BPMN/DMN XML migration tool, pack catalogue browser UI.
- **Type system**: full type lattice per S1 §3.8 GAP.
- **Performance**: stress testing at 10k+ concurrent instances, partitioned event queue if needed.

Write `docs/v0_2-backlog.md` with these items, each tagged with origin (which v0.1 GAP marker or surfaced issue).

**10.4 Pre-existing bug resolution**

Review the bug log from Tranche 1.6 (effect declaration verification). Categorise remaining bugs as:

- Fixed in v0.1.
- Deferred to v0.2 (with rationale).
- Won't fix (with rationale).

**10.5 Tag**

Apply git tag `impl/v0.1`. Push to remote.

**Inputs**: All preceding tranche outputs. All `[GAP: ...]` markers from v0.1 design.

**Outputs**:
- Updated README.
- Operational runbook.
- v0.2 backlog document.
- `impl/v0.1` tag.

**Risk notes**: None. Mechanical work.

**STOP gate — Tranche 10 review**:
v0.1 implementation tagged. Backlog captured. v0.1 is complete.

**Do not commit until Adam reviews the diff.**

---

## Summary

| Tranche | Goal | Exit criterion | Risk |
|---|---|---|---|
| 0 | Consolidate v0.1 design | `design/v0.1` tagged | Low |
| 1 | Regression baseline | Snapshot tests green, CI integrated | Coverage gaps may extend tranche |
| 2 | Atom model + parser | S1 examples parse to typed AST | Low |
| 3 | SemOS reshape | Regression suite green | **Highest** — behavioural drift |
| 4 | bpmn-lite compile | 12 examples compile | Low |
| 5 | Resolution + packs | 12 examples validate end-to-end | Low |
| 6 | Runtime core | 6 single-token examples run | **High** — novel architecture |
| 7 | Multi-token | 3 multi-token examples run | Medium — inclusive gateway edges |
| 8 | Integration | All 12 examples run | Medium — composition surprises |
| 9 | Pack catalogue (parallel-runnable) | 12 packs loaded | Low |
| 10 | Docs + handoff | `impl/v0.1` tagged | None |

**Wall-clock**: 25–35 of Adam's working sessions, distributed by day-job availability.

**Mid-tranche STOPs**: only one — between Phase 6.1 (schema migration) and the rest of Tranche 6. Schema is load-bearing enough to warrant a review point.

**Stopping points worth considering**:

- After **Tranche 5**: working unified compiler, no runtime yet. Defensible artifact.
- After **Tranche 6**: single-token runtime. Adequate for most workflow patterns.
- Full **Tranche 10**: complete v0.1.

**v0.2 anticipated work** (from `[GAP: ...]` markers):

1. `for-each` template combinator (highest priority — affects 7 of 12 packs).
2. Variable-arity atom generation in pack templates.
3. Full type lattice.
4. Conditional events.
5. Full BPMN compensation semantics.
6. Production Sage pack-matching.
7. BPMN/DMN XML migration tooling.
8. Cross-process async verb invocation.

This master plan is sufficient to drive v0.1 implementation end-to-end without further architectural decision-making. All architectural decisions are committed; remaining decisions are tactical (sequencing within tranches, pace, when to STOP).
