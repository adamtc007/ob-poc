# Code Review — Pass 5: Test quality

**Commit:** `2642126b "Implement SemOS DAG architecture phases"`
**Files in scope:** all test files added or modified in this commit:
- `rust/crates/dsl-core/tests/cbu_validity.rs` (264 LOC)
- `rust/crates/dsl-core/tests/cbu_evidence_substates.rs` (173 LOC)
- `rust/crates/dsl-core/tests/closure_lint.rs` (59 LOC)
- `rust/crates/dsl-core/tests/dag_gate_metadata.rs` (372 LOC)
- `rust/crates/dsl-core/tests/dag_validator_gate.rs` (316 LOC)
- `rust/crates/dsl-core/tests/eligibility_lint.rs` (64 LOC)
- `rust/crates/dsl-core/tests/frontier_recursive.rs` (150 LOC)
- `rust/crates/dsl-core/tests/frontier_skeleton.rs` (162 LOC)
- `rust/crates/dsl-core/tests/green_when_coverage.rs` (149 LOC)
- `rust/crates/dsl-core/tests/lux_sicav_pilot.rs` (176 LOC)
- `rust/crates/dsl-core/tests/phase2_acceptance.rs` (46 LOC)
- `rust/crates/dsl-core/tests/predicate_ast.rs` (243 LOC)
- `rust/crates/dsl-core/tests/resolver_lux_sicav.rs` (66 LOC)
- `rust/crates/dsl-core/tests/resolver_manifest.rs` (33 LOC)
- `rust/crates/dsl-core/tests/shape_rule_composition.rs` (658 LOC)
- `rust/crates/dsl-core/tests/verb_flavour_catalogue.rs` (92 LOC)
- `rust/crates/sem_os_core/tests/constellation_gate_metadata.rs` (287 LOC)

**LOC reviewed:** 3,310 across 17 new test files
**Build status:**
- ✓ `cargo check -p dsl-core` clean
- ✓ `cargo test -p dsl-core` 359 passed / 0 failed / 7 ignored
- ✓ `cargo test -p sem_os_core --test constellation_gate_metadata` (verified earlier in pass)
- ✗ **`cargo test --workspace` FAILS to compile** — 20 sites fail with `E0063: missing fields audit_class, flavour and role_guard in initializer of dsl_core::VerbConfig` across `crates/sem_os_obpoc_adapter` (2) and `rust/src/...` (18). All 20 are inside `#[cfg(test)]` modules; production code compiles. See P5-001.

**Reviewer:** Zed-Claude
**Date:** 2026-05-01

## Summary

15 findings: 4 MUST-FIX, 6 SHOULD-FIX, 2 CONSIDER, 3 NOTE. The new test files demonstrate strong assertion discipline overall — most tests assert specific values (concrete `assert_eq!` on enum variants, concrete predicate-derivation outputs, specific entity-binding state IDs), recursive Frontier tests use real recursive fixtures with cyclic test cases, the `+`-sigil parse-time guard is exercised in both DAG and constellation contexts, error paths are exercised directly (mixed-vector composition, malformed predicates, unbound entities, schema-coordination drift, cycle in derived states). No `#[ignore]` in any new test, no floating-point comparisons, no bare `.unwrap()`/`.expect()` in production source paths from this commit.

The substantive test-quality issue is the **workspace compile break** (P5-001): the commit added three required fields (`audit_class`, `flavour`, `role_guard`) to `VerbConfig` without updating the 20 test sites that construct `VerbConfig` literals across two crates. The fields are `Option<T>` with serde-default for YAML loading, so production code is fine; only test code that builds structs in Rust source breaks. Per the prompt's workflow rule #5 ("Compile and test before finalizing each pass... If the build fails, that is the first finding"), this is Pass 5's primary blocker.

The second class of issues is **fixtures aligned with bugs**: tests that cement-lock content defects identified in Pass 4 rather than catch them. `lux_sicav_pilot.rs:67` asserts `cbu_evidence.entry_state == "PENDING"` (P4-001 says it should be UPLOADED). `cbu_validity.rs:256-264` asserts the I6-violating bare `entity: "predicate"` fallback (P2-004) as expected output. Six tests in `shape_rule_composition.rs` assert the unresolved template-placeholder strings (P4-006) and the prime-broker required+optional duplication (P4-005) as expected outputs. The test suite is internally consistent — cargo test passes — but each test asserts the bug its fixture encodes.

