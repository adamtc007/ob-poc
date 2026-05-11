# Verb Dispatch Bypass Inventory

Status: audit draft for Gate A replan.

Bypass candidates:

| Surface | Evidence | Classification | Recommendation |
| --- | --- | --- | --- |
| Raw DSL execute endpoint | `/api/session/:id/execute`, `execute_session_dsl_legacy_raw_only` | Live but legacy | Restrict to explicit raw-DSL maintenance or remove from normal server build. |
| REPL fallback | `try_route_through_repl` | Live fallback | Replace with envelope-gated route or quarantine before Slice 1 acceptance. |
| Proposal engine `direct.dsl` vocabulary | `rust/src/repl/proposal_engine.rs` | Regression/bypass vocabulary | Keep only as refusal/regression guard; rename misleading tests after behavior is confirmed. |
| ACP protocol prompt handlers | `rust/src/acp_protocol.rs` | Live alternate ingress | Route through same envelope verification or formally scope outside Slice 1. |
| Direct test calls into executors | `rust/tests` dispatch/executor calls | Test-only bypass | Keep for lower-level executor tests; exclude from utterance-route acceptance. |

Gate A finding:

The highest-risk production bypass is not ordinary executor tests; it is callable live routing that can produce or execute DSL without the future envelope invariant.
