# Quarantine Register

Status: **R7 hardening complete (2026-05-11).** Per v0.5 §5.7 each entry now
carries a **named individual owner**, an **absolute retirement date**, and a
classification per the strict v0.5 §5.7 discipline:

- **CLOSED** — work is done; item retained for audit trail only.
- **OPEN — REMEDIATION SCHEDULED** — work scheduled to a named slice with
  an absolute target date.
- **NOT QUARANTINE — RECLASSIFIED** — audit revealed this item is not a
  quarantine candidate (it's load-bearing production code, not legacy
  bypass).

Per v0.5 §5.7: *Quarantined code cannot be callable from any production
path.* Items currently callable from production are explicitly NOT in
quarantine state — they're either scheduled remediation or correctly
classified production code.

## Entries

| # | Item | Owner | Status | Target date | Exclusion mechanism | Final disposition |
|---|---|---|---|---|---|---|
| 1 | `rust/config/macros/research/*.yaml` (3 macros: `client-discovery`, `regulatory-check`, `ubo-investigation`) | Adam Cearns | **CLOSED** | 2026-05-11 | Excluded from envelope projection by `MacroDefinitionSource` glob (only loads `rust/config/verb_schemas/macros/*.yaml`, not `rust/config/macros/research/*.yaml`). | Closed: research macros are non-deterministic prompt/tool schemas, never projected to production envelopes. Not lifted to registry-grade because they're not deterministic SemOS steps. |
| 2 | `POST /api/session/:id/execute` raw DSL route | Adam Cearns | **OPEN — REMEDIATION SCHEDULED** | After R8 (target: 2026-06) | Handler is `execute_session_dsl_legacy_raw_only`; returns `410 Gone` for normal session flows per Gate E bypass proof. Used only for admin-scope raw DSL tooling. | After R8 unification, decide: remove entirely vs. move to `/api/admin/execute-raw-dsl` with explicit admin auth. Pin under R8 acceptance. |
| 3 | ~~`try_route_through_repl` fallback~~ → renamed to `dispatch_to_v2_repl` under R5 (2026-05-11) | Adam Cearns | **NOT QUARANTINE — RECLASSIFIED** as production code | n/a | R5 audit (2026-05-11) found this is the canonical V2 REPL HTTP ingress for slash commands, decision replies, confirmations, and generic messages — not a legacy bypass. The original Gate A framing was wrong. | Reclassified. Bifurcation between this and the ACP DAG semantic path is real and will be removed under **R8 single-path unification** (`r8-unification-plan.md`). After R8 lands, the function becomes the *only* HTTP-layer dispatcher; ACP resolution moves inside `orchestrator.process()`. |
| 4 | ACP DAG semantic resolver direct calls from `acp_protocol.rs` (JSON-RPC server path) | Adam Cearns | **CLOSED** (deliberately retained) | n/a | The ACP JSON-RPC server (separate from the HTTP utterance route) legitimately calls `resolve_acp_dag_semantic_prompt_with_verified_envelopes` directly. This is the ACP server's whole purpose. | Closed: the resolver's public API is the ACP server's interface to it. Not a bypass; the JSON-RPC server is a separate ingress with its own auth + governance discipline. |
| 5 | Pack templates not modeled as workbook plans | Adam Cearns | **CLOSED** | 2026-05-11 | Slice 1 pack templates *are* lifted to `AcpWorkbookPlanProjection` entries per Gate C; templates excluded from macro projection. Six Slice 1 templates currently lifted (cbu-maintenance + product-service-taxonomy + onboarding-request). | Closed: lifting completed under Gate C. Non-Slice-1 templates handled when their packs reopen. |

## Strict v0.5 §5.7 invariant — current standing

> Quarantined code cannot be callable from any production path.

Every entry above either:

- **CLOSED:** no longer callable from any production path (entries 1, 5),
  OR retained as a deliberate ACP-server interface (entry 4, not a bypass).
- **OPEN — REMEDIATION SCHEDULED:** callable from production but
  scheduled for removal with an absolute target date (entry 2).
- **NOT QUARANTINE — RECLASSIFIED:** callable from production because it
  IS production code, with a named follow-up slice (R8) that will
  remove the architectural bifurcation that originally caused the
  misclassification (entry 3).

No entry is in indefinite quarantine. No entry is callable from
production "as quarantined."

## When to update this register

- New audit finding flags a path for removal → add as **OPEN — REMEDIATION SCHEDULED** with a named owner and absolute date.
- Scheduled removal lands → flip to **CLOSED** with the date.
- Audit reveals an entry is actually load-bearing production code → flip to **NOT QUARANTINE — RECLASSIFIED** with the rationale and pointer to whatever architectural slice addresses the underlying concern.
- An entry has been **OPEN** for more than one quarter past its target date → reviewer must explicitly re-scope or escalate.

## References

- v0.5 §5.7 quarantine discipline (`todo/acp-pack-context-parity-plan-v0.5.md`)
- R1 schema parity ADR (`r1-schema-parity-adr.md`)
- R8 single-path unification plan (`r8-unification-plan.md`)
- Gate E bypass proof (`gate-e-bypass-proof.md`)
