# Code Review — Pass 3: DAG validator + predicate AST and parser

**Commit:** `2642126b "Implement SemOS DAG architecture phases"`
**Files in scope:**
- `rust/crates/dsl-core/src/config/dag_validator.rs` (2,324 LOC)
- `rust/crates/dsl-core/src/config/predicate/{mod,ast,parser}.rs` (572 LOC)
- `rust/crates/dsl-core/src/config/green_when_coverage.rs` (193 LOC)
- (tests) `dag_gate_metadata.rs`, `dag_validator_gate.rs`, `closure_lint.rs`, `eligibility_lint.rs`, `predicate_ast.rs`, `green_when_coverage.rs`, `phase2_acceptance.rs`
- (governance) `docs/governance/green-when-coverage-2026.md` and `.csv`

**LOC reviewed:** 3,089 (validator + predicate + coverage source)
**Build status:** `cargo check -p dsl-core` clean. `cargo test -p dsl-core` 359 passed / 0 failed / 7 ignored across 21 test binaries (incl. doc-tests). Targeted suites:
- `dag_validator_gate.rs` 11/11
- `dag_gate_metadata.rs` 11/11
- `closure_lint.rs` 2/2
- `eligibility_lint.rs` 2/2
- `predicate_ast.rs` 6/6
- `green_when_coverage.rs` 3/3
- `phase2_acceptance.rs` 1/1

**Reviewer:** Zed-Claude
**Date:** 2026-05-01

## Summary

12 findings: 0 MUST-FIX, 5 SHOULD-FIX, 4 CONSIDER, 3 NOTE. The validator surface is the strongest part of this commit. All Phase 1.5B / §10.5 requirements are present: closure lint catches universal-quantifier-over-unbounded violations; eligibility lint catches unknown entity kinds; the `+`-sigil parse-time guard fires for both DAG taxonomy YAMLs (via the typed `Slot::additive_*` fields plus `reject_additive_predicate_vector`) and constellation map YAMLs (via the raw-YAML inspector); the schema-coordination warning surface (D-011) is fully wired with a hardening primitive (`harden_schema_coordination_warnings`) plus a "known deferred" allowlist for the one currently-authored mismatch (`deal_lifecycle.yaml::deal`); `replaceable_by_shape: false` defaults correctly across all 11 currently-authored cross-workspace constraints, and on `PredicateBinding`. The green_when coverage manifest matches the governance MD/CSV artefacts (12 covered / 196 candidate / 184 missing) and is enforced as a hard test (drift in either direction breaks the build).

The substantive gaps are at the integration boundary: `validate_dags` and `validate_resolved_template_gate_metadata` are invoked only from `cargo x reconcile` and from tests — no production binary calls them at startup, so D-020-style "boot-time" validation is effectively a CI gate, not a runtime gate. The parser produces 7 of 9 AST variants (`Count` and `Obtained` are typed but unauthored — D-018 marked them forward-compatible). The closure lint matches by slot id rather than by predicate-binding entity kind, which is fine when the two coincide (the Lux pilot does) but admits a silent miss when they diverge. Two cross-pass corrections: P1-008 (the `+`-sigil schema acceptance) is partially obsolete given P3 here — the validator does reject; P1-005 is partially obsolete in that the `green_when` parsing arm of D-020 *is* implemented in `validate_green_when_predicates`, just not in `compute_version_hash`.

## Findings

### MUST-FIX

(none — see SHOULD-FIX P3-004 for the closest call)

### SHOULD-FIX

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P3-001 — Predicate parser produces 7 of 9 AST variants; `Count` and `Obtained` are unauthored

**File:** `rust/crates/dsl-core/src/config/predicate/parser.rs`, `rust/crates/dsl-core/src/config/predicate/ast.rs`
**Lines:** parser.rs:87–101 (top-level dispatch); ast.rs:7–59 (all 9 variants)
**Spec reference:** D-018 ("Predicate AST covers conjunction, existence, state membership, attribute comparisons, universal/negative/existential quantifiers, count predicates, and obtained-validity predicates"); §4.1 example uses `total ownership accounts for ≥ 95%` which would compile to `Count`.

