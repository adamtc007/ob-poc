# EOP-DD-KYCUBO-TS.4 — Fund Pivot Realisation and Cross-Strategy Piercing
### Closing the gap between TS.2's ratified rulings and what the code actually does

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-TS.4 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-DD-KYCUBO-TS.2 (RATIFIED — the fund pivot rulings); EOP-DD-KYCUBO-TS.3 (RATIFIED, landed `075d6072`); TS.0 §5 (nominee is a capacity, not a class); V&S §6.3/§6.4, K-3/K-5/K-8 |
| **Domain** | **D1.** Determination is a derived fact. Acceptability is D2. |
| **Status** | **RATIFIED 2026-08-21.** Rulings A and B stand. Q1: keep `NomineePierceStrategy`'s name, move the mechanism into the walk. Q2: piercing carries provisionality exactly as admission does (TS.3 §2a). Q3: the delegation pin stays scoped to determination strategies for now. Build-ready. |

---

## §1 What this document is for, and the stale premise it corrects

The programme has carried a belief that six structure classes lack strategies. **That is no longer true.** `IMPLEMENTED_STRATEGY_CLASSES` contains all eleven, and eight strategies exist. The T0.3-era gap was closed by TS.1–TS.4 work in a parallel session.

The real gap is different and less visible: **two strategies are thin delegates that claim a class and forward to generic control traversal.**

- `FundControlStrategy::resolve` → `ControlProngStrategy.resolve`, verbatim. Comment: *"Thin delegate (§2.2): same traversal machinery, fund framing."*
- `NomineePierceStrategy::resolve` → `ControlProngStrategy.resolve`, verbatim. Comment: *"the unpierced-nominee guard lives at the freeze dispatch site."*

**TS.3 partly masked the first one.** Now that `ManagementMandate` is `Traverse`, generic traversal walks fund → mandate → mandate-holder → voting → persons. The *chain* is right and funds resolve. What is absent is everything TS.2 ruled **around** the chain — the parts that make the answer explicable, correctly exhausted, and honestly evidenced.

This is a worse failure shape than a missing strategy: a determination that looks complete and cannot explain itself.

## §2 Ruling A — realise TS.2's fund rulings against the working chain

TS.2's rulings are ratified. They are restated here only as an implementation contract; **none of them is reopened**.

| TS.2 ruling | Status in code | What TS.4 requires |
|---|---|---|
| Mandate holder is a **re-anchor**, not a terminal | Chain works via generic traversal (TS.3) | The re-anchor must be **recorded**: the determination names the pivot entity and the basis — *"resolved via governing mandate at X"* |
| Keys on **basis, not the label "ManCo"** | Untested for GP-of-LP | An LP must resolve via its general partner, with limited partners treated as investor-issuance. Verify; if it does not, that is the finding |
| **2a — exhaustion** falls to SMOs *of the mandate holder*, not of the fund | Generic path does not know where it pivoted | The SMO pull (TS.3 §4a, `pull_smo_on_exhaustion`) must take the **pivot entity**, not the subject |
| **2b — pivot cycles** terminate and record | Unverified | A visited set across re-anchors; a repeat visit terminates that branch and is recorded |
| **2e — co-management is a union**, per-pivot provenance | Generic traversal merges anonymously | Two governing mandates ⇒ union of persons, each carrying its own recorded pivot path; neither suppressed |
| **2f — three evidence kinds**; a mandate pivot lacking contract evidence computes **provisionally** and may not **freeze** | Absent | A stud on freeze, not on compute (CTN-2e: record freely, conclude carefully) |
| **2c/2d — delegated IM never pivots**; affiliation enters via ownership | TS.3 landed IM as `NotControl` | Verify no IM-specific code path exists — the affiliated case must arrive through ordinary ownership traversal |

**The through-line:** TS.3 gave funds a working chain. TS.4 gives that chain a **provenance, a correct exhaustion target, and an evidence gate**. Without them the fund answer is unexplainable, exhausts at the wrong entity, and freezes on unevidenced mandates.

## §3 Ruling B — piercing is a traversal rule, not a class strategy

TS.0 §5 ruled that a nominee is a **capacity held in a particular edge**, not what an entity *is*, and that piercing must be available **during any strategy**. The code does the opposite: `NomineePierceStrategy` is a class strategy selected when a subject classifies as `Nominee`, and it delegates to generic control traversal, with an unpierced-nominee guard at the freeze dispatch site.

**The consequence, stated plainly:** a nominee sitting **mid-chain** inside a fund, trust or corporate structure is not pierced, because the subject did not classify as `Nominee` and so the nominee strategy was never selected. The determination names the nominee, or stops at it — both forbidden by K-8.

