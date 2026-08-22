# EOP-DD-KYCUBO-TS.2 — Investment Fund Determination Strategy
### Closing the `investment_fund` structure class; the ManCo pivot rule

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-TS.2 |
| **Version** | 0.1 — draft for ratification (domain rulings) |
| **Binds to** | EOP-VS-KYCUBO-001 v0.6 (determination semantics); EOP-PLAN-KYCUBO-KIT-001 v0.6 §TS.2; EOP-VS-CONTAINERS-001 v0.1 (CTN-2a edge-kind vocabulary, CTN-2b control basis, CTN-3a cross-container determinations) |
| **Status** | DRAFT. Every ruling below is a domain decision — Adam ratifies; Sonnet encodes. Gated behind R4 (EdgeKind blast radius) before build. |

---

## §1 The defect being closed

`investment_fund` is one of six structure classes that classify successfully and have **no `DeterminationStrategy`**. Today it falls through to the ownership-prong path, which is not merely incomplete — it is **wrong in a specific, predictable way**: ownership traversal from a fund runs into a diffuse investor base, mostly below any threshold, frequently nominee-held, and yields either no beneficial owner or an arbitrary one. The T6.1 fail-closed guard (`StructureClassSupported`) currently converts that silent wrong answer into a refusal. This document supplies the strategy that lets the refusal become an answer.

## §2 Why funds are different (the domain fact)

In a fund structure, **economic interest and control run through different edges and terminate in different places**. Investors hold units — economic participation, no control. The Management Company directs the fund under a management mandate — control, no economic interest. Neither chain alone answers "who is the beneficial owner", and following the economic chain (the instinct inherited from corporate structures) is the specific error above.

This is why regulation pivots to control for fund structures, and why the meaningful answer for a fund typically resolves to **the ManCo's beneficial owners**, with senior managing officials as the recorded fallback.

## §3 Ruling 1 — The three holding kinds (CTN-2a, restated here as determination semantics)

| Kind | Control weight | Economic weight | Traversal behaviour |
|---|---|---|---|
| Voting/ownership | yes | yes | continue; multiply percentages along the chain |
| Investor issuance | **none** | yes | economic aggregation only; **never** contributes to a control determination; do not chase unitholders for control |
| Pooled asset (umbrella → sub-fund) | n/a | n/a | containment: a **scoping boundary**, not a holding |

**Ruling 1a — Determination is per sub-fund, not per umbrella.** Segregated liability between sub-funds is legally real; each sub-fund is its own determination subject. The pooled-asset edge is what tells the constructor where one determination ends and the next begins.

## §4 Ruling 2 — The governing-mandate pivot (the core of this strategy)

**The rule keys on BASIS, not on the label "ManCo".** The same shape appears under different names across jurisdictions and structures: a UCITS/AIFM Management Company, the **general partner of a limited partnership**, an investment adviser holding a governing mandate. All are the entity in which the fund's directing mind sits. A strategy written against the word "ManCo" silently mishandles partnership structures, where the LPs are pure investor-issuance and the GP *is* the control.

**The pivot entity is whoever holds the governing mandate.** The basis vocabulary (CTN-2b) enumerates the qualifying kinds; the strategy dispatches on the basis.

**The mandate holder is a re-anchor, not a terminal node.** Terminating traversal there names a *company* as beneficial owner, which is not an acceptable answer — the determination must resolve to natural persons, or to an explicitly recorded SMO fallback.

**The rule:** on reaching a fund whose control basis is a governing mandate, the determination **re-roots at the mandate holder** and continues up that entity's own voting/ownership chain to natural persons. The pivot is recorded in provenance, so the determination explains itself: *"resolved via governing mandate at X, whose beneficial owners are …"*.

**Ruling 2c — the Investment Manager is never a pivot basis.** An IM holds a *delegated service contract*: investment discretion within a mandate someone else owns. It is recorded (it matters for CBU membership, risk and conflicts) but it confers no control for beneficial-ownership purposes, regardless of the assets it directs. Distinguishing IM from mandate holder is a vocabulary distinction, made once, on the edge.

**Ruling 2d — an affiliated IM enters by ownership, not by contract.** Where the group owns the asset owner and also acts as IM, the IM appears in the determination through **ordinary ownership traversal**. No IM-specific rule, no exception path: the case that looks like an exception is the general machinery arriving by a different edge. If a future change makes this case require special handling, that is a signal the edge vocabulary is wrong, not that the rule needs an exception.

**Ruling 2e — multiple mandate holders produce a union, not a ranking.** Genuine co-management means two governing mandates and therefore two real control bases. Pivot on each; the determination is the **union** of natural persons found, each carrying its own recorded pivot path. A determination that suppresses one of two legitimate bases is wrong, not simpler.

**Ruling 2a — pivot exhaustion.** If the mandate holder's own chain yields no natural person above threshold, the strategy falls to senior managing officials **of the mandate holder**, not of the fund, and records that it did so and why. Silent SMO fallback is prohibited (K-8 discipline).

**Ruling 2b — pivot cycles.** Re-anchoring can revisit an entity in circular management/ownership arrangements. The strategy carries a visited set; a repeat visit terminates that branch and is recorded, never spun on.

## §4a Ruling 2f — The three evidence kinds

