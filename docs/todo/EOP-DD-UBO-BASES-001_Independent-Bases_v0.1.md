# EOP-DD-UBO-BASES-001 — Independent Bases
### Control has several doors. Any one opens. A person leaves only when the last one closes.

| | |
|---|---|
| **Document** | EOP-DD-UBO-BASES-001 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-VS-KYCUBO-001 (determination semantics, K-1 to K-35); EOP-DD-UBO-DISPATCH-001 (revised by §3); TS.3 control admission (revised by §5) |
| **Why ratified before code** | This changes the determination engine — the one component that can produce a quiet wrong answer, pinned by a `freeze` that has no inverse. The last such change (DISPATCH-001) was ratified as a table first; this is the same class and gets the same treatment. |
| **Status** | DRAFT. §3's revised table and §5's admission changes are the ruling surfaces. Two rulings already given by Adam on 2026-09-08 are recorded in §2. |

---

## §0 The problem, and whose error it was

The fuzz-harness research of 2026-09-08 asked whether a person with several independent control bases survives the removal of one. It does — but in proving it, the research found something worse: **for an ordinary company, only one basis is ever attempted.**

`dispatch_for_entity_type` maps `PrivateLimitedCompany | PublicListedCompany => ownership_prong_strategy` and nothing else. So a company's managing director, board appointee, or contracted operator is **structurally invisible** to the determination unless they also clear the ownership threshold. And `OfficerAppointment` — the kind that most literally means "managing director" — is classified `NotControl`, never traversed by any strategy, surfacing only through the SMO fallback, which fires only when nobody else was found.

**The error was in DISPATCH-001 §2, which I drafted and Adam ratified on that drafting.** I wrote "control follows equity" for companies. That mistakes the domain. Under the PSC regime the conditions are **alternative limbs applying to the same company at once**: more than 25% of shares, *or* more than 25% of voting rights, *or* the right to appoint or remove a majority of the board, *or* significant influence or control by other means. They are not alternatives selected by vehicle type. I turned "which strategies fit this vehicle" into "which single strategy", and for the modal case that under-identifies.

**This is the third instance of one family** — one channel where the domain has several — after the personhood flag and the payload/target duplication. It is the most serious of the three, because it under-identifies rather than mis-identifies: the frozen determination is not wrong about the people it names, it is silent about people it should have named.

## §1 The domain model

**Control has several doors, and any one opens.** A person may hold voting shares, hold the office of managing director, and hold a services contract with delegated authority to run the business — all with the same company, all true at once, none wrong. Each is **independently sufficient** to establish control. They are not evidence accumulating toward a threshold; they are separate doors.

**The consequence is about removal.** If the MD resigns, the shareholding still grants control. If the shares are sold, the contract still does. **A person leaves the determination only when the last basis goes.** So `disconnect` of one control edge removes a *route*; the candidate persists while any route survives.

**Percentages belong to shareholdings only.** Voting shares, non-voting shares, LP interests, units — quantities of which one holds a fraction and which multiply along a chain. Every other basis is binary: you hold the office or you do not, you hold the mandate or you do not. K-3 already forbids a percentage on a control path, and the code upholds it structurally — `effective_ownership_pct` is constructed `None` by every control strategy. **This document does not touch that.**

**The SMO fallback is a different thing.** "When no beneficial owner can be identified, name the senior managing official" is a regulatory backstop for the case where *no door opens*. It is not the same as "the MD controls". Both exist; they are distinct steps; and the first must not suppress the second.

## §2 Rulings already given — 2026-09-08

**R-A — `OfficerAppointment` and `EmploymentDelegatedAuthority` become control-admitted.** The office, and employment carrying delegated authority to run the business, are each a door. This reverses TS.3's `NotControl` classification of both. **Adam's words: "employment with delegated authority is the thing."**

**R-B — The SMO fallback survives as a separate step**, distinct from officer-as-controller. It fires when no basis of any kind admitted a person. It does not fire to *replace* a person admitted by an officer basis, and an officer basis does not suppress it when nothing else was found either.

