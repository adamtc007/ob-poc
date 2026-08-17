# EOP-PLAN-KYCUBO-KIT-001 — Implementing the UBO Construction Kit
### v0.6 — The build-out plan (T4.5 → TS), grounded in the 2026-08-12 state-of-tree reads

| | |
|---|---|
| **Document** | EOP-PLAN-KYCUBO-KIT-001 |
| **Version** | 0.6 — build-out phase; supersedes v0.5's forward sections, preserves its record |
| **Owner** | Adam Cearns |
| **Binds to** | EOP-VS-KYCUBO-KIT-001 v0.2; EOP-SA-OBP-001 v0.6; EOP-VS-KYCUBO-001 v0.6; EOP-VS-BPMN-DESIGN-003 (all in docs/eop/) |
| **Grounded in** | v0.5's execution receipts (T0–T4 done, verified live-DB) **plus direct state-of-tree reads 2026-08-12**: Precondition enum + checker (`lexicon.rs:50`, `control.rs:400`), ControlState/ObligationState (`control.rs:147`, `obligation.rs:130`), KycWorkbook wiring sweep (module-only, zero surfaces). R4/R6/R7 research PENDING — Appendix A. |
| **Status** | T4.5/T5/T6.1 build-ready now. T6.2–6.4 gated on the stud-geometry matrix ratification (EOP-DD-KYCUBO-KIT-T6_Stud-Geometry-Disposition-Matrix). TS.0–TS.4 gated on R4 + per-class design. T7 gated on R7 + charter. |

> **History:** T0 (ratifications), T1 (source of truth), T2 (placement-set), T3 (preview), T4 (zero-inference IDE session, 5/5 live-DB) are **DONE** — see v0.5 for the full record and receipts. This document plans only what remains. Discipline unchanged: RED-first gates per tranche; dep-gate stays green; rulings are ratified before encoding, never improvised mid-build.

---

## Key state-of-tree facts this plan stands on (cited; verified 2026-08-12)

1. **`Precondition` has 3 niladic variants; the only checker takes `&ControlState`** and is called at both append sites (`kyc_stream_ops.rs:79,142`). There is **no obligation-side evaluation path** — `ObligationState` is invisible to preconditions today.
2. **Nothing on the stud list needs new fold output.** `ControlState` already exposes `registered`, `structure_class`, the full edges map (superseded included), reconciliation + strategy markers; `ObligationState` exposes per-track state and `SubjectRollup.overall_state` (Approved/Rejected/AllTerminal) with `derive_subject_state()` ready-made.
3. **Preconditions ride inside `LexiconEntry` (serde) and the manifest is content-addressed** — authoring a stud changes the kit hash, so open T4 sessions correctly KitDrift at commit. Operational rule: studs land in **batches**; sessions re-open. This is the system working, not a bug.
4. **`KycWorkbook` is referenced in exactly one file — its own module.** No bin, no CLI, no MCP, no Repl surface knows it exists. The engine is proven; there is no cockpit.
5. **`check_control_preconditions` only runs when a `LexiconEntry` exists** — T6.0's entry closure was the structural prerequisite for any stud on the obligation/person families.

---

## T4.5 — Session surface wiring (the cockpit; confirmed needed by fact 4) — **SCOPE RATIFIED 2026-08-12**

