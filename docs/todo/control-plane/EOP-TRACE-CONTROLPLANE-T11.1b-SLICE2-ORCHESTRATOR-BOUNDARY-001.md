# EOP-TRACE-CONTROLPLANE-T11.1b/slice2 — `agent/orchestrator.rs` File-Level Boundary Trace

### Basis: EOP-DESIGN-CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-SPLIT-001 (ratified separation law), executing its §4 "next step"
### Status: TRACE COMPLETE, no code moved. Finding changes the shape of the split from "two piles of functions" to "one minted grant + everything downstream reads it."

## 1. Method

Full function inventory of `rust/src/agent/orchestrator.rs` (4,890 lines, 39 top-level fns/structs outside `#[cfg(test)]`), then every call site of the four verdict-producing surfaces was traced by line number:

- `compute_session_verb_surface` / `VerbSurfaceContext` (the SessionVerbSurface — mode/workflow/SemReg/lifecycle/fail-policy fusion, ranked)
- `Phase2Service::evaluate*` (legality artifact evaluation over the SemOS envelope)
- `resolve_sem_reg_verbs` (mints the `SemOsContextEnvelope` itself — CCIR resolution)
- `ctx.policy_gate.semreg_fail_closed()` / `VerbSurfaceFailPolicy` (fail-open/fail-closed posture — a policy verdict, not data)

## 2. Headline finding

**The legality computation is not concentrated at one or two call sites — it is independently recomputed at 7 separate call sites across 5 different functions**, each building its own `VerbSurfaceContext`/calling `resolve_sem_reg_verbs`/`Phase2Service::evaluate*` fresh:

