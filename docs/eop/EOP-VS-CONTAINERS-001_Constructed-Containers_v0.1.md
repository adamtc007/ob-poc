# EOP-VS-CONTAINERS-001 — Constructed Containers
### A point-in-time taxonomy service for Deal, CBU and UBO
**Principle: entities are stored; containers are constructed.**

| | |
|---|---|
| **Document** | EOP-VS-CONTAINERS-001 |
| **Version** | 0.1 — draft for ratification |
| **Owner** | Adam Cearns |
| **Relates to** | EOP-SA-OBP-001 v0.6 (two-class rule, constructor-as-trust-boundary, bitemporal law); EOP-VS-KYCUBO-001 v0.6 (determination semantics); the Java 25 DOP CBU reference implementation (June 2026) |
| **Status** | Draft. Deliverable is a standalone Java 25 data API — the DSL, compiler and SemOS governance layer are explicitly NOT in this artifact. |

---

## §1 Why this exists

Deal, CBU and UBO have been modelled as entities with stored structure. They are not entities. Each is a **container**: an anchor identity, a set of typed edges asserted between real entities, and a lifecycle of its own. Their shape changes, their membership changes, and the answer to "what did this look like?" depends on *when you ask*, *what you knew then*, and *who is asking*.

Storing that structure relationally has three costs, all of which have been paid already elsewhere in the platform: traversal is expensive and awkward; point-in-time reconstruction becomes a versioning problem for the structure as well as the facts; and viewer-dependent shape becomes post-hoc redaction rather than a property of what was built.

This service takes the opposite position. The database holds **entities and their metadata, plus the assertions that connect them** — nothing else. Every container is built in code, on demand, for a specified moment and a specified viewer. The taxonomy is a *function*, not a table.

## §2 The classification test

A subject belongs in this service iff all four hold:
1. Its identity is an **anchor** — the id carries no structure of its own.
2. Its structure is **asserted**, as typed edges between entities that exist independently.
3. Its **state is isolated** — it changes independently of the entities it connects; destroying it destroys nothing real.
4. Its **shape is a question, not a fact** — it varies by valid time, knowledge time, and viewer.

CBU and UBO pass on all four. **Deal is asserted to pass and must be tested on its own evidence** (§7 Q1) — inclusion by family resemblance is not inclusion.

## §3 Invariants

**CTN-1 — Nothing structural is persisted.** The store holds entities, their metadata, and edge assertions. No container graph, taxonomy shape, or adjacency is ever written as structure.

**CTN-2 — A container is anchor ⊕ asserted edges ⊕ its own state.** Three separable parts, folded from separate assertion streams, combined only at construction.

**CTN-2a — Edge kind determines traversal semantics; it is vocabulary, not an attribute.** Holdings divide into three kinds that behave differently and must never be flattened into one "holding" with a type field:
- **Voting/ownership** — carries both control and economic weight. Traversal continues through it; percentages multiply along the chain.
- **Investor issuance** — economic participation only, conferring no control. It must NOT multiply into a control determination, and traversal does not chase unitholders for control.
- **Pooled asset** — containment (umbrella → sub-fund), not a holding. Its role is scoping: it marks where one determination's boundary ends and the next begins. With segregated liability between sub-funds, determination is **per sub-fund, not per umbrella**.

**CTN-2b — Control edges carry a basis.** Control without ownership is the normal case, not an exception: a Management Company directs trading entities under a management mandate while economic interest runs from investors into the funds and never through the ManCo. The basis (mandate, appointment, delegation) is part of the edge, and a determination strategy may dispatch on it.

**CTN-2c — Membership is per-entity, role-typed, and withdrawn rather than deleted.** Members join a container individually, each edge carrying its role (mandate holder, investment manager, fund, trading entity, …). Removal is a **withdrawal assertion**, never a delete: the container as at any past moment has exactly the membership valid then, and dropping a member today never rewrites what was true in March. Multiple members of the same role are normal, not exceptional — co-management and multiple investment managers are ordinary structures, and any model that assumes one-of-a-role is wrong.

**CTN-2d — Obligations raised by membership are prospective.** Admitting a member may raise obligations against it (a mandate holder entering a CBU raises KYC clearance). Withdrawal ends those obligations going forward; it does not retract the historical determination or its pins, which stand as the record of what was known and decided at the time.

