# EOP-DD-KYCUBO-D2.1 — The Evaluation Engine
### The machinery D2.0 described and did not build — and a rule about the words "out of scope"

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-D2.1 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-DD-KYCUBO-D2.0 (RATIFIED — the run book, three verdicts, applicability); the 2026-08-23 reconciliation |
| **Status** | DRAFT. §2's scope line and §3's deferral rule are the ruling surfaces. |

---

## §1 What happened

D2.0 shipped a run book that cannot record a conclusion. Verified in source:

```rust
pub trait Check {
    fn check_id(&self) -> &str;
    fn applicability(&self) -> &[ApplicabilityCondition];
}                                   // there is no evaluate()

let run = EvaluationRun::new(Uuid::new_v4(), pins, vec![]);   // findings, literal
.bind(serde_json::json!([]))                                  // findings, persisted
```

`Finding` and `UnevaluableReason` do not derive `Serialize`, so no finding could reach the column even if one were constructed. Zero runs in the database carry findings or an in-scope set.

**The consequence, proven by execution:** the K-23 approval gate reads `if !run.work_list().is_empty()` over findings that are always empty. **A subject with zero recorded events was approved.** That is the "gate that cannot fail" defect, reproduced inside the tranche whose predecessor's reconciliation existed to kill it.

**The cause is a scoping error, and it was mine.** D2.0 §7 Q2 ruled the check catalogue's *contents* out of scope — what a sanctions or threshold check tests. Correct. But `evaluate()` is **machinery, not content**, and nothing distinguished them. An executor building faithfully to that scope produces exactly what landed: pins, hashes, append-only shape, staleness, derivation discipline — a correct envelope with no mechanism to fill it.

## §2 The scope line, drawn properly this time

**IN — the machinery. All of it.**
- `evaluate(&self, board: &BoardSnapshot) -> Verdict` on the `Check` trait. A check that cannot be run is not a check.
- `Serialize` on `Finding`, `UnevaluableReason` and everything they reach, so a verdict can be persisted.
- A real catalogue type the run consults, replacing the literal `&[]`.
- The persist path writing actual findings and the actual in-scope set, replacing `json!([])`.
- **One real check, end to end** — as the proof the machinery works, not as catalogue content. Recommended: *every entity on the board has a proven type*. It reads only the board, needs no compliance input, and exercises all three verdicts naturally (pass, fail, and unevaluable where a type is alleged).

**OUT — the catalogue's contents.** What a sanctions, threshold, jurisdiction or evidence-sufficiency check tests. That needs compliance input, changes constantly, and is the fast-changing half the pack split exists to isolate. **Unchanged from D2.0 Q2, and still correct.**

**The test that separates them:** if it is *how a check runs*, it is machinery and it is in scope. If it is *what a particular check concludes about BNY's obligations*, it is content and it is out.

## §3 The deferral rule — because "out of scope" has been doing too much work

Across this programme, *out of scope*, *deferred* and *blocked on the next tranche* have accumulated, and the reconciliations found some of them marked LANDED. A deferral that names nothing is indistinguishable from a job not done.

**RULE: a deferral is only valid if it states four things.**

| | |
|---|---|
| **What** | the specific thing not being built, precisely enough that its absence is testable |
| **Why** | the reason it cannot be built now — a dependency, a ruling, or an input from outside |
| **Who** | whose decision or work unblocks it |
| **When** | the tranche or condition that picks it up |

**A deferral missing any of the four is not a deferral. It is unfinished work, and it is recorded as such.**

**And two consequences that make the rule bite:**

**A deferred item is never reported as LANDED.** The 2026-08-22 reconciliation found the three fact relocations marked landed and not done, with the deliverable redefined to match what was built. That is the specific failure this rule exists to prevent.

**Every tranche's close re-states its open deferrals** in the state-of-play, with the four fields. A deferral that survives three tranches without its *when* arriving is escalated as a finding, not carried a fourth time.

## §4 The currently-parked register

Everything presently deferred across the programme, with the four fields — or marked as failing the rule.

| Item | Why | Who | When |
|---|---|---|---|
| Check catalogue contents | needs compliance input, jurisdiction by jurisdiction | Adam + compliance | after D2.1; own workstream |
| O(N²·17) placement enumeration bound | correctness landed first, deliberately | Adam (design choice on board semantics) | before a real-sized group |
| `dsl-core` two-segment limit (108 verbs, ten domains) | platform-wide, not KYC | platform owner | own ticket; first question is whether KYC text reaches it |
| `CLAUDE.md` stale verb names | ratified-doc exclusion during the rename | Adam | any time; small |
| Share-class fact layer | genuine model extension | Adam | own tranche; blocks the `assert-control` split |
| `assert-control` / `assert-economic-interest` split | cannot remove `VotingRights` before voting power is derivable | — | after share-class layer |
| Streams C/D on the tree | another workstream's work | Adam | when that stream's owner commits |

**Failing the rule — reclassified as unfinished work, not deferrals:**

| Item | Status |
|---|---|
| Obligation projection tables (6 + 93 rows, no DROP migration) | §5 said "goes with obligations". The projector went; the tables did not. **NOT DONE** — one migration |
| `evaluation_never_writes_facts` | disproved by probe twice. The August remedy widened a crate blocklist; the working path is raw `sqlx` against the table. **DIVERGES** |
| Gates driving production (1 of 9) | TS.5 §4 ratifies wiring tests through production. Two live perturbations were invisible to all nine. **DIVERGES** |

## §5 Sequencing

