# Pattern D Verbs (transition_args)

Pattern D verbs declare `transition_args` metadata which tells the V1.3
runtime gate hook which arguments carry the entity_id and optional target
state for a DAG transition. These verbs are pre-annotated in the generated
DSL files with a `;  Pattern D: transition_args present` comment.

## Why flagged?

In a future Tranche these verbs are candidates for stronger DSL-native
expression of their transition semantics — e.g. a dedicated `(transition ...)`
slot or a `(verb-transition ...)` atom kind. For now, `transition_args` is
preserved as JSON in the `:transition-args-json` slot and round-trips
losslessly through the DSL loader.

## Generator output

The `verb_to_dsl` binary annotates each Pattern D verb in the generated
`.dsl` file:

```lisp
; Pattern D: transition_args present — see docs/verb-redesigns/
(verb domain.verb-name
  :transition-args-json "{\"entity_id_arg\":\"...\",\"target_state_arg\":\"...\"}"
  ; ... other slots
)
```

## Pattern D verb source files (37 YAML files, 210 verbs)

| Source YAML | Notes |
|-------------|-------|
| `config/verbs/application-instance.yaml` | Application instance lifecycle |
| `config/verbs/attribute.yaml` | Attribute definition lifecycle |
| `config/verbs/billing.yaml` | Billing period lifecycle |
| `config/verbs/book.yaml` | Book status lifecycle |
| `config/verbs/booking-principal-clearance.yaml` | Clearance state machine |
| `config/verbs/bpmn-controller.yaml` | BPMN controller lifecycle |
| `config/verbs/capability-binding.yaml` | Capability binding lifecycle |
| `config/verbs/catalogue.yaml` | Catalogue DAG transitions |
| `config/verbs/cbu-ca.yaml` | CBU corporate action lifecycle |
| `config/verbs/cbu.yaml` | CBU DAG state transitions |
| `config/verbs/collateral-management.yaml` | Collateral lifecycle |
| `config/verbs/corporate-action-event.yaml` | Corporate action event lifecycle |
| `config/verbs/custody/settlement-chain.yaml` | Settlement chain lifecycle |
| `config/verbs/custody/trade-gateway.yaml` | Trade gateway lifecycle |
| `config/verbs/deal.yaml` | Deal DAG transitions |
| `config/verbs/delivery.yaml` | Delivery lifecycle |
| `config/verbs/entity.yaml` | Entity lifecycle transitions |
| `config/verbs/instrument-matrix.yaml` | Instrument matrix state transitions |
| `config/verbs/kyc/evidence.yaml` | Evidence lifecycle |
| `config/verbs/kyc/kyc-case.yaml` | KYC case lifecycle |
| `config/verbs/kyc/red-flag.yaml` | Red flag lifecycle |
| `config/verbs/manco-group.yaml` | Management company group lifecycle |
| `config/verbs/onboarding.yaml` | Onboarding request lifecycle |
| `config/verbs/phrase.yaml` | Phrase authoring lifecycle |
| `config/verbs/reconciliation.yaml` | Reconciliation lifecycle |
| `config/verbs/registry/holding.yaml` | Holding lifecycle |
| `config/verbs/registry/investor.yaml` | Investor lifecycle |
| `config/verbs/registry/share-class.yaml` | Share class lifecycle |
| `config/verbs/screening.yaml` | Screening lifecycle |
| `config/verbs/sem-reg/governance.yaml` | SemReg governance lifecycle |
| `config/verbs/service-consumption.yaml` | Service consumption lifecycle |
| `config/verbs/service-pipeline.yaml` | Service pipeline lifecycle |
| `config/verbs/service-resource.yaml` | Service resource lifecycle |
| `config/verbs/service-version.yaml` | Service version lifecycle |
| `config/verbs/service.yaml` | Service lifecycle |
| `config/verbs/trading-profile.yaml` | Trading profile DAG transitions (largest: 14 Pattern D verbs) |

## Total count

210 Pattern D verbs across 37 YAML source files.