**Ruled:** surface = **the Repl session machinery** (Q3); **exact-match handle resolver joins T4.5 scope** (the T4-deferred option (b) — `@handle`/entity-name → UUID by key lookup, fails closed on unknown AND ambiguous, zero fuzziness — pulled forward because the ruled interaction is "dictating verbs + entity-resolution values", and nobody dictates UUIDs); **dual execution semantics** (Q4: both, distinctly named): **`commit`** = atomic all-or-nothing exactly as built (the T4 guarantee: committed state ≡ approved preview, zero partial), and **`run`** = sequential per-verb execution, stop at first failure, previously-executed prefix STANDS, no rollback (legitimate because every prefix was itself validated during re-run-whole staging — a partial run lands a state that WAS previewed, just not the final one; mechanically a loop of single-move commits, each a real governed append).
**Scope:** wire the T4 workbook into the Repl session: open / stage (typed or dictated DSL with handle resolution at stage time; frontier placement listing on rejection) / validate (consequence preview rendered from `ControlState`) / show / `commit` / `run` / discard. Resolution happens BEFORE recognition; the canonical resolved render is what's stored (T0.2 satisfied by construction). The workbook module is the API — additive changes only, receipted.
**Gate tests (RED first):** `surface_session_end_to_end` (scripted full session, live DB); `surface_rejection_shows_placement_listing`; `surface_never_bypasses_workbook` (only write routes are `commit`/`run`); resolver: `resolves_exact` / `rejects_unknown` / `rejects_ambiguous` / `resolved_canonical_stored`; run-mode: `run_stops_at_first_failure_prefix_stands` / `run_prefix_state_matches_intermediate_preview`; the atomic `commit` gates re-run unchanged.
**Fences:** no model imports (the T4 allowlist extends to all surface glue); no fuzzy matching anywhere in resolution; if no existing entity-name/alias lookup source exists in the tree, HALT and report options — never invent a table unilaterally.
**Routing:** Sonnet.

## T5 — Bitemporal recovery (unchanged from v0.4/v0.5; start any time)

**Scope:** `recover_determination_bitemporal(subject, valid_at, known_at)`; constructor takes the axis parameter (EOP-SA-OBP-001 §I.5). Pin-set completion facts pending R6 (Appendix A) — the function lands regardless; R6 decides only whether freeze's pin record needs extending.
**Gate tests (RED first):** `axes_diverge_on_correction`; `bitemporal_matches_txtime_when_axes_align`.
**Routing:** Sonnet — both timestamps already on every event; predicate + plumbing.

## T6.1 — Precondition machinery (reshaped by facts 1–2: checker plumbing, not fold work)

