# Single Pipeline Invariants

> Server-side enforcement of a single intent pipeline.
> All generation and execution flows through `orchestrator::handle_utterance()`.

## Allowed Entrypoints

| Entrypoint | Route | ActorResolver | PolicyGate |
|---|---|---|---|
| Chat API | `POST /api/session/:id/chat` | `from_headers()` | via `AgentService.policy_gate` |
| MCP `dsl_generate` | MCP tool dispatch | `from_env()` | `PolicyGate::from_env()` |
| REPL | `orchestrator_v2` | `from_session_id()` | `PolicyGate::from_env()` |

## Forbidden Paths (gated by PolicyGate)

| Path | Gate | Default |
|---|---|---|
| `POST /api/agent/generate` | `can_use_legacy_generate()` | **denied** |
| `POST /api/agent/generate-with-tools` | `can_use_legacy_generate()` | **denied** |
| `POST /api/agent/onboard` | `can_use_legacy_generate()` | **denied** |
| `POST /api/session/:id/execute` (raw DSL) | `can_execute_raw_dsl()` | **denied** |
| Direct DSL `(...)` in pipeline | `can_use_direct_dsl()` | **denied** |
| Direct DSL `dsl:...` in pipeline | `can_use_direct_dsl()` | **denied** |

## Policy Flags (environment variables)

| Variable | Default | Effect |
|---|---|---|
| `OBPOC_STRICT_SINGLE_PIPELINE` | `true` | Enables all single-pipeline gates |
| `OBPOC_ALLOW_RAW_EXECUTE` | `false` | Allows `/execute` with raw DSL body |
| `OBPOC_ALLOW_DIRECT_DSL` | `false` | Allows `dsl:` prefix bypass in pipeline |
| `OBPOC_STRICT_SEMREG` | `true` | SemReg deny-all fails closed (no fallback) |
| `OBPOC_ALLOW_LEGACY_GENERATE` | `false` | Allows legacy `/generate` endpoints |

## Actor Resolution

| Source | Resolver | Default Role |
|---|---|---|
| HTTP API | `ActorResolver::from_headers()` | `viewer` |
| MCP | `ActorResolver::from_env()` | `viewer` |
| REPL | `ActorResolver::from_session_id()` | `viewer` |

Headers used by `from_headers()`:
- `x-obpoc-actor-id` (default: `anonymous`)
- `x-obpoc-roles` (comma-separated, default: `viewer`)
- `x-obpoc-department`
- `x-obpoc-clearance` (`public`/`internal`/`confidential`/`restricted`)
- `x-obpoc-jurisdictions` (comma-separated)

## SemReg Failure Modes

| Mode | `OBPOC_STRICT_SEMREG` | Behavior when SemReg denies all candidates |
|---|---|---|
| Strict (default) | `true` | Returns empty candidates (fail-closed) |
| Permissive | `false` | Falls back to unfiltered candidates |

## IntentTrace Fields

Every utterance processed through the orchestrator produces an `IntentTrace` with:
- `policy_gate_snapshot` — full policy state at time of processing
- `sem_reg_mode` — `strict` or `permissive`
- `sem_reg_denied_all` — whether SemReg filtered all candidates
- `dsl_source` — origin (`Chat`, `Mcp`, `Repl`)
- `bypass_used` — `direct_dsl`, `direct_dsl_legacy`, or `None`

## Static Guard

Test `test_no_duplicate_pipeline_outside_orchestrator` scans all source files
and fails if `IntentPipeline` is constructed anywhere other than
`orchestrator.rs` or `intent_pipeline.rs` (its own module).
