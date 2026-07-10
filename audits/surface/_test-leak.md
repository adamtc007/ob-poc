# Test-driven pub leakage — read-only proxy scan

**Methodology (read-only proxy, NOT a full mutate-and-revert compile-probe):** for each crate
with a `tests/` directory, we (1) grepped `crates/<name>/tests/*.rs` for `use <crate_ident>...`
and inline `<crate_ident>::` references to see what public symbols the external harness pulls
in, (2) prioritized test-flavored names (Mock/Stub/Fake/Test/Dummy/fixture/harness/scaffold,
`make_*`/`build_*`/`create_test_*`-shaped helpers), and (3) grepped the rest of the workspace
(`rust/crates/`, `rust/src/`, `rust/xtask/`), excluding that crate's own `tests/` dir, for the
same symbol name to check for a real non-test consumer. A symbol with zero hits outside its
own definition site + its own `tests/*.rs` is flagged as a finding. This is a **grep-based
proxy**, not the validated mutate-narrow-then-`cargo build --tests` probe used earlier this
session on `dsl-sage::MockLlmClient` — confidence is high but not compile-verified for every
row below; a couple of ambiguous cases were spot-checked with `Read` only, not a build. Treat
this as a prioritized worklist for the mutate-and-confirm pass, not a final signed-off list.

HEAD at time of scan: `codex/phase-1-5-governance-closure` branch, 2026-07-01.

---

