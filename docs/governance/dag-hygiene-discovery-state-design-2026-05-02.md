# DAG Hygiene Discovery State Design - 2026-05-02

Status: Implemented design packet retained as governance evidence.

## Proposed State Slot

- Carrier: `"ob-poc".cbus`
- Column: `cbu_discovery_state`
- DAG taxonomy: `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml`
- States: `PENDING`, `DISCOVERING`, `ROLLUP`, `POPULATE`, `PROVISION`, `READY`, `FAILED`, `BLOCKED`
- Initial/default state: `PENDING`

## Current Dispatch Shape

Runtime plugin ops in `rust/crates/sem_os_postgres/src/ops/service_pipeline.rs` are thin dispatchers into `ObPocServicePipelineService::dispatch_service_pipeline_verb`.

Actual write logic is in `rust/src/services/service_pipeline_service_impl.rs`.

| Verb | Current owner arg | Can identify CBU? | Current write surface | Proposed state emission |
| --- | --- | --- | --- | --- |
| `service-intent.create` | `cbu-id` | yes, direct arg | inserts `service_intents` | `PENDING` |
| `service-intent.supersede` | `intent-id` | yes, lookup `service_intents.cbu_id` before write | updates old intent to `superseded`, inserts new intent | `PENDING` for resolved CBU |
| `discovery.run` | `cbu-id` | yes, direct arg | writes `srdef_discovery_reasons`, attribute rollup/value surfaces through pipeline | `DISCOVERING` on entry, `ROLLUP` on success |
| `attributes.rollup` | `cbu-id` | yes, direct arg | writes/updates CBU attribute requirements | `ROLLUP` on entry/success |
| `attributes.populate` | `cbu-id` | yes, direct arg | writes CBU attribute values | `POPULATE` on success |
| `attributes.set` | `cbu-id` | yes, direct arg | writes one CBU attribute value | no slot transition; attribute-level repair only |
| `provisioning.run` | `cbu-id` | yes, direct arg | writes provisioning requests/events and resource instances | `PROVISION` on entry, `READY` if result has no blocked/not-ready services, otherwise `BLOCKED`; `FAILED` on execution error after state support exists |
| `readiness.compute` | `cbu-id` | yes, direct arg | writes `cbu_service_readiness` | `READY` when blocked count is 0, otherwise `BLOCKED` |
| `pipeline.full` | `cbu-id` | yes, direct arg | orchestrates discovery and provisioning directly | either emit each coarse stage internally or refactor to child-dispatch later; for Phase 4.c emit `DISCOVERING`, `ROLLUP`, `PROVISION`, then `READY`/`BLOCKED` from result |
| `service-resource.sync-definitions` | none | no CBU owner; global SRDEF catalogue sync | writes `service_resource_types` and attribute requirements | no CBU discovery-state emission |

Read-only or diagnostic verbs (`service-intent.list`, `discovery.explain`, `attributes.gaps`, `provisioning.status`, `readiness.explain`, `service-resource.check-attribute-gaps`) should not mutate `cbu_discovery_state`.

## Implementation Notes

- The SemOS op wrapper currently owns `VerbExecutionContext`, but the service implementation does not. Phase 4.c should either emit in the wrapper after dispatch or extend the service trait if result-aware emission needs to happen inside service code.
- Result-aware states need service result inspection:
  - `readiness.compute`: `blocked == 0` means `READY`; otherwise `BLOCKED`.
  - `provisioning.run`: `not_ready == 0` and `services_blocked == 0` means provisioned enough to move toward `READY`; otherwise `BLOCKED`.
  - `pipeline.full`: use `readiness.services_blocked` in the returned record.
- `FAILED` cannot be emitted after a hard `Err` unless the dispatch wrapper catches the error, emits, and rethrows. That is architecturally acceptable, but should be explicit in Phase 4.c.
- `service-intent.supersede` is implementable because the service already queries `(cbu_id, product_id, service_id)` by `intent-id` before writing.
- `service-resource.sync-definitions` is intentionally out of scope for the CBU slot because it has no owning CBU.

## Phase 4.b Readiness

No blocking arg-shape gap was found for the CBU-owned write-intent verbs. Phase 4.b can add the `cbus.cbu_discovery_state` migration and DAG slot.