**Observation:** the AST has 9 variants (`And`, `Exists`, `StateIn`, `AttrCmp`, `Every`, `NoneExists`, `AtLeastOne`, `Count`, `Obtained`). The parser dispatches to seven of them via prefix tokens (`every`, `at least one`, `no`, `<subject> exists`) and `parse_comparison`. There is no `count(...)` recogniser and no `obtained(...)` recogniser:

```
$ rg "Predicate::Count|Predicate::Obtained|count\\(|obtained\\(" crates/dsl-core/src/config/predicate/parser.rs
(no results)
```

`hydrator.rs::eval_predicate` and `dag_validator.rs::reject_unbounded_universal_quantifiers` both have arms for `Count` and `Obtained`; they are reachable only by direct `Predicate::Count {...}` construction, which no current code path performs (constructors don't appear outside test fixtures).

**Issue:** §4.1's worked CBU example references *"total ownership accounts for ≥ 95% of equity"*, which is a count/aggregate predicate. §4.2's UBO example uses `obtained(direct_owner.kyc.green_when ∈ {KYC_APPROVED})`-style prose that should compile to `Obtained`. Neither shape is currently authorable.

D-018 notes Count and Obtained "may be forward-compatible" — i.e., the typed surface is reserved without a parser yet. Current state matches that framing. The finding is that this gap is undocumented in the parser file and authors would have to read the AST source to discover it. A reviewer of an authored DAG who saw a parser failure on `count(...)` would not know whether it's a typo or an unimplemented feature.

**Disposition (suggested):** add a doc-comment on `parse_clause` listing which forms are supported and which are reserved (`count(...)`, `obtained(...)`); emit a clearer parse error when an author writes one of the reserved forms (e.g., `ParseError::ReservedForm { form: "count" }`) rather than the generic "expected qualified field" message.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P3-002 — Closure lint matches by slot id, not by predicate-binding entity kind

**File:** `rust/crates/dsl-core/src/config/dag_validator.rs`
**Lines:** 1329–1366 (`reject_unbounded_universal_quantifiers`); 512–520 (slot_closures map construction)
**Spec reference:** §10.8 acceptance #3 ("Closure lint operates against ResolvedTemplate and rejects predicates that universally quantify over `closed_unbounded` or `open` slots without aggregate-only forms"); I3.

**Observation:**

```rust
let slot_closures = template
    .slots
    .iter()
    .filter_map(|slot| slot.closure.clone().map(|closure| (slot.id.clone(), closure)))
    .collect::<HashMap<_, _>>();
// ...
Predicate::Every { set, condition } => {
    if let Some(closure @ (ClosureType::ClosedUnbounded | ClosureType::Open)) =
        slot_closures.get(&set.kind)  // <-- match by slot id
    { ... }
}
```

`set.kind` comes from the parsed predicate's set reference (e.g., `every investment_manager.state = ...` produces `set.kind = "investment_manager"`). The lint looks up `slot_closures` keyed by slot id. In the pilot, slot id and predicate-binding entity kind coincide (`investment_manager` is both a slot id and the entity kind authored in `predicate_bindings`). When they diverge, the lookup misses and the lint silently passes.

For example: if `cbu_dag.yaml` declares slot id `management_company` whose `predicate_bindings.entity = "manco"`, and an authored predicate quantifies as `every manco.state = APPROVED`, the lint tries `slot_closures.get("manco")` which is `None` (no slot with id `manco`), so even if `management_company.closure = ClosedUnbounded` the violation is missed.

**Issue:** the lint's correctness depends on a coincidence — slot id == predicate entity kind — that the spec does not require. §3.3 distinguishes `slot_id` (the resolved-template addressing key) from predicate `entity` (authored in predicate bindings) as separate concepts. The lint should consult the predicate-binding metadata to map entity kind → slot id, then look up closure on that slot.

**Disposition (suggested):** when constructing `slot_closures`, key by the predicate-binding entity kind (i.e., for each slot with a structured state machine, walk its `predicate_bindings` and insert each `binding.entity → slot.closure`). This makes the lookup robust to the slot-id-vs-entity-kind distinction. Add a fixture where the two diverge and assert the lint fires.

---

#### [SHOULD-FIX] [QUALITY] P3-003 — `validate_resolved_eligibility` silently skips when `known_entity_kinds` is empty; the default context allows this

**File:** `rust/crates/dsl-core/src/config/dag_validator.rs`
**Lines:** 1257–1279 (`validate_resolved_eligibility`); 443–446 (`DagValidationContext::default` produces empty `known_entity_kinds`)
**Spec reference:** D-005 (`EligibilityConstraint::EntityKinds` v1); §10.5 "lint catches its synthetic violation"; §10.8 acceptance #4 ("Eligibility lint catches synthetic constraint referencing unknown entity kind").

**Observation:**

```rust
fn validate_resolved_eligibility(
    location: &DagLocation,
    slot: &ResolvedSlot,
    context: &DagValidationContext,
    report: &mut DagValidationReport,
) {
    let Some(EligibilityConstraint::EntityKinds { entity_kinds }) = &slot.eligibility else {
        return;
    };
    if context.known_entity_kinds.is_empty() {
        return;  // <-- no validation if no taxonomy supplied
    }
    // ...
}
```

`DagValidationContext::default()` (which the closure_lint test uses, line 24) produces an empty `known_entity_kinds`. When the lint runs with the default context, eligibility validation is silently skipped. The eligibility_lint test deliberately supplies `["company", "cbu", "person"]`; phase2_acceptance supplies the same three kinds — neither covers the full entity taxonomy from `entity_taxonomy.yaml`.

**Issue:** there are three failure modes here:
1. A caller invoking `validate_resolved_template_gate_metadata` with the default context gets *no* eligibility validation. The function silently passes a slot whose `EligibilityConstraint::EntityKinds` lists nonexistent kinds.
2. A caller supplying a *partial* taxonomy gets false positives: any kind not in the supplied set surfaces as `EligibilityEntityKindUnknown`. Phase 2 acceptance test runs with only 3 kinds; if a real shape rule references a 4th valid-but-unsupplied kind, the test would fail.
3. The function provides no signal to the caller that "no taxonomy supplied" mode was used. The caller cannot distinguish "validation passed" from "validation silently skipped."

**Disposition (suggested):** either (a) make `known_entity_kinds` non-optional (require the caller to supply it), or (b) split `DagValidationContext` into `Strict { known_entity_kinds }` vs `Permissive` and fail loudly when neither is selected, or (c) load `entity_taxonomy.yaml` automatically when `known_entity_kinds` is empty (with explicit logging that the default taxonomy was loaded). Test `phase2_acceptance.rs` should pass the *full* taxonomy, not a 3-kind subset.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P3-004 — `validate_dags` is invoked only from `cargo x reconcile` and tests; no production binary runs it at startup

**File:** `rust/xtask/src/reconcile.rs:131` (only non-test caller)
**Spec reference:** D-020 (boot-time green_when validation); §9.7 ("the Resolver's build-time validator (Phase 1.5B and Phase 2) catches these"); §10.5 acceptance #1 (`cargo test --workspace` passes — implies test gate, not runtime gate).

**Observation:**

```
$ rg "validate_dags\\(|validate_resolved_template_gate_metadata\\(" --type rust
xtask/src/reconcile.rs:131:        validate_dags(loaded)
crates/dsl-core/src/config/dag_validator.rs:471: pub fn validate_dags(...)
crates/dsl-core/src/config/dag_validator.rs:472:     validate_dags_with_context(loaded, ...)
crates/dsl-core/src/config/dag_validator.rs:507: pub fn validate_resolved_template_gate_metadata(...)
crates/dsl-core/tests/closure_lint.rs:24: validate_resolved_template_gate_metadata(...)
crates/dsl-core/tests/closure_lint.rs:49: validate_resolved_template_gate_metadata(...)
crates/dsl-core/tests/dag_validator_gate.rs:43: validate_dags_with_context(...)
crates/dsl-core/tests/eligibility_lint.rs:30: validate_resolved_template_gate_metadata(...)
crates/dsl-core/tests/eligibility_lint.rs:54: validate_resolved_template_gate_metadata(...)
crates/dsl-core/tests/phase2_acceptance.rs:32: validate_resolved_template_gate_metadata(...)
```

No call site in `rust/src/` (the ob-poc binary), `rust/crates/ob-poc-web/`, `rust/crates/sem_os_server/`, or any other production startup path. The ob-poc-web server starts, loads `ResolverInputs::from_seed_root`, builds resolved templates on-demand, but never runs the validator.

**Issue:** D-020's framing ("boot-time green_when validation") is ambiguous — could mean "developer build / CI gate" (which `cargo x reconcile` covers) or "system startup gate" (which is not implemented). Two related risks:

1. **Drift between CI and prod.** The CI gate ensures committed YAMLs are valid. But a deployed binary loads YAMLs at runtime — if a deployment includes mutated config (live edits, environment-specific overrides, future hot-reload), the validator never runs.
2. **Schema-coordination warnings hidden in production.** The validator emits warnings (D-011) that are advisory; in production the binary runs without ever computing them, so the operator has no visibility into drift unless they re-run reconcile externally.

§9.7 is explicit: "The Resolver's build-time validator (Phase 1.5B and Phase 2) catches these" — it's framed as a *build-time* validator. So calling P3-004 a MUST-FIX overstates the spec contract. But the production-startup gap remains a real consideration.

**Disposition (suggested):** add a `validate_at_startup` call in `ob-poc-web::main` (and `sem_os_server::main`) that runs `validate_dags` + `validate_constellation_map_dir_schema_coordination_strict(known_deferred)` and aborts on errors / logs warnings. Cost is one validator pass per startup (~milliseconds). Until that lands, document explicitly in `legacy-compat-tracker.md` or in `dag_validator.rs` doc comments that the validator is a CI gate, not a runtime gate.

---

#### [SHOULD-FIX] [QUALITY] P3-005 — `green_when_coverage` test asserts on absolute totals (332 / 196 / 12 / 184); brittle to authored content additions

**File:** `rust/crates/dsl-core/tests/green_when_coverage.rs`
**Lines:** 89–95 (`real_dag_green_when_coverage_baseline_is_explicit`); 100–137 (per-workspace expected map)
**Spec reference:** §10.14 (Phase 8 acceptance: "coverage against the input set's resolved templates reaches the target threshold").

**Observation:**

```rust
assert_eq!(summary.total_states, 332, "state count drifted");
assert_eq!(summary.candidate_states, 196, "candidate count drifted");
assert_eq!(summary.covered_candidate_states, 12, "green_when coverage drifted");
assert_eq!(summary.missing_candidate_states, 184);
```

Plus an explicit per-workspace `BTreeMap` of `(total, candidate, covered, missing)` tuples for all 12 workspaces.

**Issue:** the assertions are brittle in both directions. Adding a new state, transition, or `green_when` to any DAG taxonomy fails the test, even if the addition is correct. This forces the test fixture to be updated alongside every authored content change — coupling content authoring to test maintenance.

The test design's strength is regression detection: if someone removes a `green_when`, coverage drops and the test catches it. The weakness is that it equally rejects intentional improvements ("we added a `green_when`, coverage is now 13/196") without preserving the underlying invariant ("covered does not decrease").

**Disposition (suggested):** replace absolute equality with two assertions: (a) `assert!(summary.covered_candidate_states >= MINIMUM_COVERAGE)` to catch regressions; (b) compare per-workspace coverage against the `green-when-coverage-2026.md` artefact loaded at test time, with a CI-renewable baseline. Or split into two tests: one anchored to `green-when-coverage-2026.csv` as a snapshot, one asserting non-regression of `covered_candidate_states`.

---

### CONSIDER

#### [CONSIDER] [STYLE] P3-006 — Predicate parser is hand-rolled string-splitting; brittle to edge cases

**File:** `rust/crates/dsl-core/src/config/predicate/parser.rs`
**Lines:** entire file (398 LOC)

**Observation:** the parser uses `split_once`, `strip_prefix`, `strip_suffix`, and ad-hoc whitespace normalisation:

```rust
fn normalize(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_conjuncts(input: &str) -> Vec<String> {
    let normalized = normalize(input);
    // ...
    for part in normalized.split(" AND ").map(str::trim) {
        if is_relation_scope_tail(part) {
            // re-attach to previous clause
        }
    }
}
```

State sets are hand-parsed (line 250–272). `attached_to`, `for this`, `parent` prefixes are special-cased one at a time.

**Issue:** every authored variation (case sensitivity of `AND` vs `and`, multi-line predicates with embedded whitespace, quoted state names, etc.) is a potential edge case. The 18 confirmed fixtures parse, but the parser's surface is large enough to be hard to reason about.

**Disposition (suggested):** consider migrating to a proper grammar (e.g., `pest`, `winnow`, `nom`). Cost: a few hundred LOC + grammar definition; benefit: complete parse-error reporting, easier extension to `Count` / `Obtained` (P3-001), and predictable behaviour on edge cases.

---

#### [CONSIDER] [QUALITY] P3-007 — `attached_to` clause is reattached to the *immediately previous* conjunct in `split_conjuncts`

**File:** `rust/crates/dsl-core/src/config/predicate/parser.rs`
**Lines:** 57–77

**Observation:**

```rust
fn split_conjuncts(input: &str) -> Vec<String> {
    // ...
    for part in normalized.split(" AND ").map(str::trim) {
        if is_relation_scope_tail(part) {
            if let Some(previous) = clauses.last_mut() {
                previous.push_str(" AND ");
                previous.push_str(part);
            } else {
                clauses.push(part.to_string());
            }
        } else {
            clauses.push(part.to_string());
        }
    }
    clauses
}
```

`is_relation_scope_tail(part)` returns true when `part.starts_with("attached_to ")`. The parse re-attaches such tails to the previous clause via `" AND " + part`.

**Issue:** the reattachment is positional — if an author writes `A AND attached_to X AND B AND attached_to Y`, the result is `[A AND attached_to X, B AND attached_to Y]`. That happens to be correct for the simple case, but if an author writes a top-level `attached_to` without preceding context, it's pushed as a standalone clause and `parse_clause` will fail with "expected qualified field." No specific guidance for the author.

**Disposition (suggested):** detect the malformed `attached_to` at the top level and emit a typed `ParseError::OrphanedAttachedTo`. Or grammar-refactor (P3-006) and remove this string-level glue.

---

#### [CONSIDER] [STYLE] P3-008 — `parse_attr_value` has surprising symbol-vs-number-vs-string semantics

**File:** `rust/crates/dsl-core/src/config/predicate/parser.rs`
**Lines:** 381–398

**Observation:**

```rust
fn parse_attr_value(value: &str) -> AttrValue {
    let value = value.trim();
    if matches!(value, "true" | "TRUE") { return AttrValue::Bool(true); }
    if matches!(value, "false" | "FALSE") { return AttrValue::Bool(false); }
    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        return AttrValue::Number(value.to_string());
    }
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return AttrValue::String(value[1..value.len() - 1].to_string());
    }
    AttrValue::Symbol(value.to_string())
}
```

`100` → `Number("100")`. `"100"` → `String("100")`. `'100'` → `String("100")`. `100abc` → `Symbol("100abc")`. `True` (mixed case) → `Symbol("True")` (only `true`/`TRUE` match). The hydrator's `cmp_string_or_number` (Pass 2 P2-011) silently lex-compares strings, so `100` vs `"100"` would yield different comparison semantics.

**Issue:** the value classifier is unusually strict on bool casing (only exact `true`/`TRUE`/`false`/`FALSE`) yet permissive on number-style symbols. Inconsistencies between author intent ("I quoted it because it's a string") and downstream comparison semantics are silent.

**Disposition (suggested):** document the classification rules in `parse_attr_value`'s doc comment; consider matching `bool::from_str` (lowercase only) for stricter typing; consider `value.parse::<bool>().ok()` as the bool path.

---

#### [CONSIDER] [QUALITY] P3-009 — `detect_derivation_cycles` continues iteration after Gray detection without breaking

**File:** `rust/crates/dsl-core/src/config/dag_validator.rs`
**Lines:** 1862–1899

**Observation:** when a cycle is detected (`Color::Gray`), the validator pushes `DerivedCrossWorkspaceStateCycle` and continues iterating through the remaining neighbours/stack. There is no `break` or short-circuit. The DFS state continues to visit other neighbours and may detect overlapping cycles in the same SCC and report them.

**Issue:** the test fixture has a single 2-cycle (a→b→a) which produces exactly one report. For nested cycles in the same SCC (e.g., a→b→c→a and a→c→a), the validator could produce duplicate / overlapping reports. No current authored DAG has nested cycles, so the issue is latent.

**Disposition (suggested):** acceptable as-is for the current fixture set. If real DAGs evolve to have multiple SCCs or nested cycles, deduplicate cycle reports by canonicalising the cycle path (rotate to lowest-id-first) before pushing.

---

### NOTE

#### [NOTE] P3-010 — Cross-pass correction on Pass 1 P1-008

**File:** (cross-reference) `docs/governance/code-review-2026-05-01-pass1-resolver.md` finding P1-008
**Spec reference:** D-018 final constraint.

**Observation:** Pass 1's P1-008 said the `+`-sigil schema fields on `Slot` (dag.rs) and `core_map::SlotDef` (constellation_map_def.rs) "silently drop" data. That description is incomplete. The validator's `reject_additive_predicate_vector` (`dag_validator.rs:1224-1240`) and the corresponding constellation-map-side check (`validate_raw_constellation_map_schema_coordination`, lines 1657-1677) DO surface `DagError::AdditivePredicateSigilForbidden` when the fields are populated. Tests `additive_predicate_sigil_is_rejected_in_dag_taxonomy` and `additive_predicate_sigil_is_rejected_in_constellation_map` verify this.

The combined behaviour:
- Schema accepts the field (parses without error).
- Validator (when run) rejects with `AdditivePredicateSigilForbidden`.
- Composer ignores the field even if validator wasn't run.

So the "silent drop" framing in P1-008 is correct *only* when the validator is bypassed (which matters per P3-004). Net: P1-008 should be downgraded from SHOULD-FIX to CONSIDER pending P3-004 resolution. The spec contract (parse-time validation per D-018 final constraint) is implemented; the only question is whether validation is invoked in production.

---

#### [NOTE] P3-011 — `EntityRef::Parent(kind)` requires `kind` in the same machine's `predicate_bindings`

**File:** `rust/crates/dsl-core/src/config/dag_validator.rs`
**Lines:** 1408–1448 (`collect_predicate_entity_refs`, `collect_entity_ref`)

**Observation:** the unbound-entity check collapses `EntityRef::Parent(kind)` and `EntityRef::Named(kind)` into the same lookup against `predicate_bindings`. So a predicate `parent kyc_case.state = APPROVED` on slot X requires X's `predicate_bindings` to contain an entry with `entity: kyc_case`.

**Issue:** logically, a "parent" predicate references a different slot's state. Whether the binding for the parent should live on the child slot's predicate_bindings (as the current implementation requires) or be inherited from the parent is unclear. Spec doesn't explicitly address. No current real DAG uses parent-style predicates (zero `parent <kind>` predicates in the 18 confirmed fixtures), so this is latent.

**Disposition:** document the convention. If parent-style predicates start appearing, design the cross-slot resolution explicitly.

---

#### [NOTE] P3-012 — green_when coverage tooling is honest and correctly anchored to governance artefacts

**File:** `rust/crates/dsl-core/src/config/green_when_coverage.rs`, `rust/crates/dsl-core/tests/green_when_coverage.rs`, `docs/governance/green-when-coverage-2026.{md,csv}`

**Observation:** `green_when_coverage` correctly excludes (a) entry states, (b) source-only states (no incoming transitions), (c) destinations reached only by discretionary verbs. The summary table in `green-when-coverage-2026.md` matches the test fixture's expected per-workspace tuples (12 workspaces, 332 total states, 196 candidate, 12 covered, 184 missing). The exclusion logic is verified by the synthetic test `synthetic_coverage_excludes_entry_source_and_discretionary_destinations`.

**Disposition:** no action. The Phase 8 manifest is legitimately diagnostic and matches the documented coverage.

---

## Coverage notes

**What this pass covered:**
- `dag_validator.rs` (2,324 LOC) — closure lint, eligibility lint, schema-coordination validator + hardener + known-deferred allowlist, `+`-sigil parse-time guard, cross-workspace constraint validation, derived-cross-workspace-state cycle detection, predicate parsing pipeline integration.
- `predicate/{ast,parser,mod}.rs` — full AST surface verified; parser dispatch verified for 7 of 9 variants.
- `green_when_coverage.rs` + governance artefacts.
- D-011 (warning hardening), D-018 (per-field merge precedence; `+`-sigil parse guard; predicate AST coverage; `replaceable_by_shape` defaults), D-019 (validator integration), D-020 (boot-time green_when validation — implemented as build-time per spec language).
- §10.5 acceptance criteria (closure lint synthetic violation; eligibility lint synthetic violation; schema-coordination validator runs).
- §10.8 acceptance criteria (closure lint operates against ResolvedTemplate; eligibility lint operates against ResolvedTemplate; schema-coordination warnings convert to errors via `harden_schema_coordination_warnings`).

**What this pass deliberately did not cover:**
- Authored YAML quality (Pass 4).
- Test quality of the validator's tests (Pass 5).
- Whether the `unbounded_universal_quantifier` lint correctness extends to deeply-nested predicates beyond the verified arms (`And`, `Every`, `NoneExists`, `AtLeastOne`, `Count`) — recursion is correct but not exhaustively fixture-tested.

**Inconclusive / verified by code only:**
- Schema-coordination warning hardening preserves only documented `known_deferred` entries — verified by the `strict_authored_seed_schema_coordination_preserves_known_deferred_only` test and by reading the implementation. The deferred-allowlist itself (currently one entry) is content-appropriate but not Pass 3's audit scope.
- `validate_resolved_template_gate_metadata` was confirmed against the `phase2_acceptance.rs` test, which iterates 18 leaf shape rules with constellation maps and asserts zero errors. The completeness of the lint coverage on those 18 shapes is bounded by the test's 3-kind entity taxonomy (P3-003).

## Recommended next steps

In priority order, severity-ranked.

1. **SHOULD-FIX** P3-004 (boot-time gate not wired): minimal cost, large blast-radius reduction. Even if the spec language is "build-time," the runtime gate is cheap and adds defence in depth.
2. **SHOULD-FIX** P3-002 (closure lint matches by slot id): real correctness gap once authored YAMLs differentiate slot id from predicate-binding entity kind.
3. **SHOULD-FIX** P3-003 (eligibility lint silent-skip on empty taxonomy): API design issue with concrete production risk; tighten the context contract.
4. **SHOULD-FIX** P3-001 (parser missing Count and Obtained): low-risk while no real authoring uses those forms; surface a clearer parse error so authors aren't confused.
5. **SHOULD-FIX** P3-005 (coverage test brittle): replace absolute-equality with snapshot + non-regression assertion.
6. **CONSIDER** P3-006 / P3-007 / P3-008 / P3-009: parser polish and edge-case hygiene.
7. **NOTE** P3-010 / P3-011 / P3-012: documentation and cross-pass corrections.
