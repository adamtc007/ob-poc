# Camunda 8 → bpmn-lite DSL Transpiler — Design v0.6

> **Status:** Draft for peer review — no implementation yet  
> **Date:** 2026-05-26  
> **Scope:** `dsl-migrate` crate (ob-poc) + FEEL normaliser (new) + round-trip harness + Form.io verb

---

## Changelog (v1 → v0.7)

- **§10** — tightened against existing runtime machinery: `FormPending` outcome deleted (use `RequestHumanTask` effect + `HumanTaskComplete` event, both already defined); `form_submit` REPL input kind deleted (use existing `EventKind::HumanTaskComplete`); `behavior: plugin` noted as conventional (screening.yaml:44); reclassification risk downgraded (all task kinds route identically through `is_task_kind`, `processor.rs:919`); fidelity alternative offered for `:kind user-task`; `formKey` normalisation promoted to mapping table; Q8 reframed to ask correlation key only. Net: §10.5 shrinks — most proposed-new components are existing scaffolding needing dispatch wiring, not new machinery.

## Changelog (v1 → v0.6)

- **§1** — dmn-lite is a federated external peer, not a local crate; architectural table corrected.
- **§3** — atom vocabulary corrected to match actual emitter output: `(node id :kind ...)`, `(gateway id :kind ...)`, `(flow a -> b)`; `:when` → `:condition` throughout.
- **§4** — FEEL→s-expression translation table **deleted**. The condition receiver (`RailwayEdge.condition: Option<String>`) is opaque and not evaluated by the runtime (`ScriptedAdaptor` ignores it; bpmn-runtime passes it through). The FEEL normaliser's job is strip wrappers + validate parseable subset + emit clean FEEL string verbatim. dmn-lite evaluation is a future external-peer concern.
- **§5.1** — API renamed `feel_normalize`; purpose is validate-and-strip, not translate.
- **§5.3** — Q1 reframed: `assembly.rs` already exists in `dsl-bpmn-frontend`; question is coupling strategy, not building from scratch.
- **§5.4** — Round-trip scope narrowed: validates structure + reachability only; condition executability explicitly out of scope.
- **§6** — Binary path corrected to `src/bin/migrate.rs`.
- **§7** — Q3 resolved (emit verbatim FEEL); Q4 resolved (`:condition` confirmed); Q5 rewritten against actual mapper behaviour.
- **§8** — `emitter.rs` added; binary path corrected; `:condition` used throughout.
- **§10** — Implementation order updated: s-expr translation step removed.
- **§11 NEW** — Form.io verb integration.

---

## 1. The Core Architectural Problem

Camunda 8 `.bpmn` files mix two structurally distinct concerns into a single XML document:

- **Process structure** — the shape of the workflow: start/end events, tasks, gateways, sequence flows, boundary events. The "what happens and in what order."
- **Execution rules** — the decision logic embedded in the flow: FEEL conditions on sequence flows, timer expressions, message correlation keys, DMN table references. The "under what conditions."

Our architecture keeps these concerns separate by design:

| Layer | Language | Concern | Location |
|-------|----------|---------|----------|
| Process structure | bpmn-lite DSL (s-expression) | Shape, topology, verb invocation | local crates |
| Decision logic | FEEL / dmn-lite | Guards, conditions, expression evaluation | **federated external peer** |

**dmn-lite is not a local crate.** The only dmn-lite references in the workspace are the external bus endpoint (`DMN_LITE_BUS_ENDPOINT` in `ob-poc-web`) and a manifest guard in `ob-poc-manifest-export` that explicitly states _ob-poc does not own DMN decisions_. The condition string passes through the local runtime as an opaque `Option<String>` on `RailwayEdge.condition` (confirmed: `dsl-bpmn-frontend/src/railway.rs:270`, comment "not yet evaluated") and through `dsl-lowering` into `bpmn-runtime/src/switch.rs:17` (`EdgeInfo.condition`). The only `SwitchAdaptor` present (`ScriptedAdaptor`) ignores the condition string entirely.

