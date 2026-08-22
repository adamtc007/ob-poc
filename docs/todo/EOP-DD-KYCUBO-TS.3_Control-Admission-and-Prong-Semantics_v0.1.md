# EOP-DD-KYCUBO-TS.3 — Control Admission and Prong Semantics
### Reconciling the V&S determination model with the D1 pipe vocabulary; ruling what the control whitelist admits

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-TS.3 |
| **Version** | 0.1 — draft for ratification |
| **Reconciles** | EOP-VS-KYCUBO-001 v0.6 §6.3 prong cascade, §6.4 structure-class table, §9.2 control taxonomy, K-2/K-3/K-8/K-11/K-12 — against EOP-DD-KYCUBO-TS.0/TS.1/TS.2 (all RATIFIED) and the control whitelist landed at `c7ef69ca` |
| **Domain** | **D1.** Determination is a factual conclusion derived from the graph. Whether the result is *acceptable* is D2 and lives elsewhere. |
| **Status** | **RATIFIED 2026-08-21.** §3 admission table, §4 classes, §4a SMO-pull-on-exhaustion and §2a's provisionality rule all stand. All three questions closed (§8). V&S amended for the delegated-IM ruling. Build-ready — and note this is the first tranche in the programme that deliberately changes determination outputs. |

---

## §1 What this document is for

The whitelist landed at `c7ef69ca` froze today's traversal behaviour deliberately: six newly-assertable `EdgeKind` variants are excluded from control traversal, with the semantics question left explicitly open. This document answers it — and answers it by **reconciling to the V&S rather than inventing**, because the V&S already rules more than the recent design work assumed.

**What the V&S already settles** (and TS work must not re-decide): the prong cascade Ownership → Control-by-other-means → SMO (§6.3); the primary prong per structure class (§6.4); that economic and control trees are computed independently and reconciled only at the determination (K-2); that dominant-chain control propagates **as control, never as a multiplied percentage** (K-3); that a determination resolves *through* a nominee and never terminates at one (K-8); and that intermediate nodes carry a resolution status that is a fold over upward edges plus stop-conditions (K-12).

**What the V&S does not settle, and this document does:** which typed edges the control traversal *admits*. The V&S §9.2 enumerates a control **taxonomy** — the vocabulary of edges that may be recorded — which is not the same set as the edges a determination *walks*. Conflating the two is what produced the silent over-admission the whitelist just closed.

## §2 The distinction this rests on

**Recorded ≠ traversed ≠ terminal.** Three separate properties, and every pipe has all three answered:

- **Recorded** — the edge exists in the control taxonomy, with its own validity and proof rule (V&S §9.2). Nearly everything is recorded.
- **Traversed** — the determination walks *through* it, continuing to the next node. This is what the whitelist governs.
- **Terminal / population** — the edge contributes persons to the answer without being walked through (the SMO population is the clearest case).

An edge can be recorded and never traversed without that being a defect. What is a defect is an edge being traversed *by default* because nobody decided.

## §2a The invariant this document must not break

**In all cases: assemble by allegation, then prove; and at any time, the D2 inspection may run.** Nothing in §3's admission rulings gates on proof. A mandate edge that is merely alleged still traverses; a statutory stop still stops; the SMO pull still pulls. What changes is not *whether* the determination is produced but *how well it is known* — it carries the weakest status it rested on (CTN-2h). A policy or regulatory test may be run against the board at any moment; it simply fails, and the failure is the work list.

**The sharpening this document adds: provisionality propagates through the traversal decisions, not only the edges.** Pipe classification needs the target's entity type (TS.2 §3), and admission and stop conditions are decided from that classification. So:

- An edge whose target type is only **alleged** yields a **provisional classification**, therefore a **provisional admission decision** — the traversal walked it, but on an unproven basis.
- A `Stop` at a statutory authority is itself provisional if the target's type is alleged: we stopped because we believe it is a state body, and that belief is not yet proved.
- A determination is therefore provisional if **any decision about how to traverse** rested on an alleged type — not merely if an edge was alleged.

