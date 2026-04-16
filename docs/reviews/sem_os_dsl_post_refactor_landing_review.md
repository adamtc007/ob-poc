# Sem OS + DSL Post-Refactor Landing Review

## 1. Executive summary

The Sem OS refactor materially improved capability boundaries, but it did not fully complete them. The strongest gains are in `sem_os_server` and `sem_os_harness`: `sem_os_server` now exposes a small crate-root embedding surface in `rust/crates/sem_os_server/src/lib.rs`, and `sem_os_harness` no longer exports its support modules in production. The weakest remaining area is `sem_os_core`, which is still a very broad public module tree in `rust/crates/sem_os_core/src/lib.rs`, and the wider workspace still reaches directly into `sem_os_core` and `sem_os_postgres` from root tests and `ob-poc` integration code.

The DSL refactor materially improved both the tooling edge and the runtime/public-shape boundary. `dsl-lsp` is now facade-shaped at `rust/crates/dsl-lsp/src/lib.rs`, with internal `analysis`, `handlers`, and `server` modules hidden. `rust/src/dsl_v2/mod.rs` now centers its public contract on stage seams (`syntax`, `planning`, `execution`, `tooling`) rather than exposing the old hotspot module trees directly. The broader DSL capability is still not minimal, but it is now much more capability-shaped than it was before the refactor.

Overall, the refactor made the codebase easier to reason about by capability at the crate edge, especially for `sem_os_server`, `sem_os_harness`, and `dsl-lsp`. It did not yet make the full workspace easy to reason about by capability because the root `ob-poc` crate still exposes and consumes wide internal DSL and Sem OS surfaces.

Top 5 remaining architectural concerns:

1. `sem_os_core` is still too broad to serve as a deliberate external contract; it is both domain kernel and large public namespace.
2. `sem_os_client` and `sem_os_server` are no longer fully aligned: `sem_os_client::HttpClient` still implements `/tools/*`, while `sem_os_server` intentionally does not route those endpoints in `rust/crates/sem_os_server/src/router.rs`.
3. `sem_os_obpoc_adapter` remains a real leakage point because `ob-poc` still imports `scanner` and `seeds` helpers directly, for example in `rust/src/dsl_v2/macros/attribute_seed.rs` and `rust/src/sem_reg/scanner.rs`.
4. `rust/src/dsl_v2/mod.rs` still has a larger-than-ideal root facade because macro/expansion families and selected `dsl-core` support types remain root-visible.
5. Workspace-level tests are better aligned, but several still live at `rust/tests` even when they validate internal DSL behavior that would fit better inside owning modules.

## 2. Review scope and method

### Crates and module groups inspected

Sem OS:

- `rust/crates/sem_os_core`
- `rust/crates/sem_os_client`
- `rust/crates/sem_os_server`
- `rust/crates/sem_os_postgres`
- `rust/crates/sem_os_harness`
- `rust/crates/sem_os_obpoc_adapter`
- root/workspace Sem OS-adjacent modules including `rust/src/sem_reg`, `rust/src/sem_os_runtime`, `rust/src/api`, `rust/src/mcp`, and `rust/crates/ob-poc-web/src/main.rs`

DSL:

- `rust/crates/dsl-core`
- `rust/crates/dsl-lsp`
- `rust/crates/playbook-core`
- `rust/crates/playbook-lower`
- `rust/src/dsl_v2`
- `rust/src/runbook`
- `rust/src/bpmn_integration`
- selected workspace consumers and tests

### Commands run

- `env RUSTC_WRAPPER= cargo check --workspace`
- `env RUSTC_WRAPPER= cargo test -p sem_os_core --test discovery_pipeline`
- `env RUSTC_WRAPPER= cargo test -p dsl-lsp --test parser_conformance`
- repository searches for import patterns and test locations via `rg` and `find`

### Tests run

Observed test runs:

- `sem_os_core` targeted test `discovery_pipeline` passed.
- `dsl-lsp` targeted test `parser_conformance` passed.

Not run:

- DB-dependent Sem OS and DSL integration tests under `rust/tests` and ignored crate tests.
- broad workspace test suites.

### Limitations

- This review is evidence-based from current source and targeted validation, but not from full workspace test execution.
- DB-backed and service-backed integration behavior was inspected statically unless covered by the targeted tests above.
- “Likely official contract” is an inference from crate roots, imports, and comments; it is not treated as a formal published API unless the code already reflects it.