**Consequence for the transpiler:** FEEL conditions should be emitted **verbatim** as the `:condition` string on flow atoms. The normaliser's job is to strip Camunda-specific wrappers and classify what it cannot parse — not to translate to a local s-expression format. Translation is the external peer's concern.

---

## 2. What FEEL Is (and Is Not) in This Context

FEEL (Friendly Enough Expression Language) is the DMN standard expression language. It appears in Camunda 8 BPMN files in two forms:

**Form A — Juel (Camunda 7 legacy, still valid in C8):**
```xml
<conditionExpression>${score >= 700}</conditionExpression>
```
The `${...}` wrapper is Juel syntax. Strip wrapper; inner expression is the FEEL string to emit.

**Form B — Native FEEL (Camunda 8 preferred):**
```xml
<conditionExpression>= score >= 700</conditionExpression>
```
The leading `=` is a FEEL unary test marker. Strip it; emit `score >= 700`.

**What FEEL is NOT in a BPMN file:**
- DMN decision table cell expressions — those live in `.dmn` files, not `.bpmn`
- Full FEEL programs — BPMN conditions are always single expressions

**The FEEL subset that appears in BPMN sequence flow conditions:**

| Pattern | Example | Normaliser action |
|---------|---------|------------------|
| Comparison | `score >= 700`, `status = "ACTIVE"` | Emit verbatim |
| Logical and/or | `score > 500 and risk = "LOW"` | Emit verbatim |
| Negation | `not(approved)`, `status != "DECLINED"` | Emit verbatim |
| Arithmetic | `amount * rate > threshold` | Emit verbatim |
| String membership | `status in ["PENDING","REVIEW"]` | Emit verbatim |
| Null check | `entity != null` | Emit verbatim |
| Juel wrapper | `${...}` around any of the above | Strip `${` `}`, emit inner |
| FEEL unary test | `= expr` | Strip leading `=`, emit expr |

**Out-of-scope patterns (emit `[HUMAN-RESOLVE]` with diagnostic):**
- Context dot-access: `order.amount`
- Date functions: `date("2026-01-01")`
- Built-in functions: `string length(name) > 5`
- For/some/every quantifiers

**Timer expressions** use ISO 8601 (`PT5M`, `R3/PT1H`) — not FEEL; mapped directly to a duration literal.

---

## 3. The Five-Stage Pipeline

```
Camunda 8 .bpmn (XML)
         │
         ▼
┌─────────────────────────────────────────────┐
│  Stage 1: XML Parse             [EXISTS]     │
│                                              │
│  quick-xml → BpmnProcess IR                  │
│  • elements: Vec<BpmnElement>                │
│  • sequence_flows: Vec<SequenceFlow>         │
│    ↳ each flow may carry condition_expression│
│  Location: xml_reader.rs                     │
└──────────────────┬──────────────────────────┘
                   │
          ┌────────┴────────┐
          ▼                 ▼
┌──────────────────────┐  ┌─────────────────────────┐
│ Stage 2:             │  │ Stage 3:       [NEW]     │
│ Structural Map       │  │ FEEL Normaliser          │
│ [EXISTS partial]     │  │                          │
│                      │  │ Input: condition_expr    │
│ BpmnElement →        │  │ strings from flows       │
│ bpmn-lite atoms:     │  │                          │
│                      │  │ 1. Strip Juel ${...}     │
│ (node id :kind       │  │ 2. Strip FEEL = prefix   │
│   start-event)       │  │ 3. Classify: parseable   │
│ (node id :kind       │  │    subset or HUMAN-      │
│   service-task       │  │    RESOLVE with diag     │
│   :verb invoke-fqn)  │  │ 4. Emit clean FEEL       │
│ (gateway id :kind    │  │    string verbatim as    │
│   exclusive)         │  │    :condition value      │
│ (flow a -> b)        │  │                          │
│ (flow a -> b         │  │ Does NOT translate to    │
│   :condition "...")  │  │ s-expressions.           │
│                      │  │ Location: feel_parser.rs │
│ mapper.rs            │  │ (new module)             │
│ (replace TODO with   │  │                          │
│  normalised FEEL)    │  │                          │
└──────────┬───────────┘  └────────────┬────────────┘
           │                           │
           └─────────────┬─────────────┘
                         ▼
┌─────────────────────────────────────────────┐
│  Stage 4: Assembly              [NEW]        │
│                                              │
│  Merges mapper output into single string     │
│  suitable for dsl-parser::parse_program.     │
│                                              │
│  SequenceFlow with normalised condition:     │
│    (flow gw-score -> end-approved            │
│      :condition "score >= 700")             │
│                                              │
│  NOTE: assembly.rs already exists in        │
│  dsl-bpmn-frontend (RawAtom → RailwayGraph).│
│  This stage is string concatenation only —  │
│  dsl-parser call lives in Stage 5.          │
│  No new assembly.rs needed in dsl-migrate.  │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│  Stage 5: Round-trip Validation  [NEW]       │
│                                              │
│  Assembled DSL string                        │
│    → dsl-resolution::validate_bpmn           │
│    → dsl-lowering::lower → JourneySpec       │
│    → RuntimeEngine::start (ScriptedAdaptor) │
│                                              │
│  Validates: structure + reachability only.  │
│  ScriptedAdaptor IGNORES :condition strings.│
│  Does NOT prove condition executability —   │
│  that requires the external dmn-lite peer.  │
│                                              │
│  Lives in: dsl-migrate-verify crate (new)   │
│  or bpmn-test-harness extension.            │
│  dsl-migrate stays zero intra-workspace deps│
└─────────────────────────────────────────────┘
```

