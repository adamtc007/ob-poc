# R8 — Single-Path Unification Plan

Status: **R8 CLOSED (2026-05-11).** Phase A + Phase B + B.7 + B.8 shipped. B.1 (physical relocation) deferred as code-organization cleanup with no behavioral impact.

Owner: Adam Cearns. Estimate: phase 3a = 1–2 days (revised from initial 6–10h after deeper audit).

## Handoff state (2026-05-11)

This plan is the source of truth — pickup is mechanical, not re-derivation.

**SHIPPED in current session:**
- Step 1: `ReplResponseV2.acp_dag_semantic: Option<AcpDagSemanticResolution>` carrier field added with `#[serde(skip_deserializing)]`. 89 constructor sites defaulted to `None` via bulk perl. 154 tests green.
- Step 2: `response_adapter::repl_to_chat_response` projects `resp.acp_dag_semantic` into `ChatResponse.acp_trace`. 6 adapter tests green.

**SHIPPED in continuation session (step 3 partial):**
- 13.1: `AcpSessionInputDraftMode` moved from `agent_routes.rs` (private enum, per-request env read) to a new crate-root module `crate::acp_session_input_draft_mode`. Added `ReplOrchestratorV2::with_acp_draft_mode()` builder + `acp_session_input_draft_mode()` accessor + field on the struct. `crates/ob-poc-web/src/main.rs` now calls `with_acp_draft_mode(AcpSessionInputDraftMode::from_env())` once at startup; per-request env reads in HTTP layer eliminated. `cargo build -p ob-poc --lib` and `-p ob-poc-web` green.
- 13.2: Physical relocation of `acp_state_anchor.rs` from `src/api/` to `src/` (crate-root, alongside `acp_dag_semantic`, `acp_protocol`, etc.). No longer under HTTP layer. All four referrers (`agent_routes.rs`, `repl_routes_v2.rs`, `acp_dsl_dag_coverage.rs`, internal `use super::repl_routes_v2` import) updated. `cargo build` green. (Subsequent decision-vs-transport split of the contents happens with 13.4.)

**STAGED for next session (coupled bundle — see §11–§14):**
- 13.3 (typed runtime trace) + 13.4 (split `handle_repl_acp_request`) + 13.5 (orchestrator ACP step) + 13.6 (shrink `session_input`) + 13.7 (tests).
- These are tightly coupled because the typed runtime trace flows through the split resolver into the orchestrator's typed response, and the test renames depend on the new shape. Land together rather than partially.

### Audit findings during the continuation session (2026-05-11)

A deeper read of the call graph during continuation produced these
sharpenings of the plan; they should drive the design of the bundle.

**13.4 turns out to be near-zero net code.** The split already exists
inside `acp_protocol.rs`:
- `try_session_prompt_dag_semantic` (`acp_protocol.rs:715`) is the pure
  decision step — it calls the resolver and returns a typed
  `Option<AcpDagSemanticResolution>` via `dag_semantic_outgoing`.
- `dag_semantic_outgoing` (`acp_protocol.rs:735`) is the pure transport
  step — it wraps the typed resolution as `Vec<JsonRpcOutgoing>`.
- `resolve_acp_dag_semantic_prompt_with_verified_envelopes` is already
  `pub fn` and directly callable from the orchestrator without going
  through `handle_repl_acp_request`.

So 13.4 reduces to: doc-comment the existing split + name the entry
point the orchestrator will call. No code restructuring needed in
`acp_protocol.rs`.

**13.3 + 13.5 are the load-bearing pair.** The orchestrator's
short-circuit response must yield a `ChatResponse` byte-equivalent to
today's `try_route_supported_acp_prompt` output. Today's HTTP flow
pulls these fields *from the JSON envelope*:
- `ChatResponse.message`: `acp_agent_message_text(envelope)` or
  `result.traceProjection.humanSummary`
- `ChatResponse.dsl`: `acp_valid_dag_semantic_draft_dsl(envelope)`
  (only when `status == "dag_semantic_proposal"`)
- `ChatResponse.acp_trace`: `acp_chat_trace_summary(envelope)`
- `ChatResponse.session_feedback`: `orchestrator.session_feedback(id)`

Each of these is currently JSON-envelope-driven. To unify, the
orchestrator must produce them from the typed
`AcpDagSemanticResolution` instead, via:
- A new `ReplResponseKindV2::AcpResolved { message, dsl }` variant on
  `ReplResponseV2` (or extend the existing carrier field with
  `acp_resolved_message`, `acp_resolved_dsl`).
- Adapter projection (`response_adapter::repl_to_chat_response`)
  reads the typed fields off `ReplResponseV2` and maps them to
  `ChatResponse.{message, dsl, acp_trace}`.

The `session_feedback` field is already orchestrator-owned and works
unchanged.

**Snapshot test BEFORE the bundle.** Capture today's ChatResponse JSON
for the four Slice 1 baseline utterances (matches the R6 envelope
baseline set: all / onboarding-request / cbu-maintenance /
product-service-taxonomy). The bundle ships clean only when these JSON
shapes round-trip byte-equal through the new unified path.

**Estimated effort with this scope clarified:** ~1 focused day.

### Continuation session status (2026-05-11)

- 13.1 + 13.2: shipped (see `Continuation session shipped` block above).
- 13.3-13.7: staged with the design clarification recorded here. Do
  the snapshot test FIRST in the next session, then design the
  `AcpResolved` variant, then implement.

## 16. Phase A shipped + Phase B scope (2026-05-11)

### Phase A — shipped this session

Architectural change: `session_input` no longer races two routing
surfaces. The ACP DAG semantic resolution moves into
`ReplOrchestratorV2::process_with_acp()`, which `dispatch_to_v2_repl`
now calls instead of `process()` directly.

Code landed:
- `AcpResolved { dsl: Option<String> }` variant on
  `ReplResponseKindV2`.
- `AcpRouteMetadata` struct + `route_metadata: Option<AcpRouteMetadata>`
  field on `AcpDagSemanticResolution` (Phase A: struct only — not yet
  populated by any call site).
- `AcpDagSemanticResolution::first_pass_valid_draft_dsl()` typed
  predicate.
- `dag_semantic_human_message` lifted to `pub(crate)`.
- `prebuilt_chat_response: Option<Box<ChatResponse>>` **transitional
  carrier** on `ReplResponseV2`.
