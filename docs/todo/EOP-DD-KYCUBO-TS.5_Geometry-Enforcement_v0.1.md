# EOP-DD-KYCUBO-TS.5 — Geometry Enforcement
### Wiring the type→linkage matrix into the chokepoint that actually gates writes

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-TS.5 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-DD-KYCUBO-TS.1 (RATIFIED, v0.3) §2/§2a — the matrix this enforces; TS.0 §2a source rules; TS.3 §2a provisionality; the geometry-enforcement investigation of 2026-08-21 |
| **Domain** | **D1.** |
| **Status** | DRAFT. §3 is the ruling surface. Grandfathering is NOT a question — this is a POC with test data only (ruled 2026-08-21). |

---

## §1 The finding this closes

Proven by live execution against Postgres on 2026-08-21: **the type geometry is enforced nowhere on the write path.** An illegal `ManagementMandate` edge sourced from a natural person was appended through the real governed op — new event, `seq: 2` — and rolled back only because the probe rolled it back. Four further source restrictions (officer, statutory, containment, employment) admitted illegal sources identically.

**Zero of seventeen pipes are geometry-enforced.** `check_preconditions` — the sole function used by *both* the DB-append chokepoint and the placement preview — never calls `geometry::check_type_geometry`, and no `Precondition` variant is geometry-aware. The checker structurally cannot see the geometry module.

`check_type_geometry` has exactly two non-test call sites, and neither rejects an assertion:
- `edges_invalidated_by_correction` — a **post-hoc** cascade, run only when a type is corrected, deciding which *existing* edges become invalid.
- `placement.rs:207` `geometrically_possible` — an **existence-only** gate asking whether *any* legal triple exists among typed members anywhere. It never inspects the caller's actual from/to/kind.

And `placement.rs:37`'s module doc names a function `type_geometry_gate` that **has never existed** — documentation claiming an enforcement that was never built.

## §2 Why it survived TS.1's gates

TS.1 shipped `type_geometry_refuses_impossible_linkage` and it passes honestly — because it calls the geometry checker **directly**. It proves the function is correct. It says nothing about whether anything calls it.

This is the fourth appearance of one family, and naming the fourth member is the durable output of this document:

| Shape | Instance |
|---|---|
| Declared but not enforced | `cross_slot_constraints` — 49 rules, zero consumers |
| Enforced but not declared | nine hand-written `enforce_*` predicates |
| Claimed but delegated | `FundControlStrategy`, `NomineePierceStrategy` — thin delegates |
| **Built but not wired** | **the type geometry — correct, tested, and reachable from nothing that gates a write** |

## §3 The rulings

**R1 — Geometry becomes a precondition.** `check_preconditions` already receives `&TypeRegistryState`, and the event carries from/to/kind. So the rule is directly expressible: a new `Precondition` variant looks up both endpoint types and calls `check_type_geometry`. Attached to `ubo.edge.assert-control` and `ubo.edge.assert-economic-interest`. **One variant, one chokepoint — both the append path and the preview covered by construction**, because both already route through this function.

**R2 — The preview gate becomes the real check.** `geometrically_possible`'s existence-only test is replaced by the per-triple check. Leaving it coarse would recreate exactly the preview-versus-reality divergence T4's frontier design exists to prevent: a board offering a move the append then refuses.

**R3 — No grandfathering, no migration, no legacy flag.** This is a POC with test data. Edges that violate the geometry are wrong; the dev database is reseeded. There is no assurance-profile "would not be assertable today" marker, because there is no history worth preserving.

**R4 — Fixture breakage is a finding before it is a fix.** Enforcement will break fixtures built on assumptions the geometry forbids. Each break is reported with what it assumed *before* it is corrected — a fixture that violates ratified geometry is evidence about what someone believed, and that is worth reading rather than silently patching.

**R5 — Provisionality applies unchanged.** Geometry is checked against entity types, which may be **alleged** (TS.0 §2a, TS.3 §2a). An assertion admitted on the basis of an alleged type is admitted *provisionally* — the edge exists, and the determination that later traverses it carries the weakest status it rested on. Enforcement narrows what may be asserted; it does not narrow what may be *alleged*, and it must not gate on proof (CTN-2e: record freely, conclude carefully).

