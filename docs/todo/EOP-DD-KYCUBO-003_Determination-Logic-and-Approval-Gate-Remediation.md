# Determination Logic & Approval Gate Remediation

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-003 |
| **Type** | Remediation plan (no implementation authorised) |
| **Version** | 0.1 — Draft for Adam review |
| **Owner** | Adam Cearns |
| **Author** | Claude, grounded in direct re-read of `kyc_stream_ops.rs`, `fold/control.rs`, `fold/obligation.rs`, `lexicon.rs`, `manifest.rs`, `kyc_verb_coverage.rs` on 2026-07-01 |
| **Date** | 2026-07-01 |
| **Binds to** | EOP-VS-KYCUBO-001 v0.6 *From Percentage to Determination*; supersedes/updates `kyc-ubo-determination-gap-report-and-refactor-plan-v0_1.md` (pre-W1–W7, now stale) for the items below |
| **Status** | **M1 + M3 EXECUTED** 2026-07-01 (approved by Adam, "approved get started"). M2 and M4 remain plan-only — not authorised. |

---

## 0. Executive Summary

The 2026-06-30 W1–W7 build (branch `codex/phase-1-5-governance-closure`) landed real event-sourcing infrastructure: the append-only verb stream, per-subject locking, the content-addressed lexicon manifest (for 12 of 23 verbs), disposable projections, cross-stream B2/B3 obligation emission, and supersede-never-delete edge topology. That work was reported as closing the V&S.

A fresh, direct re-read of the production code on 2026-07-01 — not a re-run of the existing test suite, an actual line-by-line read of the ops — found that the document's central thesis is not wired into the write path: **`ubo.determination.freeze` does not call `OwnershipProngStrategy`**, and **`kyc.person.approve` has no K-23 gate**. A silent payload-key bug was also found that makes `structure_class` always `None` in production. None of this was caught by the "100% coverage" suite because that suite asserts dispatch (an event was appended), not correctness (the resolved persons match a regulated determination).

**Verdict:** infrastructure-complete, determination-logic-incomplete. This plan proposes closing that specific gap — it does not reopen or expand W1–W3/W6, which are confirmed solid.

---

## 1. Confirmed gaps (evidence-grounded)