| Line | Function | What it recomputes |
|---|---|---|
| 385, 394-451 | `prepare_turn_context` | `resolve_sem_reg_verbs` → envelope, `compute_session_verb_surface` → surface, `Phase2Service::evaluate` |
| 1142, 1203 | `handle_utterance` | `Phase2Service::evaluate` (twice, against `prepared.envelope` — reads Prepared's copy, doesn't remint) |
| 1602-1645, 1676 | `legacy_handle_utterance` | `resolve_sem_reg_verbs` → envelope, `compute_session_verb_surface` → surface, `Phase2Service::evaluate_from_envelope` (own independent mint, not shared with `prepare_turn_context`'s) |
| 2120, 2125 | `legacy_handle_utterance` (fast-path-miss fallback deeper in the same fn) | `resolve_sem_reg_verbs` called a **second time inside the same function body**, `Phase2Service::evaluate_from_envelope` again |
| 2481 | `build_trace` | `Phase2Service::evaluate_from_envelope` (reads an already-resolved envelope passed in — cheap re-derivation, not a re-mint) |
| 2974-2988 | `handle_utterance_with_forced_verb` | `resolve_sem_reg_verbs` → envelope (own independent mint, 4th so far), `Phase2Service::evaluate_from_envelope`, `Phase2Service::runtime_gate_status` |
| 3206 | `default_trace_for_runtime` | `Phase2Service::evaluate` (reads `prepared.envelope`, re-derivation not re-mint) |

Of these, **4 are independent mints of the SemOS envelope** (`resolve_sem_reg_verbs` at 385, 1602, 2125, 2974) and **2 are independent computations of the SessionVerbSurface** (`compute_session_verb_surface` at 449, 1632) — meaning the same "is this legal" question is answered from scratch, on session/entity state read at slightly different points in the turn, up to 4 times per utterance depending which code path fires (`handle_utterance` → Sage succeeds → `prepare_turn_context` path is 1 mint; Sage fails/legacy → `legacy_handle_utterance` path is up to 2 mints in one function body; `handle_utterance_with_forced_verb` is its own 4th path entirely, used for the disambiguation-menu re-entry).

This is the concrete, file-level shape of the entanglement the ratified design law was written against. It also explains *why* a clean "these functions are agent-tier, those are CP-tier" split doesn't work mechanically: the legality mint is not a boundary crossed once, it's a pattern repeated ad hoc at nearly every branch point, each recomputing against locally-available context rather than reading a single passed-down verdict.

## 3. Classification of the file's other content (verdict-adjacent, not verdict-producing)

- **`emit_telemetry`** (2835) — writes `intent_events` via `ctx.pool` (real `sqlx`). Reads verdict *fields* off an already-built `OrchestratorOutcome` (fingerprints, pruned counts, TOCTOU results) for audit-of-record persistence; does not itself decide anything. CP-adjacent infrastructure (audit path), not interpretation and not a verdict producer — stays put regardless of how the split lands.
- **`persist_trace_scaffold`, `finalize_orchestrator_trace`** (1347, 1393) — trace lifecycle bookkeeping (`UtteranceTraceRepository`, DB-backed). Same class as `emit_telemetry`: CP-adjacent audit plumbing, reads outcomes, doesn't adjudicate.
- **Pure interpretation, zero verdict contact** (confirmed by absence from the call-site table above): `route`, `build_mutation_confirmation`, `can_use_sage_structure_fast_path`, `read_only_list_fallback`, `run_sage_stage`, `run_coder_stage`, `coder_result_from_compiler_selection`, `render_selection_dsl`/`render_dsl_string`, `is_data_management_focus`/`infer_data_management_domain`/`should_use_structure_first_prompt`/`data_management_rewrite` family, `dsl_similarity`, `build_sage_fast_path_result`, `build_journey_pipeline_result`, `build_journey_selection_decision`. These are genuinely clean agent-tier candidates — the ones the design law's rule 1 describes.
- **`resolve_allowed_verbs`** (3400, `pub`) — a standalone entry point, not called from within this file itself (grep confirms zero internal callers; it's an external API used elsewhere, e.g. by MCP tool handlers per its own doc comment "don't have a full OrchestratorContext"). It independently re-implements the envelope→surface pipeline for callers outside the orchestrator. Same mint pattern, 5th occurrence, different caller population — relevant to the eventual CP-grant design (external callers need the same minted grant, not their own copy of the recipe).

## 4. What this changes about the split plan

A file-level "move these functions to agent-tier, leave those in CP-tier" partition is not the right first move — cutting the file in place today would either (a) leave the 4 redundant mint sites duplicated across both crates (worse than now), or (b) require simultaneously solving "what does the CP-grant type look like" as a precondition, since there's no existing single verdict object to hand across the boundary.

**Revised, sequenced plan** (each step independently buildable/testable, matching this session's slice discipline):

1. **Introduce the `LegalityGrant` type** (CP-tier, lives in `ob-poc-control-plane` or `sem_os_core` per T11.2's eventual home) wrapping today's `(SemOsContextEnvelope, SessionVerbSurface, Phase2 artifacts, fail_policy)` tuple as one struct, with a `minted_at`/staleness marker per design-law rule 3.
2. **Replace all 4 independent `resolve_sem_reg_verbs` + `compute_session_verb_surface` mint sites** (385/1602/2125/2974, plus `resolve_allowed_verbs`'s external copy) **with one minting call** at the single earliest point in the turn where the required inputs (`ctx.scope`, `ctx.stage_focus`, `ctx.agent_mode`, resolved entity/intent) are available — almost certainly hoisted to the top of `handle_utterance`, before the `route(&intent)` branch, so both the `prepare_turn_context` and `legacy_handle_utterance` paths receive the same grant instead of minting their own.
3. **Thread the grant as a parameter** through `prepare_turn_context`, `legacy_handle_utterance`, `handle_utterance_with_forced_verb`, `build_trace`, `default_trace_for_runtime` — replacing their internal `Phase2Service::evaluate*`/envelope reads with grant field reads. This is the mechanical bulk of the work and is safe to verify incrementally (each function's behavior should be identical, since the grant is the same data these functions build today — the diff is *where* it's built, not *what* it contains).
4. **Only after step 3** does a genuine agent-tier/CP-tier file split become mechanical: the functions in §3's "pure interpretation" list plus the now-grant-consuming versions of `prepare_turn_context`/`legacy_handle_utterance`'s non-legality logic move to `ob-poc-agent`; the minting call site and the grant type itself are the CP-tier surface T11.2 formalizes.
5. **`resolve_allowed_verbs`'s external callers** get migrated onto the grant-minting call at the same time step 2 lands, closing the 5th mint site rather than leaving it as a permanent second recipe.

Steps 1-3 are pure refactor (no behavior change, fully test-covered by existing orchestrator tests) and can land as their own tranche ahead of the actual crate-boundary move in step 4. This ordering also directly satisfies design-law rule 4 (grep-provable: once step 2 lands, zero `compute_session_verb_surface`/`resolve_sem_reg_verbs` call sites remain outside the single mint function).

## 5. Status

Trace complete. Steps 1-5 above are the concrete execution plan; none started. Awaiting explicit "proceed" and a decision on which step to start with (recommend step 1: define `LegalityGrant` first, since steps 2-5 all depend on its shape existing).

## 6. Execution log (2026-07-12)

**Steps 1-3: DONE.** `rust/src/agent/legality_grant.rs` (new module) defines `LegalityGrant` (envelope + surface + phase2 + fail_policy + composite_state) and two functions:
- `mint_legality_grant()` — the full mint (envelope → composite_state → surface → phase2), one implementation. `prepare_turn_context` and `legacy_handle_utterance`'s initial mint (previously two independently hand-rolled, *inconsistent* copies — `legacy_handle_utterance` never loaded `composite_state`/`entity_state`, a real drift the collapse fixes as a byproduct) both now call it.
- `verify_envelope_legality()` — a deliberately lighter envelope+phase2-only check, for call sites validating a single already-selected verb rather than discovering/ranking candidates. `handle_utterance_with_forced_verb` now calls it. The user, mid-session, explicitly confirmed keeping this path lighter rather than forcing it through the full mint. The TOCTOU staleness recheck inside `legacy_handle_utterance` needs even less (just the envelope, no `Phase2Evaluation`) — left as a bare `resolve_sem_reg_verbs` call, matching the design law's carve-out.

Verified: `cargo check`/`clippy -D warnings` clean, all 64 `agent::orchestrator::tests::*` pass, full lib suite 2145/0 (unchanged).

**Step 5: verified already correct, no code change.** Re-reading `resolve_allowed_verbs` closely (rather than trusting the directory-grep-style original count) found it isn't actually a duplicated recipe: it already shares the one real primitive (`resolve_context_internal`) with `resolve_sem_reg_verbs`, via a different request-builder (`resolve_via_client` vs. `resolve_allowed_verbs`'s own request construction) — legitimate, because its callers (`sequencer.rs`, `mcp/handlers/core.rs`, `api/agent_routes.rs`) don't have a full `OrchestratorContext` to build the `OrchestratorContext`-shaped request from. It also never builds a `SessionVerbSurface` — it's already the "light" shape. §2's original "5th mint site" framing overstated this; corrected here.

**Step 4: BLOCKED, real precondition found, not started.** `OrchestratorContext` is itself a mixed-tier struct — it carries capability handles (`pool: PgPool`, `verb_searcher: Arc<HybridVerbSearcher>`, `policy_gate: Arc<PolicyGate>`, `sem_os_client: Option<Arc<dyn SemOsClient>>`) *and* pure interpretation metadata (`session_id`, `stage_focus`, `goals`, `pre_sage_entity_kind/name`, `recent_sage_intents`, `sage_engine`) in one type. Every confirmed-clean interpretation function (`run_sage_stage`, `run_coder_stage`, etc. — verified directly, e.g. `run_sage_stage`'s body only ever touches the metadata fields) still takes `&OrchestratorContext` **wholesale** in its signature. Physically moving these functions to `ob-poc-agent` today would either (a) hand agent-tier code the capability handles directly — the exact L1 violation this whole program exists to prevent — or (b) require inventing a second projection type (`AgentTurnContext`, mirroring `LegalityGrant`'s pattern) that splits `OrchestratorContext` into its capability half and its metadata half. (b) is real, unscoped design work, not a mechanical move, and wasn't part of the ratified design law. Per B8 (no convenience re-widening, stop and flag), this is flagged rather than hacked around. **Recommend: fold into T11.2 as a second named target alongside the keyed-door capabilities** — `OrchestratorContext` split is the same shape of problem (capability handle vs. data), just for the context object instead of the legality verdict.
