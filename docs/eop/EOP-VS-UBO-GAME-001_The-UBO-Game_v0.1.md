# EOP-VS-UBO-GAME-001 — The UBO Game
### One board. Two games. Written plainly, from the graph rather than the history.

| | |
|---|---|
| **Document** | EOP-VS-UBO-GAME-001 |
| **Version** | 0.1 — draft for ratification |
| **Supersedes** | EOP-VS-KYCUBO-001 v0.6 as the statement of intent. That document's determination semantics (K-1 to K-35, the prong cascade, the per-class control axis) remain law and are cited, not restated. Its obligation graph (§7.3-7.5, K-21 to K-28) is **withdrawn** — dissolved by D2.0 §5. |
| **Status** | DRAFT. §3's move set is the ruling surface. §7 disposes of every verb that exists today. |

---

## §1 The game, in plain terms

A **board** is a picture of who owns and controls what, for one client group, built up piece by piece.

**Blocks** are entities — companies, partnerships, funds, trusts, people. Each block has a **type**, and the type decides what can connect to it. A company has share classes, so voting connects. A partnership has an agreement, so partner designation connects. A person is never something you connect *into* — traversal ends there.

**Links** are the connections — ownership, voting, board seats, mandates, trustee powers. Each link has a kind, a direction, and evidence behind it or not.

**Position is what the links imply.** There is no separate hierarchy to maintain: who sits above whom falls out of the connections. Nothing stores depth or rank.

**Two games are played on this one board.**

**The build game** assembles it. You place blocks, connect them, attach evidence, retire what turns out to be wrong. Everything starts as an **allegation** — the client told us, or research suggested it — and is proven where it can be. You never wait to record something because you can't yet prove it.

**The inspect game** reads it. At any moment, against any state of the board, you run checks: does this meet regulation, policy, our own standards? It needs no permission and no completeness. If the board is half-built, checks fail, and **the failures are the work list**.

That is the whole idea. It is a typed graph builder and a rule runner over the same graph — no harder in principle than an abstract syntax tree, and with fewer node kinds than most.

## §2 The board

**The board never touches the entities.** Entities exist independently and are shared; the UBO game owns none of them and deletes none of them. What the game writes is **metadata about entities** — that this one is on this board, of this type, connected to that one — and that metadata is what defines membership and placement. `remove` withdraws a placement, never an entity. Nothing the build game does can destroy a block.

**The board is a selection from the group.** The group is everything discovered — every entity research or the client turned up. The **UBO board is the subset that matters to this determination**, expressed as metadata over those entities. Placing a block records that a group member belongs on the board; removing it withdraws that record. The entity is untouched and remains available to be placed again.

**No structures in the store.** This is the general rule, not a UBO detail: the database holds entities and metadata, and **every taxonomy is built on the fly in code**. There is no stored graph, no persisted hierarchy, no adjacency table. Relational stores are poor at traversal and worse at answering "what did this look like in March"; both problems disappear when the structure is a function rather than a table.

**Blocks.** An entity instance, of a declared type, with attributes. Natural persons are blocks like any other; they are simply where traversal terminates. The type catalogue is EOP-DD-KYCUBO-TS.0 §4 — ratified, and unchanged by this document.

**Links.** A typed connection from one block to another. The kind vocabulary and which kinds may connect which types is EOP-DD-KYCUBO-TS.1 §2/§2a — the geometry — ratified and unchanged.

**Status is composite, and it is a quality check rather than a flag.** Every block's placement and every link carries an epistemic status: **alleged** — someone told us — through to **verified** — the proofs have been obtained and logged. **Verified is a judgment over the set of citations, not a state anyone sets.** An assertion may need more than one *kind* of proof, and it is verified when the citations satisfy what that kind of assertion requires. So verification is a derived quality of a placement or a connection, computed from what has been logged against it.

Nothing is gated on proof; proof gates *conclusions*, not *records*. Which proof kinds an assertion requires is domain vocabulary and sits with the type and link catalogues; whether the resulting quality is *good enough to clear* is a policy question and belongs to the inspect game.

**Scope.** The board is built for a group and viewed by someone. What a viewer sees is decided when the board is constructed, not filtered afterwards — a percentage computed over a scoped view is a statement about that view.

**Time.** Every assertion carries when it was true and when we learned it. The board can be constructed as at any past moment, in either axis, and the answer is the same every time because the assertions are immutable.

