# EOP-PLAN-KYCUBO-KIT-T7 — The Plain-English Ramp

| | |
|---|---|
| **Document** | EOP-PLAN-KYCUBO-KIT-T7 |
| **Version** | 0.1 — draft **for ratification** (accept/amend/defer per section, same ceremony as the T6 matrix and TS.0) |
| **Binds to** | EOP-PLAN-KYCUBO-KIT-001 v0.6 §T7 + Appendix A R7; EOP-VS-BPMN-DESIGN-003 (ranker pattern); T0.1c mirror-not-adopt ruling (plan v0.5:19-20); T4.5 workbook surface as landed (`kyc_workbook.rs`, `kyc_workbook_surface.rs`, `kyc_entity_resolver.rs`); state-of-tree facts of 2026-08-14 (§0, R7 spike) |
| **Status** | RATIFIED 2026-08-14 — ACCEPT ALL (§1 invariants, §2 tranche shapes, §3 rulings Q1(a)/Q2(a)/Q3(yes)/Q4(draft-for-ratification)/Q5(a)). T7.0 (R7 spike) EXECUTED — §0 is its dossier. KYC capture charter (EOP-DD-KYCUBO-CAPTURE-CHARTER v0.1) drafted and RATIFIED 2026-08-14 — T7.1 and T7.2 are now unblocked per §5. **§7 amendment (2026-08-17): the "board" model — ratified vision clarification, see below. §8 amendment (2026-08-17): `ABSTENTION_CANDIDATE_ID`/`MoveAttemptOutcome` adopted from `semantic-decision-contracts`, `GameDispositionKind` deferred — see below.** |

**What T7 is.** The kit to date serves the expert who dictates DSL (T4.5: stage → membership test → preview → commit/run, zero inference). T7 is the ramp *below* that vocabulary floor: plain-English utterances proposing moves **into** the same governed pipeline. The ramp adds reach, never authority — every proposal still lands as DSL text in `stage()`, still passes the frontier placement-set membership test, still previews, still commits under studs + kit hash + §3 append. Nothing downstream waits on T7 (plan v0.6 dependency graph: independent tail).

---

## §0 R7 findings this plan stands on (spike executed 2026-08-14; all citations verified against tree)

