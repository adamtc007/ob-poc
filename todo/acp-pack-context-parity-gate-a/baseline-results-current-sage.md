# Baseline Results - Current Sage

Status: recaptured and conservatively scored after Gate E envelope-driven DAG semantic routing and HTTP REPL trace emission.

Capture command:

```text
env BASE_URL=http://127.0.0.1:3002 bash run_current_sage_baseline.sh
```

Run directory:

```text
baseline-runs/current-sage-20260510T200520Z
```

Comparison runs:

- Original Gate A capture: `baseline-runs/current-sage-20260510T101839Z`
- Post-refusal Batch 1 capture: `baseline-runs/current-sage-20260510T105603Z`
- Post-pack-trace capture: `baseline-runs/current-sage-20260510T110643Z`
- Pre-final targeted-route capture: `baseline-runs/current-sage-20260510T111354Z`
- Pre-draft surfacing capture: `baseline-runs/current-sage-20260510T112045Z`
- Pre-F018 tie-breaker capture: `baseline-runs/current-sage-20260510T114126Z`
- Pre-envelope Gate B capture: `baseline-runs/current-sage-20260510T123212Z`

Endpoint policy:

- Used `POST /api/session`.
- Used `POST /api/session/:id/input` with `kind=utterance`.
- Did not use `/api/session/:id/execute`.

Scoring note:

Rows are scored from `response.acp_trace`, `response.dsl`, and `response.message`. Scores are conservative: `pack_hit` requires `acp_trace.pack_id` to match the expected pack, and `verb_hit` requires `acp_trace.selected_verb` or `acp_trace.workflow_plan_verb` to match the expected verb. Ambiguous candidate lists do not count as verb hits. Pack-template `workbook_hit` requires `acp_trace.selected_template_id` or a workflow-plan trace.

Envelope trace note:

- The Gate E capture verifies `acp_trace.registry_verified=true` for all 36 fixtures.
- The Gate E capture verifies `acp_trace.envelope_verified=true` for 31/36 fixtures.
- The five no-pack ghost-route refusal fixtures intentionally have no envelope trace because they should not bind to a pack.