**R-C — Everywhere is a door. RULED 2026-09-08.** R-A applies to every control-walk strategy, not to companies alone. An appointed officer of a partnership, an LLP, an LLC or a state body is a candidate exactly as a company's is. Adam's words: *"everywhere is a door — frankly — the role is the door, then a switch opens the door or not."* This ratifies by name what §5's mechanism (a shared `control_admission`) already did, and corrects §3/§7, which claimed only companies change: the dispatch *table* changes one row; the *admission* change reaches every strategy that walks control, and that is intended.

**What R-C clarifies about the determination itself.** The board records that a person holds a role — the door exists. Proof confirms the role is real. **Policy decides whether that role, on this board, meets the control threshold — the switch.** So the determination **names every door with its proof status; it does not decide which doors open.** Naming an officer as a candidate is not asserting they control; it is recording that the door exists and leaving the threshold to the inspect game. This is the reports-versus-judges split applied to control.

## §3 Dispatch returns a set — the revised table

DISPATCH-001 §2 mapped each type to **one** strategy. It now maps each type to **the set of strategies that apply**. The table changes shape, not content: every prior row survives, and companies gain the control limb they were missing.

| Entity type | Strategies | Change |
|---|---|---|
| `NaturalPerson`, `SoleTrader` | — terminal | unchanged |
| `PrivateLimitedCompany`, `PublicListedCompany` | **{ownership, control}** | **control ADDED** — the PSC limbs apply together |
| `LlcUs` | {control} | unchanged (R-2026-08-27) |
| `GeneralPartnership`, `LimitedPartnership`, `Llp` | {control} | unchanged |
| `OeicIcvc`, `Sicav`, `UnitTrust`, `FortyActFund`, `LpFund`, `UmbrellaWithSubFunds` | {fund_control} | unchanged — the mandate pivot already re-anchors |
| `DiscretionaryTrust`, `FixedBareTrust`, `PensionScheme`, `CharityNotForProfit` | {trust_role} | unchanged |
| `Foundation` | {foundation_council} | unchanged (§5q merge candidate stands) |
| `CooperativeMutual` | {cooperative_member} | unchanged |
| `GovernmentDeptStatutoryCorporation` | {state_owned} | unchanged |

**Only one row changes.** That is deliberate: the error was specific to companies, and widening every row would be a different ruling nobody has made. If a second type turns out to need two limbs, that is a row edit to a ratified table with a test behind it.

**Every strategy in the set runs, and the results union.** A person found by ownership *and* by control is one candidate with two bases (§4), not two candidates.

**D1–D4 from DISPATCH-001 hold unchanged** — exhaustive, terminal explicit, unmapped refuses by name, data with a test. The table's shape changing from `Strategy` to `Set<Strategy>` does not weaken any of them.

## §4 A candidate carries its bases

`ProngCandidate` today records **one** prong, **one** chain, **one** originating event — whichever edge sorted first. `Prong::Dual` exists in the enum and is constructed nowhere.

**Ruled shape:** a candidate carries **the set of bases that admit them** — each basis being the edge kind, the edge id, and the chain that reached it. Admitted by three edges, recorded once with three bases named.

**Why this is not cosmetic:**

- A determination of record that says only "controls via voting rights" when the person also holds the office and the contract is **incomplete evidence**. An auditor reading it sees one route and cannot tell there were three.
- The inspect game needs to see all of them. "30% shareholder" and "30% shareholder who also runs the company" are different risk pictures, and a policy check that cannot see the second cannot judge it.
- It is what makes R8-style removal provable: disconnect one basis, assert the candidate survives with the *remaining* bases named. Today that assertion cannot even be stated, because nothing names which basis survived.

`Prong::Dual` is retired as a variant — the set replaces it.

## §5 Control admission — two kinds move to Traverse

