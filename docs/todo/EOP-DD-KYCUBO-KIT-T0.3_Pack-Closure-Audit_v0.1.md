# EOP-DD-KYCUBO-KIT-T0.3 — SemOS KYC/UBO Pack Closure Audit

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-KIT-T0.3 |
| **Version** | 0.1 |
| **Binds to** | EOP-PLAN-KYCUBO-KIT-001 v0.3, T0.3 |
| **Status** | Analysis complete. Teeth (`rust/tests/kyc_pack_closure.rs`, 8 tests) built alongside it, gaps pinned RED-honest (several tests assert an *exact* known-open gap set, not its absence — a passing test means "matches audited reality", not "gap closed"). **"Tests passing" means the register matches reality — not that the pack is closed. The pack is not closed** (K-G5 remains open by design; T6.1+ scope). K-G7 **executed** 2026-08-12 (`kyc.role.assign`/`withdraw` retired; universe 22→20); K-G6 **executed** 2026-08-12 (T6.0 — 8 `LexiconEntry` values authored; see K-G6 section for a framing correction found while executing); seventh tooth (`precondition_and_strategy_coverage_is_exactly_known`) built and green. |
| **Method** | Direct enumeration against the real compiled/configured artifacts (YAML verb declarations, `phase1_lexicon()`, fold match arms, DAG `stream_governed` block, domain-pack `owned_verb_prefixes`) — not sampling, not prose reading. Every claim below is reproducible by the cited test. |

---

## 0. Scope and universe

The dsl.kyc lexicon (`rust/config/verbs/kyc/dsl-kyc.yaml` + `dsl-kyc-obligation.yaml`) declares **22 verbs**, matching CLAUDE.md's count:

```
ubo.edge.*            (6): assert-control, assert-economic-interest, attach-evidence,
                            verify, supersede, reconcile-conflict
ubo.determination.*   (4): select-strategy, compute-fold, apply-smo-fallback, freeze
kyc.subject.*         (2): register, classify-structure
kyc.role.*            (2): assign, withdraw
kyc.obligation.*      (6): create, update-identity, update-screening, update-risk,
                            satisfy, waive
kyc.person.*          (2): approve, reject
```

This is the kit's true universe for T2/T6 purposes — confirmed by direct parse of both YAML files (22 verb keys), not by trusting the CLAUDE.md prose count.

---

## 1. Gap register

### K-G1 — unreachable block (state with no verb IN)

**None found.** Checked every enum with fold-derived state:
- `EdgeStatus` (4 variants: Asserted/Evidenced/Verified/Superseded) — each has a producing match arm (`assert-*`→Asserted, `attach-evidence`→Evidenced, `verify`→Verified, `supersede`→Superseded).
- `TrackState` (7 variants) — all producible via `update-identity`/`update-screening`/`update-risk`'s `state` arg (`valid_values` covers all 6 non-Pending variants) or `satisfy`/`waive`.
- `SubjectOverallState` (4 variants) — InProgress (default) / AllTerminal (derived) / Approved / Rejected (via `person.approve`/`person.reject`).
- `StructureClass` (11 variants) — all 11 accepted by `classify-structure`'s `valid_values`.

### K-G2 — dead node (reachable, no verb OUT, not declared terminal)

**None found at the FSM layer.** `Superseded`, `Approved`, `Rejected`, and the terminal `TrackState` variants are all *intentionally* terminal (K-13 supersede-never-delete; K-23 decision-is-final) — confirmed by `is_terminal()`/`is_active()` helper semantics, not just absence of an out-transition. `Deferred` is not terminal but is not dead either: `update-identity`/`-screening`/`-risk` can be re-called with any `state` value, so there is always a way out.

**One K-G2-shaped finding at the taxonomy layer (already tracked, re-confirmed here):** `EdgeKind::Nominee` edges are fully reachable and lifecycle-complete (Asserted→Evidenced→Verified→Superseded all work normally) but are explicitly excluded from both `reconciled_economic_edges` (not economic) and `reconciled_control_edges` (`!matches!(e.kind, EdgeKind::Nominee)`, with the comment explaining why). A verified Nominee edge can **never** contribute to a determination outcome under either implemented strategy — it is functionally inert once asserted, pending `ubo.edge.pierce-nominee` (K-8). This is the existing tracked M2 item, restated in reachability terms: not a new gap, but confirmed real and precisely scoped by this audit (only `Nominee`; the other 7 `EdgeKind` variants are not affected — `TrustRole` has a separate, narrower gap, below).