## 3. Sem OS current landing

### Current capability role

Observed: Sem OS is now clearly split into a service family:

- `sem_os_core` for domain types, ports, resolution, authoring, and registry logic
- `sem_os_client` for in-process and HTTP access
- `sem_os_server` for HTTP deployment
- `sem_os_postgres` for adapter implementations
- `sem_os_harness` for scenario-based harness logic
- `sem_os_obpoc_adapter` for converting `ob-poc` YAML/config into seed bundles

Inference: this is close to a standalone capability shape, but the workspace still often treats Sem OS as a family of directly accessible crates rather than primarily going through `sem_os_client`.

### Current effective public/API surface

Observed crate-root surfaces:

- `sem_os_client` exposes `http`, `inprocess`, and the `SemOsClient` trait in `rust/crates/sem_os_client/src/lib.rs`.
- `sem_os_server` now exposes only `OutboxDispatcher`, `JwtConfig`, and `build_router` in `rust/crates/sem_os_server/src/lib.rs`.
- `sem_os_postgres` exposes adapter modules plus reexports and the `PgStores` convenience struct in `rust/crates/sem_os_postgres/src/lib.rs`.
- `sem_os_harness` exposes scenario-runner functions like `run_core_scenario_suite` from `rust/crates/sem_os_harness/src/lib.rs`; its helper modules are `#[cfg(test)]`.
- `sem_os_obpoc_adapter` exposes `metadata`, `onboarding`, `scanner`, `seeds`, `build_seed_bundle`, and `build_seed_bundle_with_metadata` in `rust/crates/sem_os_obpoc_adapter/src/lib.rs`.
- `sem_os_core` still exposes a very large set of public modules directly in `rust/crates/sem_os_core/src/lib.rs`.

### Likely official/public contract

Observed contract-worthy surfaces:

- `sem_os_client::SemOsClient`
- `sem_os_client::inprocess::InProcessClient`
- `sem_os_client::http::HttpClient`
- `sem_os_server::{build_router, JwtConfig, OutboxDispatcher}`
- request/response DTOs in `sem_os_core::proto`
- identity and principal types in `sem_os_core::principal`
- core shared types in `sem_os_core::types` and seed DTOs in `sem_os_core::seeds`

Inference: these are the most capability-shaped entry points as the code stands today.

### Integration-only surfaces

Observed integration surfaces that are still public and actively consumed:

- `sem_os_core::service::{CoreService, CoreServiceImpl}` used in `rust/crates/ob-poc-web/src/main.rs`, `rust/tests/chat_verb_profiles_integration.rs`, and `rust/tests/sem_reg_integration.rs`
- `sem_os_core::ports::*` and `sem_os_postgres::PgStores` used for in-process wiring in `rust/crates/ob-poc-web/src/main.rs` and tests
- `sem_os_postgres::{PgAuthoringStore, PgScratchSchemaRunner, PgCleanupStore}` used by `rust/xtask/src/sem_reg.rs` and tests
- `sem_os_obpoc_adapter::{scanner, seeds, metadata}` used from root `ob-poc` modules such as `rust/src/dsl_v2/macros/attribute_seed.rs` and `rust/src/sem_reg/scanner.rs`

These look like Sem OS family integration seams rather than external capability contracts.

### Implementation details now successfully hidden

Observed improvements:

- `sem_os_server` no longer exports `handlers`, `error`, `middleware`, `router`, or `dispatcher` as modules; only root reexports remain in `rust/crates/sem_os_server/src/lib.rs`.
- `sem_os_harness` support modules `db`, `permissions`, and `projections` are `#[cfg(test)]` in `rust/crates/sem_os_harness/src/lib.rs`.
- `sem_os_harness` no longer forces `sem_os_postgres::store::*` access; its internal tests use crate-root adapter reexports.
- the dormant `sem_os_server/src/handlers/tools.rs` module was removed, so the server no longer preserves dead handler seams for unrouted endpoints.

### Implementation details still leaking

Observed leaks:

- `sem_os_core` still exports nearly everything from `rust/crates/sem_os_core/src/lib.rs`, including `affinity`, `diagram`, `observatory`, `grounding`, `authoring`, and many object-body modules.
- `sem_os_postgres` still exports `authoring`, `cleanup`, `constellation_hydration`, `sqlx_types`, and `store` as public modules in addition to crate-root reexports.
- `sem_os_obpoc_adapter` still exports `scanner` and `seeds`, and those paths are consumed directly by `ob-poc`.
- `sem_os_client` still includes tool-dispatch methods and `HttpClient` still calls `/tools/call` and `/tools/list` in `rust/crates/sem_os_client/src/http.rs`, but `sem_os_server` has those routes commented out in `rust/crates/sem_os_server/src/router.rs` and its integration test expects 404.