| ID | Gap | Evidence | Invariant / criterion | Severity |
|---|---|---|---|---|
| **R1** | `ubo.determination.freeze` does not call `OwnershipProngStrategy::resolve()`. It takes every distinct source-entity across *all* active edges (economic and control mixed, any %, single-hop only) as the "resolved" set. The real strategy exists only in `kyc_w7_oracle.rs`, an isolated test the verb never invokes. | `rust/src/domain_ops/kyc_stream_ops.rs:318-329` (comment: *"A full strategy run is out of scope here; we use the fold's natural-person edges as a proxy"*) | K-1, K-2, K-3, K-4, K-6; Success Criteria 1, 2, 3 | **CRITICAL** |
| **R2** | `kyc.person.approve` appends its event unconditionally. It never reads `SubjectOverallState::AllTerminal` from the obligation fold before allowing approval. | `rust/src/domain_ops/kyc_stream_ops.rs:525-535` | K-23; Success Criteria 7, 11 | **CRITICAL** |
| **R3** | `kyc.subject.classify-structure` passes its arg through as `"structure-class"` (kebab), but the fold reads `payload.get("structure_class")` (snake). Never normalized like the obligation verbs were. Result: `ControlState.structure_class` is silently always `None` in production. Not caught because the coverage test only asserts the event was appended, not that the fold recorded the class. | `rust/config/verbs/kyc/dsl-kyc.yaml:615` vs `rust/crates/ob-poc-kyc-substrate/src/fold/control.rs:251-252`; test gap at `rust/tests/kyc_verb_coverage.rs:267-276` | K-4 | **HIGH** (silent correctness bug) |
| **R4** | The 11 obligation-family verbs (`kyc.role.*`, `kyc.obligation.*`, `kyc.person.approve/reject`) are not in the substrate's `phase1_lexicon()` and are not published by `manifest.rs`. They carry no substrate-level precondition enforcement and are not part of the content-addressed, replay-pinned lexicon — only the 12 W1/W4 determination verbs are. | `rust/crates/ob-poc-kyc-substrate/src/lexicon.rs:194-298` (only 12 entries); `rust/crates/ob-poc-kyc-store/src/manifest.rs:35` (`phase1_lexicon()` is the only thing published) | K-30, K-31 | **MEDIUM** |
| **R5** | Verb families in V&S Appendix A that have zero code: `ubo.edge.pierce-nominee` / `.dispute` (no `Disputed` `EdgeStatus` variant either), `ubo.determination.record-basis` / `.reopen` / `.waive-with-authority`, `kyc.role.verify`, `kyc.subject.link-to-cbu-role` / `.assert-jurisdiction` / `.assert-regulatory-status`, `kyc.obligation.defer` / `.expire` / `.reopen`, and the entire `kyc.entity.*` institutional-KYC-profile family. | Grep-confirmed absent across `rust/config/verbs/`, `rust/src/domain_ops/kyc_stream_ops.rs`, `EdgeStatus` enum at `fold/control.rs:65-74` | K-8, K-28 (not covered by any deferred phase); the rest are Appendix A Phase-4 items | **LOW–MEDIUM** (mixed — see §2 Phase M4) |
| **R6** | No threshold is applied at all in the production freeze path (moot until R1 is fixed), and there is no reference-plane threshold table keyed by jurisdiction/structure-class — `threshold_pct` is only ever a bare function parameter in the substrate. | `rust/crates/ob-poc-kyc-substrate/src/determination.rs:75,98,318` — no caller in `kyc_stream_ops.rs` supplies it | K-6 | **MEDIUM** (becomes real once R1 lands) |
| **R7** | The "100% coverage" claim covers verb dispatch, not regulated correctness. No test currently proves Success Criteria 1–3 or 7 against the production path; `coverage_ubo_determination_freeze` freezes zero asserted edges and only checks the event exists. | `rust/tests/kyc_verb_coverage.rs:233-245` | Test-debt shadowing R1/R2 | **MEDIUM** |

---

## 2. Remediation plan

### Phase M3 — RED tests first (write before touching any op)

Each test should fail against current `main`/branch state today, proving the gap, then pass once the corresponding M1 fix lands.

- **M3.1** — Differential test: freeze a private-company fixture (mirroring `kyc_w7_oracle.rs`'s edges, through the real verb path) and assert the resolved-persons set + basis matches `OwnershipProngStrategy::resolve()` directly. RED today (freeze's proxy set will differ once axes/thresholds are mixed in a fixture where they diverge).
- **M3.2** — K-23 gate test: call `kyc.person.approve` on a subject with an obligation still `in_progress`; assert rejection. RED today (currently succeeds).
- **M3.3** — `structure_class` round-trip test: call `classify-structure`, fold the stream, assert `ControlState.structure_class == Some(StructureClass::PrivateCompany)`. RED today (currently `None`).
- **M3.4** — Fund-LP/LLP fixture test proving Success Criterion 2 (control prong surfaces the GP/manager, not passive LPs) through the real freeze path. Does not exist in any form today.

### Phase M1 — Critical wiring (blocked on M3 existing as RED)

