# The UBO Construction Kit
### Vision & Scope — AI-assisted UBO taxonomy building from DSL source

| | |
|---|---|
| **Document** | EOP-VS-KYCUBO-KIT-001 |
| **Type** | Vision & Scope (material change of approach) |
| **Version** | 0.3 — Draft for review |
| **Owner** | Adam Cearns |
| **Status** | Draft. A new V&S for the KYC/UBO pack's interactive build model. Depends on — does **not** amend — EOP-VS-BPMN-DESIGN-003 (ratified), cited as the *contract* baseline. |
| **Contract baseline** | EOP-VS-BPMN-DESIGN-003 (Sage/Utterance-Engine/Repl split; ranker-only SLM contract; single-disposition-function rule; the move/board *discipline*). |
| **Domain baseline** | EOP-SA-OBP-001 (versioned-rows substrate, the constructor, scope-is-causal); EOP-VS-KYCUBO-001 (determination semantics). |
| **Grounding** | Verified against `bpmn-lite` post-`68723b9` (chain preview shipped), `semantic-decision-contracts` @ `1d039d9`, and `ob-poc-kyc-substrate` / `ob-poc-kyc-store` per the R1–R8 implementation-research dossier. Paths cited inline; will drift. |

> **The Vision.** The objective of a KYC/UBO session is to **construct the UBO taxonomy** — the way you build a Minecraft village or a Lego model. The taxonomy/graph is the *board being built*; the DSL verbs are the *moves and tools that build it*. The taxonomy is built **dynamically from DSL source** — which is precisely what gives point-in-time recovery and a complete **intent → action** audit trail: every element of the structure traces to the governed move that placed it. The **baseline session targets a super user** — someone who knows the KYC/UBO DSL vocabulary and uses the Sage session **as an IDE to code**, their utterances close to the language itself. From that baseline, phrase banks and the SLM are progressively enriched to take instructions from **less-knowledgeable users in plainer English** and help them "code up." Expert first; assistance ramps outward.

---

## §1 The material change of approach

The prior mental model treated the UBO as a structure to be maintained — edges written, statuses updated, a graph the database holds and the application mutates. This V&S replaces it with a construction model:

> **KYC/UBO is a construction kit. The versioned node/edge types are typed blocks. The DSL verbs are the placement functions (the moves/tools). The preconditions are the stud geometry that makes only well-formed structures buildable. The DSL source — the sequence of placements — is the truth. The taxonomy graph is a construction over that source, built in code, per look. The AI is a build-assistant that proposes legal placements it cannot mis-fit.**

This imports the game-board machinery proven in the BPMN Designer PoC and reframes it from *navigation* (traverse a pre-authored board) to *construction* (build a structure from typed blocks). The reframe resolves the central finding of the BPMN→KYC solution review — that KYC has **no persisted graph to walk** — by making it the point: there is nothing to walk because there is nothing stored to walk; the structure is *built*, not traversed.

**Why the reframe fits KYC better than the domain it came from.** The BPMN board is a persisted, acyclic, structurally-validated `DesignerDag` walked by `petgraph::has_path_connecting` (`designer-graph/src/positional.rs:96-103`). KYC has an append-only stream of governed moves and a **pure fold** that reconstructs state on demand (`fold_control_versioned`, `ob-poc-kyc-substrate/src/fold/registry.rs:122` — confirmed pure: no I/O, no clock, no randomness, BTreeMap dispatch). That purity is exactly what a construction kit needs and what BPMN lacked (§4, §7).

## §2 The three-layer architecture

| Layer | What it is | Authority |
|---|---|---|
| **Source** | The UBO's cumulative **DSL S-expression** history — governed placements, entity UUIDs resolved at capture — accumulated across all its sessions | **Authoritative.** A UBO *is* its S-expression history. |
| **Kit** | The **SemOS pack**: block-types (node/edge kinds), stud geometry (preconditions), placement verbs. *What can legally be built.* | Versioned / content-addressed. The rules of construction. |
| **Build** | The **materialised state** — versioned rows, persisted snapshot, constructed taxonomy graph | **Derived.** A checkpoint/cache, regenerable from Source through Kit. Never authoritative over Source. |

