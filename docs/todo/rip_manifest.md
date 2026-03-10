# SemTaxonomy Rip Manifest

## Purpose

This manifest defines the exact replacement boundary for the SemTaxonomy rip-and-replace work. The goal is to replace the current `utterance -> Sage -> OutcomeIntent -> Coder -> DSL` pipeline while preserving the DSL execution engine, REPL, governance, runtime registry, and snapshot-backed SemOS surfaces.

## Replace

These components are on the replacement path and should be bypassed, then deleted once the new path is live.

### Sage Outcome Contract
- `rust/src/sage/mod.rs`
- `rust/src/sage/outcome.rs`
- `rust/src/sage/deterministic.rs`
- `rust/src/sage/llm_sage.rs`
- `rust/src/sage/disposition.rs`

### Coder Resolution Stack
- `rust/src/sage/coder.rs`
- `rust/src/sage/verb_resolve.rs`
- `rust/src/sage/arg_assembly.rs`
- `rust/src/sage/clash_matrix.rs`

### Orchestrator Glue
- `rust/src/agent/orchestrator.rs`
  - `run_sage_stage(...)`
  - `run_coder_stage(...)`
  - `handle_utterance(...)`
  - `legacy_handle_utterance(...)`
  - `build_sage_fast_path_result(...)`

### Agent Service / Session Glue
- `rust/src/api/agent_service.rs`
  - `build_orchestrator_context(...)`
  - `push_recent_sage_intent(...)`
  - `to_sage_explain_payload(...)`
  - `to_coder_proposal_payload(...)`
- `rust/src/session/unified.rs`
  - `recent_sage_intents`
  - `pending_mutation` fields that embed Sage/Coder products

### Harnesses That Benchmark the Old Pipeline
- `rust/tests/sage_coverage.rs`
- `rust/tests/utterance_api_coverage.rs`
- `rust/tests/coder_clash_regressions.rs`
- `rust/tests/coder_clash_matrix.rs`

## Preserve

These components remain the execution substrate and should be reused by the replacement path.

### DSL / REPL / Execution
- `rust/src/dsl_v2/*`
- `rust/src/runbook/*`
- `rust/src/api/agent_service.rs::execute_runbook(...)`

### Runtime Registry / Verb Contracts
- `rust/src/dsl_v2/runtime_registry.rs`
- `rust/src/dsl_v2/verb_registry.rs`
- `rust/config/verbs/*`

### SemOS / SemReg Query Surface
- `rust/src/sem_reg/agent/mcp_tools.rs`
- `rust/src/domain_ops/sem_reg_registry_ops.rs`
- `rust/src/domain_ops/sem_reg_schema_ops.rs`
- `rust/src/agent/verb_surface.rs`

### Existing Data Retrieval Surfaces
- `rust/src/api/entity_routes.rs`
- `rust/src/mcp/handlers/session_tools.rs`
- `rust/src/domain_ops/client_group_ops.rs`
- `rust/src/domain_ops/ubo_analysis.rs`
- `rust/src/graph/query_engine.rs`

## Cutover Strategy

1. Add new `discovery.*` verbs and implementations.
2. Add `SageSession`, `CompositionRequest`, and `ComposedRunbook` types.
3. Add a new utterance handling path that uses discovery + composition directly.
4. Move chat/session payloads to the new contract.
5. Bypass the old Sage/Coder pipeline.
6. Delete the old pipeline once parity is established.

## Safety Constraints

- Do not change the DSL execution engine during the rip.
- Do not widen governance permissions during the rip.
- Do not remove the old path until the new one is callable end-to-end.
- Prefer wrappers over SemReg/entity search reuse points instead of new query stacks.