That last point is a fact, not an inference: the current Sem OS HTTP client and HTTP server surfaces are not fully aligned.

### Crate-by-crate findings

`sem_os_core`

- Current role: domain kernel plus service boundary plus large shared namespace.
- What should be treated as stable now: `proto`, `principal`, `types`, `seeds`, `ports`, `service`.
- What still looks too broad: most other public modules at the crate root.
- Risk if tightened: medium to high, because the root workspace imports many `sem_os_core::*` modules directly.

`sem_os_client`

- Current role: the clearest Sem OS capability facade.
- Strength: explicit trait boundary, concrete `http` and `inprocess` implementations.
- Leak/mismatch: still models tool endpoints that the server no longer routes.

`sem_os_server`

- Current role: deployable HTTP surface.
- Strength: crate-root facade is now narrow and deliberate.
- Residual issue: top-of-file docs in `rust/crates/sem_os_server/src/lib.rs` still mention `/tools/*`, which no longer matches `router.rs`.

`sem_os_postgres`

- Current role: Sem OS adapter crate plus convenience assembly.
- Strength: root reexports are present and usable.
- Leak: public module tree is still wide, and `PgStores` remains a broad convenience bundle with public fields.

`sem_os_harness`

- Current role: harness/support crate rather than runtime API.
- Strength: support internals are test-only and no external `sem_os_harness::` consumers were found.
- Boundary quality: improved; this crate is no longer obviously driving production visibility.

`sem_os_obpoc_adapter`

- Current role: `ob-poc` bridge into Sem OS seed bundles.
- Strength: main bundle builders are clear top-level entry points.
- Leak: `scanner` and `seeds` remain public and are materially consumed from outside the crate.

## 4. DSL current landing

### Current capability role

Observed: DSL is split across a clean compiler kernel (`dsl-core`), a narrowed tooling crate (`dsl-lsp`), small playbook crates (`playbook-core`, `playbook-lower`), and a still-broad root runtime/compiler namespace in `rust/src/dsl_v2/mod.rs`.

Inference: the codebase has a real DSL capability, but it is not yet represented by one small official runtime facade.

### Current effective public/API surface

Observed crate-root surfaces:

- `dsl-core` exposes parser/AST/config/compiler/diagnostics modules and reexports in `rust/crates/dsl-core/src/lib.rs`.
- `dsl-lsp` exposes only `analyze_document`, encoding helpers, `EntityLookupClient`, `EntityMatch`, and `DslLanguageServer` in `rust/crates/dsl-lsp/src/lib.rs`.
- `playbook-core` and `playbook-lower` expose root reexports only in their `src/lib.rs`.
- `runbook` now exposes a root facade while keeping its internal module tree `pub(crate)` in `rust/src/runbook/mod.rs`.
- `bpmn_integration` now exposes a root facade while keeping its internal module tree `pub(crate)` in `rust/src/bpmn_integration/mod.rs`.
- `dsl_v2` now exposes a mixed but more deliberate surface in `rust/src/dsl_v2/mod.rs`: reexported `dsl-core` namespaces, a smaller set of local root exports, and the stage seams `syntax`, `planning`, `execution`, and `tooling`.

### Likely official/public contract

Observed contract-worthy surfaces:

- `dsl-core::{parse_program, BindingContext, diagnostics, config}`
- `dsl-lsp::DslLanguageServer` and the root `analyze_document` seam
- `playbook-core` parse/spec surface
- `playbook-lower` lowering surface
- stage seams at `dsl_v2::{syntax, planning, execution, tooling}`
- selected root `dsl_v2` reexports for core syntax/AST/compiler primitives and macro/expansion contracts
- `runbook::{compile_invocation, execute_runbook}` as the execution-facing contract

Inference: these are the parts that currently read like capability entry points. The rest of the exposed `dsl_v2` module tree still looks like implementation exposure.

### Tooling-facing surfaces

Observed:

- `dsl-lsp` now hides `analysis`, `handlers`, and `server` behind a crate facade in `rust/crates/dsl-lsp/src/lib.rs`.
- The test seam `analyze_document` is exported deliberately at the root and is consumed by `rust/crates/dsl-lsp/tests/lsp_harness.rs`.
- `dsl-lsp` still depends directly on `ob-poc` with the `database` feature in `rust/crates/dsl-lsp/Cargo.toml`, and internal handlers use many `ob_poc::dsl_v2::*` APIs.

Inference: `dsl-lsp` is now facade-shaped externally, but still tightly coupled internally to the broad `ob_poc::dsl_v2` surface.

### Implementation details now successfully hidden

Observed improvements:

- `dsl-lsp` no longer exports its `analysis`, `encoding`, `entity_client`, `handlers`, or `server` modules directly.
- The old test pressure on `dsl_lsp::handlers::diagnostics::analyze_document` is gone; `rust/crates/dsl-lsp/tests/lsp_harness.rs` now uses `dsl_lsp::analyze_document`.
- `playbook-core` and `playbook-lower` remain tight, facade-driven crates.

### Implementation details still leaking

Observed leaks:

- `rust/src/dsl_v2/mod.rs` still exposes a broad root barrel, especially around macro schema/registry/fixpoint types, expansion lock/policy/report types, and selected `dsl-core` support modules.
- `runbook` and `bpmn_integration` no longer leak their internal module trees, but their root facades are still fairly large because in-process assembly and execution are legitimate current use cases.
- the main remaining DSL leakage is now root-level convenience exposure, not public submodule trees.

### Crate-by-crate / module-group findings

`dsl-core`

- Current role: clean pure compiler kernel.
- Assessment: good capability boundary. Public surface is broad but coherent.

`dsl-lsp`

- Current role: tooling facade.
- Assessment: materially improved. Public root is small and clear.
- Remaining issue: internal coupling to `ob_poc::dsl_v2` is still deep, so the facade hides implementation but does not yet decouple from the broader runtime crate.

`playbook-core` and `playbook-lower`

- Current role: small DSL-adjacent kernels.
- Assessment: already facade-driven and in good shape.

`dsl_v2`

- Current role: actual runtime/compiler/tooling center of gravity.
- Assessment: still the main DSL hotspot. The root module is too broad to be considered a finished capability boundary.

`runbook`

- Current role: execution-facing DSL/runbook boundary.
- Assessment: the comments describe a small contract, but the module tree is still fully public.

`bpmn_integration`

- Current role: runtime integration with BPMN-lite.
- Assessment: still exposed as a full public tree, which reads more like implementation convenience than deliberate contract.

## 5. Test-boundary review

### Internal capability tests

Observed good patterns:

- `rust/crates/sem_os_core/tests/discovery_pipeline.rs` is an internal Sem OS capability test and passed in this review.
- `rust/crates/sem_os_harness/src/lib.rs` keeps its integration wiring under `#[cfg(test)]`, which is the right direction for harness internals.
- `rust/crates/dsl-lsp/tests/lsp_harness.rs` now uses the root `dsl_lsp::analyze_document` seam rather than deep handler imports.
- `rust/crates/dsl-lsp/tests/parser_conformance.rs` passed in this review and validates the parser/tooling contract from the crate’s own test suite.

Observed caveat:

- `dsl-lsp` tests are external integration tests under `tests/`, so they can only use public surfaces. `parser_conformance.rs` imports `ob_poc::dsl_v2::ast` and `parse_program`, which means this test suite still relies on broad public DSL runtime exposure in another crate.

### Platform / harness / orchestration tests

Observed good patterns:

- `rust/crates/sem_os_server/tests/authoring_http_integration.rs` targets the intended crate-root HTTP embedding facade via `sem_os_server::{build_router, JwtConfig}`.
- That same test explicitly asserts `/tools/list` and `/tools/call` return 404, which correctly treats the routed HTTP surface as the truth.

Observed remaining deep-import patterns:

- `rust/tests/sem_reg_integration.rs` builds `CoreServiceImpl` and `PgStores` directly.
- `rust/tests/chat_verb_profiles_integration.rs` builds an in-process Sem OS stack directly from `sem_os_core::service::CoreServiceImpl` and `sem_os_postgres::PgStores`.
- `rust/tests/semos_discovery_hit_rate.rs` uses `sem_os_core` ports and service types directly plus `sem_os_obpoc_adapter` bundle builders.
- `rust/tests/affinity_integration.rs` imports `sem_os_core::affinity`, `diagram`, `ports`, and `sem_os_postgres::PgStores` directly.
- many root DSL tests import `ob_poc::dsl_v2` submodules directly, including `entity_deps_integration.rs`, `soft_delete_db.rs`, `runbook_e2e_test.rs`, and `csg_pipeline_integration.rs`.