**Scope, three parts, one tranche:**
- **(a) Unified checker:** widen to `check_preconditions(entry, &ControlState, &ObligationState, event)` — ONE function seeing both folds (cross-fold studs like "obligation.create requires a registered subject" need both in scope; a parallel second checker would fork the discipline). Both append sites in `kyc_stream_ops.rs` updated; T2 placement-set and T3/T4 preview/validate updated to thread `ObligationState` (they currently fold control only — this is the tranche's real blast radius; enumerate it in recon).
- **(b) New variants** per the ratified matrix (EOP-DD-KYCUBO-KIT-T6 §2 — the variant inventory). All evaluable against existing state (fact 2); most are niladic; `NoDuplicateActiveEdge`/`EdgeExists`-family read the event target the same way `EvidenceCited` already does.
- **(c) The shipped exemplar — the fail-closed strategy guard:** `StructureClassSupported` on `select-strategy` AND `freeze`: `structure_class ∈ {classes with an implemented strategy}` (set pinned by the seventh tooth; today {corporate-ownership-prong classes}). Converts the 6-unimplemented-classes hole from **silently-wrong determination** to **fail-closed error** — the single highest-value stud in the pack, and it ships in this tranche regardless of the rest of the matrix.
**Gate tests (RED first):** the T6 per-stud pair (`precondition_blocks_illegal_placement` / `_admits_legal_placement`) for the exemplar; `checker_sees_both_folds` (an obligation-state-dependent stud provably evaluates); `placement_iff_precondition` property re-run green; seventh tooth updated (conscious pin edit); KitDrift-on-stud-batch demonstrated once (open session + author stud + commit → KitDrift, documented as correct).
**Fences:** exemplar + machinery ONLY — no other matrix rows encoded here (they're T6.2–6.4, post-ratification); fold functions untouched.
**Routing:** Sonnet; the checker-signature blast-radius recon pastes before code.

## T6.2–T6.4 — Precondition authoring per family (gated: matrix ratification)

Each tranche = one family's ratified matrix rows encoded, with the per-stud RED pair, the `placement_iff_precondition` re-run, and the seventh-tooth pin update. **Batch rule (fact 3):** each tranche is one kit-hash change; sessions re-open after it lands.
- **T6.2 — edge family** (`assert-control`, `assert-economic-interest`, `attach-evidence`, `supersede`, `reconcile-conflict`): registration + duplicate-edge + edge-lifecycle studs.
- **T6.3 — determination family** (`select-strategy` beyond the exemplar, `apply-smo-fallback`, `classify-structure`, `register`): ordering studs (classify-before-strategy; register-idempotence). `apply-smo-fallback` likely reuses the two EXISTING variants (ReconciledProjection + StrategySelected) — zero new machinery if the matrix ratifies as drafted.
- **T6.4 — obligation/person family** (`obligation.*` ×6, `person.*` ×2): the cross-fold studs the unified checker exists for — `SubjectRegistered` on create; `ObligationExists` + `SubjectNotDecided` on updates/satisfy/waive; **the K-23 gate**: `SubjectAllTerminal` on `person.approve` (closes the DD-003 finding that approve has no gate — the second known silently-wrong hole after the strategy guard).
**Routing per tranche:** rulings = the ratified matrix (Adam); encode + tests = Sonnet.

## TS.0–TS.4 — Structure-class strategy build-out (semantics of EOP-VS-KYCUBO-001; PENDING R4)

The 6 classes with no strategy (`trust`, `foundation`, `investment_fund`, `state_owned`, `cooperative`, `nominee`) — safe to sequence at design pace once the T6.1 exemplar makes them fail-closed.
- **TS.0 — design doc** (Opus, Adam ratifies): per-class control-basis models + the wire-vocabulary ruling (audit fact: `assert-control` has no `trust_role` value; every trust assertion collapses to `DominantInfluence`). R4 (Appendix A) sizes the `EdgeKind`-extension blast radius first.
- **TS.1 — trust** (hardest: vocabulary + strategy). **TS.2 — investment_fund + foundation.** **TS.3 — state_owned + cooperative.** **TS.4 — nominee = the K-8 `pierce-nominee` verb**: a NEW verb with full kit citizenship (YAML + op + lexicon entry + stud + fold arm + teeth — the audit's inert-Nominee finding resolved by construction, and the reintroduction discipline exercised properly).
Each TS tranche: strategy impl + per-class gate fixtures + the seventh tooth's strategy-count pin consciously updated + `StructureClassSupported`'s set widened (the guard shrinks as reality grows — fail-closed is the ratchet).

## T7 — The plain-English ramp (unchanged structure; PENDING R7 + charter)

T7.0 research spike (folded into Appendix A as R7) → T7.1 charter-gated capture → T7.2 phrase banks → T7.3 SLM ranker per DESIGN-003. Gates as in v0.5. Nothing downstream waits on this.

---

## Dependency graph (build-out)

```
[DONE: T0→T4, see v0.5] ──► T4.5 (cockpit) ──────────────► super user LIVE end-to-end
                        ├─► T5 (bitemporal; R6 informs pin-set only)
                        └─► T6.1 (machinery + fail-closed guard) ──► matrix ratified ──► T6.2 → T6.3 → T6.4
                                                                     TS.0 (R4 + design) ──► TS.1 → TS.2 → TS.3 → TS.4
T7: R7 spike ► charter ► T7.1 → T7.2 → T7.3   (independent tail)
```
Parallel-safe: T4.5 ∥ T5 ∥ T6.1 (disjoint surfaces). T6.2+ and TS.* serialize on their gates, not on each other.

## Appendix A — Remaining research (trimmed; one Sonnet pass; ANALYSIS ONLY, cite path:line)

```
R4 — STRATEGY DISPATCH + WIRE VOCABULARY (shapes TS.0). How freeze/select-strategy pick a
 strategy (arg? derived?) — paste the dispatch site. DeterminationStrategy trait as landed:
 what a new impl must provide. EdgeKind: variants today; EVERY site an added variant touches
 (enum, fold arms, assert-control YAML valid_values, serde wire, teeth). Confirm/refute:
 adding trust_role = enum+YAML+fold+tests only, no schema migration.
R6 — BITEMPORAL BASELINE (shapes T5's pin-set completion only). Every recover_*/snapshot-read
 fn in the kyc crates with its time filter; confirm as_of + committed_at on every event row;
 paste the frozen-determination pin record vs the KIT-6 pin set (viewer/axis/kit-version/
 constructor-version) — name what's missing.
R7 — T7.0 SPIKE. What utterance→verb matching exists in ob-poc TODAY (matcher, phrase table,
 invocation-phrase index, tier-0 machinery, in or out of the sage crates): implemented /
 stub / absent. What bpmn-lite tier-0 machinery is portable vs rewritten per T0.1c
 mirror-not-adopt.
FINAL: per tranche {TS.0-4, T5-pin, T7.1-3} → facts that CONFIRM / RESHAPE / DELETE it.
```

## Out of scope (fenced)
Unchanged from v0.5: the KYC capture charter (own artifact, hard-gates T7.1); bpmn-lite changes; Mode-1 construct migration; SLM thresholds (gate-time, DESIGN-003 pattern).

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1–0.4 | — | Planning era: dossier-grounded plan → zero-inference re-sequencing → T0.3 pack-closure expansion → T0 closed. Full rows in v0.5. |
| 0.5 | 2026-08-12 | T0 EXECUTED (K-G7 retired, universe 20; T6.0 closed with the renderer-correction; seventh tooth; 8 closure tests); T4 verified 5/5 live-DB. Full receipts in the v0.5 file. |
| 0.6 | 2026-08-12 | **The build-out plan**, grounded in direct state-of-tree reads (R2/R3/R5 run by Opus over the tree; facts §"Key state-of-tree facts", cited). **T4.5 confirmed** (workbook is module-only — no surface references it; the IDE has no cockpit). **T6.1 reshaped**: no new fold output needed anywhere on the stud list; the tranche is a unified two-fold checker (`ObligationState` is invisible to preconditions today) + new variants + the **fail-closed strategy guard** shipped as exemplar (silently-wrong → fail-closed for the 6 strategy-less classes). T6.2–6.4 gated on the stud-geometry disposition matrix (EOP-DD-KYCUBO-KIT-T6, issued alongside this plan). Kit-hash ripple named: stud batches KitDrift open sessions by design — batch-then-reopen rule. TS.0–4 structured (trust first, nominee = K-8 pierce-nominee verb with full kit citizenship); pending R4. T5 start-anytime; R6 informs pin-set only. T7 pending R7 + charter. Appendix A carries the trimmed R4/R6/R7 research prompt. |
| 0.6.1 | 2026-08-17 | T6.2–T6.4 confirmed executed in code (17/18 matrix rows landed, undocumented here until now — status line above is stale on this point, left as historical record rather than rewritten). **Row 9 (`kyc.subject.register`) CLOSED**: `EOP-DD-KYCUBO-KIT-T6`'s §5 (2026-08-14) ruling addressed exact-duplicate idempotency, not the real blocker (a bare per-subject `registered` flag can't distinguish self- from multi-person-candidate registration under one `subject_root`) — see the matrix's own §6 for the correction. Fixed by keying `NotAlreadyRegistered` off `ControlState.registered_entity_ids` (new field) against the event's `entity_id`, preserving the niladic-variant property; also required flipping `kyc_stream_ops.rs`'s `KycSubjectRegister::execute` call site from `validate_entry_fqn: None` to `Some("kyc.subject.register")` — the lexicon/fold fix alone was silently inert without it. All 21 dsl.kyc verbs now carry a stud. |
