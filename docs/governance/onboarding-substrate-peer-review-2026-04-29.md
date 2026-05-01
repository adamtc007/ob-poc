# SemOS Onboarding Substrate — DAG / DSL / Tollgate Peer-Review Snapshot

> **Date:** 2026-04-29 (post-refactor session).
> **Audience:** Adam (peer review), with the post-refactor model in mind.
> **Purpose:** consolidated read of the onboarding-side DAG taxonomies, their slot state machines, the DSL verbs that operate on each slot, and the green-switch tollgate predicates that gate progression. Use as a thinking aid; not a spec.
> **Authoritative spec:** `docs/todo/catalogue-platform-refinement-v1_4.md` (v1.4 DRAFT, this session). Earlier versions in archive.
> **Decision log:** `docs/governance/decision-log-2026-04-refactor.md` (D-001..D-017).
> **Snapshot scope:** the 8 DAGs in the onboarding/CBU/Deal/Product-Service/Resource thread.

---

## 1. The architectural model in one page

Every entity in SemOS has a DAG that maps its **stable states**.

A composite entity (CBU, Deal, etc.) has multiple paths that must complete; the DAG IS the resource-dependency taxonomy expressing those paths.

Each state node has a **switch (green / red)**:

- **green** = the entity satisfies that state's entry/transit criteria (its `green_when` predicate is `true`).
- **red** = it doesn't.

A **DSL verb** at a node:

1. Runs while the *source* state is green.
2. Obtains data (proofs, updates, evidence — whatever the verb's side effect is).
3. Tests whether the *destination* state's `green_when` now holds.
4. Either flips the destination green and the transition fires, OR stays at the source state with a diagnosable reason.

The destination's `green_when` IS the postcondition. There's no separate `postcondition:` field — we just look at where we're trying to go.

### The `green_when` predicate language (free-text, binary, recursive)

Every fact in `green_when` is binary: **obtained / not obtained**. Each `obtained` decomposes into:

- **exists**: the entity / row / FK is present.
- **valid**: the entity is in a state from a valid set (which itself can be the entity's own DAG green_when).

So `obtained = exists AND valid`. The recursion bottoms out at primitive facts (a passport row exists; its OCR-quality flag is set; a state column is in a known set).

Two diagnosable failure modes:

- **Missing**: required entity not present → "you don't have a passport for this UBO yet".
- **Invalid**: present but in a wrong state → "you have a passport but it failed quality check; attach a new one".

### Convention used in green_when

```
every <required_entity> exists
AND every <required_entity>.state = <required_state>
AND no <forbidden_entity> exists with state = <forbidden_state>
AND <numeric_attribute>.value >= <threshold>
```

Each line is a single binary assertion, AND'd together.

### What stays as user verbs (NOT tollgate-driven)

- **Reject** — explicit refusal; no predicate makes "rejected" true.
- **Waive** — discretionary override of a missing requirement.
- **Escalate** — explicit choice to refer up.
- **Override / force** — explicit bypass with audit trail.

These verbs may carry `precondition:` for guards (e.g., role checks) but don't have a `green_when` to gate them — they're discretionary judgments.

---

## 2. Cross-workspace gate map

The high-leverage tollgate composition for the onboarding flow:

```
Book.ready_for_deal     ←   green_when: deal exists for this client_group AND deal.state ∈ {IN_CLEARANCE, CONTRACTED}
                              │
Deal.IN_CLEARANCE       ←   entry on submit-for-bac
Deal.CONTRACTED         ←   green_when (compound triad):
                              ├ deal.bac_status = approved
                              ├ deal.kyc_clearance_status = approved
                              ├ parent kyc_case.state = APPROVED  (← evidence + UBO + red-flag + screening)
                              ├ every booking_principal_clearance.state ∈ {APPROVED, ACTIVE}  (← screening clean + reg-licence current)
                              ├ deal_contract exists
                              └ at least one deal_rate_card.state = AGREED

CBU.trade_permissioned  ←   (operational lifecycle; gated downstream of Deal.CONTRACTED)
CBU.actively_trading    ←   (gated on instrument_matrix scaffolding + BPs + custody chains)

Service.active          ←   (product/service taxonomy governed by changeset.approved tollgate)
ServiceResource         ←   provisioned per CBU; consumed by trade_gateway / settlement_chain / etc.
```

Each gate is independent; all must be green for the parent to flip.

---

## 3. DAG-by-DAG snapshot

## Book Setup

**File:** `rust/config/sem_os_seeds/dag_taxonomies/book_setup_dag.yaml` · **Workspace:** `book_setup`

Pre-onboarding scaffolding: client agreement → mandates defined → ready for deal.

### Slots (10)

#### `book` · book_lifecycle

_owner: onboarding_ops · scope: per_book · lifetime: long_lived · entity: "ob-poc".client_books · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `proposed` | ✓ | Book identified; client_group set. | — |
| `structure_chosen` |  | Jurisdiction + structure template selected. | — |
| `entities_provisioned` |  | Legal entities created for the book structure. | — |
| `cbus_scaffolded` |  | CBUs created per fund vehicle; categories set. | — |
| `parties_assigned` |  | Structural roles assigned (manco, depositary, etc.). | — |
| `mandates_defined` |  | Trading mandates authored and linked to CBUs. | — |
| `ready_for_deal` |  | Book structure complete; ready for deal.CONTRACTED handoff. | — |
| `abandoned` |  | Book-setup abandoned pre-handoff. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| proposed | **structure_chosen** | `book.select-structure` |  |
| structure_chosen | **entities_provisioned** | _(macro: structure.* invokes entity.create series)_ |  |
| entities_provisioned | **cbus_scaffolded** | `cbu.create` |  |
| cbus_scaffolded | **parties_assigned** | `cbu.assign-role` |  |
| parties_assigned | **mandates_defined** | `mandate.create` |  |
| mandates_defined | **ready_for_deal** | `book.mark-ready` |  |
| proposed, structure_chosen, entities_provisioned, cbus_scaffolded, parties_assigned, mandates_defined | **abandoned** | `book.abandon` |  |

**Terminal states:** ready_for_deal, abandoned

**DSL verbs targeting this slot (4):**

| Verb FQN | state_effect | description |
|---|---|---|
| `book.abandon` | transition | Abandon book-setup journey (pre-contract) |
| `book.create` | transition | Create a new client book (scope: client_group_id). Starts book-setup journey. |
| `book.mark-ready` | transition | Mark book as ready-for-deal (terminal-positive in book-setup journey) |
| `book.select-structure` | transition | Select jurisdiction-specific structure template for the book (e.g. struct.lux.ucits.sicav) |


#### `cbu` · (stateless)


#### `entity` · (stateless)


#### `trading_profile` · (stateless)


#### `kyc_case` · (stateless)


#### `client_group` · (stateless)


#### `workspace_root` · (stateless)


#### `structure_template` · (stateless)


#### `book_participant` · (stateless)


#### `mandate_outline` · (stateless)


### Cross-workspace constraints (2 — Mode A blocking)

- **book_cbus_scaffolded_requires_kyc_case_in_progress** (error): book → cbus_scaffolded requires the client-group KYC case to be
at least in DISCOVERY (can't create CBUs without KYC under way).
  - source: `kyc.kyc_case` state ∈ DISCOVERY, ASSESSMENT, REVIEW, APPROVED
  - target: `book_setup.book` transition `entities_provisioned -> cbus_scaffolded`
- **book_ready_requires_deal_contracted_gate** (warning): book → ready_for_deal is advisory — the Deal workspace is the
gate for commercial progression. Informational constraint flags
when book is ready_for_deal but no Deal exists / Deal not yet
at IN_CLEARANCE (Q4/Q21 (b), 2026-04-29: KYC_CLEARANCE collapsed
into IN_CLEARANCE compound state).
  - source: `deal.deal` state ∈ IN_CLEARANCE, CONTRACTED
  - target: `book_setup.book` transition `mandates_defined -> ready_for_deal`

### Cross-slot constraints (5 — intra-DAG)

- **book_entities_provisioned_requires_structure_chosen** (error): book → entities_provisioned requires structure_template selected.
  - rule: `book.status = 'entities_provisioned'` ...
- **book_cbus_scaffolded_requires_entities** (error): book → cbus_scaffolded requires at least one entity created for this book.
  - rule: `book.status = 'cbus_scaffolded'` ...
- **book_parties_assigned_requires_all_cbus_have_required_roles** (error): book → parties_assigned requires every CBU in the book to have all required roles per its structure template.
  - rule: `book.status = 'parties_assigned'` ...
- **book_mandates_defined_requires_trading_profiles** (error): book → mandates_defined requires each CBU to have at least one trading_profile in DRAFT or beyond.
  - rule: `book.status = 'mandates_defined'` ...
- **book_ready_requires_all_cbus_validated** (error): book → ready_for_deal requires every CBU to have cbus.status = VALIDATED.
  - rule: `book.status = 'ready_for_deal'` ...


## Onboarding Request

**File:** `rust/config/sem_os_seeds/dag_taxonomies/onboarding_request_dag.yaml` · **Workspace:** `onboarding_request`

Deal → Ops handoff carrier (request rows that drive provisioning).

### Slots (5)

#### `workspace_root` · (stateless)


#### `onboarding_request` · (stateless)


#### `deal` · (stateless)


#### `cbu` · (stateless)


#### `contract` · (stateless)


### Cross-workspace constraints (2 — Mode A blocking)

- **onboarding_request_requires_deal_contracted** (error): Cannot create deal_onboarding_request unless the source deal is
in CONTRACTED or later state (commercial agreement in place).
  - source: `deal.deal` state ∈ CONTRACTED, ONBOARDING, ACTIVE
  - target: `onboarding_request.onboarding_request` transition `validating -> submitted`
- **onboarding_request_requires_cbu_validated** (error): Target CBU must be VALIDATED before onboarding handoff.
  - source: `cbu.cbu` state ∈ VALIDATED
  - target: `onboarding_request.onboarding_request` transition `validating -> submitted`

### Cross-slot constraints (2 — intra-DAG)

- **onboarding_request_requires_validated_cbu** (error): Cannot submit onboarding request for a non-VALIDATED CBU.
  - rule: `onboarding_request transitions scoping → submitted` ...
- **onboarding_request_requires_contracted_deal** (error): Cannot submit onboarding request for a non-CONTRACTED deal.
  - rule: `onboarding_request transitions scoping → submitted` ...


## CBU

**File:** `rust/config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml` · **Workspace:** `cbu`

**The money-making apparatus.** Atomic operational unit; everything resolves to sets of CBUs.

### Slots (23)

#### `cbu` · cbu_discovery_lifecycle

_owner: compliance · scope: per_cbu · lifetime: long_lived · entity: "ob-poc".cbus · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DISCOVERED` | ✓ | CBU created; scope not fully determined. | — |
| `VALIDATION_PENDING` |  | Submitted for validation; evidence under review. | — |
| `VALIDATED` |  | Validation passed; hands off to operational lifecycle. | — |
| `UPDATE_PENDING_PROOF` |  | Previously VALIDATED; a change requires fresh evidence before
re-approval.
 | — |
| `VALIDATION_FAILED` |  | Validation rejected; terminal unless reopen-validation. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DISCOVERED | **VALIDATION_PENDING** | `cbu.submit-for-validation` |  |
| VALIDATION_PENDING | **VALIDATED** | `cbu.decide` |  |
| VALIDATION_PENDING | **VALIDATION_FAILED** | `cbu.decide` |  |
| VALIDATED | **UPDATE_PENDING_PROOF** | `cbu.request-proof-update` |  |
| UPDATE_PENDING_PROOF | **VALIDATION_PENDING** | `cbu.submit-for-validation` |  |
| VALIDATION_FAILED | **VALIDATION_PENDING** | `cbu.reopen-validation` |  |

**Terminal states:** VALIDATION_FAILED

**DSL verbs targeting this slot (10):**

| Verb FQN | state_effect | description |
|---|---|---|
| `cbu.begin-winding-down` | transition | Begin CBU winding-down — exit intent, no new activity, unwind positions |
| `cbu.complete-offboard` | transition | Complete CBU offboarding — terminal-positive transition to offboarded state |
| `cbu.decide` | transition | Record KYC/AML decision for CBU collective state (entities, UBOs, documents) |
| `cbu.reinstate` | transition | Reinstate suspended CBU back to operationally active |
| `cbu.reopen-validation` | transition | Reopen validation for a CBU that previously failed |
| `cbu.request-proof-update` | transition | Mark a validated CBU as requiring updated proof |
| `cbu.restrict` | transition | Apply partial trading restriction to CBU (specific markets / instruments pulled) |
| `cbu.submit-for-validation` | transition | Move a CBU into validation review |
| `cbu.suspend` | transition | Suspend CBU operational lifecycle (operational hold — regulatory, dispute, client distress) |
| `cbu.unrestrict` | transition | Lift trading restriction from CBU (restore full mandate scope) |


#### `entity_proper_person` · entity_proper_person_lifecycle

_scope: per_natural_person_entity · entity: "ob-poc".entity_proper_persons · col: person_state_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `GHOST` | ✓ | Name-only; no identifying attributes. | — |
| `IDENTIFIED` |  | Has DOB / nationality / ID numbers. | — |
| `VERIFIED` |  | Confirmed by official documents. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| GHOST | **IDENTIFIED** | `entity.identify` |  |
| IDENTIFIED | **VERIFIED** | `entity.verify` |  |

**Terminal states:** VERIFIED

**DSL verbs targeting this slot (2):**

| Verb FQN | state_effect | description |
|---|---|---|
| `entity.identify` | transition | Add identifying attributes to a ghost entity (transitions to IDENTIFIED state) |
| `entity.verify` | transition | Mark an identified entity as verified (transitions to VERIFIED state) |


#### `entity_limited_company_ubo` · entity_limited_company_ubo_lifecycle

_scope: per_limited_company_entity · entity: "ob-poc".entity_limited_companies · col: ubo_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ | UBO discovery not yet attempted. | — |
| `DISCOVERED` |  | Beneficial owners identified via research. | — |
| `PUBLIC_FLOAT` |  | Widely-held public company; no UBO lookup required. | — |
| `EXEMPT` |  | Regulator-exempt (state-owned, sovereign, etc.). | — |
| `MANUAL_REQUIRED` |  | Automated discovery failed; manual investigation needed. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **DISCOVERED** | _(backend: UBO discovery pipeline)_ |  |
| PENDING | **PUBLIC_FLOAT** | _(backend: listed-company check)_ |  |
| PENDING | **EXEMPT** | _(backend: exemption classifier)_ |  |
| PENDING | **MANUAL_REQUIRED** | _(backend: pipeline failure)_ |  |
| MANUAL_REQUIRED | **DISCOVERED** | `ubo-registry.promote-to-ubo` |  |

**Terminal states:** DISCOVERED, PUBLIC_FLOAT, EXEMPT


#### `cbu_evidence` · cbu_evidence_lifecycle

_scope: per_evidence_item · entity: "ob-poc".cbu_evidence · col: verification_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ | Evidence attached but not yet verified. | — |
| `VERIFIED` |  | Evidence reviewed and accepted. | — |
| `REJECTED` |  | Evidence rejected (insufficient, expired, fraudulent). | — |
| `EXPIRED` |  | Previously VERIFIED; time-decay invalidation. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **VERIFIED** | `cbu.verify-evidence` |  |
| PENDING | **REJECTED** | `cbu.verify-evidence` |  |
| VERIFIED | **EXPIRED** | _(backend: time-decay trigger based on evidence_type validity window)_ |  |
| EXPIRED | **PENDING** | `cbu.attach-evidence` |  |

**Terminal states:** REJECTED

**DSL verbs targeting this slot (1):**

| Verb FQN | state_effect | description |
|---|---|---|
| `cbu.verify-evidence` | transition | Mark evidence as verified or rejected |


#### `investor` · investor_lifecycle

_owner: ops · scope: per_investor · lifetime: long_lived · entity: "ob-poc".investors · col: (derived from investor lifecycle events + status)_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DRAFT` | ✓ | Investor record created; eligibility not confirmed. | — |
| `ELIGIBLE` |  | Eligibility confirmed (qualification test passed). | — |
| `ACTIVE` |  | Subscribed; holding units. | — |
| `SUSPENDED` |  | Temporary hold (KYC refresh, dispute, sanctions). | — |
| `OFFBOARDED` |  | Account closed; exit completed. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DRAFT | **ELIGIBLE** | `investor.mark-eligible` |  |
| ELIGIBLE | **ACTIVE** | `investor.activate` | investor_kyc.status = APPROVED |
| ACTIVE | **SUSPENDED** | `investor.suspend` |  |
| SUSPENDED | **ACTIVE** | `investor.reinstate` |  |
| (ACTIVE, SUSPENDED) | **OFFBOARDED** | `investor.offboard` |  |

**Terminal states:** OFFBOARDED

**DSL verbs targeting this slot (6):**

| Verb FQN | state_effect | description |
|---|---|---|
| `investor.activate` | transition | Transition from SUBSCRIBED to ACTIVE_HOLDER |
| `investor.complete-redemption` | transition | Complete redemption - return to ACTIVE_HOLDER or proceed to OFFBOARDED |
| `investor.mark-eligible` | transition | Transition from KYC_APPROVED to ELIGIBLE_TO_SUBSCRIBE |
| `investor.offboard` | transition | Transition to OFFBOARDED (terminal state) |
| `investor.reinstate` | transition | Reinstate suspended investor to previous state |
| `investor.suspend` | transition | Suspend investor (can be done from most states) |


#### `investor_kyc` · investor_kyc_lifecycle

_owner: compliance · scope: per_investor · lifetime: long_lived · entity: "ob-poc".investors · col: kyc_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `NOT_STARTED` | ✓ |  | — |
| `IN_PROGRESS` |  | KYC evidence being collected + verified. | — |
| `APPROVED` |  | KYC passed; investor can be ACTIVE. | — |
| `REJECTED` |  | KYC failed; investor cannot be ACTIVE. Terminal. | — |
| `EXPIRED` |  | Approval lapsed; refresh required before new activity. | — |
| `REFRESH_REQUIRED` |  | Material change triggered mandatory re-KYC. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| NOT_STARTED | **IN_PROGRESS** | `investor.start-kyc` |  |
| IN_PROGRESS | **APPROVED** | `investor.approve-kyc` |  |
| IN_PROGRESS | **REJECTED** | `investor.reject-kyc` |  |
| APPROVED | **EXPIRED** | _(backend: kyc_expires_at trigger)_ |  |
| APPROVED | **REFRESH_REQUIRED** | `investor.request-documents` |  |
| (EXPIRED, REFRESH_REQUIRED) | **IN_PROGRESS** | `investor.start-kyc` |  |

**Terminal states:** REJECTED

**DSL verbs targeting this slot (4):**

| Verb FQN | state_effect | description |
|---|---|---|
| `investor.approve-kyc` | transition | Transition from KYC_IN_PROGRESS to KYC_APPROVED |
| `investor.reject-kyc` | transition | Transition from KYC_IN_PROGRESS to KYC_REJECTED |
| `investor.request-documents` | transition | Transition from ENQUIRY to PENDING_DOCUMENTS |
| `investor.start-kyc` | transition | Transition from PENDING_DOCUMENTS to KYC_IN_PROGRESS |


#### `holding` · holding_lifecycle

_owner: ops · scope: per_holding · lifetime: long_lived · entity: "ob-poc".holdings · col: holding_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ | Subscription recorded; awaiting settlement. | — |
| `ACTIVE` |  | Settled; units held; dividends accrue. | — |
| `SUSPENDED` |  | Temporary hold (dispute, KYC suspend on investor). | — |
| `RESTRICTED` |  | Legal lock — court order, dispute freeze, specific restriction. | — |
| `PLEDGED` |  | Pledged as collateral against a loan/derivative; cannot redeem freely. | — |
| `FROZEN` |  | Sanctions exposure on investor; regulatory freeze. | — |
| `CLOSED` |  | Redemption complete; zero units. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **ACTIVE** | `holding.update-status` |  |
| ACTIVE | **SUSPENDED** | `holding.update-status` |  |
| SUSPENDED | **ACTIVE** | `holding.update-status` |  |
| ACTIVE | **RESTRICTED** | `holding.restrict` |  |
| RESTRICTED | **ACTIVE** | `holding.lift-restriction` |  |
| ACTIVE | **PLEDGED** | `holding.pledge` |  |
| PLEDGED | **ACTIVE** | `holding.release-pledge` |  |
| ACTIVE, SUSPENDED, RESTRICTED, PLEDGED | **FROZEN** | _(backend: sanctions hit)_ |  |
| FROZEN | **ACTIVE** | _(backend: sanctions cleared)_ |  |
| ACTIVE, SUSPENDED, PENDING, RESTRICTED | **CLOSED** | `holding.close` |  |

**Terminal states:** CLOSED

**DSL verbs targeting this slot (6):**

| Verb FQN | state_effect | description |
|---|---|---|
| `holding.close` | transition | Close a holding (mark as inactive) |
| `holding.lift-restriction` | transition | Lift legal restriction from holding |
| `holding.pledge` | transition | Pledge holding as collateral against a loan / derivative exposure |
| `holding.release-pledge` | transition | Release pledge on holding (collateral no longer required) |
| `holding.restrict` | transition | Apply legal restriction to holding (court order / dispute freeze) |
| `holding.update-status` | transition | Update holding status |


#### `service_consumption` · service_consumption_lifecycle

_owner: ops · scope: per_cbu_service_kind · lifetime: long_lived · entity: "ob-poc".cbu_service_consumption · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `proposed` | ✓ | Service identified as in scope; not yet provisioned. | — |
| `provisioned` |  | Accounts opened, routings configured, but not yet live. | — |
| `active` |  | Service live; CBU consuming it operationally. | — |
| `suspended` |  | Service paused (dispute, migration, regulatory hold). | — |
| `winding_down` |  | Service being retired; no new activity; closeout in progress. | — |
| `retired` |  | Service fully retired for this CBU. Terminal. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| proposed | **provisioned** | `service-consumption.provision` |  |
| provisioned | **active** | `service-consumption.activate` |  |
| active | **suspended** | `service-consumption.suspend` |  |
| suspended | **active** | `service-consumption.reinstate` |  |
| active, suspended | **winding_down** | `service-consumption.begin-winddown` |  |
| winding_down | **retired** | `service-consumption.retire` |  |

**Terminal states:** retired

**DSL verbs targeting this slot (6):**

| Verb FQN | state_effect | description |
|---|---|---|
| `service-consumption.activate` | transition | Activate service for operational use (provisioned → active) |
| `service-consumption.begin-winddown` | transition | Begin service wind-down for CBU (exit intent) |
| `service-consumption.provision` | transition | Provision a service for a CBU (proposed → provisioned; accounts opened, routings set) |
| `service-consumption.reinstate` | transition | Reinstate suspended service back to active |
| `service-consumption.retire` | transition | Retire service consumption (terminal-positive after winddown) |
| `service-consumption.suspend` | transition | Suspend service for CBU (service pause; restorable) |


#### `cbu_corporate_action` · cbu_corporate_action_lifecycle

_owner: compliance · scope: per_cbu_ca_event · lifetime: long_lived · entity: "ob-poc".cbu_corporate_action_events · col: ca_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `proposed` | ✓ | CA event proposed; pending internal + external review. | — |
| `under_review` |  | Board / legal / compliance review in progress. | — |
| `approved` |  | Approved for execution on effective_date. | board_review exists<br>AND board_review.state = COMPLETE<br>AND legal_review exists<br>AND legal_review.state = COMPLETE<br>AND compliance_review exists<br>AND compliance_review.state = COMPLETE<br>AND no objection exists with state = OPEN<br>AND every mandatory_disclosure exists<br>AND every mandatory_disclosure.state = LODGED |
| `effective` |  | Effective_date reached; CA is now live. Transitional. | — |
| `implemented` |  | Post-effective implementation complete. Terminal-positive. | — |
| `rejected` |  | CA rejected at review. Terminal-negative. | — |
| `withdrawn` |  | Proposer withdrew before decision. Terminal-negative. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| proposed | **under_review** | `cbu-ca.submit-for-review` |  |
| under_review | **approved** | `cbu-ca.approve` |  |
| under_review | **rejected** | `cbu-ca.reject` |  |
| proposed, under_review | **withdrawn** | `cbu-ca.withdraw` |  |
| approved | **effective** | _(backend: effective_date reached)_ |  |
| effective | **implemented** | `cbu-ca.mark-implemented` |  |

**Terminal states:** implemented, rejected, withdrawn

**DSL verbs targeting this slot (5):**

| Verb FQN | state_effect | description |
|---|---|---|
| `cbu-ca.approve` | transition | Approve CBU-level CA event (board / legal / compliance sign-off) |
| `cbu-ca.mark-implemented` | transition | Mark CBU-level CA event as implemented (post-effective; terminal-positive) |
| `cbu-ca.reject` | transition | Reject CBU-level CA event (terminal-negative) |
| `cbu-ca.submit-for-review` | transition | Submit CBU-level CA event for board / legal / compliance review |
| `cbu-ca.withdraw` | transition | Withdraw CBU-level CA event before decision (proposer retracts) |


#### `cbu_disposition` · cbu_disposition_lifecycle

_owner: compliance · scope: per_cbu · lifetime: long_lived · entity: "ob-poc".cbus · col: disposition_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `active` | ✓ | Normal disposition. No administrative flag. | — |
| `under_remediation` |  | Post-breach cleanup / enhanced monitoring. Specific evidence
refresh required. Board-level attestation may be required
depending on breach s | — |
| `soft_deleted` |  | Marked deleted but row retained (cbus.deleted_at IS NOT
NULL). Restorable via cbu.restore. Used for duplicates,
test data, records created i | — |
| `hard_deleted` |  | Row physically removed. Terminal. Requires regulatory
retention window to have elapsed.
 | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| active | **under_remediation** | `cbu.flag-for-remediation` |  |
| under_remediation | **active** | `cbu.clear-remediation` |  |
| active | **soft_deleted** | `cbu.soft-delete` |  |
| soft_deleted | **active** | `cbu.restore` |  |
| soft_deleted | **hard_deleted** | `cbu.hard-delete` |  |

**Terminal states:** hard_deleted

**DSL verbs targeting this slot (5):**

| Verb FQN | state_effect | description |
|---|---|---|
| `cbu.clear-remediation` | transition | Clear remediation flag from CBU (remediation conditions satisfied) |
| `cbu.flag-for-remediation` | transition | Flag CBU for post-breach remediation / enhanced monitoring |
| `cbu.hard-delete` | transition | Permanently remove CBU row (terminal; regulatory retention window required) |
| `cbu.restore` | transition | Restore soft-deleted CBU back to active disposition |
| `cbu.soft-delete` | transition | Soft-delete CBU (disposition: sets deleted_at; restorable) |


#### `client_group_entity_review` · client_group_entity_review_lifecycle

_scope: per_client_group_entity_link · entity: "ob-poc".client_group_entity · col: review_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `pending` | ✓ | Entity link proposed by research; awaiting review. | — |
| `confirmed` |  | Entity confirmed as part of the client group. | — |
| `rejected` |  | Entity rejected from the client group. | — |
| `needs_update` |  | Source data changed; review needs redo. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| pending | **confirmed** | _(backend: client-group researcher review — cross-workspace verb from onboarding)_ |  |
| pending | **rejected** | _(backend: client-group researcher review)_ |  |
| (confirmed, rejected) | **needs_update** | _(backend: source data change trigger)_ |  |
| needs_update | **pending** | _(backend: re-review required)_ |  |


#### `client_group` · (stateless)


#### `trading_profile` · (stateless)


#### `kyc_case` · (stateless)


#### `workspace_root` · (stateless)


#### `cbu_entity_role` · (stateless)


#### `cbu_entity_relationship` · (stateless)


#### `share_class` · share_class_lifecycle

_owner: ops+fund-admin · scope: per_share_class · lifetime: long_lived · entity: "ob-poc".share_classes · col: lifecycle_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DRAFT` | ✓ | Class authored but not yet launched. No supply. | — |
| `OPEN` |  | Accepting new subscriptions; existing holders transact freely. | — |
| `SOFT_CLOSED` |  | No new subscribers; existing holders still transact. | — |
| `HARD_CLOSED` |  | No subs AND no redemptions (regulatory hold or wind-down). | — |
| `WINDING_DOWN` |  | Redemptions being processed; no new activity. Pre-liquidation. | — |
| `LIQUIDATED` |  | Terminal. All holders redeemed; class retired. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DRAFT | **OPEN** | `share-class.launch` |  |
| OPEN | **SOFT_CLOSED** | `share-class.soft-close` |  |
| SOFT_CLOSED | **OPEN** | `share-class.reopen` |  |
| OPEN, SOFT_CLOSED | **HARD_CLOSED** | `share-class.hard-close` |  |
| HARD_CLOSED | **SOFT_CLOSED** | `share-class.lift-hard-close` |  |
| OPEN, SOFT_CLOSED, HARD_CLOSED | **WINDING_DOWN** | `share-class.begin-winddown` |  |
| WINDING_DOWN | **LIQUIDATED** | `share-class.close` |  |

**Terminal states:** LIQUIDATED

**DSL verbs targeting this slot (7):**

| Verb FQN | state_effect | description |
|---|---|---|
| `share-class.begin-winddown` | transition | Begin share-class wind-down (pre-liquidation) |
| `share-class.close` | transition | Close a share class to new subscriptions |
| `share-class.hard-close` | transition | Hard-close share class (no subscriptions AND no redemptions — regulatory / wind-down) |
| `share-class.launch` | transition | Launch share class from DRAFT to OPEN (accepting subscriptions) |
| `share-class.lift-hard-close` | transition | Lift hard-close on share class back to soft-closed |
| `share-class.reopen` | transition | Reopen soft-closed share class back to accepting subscriptions |
| `share-class.soft-close` | transition | Soft-close share class (no new subscriptions; existing holders transact) |


#### `manco` · manco_lifecycle

_owner: compliance · scope: per_manco · lifetime: long_lived · entity: "ob-poc".manco_regulatory_status · col: regulatory_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `UNDER_REVIEW` | ✓ | New manco being onboarded; not yet approved to manage mandates. | — |
| `APPROVED` |  | Manco cleared to manage new mandates. | every regulatory_licence exists<br>AND every regulatory_licence.state = CURRENT<br>AND no investigation exists with state = OPEN<br>AND periodic_review exists<br>AND periodic_review.state = COMPLETE<br>AND credit_risk_assessment exists<br>AND credit_risk_assessment.state in {WITHIN_LIMITS, LOW}<br>AND operational_risk_assessment exists<br>AND operational_risk_assessment.state in {WITHIN_LIMITS, LOW} |
| `UNDER_INVESTIGATION` |  | Regulatory action or internal investigation; new mandates
blocked; existing mandates under heightened scrutiny.
Propagates SUSPENDED to all  | — |
| `SUSPENDED` |  | Full operational hold — managed CBUs all SUSPENDED. | — |
| `SUNSET` |  | No new mandates; existing ones run to exit. Wind-down in progress. | — |
| `TERMINATED` |  | Terminal. All mandates transferred or wound down; manco retired. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| UNDER_REVIEW | **APPROVED** | `manco.approve` |  |
| UNDER_REVIEW | **TERMINATED** | `manco.reject` |  |
| APPROVED | **UNDER_INVESTIGATION** | `manco.flag-regulatory` |  |
| UNDER_INVESTIGATION | **APPROVED** | `manco.clear-regulatory` |  |
| UNDER_INVESTIGATION | **SUSPENDED** | `manco.suspend` |  |
| SUSPENDED | **UNDER_INVESTIGATION** | `manco.partial-reinstate` |  |
| APPROVED, UNDER_INVESTIGATION, SUSPENDED | **SUNSET** | `manco.begin-sunset` |  |
| SUNSET | **TERMINATED** | `manco.terminate` |  |

**Terminal states:** TERMINATED

**DSL verbs targeting this slot (8):**

| Verb FQN | state_effect | description |
|---|---|---|
| `manco.approve` | transition | Approve manco for managing mandates (new-manco onboarding) |
| `manco.begin-sunset` | transition | Begin manco sunset (no new mandates; existing run to exit) |
| `manco.clear-regulatory` | transition | Clear manco from regulatory investigation (back to APPROVED) |
| `manco.flag-regulatory` | transition | Flag manco for regulatory investigation (transitions APPROVED → UNDER_INVESTIGATION) |
| `manco.partial-reinstate` | transition | Partially reinstate suspended manco (returns to UNDER_INVESTIGATION) |
| `manco.reject` | transition | Reject manco onboarding (terminal-negative) |
| `manco.suspend` | transition | Fully suspend manco — cascades SUSPENDED to all managed CBUs |
| `manco.terminate` | transition | Terminate manco (all mandates transferred or wound down) |


#### `investor_role` · (stateless)


#### `legal_entity` · (stateless)


#### `edge` · (stateless)


#### `temporal` · (stateless)


### Cross-workspace constraints (4 — Mode A blocking)

- **cbu_validated_requires_kyc_case_approved** (error): CBU cannot transition DISCOVERY.VALIDATION_PENDING → VALIDATED
unless the sponsoring KYC case is APPROVED.
  - source: `kyc.kyc_case` state ∈ APPROVED
  - target: `cbu.cbu` transition `VALIDATION_PENDING -> VALIDATED`
- **service_consumption_requires_active_service** (error): cbu.service_consumption cannot transition proposed → provisioned
unless the referenced product_maintenance.service has reached
lifecycle_status = active in the catalogue.
  - source: `product_maintenance.service` state ∈ active
  - target: `cbu.service_consumption` transition `proposed -> provisioned`
- **service_consumption_requires_deal_contracted** (error): cbu.service_consumption cannot transition proposed → provisioned
unless a Deal exists in CONTRACTED/ONBOARDING/ACTIVE for the same
client_group as this CBU.
  - source: `deal.deal` state ∈ CONTRACTED, ONBOARDING, ACTIVE
  - target: `cbu.service_consumption` transition `proposed -> provisioned`
- **service_consumption_active_requires_live_binding** (error): cbu.service_consumption cannot transition provisioned → active unless
the capability_binding for the consumed (cbu, service) pair is LIVE
and the underlying application_instance is in ACTIVE state. Without
this gate a CBU could be marked actively-consuming a service that has
no live BNY application instance to fulfil it.
  - source: `lifecycle_resources.capability_binding` state ∈ LIVE
  - target: `cbu.service_consumption` transition `provisioned -> active`

### Cross-slot constraints (9 — intra-DAG)

- **cbu_validated_requires_evidence_set_verified** (error): CBU → VALIDATED requires ALL required cbu_evidence rows at
status = VERIFIED. Required set depends on kyc_scope_template.
  - rule: `cbus.status = 'VALIDATED'` ...
- **cbu_validated_requires_commercial_client_entity** (error): CBU → VALIDATED requires commercial_client_entity_id set (parent legal entity identified).
  - rule: `cbus.status = 'VALIDATED'` ...
- **cbu_validated_requires_uboes_in_terminal_state** (error): CBU → VALIDATED requires all limited-company entities in the
ownership chain have ubo_status in terminal (DISCOVERED, PUBLIC_FLOAT, EXEMPT).
  - rule: `cbus.status = 'VALIDATED'` ...
- **investor_active_requires_kyc_approved** (error): investor.status = ACTIVE requires investor_kyc.status = APPROVED.
  - rule: `investors.status = 'ACTIVE'` ...
- **holding_active_requires_investor_active** (error): holding.status = ACTIVE requires parent investor.status = ACTIVE.
  - rule: `holdings.holding_status = 'ACTIVE'` ...
- **holding_suspended_cascades_from_investor** (informational): If investor.status = SUSPENDED, all related holdings must also be
SUSPENDED (informational; auto-cascaded).
  - rule: `investors.status = 'SUSPENDED'` ...
- **entity_verified_required_for_ubo_role** (error): Entity assigned to a UBO-bearing role (ownership / control)
must be VERIFIED (for natural persons) or UBO-terminal (for
limited companies).
  - rule: `cbu_entity_roles WHERE role IN ('ownership', 'control')` ...
- **cbu_update_pending_proof_blocks_new_subscriptions** (error): A CBU in UPDATE_PENDING_PROOF cannot accept new investor
subscriptions (can continue servicing existing investors).
  - rule: `cbus.status = 'UPDATE_PENDING_PROOF'` ...
- **investor_offboarded_requires_all_holdings_closed** (error): investor.status = OFFBOARDED requires all holdings CLOSED.
  - rule: `investors.status = 'OFFBOARDED'` ...

### Derived cross-workspace state (1 — Mode B aggregation)

- **cbu_operationally_active**: Canonical CBU tollgate — "is this CBU permitted to transact with
BNY?" Aggregates readiness across all contributing workspaces.
The CBU is the money-making apparatus; this aggregate is the
gate on its


## Deal

**File:** `rust/config/sem_os_seeds/dag_taxonomies/deal_dag.yaml` · **Workspace:** `deal`

Commercial agreement to service the CBU. Two-axis lifecycle: commercial (sales→IN_CLEARANCE→CONTRACTED) + operational (ONBOARDING→ACTIVE→OFFBOARDED).

### Slots (22)

#### `deal` · deal_commercial_lifecycle

_owner: sales+BAC · scope: per_deal · lifetime: long_lived · entity: "ob-poc".deals · col: deal_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PROSPECT` | ✓ | Initial opportunity created. | — |
| `QUALIFYING` |  | Participants added; product shortlist forming. | — |
| `NEGOTIATING` |  | Active rate-card negotiation chain. | — |
| `IN_CLEARANCE` |  | Compound clearance phase. Two parallel substate columns:
  deals.bac_status ∈ (pending, in_review, approved, rejected)
  deals.kyc_clearance | — |
| `CONTRACTED` |  | Commercial lifecycle terminal-positive. Junction to the
operational dual_lifecycle chain (ONBOARDING → ACTIVE →
SUSPENDED / WINDING_DOWN → O | deal.bac_status = approved<br>AND deal.kyc_clearance_status = approved<br>AND parent kyc_case exists<br>AND parent kyc_case.state = APPROVED<br>AND every booking_principal_clearance exists<br>AND every booking_principal_clearance.state in {APPROVED, ACTIVE}<br>AND deal_contract exists<br>AND deal_contract.state in {DRAFT, EXECUTED}<br>AND every deal_rate_card exists<br>AND at least one deal_rate_card.state = AGREED |
| `LOST` |  | Terminal-negative: competitor won the deal. | — |
| `REJECTED` |  | Terminal-negative: internal BAC or KYC said no. | — |
| `WITHDRAWN` |  | Terminal-negative: client walked away. | — |
| `CANCELLED` |  | Terminal-negative: we walked away. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PROSPECT | **QUALIFYING** | deal.create / deal.update-status |  |
| QUALIFYING | **NEGOTIATING** | deal.update-status / deal.create-rate-card |  |
| NEGOTIATING | **IN_CLEARANCE** | `deal.submit-for-bac` | at least one deal_rate_card in AGREED |
| IN_CLEARANCE | **IN_CLEARANCE** | deal.bac-approve / deal.bac-reject / deal.update-kyc-clearance |  |
| IN_CLEARANCE | **CONTRACTED** | deal.update-status / deal.add-contract | deals.bac_status = 'approved' AND deals.kyc_clearance_status = 'approved' AND group KYC case status = APPROVED (V1.3-1 cross-workspace) |
| IN_CLEARANCE | **REJECTED** | deal.bac-reject / deal.reject | deals.bac_status = 'rejected' OR deals.kyc_clearance_status = 'rejected' OR KYC failed (case.status = REJECTED or DO_NOT_ONBOARD) |
| PROSPECT, QUALIFYING, NEGOTIATING, IN_CLEARANCE | **LOST** | `deal.mark-lost` |  |
| PROSPECT, QUALIFYING, NEGOTIATING, IN_CLEARANCE | **WITHDRAWN** | `deal.mark-withdrawn` |  |
| PROSPECT, QUALIFYING, NEGOTIATING, IN_CLEARANCE | **CANCELLED** | `deal.cancel` |  |

**Terminal states:** CONTRACTED, LOST, REJECTED, WITHDRAWN, CANCELLED

**DSL verbs targeting this slot (11):**

| Verb FQN | state_effect | description |
|---|---|---|
| `deal.bac-approve` | transition | BAC approval decision — sets deals.bac_status='approved' (parallel substate of IN_CLEARANCE; CONTRACTED gated on bac_sta |
| `deal.bac-reject` | transition | BAC rejection — sets deals.bac_status='rejected'; if KYC has not also rejected, deal_status remains IN_CLEARANCE pending |
| `deal.begin-winding-down` | transition | Begin deal winding down — exit intent from active servicing |
| `deal.cancel` | transition | Cancel a deal (soft delete - sets status to CANCELLED) |
| `deal.mark-lost` | transition | Mark deal as lost (competitor won) |
| `deal.mark-withdrawn` | transition | Mark deal as withdrawn (client walked away) |
| `deal.reinstate` | transition | Reinstate suspended deal back to active |
| `deal.reject` | transition | Reject deal (terminal-negative; internal decision) |
| `deal.submit-for-bac` | transition | Submit deal to Business Acceptance Committee — writes deal_status='IN_CLEARANCE' and bac_status='in_review' (Q21 (b) sub |
| `deal.suspend` | transition | Suspend active deal (operational hold; restorable) |
| `deal.update-status` | transition | Transition deal status with lifecycle validation, including pre-contract KYC clearance |


#### `deal_product` · deal_product_lifecycle

_scope: per_deal_product · entity: "ob-poc".deal_products · col: product_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PROPOSED` | ✓ |  | — |
| `NEGOTIATING` |  |  | — |
| `AGREED` |  |  | — |
| `DECLINED` |  | Terminal-negative; client declined the product. | — |
| `REMOVED` |  | Terminal; product pulled from the deal scope. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PROPOSED | **NEGOTIATING** | `deal.update-product-status` |  |
| NEGOTIATING | **AGREED** | deal.update-product-status / deal.agree-rate-card |  |
| (PROPOSED, NEGOTIATING) | **DECLINED** | `deal.update-product-status` |  |
| (PROPOSED, NEGOTIATING, AGREED) | **REMOVED** | `deal.remove-product` |  |

**Terminal states:** AGREED, DECLINED, REMOVED

**DSL verbs targeting this slot (1):**

| Verb FQN | state_effect | description |
|---|---|---|
| `deal.remove-product` | transition | Remove a product from the deal scope (sets status to REMOVED) |


#### `deal_rate_card` · deal_rate_card_lifecycle

_owner: sales+pricing-committee · scope: per_rate_card · entity: "ob-poc".deal_rate_cards · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DRAFT` | ✓ | Authoring lines; not yet shared. | — |
| `PENDING_INTERNAL_APPROVAL` |  | Submitted to pricing committee for internal approval (R-5
G-2). Required when discount > threshold / bespoke model /
new jurisdiction.
 | — |
| `APPROVED_INTERNALLY` |  | Pricing committee approved; ready to propose to counterparty. | — |
| `PROPOSED` |  | Sent to counterparty; awaiting response. | — |
| `COUNTER_PROPOSED` |  | Counterparty proposed edits; ball in our court. | — |
| `AGREED` |  | Both sides agreed. At most one AGREED per (deal, contract, product). | — |
| `SUPERSEDED` |  | Superseded by newer AGREED. Terminal; history. | — |
| `CANCELLED` |  | Withdrawn before agreement. Terminal. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DRAFT | **PENDING_INTERNAL_APPROVAL** | `deal.submit-for-pricing-approval` | discount > threshold OR bespoke_model OR new_jurisdiction |
| PENDING_INTERNAL_APPROVAL | **APPROVED_INTERNALLY** | `deal.pricing-approve` |  |
| PENDING_INTERNAL_APPROVAL | **DRAFT** | `deal.pricing-reject` |  |
| APPROVED_INTERNALLY | **PROPOSED** | `deal.propose-rate-card` |  |
| DRAFT | **PROPOSED** | `deal.propose-rate-card` | discount within threshold AND standard_model |
| PROPOSED | **COUNTER_PROPOSED** | `deal.counter-rate-card` |  |
| COUNTER_PROPOSED | **PROPOSED** | `deal.propose-rate-card` |  |
| PROPOSED, COUNTER_PROPOSED | **AGREED** | `deal.agree-rate-card` |  |
| AGREED | **SUPERSEDED** | _(backend: new AGREED for same (deal, contract, product))_ |  |
| DRAFT, PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY, PROPOSED, COUNTER_PROPOSED | **CANCELLED** | _(implicit: rate card removed or deal cancelled)_ |  |

**Terminal states:** SUPERSEDED, CANCELLED

**DSL verbs targeting this slot (6):**

| Verb FQN | state_effect | description |
|---|---|---|
| `deal.agree-rate-card` | transition | Finalise rate card - both parties agree. Lines become immutable. |
| `deal.counter-rate-card` | transition | Client counter-offer - creates new version via clone |
| `deal.pricing-approve` | transition | Pricing committee approval of rate card — advances to ready-to-propose |
| `deal.pricing-reject` | transition | Pricing committee rejection — rate card returns to DRAFT for revision |
| `deal.propose-rate-card` | transition | Submit rate card for client review |
| `deal.submit-for-pricing-approval` | transition | Submit rate card to pricing committee for internal approval (bespoke / discount threshold / new jurisdiction) |


#### `deal_onboarding_request` · deal_onboarding_request_lifecycle

_scope: per_onboarding_request · entity: "ob-poc".deal_onboarding_requests · col: request_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ |  | — |
| `IN_PROGRESS` |  |  | — |
| `BLOCKED` |  | Held on external dependency (usually a KYC re-open or document gap). | — |
| `COMPLETED` |  | Terminal-positive; CBU fully onboarded for this deal scope. | — |
| `CANCELLED` |  | Terminal-negative. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **IN_PROGRESS** | `deal.update-onboarding-status` |  |
| IN_PROGRESS | **COMPLETED** | `deal.update-onboarding-status` |  |
| (PENDING, IN_PROGRESS) | **BLOCKED** | `deal.update-onboarding-status` |  |
| BLOCKED | **IN_PROGRESS** | `deal.update-onboarding-status` |  |
| (PENDING, IN_PROGRESS, BLOCKED) | **CANCELLED** | `deal.update-onboarding-status` |  |

**Terminal states:** COMPLETED, CANCELLED

**DSL verbs targeting this slot (1):**

| Verb FQN | state_effect | description |
|---|---|---|
| `deal.update-onboarding-status` | transition | Update onboarding request status |


#### `deal_document` · deal_document_lifecycle

_scope: per_deal_document · entity: "ob-poc".deal_documents · col: document_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DRAFT` | ✓ |  | — |
| `UNDER_REVIEW` |  |  | — |
| `SIGNED` |  | Counterparty signature obtained. | — |
| `EXECUTED` |  | Fully executed; binding. Terminal-positive. | — |
| `SUPERSEDED` |  | Replaced by a newer version. | — |
| `ARCHIVED` |  | Retained for audit; no longer operationally live. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DRAFT | **UNDER_REVIEW** | `deal.update-document-status` |  |
| UNDER_REVIEW | **SIGNED** | `deal.update-document-status` |  |
| SIGNED | **EXECUTED** | `deal.update-document-status` |  |
| (EXECUTED, SIGNED) | **SUPERSEDED** | `deal.update-document-status` |  |
| (EXECUTED, SUPERSEDED) | **ARCHIVED** | `deal.update-document-status` |  |

**Terminal states:** EXECUTED, SUPERSEDED, ARCHIVED

**DSL verbs targeting this slot (1):**

| Verb FQN | state_effect | description |
|---|---|---|
| `deal.update-document-status` | transition | Update document status (e.g. DRAFT → SIGNED → EXECUTED) |


#### `deal_ubo_assessment` · deal_ubo_assessment_lifecycle

_scope: per_entity_in_deal · entity: "ob-poc".deal_ubo_assessments · col: assessment_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ |  | — |
| `IN_PROGRESS` |  |  | — |
| `COMPLETED` |  | Terminal-positive; risk-rated (LOW/MEDIUM/HIGH). | — |
| `REQUIRES_EDD` |  | Enhanced due diligence required; routes to KYC. | — |
| `BLOCKED` |  | PROHIBITED risk rating — deal cannot proceed for this entity. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **IN_PROGRESS** | `deal.update-ubo-assessment` |  |
| IN_PROGRESS | **COMPLETED** | `deal.update-ubo-assessment` |  |
| (IN_PROGRESS, COMPLETED) | **REQUIRES_EDD** | `deal.update-ubo-assessment` |  |
| (IN_PROGRESS, REQUIRES_EDD) | **BLOCKED** | `deal.update-ubo-assessment` |  |

**Terminal states:** COMPLETED, BLOCKED

**DSL verbs targeting this slot (1):**

| Verb FQN | state_effect | description |
|---|---|---|
| `deal.update-ubo-assessment` | transition | Update UBO assessment status and risk rating |


#### `billing_profile` · billing_profile_lifecycle

_owner: ops+finance · scope: per_billing_profile · lifetime: long_lived · entity: "ob-poc".fee_billing_profiles · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DRAFT` | ✓ |  | — |
| `ACTIVE` |  | Billing cycle running; periods being created. | — |
| `SUSPENDED` |  | Temporarily paused (dispute, credit hold, winding-down). | — |
| `CLOSED` |  | Terminal; deal offboarded or billing retired. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DRAFT | **ACTIVE** | `billing.activate-profile` | bound deal_rate_card.status = AGREED |
| ACTIVE | **SUSPENDED** | `billing.suspend-profile` |  |
| SUSPENDED | **ACTIVE** | `billing.activate-profile` |  |
| (DRAFT, ACTIVE, SUSPENDED) | **CLOSED** | `billing.close-profile` |  |

**Terminal states:** CLOSED

**DSL verbs targeting this slot (3):**

| Verb FQN | state_effect | description |
|---|---|---|
| `billing.activate-profile` | transition | Activate a billing profile (CBU is live and generating activity) |
| `billing.close-profile` | transition | Close billing profile (offboarding) |
| `billing.suspend-profile` | transition | Suspend billing (e.g. dispute, investigation) |


#### `billing_period` · billing_period_lifecycle

_scope: per_billing_period · entity: "ob-poc".fee_billing_periods · col: calc_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ | Period created; calculation not yet run. | — |
| `CALCULATED` |  | Fees computed; pending analyst review. | — |
| `REVIEWED` |  | Reviewed by ops; ready for approval. | — |
| `APPROVED` |  | Approved for invoicing. | — |
| `INVOICED` |  | Invoice emitted. Terminal-positive. | — |
| `DISPUTED` |  | Client raised a dispute; out-of-band resolution required. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **CALCULATED** | `billing.calculate-period` |  |
| CALCULATED | **REVIEWED** | `billing.review-period` |  |
| REVIEWED | **APPROVED** | `billing.approve-period` |  |
| APPROVED | **INVOICED** | `billing.generate-invoice` |  |
| (CALCULATED, REVIEWED, APPROVED, INVOICED) | **DISPUTED** | `billing.dispute-period` |  |

**Terminal states:** INVOICED, DISPUTED

**DSL verbs targeting this slot (5):**

| Verb FQN | state_effect | description |
|---|---|---|
| `billing.approve-period` | transition | Approve billing period for invoicing |
| `billing.calculate-period` | transition | Run fee calculation for a billing period |
| `billing.dispute-period` | transition | Client disputes a billing period |
| `billing.generate-invoice` | transition | Generate invoice from approved billing period |
| `billing.review-period` | transition | Mark billing period as reviewed, optionally apply adjustments |


#### `client_group` · (stateless)


#### `group_kyc_clearance` · (stateless)


#### `kyc_case` · (stateless)


#### `cbu` · (stateless)


#### `contract` · (stateless)


#### `workspace_root` · (stateless)


#### `deal_participant` · (stateless)


#### `deal_contract` · (stateless)


#### `rate_card_line` · (stateless)


#### `deal_sla` · deal_sla_lifecycle

_owner: ops · scope: per_sla · lifetime: long_lived · entity: "ob-poc".deal_slas · col: sla_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `NEGOTIATED` | ✓ | SLA agreed but deal not yet CONTRACTED. | — |
| `ACTIVE` |  | Deal CONTRACTED; SLA being measured. | — |
| `BREACHED` |  | Measured performance fell below SLA threshold; penalty
calculation triggered (feeds into billing_period via
billing workflow).
 | — |
| `IN_REMEDIATION` |  | Corrective action in progress; breach being resolved. | — |
| `RESOLVED` |  | Breach mitigated; SLA returns to ACTIVE. | — |
| `WAIVED` |  | Client waived the breach (out-of-band concession). | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| NEGOTIATED | **ACTIVE** | _(backend: deal.CONTRACTED)_ |  |
| ACTIVE | **BREACHED** | _(backend: SLA measurement fails threshold)_ |  |
| BREACHED | **IN_REMEDIATION** | `deal.start-sla-remediation` |  |
| IN_REMEDIATION | **RESOLVED** | `deal.resolve-sla-breach` |  |
| RESOLVED | **ACTIVE** | _(backend: remediation complete; return to monitoring)_ |  |
| BREACHED | **WAIVED** | `deal.waive-sla-breach` |  |

**DSL verbs targeting this slot (3):**

| Verb FQN | state_effect | description |
|---|---|---|
| `deal.resolve-sla-breach` | transition | Resolve SLA breach (remediation complete; transition to RESOLVED) |
| `deal.start-sla-remediation` | transition | Begin remediation of a breached SLA (transition BREACHED → IN_REMEDIATION) |
| `deal.waive-sla-breach` | transition | Waive SLA breach (client concession; breach not pursued) |


#### `pricing_config` · (stateless)


#### `client_principal_relationship` · (stateless)


#### `contract_template` · (stateless)


#### `billing_account_target` · (stateless)


### Cross-workspace constraints (2 — Mode A blocking)

- **deal_contracted_requires_kyc_approved** (error): Deal cannot progress IN_CLEARANCE → CONTRACTED unless the
client-group KYC case is APPROVED in the KYC workspace.
Q4/Q21 (b) (2026-04-29, D-004): retargeted to IN_CLEARANCE.
  - source: `kyc.kyc_case` state ∈ APPROVED
  - target: `deal.deal` transition `IN_CLEARANCE -> CONTRACTED`
- **deal_contracted_requires_bp_approved** (error): Deal → CONTRACTED requires every booking-principal clearance
attached to this deal to be in APPROVED or ACTIVE state. Third
leg of Adam's deal tollgate triad (BAC + KYC + BP).
Q4/Q21 (b) (2026-04-29, D-004): retargeted to IN_CLEARANCE.
  - source: `booking_principal.clearance` state ∈ APPROVED, ACTIVE
  - target: `deal.deal` transition `IN_CLEARANCE -> CONTRACTED`

### Cross-slot constraints (10 — intra-DAG)

- **deal_contracted_requires_agreed_rate_card** (error): Deal → CONTRACTED requires at least one AGREED rate card for the
deal. Rate cards must complete the propose/counter/agree chain.
  - rule: `deals.deal_status = 'CONTRACTED'` ...
- **deal_contracted_requires_bac_approved** (error): Deal → CONTRACTED requires deals.bac_status = 'approved'. Third
leg of the IN_CLEARANCE compound gate (BAC + KYC + BP).
  - rule: `deals.deal_status = 'CONTRACTED'` ...
- **deal_active_requires_all_onboarding_complete** (error): Deal → ACTIVE requires all onboarding requests COMPLETED.
  - rule: `deals.deal_status = 'ACTIVE'` ...
- **deal_active_requires_active_billing_profile** (error): Deal → ACTIVE requires at least one ACTIVE fee_billing_profile.
Rationale: ACTIVE means revenue flowing; no active profile means
no revenue recognition, which contradicts the business semantic.
  - rule: `deals.deal_status = 'ACTIVE'` ...
- **rate_card_agreed_uniqueness** (error): At most ONE AGREED rate card per (deal, contract, product).
New AGREED automatically SUPERSEDES the previous AGREED. DB-enforced
via idx_deal_rate_cards_one_agreed (migration 069).
  - rule: `COUNT deal_rate_cards WHERE deal_id = X AND contract_id = Y` ...
- **billing_profile_activation_requires_agreed_rate_card** (error): fee_billing_profiles.status = ACTIVE requires the bound
deal_rate_card.status = AGREED.
  - rule: `fee_billing_profiles.status = 'ACTIVE'` ...
- **billing_period_creation_requires_active_profile** (error): fee_billing_periods require parent profile = ACTIVE.
  - rule: `fee_billing_periods exists` ...
- **deal_offboarded_requires_all_billing_closed** (error): Deal → OFFBOARDED requires all billing_profile.status = CLOSED.
  - rule: `deals.deal_status = 'OFFBOARDED'` ...
- **deal_ubo_assessment_blocked_halts_deal** (error): If any deal_ubo_assessment.assessment_status = BLOCKED
(PROHIBITED risk rating), the deal cannot advance past
KYC_CLEARANCE. Manual remediation or deal cancellation required.
  - rule: `EXISTS deal_ubo_assessments WHERE deal_id = this.deal_id` ...
- **counter_proposal_returns_agreed_to_counter** (informational): When a previously AGREED rate card has lines updated, the
rate-card status returns to COUNTER_PROPOSED (business rule —
substantive edits invalidate the agreement). Enforced by verb
lifecycle, not validator.
  - rule: `deal_rate_cards.status = 'AGREED' AND lines updated` ...


## Booking Principal

**File:** `rust/config/sem_os_seeds/dag_taxonomies/booking_principal_dag.yaml` · **Workspace:** `booking_principal`

Per-deal booking-principal clearance (compound triad leg with BAC + KYC).

### Slots (2)

#### `workspace_root` · (stateless)


#### `clearance` · booking_principal_clearance_lifecycle

_owner: ops+credit · scope: per_deal_principal · lifetime: long_lived · entity: "ob-poc".booking_principal_clearances · col: clearance_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ | Clearance request created; screening not yet started. | — |
| `SCREENING` |  | Screening in progress (sanctions, credit, regulatory). | — |
| `APPROVED` |  | Screening passed; ready to activate. Gate-eligible. | every screening_check exists<br>AND every screening_check.state = PASSED<br>AND no red_flag exists with state = OPEN AND attached_to this clearance<br>AND regulatory_licence exists<br>AND regulatory_licence.state = CURRENT |
| `REJECTED` |  | Screening failed. Reopenable to PENDING for retry. | — |
| `ACTIVE` |  | Clearance live; principal cleared to book the deal. | — |
| `SUSPENDED` |  | Temporarily held (regulatory action, dispute, audit). | — |
| `REVOKED` |  | Terminal — clearance withdrawn permanently. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **SCREENING** | `booking-principal-clearance.start-screening` |  |
| SCREENING | **APPROVED** | `booking-principal-clearance.approve` |  |
| SCREENING | **REJECTED** | `booking-principal-clearance.reject` |  |
| REJECTED | **PENDING** | `booking-principal-clearance.reopen` |  |
| APPROVED | **ACTIVE** | `booking-principal-clearance.activate` |  |
| ACTIVE | **SUSPENDED** | `booking-principal-clearance.suspend` |  |
| SUSPENDED | **ACTIVE** | `booking-principal-clearance.reinstate` |  |
| APPROVED, ACTIVE, SUSPENDED | **REVOKED** | `booking-principal-clearance.revoke` |  |

**Terminal states:** REVOKED

**DSL verbs targeting this slot (8):**

| Verb FQN | state_effect | description |
|---|---|---|
| `booking-principal-clearance.activate` | transition | Transition clearance APPROVED → ACTIVE (clearance live for this deal) |
| `booking-principal-clearance.approve` | transition | Transition clearance SCREENING → APPROVED (screening passed) |
| `booking-principal-clearance.reinstate` | transition | Transition clearance SUSPENDED → ACTIVE (lift suspension) |
| `booking-principal-clearance.reject` | transition | Transition clearance SCREENING → REJECTED (screening failed; reopenable) |
| `booking-principal-clearance.reopen` | transition | Transition clearance REJECTED → PENDING (retry after remediation) |
| `booking-principal-clearance.revoke` | transition | Transition clearance APPROVED\|ACTIVE\|SUSPENDED → REVOKED (terminal — withdraw clearance) |
| `booking-principal-clearance.start-screening` | transition | Transition clearance PENDING → SCREENING (begin sanctions / credit / regulatory screening) |
| `booking-principal-clearance.suspend` | transition | Transition clearance ACTIVE → SUSPENDED (regulatory hold, audit, dispute) |



## Product / Service Taxonomy

**File:** `rust/config/sem_os_seeds/dag_taxonomies/product_service_taxonomy_dag.yaml` · **Workspace:** `product_maintenance`

Catalogue authoring: governed service definitions + versions.

### Slots (6)

#### `workspace_root` · (stateless)


#### `product` · (stateless)


#### `service` · service_lifecycle

_owner: stewards · scope: per_service · lifetime: long_lived · entity: "ob-poc".services · col: lifecycle_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `ungoverned` | ✓ | Pre-governance; service exists but is not formally defined. | — |
| `draft` |  | Service being authored within a changeset. | — |
| `active` |  | Published; consumers reference this service. | — |
| `deprecated` |  | Replaced by newer version; migrate consumers. | — |
| `retired` |  | Terminal; no longer in use. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| ungoverned | **draft** | `service.define` |  |
| draft | **active** | _(backend: changeset.publish includes this service)_ |  |
| active | **draft** | `service.propose-revision` |  |
| active | **deprecated** | `service.deprecate` |  |
| deprecated | **retired** | `service.retire` |  |

**Terminal states:** retired

**DSL verbs targeting this slot (4):**

| Verb FQN | state_effect | description |
|---|---|---|
| `service.define` | transition | Promote an ungoverned service into the governed draft state |
| `service.deprecate` | transition | Mark an active service as deprecated |
| `service.propose-revision` | transition | Move an active service back to draft for revision |
| `service.retire` | transition | Retire a deprecated service (terminal) |


#### `service_version` · service_version_lifecycle

_owner: stewards · scope: per_service_version · lifetime: long_lived · entity: "ob-poc".service_versions · col: lifecycle_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `drafted` | ✓ | Version drafted; under authoring. | — |
| `reviewed` |  | Submitted for and passed review. | — |
| `published` |  | Active version; consumers may reference. | — |
| `superseded` |  | Newer version published; this one no longer current. | — |
| `retired` |  | Terminal; removed from circulation. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| drafted | **reviewed** | `service-version.submit-for-review` |  |
| reviewed | **published** | `service-version.publish` |  |
| published | **superseded** | _(backend: new published version of same service)_ |  |
| published, superseded | **retired** | `service-version.retire` |  |

**Terminal states:** retired

**DSL verbs targeting this slot (3):**

| Verb FQN | state_effect | description |
|---|---|---|
| `service-version.publish` | transition | Publish a reviewed service version |
| `service-version.retire` | transition | Retire a service version (terminal) |
| `service-version.submit-for-review` | transition | Submit a drafted service version for review |


#### `service_resource` · (stateless)


#### `attribute` · (stateless)



## Instrument Matrix

**File:** `rust/config/sem_os_seeds/dag_taxonomies/instrument_matrix_dag.yaml` · **Workspace:** `instrument_matrix`

Service resources, trading profiles, settlement chains, trade gateways — the operational substrate per CBU.

### Slots (22)

#### `workspace_root` · (stateless)


#### `group` · group_discovery_lifecycle

_scope: per_group · entity: "ob-poc".client_group · col: discovery_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `not_started` | ✓ | Group record created; no discovery activity yet. | — |
| `in_progress` |  | Research / GLEIF import running. | — |
| `complete` |  | Discovery confirmed; group facts validated. | — |
| `stale` |  | Time-based staleness trigger fired; refresh needed. | — |
| `failed` |  | Research fault; needs investigation. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| not_started | **in_progress** | `research pipeline kick-off (upstream trigger)` |  |
| in_progress | **complete** | `discovery confirmed signal (backend)` |  |
| in_progress | **failed** | `research fault signal (backend)` |  |
| complete | **stale** | `time-decay trigger` |  |
| stale | **in_progress** | `refresh trigger` |  |
| failed | **in_progress** | `retry trigger` |  |


#### `trading_profile_template` · trading_profile_template_lifecycle

_scope: per_template · entity: "ob-poc".cbu_trading_profiles · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `available` | ✓ | Template is published and available for cloning to new CBUs. | — |
| `unavailable` |  | Template retired or deprecated. Existing CBU instances cloned
from it remain unaffected — template change is forward-only.
 | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| available | **unavailable** | `trading-profile.retire-template` |  |

**Terminal states:** unavailable


#### `settlement_pattern_template` · settlement_pattern_lifecycle

_entity: "ob-poc".cbu_settlement_chains · col: (derived — see below)_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `draft` | ✓ | Chain created but no hops defined. | — |
| `configured` |  | Hops added; locations defined; SSIs attached. | — |
| `reviewed` |  | Ops / treasury review signed off. | — |
| `parallel_run` |  | Running alongside incumbent chain for 1-2 cycles. | — |
| `live` |  | Active settlement chain; new trades route through. | — |
| `suspended` |  | Operational hold (sub-custodian issue, CA in-flight). | — |
| `deactivated` |  | Chain retired. `superseded_by` may reference replacement. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| draft | **configured** | `settlement-chain.add-hop` |  |
| draft | **configured** | `settlement-chain.define-location` |  |
| configured | **reviewed** | `settlement-chain.request-review` |  |
| reviewed | **parallel_run** | `settlement-chain.enter-parallel-run` |  |
| parallel_run | **live** | `settlement-chain.go-live` |  |
| parallel_run | **reviewed** | `settlement-chain.abort-parallel-run` |  |
| live | **suspended** | `settlement-chain.suspend` |  |
| suspended | **live** | `settlement-chain.reactivate` |  |
| live | **deactivated** | `settlement-chain.deactivate-chain` |  |
| suspended | **deactivated** | `settlement-chain.deactivate-chain` |  |

**Terminal states:** deactivated

**DSL verbs targeting this slot (8):**

| Verb FQN | state_effect | description |
|---|---|---|
| `settlement-chain.abort-parallel-run` | transition | Abort parallel_run — roll back to reviewed state (issues found) |
| `settlement-chain.add-hop` | transition | Add intermediary hop to settlement chain |
| `settlement-chain.deactivate-chain` | transition | Deactivate a settlement chain |
| `settlement-chain.enter-parallel-run` | transition | Enter parallel-run state — new chain runs alongside incumbent for 1-2 cycles |
| `settlement-chain.go-live` | transition | Activate chain as live (parallel_run → live) |
| `settlement-chain.reactivate` | transition | Reactivate a suspended settlement chain (suspended → live) |
| `settlement-chain.request-review` | transition | Submit a configured settlement chain for ops / treasury review |
| `settlement-chain.suspend` | transition | Suspend a live settlement chain (operational hold) |


#### `isda_framework` · (stateless)


#### `corporate_action_policy` · (stateless)


#### `trade_gateway` · trade_gateway_lifecycle

_entity: (hybrid — JSON document + is_active column for query speed) · col: (document body — not SQL CHECK)_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `defined` | ✓ | Gateway config authored; IM-side ready. | — |
| `enabled` |  | Config complete on IM side; awaiting counterparty. | — |
| `active` |  | Counterparty confirmed; session live. | — |
| `suspended` |  | Operational hold (cert expiry, session down). | — |
| `retired` |  | Gateway relationship terminated. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| defined | **enabled** | `trade-gateway.enable-gateway` |  |
| enabled | **active** | `trade-gateway.activate-gateway` |  |
| active | **suspended** | `trade-gateway.suspend-gateway` |  |
| suspended | **active** | `trade-gateway.reactivate-gateway` |  |
| active | **retired** | `trade-gateway.retire-gateway` |  |
| suspended | **retired** | `trade-gateway.retire-gateway` |  |

**Terminal states:** retired

**DSL verbs targeting this slot (3):**

| Verb FQN | state_effect | description |
|---|---|---|
| `trade-gateway.enable-gateway` | transition | Enable gateway connectivity for CBU |
| `trade-gateway.reactivate-gateway` | transition | Reactivate a suspended trade gateway (suspended → active) |
| `trade-gateway.retire-gateway` | transition | Retire a trade gateway (terminal — broker relationship ended) |


#### `cbu` · (stateless)


#### `trading_profile` · trading_profile_lifecycle

_owner: trading+ops · scope: per_profile · lifetime: long_lived · entity: "ob-poc".cbu_trading_profiles · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DRAFT` | ✓ | Profile being edited; not yet submitted. | — |
| `SUBMITTED` |  | Submitted for compliance / ops review. | — |
| `APPROVED` |  | Approved but not yet activated. Ready for parallel run. | compliance_review exists<br>AND compliance_review.state = COMPLETE<br>AND ops_review exists<br>AND ops_review.state = COMPLETE<br>AND every required committee_signoff exists<br>AND every required committee_signoff.state = SIGNED<br>AND no objection exists with state = OPEN |
| `PARALLEL_RUN` |  | Running shadow alongside incumbent (or empty for new mandate). | — |
| `ACTIVE` |  | Mandate live — trading flowing. | — |
| `SUSPENDED` |  | Operational hold. | — |
| `REJECTED` |  | Rejected in review — revertible to draft. | — |
| `SUPERSEDED` |  | Version replaced by newer version (Adam pass-3 recommendation).
Distinct from ARCHIVED which means 'mandate terminated.'
 | — |
| `ARCHIVED` |  | Terminal — mandate terminated. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DRAFT | **SUBMITTED** | `trading-profile.submit` |  |
| SUBMITTED | **APPROVED** | `trading-profile.approve` |  |
| SUBMITTED | **REJECTED** | `trading-profile.reject` |  |
| REJECTED | **DRAFT** | `trading-profile.create-draft` |  |
| APPROVED | **PARALLEL_RUN** | `trading-profile.enter-parallel-run` |  |
| PARALLEL_RUN | **ACTIVE** | `trading-profile.go-live` |  |
| PARALLEL_RUN | **APPROVED** | `trading-profile.abort-parallel-run` |  |
| ACTIVE | **SUSPENDED** | `trading-profile.suspend` |  |
| SUSPENDED | **ACTIVE** | `trading-profile.reactivate` |  |
| ACTIVE | **SUPERSEDED** | `trading-profile.supersede` |  |
| ACTIVE | **ARCHIVED** | `trading-profile.archive` |  |
| SUSPENDED | **ARCHIVED** | `trading-profile.archive` |  |

**Terminal states:** ARCHIVED, SUPERSEDED

**DSL verbs targeting this slot (14):**

| Verb FQN | state_effect | description |
|---|---|---|
| `trading-profile.abort-parallel-run` | transition | Abort parallel run — roll back to approved |
| `trading-profile.activate` | transition | DEPRECATED — use trading-profile.go-live instead.

Legacy verb that directly transitioned APPROVED → ACTIVE. The pilot
P |
| `trading-profile.approve` | transition | Approve a pending trading profile, activating it (transitions PENDING_REVIEW -> ACTIVE) |
| `trading-profile.archive` | transition | Archive a trading profile (soft delete, transitions to ARCHIVED status) |
| `trading-profile.enter-parallel-run` | transition | Enter parallel_run state (approved → parallel_run) |
| `trading-profile.go-live` | transition | Go-live on a mandate (parallel_run → active) |
| `trading-profile.lift-restriction` | transition | Lift trading-profile restriction (restore full mandate scope) |
| `trading-profile.reactivate` | transition | Reactivate a suspended mandate (suspended → active) |
| `trading-profile.reject` | transition | Reject a pending trading profile (transitions PENDING_REVIEW -> DRAFT with rejection reason) |
| `trading-profile.restrict` | transition | Apply partial trading restriction to mandate (pull specific markets / instruments) |
| `trading-profile.retire-template` | transition | Retire a template (available → unavailable) |
| `trading-profile.submit` | transition | Submit a validated trading profile for client review (transitions VALIDATED -> PENDING_REVIEW) |
| `trading-profile.supersede` | transition | Supersede active version with newer version (active → superseded) |
| `trading-profile.suspend` | transition | Suspend active mandate (operational hold) |


#### `trading_activity` · trading_activity_lifecycle

_owner: trading+ops · scope: per_cbu · lifetime: long_lived · entity: "ob-poc".cbu_trading_activity · col: activity_state_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `never_traded` | ✓ | trading_profile is ACTIVE but first_trade_at IS NULL. CBU is
permissioned but not yet executing.
 | — |
| `trading` |  | first_trade_at IS NOT NULL AND last_trade_at within activity
window (e.g. last 90 days). Actively making money.
 | — |
| `dormant` |  | first_trade_at IS NOT NULL but last_trade_at older than
dormancy threshold. Trading-capable but not currently
executing — flags review (is t | — |
| `suspended` |  | Trading activity paused alongside trading_profile.SUSPENDED
(mirror state).
 | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| never_traded | **trading** | _(backend: first trade posted — event from settlement pipeline)_ |  |
| trading | **dormant** | _(backend: last_trade_at + dormancy_window < now)_ |  |
| dormant | **trading** | _(backend: new trade posted)_ |  |
| never_traded, trading, dormant | **suspended** | _(backend: trading_profile.SUSPENDED triggers mirror)_ |  |
| suspended | **trading** | _(backend: trading_profile.reactivate triggers mirror)_ |  |


#### `custody` · (stateless)


#### `booking_principal` · (stateless)


#### `cash_sweep` · (stateless)


#### `service_resource` · service_resource_lifecycle

_entity: "ob-poc".service_resource_types · col: (derived from is_active + provisioning_strategy)_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `provisioned` | ✓ | Resource definition authored and available. | — |
| `activated` |  | Resource instance created and running for at least one CBU. | — |
| `suspended` |  | Resource paused — existing instances remain but no new provisioning. | — |
| `decommissioned` |  | Terminal — resource retired from catalogue. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| provisioned | **activated** | `service-resource.activate` |  |
| activated | **suspended** | `service-resource.suspend` |  |
| suspended | **activated** | `service-resource.reactivate` |  |
| activated | **decommissioned** | `service-resource.decommission` |  |
| suspended | **decommissioned** | `service-resource.decommission` |  |

**Terminal states:** decommissioned

**DSL verbs targeting this slot (3):**

| Verb FQN | state_effect | description |
|---|---|---|
| `service-resource.activate` | transition | Activate a service resource instance |
| `service-resource.reactivate` | transition | Reactivate suspended service resource (suspended → activated) |
| `service-resource.suspend` | transition | Suspend a service resource instance |


#### `service_intent` · service_intent_lifecycle

_scope: per_intent · entity: "ob-poc".service_intents · col: status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `active` | ✓ | Intent is in force; drives Discovery Engine expansion. | — |
| `suspended` |  | Temporarily paused — existing resources retained. | — |
| `cancelled` |  | Terminal — intent withdrawn. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| active | **suspended** | `service-intent.suspend` |  |
| suspended | **active** | `service-intent.resume` |  |
| active | **cancelled** | `service-intent.cancel` |  |
| suspended | **cancelled** | `service-intent.cancel` |  |

**Terminal states:** cancelled

**DSL verbs targeting this slot (3):**

| Verb FQN | state_effect | description |
|---|---|---|
| `service-intent.cancel` | transition | Cancel a service intent (terminal) |
| `service-intent.resume` | transition | Resume a suspended service intent (suspended → active) |
| `service-intent.suspend` | transition | Suspend an active service intent (temporary hold) |


#### `booking_location` · (stateless)


#### `legal_entity` · (stateless)


#### `product` · (stateless)


#### `delivery` · delivery_lifecycle

_entity: "ob-poc".service_delivery_map · col: delivery_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PENDING` | ✓ | Delivery requested; not yet started. | — |
| `IN_PROGRESS` |  | Delivery under way. | — |
| `DELIVERED` |  | Delivery complete. | every delivery_line_item exists<br>AND every delivery_line_item.state = CONFIRMED_RECEIVED<br>AND no exception exists with state = OPEN<br>AND counterparty_acknowledgement exists<br>AND counterparty_acknowledgement.state = RECEIVED |
| `FAILED` |  | Delivery failed. | — |
| `CANCELLED` |  | Delivery cancelled before completion. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PENDING | **IN_PROGRESS** | `delivery.start` |  |
| IN_PROGRESS | **DELIVERED** | `delivery.complete` |  |
| IN_PROGRESS | **FAILED** | `delivery.fail` |  |
| PENDING | **CANCELLED** | `delivery.cancel` |  |
| IN_PROGRESS | **CANCELLED** | `delivery.cancel` |  |

**Terminal states:** DELIVERED, FAILED, CANCELLED

**DSL verbs targeting this slot (2):**

| Verb FQN | state_effect | description |
|---|---|---|
| `delivery.cancel` | transition | Cancel a delivery before completion |
| `delivery.start` | transition | Start a pending delivery (PENDING → IN_PROGRESS) |


#### `reconciliation` · reconciliation_config_lifecycle

_entity: (new — P.2 schema work or reuse of existing recon table if present) · col: (new)_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `draft` | ✓ | Recon config authored but not yet active. | — |
| `active` |  | Recon config governing actual recon streams. | — |
| `suspended` |  | Recon temporarily paused. | — |
| `retired` |  | Recon config decommissioned. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| draft | **active** | `reconciliation.activate` |  |
| active | **suspended** | `reconciliation.suspend` |  |
| suspended | **active** | `reconciliation.reactivate` |  |
| active | **retired** | `reconciliation.retire` |  |
| suspended | **retired** | `reconciliation.retire` |  |

**Terminal states:** retired

**DSL verbs targeting this slot (4):**

| Verb FQN | state_effect | description |
|---|---|---|
| `reconciliation.activate` | transition | Activate a reconciliation config (draft → active) |
| `reconciliation.reactivate` | transition | Re-activate a suspended reconciliation (suspended → active) |
| `reconciliation.retire` | transition | Retire a reconciliation config (terminal) |
| `reconciliation.suspend` | transition | Suspend an active reconciliation (operational hold) |


#### `corporate_action_event` · corporate_action_event_lifecycle

_entity: (new — per-mandate CA event tracking) · col: (new)_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `election_pending` | ✓ | CA event attached to mandate; election not yet made. | — |
| `elected` |  | Mandate actively elected a choice. | — |
| `default_applied` |  | Cutoff passed; default option from policy auto-applied. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| election_pending | **elected** | `corporate-action-event.elect` |  |
| election_pending | **default_applied** | _(automatic trigger at cutoff)_ |  |

**Terminal states:** elected, default_applied

**DSL verbs targeting this slot (2):**

| Verb FQN | state_effect | description |
|---|---|---|
| `corporate-action-event.attach` | transition | Attach a CA event to a mandate (entry state: election_pending) |
| `corporate-action-event.elect` | transition | Make an election on a pending CA event |


#### `collateral_management` · collateral_management_lifecycle

_entity: (new — per-mandate CSA/collateral config) · col: (new)_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `configured` | ✓ | CSA config authored; collateral schedule set. | — |
| `active` |  | Collateral management live for this mandate. | — |
| `suspended` |  | Collateral operations paused. | — |
| `terminated` |  | Terminal — collateral arrangement unwound. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| configured | **active** | `collateral-management.activate` |  |
| active | **suspended** | `collateral-management.suspend` |  |
| suspended | **active** | `collateral-management.reactivate` |  |
| active | **terminated** | `collateral-management.terminate` |  |
| suspended | **terminated** | `collateral-management.terminate` |  |

**Terminal states:** terminated

**DSL verbs targeting this slot (5):**

| Verb FQN | state_effect | description |
|---|---|---|
| `collateral-management.activate` | transition | Activate collateral management (configured → active) |
| `collateral-management.configure` | transition | Configure collateral management for a CBU (entry state) |
| `collateral-management.reactivate` | transition | Reactivate suspended collateral (suspended → active) |
| `collateral-management.suspend` | transition | Suspend collateral operations (operational hold) |
| `collateral-management.terminate` | transition | Terminate collateral management (terminal) |


### Cross-workspace constraints (1 — Mode A blocking)

- **mandate_requires_validated_cbu** (error): Trading profile cannot leave DRAFT unless the CBU is VALIDATED in the CBU workspace's discovery lifecycle.
  - source: `cbu.cbu` state ∈ VALIDATED
  - target: `instrument_matrix.trading_profile` transition `DRAFT -> SUBMITTED`

### Cross-slot constraints (10 — intra-DAG)

- **mandate_requires_validated_cbu** (error): trading_profile cannot progress past DRAFT unless cbu is VALIDATED.
  - rule: `trading_profile.status NOT IN [DRAFT]` ...
- **mandate_active_requires_live_settlement** (error): A trading_profile cannot go ACTIVE without at least one LIVE
settlement_pattern chain.
  - rule: `trading_profile.status = ACTIVE` ...
- **archived_mandate_cascades_dependents** (warning): When trading_profile becomes ARCHIVED, dependent slots
(settlement_pattern live chains, trade_gateway active gateways,
cash_sweep active configs, service_intent active rows) must
transition to terminal or suspended.
  - rule: `trading_profile.status = ARCHIVED` ...
- **cbu_suspended_implies_mandate_suspended** (warning): When cbu is SUSPENDED, any active trading_profile should be
in SUSPENDED state (operational consistency).
  - rule: `cbu.status = SUSPENDED AND EXISTS(trading_profile WHERE cbu_id=this.cbu_id AND status = ACTIVE)` ...
- **cbu_archived_requires_mandate_archived** (error): CBU cannot be archived while any trading_profile is not archived.
  - rule: `cbu.status = ARCHIVED` ...
- **retired_gateway_prunes_routing_rules** (error): Retiring a trade_gateway must prune any routing rules that
reference it on an active trading_profile.
  - rule: `trade_gateway.state = retired` ...
- **deactivated_chain_requires_universe_recheck** (warning): Deactivating a settlement_pattern chain leaves some universe
un-settleable. Re-validate go-live readiness.
  - rule: `settlement_pattern.state = deactivated` ...
- **decommissioned_resource_cascades_intent** (error): service_resource decommissioned → any service_intent referencing
this resource must be cancelled (otherwise the intent is stranded).
  - rule: `service_resource.state = decommissioned` ...
- **isda_coverage_required_for_derivative_trading** (error): trading_profile active with derivative instruments in scope
requires at least one ISDA coverage for each OTC counterparty.
  - rule: `trading_profile.status = ACTIVE` ...
- **collateral_management_active_requires_isda** (error): collateral_management can only be ACTIVE if ISDA coverage exists.
  - rule: `collateral_management.state = active` ...


## Lifecycle Resources

**File:** `rust/config/sem_os_seeds/dag_taxonomies/lifecycle_resources_dag.yaml` · **Workspace:** `lifecycle_resources`

Application instances + capability bindings — runtime infrastructure lifecycle.

### Slots (4)

#### `workspace_root` · (stateless)


#### `application` · (stateless)


#### `application_instance` · application_instance_lifecycle

_owner: ops+platform · scope: per_application_instance · lifetime: long_lived · entity: "ob-poc".application_instances · col: lifecycle_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `PROVISIONED` | ✓ | Instance row created; not yet serving traffic. | — |
| `ACTIVE` |  | Instance live and healthy; serving bindings. | — |
| `MAINTENANCE_WINDOW` |  | Planned maintenance — traffic paused; bindings still bound. | — |
| `DEGRADED` |  | Health-check signal: instance impaired but partially serving. | — |
| `OFFLINE` |  | Operational hold — incident, security freeze, or capacity action. | — |
| `DECOMMISSIONED` |  | Terminal — instance retired; bindings cascaded to RETIRED. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| PROVISIONED | **ACTIVE** | `application-instance.activate` |  |
| ACTIVE | **MAINTENANCE_WINDOW** | `application-instance.enter-maintenance` |  |
| MAINTENANCE_WINDOW | **ACTIVE** | `application-instance.exit-maintenance` |  |
| ACTIVE | **DEGRADED** | _(backend: health-check signal)_ |  |
| DEGRADED | **ACTIVE** | _(backend: health-check signal)_ |  |
| ACTIVE | **OFFLINE** | `application-instance.take-offline` |  |
| OFFLINE | **ACTIVE** | `application-instance.bring-online` |  |
| ACTIVE, OFFLINE, MAINTENANCE_WINDOW, DEGRADED | **DECOMMISSIONED** | `application-instance.decommission` |  |

**Terminal states:** DECOMMISSIONED

**DSL verbs targeting this slot (6):**

| Verb FQN | state_effect | description |
|---|---|---|
| `application-instance.activate` | transition | Activate a provisioned (or offline / maintenance) instance — transition to ACTIVE |
| `application-instance.bring-online` | transition | Bring an OFFLINE instance back to ACTIVE |
| `application-instance.decommission` | transition | Decommission an instance — terminal transition; cascades bindings to RETIRED |
| `application-instance.enter-maintenance` | transition | Enter maintenance window — pause traffic without retiring bindings |
| `application-instance.exit-maintenance` | transition | Exit maintenance window — return instance to ACTIVE |
| `application-instance.take-offline` | transition | Take instance OFFLINE (operational hold — incident, security freeze, capacity action) |


#### `capability_binding` · capability_binding_lifecycle

_owner: ops+platform · scope: per_capability_binding · lifetime: long_lived · entity: "ob-poc".capability_bindings · col: binding_status_


**States**

| State | Entry? | Description | Green-switch |
|---|---|---|---|
| `DRAFT` | ✓ | Binding declared; no traffic; pre-pilot. | — |
| `PILOT` |  | Limited load; canary cohort; pilot validation. | — |
| `LIVE` |  | Full production traffic. Downstream service_consumption can rely on it. | — |
| `DEPRECATED` |  | Marked for retirement; no new consumers; existing traffic drains. | — |
| `RETIRED` |  | Terminal — no traffic; binding closed. | — |

**Transitions**

| From | → To | Via | Precondition |
|---|---|---|---|
| DRAFT | **PILOT** | `capability-binding.start-pilot` |  |
| PILOT | **LIVE** | `capability-binding.promote-live` |  |
| PILOT | **DRAFT** | `capability-binding.abort-pilot` |  |
| LIVE | **DEPRECATED** | `capability-binding.deprecate` |  |
| DEPRECATED | **RETIRED** | `capability-binding.retire` |  |

**Terminal states:** RETIRED

**DSL verbs targeting this slot (5):**

| Verb FQN | state_effect | description |
|---|---|---|
| `capability-binding.abort-pilot` | transition | Abort pilot — return binding to DRAFT (pilot did not validate) |
| `capability-binding.deprecate` | transition | Deprecate a LIVE binding — no new consumers; existing traffic drains |
| `capability-binding.promote-live` | transition | Promote pilot binding to LIVE — full production traffic |
| `capability-binding.retire` | transition | Retire a deprecated binding — terminal |
| `capability-binding.start-pilot` | transition | Start pilot — limited load, canary cohort |


### Cross-slot constraints (2 — intra-DAG)

- **binding_live_requires_instance_active** (error): A capability_binding can only be LIVE if its parent
application_instance is ACTIVE. Bindings on
MAINTENANCE_WINDOW / DEGRADED / OFFLINE / DECOMMISSIONED instances
cannot be LIVE.
  - rule: `capability_bindings.binding_status = 'LIVE'` ...
- **binding_pilot_requires_instance_serving** (error): A capability_binding cannot be in PILOT against a DECOMMISSIONED
or OFFLINE instance.
  - rule: `capability_bindings.binding_status = 'PILOT'` ...

---

## 4. Open questions for peer review

1. **Predicate granularity.** The 13 worked-example green_when predicates use compound facts (e.g., "every evidence_requirement exists AND every evidence_requirement.state = VERIFIED"). Should we break out per-evidence-type sub-states (e.g., separate ID_OBTAINED → ID_VERIFIED nodes), or keep compound predicates at the parent state? Compound is simpler; granular is more diagnostic.

2. **Predicate parsing.** Currently free-text English. Should v1.5 introduce structured fields:
   - `required_entities:` (list of existence assertions)
   - `valid_states:` (entity → state-set mapping)
   - so the runtime can mechanically evaluate without regex/NLP?

3. **Tollgate-driven vs verb-driven.** Of the ~280 currently verb-driven transitions across the 8 DAGs, the bulk are likely tollgate-driven under P19. Do we:
   - **(a)** Sweep all 280 and add green_when on destination states?
   - **(b)** Only add green_when on terminal/critical-junction states; leave intermediate transitions verb-driven?
   - **(c)** Treat green_when as opt-in per-state — author writes one when the test is meaningful?

4. **Ambiguous verbs.** Some verb names suggest tollgate semantics (`approve`, `complete`, `validate`, `advance`) — those are the 10 worked examples. But many others are ambiguous (`activate`, `decide`, `verify`, `confirm`). For each, is the action (a) flipping a single fact, or (b) testing aggregate facts? The audit harness flags 161 ambiguous transitions; needs Adam's per-case judgment.

5. **Cross-workspace cascade.** When a child green_when fact updates (e.g., a UBO becomes APPROVED), should the parent (kyc_case) auto-recompute its state? The current model implies yes (the runtime walks the green_when tree on substrate change), but the cascade trigger isn't formalized. Worth a v1.5 explicit `cascade_on:` field?

6. **CBU as composite.** CBU has many slots (cbu, manco, share-class, holding, investor, investor_kyc, cbu_corporate_action, cbu_disposition, cbu_evidence, cbu_specialist_role, service_consumption). Should each slot's green_when be expressed at the slot level, or should there be a *CBU-level* aggregate green_when that says "all required slots green"? Today it's slot-level only.

---

## 5. Open structural concerns

- **TODO §4.2 reconciliation surfaced 0 actual drift** — substrate audit was based on a stale snapshot. All 9 clusters claimed to be drifting are already DAG-canonical in the live YAML. Closure logged as D-015.
- **Phase 4 §4.3 partial** — 5 KYC verbs added; broader green_when sweep across ~280 verb-driven transitions deferred.
- **Carrier completeness** — 8 carrier migrations applied (S-1, S-3, S-7, S-9, S-12, S-25, IN_CLEARANCE substates). 6 carriers (S-4, S-6, S-10, S-11, S-13, S-14) deferred per substrate audit §7.3.
- **CI gates** — `reconcile carrier-completeness` and `reconcile drift-check` not yet implemented. Placeholder in v1.4 spec; implementation pending.
- **Test coverage** — 36 of 61 state machines have zero tests.

---

_End of snapshot._


## DAG-by-DAG quick stats

| DAG | Workspace | Slots | States (total) | Transitions | green_when set | Cross-WS gates |
|---|---|---|---|---|---|---|
| `cbu_dag.yaml` | cbu | 23 | 68 | 74 | **2** | 4 |
| `deal_dag.yaml` | deal | 22 | 54 | 52 | **1** | 2 |
| `product_service_taxonomy_dag.yaml` | product_maintenance | 6 | 10 | 9 | **0** | 0 |
| `instrument_matrix_dag.yaml` | instrument_matrix | 22 | 55 | 66 | **2** | 1 |
| `booking_principal_dag.yaml` | booking_principal | 2 | 7 | 8 | **1** | 0 |
| `onboarding_request_dag.yaml` | onboarding_request | 5 | 0 | 0 | **0** | 2 |
| `lifecycle_resources_dag.yaml` | lifecycle_resources | 4 | 11 | 13 | **0** | 0 |
| `book_setup_dag.yaml` | book_setup | 10 | 8 | 7 | **0** | 2 |