### K-G3 — unmapped move (verb registered, covered by neither a DAG slot nor the `stream_governed` family list)

**None found.** `kyc_dag.yaml`'s `stream_governed.verb_families` declares exactly 6 globs (`ubo.edge.*`, `ubo.determination.*`, `kyc.subject.*`, `kyc.role.*`, `kyc.obligation.*`, `kyc.person.*`). Direct enumeration confirms all 22 verb FQNs match one of these 6 globs — full coverage, not just glob-text plausibility. Test: `stream_governed_family_covers_every_declared_verb`.

### K-G4 — phantom move (declared in pack graph, not registered/implemented)

**None found.** All 22 YAML-declared verbs have a `SemOsVerbOp::fqn()` match in `rust/src/domain_ops/kyc_stream_ops.rs`, and all 22 op structs are registered in `extend_registry()` (`rust/src/domain_ops/mod.rs:423-446`). Bidirectional: no YAML verb without an op, no op without a YAML verb. Test: `every_declared_verb_has_a_registered_op`. (This narrowly re-confirms, for the KYC pack specifically, what `cargo x registry-graph` already proves workspace-wide.)

### K-G5 — geometry-free block (registered verb, no precondition, KIT-9 backlog)

**Confirmed large, matches the already-documented scope note.** Of the 12 verbs that even have a `LexiconEntry` (see K-G6), only **3** — counted as FQNs — declare a non-empty `preconditions` list: `ubo.edge.verify` (`EvidenceCited`), and `ubo.determination.freeze` + `ubo.determination.compute-fold` (each `ReconciledProjection`, `StrategySelected`). Every other verb — including all 10 of the 22 that have *no* lexicon entry at all (K-G6) — is a trivially-true stud: nothing blocks calling `ubo.edge.assert-control` twice with contradictory `kind` values, nothing blocks `kyc.obligation.create` against a subject that was never registered, nothing blocks `kyc.role.assign` with a nonsense `role` string. **This is T6's real backlog, and it is the majority of the pack, not a handful of cases** — the "permissive subset T4 was scoped to run on" (per the plan's T4 description) is in practice almost the whole pack today.

