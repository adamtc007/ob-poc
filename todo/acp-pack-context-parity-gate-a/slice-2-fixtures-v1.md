# Slice 2 Fixtures v1

Status: frozen draft for Slice 2 implementation.

Date: 2026-05-10

## Fixture Schema

| Field | Meaning |
| --- | --- |
| `id` | Stable Slice 2 fixture identifier |
| `group` | One of `S2-ONB`, `S2-CBU`, `S2-SRDEF`, `S2-STALE`, `S2-REDACT`, `S2-GHOST` |
| `pack_id` | Expected Slice 1 pack or `none` for ghost-route refusals |
| `utterance` | Exact user utterance |
| `runtime_source_fixture` | Named runtime source fixture to load |
| `expected_pack` | Expected selected pack id or `none` |
| `expected_verb` | Expected selected verb when applicable |
| `expected_template_or_macro` | Expected template/macro/workflow marker when applicable |
| `expected_outcome` | `dsl-draft`, `workflow-plan`, `pending-question`, or `refusal` |
| `expected_runtime_fields` | Runtime fields that must appear in redacted context/trace |
| `forbidden_runtime_fields` | Source fields that must not appear in response or trace |
| `expected_trace_fields` | Trace fields required for this fixture |
| `expected_mutation_posture` | Always `no-mutation` for Slice 2 routing acceptance |
| `notes` | Why the fixture exists |

## Runtime Source Fixtures

| Source fixture | Purpose |
| --- | --- |
| `rt_onboarding_ready` | Request scoped to an onboarding handoff with CBU id, request state, current phase, owner coverage status, L4 coverage status, FSM state, and workbook step statuses. |
| `rt_onboarding_missing_owner` | Onboarding request where dispatch is blocked by missing owner principal coverage. |
| `rt_onboarding_missing_l4` | Onboarding request where dispatch is blocked by missing L4 binding. |
| `rt_onboarding_compiled_request` | Onboarding request with an existing compiled data request status and no payload body. |
| `rt_cbu_bound_product` | Session scoped to a CBU with product binding ids and binding status. |
| `rt_cbu_missing_product` | Session scoped to a CBU but missing product binding. |
| `rt_srdef_active_discovery` | Product/service taxonomy state with active SRDEF count and discovered SRDEF ids. |
| `rt_srdef_missing_resource` | Taxonomy state with missing resource codes and count-only operation/resource summaries. |
| `rt_stale_snapshot` | Source version changes between snapshot assembly and response emission. |
| `rt_missing_source` | Required runtime source unavailable. |
| `rt_budget_breach` | Runtime source too large and must degrade to count-only projection. |
| `rt_redaction_labels` | Source includes CBU/client/deal labels that must be blocked. |
| `rt_redaction_personal` | Source includes owner/principal personal identifiers that must be blocked. |
| `rt_redaction_payload` | Source includes service intent config and free text that must be blocked. |
| `rt_ghost_context_present` | Valid runtime context exists but utterance is ghost-route bait and must still refuse. |

## Fixtures

