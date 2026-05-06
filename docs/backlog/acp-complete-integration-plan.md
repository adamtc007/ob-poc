# Complete ACP Integration Plan

## Scope

This tranche completes the ACP integration boundary for ob-poc:

- ACP JSON-RPC stdio server for Zed/custom ACP clients.
- Baseline ACP lifecycle methods: `initialize`, `authenticate`, `session/new`, `session/load`, `session/list`, `session/prompt`, `session/cancel`, and `session/close`.
- ACP session updates for prompt responses.
- ob-poc extension methods for governed context assembly and KYC update-status dry-run.
- Explicit mutation refusal at the ACP boundary; mutation continues through workbook approval and compiled runbook gates only.
- HTTP/React discovery of ACP capabilities and session lifecycle.

## Non-Negotiable Guardrails

- ACP stdout must contain only newline-delimited JSON-RPC messages.
- ACP may assemble Sage context and validate dry-runs.
- ACP may not mutate first-class state directly.
- Any future mutation path must compile to `CompiledRunbook` and execute via `execute_runbook()`.
- The production KYC Domain Pack remains dry-run-only until a separate approval enables a mutation manifest.

## Implementation Map

- `rust/src/acp_protocol.rs`: ACP JSON-RPC protocol and dispatch.
- `rust/src/bin/ob_poc_acp.rs`: launchable stdio ACP server.
- `rust/src/acp.rs`: transport-neutral safety/domain adapter.
- `rust/src/api/repl_routes_v2.rs`: HTTP ACP capability/open/close/context routes.
- `ob-poc-ui-react/src/api/acp.ts`: React ACP client types and calls.

## Regression Gates

- `cargo test -p ob-poc acp_protocol -- --nocapture`
- `cargo test -p ob-poc api::repl_routes_v2::tests::test_acp -- --nocapture`
- `cargo build -p ob-poc --bin ob_poc_acp`
- `npm run test:run -- acp`
- `npm run build`
