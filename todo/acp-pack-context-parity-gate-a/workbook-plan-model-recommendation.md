# Workbook Plan Model Recommendation

Status: audit draft for Gate A replan.

Evidence:

- 7 workflow YAML files under `rust/config/workflows`.
- 12 journey packs with pack-local `templates`.
- The baseline fixtures expect both `dsl-draft` and `workflow-plan` outcomes.

Recommendation:

Treat workbook plans as first-class SemOS planning entities when they affect routing, state interpretation, or macro selection. Do not collapse them into macros by naming convention.

Minimal model for Slice 1:

| Field | Purpose |
| --- | --- |
| `plan_id` | Stable routeable id. |
| `pack_id` | Pack scope and collision boundary. |
| `trigger_phrases` | Route hints separate from verb invocation phrases. |
| `required_bindings` | Pending-question source. |
| `steps` | Ordered verbs/macros with binding expressions. |
| `risk_policy` | Confirmation/HITL/dry-run gates. |
| `state_effects` | Static expected state transitions, not runtime instances. |
| `refusal_conditions` | Structured refusal reasons. |

Gate B decision:

Lift `onboarding-request` workflow-plan fixtures and `cbu-maintenance` create/add-role templates first. Defer unrelated KYC workflow plans unless the reviewer expands Slice 1.
