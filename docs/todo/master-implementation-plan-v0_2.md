# ob-poc v0.2 Implementation — Master Plan

**Status**: Draft v1.0 — generated post-v0.1 shipment
**Scope**: Production Sage, `for-each` template combinator, Camunda 8 migration tool, diagram renderer, operational hardening, compliance-officer review pilot, additional decision packs, VerbConfig YAML retirement
**Workflow**: Adam architects and reviews; Sonnet (in Zed) implements. Tranche-level STOP gates; lighter ceremony than v0.1 — v0.1 proved the workflow.
**Release strategy**: single coherent v0.2 release; ship all tranches together when complete.
**Centre of gravity**: Sage (AI authoring). The v&s paper's central claim — that AI can participate safely in real system work — depends on Sage actually working in production.
**Estimated wall-clock**: 20–30 of Adam's working sessions; less than v0.1 because the language layer is stable and most work is greenfield against settled architecture.

---

## How v0.2 differs from v0.1

v0.1 reshaped a working SemOS into a new language layer while preserving behaviour. That was the hardest possible kind of work — refactor under regression protection. The discipline (Tranche 1 regression baseline before Tranche 3 SemOS reshape) was load-bearing.

v0.2 is mostly **greenfield against a settled architecture**. The DSL, compiler, runtime, and pack catalogue are stable. Sage builds on top. The migration tool builds against bpmn-lite's settled verb catalogue. The diagram renderer reads existing typed graphs. None of this work threatens existing behaviour.

This permits lighter ceremony:

- **No regression baseline tranche.** v0.1's Tranche 1 protected the SemOS reshape. v0.2 has no equivalent refactor to protect. v0.1's test suite continues running as the baseline.
- **No mid-tranche STOPs.** v0.1's only mid-tranche STOP was the schema migration in Tranche 6. v0.2 has no schema migration on a comparable scale (Sage adds tables, but they are additive, not load-bearing on existing behaviour).
- **Tighter tranche reviews.** v0.1 tranche reviews were heavy because the diffs were large (~1,300 verbs reshaped). v0.2 tranche reviews are lighter because the diffs are bounded by capability scope.
- **Parallel-runnable tranches.** Several v0.2 tranches are independent (e.g., the diagram renderer is independent of Sage; the migration tool is independent of `for-each`). Sequencing reflects priority, not dependency.

The discipline that stays:

- **Tranche-level STOP gates** with concrete exit criteria.
- **"Do not commit until Adam reviews the diff"** at every tranche close.
- **All v0.1 tests must remain green** after every tranche.

---

## Reference state

v0.2 starts from `impl/v0.1` tagged in `adamtc007/ob-poc`. Key facts:

- DSL workspace: 9 crates (`dsl-core`, `dsl-parser`, `dsl-ast`, `dsl-semos-frontend`, `dsl-bpmn-frontend`, `dsl-resolution`, `dsl-lowering`, `dsl-diagnostics`, `bpmn-runtime`).
- 12 decision packs registered, 7 of which are representative fixed-arity forms pending `for-each`.
- 1,332 SemOS verbs reshaped to unified DSL format; 155 `.dsl` files.
- 124 SemOS regression snapshot tests, 89 v0.1 implementation tests, 6 round-trip validation tests. **219 tests must remain green through all v0.2 tranches.**
- Postgres schema migrations applied; runtime operational.
- Sage interaction documented (Session 3-patch §8.4); production Sage stubbed (Tranche 8.2 of v0.1).
- VerbConfig YAML loading retained as fallback (Tranche 3.7 hold).
- BGE/Candle embeddings infrastructure in place from earlier ob-poc work.

---

## Tranches

Eight tranches. Tranche 0 is small (consolidation, planning, embeddings audit). Tranches 1–4 are Sage. Tranches 5–7 are supporting capabilities. Tranche 8 is integration and release.

