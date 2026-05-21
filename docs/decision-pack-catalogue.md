# Decision Pack Catalogue

> **Generated from:** `rust/dsl-source/packs/*.dsl`  
> **Tranche:** 9 (DSL v0.1)  
> **Last updated:** 2026-05-21

This catalogue documents the 12 decision packs shipped with the ob-poc unified
DSL v0.1.  Each pack is a governed, versioned, reusable decision-pattern recipe.
Sage discovers packs via the pack registry and expands them into concrete DSL
fragments at authoring time.

---

## Pack 1: `conjunctive-gate`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, onboarding, screening  

**Description**: All N conditions must be satisfied; single gateway routes to
enhanced or standard path.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| conditions | list-of-condition-expr | yes | Conditions that must ALL be true for the enhanced path |
| gate-name | symbol | yes | Name for the generated gateway atom |
| enhanced-path | node-ref | yes | Target node when all conditions hold |
| standard-path | node-ref | yes | Target node (default) when any condition fails |

**Example utterances**:
- "all checks must pass before activation"
- "only proceed if KYC, screening, and UBO are all approved"
- "all conditions satisfied -> enhanced path, otherwise standard"
- "when every requirement is met, route to fast track"
- "all of these must be true before we can activate"

**Structural signature**: `:conditions-composition and :gateway-kind exclusive :outcomes 2`

---

## Pack 2: `disjunctive-gate`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, screening, onboarding  

**Description**: Any one of N conditions routes to escalation path; standard
path if none hold.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| conditions | list-of-condition-expr | yes | Conditions; any one being true routes to the escalation path |
| gate-name | symbol | yes | Name for the generated gateway atom |
| escalation-path | node-ref | yes | Target node when any condition holds |
| standard-path | node-ref | yes | Target node (default) when no condition holds |

**Example utterances**:
- "if any red flag is present, escalate"
- "any one of these conditions triggers enhanced review"
- "escalate if KYC rejected OR sanctions hit OR PEP positive"
- "if any risk indicator fires, route to compliance"
- "any of these conditions -> heightened scrutiny"

**Structural signature**: `:conditions-composition or :gateway-kind exclusive :outcomes 2`

---

## Pack 3: `sanction-hit-escalation`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, screening, compliance  

**Description**: Sanctions check service task; hard-block exclusive gateway:
any hit value escalates immediately.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| sanctions-check-name | symbol | yes | Name for the generated sanctions check service task node |
| sanctions-gate-name | symbol | yes | Name for the generated gateway |
| sanctions-field | string | yes | Data location where the sanctions check writes its result |
| hit-value | string | no (default: "hit") | The result value that constitutes a hit |
| escalation-path | node-ref | yes | Target node when a hit is detected |
| clear-path | node-ref | yes | Target node (default) when clear |

**Example utterances**:
- "if there's a sanctions match, immediately escalate to compliance"
- "sanctions hit -> hard block, route to compliance officer"
- "screening: positive sanctions result overrides everything"
- "any sanctions hit must go to manual review regardless"
- "hard block on sanctions: escalate immediately"

**Structural signature**: `:check-kind sanctions-lookup :gateway-kind exclusive :hard-block true :outcomes 2`

---

## Pack 4: `parallel-evaluation-with-veto`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, screening  

**Description**: Two parallel evaluation tasks; any single veto at join blocks
the application.  Representative template for N=2; N>=3 follows the same
structural pattern.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| fork-name | symbol | yes | Name for the parallel fork gateway |
| join-name | symbol | yes | Name for the parallel join |
| post-join-gate | symbol | yes | Name for the gateway after the join |
| eval-task-1 | node-ref | yes | First existing evaluation task node |
| eval-task-2 | node-ref | yes | Second existing evaluation task node |
| veto-field | string | no (default: "veto-result") | Data location holding veto result |
| vetoed-path | node-ref | yes | Path taken when any branch vetoes |
| approved-path | node-ref | yes | Path taken when all branches approve |

**Example utterances**:
- "run all checks in parallel; if any rejects, the whole application is rejected"
- "parallel screening: a single hit blocks the process"
- "concurrent evaluation with veto semantics"
- "all these checks happen simultaneously; any failure fails the whole thing"
- "parallel due diligence; one veto is enough to reject"

**Structural signature**: `:evaluation-order parallel :join-kind parallel :veto-semantics union-any :post-join-gateway exclusive :outcomes 2`

---

## Pack 5: `periodic-refresh-trigger`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, periodic-review  

**Description**: Exclusive gateway: if timestamp field age exceeds threshold
months, route to refresh; otherwise continue.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| age-gate-name | symbol | yes | Name for the generated gateway |
| timestamp-field | string | yes | Data location of the last-refreshed timestamp |
| threshold-months | integer | yes | Age threshold in months |
| refresh-path | node-ref | yes | Path taken when the record is stale |
| current-path | node-ref | yes | Path taken when within threshold (default) |

