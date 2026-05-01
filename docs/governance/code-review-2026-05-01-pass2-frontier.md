# Code Review — Pass 2: Frontier hydrator + recursive walk + cycle detection

**Commit:** `2642126b "Implement SemOS DAG architecture phases"`
**Files in scope:**
- `rust/crates/dsl-core/src/frontier/mod.rs` (65 LOC)
- `rust/crates/dsl-core/src/frontier/hydrator.rs` (426 LOC)
- (tests) `rust/crates/dsl-core/tests/frontier_skeleton.rs` (162 LOC), `rust/crates/dsl-core/tests/frontier_recursive.rs` (150 LOC)

**LOC reviewed:** 491 (frontier source) + 312 (tests)
**Build status:** `cargo check -p dsl-core` clean. `cargo test -p dsl-core --test frontier_skeleton --test frontier_recursive` 7/7 passed.
**Reviewer:** Zed-Claude
**Date:** 2026-05-01

## Summary

13 findings: 4 MUST-FIX, 5 SHOULD-FIX, 3 CONSIDER, 1 NOTE. The Frontier types match §3.2's shapes (with `via_verb` divergence), all 7 tests pass, and the leaf evaluator correctly produces `Green` and `Red` for the simple synthetic-fact fixtures. The substantive issue is that the implementation is a **synthetic-fact evaluator, not a substrate-reading hydrator**: facts are pushed in by the caller via `EntityRef.facts: BTreeMap<...>` rather than read from substrate at all. That makes the Phase 3 skeleton a stub for I8 (compute-on-read against substrate), §3.2's "sub-100ms target", §4.2's `WITH RECURSIVE` SQL compilation, and §4.3's cycle-detection-via-CYCLE-clause — none of these can be exercised today because no substrate path exists. This is consistent with §10's framing of Phase 3 as a "skeleton," but the gap should be acknowledged. Within the synthetic-fact evaluator, four real defects: (1) `AwaitingCompleteness` and `Discretionary` `GreenWhenStatus` variants are defined but never returned, so two of four states are dead surface; (2) cycle detection encodes `CycleDetected` as a debug-formatted string in `InvalidFact.reason` rather than the structured variant the spec mandates, and `has_cycle` re-detects via string-prefix probe; (3) recursive walk has no `max_depth` enforcement (cycle detection guards against revisits but not linear depth); (4) the I6 anti-pattern fallback "predicate evaluated false" is reachable from `Predicate::Count` and `Predicate::NoneExists` failure paths.

## Findings

### MUST-FIX

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P2-001 — `CycleDetected` is encoded as a debug-formatted string, not a structured variant

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 335–339 (`has_cycle`), 361–368 (cycle push site)
**Spec reference:** §4.3 (`CycleDetected { entities: Vec<EntityRef> }` — structured variant of `Red { invalid }`); I6 (diagnostic completeness).

**Observation:**

```rust
if path.contains(&id) {
    let mut cycle = path.clone();
    cycle.push(id);
    invalid.push(InvalidFact {
        entity: kind.to_string(),
        reason: format!("CycleDetected{{path:{cycle:?}}}"),
    });
    continue;
}
```

```rust
fn has_cycle(ctx: &EvalContext<'_>, set: &EntitySetRef) -> bool {
    ctx.invalid
        .iter()
        .any(|invalid| invalid.entity == set.kind && invalid.reason.starts_with("CycleDetected"))
}
```

The cycle is recorded as `InvalidFact { entity: "ubo", reason: "CycleDetected{path:[\"ubo-1\", \"ubo-2\", \"ubo-1\"]}" }`. The cycle path is rendered through `Debug` formatting, embedded in a free-text `reason: String`. `has_cycle` then re-detects cycles by string-prefix-matching `"CycleDetected"` on the reason field.

**Issue:** §4.3 explicitly specifies a structured cycle variant: *"Both produce the same diagnostic on cycle detection: a `CycleDetected { entities: Vec<EntityRef> }` variant of `Red { invalid }`."* The current implementation:

1. Loses structure: downstream consumers (the agent prompt layer, the Observatory) must parse a `Debug`-formatted string to recover the cycle path. The path entries are quoted with embedded backslash-escape sequences (`"\\\"ubo-1\\\""` in the JSON-serialised form), which makes parsing fragile.
2. Violates I6: "Bare 'predicate failed' is a defect." The cycle detail is technically present but not in a form a caller can reason about.
3. Couples cycle detection to string parsing internally: `has_cycle` performs a `starts_with("CycleDetected")` probe on `InvalidFact.reason` to know whether a cycle was already detected for a given set. That's a control-flow concern flowing through what is supposed to be a diagnostic data channel.

The test `cyclic_ubo_chain_is_detected_and_reported_as_invalid_fact` asserts on `invalid[0].reason.starts_with("CycleDetected")` — the test itself adopts the string-prefix anti-pattern.

**Disposition (suggested):** introduce a typed `InvalidFact` variant — either an enum (`InvalidFact::CycleDetected { entities: Vec<String> }` plus the existing `InvalidFact::AttrFailed { entity, reason }`) or an additional `kind: InvalidKind` field. Replace the `format!` push and the `has_cycle` string probe with structured checks. Update the test to assert on the structured variant's contents.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P2-002 — `AwaitingCompleteness` and `Discretionary` `GreenWhenStatus` variants are unreachable

