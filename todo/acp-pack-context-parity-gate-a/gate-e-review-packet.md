# Gate E Review Packet

Status: ready for peer review.

Date: 2026-05-10

## Decision Requested

Approve Gate E as complete for the current Slice 1 static context scope and allow Slice 2 runtime context planning to start.

This review does not approve Slice 2 production implementation.

## Scope Accepted

Slice 1 covered static context for:

- `onboarding-request`
- `cbu-maintenance`
- `product-service-taxonomy`

Accepted surfaces:

- pack projection
- verb contract projection
- workbook/template projection
- registry-grade macro classification
- execution-policy projection
- envelope v2 deterministic registry state
- verified envelope-backed ACP routing trace
- HTTP session trace projection
- ghost-route refusal behavior
- public API allowlist for current ACP boundary

## Evidence

| Evidence | File or command | Result |
| --- | --- | --- |
| Static context acceptance | `cargo test acp_static_context_acceptance -- --nocapture` | passed, 3 tests |
| Policy metadata projection | `cargo test slice1_projection_includes_policy_metadata -- --nocapture` | passed |
| ACP protocol prompt lane | `cargo test session_prompt_ -- --nocapture` | passed, 17 tests |
| HTTP envelope trace persistence | `cargo test test_acp_gateway_prompt_persists_dag_semantic_envelope_trace -- --nocapture` | passed |
| ACP-before-REPL ordering | `cargo test test_generic_dag_prompt_routes_through_acp_before_repl_on_normal_input -- --nocapture` | passed |
| Single-path invariant lane | `cargo test --test gate_e_single_path_invariant -- --nocapture` | passed, 3 tests |
| Public API allowlist | `cargo run -p xtask -- pub-lint` | passed, 164 checked items across 5 ACP boundary files |
| Formatting checkpoint | `cargo fmt --check` | passed after the single-path invariant lane was added |
| W1 envelope-driven baseline | `BASE_URL=http://127.0.0.1:3002 bash run_current_sage_baseline.sh` | captured all 36 fixtures |

## Baseline Result

Source run:

```text
baseline-runs/current-sage-20260510T200520Z
```

Aggregate result:

| Metric | Result |
| --- | --- |
| `pack_hit` | 31/36 |
| `verb_hit` | 31/31 |
| draft-expected first-pass DSL | 5/5 |
| invented verbs | 0 |
| invented macros | 0 |
| prose-only failures | 0 |
| refusal quality | 10/10 |
| registry verified | 36/36 |
| envelope verified | 31/36 |

The five non-envelope cases are expected ghost-route refusals with no selected pack:

- F021
- F022
- F023
- F024
- F035

## Bypass Result

`gate-e-bypass-proof.md` records:

- all five ghost-route fixtures return structured refusal through `POST /api/session/:id/input`
- no ghost-route fixture emits DSL
- no ghost-route fixture permits mutation
- direct normal-payload use of `POST /api/session/:id/execute` returns `410 Gone`

`tests/gate_e_single_path_invariant.rs` now pins this continuously by checking:

- `/api/session/:id/input` remains the normal utterance route
- legacy `/api/session/:id/chat` is not registered
- `/api/session/:id/execute` is guarded by `execute_session_dsl_legacy_raw_only`
- ACP semantic routing runs before generic REPL fallback
- generated ghost-route bait produces structured refusal, no DSL, and no mutation permission

## Residuals

The following item remains open by decision, not as a Gate E blocker:

- decide whether to expand public API allowlist enforcement beyond the ACP boundary and freeze historical root-crate public surface now

That decision belongs to crate-discipline review. Current Gate E public-surface control is limited to the ACP boundary already enforced by `xtask pub-lint`.

## Review Questions

Reviewers should approve or challenge:

1. Whether the Slice 1 static context scope is sufficient to close Gate E.
2. Whether the five ghost-route refusals should remain no-pack envelope cases or receive a synthetic policy envelope in a future slice.
3. Whether `tests/gate_e_single_path_invariant.rs` is acceptable as regular CI coverage for the single-path invariant.
4. Whether broader public API freeze work is required before Slice 2 planning starts.
5. Whether Slice 2 may begin as planning-only work under `slice-2-runtime-context-planning.md`.

## Proposed Decision

Approve Gate E for current Slice 1 scope.

Start Slice 2 runtime context planning only. Do not start Slice 2 production runtime projection, resolver wiring, or envelope schema changes until the Slice 2 plan receives peer review.
