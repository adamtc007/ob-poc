# The Super-User IDE Session — T4 Design
### Design Doc — EOP-PLAN-KYCUBO-KIT-001 §T4 (closes KIT-10/11)

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-KIT-T4 |
| **Type** | Design Doc — pre-implementation, for sign-off per the plan's own gate |
| **Version** | 0.2 — Review edits applied (recognition frontier, kit pinning, gate strengthening) |
| **Owner** | Adam Cearns (ruling), drafted by agent |
| **Status** | **Draft. Not implemented. No code changes in this doc.** |
| **Binds to** | `EOP-PLAN-KYCUBO-KIT-001` v0.3 §T4 (mandate + gate tests); `EOP-VS-KYCUBO-KIT-001` v0.2 §2a/§6/§7/§14 Q3 (baseline scope, session model, the open question this doc must answer); `EOP-SA-OBP-001` v0.6 §I.3/§II.4 (the constructor discipline this session overlays); T1 (`render_intent_event_to_sexpr`), T2 (`enumerate_placement_set`/`PlacementSet`), T3 (`preview`) — all landed and green, this design composes them without changing any of them |

> Per the plan: *"T4 design doc first (mine, for your sign-off) — it must answer V&S §14 Q3 (lookahead surfaces move-ranking, consequence-evaluation, or both as distinct affordances)."* This doc exists to get that ruling, plus the `ValidVerbSetEngine` disposition the plan also defers to T4, before any T4 code is written. **Nothing here is built yet.**

---

## §0 What T4 is, precisely

KIT-11's zero-inference baseline: a complete session for the DSL-literate super user — stage moves, preview lines of play, commit validated workbooks — with **no model anywhere in the path**. KIT-10's versioning: a session opens as kit ⊕ last snapshot, where the snapshot is a *regenerable checkpoint*, never authoritative.

T4 is explicitly a **composition** tranche, not a new-mechanism tranche. Every hard part it needs already exists and is gate-tested:

| Need | Already have |
|---|---|
| "What's legal right now?" | T2 `enumerate_placement_set(subject, state, lexicon) -> PlacementSet` |
| "Is this staged sequence legal end-to-end?" | T3 `preview(committed, candidates, lexicon) -> Result<ControlState, KycError>` |
| "What DSL text does a staged move render as?" | T1 `render_intent_event_to_sexpr(event, entry) -> String` |
| "Parse typed DSL text back into a candidate move" | `dsl_parser::parse` (the real workspace lexer/parser — already the T1 roundtrip test's oracle, not a new component) |
| "Commit a validated line for real" | `ob_poc_kyc_seam::append_in_scope` → `PgKycEventStore::append` (the existing §3 append protocol, already re-validates preconditions at placement, already takes `source_text`) |

T4's job is to wire a **workbook/session shape** around these five pieces, decide the two things the plan explicitly left open, and specify the gate tests concretely enough to write RED-first.

---

## §1 Ruling on §14 Q3 — what "lookahead" means here

> *"Is workbook lookahead about ranking candidate moves (utterance fit) or evaluating consequences (does this placement resolve the determination)? Different 'aheads'; T4 design names which the workbook surfaces (possibly both, as distinct affordances)."*

**Ruling: both, as two distinct, separately-named affordances. Both exist at baseline in deterministic form; what is absent at baseline is *ranking*, by construction of KIT-11.**

1. **Recognition** ("which legal move did the user mean?") — utterance/typed-text → a specific `LegalMove` from the current `PlacementSet`. At baseline this is **tier-0 deterministic matching only** (§6 below): parse what was typed with the real `dsl_parser`, and accept it iff the resulting `(verb_fqn, target)` pair is a member of the **frontier placement set** (§6 step 3 — computed over the state after all already-staged moves). There is no ranking, no scoring, no "closest match" — an unrecognised or currently-illegal move is refused with the reason, not guessed at. **Recognition-as-ranking is explicitly out of scope for T4** — that is the SLM ramp's job (T7, `EOP-VS-BPMN-DESIGN-003`'s ranker-only `SlmResult` contract), and ranking cannot exist at baseline without contradicting KIT-11's "zero model in the path."
2. **Consequence preview** ("what does staging this do to the build?") — for a staged workbook (an ordered list of already-*recognised* candidate moves), `preview(committed, candidates, lexicon)` returns the resulting `ControlState`. The IDE surfaces this state directly: which edges exist and at what status, whether the subject is reconciled, whether a strategy is selected — i.e. **is the workbook, if committed as staged, closer to a freezable determination?** This is a deterministic *read* of T3's output, not a score. No new substrate code — the session layer just presents `ControlState` fields the user already has vocabulary for (they're literally the DSL's own state names).

