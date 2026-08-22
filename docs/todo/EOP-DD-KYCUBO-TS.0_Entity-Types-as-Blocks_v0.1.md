# EOP-DD-KYCUBO-TS.0 — Entity Types as Blocks
### The core block set, the control-pipe vocabulary, and the UBO construction sequence

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-TS.0 |
| **Version** | 0.2 — pruned to the confident core (ruled 2026-08-19: "if in doubt, leave it out") |
| **Binds to** | EOP-VS-KYCUBO-KIT-001 v0.2; EOP-VS-KYCUBO-001 v0.6; EOP-VS-CONTAINERS-001 v0.2 (CTN-2a/2b/2e–2h) |
| **Supersedes** | The "four mechanism families" sketch of 2026-08-19 — that analysis dispatched off `structure_class`, which is mis-levelled (§5). |
| **Coverage** | BNY KYC is **institutional and retail** — both in scope. v0.2 carries only vehicle types and pipes held with confidence; §7 lists what was deliberately deferred rather than guessed. |
| **Status** | **RATIFIED 2026-08-19.** The catalogue (§4), pipe vocabulary (§3), construction sequence (§2) and the mis-levelling fix (§5) stand as written. Deferred set (§7) is reopenable against the real onboarding book. Entity type replaces `structure_class` as the dispatch key. Assurance, policy and the clearance decision are a **separate document** (EOP-DD-KYCUBO-ASSURANCE-001, to follow) and are **D2** — they do not gate any D1 work. |

---

## §1 The correction: the entity type is the block

Determination strategy has been dispatching off `structure_class`. That is the wrong level. **What an entity *is* determines how control over it can exist at all.** A company has share classes, so control runs through voting rights. A partnership has a partnership agreement, so control runs through the general partner or the agreement's terms. A fund with an external management company has its directing mind outside itself. The control mechanism is not selected by a strategy — it is *implied by the vehicle*, and the sub-type carries the attributes that pin it down.

So the kit's core blocks are **entity types**. Each declares which **control pipes** can legitimately connect to or from it. That is stud geometry one level below where the kit has been applying it, and every determination strategy is downstream of it.

## §2 The construction sequence

Every phase *records*; none blocks (CTN-2e). Each produces assertions that begin as allegations.

| Phase | What happens | Status on exit |
|---|---|---|
| **P0 — Scope the group** | Establish which group is being onboarded; discover candidate members (client statement, GLEIF/LEI research, registry search). | Membership set **alleged**, with sources and search date. Completeness is a diligence assertion, never a proof (§6.3). |
| **P1 — Instantiate entities** | Create each discovered member as an entity instance, natural persons included. | Instances **alleged**; type **alleged**. |
| **P2 — Prove the type** | Establish what each entity actually is, from its constitutional evidence (§4). | Type **proved**, or still alleged. |
| **P3 — Derive the pipes** | The proven type determines which control-pipe kinds are permitted (§3, §4). | Permitted pipe set — derived, never stored. |
| **P4 — Connect** | Assert control and economic links using only permitted pipes. | Links **alleged**. |
| **P5 — Prove the links** | Obtain evidence per link (share register, partnership agreement, trust deed, board filing, employment contract). | Links **proved** where obtainable. |
| **P6 — Determine** | Construct the lineage; compute the determination over it. | Determination + assurance profile (separate document). |

**P2 gates P3, and P3 gates the legality of P4 — but nothing gates recording.** A link may be asserted over an entity whose type is still alleged; what it cannot be is *verified*, because the pipe's validity is not yet established.

## §2a Compound assurance

A link's assurance is bounded by the **weakest** of three things: the source entity's type status, the target entity's type status, and the link's own evidence. A perfectly evidenced share-register entry into an entity whose type is still alleged is **not** a verified control link — you do not yet know that voting shares are the right pipe for that vehicle. This extends CTN-2h's weakest-link rule from edges to types.

## §3 The control-pipe vocabulary (the studs)

Thirteen pipes, each a distinct edge kind with its own evidence expectation and traversal behaviour. Pruned to those held with confidence; deferred candidates are in §7.