**R6 — Untyped endpoints.** Where an endpoint has no type asserted at all, the geometry cannot be evaluated. **Ruling: admit, provisionally, and record that geometry was unevaluable.** Refusing would break the ratified opening state, where entities exist before their types are proved and links are asserted from client statements. This is the one place R1 must not fail closed, and the reason is CTN-2e, not convenience.

## §4 The wired-not-just-built tooth pattern

The durable output of this document is a **test pattern**, applied beyond this tranche.

**The rule: every rule component gets a test that drives it through the real chokepoint, not through the function directly.** A unit test on a rule function proves the rule is correct. It cannot prove anything calls it. Both tests are worth having; only the second one catches this defect class.

**Concretely, per rule component:**
1. A *correctness* test on the function in isolation (what TS.1 had).
2. A ***wiring*** test that constructs an input the rule must refuse, drives it through the **production entry point** — the governed op, the append path, whatever actually gates writes — and asserts refusal. If a component has no such entry point, that absence is the finding.

**Tooth for this tranche:** `every_geometry_rule_is_reachable_from_the_write_path` — for a representative illegal triple per source restriction in TS.1 §2a, drive the real `assert-control` path and assert refusal. RED today for all of them; green when R1 lands; and it stays as the permanent guard against the wiring being lost again.

**Retrospective application, recommended not required:** the same question is worth asking of every other rule component in the pack — do the studs, the admission classes, and the closure rules each have a wiring test, or only a correctness test? That sweep is a separate small tranche and should not ride along inside this one.

## §5 Gate tests (RED first)

- `illegal_source_type_is_refused_at_the_op` — the headline, and the exact case proven admissible on 2026-08-21: a `ManagementMandate` edge sourced from a natural person is **refused by the governed op**, live, not merely by a direct call to the geometry function.
- `every_geometry_rule_is_reachable_from_the_write_path` — §4's tooth, one illegal triple per source restriction.
- `illegal_target_type_is_refused` — the target side of the matrix (TS.1 §2), not only §2a's source side.
- `preview_and_append_agree` — property: for a sample of triples, `enumerate_placement_set` offers a move **iff** the append would admit it. This is the R2 gate and the one that keeps the board honest.
- `untyped_endpoint_admits_provisionally` — R6: an endpoint with no asserted type admits, records geometry-unevaluable, and the determination that traverses it is provisional.
- `alleged_type_admits_provisionally` — R5: admitted on an alleged type, recorded as provisional, weakest-link status propagates.
- `geometry_refusal_is_distinguishable_from_stud_refusal` — the two constraint layers (TS.1 §1) must produce distinct errors: "not a move" versus "not legal here". Callers and the REPL surface depend on telling them apart.
- `type_correction_still_cascades` — regression: `edges_invalidated_by_correction` keeps working; R1 adds a call site, it does not move the existing one.

Every gate proven able to fail: perturb, observe red, restore, observe green.

## §6 Open questions

**Q1 — Economic pipes.** R1 attaches to `assert-control` and `assert-economic-interest`. Confirm the economic verb needs the same treatment — the matrix constrains economic pipes (2, 4, 15) as well as control ones, so the answer is presumably yes, but it doubles the fixture surface.
**Q2 — Other assertion verbs.** Do any other verbs create edges? If `supersede` or `attach-evidence` can introduce or alter an edge's kind or endpoints, they need the same gate. Recon question, answered in the build's Phase 0.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-21 | Initial draft, closing the largest finding of the programme: the type geometry is enforced nowhere on the write path — proven by appending an illegal edge live against Postgres. Zero of seventeen pipes gated. Rulings: geometry becomes a `Precondition` so the single chokepoint covers both append and preview (R1); the existence-only preview gate becomes the real per-triple check (R2); no grandfathering — POC with test data (R3); fixture breakage is reported before it is fixed (R4); provisionality unchanged, and untyped endpoints admit provisionally rather than failing closed, because CTN-2e forbids gating records on proof (R5/R6). §4 names the defect family's fourth member — **built but not wired** — and gives the durable countermeasure: every rule component needs a wiring test through the production entry point, not only a correctness test on the function. |