The failure mode this forecloses: a confidently-presented determination whose traversal path was chosen on unproven type assertions. The assurance profile must carry *why* it is provisional — alleged edge, alleged type, or an admission decision taken on an alleged classification — because those are different remediation tasks.

## §3 The admission ruling, per pipe

Grounded in V&S §6.4's control-axis column. "Admit" means the control traversal walks through it.

| Pipe | V&S basis | Ruling | Why |
|---|---|---|---|
| 1 Voting shares | §6.4 private company: *voting* | **ADMIT** (already) | The control axis proper |
| 5 Board appointment | §6.4 private company: *board appointment* | **ADMIT** (already) | Named in the control axis |
| 3 GP designation | §6.4 LP/PE: *GP statutory* | **ADMIT** (already) | Named; "GP/manager line traced to persons" |
| 3 (LLP) Designated member | §6.4 LLP: *designated members* | **ADMIT** (already) | Named |
| 8/9 Trust roles, reserved powers | §6.4 trust, foundation: *role-based* | **ADMIT** (already) | Role enumeration is the prong for these classes |
| 13 Contractual control | §6.4 LP/PE: *LPAC veto*; §9.2 *veto* | **ADMIT** (already) | Veto and shareholder agreements are the named mechanism |
| — Dominant influence | §9.2; K-3 | **ADMIT** (already), propagating as control, never multiplied | K-3 is explicit |
| **7 Management mandate** | §6.4 investment fund: *ManCo / AIFM*; LP/PE: *delegated IM* | **ADMIT — new** | The V&S makes ManCo/AIFM **the** control axis for funds. Excluding it leaves fund determinations with no control path at all |
| **11 Membership rights** | §6.4 cooperative: *board; voting (often one-member-one-vote)* | **ADMIT — new** | Named as the co-op control axis; "economic % often meaningless" makes this the only route |
| **12 Statutory authority** | §6.4 state-owned: *public officials → SMO / special-handling* | **STOP with recorded reason** | The V&S routes this to SMO/special-handling, not onward traversal. Walking into a sovereign is not a determination |
| **6 Officer appointment** | §6.4 listed: *directors/officers may still be in scope*; fund directors in role map | **DO NOT ADMIT** (ruled 2026-08-21) | Officers are recorded and available, but are **not** eagerly contributed: they may not be needed or relevant to the KYC/AML decision. The SMO fallback pulls from them **on exhaustion** (§4a), never by edge admission |
| **14 Employment** | not in §9.2's taxonomy | **DO NOT ADMIT** | Employment is an *obligation basis* (K-21 — signatory, officer role), not a means of control. Recorded, never traversed |
| **16 Containment** | §6.4 fund: *sub-fund / compartment* | **DO NOT ADMIT** | Structural scoping (TS.1 §2a): it bounds *which* determination, it is not a control relationship |
| 15 Unit issuance | §6.4 fund/LP: *investors route out* | **DO NOT ADMIT** | Explicit in the V&S — investors route out; economic axis only (K-2) |
| 2/4 Economic pipes | K-2 | **DO NOT ADMIT to control** | Economic tree is computed separately and reconciled at the determination |
| 17 Nominee | K-8 | **PIERCE — never admit, never terminate** | Resolves *through*; a traversal rule, not an admission (TS.0 §5) |

## §4 Admission classes — and why "population" is not one of them

The whitelist as landed is a boolean. §3 shows it is one distinction short, but **not** the one first drafted. The 2026-08-21 ruling on officers ("no — it may not be needed or relevant to the KYC/AML decision") settles it: **the SMO population is pulled on exhaustion, never pushed by edge admission.** An edge does not contribute persons because it exists; the strategy asks for them when its walk has run out. That removes a class rather than adding one.

