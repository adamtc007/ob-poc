# EOP-DD-KYCUBO-TS.1 — The Assembly Board: Type Geometry and Moves
### D1 — establishing the facts. What may be asserted, by whom, into what.

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-TS.1 |
| **Version** | 0.1 — draft for ratification |
| **Binds to** | EOP-DD-KYCUBO-TS.0 (RATIFIED 2026-08-19) — the block set and pipe vocabulary; EOP-VS-CONTAINERS-001 v0.2; EOP-VS-KYCUBO-KIT-001 v0.2 |
| **Domain** | **D1 only.** Nothing in this document asks whether a board is acceptable. Compliance, policy and regulatory assessment are D2 and live elsewhere. |
| **Status** | DRAFT. §2's grid and §3's move set are the ruling surfaces. |

---

## §0 The two domains, and why this document is D1

**D1 — establish the facts.** Assemble the alleged set of entities and connections; prove what can be proved; build the best possible taxonomy. **D2 — assess the facts.** Take the taxonomy and judge it against regulation, BNY policy, legal barriers.

**D2 requires no completeness gate.** It may run against a board in any state — three alleged edges, nothing proved, whatever. It simply fails, and *the failure is the work list*. That is why D1 comes first and why nothing in D1 needs to know policy exists.

**The interface is one-directional and narrow:** D2 receives the taxonomy at (T, axis, scope) with its determination, its assurance profile and its pins. It never reaches back into raw assertions. If it does, the boundary is broken.

## §1 The board

| Element | What it is |
|---|---|
| **Blocks** | Entity instances, each of a ratified type (TS.0 §4). Natural persons are blocks like any other. |
| **Studs** | The **type→linkage matrix** (§2) — which pipe kinds may connect which types. The primary constraint, and the piece that does not exist today. |
| **Board position** | The constructed taxonomy at (T, axis, scope): nodes, linkages, container state, each element carrying its proof status. |
| **Legal move set** | Derived from the type geometry plus current position. Computed, never enumerated by hand. |

**Two constraint layers, in order.** The **type geometry** decides whether a linkage kind is *possible at all* between two types — a voting-share linkage into a partnership is not a wrong move, it is not a move. The **studs already ratified** (registration before assertion, no duplicate active linkage, linkage lifecycle) then constrain whether a possible move is legal *in this position*. Type geometry is structural; studs are positional.

## §2 The type→linkage matrix (the geometry)

Pipe numbers are TS.0 §3. Read as: **rows are the target** (the entity being controlled or held), **columns are the pipe**. A cell marks the pipe permitted *into* that type. Who may sit at the other end is §2a.

| Target type | Permitted incoming pipes |
|---|---|
| Private limited company | 1 voting shares · 2 non-voting · 5 board appointment · 6 officer · 13 contractual · 14 employment |
| Public / listed company | 1 · 2 · 5 · 6 · 13 · 14 |
| LLC (US) | 1 · 5 · 6 · 13 · 14 |
| General partnership | 3 GP designation · 5 · 13 · 14 |
| Limited partnership | 3 · 4 limited interest · 7 management mandate · 13 |
| LLP | 3 · 5 · 13 · 14 |
| OEIC / ICVC | 5 · 7 · 15 unit issuance · 16 containment |
| SICAV | 5 · 7 · 15 · 16 |
| Unit trust | 7 · 8 trustee powers · 15 · 16 |
| 40 Act fund | 5 · 7 · 15 · 16 |
| LP fund | 3 · 4 · 7 · 15 |
| Umbrella (with sub-funds) | 16 only |
| Discretionary trust | 8 · 9 reserved powers · 10 beneficiary |
| Fixed / bare trust | 8 · 10 |
| Foundation | 8 · 9 · 10 |
| Pension scheme | 8 · 10 |
| Cooperative / mutual | 5 · 6 · 11 membership · 14 |
| Charity / not-for-profit | 5 · 6 · 8 · 14 |
| Government dept / statutory corporation | 5 · 6 · 12 statutory · 14 |
| Sovereign wealth vehicle | its actual vehicle type's row, plus 12 |
| Natural person | **none** — a person is never controlled; traversal terminates here |
| Sole trader | none — collapses to the person |

**§2 v0.3 corrections (2026-08-19, found in execution):**
- **Pipe 14 (employment) was omitted from every target row** in v0.2 while §2a permitted it — an editorial defect, not a ruling. Now added to the operating types that have employees: corporates, partnerships/LLP, cooperative, charity, statutory corporation. **Deliberately NOT added to funds** — a fund is externally managed (pipe 7); it does not employ.
- **Pipe 17 (nominee holding) correctly appears in no target row.** It is not a linkage *into* a type; it is a **capacity overlay on an existing holding** (TS.0 §5) — X holds shares *as nominee for* Y. It is governed by the pierce-and-substitute traversal rule, not by ordinary geometry. Stated here so its absence reads as a decision rather than an omission.

