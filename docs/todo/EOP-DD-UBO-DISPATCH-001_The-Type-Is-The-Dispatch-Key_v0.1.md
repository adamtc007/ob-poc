# EOP-DD-UBO-DISPATCH-001 — The Type Is The Dispatch Key
### The 22 → 8 mapping. A ratification artifact, not an implementation detail.

| | |
|---|---|
| **Document** | EOP-DD-UBO-DISPATCH-001 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-VS-UBO-GAME-001 §3.3 (no `structure-class`); TS.0 §1 (the entity type is the block) and §4 (the catalogue); TS.3 (control admission); TS.4 (fund pivot, piercing) |
| **Why this exists** | The 2026-08-27 review found `determination.rs` contains **zero** references to `EntityType`. `strategy_for_structure_class(&StructureClass)` is the sole bridge into the determination engine. Deleting `structure-class` therefore requires a mapping that **does not exist in any form** — and 11 → 8 is not 22 → 8. |
| **Why it is ratified before code** | This is the only change in the replacement that can produce a **quiet** wrong answer. Everything else fails loudly — a compile error, an empty board, a hard-error fold. Put a type on the wrong arm and `freeze` pins a plausible, wrong determination of record. **`freeze` has no inverse.** |
| **Status** | DRAFT. §2 is the ruling surface. §3's three hard cases need Adam specifically. |

---

## §1 The eight strategies, and what each assumes

| Strategy | Assumes about the vehicle |
|---|---|
| `ownership_prong_strategy` | control follows equity; traverse shareholdings and multiply |
| `control_prong_strategy` | control is asserted separately from equity — board seats, GP designation, mandates, veto |
| `trust_role_strategy` | there are no owners; the answer is the fiduciary role-holders, enumerated |
| `fund_control_strategy` | the directing mind sits **outside** the vehicle, under a governing mandate — pivot and re-anchor there (TS.4 Ruling A) |
| `foundation_council_strategy` | role enumeration, as trust, but over founder/council/beneficiaries |
| `state_owned_strategy` | traversal terminates in a public body; record the stop and fall to officials |
| `cooperative_member_strategy` | one member one vote; economic percentage is meaningless |
| `nominee_pierce_strategy` | a capacity, not a vehicle — piercing is a traversal rule available during any strategy (TS.0 §5, TS.4 Ruling B) |

## §2 The mapping — proposed