| # | Pipe | Confers | Traversal | Typical evidence |
|---|---|---|---|---|
| 1 | **Voting shares / class rights** | control + economic | continue, multiply | share register, cap table, articles |
| 2 | **Non-voting / preference shares** | economic only | economic aggregation | share register, articles |
| 3 | **General-partner designation** | control | continue via the GP | partnership agreement, registry |
| 4 | **Limited partnership interest** | economic only | economic only | partnership agreement, LP register |
| 5 | **Board / director appointment** | control | continue to appointee | registry filing, board minute |
| 6 | **Officer / senior management appointment** | control (the SMO population) | terminal population | registry, contract |
| 7 | **Management mandate** (ManCo / AIFM / adviser with governing mandate) | control | **re-anchor** (TS.2 pivot) | management agreement, prospectus, regulator register |
| 8 | **Trustee powers** | control (fiduciary) | enumerate role-holders | trust deed |
| 9 | **Reserved powers** (settlor, protector) | control | enumerate | trust deed |
| 10 | **Beneficiary entitlement** | economic; control only where fixed and vested | enumerate, or record as a class | trust deed, distribution records |
| 11 | **Membership rights** (cooperative, mutual) | control, typically one-member-one-vote | terminate with recorded rationale | rules, member register |
| 12 | **Statutory authority** | control by law, not ownership | terminate with recorded rationale | statute, instrument of incorporation |
| 13 | **Contractual control** (shareholder agreement, veto rights) | control | continue | the agreement itself |
| 14 | **Employment / delegated authority** | control (natural persons) | terminal | employment contract, delegation |
| 15 | **Unit / share issuance to investors** | economic only — **never control** | never contributes to control | register of unitholders |
| 16 | **Pooled-asset containment** (umbrella → sub-fund) | neither | scoping boundary | constitutional documents |
| 17 | **Nominee holding** (a capacity, not a type — §5) | passes through | **pierce and substitute**, then continue | nominee agreement, declaration of trust |

**Ruled (was Q2):** contractual control stays a single pipe. Splitting it into shareholder agreement / veto / golden share adds vocabulary without changing traversal, and golden shares are rare enough to be a deferred case.

## §4 The entity-type catalogue

Pipe numbers refer to §3. "Type proof" is the evidence establishing what the entity *is* (P2), distinct from evidence for any link.

### 4.1 Natural persons (retail core; the terminus of every institutional traversal)

| Type | Key attributes | Pipes it can hold | Type proof |
|---|---|---|---|
| Natural person | nationality/ies, residence, date of birth, PEP status, occupation | 1,2,3,4,5,6,8,9,10,13,14 (as holder) | identity document, verified biographic record |
| Sole trader | no separate legal personality — control **is** the person | collapses to the person | registration where it exists, otherwise the person |

### 4.2 Corporates

| Type | Key attributes | Permitted control pipes | Type proof |
|---|---|---|---|
| Private limited company | jurisdiction, share classes, registry number | 1,2,5,6,13 | certificate of incorporation, registry extract, articles |
| Public / listed company | listing venue, regulated-market status | 1,2,5,6,13 | registry extract plus listing evidence |
| LLC (US) | **member-managed vs manager-managed** — decisive for where control sits | 1,5,6,13 | operating agreement, state filing |

*Listed-company enquiry relief is a **policy** rule, not a structural one — it belongs in the assurance document.*

### 4.3 Partnerships

| Type | Key attributes | Permitted control pipes | Type proof |
|---|---|---|---|
| General partnership | all partners hold control | 3,5,13 | partnership agreement |
| Limited partnership | GP controls; LPs economic only | 3,4,7,13 | LP agreement, registry |
| LLP | member-managed per the agreement | 3,5,13 | LLP agreement, registry |

### 4.4 Funds and collective investment

| Type | Key attributes | Permitted control pipes | Type proof |
|---|---|---|---|
| OEIC / ICVC | corporate fund; ACD or ManCo directs | 5,7,15,16 | instrument of incorporation, prospectus |
| SICAV | corporate fund; ManCo or self-managed | 5,7,15,16 | articles, prospectus |
| Unit trust | trustee holds legal title; ManCo directs | 7,8,15,16 | trust deed, prospectus |
| 40 Act fund (US registered investment company) | board of directors plus investment adviser | 5,7,15,16 | registration statement, prospectus |
| LP fund (private equity / hedge) | GP plus ManCo; LPs economic | 3,4,7,15 | LP agreement, offering document |
| Umbrella with sub-funds | **segregated liability** — determination per sub-fund | 16, plus each sub-fund's own | umbrella and sub-fund documents |

### 4.5 Trusts and fiduciary structures

| Type | Key attributes | Permitted control pipes | Type proof |
|---|---|---|---|
| Discretionary trust | trustee discretion; class of beneficiaries | 8,9,10 | trust deed |
| Fixed / bare trust | beneficiary entitlement is fixed | 8,10 | trust deed |
| Foundation | founder, council, beneficiaries — **role enumeration, not traversal** | 8,9,10 | foundation charter, registry |
| Pension scheme | trustee-governed; members are beneficiaries | 8,10 | trust deed, scheme rules |

### 4.6 Mutual, sovereign and not-for-profit

| Type | Key attributes | Permitted control pipes | Type proof |
|---|---|---|---|
| Cooperative / mutual | one member one vote; members individually immaterial | 5,6,11 | rules, registry |
| Charity / not-for-profit | trustees or directors; no owners | 5,6,8 | charity registry |
| Government department / statutory corporation | control by **statute**, not ownership | 5,6,12 | statute, instrument |
| Sovereign wealth vehicle | **whatever vehicle it actually is** — a company, a statutory corporation or a fund; that choice determines the pipes | its actual type's pipes, plus 12 | statute plus the vehicle's own documents |

## §5 What is NOT an entity type (the mis-levelling fix)

