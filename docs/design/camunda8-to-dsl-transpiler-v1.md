# Camunda 8 → bpmn-lite DSL Transpiler — Design v1

> **Status:** Draft for peer review — no implementation yet  
> **Date:** 2026-05-26  
> **Scope:** `dsl-migrate` crate (ob-poc) + FEEL parser (new) + round-trip harness

---

## 1. The Core Architectural Problem

Camunda 8 `.bpmn` files mix two structurally distinct concerns into a single XML document:

- **Process structure** — the shape of the workflow: start events, tasks, gateways, sequence flows, boundary events, end events. This is the "what happens and in what order."
- **Execution rules** — the decision logic embedded in the flow: FEEL conditions on sequence flows, timer expressions, message correlation keys, DMN table references. This is the "under what conditions."

Our architecture keeps these concerns separate by design:

| Layer | Language | Concern |
|-------|----------|---------|
| Process structure | bpmn-lite DSL (s-expression) | Shape, topology, verb invocation |
| Decision logic | dmn-lite DSL (s-expression) | Guards, conditions, expression evaluation |

The transpiler must respect this boundary. When consuming a Camunda 8 source file, it must **split** the file along this seam rather than treating either half as a black box. The current `dsl-migrate` implementation partially does Stage 1 (XML parse) and Stage 2 (structural mapping) but stalls at FEEL conditions — emitting `[HUMAN-RESOLVE]` markers instead of parsing them. That is the gap.

---

## 2. What FEEL Is (and Is Not) in This Context

FEEL (Friendly Enough Expression Language) is the DMN standard expression language. It appears in Camunda 8 BPMN files in two forms:

**Form A — Juel (Camunda 7 legacy, still valid in C8):**
```xml
<conditionExpression>${score >= 700}</conditionExpression>
```
The `${...}` wrapper is Juel syntax. The inner expression is Java EL, not pure FEEL.

**Form B — Native FEEL (Camunda 8 preferred):**
```xml
<conditionExpression>= score >= 700</conditionExpression>
```
The leading `=` is a FEEL unary test. `score >= 700` is a FEEL expression.

**What FEEL is NOT in a BPMN file:**
- DMN decision table cell expressions — those live in `.dmn` files, not `.bpmn`
- Full FEEL programs — BPMN conditions are always single expressions, never multi-statement

**The FEEL subset that appears in BPMN sequence flow conditions:**

| Pattern | Example | Frequency |
|---------|---------|-----------|
| Comparison | `score >= 700`, `status = "ACTIVE"` | Very common |
| Logical and/or | `score > 500 and risk = "LOW"` | Common |
| Negation | `not(approved)`, `status != "DECLINED"` | Common |
| Arithmetic | `amount * rate > threshold` | Occasional |
| String membership | `status in ["PENDING","REVIEW"]` | Occasional |
| Null check | `entity != null` | Occasional |
| Juel wrapper | `${...}` around any of the above | Legacy common |

**Out of scope for BPMN conditions (full FEEL features, rare or absent in flow guards):**
- `for` / `some` / `every` quantifiers
- Date arithmetic beyond ISO duration literals
- Context and list comprehension
- Function definitions

**Timer expressions** use ISO 8601 (`PT5M`, `R3/PT1H`, `2026-01-01T00:00:00Z`) — this is **not FEEL** and maps directly to a duration/date literal in the DSL without a FEEL parser.

---

## 3. The Five-Stage Pipeline