**Ruling: the admission function returns a class, not a bool.**

| Class | Meaning | Pipes |
|---|---|---|
| `Traverse` | walk through; continue to the next node | 1, 3, 5, 8, 9, 13, dominant influence, **7 (new)**, **11 (new)** |
| `Stop` | traversal halts here, recording the reason; no onward walk | **12 (new)** |
| `NotControl` | never part of the control walk | 2, 4, **6**, 14, 15, 16 |
| `Pierce` | substitute the underlying holder and continue (K-8) | 17 |

`Pierce` is listed for completeness — it is a traversal rule available during any strategy (TS.0 §5), not a member of the admission set.

## §4a The SMO fallback is a strategy behaviour, not an edge property

V&S §6.3's third prong fires when ownership and control are exhausted. It is **pull, on exhaustion**: the strategy, having found no natural person, asks for the officer/SMO population of the entity it exhausted at (TS.2 Ruling 2a — of the mandate holder, not the fund), records that it did so and why, and stops. Silent SMO fallback is prohibited (K-8 discipline).

Modelling this as an admission class would have contributed officers eagerly at every node — naming officers as beneficial owners in structures where ownership resolved perfectly well. The pull model keeps officers recorded, available, and out of answers that do not need them.


## §5 Two reconciliation defects — the V&S must resolve these, not this document

**5.1 — Delegated IM.** V&S §6.4 lists *delegated IM* in the control axis for both LP/PE funds and investment funds, and §9.2 lists it in the control taxonomy. But the ratified ruling of 2026-08-19 (TS.2 §4 Rulings 2c/2d) is that an IM holds a delegated service contract, is **never a pivot basis**, and enters a determination only via ordinary ownership affiliation. These cannot both stand as written.

*Proposed resolution, for the V&S to adopt:* the IM is **recorded** in the control taxonomy (it matters for risk, conflicts and CBU membership) but is **`NotControl`** for traversal — the governing mandate (ManCo/AIFM/GP) is the control axis, and a delegated manager under that mandate is not. This preserves both documents' intent: §6.4's "GP/manager line traced to persons" is satisfied by pipe 7, and the later ruling's substance is preserved. **Adam ratifies; the V&S §6.4 cell and §9.2 entry are amended accordingly.** Until then, this document treats delegated IM as `NotControl` and flags the divergence rather than silently resolving it.

**5.2 — Proof ratchet versus computed status.** V&S K-11 says edge state advances only through a governed transition with cited evidence, **never set directly**. CTN-2f (containers V&S v0.2) says status is **computed** from evidence and horizon, never stored. Read carelessly these conflict.

*Proposed resolution:* they are compatible and describe different objects. The **governed transition is over evidence** — citing evidence is the ratcheted act. The **status is derived** from the evidence plus the review horizon at the moment being asked about. K-11's "never set directly" is exactly CTN-2f's "never stored as a flag". Recommend a clarifying sentence in both documents rather than a change to either.

## §6 What this changes in code

- `is_admitted_as_control(&EdgeKind) -> bool` becomes `control_admission(&EdgeKind) -> ControlAdmission` with the four-way enum of §4. Exhaustive match, no catch-all — unchanged discipline.
- `ManagementMandate` and `MembershipRights` move from excluded to `Traverse`. **This is the first behavioural change to determinations in this programme** — the whitelist tranche was equivalence-preserving by design; this one is not, and its gates must assert the *intended* difference rather than sameness.
- `StatutoryAuthority` becomes `Stop` — traversal halts, recording the reason. `OfficerAppointment` becomes `NotControl`; the SMO population is pulled on exhaustion by the strategy (§4a), never contributed by edge admission.
- `Employment`, `Containment`, `UnitIssuance` and the economic pipes stay out, now with a V&S citation as the reason rather than an accident of filter logic.
- `edge_kind_strategy_admission_is_exactly_known` re-pinned from "deliberate exclusion" to the four-way classification.