**File:** `rust/crates/dsl-core/src/frontier/mod.rs`, `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** mod.rs:40–41 (variants); hydrator.rs:67–110 (`evaluate_destination` returns only `Green` or `Red`)
**Spec reference:** §3.2 (`GreenWhenStatus` enum with four variants); §5.2 (open-closure → `AwaitingCompleteness`); §3.5 + I4 (Discretionary destinations are always offered).

**Observation:** `evaluate_destination` returns `GreenWhenStatus::Green` or `GreenWhenStatus::Red { ... }`. Grepping the entire frontier module for `AwaitingCompleteness` or `Discretionary` outside the `mod.rs` definitions returns zero matches:

```
$ rg "AwaitingCompleteness|Discretionary" crates/dsl-core/src/frontier
crates/dsl-core/src/frontier/mod.rs:40:    AwaitingCompleteness(CompletenessAssertionStatus),
crates/dsl-core/src/frontier/mod.rs:41:    Discretionary(DiscretionaryReason),
crates/dsl-core/src/frontier/mod.rs:63:pub struct DiscretionaryReason {
```

`hydrator.rs` does not consult `slot.completeness_assertion`, `slot.role_guard`, `slot.justification_required`, or `slot.closure` (and therefore never fires the `AwaitingCompleteness` path that §5.2 prescribes for `open` slots).

**Issue:** §3.5 lists four user-visible destination categories. Without `AwaitingCompleteness` and `Discretionary`, the agent prompt layer cannot distinguish (a) a tollgate that is structurally green from a discretionary verb that is always offerable, or (b) an open slot where the aggregate is fine but the completeness assertion has expired. Both are first-class flows in the spec.

§5.2: *"For `open` slots, the hydrator computes aggregates over the known population AND evaluates the slot's completeness assertion. The frontier carries `AwaitingCompleteness` if the assertion has expired or never been authored; `Green` if assertion is fresh and aggregates pass; `Red` if an aggregate fails regardless of assertion freshness."* None of this branching exists in `evaluate_destination`.

§3.5: discretionary destinations are "always offered as escape hatches." With no `Discretionary` emission, the agent has no way to surface I4-flavoured verbs through the Frontier.

**Disposition (suggested):** in `evaluate_destination`, branch on:
1. The destination's "discretionary" classification (likely keyed off the verb's flavour annotation produced in Phase 7) — return `GreenWhenStatus::Discretionary` with a reason.
2. `slot.closure == Some(ClosureType::Open)` — combine the aggregate evaluation with `slot.completeness_assertion` freshness, returning `AwaitingCompleteness` per §5.2's truth table.

Add tests that author an `Open` slot with a stale completeness assertion and assert the `AwaitingCompleteness` variant; author a discretionary destination and assert `Discretionary`.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P2-003 — Recursive walk has no `max_depth` enforcement

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 311–376 (`set_facts`, `collect_recursive_set`)
**Spec reference:** §3.2 (sub-100ms target for depth-3 CBUs); §5.1 (bounded structures eager-walk); §3.3 schema (`max_depth: Option<usize>`); I7 (recursion bottoms out).

**Observation:** `collect_recursive_set` recurses on `parent_id` linkage with no depth bound:

```rust
fn collect_recursive_set(
    kind: &str,
    parent_id: &str,
    facts: &[FrontierFact],
    path: &mut Vec<String>,
    out: &mut Vec<FrontierFact>,
    invalid: &mut Vec<InvalidFact>,
) {
    for fact in facts.iter().filter(|fact| {
        fact.attrs.get("parent_id").is_some_and(|value| value == parent_id)
    }) {
        // ... cycle check on `path` ...
        out.push(fact.clone());
        path.push(id.clone());
        collect_recursive_set(kind, &id, facts, path, out, invalid);
        path.pop();
    }
}
```

`slot.max_depth: Option<usize>` is on `ResolvedSlot` but never consulted. Grep:

```
$ rg "max_depth" crates/dsl-core/src/frontier
(no results)
```

**Issue:** I7 says recursion must "bottom out and detect cycles." Cycle detection is present (via `path: Vec<String>`), but linear depth is unbounded. A pathological fact tree without cycles but very deep (e.g., 10,000-level natural-person ownership chain — unrealistic but unbounded) will recurse to that depth, exhausting the stack.

§5.1's sub-100ms target depends on bounded depth; the Lux SICAV pilot has `max_depth: Some(10)` authored on the recursive UBO slot (per `frontier_recursive.rs:45`). The schema field exists, the test fixture sets it, the implementation ignores it.

The cycle-detection mechanism guards revisits but not depth; a deep acyclic chain is not a cycle, so the visited-set protection does not trigger.

**Disposition (suggested):** thread `max_depth: Option<usize>` from `ResolvedSlot` into `set_facts` / `collect_recursive_set`. On exceeding `max_depth`, emit a structured `InvalidFact::MaxDepthExceeded { kind, depth: usize }` (or analogous typed variant from P2-001) and stop descending.

---

#### [MUST-FIX] [QUALITY] [COMPLIANCE] P2-004 — Bare "predicate evaluated false" diagnostic violates I6

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 90–95 (fallback push), 178–197 (`Predicate::Count`), 155–161 (`Predicate::NoneExists`)
**Spec reference:** I6 ("Bare 'predicate failed' is a defect"); §3.5 (the agent's next-turn reasoning depends on knowing *which* fact prevented green).

**Observation:**

```rust
if eval_predicate(&predicate, &mut ctx, None) {
    GreenWhenStatus::Green
} else {
    if ctx.missing.is_empty() && ctx.invalid.is_empty() {
        ctx.invalid.push(InvalidFact {
            entity: "predicate".to_string(),
            reason: "predicate evaluated false".to_string(),
        });
    }
    GreenWhenStatus::Red {
        missing: ctx.missing,
        invalid: ctx.invalid,
    }
}
```

The fallback pushes `InvalidFact { entity: "predicate", reason: "predicate evaluated false" }` — exactly the I6 anti-pattern.

When does the fallback fire? When the top-level predicate returned `false` but no sub-evaluator pushed missing/invalid. Tracing:

- `Predicate::Count` (lines 178–197) returns `cmp_u64(count, *op, *threshold)` with no push to missing/invalid on failure. A count predicate failing the threshold goes to the fallback.
- `Predicate::NoneExists` (lines 155–161): returns false either via `has_cycle` or because `facts.iter().all(|fact| !eval_predicate(condition, ...))` returned false (i.e., some fact matched the inner condition — which is the failure semantics of `NoneExists`). Inner-predicate evaluation pushes missing/invalid for ITS OWN failure detection — but a NoneExists failure means the inner predicate SUCCEEDED, not failed. So the inner pushes are not generated, and the NoneExists failure goes to the fallback.
- `Predicate::And` if a child returns false but the child pushed nothing.

**Issue:** the bare-fallback's existence is a code smell — every predicate variant should push structured detail on its failure path. Two of the predicate variants (`Count`, `NoneExists`) have known paths into the fallback, producing useless `entity: "predicate"` / `reason: "predicate evaluated false"` outputs.

The agent's next-turn reasoning depends on diagnostic detail. A red CBU with a single InvalidFact saying "predicate evaluated false" tells the agent nothing about which UBO chain or which evidence is the problem.

**Disposition (suggested):**
- `Predicate::Count` should push `InvalidFact::CountThresholdFailed { kind, observed: u64, op, threshold }` on failure.
- `Predicate::NoneExists` should track which fact(s) matched the inner condition (which is the violation case) and push `InvalidFact::ForbiddenMemberPresent { kind, fact_id }`.
- The fallback should be replaced by a `debug_assert!` — reaching it means a predicate variant didn't push, which is now a bug to fix at the source rather than mask.

---

### SHOULD-FIX

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P2-005 — The Frontier hydrator does not read substrate; facts are pushed in by the caller

**File:** `rust/crates/dsl-core/src/frontier/mod.rs`, `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** mod.rs:8–14 (`EntityRef.facts`); hydrator.rs:15 ("Synthetic fact set used by the Phase 3 skeleton hydrator")
**Spec reference:** §3.2 (compute-on-read, sub-100ms target, recursive walks compile to SQL); §4.2 (`WITH RECURSIVE`); §4.3 (PG14+ `CYCLE` clause); I8.

**Observation:**

```rust
pub struct EntityRef {
    pub slot_id: String,
    pub entity_id: String,
    pub current_state: String,
    pub facts: FrontierFacts,
}

pub type FrontierFacts = BTreeMap<String, Vec<FrontierFact>>;

pub struct FrontierFact {
    pub state: Option<String>,
    pub attrs: BTreeMap<String, String>,
}
```

The caller of `hydrate_frontier` provides `EntityRef.facts` as a pre-computed `BTreeMap`. The hydrator does not consult any substrate (no `sqlx`, no `&PgConnection`, no cache). Recursive walks are in-memory pointer-following (`parent_id` attribute in synthetic facts), not SQL.

**Issue:** §3.2 explicitly defines the Frontier as "the answer to 'what's the entity's state and what's reachable from it, against current substrate?' — recomputed on every fetch (I8)." The current implementation has no substrate-reading capability — it is a fact-evaluation engine over a caller-supplied dictionary. Phase 3's framing in §10 calls it a "skeleton," which is consistent with this gap, but the implication is that several spec-mandated properties cannot be tested today:

- "Recursive walks compile to SQL" (§3.2) — does not, in-memory recursion only.
- "Sub-100ms target" (§3.2) — un-measurable; depends entirely on caller's fact dictionary size.
- "PG14+ `CYCLE` clause OR visited-set CTE" (§4.3) — visited-set is implemented, but in Rust, not SQL.
- "Compute-on-read" (I8) — true tautologically (no caching), but only against synthetic facts.

The substantive risk: when the Phase 4/5/6 work lands the substrate path, the hydrator's predicate-evaluator will need to be either swapped wholesale or kept in parallel, because in-memory predicate evaluation against `BTreeMap<String, Vec<FrontierFact>>` is not a semantic match for `WITH RECURSIVE` SQL aggregates. The current code's design (pull facts from a generic dictionary, recurse via `parent_id` magic-string heuristic) does not extend cleanly to SQL aggregate compilation.

**Disposition (suggested):** acknowledge in the doc-comment of `hydrate_frontier` that this is the synthetic-fact skeleton and that substrate hydration is deferred to a downstream phase. More substantively, design the Phase 4/5/6 substrate path now (even if not implemented) so the predicate evaluator's surface aligns with what `WITH RECURSIVE` will produce — otherwise the skeleton's assumptions will be load-bearing on the next phase's API. Specifically: `set_facts` should not be the abstraction; the abstraction should be a query interface that the in-memory evaluator and a SQL evaluator can both implement.

---

#### [SHOULD-FIX] [QUALITY] P2-006 — `set_facts` uses the magic-string `parent_id` attribute to detect "is recursive"

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 311–333

**Observation:**

```rust
fn set_facts(set: &EntitySetRef, ctx: &mut EvalContext<'_>) -> Vec<FrontierFact> {
    let Some(facts) = ctx.facts.get(&set.kind) else {
        return Vec::new();
    };
    if !facts
        .iter()
        .any(|fact| fact.attrs.contains_key("parent_id"))
    {
        return facts.clone();
    }
    // ... recursive walk
}
```

Whether to walk recursively is decided by inspecting facts for a hardcoded attribute name `"parent_id"`. The `EntitySetRef.scope: Option<RelationScope>` and the slot's `cardinality: Option<Cardinality>` are not consulted.

**Issue:** the recursion choice should come from authored data — either the slot's `cardinality: Recursive` or an explicit predicate-AST signal — not a magic-string probe on caller-supplied attributes. If a non-recursive slot's facts happen to carry an unrelated `parent_id` attribute, the hydrator silently flips into recursive mode. If a recursive slot's facts use a different attribute name (`parent_uuid`, `owner_id`, `report_to`), the hydrator silently treats it as flat.

This is a Phase 3 substrate-shim that should be replaced. Note that no shape-rule or constellation-map data names a parent-id column; the `"parent_id"` string is invented by the test fixtures.

**Disposition (suggested):** replace the heuristic with an explicit signal — e.g., `EntitySetRef` could carry a `recursion: Option<RecursionConfig>` field, populated by the parser when the predicate uses recursive forms. The slot's `cardinality::Recursive { parent_attr: String }` (or equivalent) names the attribute. Keep the magic-string fallback only for the in-memory test fixtures, behind a debug-only path.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P2-007 — `collect_recursive_set` flattens the chain; nested predicate evaluation per node is lost

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 322–333 (caller flattens `out: Vec<FrontierFact>`), 341–376 (collector accumulates flat list)
**Spec reference:** §4.2 (UBO recursive `green_when` evaluates per-node, with branching on `corporate_kind` vs `natural_person_kind`).

**Observation:** the recursive collector accumulates all reachable descendants into one flat `Vec<FrontierFact>`; the caller (`Predicate::Every`) iterates that flat list with the inner condition:

```rust
Predicate::Every { set, condition } => {
    let facts = set_facts(set, ctx);
    // ...
    facts.iter().all(|fact| eval_predicate(condition, ctx, Some(fact)))
}
```

So `every direct_owner.kyc.green_when ∈ {KYC_APPROVED}` becomes "for every node anywhere in the recursive descent, the inner condition holds." The structural recursion in §4.2 — *"for each direct_owner with corporate_kind: direct_owner.ubo_chain.RESOLVED.green_when (recursion); for each direct_owner with natural_person_kind: direct_owner.kyc.green_when ∈ {KYC_APPROVED}"* — is collapsed into a single uniform condition.

**Issue:** §4.2's intent is per-node-typed predicate evaluation: corporate-kind owners are recursively validated against their own `ubo_chain.RESOLVED.green_when`; natural-person owners are validated against KYC. The flattening loses the kind-conditional branching. It happens to give the right answer for the simple test fixture (`every ubo.state = VERIFIED` — one uniform condition), but does not implement §4.2 faithfully.

When real UBO data with mixed natural-person and corporate descendants is hydrated, the current implementation cannot express "regulated funds are acceptable terminals" or "corporate UBO chains must themselves resolve."

**Disposition (suggested):** the predicate AST already has `Predicate::Obtained { entity, validity }`; the recursion case should compose nested `Predicate::Every` calls per-node-kind. Likely needs a richer set descriptor (kind-conditioned subsets) or a `Predicate::Recurse` variant. Defer until the Phase 4/5/6 substrate path lands, but document the gap so it isn't silently inherited.

---

#### [SHOULD-FIX] [QUALITY] P2-008 — Hydrator does not branch on `slot.closure`; closed_unbounded slots are silently iterated

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 311–376 (`set_facts` does not consult closure)
**Spec reference:** I3 (closure governs predicate authoring discipline); §5.2 (aggregate-only for unbounded).

**Observation:** `set_facts` calls `ctx.facts.get(&set.kind)` regardless of `slot.closure`. There is no path that handles `closed_unbounded` differently from `closed_bounded`; both eagerly iterate the (caller-supplied) fact dictionary.

**Issue:** I3 says "the runtime evaluator never iterates an unbounded or open child population in user-space code; aggregate predicates compile to SQL `NOT EXISTS` / `COUNT(*) FILTER (...)`." The current code violates the directive — though only because it's running against synthetic facts where "iterating the dictionary" is cheap. When the substrate path lands, blindly iterating an unbounded population would be the cardinality bomb I3 names.

This is partly a Pass 3 concern (the validator enforces predicate authoring discipline by closure type). For Pass 2: the hydrator should at minimum branch on `slot.closure` and refuse to `set_facts` for `closed_unbounded` / `open` slots in user-space iteration paths — instead routing through aggregate forms. Today, no such branching exists; the closure metadata is ignored entirely by the hydrator.

**Disposition (suggested):** add closure-aware branching in `set_facts`: for `closed_bounded`, iterate as today; for `closed_unbounded` and `open`, refuse to materialise and require aggregate predicates only. Combined with predicate-author lint (Pass 3), this enforces I3 at both author time and run time.

---

#### [SHOULD-FIX] [QUALITY] [COMPLIANCE] P2-009 — `via_verb` is `Option<String>`, not `VerbFqn`; transitions without verbs surface as `None` reachable destinations

**File:** `rust/crates/dsl-core/src/frontier/mod.rs`, `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** mod.rs:24–30; hydrator.rs:39–54
**Spec reference:** §3.2 (`ReachableDestination { destination_state, via_verb: VerbFqn, status }`).

**Observation:** spec defines:

```rust
struct ReachableDestination {
    destination_state: StateRef,
    via_verb:          VerbFqn,
    status:            GreenWhenStatus,
}
```

Code:

```rust
pub struct ReachableDestination {
    pub destination_state: String,
    pub via_verb: Option<String>,
    pub status: GreenWhenStatus,
}
```

`via_verb` is optional because the upstream `ResolvedTransition.via` is `Option<String>` (which itself comes from `TransitionDef.via: Option<YamlValue>`). Transitions authored without a `via:` field surface in the Frontier with `via_verb: None`.

**Issue:** spec's contract is that a `ReachableDestination` is reachable *via* a specific verb; the agent fires that verb to make the transition. A destination with `via_verb: None` is, semantically, unreachable from the agent's perspective — it cannot be fired. Surfacing such destinations in `frontier.reachable` is misleading; the agent prompt would offer the destination and the agent could not act on it.

If transitions without `via:` exist in authored YAMLs (automatic / system-driven transitions), the Frontier should either filter them out or carry a different status (e.g., `GreenWhenStatus::SystemDriven`) — not surface a `None` verb the agent can't invoke.

**Disposition (suggested):** in `hydrate_frontier`, filter out transitions where `transition.via` is `None`, OR define a typed destination kind that distinguishes agent-fireable from system-driven transitions. Match spec's `via_verb: VerbFqn` (non-optional) on the agent-fireable case.

---

### CONSIDER

#### [CONSIDER] [STYLE] P2-010 — `EntityRef::Parent` and `EntityRef::Scoped` are evaluated identically to `EntityRef::Named`

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 212–225, 247–272, 289–298
**Spec reference:** predicate AST design (Pass 3 territory); §3.2 implicit.

**Observation:** every match arm collapses the three named variants:

```rust
PredicateEntityRef::Named(kind)
| PredicateEntityRef::Parent(kind)
| PredicateEntityRef::Scoped { kind, .. } => {
    // identical handling for all three
}
```

The semantic distinctions — `Parent` (the predicate's hosting entity), `Scoped { scope }` (scope-qualified) — are erased. The `scope` payload of `Scoped` is unused (`{ kind, .. }`).

**Issue:** for the synthetic-fact evaluator, this is acceptable as a placeholder. For the substrate path, scope-qualified lookups (e.g., "evidence attached_to this UBO") are essential to correctness. The current code silently drops the scope.

**Disposition (suggested):** comment that scope handling is a Phase 4+ concern, or implement a stub that returns an `InvalidFact { reason: "scoped lookups not yet implemented" }` instead of silently behaving like `Named`. Either makes the gap visible.

---

#### [CONSIDER] [STYLE] P2-011 — `cmp_string_or_number` falls back to identical comparators for both string and number paths

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 389–408

**Observation:**

```rust
fn cmp_string_or_number(left: &str, op: CmpOp, right: &str) -> bool {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left), Ok(right)) => match op {
            CmpOp::Eq => left == right,
            // ... 5 more arms
        },
        _ => match op {
            CmpOp::Eq => left == right,
            // ... 5 more arms (now comparing strings)
        },
    }
}
```

Both branches enumerate all six `CmpOp` arms; the upper branch compares `f64`, the lower compares `&str`. For `Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge` on strings, lexicographic comparison is used silently.

**Issue:** number-vs-string fallback is implicit. If only one side parses as a number (e.g., comparing `"100"` to `"NA"`), both fall to lexicographic. The function name suggests intent ("string or number") but the silent fallback can produce surprising results — comparing a numeric attribute to a non-numeric expected value silently lex-compares.

**Disposition (suggested):** at minimum, push a structured note to invalid when the comparison is non-strict (mixed types). A full fix would type the attribute values at parse time so the evaluator knows whether numeric comparison applies.

---

#### [CONSIDER] [STYLE] P2-012 — `eval_exists` for `EntityRef::This` returns true when `current_state` is non-empty

**File:** `rust/crates/dsl-core/src/frontier/hydrator.rs`
**Lines:** 211

**Observation:**

```rust
PredicateEntityRef::This => this_fact.is_some() || !ctx.current_state.is_empty(),
```

`Exists this` returns true if the current entity has a non-empty `current_state`. That's a proxy for "this entity exists at all" (which is presumably always true at hydration time — you can't hydrate the frontier of a nonexistent entity).

**Issue:** the `current_state.is_empty()` test is a string-emptiness check, not a true existence check. If `current_state` is `""` for some reason, `exists this` returns false — which conflates state-emptiness with existence. The semantics of "does this entity exist" should not depend on the formatting of its current state.

**Disposition (suggested):** return `true` unconditionally for `Exists This` (the entity always exists during its own frontier hydration), or reframe the check to whatever invariant is being protected.

---

### NOTE

#### [NOTE] P2-013 — `current_state` is duplicated on `InstanceFrontier` and on the embedded `EntityRef`

**File:** `rust/crates/dsl-core/src/frontier/mod.rs`
**Lines:** 8–22

**Observation:**

```rust
pub struct EntityRef {
    pub slot_id: String,
    pub entity_id: String,
    pub current_state: String,
    pub facts: FrontierFacts,
}