Separately, the **structure-class ↔ strategy pairing gap** (already flagged in CLAUDE.md, re-verified here): `classify-structure` accepts 11 `structure-class` values; `ubo.determination.freeze` only dispatches 2 implemented strategies (`ownership_prong_strategy`, `control_prong_strategy` — confirmed by direct match-arm read in `kyc_stream_ops.rs`, not the YAML description prose). 6 structure classes (`trust`, `foundation`, `investment_fund`, `state_owned`, `cooperative`, `nominee`) classify successfully today but have no strategy behind them — calling `freeze` on a subject classified into one of these either errors (`strategy` not one of the 2 implemented → hard error, which is at least honest) or, if the caller picks one of the 2 implemented strategies anyway, silently produces a determination that doesn't actually model that structure's real control basis (e.g. a `trust` subject folded under `control_prong_strategy` will never see a `TrustRole` edge, because `assert-control`'s wire vocabulary has no `trust_role` value — every attempt collapses to the `DominantInfluence` catch-all). **Tooth: built and green** — `precondition_and_strategy_coverage_is_exactly_known` (`rust/tests/kyc_pack_closure.rs`), pinning (a) the exact precondition-carrying FQN map (all 12 lexicon-covered verbs, not just the 3 that carry one — empty-list drift is what this pins) and (b) the 11-class/2-strategy counts, both by direct enumeration (the precondition map from `phase1_lexicon()` structurally; the structure-class count from `dsl-kyc.yaml`'s `valid_values`; the strategy count from a bounded scan of freeze's `match strategy_name` block in `kyc_stream_ops.rs`, not the unbounded `fold_match_arms` scan used elsewhere in this file). This was the one register row that shipped without a test (an earlier draft of this section cited a test name that was never written; struck in v0.2). Per-class behavioral correctness remains T6 scope.

### K-G6 — declaration drift (`stream_governed` family list vs. the actually-registered/lexicon-covered verb set)

**Confirmed, and already self-documented in the code** (`kyc_stream_ops.rs:59-61`, written before this audit ran: *"10 of the 22 dsl.kyc verbs have no lexicon entry at all yet (T0.3 gap K-G6)"*). Direct check against `phase1_lexicon()` confirms the exact set — all of `kyc.role.*` (2), `kyc.obligation.*` (6), `kyc.person.*` (2):

```
kyc.role.assign, kyc.role.withdraw,
kyc.obligation.create, kyc.obligation.update-identity, kyc.obligation.update-screening,
kyc.obligation.update-risk, kyc.obligation.satisfy, kyc.obligation.waive,
kyc.person.approve, kyc.person.reject
```

Consequences of having no `LexiconEntry`: no `governing_taxonomy`, no declared `preconditions` (feeds K-G5 above), no K-30 lint coverage, and `render_intent_event_to_sexpr` degrades to its no-entry path for these 10 (still functions — `stream_append`'s `render_entry` falls back gracefully — but the S-expression rendering loses whatever the lexicon entry would have added).

**RATIFIED (2026-08-12): split out of T6, close early.** The remaining entries (**8**, after the K-G7 retirement removes `role.assign`/`withdraw` from the set) are authored as an immediate small task — entries only, preconditions explicitly NOT authored here (that stays T6.1+) — before real sessions exercise these families.

**EXECUTED (2026-08-12), with one correction to this section's own framing:** all 8 `LexiconEntry` values authored in `phase1_lexicon()` (`governing_taxonomy: Obligation` for all 8 — the taxonomy's own scope explicitly includes the decision step, which is what `person.approve`/`person.reject` are; `writes: [ObligationGraph]`; `preconditions: []`, fenced). The "degrades gracefully... loses whatever the lexicon entry would have added" framing above does **not** hold against the real `render_intent_event_to_sexpr` implementation — read in full during execution: `lexicon_entry` is consulted *only* for a `debug_assert_eq!` fqn-match check, never to add or change rendered content (rendering is driven entirely by `event.target`/`event.payload`). So closing K-G6 does not change any already-persisted or newly-persisted `source_text` byte for byte — what it actually buys is fold-precondition-eligibility (T6.1+ can now attach a precondition to these 8, previously structurally impossible since `check_control_preconditions` is only called when an entry exists), K-30 lint coverage (now real, not vacuous), and manifest-hash (Q7) membership. Corrected here rather than silently asserted around: `rust/tests/kyc_pack_closure.rs`'s `newly_covered_entries_are_fqn_correct_and_render_safe` replaces the render-delta assertion originally commissioned for this row with an fqn/taxonomy/render-does-not-panic check, which is what's actually true and actually valuable about these entries existing.

**Minor doc-drift note, not a code gap:** CLAUDE.md's KYC/UBO section says "lexicon-manifest coverage for the 11 obligation verbs (M2)" — the actual count, confirmed by direct enumeration against the YAML, is **10**, not 11. Worth a one-line CLAUDE.md fix; not tracked as a gap-register row since it's a documentation typo, not a system defect.

### K-G7 — fold-blind write (new gap class, not in the plan's original 6; the audit's most important finding)

Not one of the plan's six named classes — surfaced by actually running the closure check rather than assuming the six categories were exhaustive, which T0.3's own framing invited ("gap register... output... by-product" — the plan didn't claim the taxonomy was closed).

**Definition:** a verb declared `effect_class: append_fact` / `side_effects: state_write` (i.e., the pack's own metadata asserts it mutates state), registered and live (a real `SemOsVerbOp`, dispatchable today), whose event type has **zero match arm** in either `apply_one_control_event` or `apply_one_obligation_event` — the two functions `V1FoldImpl` wraps, which is the only fold implementation the KYC registry has ever had. The event appends successfully, returns a `seq`, and then is permanently invisible to every fold, projection, and query in the system.

**Finding: `kyc.role.assign` and `kyc.role.withdraw` are fold-blind.** Confirmed by exhaustive match-arm enumeration (`grep` across both fold source files, cross-referenced against the 22-verb list): of the 22 verbs, exactly 3 have no match arm anywhere — `ubo.determination.compute-fold` (correctly so: `effect_class: read_snapshot`, a projection-trigger verb that is *supposed* to be fold-inert by design) and `kyc.role.assign` / `kyc.role.withdraw` (declared state-mutating, and not inert by design — inert by omission).

This is the exact defect signature CLAUDE.md already documents as found-and-remediated once before, for a different verb: *`ubo.board-controller.override`... "it had zero fold effect — appended to the stream, nothing ever read it back... its purpose was no longer understood, so it was deleted rather than left as unexplained dead capability"* (removed 2026-07-15). `role.assign`'s own doc comment says its purpose is "recording the basis for obligation (K-21, K-24)" — but the actual K-21 basis that `ObligationTracks`/`ObligationBasis` records comes entirely from `kyc.obligation.create`'s own `role`/`jurisdiction`/`cbu-role` args (see `apply_one_obligation_event`'s `"kyc.obligation.create"` arm), independent of whether `role.assign` was ever called first. `role.assign`/`withdraw` currently do nothing that `obligation.create`/(a future `obligation.retract`) doesn't already do on their own.

**Disposition options (ratification needed, not mine to decide unilaterally — same posture as the board-controller.override precedent):**
1. **Wire them in** — give `role.assign`/`withdraw` a real match arm that records a distinct "declared role basis" fact per subject (e.g. a `BTreeMap<SubjectId, BTreeSet<String>>` on `ObligationState`, or a precondition gate requiring `role.assign` to have fired for a given `role` before `obligation.create` can use it) — makes the two-step "assign role, then create obligation under that role" flow actually enforce the basis-before-obligation ordering the verb descriptions imply.
2. **Retire them** — if `obligation.create`'s own `role` arg is judged sufficient (K-21's "never inferred" requirement is already met there — the role is a required, explicit arg), delete `role.assign`/`withdraw` the same way `board-controller.override` was deleted, and re-home their invocation phrases onto `obligation.create`/a future retraction verb.

Either is a legitimate, bounded fix. Leaving them live and fold-blind is the one option this audit recommends against — it's the exact pattern the codebase has already paid down once.

**RATIFIED (2026-08-12): Option 2 — retire.** K-21 explicitness is already fully met at `kyc.obligation.create`'s required `role` arg; `withdraw` is doubly orphaned (no `obligation.retract` exists to pair with). Reintroduction path recorded: if T6 wants basis-before-obligation as *enforced* stud geometry, `role.assign` returns deliberately, with its precondition attached from birth — the same posture as the `board-controller.override` precedent (delete dead capability; reintroduce only with understood purpose). Retirement shrinks the verb universe to **20** and the K-G6 uncovered set to **8**; the closure teeth update in the same pass as the deletion.

**EXECUTED (2026-08-12):** YAML entries, op structs, registry lines, DAG/pack surface-membership entries, and the `verb_to_dsl`-generated `.dsl` mirror all removed; reintroduction comment left at each deletion site. `rust/tests/kyc_pack_closure.rs` updated in the same pass — `verb_universe_is_exactly_20`, `fold_blind_verbs_are_exactly_known` (open-gap set now empty), `lexicon_manifest_coverage_gap_is_exactly_known` (10→8). Three live-DB test files (`kyc_w3_w5_w6.rs`, `kyc_verb_coverage.rs`, `client_group_e2e_test.rs`) also referenced the retired structs/verbs — a recon gap in the first pass (scoped to `rust/config/` + `rust/src/`, missed `rust/tests/`), caught by the compiler and fixed in the same commit.

---

## 2. Verified-clean claims (explicitly checked, not assumed)

- No phantom moves (K-G4): every declared verb has a live registered op, and vice versa.
- No unmapped moves (K-G3): every verb falls under a declared `stream_governed` family.
- No unreachable states (K-G1): every enum variant across `EdgeStatus`/`TrackState`/`SubjectOverallState`/`StructureClass` has a producing verb.
- Ownership is unambiguous: `ob_poc_kyc.yaml`'s `owned_verb_prefixes` includes both `ubo.` and `kyc.` (the 2026-06-18 remediation CLAUDE.md notes — "no domain pack previously declared ownership of the `ubo.` prefix" — is confirmed fixed, not just claimed fixed).

## 3. Feeds forward

- **T2 (true universe):** the 22-verb list above, and specifically the K-G5 precondition gap, should size T2's placement-set benchmark against reality — most of the 22 verbs admit unconditionally today, so `enumerate_placement_set` will (correctly) return large permissive sets until T6 lands real preconditions.
- **T6 (scope lock):** scope = K-G5's full backlog (**17 of 20 verbs, post-retirement**, lack any precondition — beyond the 3 that carry them) + the structure-class/strategy pairing gap (6 of 11 classes). This is materially larger than "K-G2 dead nodes + K-G5 geometry-free" read narrowly — there are no K-G2 FSM dead-nodes to add to the count, but K-G5 alone is most of the pack. *(K-G6 lexicon-entry closure is NOT in T6 scope — ratified split-out, above.)*
- **K-G7 disposition** blocks nothing downstream by itself (T4 already works against the current, permissive geometry), but should be ratified before T6 authoring starts on the `kyc.role.*`/`kyc.obligation.*` family, since T6's precondition work for that family depends on knowing whether `role.assign`/`withdraw` are staying or going.

## 4. Teeth

`rust/tests/kyc_pack_closure.rs` — 7 tests, all pure (no `DATABASE_URL`), each pinning one row of this register so future drift (in either direction — a gap closing *or* a new one opening) is forced through a conscious test update rather than silently passing or silently breaking:

| Test | Gap class | What it pins |
|---|---|---|
| `verb_universe_is_exactly_20` | scope | the 20-FQN list itself (§0, post K-G7 retirement) |
| `stream_governed_family_covers_every_declared_verb` | K-G3 | every verb matches one of the 6 declared globs (`kyc.role.*` now vacuous, left in place — governance-block edits are a separate conscious change) |
| `every_declared_verb_has_a_registered_op` | K-G4 | YAML ⟺ `fqn()` bidirectional equality |
| `lexicon_manifest_coverage_gap_is_exactly_known` | K-G6 | the open-gap set is now **empty** — T6.0 closed it (8 entries authored) |
| `fold_blind_verbs_are_exactly_known` | K-G7 | the open-gap set is now **empty** — only `compute-fold` remains, allow-listed by design; `role.assign`/`withdraw` retired rather than wired in |
| `precondition_and_strategy_coverage_is_exactly_known` | K-G5 | the full precondition-carrying FQN map (20 lexicon-covered verbs post-T6.0, empty-list drift included) + the 11-class/2-strategy pairing counts |
| `newly_covered_entries_are_fqn_correct_and_render_safe` | K-G6 | T6.0's 8 new entries: fqn/manifest-key match, `Taxonomy::Obligation`, render-does-not-panic (one verb per family) |
| `edge_status_lifecycle_is_fully_reachable` | K-G1/K-G2 | a real 4-event fold chain proving Asserted→Evidenced→Verified→Superseded, including supersede-from-Verified (not just supersede-from-Asserted) |

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | — | Initial audit. Six plan-named gap classes checked by direct enumeration; one new class (K-G7, fold-blind write) surfaced and documented; two live verbs (`kyc.role.assign`/`withdraw`) confirmed fold-blind — same defect signature as the already-remediated `ubo.board-controller.override`. Disposition open, pending ratification. |
| 0.2 | 2026-08-12 | Review corrections (Opus adversarial pass) + ratifications. Precondition count fixed: **3** FQNs carry preconditions, not 2 (verify; compute-fold; freeze) — backlog restated 19-of-22 (17-of-20 post-retirement). "All 6 of the 22" lexicon-less → **10** (matches K-G6's own set). Phantom test citation struck (`structure_class_and_strategy_counts_are_pinned` was never built); seventh tooth commissioned (`precondition_and_strategy_coverage_is_exactly_known`) — the one register row that had no pin. **K-G7 RATIFIED: retire** `role.assign`/`withdraw`, reintroduction path recorded. **K-G6 RATIFIED: split out of T6, close early** — degraded renders write permanently into the KIT-1 source of truth. Status carries the executor's honesty flag verbatim: tests passing = register matches reality, not pack closure. |