**Example utterances**:
- "if KYC was last refreshed more than 12 months ago, trigger a refresh"
- "periodic KYC refresh: escalate if stale"
- "check if last review is older than the configured period"
- "time-based trigger: refresh if over threshold age"
- "annual review: if more than 12 months, re-verify"

**Structural signature**: `:input-kind timestamp :check-kind age :gateway-kind exclusive :outcomes 2`

---

## Pack 6: `required-evidence-checklist`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, onboarding  

**Description**: Three sequential evidence tasks; final gateway evaluates
aggregate.  Representative for N=3 tasks.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| task-1 | node-ref | yes | First existing evidence task node |
| task-2 | node-ref | yes | Second evidence task node |
| task-3 | node-ref | yes | Third evidence task node |
| checklist-gate-name | symbol | yes | Name for the final evaluation gateway |
| approval-path | node-ref | yes | Path taken when aggregate condition holds |
| rejection-path | node-ref | yes | Path taken (default) when aggregate fails |
| aggregate-condition | condition-expr | yes | Boolean over evidence task outputs; must hold for approval-path |

**Example utterances**:
- "collect and verify all required documents before making a decision"
- "sequential evidence checklist: ID, address, source of wealth"
- "each piece of evidence must be verified in order"
- "step-by-step document verification before final approval"
- "checklist: all evidence collected and verified -> proceed"

**Structural signature**: `:evaluation-order sequential :evidence-collection true :final-gateway exclusive :outcomes 2`

---

## Pack 7: `threshold-band-routing`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, ubo  

**Description**: Numeric value partitioned into 3 bands; each band routes to a
distinct path.  Representative for 3 bands.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| band-gate-name | symbol | yes | Name for the routing gateway |
| input-field | string | yes | Data location of the numeric value to classify |
| threshold-low | integer | yes | Upper bound of the low band (inclusive) |
| threshold-mid | integer | yes | Upper bound of the medium band (inclusive) |
| path-low | node-ref | yes | Path for values at or below threshold-low |
| path-mid | node-ref | yes | Path for values in the mid band |
| path-high | node-ref | yes | Path for values above threshold-mid (default) |

**Example utterances**:
- "route by ownership percentage: below 10% is minor, 10-25% is significant, above 25% is controlling"
- "tiered risk scoring: low/medium/high bands"
- "threshold-based routing on credit limit"
- "bands: 0-25% standard, 25-50% enhanced, 50%+ controlling"
- "ownership tier routing"

**Structural signature**: `:input-kind numeric :gateway-kind exclusive :band-count 3 :band-semantics ordered-threshold`

---

## Pack 8: `multi-jurisdiction-overlay`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, deal, compliance  

**Description**: Jurisdiction-conditional routing to jurisdiction-specific
processes.  Representative for 2 explicit jurisdictions plus default.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| jur-gate-name | symbol | yes | Name for the jurisdiction routing gateway |
| jurisdiction-field | string | yes | Data location holding the ISO jurisdiction code |
| jurisdiction-a | string | yes | Jurisdiction code for the first explicit path |
| path-a | node-ref | yes | Path for jurisdiction-a |
| jurisdiction-b | string | yes | Jurisdiction code for the second explicit path |
| path-b | node-ref | yes | Path for jurisdiction-b |
| default-path | node-ref | yes | Path for all other jurisdictions |

**Example utterances**:
- "apply UK rules for UK clients, EU rules for EU clients, otherwise global standard"
- "jurisdiction-specific compliance routing"
- "different process per domicile"
- "route by jurisdiction: each country has its own requirements"
- "apply the relevant regulatory regime based on jurisdiction"

**Structural signature**: `:routing-key jurisdiction-string :gateway-kind exclusive :outcomes variable`

---

## Pack 9: `linked-switch-chain`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, onboarding  

**Description**: Sequential exclusive gateways; each check may fast-exit or
proceed to next.  Representative template for N=2 checks; N>=3 follows the same
structural pattern.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| gate-1-name | symbol | yes | Name for the first gateway |
| gate-2-name | symbol | yes | Name for the second gateway |
| condition-1 | condition-expr | yes | First check: if this FAILS, take exit-path-1 |
| condition-2 | condition-expr | yes | Second check: if this FAILS, take exit-path-2 |
| exit-path-1 | node-ref | yes | Fast-exit when condition-1 fails |
| exit-path-2 | node-ref | yes | Fast-exit when condition-2 fails |
| final-path | node-ref | yes | Destination when both checks pass |

**Example utterances**:
- "first verify identity, then check sanctions - exit early on any failure"
- "sequential checks with early exit on failure"
- "step-by-step eligibility: verify each requirement in order"
- "chain of compliance checks, each with a rejection path"
- "waterfall decision: each gate can reject before the next"

