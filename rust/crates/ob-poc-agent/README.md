# sage-acp — Sage ACP server (operator guide)

`sage-acp` is the launchable Agent Client Protocol server. It speaks
newline-delimited JSON-RPC 2.0 over stdio (stdout = protocol, stderr
= diagnostics) and is the in-process planning loop for Sage:
constellation hydration, frontier computation, blocker detection,
prompt assembly, GoalProposalTrace emission, audit fan-out.

Capability charter and discipline live in `Cargo.toml` and the Sage
ACP capability plan (`docs/todo/capability-crate-restructure-v1.md`
§6 decision 6).

## Build & run

```bash
# Default: in-process MCP transport + in-process runbook channel +
# deterministic LLM fallback (no API keys).
cargo run -p ob-poc-agent --bin sage-acp

# Full out-of-process spike — spawn sem_os_mcp + dsl-lsp as
# subprocesses, hit Anthropic for tool-call drafting:
cargo build -p sem_os_mcp -p dsl-lsp -p ob-poc-agent
ANTHROPIC_API_KEY=sk-ant-... \
  OBPOC_SAGE_MCP_TRANSPORT=subprocess \
  OBPOC_SAGE_RUNBOOK_CHANNEL=subprocess \
  cargo run -p ob-poc-agent --bin sage-acp
```

The server reads one JSON-RPC request per line on stdin and writes
responses + notifications per line to stdout. All diagnostics go to
stderr — never inject anything else into stdout, or the editor on the
other side will reject the frame.

## Environment-variable surface

Every variable is optional; defaults are chosen so a bare
`cargo run -p ob-poc-agent --bin sage-acp` launches without setup.
Empty / whitespace-only values are treated as unset.

### Pack selection

| Variable           | Default               | Purpose |
|--------------------|-----------------------|---------|
| `OBPOC_PACKS_DIR`  | `rust/config/packs`   | Directory holding pack YAML manifests. Relative paths resolve from the process working directory. |
| `SAGE_PACK_ID`     | `book-setup`          | Pack the session anchors to at startup. The planning loop's allowed-verb surface is the union of the pack's `allowed_verbs` plus what the active workspace exposes. |

### LLM provider (BYOK)

Sage uses one provider per process. Provider precedence is, in order:

1. `OBPOC_SAGE_LLM_PROVIDER` if set (forces the named provider; the
   matching key must also be set or the loop falls back to
   deterministic mode with a stderr warning).
2. `ANTHROPIC_API_KEY` if set (auto-picks Anthropic).
3. `OPENAI_API_KEY` if set (auto-picks OpenAI).
4. Otherwise: deterministic-fallback mode — the planning loop picks
   the first allowed verb. Useful for CI and conformance runs.

| Variable                   | Values                       | Purpose |
|----------------------------|------------------------------|---------|
| `OBPOC_SAGE_LLM_PROVIDER`  | `anthropic` \| `openai`      | Force provider when both keys are present. |
| `ANTHROPIC_API_KEY`        | `sk-ant-...`                 | Anthropic BYOK. Model controlled by `ANTHROPIC_MODEL` (see `ob-agentic` for defaults). |
| `OPENAI_API_KEY`           | `sk-...`                     | OpenAI BYOK. Model controlled by `OPENAI_MODEL` (see `ob-agentic`). |

### MCP transport (SemOS knowledge surface)

The MCP client backs both the knowledge-query path and the
constellation hydrator. Pick the transport at startup:

| Variable                     | Values                              | Default      | Purpose |
|------------------------------|-------------------------------------|--------------|---------|
| `OBPOC_SAGE_MCP_TRANSPORT`   | `in_process` \| `subprocess`        | `in_process` | `in_process` keeps the `sem_os_mcp` server in this address space (CI-safe, zero spawn cost). `subprocess` spawns the `sem_os_mcp` binary and speaks newline-delimited JSON-RPC over its stdio. |
| `OBPOC_SAGE_MCP_BIN`         | absolute or relative path           | sibling      | When transport is `subprocess`, overrides the binary path. Default is `sem_os_mcp` next to the running `sage-acp` binary. |

Unknown transport values log a warning and fall back to `in_process`.

### Runbook channel (Sage ↔ REPL)

The runbook channel is how Sage submits and revalidates draft
runbooks. Defaults to a parse-only local channel; flip to
`subprocess` to exercise the full `dsl-lsp` analyser.

| Variable                       | Values                            | Default      | Purpose |
|--------------------------------|-----------------------------------|--------------|---------|
| `OBPOC_SAGE_RUNBOOK_CHANNEL`   | `in_process` \| `subprocess`      | `in_process` | `in_process` = `LocalRunbookChannel` (parse-only via `dsl_core::parser`). `subprocess` = spawn `dsl-lsp` and speak proper LSP traffic (Content-Length framing, `initialize` handshake, `publishDiagnostics`). |
| `OBPOC_SAGE_LSP_BIN`           | absolute or relative path         | sibling      | When channel is `subprocess`, overrides the binary path. Default is `dsl-lsp` next to the running `sage-acp` binary. |

Unknown channel values log a warning and fall back to `in_process`.

### Audit fan-out

