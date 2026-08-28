# EOP-VS-UBO-GAME-001 — The UBO Game
### One board. Two games. Written plainly, from the graph rather than the history.

| | |
|---|---|
| **Document** | EOP-VS-UBO-GAME-001 |
| **Version** | 0.1 — draft for ratification |
| **Supersedes** | EOP-VS-KYCUBO-001 v0.6 as the statement of intent. That document's determination semantics (K-1 to K-35, the prong cascade, the per-class control axis) remain law and are cited, not restated. Its obligation graph (§7.3-7.5, K-21 to K-28) is **withdrawn** — dissolved by D2.0 §5. |
| **Status** | DRAFT. §3's move set is the ruling surface. §7 disposes of every verb that exists today. |

---

## §0 The problem

**What KYC actually asks.** Who ultimately owns and controls this client? Answer it for a corporate group that may run to dozens of entities across several jurisdictions, where ownership runs through holding companies, control runs through management mandates that own nothing, nominees hold shares for people whose names are elsewhere, and funds are directed by a company that is not part of them.

**Four things make that hard, and they are what this design is built against.**

**1. You are told before you are shown.** A client describes their structure; research suggests more. None of it is proven on the day it is recorded, and much of it never will be. A system that requires proof before it will record a fact cannot represent the first six weeks of any onboarding — so allegation must be a first-class state, not an error, and proof must be something that arrives later and can be withdrawn again.

**2. The answer must survive being asked years later.** A regulator asks what you concluded in March, on what basis, and why. That is two different questions — *what was true then* and *what did you know then* — and a system that stores current state can answer neither. If any conclusion is stored rather than derived, it drifts from the facts underneath it and nobody notices.

**3. The structures are wildly various, and the rules follow the vehicle.** A company has share classes, so control follows votes. A partnership has an agreement, so control follows designation. A trust has neither, so control is the fiduciary roles. A fund is directed from outside itself. Encode those as special cases and you get a system that is confidently wrong on the fifth structure it meets — which is worse than one that refuses, because nobody looks at a plausible answer.

**4. What is *acceptable* changes constantly; what things *are* does not.** Regulations move, thresholds move, policies differ by region and by product. How a limited partnership expresses control has not changed in a century. Bind those together and every policy tweak destabilises the model of the world.

**And one thing that makes it harder than it sounds.** The people doing this work are not modellers. They are analysts assembling a picture from documents, calls and registry searches — building, correcting, and rebuilding as diligence proceeds. A system that presents this as form-filling over a fixed schema fights them; a system that lets them place a piece, connect it, take it out again and try another arrangement matches what they are actually doing.

**What this design solves for, then:** a picture that can be assembled from allegations and hardened by proof; that can be reconstructed exactly as at any past moment in either axis; whose rules about what can connect to what come from the vehicle rather than from special cases; that separates *what is* from *whether that is acceptable* so the second can churn without disturbing the first; and that behaves, in the hands of an analyst, like building rather than filing.

Everything below follows from those five things. Several of the rulings look unusual against ordinary practice — nothing structural is stored, no state is updated in place, status is computed rather than recorded, a disconnected block is a normal position. **Each one is there because of a specific failure it prevents**, and where that is not obvious the rule says which.

## §1 The game, in plain terms

A **board** is a picture of who owns and controls what, for one client group, built up piece by piece.

**Blocks** are entities — companies, partnerships, funds, trusts, people. Each block has a **type**, and the type decides what can connect to it. A company has share classes, so voting connects. A partnership has an agreement, so partner designation connects. A person is never something you connect *into* — traversal ends there.

**Links** are the connections — ownership, voting, board seats, mandates, trustee powers. Each link has a kind, a direction, and evidence behind it or not.

**Position is what the links imply.** There is no separate hierarchy to maintain: who sits above whom falls out of the connections. Nothing stores depth or rank.

**Two games are played on this one board.**