1. **The full utterance→verb pipeline exists in ob-poc and already covers the 21 dsl.kyc verbs.** `HybridVerbSearcher::search` (`rust/src/mcp/verb_search.rs:503,670-682`, tier ladder at `:660-668`); intent pipeline (`rust/src/mcp/intent_pipeline.rs:355-361` — NL → verb search → LLM extracts *args only*, never writes DSL); all 21 verbs carry `invocation_phrases` (13 blocks in `dsl-kyc.yaml`, 8 in `dsl-kyc-obligation.yaml`); **all 21 are already embedded** — 231 patterns live in `"ob-poc".verb_pattern_embeddings` (verified against data_designer 2026-08-14). The dsl.kyc verbs are already discoverable through the general ob-poc chat pipeline *outside* the workbook.
2. **The kyc crates have zero dependency path to any of it** (substrate: pure, `ob-poc-kyc-substrate/Cargo.toml:8-18`; store: substrate+sqlx; seam: no ob-poc dep) — but **the workbook itself lives in the ob-poc app crate** (`rust/src/domain_ops/kyc_workbook.rs`, `rust/src/repl/kyc_workbook_surface.rs`), the same crate as `mcp::verb_search`. A `use crate::mcp::…` in the workbook module would compile today.
3. **The zero-inference gate has a blind spot for exactly that path.** The T4 allowlist test scans external-crate imports only and **explicitly skips `crate`/`super`/`self` roots** (`rust/tests/kyc_workbook.rs:409-411`). A crate-internal ramp wired into the workbook would NOT trip the gate. T7 must extend the gate deliberately or the zero-inference claim silently erodes (§3, ruling Q1).
4. **`stage()` accepts raw DSL text** (`kyc_workbook.rs:296-302`): parse → exactly-one-atom → `ParsedMove` → **frontier placement-set membership test** — reject with `NotCurrentlyLegal` listing every legal move, never a guess (`:314-333`). Pre-stage handle resolution is exact-match fail-closed (`kyc_entity_resolver.rs:1-27`). **This is the ramp's injection point: resolved-DSL-text in, staged-move-or-typed-refusal out.**
5. **DESIGN-003's ranker pattern maps 1:1 onto what T2 already built.** DESIGN-003 (`docs/eop/EOP-VS-BPMN-DESIGN-003.md`) rules: contextual ranking over a deterministically constructed content-addressed candidate board; tier-0 = "existing Candle phrase matcher (high-recall retrieval)" (`:591`); tier-1 = sealed cross-encoder returning only a scalar score; **deterministic policy — never the model — issues every disposition** (select/clarify/abstain-via-`NONE_OF_THE_ABOVE`/escalate); data governance is a phase entry gate. The KYC `PlacementSet` (`enumerate_placement_set`, board hash, canonical ordering, `NONE_OF_THE_ABOVE` — all T0.1c-ratified) IS that board.
6. **bpmn-lite has nothing to port.** The `bpmn-lite/` directory is absent from this machine (CLAUDE.md's monorepo claim is stale); the one cited artifact (`resolve_hypothetical_chain`) is chain-preview semantics KYC already mirrors in `preview()`. T0.1c rules mirror-the-discipline anyway. Every "port bpmn tier-0 code" subtask is DELETED.
7. **The KYC capture charter is ABSENT** — no doc matches (only `charter-reconciliation-v1.md`, an unrelated pub-surface budget). The corpus consistently marks it "its own artifact, hard-gates T7.1" (plan v0.6:103; Construction Kit v0.2:156). The gate is real and unbuilt.
8. **Capture has natural tap points already:** every committed move persists its resolved `source_text` via `append_in_scope` (`kyc_workbook.rs:388-392`); every refusal is a typed `RecognitionError` (`:68-84`). Utterance→move capture needs only a thin recorder around `stage()`/`dispatch()` — no engine changes.
9. **Sage crates are contracts + deterministic pre-classification only** (`ob-poc-sage/src/engine.rs:52-54`, `pre_classify.rs:1-9`); all matcher machinery stays in `rust/src/mcp`. Consistent with DESIGN-003's "Sage is enrichment-and-escalation, never a mandatory upstream stage".

---

## §1 The ruled shape (proposed)

**One sentence:** the ramp is a **proposal generator in front of `stage()`** — utterance → placement-set-restricted retrieval (tier-0) → \[T7.3: ranker (tier-1)\] → deterministic disposition → a rendered DSL line offered to the user → on acceptance, the *ordinary* T4.5 stage path runs unchanged.

Invariants (mirroring T0.1c + DESIGN-003, non-negotiable):

- **I-1 Proposal-only.** The ramp NEVER calls `stage()`/`commit()`/`run()` itself. Its sole output is a candidate DSL line (+ explanation) presented for the user's explicit acceptance; acceptance feeds the existing `Stage{text}` command verbatim.
- **I-2 Board-restricted.** Candidates come ONLY from the current frontier `PlacementSet` — the ramp cannot propose a move the membership test would reject. (Retrieval scores placement-set moves; it does not search the open 1,281-verb estate.)
- **I-3 Deterministic disposition.** Score thresholds decide select / clarify / abstain (`NONE_OF_THE_ABOVE` is a first-class board candidate); the model never issues a disposition, only scalars. Multi-peak → clarify, never auto-pick.
- **I-4 Args stay resolver-owned.** Entity references resolve through the existing exact-match `kyc_entity_resolver` (fail-closed, zero fuzz). The ramp may *slot* a resolved handle into a proposal template; it never invents UUIDs or fuzzy-matches names (DESIGN-003 Q7(a) mirrored).
- **I-5 Gate-extended.** The zero-inference allowlist gate is extended so the workbook module family remains provably ramp-free: the ramp lives in its own module (`rust/src/repl/kyc_ramp.rs`, name indicative), and a new tooth forbids `crate::mcp`/`crate::agent`/ramp imports inside `kyc_workbook.rs` + `kyc_workbook_surface.rs` (closing §0.3). Direction of dependency: ramp → workbook types, never workbook → ramp.
- **I-6 Everything captured is governed.** No utterance is recorded, retained, or used for training/tuning outside the terms of the ratified capture charter (T7.1's hard gate).

## §2 Tranches (reshaped by R7)

### T7.1 — Charter, then capture (HARD-GATED on the charter artifact)

**Gate first:** the KYC capture charter is authored and ratified as its own artifact before any capture code. Minimum rulings the charter must contain (mirroring DESIGN-003's Q9/D3 entry-gate): what is captured (utterance text? refusals? acceptance/edit deltas?), retention + PII posture (utterances name real persons/UBOs), who may read the corpus, what tuning use is permitted, and the promotion criteria a T7.3 model must meet before it touches a live session.
**Then the code (one kit-adjacent batch):** a thin recorder around `KycWorkbookCommand` dispatch — captures `(utterance, placement_set_hash, proposal, disposition, user_action {accepted|edited|rejected}, staged_move_id?)` per charter terms. §0.8: tap points exist; no workbook engine change.
**Gate tests (RED first):** `capture_records_accepted_proposal_roundtrip`; `capture_respects_charter_scope` (nothing recorded outside charter'd fields); `workbook_module_remains_ramp_free` (the I-5 tooth — lands HERE, before any ramp code exists, so the fence precedes the thing it fences).

### T7.2 — Phrase estate scoping (RESHAPED: pin, don't build)

R7 deletes "create phrase banks from scratch": 21/21 verbs already phrased + embedded (§0.1). The real work:
1. **Kit-pin the phrase surface.** The invocation-phrase estate for the 21 verbs becomes part of the governed kit view: a closure tooth pins phrase-set coverage (every kit verb has ≥N phrases; no phrase maps to a retired verb — the K-G7 lesson applied to phrases).
2. **Placement-set-restricted retrieval (tier-0).** A retrieval function `rank_placement_set(utterance, &PlacementSet) -> Vec<(MoveId, score)>` that scores ONLY the frontier's legal moves using the existing Candle embeddings (mirror of HybridVerbSearcher's semantic tier, restricted to the board — mirror-not-adopt: new small function against `verb_pattern_embeddings`, not a dependency on `mcp::verb_search`). `NONE_OF_THE_ABOVE` scored alongside.
3. **Deterministic disposition v0** (threshold policy, no model): select if top-1 clears the bar with margin; clarify on multi-peak; abstain otherwise. This makes the ramp usable BEFORE T7.3 — tier-0 + thresholds is a shippable v0 ramp.
**Gate tests (RED first):** `ramp_only_proposes_board_moves` (I-2); `ramp_never_stages` (I-1 — proposal object has no write path); `phrase_coverage_tooth`; `multi_peak_clarifies_never_selects`; end-to-end: scripted utterance → proposal → user accept → ordinary stage → preview matches T4 semantics.

### T7.3 — SLM ranker (DESIGN-003 mirrored; gated on T7.1 charter + T7.2 corpus)

Tier-1 cross-encoder over the tier-0 shortlist, per DESIGN-003: sealed content-addressed model artifact, Candle runtime, returns scalar `FiniteScore` per candidate only; the T7.2 disposition policy consumes the scores unchanged (the policy is the constant; the scorer is the upgrade). Training corpus = T7.1's captured, charter-governed pairs. Promotion gated by absolute criteria set at gate time (recall@K on placement-set boards, false-select cap, abstention coverage, latency) — thresholds are explicitly OUT of this plan (plan v0.6 §Out-of-scope, unchanged).
**Gate tests (RED first):** `ranker_output_is_scores_only` (no disposition authority); `sealed_artifact_hash_pinned` (kit-hash-style pin on the model artifact); the T7.2 e2e re-run unchanged with ranker active.

## §3 Rulings needed before build (ratify per item)

| # | Ruling | Options | Recommendation |
|---|---|---|---|
| Q1 | Closing the §0.3 gate blind spot | (a) extend the allowlist tooth to forbid `crate::`-internal imports of ramp/mcp/agent modules inside the workbook module family; (b) move the workbook into the seam crate (structural, big) | **(a)** — small, lands in T7.1 as `workbook_module_remains_ramp_free`, before any ramp code exists |
| Q2 | Ramp surface | (a) new `KycWorkbookCommand::Propose{utterance}` in the T4.5 REPL surface; (b) separate command family | **(a)** — one cockpit, proposal renders exactly like a typed line awaiting `stage` |
| Q3 | Does the v0 ramp ship on tier-0+thresholds before any SLM exists? | yes / no (wait for T7.3) | **yes** — DESIGN-003's tier-0 is "the existing Candle phrase matcher"; the board restriction makes even v0 fail-closed |
| Q4 | Charter authorship | Adam authors; Claude drafts-for-ratification (same ceremony as TS.0) | **draft-for-ratification** — but it remains its own artifact, gating T7.1, per plan v0.6:103 |
| Q5 | Arg extraction in proposals | (a) template + resolver-slotted handles only (no LLM); (b) LLM arg extraction as in the ob-poc intent pipeline | **(a) for v0** — the placement-set move already names verb+target; most kit verbs need few free args; escalate to (b) only if v0 refusal telemetry demands it |

## §4 What is deliberately NOT in T7

No bpmn-lite code ports (§0.6 — nothing on disk, nothing left to port). No embedding-population work (§0.1 — done, 231 patterns). No new fold output, no lexicon/stud changes, no workbook engine changes (the ramp is strictly in front of `stage()`). No SLM thresholds (gate-time, DESIGN-003 pattern). No general-chat changes — the ob-poc pipeline already reaches dsl.kyc verbs outside the workbook; T7 governs only the workbook ramp. The charter itself (own artifact).

## §5 Sequence + dependency

```
T7.0 (R7) DONE ─► charter drafted ─► charter RATIFIED ─► T7.1 (recorder + I-5 tooth)
                                                        └► T7.2 (pin + tier-0 ramp v0)  ─► T7.3 (ranker)
```
T7.1 ∥ T7.2 after the charter (disjoint surfaces: recorder vs retrieval); T7.3 serializes on both (needs corpus + board ramp). Each tranche = one batch; RED-first gates; dep-gate stays green throughout (no kyc-crate deps change at all — everything lands in the ob-poc app crate).

## §6 Ratification

Reply per section (§1 invariants, §2 tranche shapes, §3 rulings Q1–Q5), or "accept all". On ratification of Q4, the charter draft is the next artifact; T7.1 code waits for the charter's ratification, not this plan's.

## §7 Amendment (2026-08-17) — the "board" model, ratified

Appended per the ISA-002/DESIGN-003 amendment pattern (append, don't rewrite
in place — see DESIGN-003 v0.6→v0.7 §20). Adam's own words, ratified as this
plan's standing vision for what `PlacementSet`/`LegalMove`/`board_hash`
*mean*, not merely how they're implemented:

> "Board" here should not mean a fixed chessboard layout; it means the
> authoritative, inspectable state over which rules determine the next legal
> transformations.
>
> For UBO, separate two layers:
>
> ```text
> SemOS taxonomy / ontology / DSL schemas
>     = game definition, pieces, constraints, and move grammar
>
> Current UBO instance graph + focus + revision + history
>     = the constructed board / current position
>
> A typed node-DSL transformation
>     = a legal move at that position
> ```
>
> So the UBO game is closer to Lego, a graph-rewriting system, or a
> construction game than chess:
>
> - The board is built as play proceeds.
> - Each accepted move changes the board.
> - That changed board changes what may legally be added, connected,
>   refined, replaced, deprecated, or removed next.
> - The "pieces" are typed entities, relations, claims, classifications,
>   authorities, evidence links, and scopes — not physical tokens.
> - The SemOS DAG/taxonomy supplies the semantic constraints that make a
>   transformation meaningful and legal.
>
> A precise formulation:
>
> > A UBO board is the current authoritative constructed UBO instance,
> > interpreted against the admitted SemOS taxonomy and governed DSL. A legal
> > move is a position-bound typed graph transformation whose resulting
> > instance admits under those rules.
>
> The taxonomy itself is slightly ambiguous because it can play two roles:
>
> 1. As an admitted, relatively stable semantic substrate, it is part of the
>    rules/arena: it defines types, relations, constraints, and permitted
>    DSL operations.
> 2. If users are themselves editing the taxonomy, then that taxonomy
>    revision is also the mutable constructed board for a higher-order
>    taxonomy-authoring game.
>
> Recursive model:
>
> ```text
> UBO instance-authoring game
>   board: current UBO instance
>   rules: admitted taxonomy + instance DSL
>
> Taxonomy-authoring game
>   board: current taxonomy/SemOS graph
>   rules: meta-taxonomy + taxonomy DSL
> ```
>
> User-facing language: "semantic construction board" or "governed graph
> workspace" — avoiding the misleading implication that the topology is
> static — while retaining `DesignPosition`/`Gameboard` in the formal model,
> preserving the game-theoretic properties: explicit state, legal moves,
> authority, previews, history, correction, and changing move sets.

### §7.1 Where the built code already matches this

`enumerate_placement_set` (`crates/ob-poc-kyc-substrate/src/placement.rs`) is
not a fixed layout — it is recomputed fresh, every call, from a refold of the
append-only `kyc_intent_events` stream (`KycWorkbook::validate()`,
`rust/src/domain_ops/kyc_workbook.rs`). `board_hash` is a content hash over
the ordered move-id list *at that folded position*, not a static version
number — it changes the instant the folded state changes. `LegalMove` is a
`verb_fqn` bound to a `target`, admitted iff the verb's lexicon preconditions
hold against the *current* fold — a position-bound typed transformation, not
a piece on a grid. This is the **instance-authoring game** layer of the
recursive model above, and it is the layer T7 (this plan) operates in front
of: `Propose`/`PendingProposal` only ever rank and remember candidates
`enumerate_placement_set` already produced; they never construct, cache, or
freeze a board of their own (I-1, I-2).

### §7.2 Where it does not (yet) — scope boundary, not a gap in this plan

The **taxonomy-authoring game** (rules-as-board, one level up) does not exist
for KYC today. `enumerate_placement_set`'s `lexicon: &LexiconManifest`
argument is `phase1_lexicon()` — a hardcoded Rust manifest (currently 21 of
the pack's verbs; `placement.rs`'s own doc comment flags the drift as
"K-G6... a lexicon-closure problem, not a placement-set problem"), not a
governed, versioned, publishable SemOS object the way CBU/Catalogue DAG
taxonomies are elsewhere in this repo. Whether the KYC lexicon should become
an authored board of its own is a real, separate design question — out of
T7's scope (§4) and not addressed by this amendment; recorded here only so
the boundary is explicit rather than silently assumed.

### §7.3 Naming note

`DesignPosition`/`LegalMove`/`GameDisposition`/`MoveAttemptOutcome` are the
BPMN Designer's gameboard turn-model vocabulary
(`docs/eop/EOP-VS-BPMN-DESIGN-003.md` v0.7 §20, ratified 2026-08-11). KYC's
`PlacementSet`/`MoveId`/`LegalMove`/`board_hash` (`placement.rs`) is a
deliberately independent instance of the same pattern — "mirror the
gameboard discipline, do not adopt its types" (`placement.rs:1-8`, citing
T0.1 rec c). Both are conceptually the construction-game model above; they
are not, and are not required to be, the same code.

## §8 Amendment (2026-08-17) — gameboard vocabulary adopted from `semantic-decision-contracts`

**Refines §7.3, does not contradict it.** §7.3 stated BPMN and KYC's board/move
types "are not, and are not required to be, the same code" — that remains true
for the *graph-shaped* types (`DesignPosition`/`LegalMove`/`GraphRevision`/
`GraphDeltaPreview`: BPMN authoring-session concepts — focus/viewport,
compiler profile, policy identity, edit history — with no honest KYC
equivalent; forcing them onto KYC's event-stream-native board would mean
stubbing fields with no real meaning). What changed: the *bare, graph-agnostic
vocabulary* genuinely is domain-agnostic, and — following direct investigation
of `bpmn-lite`'s `codex/bpmn-gameboard-refactor` branch, which proved this
vocabulary out for BPMN — is now literally shared code, not a parallel
hand-rolled mirror:

- **`ABSTENTION_CANDIDATE_ID`** (`ob-poc-kyc-substrate/src/placement.rs`):
  `PlacementSet::NONE_OF_THE_ABOVE` is now a re-export of
  `semantic_decision_contracts::ABSTENTION_CANDIDATE_ID` (byte-identical
  value; zero behavioral change).
- **`MoveAttemptOutcome`** (`rust/src/domain_ops/kyc_ramp_capture.rs`):
  replaces the local 3-value `UserAction` (`accepted`/`edited`/`rejected`)
  with the shared 10-value terminal-outcome vocabulary. Real precision gain:
  distinguishes the board refusing a proposal as no-longer-legal
  (`compiler_refused`) from the operator declining an accepted one
  (`rejected_by_user`, not yet produced by any call site), and an operator
  staging a different-but-still-legal move (`corrected`) from an exact match
  (`applied`). Required its own charter amendment —
  `EOP-DD-KYCUBO-CAPTURE-CHARTER_v0.1.md` §7 — since the charter locks §1's
  value vocabulary by name, not just its field list.

**Deliberately NOT adopted this pass:** `Disposition`
(`select`/`clarify`/`abstain`, `kyc_ramp_capture.rs`) onto `GameDispositionKind`.
`Select`→`ProposeMove` and `Clarify`→`ClarifyMoves` map exactly, but KYC's
`Abstain` ("no candidate ranked confidently enough") has no exact match among
`GameDispositionKind`'s 10 variants — `OutOfScope` is the closest fit but
claims something stronger ("this utterance isn't about this domain at all")
than KYC means. Forcing it would also require a second charter amendment for
an admittedly-imperfect mapping. `Disposition::Abstain` is confirmed dead on
the write path today (an `Abstain` disposition never builds a
`PendingProposal`, so it never reaches the capture table) — there is no
pressing correctness reason to force this now. Left as a flagged, explicit
follow-up, not silently dropped.

**Dependency mechanics:** `semantic-decision-contracts` comes from
`github.com/adamtc007/dsl` — the same upstream repo `sem_os_core`/
`sem_os_ontology`/`sem_os_policy` already come from — pinned alongside them in
`rust/Cargo.toml [workspace.dependencies]`. The pin moved from `a38eefe1`
(2026-08-05, no gameboard types) to `1d039d9` (2026-08-11, adds
`GraphStateHash` + the gameboard module — a commit on the upstream `dsl`
repo's own `refactor/sem-os-pack-policy` branch, not yet merged to its
`main`; pinned by immutable commit hash, which is stable regardless of branch
status). `ob-poc-kyc-substrate/Cargo.toml`'s "no sem_os_core (git dep)" rule
gained a named exception: `semantic-decision-contracts` is a bare
host-neutral vocabulary crate (`hex`/`serde`/`sha2`/`thiserror` only, no DB),
materially lighter than what that rule was written to keep out.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-14 | Initial plan. T7.0/R7 executed and folded in as §0 (9 findings). T7.2 reshaped create→pin (21/21 verbs already phrased + embedded). All bpmn-lite port subtasks DELETED (repo absent; discipline already mirrored). New finding: zero-inference gate skips `crate::` imports — I-5/Q1 closes it. v0 ramp shippable on tier-0+thresholds ahead of any SLM. |
| 0.1 §7 | 2026-08-17 | Amendment (append-only, ISA-002/DESIGN-003 pattern): ratified Adam's "board" vision — construction/graph-rewriting model, not a fixed chessboard; two-layer recursive model (instance-authoring game vs. taxonomy-authoring game); confirmed the built `PlacementSet`/`board_hash`/`LegalMove` machinery matches the instance-authoring layer; flagged the taxonomy-authoring layer as not yet built (real, separate, out-of-scope question); noted `DesignPosition`/`Gameboard` (BPMN) vs `PlacementSet` (KYC) are independent, deliberately unshared instances of the same pattern. |
| 0.1 §8 | 2026-08-17 | Amendment: adopted `semantic-decision-contracts`' `ABSTENTION_CANDIDATE_ID` + `MoveAttemptOutcome` directly (dsl rev bumped to `1d039d9`, `ob-poc-kyc-substrate` dependency exception named); capture charter amended (§7) for the widened `user_action` vocabulary; `GameDispositionKind` adoption explicitly deferred (imperfect `Abstain` mapping, would need a second charter amendment, `Disposition::Abstain` confirmed dead code on the write path). Graph-shaped types (`DesignPosition`/`LegalMove`) remain unadopted per §7.3 — this amendment only widens what "mirror, don't adopt" excludes. |