| # | Entity type | Strategy | Why |
|---:|---|---|---|
| 1 | `NaturalPerson` | **none — terminal** | A person is never a subject of determination; traversal ends here. Must be *explicitly* unmapped, not fail-closed by omission (§4) |
| 2 | `SoleTrader` | **none — collapses to the person** | No separate legal personality (TS.0 §4.1) |
| 3 | `PrivateLimitedCompany` | `ownership_prong_strategy` | Share classes; control follows equity |
| 4 | `PublicListedCompany` | `ownership_prong_strategy` | Same mechanism. Listing relief is a **policy test against the built board**, not a dispatch choice (TS.3 §8 Q3) |
| 5 | `LlcUs` | `control_prong_strategy` — **RULED 2026-08-27** | LLCs are managed by **partners and appointed officers**. That is control by designation and appointment, not control following equity — the same mechanism as a partnership. The member-managed/manager-managed distinction does not change the strategy: both resolve through who is designated to manage, so one type and one arm suffice |
| 6 | `GeneralPartnership` | `control_prong_strategy` | All partners control by the agreement, not by equity |
| 7 | `LimitedPartnership` | `control_prong_strategy` | GP designation controls; LPs are economic only |
| 8 | `Llp` | `control_prong_strategy` | Member-managed per agreement — matches today's mapping |
| 9 | `OeicIcvc` | `fund_control_strategy` | ACD/ManCo directs; investors route out |
| 10 | `Sicav` | `fund_control_strategy` | ManCo or self-managed board |
| 11 | `UnitTrust` | `fund_control_strategy` | ManCo directs; the trustee holds legal title but does not direct. **Note the tension with 15/16** — this is a trust-shaped vehicle taking the fund strategy, correctly, because its directing mind is a mandate holder |
| 12 | `FortyActFund` | `fund_control_strategy` | Board plus adviser under a governing mandate |
| 13 | `LpFund` | `fund_control_strategy` | GP **and** ManCo; the mandate pivot is the meaningful path. Distinguished from 7 by being a fund: LPs are investor-issuance and route out |
| 14 | `UmbrellaWithSubFunds` | `fund_control_strategy` — **RULED 2026-08-27** | An umbrella is **a fund like any other** — it rolls up assets from feeder funds, and it is directed by a ManCo exactly as its sub-funds are. It is a determination subject in its own right. What is distinct is the **pooling relationship**: an umbrella holds **participation shares** in the funds it pools. That is a separate pipe category from voting/ownership, and it carries no control — see §3 Q2 |
| 15 | `DiscretionaryTrust` | `trust_role_strategy` | Role enumeration |
| 16 | `FixedBareTrust` | `trust_role_strategy` | Role enumeration; fixed entitlement may make beneficiaries determinate |
| 17 | `Foundation` | `foundation_council_strategy` | Founder, council, beneficiaries |
| 18 | `PensionScheme` | `trust_role_strategy` | Trustee-governed; members are beneficiaries (TS.0 §4.5) |
| 19 | `CooperativeMutual` | `cooperative_member_strategy` | One member one vote |
| 20 | `CharityNotForProfit` | `trust_role_strategy` — **RULED 2026-08-27** | Charities are trusts. Role enumeration |
| 21 | `GovernmentDeptStatutoryCorporation` | `state_owned_strategy` | Control by statute |
| 22 | `SovereignWealthVehicle` | **NOT AN ENTITY TYPE — RULED 2026-08-27** | A sovereign wealth vehicle is **a group**, not a single legal entity. It is usually a collection of funds, but not always — it can include LLPs and other vehicles. It is never a block, so it takes no strategy; the funds and LLPs *within* it are the blocks, and each dispatches on its own type. **This is the same category error as `structure-class`**: a description of the shape at the top of a chain, filed among descriptions of what a single entity is. It is removed from the catalogue (TS.0 §4.6) rather than mapped |

**Where `nominee_pierce_strategy` went.** Nowhere — deliberately. Nominee is a **capacity held in an edge**, not a vehicle (TS.0 §5), and piercing is already a traversal rule available during any strategy (TS.4 Ruling B, landed). Under a type-keyed dispatch there is no type to map it from. **Retire the strategy as a dispatch target; keep the traversal rule.**

**Score: 21 types map or are terminal by design. 1 is removed from the catalogue entirely. No open questions.**

## §3 The four rulings, recorded

**Q1 — `LlcUs`. RULED: `control_prong_strategy`.** LLCs are managed by **partners and appointed officers** — control by designation and appointment, the same mechanism as a partnership, not control following equity. The member-managed/manager-managed distinction does not change the strategy, because both resolve through who is designated to manage. One type, one arm.

**Q2 — `UmbrellaWithSubFunds`. RULED: `fund_control_strategy`, and it IS a determination subject.** An umbrella is a fund like any other: it rolls up assets from feeder funds and is directed by a ManCo exactly as its sub-funds are. What is distinct is the relationship, not the vehicle — an umbrella holds **participation shares** in the funds it pools. *(Consequence for the pipe vocabulary in §3a.)*

**Q3 — `CharityNotForProfit`. RULED: `trust_role_strategy`.** Charities are trusts.

**Q4 — `SovereignWealthVehicle`. RULED: not an entity type; remove it.** It is a **group** — usually funds, but it can include LLPs and other vehicles. It is never a single legal entity and therefore never a block, so it takes no strategy. The funds and LLPs within it are the blocks, each dispatching on its own type. This is the same category error as `structure-class`: a description of the shape at the top of a chain, filed among descriptions of what a single entity is.

## §3a Two consequences that reach beyond the mapping

