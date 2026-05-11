# Utterance Route Path Inventory

Status: audit draft for Gate A replan.

Evidence commands:

- `rg -n "route\\(|/api/session|/api/repl|utterance|try_route|resolve_acp_dag_semantic_prompt|execute_session_dsl_legacy_raw_only" rust/src/api rust/src/acp_protocol.rs rust/src/acp_dag_semantic.rs -S`
- `rg -n "legacy|fallback|bypass|direct\\.dsl|execute_session_dsl_legacy_raw_only|try_route_through_repl|try_route_supported_acp_prompt|resolve_acp_dag_semantic_prompt|SessionInputRequest" rust/src rust/tests rust/examples rust/docs -S`

Ingress paths found:

| Path/function | Classification | Finding |
| --- | --- | --- |
| `POST /api/session/:id/input` in `rust/src/api/agent_routes.rs` | Live and authoritative candidate | Route comments identify this as the unified input endpoint. |
| `session_input` | Live and authoritative candidate | Accepts `SessionInputRequest::Utterance` and decision replies. |
| `try_route_supported_acp_prompt` | Live but pre-envelope | Routes supported ACP prompts before REPL fallback. |
| `try_route_through_repl` | Live fallback path | Needs Gate B decision: rip, same-slice replace, or envelope-gated fallback. |
| `POST /api/session/:id/execute` | Live but legacy raw DSL | Code labels it legacy raw-DSL only. Must not be normal utterance path. |
| Removed `/api/session/:id/chat` | Dead route, vocabulary remains in tests/comments | Keep removal; clean naming/comments during remediation. |
| `resolve_acp_dag_semantic_prompt` | Live semantic resolver | Added/current untracked source surface; must be classified before envelope wiring. |
| ACP protocol prompt handlers in `rust/src/acp_protocol.rs` | Live alternate prompt surface | Needs explicit relationship to unified session input. |

Gate A finding:

There is one likely authoritative HTTP utterance endpoint, but the executable path is not single-path yet because pre-envelope ACP routing, REPL fallback, legacy raw DSL execution, and ACP protocol prompt handling remain callable surfaces.