```
Camunda 8 .bpmn (XML)
         │
         ▼
┌─────────────────────────────────────────────┐
│  Stage 1: XML Parse          [EXISTS]        │
│                                              │
│  quick-xml → BpmnProcess IR                  │
│  Outputs:                                    │
│  • elements: Vec<BpmnElement>                │
│  • sequence_flows: Vec<SequenceFlow>         │
│    ↳ each flow may carry condition_expression│
│                                              │
│  Location: xml_reader.rs                     │
└──────────────────┬──────────────────────────┘
                   │
          ┌────────┴────────┐
          ▼                 ▼
┌──────────────────┐  ┌──────────────────────────┐
│ Stage 2:         │  │ Stage 3:        [NEW]     │
│ Structural Map   │  │ FEEL Parser               │
│ [EXISTS partial] │  │                           │
│                  │  │ Input:                    │
│ BpmnElement →   │  │   condition_expression    │
│ bpmn-lite atoms │  │   strings from flows      │
│                  │  │                           │
│ • (start ...)    │  │ Strips ${ } Juel wrapper  │
│ • (end ...)      │  │ Strips leading = FEEL     │
│ • (task ...)     │  │ Parses expression into    │
│ • (gateway ...)  │  │ dmn-lite s-expr string    │
│ • (boundary ...) │  │                           │
│                  │  │ `score >= 700`            │
│ Location:        │  │ → `(>= score 700)`        │
│ mapper.rs        │  │                           │
│ (remove          │  │ Location: feel_parser.rs  │
│  HUMAN-RESOLVE)  │  │ (new module)              │
└────────┬─────────┘  └─────────────┬────────────┘
         │                          │
         └─────────────┬────────────┘
                       ▼
┌─────────────────────────────────────────────┐
│  Stage 4: Assembly              [NEW]        │
│                                              │
│  Merges structural atoms + guard             │
│  expressions into a single SourceFile        │
│                                              │
│  SequenceFlow with condition:                │
│    structural atom:  (-> a b)               │
│    guard expression: (>= score 700)         │
│    assembled:        (-> a b :when          │
│                        (>= score 700))      │
│                                              │
│  Output: SourceFile { atoms: Vec<RawAtom> } │
│  (the dsl-parser's canonical input type)    │
│                                              │
│  Location: assembly.rs (new module)         │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│  Stage 5: Round-trip Validation  [NEW]       │
│                                              │
│  SourceFile                                  │
│    → dsl-resolution::validate_bpmn           │
│    → dsl-lowering::lower → JourneySpec       │
│    → RuntimeEngine::start (stub executor)   │
│    → process advances to expected terminal  │
│                                              │
│  Failure here means the emitted DSL is       │
│  syntactically valid but not executable —    │
│  a category of bug string-match tests miss. │
│                                              │
│  Location: roundtrip.rs (new module) or      │
│  extended bpmn-test-harness                  │
└─────────────────────────────────────────────┘
```

---

## 4. FEEL → dmn-lite Expression Mapping

The FEEL parser (Stage 3) produces dmn-lite s-expression strings. These are then embedded as `:when` guards on sequence flow atoms.

### 4.1 Expression translation table

| FEEL | dmn-lite s-expr | Notes |
|------|-----------------|-------|
| `score >= 700` | `(>= score 700)` | |
| `status = "ACTIVE"` | `(= status "ACTIVE")` | |
| `amount != 0` | `(!= amount 0)` | |
| `a > 1 and b < 5` | `(and (> a 1) (< b 5))` | |
| `a > 1 or b < 5` | `(or (> a 1) (< b 5))` | |
| `not(approved)` | `(not approved)` | |
| `not(a = "X")` | `(not (= a "X"))` | |
| `amount * rate > threshold` | `(> (* amount rate) threshold)` | |
| `status in ["PENDING","REVIEW"]` | `(in status ["PENDING" "REVIEW"])` | |
| `entity != null` | `(not-null entity)` | |
| `= score >= 700` | strip leading `=`, then parse | FEEL unary test prefix |
| `${score >= 700}` | strip `${` `}`, then parse | Juel wrapper |

### 4.2 Parser grammar (subset required)

```
expr      = logical
logical   = comparison (("and" | "or") comparison)*
comparison = arithmetic (op arithmetic)?
op        = ">=" | "<=" | "!=" | "=" | ">" | "<"
arithmetic = unary (("+" | "-" | "*" | "/") unary)*
unary     = "not" "(" expr ")" | primary
primary   = literal | identifier | "(" expr ")" | list | null-check
literal   = number | string | "true" | "false" | "null"
list      = "[" (literal ("," literal)*)? "]"
identifier = [a-zA-Z_][a-zA-Z0-9_-]*
```