| # | Name | Scope | Wall-clock |
|---|---|---|---|
| 0 | Foundation: backlog consolidation + embeddings audit + `for-each` | Mechanical foundations | 2–3 sessions |
| 1 | Sage core: pack matching with embeddings + LLM rank | Pack catalogue → ranked candidates | 3–4 sessions |
| 2 | Sage parameter extraction and confirmation | Selected pack → parameter binding → user confirm | 3–4 sessions |
| 3 | Sage instantiation pipeline | Confirmed pack + parameters → DSL emission + provenance | 2–3 sessions |
| 4 | Sage end-to-end integration | Full loop: utterance → execution | 2–3 sessions |
| 5 | Camunda 8 migration tool | XML in, DSL out, human-supervised | 3–4 sessions |
| 6 | Diagram renderer | DSL → SVG | 2–3 sessions |
| 7 | Operational hardening | Observability, metrics, scale | 2–3 sessions |
| 8 | Compliance pilot + integration + release | Empirical validation + v0.2 release | 2 sessions |

---

## Tranche 0 — Foundation

**Goal**: Tie up loose ends from v0.1 and lay the groundwork for Sage. Three logically distinct pieces of work clustered as one tranche because each is small and they share context.

**Exit criterion**:
- `[GAP: ...]` markers from v0.1 design documents catalogued into `docs/v0_2-backlog.md`.
- Existing BGE/Candle embeddings infrastructure audited; integration interface for Sage documented.
- `for-each` template combinator implemented in `dsl-resolution`; the 7 representative-only packs (linked-switch-chain, parallel-evaluation-with-veto, cascading-decision, decision-table-classification, threshold-band-routing, required-evidence-checklist, multi-jurisdiction-overlay) re-expressed using `for-each`; pack validation suite updated.
- All v0.1 tests still green.

**Sub-phases**:

**0.1 v0.2 backlog cataloguing.** Sweep v0.1 design documents for `[GAP: ...]` markers. Aggregate into `docs/v0_2-backlog.md` with categorisation (Sage, language, runtime, tooling, governance, ops). Cross-reference against this master plan to verify nothing is missed.

**0.2 Embeddings infrastructure audit.** Locate the existing BGE/Candle integration in the ob-poc workspace. Document its current interface: input format, embedding dimensions, similarity computation, caching behaviour. Identify whether the existing interface can be called from the new `dsl-*` crate workspace, or whether a thin adapter crate (`dsl-sage-embeddings`) needs to be added. Produce a one-page integration note for Tranche 1's pack matcher to consume.

**0.3 `for-each` template combinator implementation.** Add the `for-each` form to the pack template grammar in `dsl-parser`. Syntax (proposal, subject to refinement during implementation):

```
(decision-pack threshold-band-routing
  :parameters [(bands :list of band-spec)]
  :template
    (gateway ,gateway-name
      (for-each ,band in ,bands
        (branch :condition (in-range ,(band/min) ,(band/max))
                :next ,(band/destination)))))
```

Semantics: at instantiation, the `for-each` expands once per element in the bound list, with the loop variable in scope inside the body. Bindings are scalar (`,band`) or accessor (`,(band/field)`).

Implementation:
- Parser: recognise `(for-each <var> in <param-ref> <body>)` form.
- AST: new `TemplateForEach` node type.
- Resolution: validate that the parameter is list-typed; validate that loop-variable accessors resolve to known fields of the element type.
- Expansion (in `dsl-bpmn-frontend` or pack expansion module — whichever owns instantiation): unroll the loop into N copies of the body, substituting the loop variable into each.

**0.4 Re-express the 7 packs.** Update the seven representative-only seed packs to use `for-each`. Validate each parses, resolves, and produces expected expansion when instantiated with synthetic parameters. Add tests for variable-arity instantiation (N=1, N=3, N=10).

**0.5 Documentation update.** Pack catalogue documentation reflects that all 12 packs now support full variable arity. Remove the v0.1 caveats.

**STOP gate**: All 12 packs functional with full arity. All v0.1 tests green. Embeddings integration note ready for Tranche 1.

---