---

## 4. FEEL Normaliser Contract

The normaliser (Stage 3) is **not a translator**. It is a classifier and wrapper-stripper.

### 4.1 Normaliser API

```rust
pub enum FeelNormaliseResult {
    /// Clean FEEL expression, ready to emit as :condition string.
    Clean(String),
    /// Contains constructs outside the supported subset. Emit [HUMAN-RESOLVE].
    NeedsReview { stripped: String, reason: String },
}

/// Normalise a raw conditionExpression string from Camunda 8 XML.
///
/// - Strips Juel ${...} wrappers.
/// - Strips FEEL unary-test leading `=`.
/// - Classifies the inner expression against the supported subset.
/// - Returns Clean(expr) or NeedsReview with a diagnostic.
pub fn feel_normalise(raw: &str) -> FeelNormaliseResult
```

### 4.2 Stripping rules

| Input form | Strip rule | Example in → out |
|------------|-----------|-----------------|
| `${expr}` | Remove `${` and `}` | `${score >= 700}` → `score >= 700` |
| `= expr` | Remove leading `= ` | `= score >= 700` → `score >= 700` |
| `expr` | No-op | `score >= 700` → `score >= 700` |

### 4.3 Classification — supported subset (emit Clean)

Parseable with a simple recursive-descent pass:

```
expr       = logical
logical    = comparison (("and" | "or") comparison)*
comparison = arithmetic (op arithmetic)?
op         = ">=" | "<=" | "!=" | "=" | ">" | "<"
arithmetic = term (("+" | "-") term)*
term       = unary (("*" | "/") unary)*
unary      = "not" "(" expr ")" | primary
primary    = literal | identifier | "(" expr ")" | list
literal    = number | string | "true" | "false" | "null"
list       = "[" (literal ("," literal)*)? "]"
identifier = [a-zA-Z_][a-zA-Z0-9_-]*
```

### 4.4 Out-of-scope (emit NeedsReview with diagnostic)

- Dot-access context: `order.amount`
- Date/time functions: `date(...)`, `duration(...)`
- Built-in string/numeric functions
- Quantifiers: `for`, `some`, `every`
- Multi-valued contexts and list comprehension

---

## 5. New Components — Scope and Location

### 5.1 `feel_parser.rs` — new module in `dsl-migrate`

Implements `feel_normalise` (§4.1). No crate deps beyond `thiserror`. Pure string-in, string-out or diagnostic-out.

### 5.2 Updated `mapper.rs`

Replace the current `map_sequence_flow_with_status` FEEL path:

```rust
// Current (emits placeholder):
"; [HUMAN-RESOLVE] FEEL condition: {}\n(flow {} -> {} :condition \"TODO\")"

// After:
match feel_parser::feel_normalise(cond) {
    FeelNormaliseResult::Clean(expr) =>
        format!("(flow {} -> {} :condition \"{}\")", src, tgt, expr),
    FeelNormaliseResult::NeedsReview { stripped, reason } =>
        format!("; [HUMAN-RESOLVE] {}\n(flow {} -> {} :condition \"{}\")",
                reason, src, tgt, stripped),
}
```

The test `feel_expressions_become_human_resolve` must be updated — after this change, `${score >= 700}` normalises to Clean and no longer produces a HUMAN-RESOLVE marker.

### 5.3 Round-trip verifier — `dsl-migrate-verify` crate (new) **or** `bpmn-test-harness` extension

**Q1 decision (see §7):** `dsl-migrate` has zero intra-workspace deps today and should stay that way — it is a pure XML-in / DSL-string-out tool. The round-trip step (parse DSL → `validate_bpmn` → `lower` → `RuntimeEngine`) requires `dsl-resolution`, `dsl-lowering`, `bpmn-runtime` — which belong in a separate verifier crate rather than in `dsl-migrate` itself.

`dsl-migrate-verify` takes the string output of `dsl-migrate::emit()` and confirms structural + reachability validity only. It does **not** evaluate `:condition` strings.

### 5.4 CLI

Current binary: `src/bin/migrate.rs`

After v1, add optional `--verify` flag that calls `dsl-migrate-verify` if the crate is present. Keeps `dsl-migrate` itself dep-free; verification is opt-in at the binary level.

---

## 6. Open Questions for Peer Review

**Q1. Round-trip verifier location — `dsl-migrate-verify` crate or `bpmn-test-harness` extension?**  
Recommendation: new `dsl-migrate-verify` crate. It's a distinct capability (import verification) from general harness testing, and keeping it separate avoids pulling migration deps into the test harness. If the harness already has the right structure, extend it instead.

**Q2. `feel_parser.rs` — own crate or module in `dsl-migrate`?**  
Recommendation: module-first. Promote to `feel-parser` crate only when a second consumer appears (e.g. DMN table importer). A standalone crate for ~200 lines is premature.

**Q3. dmn-lite evaluation of `:condition` strings — RESOLVED**  
FEEL is emitted verbatim. dmn-lite (external peer) owns evaluation. No local translation. The condition is opaque to the bpmn-lite runtime until the external peer is wired.

**Q4. `:condition` keyword — RESOLVED**  
Confirmed: `dsl-parser/src/parser.rs:818` test and `dsl-bpmn-frontend/src/assembly.rs:800` both use `"condition"`. The attachment point exists and works. No new keyword needed.

**Q5. Camunda 8 user tasks — rebaselined**  
Current mapper behaviour (verified in `mapper.rs:280`): `bpmn:userTask` → `(node id :kind user-task)`. No assignee handling, no `[HUMAN-RESOLVE]`. The output is structurally valid DSL. Question for peer: is `(node id :kind user-task)` a sufficient v1 target, or does the runtime need a different node kind? User tasks with `formKey` will gain a richer target in §11 (Form.io verb), but that is additive — it does not block v1.

**Q6. Verb resolver — scope for real Camunda workflows**  
28-entry table covers ob-poc domains. Unknown `camunda:topic` values → `[HUMAN-RESOLVE]`. Is the expectation that this table grows per-deployment, or is HUMAN-RESOLVE the permanent strategy for unknowns?

---

## 7. What Is Already Built (Current `dsl-migrate` State)