**What the board is not.** It is not stored as a structure. The store holds blocks, links and their attributes; the board is constructed on demand by folding them. There is no snapshot that can drift from its source, because there is no snapshot that is authoritative.

## §3 The build game

### §3.1 The move set — eight moves, four reversible pairs

Derived from the graph, not from what exists. Two moves merge only if they differ **by a value** *and* share their preconditions; where preconditions genuinely differ, the split is carrying a rule and collapsing it would hide that rule in a body where nothing checks it.

| Move | What it does | Arguments |
|---|---|---|
| **`place`** | put a group member on the board, in this slot, as this type | group, entity, entity type, attributes |
| **`remove`** | take it off the board | group, entity |
| **`connect`** | a link exists, from this block to that one, of this kind | group, from, to, kind, kind-specific attributes |
| **`disconnect`** | that link is not on the board | group, link |
| **`evidence`** | log a proof, of this kind, supporting that assertion | group, target (a placement or a link), proof kind, document reference |
| **`retract`** | that proof no longer stands | group, citation |
| **`enquiry`** | this is what we searched, and where | group, sources consulted, searches run, date |
| **`freeze`** | commit to the computed determination as the answer of record | group, subject, policy version |

**Every building action is reversible, because building is what this is.** You place a block and you take it off; you connect two blocks and you disconnect them; you cite a document and you retract the citation. Breaking and rebuilding is normal working, not exception handling — the opening state is a board of allegations and material rework is expected as diligence proceeds.

**`enquiry` has no inverse** — you cannot un-search. **`freeze` has no inverse** — a determination of record is superseded by a later freeze, never withdrawn.

**In or out. The reason is the board.** A block is on the board or it is not; a link is on the board or it is not. The UBO board is the context, and that is all the reason a removal needs. No lifecycle vocabulary, no ceased-versus-corrected taxonomy on the move itself — the assertion stream already records *when* something went on and *when* it came off, which is what a reader actually needs.

**Reversal is not deletion, and never touches an entity.** `remove` withdraws a placement; the entity is untouched and remains in the group, available to be placed again. Everything is append-only underneath (R5), so the board as at any past moment is exactly what it was.

**Eight, not nineteen — four reversible pairs.** What multiplied the old set was *lifecycle* concerns promoted into *vocabulary*: evidence and correction are things that happen **to** an assertion, not different kinds of assertion.

### §3.2 What each move absorbs, and why

**`place` absorbs register + assert-type.** The entity's type is a fact about the entity, not about this board — but it is an **argument to the placement**, because the slot assembly is where the type does its work: it decides which links can reach the block. Placing a block without saying what it is has no use, since a typeless block is inert. One move, and the seven-phase sequence collapses because "instantiate" and "prove type" stop being separate acts of expression.

**`connect` absorbs assert-control + assert-economic-interest.** They differ by kind, and geometry already validates the classified pipe. Their preconditions are the same: both endpoints exist, geometry permits the triple, no duplicate active link of that kind. **This merge passes the test.**

**`remove` absorbs member-withdrawal**, and **type-correction** in combination: a wrong type is `remove` then `place` with the right one. Rebuilding is the normal way to correct a board.

**`disconnect` absorbs supersession.** The link is no longer on the board.

**`retract` is new, and it is the demotion path.** Withdrawing a citation is how a proven assertion returns to alleged — evidence found to be forged, a source retracted, a horizon passed. Status is computed from citations (§2), so removing a citation demotes by construction rather than by a status-setting move.

**`freeze` is not an assertion and is the odd move out.** Nothing is handed to us; it commits to something computed. Three of its rules cannot be preconditions because they need the determination's own output. **Consequence, ruled: a surface that cannot compute the basis may not produce the verdict.** Freeze is unavailable to any surface that does not run the determination.

### §3.3 What is deliberately absent

**No `structure-class`.** `place` carries the type, and the type is the dispatch key (TS.0 §1). A second vocabulary for what a thing is was ruled against and is now removed rather than half-removed.

**No `reconcile`.** Two conflicting assertions both stand, each with provenance. Resolution is `disconnect` the one that is wrong, or `connect` the one that is right. A third path to what two moves already do is how vocabularies grow.