The existing `structure_class` enum mixes three different kinds of thing, which is why six of its eleven values resisted having a strategy written for them — several are not strategy-shaped at all.

| Concept | What it actually is | Where it belongs |
|---|---|---|
| **Nominee** | a **capacity** held in a particular edge, not what the entity is (a nominee is usually a company or a person) | pipe 17 — a traversal rule that pierces and substitutes, available **during any strategy**, never a class strategy of its own |
| **State-owned** | a **fact about who sits at the top of the chain**, not a vehicle | a terminal condition on traversal; the vehicle's own type (§4.6) supplies the pipes |
| **Trustee / custodian** | a capacity, like nominee | pipe 8, or 17 where legal title is held for another |
| **Branch** | not a separate legal person | resolve to head office at construction |
| **Joint holders** | a relationship between persons | account-level, not entity-level |
| **ETF** | a wrapper over some other vehicle | the wrapper's own type supplies the pipes |
| **Listed status** | an attribute of a corporate type, with policy consequences | attribute in §4.2; the enquiry relief is a policy rule |

**Consequence for the plan:** the "six classes needing six strategies" framing dissolves. What is needed is this catalogue plus per-type pipe vocabularies; traversal behaviour derives from the pipes actually populated in the lineage, not from a class label.

## §6 Rules that fall out

**6.1 — Type correction cascades to edges.** If an entity's type is corrected, links asserted under the old type may no longer be permitted pipe kinds. Type correction **never silently deletes edges**: affected edges demote and are flagged for re-assertion under the corrected vocabulary, and any determination that traversed them is marked stale until resolved. This is refutation at the type level (CTN-2g), and more disruptive than edge refutation because it can orphan a whole set at once.

**6.2 — Permitted pipes are derived, never stored.** The pipe set is a function of the proven type, computed at construction. A stored pipe list would drift from the type the moment the type is corrected (CTN-2f).

**6.3 — Completeness is a diligence assertion, not a proof.** A member's existence is provable; the *absence* of undiscovered members is not. Group completeness records sources consulted, searches run and the date — sufficiency of enquiry — and carries into the assurance profile as a first-class item rather than being implied by silence.

**6.4 — Natural person is the terminus, and a type like any other.** Every traversal is trying to reach it; persons hold pipes exactly as companies do.

## §7 Deliberately deferred (ruled 2026-08-19: "if in doubt, leave it out")

Not judged unnecessary — judged **not yet held with enough confidence to encode**. Each returns only when someone with direct knowledge of BNY's book confirms both the vehicle and its control mechanism.

**Vehicle types deferred:** FCP and other contractual funds without legal personality; Delaware statutory trust; Scottish LP; unlimited company and unlimited partnership; SPV and securitisation vehicles including orphan-trust structures; purpose and charitable trusts as distinct from the charity type; Liechtenstein Anstalt and comparable civil-law forms; association and club; central bank and supranational.

**Pipes deferred:** power of attorney and signatory authority (an authority concept, not a beneficial-ownership pipe — likely belongs to mandates rather than here); golden share as distinct from contractual control (13).

**Attributes deferred:** jurisdiction-specific registry identifier kinds beyond LEI and company number.

## §8 Open questions

**Q1 — Coverage against the real book.** Which of §7's deferred types actually appear in BNY's onboarding queue, and in what volume? That, not completeness for its own sake, decides what returns first.
**Q2 — RESOLVED.** Contractual control stays one pipe (§3).
**Q3 — Attribute depth.** Which attributes are control-relevant (belonging in the block) versus merely descriptive (belonging in entity metadata)?
**Q4 — Type-proof standards.** What evidence proves each type to KYC/AML standard, and does the standard differ between institutional and retail? Overlaps the assurance document.
**Q5 — Where the catalogue lives.** It is vocabulary, so it wants a single declaration source (CTN-13) feeding both code and config — which interacts with the R0/R5 declaration-surface convergence.

## §9 Sequencing consequence

TS.0 replaces the four-mechanism-families plan and **precedes every TS tranche**. Once catalogue and pipes are ratified and encoded, the per-class strategies largely become derivations rather than bespoke implementations. The kit plan's TS section should be re-cut around this: catalogue and pipes first, then traversal behaviour per pipe, then residual per-type rulings.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-19 | Initial draft. Entity type as the kit's core block, control pipe as the stud; supersedes the four-mechanism-families sketch. Seven-phase construction sequence; compound assurance bounded by the weakest of source type, target type and link evidence; eighteen pipes; catalogue across six families; §5 separates capacities, chain facts and non-entities from genuine types. |
| 0.2 | 2026-08-19 | **Pruned to the confident core** per ruling "if in doubt, leave it out". Vehicle types cut from ~30 to 20 and pipes from 18 to 17; everything removed is listed in §7 with the reason (confidence, not irrelevance) so nothing is silently lost. Q2 resolved: contractual control stays a single pipe. ETF added to §5 as a wrapper rather than a type. Deferred set is explicitly reopenable against the real onboarding book (Q1). |
