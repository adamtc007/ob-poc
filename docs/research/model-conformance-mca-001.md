# MCA-001 — Model Conformance Audit (Scoped Run: Agent Conversational Boundary / Flow-Call-Chain)

**Date:** 2026-07-11
**Auditor scope directive:** "the target is the flow / call chain" — this run executes clause family **AB1–AB7** (the Sage ↔ Control Plane ↔ REPL conversational boundary) plus the L2/C2 touches AB2/AB4 depend on. The full §15 topology census (L1/L3, C1/C3, K1–K3 at crate granularity) was **not** run this pass — out of scope per the narrowing directive, not omitted by oversight. See MCA-6 for the backlog item.

**Arbitration pass (same day):** the architect ruled on all three escalations raised by the first pass. E-2 and E-3 are CLOSED and applied below (AB1's retirement path now explicitly targets mediation per §15.6, not checkpoint-enforce; AB5's verdict flipped MODEL-SILENT → NONCONFORMANT via the newly-ratified readership rule). E-1 (v0.4 ratification) is now CLOSED (2026-07-11, later same day): the architect supplied Amendment 1's draft text; deltas applied mechanically to the repo copy — `docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.md` is now the checked-in precedence-1 document, with new §15 (ratified R-a), inverted §8, and §12 criteria 13-15. AB2's root-cause framing (tier-split, not a missing lock) is now the stated remedy basis for AB2/AB4/AB5 alike.

---

## Precondition failure — recorded before Phase 1, per audit discipline

The audit prompt cites precedence-1 source `EOP-VS-CONTROLPLANE-001 v0.4` (incl. Amendment 1 "Clearing-House Mandate," §15, inverted §8, §12 criteria 13–15) and precedence-2 `EOP-RB-CONTROLPLANE-001`. **Neither exists in this repository.** The only committed version is `docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.3.md`, which contains no §15, no L1–L3/C1–C3/K1–K3 topology sections, and no "Clearing-House Mandate" text (verified: `grep -rl "v0\.4\|Clearing-House Mandate\|inverted §8\|§15\.5"` across all `.md` files in the repo returns zero hits). No standalone Addenda A–C files exist either (Addendum C entered this session only as pasted chat text, never committed).

Per the architect's direction (option 2, this session): this run proceeds using **only the AB1–AB7 clause text embedded in the audit prompt itself** as the operative specification, since that text is self-contained enough to execute real probes against. Every clause below is cited as "audit-prompt text" — **not** as a page/section citation into the (absent) v0.4 document, because that document cannot be read. This gap is itself recorded as an escalation (MCA-6, item E-1) and a model-silent item (MCA-5) — the precedence-1 document is not checked into the repository the ledger otherwise treats as authoritative.

---

## MCA-1 — Clause Register (AB family)

| ID | Source citation | Testable obligation | Proof method |
|----|-----------------|----------------------|---------------|
| AB1 | audit-prompt text, "§15.1" (unverifiable — v0.4 absent) | Every utterance ingress reaches the CP before parsing/pack selection | Trace: ingress → first CP contact, file:line |
| AB2 | audit-prompt text, "inverted §8" (unverifiable) | Sage invoked BY the CP with granted context; holds no capability keys | Compile-probe: capability call from Sage code with no CP-issued context |
| AB3 | audit-prompt text, "§6.13.1" (unverifiable) | Every candidate intent carries interpretation attestation; unattested → rejected at admission | Trace return path + type-level construction check |
| AB4 | audit-prompt text, "§15.5 as ratified" (unverifiable) | Sage's interpretation-context reads occur only via ratified lens/broker mechanism | grep Sage crate(s) for direct store/SemOS clients |
| AB5 | audit-prompt text | Clarification loop stays keyless; session-state writes classified agent-local vs. operational; operational writes cross CP | Trace clarification-loop state writes to their sink |
| AB6 | audit-prompt text | REPL dual-nature contained; `raw-dsl-dev` absent from production, and even dev builds route through `evaluate()` | grep for raw-DSL bypass surface, re-prove absence |
| AB7 | audit-prompt text | Non-intent (pure read/Q&A/telemetry) utterances still conform to mediation — "it's only a read" is not exempt | Trace a contextual-query utterance end-to-end |