**What "both, as distinct affordances" resolves:** the two questions are answered by two different functions over two different inputs (`enumerate_placement_set` over the **frontier state** — `preview(committed, staged[0..n])`, the state after all currently-staged moves — for recognition-eligibility of the next move; `preview` over the full *committed ++ staged* for consequence), and the IDE must not conflate them into one "recommendation" surface — that conflation is exactly the door KIT-8 closes ("the AI proposes placements only... never binds open free-text arguments" — recognition and consequence are read-only projections, not proposals, at baseline since there is no AI in this tranche at all).

**What is explicitly NOT at baseline:** "which move should I make next to reach a determination" (a planning/search affordance) is neither of the two named aheads and is out of scope — it would require ranking candidate *sequences*, not recognising one typed move or previewing one staged sequence. Flag for a future tranche if wanted; not KIT-11.

---

## §2 Ruling on `ValidVerbSetEngine` — retire, do not implement, for the KYC workbook

The plan: *"`ValidVerbSetEngine` (trait, zero impls) gets its first implementation here or is retired for the workbook type — decided in T4 design, recorded either way."*

**Ruling: retire it for this type. Do not implement.**

Checked the actual trait (`ob-poc-sage::engine::ValidVerbSetEngine`, `crates/ob-poc-sage/src/valid_verb_set.rs`): it is shaped around `(workspace, constellation_id, entity_state) -> ValidVerbSet`, computed from `sem_os_core` constellation maps and FSM transitions (`VerbSource::FsmTransition`, `client_group_id`, `constellation_id`), feature-gated behind `database`. That is a **different legality mechanism** — DAG-slot/FSM-transition-shaped, cross-workspace-generic, SemOS-store-backed — from what a KYC workbook needs, which is **precondition-shaped, event-fold-backed**, and deliberately store-free at the substrate layer (`ob-poc-kyc-substrate` has no SemOS/DB dependency — the dep-gate enforces this).

Forcing `PlacementSet` through `ValidVerbSet`'s shape would mean either (a) inventing fake `VerbSource::FsmTransition` provenance for moves that were never FSM transitions, or (b) growing `ValidVerbSetEngine` a second, incompatible computation mode — both are the exact "conflation" the KIT-001 V&S already ruled against once, for `semantic-decision-contracts` (T0.1c: mirror the discipline, don't force-fit a structurally mismatched type). Same call here, same reasoning, applied to an in-repo trait instead of an external crate.

**Consequence:** the KYC workbook exposes its own `PlacementSet`/`LegalMove` (already built, T2) as *the* legality surface. `ValidVerbSetEngine` stays exactly as it is — a zero-impl trait for the constellation/FSM domain — untouched by this tranche. If a future need arises to present KYC moves through the generic Sage `ValidVerbSet` surface (e.g. a unified cross-workspace verb palette), that is an adapter written *later*, at the boundary, translating `PlacementSet` → `ValidVerbSet` for display purposes only — never the other way, and never as the workbook's own legality source. Not scoped here; noted so the option isn't lost.

---

## §3 Where this lives

The five pieces T4 composes (§0 table) span three existing layers: `ob-poc-kyc-substrate` (pure — T1/T2/T3), `ob-poc-kyc-seam` (the DB/dsl-runtime chokepoint), and `dsl_parser` (workspace-wide, no KYC dependency). A workbook/session needs to hold state across turns (staged moves not yet committed) and, on commit, drive the **real governed append path** — which means it needs `dsl-runtime`'s `TransactionScope` and eventually a DB connection. That disqualifies `ob-poc-kyc-substrate` (must stay pure, dep-gated) as the home.