**The build game** assembles it. You place blocks, connect them, attach evidence, retire what turns out to be wrong. Everything starts as an **allegation** — the client told us, or research suggested it — and is proven where it can be. You never wait to record something because you can't yet prove it.

**The inspect game** reads it. At any moment, against any state of the board, you run checks: does this meet regulation, policy, our own standards? It needs no permission and no completeness. If the board is half-built, checks fail, and **the failures are the work list**.

That is the whole idea. It is a typed graph builder and a rule runner over the same graph — and it has fewer node kinds than most compilers. Small, though — as §1a explains — it is not shaped like a compiler's tree at all.

## §1a Closer to Go than to a tree

The instinct most implementers bring to a graph is the wrong one here, and it is worth naming before it costs a defect.

**An abstract syntax tree, or a table with referential integrity, holds its invariant at every instant.** Every node has a parent. A dangling subtree is corrupt. A foreign key pointing nowhere is by definition a fault, and the right response is to prevent it — refuse the write, or cascade the delete.

**This board is not that. It is closer to Go.** A stone is legal on its own. Connection is built up over the course of play; groups form and dissolve as the game continues; an isolated stone is not a malformed position but an ordinary one. You throw tiles onto the board and then arrange them.

**Concretely: broken branches and disconnected blocks are normal and expected.** A board *opens* with every entity unconnected. Mid-build, blocks sit in the pool waiting to be arranged. `remove` prunes the links touching a block and leaves the far ends less connected, which is a legal position, not damage.

**So the build game has no completeness invariant.** Nothing requires the board to be connected, at any moment, ever.

**Completeness is judged, not enforced** — and by two things downstream, both of which look at a position rather than at a move. The **determination** traverses, and a traversal that finds no path resolves to nothing; `freeze` then refuses a silent determination. The **inspect game** runs a quality check against a threshold the applicable policy sets. Neither is a rule about what may be placed; both are judgments about whether the arrangement is far enough along.

This is *record freely, conclude carefully* (R1) one level up: the board tolerates any arrangement, and the conclusions drawn from it are honest about the state it is in.

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

**R5 — Nothing is deleted.** `remove` and `disconnect` take things off the board; the assertions that they were once there stand. The board as at any past moment is exactly what it was.

**R6 — Every surface builds the same event.** One function maps declared arguments to a stored event; every surface calls it. A second constructor is how two surfaces come to disagree, and it is forbidden rather than merely discouraged.

**R7 — Every rule is enforced where writes happen.** A rule expressed only in one surface's code is not a rule. Preconditions are declared on the move and evaluated at the chokepoint.

**R9 — The geometry is read forward as affordances, not only backward as refusals.** The same table that refuses an illegal write tells a session what it *may* do next. `place` offers **the entity types that may be added**; `connect` offers, for a chosen source, **the pipes valid out of it and the targets each may reach** — both directions, from `source_permits` and `target_permits`. This is a higher-order game rule: nothing new is computed, the matrix is simply consulted in the offering direction as well as the judging one. R2's rule that preview and append agree (C2) means they cannot disagree by construction — one table, two readings.

**The two halves of placing a block, which must not be conflated.** *What kind of thing may go here* is a game rule, answerable now from the geometry. *Which specific entity is it* is a reference to something that exists independently in the store, resolved by lookup. The game needs a way to **name** an entity; it does not need a pre-discovered universe to know which kinds are legal. Conflating the two makes the board look as though it requires a discovery layer before it can offer anything, and it does not.

**Granularity, ruled:** `place` offers affordances at **type level** — these kinds may be added. `connect` offers **concrete triples** — this source, this pipe, this target — because a link is only legal against real endpoints. Type-level for placement keeps the offer small; concrete for connection is what C2 requires, and it is where the quadratic term lives.

**R8 — Every prior board shape is reachable again.** No legal move may produce a board you cannot get back out of: no orphan links, no stranded placements, and no one-way doors — a move whose consequences some other rule makes unrecoverable.

