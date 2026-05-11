# Slice 2 Snapshot Consistency Model

Status: draft; implementation blocked pending review.

Date: 2026-05-10

## Model

Slice 2 uses a request-scoped runtime snapshot.

One user utterance gets one runtime snapshot id. All runtime fields projected into ACP context for that utterance must be derived from that snapshot.

## Snapshot Identity

Required fields:

| Field | Meaning |
| --- | --- |
| `snapshot_id` | UUID or content-addressed id for this runtime read set |
| `snapshot_created_at` | UTC timestamp when the read set was assembled |
| `source_version_refs` | source ids and version/hash/timestamp refs |
| `runtime_hash` | deterministic hash of the redacted runtime projection |
| `static_envelope_hash` | static pack envelope hash consumed with this runtime projection |
| `projection_hash` | combined hash of static envelope hash plus runtime hash |

## Read Boundary

Preferred initial implementation:

1. Resolve pack using the existing Slice 1 static path.
2. Verify static envelope registry state.
3. Build runtime read set from in-memory session state where possible.
4. For DB-backed fields, read within a single transaction or explicit snapshot boundary.
5. Redact the runtime projection.
6. Hash the redacted projection.
7. Attach runtime trace fields to response and persisted trace.

If the system cannot produce a single consistent read boundary, it must fail closed with a structured diagnostic.

## Drift Detection

Snapshot drift means any source version ref changes between snapshot assembly and response emission.

Required behavior:

| Drift case | Response |
| --- | --- |
| source changed before projection hash is finalized | retry once or refuse as stale |
| source changed after projection hash but before response emission | refuse as stale |
| source unavailable | pending question or structured refusal |
| source too large for budget | count-only reduced projection |
| source contains blocked fields needed for answer | redaction refusal/pending |

Preferred diagnostic code prefix:

```text
runtime_context_stale_
```

## Freshness Policy

Initial policy id:

```text
slice2_runtime_context_same_request_v1
```

Rules:

- runtime context is valid only for the current utterance response
- no cross-turn reuse unless the same `snapshot_id` is explicitly revalidated
- timestamps older than the current request are allowed only as source facts, not as freshness proof
- stale or missing source refs must not produce DSL drafts

## Hashing Rules

The runtime hash should be computed from canonical JSON after redaction.

Excluded from `runtime_hash`:

- wall-clock response duration
- raw prompts
- raw completions
- logs
- unredacted source payloads

Included in `runtime_hash`:

- schema version
- pack id
- snapshot id
- redaction policy id
- freshness policy id
- redacted runtime field values
- blocked field codes
- source refs

`projection_hash` should bind:

- static envelope hash
- runtime hash
- runtime schema version
- pack id

## Trace Requirements

Persisted trace should include:

- `runtime_schema_version`
- `runtime_snapshot_id`
- `runtime_hash`
- `runtime_verified`
- `runtime_redaction_policy`
- `runtime_freshness_policy`
- `static_envelope_hash`
- `projection_hash`
- `runtime_blocked_field_codes`

It must not include unredacted source payloads.

## Tests

Required focused tests before broad validation:

1. same input and same source refs produce the same `runtime_hash`
2. blocked field present in source does not appear in projection or trace
3. source version drift produces stale diagnostic and no DSL
4. budget breach produces count-only projection
5. ghost-route bait with runtime context still produces refusal/no DSL

## Review Decision

Proposed decision: implement request-scoped snapshots only. Defer cached or cross-turn runtime context until the one-shot snapshot path is proven.