A group's control is KYC'd from three distinct evidence families, and a determination is **incomplete** without all three present for the entities it traverses:
1. **Shareholding chain** — the ownership/voting edges (who owns the mandate holder).
2. **Management contracts** — the mandate evidence attached to the control edge (that the mandate exists, its scope, its term).
3. **Officers** — director/SMO roles, which are both the fallback population (2a) and evidence of the directing mind.
These are natural stud candidates for TS.2's own preconditions — but **on freezing, not on computing** (CTN-2e/CTN-3a). A pivot whose mandate contract evidence is absent still computes a **provisional** determination, labelled by the weakest status it traversed; what it may not do is *freeze*. Refusing to compute would break the onboarding opening state, where the whole structure is alleged and nothing is yet evidenced.

## §5 Ruling 3 — Wire vocabulary extension

`assert-control` today has no value expressing a management mandate, so every fund control assertion collapses into the `DominantInfluence` catch-all — the same defect shape the audit found for trust structures. This strategy requires the basis to be expressible on the edge (CTN-2b).
**Blast radius (confirm against R4 before build):** the `EdgeKind`/basis enum, its fold arms, `assert-control`'s YAML `valid_values`, the serde wire values, and the closure teeth pinning them. Expected to be enum + YAML + fold + tests, no schema migration — **R4 confirms or refutes this**.

## §6 Gate tests (RED first)

- `fund_resolves_via_mandate_holder_not_unitholders` — a fund with investor-issuance edges and a mandate-based control edge resolves to the mandate holder's natural persons; the unitholders appear in no control determination.
- `gp_of_partnership_pivots_like_manco` — an LP structure resolves via the general partner; the limited partners are treated as investor-issuance. Proves the basis-not-label rule (§4).
- `im_contract_never_pivots` — an IM edge of any size or discretion produces no pivot and no control contribution.
- `affiliated_im_appears_via_ownership` — group owns the asset owner AND acts as IM ⇒ the IM's people appear in the determination through the ownership chain, with NO IM-specific code path involved (assert the provenance names ownership, not the IM contract).
- `co_management_unions_both_pivots` — two governing mandates ⇒ union of natural persons, each carrying its own recorded pivot path; neither is suppressed.
- `pivot_computes_provisionally_without_evidence` — a mandate-based pivot with NO contract evidence still returns a determination, labelled provisional/alleged; it is the *freeze* that refuses (Ruling 2f). Onboarding's opening state must not be blocked.
- `provisional_determination_carries_weakest_status` — a chain mixing verified and alleged edges yields a determination labelled by the weakest link, not the strongest.
- `investor_issuance_never_confers_control` — property: adding investor-issuance edges of any size never changes a control determination.
- `sub_funds_determine_independently` — two sub-funds under one umbrella with different ManCos yield two different determinations; the umbrella yields neither by aggregation.
- `manco_pivot_is_recorded` — the determination's provenance names the pivot and the basis; absence of the record fails.
- `pivot_exhaustion_falls_to_manco_smo` — no natural person above threshold ⇒ SMO of the ManCo, explicitly recorded, never silent.
- `pivot_cycle_terminates` — circular management arrangement terminates and records, does not spin.
- `structure_class_supported_widens_consciously` — the T6.1 pinned set gains `investment_fund`; a conscious test edit with red→green evidence.

## §7 Open questions

**Q1 — Threshold for the mandate holder's chain.** Does the mandate holder's own ownership chain use the standard threshold, or does the pivot reset it? Regulatory input likely.
**Q2 — RESOLVED (2026-08-19).** Multiple mandate holders → union, not ranking (Ruling 2e). IM is never a pivot basis (2c); an affiliated IM enters via ownership with no special path (2d). The pivot keys on basis, not the label "ManCo" (§4), covering GP-of-LP and adviser-with-governing-mandate.
**Q3 — Does the economic chain still get recorded?** The investor-issuance aggregate is not a control answer, but it may be a required disclosure in its own right. Compute-and-record, or omit?
**Q4 — Relationship to `foundation`.** Foundations may share the pivot shape (council/board as control basis, no owners). If so, TS.2's machinery generalises and TS.2/TS.3 sequencing should change.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-19 | Initial draft. Three holding kinds with distinct traversal semantics; determination per sub-fund; the mandate holder as re-anchor rather than terminal, with recorded pivot, exhaustion-to-SMO, and cycle termination; wire-vocabulary extension for control basis pending R4's blast radius. |
| 0.2 | 2026-08-19 | ManCo/IM rulings folded in. **Pivot keys on BASIS, not the label "ManCo"** — covers GP-of-LP (limited partners are investor-issuance, the GP is the control) and adviser-with-governing-mandate; a label-keyed strategy silently mishandles partnerships. New rulings: 2c IM is never a pivot basis (delegated service contract, not control); 2d an affiliated IM enters via ordinary ownership traversal with NO special path — if that case ever needs special handling, the edge vocabulary is wrong; 2e multiple mandate holders produce a UNION with per-pivot provenance, not a ranking; 2f the three evidence kinds (shareholding chain, management contracts, officers) with a stud candidate — pivot without mandate evidence refuses rather than resolves. Gate tests extended to 12 (GP pivot, IM-never-pivots, affiliated-IM-via-ownership asserting no IM code path, co-management union, evidence-absent refusal). Q2 RESOLVED. |