This is a hand-written recursive-descent parser (same pattern as dsl-parser). No external parser combinator needed for this scope.

### 4.3 Unresolvable FEEL (still `[HUMAN-RESOLVE]`)

Some FEEL patterns in the wild are out of scope for v1:
- Context access: `order.amount` (dot notation for nested context)
- Date functions: `date("2026-01-01")`  
- Built-in functions: `string length(name) > 5`
- For/some/every quantifiers

These should still emit `[HUMAN-RESOLVE]` with a diagnostic noting the specific construct that wasn't parsed, not a blanket failure.

---

## 5. New Components — Scope and Location

### 5.1 `feel_parser.rs` — new module in `dsl-migrate`

```rust
/// Parse a FEEL condition expression (as found in Camunda 8 sequenceFlow
/// conditionExpression) into a dmn-lite s-expression string.
///
/// Handles:
///   - Juel wrapper stripping: `${...}` → inner
///   - FEEL unary test prefix: `= expr` → `expr`
///   - Full expression parse → s-expr string
///
/// Returns `Err(FeelParseError)` for out-of-scope constructs.
/// Caller emits [HUMAN-RESOLVE] on error.
pub fn feel_to_dmn_lite(feel: &str) -> Result<String, FeelParseError>
```

**Does not** depend on dmn-lite crate directly — produces a string that is valid dmn-lite syntax. Keeping it string-output avoids a new crate dependency and keeps the parser self-contained.

### 5.2 Updated `mapper.rs`

Remove the `is_feel_expression` + `[HUMAN-RESOLVE]` path for conditions. Replace with a call to `feel_parser::feel_to_dmn_lite(condition)`. On `Ok` → embed in `:when` clause. On `Err` → `[HUMAN-RESOLVE]` with the specific parse error as diagnostic.

### 5.3 `assembly.rs` — new module in `dsl-migrate`

Takes the `Vec<String>` atom lines from `mapper.rs` and produces a `dsl_parser::SourceFile`. This is the bridge from the migration string representation into the canonical dsl-parser AST, enabling the round-trip.

**Dependency note:** this module adds `dsl-parser` as a dep of `dsl-migrate`. Currently `dsl-migrate` has no intra-workspace deps (pure XML-in, string-out). This is the first structural coupling — needs a decision (§7 open questions).

### 5.4 `roundtrip.rs` — new module in `dsl-migrate` or `bpmn-test-harness`

```rust
pub struct RoundTripResult {
    pub source_file: SourceFile,
    pub validation: ValidateResponse,
    pub journey_spec: Option<JourneySpec>,
    pub execution_trace: Option<Vec<String>>,  // token path through the graph
}

pub fn validate_and_execute(dsl_source: &str, process_name: &str) -> RoundTripResult
```

Runs: `validate_bpmn` → `lower` → `RuntimeEngine::start` with a stub `VerbInvoker` that records invocations and always returns success. Proves the emitted DSL is executable, not just syntactically valid.

---

## 6. What the CLI Becomes

Current:
```
dsl-migrate input.bpmn output.dsl [--report report.json]
```

After v1:
```
dsl-migrate input.bpmn output.dsl [--report report.json] [--roundtrip]
```

`--roundtrip` runs Stage 5 and appends execution trace to the report. Exit 0 = clean migration and executable. Exit 2 = human-resolve items remain. Exit 3 = DSL emitted but round-trip failed (structural issue in emitted code).

---

## 7. Open Questions for Peer Review

**Q1. assembly.rs coupling — where does it live?**  
Adding `dsl-parser` as a dep of `dsl-migrate` couples the migration tool to the DSL compiler chain. Alternative: keep `dsl-migrate` string-only (no dsl-parser dep) and put `assembly.rs` + `roundtrip.rs` in a new `dsl-migrate-verify` crate or in `bpmn-test-harness`. Tradeoff: cleaner separation vs more crates.

**Q2. feel_parser — own crate or module?**  
The FEEL parser is conceptually independent — it could serve other consumers (e.g. a future DMN table importer). Case for own crate: `feel-parser` with no deps beyond `thiserror`. Case for module: simpler, less overhead until there's a second consumer. Recommend module-first, promote to crate if DMN import arrives.