The board opens as a **pool of unconnected entities** — the group universe, islands and satellites — and every block must be returnable to that pool. **`remove` is never refused.** It has no effect outside this board, so pulling a block out is always available; the links that touched it are cleaned up with it.

**RULED 2026-08-27: `remove` prunes the links that touch the block, and nothing else.** The event says only *remove this block*. The fold, seeing the block withdrawn, marks the links that touched it inactive. **It does not cascade onward.** Nothing propagates past the pruned links: the blocks at their far ends stay exactly where they are, keep every other link they have, and simply become less connected. A block that ends up with no links at all is **not** an error state — it is a normal member of the pool. A board opens with every entity unconnected, and a board with disconnected members mid-build is the same ordinary condition.

The mechanism matters: computing the pruned set **at write time and storing it in the event** is what made `type-correction` impure and its replay dependent on a caller-supplied list. Deriving it **at fold time** keeps the event pure, keeps `canonical_event_shape` pure, and makes replay recompute the same result deterministically with nothing hidden.

*(This supersedes an earlier ruling that `remove` should refuse while links touch the block. Two of that ruling's three reasons — impure payload, hidden state in the event — assumed the pruned set was written into the payload and dissolve once it is a fold consequence. The third, that a refusal tells the operator what to do, was a preference rather than a correctness argument, and it fits an administrative model rather than a building one: you pull a block out and what was attached to it comes away.)*

**Scoping, which matters once an entity sits on more than one board:** placement, links and pruning are all **per board**. The same entity may be placed on a UBO board and in several CBUs — a management company is the standard case. Removing it from one board withdraws that board's placement and prunes that board's links, and touches nothing on any other. Everything the game does, it does about *this* board.

**Consequence for reaching a prior shape:** `remove` and `place` are not exact inverses, because `place` does not restore the links that were pruned. Reaching a prior shape means placing the block and reconnecting. That is more moves, not a trapdoor — R8's property is *every prior board shape is reachable again*, not *every move has a symmetric inverse*. **Viewing** a prior state needs no moves at all: the stream is immutable, so any past board is constructed by folding to that moment.

**This remains directly fuzzable** and is the strongest property in the harness: build a random legal sequence, then assert that every intermediate board shape can be reached again by forward moves, and that a fully emptied board is the pool with no orphan links and no stranded placements. Any shape that cannot be reached again is a trapdoor with a concrete reproduction.

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

**Q1 — RULED 2026-08-27: `remove` then `place`.** Correcting a block's type is two moves, not a superseding placement. Rebuilding is the natural way to correct a board, `place` keeps meaning one thing, and the change is explicit in the stream rather than implied by an update.
**Q2 — RESOLVED** by the reversible pairs: the awkward one-move-over-three-targets question disappears.
**Q3 — `connect` and the link id.** Today a caller may supply one, which makes the move unstageable through the board, or omit it and lose the ability to reference it. *Recommendation: the system mints it and returns it; a caller-chosen id is a stored identifier the board cannot predict.*
**Q4 — RESOLVED 2026-08-25.** Verified is a composite quality derived from the logged proofs (§2). No move sets it; `evidence` carries a **proof kind**, and verification is set-satisfaction over what has been logged. Which proof kinds an assertion requires is domain vocabulary; whether the result is good enough to clear is policy.
**Q5 — game membership.** Which moves are legal in both games? Reading the board plainly is; `freeze` is arguably the seam. Needs stating so §C1's completeness property has something to check against.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-25 | Initial draft, written from the graph rather than the accumulated vocabulary, after a gap-to-vision assessment found the human surface could not build a board at all and a per-verb study found nine of sixteen moves silently broken through it. Establishes one board and two games; six build moves against nineteen existing verbs; three removals with `verification`'s dependency named. Supersedes EOP-VS-KYCUBO-001 v0.6 as the statement of intent while preserving its determination semantics; withdraws its obligation graph, dissolved by D2.0 §5. R6 (one event constructor, every surface) and C3 (every declared thing owned, reachable, dispatchable) are the two rules this system has repeatedly broken and are stated as law rather than practice. |
