# EOP-DD-KYCUBO-D2.0 — Check Scope and the Run Book
### The evaluation pack's foundation: what is in scope, what a run records, and what "cleared" rests on

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-D2.0 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | The D1/D2 split (ratified 2026-08-19); a pack is a capability (TS.6, ratified 2026-08-21); the four-segment scheme (ratified 2026-08-22); CTN-2e/2f/2h; TS.0 §2a compound assurance |
| **Domain** | **D2.** Tests the board and rules on the result. Writes only to run and decision records — never the fact stream. |
| **Status** | **RATIFIED 2026-08-22** on Q1 (applicability as a closed set of typed condition kinds) and Q3 (a run is UBO-group level). Build-ready. Q2 and Q4 are answerable inside the build; Q5 is noted and belongs elsewhere. |

---

## §1 What this is for

D1 is complete: the board of truth is assembled, geometry-enforced, and its determinations explain themselves. Nothing yet asks whether the board is **acceptable**.

**The founding property, ratified 2026-08-19:** evaluation runs against a board in **any** state. It needs no completeness gate, no readiness check, no orchestration. A board of three alleged edges is a legitimate input; the run simply fails, and *the failure is the work list*. That is why D1 came first and why nothing in D1 knows policy exists.

## §2 The boundary, restated because it is load-bearing

**Evaluation reads:** the constructed taxonomy at (T, axis, scope), its determination, its assurance profile, and their pins. **Never** raw assertions. **Evaluation writes:** run records and decision records. **Never** the fact stream — now true by construction, since `ob-poc-kyc-decide` has no path to an append in its dependency graph.

**An external screening or sanctions call is fact-gathering, not a check.** It is `kyc_ubo.assert.entity.screening` in the Assembly pack: screening was run, against provider P, on date D, with result R. The check then *assesses that recorded fact* and stays pure and reproducible. Without this split one check reaches outside, becomes non-reproducible, and purity is lost for the sake of a single verb.

## §3 Scope selection — the gap

Nothing in the system answers *which checks does this board need?* Today that question has no vocabulary at all.

**The model:** a check declares what makes it applicable; the in-scope set is **computed from the board**. This is the placement set one layer up — the board's contents determine the legal move set in D1, and determine the applicable check set in D2. A trust in the structure pulls in trust checks; a sovereign vehicle pulls in state-owned handling; a jurisdiction or risk rating pulls in others. Some checks are unconditional.

**RULED 2026-08-22 — a closed set of typed condition kinds, composed declaratively.**

A check declares its applicability from a **fixed vocabulary** of condition kinds, each implemented in typed Rust: *entity type present on the board*, *jurisdiction present*, *risk rating at or above*, *structure class present*, *unconditional*. The check's declaration composes them; the code defines what each means.

This is deliberately **not** an expression language: no arbitrary predicates, no parser, no runtime-authored logic. The trapdoor named in the 2026-08-18 review stays shut — a homegrown evaluator would put determinism and reproducibility on a hand-written interpreter rather than on the type system. Adding a *new kind* of condition is a code change and a ruling; adding a *check* that composes existing kinds is a declaration.

The rejected alternatives, recorded: applicability wholly in code (enforceable, but every policy tweak becomes a rebuild — wrong for the fast-changing half of the system), and free-form predicates (the interpreter trapdoor).

**One invariant regardless:** the in-scope set is **recomputed on every run**, never stored. A trust added today pulls in trust checks that were not in scope yesterday. The run book records the set *as computed at run time* (§4), so "the board grew and now needs three more checks" falls out of comparing that record against a fresh computation — with no separate staleness mechanism.

## §4 The run book

**Three verdicts, not two.**

| Verdict | Meaning |
|---|---|
| `pass` | the check was evaluated and satisfied |
| `fail` | the check was evaluated and violated — a finding, citing the facts it rests on |
| `unevaluable` | the check is in scope and **cannot yet be evaluated**, with the reason recorded |

**`unevaluable` is the work list.** A check whose facts are not present yet has not failed — collapsing it into `fail` loses the difference between *we checked and it is wrong* and *we cannot check yet*, which are different tasks for different people. This mirrors D1's own provisionality: the assurance profile already distinguishes alleged edge, alleged type, and admission decided on an alleged classification (TS.3 §2a), and those distinctions are exactly what an `unevaluable` reason should carry.

**A run record holds:** an identity; the **board state hash** (the constructed taxonomy at its T, axis and scope); the **evaluation pack version hash**; valid time and knowledge time; who or what triggered it; the **in-scope check set as computed**; and per check, a verdict plus findings citing the facts relied on.

**Three properties do the work:**

**Runs are append-only.** Re-running creates a new run. The old one stands as what was concluded then, from what was known then — the bitemporal law applied to conclusions rather than facts.

**The current work list is derived, never stored.** It is the failing and unevaluable findings of the latest run: a query, not a status field. Nothing to reconcile, nothing to go silently stale.