TS.3 classifies `OfficerAppointment` and `EmploymentDelegatedAuthority` as `NotControl`. Per R-A, both become **`Traverse`**. This is the second revision to a ratified TS.3 classification (the first was `ManagementMandate`, 2026-08-20).

**What does not change:** `EconomicInterest` stays `NotControl` — it is ownership, walked by the ownership strategy. `Containment` stays `NotControl`. `StatutoryAuthority` stays `Stop`. `Nominee` stays `Pierce`.

**The consequence for the exhaustion gate.** `pull_smo_on_exhaustion` currently returns `None` the moment *any* prior candidate exists. Under R-B it must instead fire only when **no basis of any kind** — ownership, control including officer, role — admitted anyone. An officer admitted through `Traverse` is a candidate, not a fallback; the fallback is for when even that found nobody.

## §6 Finding #3 — separate, small, closed alongside

The fuzz property named "control never multiplied along a chain" found a single `connect` edge asserted at **972%**. Not multiplication, not control — an unvalidated input: `connect`'s `percentage` has no bound anywhere in the write path.

**Two things, both mechanical:** a `Precondition` bounding `percentage` to `[0, 100]` on `connect`, same shape and chokepoint as `ConnectEndpointsNotWithdrawn`; and **rename the property** — it was never about control, and its name would mislead the next reader exactly as finding #2's did.

## §7 Gate tests (RED first)

- `company_dispatch_runs_both_limbs` — an ordinary company with a controlling officer and no qualifying shareholder resolves the officer. **Fails today: the officer is invisible.**
- `candidate_carries_every_admitting_basis` — a person admitted by three edges is one candidate with three bases named.
- `disconnect_removes_a_route_not_the_person` — N independent bases; remove any N−1; the person survives with the remaining basis named. **The fuzz invariant, promoted.**
- `officer_is_a_door_not_a_fallback` — an MD who is also a minor shareholder is admitted by the officer basis regardless of whether the shareholding clears threshold.
- `smo_fallback_fires_only_on_total_exhaustion` — R-B: it fires when no basis admitted anyone, and not otherwise.
- `dispatch_table_matches_the_ratified_set` — §3 pinned; exhaustive over all 21 types.
- `percentage_is_bounded_at_connect` — finding #3; 972 is refused at the chokepoint on both surfaces.
- `prong_dual_is_gone` — the variant is retired.
- **Fuzz:** the determination-invariants target gains the survival property, and its percentage property is renamed.

Each fail-proven by perturb → red → restore → green. **Determinations WILL change for companies with officer control — that is the point. Any change for any OTHER type is a finding: STOP and report.**

## §8 Open questions

**Q1 — Does `BoardAppointment` need the same treatment?** It is already `Traverse`, so a board seat is a door today. But is *one* seat control, or only a majority? The PSC limb is "right to appoint or remove a majority" — a single non-executive seat is not that. *Recommendation: leave as is; the inspect game's threshold judges whether one seat suffices, which is exactly the build-reports/inspect-judges split.*
**Q2 — Should any other type gain a second limb?** §3 changes only companies. An LLP or partnership with a designated member *and* an appointed manager arguably has two doors too. *Recommendation: not now — no evidence they under-identify, and widening without evidence is how the last table went wrong.*

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-09-08 | Initial draft, after the fuzz-harness research found that ordinary companies dispatch to ownership only, making a controlling officer structurally invisible. The error is recorded as mine, in DISPATCH-001 §2, which mistook the PSC limbs — alternative conditions applying to one company — for alternatives selected by vehicle type. Two rulings by Adam recorded: `OfficerAppointment` and `EmploymentDelegatedAuthority` become control-admitted ("employment with delegated authority is the thing"), and the SMO fallback survives as a distinct step for total exhaustion. Dispatch returns a set; companies gain the control limb; a candidate carries every admitting basis; `Prong::Dual` retires. Finding #3 is re-characterised as an unbounded percentage input and closed alongside. |