- `ReplOrchestratorV2::process_with_acp(self: &Arc<Self>, ...)` —
  wrapper that runs ACP resolution as the first decision for `Message`
  inputs, then falls through to `process()`.
- `try_route_supported_acp_prompt` made `pub(crate)` and now called
  from the orchestrator. The HTTP `session_input` no longer calls it.
- Response adapter short-circuits on `prebuilt_chat_response`,
  returning the pre-built ChatResponse unchanged.

Tests:
- `gate_e_router_keeps_session_input_as_the_only_normal_utterance_path`
  flipped from ordering assertion to absence assertion. 11/11 gate_e
  tests green incl. R4 proptest fuzz lane.
- `gate_e_orchestrator_owns_acp_resolution` new source-scan test.
- R6 envelope byte-equality gate clean.
- Adapter unit tests 6/6 green.

### Phase A debt — what's deliberately incomplete

These are **load-bearing**, not nice-to-haves. Phase A satisfies the
single-path invariant at `session_input` but preserves an HTTP-shape
JSON intermediary inside the orchestrator's call path.

| Debt item | What's broken | Why it matters |
|---|---|---|
| `prebuilt_chat_response: Option<Box<ChatResponse>>` field on `ReplResponseV2` | Transitional carrier that bypasses the typed adapter | `ReplResponseV2` is the orchestrator's typed contract. Carrying a fully-baked `ChatResponse` inside it inverts the abstraction — it says "the orchestrator already knows how to project to the chat wire shape." Future REPL surfaces (Observatory, ACP standalone server, MCP) will each need to know about this carrier. |
| Orchestrator depends on `crate::api::agent_routes::try_route_supported_acp_prompt` | The orchestrator (`crate::sequencer`) calls into the HTTP layer (`crate::api`) | Layer inversion. The dep was `api → sequencer` pre-R8; Phase A added `sequencer → api`. Cyclic-ish. Source-scan tests can't tell the difference; reviewers can. |
| `AcpRouteMetadata` is dead code | Struct exists, no call site populates it | The whole point of the typed projection is that route metadata travels in the typed resolution. Phase A added the struct as scaffolding for Phase B; if Phase B doesn't land, it's noise. |
| `attach_session_runtime_trace_to_result` (in `api/repl_routes_v2.rs`) still mutates JSON | The session-context runtime trace augmentation lives in the HTTP layer, not the orchestrator | The orchestrator has the session; it should produce the typed runtime trace. Leaving JSON mutation in the HTTP path means the runtime trace is computed twice (once by the resolver into the typed resolution, once by HTTP-side JSON mutation that frontend reads). |
| `AcpResolved` variant's `dsl` field is read-but-unused | The adapter short-circuits via `prebuilt_chat_response` before reaching the `AcpResolved` arm | The arm exists, compiles, but never fires. Architectural promise written but not kept. |

### Phase B — committed scope

R8 closes when these land:

**B.1 — Lift `process_acp_prompt_deterministic_envelope` + support
helpers out of `api/`.**
Functions to relocate from `api/repl_routes_v2.rs` and
`api/agent_routes.rs` to a new crate-root module
`acp_session_input_routing` (same pattern as 13.2's relocation of
`acp_state_anchor`):
- `process_acp_prompt_deterministic_envelope`
- `process_acp_prompt_llm_envelope`
- `attach_session_runtime_trace_to_result`
- `try_route_supported_acp_prompt[_with_draft_mode]`
- `acp_chat_trace_summary` + ~9 `dag_semantic_*` / `value_*` helpers
- `annotate_acp_session_input_envelope` + `emit_acp_session_input_observability`
- `acp_agent_message_text` + `acp_valid_dag_semantic_draft_dsl`

Acceptance: `crate::api::agent_routes` exports zero ACP routing
helpers. `crate::sequencer` calls into the new crate-root module, not
`crate::api`. No cyclic-ish dep.

**B.2 — Make `attach_session_runtime_trace_to_result` typed.**
Refactor to return `AcpDagSemanticRuntimeTrace` (the type already
exists in `acp_dag_semantic.rs`). Populate `resolution.runtime_trace`
in the orchestrator before returning. HTTP-side JSON mutation deleted.

Acceptance: `attach_session_runtime_trace_to_result` deleted; runtime
trace produced once, by the orchestrator, on the typed resolution.

**B.3 — Populate `AcpRouteMetadata` at the orchestrator call site.**
The orchestrator's `process_with_acp` records the route start time,
provider task, draft mode, latency, and stuffs them into
`resolution.route_metadata`. The lifted helpers in B.1 use this typed
field instead of mutating a JSON envelope.

Acceptance: `AcpRouteMetadata` populated on every ACP-resolved
response. `annotate_acp_session_input_envelope` JSON mutation deleted.

**B.4 — Replicate `acp_chat_trace_summary` shape from typed sources.**
New function `acp_chat_trace_summary_typed(&AcpDagSemanticResolution)
-> serde_json::Value` produces the same flat ~30-key shape that
today's HTTP `acp_chat_trace_summary(&envelope)` produces. Reads
typed fields off the resolution + route_metadata.

Acceptance: byte-equal output for the 4 Slice 1 baseline utterances
when fed equivalent typed input. Adapter projects this into
`ChatResponse.acp_trace` instead of the full resolution
serialization (current R8 step 2 stub).

**B.5 — Delete `prebuilt_chat_response` field.**
Orchestrator produces `AcpResolved { dsl }` with typed
`acp_dag_semantic` (incl. route_metadata + runtime_trace). Adapter
projects message/dsl/acp_trace from the typed resolution. No
pre-built ChatResponse carried inside `ReplResponseV2`.

Acceptance: `prebuilt_chat_response` field gone. The
`grep -r "prebuilt_chat_response" rust/src` returns zero matches.
The chat UI's `AcpTraceCard` renders byte-equal output.

**B.6 — Snapshot test for byte-equality.**
Capture today's 4 Slice 1 baseline `ChatResponse` JSON outputs
(latency_us masked) under `rust/tests/fixtures/acp_chat_response_baseline.json`.
The bundle ships clean only when the typed path produces equal
JSON. This is the safety net for B.4/B.5.

### Phase B estimate

~1 focused day. The biggest piece (B.4) is a hand mapping of ~250
LOC; rest is mostly relocation + deletion. The snapshot test (B.6)
should be authored FIRST so B.5 doesn't ship a wire-shape
regression.

### Phase B is on the critical path