pub struct InstanceFrontier {
    pub entity_ref: EntityRef,
    pub current_state: String,
    pub reachable: Vec<ReachableDestination>,
}
```

`InstanceFrontier.current_state` and `InstanceFrontier.entity_ref.current_state` are both populated from the same source (`hydrator.rs:57–58`). The spec carries both fields too (§3.2 `struct InstanceFrontier { entity_ref, current_state, reachable }`), so this is spec-consistent; nonetheless, two copies of the same value can drift.

**Disposition (suggested):** if spec compatibility allows, drop the top-level `current_state` and read it through `entity_ref.current_state`. If spec compatibility requires both, add a debug assertion that they match.

---

## Coverage notes

**What this pass covered:**
- `frontier/mod.rs` and `frontier/hydrator.rs` end-to-end.
- §3.2 (frontier shape, `ReachableDestination`, `GreenWhenStatus`), §4.2 (UBO recursive case), §4.3 (cycle detection), §5.1 / §5.2 (bounded vs unbounded vs open closure), I6 (diagnostic completeness), I7 (recursion bottoms out + cycles), I8 (compute-on-read).
- Predicate-AST consumption (the evaluator branches on `Predicate::And/Exists/StateIn/AttrCmp/Every/NoneExists/AtLeastOne/Count/Obtained` — full AST coverage, though note that `Predicate::Obtained::Validity::DelegatedToEntityDag` is implemented as a bare `eval_exists`, which is a forward-compatible stub).
- 7 frontier tests run (`frontier_skeleton.rs` × 4, `frontier_recursive.rs` × 3) — all pass.

**What this pass deliberately did not cover:**
- The predicate parser (`config/predicate/parser.rs`) — Pass 3 scope.
- The predicate AST design (`config/predicate/ast.rs`) — Pass 3 scope.
- DAG validator integration of green_when validation (`config/dag_validator.rs::validate_green_when_predicates` per §10.3) — Pass 3 scope.
- Substrate / SQL hydration — does not exist in this commit (Phase 4+).
- Cascade-on-write reactivity (§5.3) — Phase 9 deliverable, out of this commit's scope.

**Inconclusive / verified by code only:**
- I8 compute-on-read: tautologically true because no caching exists, but the synthetic-fact-evaluator framing means the substantive I8 property (substrate as authoritative) cannot be exercised. P2-005 documents the gap.
- I7 recursion-bottoms-out: cycle detection works for the in-memory case (verified by `cyclic_ubo_chain_is_detected_and_reported_as_invalid_fact`), but linear depth is unbounded (P2-003).
- §4.2 per-node typed predicate evaluation: verified absent by code reading; tests do not exercise mixed natural-person vs corporate-kind owners.

## Recommended next steps

In priority order, severity-ranked.

1. **MUST-FIX** P2-001 (CycleDetected as debug string): trivial structural fix; replaces three coupled hacks (push site, `has_cycle` probe, test assertion) with one typed variant.
2. **MUST-FIX** P2-002 (AwaitingCompleteness / Discretionary unreachable): the agent prompt layer downstream depends on these variants for I4 and §5.2 flows; landing them is required before the Frontier can be the agent's primary disclosure surface (§3.5).
3. **MUST-FIX** P2-004 (bare "predicate evaluated false"): fix `Predicate::Count` and `Predicate::NoneExists` failure paths to push structured detail; convert the fallback to a `debug_assert!`.
4. **MUST-FIX** P2-003 (max_depth not enforced): cheap to thread through; protects against pathological-depth fact trees.
5. **SHOULD-FIX** P2-005 (substrate path absent): document the synthetic-fact framing in `hydrate_frontier`'s doc comment; design the Phase 4/5/6 substrate API now so it doesn't constrain the in-memory evaluator's escape route later.
6. **SHOULD-FIX** P2-006 / P2-007 (parent_id heuristic + flat collection): both are Phase 3 substrate-shims that should be flagged for replacement when the substrate path lands.
7. **SHOULD-FIX** P2-008 (closure-aware iteration): I3's runtime invariant. Add the dispatch on `slot.closure` so cardinality bombs are statically prevented.
8. **SHOULD-FIX** P2-009 (`via_verb` optional): filter or distinguish system-driven transitions; do not surface them as agent-actionable.
9. **CONSIDER** P2-010 / P2-011 / P2-012 / **NOTE** P2-013: low-priority polish.
