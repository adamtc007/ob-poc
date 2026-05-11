# Slice 2 Real Runtime Source Integration Plan

Status: accepted for Slice 2; direct database-backed sources deferred to a separate Slice 3/runtime-source-adapter plan.

Date: 2026-05-11

## Decision

Keep `src/acp_runtime_context.rs` transport-neutral. It should continue to accept an already scoped `AcpRuntimeContextSource` and produce a redacted deterministic projection.

Add real source collection outside that module, at the session-input boundary where the request has:

- `session_id`
- selected pack and verb/template
- verified static envelope hash
- current `ReplSessionV2` snapshot from `ReplOrchestratorV2::get_session`
- current workflow plan, missing bindings, and pending decision state

No database-wide or raw discovery read should enter Slice 2 until a named source adapter and fixture group exists for it.

## Proposed Code Shape

1. Add a source builder near the DAG semantic/session-input boundary:
   - candidate module: `src/acp_runtime_context_sources.rs`
   - input: `AcpRuntimeContextBuildInput`
   - output: `AcpRuntimeContextSource`
2. Extend DAG semantic envelope attachment to accept an optional scoped runtime source:
   - current behavior remains fallback/synthetic for tests and ACP protocol-only calls
   - session input can pass the real request-scoped source after loading `ReplSessionV2`
3. Keep trace fields unchanged:
   - `runtime_hash`
   - `projection_hash`
   - `static_envelope_hash`
   - `snapshot_id`
   - redaction and freshness policy ids

## Initial Source Mapping

| Pack | Real source | Allowed fields |
| --- | --- | --- |
| `onboarding-request` | `ReplSessionV2.runbook_plan`, `runbook_plan_cursor`, pending decision/bindings | `workbook_step_statuses`, `run_sheet_cursor`, `missing_binding_codes`, `blocker_codes`, `fsm_state` |
| `cbu-maintenance` | `ReplSessionV2.bindings`, `cbu_ids`, entity scope, pending required args | `cbu_id`, `product_binding_ids`, `binding_status`, `missing_binding_codes` |
| `product-service-taxonomy` | selected verb, pending bindings, count-only discovery summaries already in session context | `active_srdef_count`, `discovered_srdef_ids`, `missing_resource_codes`, `missing_resource_count`, `blocker_codes` |

Any label, free text, raw payload, document body, SQL row, provider config, URL, or commercial term remains blocked by `slice2_runtime_context_redaction_v1`.

## Snapshot Rule

Use a single request-scoped session snapshot:

1. Load `ReplSessionV2` once at the start of ACP session-input routing.
2. Derive a deterministic `snapshot_id` from:
   - `session_id`
   - `session.updated_at` or `last_active_at`
   - selected pack
   - selected verb/template
   - static envelope hash
3. Build source fields only from that loaded session value.
4. Do not re-read session state while building the response.

If the session is missing, stale, or cannot be loaded, emit a verified runtime projection with `missing_source_codes` and no guessed fields.

## Implementation Order

1. [x] Add the source builder module with unit tests over synthetic `ReplSessionV2` values.
2. [x] Wire session-input DAG semantic routing to load one `ReplSessionV2` snapshot before runtime projection.
3. [x] Add a real-source fixture subset that asserts:
   - current synthetic fields remain present
   - session-derived source refs include only ids/hashes
   - forbidden labels and free text remain absent
4. [x] Extend `gate_e_single_path_invariant` only if the route changes.
5. [x] Rerun focused validation:
   - `cargo check`
   - `cargo fmt --check`
   - `cargo clippy --lib -- -D warnings`
   - `cargo clippy --test gate_e_single_path_invariant -- -D warnings`
   - `cargo test acp_runtime_context_sources::tests -- --nocapture`
   - `cargo test --test gate_e_single_path_invariant -- --nocapture`
   - Slice 2 HTTP fixture runner

## Stop Conditions

Do not proceed to direct database-backed runtime reads until the session-derived source builder is green and reviewed.

Do not add new public APIs unless `xtask pub-lint` is updated intentionally.

Do not alter runtime trace field names; consumers already rely on the Slice 2 trace contract.