**Ruling: piercing moves into the traversal itself.** When the walk meets a `Pierce`-classified edge (TS.3 §4), it substitutes the underlying holder and continues, recording the pierce. This holds in every strategy, at every depth. `NomineePierceStrategy` becomes what the others are — a framing over the shared walk — and the freeze-site guard remains as defence in depth rather than as the mechanism.

**Open for ratification (§6 Q1):** whether the class strategy is retired outright or kept as a named framing. Recommendation: keep the name, move the mechanism — retiring it would churn the strategy registry and the closure teeth for no behavioural gain.

## §4 The delegate audit — a general rule, not two fixes

Two thin delegates were found by reading the code rather than by any gate catching them. A strategy that names a class and forwards verbatim to another is **indistinguishable from a correct implementation** at every level the tests currently inspect: it compiles, it resolves, its class is in `IMPLEMENTED_STRATEGY_CLASSES`, and its gates pass.

**Ruling: a thin delegate is legitimate, but must be declared.** A strategy that delegates does so explicitly and is *pinned as delegating*, so the register distinguishes "implemented" from "framed over another implementation". Then a delegate that should have grown into a real implementation is visible in the pin rather than discoverable only by reading source.

**Tooth:** `strategy_delegation_is_exactly_known` — pins which strategies are genuine implementations and which are declared delegates. Today: `FundControlStrategy` and `NomineePierceStrategy` delegate; TS.4 makes the first a real implementation and moves the second's mechanism into the walk. Any future delegate must edit the pin consciously.

This is the same defect family the programme keeps meeting — declared-but-not-enforced, enforced-but-not-declared, and now **claimed-but-delegated**. Each was invisible until a pin made the claim explicit.

## §5 Gate tests (RED first)

**Fund pivot (Ruling A)**
- `pivot_is_recorded_with_basis` — the determination names the pivot entity and the governing-mandate basis. Absence of the record fails.
- `gp_of_lp_resolves_via_general_partner` — an LP resolves through its GP; limited partners appear in no control determination. Proves basis-not-label.
- `exhaustion_pulls_smo_of_pivot_entity` — no natural person above threshold ⇒ SMOs of the **mandate holder**, not of the fund. Assert the entity identity, not merely that some SMO appeared.
- `co_management_unions_with_per_pivot_paths` — two governing mandates ⇒ union, each person carrying its own pivot path; neither suppressed.
- `pivot_cycle_terminates_and_records` — circular management arrangement halts that branch and records it.
- `mandate_pivot_without_evidence_computes_but_cannot_freeze` — Ruling 2f and CTN-2e together: compute provisionally, refuse the freeze.
- `no_im_specific_path_exists` — structural: affiliated IM cases resolve through ownership traversal, and no IM-keyed branch exists in any strategy.

**Cross-strategy piercing (Ruling B)**
- `mid_chain_nominee_is_pierced_in_every_strategy` — the headline: a nominee inside a fund, a trust and a corporate chain is pierced in all three, with the subject classified as its own type, never as `Nominee`.
- `pierce_is_recorded` — each substitution records the nominee, the underlying holder, and the evidence relied on.
- `determination_never_terminates_at_a_nominee` — K-8, re-proven at traversal depth rather than only at the freeze site.
- `pierce_cycle_terminates` — nominee chains that loop halt and record.

**Register**
- `strategy_delegation_is_exactly_known` — §4's pin, RED-honest.

Every new gate proven able to fail: perturb, observe red, restore, observe green.

## §6 Open questions

**Q1 — `NomineePierceStrategy`'s fate.** Retire it, or keep the name as a framing over the shared walk once the mechanism moves? *Recommendation: keep the name, move the mechanism.*
**Q2 — Pierce and provisionality.** Piercing substitutes an underlying holder on the basis of a nominee declaration. If that declaration is only **alleged**, the substitution is a traversal decision taken on unproven ground — TS.3 §2a says the determination is then provisional. Confirm piercing carries provisionality the same way admission does. *Recommendation: yes, and for the same reason.*
**Q3 — Delegate audit scope.** §4's pin covers the eight determination strategies. Should the same claimed-but-delegated check extend to other registries in the pack (ops, folds), or stay scoped here for now?

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-21 | Initial draft. **Corrects a stale programme premise**: all eleven structure classes have strategies; the T0.3-era "six classes unimplemented" gap is closed. The real gap is two thin delegates — `FundControlStrategy` and `NomineePierceStrategy` both forward verbatim to `ControlProngStrategy`. Ruling A realises TS.2's ratified fund rulings against the chain TS.3 made work: pivot provenance, exhaustion at the mandate holder rather than the fund, co-management union with per-pivot paths, and an evidence stud on freeze rather than on compute. Ruling B moves nominee piercing from a class strategy into the traversal itself, per TS.0 §5 — otherwise a mid-chain nominee inside a fund or trust is never pierced, which K-8 forbids. §4 generalises the finding into a declared-delegation pin, naming a third member of the programme's recurring defect family: claimed-but-delegated. |