| Component | Status | Location |
|-----------|--------|----------|
| XML parser (BpmnProcess IR) | ✅ complete | `xml_reader.rs` |
| Structural mapper | ✅ complete | `mapper.rs` |
| Sequence flow emitter (conditions → TODO placeholder) | ✅ partial | `mapper.rs` |
| Verb resolver (28 entries) | ✅ complete | `verb_resolver.rs` |
| Coverage reporter | ✅ complete | `reporter.rs` |
| Emitter (orchestrates map + report) | ✅ complete | `emitter.rs` |
| CLI binary | ✅ complete | `src/bin/migrate.rs` |
| 5 corpus BPMN fixtures | ✅ complete | `tests/corpus/` |
| 9 integration tests | ✅ passing | `tests/migration_tests.rs` |
| FEEL normaliser | ❌ missing | — |
| Conditions → clean `:condition` string | ❌ missing (currently `"TODO"`) | — |
| Round-trip structural verifier | ❌ missing | — |
| Form.io verb (`dsl.form`) | ❌ missing | — |

---

## 8. Non-Goals for v1

- **FEEL evaluation** — the runtime does not evaluate `:condition` strings; that is the external dmn-lite peer's responsibility.
- **DMN table import** — `.dmn` files are a separate pipeline. Out of scope.
- **Zeebe-specific extensions** — `zeebe:calledElement`, input/output variable mappings, task headers.
- **Sub-process flattening** — IR parses them; mapping strategy undefined.
- **Migration of running instances** — source-to-source transpiler for definitions only.

---

## 9. Proposed Implementation Order

1. `feel_parser.rs` — `feel_normalise` + unit tests for all patterns in §4.2–§4.4
2. Update `mapper.rs` — replace `"TODO"` condition path with `feel_normalise`
3. Update `feel_expressions_become_human_resolve` test — now expects Clean output for `${score >= 700}`
4. Expand corpus — `feel_conditions_complex.bpmn` covering out-of-scope patterns (verifies HUMAN-RESOLVE still fires for those)
5. **Resolve Q1** — decide verifier crate vs harness extension
6. `dsl-migrate-verify` (or harness extension) — structural + reachability round-trip
7. Add `--verify` flag to `src/bin/migrate.rs`
8. Form.io verb — see §10

---

## 10. Form.io Verb Integration

### 10.1 Architectural position

Form.io is a callout from a running BPMN execution surfaced as a standard SemOS verb `dsl.form`. It is **not** a separate bridge crate — and it should not become one.

dmn-lite is external because ob-poc explicitly disclaims decision authority over DMN (confirmed: manifest guard in `ob-poc-manifest-export/src/lib.rs`). Form.io owns nothing in that sense; it is a UI mechanism inside an ob-poc-owned process. A `form-bridge` crate would repeat the v0.6 `assembly.rs` mistake — new structure sitting next to machinery that already does the job.

**The park/resume machine already exists.** `bpmn-runtime/src/verb.rs:6-7`: when a verb has no registered handler, the token parks via `create_pending_wait` (confirmed: `processor.rs:748`) and resumes on `EventKind::VerbCompletion`. For `dsl.form`, the verb IS registered — it runs, emits a `VerbEffect::RequestHumanTask`, and the processor creates a `pending_wait` of kind `"human_task"`. Resume fires via `EventKind::HumanTaskComplete` (confirmed: `types.rs:101`, already in the `EventKind` enum).

**Critical nuance:** `VerbEffect::RequestHumanTask` and `EventKind::HumanTaskComplete` are **defined but unhandled**. The type scaffolding is correct; the dispatch arms in `processor.rs` are missing. That is the work — ~20–30 lines completing existing scaffolding, not new machinery.

**Flow:**
1. Runtime reaches `dsl.form` node → calls registered verb handler
2. Handler fetches Form.io schema, returns `VerbOutput { effects: [RequestHumanTask { role, form_data }] }`
3. Processor handles `RequestHumanTask` (new arm needed): creates `pending_wait("human_task", token_id, correlation_key)`
4. Session response carries form schema to UI; cockpit renders it via Form.io SDK
5. User interacts (display-only or capture)
6. Submission delivers `HumanTaskComplete` event (new handler arm needed): resumes fiber with form data in process context

### 10.2 Two interaction modes

| Mode | Purpose | Returns |
|------|---------|---------|
| `display` | Show process state / summary; single Continue | Ack only — no data captured |
| `capture` | Collect user input (fields, selections, buttons) | Form submission as named variables |