**No reason vocabulary.** In or out is the whole distinction. The stream records when a thing went on the board and when it came off; that is what a reader needs, and a lifecycle taxonomy on top of it is navel-gazing.

**No status-setting move.** Status is computed (§2). A move that sets it would be a fact about our bookkeeping, not about the world.

**No obligation moves.** Dissolved (D2.0 §5). Checks run and fail; the failures are the work list.

**No verb per entity type or per link kind.** Those are arguments. Nineteen verbs became nineteen alignment surfaces; six become six.

### §3.4 The rules of the build game

**R1 — Record freely, conclude carefully.** Nothing is gated on proof. A block is placed and a link connected on a client's word alone; that is the normal opening state, not a degraded one. Proof gates conclusions.

**R2 — The type decides the links.** Geometry is consulted at the moment of writing, on the real triple, through the one chokepoint every surface uses. A link the types forbid is *not a move* — a different refusal from a link that is merely illegal in this position.

**R3 — Unknown is alleged.** There is no third state. An entity whose type nobody knows is not "unclassified"; it is alleged, and the existing epistemic model already carries that.

**R4 — Provisionality propagates through decisions, not only facts.** If a link was admitted on the basis of a type that is itself only alleged, the conclusion drawn from it is provisional — and the record says *which* of the three reasons applied.

**R5 — Nothing is deleted.** `retire` ends an assertion prospectively or corrects it retroactively, and says which. The board as at any past moment is exactly what it was.

**R6 — Every surface builds the same event.** One function maps declared arguments to a stored event; every surface calls it. A second constructor is how two surfaces come to disagree, and it is forbidden rather than merely discouraged.

**R7 — Every rule is enforced where writes happen.** A rule expressed only in one surface's code is not a rule. Preconditions are declared on the move and evaluated at the chokepoint.

## §4 The inspect game

### §4.1 The shape

A **check** declares when it applies and what it tests. The applicable set is **computed from the board** — a trust present pulls in trust checks — never configured per client. Running the checks produces a **run**: a record pinning the board it read, the policy version in force, when, who triggered it, and per check a verdict.

**Three verdicts.** *Pass.* *Fail* — a finding, citing the facts it rests on. *Unevaluable* — in scope, and the facts are not there yet, with the reason recorded. A check that cannot yet be evaluated has not failed, and collapsing the two loses the difference between a finding and a task.

**The work list is derived** — the failing and unevaluable findings of the latest run. Not stored, not tracked, not reconciled.

**Staleness is a comparison** — the board's hash now against the hash the run pinned. Two clocks run independently: facts age as evidence passes its horizon, and policy changes as the pack version moves. Either triggers a re-run.

### §4.2 The rules of the inspect game

**I1 — It runs at any board state.** No completeness gate, no readiness check, no orchestration. A board of three alleged links is a legitimate input.

**I2 — Checks are pure over the board.** They read and return findings. They write nothing to the fact record. A check that reaches outside — a live sanctions call — is not a check: the call is a *fact-gathering* act on the build side, and the check assesses the recorded result.

**I3 — Runs are append-only.** Re-running creates a new run. What was concluded then stands as what was concluded then.

**I4 — A verdict cites its basis.** An approval, rejection or waiver names the run it relied on. A verdict with nothing cited is refused.

**I5 — Failure is the output, not an error.** A board that fails its checks has not malfunctioned. The findings are the point.

## §5 Rules common to both games

**C1 — One board, two games; a move may be legal in both.** Ownership is not exclusive — some moves genuinely belong to both games. What must hold is **completeness**: every move is claimed by at least one game, every claimed move exists, and every move the board offers in a game is dispatchable in that game.

**C2 — The board offers only moves that will be admitted.** What a session is shown and what the write path accepts are the same computation, over the same triple.

**C3 — Every declared thing is owned, reachable and dispatchable.** The general form of every alignment failure this system has had. It is checked mechanically or it drifts.

**C4 — The board is computed, never stored.** Any cached construction is regenerable and content-hashed; truth is the assertions plus the constructor.

**C5 — The vocabulary is expected to change.** This is a dynamic taxonomy: new types, new link kinds, new checks. Alignment therefore cannot be a one-time reconciliation; it is a property that must hold as the vocabulary evolves, which means a gate rather than a cleanup.

## §6 What is out of scope

