# Public API Freeze Decision

Status: draft decision; not a Gate E blocker.

Date: 2026-05-10

## Current State

Gate E implemented public API allowlist enforcement for the ACP boundary only.

Current enforced files:

- `src/lib.rs`
- `src/acp_dag_semantic.rs`
- `src/acp_pack_context_envelope_v2.rs`
- `src/acp_registry_projection.rs`
- `src/acp_static_context_acceptance.rs`

Current command:

```text
cargo run -p xtask -- pub-lint
```

Current result:

```text
Public API allowlist clean: 164 checked items across 5 files
```

## Decision

Do not freeze the whole historical root-crate public API before Slice 2 planning.

Freeze only new Slice 2 ACP runtime context surfaces as they are introduced.

## Rationale

The root crate has broad historical `pub` surface across API, session, runbook, service resources, graph, Sage, sem-reg, and UI-facing modules. Freezing all of it now would mix two unrelated efforts:

- ACP pack-context parity
- root-crate decomposition and public API cleanup

That broader freeze is valuable, but it is not required to plan Slice 2 runtime context and would create unnecessary review noise.

## Boundary Rule For Slice 2

Any new Slice 2 runtime context module must be added to `tools/public-api-allowlist.txt` enforcement before production implementation is marked complete.

Expected candidate files once implemented:

- `src/acp_runtime_context.rs`
- `src/acp_runtime_context_projection.rs`
- `src/acp_runtime_context_redaction.rs`
- `src/acp_runtime_context_snapshot.rs`

Actual filenames may differ, but any public ACP runtime context surface must be in the allowlist gate.

## Recommended Broader Freeze Path

Handle historical root-crate public surface as a separate crate-discipline slice:

1. inventory current public modules by domain
2. classify each public item as external API, crate-internal, test-only, or legacy
3. convert obvious internal items to `pub(crate)`
4. add parser-backed public API inventory when rustdoc JSON or syn-backed inspection is available
5. introduce per-domain allowlists only after module ownership is clear

## Review Questions

1. Is ACP-boundary-only enforcement sufficient for Slice 2 planning?
2. Should Slice 2 runtime context modules be added to the existing allowlist tool or get a separate allowlist file?
3. Should `src/lib.rs` stop publicly exporting newly introduced runtime internals by default?
4. Should broader freeze be scheduled before or after crate decomposition?

## Proposed Decision

Approve ACP-boundary-plus-new-Slice-2 enforcement for now. Defer full root-crate public API freeze to crate-discipline review.
