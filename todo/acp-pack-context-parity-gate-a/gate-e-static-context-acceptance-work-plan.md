# Gate E Static Context Acceptance Work Plan

Status: Gate E static context acceptance is complete for the current Slice 1 scope. Static context acceptance is implemented and passing for the frozen Slice 1 fixture set, envelope-driven Sage has been rerun against W1, DAG semantic HTTP trace emission is enforced, bypass proof is recorded, and the continuous single-path invariant lane is in the regular integration test set.

Completed in this slice:

1. [x] Added `acp_static_context_acceptance` as the first Gate E executable harness.
2. [x] Verified the Slice 1 pack set is exactly `onboarding-request`, `cbu-maintenance`, and `product-service-taxonomy`.
3. [x] Verified the diagnostic taxonomy covers ambiguous pack, forbidden verb, legacy route bait, missing binding, and unsupported macro tier.
4. [x] Verified development online registry state loads through signed active envelope verification.
5. [x] Verified every active envelope carries v2 schema, active lifecycle, source projection hash, required section hashes, content hash chain, no budget omissions, and projection-fidelity checks for verb bindings, verb effects, and macro tiers.
6. [x] Verified all 36 frozen baseline fixtures have static pack, verb/macro, template/workflow, pending-question, refusal, and ghost-route refusal coverage.
7. [x] Fixed the F027 static policy gap by treating `outbox_write`, three-axis state transitions, emitting effects, and confirmation-tier consequence metadata as gated execution-policy surfaces.
8. [x] Added regression coverage for `onboarding.dispatch-ready-slices` so the owner/HITL refusal surface remains projected.
9. [x] Added initial CI/pre-commit public API allowlist enforcement for the root crate ACP boundary through `cargo run -p xtask -- pub-lint`.
10. [x] Added envelope-backed ACP DAG semantic resolution for Slice 1. The ACP protocol prompt route now loads verified active envelope registry state, emits registry trace refs for all DAG semantic responses, emits envelope trace refs for Slice 1 pack responses, and leaves non-Slice pack routing intact.
11. [x] Enforced persisted HTTP REPL/session trace emission for DAG semantic HTTP responses. Gateway and session-input responses now carry/persist registry verification, envelope hash, pack id, projection hash, selected verb, selected template, and selected macro fields where present.
12. [x] Re-ran the frozen 36-fixture W1 baseline against envelope-driven Sage through `POST /api/session/:id/input`.
13. [x] Compared envelope-driven metrics to the frozen Slice 1 acceptance threshold.
14. [x] Proved the ghost-route fixtures do not bypass into execution: all five bait utterances return structured refusal/no DSL through `/input`, and direct `/execute` returns `410 Gone`.
15. [x] Added `tests/gate_e_single_path_invariant.rs` to pin the production router shape, legacy execute 410 guard, ACP-before-REPL ordering, and generated ghost-route bait refusals.

Current verification checkpoint:

- `cargo check` passed after each code edit.
- `cargo test acp_static_context_acceptance -- --nocapture` passed with 3 tests.
- `cargo test slice1_projection_includes_policy_metadata -- --nocapture` passed.
- `cargo run -p xtask -- pub-lint` passed with 161 checked items across 5 ACP boundary files.
- `cargo test session_prompt_ -- --nocapture` passed with 17 protocol/session prompt tests, including Slice 1 envelope trace assertions and non-Slice pack fallback.
- `cargo run -p xtask -- pub-lint` passed with 164 checked items across 5 ACP boundary files after the verified resolver public API was blessed.
- `cargo test test_acp_gateway_prompt_persists_dag_semantic_envelope_trace -- --nocapture` passed.
- `cargo test test_generic_dag_prompt_routes_through_acp_before_repl_on_normal_input -- --nocapture` passed.
- `SERVER_PORT=3002 cargo run -p ob-poc-web` started the local baseline server.
- `BASE_URL=http://127.0.0.1:3002 bash run_current_sage_baseline.sh` captured all 36 fixtures at `baseline-runs/current-sage-20260510T200520Z`.
- Envelope-driven W1 score: `pack_hit` 31/36, `verb_hit` 31/31, draft-expected first-pass DSL 5/5, invented verbs/macros 0, prose-only failures 0, refusal quality 10/10, registry verified 36/36, envelope verified 31/36.
- `gate-e-bypass-proof.md` records ghost-route refusal evidence plus the direct `/execute` `410 Gone` probe.
- `cargo fmt --check` passed after the single-path invariant lane was added.
- `cargo test --test gate_e_single_path_invariant -- --nocapture` passed with 3 tests.
- Deterministic envelope artifact evidence was refreshed in `gate-d-envelope-v2-work-plan.md` because the projection hash changed after the policy fix.

Gate E remaining work:

1. [x] Wire the ACP protocol DAG semantic prompt path so Slice 1 pack responses consume verified active envelope registry state and emit envelope hashes.
2. [x] Re-run the W1 baseline fixture set against envelope-driven Sage.
3. [x] Compare envelope-driven metrics to the frozen Slice 1 acceptance threshold.
4. [x] Enforce persisted REPL/session trace emission for DAG semantic HTTP REPL responses, including envelope hash, pack id, projection hash, and selected verb/macro/template where present.
5. [x] Prove no direct utterance-to-execution bypass remains for the ghost-route fixtures.
6. [x] Add the continuous single-path invariant/fuzz property test to CI or a documented nightly lane.
7. [ ] Decide whether to expand public API allowlist enforcement beyond the ACP boundary; this is a review-scope decision, not a Gate E blocker for the current Slice 1 acceptance.

Gate E status:

- Static context acceptance: complete for current Slice 1 scope.
- Envelope-driven ACP protocol prompt acceptance: implemented for Slice 1 DAG semantic responses.
- Envelope-driven HTTP REPL acceptance: implemented for DAG semantic HTTP responses.
- Envelope-driven W1 metric acceptance: met the frozen Slice 1 threshold.
- Full Gate E: complete for current Slice 1 scope; only the optional broader public API freeze decision remains.