**Proposed: a new module, not a new crate — `rust/src/domain_ops/kyc_workbook.rs`, alongside `kyc_stream_ops.rs`.** Reasoning:
- The commit step should be **literally the same dispatch** real `dsl.kyc` verb execution already uses (§5) — not a parallel append mechanism. That code already lives in `rust/src/domain_ops/`, wired into the app's `SemOsVerbOpRegistry` and `ReplOrchestratorV2`. A sibling module reuses that wiring instead of duplicating it.
- A new crate would need to depend on `dsl-runtime`, `ob-poc-kyc-seam`, `dsl_parser`, and (for commit) the app's verb registry — at that point it *is* app-layer, and a new crate boundary buys type-safety isolation the substrate already provides one layer down, at the cost of a sixth KYC crate to keep straight. Not worth it for a composition tranche.
- **Zero-inference is enforced at the module boundary, not the crate boundary** (§6/§8): a source-scanning invariant test over this one file (same pattern as `runbook::invariant_tests`, `PACK001`) is exactly as strong as a dep-gate script would be here, without the crate overhead.

**Flagging for your review, not decided unilaterally:** this is the one placement choice the plan didn't pre-rule. If you'd rather this be its own crate (e.g. anticipating the session type growing complex enough to want its own test/doc boundary, or wanting it out of `rust/src` for reuse from `ob-poc-web` without pulling in the rest of `ob-poc`), say so and I'll re-scope before RED tests are written.

---

## §4 The workbook type — KYC-native, mirroring `ProposalWorkbook`'s shape

Per KIT-001 §T4 and the V&S's own framing (mirror discipline, not adoption — same T0.1c pattern applied consistently): a small, KYC-native staged-sequence type. No new persistence — a workbook lives for the duration of a session (in-memory / session-scoped), same as `ReplSessionV2`'s existing run sheet.

```rust
/// One recognised, not-yet-committed candidate move (§6 — the output of
/// tier-0 recognition against the FRONTIER placement set, never a guess).
pub struct StagedMove {
    pub event: IntentEvent,      // fully formed — verb, target, payload, as_of
    pub source_text: String,     // T1: rendered at stage time, not commit time —
                                  // what the user sees IS what gets committed
    pub legal_move: MoveId,      // which PlacementSet candidate this was recognised as
}

/// The staged sequence for one subject, open against one loaded history,
/// validated against ONE pinned kit for the whole session (KIT-10: session = kit ⊕ snapshot).
pub struct KycWorkbook {
    pub subject: SubjectId,
    pub committed: Vec<IntentEvent>,   // loaded at open (§7) — never mutated in-session
    pub staged: Vec<StagedMove>,       // append-only until commit or discard
    pub kit: LexiconManifest,          // captured at open — ALL in-session recognition and
    pub kit_hash: String,              //   validation use this pin, never a per-call parameter
}
```

`KycWorkbook` itself holds **no DB handle and no `TransactionScope`** — opening, staging and validating are pure operations over `committed`/`staged` plus a `LexiconManifest`; only `commit()` (§5) needs a scope, and it takes one as a parameter rather than owning one. This keeps `preview_is_pure`-style testability for the workbook's own validate loop, matching T3's own purity discipline one layer up.

---

## §5 The session model — open / stage / validate / commit

Mapped onto §6 of the Construction-Kit V&S, made concrete:

**Open.**
```rust
async fn open_workbook(conn: &mut PgConnection, subject: SubjectId) -> Result<KycWorkbook, StoreError> {
    let committed = PgKycEventStore::load_events(conn, subject).await?; // existing, T1-era
    let (kit, kit_hash) = load_current_lexicon_manifest(conn).await?;   // KIT-10: pin the kit to the session
    Ok(KycWorkbook { subject, committed, staged: vec![], kit, kit_hash })
}
```
New-UBO empty-baseplate path = `load_events` returns `[]` (already the natural empty case — `fold_control(&[])` is `ControlState::default()`, exercised by T2/T3's `empty_state()` tests already). **No new "create baseplate" verb or table** — an empty committed history already *is* the empty baseplate; nothing to special-case.

**Stage.** Recognise typed DSL text against the **frontier** placement set (§6 step 3), producing a `StagedMove`, appended to `workbook.staged`. Pure; no DB.

**Validate (the Repl's job — re-run-whole).**
```rust
fn validate(workbook: &KycWorkbook) -> Result<ControlState, KycError> {
    let candidates: Vec<IntentEvent> = workbook.staged.iter().map(|m| m.event.clone()).collect();
    preview(&workbook.committed, &candidates, &workbook.kit)   // T3 verbatim, against the session's PINNED kit (KIT-10)
}
```
Called after every stage (not just before commit) — "the Repl continuously re-runs the whole workbook" (§6 of the V&S). Because T3 is already `O(staged-length × fold-cost)` and sessions are "tens of moves" (KIT-4's own sizing claim, already load-bearing for T2's §14 Q2 benchmark), re-validating on every keystroke-equivalent is cheap by the same argument that justified re-run-whole reconstruction over incremental patching.

**Commit — replay through the real governed append, verb by verb, not a bespoke append.**
```rust
async fn commit(workbook: KycWorkbook, scope: &mut dyn TransactionScope, registry: &FoldRegistry) -> Result<Vec<AppendOutcome>, StoreError> {
    // KIT-10 kit-drift check: the session validated against a pinned kit. If the live
    // manifest has moved, fail with a NAMED kit-drift error — re-validating against the
    // new kit is an explicit user action (re-open), never a silent substitution.
    let live_hash = load_current_lexicon_manifest_hash(scope.executor()).await?;
    if live_hash != workbook.kit_hash {
        return Err(StoreError::KitDrift { pinned: workbook.kit_hash.clone(), live: live_hash });
    }
    // Re-validate immediately before commit against the scope's own connection —
    // the workbook may have gone stale since the last `validate()` call (another
    // session wrote to this subject meanwhile). This is NOT redundant with the
    // per-event re-fold `append_in_scope` already does internally (K-14) — this
    // is a whole-chain fail-fast so a workbook that's gone stale rejects with
    // ONE clear error, instead of committing its first N legal moves and only
    // then hitting the (n+1)th's now-stale precondition failure mid-transaction.
    let fresh_committed = PgKycEventStore::load_events(scope.executor(), workbook.subject).await?;
    preview(&fresh_committed, &workbook.staged.iter().map(|m| m.event.clone()).collect::<Vec<_>>(), &workbook.kit)
        .map_err(StoreError::from)?;

    let mut outcomes = Vec::with_capacity(workbook.staged.len());
    for staged in &workbook.staged {
        let outcome = append_in_scope(
            scope, registry, &staged.event, &staged.source_text,
            |state| {
                let entry = workbook.kit.get(staged.event.verb_fqn.as_str())
                    .ok_or_else(|| KycError::UnknownVerb(staged.event.verb_fqn.clone()))?; // never panic in the governed write path
                check_control_preconditions(entry, state, &staged.event)
            },
        ).await?;
        outcomes.push(outcome);
    }
    Ok(outcomes)
}
```
This is **the exact same `append_in_scope` chokepoint real `dsl.kyc` verb dispatch uses** (`kyc_stream_ops.rs`) — commit is not a new write path, it is the workbook driving N ordinary governed verb appends in staged order, inside one transaction. T1's source-text capture is already a parameter of `append`; nothing new to wire. If any staged move fails its per-placement re-check (K-14, inside `append_in_scope`) — which the whole-chain `preview` re-check above should already have caught, so this would only fire under a race the fail-fast didn't close in time — the transaction rolls back and **nothing partial lands**, satisfying `illegal_mid_chain_rejected`'s spirit at the commit boundary too, not just T3's in-memory one.

**"Snapshot refresh" — ruled out as a new persistence surface.** Searched the tree: no session-snapshot cache table exists today. KIT-4 (re-run-whole, "trivial for a nom-class constructor" at session scale) and KIT-10 ("snapshot is a regenerable checkpoint, **never authoritative**") together mean T4 does not need to build one — "refresh" is simply: the *next* `open_workbook` call re-runs `load_events` + fold, which is already correct and already cheap. Building a snapshot cache now would be optimizing a cost nobody has measured as a problem, for a concept the V&S itself says is allowed to not exist. If real usage later shows `load_events` + full fold getting expensive (hundreds of moves per subject, not "tens"), that's a follow-up perf tranche with its own benchmark gate — not T4. *(Plan-wording reconciliation: EOP-PLAN v0.3 §T4's "commit = replay + capture + refresh" is satisfied by recompute-at-next-open — the snapshot IS the re-fold, a regenerable identity checkpoint per KIT-10; recorded here so the plan's wording doesn't read as an unbuilt promise.)*

---

## §6 Recognition — tier-0 deterministic matching, concretely

The super user types DSL text close to the real grammar (V&S §2a's own framing — "their utterances are close to the language itself"). Recognition:

1. Parse with the real workspace parser: `dsl_parser::parse(text) -> (SourceFile, diagnostics)`. Any parse diagnostic → reject with the diagnostic, verbatim. This is the identical parser T1's `sexpr_roundtrip_property` already uses as its oracle — not a new grammar, not a subset, not a "close enough" fuzzy layer.
2. Map the parsed `RawAtom.kind` (verb FQN) + its slots (target/payload fields) to a candidate `IntentEvent` — using the *same* field-shape T1's renderer proves round-trips (`parse(render(event)) ≡ event`), run in reverse. This needs one new pure function, the render's mirror: `sexpr_to_intent_event_draft(atom, subject, as_of) -> Result<IntentEventDraft, RecognitionError>` — genuinely new code (T1 only built the forward direction), scoped narrowly: it fills an `IntentEventDraft`/payload from slots, it does not validate legality (that's step 3).
3. Check `(verb_fqn, target)` against the **frontier placement set**: `enumerate_placement_set` evaluated over `preview(committed, staged[0..n])` — the state *after* every already-staged move, not committed-only state. This is what lets the canonical dependent chain (`assert-control → attach-evidence → verify`) stage move by move: against committed-only state, moves 2 and 3 would be wrongly refused because their predecessors haven't landed yet. (Empty `staged` ⇒ the frontier *is* the committed state — no special case.) Not a member → reject with "not currently legal" (and, since the frontier `PlacementSet` is already computed, *which* moves currently are legal — a listing, not a guess).
4. Admitted → wrap as `StagedMove`, with `source_text` = **the canonical resolved re-render** (`render_intent_event_to_sexpr` applied to the recognised event). **Ruled in review (was open Q2): T0.2 already decides this** — the ratified rule stores the *resolved* S-expression, and typed text may contain pre-resolution handles that resolution rewrites; so the stored form is the canonical render, and the verbatim typed text is ephemeral display only. `session_roundtrip` asserts the canonical form.

**No fuzzy matching, no Levenshtein-nearest, no embedding search, no LLM call anywhere in steps 1–4.** Step 1's failure mode is a parser diagnostic (deterministic, already gate-tested by `dsl-parser`'s own suite); step 3's failure mode is set non-membership (deterministic, `PlacementSet::admits` is a plain equality scan). This is what makes `zero_inference_assertion` (§8) mechanically checkable rather than a claim to trust.

---

## §7 What T4 does *not* build

- No new snapshot/cache table (§5).
- No new append mechanism (§5 — commit reuses `append_in_scope` verbatim).
- No `ValidVerbSetEngine` implementation (§2).
- No ranking, scoring, or "best match" recognition (§1, §6).
- No natural-language input surface — that is T7, charter-gated, behind the SLM ramp. A super user who types plain English at T4's parser simply gets parse diagnostics; that is correct behaviour for this tranche, not a bug to smooth over.
- No closing of K-G6 (10 of 22 `dsl.kyc` verbs still undeclared in `phase1_lexicon()`). T4's placement-set is exactly as complete as the lexicon real dispatch already validates against — confirmed by inspection: `kyc_stream_ops.rs` calls `phase1_lexicon()` too, so T4 is not narrower than production, it shares the identical live scope boundary and will widen automatically whenever K-G6 closes, with no T4-side change required.

---

## §8 Gate tests, made concrete (RED first)

Per the plan's four names, with the actual assertion each should make:

- **`session_roundtrip`** — `open_workbook` on an existing subject with N committed events → stage 3 recognised moves **forming a dependent chain (`assert-control → attach-evidence → verify` — each legal only after its predecessor)** (via §6, from literal DSL text) → `validate()` returns `Ok` → `commit()` → re-`open_workbook` the same subject and assert the freshly loaded+folded `ControlState` is bit-identical to the workbook's last `validate()` output before commit. Proves KIT-4 (re-run-whole after commit reproduces the pre-commit preview) end to end, not just at the T3 layer — AND proves the frontier recognition (§6 step 3): a committed-only recognition frontier would refuse moves 2–3 of this fixture, while three *independent* moves would false-green that defect. The dependent chain is mandatory, not a flavour choice.
- **`new_ubo_from_baseplate`** — `open_workbook` on a subject with zero committed events (never appended to) → assert `workbook.committed.is_empty()` and `validate()` on an empty `staged` returns `ControlState::default()` → stage+commit a `kyc.subject.register` move → re-open → assert `state.registered == true`. Proves the empty-baseplate path needs no special-casing (§5).
- **`invalid_workbook_blocks_commit`** — stage 3 moves where move 2 is illegal (mirrors T3's `illegal_mid_chain_rejected` fixture) → `validate()` returns `Err` before any commit attempt is even possible through the intended flow → additionally, directly call `commit()` on the invalid workbook (simulating a caller that skipped `validate()`) and assert it *also* rejects (the fail-fast re-check in §5's `commit`) with zero rows appended for subject in the DB — a real integration test against a transaction that gets rolled back and inspected.
- **`zero_inference_assertion`** — a **source-scanning invariant test** (same family as `runbook::invariant_tests`, `PACK001`) over `kyc_workbook.rs` (or wherever §3 lands), as an **import ALLOWLIST, not a keyword denylist**: `include_str!` the module, parse its `use` statements, and assert every import resolves into a closed allowlist — `std`, `ob_poc_kyc_substrate`, `dsl_parser`, `ob_poc_kyc_seam`, the `dsl-runtime` scope/transaction types, plus error/serde/test-support crates (exact list fixed at implementation time and committed WITH the test). An allowlist **fails closed** on any new import; a keyword denylist rots as client paths change and misses helper-module indirection entirely. Any new sibling helper module the workbook calls into joins the scanned set (the test asserts the module list it covers, so an escapee is a visible diff, not a silent hole). Plus the `cargo tree`-style feature check as before (mirroring `check_kyc_substrate_deps.sh`'s shape) confirming the functions build without any model-client feature enabled, where such a gate exists; if it doesn't, the allowlist scan carries the gate, documented as such.
- **`stale_snapshot_recovers`** — open a workbook, stage a legal move, then (in a second concurrent connection) commit a *different* legal move to the same subject that changes the folded state — then commit the first workbook and assert it re-validates against the *true current* state (§5's fail-fast) rather than the state it was opened against, either succeeding (if still legal against the new state) or failing with a clear staleness-attributable error (if not) — never silently committing against stale preconditions. This is the one gate test that needs a real two-connection integration harness, not just in-memory fixtures.

---

## §9 Open questions — resolved in review (v0.2)

1. **Module vs crate placement (§3)** — **RESOLVED: module accepted** (`rust/src/domain_ops/kyc_workbook.rs`), *conditional on* the §8 import-allowlist gate carrying the zero-inference guarantee at the module boundary. Pure-core migration into the substrate remains a later option if the workbook grows; not now.
2. **`source_text` canonicalisation (§6 step 4)** — **RESOLVED: canonical resolved re-render, and T0.2 already decides it.** The ratified rule stores the *resolved* S-expression; typed text may contain pre-resolution handles that resolution rewrites, so the stored form is `render_intent_event_to_sexpr` re-applied post-resolution. Verbatim typed text is ephemeral display only. `session_roundtrip` asserts the canonical form.
3. **`zero_inference_assertion` list (§8)** — **SUPERSEDED** by the import allowlist: no denylist to curate; the allowlist is fixed at implementation time and committed with the test, failing closed on anything new.

Everything else in this doc (§1 Q3 ruling, §2 `ValidVerbSetEngine` ruling, §5 commit-reuses-`append_in_scope`, §5 no-new-snapshot-table, §7 scope fence) is a considered recommendation, not a coin-flip — happy to defend any of it, but it's ready for your sign-off as written.

---

## Change Log

| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-12 | Initial draft. Answers §14 Q3 (both aheads, named, recognition-only at baseline) and the `ValidVerbSetEngine` question (retire for the KYC workbook type). Composes T1/T2/T3 without modifying any of them. Three open questions flagged for review (§9); everything else is a ready-to-defend recommendation. |
| 0.2 | 2026-08-12 | Review edits (Opus adversarial pass). **Recognition frontier fixed**: staging move n+1 checks the placement set over `preview(committed, staged[0..n])`, not committed-only — committed-only wrongly refused dependent chains (assert→attach→verify) and its gate fixture would have false-greened on three independent moves. **Kit pinned to the session** (KIT-10): `KycWorkbook` captures manifest+hash at open; `validate()`/`commit()` use the pin; commit fails with a named kit-drift error on live-hash mismatch. `zero_inference_assertion` restated as an import **allowlist** (fails closed; supersedes the denylist). `session_roundtrip` fixture mandated as a dependent chain. Commit-path `.expect` replaced with a typed error (never panic in the governed write path). §1 wording de-muddied (both affordances exist deterministically at baseline; *ranking* is what's absent). §9 resolved: module placement accepted conditional on the allowlist gate; `source_text` = canonical resolved render per T0.2. Plan-wording reconciliation recorded in §5 (snapshot refresh = recompute-at-next-open). |
