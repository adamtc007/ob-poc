# Slice 2 Runtime Context Planning

Status: approved planning packet; production implementation started against the frozen Slice 2 fixture set.

Date: 2026-05-10

## Purpose

Slice 2 adds bounded runtime context to the Slice 1 static pack context path.

Slice 1 answered: what can this pack do, under which static policy, with which templates and verbs?

Slice 2 should answer: what is the current safe runtime state needed to choose, refuse, or ask the next question without leaking data, using stale state, or bypassing the single utterance path?

## Non-Goals

Slice 2 must not:

- add a new utterance ingress route
- relax `/api/session/:id/execute`
- bypass HITL, workbook, runbook, or dry-run gates
- project unredacted PII, secrets, raw documents, or arbitrary database rows
- turn runtime context into an execution trigger
- mutate pack envelopes without a versioned schema decision
- broaden public API beyond the approved ACP boundary without review

## Candidate Runtime Fields

| Field | Applies to | Source class | Required redaction | Freshness target |
| --- | --- | --- | --- | --- |
| existing onboarding request summary | `onboarding-request` | session/workbook state | redact free text; keep ids, lifecycle, missing bindings | same request snapshot |
| CBU/product binding summary | `cbu-maintenance` | entity/workbook bindings | ids and labels only; no confidential economics | same request snapshot |
| active SRDEF discovery count | `product-service-taxonomy` | discovery state | count only; no raw discovery payloads | same request snapshot |
| expected slice count | `onboarding-request` | static plus runtime workbook state | integer only | same request snapshot |
| expected attribute count | `onboarding-request`, `product-service-taxonomy` | static plus runtime workbook state | integer only | same request snapshot |
| owner principal coverage | `onboarding-request` | principal binding state | role and coverage status only; no personal data | same request snapshot |
| L4 binding blockers | `onboarding-request` | validation state | blocker code and missing binding only | same request snapshot |
| existing compiled data request status | `onboarding-request` | workbook/runbook state | status, id, timestamp; no payload body | same request snapshot |
| current FSM instance state | `onboarding-request` | state machine | state id, transition eligibility, blocker codes | same request snapshot |
| current macro/workbook plan progress | all Slice 1 packs | workbook state | step ids, status, blocker codes | same request snapshot |

## Runtime Context Contract

Every runtime context projection should carry:

| Field | Meaning |
| --- | --- |
| `schema_version` | Runtime context schema version. |
| `pack_id` | Pack receiving the runtime projection. |
| `session_id` | Session scope. |
| `snapshot_id` | Stable id for the data snapshot used by the response. |
| `snapshot_created_at` | Timestamp when the projection snapshot was built. |
| `source_refs` | Redacted source identifiers used to build the projection. |
| `redaction_policy` | Named policy applied to the projection. |
| `freshness_policy` | Named staleness/freshness rule. |
| `runtime_hash` | Deterministic hash of the redacted runtime projection. |
| `static_envelope_hash` | Envelope hash of the static pack context consumed with the runtime projection. |
| `projection_hash` | Hash linking static plus runtime projection material. |
| `verified` | True only when snapshot, redaction, and static envelope links validate. |

## Redaction Rules

Runtime context must be deny-by-default.

Allowed by default:

- ids already present in the current session context
- enum state values
- counts
- booleans
- missing-field codes
- blocker codes
- workbook step ids
- timestamps needed for freshness
- pack ids, verb ids, template ids, macro ids

Blocked by default:

- raw document text
- email addresses
- personal names unless already normalized as principal ids
- account numbers
- payment details
- secrets, tokens, credentials
- arbitrary SQL rows
- free-text user notes
- unbounded discovery payloads
- raw LLM prompt or completion text outside the existing trace policy

Any blocked field can only be projected after a named redaction policy is added and reviewed.

## Freshness And Snapshot Consistency

Slice 2 should use one request-scoped snapshot per utterance.

Rules:

1. Static envelope verification happens before runtime projection is accepted.
2. Runtime source reads happen against a single snapshot boundary.
3. The response trace records `snapshot_id`, `runtime_hash`, `static_envelope_hash`, and `projection_hash`.
4. If required runtime data is unavailable, stale, or inconsistent, Sage must ask a bounded pending question or return a structured refusal. It must not guess.
5. Snapshot drift during a request invalidates the runtime projection and forces a retry/refusal path.

## Budget Policy

Initial planning budget:

| Budget | Limit |
| --- | --- |
| runtime fields per pack | maximum 12 |
| source refs per projection | maximum 20 |
| text fields | none by default |
| list lengths | maximum 10 unless count-only |
| serialized runtime projection | target under 8 KiB per pack response |
| trace refs | ids and hashes only |

Any budget breach should produce a structured diagnostic and use a reduced count-only projection.