| Crate | Finding | Suggested fix |
|---|---|---|
| `bpmn-controller` | None found — `K8sClient`, `provision_pool`, `deprovision_pool`, `pool_status`, `instance_status`, `list_tenant_instances`, `start_instance` all consumed by `rust/src/domain_ops/bpmn_controller_ops.rs` (production). | No action. |
| `bpmn-runtime` | None found — `apply_merge_protocol`, `MergeResult`, `WriteLogEntry`, `ActiveToken`, `InMemoryJourneyStore`, `JourneyStore`, `PostgresJourneyStore` used in crate's own `src/` plus real downstream consumers (`ob-poc-web/src/process_registry.rs`, `bpmn-test-harness/src/lib.rs`, `dsl-migrate-verify/src/lib.rs`). | No action. |
| `bpmn-test-harness` | None found — `Scenario`, `instantiate_pack`, `compile_dsl`, `dsl_resolution` re-export defined/used in its own `src/lib.rs`, and consumed by `dsl-migrate-verify` as a real downstream crate. Whole-crate purpose is test infra as a library, as designed. | No action. |
| `dsl-bpmn-frontend` | None found — `assemble` used widely across `dsl-resolution`, `ob-poc-boundary`, `dsl-sage`, `dsl-render`, `rust/src/{plan_builder,runbook,agent,api,sem_reg}`. | No action. |
| `dsl-lsp` | None found — `analyze_document` used in crate's own `src/server.rs`, `src/handlers/diagnostics.rs`, `src/handlers/mod.rs` (production LSP handlers). | No action. |
| `dsl-migrate` | None found — no `use` imports in tests, only fully-qualified calls to core public API (`parse_bpmn_xml`, `emit`, `MigrationStatus`); nothing test-flavored. | No action. |
| `dsl-migrate-verify` | None found — `verify_dsl_source` defined and consumed within its own `src/lib.rs`; crate's purpose is standalone verification tooling. | No action. |
| `dsl-render` | None found — only inline calls to core public API (`render`, `render_dsl`, `RenderOptions`), no test-flavored names. | No action. |
| `dsl-resolution` | None found — `PackRegistry`, `validate_bpmn`, `load_packs_from_dir`, `resolve` heavily consumed by `dsl-sage`, `ob-poc-web`, `ob-poc-boundary`, `ob-poc-agent`, `ob-poc-journey`, `rust/src/dsl_v2`, `rust/src/journey`, `rust/xtask/{verbs,reconcile}`. | No action. |
| `dsl-runtime` | **Hit.** `ScenarioRunner` + `LiveScenarioRunner` (`crates/dsl-runtime/src/cross_workspace/test_harness/{runner.rs,live.rs}`, re-exported `src/lib.rs:149` + `test_harness/mod.rs:49,51`), plus mocks `MockChildEntityResolver`/`MockPredicateResolver`/`MockSlotStateProvider` (`test_harness/mocks.rs`) — referenced only inside the `test_harness` module itself and consumed only by `crates/dsl-runtime/tests/cross_workspace_dag_scenarios.rs` + `cross_workspace_dag_live_scenarios.rs`. No other crate depends on `dsl_runtime::test_harness`. | Gate the whole `cross_workspace::test_harness` module (and its `pub use` in `lib.rs:149`) behind `#[cfg(any(test, feature = "test-util"))]` / a `test-util` feature. |
| `dsl-sage` | Already investigated earlier this session (skip per task instructions) — `MockLlmClient`, confirmed via full mutate-and-revert probe, only consumed by `dsl-sage/tests/pack_matching_eval.rs`. | (Prior finding — fix TBD in later pass.) |
| `dsl-semos-frontend` | **Hit.** `load_verbs_from_dsl_dir` (`crates/dsl-semos-frontend/src/loader.rs:37`, re-exported via `pub mod loader` in `src/lib.rs`). The entire crate has **zero dependents** anywhere in the workspace (only appears in root `Cargo.toml` member list) — sole consumer is its own `tests/round_trip.rs`. | Narrow to `pub(crate)` (or move check into internal `#[cfg(test)]`) unless deliberately a not-yet-wired Tranche-3 entry point — flag to owner; crate currently has zero production callers at all. |
| `governed_query_proc` | None found — `tests/ui/` directory exists but is **empty** (no `.rs` files present). | No action. |
| `inspector-projection` | None found — `validate`, `InspectorProjection`, `Node`, `NodeId`, `NodeKind`, `Provenance`, `RefOrList`, `RefValue`, `RenderPolicy`, `ValidationError` all have real production consumers, notably `rust/src/api/graph_routes.rs`. | No action. |
| `ob-poc-bus-handler` | None found — `NoopResultDispatcher`, `ObPocBusHandler`, `VerbExecutor`, `VerbExecutorError`, `VerbOutcome` all consumed by `rust/crates/ob-poc-web/src/bus_runtime.rs`. | No action. |
| `ob-poc-kyc-seam` | None found — `append_in_scope`, `map_principal`, `IntentEventDraft` all consumed by `rust/src/domain_ops/kyc_stream_ops.rs` (and `ob-poc-kyc-store`). | No action. |
| `ob-poc-kyc-store` | **Hit.** `publish_manifest` + `ManifestPublishOutcome` (`src/manifest.rs`), `PgKycProjector` + `PgKycProjectionDrainer` + `CONTROL_EDGE_PROJECTION_EFFECT` + `PgKycObligationProjector` + `PgKycObligationDrainer` + `OBLIGATION_PROJECTION_EFFECT` (`src/projection.rs`) — all `pub`, re-exported via `lib.rs`, consumed only by `tests/manifest.rs`, `tests/drainer.rs`, `tests/projection.rs`, `tests/exit_criteria.rs`. Zero consumers in `rust/src`, `ob-poc-web`, or `xtask` — described in CLAUDE.md as W6 "disposable stream projections" (K-34) but not yet wired into any running binary. | Narrow to `pub(crate)` + expose via `#[cfg(any(test, feature = "test-util"))]` re-export for the harness — OR if wiring into `ob-poc-web` startup (drainer background task) is imminent, track as a known production-gap, not a pure leak. |
| `ob-poc-kyc-substrate` | **Hit.** `RecoveryPin` + `DeterminationInProgress` + `recover_determination_at` (`src/determination.rs`, re-exported `lib.rs:24-26`) — consumed only by `tests/kyc_slice.rs` (exit-criterion 5, replay determinism). No hit in `rust/src/domain_ops/kyc_stream_ops.rs` or elsewhere. | Narrow to `pub(crate)`, move replay-determinism proof to internal `#[cfg(test)]`, or gate behind `test-util` feature — unless W1 replay/recovery is about to be wired into production (K-1 traceability), in which case note as an unimplemented-consumer gap rather than a pure leak. |
| `ob-poc-macros` | None found (expected for proc-macro crate) — `compile_tests.rs` drives `trybuild` against `#[derive(IdType)]`, the macro's actual public contract. | No action. |
| `ob-poc-manifest-export` | No `tests/*.rs` files at all — `tests/` contains only a `fixtures/` subdirectory. | N/A — no leakage possible. |
| `ob-poc-taxonomy` | None found for leakage — no top-level `tests/*.rs` files, only `tests/support/{mod.rs,semtaxonomy_seed.rs}` (`pub(crate)`-scoped) which is not referenced by any test binary (orphaned scaffolding). | No action for leakage; separately flag `tests/support/` as unused dead code (out of scope here). |
| `sem_os_obpoc_adapter` | None found — `tests/` contains only `fixtures/*.toml` (no `.rs`). The `integration_tests` module lives under `src/integration_tests/` (internal, not an external `tests/` dir), so out of scope by definition. | No action. |
| `sem_os_server` | None found — `build_router` and `JwtConfig` both used by `src/main.rs` (production entrypoint). | No action. |
| `ob-poc` (`rust/tests/` external harness, 31 files) | **Inverse finding (over-narrowing, not leakage):** `StubExecutor`, defined `pub(crate)` at `rust/src/sequencer.rs:136`, is imported by `rust/tests/bpmn_integration_test.rs:770` and `rust/tests/bpmn_e2e_harness_test.rs:53` (`use ob_poc::sequencer::{DslExecutionOutcome, DslExecutorV2, StubExecutor};`). These two `--features database` harnesses currently **fail to compile** (cached rustc diagnostic: `error[E0603]: struct 'StubExecutor' is private`) — silent because both are DB-gated and excluded from normal `cargo x pre-commit`/`cargo test --lib` runs. Likely introduced when `StubExecutor` was tightened to `pub(crate)` during the Tier-A pub-surface cleanup without checking these two harnesses. No other pure test-driven-pub-leakage found across all 31 files — all other `ob_poc::`-qualified symbols pulled by the harnesses (`sage::*`, `bpmn_integration::*`, `repl::runbook::*`, `repl::types_v2::*`, `sequencer::{DslExecutionOutcome,DslExecutorV2,ReplOrchestratorV2}`, `sequencer_tx::PgTransactionScope`, `journey::router::PackRouter`, `sem_reg::types::ObjectType`, `services::ObPocAttributeService`, `domain_ops::kyc_stream_ops::*`) have confirmed production consumers in `rust/src/` or `ob-poc-web`. No test-support-shaped `pub mod` at crate root. | Restore `pub` on `StubExecutor`/`ParkableStubExecutor` in `sequencer.rs` (both are trivial test doubles, legitimate to expose to external test crates) — OR duplicate a local stub executor inside each of the two test files so `sequencer.rs` stays `pub(crate)`-clean. This is a **compile-break to fix**, not a leak to close. |

## Not covered / needs separate note

- `dsl-sage` explicitly skipped per task instructions (already closed out earlier this session: `MockLlmClient`, confirmed via full mutate-and-revert probe against `dsl-sage/tests/pack_matching_eval.rs`).

## Summary counts

- Crates scanned: 22 (of the ~23 listed, `dsl-sage` skipped as already done) + `ob-poc` external harness = 23 total entries.
- Genuine test-driven-pub-leakage hits: **4** (`dsl-runtime` test_harness module, `dsl-semos-frontend::load_verbs_from_dsl_dir`, `ob-poc-kyc-store` manifest/projection surface, `ob-poc-kyc-substrate` recovery surface).
- Inverse finding (over-narrowed, breaks compile): **1** (`ob-poc::sequencer::StubExecutor`).
- Crates with no `tests/*.rs` content at all (fixtures-only or empty): `ob-poc-manifest-export`, `sem_os_obpoc_adapter`, `governed_query_proc` (empty `ui/` dir).
- Clean (no findings): 16 crates.
