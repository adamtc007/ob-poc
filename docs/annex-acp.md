# ACP Annex

> **Status:** Partial — Phase A/B/C of the ACP audit remediation plan have landed.
> Phase C is partial (REST fully through the facade; stdio partially). Follow-up work
> (remaining stdio handlers, AcpPromptRouter, live-overlay folding into the facade) is
> tracked at the bottom of this annex.

## What ACP is

The Agent Client Protocol (ACP) is the rich agent-editor projection surface for
SemOS discovery. It is **not** the policy authority or the mutation authority.

- **Visibility:** Sage/editor may observe any classification-permitted projection
  the Domain Pack exposes.
- **Authority:** Direct ACP mutation is refused. Mutation is only available
  through workbook approval and the compiled runbook execution gate.

Two ACP personas exist: `sage:planning` and `sage:execution`. Discovery,
planning, explanation, and attestation are Sage workflow phases — not ACP modes.

## Four-layer architecture

```
              ┌──────────────────────────────────────────┐
TRANSPORT     │   REST (api/repl_routes_v2)              │
              │   stdio JSON-RPC (acp_protocol)          │
              └──────────────────────────────────────────┘
                          │ both transports call into …
                          ▼
              ┌──────────────────────────────────────────┐
ROUTER        │   AcpJsonRpcAgent::session_prompt (stdio)│
              │   acp_gateway_route_with_llm_client(REST)│
              └──────────────────────────────────────────┘
                          │ both routers call into …
                          ▼
              ┌──────────────────────────────────────────┐
FACADE        │   AcpFacade  (acp_facade.rs)             │
              │   single (session, manifest, op) entry   │
              └──────────────────────────────────────────┘
                          │ facade is the only caller of …
                          ▼
              ┌──────────────────────────────────────────┐
DOMAIN        │   crate::acp::*  (acp.rs)                │
              │   pure transport-neutral domain fns      │
              └──────────────────────────────────────────┘
```

**The R8 single-path invariant** is enforced at the FACADE layer: transports must
not call `crate::acp::*` directly. New transports/handlers always go through
`AcpFacade`.

## The facade contract

`rust/src/acp_facade.rs` exposes `AcpFacade` with two method variants per domain
operation:

| Variant | Used by | Behavior |
|---|---|---|
| `<op>(session_id, …)` | REST | Synthesizes a fresh `AcpSession` each call. |
| `<op>_for(session, …)` | stdio | Operates on a caller-owned `AcpSession`. |

Stdio takes the `_for` variant because `AcpJsonRpcAgent` caches sessions in a
`BTreeMap<Uuid, AcpSession>` to enforce the `Closed` state transition across
multiple JSON-RPC requests in a single session lifetime. REST has no such
cache — each REST request synthesizes a fresh session by construction.

### Operations on the facade

- `policy` / `policy_for`
- `projections_list` / `projections_list_for`
- `projection_get` / `projection_get_for`
- `context_assemble` / `context_assemble_for`
- `kyc_case_state_discover`
- `kyc_dry_run`
- `language_pack`
- `kyc_language_pack`
- `kyc_language_loop_timed`
- `open_session_with_persona`, `close_session`

The canonical Domain Pack manifest loader `load_ob_poc_kyc_domain_pack()` also
lives in `acp_facade.rs`. `repl_routes_v2.rs` and `acp_protocol.rs` retain
their own loader symbols for source compatibility, but both delegate to the
facade's loader.

## HTTP routes

| Endpoint | Purpose |
|---|---|
| `GET /api/session/:id/acp/policy` | ACP-visible SemOS policy/capability decisions |
| `GET /api/session/:id/acp/projections` | ACP-visible projection catalogue |
| `GET /api/session/:id/acp/projections/:kind` | Typed projection envelope (live overlay or declared source) |
| `POST /api/session/:id/acp/open` | Open ACP adapter session (no direct mutation capability) |
| `POST /api/session/:id/acp/close` | Close ACP adapter session |
| `POST /api/session/:id/acp/context` | Assemble redacted Sage context via Domain Pack discovery policy |
| `POST /api/session/:id/acp/gateway` | Prompt routing via the gateway flow |

