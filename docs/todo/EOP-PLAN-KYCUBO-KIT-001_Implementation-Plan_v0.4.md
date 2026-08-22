# EOP-PLAN-KYCUBO-KIT-001 — Implementing the UBO Construction Kit
### Tranche-based implementation plan for EOP-VS-KYCUBO-KIT-001 v0.2

| | |
|---|---|
| **Document** | EOP-PLAN-KYCUBO-KIT-001 |
| **Version** | 0.5 — T0 closed and executed; T4 verified |
| **Owner** | Adam Cearns |
| **Binds to** | EOP-VS-KYCUBO-KIT-001 **v0.2** (persona ladder, KIT-11/12); EOP-SA-OBP-001; EOP-VS-BPMN-DESIGN-003 |
| **Grounded in** | The R1–R8 dossier (bpmn-lite post-`68723b9`; `semantic-decision-contracts` @ `1d039d9`; ob-poc kyc crates) |
| **Status** | **T0 CLOSED AND EXECUTED** (2026-08-12): K-G7 retired, K-G6/T6.0 closed, seventh closure tooth built, T4 verified 5/5 against a live DB. T1–T3 already landed; T5/T6.1+/T7 remain. |

> **Sequencing logic.** The Vision's baseline is a **zero-inference DSL IDE for the super user** (KIT-11) — so the plan drives to that milestone first: source of truth (T1) → placement-set (T2) → preview (T3) → **the IDE session (T4, the baseline product)**. Bitemporal recovery (T5) and stud-geometry authoring (T6) run as parallel tails. The plain-English ramp (T7) comes last, bootstrapped from expert corpus (KIT-12), behind its own charter gate. **Every tranche is gated by RED-first tests written before the build** — a tranche closes only when its gate tests go red→green and the substrate dep-gate stays green.

---

## T0 — Ratifications (CLOSED 2026-08-12; recommendations were attached, all accepted)