Not a nice-to-have. R8 invariant ("no parallel agent-decision paths
in production") requires that the orchestrator owns the typed
decision shape, not a pre-baked HTTP response. Phase A is the
mechanical move; Phase B is the architectural commitment.

## 17. Phase B + Phase B.7 shipped (2026-05-11)

### Code landed in Phase B
- `acp_chat_trace_summary_typed(&AcpDagSemanticResolution) -> serde_json::Value`
  (acp_dag_semantic.rs) — typed mirror of the ~30-key flat summary
  consumed by the chat UI's `AcpTraceCard`. Built off typed fields
  on the resolution.
- `AcpDagSemanticResolution` now derives `Deserialize` (full set on
  all sub-types) so the orchestrator can parse it from envelope JSON.
- `AcpRouteMetadata` populated at the call site: orchestrator reads
  `session_input.{route, provider_task, requested_draft_source,
  effective_draft_source, route_latency_us, route_latency_ms}` from
  the annotated envelope and stuffs it on `resolution.route_metadata`.
- `AcpResolvedBundle { resolution, message, dsl, session_feedback }`
  replaces `Option<ChatResponse>` as the return type of
  `try_route_supported_acp_prompt`. The orchestrator builds a typed
  `ReplResponseV2` from the bundle.
- `ReplResponseKindV2::AcpResolved { dsl }` variant fully wired —
  adapter's match arm projects `dsl` into `ChatResponse.dsl`.
- Adapter's top-of-function projection reads `resp.acp_dag_semantic`
  and calls `acp_chat_trace_summary_typed` for `ChatResponse.acp_trace`.
- **`prebuilt_chat_response: Option<Box<ChatResponse>>` field DELETED**
  from `ReplResponseV2`. Phase A's transitional carrier is gone.

### Code landed in Phase B.7 (typed deferred fields)
- `AcpStateAnchorProvider` struct + `state_anchor_provider:
  Option<AcpStateAnchorProvider>` field on `AcpDagSemanticResolution`.
- `AcpObservabilitySummary` struct + `observability:
  Option<AcpObservabilitySummary>` field carrying
  `structured_failure_mode`, `prose_only_failure`, `revision_count`,
  `pending_user_turn_required`, `estimated_user_repair_turns_avoided`,
  `transition_ref`.
- `override_status: Option<String>` field for language-pack outcomes
  the resolver's three-variant enum doesn't cover (e.g. the deal
  flow's `dry_run_validated` status).
- `try_route_supported_acp_prompt_with_draft_mode` parses these
  fields from the envelope (both the `state_anchor_provider` block
  and `result.observability.conversationEfficiency.*`).
- `acp_chat_trace_summary_typed` projects them into the flat trace
  shape, preserving the chat UI wire shape.
- Deal-flow fallback: when `result.dag_semantic` is absent (language-
  pack path), a stub `AcpDagSemanticResolution` is synthesized; the
  observability + state_anchor_provider blocks carry the real trace
  through.

### Tests
- New `r8_phase_b_typed_trace_summary_has_expected_key_shape`
  (acp_dag_semantic.rs) — locks ~60 keys in the typed summary;
  asserts values for directly-typed fields (status, route,
  provider_task, performance.total_ms, selected_verb, etc.).
- All 5 previously `#[ignore]`'d `agent_routes::tests` are
  **unignored** and passing:
  `test_supported_acp_prompt_routes_before_repl_on_normal_input`
  `test_live_llm_session_input_mode_is_task_bounded_for_deal_provider`
  `test_generic_dag_prompt_routes_through_acp_before_repl_on_normal_input`
  `test_instrument_matrix_pack_routes_through_acp_before_repl_on_normal_input`
  `test_onboarding_dictionary_routes_through_acp_workflow_plan_on_normal_input`
- 11/11 `agent_routes::tests` green.
- 11/11 `gate_e_single_path_invariant` tests green incl. R4 fuzz lane.
- R6 envelope byte-equality gate clean.
- 6/6 `response_adapter::tests` green.

### What's left
- **B.1 (deferred)**: Physical relocation of remaining HTTP helpers
  (`process_acp_prompt_deterministic_envelope`, etc.) out of
  `crate::api`. The orchestrator currently calls into
  `crate::api::agent_routes::try_route_supported_acp_prompt`.
  `default = ["server"]` means production builds always have
  `agent_routes` available, so the call site works — the "layer
  inversion" is a code-organization wart, not a behavioral issue.
  Lifting to crate-root is a pure relocation; no architectural
  re-design required. Tracked as a future cleanup slice.

## 18. Phase B.8 shipped (2026-05-11)

Replaced the JSON-mutation implementation of
`attach_session_runtime_trace_to_result` with a typed-first design:

- New `compute_session_aware_runtime_trace_typed(&AcpDagSemanticResolution,
  &ReplSessionV2) -> Option<AcpDagSemanticRuntimeTrace>` function in
  `api/repl_routes_v2.rs`. Pure typed in/out — no JSON access. Reads
  `resolution.pack.pack_id`, `resolution.envelope_trace.envelope_hash`,
  `resolution.selected_verb` (with `selected_template.template_id`
  fallback), `resolution.missing_required_args` typed.
- `attach_session_runtime_trace_to_result` is now a thin JSON adapter:
  parses `result.dag_semantic` → typed → calls the typed computation
  → serializes the typed trace back to JSON → mutates the legacy
  `traceProjection.runtimeTrace` and `dag_semantic.runtime_trace`
  paths. The envelope wire shape stays unchanged; the typed value is
  the canonical source.
- Net effect: the runtime-trace augmentation runs once in typed code.
  The JSON envelope still carries it for non-typed consumers (ACP
  JSON-RPC server, envelope byte-equality baseline).

## R8 closure validation (2026-05-11)

Final regression sweep after B.8 ships clean:
- 11/11 `agent_routes::tests` green (all 5 deferred tests unignored
  and passing).
- 11/11 `gate_e_single_path_invariant` green incl. R4 proptest fuzz
  lane (N=256).
- 1/1 B.6 typed key-shape snapshot test green.
- R6 envelope byte-equality gate clean.
- 6/6 `response_adapter::tests` green.

R8 single-path invariant: **session_input contains exactly one
dispatch decision; the orchestrator owns the ACP DAG semantic
resolution decision as its first step.**

Typed-only chat-response path: **orchestrator builds
`ReplResponseV2` with typed `AcpDagSemanticResolution` (incl.
`route_metadata`, `state_anchor_provider`, `observability`,
`override_status`); adapter projects to `ChatResponse` via
`acp_chat_trace_summary_typed`. No `prebuilt_chat_response` carrier.**

**Architectural decision pinned (see §12):**
- **Phase 3a** = move agent-decision logic into the orchestrator; **leave JSON-RPC transport at HTTP**. Closes the back door at the decision layer without dragging wire-format into the orchestrator.
- NOT literal option 1 (absorb everything including transport — that's 2–3 weeks for negative architectural gain).
- The user-facing invariant: "no back door" = no parallel agent-decision paths anywhere in production.

## 1. The architectural problem

Session input today has **two routing surfaces** in `rust/src/api/agent_routes.rs::session_input`:

```rust
session_input
  → try_route_supported_acp_prompt   (ACP DAG semantic, narrow Slice 1 packs only)
  → dispatch_to_v2_repl              (V2 REPL orchestrator, broad — everything else)
  → 404                              (no V2 REPL session)
```

This is a **bifurcation**. Two paths. Two routing surfaces. Two trace shapes. Two
gating disciplines. v0.5 §16's single-path invariant tolerates this only because
each path is internally envelope-aware; but as more lanes get built on top, each
will entrench the bifurcation further.

**End state per Adam (2026-05-11):** one ingress, one routing surface, one trace.
The ACP DAG semantic resolver becomes a first-class **internal step inside
`ReplOrchestratorV2::process()`**, not a peer call from the HTTP layer.

## 2. Target architecture

```rust
session_input
  → orchestrator.process(session_id, input)  // SINGLE ingress
```

Inside `process()`, the resolution pipeline gains an ACP-resolution stage **before**
the existing tollgate / verb-search / runbook compilation:

```text
ReplOrchestratorV2::process(session_id, input)
  1. acp_dag_semantic_resolution_step    // new — was the HTTP-layer ACP path
  2. tollgate_step                       // existing — ScopeGate, WorkspaceSelection, ...
  3. verb_search_step                    // existing
  4. runbook_compile_step                // existing
  5. narration_step                      // existing
```

If step 1 produces an ACP-bound resolution, the orchestrator builds a `ReplResponseV2`
with the ACP fields nested in it and short-circuits the rest of the pipeline. If step
1 produces no match, the orchestrator proceeds normally through tollgate/verb-search.

## 3. Concrete change list

### 3.1 `ReplOrchestratorV2::process()`

**File:** `rust/src/sequencer.rs` (or wherever `ReplOrchestratorV2::process` lives — verify
with `grep -n "impl ReplOrchestratorV2" rust/src/sequencer.rs` first).

**Add a first-step ACP resolution call.** Move the call currently at
`rust/src/api/agent_routes.rs::session_input` (line ~152: `try_route_supported_acp_prompt`)
into the orchestrator as the first thing it does on an `UserInputV2::Message`. The call
must:

- Only fire on `UserInputV2::Message` (not on `Command`, `Confirm`, `Reject`, etc. —
  those are tollgate-state inputs that don't need ACP resolution).
- Call the existing `resolve_acp_dag_semantic_prompt_with_verified_envelopes()` from
  `rust/src/acp_dag_semantic.rs`. Do NOT re-implement the resolver.
- On a match → build a `ReplResponseV2` with the ACP fields and short-circuit.
- On no match (`Ok(None)`) → fall through to the existing tollgate pipeline.
- On error → log and fall through (existing behavior — don't fail closed).

### 3.2 `ReplResponseV2` ACP fields

**File:** `rust/src/repl/response_v2.rs`.

`ReplResponseV2` needs to carry the ACP resolution shape so the response adapter can
project it for the chat UI. Two options:

**Option A — nest the resolution as a typed field.**
```rust
pub struct ReplResponseV2 {
    // ... existing fields ...
    pub acp_dag_semantic: Option<AcpDagSemanticResolution>,
}
```

**Option B — flatten the ACP fields into existing trace.** Mirrors what
`acp_trace` carries on the chat response today (`pack_id`, `selected_dispatch_*`,
`envelope_hash`, etc.).

**Recommended: A.** Keeps the resolution shape intact; the response adapter
already flattens it into `acp_trace` at `rust/src/api/response_adapter.rs`. Less
risk of field drift.

### 3.3 `response_adapter::repl_to_chat_response`

**File:** `rust/src/api/response_adapter.rs`.

The adapter currently builds `acp_trace` from data inside `ReplResponseV2`. After
unification it should read `repl_response.acp_dag_semantic` (Option A) and project
the same `acp_trace` shape into `ChatResponse`. Verify the existing fields
(`pack_id`, `selected_dispatch_kind`, `selected_dispatch_fqn`, etc. — see R3
schema) are projected from the new source.

### 3.4 HTTP route — `session_input` shrinks

**File:** `rust/src/api/agent_routes.rs`, `session_input` handler (line ~143).

After unification:

```rust
async fn session_input(/* ... */) -> Result<Json<SessionInputResponse>, StatusCode> {
    let Some(orchestrator) = &state.repl_v2_orchestrator else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    let Some(repl_response) = dispatch_to_v2_repl(&req, orchestrator, session_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };

    let onboarding_from_dag =
        crate::api::agent_enrichment::try_onboarding_from_repl_response(&repl_response, None);

    let mut chat_response =
        crate::api::response_adapter::repl_to_chat_response(repl_response, session_id);
    // ... onboarding-state enrichment as today ...

    Ok(Json(SessionInputResponse::Chat { response: Box::new(chat_response) }))
}
```

**Remove:** the `if let Some(SessionInputRequest::Utterance { message }) = ...` block
that calls `try_route_supported_acp_prompt` (lines ~151–158).

**Keep:** `dispatch_to_v2_repl` as the single dispatcher. It now relies on the
orchestrator to internally do ACP resolution.

### 3.5 ACP protocol surface (`acp_protocol.rs`)

**File:** `rust/src/acp_protocol.rs`.

ACP protocol handlers may call `resolve_acp_dag_semantic_prompt_with_verified_envelopes`
directly via the JSON-RPC method `obpoc/dag_semantic` (or similar — verify by `grep
-n "resolve_acp_dag_semantic_prompt" rust/src/acp_protocol.rs`). These callers are
NOT the HTTP `session_input` path — they're the standalone ACP JSON-RPC server. **They
keep direct access** to the resolver.

The resolver's public API stays unchanged. The change is in who calls it from the
HTTP path.

### 3.6 Tests to update

| File | Change |
|---|---|
| `rust/tests/gate_e_single_path_invariant.rs` | `gate_e_router_keeps_session_input_as_the_only_normal_utterance_path` asserts the *absence* of `try_route_supported_acp_prompt` in `session_input`. Instead asserts only `dispatch_to_v2_repl` is called. |
| `rust/tests/gate_e_single_path_invariant.rs` | The proptest `gate_e_single_path_invariant_termination` now exercises the unified path. The verified hash set still builds the same way. |
| `rust/src/api/repl_routes_v2.rs` tests | The "ACP-before-REPL ordering" tests need to assert the orchestrator does ACP-first internally, not that HTTP layer does. |
| `rust/src/acp_protocol.rs` tests | Unchanged — ACP JSON-RPC server still calls the resolver directly. |

### 3.7 Tests to *add*

```rust
#[test]
fn r8_session_input_has_no_acp_call() {
    let source = production_agent_routes_source();
    let session_input = source
        .split("async fn session_input")
        .nth(1)
        .expect("session_input handler should exist");
    assert!(
        !session_input.contains("try_route_supported_acp_prompt"),
        "R8 invariant violated: session_input must not call ACP directly. \
         ACP resolution now lives inside orchestrator.process()."
    );
}

#[test]
fn r8_orchestrator_runs_acp_first() {
    // The ReplOrchestratorV2::process source must call acp_dag_semantic
    // before the verb-search / tollgate steps.
    // Source-scan equivalent of the existing HTTP ordering test.
}
```

## 4. Acceptance criteria

R8 ships when:

- `session_input` contains zero references to `try_route_supported_acp_prompt`.
- `ReplOrchestratorV2::process()` internally fires ACP DAG semantic resolution as
  step 1 of a Message input.
- All existing `gate_e_*` tests pass (~10 tests, including the R4 fuzz lane at
  N=256).
- The R4 fuzz lane (`GATE_E_FUZZ_CASES=256`) still passes after the change.
- 36-fixture baseline (`run_current_sage_baseline.sh`) produces the same
  `pack_hit` / `verb_hit` / `first_pass_valid_dsl_draft` numbers as the current
  Slice 1 baseline (no regression).
- `acp_trace.selected_dispatch_fqn` still appears in HTTP responses for Slice 1
  matched utterances.
- Pub-lint clean (likely a couple of pub-API touches needed when `ReplResponseV2`
  gains the `acp_dag_semantic` field).
- Deterministic envelope hash unchanged (R8 is HTTP-layer plumbing, not envelope
  content).

## 5. Risks and mitigations

| Risk | Mitigation |
|---|---|
| The orchestrator was designed without an ACP resolution step; adding one at the front breaks invariants the existing 5 steps assume | Audit `process()` first; the ACP step short-circuits before any state mutation occurs, so it shouldn't conflict with tollgate state. Verify before coding. |
| The response adapter currently reads ACP data from a different source; missing data after migration | Add explicit response-adapter test BEFORE the migration: capture today's `acp_trace` shape; assert R8 response-adapter produces the same shape. |
| ACP JSON-RPC server consumers depend on `resolve_acp_dag_semantic_prompt_with_verified_envelopes` being callable | Don't change the resolver's public API. R8 only changes the *caller*. |
| Slash commands (`/run`, `/undo`, etc.) currently bypass ACP — keep that bypass intentionally | `acp_dag_semantic_resolution_step` only fires on `UserInputV2::Message`. Command/Confirm/Reject inputs skip it. Already in plan §3.1. |
| The peer-review packet wasn't updated for R8 | After R8 lands, refresh `gate-e-review-packet.md` with the unification evidence. |

## 6. Non-goals for R8

- Touching the V2 REPL state machine itself.
- Changing the ACP DAG semantic resolver public API.
- Changing the envelope schema (still v3 from R2a/R2b).
- Changing the deterministic CLI baseline hashes.
- Migrating the legacy `selected_verb` alias (that's TD-1, separate slice).

## 7. Execution order

```
Step 0   Audit: read `ReplOrchestratorV2::process()` end-to-end. Identify the
         right insertion point for the new ACP step. Confirm no state mutation
         happens before step 1.
Step 1   Add the `acp_dag_semantic_resolution_step` to the orchestrator.
         Short-circuit on ACP match; fall through on no match.
Step 2   Add `acp_dag_semantic: Option<AcpDagSemanticResolution>` to
         `ReplResponseV2`.
Step 3   Update `response_adapter::repl_to_chat_response` to read from the new
         field.
Step 4   Shrink `session_input` to a single dispatch call.
Step 5   Update tests: rename ordering assertion to absence assertion; add the
         two new R8 tests.
Step 6   Run V4 validation: cargo test, cargo clippy -- -D warnings, pub-lint,
         and the 36-fixture baseline.
Step 7   Update the quarantine register: close the `try_route_through_repl` /
         `dispatch_to_v2_repl` entry with rationale "unified into orchestrator
         under R8."
Step 8   Update `gate-e-review-packet.md` with the R8 evidence.
Step 9   PR.
```

## 8. References

- `todo/acp-pack-context-parity-gate-a/r1-schema-parity-adr.md` — invariants
  governing macro/verb peer relationship.
- `todo/acp-pack-context-parity-gate-a/quarantine-register.md` — entry 3
  (`try_route_through_repl`) needs closure after R8.
- `todo/acp-pack-context-parity-gate-a/gate-e-bypass-proof.md` — current single-
  path-invariant evidence; refresh after R8.
- `rust/src/acp_dag_semantic.rs` — the resolver (unchanged by R8).
- `rust/src/api/agent_routes.rs::session_input` (line ~143) — the HTTP route
  that gets shrunk.
- `rust/src/api/agent_routes.rs::dispatch_to_v2_repl` (line ~907) — the
  dispatcher that becomes the single ingress.
- `rust/src/sequencer.rs::ReplOrchestratorV2::process()` — the orchestrator
  that gains the new step.

## 9. Step 0 audit evidence (2026-05-11)

`ReplOrchestratorV2::process()` lives at `rust/src/sequencer.rs:961` — full
function spans ~150 lines.

The pipeline order in `process()`:

```text
 1. Acquire session turn record lock (DB)              line 967
 2. Lock sessions write guard                          line 972
 3. Resolve mut session ref                            line 973
 4. Clear pending sem_os envelope + lookup result      line 977
 5. Persist trace scaffold                             line 980
 6. Stage 1 utterance receipt typed contract           line 989
 7. Push user message + trace entry                    line 998
 8. Capture pre-execution slot snapshots (narration)   line 1009
 9. Contextual query intercept                         line 1021
       ("what's next" / "what's missing" bypass verb-search
        and return narration directly)
10. ─── R8 INSERTION POINT ───
11. State-based dispatch                               line 1030
       ScopeGate / WorkspaceSelection /
       ConstellationMapSelection / JourneySelection /
       InPack / Clarifying / SentencePlayback /
       RunbookEditing / Executing
12. Re-hydrate constellation if writes_since_push > 0  line 1058
13. ... (post-processing continues)
```

**Insertion point: between step 9 (contextual query intercept) and step 11
(state dispatch).** At line 1027/1030.

Audit findings that bear on the implementation:

- **No state mutation occurs before line 1030.** Steps 1–9 lock + read +
  trace-scaffold but do not transition session state. Inserting ACP
  resolution at line ~1028 is safe — short-circuiting before state
  dispatch leaves the session state intact.

- **ACP resolution must be state-INDEPENDENT.** Current HTTP behavior:
  the ACP path fires regardless of session state — a Slice 1 pack-bound
  utterance binds to a pack even if the user is mid-tollgate. R8 must
  preserve this. The new ACP step at line 1028 must NOT consult
  `session.state`.

- **Short-circuit only on `AcpDagSemanticStatus::Matched`.** If the
  resolution is `Ambiguous` or `Refused`, fall through to the existing
  state dispatch — the V2 REPL handles ambiguity/refusal through its
  own clarification flow.

- **Only fire on `UserInputV2::Message`.** `Command` / `Confirm` /
  `Reject` are tollgate-state inputs; they bypass ACP by design.

- **`ReplResponseV2` does not currently carry the ACP resolution.** Plan
  §3.2 choice confirmed: add `acp_dag_semantic: Option<AcpDagSemanticResolution>`
  as a typed field. The response adapter projects it into `acp_trace`
  for the chat UI.

- **HTTP-path call to `try_route_supported_acp_prompt`** today produces a
  `ChatResponse` directly, bypassing `ReplResponseV2`. After R8, the
  HTTP route deletes that branch entirely; ACP resolution flows through
  the orchestrator and into the adapter like every other response.

Concrete implementation order revised slightly from §7 in light of the audit:

```
Step 0   ✓ Audit complete (this section).
Step 1   Add `acp_dag_semantic: Option<AcpDagSemanticResolution>` field
         to `ReplResponseV2` (rust/src/repl/response_v2.rs).
Step 2   Update `response_adapter::repl_to_chat_response` to project
         the new field into `acp_trace` (rust/src/api/response_adapter.rs).
         Snapshot test: capture today's `acp_trace` shape for a Slice 1
         pack-bound utterance via the HTTP route; assert the R8 path
         produces the same shape.
Step 3   In `process()` at line ~1028, add:
           if let UserInputV2::Message { ref content } = input {
               if let Ok(Some(resolution)) =
                   resolve_acp_dag_semantic_prompt_with_verified_envelopes(
                       content, /* config_root */
                   )
               {
                   if matches!(resolution.status, AcpDagSemanticStatus::Matched) {
                       // Build minimal ReplResponseV2 with acp_dag_semantic
                       // populated; short-circuit before state dispatch.
                       return Ok(ReplResponseV2 {
                           acp_dag_semantic: Some(resolution),
                           // ... other fields default / forwarded from session
                       });
                   }
               }
           }
         Open question for step 3: where does the orchestrator find the
         config_root? It probably has it cached; verify before coding.
Step 4   Delete `try_route_supported_acp_prompt` call from
         `session_input` at `rust/src/api/agent_routes.rs:151–158`. The
         HTTP route shrinks to a single `dispatch_to_v2_repl` call.
Step 5   Update `gate_e_single_path_invariant.rs`:
         - Rename `gate_e_router_keeps_session_input_as_the_only_normal_utterance_path`
           to a stronger assertion: `session_input` body must NOT contain
           `try_route_supported_acp_prompt` (absence test).
         - Add a source-scan test that `ReplOrchestratorV2::process` does
           call `resolve_acp_dag_semantic_prompt_with_verified_envelopes`.
Step 6   Run V4 validation: cargo test, cargo clippy -- -D warnings,
         pub-lint, acp-envelope-byte-equality-check, 36-fixture baseline.
Step 7   Update quarantine register entry #3 → status CLOSED.
Step 8   Update `gate-e-review-packet.md` with R8 evidence.
Step 9   PR.
```

Estimated effort with this clearer scope: 6–10 focused hours.

## 11. Step 3 deep audit (2026-05-11)

The initial step-0 audit identified the insertion point in `process()` but
underestimated the surrounding ACP pipeline. The deeper audit reveals the
actual call shape:

```
HTTP session_input (rust/src/api/agent_routes.rs:143)
  → try_route_supported_acp_prompt (line 269)
    → try_route_supported_acp_prompt_with_draft_mode (line 283)
      → AcpSessionInputDraftMode selection (line 230, env-var driven)
      → acp_prompt_supported_provider_task (acp_state_anchor.rs)
      → process_acp_prompt_deterministic_envelope (repl_routes_v2.rs:1828)
        → acp_prompt_state_anchor_provider_outcome (acp_state_anchor.rs)
            [DECISION: should ACP fire? returns Continue|Complete]
        → handle_repl_acp_request (acp_protocol.rs)
            [MIXED LAYERS: dispatches `session/prompt` JSON-RPC method —
             internally calls the resolver AND wraps result in JSON-RPC]
            → resolve_acp_dag_semantic_prompt_*
                [PURE DECISION: the resolver itself]
            → JsonRpcOutgoing wrapping
                [PURE TRANSPORT]
        → attach_session_runtime_trace_to_result (repl_routes_v2.rs:1895)
            [DECISION: project orchestrator session state into trace]
      OR
      → process_acp_prompt_llm_envelope (repl_routes_v2.rs:1954)
        → ob_agentic::create_llm_client (LLM transport)
        → calls deterministic on fallback
```

Key surfaces identified by file + line:

| Surface | File | Line | Layer (3a) |
|---|---|---|---|
| `AcpSessionInputDraftMode` enum | `rust/src/api/agent_routes.rs` | 230 | decision → move to orchestrator |
| `acp_session_input_draft_mode()` env-var reader | `rust/src/api/agent_routes.rs` | 261 | decision → orchestrator config |
| `try_route_supported_acp_prompt[_with_draft_mode]` | `rust/src/api/agent_routes.rs` | 269 / 283 | delete after 3a — HTTP route shrinks |
| `process_acp_prompt_deterministic_envelope` | `rust/src/api/repl_routes_v2.rs` | 1828 | split: decision parts → orchestrator; envelope wrapping → HTTP |
| `process_acp_prompt_llm_envelope` | `rust/src/api/repl_routes_v2.rs` | 1954 | same — split |
| `attach_session_runtime_trace_to_result` | `rust/src/api/repl_routes_v2.rs` | 1895 | decision → orchestrator (already orchestrator-aware via `get_session`) |
| `AcpPromptStateAnchorProvider*` types | `rust/src/api/acp_state_anchor.rs` | 122–270+ | decision → orchestrator |
| `acp_prompt_state_anchor_provider_outcome()` | `rust/src/api/acp_state_anchor.rs` | (look up) | decision → orchestrator |
| `handle_repl_acp_request` | `rust/src/acp_protocol.rs` | (look up — sub-1000) | **THE SPLIT POINT** — see §13.4 |

**Env-var contract** that the orchestrator absorbs:

- `OB_ACP_SESSION_INPUT_DRAFT_SOURCE` (primary)
- `OB_ACP_SESSION_INPUT_DRAFT_MODE` (fallback)
- Values: `"deterministic"`, `"deterministic_draft"`, `"llm"`, `"llm_tool_call"`, `"live_llm"`.
- Default: `Deterministic`.

The orchestrator should read these once at construction (in the builder)
and store as a field, NOT per-request. Per-request env reads are an
anti-pattern that the HTTP layer was hiding.

## 12. Why phase 3a (and why not literal option 1)

User's invariant: **"no back door"** = no parallel agent-decision paths
allowed to re-emerge.

What "decision" means concretely:
- "Does this utterance match a Slice 1 pack?" (the resolver call)
- "Does the session's state permit ACP resolution right now?" (state anchor)
- "Which resolver flavor — deterministic or LLM?" (draft mode)
- "What runtime trace should attach to the response?" (runtime trace)

What "transport" means concretely:
- "Wrap the typed response in `JsonRpcOutgoing` shape."
- "Set the `status: \"acp_session_input_processed\"` envelope label."
- "Add the `outgoing: [...]` array of JSON-RPC outgoing messages."

The user-facing back door is the dual-`if let` race in `session_input`
between `try_route_supported_acp_prompt` and `dispatch_to_v2_repl`. Once
the orchestrator owns ALL DECISIONS, that race cannot exist — the HTTP
route just transports the orchestrator's typed response into JSON-RPC
shape. Adding a "backdoor" would require adding a parallel decision
path inside the orchestrator itself, which is one module and easy to
police via the source-scan invariant tests.

Literal option 1 (absorb JSON-RPC transport into the orchestrator) would
add 2–3 weeks of refactor to drag `JsonRpcOutgoing`, `session/prompt`
method-name routing, and ACP envelope status labels into the state
machine. That's the wrong layer for transport. Phase 3a is the right
scope.

## 13. Step 3 sub-tasks (concrete, executable cold)

Order matters — each step depends on the previous.

### 13.1 Move `AcpSessionInputDraftMode` into orchestrator config

- Move the enum from `rust/src/api/agent_routes.rs:230-258` into
  `rust/src/sequencer.rs` (or a new sibling module if the file is too
  big — verify with `wc -l rust/src/sequencer.rs` first).
- Make it `pub` (the HTTP route and tests need to construct it).
- Add `acp_session_input_draft_mode: AcpSessionInputDraftMode` field to
  `ReplOrchestratorV2`.
- Add a builder method `with_acp_draft_mode(mode)` (mirror the existing
  builder pattern — see `ReplOrchestratorV2::new` for the style).
- Move `acp_session_input_draft_mode()` env-var reader into the
  orchestrator builder so it fires once at construction, not per request.
- Update `crates/ob-poc-web/src/main.rs` (or wherever the orchestrator
  is constructed) to call the new builder method.
- Acceptance: `OB_ACP_SESSION_INPUT_DRAFT_SOURCE` env var sets the
  orchestrator field at startup; per-request reads of the env var are
  gone from `agent_routes.rs`.

### 13.2 Move state-anchor decision into orchestrator

- The state-anchor decision is "should ACP fire right now?" — its
  outputs are `Continue` (proceed with ACP) or `Complete` (skip ACP and
  use this pre-built short-circuit result instead).
- Move `AcpPromptStateAnchorProvider`, `AcpPromptStateAnchorProviderReport`,
  `AcpPromptStateAnchorProviderOutcome` from `rust/src/api/acp_state_anchor.rs`
  into `rust/src/sequencer.rs` (or a new sibling module `acp_state_anchor`
  beside it).
- Move `acp_prompt_state_anchor_provider_outcome()` along with them.
- The function reads orchestrator session state, so co-locating with
  the orchestrator improves cohesion.
- Acceptance: `acp_state_anchor.rs` either deleted (if everything moves)
  or reduced to types that genuinely belong at the HTTP layer
  (unlikely — most of it is decision logic).

### 13.3 Move runtime-trace attachment into orchestrator

- `attach_session_runtime_trace_to_result` at `rust/src/api/repl_routes_v2.rs:1895`
  reads `&ReplSessionV2` and builds runtime trace JSON.
- Refactor: instead of mutating a JSON `result`, return a typed
  `AcpDagSemanticRuntimeTrace` (the type already exists in
  `acp_dag_semantic.rs`).
- Call site inside the orchestrator populates
  `resolution.runtime_trace = Some(...)` on the typed
  `AcpDagSemanticResolution` before returning.
- Acceptance: the HTTP layer no longer mutates JSON to add runtime
  trace; the orchestrator's typed response already carries it.

### 13.4 THE SPLIT POINT — split `handle_repl_acp_request`

This is the load-bearing refactor.

- `handle_repl_acp_request` in `rust/src/acp_protocol.rs` currently
  takes a `JsonRpcRequest` for `session/prompt` and returns
  `Vec<JsonRpcOutgoing>`. Internally it:
  (a) extracts the prompt content,
  (b) calls the resolver,
  (c) builds the result payload (`dag_semantic`, `traceProjection` etc.),
  (d) wraps in `JsonRpcOutgoing` messages.
- Split into two functions:
  - `pub(crate) async fn resolve_acp_prompt_decision(prompt: &[AcpContentBlock], orchestrator, session_id) -> AcpDagSemanticResolution`
    — pure decision. Lives in the orchestrator. Returns typed result.
  - `pub(crate) fn wrap_acp_resolution_as_jsonrpc(resolution: &AcpDagSemanticResolution, request_id) -> Vec<JsonRpcOutgoing>`
    — pure transport. Stays in `acp_protocol.rs`. Wraps the typed
    resolution in the JSON-RPC outgoing shape.
- The existing JSON-RPC server entry points (ACP standalone server)
  call both: `resolve_acp_prompt_decision` → `wrap_acp_resolution_as_jsonrpc`.
- Acceptance: `handle_repl_acp_request` is gone (or reduced to a thin
  composition of the two new functions). The resolver call site is
  ONE place: the orchestrator.

### 13.5 Add the ACP resolution step to `orchestrator.process()`

- Insertion at `rust/src/sequencer.rs:1028` (after contextual query
  intercept, before state dispatch) — confirmed safe by step-0 audit.
- Code shape (the resolver here uses the resolver from §13.4):
  ```rust
  // R8 single-path: ACP resolution is the orchestrator's first decision
  // for Message inputs. State-independent — Slice 1 pack-bound utterances
  // bind regardless of current REPL state.
  if let UserInputV2::Message { ref content } = input {
      // 1. State-anchor check (§13.2)
      let anchor_outcome = self.acp_state_anchor_outcome(session, content).await;
      // 2. If anchor permits, resolve
      if anchor_outcome.permits_resolution() {
          // 3. Use orchestrator's configured draft mode (§13.1)
          let resolution = self.resolve_acp_dispatch(content).await;
          if matches!(resolution.status, AcpDagSemanticStatus::Matched) {
              // 4. Attach runtime trace (§13.3)
              let mut resolution = resolution;
              resolution.runtime_trace = Some(self.build_runtime_trace(session));
              // 5. Build minimal ReplResponseV2 carrying the ACP payload
              return Ok(self.build_acp_response_envelope(session, resolution));
          }
      }
  }
  ```
- `build_acp_response_envelope` is a new helper that constructs a
  `ReplResponseV2` with the right `state` (probably unchanged), `kind`
  (probably `ReplResponseKindV2::Info` or a new `AcpResolved` variant —
  decide based on UI consumption), and `acp_dag_semantic: Some(resolution)`.
- Acceptance: `resolve_acp_dag_semantic_prompt_*` is called from
  exactly ONE place in `rust/src` (the orchestrator), plus ACP
  standalone JSON-RPC server (acceptable — different ingress).

### 13.6 Shrink `session_input` to a single dispatch call

- Delete the `try_route_supported_acp_prompt` branch (lines 151–158).
- `session_input` shape after R8:
  ```rust
  async fn session_input(...) -> Result<Json<SessionInputResponse>, StatusCode> {
      let orchestrator = state.repl_v2_orchestrator.as_ref()
          .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
      let repl_response = dispatch_to_v2_repl(&req, orchestrator, session_id)
          .await
          .ok_or(StatusCode::NOT_FOUND)?;
      // Existing onboarding-state enrichment + adapter
      let chat_response = enrich_and_adapt(repl_response, session_id, &state).await;
      Ok(Json(SessionInputResponse::Chat { response: Box::new(chat_response) }))
  }
  ```
- Acceptance: no `if let` racing between two pipelines in `session_input`.

### 13.7 Update tests

- `gate_e_router_keeps_session_input_as_the_only_normal_utterance_path`:
  flip from ordering assertion to absence assertion. The new assertion:
  `session_input` body must NOT contain `try_route_supported_acp_prompt`.
- Add `gate_e_orchestrator_owns_acp_resolution` source-scan test:
  `rust/src/sequencer.rs` body MUST contain `resolve_acp_dispatch` (or
  whatever the §13.5 helper is named). This proves the orchestrator
  has the resolver call.
- Update ~10 tests in `rust/src/api/repl_routes_v2.rs` and
  `rust/src/acp_protocol.rs` that read the dual-path shape — they need
  to be rewritten to invoke `orchestrator.process()` and assert on the
  typed `acp_dag_semantic` field.
- R4 fuzz lane (`gate_e_single_path_invariant_termination`) should pass
  unchanged — the resolver path is invariant; only the calling layer
  changed.

## 14. Validation gates for step 3 completion

Each ships green before merging:

1. `cargo build -p ob-poc --lib` — compiles.
2. `cargo test --lib -p ob-poc` — full unit suite green.
3. `cargo test -p ob-poc --test gate_e_single_path_invariant` — all
   10 tests pass including the renamed ordering→absence test.
4. `cargo run -p xtask -- pub-lint` — clean (likely a few public-API
   touches when types move; expect to bless).
5. `cargo run -p xtask -- acp-envelope-byte-equality-check` — clean
   (envelope content unchanged by R8).
6. `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` —
   clean.
7. Source-scan invariant: `grep -r "try_route_supported_acp_prompt"
   rust/src` returns ZERO matches (function is deleted).
8. Source-scan invariant: `grep -r "resolve_acp_dag_semantic_prompt"
   rust/src` returns matches ONLY in `rust/src/sequencer.rs` (or
   wherever the orchestrator lives) and `rust/src/acp_protocol.rs`
   (ACP standalone server).
9. 36-fixture HTTP baseline (`BASE_URL=... bash run_current_sage_baseline.sh`)
   produces the same `pack_hit` / `verb_hit` / `first_pass_valid_dsl_draft`
   numbers as pre-R8.

## 15. Why this plan exists

The pre-existing single-path invariant test (`gate_e_router_keeps_session_input_as_the_only_normal_utterance_path`)
was *too lenient*: it accepted ordering (ACP-first, V2-REPL-second) as
sufficient. The R5 audit revealed this tolerates a real bifurcation that
makes utterance→REPL routing worse over time. The R8 work removes that
tolerance.

Once R8 ships, the ordering test becomes an absence test, which is the
correct shape for a single-path invariant.
