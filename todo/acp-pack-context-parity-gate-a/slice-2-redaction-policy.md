# Slice 2 Redaction Policy

Status: draft; implementation blocked pending review.

Date: 2026-05-10

## Policy Name

Initial policy id:

```text
slice2_runtime_context_redaction_v1
```

## Default Rule

Deny by default.

A runtime field may be projected only if it is listed in the allowlist below or receives an explicit reviewed policy entry.

## Allowed Field Classes

| Class | Examples | Notes |
| --- | --- | --- |
| Stable ids | session id, request id, CBU id, deal product id, snapshot id | allowed only when already scoped to the session |
| Pack ids | `onboarding-request`, `cbu-maintenance`, `product-service-taxonomy` | static/public config identifiers |
| Verb/template/macro ids | selected ids already surfaced by Slice 1 trace | registry identifiers only |
| Enum states | request state, phase, entry status, FSM state | no free text |
| Counts | active SRDEF count, missing binding count, owner coverage count | preferred over lists |
| Booleans | verified, stale, missing, ready, blocked | allowed |
| Timestamps | snapshot created, source refreshed, request created/completed | truncate to second precision unless exact audit time is required |
| Blocker codes | missing owner, stale snapshot, redaction denied | code only |
| Hashes | static envelope hash, runtime hash, projection hash | allowed |

## Blocked Field Classes

| Class | Examples | Reason |
| --- | --- | --- |
| Personal identifiers | names, emails, phone numbers, user names | PII |
| Commercial details | fees, rates, contract refs, client economics | not needed for routing |
| Raw documents | extracted clauses, uploaded text, data request body | unbounded sensitivity |
| Prompt/completion text | raw LLM prompt, raw LLM answer | trace policy only, not runtime context |
| Raw DSL | full draft body, raw args | execution surface and possible data leak |
| Free text | notes, comments, action labels, result summaries | unbounded data |
| Provider config | URLs, credentials, config JSON | secret/confidential risk |
| Full graphs | node/edge payloads, ownership percentages | too broad and sensitive |

## Label-Safe Decision

Initial Slice 2 policy treats labels as blocked unless reviewed.

Blocked for v1:

- `deal_name`
- `client_group_name`
- `cbu_name`
- `created_by`
- `type_name`
- `op_name`
- `resource_name`

Allowed substitutes:

- ids
- enum states
- counts
- missing-field codes
- blocker codes

## Projection Shape

Every runtime projection should include:

```json
{
  "redaction_policy": "slice2_runtime_context_redaction_v1",
  "redacted_count": 0,
  "blocked_field_codes": [],
  "runtime_fields": {}
}
```

If a field is blocked, record only a code:

```json
{
  "blocked_field_codes": ["label.cbu_name", "payload.service_intent_options"]
}
```

Do not include blocked values in response bodies, traces, logs, diagnostics, or test snapshots.

## Failure Behavior

If a required runtime field is blocked:

1. Do not draft DSL.
2. Do not fall back to raw source data.
3. Return a structured pending question or refusal.
4. Include only blocker codes and redaction policy id.

Preferred diagnostic code prefix:

```text
runtime_context_redacted_
```

## Test Requirements

Slice 2 fixtures must assert forbidden-field absence across:

- HTTP response body
- ACP response metadata
- persisted session trace
- baseline run raw output files

Forbidden-field fixtures should include at least:

- CBU name present in source but absent from projection
- owner/principal email present in source but absent from projection
- service intent config present in source but absent from projection
- free-text note present in source but absent from projection

## Review Decision

Proposed decision: approve `slice2_runtime_context_redaction_v1` as id/enum/count/hash-only. Require a separate review before any human-readable names or free-text summaries enter runtime context.