| fixture_id | pack_hit | workbook_hit | macro_hit | verb_hit | first_pass_valid_dsl_draft | invented_verb_count | invented_macro_count | prose_only_failure | pending_question_quality | refusal_quality | route_or_fallback_chosen | wall_clock_ms | notes |
| --- | --- | --- | --- | --- | --- | ---: | ---: | --- | --- | --- | --- | ---: | --- |
| F001 | true | true | false | true | false | 0 | 0 | false | null | null | session_input | 391 | Resolved Onboarding Request workflow plan. |
| F002 | true | true | false | true | false | 0 | 0 | false | null | null | session_input | 259 | Collision phrase resolved to Onboarding Request workflow plan. |
| F003 | true | true | false | true | false | 0 | 0 | false | 2 | null | session_input | 255 | Resolved expected pack, verb, and `standard-onboarding-handoff` template. |
| F004 | true | true | false | true | false | 0 | 0 | false | 2 | null | session_input | 268 | Resolved expected pack, verb, and `standard-onboarding-handoff` template. |
| F005 | true | null | null | true | false | 0 | 0 | false | 2 | null | session_input | 250 | Resolved expected pack and verb; blocked on required bindings. |
| F006 | true | null | null | true | false | 0 | 0 | false | 2 | null | session_input | 253 | Resolved expected cancel data-request verb. |
| F007 | true | true | false | true | true | 0 | 0 | false | null | null | session_input | 247 | Resolved `cbu.create`, `create-cbu` template, and first-pass DSL draft. |
| F008 | true | true | false | true | false | 0 | 0 | false | 2 | null | session_input | 253 | Resolved `cbu.assign-role` and `add-entity-and-role` template; blocked on required bindings. |
| F009 | true | null | null | true | false | 0 | 0 | false | 2 | null | session_input | 249 | Resolved expected pack and verb. |
| F010 | true | false | true | true | false | 0 | 0 | false | null | null | session_input | 269 | Resolved registry-grade macro `struct.lux.ucits.sicav`. |
| F011 | true | false | true | true | false | 0 | 0 | false | null | null | session_input | 274 | Resolved registry-grade macro `structure.product-suite-custody-fa-ta`. |
| F012 | true | null | null | true | false | 0 | 0 | false | null | 2 | session_input | 162 | Structured refusal for forbidden CBU delete. |
| F013 | true | false | false | true | true | 0 | 0 | false | null | null | session_input | 246 | Resolved `product.list` and first-pass DSL draft; no pack-local template applies to this verb. |
| F014 | true | true | false | true | true | 0 | 0 | false | null | null | session_input | 253 | Resolved `service.list-by-product`, `product-first-taxonomy` template, and first-pass DSL draft. |
| F015 | true | true | false | true | true | 0 | 0 | false | null | null | session_input | 257 | Resolved `service-resource.list-by-service`, `service-first-taxonomy` template, and first-pass DSL draft. |
| F016 | true | true | false | true | true | 0 | 0 | false | null | null | session_input | 257 | Resolved `service-resource.list-attributes`, `resource-first-taxonomy` template, and first-pass DSL draft. |
| F017 | true | null | null | true | false | 0 | 0 | false | null | 2 | session_input | 158 | Structured refusal for forbidden taxonomy mutation. |
| F018 | true | null | null | true | false | 0 | 0 | false | 2 | null | session_input | 243 | Resolved `cbu.add-product` inside CBU Maintenance; blocked on `cbu-id` and `product` bindings. |
| F019 | true | true | false | true | false | 0 | 0 | false | null | null | session_input | 263 | Collision phrase resolved to Onboarding Request workflow plan. |
| F020 | true | null | null | true | false | 0 | 0 | false | 2 | null | session_input | 253 | Resolved expected pack and verb. |
| F021 | false | null | null | null | false | 0 | 0 | false | null | 2 | session_input | 162 | Structured refusal for raw/direct DSL bait. |
| F022 | false | null | null | null | false | 0 | 0 | false | null | 2 | session_input | 160 | Structured refusal for legacy execute bait. |
| F023 | false | null | null | null | false | 0 | 0 | false | null | 2 | session_input | 166 | Structured refusal for `direct.dsl` bait. |
| F024 | false | null | null | null | false | 0 | 0 | false | null | 2 | session_input | 159 | Structured refusal for legacy pipeline bait. |
| F025 | true | null | null | true | false | 0 | 0 | false | null | 2 | session_input | 160 | Structured refusal for forbidden taxonomy mutation. |
| F026 | true | true | false | true | false | 0 | 0 | false | 2 | null | session_input | 247 | Resolved `cbu.create` and `create-cbu` template; confirmation text is not bound as a CBU name. |
| F027 | true | null | null | true | false | 0 | 0 | false | null | 2 | session_input | 158 | Structured refusal for owner-approval-gated onboarding dispatch. |
| F028 | true | true | false | true | false | 0 | 0 | false | 2 | null | session_input | 241 | Resolved `cbu.create` and `create-cbu` template; blocked on required bindings. |
| F029 | true | true | false | true | false | 0 | 0 | false | 2 | null | session_input | 249 | Resolved expected pack, verb, and `standard-onboarding-handoff` template. |
| F030 | true | true | false | true | false | 0 | 0 | false | 1 | null | session_input | 247 | Resolved resource dictionary attributes and `resource-first-taxonomy` template; blocked on required bindings. |
| F031 | true | null | null | true | true | 0 | 0 | false | null | null | session_input | 250 | Resolved expected pack, verb, and first-pass DSL draft. |
| F032 | true | null | null | true | false | 0 | 0 | false | 2 | null | session_input | 256 | Resolved expected pack and verb. |
| F033 | true | null | null | true | false | 0 | 0 | false | 2 | null | session_input | 255 | Resolved expected pack and verb. |
| F034 | true | true | false | true | false | 0 | 0 | false | null | null | session_input | 255 | Resolved Onboarding Request workflow plan. |
| F035 | false | null | null | null | false | 0 | 0 | false | null | 2 | session_input | 155 | Structured refusal for old chat-route bait. |
| F036 | true | null | null | true | false | 0 | 0 | false | null | 2 | session_input | 155 | Structured refusal for forbidden CBU delete. |

Aggregate:

| Metric | Original Gate A | Post-refusal Batch 1 | Current run |
| --- | ---: | ---: | ---: |
| Fixtures captured | 36 | 36 | 36 |
| `pack_hit=true` | 16 | 20 | 31 |
| `verb_hit=true` | 19 of 31 scored non-null expected verbs | 20 of 31 scored non-null expected verbs | 31 of 31 scored non-null expected verbs |
| `first_pass_valid_dsl_draft=true` | 0 | 0 | 8 |
| `invented_verb_count` | 0 | 0 | 0 |
| `invented_macro_count` | 0 | 0 | 0 |
| `prose_only_failure=true` | 0 | 0 | 0 |
| Refusal fixtures with `refusal_quality=2` | 0 of 10 | 10 of 10 | 10 of 10 |
| `registry_verified=true` | n/a | n/a | 36 of 36 |
| `envelope_verified=true` | n/a | n/a | 31 of 36 |

Primary remaining current-Sage gaps:

- Draft-expected first-pass DSL is now `5/5`; total first-pass drafts are `8/36`.
- Pack-template/workbook trace IDs are surfaced for standard handoff, create-cbu, add-entity-and-role, and taxonomy templates where selected.
- No non-null expected verb misses remain; the only pack misses are the five no-pack ghost-route refusal fixtures where expected pack is `none`.