## Trace Requirements

HTTP and ACP responses that consume runtime context should expose:

- `runtime_schema_version`
- `runtime_pack_id`
- `runtime_snapshot_id`
- `runtime_hash`
- `runtime_verified`
- `runtime_redaction_policy`
- `runtime_freshness_policy`
- `static_envelope_hash`
- `projection_hash`
- selected pack, verb, template, and macro ids where present

Persisted trace must allow a reviewer to prove:

- which static envelope was used
- which runtime snapshot was used
- which redaction policy was used
- whether the runtime projection was verified
- that no mutation ran during draft/refusal/pending-question response generation

## Fixture Plan

Create a frozen Slice 2 fixture set before implementation.

Minimum fixture groups:

| Group | Purpose | Count |
| --- | --- | --- |
| S2-ONB | onboarding runtime summary, blockers, owner coverage, FSM state | 8 |
| S2-CBU | CBU/product binding summary and missing binding questions | 5 |
| S2-SRDEF | active discovery counts and read-only service taxonomy context | 5 |
| S2-STALE | stale snapshot and drift refusal/pending behavior | 4 |
| S2-REDACT | redaction denial and count-only fallback | 4 |
| S2-GHOST | ghost-route bait with runtime context present | 5 |

Minimum total: 31 fixtures.

Each fixture should record:

- fixture id
- pack id
- utterance
- runtime source fixture
- expected selected pack
- expected selected verb/template/macro where applicable
- expected runtime fields
- forbidden runtime fields
- expected pending/refusal/draft status
- expected trace fields
- expected mutation posture

## Acceptance Threshold

Proposed Slice 2 threshold:

- 100% of runtime-consuming responses include verified static envelope hash and runtime hash.
- 100% of runtime-consuming responses cite the runtime snapshot id.
- 0 forbidden runtime fields appear in response body, trace body, or persisted trace.
- 0 ghost-route fixtures emit DSL or mutation permission.
- 0 stale/drift fixtures produce a draft as if data were fresh.
- At least 90% of non-stale runtime fixtures select the expected pack.
- At least 90% of non-stale runtime fixtures select the expected verb/template/macro when specified.
- Pending-question quality must score `2` for all missing-binding fixtures.
- Refusal quality must score `2` for all stale, redaction, and ghost-route fixtures.

## Workstreams

| Workstream | Output | Blocks implementation? |
| --- | --- | --- |
| S2-W1 runtime source inventory | list of allowed source tables/services/state holders | yes |
| S2-W2 redaction policy | named allowlist and denylist by field | yes |
| S2-W3 snapshot model | request-scoped snapshot design and drift behavior | yes |
| S2-W4 runtime projection schema | versioned runtime context contract | yes |
| S2-W5 fixture and scoring schema | frozen fixtures and acceptance threshold | yes |
| S2-W6 trace extension plan | HTTP, ACP, and persisted trace field plan | yes |
| S2-W7 implementation plan | ordered production changes and verification lane | yes |

## Implementation Gate

Production work may start only after peer review accepts:

1. runtime source inventory
2. redaction policy
3. snapshot consistency model
4. runtime projection schema
5. frozen Slice 2 fixtures
6. acceptance threshold
7. trace extension plan
8. verification lane

Until then, allowed work is limited to planning docs, fixture design, and read-only source inventory.

## Risks

| Risk | Mitigation |
| --- | --- |
| Runtime context leaks sensitive data | deny-by-default redaction, forbidden-field fixture checks, trace checks |
| Runtime state goes stale mid-response | request-scoped snapshot id, drift invalidation |
| Runtime projection becomes another bypass path | single-path invariant extended with runtime-present ghost fixtures |
| Trace grows too large | ids and hashes only; count-only fallback |
| Static and runtime hashes diverge silently | combined `projection_hash` plus explicit `static_envelope_hash` and `runtime_hash` |
| Resolver guesses when runtime data is absent | structured pending/refusal fixtures |

## Immediate Next Actions

1. [x] Create `slice-2-runtime-source-inventory.md`.
2. [x] Create `slice-2-redaction-policy.md`.
3. [x] Create `slice-2-snapshot-consistency-model.md`.
4. [x] Record the public API freeze decision in `public-api-freeze-decision.md`.
5. [x] Record the validation policy in `final-validation-plan.md`.
6. [x] Draft `slice-2-fixtures-v1.md` and `slice-2-fixtures-v1.jsonl`.
7. [x] Review this planning packet before production implementation.
8. [x] Implement the transport-neutral runtime projection layer and fixture-backed acceptance lane.
9. [x] Wire runtime projection into the DAG semantic response/trace path.
10. [x] Run the Slice 2 fixture set through the HTTP baseline runner.
