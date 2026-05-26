# Camunda 8 → bpmn-lite DSL Transpiler — Design v0.6

> **Status:** Draft for peer review — no implementation yet  
> **Date:** 2026-05-26  
> **Scope:** `dsl-migrate` crate (ob-poc) + FEEL normaliser (new) + round-trip harness + Form.io verb

---

## Changelog (v0.7 → v0.8)

- **§10** — explicit Rust/JS process boundary diagram added; form.io rendering + schema resolution confirmed JS-side only; Rust verb is form.io-agnostic (data + park). §10.5 split into Rust-side and JS-side component tables. Q8 resolved (token_id correlation, implemented in T3). Q10 added (schema authority — open for board, EOP governance connection).

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

### 10.1 Architectural position and process boundary

Form.io is a callout from a running BPMN execution surfaced as a standard SemOS verb `dsl.form`. It is **not** a separate bridge crate.

dmn-lite is external because ob-poc explicitly disclaims decision authority over DMN. Form.io owns nothing; it is a UI rendering mechanism inside an ob-poc-owned process. A `form-bridge` crate would invent structure next to machinery that already exists.

**Explicit Rust / JS boundary:**

```
Rust half (form.io-agnostic)          wire           JS half (form.io-specific)
─────────────────────────────        ──────         ─────────────────────────────
dsl.form verb:                      outbound:       FormioForm component:
  resolve :context slots →           {form_ref,       GET /api/forms/:ref → schema
    prefill_data                      prefill_data,    Formio.createForm(el, schema)
  emit RequestHumanTask               mode}            instance.submission = {data: prefill_data}
    {role, form_data}                                  user interacts
  park pending_wait                  inbound:        on submit:
  resume on HumanTaskComplete         {data: {...}}    POST /api/forms/:token_id/submit
    → inject submission data        (HumanTaskComplete  → ob-poc-web dispatches
      into process context            event)            HumanTaskComplete event
```

**The Rust verb never fetches a form schema, never renders, never references form.io at all.** It deals only in data objects. If the form engine is swapped, only the React side changes.

**Park/resume:** `RequestHumanTask` effect → `create_pending_wait("human_task", token_id)` → parks fiber. Resume via `HumanTaskComplete` event (already dispatches to `handle_verb_completion` at `processor.rs:34`). Both wired in T3.

**Flow:**
1. Runtime reaches `dsl.form` node → calls registered verb handler
2. Handler resolves context, returns `VerbOutput { effects: [RequestHumanTask { role, form_data: {form_ref, mode, prefill_data} }] }`
3. Processor parks fiber via `pending_wait("human_task", token_id)`
4. Session response carries `{form_ref, prefill_data, mode}` to cockpit
5. React side fetches schema, renders form, prefills, user submits
6. `POST /api/forms/:token_id/submit` → `HumanTaskComplete` → fiber resumes with submission data in context

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

The `behavior: plugin` verb-registry entry follows the standard pattern used by 10+ existing verbs (e.g. `config/verbs/screening.yaml:44`). Nothing structurally novel there.

**Rust side (form.io-agnostic):**

| Component | What's new |
|-----------|-----------|
| `dsl.form` verb YAML | Conventional `behavior: plugin` entry. Args: `form_ref`, `mode`, `context`, `output_binding`. |
| `DslFormOp` — `SemOsVerbOp` impl | Resolves `:context` slots → `prefill_data`; returns `VerbOutput { effects: [RequestHumanTask { role, form_data: {form_ref, mode, prefill_data} }] }`. No schema fetch. |
| `GET /api/forms/:ref` endpoint | Serves local JSON form schemas from `config/forms/`. Schema resolution happens JS-side via this endpoint. |
| `POST /api/forms/:token_id/submit` endpoint | Accepts form submission; dispatches `HumanTaskComplete` event into runtime; fiber resumes. |

**JS/React side (form.io-specific, ob-poc-ui-react/):**

| Component | What's new |
|-----------|-----------|
| `features/forms/FormioForm.tsx` | Fetches schema via `GET /api/forms/:ref`; `Formio.createForm(el, schema)`; `instance.submission = {data: prefill_data}`; on submit POSTs to `/api/forms/:token_id/submit`. |
| `api/forms.ts` | Typed API client for forms endpoints. |
| Session response wiring | Cockpit detects `form_pending` in session response, renders `<FormioForm />` inline. |
| Form.io SDK install | `@formio/react` or `formiojs` in package.json. |

### 10.6 Open questions

**Q7. Form.io hosting** — cloud vs self-hosted vs local JSON files? For v1: local JSON files at `rust/config/forms/*.json`, served by `GET /api/forms/:ref`. Form.io cloud / self-hosted Mongo can come later behind the same endpoint. The Rust verb is unaffected — it emits `form_ref` only.

**Q8. Correlation key — RESOLVED.** Correlation key = `token_id.to_string()` (T3 implementation). One form per token. `POST /api/forms/:token_id/submit` carries the token_id in the URL; ob-poc-web dispatches `HumanTaskComplete` with `token_id` in the event payload. `handle_verb_completion` reads `token_id` from the event directly.

**Q9. formKey normalisation edge cases** — Camunda 8 `camunda-forms:embedded:` forms embed the full JSON schema inline in the BPMN XML rather than referencing an external key. If the transpiler encounters an embedded schema, should it extract and store it, or always defer to HUMAN-RESOLVE? Scope question for the transpiler, not the runtime. V1 defers embedded schemas to HUMAN-RESOLVE.

**Q10. Schema authority (open for board — do not resolve without governance discussion).** Two options:

- **Option A — SemOS governs form schemas:** form schemas are versioned artefacts inside SemReg (like verb contracts, attribute defs). ob-poc stores and versions the JSON; `GET /api/forms/:ref` serves from SemReg. Submissions are tagged with the schema version they were captured against (audit trail). Full governance lifecycle.

- **Option B — Form.io portal+Mongo is the authority:** ob-poc holds only `form_ref` strings. Form.io's own revision system (each publish stored as a numbered revision) governs schema evolution. ob-poc is a consumer, not an owner.

This is the same "who owns this authority" boundary test that put dmn-lite outside the workspace. It bears directly on the EOP paper's data-governance-as-first-class stance: if form schemas govern what data a user can submit in a regulated workflow, they may be subject to the same change-control discipline as attribute definitions. That is not a small call — it determines whether form schemas get the full changeset/publish lifecycle or live outside the SemOS governance plane.
