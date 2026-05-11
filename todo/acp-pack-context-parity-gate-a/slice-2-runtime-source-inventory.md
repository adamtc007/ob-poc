# Slice 2 Runtime Source Inventory

Status: draft; implementation blocked pending review.

Date: 2026-05-10

## Purpose

This inventory names the runtime sources that Slice 2 may project into ACP pack context. It is intentionally conservative: only bounded, redacted, request-scoped state is eligible.

## Allowed Source Classes

| Source class | Candidate module/type | Allowed projection | Blocked projection |
| --- | --- | --- | --- |
| Session identity and lifecycle | `src/session/unified.rs`, `UnifiedSession`, `SessionState`, `StateSnapshot` | session id, state enum, state snapshot id, snapshot timestamp, run-sheet cursor | raw message history, full run sheet body, free-text action labels unless normalized |
| State stack snapshot | `src/session/unified.rs`, `StateStack`, `StateSnapshot` | snapshot id, timestamp, entity scope kind, cursor | full entity payload, arbitrary labels |
| Session deal context | `src/api/deal_types.rs`, `SessionDealContext` | deal id, deal status, client group presence | deal name, client group name unless reviewed as label-safe |
| Onboarding handoff summary | `src/api/deal_types.rs`, `OnboardingRequestSummary` | request id, CBU id, request state, current phase, deal product id, created/completed timestamps | CBU name, creator identity, commercial deal details |
| Run sheet progress | `src/session/unified.rs`, `RunSheet`, `RunSheetEntry`, `EntryStatus` | entry ids, status enum, cursor, blocker code | raw DSL, raw args, error text with user data |
| Research/macro state | `src/session/research_context.rs`, `ResearchContext`, `ResearchState` | state enum, pending/approved counts, macro id if registry-grade | research result body, edits, prompt/completion text |
| Policy snapshot | `src/policy/gate.rs`, `PolicyGate`, `PolicySnapshot` | strict flags and capability posture | actor personal data |
| Service resource discovery | `src/service_resources/discovery.rs`, `ResourceDiscoveryEngine`, discovery result types | count of active/discovered SRDEFs, SRDEF ids, blocker codes | raw service intent options, raw discovery payload, provider config |
| Taxonomy discovery | `src/taxonomy/ops.rs`, `Discovery`, `OpInfo`, `ResourceInfo`, `Gap` | type code, op code, required/optional counts, missing resource code | CBU name, free-text names, provisioning config |
| Entity graph summary | `src/graph/types.rs`, `EntityGraph` | node/edge counts, current entity id if already scoped, ownership/control depth counts | names, percentages, full graph, raw relationship payload |
| ACP trace state | `src/repl/session_trace.rs`, `TraceOp` | trace ids, hashes, selected pack/verb/template/macro ids, verification booleans | raw prompt/completion text, raw DSL body beyond existing approved trace policy |

## Explicitly Excluded For Slice 2

| Source | Reason |
| --- | --- |
| Arbitrary SQL query results | no bounded redaction or snapshot contract yet |
| Raw documents and document bundles | high leak risk; needs separate content redaction policy |
| Raw LLM prompts/completions | already governed by trace policy; not runtime pack context |
| Commercial economics, fees, rate cards | not needed for Slice 2 pack routing |
| Free-text user notes | unbounded PII/confidential content risk |
| Research macro result payloads | excluded from Slice 1 production context and still not runtime-safe |
| Full ownership graph | too large and sensitive; only count/depth summaries eligible |

## Per-Pack Initial Source Set

### `onboarding-request`

Allowed:

- `OnboardingRequestSummary.request_id`
- `OnboardingRequestSummary.cbu_id`
- `OnboardingRequestSummary.request_state`
- `OnboardingRequestSummary.current_phase`
- `OnboardingRequestSummary.deal_product_id`
- `StateSnapshot.id`
- `StateSnapshot.timestamp`
- run-sheet step statuses
- missing binding/blocker codes
- owner/principal coverage status as role/count only

Blocked:

- `OnboardingRequestSummary.cbu_name`
- `OnboardingRequestSummary.created_by`
- raw compiled data request payloads
- owner names, emails, or personal identifiers

### `cbu-maintenance`

Allowed:

- CBU id already in session scope
- product binding ids already in session scope
- binding status enum
- missing product binding codes
- run-sheet step statuses

Blocked:

- CBU legal name unless label-safe policy is approved
- confidential economics
- product contract details
- arbitrary entity graph rows

### `product-service-taxonomy`

Allowed:

- active SRDEF count
- discovered SRDEF ids
- missing resource codes
- operation/resource required counts
- discovery freshness timestamp

Blocked:

- service intent option payloads
- provider config
- instance URLs
- provisioning credentials
- unbounded discovery payloads

## Required Inventory Gaps

Before implementation, review must decide:

1. Whether CBU/deal/client names are ever label-safe in runtime context.
2. Whether owner/principal coverage can use stable principal ids or only role/count status.
3. Which module owns the request-scoped snapshot builder.
4. Whether SRDEF ids are always non-sensitive in all environments.
5. Whether runtime context reads can be satisfied from in-memory session state first, with DB-backed reads deferred.

## Review Decision

Proposed decision: approve only id, enum, count, timestamp, status, blocker-code, and hash fields for the first Slice 2 implementation. Defer label-safe names and DB-backed raw payloads.