Both are carried in `form_data: serde_json::Value` on `RequestHumanTask`. The field is already `serde_json::Value` — flexible enough to hold `{ "form_ref": "...", "mode": "capture", "context": {...} }` without structural change.

### 10.3 DSL representation

```
(node "review-kyc-summary" :kind user-task
  :verb dsl.form
  :form-ref "kyc.review-summary"
  :mode display
  :context #{kyc_result entity_name risk_score})

(node "collect-missing-docs" :kind user-task
  :verb dsl.form
  :form-ref "onboarding.document-checklist"
  :mode capture
  :output-binding doc_submissions)
```

Note `:kind user-task` — see §10.4.

### 10.4 Camunda 8 transpiler mapping

Camunda `bpmn:userTask` with a `formKey` attribute → `(node id :kind user-task :verb dsl.form :form-ref <normalised-ref>)`.

**Reclassification is behaviourally inert.** Confirmed: `processor.rs:919–930`, `is_task_kind()` routes all task kinds — `service-task`, `user-task`, `send-task`, etc. — identically through one match arm. Verb presence, not `:kind`, drives execution. Flipping `:kind` from `user-task` to `service-task` changes nothing at runtime. Recommendation: **keep `:kind user-task`** and add `:verb dsl.form` — same execution, truer to the Camunda source. Reviewer may override.

**`formKey` normalisation — mapping table:**

| `formKey` prefix | Rule | `:form-ref` output |
|-----------------|------|-------------------|
| `camunda-forms:embedded:` | Strip prefix | `embedded/<rest>` |
| `deployment:` | Strip prefix | `deployment/<rest>` |
| Plain key (no prefix, no `:`) | Pass through | `<key>` |
| `classpath:`, `bpmn:`, other prefixed | Cannot normalise | `[HUMAN-RESOLVE]` |
| Absent / empty | No form | Emit plain `(node id :kind user-task)` |

Unknown prefixes follow the same "normalise or defer" shape as the verb resolver.

Tasks without `formKey` continue to emit `(node id :kind user-task)` — Q5 unchanged.

### 10.5 What is genuinely new (small list)

The `behavior: plugin` verb-registry entry follows the standard pattern used by 10+ existing verbs (e.g. `config/verbs/screening.yaml:44`). Nothing structurally novel there.

| Component | What's new |
|-----------|-----------|
| `dsl.form` verb YAML | Conventional `behavior: plugin` entry. Args: `form_ref`, `mode`, `context`, `output_binding`. |
| `DslFormOp` — `SemOsVerbOp` impl | `fetch_schema(form_ref)` call + returns `VerbOutput { effects: [RequestHumanTask {...}] }`. |
| `RequestHumanTask` dispatch arm | ~15 lines in `processor.rs`: handle effect, create `pending_wait("human_task", ...)`. |
| `HumanTaskComplete` handler arm | ~15 lines in `processor.rs`: find pending_wait by correlation, resume fiber with payload. |
| Form.io schema store | Stores/retrieves form JSON by ref. Can be Form.io cloud, self-hosted, or local JSON files behind an ob-poc-web endpoint. |
| UI form renderer | React component receiving form schema in session response, renders via Form.io JS SDK. Lives in `ob-poc-ui-react/` — not a Rust concern. |

### 10.6 Open questions

**Q7. Form.io hosting** — cloud vs self-hosted vs local JSON files? The `fetch_schema` impl needs a concrete target. Local JSON files behind `GET /api/forms/:ref` in ob-poc-web is the simplest start.

**Q8. Correlation key for pending_wait** — the `create_pending_wait` call (confirmed: `processor.rs:748`) takes a correlation kind and value. For `"human_task"` waits, what uniquely identifies a form instance so the `HumanTaskComplete` event routes to the right token? Options: `token_id` (simpler, one form per token), or a generated `form_submission_id` (allows tracking). Reviewer to decide.

**Q9. formKey normalisation edge cases** — Camunda 8 `camunda-forms:embedded:` forms embed the full JSON schema inline in the BPMN XML rather than referencing an external key. If the transpiler encounters an embedded schema, should it extract and store it, or always defer to HUMAN-RESOLVE? Scope question for the transpiler, not the runtime.