## Tranche 1 — Sage core: pack matching

**Goal**: Implement the pack-matching layer. Utterance + context → ranked list of candidate packs. This is the load-bearing AI component of Sage.

**Exit criterion**:
- Given a natural-language utterance, the matcher returns a ranked list of candidate packs with confidence scores and human-readable reasoning.
- Top-1 accuracy on a curated 50-utterance evaluation set ≥ 80%.
- Top-3 accuracy on the same set ≥ 95%.
- The matcher operates within latency budget: < 2 seconds end-to-end for a single utterance against the 12-pack catalogue.

**Sub-phases**:

**1.1 Pack utterance-binding catalogue.** Each pack has a `:utterance-bindings` slot listing canonical example utterances (e.g., "all of these must be true before proceeding"; "require KYC and UBO and sanctions"). Ensure each of the 12 packs has 3–5 utterance-bindings. Where v0.1 packs lack this, author them now.

**1.2 Embedding generation.** For each pack, generate embeddings for the pack's utterance-bindings. Store in a pack-embedding index keyed by `(pack-name, binding-index)`. Use the BGE infrastructure from the Tranche 0 audit.

**1.3 Candidate retrieval.** Implement the retrieval layer:
- Embed the user's utterance.
- Compute cosine similarity against all stored pack-binding embeddings.
- For each pack, take its highest-scoring binding similarity as the pack's candidate score.
- Return top-K (K=5 by default) packs by candidate score.

**1.4 LLM ranking layer.** Implement the ranking layer:
- Given the user's utterance and top-K candidate packs, build an LLM prompt that asks for ranking with reasoning.
- Prompt structure: utterance + per-pack [name, description, parameters, 1–2 example utterances]. Ask for ranked list with one-sentence rationale per pack.
- LLM call (Claude via API, configured per ob-poc conventions). Parse response into structured `RankedCandidate` records.

**1.5 Confidence scoring.** Combine the embedding similarity score and the LLM ranking position into a single `confidence: f32` in `[0, 1]`. Document the scoring function in code.

**1.6 Evaluation harness.** Build a structured evaluation harness:
- Curated set of 50 utterances spanning the 12 packs, each labelled with the correct pack.
- Run the full matcher pipeline against the set.
- Report top-1 accuracy, top-3 accuracy, average latency, average LLM token cost.
- Surfaced failures inspected: was the retrieval wrong, or the ranking wrong, or the test labelling debatable?

**1.7 Matcher service interface.** Wrap the matcher as a service callable from the REPL and from Tranche 2's parameter extraction stage. Interface:

```rust
pub fn match_packs(utterance: &str, context: &SageContext) -> Vec<RankedCandidate>;

pub struct RankedCandidate {
    pub pack_name: PackName,
    pub pack_version: Version,
    pub confidence: f32,
    pub rationale: String,
}
```

**STOP gate**: Evaluation harness reports ≥ 80% top-1 accuracy and ≥ 95% top-3 accuracy. Latency within budget. All v0.1 tests green.

**Note on the accuracy thresholds**: 80% top-1 / 95% top-3 are stretch targets. If the system reaches 70% top-1 / 90% top-3, that's still operationally useful — the human confirmation step in Tranche 2 will catch mismatches in the remaining 10% of cases. If the numbers come in materially below those, the evaluation set may be the wrong shape or the embeddings/LLM combination may need tuning. Don't redesign the architecture; investigate the data first.

---

## Tranche 2 — Sage parameter extraction and confirmation

**Goal**: Given a selected pack, extract parameter values from the user's utterance and conversation history. Drive the confirmation interaction with the user.

**Exit criterion**:
- For each of the 12 packs, given an utterance plus the selected pack, the extractor proposes parameter values for the pack's declared parameters with confidence scores.
- A confirmation interaction is implemented: the system shows the user what it proposes to instantiate; the user confirms, edits, or rejects.
- On rejection or major edit, the flow returns to Tranche 1 (re-select pack) or accepts user-supplied values.