**CTN-2e — Assertion and attestation are separate axes: record freely, conclude carefully.** Every edge carries an epistemic status — **alleged** (the client told us), corroborated, **verified** (evidenced) — independent of whether it exists. Existence is never gated on proof. A Management Company or Investment Manager can be admitted to a CBU with no evidence at all and with the UBO container empty or barely populated: at that point the membership is an *allegation*, correctly recorded as one, with its source provenance (who told us, when, in what document or call). This is the normal opening state of every onboarding, not a degraded one. Verification gates **conclusions**, never **records**.

**CTN-2f — Status is computed, never stored.** The store holds the evidence, its kind and its date. Status is derived at construction time against the review horizon in force for that entity's risk rating **at the moment being asked about**. Periodic review then falls out with no batch job and no window in which a stored flag and reality disagree, and "was this verified as at March?" correctly uses March's horizon and March's risk rating. A stored `verified` boolean that silently ages is the declared-but-not-enforced defect class in another costume.

**CTN-2g — Three distinct movements away from verified; never conflate them.**
- **Demotion (staleness)** — the evidence aged out. The assertion is probably still true; we can no longer attest it. Prospective, nothing about the past changes. This is what triggers periodic review.
- **Refutation** — the assertion is not true. Not a status change: a **withdrawal** of the edge (CTN-2c) with a recorded reason.
- **Correction vs cessation** — hiding inside refutation, and the review must record which it found: did the relationship *end* (edge valid until June; the March determination was right), or was it *never* true (the whole valid range is corrected; the March determination was wrong though reasonable at the time)? Same discovery, different answers to "what was true in March" versus "what did we believe in March". Nothing downstream can infer this; it must be stated.

**CTN-2h — Staleness lapses currency; it never retracts a record.** A frozen determination stands permanently as the record of what was decided and on what basis. Demotion makes it **stale, not wrong**: currency lapses, a review obligation is raised, clearance moves, container state follows — all prospectively. Refutation is the harsher path: the graph reconstructed today has a different shape, the determination materially changes, and membership itself may be withdrawn.

**CTN-3 — Container state is isolated from its member entities.** It changes independently of them. Membership change does not invalidate container state. Destruction is a container-state transition that removes no entity.

**CTN-3a — Cross-container dependencies bind state transitions, never membership.** A container's *state* may depend on an answer computed in another container — CBU validation depends on KYC clearance, which depends on a determination in the UBO container. That dependency binds the **transition** only. Admitting or withdrawing a member is always available regardless of what is proven or populated elsewhere. Concretely: membership needs no evidence; a **provisional determination** may be computed at any time over alleged edges and is labelled by the weakest status it traversed; **freezing** a determination requires the edges it traversed to be verified; clearance requires a frozen determination; CBU validation requires clearance. Each stage gates the next and none blocks the one before it. The depending state records the determination it relied on together with that determination's pins and statuses (CTN-6), so "validated against what, and how well was it known" is answerable without recomputation.

**CTN-4 — Containers are constructed, deterministically.** `(anchor, validAt, knownAt, scope) → Taxonomy` is a pure function of the assertions in scope. Same inputs, same output, bit-identically, forever.

**CTN-5 — Scope is a construction parameter, derived from the caller's identity.** Never a request field, never a caller-supplied flag. Edges outside scope are not built — not built-then-hidden.

**CTN-6 — Every computed answer carries what produced it.** Any determination, aggregate or lifecycle decision records the valid time, knowledge time, scope, and ruleset version it was computed under. An answer without its pins is not reusable.

**CTN-7 — Construction caches are never authoritative.** Any cached construction is a regenerable, content-hashed checkpoint whose provenance identifies the assertions it was built from. Truth is always the assertions plus the constructor.

**CTN-7a — Two different things get called "snapshot"; they take opposite rules.**
- **State version rows** — the container's lifecycle state at each change. These **are** truth: append-only, never updated, read directly at T. CTN-7 does not apply to them.
- **Construction caches** — a taxonomy built from assertions and kept for speed. These are **never** truth, per CTN-7: regenerable, discardable, content-hashed.
Naming them apart matters more than it looks: one word for both eventually means someone rebuilds a state row or trusts a stale cache. The asymmetry is deliberate and worth stating plainly — **container state is read at T; container shape is constructed at T.** Two mechanisms, both bitemporal, both append-only, each suited to what it holds.

**CTN-7b — No updates, ever; and it is a permission, not a discipline.** Every state change writes a new version row. There are no `UPDATE`s and no `DELETE`s — destruction is a row (CTN-9). This is enforced at the **grant level**: application roles hold `INSERT` only on these tables, so history-rewriting is refused by the database rather than avoided by convention. A rule that can be enforced by a permission should never be left as a habit.

