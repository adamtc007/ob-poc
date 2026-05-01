# Verb Flavour Classification 2026

Phase 7 tracker for SemOS DAG architecture §10.13.

## Completion Summary

Status: code-complete mechanical sweep.

The architecture estimate said "~149 verbs"; the current catalogue contains 149 YAML files and 1,288 concrete verb entries under `domains.*.verbs`. The Phase 7 acceptance surface is therefore 1,288 verb entries.

Classification counts:

| Flavour | Count |
|---|---:|
| `attribute_mutating` | 938 |
| `instance_adding` | 184 |
| `discretionary` | 166 |
| `tollgate` | 0 |

The full generated manifest is [verb-flavour-classification-2026.csv](verb-flavour-classification-2026.csv).

`flavour: tollgate` is intentionally unused in this sweep. Existing verbs named `tollgate.*` execute evaluation logic, mutate evaluation/override records, or disclose metrics; §2 I5 reserves tollgate flavour for empty-body gate verbs.

## Batch 7A — `rust/config/verbs/kyc/tollgate.yaml`

Status: applied as part of the full-corpus sweep.

| Verb FQN | Starting signals | Proposed flavour | Required aux fields | Rationale / review notes |
|---|---|---|---|---|
| `tollgate.evaluate` | `behavior: plugin`; `side_effects: state_write`; `state_effect: preserving`; `consequence: reviewable`; action signal `execute/evaluate` | `attribute_mutating` | none | Records an evaluation result and metrics. It is not a lifecycle tollgate verb with an empty body; it mutates operational evaluation records. |
| `tollgate.get-metrics` | `behavior: plugin`; `side_effects: facts_only`; `external_effects: [observational]`; `consequence: benign`; action signal `read/compute` | `attribute_mutating` | none | Read/compute disclosure surface. Phase 7 flavour enum has no read-only variant; classify as shape-blind attribute/read computation, not discretionary or instance-adding. |
| `tollgate.set-threshold` | `behavior: crud`; `operation: update`; `side_effects: state_write`; `consequence: reviewable`; action signal `update/set` | `discretionary` | `role_guard`, `audit_class` | Changes control policy thresholds. Needs explicit authority and audit classification even though the current three-axis baseline is only `reviewable`. |
| `tollgate.override` | `behavior: plugin`; `side_effects: state_write`; `external_effects: [emitting]`; `consequence: requires_explicit_authorisation`; action signal `approve/override` | `discretionary` | `role_guard`, `audit_class` | Management override of a failed gate is discretionary by definition. |
| `tollgate.list-evaluations` | `behavior: crud`; `operation: list_by_fk`; `side_effects: facts_only`; `external_effects: [observational]`; `consequence: benign`; action signal `list` | `attribute_mutating` | none | Read/list disclosure. Phase 7 flavour enum has no read-only variant; classify outside tollgate/discretionary/instance-adding. |
| `tollgate.list-thresholds` | `behavior: crud`; `operation: select`; `side_effects: facts_only`; `external_effects: [observational]`; `consequence: benign`; action signal `list` | `attribute_mutating` | none | Read/list disclosure of threshold config. |
| `tollgate.get-decision-readiness` | `behavior: plugin`; `side_effects: facts_only`; `external_effects: [observational]`; `consequence: benign`; action signal `read/compute` | `attribute_mutating` | none | Readiness computation only; no lifecycle progression. |
| `tollgate.list-overrides` | `behavior: crud`; `operation: select`; `side_effects: facts_only`; `external_effects: [observational]`; `consequence: benign`; action signal `list` | `attribute_mutating` | none | Read/list disclosure of existing discretionary override records. |
| `tollgate.expire-override` | `behavior: crud`; `operation: update`; `side_effects: state_write`; `external_effects: [emitting]`; `consequence: requires_explicit_authorisation`; action signal `update/expire` | `discretionary` | `role_guard`, `audit_class` | Early revocation/expiration of an approved override is an authority-bearing discretionary action. |
| `tollgate.check-gate` | `behavior: plugin`; `side_effects: state_write`; `state_effect: preserving`; `external_effects: [observational]`; `consequence: benign`; action signal `execute/check` | `attribute_mutating` | none | Writes/returns gate evaluation detail but is not the architecture's empty-body `tollgate` flavour. Naming is misleading; review before annotation. |

## Batch 7A Open Review Points

- The Phase 7 flavour enum has no `read_only` value. Read-only/list/compute verbs in this batch are proposed as `attribute_mutating` because they are neither instance-adding, discretionary, nor empty-body tollgate verbs.
- `tollgate.evaluate` and `tollgate.check-gate` are not proposed as `tollgate` flavour despite their names. They execute evaluation logic and/or write evaluation records, while §2 I5 says tollgate verbs have empty bodies.
- Suggested discretionary metadata, if Batch 7A is approved:
  - `tollgate.set-threshold`: `role_guard.any_of: [compliance_admin, mlro, senior_compliance]`; `audit_class: policy_threshold_change`.
  - `tollgate.override`: `role_guard.any_of: [senior_compliance, mlro, executive, board]`; `audit_class: tollgate_override`.
  - `tollgate.expire-override`: `role_guard.any_of: [senior_compliance, mlro, executive]`; `audit_class: tollgate_override_expiry`.