- **M1.1** Fix the `structure-class` / `structure_class` payload-key mismatch (R3) — same normalize-on-ingest pattern already used for obligation verbs (`normalize_obligation_payload`); add a matching helper for the classify-structure op, or extend the existing one.
- **M1.2** Wire `OwnershipProngStrategy` into `ubo.determination.freeze` (R1): read `selected_strategy` + `structure_class` off the folded `ControlState`; dispatch to the strategy the structure class selects (today only `ownership_prong_strategy` exists — control-by-other-means and SMO-fallback strategies are separate verbs already, `apply-smo-fallback` — confirm whether freeze should also invoke that fallback automatically or continue to require it as a preceding verb call); compute reconciled economic and control edge sets independently (K-2) before handing them to the strategy; record prong/basis per resolved person on the freeze event payload, not just a bare person-ID list.
- **M1.3** Add the K-23 gate to `kyc.person.approve` (R2): fold obligations for the subject, compute `SubjectOverallState`, reject with a `PreconditionFailed`-style error unless `AllTerminal`. Decide whether this check lives in the substrate (extending `phase1_lexicon()` per R4) or as an explicit check inside the op — recommend the former for consistency with the existing ratchet pattern.
- **M1.4** Threshold sourcing (R6): minimal reference-plane threshold value (even a single config default is fine to start, per-jurisdiction/structure-class table can follow) so `freeze` has a real, non-hardcoded `threshold_pct` to pass into the strategy call from M1.2.

### Phase M2 — Governance completeness

- **M2.1** Extend `phase1_lexicon()` / republish the manifest to cover the 11 obligation-family verbs (R4): governing taxonomy, writes-fold, authority, preconditions (including the K-23 gate from M1.3) for `kyc.role.*`, `kyc.obligation.*`, `kyc.person.approve/reject`.
- **M2.2** Add `ubo.edge.pierce-nominee` (K-8): new verb + fold rule forbidding a determination from terminating at a `Nominee`-classed edge; must resolve through to the beneficial controller.
- **M2.3** Decide scope for `ubo.edge.dispute` / `Disputed` edge status — currently absent entirely (no enum variant). Flag as an open question rather than assuming it's needed; V&S §15 doesn't force it.

### Phase M4 — Explicitly deferred pending your decision

- **`kyc.entity.*` institutional KYC profile family (K-28)** — this is a net-new capability (profile model + ~7-9 verbs), not a quick fix. Recommend scoping as its own workstream rather than folding into this remediation.
- **`kyc.subject.link-to-cbu-role` / `.assert-jurisdiction` / `.assert-regulatory-status`, `kyc.role.verify`, `kyc.obligation.defer` / `.expire` / `.reopen`, `ubo.determination.record-basis` / `.reopen` / `.waive-with-authority`** — all Appendix A Phase-4 items per the V&S's own phasing table. Propose leaving deferred unless you want them pulled forward now.

---

## 3. Sequencing recommendation

M3 (RED tests) → M1 (turn RED green — the critical wiring) → M2 (governance completeness) → M4 as a separate, later decision. Nothing in M4 blocks M1–M3.

---

## 4. Explicitly not touched by this plan

W1 (verb stream substrate), W2 mechanics for the 12 already-covered verbs, W3 role-basis recording, W6 projections/drainers, and the B2/B3 cross-stream emission logic — all re-confirmed solid during this review and out of scope for remediation.

---

## Sign-off

**Status: DRAFT FOR REVIEW.** No code, schema, verb YAML, or lexicon changes are authorised against this plan until you approve it or redirect scope.

---

## Execution log (2026-07-01)

**M3 (RED tests) + M1 (critical wiring) executed together**, one commit-ready slice, not yet committed. `rust/tests/kyc_m3_remediation.rs` (4 tests) proven RED against pre-fix code via `git stash`, then GREEN after the fix — both states captured below.