**CTN-7c — A version row records an intent, not a diff.** A row carrying only *what the state became* is a change log. An audit trail carries **the verb, the actor, the arguments, and the causation link** — why it changed, on whose authority, in response to what. The row is the fingerprint of an intent.

**CTN-7d — Version rows carry asserted state only.** Anything derived — verification status (CTN-2f), provisional determinations, computed rollups — stays computed at read time and never lands in a version row. An immutable row containing a stored `verified` flag is permanently wrong the moment its horizon passes, with an audit trail proving the error was deliberate.

**CTN-8 — The API is the only interface.** Raw tables are not a supported read path. Access control lives on the tables as well as in the code: what the API cannot show, no connection may select.

**CTN-9 — Destruction is a state, never a delete.** A destroyed container remains fully reconstructible as at any moment before its destruction.

**CTN-10 — One chokepoint, structurally enforced.** Every state change passes through a single enforcement entry point. Persistence is not reachable from outside it — enforced by module/package boundaries at compile time, not by convention or review.

**CTN-11 — Exhaustiveness is the primary safety property.** Domain alternatives are sealed hierarchies switched without `default`. A new edge kind, role, or lifecycle event that some handler forgets is a compile error, not a runtime surprise.

**CTN-12 — Wire types never enter the domain.** Serialization formats (JSON now, protobuf later) are boundary DTOs converted at one adapter layer into sealed domain records. Unknown and unset values are explicit errors there, never silent defaults.

**CTN-13 — One vocabulary, generated.** Node kinds, edge/role kinds and lifecycle states have a single declaration source; Java types and wire schema are generated from it. Two hand-maintained vocabularies of the same thing is the drift machine, and no discipline survives it.

**CTN-14 — Correctness is demonstrated, not asserted.** Where a Rust implementation of the same domain exists, differential conformance against it — same inputs, both implementations, compared constructed graphs and determinations — is the correctness mechanism, run in CI.

## §4 Positions

**P1 — Relational stores are the wrong home for traversable structure, and the right home for facts.** This is not a criticism of the database; it is a division of labour. Assertions are rows and belong in rows. Graphs are computed and belong in memory.

**P2 — Three containers, one machinery, three vocabularies.** Deal, CBU and UBO share the constructor contract, the fold discipline and the scope model. They do NOT share types that name domain concepts. A shared component that knows what a *role* or an *ownership edge* is has over-reached.

**P3 — Data-oriented Java, no framework.** Records, sealed interfaces, pattern matching. No Spring, no ORM, no runtime reflection in the domain path. Logic lives outside the data; data is immutable once published; construction accumulates mutably in a local and freezes on publish.

**P4 — Rules as code, not configuration — for this artifact.** The platform's YAML-declared geometry answers a different audience. Here the maintainers are Java developers, and sealed-type rules give them exhaustiveness, tests, refactoring and version control. Geometry that requires the constructed graph lives in Java; only row-local invariants that must hold regardless of caller live in stored procedures.

**P5 — Transport is a boundary detail.** REST for the initial implementation; gRPC when the consumer set justifies it. The adapter layer (CTN-12) exists precisely so this is a swap at the edge and not a change to the domain.

**P6 — The API is the product.** Not a convenience layer over tables: the assertions are genuinely unreadable without the constructor (bitemporal supersession makes naive queries silently wrong). This is the argument for the boundary, and it is a technical one, not a governance preference.

## §5 In scope

- The Deal, CBU and UBO **data model**: entities, metadata, typed edge assertions, container lifecycle assertions — with bitemporal validity and inline provenance on every row.
- The **taxonomy constructor** for each container: fold assertions → build adjacency → publish immutable taxonomy at `(validAt, knownAt, scope)`.
- **Render**: stable serialization of a constructed taxonomy — nodes, edges, container state, and the pins that produced it (CTN-6).
- **Lifecycle state changes** through the single chokepoint, persisted via stored procedures.
- **Read surface**: point-in-time reads, paged/bulk reads, and scoped search — designed deliberately, because whatever the API does not expose will drive consumers around it (§7 Q3).
- **Scope enforcement** from the authenticated principal, including the redaction cases that motivate it (officers, actual shareholdings).
- **Differential conformance harness** against the Rust implementation.

## §6 Out of scope (fenced)