**Staleness is computed.** A run is stale when the board hash it pinned differs from the board's hash now. No flag, no job — and "re-run needed" falls out of comparing two hashes.

**Two clocks, independently.** Facts age (evidence passes its horizon, status demotes, the board hash moves) and policy changes (the evaluation pack version moves). Either triggers re-assessment on its own. Periodic review is the first clock ticking; a regulatory change is the second.

## §5 What this tranche absorbs

**The obligation dissolution** (TS.6 §5a, deferred pending this document):
- `kyc_ubo.assert.obligation.creation` — **dissolved**. Nobody hands over an obligation; you run the checks and they fail. The finding is the record.
- `kyc_ubo.assert.obligation.satisfaction` — **dissolved**. Nothing is satisfied; you assert the missing fact and re-run. The new run supersedes the old.
- `kyc_ubo.assert.obligation.waiver` — **moves to Evaluation** as `kyc_ubo.decide.obligation.waiver`. The one genuine act among the six: a human rules that a failing check does not apply, with reason and authority, citing the run.

**The obligation projection.** The outbox removal (2026-08-22) deleted the queue and drainers but left the obligation projection standing, noted as belonging here. It goes with obligations.

**Consequence to face rather than defer:** the assurance profile and the `decide` basis currently reference obligation-fold state. The reconciliation flagged that `decide_verbs_cite_their_basis` cites an obligation-fold snapshot rather than *a finding, a run, or a determination* — the three §8 named. Dissolving obligations forces that to be corrected, which is the right outcome: **a verdict cites the run it relied on.**

## §6 Gate tests (RED first)

- `checks_run_at_any_board_state` — the founding property: a board of three alleged edges over alleged types produces a run, with verdicts, and no error. Nothing gates on completeness.
- `in_scope_set_is_computed_not_stored` — adding a trust to the board changes the in-scope set on the next run, with no configuration change anywhere.
- `unevaluable_is_not_fail` — a check whose facts are absent returns `unevaluable` with a reason, never `fail`. Property: adding the missing fact and re-running flips it to pass or fail, never the reverse.
- `work_list_is_derived_from_latest_run` — structural: no stored work-list state exists; the list is a query over the newest run.
- `staleness_is_hash_comparison` — a run whose pinned board hash differs from the board's current hash reports stale, with no flag written anywhere.
- `runs_are_append_only` — re-running never mutates a prior run; the old verdicts stand.
- `run_pins_are_complete` — every run records board hash, pack version, both times, trigger, and the in-scope set. A run missing any pin is refused.
- `decide_cites_a_run` — replaces the current basis gate. A verdict cites a finding, a run, or a determination — and the gate must be **falsifiable**: prove an uncited verdict is refused, rather than relying on a NOT NULL column that no path can violate.
- `evaluation_never_writes_facts` — re-proven at this layer now that the pack has members that do real work.

Every gate proven able to fail: perturb, observe red, restore, observe green.

## §7 Open questions

**Q1 — RULED 2026-08-22 (§3).** A closed set of typed condition kinds, composed declaratively. Not an expression language.
**Q2 — the check catalogue's contents.** This document defines the *machinery*; what a sanctions, threshold or evidence-sufficiency check actually tests is a separate and large ruling surface. Confirm it stays out of scope here.
**Q3 — RULED 2026-08-22: a run is UBO-group level.** One run covers the whole group, pins one board hash at one moment, and carries findings tagged per subject. Consistent with KYC running group-level down (TS.1 §2b) and with clearance being a decision about a client relationship rather than an individual entity. It also removes a staleness-mismatch problem by construction: there is no case where subject A was checked this morning and subject B last week, so "is the group cleared?" never has to reconcile runs taken at different moments.
**Q4 — the listed-entity carve-out.** Ratified as D2 (TS.3 §8 Q3): regulated-market listing is a test *against* a built UBO, not part of building it. It lands here as a check, or as an applicability condition that removes checks from scope. Which?
**Q5 — the O(N²·17) placement enumeration.** Not this document's problem, but the same "computed on read" discipline now has a quadratic term in D1. Worth a bound before either surface meets a real-sized group.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-22 | Initial draft. Establishes the evaluation pack's foundation on the ratified property that checks run against a board in **any** state — no completeness gate, and the failure *is* the work list. §3 names the gap: scope selection had no vocabulary; a check declares applicability and the in-scope set is computed from the board, the placement set one layer up. §4's three verdicts make `unevaluable` first-class — a check whose facts are absent has not failed, and collapsing it into `fail` loses the distinction between a finding and a task. The run book is append-only with board hash and pack version pinned; the work list and staleness are both **derived**, never stored, giving two independent clocks — facts aging and policy changing. §5 absorbs the obligation dissolution and forces the `decide` basis to cite a run rather than an obligation-fold snapshot. Q1 (applicability as data or code) blocks the build; the recommendation is a small closed set of typed condition kinds, deliberately not an expression language. |