Between Source and Build sits the **constructor** (§5): a deterministic, parameterised function reading the S-expression source, resolving precedence, assembling the taxonomy graph for a point in time and viewer — the same nom→AST→graph move a compiler makes.

**KIT-1.** The DSL S-expression history (UUID-resolved) is the sole source of truth for a UBO. Rows, snapshot and graph are derived and regenerable from it. *(Implementation status: PARTIAL — structured events are persisted; the resolved S-expression text is not yet captured. Closing this is Tranche 1 of the plan.)*

**KIT-2.** The database persists **no** graph or taxonomy structure. Structure is constructed in code, per look, from the source.

## §2a The user model — expert first, assistance ramps outward

The session has a **persona ladder**, and the ladder is an architectural constraint, not a UX note:

1. **Baseline: the super user.** DSL-literate in the KYC/UBO vocabulary; uses the Sage session **as an IDE** — staging moves, previewing lines of play, committing validated workbooks. Their utterances are close to the language, so **deterministic tier-0 matching suffices**: exact/near-exact verb and argument recognition, no inference required.
2. **The ramp: the assisted user.** Phrase banks and the SLM are enriched — from real expert usage — to map **plainer-English intent** onto the same governed moves, helping a less-knowledgeable user "code up." The assisted user's moves pass through the *identical* placement-set, preview, validation and commit machinery; the ramp changes how intent is *recognised*, never what is *legal*.

**KIT-11 (zero-inference baseline).** The session is complete and fully functional for the super user with **no model in the path**: deterministic matching, generated placement-set, preview, Repl validation, commit. The SLM/plain-English layer is an accessibility ramp, never a load-bearing component. *(Consequence: the game loop ships and is useful before any SLM exists; the model arrives later behind a promotion gate, per the DESIGN-003 discipline — "zero direct model-authorised executions by construction" is inherited and strengthened: at baseline, zero model involvement at all.)*

**KIT-12 (the corpus factory).** Expert sessions generate the real utterance→move pairs that train and calibrate the plain-English ramp. Capture and training use of that corpus is **gated by a KYC capture charter** (the Q9-charter equivalent for this pack — its own artifact, per the DESIGN-003 §18 fence). The enrichment layer is bootstrapped from governed expert use, not from synthetic guesswork.

## §3 The kit — blocks, stud geometry, placement

**Blocks.** The versioned node/edge types: control edge, economic-interest edge, evidence attachment, obligation, role binding, determination snapshot. Each has a shape (what it may connect to) and a **stud pattern** (what may legally snap onto it).

**Placement functions.** The DSL verbs — `ubo.edge.assert-control`, `.assert-economic-interest`, `.attach-evidence`, `.verify`, `.supersede`, `ubo.determination.reconcile-conflict`, `.select-strategy`, `.compute-fold`, `.apply-smo-fallback`, `.freeze`, and the wider registered set *(exact denominator under reconciliation — plan T0.3: this pass counts 12 registered verbs, the earlier review counted 19; two denominators suspected)*.

**Stud geometry.** A verb is legal only where its block's studs mate with the current build: `check_control_preconditions(lexicon_entry, state, event)` (`fold/control.rs:398-438`), run under the per-subject lock after re-folding, before insert (`store.rs:176-181`). A `verify` block snaps only onto an evidenced edge (`EvidenceCited`); `compute-fold`/`freeze` snap only onto a reconciled, strategy-selected build (`ReconciledProjection` + `StrategySelected`). Preconditions are **statically enumerable** ahead of execution via `LexiconEntry.preconditions` (dossier R3c) — the fact that makes the placement-set generable (§4).