**Sub-phases**:

**2.1 Parameter extraction prompt design.** For each pack parameter, design an LLM extraction prompt that takes the utterance, conversation history, the pack metadata, and asks for proposed values with rationale. Structured output (JSON or similar) parseable into typed parameter values.

**2.2 Parameter extractor implementation.** Wrap the prompt design into an extractor that, given `(utterance, history, pack)`, returns:

```rust
pub struct ParameterProposal {
    pub parameter_name: ParameterName,
    pub proposed_value: Value,
    pub confidence: f32,
    pub rationale: String,
    pub source_phrase: Option<String>,  // span from utterance that motivated the value
}
```

**2.3 Type validation.** Proposed values are type-checked against the pack's parameter declarations. Type mismatches surface as low-confidence proposals with explanatory notes.

**2.4 Confirmation UI/interaction protocol.** Define the structured confirmation protocol. The UI is out of scope for this tranche (it lives in `ob-poc-ui-react`); the protocol is the contract:

```rust
pub struct ConfirmationRequest {
    pub pack: PackName,
    pub pack_version: Version,
    pub proposed_parameters: Vec<ParameterProposal>,
    pub preview_dsl: String,  // rendered DSL with proposed parameters substituted
}

pub enum ConfirmationResponse {
    Accept,
    EditParameter { name: ParameterName, new_value: Value },
    RejectPack,  // returns to Tranche 1 for re-matching
    Cancel,
}
```

**2.5 Edit loop.** When the user edits a parameter value, re-validate types, refresh the preview DSL, and re-present. Multiple edits per session allowed.

**2.6 Conversation history threading.** The extractor sees not just the current utterance but the recent conversation. Implement a conversation context window of N turns (start with N=10; tune from evaluation data).

**2.7 Evaluation against the same 50-utterance set.** For each utterance whose pack was correctly identified in Tranche 1, test parameter extraction. Report accuracy at the parameter level (how often is each parameter correctly extracted?). Surface failures and inspect.

**STOP gate**: For all 12 packs, parameter extraction proposes plausible values with rationale. Confirmation protocol is sound. All v0.1 tests green. Tranche 1 evaluation set extended to cover parameter extraction quality.

---

## Tranche 3 — Sage instantiation pipeline

**Goal**: Given a confirmed pack and parameter values, produce DSL artifacts (structural atoms + provenance atom) ready for compilation, testing, and deployment.

**Exit criterion**:
- Confirmed instantiation produces well-formed DSL source.
- Provenance atom is emitted recording: pack name, pack version, instantiation timestamp, authoring user, conversation reference, confirmed parameters.
- The produced DSL compiles cleanly through the v0.1 compiler.
- The produced DSL runs cleanly through the v0.1 test harness with synthetic scenarios.

**Sub-phases**:

**3.1 Template expansion.** The pack template is already validated and resolvable from v0.1. Substitute the confirmed parameters into the template using the `,name`, `,@name`, and `for-each` forms from Tranche 0. Produce expanded structural atoms.

**3.2 Provenance emission.** Generate the `(provenance ...)` declarative atom per S1 §3.10:

```
(provenance
  :covers [<expanded-atom-names>]
  :source-id conjunctive-gate
  :version "1.0"
  :instantiated-at "2026-05-21T14:30:00Z"
  :instantiated-by "user@example.com"
  :session-id "session-uuid"
  :parameters {
    gateway-name: "kyc-complete-check"
    conditions: ["kyc-approved" "ubo-resolved" "sanctions-clear"]
  })
```

**3.3 DSL emission.** Serialise expanded atoms + provenance atom to DSL source text. Pretty-print with consistent indentation. Output destination is configurable (file path, in-memory string, REPL response).

**3.4 Compile validation.** Run the produced DSL through the v0.1 compile pipeline (parse → assemble → resolve → lower). Surfaces any errors before presenting to user.

**3.5 Test scenario suggestion.** For each pack, the pack metadata includes example test scenarios. Adapt them to the instantiated parameters and offer them to the user for execution against the v0.1 test harness. The user can accept, edit, or skip.