Audit records emit through one or more sinks. Both sinks are
optional; with neither configured, records are dropped (an explicit
`[sage-acp] No audit sinks wired …` line is logged at startup).

| Variable                      | Values                           | Default                                   | Purpose |
|-------------------------------|----------------------------------|-------------------------------------------|---------|
| `OBPOC_SAGE_AUDIT`            | path \| `none`                   | XDG / HOME-derived path (see below)       | JSONL local sink. `none` disables the local sink. A path enables and overrides the default location. |
| `OBPOC_SAGE_OTLP_ENDPOINT`    | OTLP HTTP collector URL          | unset (disabled)                          | When set, fans out audit records to an OTLP collector under service name `sage-acp`. |

Default JSONL path resolution (when `OBPOC_SAGE_AUDIT` is unset):

1. `$XDG_STATE_HOME/sage-acp/audit.jsonl` if `XDG_STATE_HOME` is set.
2. `$HOME/.cache/sage-acp/audit.jsonl` if `HOME` is set.
3. `./sage-acp-audit.jsonl` as last-resort fallback.

When both sinks fire, audit records are written to both — the JSONL
sink is point-in-time replay-grade, the OTLP sink feeds observability.

## Run modes at a glance

| Mode                          | Env to set                                                                                         | Use case |
|-------------------------------|----------------------------------------------------------------------------------------------------|----------|
| Deterministic spike           | _none_                                                                                             | CI, harness fixtures, smoke. No LLM, no subprocesses. |
| BYOK in-process               | `ANTHROPIC_API_KEY` _or_ `OPENAI_API_KEY`                                                          | Local dev with one LLM, fast loop iteration. |
| Out-of-process MCP            | `OBPOC_SAGE_MCP_TRANSPORT=subprocess`                                                              | Exercise the production MCP wire, keep the analyser in-process. |
| Out-of-process LSP            | `OBPOC_SAGE_RUNBOOK_CHANNEL=subprocess`                                                            | Full `dsl-lsp` analyser through real LSP framing. |
| Full out-of-process           | both `OBPOC_SAGE_*_TRANSPORT=subprocess` flags                                                     | Closest to production topology. |
| Observability                 | `OBPOC_SAGE_OTLP_ENDPOINT=https://otel.example/v1/logs`                                            | Stream audit records to the org's OTLP collector. |
| Local audit override          | `OBPOC_SAGE_AUDIT=/tmp/sage-acp-run.jsonl`                                                         | Capture one run's audit trail at a known location. |
| Audit off                     | `OBPOC_SAGE_AUDIT=none` (and don't set `OBPOC_SAGE_OTLP_ENDPOINT`)                                 | Tests that assert no audit side-effects. |

## Provider precedence summary

```
LLM:
  OBPOC_SAGE_LLM_PROVIDER=anthropic + ANTHROPIC_API_KEY → Anthropic
  OBPOC_SAGE_LLM_PROVIDER=openai    + OPENAI_API_KEY    → OpenAI
  (no force)                        + ANTHROPIC_API_KEY → Anthropic
  (no force)                        + OPENAI_API_KEY    → OpenAI
  (no force, no keys)                                   → deterministic

MCP transport:
  OBPOC_SAGE_MCP_TRANSPORT=subprocess + (OBPOC_SAGE_MCP_BIN || sibling) → subprocess
  otherwise                                                            → in-process

Runbook channel:
  OBPOC_SAGE_RUNBOOK_CHANNEL=subprocess + (OBPOC_SAGE_LSP_BIN || sibling) → subprocess
  otherwise                                                              → in-process
```

## Diagnosing a launch

Every wiring decision logs one stderr line at startup. A clean
in-process boot looks like:

```
[sage-acp] Loaded pack 'book-setup' (N allowed verbs, M forbidden) hash=...
[sage-acp] SemOS MCP server constructed (bridge: stub, N tools)
[sage-acp] MCP transport wired (provider: sem_os_mcp@in-process)
[sage-acp] SemOS knowledge client wired (provider: sem_os_mcp@in-process)
[sage-acp] Constellation hydrator wired (provider: sem_os_mcp@in-process)
[sage-acp] Runbook channel: in-process LocalRunbookChannel
[sage-acp] GoalProposalTrace sink wired (provider: phase-3-spike)
[sage-acp] Local JSONL audit sink: /…/audit.jsonl
[sage-acp] OTLP audit exporter disabled (OBPOC_SAGE_OTLP_ENDPOINT unset)
[sage-acp] Audit fan-out: jsonl (1 sinks)
[sage-acp] Server started
```

Subprocess transports emit an additional `Spawning …: <path>` line
before the transport-wired line. A missing sibling binary fails fast
with a clear `set OBPOC_SAGE_*_BIN or build with cargo build -p …`
message.

## Related

- Sage ACP capability plan: `docs/todo/capability-crate-restructure-v1.md`
- Rip-first completion memo: `docs/sage-acp-rip-first-completion-memo.md`
- ACP boundary (discovery/projection surface): `rust/crates/ob-poc-boundary/`
- SemOS MCP server: `rust/crates/sem_os_mcp/`
- Runbook analyser / LSP server: `rust/crates/dsl-lsp/` + `rust/crates/dsl-runtime/`