**Q3. dmn-lite s-expression format — needs verification**  
The mapping table in §4.1 assumes a dmn-lite s-expression syntax of `(>= score 700)`. This needs to be verified against the actual dmn-lite parser input format. If dmn-lite uses different operator names or syntax, the translation table changes. **Reviewer action: confirm dmn-lite expression syntax.**

**Q4. `:when` clause on flow atoms — is this the right attachment point?**  
The current bpmn-lite DSL has `(-> source target)` for sequence flows. The proposal is `(-> source target :when (dmn-lite-expr))`. Is `:when` already a supported keyword in the dsl-parser/dsl-bpmn-frontend? If not, this needs to be added there before Stage 4 can produce valid output. **Reviewer action: confirm or specify the guard syntax.**

**Q5. Camunda 8 user tasks — DSL target?**  
User tasks (`bpmn:userTask`) have no direct equivalent in bpmn-lite (which is verb-driven, no human task concept). Current behaviour: mapped to a generic `(task id :kind user-task)` atom with `[HUMAN-RESOLVE]` on the assignee. Is this acceptable for v1, or should user tasks be rejected outright?

**Q6. Scope of verb resolver — sufficient for real Camunda workflows?**  
The current 28-entry mapping table covers ob-poc's own domain verbs. A real Camunda 8 workflow from a customer will have worker topics we don't know. The resolver currently returns `None` → `[HUMAN-RESOLVE]`. This is correct but needs a clear path: is the verb table expected to grow per-customer, or is `[HUMAN-RESOLVE]` the permanent answer for unknown topics?

---

## 8. What Is Already Built (Current `dsl-migrate` State)

| Component | Status | Location |
|-----------|--------|----------|
| XML parser (BpmnProcess IR) | ✅ complete | `xml_reader.rs` |
| Structural mapper (no conditions) | ✅ complete | `mapper.rs` |
| Verb resolver (28 entries) | ✅ complete | `verb_resolver.rs` |
| Coverage reporter | ✅ complete | `reporter.rs` |
| CLI binary | ✅ complete | `bin/dsl_migrate.rs` |
| 5 corpus BPMN fixtures | ✅ complete | `tests/corpus/` |
| 9 integration tests | ✅ passing | `tests/migration_tests.rs` |
| FEEL parser | ❌ missing | — |
| Condition → `:when` assembly | ❌ missing | — |
| SourceFile assembly (dsl-parser bridge) | ❌ missing | — |
| Round-trip validation + execution | ❌ missing | — |

---

## 9. Non-Goals for v1

- **DMN table import** — `.dmn` files are a separate input format and a separate pipeline. Out of scope here; this doc covers FEEL expressions embedded in BPMN flow conditions only.
- **Zeebe-specific extensions** — `zeebe:calledElement`, `zeebe:input`/`zeebe:output` variable mappings, `zeebe:taskHeaders` — out of scope for v1.
- **Sub-process flattening** — collapsed sub-processes in Camunda export. The IR already parses them; mapping strategy is not defined.
- **Migration of running instances** — this is a source-to-source transpiler for workflow definitions, not instance migration.
- **FEEL optimisation** — no constant folding, no static evaluation of literals. Emit the expression faithfully; let the dmn-lite VM evaluate at runtime.

---

## 10. Proposed Implementation Order

If approved after peer review:

1. `feel_parser.rs` — parser + unit tests for all patterns in §4.1 + known-unresolvable cases
2. Update `mapper.rs` — replace `[HUMAN-RESOLVE]` condition path with `feel_parser::feel_to_dmn_lite`
3. Expand corpus — add a `feel_conditions_complex.bpmn` fixture covering all patterns in §4.1
4. Resolve Q3 + Q4 (dmn-lite syntax + `:when` keyword) before proceeding to Stage 4
5. `assembly.rs` — SourceFile bridge (depends on Q1 decision)
6. `roundtrip.rs` — validation + execution harness (depends on Q1 + Q4)
7. Update CLI with `--roundtrip` flag