**KIT-9 (core authoring work).** Every block-type must have defined stud geometry — a precondition, even if trivially true. An under-specified block mates to anything and is a **defect**. Today 3 of the registered verbs carry non-trivial preconditions; **authoring the remaining stud geometry is a central deliverable** (plan T6), incrementally — a trivially-true precondition is a legal (permissive) block, so the game runs from day one and tightens as geometry is authored.

## §4 The board is a generated placement-set

KYC has a *gate* ("is this one verb legal?"), not a stored *board*. **You do not invert the gate; you apply it across the block set.**

> **The placement-set is generated by evaluating each block-type's stud geometry against the constructed current build and collecting the ones that fit:** `board = verbs.filter(|v| v.precondition.holds(constructed_state))`. It is a **projection of the constructor's output through the precondition predicates** — exactly parallel to the determination being a *traversal* of the same output (`DeterminationStrategy::resolve`, `determination.rs:70-79`). Same build, two readings.

**KIT-3.** A move is legal iff its stud geometry holds against the reconstructed current build. The placement-set is generated by filtering block-types through preconditions against the reconstruction — never authored, never persisted, never gate-inverted.

The reusable asset from BPMN is the **pipeline discipline** (universe → context filter → policy filter → canonical order → content hash → `NONE_OF_THE_ABOVE`), not its concrete `LegalityOracle` — KYC's is precondition-backed, not petgraph-backed (§10).

## §5 The constructor — re-run-whole, and per-viewer flexibility

**KIT-4.** Reconstruction is total (re-run-whole over the move sequence), never incremental. No structure is maintained; nothing can go stale; a superseding move yields a *different correct build*, never a propagated break.

Re-run-whole **eliminates the cascade-invalidation bug class**: patch-incrementally and a superseding move forces break-propagation (walk, find dependents, unwind); re-run-whole and the constructor simply produces the one correct build for the moves as they now stand. Sessions are tens of moves — trivial for a nom-class constructor — so re-run-whole is both faster *and* the version with no correctness hazard. Compiler instinct: re-parse; don't surgically patch the AST.

**KIT-5.** The constructor is parameterised: `S-expr source × (time T, axis, viewer, kit-version) → taxonomy graph`. Many graphs from one source — regulator (full evidential), relationship manager (collapsed summary), auditor (knowledge-axis "what we believed"), determination engine (reconciled verified edges only), another jurisdiction (same source under different control/threshold rules). This flexibility is the *direct consequence of not persisting structure* and is unreachable in any schema-baked model.

**KIT-6.** For a regulated determination, exactly one graph is authoritative: the **frozen** view — pinned viewer, axis, kit-version, constructor-version at the frozen `as_of`, content-addressed. Flexibility is for reading; the frozen determination nails one interpretation, reproducibly. *(Time-axis status: recovery today filters transaction time only — `recover_determination_at` on `committed_at`; the valid-time axis parameter is a plan tranche, T5.)*

## §6 The session model

A **session** produces a **workbook** — a staged sequence of proposed placements — validated ready-to-execute by the Repl, then committed to the UBO's source.