**Structural signature**: `:evaluation-order sequential :gateway-kind exclusive :early-exit true :fixed-checks 2`

---

## Pack 10: `cascading-decision`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, deal  

**Description**: Two-stage decision: first decision classifies; second decision
applies the appropriate ruleset for the classification.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| primary-eval-name | symbol | yes | Name for the primary evaluation task |
| primary-gate-name | symbol | yes | Name for the primary routing gateway |
| primary-decision | decision-ref | yes | Decision table for primary classification |
| output-field | string | yes | Data location where the primary classification is written |
| class-a-value | string | yes | The classification value that routes to path-a |
| path-a | node-ref | yes | Path for class-a classification |
| path-b | node-ref | yes | Default path for all other classifications |

**Example utterances**:
- "first classify by entity type, then apply the appropriate rules for that type"
- "two-stage decision: entity type determines which ruleset applies"
- "primary classification feeds secondary decision"
- "the first check determines which second check to run"
- "cascading rules: output of step 1 selects step 2"

**Structural signature**: `:stages 2 :evaluation-order sequential :gateway-kind exclusive :first-output-drives-second true`

---

## Pack 11: `decision-table-classification`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, deal, im  

**Description**: Single business-rule-task evaluating a named decision table;
output routes to classification-specific paths.  Representative for 2 explicit
paths plus default.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| classify-name | symbol | yes | Name for the classification task |
| route-gate-name | symbol | yes | Name for the routing gateway |
| decision | decision-ref | yes | Decision table reference |
| output-field | string | yes | Data location where the classification result is written |
| class-a-value | string | yes | Classification value that routes to path-a |
| path-a | node-ref | yes | Path for class-a result |
| default-path | node-ref | yes | Path for all other classifications |

**Example utterances**:
- "classify the investor type and route accordingly"
- "use the risk classification table to determine next steps"
- "apply the CBU category ruleset and branch on result"
- "run the eligibility decision table"
- "DMN classification -> routing"

**Structural signature**: `:gateway-kind exclusive :classification true :hit-policy dmn-compatible :outcomes variable`

---

## Pack 12: `manual-override-checkpoint`

**Version**: 1.0.0  
**Governance**: active  
**Domain scope**: cbu, kyc, compliance, governance  

**Description**: Automated decision presented to human for confirmation or
override; final routing on human decision.

**Parameters**:
| Name | Type | Required | Description |
|---|---|---|---|
| auto-eval-name | symbol | yes | Name for the automated evaluation task |
| review-task-name | symbol | yes | Name for the human review task |
| override-gate-name | symbol | yes | Name for the override routing gateway |
| auto-decision | decision-ref | yes | Decision table powering the automated evaluation |
| reviewer-role | string | yes | Role authorised to review and override |
| auto-result-field | string | yes | Data location where the auto-decision result is written |
| confirmed-path | node-ref | yes | Path when human confirms the auto-decision |
| override-path | node-ref | yes | Path when human overrides the auto-decision |

**Example utterances**:
- "automatically assess risk but allow a compliance officer to override"
- "system recommendation with human approval checkpoint"
- "automated decision with manual override capability"
- "present the auto-assessment to the reviewer for sign-off or correction"
- "4-eyes check: algorithm recommends, human confirms"

**Structural signature**: `:automation-level hybrid :human-in-loop true :gateway-kind exclusive :outcomes 2`

---

## Summary Table

| # | Pack name | Domain scope | Gateway kind | Outcomes | Human-in-loop |
|---|---|---|---|---|---|
| 1 | conjunctive-gate | cbu, kyc, onboarding, screening | exclusive | 2 | no |
| 2 | disjunctive-gate | cbu, kyc, screening, onboarding | exclusive | 2 | no |
| 3 | sanction-hit-escalation | cbu, kyc, screening, compliance | exclusive | 2 | no |
| 4 | parallel-evaluation-with-veto | cbu, kyc, screening | parallel + exclusive | 2 | no |
| 5 | periodic-refresh-trigger | cbu, kyc, periodic-review | exclusive | 2 | no |
| 6 | required-evidence-checklist | cbu, kyc, onboarding | exclusive | 2 | no |
| 7 | threshold-band-routing | cbu, kyc, ubo | exclusive | 3 | no |
| 8 | multi-jurisdiction-overlay | cbu, kyc, deal, compliance | exclusive | variable | no |
| 9 | linked-switch-chain | cbu, kyc, onboarding | exclusive | early-exit | no |
| 10 | cascading-decision | cbu, kyc, deal | exclusive | 2 | no |
| 11 | decision-table-classification | cbu, kyc, deal, im | exclusive | variable | no |
| 12 | manual-override-checkpoint | cbu, kyc, compliance, governance | exclusive | 2 | yes |