## §7 Gate tests (RED first)

- `fund_with_manco_now_resolves` — a fund whose only control path is a management mandate produces a determination instead of nothing. Proves the point of admitting pipe 7.
- `cooperative_resolves_via_membership` — a co-op with membership-rights edges and meaningless economic percentages resolves via control.
- `statutory_authority_stops_with_reason` — traversal halts at a state body and records *why*; it does not walk onward and does not eagerly contribute officials.
- `officers_contribute_only_on_exhaustion` — officers appear in a determination ONLY where ownership and control exhausted; in a structure that resolves cleanly, no officer appears. Adding officer edges to a resolving structure changes nothing.
- `smo_fallback_is_recorded_never_silent` — an exhausted walk that pulls the SMO population records the pull, the entity it exhausted at, and the reason.
- `employment_and_containment_never_traversed` — property: adding either edge, in any quantity, changes no determination.
- `unit_issuance_never_confers_control` — investors route out (V&S §6.4), re-proven at this layer.
- `control_is_not_multiplied` — K-3: a dominant-chain control path propagates as control with no percentage arithmetic anywhere on it.
- `axes_stay_independent_until_determination` — K-2: economic and control folds are computed separately; no code path collapses them before the determination.
- `admission_is_exhaustive` — structural: a new `EdgeKind` variant is a compile error in `control_admission`.
- `determination_runs_at_any_board_state` — §2a: a board of purely alleged edges over alleged types still produces a determination, labelled provisional; nothing in the traversal gates on proof.
- `traversal_decisions_carry_provisionality` — §2a's sharpening: where an admission or stop decision was taken on an alleged target type, the determination is provisional AND the assurance profile names *that* as the reason, distinctly from an alleged edge.
- `statutory_stop_on_alleged_type_is_provisional` — the narrow case: we stopped because we believe it is a state body; that belief is not yet proved, and the stop says so.

## §8 Open questions — ALL RESOLVED 2026-08-21

**Q1 — Delegated IM. RESOLVED: the ruling stands; the V&S is amended.** The IM relationship is contractual — an investment mandate — and is never a control basis for determination. The apparent exception (the asset owner *is* the IM) is not an exception: that entity is already in the chain via ownership and ordinary traversal finds it, with no IM-specific path. Confirmed against the hedge-fund split-LLP case — one LLP executing the derivatives strategy, one managing investors: the executing manager holds a mandate to trade, not control of the vehicle, and is recorded and screened but never traversed. Control sits with the governing mandate holder.

**Q2 — Officer scope. RESOLVED: no eager contribution.** Officers may not be needed or relevant to the KYC/AML decision, so they are recorded and pulled only on exhaustion (§4a). This simplified the model rather than complicating it — `TerminalPopulation` was dropped as an admission class.

**Q3 — Listed-entity carve-out. RESOLVED: out of D1.** Regulated-market listing is a **test against** a built UBO, not part of building it. It is a policy relief and belongs in the assurance/clearance document (D2). The traversal does not know that listing exists.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-21 | Initial draft. Reconciles the landed control whitelist against V&S v0.6 §6.3/§6.4/§9.2 and K-2/K-3/K-8/K-11/K-12 rather than deciding afresh — the V&S already rules the prong cascade and the per-class control axis. Establishes recorded ≠ traversed ≠ terminal, and replaces the boolean whitelist with a four-way admission class. Admits management mandate and membership rights (both named as control axes in §6.4); makes statutory authority and officer appointment terminal SMO-population edges; keeps employment, containment and unit issuance out with V&S citations. Surfaces two reconciliation defects for the V&S to resolve: delegated IM (§6.4 lists it as control axis; the 2026-08-19 ruling says it never pivots) and the apparent K-11 versus CTN-2f tension (compatible — the ratchet is over evidence, the status is derived). Notes that this is the first behaviourally non-equivalent determination change in the programme. |