Assessment:

- The split is improved, but not complete.
- Harness crates are less problematic than before.
- Workspace-level integration tests still function as architectural backchannels into Sem OS and DSL internals.
- Existing tests should not be treated as proof that these deep surfaces are the right public contract; they are evidence that those surfaces are still depended on.

### Stale tests or stale seams

Observed:

- `sem_os_server` no longer preserves a tools handler module, and its HTTP integration test correctly expects 404 on `/tools/*`.
- However, the Sem OS HTTP client still implements those endpoints. That is a stale contract seam in code, not just in tests.

## 6. Shared kernel review

### Shared areas and classification

`dsl-core`

- Classification: true shared kernel / foundation contract.
- Evidence: used by `ob-poc`, `dsl-lsp`, `sem_os_obpoc_adapter`, `xtask`, and `ob-agentic`.
- Boundary quality: good.

`playbook-core` and `playbook-lower`

- Classification: true shared kernel, but DSL-local rather than Sem OS + DSL shared.
- Evidence: used by `dsl-lsp` and `xtask`.

`sem_os_core::{proto, principal, types, ports, service}`

- Classification: Sem OS family kernel / foundation contract.
- Evidence: consumed by `sem_os_client`, `sem_os_server`, `sem_os_postgres`, `ob-poc-web`, `xtask`, and tests.
- Boundary quality: functional but broad.

`sem_os_core::affinity`, `diagram`, and `observatory`

- Classification: Sem OS implementation detail currently reused by DSL or root platform code.
- Evidence: `rust/tests/affinity_integration.rs`, `rust/src/domain_ops/affinity_ops.rs`, `rust/src/domain_ops/sem_os_schema_ops.rs`, and `rust/src/api/observatory_routes.rs`.
- Boundary quality: looks more like reuse of Sem OS internals than a neutral shared kernel.

`sem_os_obpoc_adapter::scanner` and `seeds`

- Classification: accidental leak that should sit behind a narrower contract.
- Evidence: direct use in `rust/src/dsl_v2/macros/attribute_seed.rs` and `rust/src/sem_reg/scanner.rs`.

`ob_poc::dsl_v2` consumed by `dsl-lsp`

- Classification: DSL implementation detail currently reused by a tooling consumer.
- Evidence: `rust/crates/dsl-lsp/src/handlers/*` and `rust/crates/dsl-lsp/src/analysis/v2_adapter.rs`.
- Boundary quality: serviceable, but too coupled to broad runtime internals.

## 7. Public surface quality assessment

### Deliberate and facade-driven

- `rust/crates/sem_os_server/src/lib.rs` is now deliberate and facade-driven.
- `rust/crates/dsl-lsp/src/lib.rs` is now deliberate and facade-driven.
- `rust/crates/playbook-core/src/lib.rs` and `rust/crates/playbook-lower/src/lib.rs` are compact and coherent.
- `rust/crates/sem_os_client/src/lib.rs` is the cleanest capability boundary in the Sem OS family.

### Too broad

- `rust/crates/sem_os_core/src/lib.rs` remains too broad.
- `rust/src/dsl_v2/mod.rs` remains too broad.
- `rust/src/runbook/mod.rs` and `rust/src/bpmn_integration/mod.rs` remain broader than their conceptual comments suggest.

### Still convenience-driven

- `rust/crates/sem_os_postgres/src/lib.rs` still exports both module tree and crate-root adapter types.
- `rust/crates/sem_os_obpoc_adapter/src/lib.rs` still exports helper namespaces used directly for convenience.
- `rust/crates/ob-poc-web/src/main.rs` still deep-imports DSL helper paths such as `ob_poc::dsl_v2::gateway_resolver::gateway_addr`.

## 8. What improved in this refactor

1. `sem_os_server` is now much easier to reason about as a standalone server crate.
   Example: `rust/crates/sem_os_server/src/lib.rs` now exposes only `build_router`, `JwtConfig`, and `OutboxDispatcher`.

2. `dsl-lsp` no longer exposes its internal module tree.
   Example: `rust/crates/dsl-lsp/tests/lsp_harness.rs` uses `dsl_lsp::analyze_document` rather than `dsl_lsp::handlers::diagnostics::analyze_document`.