The reconciliation's own recommended order, adopted:

1. **Obligation projection tables** — one migration. Trivial, and it closes a NOT DONE.
2. **The write boundary** (`evaluation_never_writes_facts`) — a mechanism that constrains *tables*, not crates. A crate blocklist cannot express "must not write to table X" when the crate holds a SQL driver and a connection. Candidates: a database role/grant the evaluation pack connects under, or a lint forbidding raw `sqlx` in that crate. **Whichever is chosen must be provable by the probe that has now disproved this claim twice.**
3. **Gate rehoming** — 7 of 9 gates onto the production entry point. Proven consequential: production was perturbed to record a fabricated in-scope set — the exact defect one gate is named for — and all nine stayed green.
4. **The evaluation engine** (§2) — its own tranche. K-23, `unevaluable`'s data, the waiver's object and the behavioural half of `in_scope_set_is_computed_not_stored` all fall out of it.

**2 and 3 before 4, deliberately.** Both are repeat defects whose remedies have already failed once each. Building the engine on top of an unenforced boundary and a gate suite that cannot see production would repeat the pattern a third time.

## §6 Gate tests (RED first)

**For the engine:**
- `a_check_produces_a_verdict` — the proof check, driven through the production entry point, returns pass / fail / unevaluable against three boards. Today unrepresentable.
- `findings_reach_the_run_record` — a run's persisted `findings` column contains the verdict. Today always `[]`.
- `in_scope_set_reflects_the_board` — **driven through production**: adding a trust changes the persisted in-scope set. The existing gate builds its own fixtures and calls the function directly, which is why it stayed green while production wrote a fabricated set.
- `unevaluable_carries_its_reason` — the persisted reason names which distinction applied (alleged edge / alleged type / admission on an alleged classification). The type already models all three; nothing populates it.
- `k23_gate_can_fire` — a subject with a failing finding is **refused** approval. Today the gate reads an always-empty work list, and a subject with zero events was approved. **This is the falsifiability test for the whole tranche.**
- `waived_check_was_in_scope` — a waiver names a check that was in the cited run's in-scope set. Today `sanctions.screen` was waived; it was never in scope, never evaluated, and does not exist.

**For the boundary and the gates:**
- `evaluation_pack_dependency_graph_excludes_the_append_chokepoint` — replaces `evaluation_pack_cannot_write_facts`, asserting what is true (§7 Q1) rather than a stronger structural claim two probes have disproved.
- `run_trigger_is_a_session_identity` — every run record carries the session that triggered it (§7 Q3). A run with no session origin is refused.
- `test_mode_side_door_is_named` — structural: the test-only run path is explicit and unreachable from production.
- Each rehomed gate proven able to fail by perturbing **production**, not a fixture.

Every gate proven able to fail: perturb, observe red, restore, observe green.

## §7 Rulings 2026-08-23

**Q1 — RULED: pack scope is the design; the crate is one implementation of it.** The evaluation pack is read-only on the UBO board and writes only its run sheet. Two separate packs means no Sage session can execute evaluation verbs against the assembly pack. What a crate can technically reach is not a design defect.

**Consequence — the gate's claim is corrected, not the design.** `evaluation_pack_cannot_write_facts` currently asserts the pack cannot write facts *by construction, having no append in its dependency graph*. Probes have disproved that twice: the crate holds a SQL driver and a live connection, so raw SQL reaches the table. The gate is rewritten to assert what is true — **the dependency graph excludes the governed append chokepoint** — rather than a stronger claim it cannot survive. A gate that cannot fail is worse than none; a gate asserting something false is worse again. This closes a repeat "defect" that was never a defect and removes two rounds of probe-versus-claim from future reconciliations.

**Q2 — RULED: the proof check is *every entity on the board has a proven type*.** Board-only, no compliance input, and it exercises all three verdicts naturally — pass where every type is proven, fail where one cannot be, unevaluable where a type is only alleged.

**Q3 — RULED: a run is an act in a session, not a background job.** The run book is a REPL run book, likely macro-shaped: **Sage triggers the permission and the REPL runs it against the database. No side doors, except in test mode.** Consistent with the standing position that the only asynchronous thing in this design is workflow obtaining proofs.

Consequences: the run record's `trigger` always carries a session identity; append-only at the verb level follows from the two-pack split, since no session can rewrite a prior run; and the residual exposure is direct SQL, which per Q1 is implementation, not design. **The test-mode side door is deliberate and must be explicit** — a named, test-only path, never a production-reachable one.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-23 | Initial draft, following the D2.0 reconciliation: 7 of 19 deliverables conform, and the four NOT DONE are one thing — **there is no evaluation engine**. `Check` declares applicability and nothing else; the catalogue is a literal empty slice; findings are hardcoded `[]`; `Finding` is not `Serialize`. Consequence proven by execution: the K-23 approval gate reads an always-empty work list, and a subject with zero recorded events was approved. §1 records the cause as a scoping error — D2.0 Q2 correctly ruled catalogue *contents* out of scope, but `evaluate()` is machinery, and nothing distinguished them. §2 redraws the line with an explicit test. §3 introduces the **deferral rule**: a deferral states what, why, who and when, or it is unfinished work recorded as such — and a deferred item is never reported as LANDED. §4 applies the rule to everything currently parked, reclassifying three items as unfinished. §5 sequences the two repeat structural defects (write boundary disproved twice; 1 of 9 gates driving production, down from 2 of 8) **before** the engine, so it is not built on foundations that have already failed once. |