## §2a Who may sit at the source end

A pipe constrains both ends. Source constraints are looser than target constraints but not absent.

| Pipe | Permitted source types |
|---|---|
| 1 voting shares, 2 non-voting | any type that can hold property: all corporates, partnerships, funds, trusts (via trustee), natural persons |
| 3 GP designation | corporates and natural persons only — a fund is not a GP of itself |
| 4 limited interest | any holder, including funds and persons |
| 5 board appointment | natural persons; corporate directors where the jurisdiction permits (attribute on the source) |
| 6 officer appointment | **natural persons** (ruled 2026-08-19) — officers, managing partners and regulated persons holding the role. Management companies and management partnerships are reached as hybrids: the entity holds a *mandate* (pipe 7) over the fund, while its own managing individuals reach the entity via this pipe. |
| 7 management mandate | **corporate only** (ruled 2026-08-19) — a regulated management entity, never a natural person |
| 8 trustee powers | corporates (corporate trustee) and natural persons |
| 9 reserved powers | natural persons, and corporates acting as protector |
| 10 beneficiary entitlement | natural persons, corporates, charities, and **classes** (a class is not an entity — §3a) |
| 11 membership rights | natural persons and corporates |
| 12 statutory authority | government departments and sovereign bodies only |
| 13 contractual control | any type |
| 14 employment / delegated authority | **natural persons only** (ruled 2026-08-19), into a corporate or fund. A company does not hold an employment contract as employee — a corporate service arrangement is contractual control (13). |
| 15 unit issuance | any investor type |
| 16 containment | umbrella → sub-fund only; both must be fund types |
| 17 nominee holding | corporates and natural persons |

**§2a RATIFIED 2026-08-19.** Pipes 6 and 14 are natural-person-sourced; pipe 7 is corporate-only. The general principle stands behind all three: **the entity type decides the links.** A management company or management partnership is a hybrid — it holds the mandate as a corporate, and its own managing individuals reach it as officers.

## §2b Scope: KYC runs group-level down

**RATIFIED 2026-08-19.** The group is the unit of work: KYC is assembled from the group downwards, not from a single anchor outwards. A group may contain several subjects that each require a determination; the group scopes the member set, and determinations are computed within it. Moves 1 and 6 (`admit-member`, `withdraw-member`) operate at group level.

## §2c The board is a basket of references

**RATIFIED 2026-08-19.** The UBO board is **separate from the entities themselves** — it holds links to them, for those that are relevant members. Entities live independently and are shared; the board references them. Consequences: an entity ceasing to exist is **not** a board concern — it is removed from the board by withdrawing its linkages and membership (moves 5 and 6). No tenth move is needed, and the board never owns an entity's own lifecycle.

## §3 The moves

Nine moves. Every one records; none blocks on proof (CTN-2e). Each is an assertion with provenance, valid time and knowledge time.

| # | Move | What it asserts | Positional constraints (studs) |
|---|---|---|---|
| 1 | **admit-member** | this entity is a member of this group | group exists; entity instance exists |
| 2 | **assert-type** | this entity is of this type | entity exists; supersedes any prior type assertion |
| 3 | **assert-linkage** | source holds *pipe* over target | **type geometry (§2, §2a) permits it**; both endpoints exist; no duplicate active linkage of the same kind |
| 4 | **attach-evidence** | this document/source evidences a type or a linkage | target assertion exists and is not withdrawn |
| 5 | **withdraw-linkage** | this linkage is not, or is no longer, true | linkage exists and is active; **must state cessation or correction** (CTN-2g) |
| 6 | **withdraw-member** | this entity is not, or is no longer, a group member | membership exists and is active |
| 7 | **correct-type** | the type was wrong (not: has changed) | entity exists; triggers the §4 cascade |
| 8 | **record-enquiry** | sources consulted, searches run, date — the diligence record | group exists |
| 9 | **construct** | build the taxonomy at (T, axis, scope) | pure; reads only; never mutates |

**Deliberately absent:** any move that sets a status. Status is computed from evidence and horizon (CTN-2f), never asserted. Any move that deletes. Any move that renders a verdict — that is D2.

## §3a Classes are not entities

A class of beneficiaries ("the settlor's issue") is a real target of pipe 10 and is not an entity instance. It is a **described set** with its own assertion and its own evidence, and it terminates traversal with a recorded rationale rather than resolving to natural persons. Modelling it as a pseudo-entity would put a fiction in the block set.

## §4 The type-correction cascade

`correct-type` (move 7) is the most disruptive move on the board, because the type is what makes linkages possible at all. When a type is corrected, linkages asserted under the old type may no longer be permitted by §2.

**The rule:** affected linkages are **never silently deleted**. They demote to a flagged state requiring re-assertion under the corrected vocabulary, any determination that traversed them is marked stale, and the correction records which linkages it invalidated. A type correction that quietly orphans half a structure is the worst outcome available on this board.