3. Harness internals no longer obviously force production visibility.
   Example: `rust/crates/sem_os_harness/src/lib.rs` gates helper modules behind `#[cfg(test)]`.

4. Dead server seams were actually removed rather than left public.
   Example: `rust/crates/sem_os_server/src/handlers/tools.rs` is gone, and `router.rs` treats `/tools/*` as intentionally absent.

5. The crate-level story is clearer for peer readers.
   Sem OS now reads as client/server/core/postgres/harness/adapter, and DSL now reads as core/LSP/playbook/root-runtime even though the root runtime is still too wide.

## 9. Remaining leaks / structural hotspots

### High-confidence cleanup follow-ups

1. Align `sem_os_client` and `sem_os_server` around tool endpoints.
   Observation: `HttpClient` still calls `/tools/*`, while `router.rs` omits them and server tests expect 404.

2. Reduce public module exposure in `sem_os_postgres`.
   Observation: root reexports exist, but `authoring`, `cleanup`, `constellation_hydration`, `sqlx_types`, and `store` are still public modules.

3. Tighten stale docs/comments where they no longer match the routed surface.
   Observation: `sem_os_server/src/lib.rs` still documents `/tools/*` as if they exist.

### Deeper API/facade design decisions

1. Decide what the minimal `sem_os_core` external contract actually is.
2. Decide whether `sem_os_obpoc_adapter::scanner` and `seeds` are true bridge API or temporary leakage.
3. Decide what the official DSL runtime facade is beyond `dsl-core` and `dsl-lsp`.
4. Decide whether `runbook` and `bpmn_integration` should remain public module trees or become narrower facades.

### Test-boundary follow-ups

1. Root Sem OS integration tests still assemble `CoreServiceImpl` and `PgStores` directly.
2. Root DSL integration tests still deep-import `ob_poc::dsl_v2::*` submodules.
3. `dsl-lsp` tests are cleaner than before, but some still depend on `ob_poc::dsl_v2` public internals because they live in `tests/` rather than inside source modules.

## 10. Recommended next slices

1. Sem OS contract alignment slice.
   Scope: align `sem_os_client` with the actual `sem_os_server` HTTP surface, and clean stale route docs/comments.

2. `sem_os_postgres` facade tightening slice.
   Scope: keep root reexports and reduce public module exposure where callers can already use the root.

3. Sem OS test-boundary slice.
   Scope: move root tests that build `CoreServiceImpl` and `PgStores` directly into Sem OS family crates where practical, or introduce a narrow shared test bootstrap seam.

4. `sem_os_obpoc_adapter` bridge slice.
   Scope: audit current `scanner` and `seeds` consumers and reduce direct helper imports where a small top-level bridge function would suffice.

5. DSL test-boundary slice.
   Scope: move root tests that validate `entity_deps`, expansion, and planner internals closer to `dsl_v2` modules, leaving black-box runtime tests at the workspace level.

6. DSL facade-definition slice.
   Scope: define and enforce a narrower top-level `dsl_v2` or `runbook` contract before attempting broad visibility reduction.

## 11. Peer review appendix

### Questions for peer reviewer

1. Is `sem_os_core` currently serving two incompatible roles: stable Sem OS kernel and convenience namespace for the monolith?
2. Should `sem_os_obpoc_adapter` be treated as a public bridge crate, or is its current `scanner` and `seeds` exposure mostly accidental?
3. For DSL, is `runbook` the right long-term execution facade, or should the capability boundary stay centered on `dsl_v2` with a narrower export policy?
4. Are the root integration tests still providing useful black-box assurance, or are they mostly preserving internal reachability?

### Areas where current shape may still be misleading

- `sem_os_server` looks cleaner than the effective Sem OS boundary because the workspace still constructs `CoreServiceImpl` and `PgStores` directly in multiple places.
- `dsl-lsp` looks cleaner than the effective DSL boundary because it is still implemented on top of the broad `ob_poc::dsl_v2` surface.
- comments in `runbook` describe a small execution contract, but the module tree is still widely public.

### Places where a second architectural opinion is most valuable

- the minimum stable `sem_os_core` contract
- whether `sem_os_client` should be the dominant Sem OS platform boundary in more of `ob-poc`
- the intended top-level DSL runtime surface between `dsl-core`, `dsl_v2`, `runbook`, and `bpmn_integration`
- which root tests are genuinely black-box and which should be moved inward
