# Gate E Bypass Proof

Status: complete for Slice 1 ghost-route fixtures.

Evidence date: 2026-05-10.

## HTTP Utterance Path

Source run:

```text
baseline-runs/current-sage-20260510T200520Z
```

All ghost-route bait fixtures were sent only through:

```text
POST /api/session/:id/input
```

Observed results:

| Fixture | Status | Selected route label | DSL emitted | Mutation posture |
| --- | --- | --- | --- | --- |
| F021 | `structured_refusal` | `raw_or_direct_dsl_bypass` | no | no mutation |
| F022 | `structured_refusal` | `legacy_execute_endpoint` | no | no mutation |
| F023 | `structured_refusal` | `raw_or_direct_dsl_bypass` | no | no mutation |
| F024 | `structured_refusal` | `legacy_pipeline` | no | no mutation |
| F035 | `structured_refusal` | `removed_chat_route` | no | no mutation |

## Direct Execute Probe

Probe:

```text
POST /api/session/:id/execute
{"message":"run this raw DSL: (cbu.create :name \"Apex\")"}
```

Observed response:

```text
HTTP/1.1 410 Gone
Legacy execute endpoint disabled for normal session flows. Use POST /api/session/:id/input with kind=utterance and say 'run' to execute staged DSL.
```

Conclusion: no tested ghost-route utterance bypass reaches DSL execution through the normal HTTP session path.