Third class: **the cbu_validity test harness uses synthetic UPPERCASE state IDs that match the predicate's UPPERCASE references**, even though the actual mandate lifecycle (`trading_profile_lifecycle.yaml`) is lowercase. The synthetic fixture creates a closed loop that confirms the predicate works against UPPERCASE inputs; the production substrate path that exposes lowercase state IDs is unreachable from the test harness. P4-002's silent-runtime-failure cannot be detected by the current test design.

Fourth class: **regression tests anchored to absolute totals** (P3-005's pattern) — `verb_flavour_catalogue.rs:22` and `:72` assert exact verb counts (1,288 and 166); `green_when_coverage.rs` asserts exact 332/196/12/184 totals. These detect drift in either direction but force test-fixture updates on every authored content addition.

## Findings

### MUST-FIX

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P5-001 — `cargo test --workspace` fails to compile: 20 `VerbConfig` literal sites missing the new `audit_class` / `flavour` / `role_guard` fields

**Files (failure sites, all under `#[cfg(test)]`):**
- `crates/sem_os_obpoc_adapter/src/lib.rs:168`
- `crates/sem_os_obpoc_adapter/src/scanner.rs:979`
- `rust/src/dsl_v2/enrichment.rs:277`, `:380`
- `rust/src/dsl_v2/runtime_registry.rs:1098`, `:1321`
- `rust/src/session/verb_tiering_linter.rs:684`, `:733`, `:782`
- `rust/src/runbook/step_executor_bridge.rs:600`
- `rust/src/repl/verb_config_index.rs:517`
- `rust/src/repl/intent_service.rs:454`, `:511`
- `rust/src/sem_reg/scanner.rs:462`
- `rust/src/sage/arg_assembly.rs:188`
- `rust/src/sage/coder.rs:437`, `:522`
- `rust/src/sage/verb_index.rs:629`, `:696`, `:803`

**Spec reference:** Pass 5 prompt — "Test pass before finalizing: `cargo test --workspace`"; Pass 5 prompt §10.13 (Phase 7 verb flavour classification).

**Observation:** Phase 7 added three fields to `VerbConfig`:

```rust
// crates/dsl-core/src/config/types.rs
pub struct VerbConfig {
    pub description: String,
    pub behavior: VerbBehavior,
    // ... existing fields ...

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavour: Option<VerbFlavour>,            // NEW

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_guard: Option<VerbRoleGuard>,       // NEW

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_class: Option<String>,             // NEW
}
```

The fields are `Option<T>` with serde defaults — YAML loading works without authoring them. But Rust struct literals require every field to be initialised explicitly. Twenty test sites construct `VerbConfig { ... }` without the new fields:

```
$ cargo test --workspace --no-fail-fast 2>&1 | grep "could not compile"
error: could not compile `sem_os_obpoc_adapter` (lib test) due to 2 previous errors
error: could not compile `ob-poc` (lib test) due to 18 previous errors
```

`cargo check -p dsl-core` is clean, `cargo test -p dsl-core` is clean (the dsl-core tests don't construct `VerbConfig` literals). The breakage surfaces only when the test-mode compilation extends across to `sem_os_obpoc_adapter` and `ob-poc`.

**Issue:** the workspace test gate is broken. The prompt's Pass 5 acceptance is `cargo test --workspace`; this does not pass. The fix per finding is mechanical (add `flavour: None, role_guard: None, audit_class: None` to all 20 sites), but the broader pattern is concerning — adding required fields to a widely-constructed struct without a builder or default is a maintenance hazard. P5-011 / P5-012 surface the same concern from the test-design angle.

**Disposition (suggested):** add `..Default::default()` support to `VerbConfig` (derive or manual `Default` impl), then the test-site updates collapse to no-ops. Alternatively, mechanically add the three `None` fields to all 20 sites. Either way, `cargo test --workspace` must compile before this commit is shippable.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P5-002 — `lux_sicav_pilot.rs:67` cement-locks the `cbu_evidence.entry_state` defect from P4-001

**File:** `rust/crates/dsl-core/tests/lux_sicav_pilot.rs`
**Lines:** 64–69 (the `for (slot_id, expected_kind, expected_entry_state)` loop)
**Spec reference:** D-018 table row for `entry_state` ("State not in resolved machine → ResolveError"); §10.6 acceptance #2.

**Observation:**

```rust
for (slot_id, expected_kind, expected_entry_state) in [
    ("entity_proper_person", Some("person"), "GHOST"),
    ("entity_limited_company_ubo", Some("company"), "PENDING"),
    ("cbu_evidence", None, "PENDING"),                       // <-- expects PENDING
    ("share_class", None, "DRAFT"),
] {
    let slot = slots.get(slot_id).unwrap_or_else(|| panic!("{slot_id} present"));
    assert_eq!(slot.entry_state.as_deref(), Some(expected_entry_state), "{slot_id}");
}
```

The test asserts `cbu_evidence.entry_state == "PENDING"`. Pass 4 P4-001 documented that the cbu_evidence inline state machine's actual entry-flagged state is `UPLOADED` (line 549–550 in `cbu_dag.yaml`), not PENDING. The state machine was renamed during this commit; the slot's `entry_state` annotation was missed; this test asserts the unrenamed value.

**Issue:** the test passes because the YAML carries `PENDING` and the test asserts `PENDING`. Both are wrong — the inline state machine has no `PENDING` state. A correct test would resolve `entry_state` against the slot's state-machine `states` list and reject any value that doesn't appear there. This test's structure (assert against a hardcoded expected value) cannot detect the inconsistency.

This is the prompt's exact "fixture aligned with bug" pattern. Fix in tandem with P4-001: change YAML and test to `UPLOADED`, AND add a structural test that asserts `slot.entry_state` is in `slot.state_machine.states[*].id`.

**Disposition (suggested):** fix YAML (P4-001 disposition), update test to assert `UPLOADED`, add a cross-validation test that walks every slot's `entry_state` and asserts it appears in the inline state machine's state list.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P5-003 — `cbu_validity.rs:256-264` asserts the I6-violating bare-fallback diagnostic as expected output

**File:** `rust/crates/dsl-core/tests/cbu_validity.rs`
**Lines:** 224–232 (`cbu_validated_fails_when_disqualifying_flag_exists` test); 256–264 (`assert_red_with_fallback_diagnostic` helper)
**Spec reference:** I6 ("When a `green_when` evaluates red, the dispatcher returns structured diagnostics... Bare 'predicate failed' is a defect"); Pass 2 P2-004.

**Observation:**

```rust
fn assert_red_with_fallback_diagnostic(status: GreenWhenStatus) {
    let GreenWhenStatus::Red { missing, invalid } = status else {
        panic!("expected red status");
    };
    assert!(missing.is_empty(), "unexpected missing facts: {missing:?}");
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].entity, "predicate");                       // <-- bare entity
    assert_eq!(invalid[0].reason, "predicate evaluated false");       // <-- bare reason
}

#[test]
fn cbu_validated_fails_when_disqualifying_flag_exists() {
    // ...
    assert_red_with_fallback_diagnostic(validated_status(facts));
}
```

The test deliberately exercises a `Predicate::NoneExists` failure path (`no investor_disqualifying_flag exists` becomes false when one exists) and asserts the bare-string fallback that Pass 2 P2-004 named as "exactly the I6 anti-pattern."

The other six `cbu_validated_fails_when_*` tests in this file use `assert_red_invalid_entity` which checks for a specific entity name in the invalid list. Only the `disqualifying_flag` path asserts the bare diagnostic — because `Predicate::NoneExists`'s failure path doesn't push structured detail (Pass 2 P2-004 named this specific gap).

**Issue:** the test cement-locks the I6 violation as the expected outcome. Per spec, this case should produce a structured diagnostic (e.g., `InvalidFact::ForbiddenMemberPresent { kind: "investor_disqualifying_flag", fact_id: ... }`), and the test should assert that. Fixing the underlying defect (P2-004's disposition) requires updating this test. As long as the test stands, the defect cannot be fixed without a test failure that some reviewer must intentionally update.

**Disposition (suggested):** fix in tandem with P2-004 — add structured pushing for `NoneExists` failure paths, then rewrite this test to assert the structured variant. Until then, document that the test's current shape is a known I6 bypass.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P5-004 — `cbu_validity.rs` mandate-state test cannot detect P4-002 (uppercase predicate vs lowercase trading_profile state machine)

**File:** `rust/crates/dsl-core/tests/cbu_validity.rs`
**Lines:** 12–19 (`CBU_VALIDATED_GREEN_WHEN`); 100–120 (`happy_facts`); 197–202 (`cbu_validated_fails_when_mandate_is_not_approved_or_active`)
**Spec reference:** I8 (compute-on-read against substrate); Pass 2 P2-005 (synthetic-fact framing); Pass 4 P4-002.

**Observation:**

```rust
const CBU_VALIDATED_GREEN_WHEN: &str = r#"
...
AND every mandate.state in {APPROVED, ACTIVE}              # <-- uppercase predicate
...
"#;

fn happy_facts() -> BTreeMap<String, Vec<FrontierFact>> {
    BTreeMap::from([
        ...
        ("mandate".to_string(), vec![state_fact("APPROVED")]),  # <-- uppercase fact
        ...
    ])
}

#[test]
fn cbu_validated_fails_when_mandate_is_not_approved_or_active() {
    let mut facts = happy_facts();
    facts.insert("mandate".to_string(), vec![state_fact("DRAFT")]);  # <-- uppercase fact
    assert_red_invalid_entity(validated_status(facts), "this");
}
```

The test creates synthetic UPPERCASE mandate facts, parses the UPPERCASE predicate, and verifies the synthetic happy-path is green and the synthetic unhappy-path is red. The whole loop closes inside the test fixture — never touching the actual mandate lifecycle owned by Instrument Matrix.

Per Pass 4 P4-002, the production `trading_profile_lifecycle.yaml` uses lowercase state IDs (`approved`, `active`, `draft`, etc.). When the predicate is evaluated against real substrate, `every mandate.state in {APPROVED, ACTIVE}` will never match because the substrate reports lowercase. The test's synthetic UPPERCASE facts hide this defect.

**Issue:** the test's pass/fail outcome is determined entirely by the case the fixture chose to author. A test with the spec-aligned design (substrate-backed, lowercase facts driven by `trading_profile_lifecycle`) would fail today, exposing P4-002. The current synthetic-fact framing (Pass 2 P2-005) is a precondition for this gap.

**Disposition (suggested):** add an integration-style test (gated on a substrate fixture, possibly `#[ignore]` until the runtime hydrator lands) that uses the actual `trading_profile_lifecycle` state machine's ID conventions. Until then, document that all `cbu_validity.rs` mandate-related tests pass synthetic UPPERCASE through synthetic UPPERCASE — they don't validate the production cross-workspace path.

---

### SHOULD-FIX

#### [SHOULD-FIX] [QUALITY] P5-005 — `shape_rule_composition.rs` tests assert P4-005's prime-broker required+optional duplication as expected output

**File:** `rust/crates/dsl-core/tests/shape_rule_composition.rs`
**Lines:** 297–299 (`shape_rule_composition_extracts_ie_hedge_icav_macro_facts`); 593–595 (`shape_rule_composition_extracts_cross_border_macro_facts` / hedge case)
**Spec reference:** §7 (shape-aware authoring); Pass 4 P4-005.

**Observation:**

```rust
// ie_hedge_icav (lines 296-310)
assert_eq!(
    template.structural_facts.required_roles,
    vec!["aifm", "depositary", "prime-broker"]
);
assert_eq!(
    template.structural_facts.optional_roles,
    vec![
        "investment-manager",
        "administrator",
        "auditor",
        "prime-broker",          // <-- duplicate role
        "executing-broker"
    ]
);

// hedge.cross-border (lines 583-596)
required_roles: &["aifm", "depositary", "prime-broker"],
optional_roles: &[
    "investment-manager",
    "administrator",
    "auditor",
    "prime-broker",              // <-- duplicate role
],
```

The test fixtures match the YAML defects flagged in Pass 4 P4-005 (`hedge_cross_border.yaml` and `ie_hedge_icav.yaml` list `prime-broker` in both lists).

**Issue:** when the YAMLs are fixed to remove the duplication, these tests will fail. They assert the bug as expected output. Fix in tandem with P4-005.

**Disposition (suggested):** decide canonical shape-rule semantics for prime-broker (required for hedge-fund-with-prime-broker variants? optional for those without?). Update YAMLs and tests together. Optionally add an authoring lint that rejects role-list overlap.

---

#### [SHOULD-FIX] [QUALITY] P5-006 — `shape_rule_composition.rs` tests assert unresolved template-placeholder strings as expected `structural_facts` values

**File:** `rust/crates/dsl-core/tests/shape_rule_composition.rs`
**Lines:** 583–612 (cross-border tests); 510 (us_private_fund_delaware_lp `trading_profile_type`)
**Spec reference:** §3.3 (StructuralFacts as authored declarative inputs); Pass 4 P4-006.

**Observation:**

```rust
Expected {
    shape: "struct.hedge.cross-border",
    jurisdiction: "${arg.master_jurisdiction.internal}",   // <-- placeholder asserted as fact
    ...
},
Expected {
    shape: "struct.pe.cross-border",
    jurisdiction: "${arg.main_fund_jurisdiction.internal}", // <-- placeholder asserted as fact
    ...
},
Expected {
    shape: "struct.us.private-fund.delaware-lp",
    trading_profile_type: "${arg.fund_type.internal}",      // <-- placeholder asserted as fact
    ...
},
```

The tests fixate on the unresolved placeholder strings as the expected jurisdiction / trading_profile_type values. Per Pass 4 P4-006, structural_facts are supposed to be authored truths, not template variables; storing `${arg....}` strings in `template.structural_facts.jurisdiction` is a content authoring defect.

**Issue:** the test design says "the placeholder string IS the fact." That makes downstream consumers reading `template.structural_facts.jurisdiction.as_deref() == Some("LU")` semantically wrong for cross-border shapes (they'd get the unresolved string).

**Disposition (suggested):** define structural-fact semantics — placeholders are not facts. Either author a canonical sentinel (`jurisdiction: cross-border`) or change `StructuralFacts.jurisdiction` to an enum (`Concrete(String)` vs `DeferredArg(String)`) and update tests to assert the deferred variant.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P5-007 — `frontier_recursive.rs` cycle-detection test uses string-prefix probing (`reason.starts_with("CycleDetected")`)

**File:** `rust/crates/dsl-core/tests/frontier_recursive.rs`
**Lines:** 132–150 (`cyclic_ubo_chain_is_detected_and_reported_as_invalid_fact`)
**Spec reference:** §4.3 (`CycleDetected { entities: Vec<EntityRef> }` as structured variant of `Red { invalid }`); I6; Pass 2 P2-001.

**Observation:**

```rust
#[test]
fn cyclic_ubo_chain_is_detected_and_reported_as_invalid_fact() {
    // ... build a fixture with ubo-1 → ubo-2 → ubo-1 ...
    let GreenWhenStatus::Red { missing, invalid } = &frontier.reachable[0].status else {
        panic!("expected red destination");
    };
    assert!(missing.is_empty());
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].entity, "ubo");
    assert!(invalid[0].reason.starts_with("CycleDetected"));   // <-- string prefix
}
```

The test asserts on the reason field's textual prefix. Pass 2 P2-001 flagged this as the I6 anti-pattern — cycle data encoded as a Debug-formatted string in `InvalidFact.reason`, with `has_cycle` doing internal string-prefix probes. The test reinforces the encoding by asserting against it.

**Issue:** when P2-001's structural fix lands (typed `InvalidFact::CycleDetected { entities: Vec<...> }`), this test will need to be rewritten. The current shape is a known anti-pattern that the test happens to validate.

**Disposition (suggested):** rewrite the test to assert on the structured variant — once P2-001 lands. Until then, the test is technically correct against the current code; flag it as paired-with-P2-001.

---

#### [SHOULD-FIX] [QUALITY] P5-008 — `verb_flavour_catalogue.rs` and `green_when_coverage.rs` anchor on absolute totals; brittle to authored content additions

**File:** `rust/crates/dsl-core/tests/verb_flavour_catalogue.rs` (lines 22, 72), `rust/crates/dsl-core/tests/green_when_coverage.rs` (line 89–95, 100–137)
**Spec reference:** Pass 3 P3-005.

**Observation:**

```rust
// verb_flavour_catalogue.rs
assert_eq!(total, 1288, "real verb catalogue count drifted");
assert_eq!(checked, 166, "discretionary count drifted");

// green_when_coverage.rs
assert_eq!(summary.total_states, 332, "state count drifted");
assert_eq!(summary.candidate_states, 196, "candidate count drifted");
assert_eq!(summary.covered_candidate_states, 12, "green_when coverage drifted");
assert_eq!(summary.missing_candidate_states, 184);
```

Each absolute total must be updated whenever a verb is added/removed or a state is added/changed. CI would block any authored content change until the test fixture is updated alongside.

**Issue:** the design catches both regression (coverage decreases) and progression (coverage increases). A test that splits these — minimum-coverage assertion + snapshot artefact + non-regression check — would let progression pass without forcing test updates while still catching regressions. Pass 3 P3-005 surfaced this for `green_when_coverage.rs`; same applies to `verb_flavour_catalogue.rs`.

**Disposition (suggested):** replace absolute-equality with two assertions: (a) `assert!(total >= 1288)` (regression detection), (b) snapshot manifest comparison loaded from a CI-renewable file. The current pattern is intentional for v1.4 substrate-stability gating but creates churn.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P5-009 — Phase 1.5C Acceptance #5 ("byte-comparable to lux_aif_raif modulo intentional differences") has no dedicated test

**File:** `rust/crates/dsl-core/tests/lux_sicav_pilot.rs`
**Spec reference:** §10.6 acceptance #5.

**Observation:** `lux_sicav_pilot.rs` asserts:
- The pilot slots exist (`management_company`, `depositary`, `investment_manager`, `mandate`, `administrator`, `auditor`).
- `domiciliation_agent` is absent (per 1.5C explicit-deferral).
- The four pilot slots have the gate-metadata fields populated.

But it does NOT assert the structural blocks of `administrator` and `auditor` (the join, cardinality, depends_on, state_machine, overlays, verbs blocks) are byte-equivalent to `struct_lux_aif_raif.yaml`'s equivalents, which is what acceptance #5 specifically calls out.

Pass 4 P4-010 verified by inspection that the structural blocks ARE byte-equivalent — but no test cement-locks that. A future shape-rule maintainer who modifies `struct_lux_ucits_sicav.yaml::administrator` (e.g., changes `state_machine: entity_kyc_lifecycle` → something else) would not break this test.

**Issue:** the prompt explicitly calls this out: "§10.6 acceptance #5 says 'the new slots in struct_lux_ucits_sicav.yaml are byte-comparable to the equivalents in struct_lux_aif_raif.yaml modulo intentional UCITS-vs-AIF differences.' Find the test that verifies this. If absent, that's a finding (acceptance criterion not actually tested)." Confirmed absent.

**Disposition (suggested):** add a test that loads both YAMLs, extracts the `administrator` and `auditor` slots' structural blocks (sans gate-metadata, sans intentional UCITS-vs-AIF differences), and asserts equality. The "intentional differences" need to be enumerated explicitly in the test (e.g., the auditor's regulator reference if applicable).

---

#### [SHOULD-FIX] [QUALITY] P5-010 — `resolver_lux_sicav.rs:26` asserts shape-only check on transitions (`!template.transitions.is_empty()`)

**File:** `rust/crates/dsl-core/tests/resolver_lux_sicav.rs`
**Lines:** 26
**Spec reference:** Pass 5 prompt anti-pattern example ("a test like `assert!(template.slots.is_empty() == false)` is weak").

**Observation:**

```rust
assert!(!template.transitions.is_empty());
```

The Pass 5 prompt explicitly calls out this as the weak shape-check anti-pattern. The other five assertions in the same test are all strong-form (`assert_eq!(slot.closure, Some(ClosureType::ClosedBounded))`, `assert_eq!(slot.cardinality_max, Some(1))`, etc.). This one line is the outlier.

**Issue:** transitions number / contents are not asserted. A regression that drops transitions (e.g., deletes a state machine's `transitions:` block in a DAG YAML) would produce `template.transitions.len() == 0` and break this test, but a regression that drops some transitions and adds others wouldn't be caught.

**Disposition (suggested):** replace with a strong-form check, e.g., assert that the cbu slot has at least one transition with `to == "VALIDATED"`, and that the cbu_evidence slot has all expected transitions (`UPLOADED → REVIEWED`, `REVIEWED → APPROVED`, `REVIEWED → REJECTED`).

---

### CONSIDER

#### [CONSIDER] [STYLE] P5-011 — Multiple test files duplicate large `template()` builders for synthetic `ResolvedTemplate` construction

**File:** `cbu_validity.rs:21-81`, `cbu_evidence_substates.rs:12-86`, `frontier_recursive.rs:12-75`, `frontier_skeleton.rs:12-91`

**Observation:** four test files each define a ~70 LOC `template()` builder that constructs `ResolvedTemplate { workspace, composite_shape, structural_facts, slots: vec![ResolvedSlot { ... 30+ fields ... }], transitions: vec![...], version, generated_at, generated_from }`. Most fields are defaults (`None`, `Vec::new()`, `BTreeMap::new()`, `SlotProvenance::default()`) that need to be specified explicitly.

**Issue:** when `ResolvedTemplate` or `ResolvedSlot` gain a new field (cf. P5-001 — exactly what happened with `VerbConfig`), every fixture has to be updated. The boilerplate inflates each test file; a builder pattern (`ResolvedSlot::new(id).with_state_machine(sm).with_closure(...)`) would localize the cost.

**Disposition (suggested):** add a `dsl-core` test-helper module (or a separate crate, gated on `#[cfg(test)]`) that provides builder functions. Reduces duplication and insulates fixtures from struct-shape changes.

---

#### [CONSIDER] [STYLE] P5-012 — `PredicateBinding` literals require explicit `extra: BTreeMap::new()` and `replaceable_by_shape: false`

**File:** `cbu_validity.rs:97`, `cbu_evidence_substates.rs:101`, `frontier_skeleton.rs:32`, `frontier_recursive.rs:32`

**Observation:** every test fixture that constructs a `PredicateBinding` has to specify `extra: BTreeMap::new()` and `replaceable_by_shape: false` — both fields are `#[serde(default)]` for YAML loading but require explicit init in struct literals.

**Issue:** same maintenance hazard as P5-011. Once a `Default` impl exists or a builder lands, these go away.

**Disposition (suggested):** derive `Default` on `PredicateBinding` or provide a constructor `PredicateBinding::new(entity, source_kind)`.

---

### NOTE

#### [NOTE] P5-013 — All test files honour the test-boundary rule (external harnesses use only public API)

**File:** all 17 test files

**Observation:** every test file lives at `rust/crates/{crate}/tests/` and consumes only `pub` API from `dsl_core` / `sem_os_core`. No `#[cfg(test)]` reaches across crate boundaries. The CLAUDE.md test boundary rule ("Tests are either crate-internal (#[cfg(test)] inside src/) or external harnesses (rust/tests/). No test crosses crate boundaries — external tests use only the crate's public API.") is honoured.

**Disposition:** no action.

---

#### [NOTE] P5-014 — No floating-point comparisons in any new test file

**File:** all 17 test files

**Observation:** searched for `f64`, `f32`, `assert_eq!(.*, .*\.\d+)` in all new test files — zero matches. The `green_when_coverage_summary.coverage_percent()` returns `f64` (line 39 in `green_when_coverage.rs`) but the tests don't assert on it.

**Disposition:** no action.

---

#### [NOTE] P5-015 — No `#[ignore]` in any new test file; all 7 ignored tests in the workspace are pre-existing and have recorded reasons

**File:** all 17 test files

**Observation:**

```
$ rg "#\[ignore\]" crates/dsl-core/tests/
(no results)

$ rg "#\[ignore\]" crates/
crates/ob-semantic-matcher/src/embedder.rs:333: #[ignore] // Requires model download (× 5 instances)
crates/ob-templates/src/registry.rs:268:        #[ignore] // TODO: Fix search index — pre-existing
crates/sem_os_server/tests/authoring_http_integration.rs: #[ignore] // requires DATABASE_URL (× 10 instances)
crates/sem_os_harness/src/lib.rs: #[ignore] // Requires Postgres (× 3 instances)
```

Every `#[ignore]` outside the new test surface has a recorded reason (model download, DATABASE_URL, Postgres). One `TODO: Fix search index` is pre-existing and not introduced by this commit.

**Disposition:** no action.

---

## Coverage notes

**What this pass covered:**
- 17 new test files end-to-end (3,310 LOC).
- `cargo test -p dsl-core` (the per-crate gate) — 359 passed / 0 failed.
- `cargo test --workspace` — fails to compile per P5-001.
- Test assertion strength evaluated against the prompt's specific verifications: each test asserts a specific value (mostly ✅, P5-010 outlier); edge cases exercised (✅, error paths in mixed-vector composition, malformed predicates, unbound entities, schema-coordination drift, cycle in derived states); recursive Frontier tests use real recursion + cyclic fixture (✅, P5-007 caveat on string-prefix assertion).
- Phase 8 coverage test verifies manifest matches CSV/MD (✅, the test's per-workspace tuples line up with `green-when-coverage-2026.md`'s table).
- Phase 1.5C acceptance #5 byte-comparable test — absent (P5-009).
- `#[ignore]` audit — clean (P5-015).
- Floating-point audit — clean (P5-014).
- `unwrap()` / `expect()` audit on new src/ paths — clean (only `unwrap_or_default()` / `unwrap_or(...)` patterns, no panics; doc-test in `parser.rs:31` uses `expect("predicate parses")` which is in a doc example, not production source).
- Test boundary rule — honoured (P5-013).

**What this pass deliberately did not cover:**
- Domain semantics of test assertions (e.g., is `every entity_proper_person.state = VERIFIED` semantically correct for the CBU validation predicate? — that's a domain question outside the reviewer's scope).
- Property-based tests (none added in this commit; `proptest`/`quickcheck` not used).
- Test runtime / parallelism behaviour.

**Inconclusive / verified by code only:**
- The `cargo test --workspace` compile failure was confirmed at the cli; the 20 failure sites listed in the prompt are reproduced by the build's error output.
- Whether the four "fixtures aligned with bugs" findings (P5-002, P5-003, P5-005, P5-006) reflect intentional cement-locking or oversight — the test authors' intent is unknowable from the code alone.

## Recommended next steps

In priority order. P5-001 must be fixed before this commit ships; everything else can flow with the cross-pass remediation tracker.

1. **MUST-FIX** P5-001 (`cargo test --workspace` broken): mechanical fix. 20 sites + `Default` impl. Block merge until clean.
2. **MUST-FIX** P5-002 / P5-003 / P5-004 (fixtures aligned with content/diagnostic bugs): fix in tandem with P4-001, P2-004, P4-002 respectively. Each test fix is a one- or two-line change once the underlying defect is fixed.
3. **SHOULD-FIX** P5-005 / P5-006 (shape-rule prime-broker + placeholder fixtures): pair with P4-005 / P4-006 YAML fixes.
4. **SHOULD-FIX** P5-007 (cycle-detection string-prefix assertion): pair with P2-001 structural fix.
5. **SHOULD-FIX** P5-008 (absolute-totals tests): replace with snapshot + non-regression pattern.
6. **SHOULD-FIX** P5-009 (acceptance #5 byte-comparable test absent): add the test.
7. **SHOULD-FIX** P5-010 (weak `is_empty()` assertion in `resolver_lux_sicav.rs:26`): replace with strong-form check.
8. **CONSIDER** P5-011 / P5-012: builder pattern to reduce fixture maintenance cost.
9. **NOTE** P5-013 / P5-014 / P5-015: no action.