**Note the asymmetry with `withdraw-linkage`:** correcting a type says the *world* was misdescribed; withdrawing a linkage may say either that (correction) or that the relationship ended (cessation). Move 5 must state which; move 7 is always correction — a vehicle does not change what it is, though it may be wound up, which is a different assertion again.

## §5 What this replaces in existing work

- **T2's placement set** currently derives legality from lexicon preconditions alone. It must take the **type→linkage matrix as its primary constraint source**, with the ratified studs as the positional layer on top. Type geometry first, position second.
- **`structure_class` dispatch retires *at the placement layer*.** Entity type is the geometry key. **Corrected in execution (2026-08-19):** `StructureClass` also drives *determination-strategy selection* for `select-strategy`/`freeze`, which is **D2-adjacent and out of D1 scope** — it stays exactly as it is. Only the placement/geometry key changes. Full retirement awaits a per-entity-type strategy-selection design, which is separate work.
- **Known vocabulary debt (accepted in execution, must not be forgotten):** TS.0's 17-pipe vocabulary is finer than the substrate's existing 8-variant `EdgeKind`. Rewriting `EdgeKind` would have meant a fold rewrite plus every fixture, so a separate `Pipe` enum plus an `EdgeKind → Pipe` classifier was introduced instead. **Consequence: the matrix is fully encoded but only partially enforceable** — geometry can only bite on pipes `EdgeKind` can express. Officer, board, mandate, membership, statutory, employment, unit-issuance and containment linkages are declared in geometry and not yet assertable. Two vocabularies joined by a classifier is the drift shape this programme keeps paying for; convergence onto one is required work, with the classifier pinned by a tooth until then.
- **The ratified stud rows stand**, re-read as positional constraints: registration before assertion, no duplicate active linkage, linkage lifecycle, decision finality.
- **Nothing in D2 is in scope**: the 49 `cross_slot_constraints`, the nine hand-written `enforce_*` predicates, clearance, and the assurance/policy document are all assessment, not fact-establishment. They queue behind D1 and move to the policy layer when it exists.

## §6 Gate tests (RED first)

- `type_geometry_refuses_impossible_linkage` — a voting-share linkage into a partnership is refused as *not a move*, with a distinct error from a stud violation.
- `type_geometry_permits_every_matrix_cell` — property over §2/§2a: every permitted (source type, pipe, target type) triple is admitted on an otherwise-empty board.
- `matrix_is_exactly_known` — RED-honest pin of the full grid; adding a type or pipe forces a conscious edit.
- `person_is_never_a_target` — no pipe may terminate *into* a natural person.
- `officer_and_employment_are_person_sourced` — pipes 6 and 14 refuse corporate sources.
- `mandate_refuses_person_source` — pipe 7 refuses a natural-person source.
- `correct_type_demotes_never_deletes` — after a type correction that invalidates linkages, the linkages exist in a flagged state, the determination is stale, and the record names what was invalidated.
- `class_target_terminates_with_rationale` — a class beneficiary terminates traversal and records why, rather than resolving to persons.
- `no_move_sets_status` — structural: no move in §3 writes a status field.
- `construct_is_pure` — `construct` performs no writes against a live store.

## §7 Open questions — ALL RESOLVED 2026-08-19

**Q1 — Source restrictions.** RESOLVED: 6 and 14 natural-person-sourced; 7 corporate-only (§2a).
**Q2 — Corporate directors.** Standing as drafted: an attribute on the source, not a separate pipe — consistent with "the entity type decides the links".
**Q3 — Group versus anchor.** RESOLVED: KYC is group-level down; the group is the unit of work (§2b).
**Q4 — Wind-up.** RESOLVED: not a board concern. The board is a basket of references (§2c); an entity ceasing to be is removed by withdrawing its linkages and membership. No tenth move.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-19 | Initial draft on ratified TS.0. Establishes D1's board: blocks are typed entity instances, studs are the **type→linkage matrix** (§2 target-side, §2a source-side) — the primary constraint, previously absent. Nine moves, none blocking on proof, none setting status, none rendering a verdict. Classes modelled as described sets rather than pseudo-entities. Type-correction cascade demotes and records, never deletes. Ten gate tests. Names what this replaces in T2 and what queues behind D1 as assessment work. |
| 0.2 | 2026-08-19 | **RATIFIED.** All four open questions closed. §2a source restrictions ruled: officer (6) and employment (14) natural-person-sourced, management mandate (7) corporate-only — under the general principle that **the entity type decides the links**; management companies and partnerships are hybrids, holding the mandate as a corporate while their own managing individuals reach them as officers. New §2b: **KYC runs group-level down** — the group is the unit of work and may contain several subjects needing determinations. New §2c: **the board is a basket of references**, separate from the entities themselves; an entity ceasing to exist is therefore not a board concern and needs no tenth move — its linkages and membership are withdrawn. Corporate directors stand as a source attribute, not a pipe. |