**The catalogue drops to 21 types.** `SovereignWealthVehicle` comes out of TS.0 §4.6. T3 encoded a 22×17 geometry matrix, so its row, the `matrix_is_exactly_known` pin, `ALL_ENTITY_TYPES`, and any fixture naming it all move with it. Mechanical, but it must be done deliberately rather than discovered.

**Pipe 16 may be described wrongly, and this ruling is what exposes it.** TS.0 has "pooled-asset containment" as a **scoping boundary** — not a holding, carrying neither control nor economic weight. Q2 says an umbrella holds **participation shares** in the funds it pools: a holding, economic, carrying no control. That is closer to unit issuance (pipe 15) than to containment, and it changes what the pipe *is* rather than what it is called. **Establish how T3 actually encoded it before building dispatch on top** — if it is encoded as a scoping boundary, either the encoding or TS.0's description is wrong, and that is a finding to report rather than a thing to quietly reinterpret.

## §4 The rules the mapping must obey

**D1 — Unmapped is refused, loudly and by name.** Today `IMPLEMENTED_STRATEGY_CLASSES` and `has_strategy()` fail closed for unimplemented classes. That discipline **must survive across 21 arms**, and it is the whole safety property: a type with no strategy refuses `freeze` with a message naming the type, rather than falling to a default.

**D2 — Terminal is explicit, not accidental.** `NaturalPerson` and `SoleTrader` are unmapped **on purpose** — a person is never a determination subject, and a sole trader collapses to the person. They must be distinguishable in the code from a type someone forgot: an explicit `NotADeterminationSubject` arm, never an omission that happens to fail closed. Otherwise the next type added inherits their silence.

**D3 — Exhaustive match, no catch-all.** A new entity type is a compile error until someone rules on its strategy. This is the same discipline as `control_admission` and it is what stops the mapping rotting as the catalogue grows (§C5: the vocabulary is expected to change).

**D4 — The mapping is data with a test, not a function with arms.** Pin it as a table with an exhaustiveness gate over `ALL_ENTITY_TYPES`, so adding a type forces a conscious edit to a ratified artifact rather than a quiet arm in a match.

## §5 Gate tests (RED first)

- `every_entity_type_has_a_ruling` — all 22 are mapped or explicitly terminal; none falls through.
- `unmapped_type_refuses_freeze_by_name` — the fail-closed property, with the type named in the refusal.
- `terminal_types_are_explicit` — `NaturalPerson`/`SoleTrader` are `NotADeterminationSubject`, distinguishable from an unruled type.
- `dispatch_is_exhaustive` — a new `EntityType` variant is a compile error.
- `mapping_matches_the_ratified_table` — the code agrees with this document, pinned.
- **Per-type behavioural gates for the five that change strategy relative to today** — the ones where 22→8 is not a relabelling of 11→8. These are where a quiet wrong answer would hide.

## §6 What this replaces

`strategy_for_structure_class` (`fold/control.rs:701`), `IMPLEMENTED_STRATEGY_CLASSES`, `Precondition::StructureClassified`, `Precondition::StructureClassSupported`, `ControlState::has_strategy()`, `STRUCTURE_CLASS_WIRE_VALUES`, and the `implemented_class_split_matches_strategy_arms` test — 97 non-comment references across 10 production files, and 101 events in the stream carrying the retired verb.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-27 | Initial draft, following the finding that `determination.rs` never references `EntityType` and that the 22 → 8 mapping does not exist in any form. 17 of 22 map cleanly; 2 are terminal by design; 3 need domain rulings — `LlcUs` (member- vs manager-managed, one type with two control mechanisms), `UmbrellaWithSubFunds` and `SovereignWealthVehicle` (types that are not determination subjects, or that resolve to another type), and `CharityNotForProfit` (trust roles versus company form). `nominee_pierce_strategy` is retired as a dispatch target — nominee is a capacity, and piercing is already a traversal rule. Four rules the mapping must obey, chief among them that terminal is explicit rather than accidental, so the next type added does not inherit a silence. |