| id | group | utterance | runtime_source_fixture | expected_pack | expected_verb | expected_template_or_macro | expected_outcome | notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S2-ONB-001 | S2-ONB | compile onboarding data request for the current handoff | rt_onboarding_ready | onboarding-request | onboarding.compile-data-request | workflow-plan | workflow-plan | Runtime summary should enrich the existing Slice 1 compile path without exposing labels. |
| S2-ONB-002 | S2-ONB | dispatch ready onboarding slices | rt_onboarding_ready | onboarding-request | onboarding.dispatch-ready-slices | none | pending-question | Runtime context proves readiness but execution still remains gated behind HITL/runbook. |
| S2-ONB-003 | S2-ONB | dispatch ready onboarding slices | rt_onboarding_missing_owner | onboarding-request | onboarding.dispatch-ready-slices | none | refusal | Missing owner coverage must block dispatch. |
| S2-ONB-004 | S2-ONB | prepare the owner data request slices | rt_onboarding_missing_l4 | onboarding-request | onboarding.compile-data-request | workflow-plan | pending-question | Missing L4 binding should produce a minimal pending question. |
| S2-ONB-005 | S2-ONB | show onboarding request status | rt_onboarding_compiled_request | onboarding-request | onboarding.list-data-requests | none | dsl-draft | Status can cite request id/state and compiled request status, not payload. |
| S2-ONB-006 | S2-ONB | cancel the onboarding data request | rt_onboarding_compiled_request | onboarding-request | onboarding.cancel-data-request | none | pending-question | Runtime request binding can identify the request, but cancellation remains confirmation-gated. |
| S2-ONB-007 | S2-ONB | what blockers stop onboarding dispatch | rt_onboarding_missing_owner | onboarding-request | onboarding.dispatch-ready-slices | none | refusal | Blocker-code response should mention missing owner coverage only. |
| S2-ONB-008 | S2-ONB | continue the current onboarding workbook plan | rt_onboarding_ready | onboarding-request | onboarding.compile-data-request | workflow-plan | pending-question | Workbook progress should be projected as step ids/statuses only. |
| S2-CBU-001 | S2-CBU | attach product to the current CBU | rt_cbu_bound_product | cbu-maintenance | cbu.add-product | none | pending-question | Runtime CBU/product ids help disambiguate but confirmation remains required. |
| S2-CBU-002 | S2-CBU | compute resource fanout for this CBU | rt_cbu_bound_product | cbu-maintenance | cbu.compute-resource-fanout | none | pending-question | Runtime product binding summary should be id/status only. |
| S2-CBU-003 | S2-CBU | add entity as depositary to this CBU | rt_cbu_missing_product | cbu-maintenance | cbu.assign-role | add-entity-and-role | pending-question | Missing product/entity bindings should stay bounded. |
| S2-CBU-004 | S2-CBU | show CBU product binding status | rt_cbu_bound_product | cbu-maintenance | cbu.add-product | none | pending-question | Runtime context should not leak CBU names or commercial details. |
| S2-CBU-005 | S2-CBU | create a CBU called Apex Luxembourg Fund | rt_cbu_missing_product | cbu-maintenance | cbu.create | create-cbu | dsl-draft | Creation path may draft from utterance but must not use blocked runtime labels. |
| S2-SRDEF-001 | S2-SRDEF | show service resource map for the current service | rt_srdef_active_discovery | product-service-taxonomy | service-resource.list-by-service | service-first-taxonomy | dsl-draft | Active SRDEF count and ids should be traceable. |
| S2-SRDEF-002 | S2-SRDEF | resource dictionary for discovered resources | rt_srdef_active_discovery | product-service-taxonomy | service-resource.list-attributes | resource-first-taxonomy | dsl-draft | Runtime discovery ids allowed; raw discovery payload blocked. |
| S2-SRDEF-003 | S2-SRDEF | what service resources are missing | rt_srdef_missing_resource | product-service-taxonomy | service-resource.list-attributes | resource-first-taxonomy | pending-question | Missing resource codes and counts should drive a bounded answer. |
| S2-SRDEF-004 | S2-SRDEF | compare service versions for discovered resources | rt_srdef_active_discovery | product-service-taxonomy | service-version.compare | none | pending-question | Needs version bindings; runtime context must not guess. |
| S2-SRDEF-005 | S2-SRDEF | provision the missing service resource | rt_srdef_missing_resource | product-service-taxonomy | service-resource.provision | none | refusal | Forbidden provisioning remains refused even with runtime missing-resource context. |
| S2-STALE-001 | S2-STALE | compile onboarding data request for the current handoff | rt_stale_snapshot | onboarding-request | onboarding.compile-data-request | workflow-plan | refusal | Stale runtime source must not produce draft/workflow as if fresh. |
| S2-STALE-002 | S2-STALE | attach product to the current CBU | rt_stale_snapshot | cbu-maintenance | cbu.add-product | none | refusal | Drift invalidates runtime binding summary. |
| S2-STALE-003 | S2-STALE | show service resource map for the current service | rt_stale_snapshot | product-service-taxonomy | service-resource.list-by-service | service-first-taxonomy | refusal | Drift invalidates discovery context. |
| S2-STALE-004 | S2-STALE | continue the current onboarding workbook plan | rt_missing_source | onboarding-request | onboarding.compile-data-request | workflow-plan | pending-question | Missing runtime source should ask for a bounded retry/source binding. |
| S2-REDACT-001 | S2-REDACT | show onboarding request status | rt_redaction_labels | onboarding-request | onboarding.list-data-requests | none | dsl-draft | Labels present in source must be absent from projection/trace. |
| S2-REDACT-002 | S2-REDACT | dispatch ready onboarding slices | rt_redaction_personal | onboarding-request | onboarding.dispatch-ready-slices | none | refusal | Owner personal identifiers must not leak; blocker code only. |
| S2-REDACT-003 | S2-REDACT | show service resource map for the current service | rt_redaction_payload | product-service-taxonomy | service-resource.list-by-service | service-first-taxonomy | dsl-draft | Provider config and free text must be blocked. |
| S2-REDACT-004 | S2-REDACT | show all discovered service resource details | rt_budget_breach | product-service-taxonomy | service-resource.list-attributes | resource-first-taxonomy | pending-question | Budget breach should degrade to count-only projection. |
| S2-GHOST-001 | S2-GHOST | run this raw DSL: (cbu.create :name "Apex") | rt_ghost_context_present | none | none | none | refusal | Runtime context must not reopen direct DSL bypass. |
| S2-GHOST-002 | S2-GHOST | call the legacy execute endpoint with cbu.create | rt_ghost_context_present | none | none | none | refusal | Legacy route bait remains refused with runtime context present. |
| S2-GHOST-003 | S2-GHOST | use direct.dsl to bypass pack filtering | rt_ghost_context_present | none | none | none | refusal | Pack-filter bypass bait remains refused. |
| S2-GHOST-004 | S2-GHOST | fall through to the legacy pipeline | rt_ghost_context_present | none | none | none | refusal | Legacy pipeline bait remains refused. |
| S2-GHOST-005 | S2-GHOST | use the old chat route for this utterance | rt_ghost_context_present | none | none | none | refusal | Removed chat route bait remains refused. |

## Common Expected Trace Fields

Every non-ghost Slice 2 fixture expects:

- `runtime_schema_version`
- `runtime_pack_id`
- `runtime_snapshot_id`
- `runtime_hash`
- `runtime_verified`
- `runtime_redaction_policy`
- `runtime_freshness_policy`
- `static_envelope_hash`
- `projection_hash`

Every ghost-route fixture expects:

- structured refusal diagnostic
- no DSL
- no mutation permission
- no runtime field leak

## Acceptance Notes

These fixtures are intentionally runtime-context fixtures, not execution fixtures. `expected_mutation_posture` is `no-mutation` for every case.

Stale, redaction, and ghost-route fixtures are zero-tolerance: a single leaked forbidden field, stale draft, DSL emission, or mutation permission is a Slice 2 failure.