**Removed in Phase B1 of the audit:** `GET /api/session/:id/acp/capabilities`.
This endpoint duplicated `/acp/policy` and additionally spun up a throwaway
`AcpJsonRpcAgent` to render a JSON-RPC `initialize` payload over HTTP. ACP
clients use stdio for protocol metadata; HTTP consumers should call
`/acp/policy` for policy and refer to the `ob_poc_acp` binary docs for stdio
launch metadata.

## stdio JSON-RPC

`rust/src/bin/ob_poc_acp.rs` launches the stdio agent. Methods are dispatched
in `AcpJsonRpcAgent::handle_request` (`acp_protocol.rs:419`).

Dispatch-table conventions:

- **Aliased methods.** `request_permission` and `obpoc/request_permission`
  dispatch to the same handler. The plain form is the ACP-standard name; the
  `obpoc/` prefix is the namespaced form. Both are accepted so ACP clients can
  use either convention.

- **Explicit-refuse list.** `write_text_file | fs/write_text_file |
  create_text_file | terminal/new | terminal/create` all return
  `INVALID_REQUEST` with structured authority-denied data. The variants cover
  the different forms used across ACP client implementations (root vs `fs/`
  namespace; `new` vs `create` for terminal). Listing every form ensures
  clients get the structured denial rather than `METHOD_NOT_FOUND` from the
  catch-all.

## The live-overlay vs declared-source projection model

Two functions assemble projection envelopes:

1. **`acp::build_acp_projection`** (in `acp.rs`) — the canonical declared-source
   view. Returns schema, source reference, classification, and a
   `declared_projection_source` placeholder payload for projection kinds that
   require host materialization. Transport-neutral and session-data-free.

2. **`build_live_acp_projection`** (in `api::repl_routes_v2`) — HTTP-only live
   overlay. Returns `Some(envelope)` when the projection kind has a live
   materializer against the current `ReplSessionV2`, or `None` to let the
   caller fall back to the declared-source view.

These are **not** duplicates. ACP/stdio clients want the declared-source view
(schema for URI dereferencing). HTTP UIs want live session data overlaid on
top. The REST handler `get_acp_projection_value_for_state` overlays the live
data when a live REPL session exists, else falls through to the facade's
declared-source view.

**Follow-up:** the live overlay still lives in `repl_routes_v2.rs` because it
depends on `ReplSessionV2`. Folding it into the facade would require the
facade to depend on REPL types. A future slice could introduce a
`LiveProjectionOverlay` trait so the facade can accept an optional overlay
without taking a `ReplSessionV2` dependency.

## Follow-up work

These items were scoped to a later slice when Phase C of the audit landed:

- **Stdio facade adoption — remaining handlers.** `obpoc_projection_get`,
  `obpoc_kyc_case_state_discover`, `obpoc_language_pack_get`,
  `obpoc_kyc_update_status_language_loop`, and `obpoc_kyc_dry_run` still
  inline the `(load_manifest, cache_lookup_or_open_session, domain_fn)`
  pattern. They will follow the same conversion as `obpoc_policy` /
  `obpoc_projection_list` / `obpoc_context` after the arg-extraction blocks
  in each handler are refactored.

- **`AcpPromptRouter`.** `session_prompt` (`acp_protocol.rs:601`) currently
  has implicit probe ordering — `try_session_prompt_kyc_update_status` runs
  first and can short-circuit before `try_session_prompt_dag_semantic` is
  evaluated. Centralize this into an explicit prompt router with a stable
  order rule documented in one place.

- **Live overlay → facade.** See the model section above. Requires either an
  overlay trait or moving the overlay closer to the facade.

- **Determinism replay.** Per v0.5 §16 the build-determinism invariant
  requires byte-equal envelope output for the same inputs. Phase C touched
  the gateway path; a fixture-replay before/after is recommended before
  the remaining stdio handlers are migrated.

## See also

- `rust/src/acp_facade.rs` — facade source.
- `rust/src/acp.rs` — pure domain functions.
- `rust/src/acp_protocol.rs` — stdio JSON-RPC dispatch.
- `rust/src/api/repl_routes_v2.rs` — REST handlers.
- `rust/src/bin/ob_poc_acp.rs` — stdio binary entry point.