- **Start.** Overlay the **kit** (SemOS pack) and the **persisted last snapshot** (the build-so-far). *New UBO* = empty baseplate. *Existing UBO* = load the saved build; this session's moves extend it.
- **Stage.** New DSL moves proposed into the workbook — typed directly by the super user, or (later ramp) recognised from plainer English. Each is checked for stud-fit against the reconstruction-so-far.
- **Validate (the Repl's job).** The Repl continuously **re-runs the whole workbook DSL** to confirm the staged build is a legal, executable construction — before commit. The Repl validates *ready-to-execute*; it does not mutate the record as the analyst types.
- **Commit.** A validated workbook **appends its DSL sequence to the UBO's source history** and refreshes the snapshot — each move re-validated at placement against the real state through the governed append path (`FOR UPDATE` lock → re-fold → precondition gate → insert at `next_seq`; dossier R4).

**KIT-10.** The kit is versioned/content-addressed. A session overlays kit ⊕ last snapshot. The snapshot is a regenerable checkpoint, never authoritative over source.

## §7 Lookahead — hold the block before you glue it

**KIT-7.** Lookahead is read-only speculative reconstruction: candidate moves appended to the sequence **in memory** and re-run through the constructor, never touching the source. **Committing a lookahead line replays each move through the real governed append, re-validated at placement.** The preview was only ever a preview.

The mechanism is now *proven next door and simpler here*: bpmn-lite shipped `resolve_hypothetical_chain` (`utterance-engine/src/bpmn_board.rs:464`, commit `68723b9`) — staged-clone chain with per-step admission. KYC mirrors those **semantics** without the DAG clone, because the fold is already callable store-free on `committed ++ candidates` (dossier R4). The per-step discipline carries over exactly: each candidate validates against the state folded from committed + prior candidates.

## §8 The AI — a mis-fit-proof build assistant, arriving on the ramp

**KIT-8.** The AI proposes **placements only**. It cannot introduce block-types, alter stud geometry, mis-fit a block (preconditions make illegal placements unbuildable regardless of proposal), or bind open free-text arguments — those are ordinary governed validation, outside the kit.

Sequenced by the persona ladder (§2a):

- **Baseline (no AI):** tier-0 deterministic matching for the DSL-literate super user. The full contract inheritance from DESIGN-003 — ranker-only `SlmResult`/`FiniteScore`, single terminal `ProposalDisposition::decide`, no model ever issues a disposition — applies *when the ramp arrives*; at baseline there is nothing to constrain because nothing infers.
- **The ramp:** phrase banks enriched from expert-session corpus (KIT-12, charter-gated), then the SLM as ranker over the generated placement-set for plainer-English intent. **Verb selection** ports the ranker-only contract (move space closed on verb identity). **Argument binding does not become a game move**: open-payload fields (`role`, `jurisdiction`, `strategy`-as-bare-string — `fold/control.rs:361-367`, evidence free-text) are the block's *paintable surface*, bound after placement by governed validation. Pretending free text is a board slot would corrupt the closed-move property that makes ranking sound.

## §9 Source of truth — S-expressions, not rows

> **The stored, authoritative artifact is the DSL S-expression history with entity UUIDs resolved at capture. The versioned rows are the materialisation; the taxonomy graph is the construction. If source and materialisation ever diverged, the source is authoritative — rows regenerate from S-expressions, not vice versa.**

For a regulated determination this is the stronger audit position: the source records the **reasoning** — the exact sequence of governed intents, replayable to the identical result — not merely the outcome. A UBO is a stored, replayable, portable **program**. *(The complete intent → action/move audit trail of the Vision is this property: every element of the built taxonomy traces to the governed move that placed it, and the whole build replays from source.)*

## §10 Dependencies and reuse (corrected against the tree)

- **The contract discipline is the reusable asset; the contracts crate is not (as-is).** Dossier R2 verdict on `semantic-decision-contracts` @ `1d039d9`: *nominally domain-agnostic, operationally graph-specific* — `GraphRevision`, `GraphDeltaPreview`, `GraphElementRef`-as-anchor presume a mutable versioned graph an append-only stream doesn't have. **Position #7 amended (v0.2): KYC builds native board/move types that mirror the contract discipline** — content-addressed move identity, canonical ordering, `NONE_OF_THE_ABOVE`, board content-hash, single-disposition-function — **adopting the crate only if/when its graph-specific types are split from the generic core** (a stated later convergence, not now). *(Pending formal T0.1 ratification in the plan.)*
- **Position #7 superseded (v0.3, 2026-08-17):** see §10a. The R2 graph-coupling concern is real but narrower than the v0.2 wording implied — it applies to `DesignPosition`/`LegalMove`/`MoveAttempt`/`GraphRevision`/`GraphContentHash`/`GraphStateHash` (all genuinely graph-shaped), not to the bare closed-set vocabulary (`ABSTENTION_CANDIDATE_ID`, `GameDispositionKind`, `MoveAttemptOutcome`), which carries zero graph coupling. `ob-poc-kyc-substrate` now depends directly on `semantic-decision-contracts` for exactly that vocabulary subset; KYC's own board/move types (`PlacementSet`/`LegalMove`(KYC)/`MoveId`/`board_hash`) remain KYC-native and un-adopted, per the original reasoning.
- **Reusable with change:** the pipeline shape, the `LegalityOracle`/`BoardUniverseProvider` trait *interfaces*, the ranker-only SLM contract, the single-disposition-function rule, clone-before-ratify staging, and now the **shipped chain-preview semantics** (per-step admission) — minus the DAG clone.
- **BPMN-specific, not applicable:** `DesignerDag`, `ops::apply`/`admit`, V-1..V-11 (`bpmn-lite-types::v2_verifier`). Do not lift `utterance-engine` wholesale.
- **Do not conflate:** `ob-poc-sage::ValidVerbSet`/`ValidVerbSetEngine` — DTO + trait, **zero implementations** (dossier R6); a separate partial artifact, addressed (implement or retire) in plan T4.

## §10a Position #7 superseded (v0.3, 2026-08-17)

Landed in `ob-poc-kyc-substrate` this session, against `semantic-decision-contracts` @ `1d039d9` (the same commit R2 reviewed):

- `Cargo.toml`: direct dependency, a named exception to the crate's existing "no sem_os_core (git dep)" boundary comment.
- `placement.rs`: `NONE_OF_THE_ABOVE` now re-exports `semantic_decision_contracts::ABSTENTION_CANDIDATE_ID` (byte-identical value — a pure rename, zero behavioral change).
- `domain_ops/kyc_ramp_capture.rs` + `repl/kyc_workbook_surface.rs`: the capture-correlation `UserAction` enum (`Accepted`/`Edited`/`Rejected`) replaced by `MoveAttemptOutcome` (10 variants) — `Applied`/`Corrected`/`CompilerRefused` mapped at the `Stage` call sites, giving the capture path real distinctions it didn't have as a 3-value hand-roll (board-rejected-as-illegal vs operator-declined-a-legal-proposal). Migration widened the `kyc_ramp_capture.user_action` CHECK constraint to the full variant set.

**Why this narrows, rather than contradicts, R2.** `ABSTENTION_CANDIDATE_ID` is a bare `&str` const; `GameDispositionKind`/`MoveAttemptOutcome` are closed `serde`-only enums with no graph-shaped field on any variant. None of the three reference `GraphRevision`, `GraphElementRef`, or any board-position concept — the graph-specific coupling R2 flagged lives entirely in `DesignPosition`/`LegalMove`/`MoveAttempt`, which KYC still does **not** depend on and still mirrors natively (`PlacementSet`/`LegalMove`(KYC)/`MoveId`/`board_hash` unchanged — event-stream-shaped, no adoption). This is confirmed by direct source read of `semantic-decision-contracts/src/gameboard.rs` before landing, not by re-litigating R2's verdict on the graph types, which stands.

Position #7's underlying warning — don't let type-satisfaction convenience smuggle a graph-shaped model into an event-stream substrate — is upheld, not overturned; it's now scoped correctly to the types that actually carry that risk.

## §11 Invariants (consolidated)

- **KIT-1** — S-expression history is the sole source of truth; all else derived and regenerable. *(PARTIAL in tree; plan T1.)*
- **KIT-2** — the DB persists no graph/taxonomy; structure is constructed in code per look.
- **KIT-3** — a move is legal iff its stud geometry holds against the reconstruction; the placement-set is generated, never authored/persisted/inverted.
- **KIT-4** — reconstruction is re-run-whole, never incremental.
- **KIT-5** — the constructor is parameterised; many graphs from one source.
- **KIT-6** — exactly one authoritative graph per regulated determination: the frozen, pinned, content-addressed view.
- **KIT-7** — lookahead is read-only speculative reconstruction; commit replays through the real governed append.
- **KIT-8** — the AI proposes placements only; never block-types, stud geometry, or open-arg binding.
- **KIT-9** — every block-type has defined stud geometry; an under-specified block is a defect.
- **KIT-10** — the kit is versioned; session = kit ⊕ snapshot; snapshot is a regenerable checkpoint.
- **KIT-11** — the session is complete for the super user with zero inference in the path; the SLM is a ramp, never load-bearing.
- **KIT-12** — expert sessions are the charter-gated corpus factory for the plain-English ramp.

## §12 Positions taken (with reasons)

| # | Position | Reason | Trade-off / boundary |
|---|---|---|---|
| 1 | Re-run-whole reconstruction, not incremental | Deletes the cascade-invalidation bug class; sessions tiny for a nom-class constructor | Recompute per position — trivial at session scale |
| 2 | Authoring the missing preconditions is in scope — it *is* defining the kit | An under-specified block permits impossible structures | Mechanism + discipline ruled here; per-verb authoring incremental (T6), game live from day one on the permissive subset |
| 3 | Lookahead is read-only preview; commit replays each move through the real append | Speculative fold must never touch the regulated record | No direct speculative-commit path exists, by construction |
| 4 | S-expression source is authoritative; rows are the cache | Source records reasoning — the stronger audit posture; rows regenerate from source | Snapshot must be reproducible under a pinned constructor version |
| 5 | The DB holds no graph; taxonomy built in code, per viewer | Per-viewer flexibility (KIT-5) unreachable with schema-baked graphs | Concentrates trust in the pinned, attestable constructor |
| 6 | SLM selects moves; open free-text args bound outside the game | Preserves the closed-move property that makes ranking sound | Arg-validation stays ordinary governed validation, not a new tier |
| 7 *(superseded v0.3 — see §10a)* | Adopt the bare closed-set vocabulary (`ABSTENTION_CANDIDATE_ID`, `GameDispositionKind`, `MoveAttemptOutcome`) directly as a dependency; continue to mirror-not-adopt the graph-shaped types (`DesignPosition`/`LegalMove`/`GraphRevision`) | Direct source inspection found the R2 graph-coupling concern applies only to the graph-shaped subset, not the vocabulary subset | KYC board/move types remain native; only the outcome/disposition vocabulary is shared |
| 8 | Regulated determination pins one frozen view; flexibility is for reading | "Which graph is real?" must answer crisply for a regulator | Frozen view carries viewer/axis/kit-version/constructor-version pins |
| 9 *(new v0.2)* | Super-user-first: the session ships as a zero-inference DSL IDE; the plain-English ramp follows, bootstrapped from expert corpus | De-risks the whole programme (no model in the critical path at launch); generates the *real* corpus the ramp needs; inherits and strengthens the DESIGN-003 safety posture | Plain-English users wait for the ramp; corpus capture is charter-gated (KIT-12) before any training use |

## §13 Scope

**In scope.** The construction-kit model; the generated placement-set (KIT-3); re-run-whole reconstruction (KIT-4); the parameterised constructor and per-viewer flexibility (KIT-5) with the frozen-view boundary (KIT-6); the session/workbook/Repl-validate/commit loop (§6); read-only lookahead with replay-commit (§7); the **super-user IDE baseline and the persona ladder** (§2a, KIT-11/12); the SLM-as-build-assistant contract with the verb-select/arg-bind split (§8); the stud-geometry authoring discipline; the dependency boundary (§10).

**Out of scope (own home).** Determination *semantics* (EOP-VS-KYCUBO-001). Persistence/constructor law (EOP-SA-OBP-001). The BPMN Designer model (EOP-VS-BPMN-DESIGN-003 — depended on, not amended). **The KYC corpus, threat model, capture charter and SLM promotion gate — its own artifact** (the KYC Q9-charter equivalent), per the DESIGN-003 §18 fence; KIT-12 establishes the frame, not the authorisation.

**The fence.** This is the KYC pack's construction-kit V&S. It reuses BPMN contracts; it does not extend BPMN scope. Live user-capture and training use of any KYC corpus is gated separately, not authorised here.

## §14 Open questions

1. **Speculative-preview API shape** — thin `fold(committed ++ candidates)` wrapper (steer) vs a first-class speculative-store concept. *(Narrowed by the dossier: the pure-fold primitive is confirmed callable store-free; T3 assumes the wrapper.)*
2. **Placement-set generation cost** — measure the per-position enumeration (steer: trivial at 12–19 verbs; close with a benchmark in T2).
3. **Determination-lookahead scoring** — is workbook lookahead about *ranking candidate moves* (utterance fit) or *evaluating consequences* (does this placement resolve the determination)? Different "aheads"; T4 design names which the workbook surfaces (possibly both, as distinct affordances).
4. **Bitemporal recovery** — the valid-time axis parameter (T5).
5. **Precondition authoring order** — incremental per verb-family (steer ratified into T6's shape).
6. *(new)* **Verb denominator** — 12 registered vs 19 surveyed; two-denominator hypothesis; resolved by plan T0.3 before T6 locks scope.

## §15 Relationship to the document set

- **EOP-SA-OBP-001** — the substrate law this V&S builds the interactive model over.
- **EOP-VS-KYCUBO-001** — the determination semantics; this V&S is *how a determination is built*, not *what it means*.
- **EOP-VS-BPMN-DESIGN-003** — the contract baseline; navigation → construction reframe for KYC.
- **EOP-DD-KYCUBO-001/002** — the substrate/append implementation; "commit appends to source, re-validated at placement" *is* that append protocol.
- **EOP-PLAN-KYCUBO-KIT-001** — the tranche-based implementation plan executing this V&S.

## Change Log

| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-07-03 | Initial V&S: construction-kit thesis; S-expression source of truth; generated placement-set; re-run-whole; per-viewer constructor; frozen-view pin; session loop; pure-fold lookahead; SLM verb-select/arg-bind split; ten invariants, eight positions. |
| 0.2 | — | **The Vision stated** (taxonomy = the board/village; DSL = the moves/tools; dynamic build from DSL ⇒ PITR + complete intent→action audit trail). **Persona ladder added** (§2a): super-user IDE baseline with zero inference in the path (KIT-11); plain-English ramp bootstrapped from charter-gated expert corpus (KIT-12); position #9. **Position #7 amended** per dossier R2 (mirror the contracts discipline in KYC-native types; adopt the crate on later split). Grounding refreshed post-`68723b9` (chain preview shipped; lookahead mirrors proven semantics); KIT-1 marked PARTIAL-in-tree; verb-denominator discrepancy logged (§14 Q6); precondition count corrected to 3-of-registered. |
| 0.3 | 2026-08-17 | **Position #7 superseded** (§10a): direct source read of `semantic-decision-contracts/src/gameboard.rs` @ `1d039d9` found the R2 graph-coupling concern applies only to `DesignPosition`/`LegalMove`/`GraphRevision`, not to the bare closed-set vocabulary (`ABSTENTION_CANDIDATE_ID`, `GameDispositionKind`, `MoveAttemptOutcome`). `ob-poc-kyc-substrate` now depends directly on `semantic-decision-contracts` for that vocabulary subset (dsl pin bumped to `1d039d9`); KYC's board/move types remain native and un-adopted. Landed: `NONE_OF_THE_ABOVE` re-exports `ABSTENTION_CANDIDATE_ID`; capture-correlation's `UserAction` replaced by `MoveAttemptOutcome`. |