The DSL, its compiler, and the SemOS governance layer — **explicitly not part of this deliverable**. The Rust platform's placement-set/evaluator machinery. Any UI. Analytics, BI feeds and warehouse replication (these are read-surface requests and must come through §5's read surface if they come at all). Migration of existing stored structures (its own exercise once the model is ratified).

## §7 Open questions for ratification

**Q1 — Does Deal actually pass §2?** Test it on its own evidence, not by association with CBU. If Deal is attribute-heavy with a lifecycle and few traversed relationships, it is an entity and belongs elsewhere; forcing it into this service would be symmetry for its own sake.

**Q2 — Asserted or derived container state, per container?** CBU's `VALIDATED` is asserted (with cross-checks); UBO's determination is derived by traversal; UBO also carries asserted subject state. Each container must declare which of its state is asserted (append an event) and which is derived (re-evaluate a function). Getting this wrong stores what should be computed, or recomputes what someone actually decided.

**Q3 — What is the read surface, concretely?** Bulk export, aggregate/reporting queries and cross-container search are exactly what an API-only boundary makes awkward, and exactly what produces a sanctioned bypass six months in. Name them now, scope-applied, or they will be improvised later by someone with a deadline.

**Q4 — Construction cost at real cardinality.** Construct-on-demand is free at tens or hundreds of edges. Measure the largest real container before deciding whether CTN-7's cached checkpoint is needed at all. Do not build the cache first.

**Q5 — Where does the vocabulary source live (CTN-13)?** The platform's YAML is the obvious candidate, but it lives on the other side of the §6 fence. If the Java artifact must stand alone, its vocabulary source must too — and then conformance (CTN-14) is what keeps the two aligned.

**Q6 — Write governance.** With the SemOS layer fenced out, the Java service can write state the platform's evaluator would have refused. Ratify the intended position: bounded fail-closed invariants in procedures as the last line, rich geometry in Java, and an explicit statement of what this service does *not* enforce — so the boundary is a decision, not a discovery.

## §8 What "done" looks like

A consumer with valid credentials can ask for a Deal, CBU or UBO taxonomy as at any moment, in any axis, and receive exactly the graph their scope permits — together with the pins that make the answer reproducible. They can change container lifecycle state through the same API and no other path. They cannot reach the tables. The same question asked twice returns bit-identical answers, and the same question asked of the Rust implementation returns the same answer as this one.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-19 | Initial draft. Establishes the container class (anchor ⊕ asserted edges ⊕ isolated state), the four-part classification test, and 14 invariants — nothing structural persisted; deterministic construction at (validAt, knownAt, scope); scope from caller identity, not request; answers carry their pins; snapshots never authoritative; destruction as state; single structurally-enforced chokepoint; exhaustiveness as the primary safety property; wire types excluded from the domain; one generated vocabulary; differential conformance as the correctness mechanism. Six positions incl. rules-as-code for this audience and transport-as-boundary-detail (REST first, gRPC later). Deliverable fenced to the data API: DSL, compiler and SemOS explicitly excluded. Six open questions, of which Q1 (does Deal qualify) and Q6 (write governance with the governance layer fenced out) are the two that change the shape of the artifact. |
| 0.2 | 2026-08-19 | Edge semantics and epistemics folded in from the ManCo/IM and allegation discussions. **CTN-2a** three holding kinds as vocabulary (voting/ownership traverses and multiplies; investor issuance is economic-only and never confers control; pooled asset is a scoping boundary, determination per sub-fund). **CTN-2b** control edges carry a basis — control without ownership is the normal case. **CTN-2c/2d** membership is per-entity, role-typed, withdrawn not deleted, multiple-of-a-role normal; membership obligations are prospective. **CTN-2e** record freely, conclude carefully — a ManCo or IM joins with no evidence and an empty UBO container; that membership is an allegation, correctly recorded as one. **CTN-2f** status is computed from evidence + horizon + risk rating at the moment asked about, never stored as a flag. **CTN-2g** three movements away from verified kept apart: demotion (staleness, triggers periodic review), refutation (withdrawal), and correction-vs-cessation (which the review MUST record — nothing downstream can infer it). **CTN-2h** staleness lapses currency, never retracts a record. **CTN-3a** rewritten: cross-container dependencies gate state transitions, never membership — membership needs nothing; provisional determinations compute over alleged edges labelled by the weakest link; freezing requires verified edges; clearance requires a frozen determination; validation requires clearance. **CTN-7a–7d** the snapshot split (state version rows ARE truth; construction caches never are), append-only enforced by grant not habit, version rows record an intent rather than a diff, and carry asserted state only. |