**T0.1 — Contracts-crate stance (amends V&S position #7 — amendment in V&S v0.2).**
R2: `semantic-decision-contracts` is *operationally graph-specific* (`GraphRevision`, `GraphDeltaPreview`, `GraphElementRef` anchors presume a mutable versioned graph). **RATIFIED: (c)** — KYC-native board/move types mirroring the contract *discipline* (content-addressed move identity, canonical ordering, `NONE_OF_THE_ABOVE`, board content-hash, single-disposition-function); crate adoption only if/when its graph-specific types split from a generic core (stated later convergence).

**T0.2 — S-expression: persist-at-capture vs render-on-demand.**
**RATIFIED: persist at capture** — the validated workbook's resolved S-expression is what gets stored (KIT-1 literally satisfied); one-time backfill render for existing history, provenance-marked `backfilled`; the renderer retained as a CI **equivalence check** (round-trip: `parse(source_text) ≡ stored event`), not as the source of truth. Derive-on-demand quietly promotes the renderer to a second crown-jewel trust component.

**T0.3 — SemOS KYC/UBO pack closure review (expanded: verb recount + graph/DSL node-set closure).**
Analysis-only audit — the state-graph-closure discipline applied one layer up, at the pack/kit level — establishing the kit's true universe before T2 locks the placement-set and T6 locks authoring scope. Gap classes, in kit terms:
- **K-G1 unreachable block** — a state in the fold's enum state-space (EdgeStatus, obligation track states, rollup states) with no verb that transitions INTO it.
- **K-G2 dead node** — a state reachable but with no verb OUT, not declared terminal. *(Known prior: `Deferred`/`Expired` had fold arms, zero call sites.)*
- **K-G3 unmapped move** — a verb registered in the lexicon but covered by **neither** a DAG slot **nor** the `stream_governed:` family list (true drift; post-RW-4, absence-from-DAG alone is not a gap).
- **K-G4 phantom move** — a verb declared in pack graph/DAG that is not registered/implemented.
- **K-G5 geometry-free block** — a registered verb with no precondition (trivially-true studs); the KIT-9 defect register, feeding T6 scope directly.
- **K-G6 declaration drift** — the `stream_governed:` verb-family list vs the actually-registered verb set (has the RW-4 exemption drifted since bpmn-lite changes landed?).
- **K-G7 fold-blind write** *(seventh class, surfaced by the audit itself)* — a verb declared state-mutating with zero fold match arm: events append, return a seq, and are permanently invisible to every fold, projection and query.

**T0.3 COMPLETE (2026-08-12).** Audit: `EOP-DD-KYCUBO-KIT-T0.3_Pack-Closure-Audit` (v0.2, corrections applied); teeth: `rust/tests/kyc_pack_closure.rs` (6 pure tests, green; 7th commissioned). Outcome: K-G1–K-G4 **clean** (verified structurally); denominator closed at **22 verbs** (all op-registered; 12 lexicon-covered; 3 precondition-carrying); K-G5 confirmed as most of the pack (T6's real backlog); K-G6 confirmed (10 uncovered); **K-G7 found live**: `kyc.role.assign`/`withdraw` fold-blind. **Ratified:** K-G7 → **retire both verbs** (reintroduction path recorded; universe → 20, uncovered set → 8); K-G6 → **split out of T6 as T6.0, close early** (uncovered verbs write degraded renders permanently into the KIT-1 source of truth); register accepted with corrections (precondition count = 3; phantom test citation struck; seventh tooth `precondition_and_strategy_coverage_is_exactly_known` commissioned). "Tests passing" means the register matches reality — not that the pack is closed.

---

## T1 — S-expression source of truth (closes KIT-1)

**Scope** (per T0.2): pure substrate fn `render_intent_event_to_sexpr(event, lexicon_entry) -> String`; `source_text` column on `kyc_intent_events` (nullable through backfill, then NOT NULL for new appends); capture-at-append in the store path; one-time backfill migration; provenance marking.
**Gate tests (RED first):**
- `sexpr_roundtrip_property`: for generated events across all verb kinds, `parse(render(event)) ≡ event` — the CI equivalence gate.
- `append_captures_source`: a governed append persists non-null resolved `source_text`; UUIDs resolved, not placeholders.
- `history_replayable_as_dsl`: a subject's full history renders to a replayable DSL sequence; re-executing it against an empty store reproduces the folded state bit-identically.
- Dep-gate: `check_kyc_substrate_deps.sh` green (renderer is pure, lives in substrate).
**Routing:** Sonnet — pure function + migration + property tests against a fixed contract.

## T2 — The placement-set generator (closes KIT-3)

**Scope:** KYC-native board/move types per T0.1c; a precondition-backed legality component in the substrate: enumerate `LexiconEntry.preconditions` across the registered verb set, evaluate against the folded `ControlState`, emit the placement-set with canonical ordering, `NONE_OF_THE_ABOVE`, and a board content-hash. Pure: `(state, lexicon) -> PlacementSet`. Includes the §14-Q2 cost benchmark.
**Gate tests (RED first):**
- `placement_iff_precondition`: property — a verb appears in the set **iff** its precondition holds against the state (differential against `check_control_preconditions` as oracle, per verb, per generated state).
- `placement_set_deterministic`: same state ⇒ bit-identical set and hash (BTreeMap discipline; no HashMap iteration).
- `abstain_always_present`; `canonical_order_stable`.
- Benchmark recorded (closes V&S §14 Q2); dep-gate green.
**Routing:** the board-type contract is a half-day design (Opus) for sign-off; the build is Sonnet.

## T3 — Preview / lookahead (closes KIT-7)

**Scope:** the thin wrapper the dossier confirms suffices — `preview(committed, candidates, registry) -> ControlState` (fold over `committed ++ candidates`, zero store involvement); **chain preview mirroring `resolve_hypothetical_chain` semantics** (per-step admission: candidate *n* validates against the state folded from committed + candidates 1..n-1) without the DAG clone; commit-by-replay only (no speculative-commit path exists, by construction).
**Gate tests (RED first):**
- `preview_is_pure`: no store dependency in preview's call graph (dep-gate + a test asserting zero writes/reads against a live store).
- `per_step_matches_single_move`: differential — chain-preview step validation ≡ the real single-move validation, verb by verb.
- `committed_line_equals_preview`: committing a previewed line through the real append path yields state bit-identical to the line's final preview.
- `illegal_mid_chain_rejected`: a chain with an illegal step *n* rejects at *n* with steps 1..n-1 uncommitted.
**Routing:** Sonnet; the differential test is the one to eyeball.

## T4 — The super-user IDE session (the baseline product; closes KIT-10/11)

**Scope:** the milestone the Vision names — a complete, **zero-inference** session for the DSL-literate user: session open = kit ⊕ last snapshot (new-UBO empty-baseplate path and existing-UBO path); a KYC proposal-workbook type (staged move sequence, KYC-native); **tier-0 deterministic matching only** (no model anywhere in the path); the Repl validate loop = re-run-whole over the staged workbook (T2 ∘ T3 composed); commit = T3 replay + T1 source capture + snapshot refresh (= recompute-at-next-open, per the T4 design doc's plan-wording reconciliation). `ValidVerbSetEngine`: **RETIRED for this type** (ruled in the T4 design doc, v0.2). Design doc: `EOP-DD-KYCUBO-KIT-T4_Super-User-IDE-Session-Design` v0.2 (review edits applied — recognition frontier, kit pinning, gate strengthening; UUID-literal baseline, resolver deferred per ratified option (a)).
**Gate tests (RED first):**
- `session_roundtrip`: dependent 3-move chain (assert-control → attach-evidence → verify) — mandatory fixture; commit; re-open reconstruction ≡ final preview, bit-identically; canonical resolved `source_text` asserted.
- `new_ubo_from_baseplate`: the empty-start path end-to-end.
- `invalid_workbook_blocks_commit`: validate-blocks AND direct-commit-rejects, zero rows appended (real rolled-back transaction, inspected).
- `zero_inference_assertion`: import ALLOWLIST over the module (fails closed) + feature check where one exists.
- `stale_snapshot_recovers`: two-connection harness; commit re-validates against true current state, never silently stale.
**Routing:** design doc signed off; build prompt issued (Sonnet).

## T5 — Bitemporal recovery (parallel tail; completes KIT-6's pin set)

**Scope:** `recover_determination_bitemporal(subject, valid_at, known_at)`; the constructor takes the axis parameter (EOP-SA-OBP-001 §I.5 target). Parallel to T2–T4; nothing before T4 depends on it, but the frozen-view pin is incomplete without it.
**Gate tests (RED first):** `axes_diverge_on_correction` (a superseded past fact yields different graphs on valid-time vs knowledge-time axes — the regulator's two questions, answered differently, provably); `bitemporal_matches_txtime_when_axes_align`.
**Routing:** Sonnet — both timestamps already on the events; predicate + plumbing.

## T6 — Stud-geometry authoring (closes KIT-9, incrementally; scope locked by T0.3)

**T6.0 — K-G6 lexicon-entry closure (early, independent, runs any time after the K-G7 retirement).** Author `LexiconEntry` coverage for the **8** remaining uncovered verbs (`kyc.obligation.*` ×6, `kyc.person.*` ×2). Entries only — **preconditions explicitly NOT authored here** (that is T6.1+). Rationale (ratified): uncovered verbs append the degraded fallback render permanently into the authoritative `source_text` (KIT-1). Gate: `lexicon_manifest_coverage_gap_is_exactly_known` flips from pinning a 10-gap to pinning **closure** (empty uncovered set), plus a render-delta check proving entries change the persisted form. Prompt issued (Sonnet).

**T6.1+ — precondition authoring.** Scope (post-T0.3, post-retirement): **17 of 20 verbs** lack any precondition (beyond the 3 that carry them) + the structure-class/strategy pairing gap (6 of 11 classes). The authoring **discipline** first (how a precondition is declared, tested, gated — one worked exemplar per precondition kind), then per-verb-family: edge family → determination family → obligation/person families. The game runs from T4 on the permissive subset and tightens as each family lands.
**Gate tests (RED first), per family:** each new precondition ships with a red-first pair — `precondition_blocks_illegal_placement` / `precondition_admits_legal_placement` — plus the T2 `placement_iff_precondition` property re-run (the board automatically reflects new geometry, no board-side change) and the seventh closure tooth updated (the precondition map is pinned; authoring a precondition is a conscious pin edit).
**Routing:** the preconditions are **domain rulings** (Adam; possibly compliance input on some); encoding + tests are Sonnet.

## T7 — The plain-English ramp (last; closes KIT-12's frame; charter-gated)

**Scope, staged:**
- **T7.0 — research spike (bounded, the plan's one acknowledged gap):** what utterance→verb matching exists in *ob-poc today* (any matcher/phrase-bank infra outside bpmn-lite), and what tier-0 machinery is worth porting vs writing. One dossier-style pass, cited, before T7 design locks.
- **T7.1 — corpus capture plumbing** (charter-gated before any *use*): expert-session utterance→move pairs captured per KIT-12; the **KYC capture charter is its own artifact** and a hard gate — no training use without it.
- **T7.2 — phrase banks:** enriched from captured expert corpus; deterministic expansion (still no model in the disposition path).
- **T7.3 — SLM ranker:** the DESIGN-003 contract applied to the KYC placement-set — ranker-only `SlmResult`/`FiniteScore`, single terminal disposition, promotion ladder (shadow → suggest-only) with thresholds ruled at gate time against real data.
**Gate tests (RED first):** `ramp_changes_recognition_not_legality` (an assisted-user move passes the identical placement/validation/commit machinery as a typed one — structural differential); `no_model_disposition` (the SLM's output cannot reach a disposition except through `decide` — the DESIGN-003 invariant, re-proven here); capture off-by-default until the charter artifact exists (a config-level test).
**Routing:** T7.0/T7.2 Sonnet; T7.3 inherits the DESIGN-003 build pattern.

---

## Dependency graph

```
T0.1 ─┬─► T2 ─┬─► T4 ══ BASELINE MILESTONE (zero-inference IDE live)
T0.2 ─┴─► T1 ─┤          ▲            │
              └─► T3 ────┘            ├─► T7.1 → T7.2 → T7.3 (ramp; charter-gated)
T0.3 ✅ ────────────────► K-G7 retirement → T6.0 (K-G6 entries, early) ; T2 universe = 20 ; T6.1+ scope = 17-of-20 + strategy pairing
T5 ── parallel ──────────► KIT-6 frozen-view completeness
T7.0 (spike) ────────────► T7 design lock
```

## Out of scope (fenced)
The KYC corpus **charter** (own artifact — T7.1 hard-gates on it); any bpmn-lite change (T0.1's convergence is later, stated); Mode-1 construct migration (bible §II.1 — separate programme); SLM threat model / promotion thresholds (ruled at gate time, DESIGN-003 pattern).

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | — | Initial plan from the R1–R8 dossier: three T0 ratifications, six tranches, source-first sequencing. |
| 0.2 | — | Re-sequenced to the **super-user zero-inference baseline** (V&S v0.2 §2a): T4 reframed as the IDE-session milestone with a structural `zero_inference_assertion` gate; **T7 added** — the plain-English ramp (research spike → charter-gated corpus capture → phrase banks → SLM), last by design per KIT-11/12; explicit RED-first gate tests on every tranche; T0 ratifications carried (still open). |
| 0.3 | — | **T0.3 expanded into the SemOS KYC/UBO pack closure review**: the state-graph-closure discipline applied at the pack/kit level — six kit-terms gap classes (K-G1 unreachable block, K-G2 dead node, K-G3 unmapped move outside both DAG slots and the `stream_governed` families, K-G4 phantom move, K-G5 geometry-free block feeding T6, K-G6 `stream_governed` declaration drift). Dead-node scope is the **fold's enum state-space**, not just DB CHECKs. Feeds T2 (true universe) and T6 (scope); subsumes the verb recount. |
| 0.4 | 2026-08-12 | **T0 closed.** T0.3 complete (audit v0.2 + 6 teeth green): K-G1–K-G4 clean; **K-G7 fold-blind-write surfaced as the seventh gap class** — `role.assign`/`withdraw` retired (ratified; reintroduction path recorded; universe → 20). K-G6 split out as **T6.0** (early lexicon-entry closure — degraded renders were writing permanently into the KIT-1 source of truth). T6.1+ scope restated: 17-of-20 preconditions + 6-of-11 strategy pairing. Seventh tooth commissioned. T4 design doc signed off at v0.2; T4 build prompt issued (Sonnet; UUID-literal baseline, resolver deferred per ratified option (a)). Executor: Sonnet in Zed (was Codex). |
| 0.5 | 2026-08-12 | **T0 EXECUTED.** K-G7: `kyc.role.assign`/`withdraw` deleted (YAML, op structs, registry, DAG/pack surface membership, generated `.dsl` mirror), universe 22→20; a first-pass recon gap (missed `rust/tests/`) was compiler-caught and fixed in the same commit. K-G6/**T6.0 closed**: 8 `LexiconEntry` values authored (entries only, no preconditions); found and corrected a factual error in T0.3's own K-G6 framing while executing — `render_intent_event_to_sexpr` never used the entry to change rendered text (only a debug-only fqn-match assert), so the commissioned "render-delta" tooth was replaced with an fqn/taxonomy/render-safety tooth instead of forcing a false assertion. Seventh tooth (`precondition_and_strategy_coverage_is_exactly_known`) built and green; `kyc_pack_closure.rs` now 8 tests. **T4 verified**: the already-built `kyc_workbook.rs` (module + 5 gate tests) matches design v0.2 exactly (frontier recognition, kit pinning, never-panic commit path, dependent-chain fixture) — 5/5 green against a live DB, no changes needed. Full receipts: `ob-poc-kyc-substrate` 41/41, scoped clippy clean, dep-gate PASS. T5/T6.1+/T7 untouched. |