**3.6 Test execution.** Selected scenarios run through the v0.1 test harness. Results surface as pass/fail with diagnostics.

**3.7 Deployment gate.** A successful compile + test cycle produces a deployable artifact. The actual deployment requires explicit user confirmation per the v&s paper's box 5. Deployment writes the DSL source to the workspace, registers it in the pack provenance registry, and makes it executable.

**STOP gate**: For all 12 packs, end-to-end instantiation produces compiling, test-passing DSL with provenance. Deployment gate is explicit. All v0.1 tests green.

---

## Tranche 4 — Sage end-to-end integration

**Goal**: Stitch Tranches 1–3 together into a coherent end-to-end Sage authoring flow. Wire to the REPL and to the React UI. Run the full loop from utterance to deployed execution.

**Exit criterion**:
- The full loop runs end-to-end through the REPL: utterance → pack match → parameter extraction → confirmation → instantiation → compile → test → deploy → execute → audit.
- The same loop runs end-to-end through the React UI (basic interaction; UI polish is post-v0.2).
- 12 end-to-end demonstrations: one per seed pack, each documented as a worked example showing utterance, intermediate Sage state, final DSL, test results, and execution trace.
- The audit log captures the full provenance chain: authored by Sage at time T, from utterance U, matched pack P at confidence C, parameters P, confirmed by user U at time T', tests run at time T'', deployed at time T''', executed at time T''''.

**Sub-phases**:

**4.1 Sage orchestrator.** A top-level orchestrator coordinates the matcher (Tranche 1), the extractor (Tranche 2), the instantiator (Tranche 3), and the surrounding gate machinery (compile, test, deploy). State machine over the conversation:

```
States: Listening → Matching → Confirming → Instantiating → Compiling → Testing → Deploying → Executing → Done
Transitions are user-driven (Accept/Edit/Reject) or system-driven (Compile success/fail, Test pass/fail).
```

**4.2 REPL integration.** New REPL endpoints:
- `sage_start(utterance)` — initiates a session.
- `sage_continue(session_id, response)` — supplies the user's confirmation or edit.
- `sage_status(session_id)` — returns current state machine state.

**4.3 React UI integration.** Basic UI that surfaces:
- Utterance input field.
- Pack match list with confidence and rationale.
- Parameter confirmation panel with editable fields and DSL preview.
- Test results summary.
- Deployment confirmation modal.
- Execution status display.

UI polish (animation, accessibility, mobile responsiveness) is post-v0.2.

**4.4 Audit log integration.** Each transition in the Sage state machine writes to the audit log per the v&s paper's box 7. The audit log gains a `sage_session` table linking session ID to all the artifacts produced.

**4.5 12 worked examples.** For each seed pack, capture a worked example:
- The utterance.
- The matcher's top-3 candidates with confidence.
- The user's pack selection.
- The extractor's parameter proposals.
- The user's confirmation.
- The emitted DSL.
- The compile result.
- The selected test scenarios and their results.
- The deployment.
- The execution trace.

These become `docs/v0_2-worked-examples/` and are referenced from the v0.2 release notes.

**STOP gate**: The full loop runs end-to-end for all 12 packs. Audit captures all 7 boxes from the v&s loop diagram. All v0.1 tests green.

---

## Tranche 5 — Camunda 8 migration tool

**Goal**: Build a one-way migration tool that ingests Camunda 8 BPMN XML and produces bpmn-lite DSL. Human-supervised, partial coverage, lossy where Camunda 8 has features bpmn-lite deliberately rejects.

**Exit criterion**:
- The tool processes Camunda 8 XML and emits bpmn-lite DSL plus a coverage report.
- For a curated set of representative Camunda 8 models (5–10 real-world or realistic-synthetic), the tool produces DSL that compiles and produces sensible test execution.
- Where the tool cannot translate (FEEL expressions, BPMN extensions outside bpmn-lite's verb catalogue), it surfaces specific markers that a human must resolve before the DSL is deployable.

**Sub-phases**:

**5.1 Camunda 8 XML parsing.** Use an existing BPMN XML parser library or write a thin reader. Produce a typed intermediate representation of the Camunda 8 model.

**5.2 Element mapping catalogue.** For each Camunda 8 element used in the input set, declare its bpmn-lite equivalent (or its handling strategy: rejected, marked-for-human, lowered):
- BPMN tasks (service, user, manual, business rule, script) → bpmn-lite tasks with appropriate verb references.
- BPMN gateways (exclusive, inclusive, parallel, event-based) → bpmn-lite gateways with switch adaptor stubs.
- BPMN events (start, end, intermediate, boundary, error, timer, message, signal) → bpmn-lite events.
- BPMN flows (sequence, conditional, default) → bpmn-lite flows.
- FEEL expressions on gateway conditions → marked-for-human (no automatic translation; human supplies S-expression equivalent).
- Camunda-specific extensions → rejected with diagnostic.
- BPMN compensation → marked-for-human (bpmn-lite has scope-limited compensation).

**5.3 Verb resolution.** For service tasks, attempt to match the Camunda task's named operation (often via `implementation` attribute) to a known SemOS verb. Where the match is direct, use the verb. Where it's not, generate a stub verb declaration that the human must resolve.

**5.4 DSL emission.** Produce bpmn-lite DSL source. Include a `migration-source` declarative atom recording the Camunda 8 file path, hash, and migration timestamp.

**5.5 Coverage report.** Generate a report summarising:
- Total elements in source: N
- Cleanly migrated: M
- Marked for human resolution: K
- Rejected: R
- For each marked/rejected element, the specific reason and the source location.

**5.6 Migration test corpus.** Curate 5–10 Camunda 8 models. Run the tool. Resolve marked elements manually. Verify produced DSL compiles, tests, and executes correctly. Iterate the mapping catalogue when failures surface.

**5.7 Documentation.** Migration tool user guide: how to run, how to interpret coverage report, how to resolve marked elements, what is and is not supported.

**STOP gate**: Migration tool processes the test corpus to compiling DSL after human resolution of marked elements. Coverage report is informative. All v0.1 and v0.2 tests green.

---

## Tranche 6 — Diagram renderer

**Goal**: Produce SVG diagrams from DSL railway sources. Read-only renderer (not a visual editor). For compliance-officer review, documentation, and external comprehension.

**Exit criterion**:
- Given a bpmn-lite DSL source, the renderer produces an SVG that legibly shows the railway.
- Node kinds (start, end, task, gateway, subprocess, boundary event) are visually distinct.
- Gateway kinds (exclusive, inclusive, parallel) are visually distinct.
- Decision packs are visually annotated where used (a subtle marker showing "this gateway was instantiated from pack X").
- Layout is automatic (no manual placement); reads naturally left-to-right or top-to-bottom.

**Sub-phases**:

**6.1 Graph layout algorithm.** Use an existing layout library (e.g., `layout-rs`, `dagre-rs`, or shell out to `graphviz`) to produce node coordinates. Tune layout parameters for railway aesthetics (consistent node spacing, edge bundling).

**6.2 SVG generation.** Render the laid-out graph as SVG with:
- Distinct shapes for each node kind.
- Labelled edges with conditions where present.
- Gateway diamond markers with directional cues.
- Boundary events as smaller circles attached to host nodes.
- Subprocess scopes as enclosing rectangles.

**6.3 Pack provenance annotation.** Where structural atoms have a covering `(provenance ...)` atom, annotate the corresponding nodes with a subtle pack-name marker.

**6.4 Configuration.** Style configuration via a config file (colours, fonts, sizing). Defaults match BNY/ob-poc visual conventions where they exist.

**6.5 REPL and CLI integration.** New endpoints:
- REPL: `render_diagram(dsl_source) → SvgString`.
- CLI: `obpoc render <input.dsl> <output.svg>`.

**6.6 React UI integration.** The UI surfaces rendered diagrams in the Observatory and in the Sage confirmation panel (so the user sees the diagram of the proposed instantiation before confirming).

**6.7 Test corpus.** Render all 12 worked examples from v0.1 (Session 3 §9) and all 12 worked examples from Tranche 4 v0.2. Inspect for legibility.

**STOP gate**: All 24 worked examples render to legible SVGs. UI integration functional. All v0.1 and v0.2 tests green.

---

## Tranche 7 — Operational hardening

**Goal**: Bring the runtime from "correct by construction" to "operationally robust for sustained production load". Observability, metrics, monitoring, scale, retention.

**Exit criterion**:
- Structured metrics emitted from runtime per S2 §6.12. Wired to a metrics collector (Prometheus or compatible).
- Audit log retention policy implemented; old entries archive per declared TTL.
- Performance smoke test: 1,000 concurrent journey instances, mixed workload, sustained for 1 hour. No deadlocks; no journey-log inconsistency; latency budget met.
- Runtime operations runbook documented: deployment, monitoring queries, common failure modes and responses, backup and recovery.

**Sub-phases**:

**7.1 Metrics inventory.** Per S2 §6.12: instance counts by state, verb invocation rates, gateway decision rates, timer pending counts, wait pending counts, parallel-join fan-out distributions. Implement metrics with `prometheus` crate or equivalent.

**7.2 Audit log retention.** `bpmn_audit` and `journey_log` retention policy:
- Active and recent instances: full detail, online.
- Completed instances older than N days: archived to `bpmn_audit_archive`, queryable but slower.
- Compliance-mandated retention beyond N years: cold storage with documented restore path.

Implement the archival job. Configurable N.

**7.3 Performance smoke test.** Generate synthetic load:
- 1,000 instance starts spread over a configurable ramp-up.
- Mixed verb invocations (some synchronous, some long-wait).
- Some parallel-fork instances.
- Some timer-triggered escalations.
Run for 1 hour. Monitor metrics. Assert no deadlocks, journey-log consistency, latency within budget.

**7.4 Partitioning analysis.** If the smoke test surfaces scale limits, prototype event-queue partitioning. The Postgres `FOR UPDATE SKIP LOCKED` pattern from v0.1 handles modest concurrency; serious load may need partition keys. This is exploratory — only build if smoke test indicates need.

**7.5 Runbook.** Operational runbook for the runtime:
- Deployment procedure (schema migrations, runtime startup, dependency ordering).
- Monitoring queries (which instances are stuck? what's the timer queue depth? what's the journey log growth rate?).
- Common failure modes (instance stuck in Waiting; journey log inconsistency; timer worker crashed; switch adaptor unresponsive) with diagnostic queries and remediation steps.
- Backup and recovery (which tables to back up; how to restore from a known-good snapshot; how to verify integrity after restore).

**7.6 Alerting templates.** Per S2 §6.12 alerting thresholds: instance failure rate, event queue depth, timer queue depth, audit log growth rate. Translate to Prometheus alert rules.

**STOP gate**: Smoke test passes. Runbook complete. Metrics flowing. All v0.1 and v0.2 tests green.

---

## Tranche 8 — Compliance pilot, integration, release

**Goal**: Empirically validate the compliance-officer-reviewability claim. Integrate all v0.2 capabilities. Tag the release.

**Exit criterion**:
- Compliance pilot conducted with at least one real compliance officer (or, if unavailable, a credible proxy — domain-expert reviewer with regulatory experience).
- Pilot report documents findings: what was readable, what was confusing, what training would help, whether the diagram renderer materially improved comprehension.
- All v0.2 capabilities integrated: Sage operational, migration tool available, diagram renderer wired, operational hardening in place, all 12 packs functional with full arity.
- v0.2 release notes drafted.
- `impl/v0.2` tag applied.

**Sub-phases**:

**8.1 Pilot design.** Structure the pilot:
- Show the reviewer 3–5 worked examples spanning packs (KYC, sanctions, sign-off, jurisdictional routing).
- Show each as: utterance + Sage interaction screenshots + final DSL + rendered diagram.
- Ask: "Can you tell me what this workflow does? Can you tell me whether it captures the rule correctly? What would help you read it more easily?"
- Record responses verbatim.

**8.2 Pilot execution.** Run the pilot. 30-60 minute session, audio-recorded if consent given, structured notes either way.

**8.3 Pilot report.** Document findings:
- What was readable without explanation.
- What required clarification.
- What was misread or misunderstood.
- What changes (to DSL, to diagram renderer, to surrounding docs) would help.
- Recommendation: is the claim defensible, conditionally defensible, or weaker than the v&s paper states?

**8.4 v&s paper revision (if needed).** If the pilot surfaces material gaps, revise the v&s paper's §9 (Honest Commitments) to reflect what was actually learned. Honesty is part of the architectural defence.

**8.5 v0.2 release notes.** Draft the release notes covering:
- Sage operational (with link to worked examples).
- Migration tool available (with coverage statement).
- Diagram renderer (with sample SVGs).
- Operational hardening (with metric inventory and runbook).
- All 12 packs functional with full arity.
- Known gaps remaining for v0.3.

**8.6 Tag.** Apply `impl/v0.2`. Push to remote. Update top-level README to point to v0.2 documentation.

**STOP gate**: Pilot complete with report. Release notes published. All artifacts integrated and tested end-to-end. `impl/v0.2` tagged. All v0.1 and v0.2 tests green.

---

## Summary

| # | Name | Sessions | Cumulative |
|---|---|---|---|
| 0 | Foundation (backlog + embeddings + `for-each`) | 2–3 | 2–3 |
| 1 | Sage core (pack matching) | 3–4 | 5–7 |
| 2 | Sage parameter extraction | 3–4 | 8–11 |
| 3 | Sage instantiation pipeline | 2–3 | 10–14 |
| 4 | Sage end-to-end integration | 2–3 | 12–17 |
| 5 | Camunda 8 migration tool | 3–4 | 15–21 |
| 6 | Diagram renderer | 2–3 | 17–24 |
| 7 | Operational hardening | 2–3 | 19–27 |
| 8 | Compliance pilot + release | 2 | 21–29 |

**Wall-clock**: 20–30 of Adam's working sessions, distributed by day-job availability. Less than v0.1 because (a) the language layer is stable, (b) most work is greenfield, (c) lighter ceremony.

**Centre of gravity**: Tranches 1–4 (Sage) are the v&s paper's central claim becoming operational. Without these, v0.2 is a refinement release; with these, v0.2 is when the AI-safe intent boundary becomes real, not aspirational.

**Stopping points worth considering**:
- After **Tranche 4**: Sage operational; migration, rendering, ops, compliance pilot deferred. Adequate to validate the v&s argument; not adequate for external rollout.
- After **Tranche 7**: production-deployable. Compliance pilot remaining.
- After **Tranche 8**: complete v0.2.

**Parallel-runnable**: Tranches 5, 6, 7 are largely independent of each other and of Tranches 1–4 (though 6 is more useful after 4 so the rendered diagrams cover Sage-authored workflows). If you ever have more bandwidth than Sonnet, these can be interleaved.

**v0.3 anticipated work** (anticipated from current trajectory):
- Production Sage learning loop — refining matching/extraction quality from production usage data.
- Additional decision packs from operational experience.
- Migration tool extensions (additional Camunda extensions, possibly Activiti, possibly Zeebe-specific patterns).
- Visual diagram editor (writing DSL through diagrams as an alternative input mode).
- Full type lattice from S1 §3.8 GAP.
- Cross-process async verb invocation.
- VerbConfig YAML retirement (the long-deferred Tranche 3.7 from v0.1).

This master plan is sufficient to drive v0.2 implementation end-to-end. All architectural decisions are settled; remaining decisions are tactical.