- **M1.1** — fixed via `normalize_register_payload`/`normalize_classify_structure_payload` in `kyc_stream_ops.rs`: both now stamp `entity_id` (defaulting to `subject-id`) and `classify-structure` renames kebab `structure-class` → snake `structure_class`. Added optional `entity-id` arg to `subject.register`/`subject.classify-structure` YAML so one determination stream can register the intermediate entities/persons in its own ownership chain, matching the substrate's own `kyc_slice.rs` fixture shape.
- **M1.2** — `ubo.determination.freeze` now: reads `control.selected_strategy` and dispatches to a real `DeterminationStrategy` (only `ownership_prong_strategy` exists; any other name fails loudly rather than silently defaulting — new `m3_4` test locks this in); resolves `subject_entity_id` via the newly-`pub` `find_subject_entity` (shared with `recover_determination_at`); computes `natural_persons_from_events` + `reconciled_economic_edges`; applies `threshold_pct` (arg, default 25.0 — M1.4's interim); enforces K-5 (errors if silent); records `candidates`/`smo_result`/`strategy`/`threshold_pct` on the freeze event payload (K-1/K-35 — the event is now the audit record, not a bare marker) and returns them in the verb outcome.
- **Also found and fixed while wiring M1.2**: `stream_append`'s `validate_entry_fqn` for freeze was `None`, so the lexicon's declared `ReconciledProjection`/`StrategySelected` preconditions were dead code — freeze never actually checked K-14 before this fix. Changed to `Some("ubo.determination.freeze")`.
- **M1.3** — `kyc.person.approve` now folds obligations pre-append and rejects with a `K-23`-tagged error unless `SubjectOverallState::AllTerminal`.
- **M1.4** — `determination.freeze` gained an optional `threshold-pct` YAML arg (decimal), default 25.0 in the op. A governed per-jurisdiction reference-plane table is still open (not built — flagged as-is, not silently expanded).
- **Regression found and fixed**: the pre-existing `coverage_ubo_determination_freeze` test in `kyc_verb_coverage.rs` used strategy string `"ownership_prong"` (not the canonical `"ownership_prong_strategy"`) and asserted no edges/classify/reconcile — it only passed before because freeze ignored the strategy value entirely. Updated to use the canonical name, added `classify-structure` + `reconcile-conflict` + an SMO fallback (no qualifying edges in this smoke test, so K-5 requires SMO to avoid a silent determination).
- **Test evidence**: `kyc_m3_remediation.rs` 4/4 RED before fix (`git stash` of the 4 changed files) → 4/4 GREEN after (`git stash pop`). Full re-run of the whole kyc/ubo surface after the fix: `ob-poc-kyc-substrate`/`ob-poc-kyc-store`/`ob-poc-kyc-seam` crate tests (kyc_slice 19/19, append_protocol 6/6, drainer 1/1, exit_criteria 4/4, manifest 1/1, projection 3/3, recovery 2/2, seam 3/3) all green; `rust/tests/kyc_*.rs` (stream_ops, verb_coverage 17/17, w3_w5_w6, w7_oracle, m3_remediation 4/4) all green; `test_plugin_verb_coverage` + `test_no_rust_only_verbs_in_registry` green; `check_kyc_substrate_deps.sh` PASS; `cargo x verbs compile` (Updated: 3, matching the 3 touched verbs) + `cargo x verbs check` (1282/1282 up-to-date, 0 mismatch); `cargo test -p ob-poc --lib` 2260 passed / 1 pre-existing unrelated failure (`constellation_map_tests::loads_all_constellation_map_yamls` — references deleted legacy verb `control.add`, pre-dates this session per prior-session memory) / 149 ignored.
- **Pre-existing, out-of-scope issue surfaced by `cargo x verbs lint`** (not touched — flagging only): pack `kyc-case` still references two deleted legacy verbs, `ubo.list-controllers` and `ownership.compute` — stale pack-reference cleanup left over from the earlier W1–W7 rip-and-replace, not caused by this remediation.
- **Not done**: M2 (lexicon-manifest extension to the 11 obligation verbs, `pierce-nominee`) and M4 (institutional KYC profile, Phase-4 verbs, a real control-prong strategy for fund-LP/LLP — Success Criterion 2 is still open) remain as planned, unauthorised.
- **Nothing committed** — all changes are in the working tree on `codex/phase-1-5-governance-closure`, pending your review.