All seven clauses have an executable proof method (no clause is unprovable-escalated).

---

## MCA-2/3 — Execution (topology + mechanism, combined for this scoped run)

### AB1 — NOT independently gated pre-parse; runs as async, non-blocking shadow observation after interpretation

Ingress: `POST /api/session/:id/input` → `session_input` (`src/api/agent_routes.rs:147`) → `dispatch_to_v2_repl` → `ReplOrchestratorV2::process` (`src/sequencer.rs:1315`). Inside `process()`: contextual-query intercept fires at `src/sequencer.rs:1417-1420` (returns immediately, bypassing everything downstream — see AB7), then Sage pre-classification fires at `src/sequencer.rs:1432` — **both before any `ob-poc-control-plane` code executes.** The crate's `evaluate_shadow`/`evaluate_with_report` is invoked only deep inside per-verb dispatch (`src/sequencer.rs:7934-7965`), spawned into an async task (`src/sequencer.rs:7970-7985`) that never blocks the calling turn, and is explicitly self-documented in-code as non-gating ("Never gates dispatch... this only observes and records," `src/sequencer.rs:7808-7811`).

**Verdict: MIGRATION-PENDING.** This is a real violation of AB1 as worded (CP is contacted after interpretation, not before, and even then doesn't gate) — but it satisfies both conditions of the two-part test: (a) explicitly identified as transitional by the plan's own repeatedly-cited "shadow-first posture" (Addendum B §0, referenced throughout the ownership ledger's T9/T10 entries — e.g. "every path stays envelope-less/legacy until it individually graduates"), and (b) ledger-tracked with a real retirement path (`OB_POC_CONTROL_PLANE_ENFORCE_VERBS`, the graduation criterion of "≥500 production shadow evaluations with zero divergence" recorded at `docs/research/control-plane-ownership-ledger.md:400`). Recorded as MIGRATION-PENDING, not NONCONFORMANT — but flagged that AB1's own wording ("reaches the CP BEFORE parsing") describes a materially stronger obligation (pre-parse mediation of the utterance itself) than what shadow-first graduation is building toward (post-parse, per-verb shadow observation) — these may not be the same target state at all. See MCA-6 escalation E-2.

### AB2 — NONCONFORMANT, BLOCKER (real compile-probe run and reverted)

`src/sage/` is `pub(crate) mod sage` declared in `src/lib.rs:184` — the **same compilation unit** as `src/sequencer.rs` (the dispatcher, which directly uses `dsl_runtime::TransactionScope`/`VerbExecutionPort`). `ob-poc`'s own `Cargo.toml` depends directly on `dsl-runtime` (line 54) and `sem_os_postgres` (line 53). `crates/ob-poc-sage`'s own Cargo.toml comment (lines 8-16) documents that the execution-tier-adjacent Sage modules (`llm_sage.rs`, `deterministic.rs`, `valid_verb_set.rs`) were deliberately kept **inside `ob-poc`** rather than the pure `ob-poc-sage` crate, "to wire Sage into the execution tier."

**Scratch probe executed:** appended a function to `src/sage/llm_sage.rs` taking `&mut dyn dsl_runtime::TransactionScope` and calling `.scope_id()` — a direct capability-tier call from inside Sage's own module, with zero CP-issued context of any kind. `cargo check -p ob-poc --lib` — **compiled cleanly, zero errors.** Probe removed immediately after (file diff confirmed empty, `cargo check` re-run clean).

This is exactly the negative-proof case AB2 requires to be structurally impossible ("must not compile under L2"). It compiles. Per the audit's own severity rule, this is a **BLOCKER regardless of current runtime behaviour** — the Explore agent's independent trace confirmed `llm_sage.rs` does *not currently* make such a call (it only imports `ob_agentic::llm_client` and sibling Sage types), so there is no live exploit today. The finding is that nothing in the crate/module structure prevents one from being added — "the structure IS the control" is not true here; only convention is.

### AB3 — CONFORMANT (stronger than the clause's own framing)

`OutcomeIntent` carries a mandatory (non-`Option`) `confidence: SageConfidence` field (`crates/ob-poc-sage/src/outcome.rs:264`, enum `High/Medium/Low`). Because the field is mandatory at the type level, an "unattested" candidate cannot be constructed at all — stronger than a runtime rejection-on-missing-attestation check. The attestation is consumed, not decorative: `src/agent/orchestrator.rs:1246` routes low-confidence or clarification-pending candidates away from direct dispatch; `:1695`, `:2387-2390` gate scoring/flow elsewhere on the same field. Every write-shaped candidate is additionally wrapped in `UtteranceDisposition::Delegate` (`crates/ob-poc-sage/src/disposition.rs:9-15`), which the orchestrator turns into a `PendingMutation` requiring an explicit human-readable confirmation string before dispatch (`src/agent/orchestrator.rs:220-257`).

Note: the clause's own language ("deterministically rejected at admission") describes a hard-reject semantics; the actual code routes low-confidence candidates to a clarification loop rather than rejecting outright. Recorded as a wording mismatch, not a conformance failure — the effect (no low-confidence candidate reaches dispatch unconfirmed) is achieved by a different, arguably safer mechanism (ask again) than the clause literally describes (reject).

### AB4 — NONCONFORMANT

`crates/ob-poc-sage/src/session_context.rs:8,121-460` runs direct `sqlx::query`/`sqlx::query_as` against its own `PgPool` (session create/update/load, client-group listing, discovery-status/CBU/case/workstream lookups) from **inside the Sage crate itself**, gated only behind the crate's own `database` cargo feature — not behind any CP-supplied lens, envelope, or broker. This is precisely the pattern AB4's proof method ("grep Sage's crate for direct store/SemOS read clients outside it") is written to catch. Not found as a registered item anywhere in `docs/research/control-plane-ownership-ledger.md` (grepped: zero hits for "session_context," "lens," "brokered"). No retirement path exists, so the two-condition MIGRATION-PENDING test fails on condition (b).

The production LLM classifier itself (`llm_sage.rs`, `deterministic.rs`) does not query directly — it consumes a caller-assembled `SageContext` (`src/sage/mod.rs:69`). The violation is localized to `session_context.rs`'s helper functions, not the classification logic.

### AB5 — NONCONFORMANT (resolved from MODEL-SILENT via the ratified readership rule)

**Update (architect ruling on E-3, same day):** the ambiguity below is resolved by a standing rule, now in the ownership ledger ("MCA-001 addendum — AB5 classification rule"): *a write is operational if any capability, gate, or audit-of-record path reads it; agent-local only if the sole consumer is the agent tier itself, for conversational continuity.* Applying it, traced (not assumed):

In-turn disambiguation state (`ReplStateV2::ScopeGate { pending_input, candidates }`, `src/sequencer.rs:4237-4240`) is checkpointed via `persist_session_checkpoint_inner` → `save_session_snapshot` (`src/sequencer.rs:785-845`) into `"ob-poc".repl_sessions_v2.state`. `SessionRepository::load_session` (`src/repl/session_repository.rs:235-294`) reads that row back on resume and reconstructs `ReplSessionV2.scope`/`.stage_focus`. Those fields feed `VerbSurfaceContext` at `src/agent/orchestrator.rs:435-448`, consumed by `compute_session_verb_surface` (`src/agent/verb_surface.rs:324`) to produce `SessionVerbSurface.allowed_fqns()` — which directly gates dispatch-adjacent search: `orchestrator.rs:1846-1865` narrows the allowed verb set to a constrained-match candidate only when it's already a member of that surface.

The checkpoint write is decision-relevant by the rule's own test (it can influence a future turn's dispatch-adjacent gating), therefore operational, therefore should cross the CP under AB5 as written — and currently does not. **Verdict: NONCONFORMANT.** Not found as a registered transitional item; added to MCA-4 below.

### AB6 — CONFORMANT (independently re-verified, exceeds the clause's own bar)

`execute_session_dsl_raw` (`src/api/agent_routes.rs:2024-2036`) unconditionally returns `403 FORBIDDEN` whenever `req.dsl.is_some()` — not behind any `#[cfg(feature=...)]` or `#[cfg(test)]` gate. `execute_session_dsl_legacy_raw_only` returns `410 GONE` unless `is_raw_execute_request` is true, which then still hits the same forbidding branch (`:1948`). `crates/ob-poc-boundary/src/policy/gate.rs:15-17` documents `OBPOC_ALLOW_RAW_EXECUTE` and `PolicyGate::can_execute_raw_dsl` as removed with "no config flag reopens the bypass." Verified across both prod and test-compiled code paths (not merely feature-gated out of a prod build). This exceeds AB6's bar (which only requires routing through `evaluate()` in dev builds) — the path is closed outright, in every build.

### AB7 — NONCONFORMANT

`is_contextual_query(content)` is checked at `src/sequencer.rs:1417-1420`, at the very top of `process()`, before Sage classification (`:1432`) and before any CP contact. `handle_contextual_query` (`src/sequencer.rs:4856-4877`) reads only already-hydrated in-memory session state (`session.workspace_stack.last()?.hydrated_state`) and calls `query_narration` (`src/agent/narration_engine.rs:123-159`), a pure function with zero `sqlx`/`PgPool` imports anywhere in that file. The clause's own text is explicit that "it's only a read" is not an exemption class — this utterance class is served with zero mediation of any kind (no lens, no broker, no CP contact) on the turn it's served. Not found as a registered transitional item in the ledger.

### C2 — metric absence confirmed

`capability_invocations_without_cp_provenance` — grepped across the full `rust/` tree (excluding `target/`): **zero hits.** The metric does not exist. Per the audit's own rule, the absence itself is the finding (C2 clause execution, in service of AB2's severity context).