**The check catalogue's contents.** What a sanctions, threshold or jurisdiction check tests needs compliance input, changes constantly, and is deliberately not architecture. The machinery is in scope; the rules are not.

**CBU and Deal.** Containers of the same kind (EOP-VS-CONTAINERS-001), and this design should generalise to them — but generalising is a later act, not a claim made here.

**The determination's semantics.** EOP-VS-KYCUBO-001's prong cascade, per-class control axis and K-invariants stand. This document changes how moves are expressed, not what a determination means.

## §7 The suspect list — disposition of all nineteen existing verbs

"In the vicinity is suspect." Every verb that exists today, against the six.

| Existing verb | Disposition |
|---|---|
| `assert.subject.register` | → **`place`** (merged with type) |
| `assert.subject.type` | → **`place`** |
| `assert.subject.type-correction` | → **`remove`** + **`place`** (rebuild with the right type) |
| `assert.subject.member-withdrawal` | → **`remove`** |
| `assert.subject.enquiry` | → **`enquiry`** (unchanged in intent; its payload keys are broken on one surface) |
| `assert.subject.structure-class` | **REMOVE** — the type is the dispatch key (TS.0 §1). Second vocabulary for one concept |
| `assert.edge.control` | → **`connect`** |
| `assert.edge.economic-interest` | → **`connect`** (differs by kind; preconditions identical) |
| `assert.edge.evidence` | → **`evidence`** |
| `assert.edge.verification` | **REMOVE** — verified is a **composite quality derived from the logged proofs** (§2), not a state anyone sets. An assertion may need several kinds of proof; it is verified when the citations satisfy what that kind of assertion requires. A move that stamps it asserts our bookkeeping rather than the world. Landing this requires the derivation and the per-assertion proof-kind requirements to land with it |
| `assert.edge.supersession` | → **`disconnect`** |
| `assert.edge.reconciliation` | **REMOVE** — a third path to `disconnect` the wrong one or `connect` the right one. Its one declared argument is read by no fold arm |
| `assert.entity.screening` | → **`place`** attributes (a fact about a block). Currently broken and moot |
| `assert.entity.identity` | → **`place`** attributes. Same |
| `assert.entity.risk` | → **`place`** attributes. Same |
| `decide.determination.freeze` | → **`freeze`**, with §3.2's ruling: unavailable to any surface that cannot compute the basis |
| `decide.subject.approve` | inspect game — unchanged |
| `decide.subject.reject` | inspect game — unchanged |
| `decide.obligation.waiver` | inspect game — retarget from an obligation to a **finding** in the cited run |

**Nineteen → eight build moves plus three inspect verdicts.** Three removals; `verification`'s carries the proof-kind derivation with it.

## §8 Open questions

**Q1 — `place` on a block already on the board, with a different type.** Is that `remove` then `place`, or does `place` supersede in one move? *Recommendation: `remove` then `place` — rebuilding is the normal way to correct a board, and it keeps `place` from carrying update semantics.*
**Q2 — RESOLVED** by the reversible pairs: the awkward one-move-over-three-targets question disappears.
**Q3 — `connect` and the link id.** Today a caller may supply one, which makes the move unstageable through the board, or omit it and lose the ability to reference it. *Recommendation: the system mints it and returns it; a caller-chosen id is a stored identifier the board cannot predict.*
**Q4 — RESOLVED 2026-08-25.** Verified is a composite quality derived from the logged proofs (§2). No move sets it; `evidence` carries a **proof kind**, and verification is set-satisfaction over what has been logged. Which proof kinds an assertion requires is domain vocabulary; whether the result is good enough to clear is policy.
**Q5 — game membership.** Which moves are legal in both games? Reading the board plainly is; `freeze` is arguably the seam. Needs stating so §C1's completeness property has something to check against.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-25 | Initial draft, written from the graph rather than the accumulated vocabulary, after a gap-to-vision assessment found the human surface could not build a board at all and a per-verb study found nine of sixteen moves silently broken through it. Establishes one board and two games; six build moves against nineteen existing verbs; three removals with `verification`'s dependency named. Supersedes EOP-VS-KYCUBO-001 v0.6 as the statement of intent while preserving its determination semantics; withdraws its obligation graph, dissolved by D2.0 §5. R6 (one event constructor, every surface) and C3 (every declared thing owned, reachable, dispatchable) are the two rules this system has repeatedly broken and are stated as law rather than practice. |