---

## MCA-4 — Conformance Gap Register (T11 mesh-retirement backlog)

| Clause | Verdict | Severity | File:line | Evidence | Retirement path |
|--------|---------|----------|-----------|----------|------------------|
| AB2 | NONCONFORMANT | **BLOCKER** | `src/lib.rs:184` (`pub(crate) mod sage` in the same crate as the dispatcher); `Cargo.toml:53-54` (`ob-poc` depends on `dsl-runtime`/`sem_os_postgres` directly); `crates/ob-poc-sage/Cargo.toml:8-16` (execution-tier Sage modules deliberately kept in `ob-poc`) | Live compile-probe: appended a `dsl_runtime::TransactionScope`-consuming fn to `src/sage/llm_sage.rs`; `cargo check -p ob-poc --lib` compiled clean; probe reverted | None registered. Needs: either a crate split isolating `llm_sage.rs`/`deterministic.rs`/`valid_verb_set.rs` into a capability-free crate (mirroring `ob-poc-sage`'s own extraction boundary), or a dep-gate lint (`scripts/check_kyc_substrate_deps.sh`-style) forbidding execution-tier imports from `src/sage/*.rs` specifically |
| AB4 | NONCONFORMANT | Moderate | `crates/ob-poc-sage/src/session_context.rs:8,121-460` | Direct `sqlx::query`/`query_as` against `PgPool` inside the Sage crate, gated only by the crate's own `database` feature, not by any CP-supplied lens/envelope | None registered. Needs either: relocate `session_context.rs`'s DB helpers out of `ob-poc-sage` into the caller (mirroring the `llm_sage`/`deterministic` split already documented in the crate's own Cargo.toml), or register it as an explicit transitional item with a stated target mechanism |
| AB7 | NONCONFORMANT | Moderate | `src/sequencer.rs:1417-1420` (intercept), `src/agent/narration_engine.rs:123-159` (`query_narration`, zero DB/CP imports) | Trace: contextual-query utterances return before Sage classification and before any CP contact | None registered. Needs either: route contextual queries through a lightweight CP-observed lens (even read-only observation would satisfy "mediation," per the clause's framing), or an explicit ratified exemption for pure-read/no-mutation utterance classes (which the clause as given currently forecloses) |
| AB5 | NONCONFORMANT | Moderate | `src/sequencer.rs:785-845` (checkpoint write), `src/repl/session_repository.rs:235-294` (`load_session` read-back), `src/agent/orchestrator.rs:435-448` (feeds `VerbSurfaceContext`), `orchestrator.rs:1846-1865` (gates `surface_allowed` narrowing) | Traced per the ratified readership rule: checkpoint → `load_session` → `scope`/`stage_focus` → `VerbSurfaceContext` → `SessionVerbSurface.allowed_fqns()` → dispatch-adjacent gating | None registered. Converges with AB4's remedy: once the agent tier can no longer write/read the DB directly (AB2's tier split), the checkpoint's CP-crossing requirement becomes a natural consequence of routing session persistence through the CP boundary rather than a separate patch |
| AB1 | MIGRATION-PENDING | N/A (tracked, waypoint gap) | `src/sequencer.rs:1417-1432` (pre-CP interpretation), `:7934-7985` (async, non-blocking, shadow-only CP contact), `:7808-7811` (self-documented non-gating) | Ledger-tracked shadow-first posture (condition a: §15.6 checkpoint topology is explicitly transitional model state, per architect ruling on E-2) + `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` graduation criterion (condition b) | **Ruling (architect, E-2): stands as MIGRATION-PENDING, not NONCONFORMANT — but the registered retirement path terminates one waypoint short.** Graduation completes the *checkpoint* topology (enforce at inserted points); AB1 describes *mediation* (CP owns from the utterance) — §15.6 makes checkpoint the explicitly transitional state toward mediation. What keeps this honestly pending: the T11 plan existing with mediation as its stated terminus, not merely checkpoint-graduation. This is now load-bearing, not optional — T11 must not stop at checkpoint-enforce |

---

## MCA-5 — MODEL-SILENT Register

- ~~AB5 classification ambiguity~~ — **resolved, see the AB5 entry above and the ledger's standing readership rule.** No longer model-silent.
- **Three distinct "Sage" locations exist** (`crates/ob-poc-sage`, `crates/dsl-sage`, `src/sage/`) and the model text supplied in the audit prompt doesn't distinguish them. `crates/dsl-sage` ("utterance → ranked decision-pack candidates") is a workspace member with zero external callers (`grep dsl_sage` outside its own crate: no hits) — dormant, not on any live path. The model doesn't address whether a dormant, unreferenced crate matching the AB family's subject name should itself be a finding (dead code vs. abandoned migration vs. future work) — recorded, not judged.

---

## MCA-6 — Escalation Register (architect decisions only — no findings disposed of here)

- **E-1**: `EOP-VS-CONTROLPLANE-001 v0.4` did not exist in this repository at audit time; only v0.3 was checked in. **Ruling: architect's own debt, cleared same day.** Architect supplied Amendment 1's full draft text; deltas applied mechanically (new §15 incl. ratified R-a, §8 direction inverted at 8.1/8.5/8.6, §12 criteria 13-15 added, change-log entry) to `docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.md`, which now supersedes v0.3 as the checked-in precedence-1 document. **CLOSED.**
- **E-2**: AB1 as worded describes pre-parse, utterance-level CP mediation; the live T9/T10 shadow-first program builds post-parse, per-verb-dispatch shadow observation. **Ruling: RESOLVED — auditor's suspicion confirmed correct.** These are not the same target state: graduation completes the *checkpoint* topology, AB1 describes *mediation* (CP owns from the utterance) — §15.6 makes checkpoint the explicitly transitional state en route to mediation, per the architect. AB1's MIGRATION-PENDING verdict stands on that basis, but its registered retirement path (checkpoint-enforce graduation) is now known to terminate one waypoint short of AB1's actual target — the T11 plan must carry mediation as its stated terminus, not stop at checkpoint-enforce. CLOSED as an escalation; reopened as a scoping constraint on T11 (see MCA-4).
- **E-3**: AB5's agent-local/operational classification criterion was unspecified. **Ruling: RESOLVED — standing rule ratified** (readership test: operational iff any capability/gate/audit-of-record path reads it), recorded in the ownership ledger, applied to the AB5 finding above (verdict flipped MODEL-SILENT → NONCONFORMANT). CLOSED.

---

## MCA-7 — Misdocumentation Register

No misdocumentation found this pass. `src/sequencer.rs:7808-7811`'s in-code claim that CP evaluation "never gates dispatch, only observes and records" was independently verified true (async spawn, `legacy_outcome` computed and returned before the CP call, `src/sequencer.rs:7934-7995`). `crates/ob-poc-boundary/src/policy/gate.rs:15-17`'s claim that the raw-DSL bypass cannot be reopened was independently re-verified true (AB6). `crates/ob-poc-sage/Cargo.toml:8-16`'s in-scope/out-of-scope split accurately describes where the execution-tier-coupled Sage modules actually live (AB2's finding is about the *consequence* of that documented, honest split — the crate boundary doesn't structurally enforce what the comment describes as the intent — not about the comment being wrong).

---

## MCA-0 — Executive Summary

**Verdict: NONCONFORMANT.** Four unregistered violations exist (AB2 BLOCKER — root cause: agent-tier code shares a compilation unit with the dispatcher, so L2's keyed-door construct has nothing to key; AB4, AB5, AB7 moderate — AB4 and AB5 converge on the identical gate, `SessionVerbSurface`/`surface_allowed`, and both close as a structural consequence of AB2's remedy). One violation (AB1) passes the two-condition MIGRATION-PENDING test on architect ruling, with its retirement path now explicitly required to terminate at mediation (§15.6), not checkpoint-enforce. Two clauses are CONFORMANT (AB3, AB6 — both exceed the clause's own literal bar). Zero clauses remain unverdicted — the one MODEL-SILENT item (AB5) was resolved same-day via a new standing rule, now in the ownership ledger.

**Mesh remainder:** `capability_invocations_without_cp_provenance` metric — **absent** (not merely zero; the metric does not exist in the codebase). Per architect direction, this is the first item of any future T11 tranche: measure before retiring, retire before locking.

**Clause coverage:** 7/7 AB-family clauses executed with real evidence (2 CONFORMANT, 1 MIGRATION-PENDING, 4 NONCONFORMANT, 0 unresolved). Zero clauses skipped. One scratch compile-probe run and reverted (AB2). Precondition gap (source model v0.4 absent) recorded as E-1 — CLOSED same day once the architect supplied Amendment 1's draft text; did not block this run because the audit prompt's own embedded clause text was sufficient to execute all seven probes.

**Scope note:** this run covers the AB family only, per the architect's explicit narrowing to "the flow / call chain." The full §15 topology census (L1/L3, C1/C3, K1–K3 crate-dependency edges beyond what AB2/AB4 touched) was not executed. Architect direction: do not cut the T11 plan from this AB-scoped register — schedule the full MCA run first (once v0.4 is ratified), and cut T11 from the *complete* MCA-4.

MCA COMPLETE — E1..E5 satisfied — verdict: NONCONFORMANT — mesh remainder: metric-absent
