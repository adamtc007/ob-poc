# DAG Business-Semantic Review — Evidence Base (2026-04-29)

> **Scope:** Structured evidence base for business-correctness review of declared entity state machines across the twelve DAG taxonomies in `rust/config/sem_os_seeds/dag_taxonomies/`.
> **Output consumer:** Adam (BNY custody-banking domain expert) — uses this document to make business verdicts on each state machine.
> **What this document is NOT:** business verdicts, recommended changes, severity classifications, schema remediation proposals, or relitigation of v1.2 / substrate-audit-2026-04-29.
> **Reference context (NOT under review):** `docs/todo/catalogue-platform-refinement-v1_2.md`, `docs/governance/substrate-audit-2026-04-29.md`.
> **Output discipline:** Tiered per resumption guidance — Section 1 fields A–E (states, transitions, cross-workspace constraints, carrier evidence) are populated for every state machine; fields F–J (verb impl summaries, code-path write sites, migration history, tests, internal docs) are presented as evidence pointers citing substrate-audit IDs (S-1..S-28) and migration filenames rather than inlined deep extracts. Section 3 (DAG-vs-code consistency) is partially populated from substrate-audit findings; full code-grep pass deferred.

---

## Section 0 — Inventory

### 0.1 Workspace catalogue

| # | Workspace | DAG file | State machines | States (sum) | Transitions (sum) | XW constraints | Dual lifecycle | Carrier presence |
|---|---|---|---|---|---|---|---|---|
| 1 | Catalogue | `catalogue_dag.yaml` | 1 | 5 | 5 | 0 | no | full |
| 2 | SessionBootstrap | `session_bootstrap_dag.yaml` | 0 (overall_lifecycle 2-phase only) | 0 | 0 | 0 | no | n/a — transient |
| 3 | OnboardingRequest | `onboarding_request_dag.yaml` | 0 (overall_lifecycle 7-phase only; all slots reconcile-existing) | 0 | 0 | 2 | no | reconcile to Deal-owned `deal_onboarding_requests` |
| 4 | ProductMaintenance | `product_service_taxonomy_dag.yaml` | 2 | 10 | 9 | 0 | no | full |
| 5 | BookingPrincipal | `booking_principal_dag.yaml` | 1 | 7 | 9 | 0 (gates declared in deal_dag) | no | full (mig 2026-04-29) |
| 6 | LifecycleResources | `lifecycle_resources_dag.yaml` | 2 | 11 | 13 | 0 (R4 deferred) | no | full (mig 2026-04-27) |
| 7 | KYC | `kyc_dag.yaml` | 12 | 87 | 80+ | 0 | no | full all 11 carriers |
| 8 | InstrumentMatrix | `instrument_matrix_dag.yaml` | 12 | 60+ | 50+ | 1 | no | partial — 4 carriers missing or boolean-collapsed |
| 9 | CBU | `cbu_dag.yaml` | 14 (13 stateful + 1 dual) | 70+ | 65+ | 4 | yes (cbu primary + operational) | partial — 5 carriers DAG-first |
| 10 | Deal | `deal_dag.yaml` | 10 (9 stateful + 1 dual) | 67 | 60+ | 2 | yes (deal commercial + operational) | partial — 4 CHECK gaps / column missing |
| 11 | BookSetup | `book_setup_dag.yaml` | 1 | 8 | 7 | 2 | no | DAG-first — `client_books` table missing |
| 12 | SemOsMaintenance | `semos_maintenance_dag.yaml` | 6 (5 stateful + 1 dual) | 33 | 30+ | 0 | yes (attribute_def external + internal) | full all 5 governance carriers |

**Estate totals:** 12 DAGs · 61 state machines · ~358 declared states · ~330 declared transitions · 11 cross-workspace constraints · 4 dual-lifecycle declarations · 9 of 12 carrier sets fully materialised in schema.

**Note on the "twelve workspaces" framing.** v1.2 §4.1 enumerates eleven workspaces; the Catalogue workspace was added in Tranche 3 (catalogue_dag.yaml) and is the twelfth. This document treats all twelve as in-scope per the substrate audit's §0.2 totals.

### 0.2 State-machine inventory (flat catalogue)

Sortable. Index used as Section 1 sub-section identifier.

| Index | Workspace | DAG file | State machine name | State count | Transition count | XW constraints | Dual? | Carrier table | Carrier state column | Verbs in DAG transitions |
|---|---|---|---|---|---|---|---|---|---|---|
| M-001 | catalogue | catalogue_dag.yaml | catalogue_proposal_lifecycle | 5 | 5 | 0 | no | `"ob-poc".catalogue_proposals` | `status` | 5 |
| M-002 | product_maintenance | product_service_taxonomy_dag.yaml | service_lifecycle | 5 | 5 | 0 | no | `"ob-poc".services` | `lifecycle_status` | 4 (1 backend) |
| M-003 | product_maintenance | product_service_taxonomy_dag.yaml | service_version_lifecycle | 5 | 4 | 0 | no | `"ob-poc".service_versions` | `lifecycle_status` | 3 (1 backend) |
| M-004 | booking_principal | booking_principal_dag.yaml | booking_principal_clearance_lifecycle | 7 | 9 | 0 (gates declared deal-side) | no | `"ob-poc".booking_principal_clearances` | `clearance_status` | 8 |
| M-005 | lifecycle_resources | lifecycle_resources_dag.yaml | application_instance_lifecycle | 6 | 9 | 0 | no | `"ob-poc".application_instances` | `lifecycle_status` | 7 (2 backend) |
| M-006 | lifecycle_resources | lifecycle_resources_dag.yaml | capability_binding_lifecycle | 5 | 5 | 0 | no | `"ob-poc".capability_bindings` | `binding_status` | 5 |
| M-007 | kyc | kyc_dag.yaml | kyc_case_lifecycle | 11 | 12 | 0 | no | `"ob-poc".cases` | `status` | 7 |
| M-008 | kyc | kyc_dag.yaml | entity_kyc_lifecycle | 8 | 8 | 0 | no | (derived from `entity_workstreams` + `kyc_ubo_registry`) | (derived) | 6 (2 backend) |
| M-009 | kyc | kyc_dag.yaml | entity_workstream_lifecycle | 10 | 10 | 0 | no | `"ob-poc".entity_workstreams` | `status` | 7 (1 backend) |
| M-010 | kyc | kyc_dag.yaml | screening_lifecycle | 8 | 9 | 0 | no | `"ob-poc".screenings` | `status` | 6 (3 backend) |
| M-011 | kyc | kyc_dag.yaml | ubo_evidence_lifecycle | 4 | 4 | 0 | no | `"ob-poc".ubo_evidence` | `verification_status` | 3 (1 backend) |
| M-012 | kyc | kyc_dag.yaml | kyc_ubo_registry_lifecycle | 9 | 9 | 0 | no | `"ob-poc".kyc_ubo_registry` | `status` | 5 (4 backend) |
| M-013 | kyc | kyc_dag.yaml | kyc_ubo_evidence_lifecycle | 7 | 6 | 0 | no | `"ob-poc".kyc_ubo_evidence` | `status` | 5 (1 backend) |
| M-014 | kyc | kyc_dag.yaml | red_flag_lifecycle | 6 | 6 | 0 | no | `"ob-poc".red_flags` | `status` | 5 (1 backend) |
| M-015 | kyc | kyc_dag.yaml | doc_request_lifecycle | 9 | 8 | 0 | no | `"ob-poc".doc_requests` | `status` | 6 (1 backend) |
| M-016 | kyc | kyc_dag.yaml | outreach_request_lifecycle | 7 | 6 | 0 | no | `"ob-poc".outreach_requests` | `status` | 5 (1 backend) |
| M-017 | kyc | kyc_dag.yaml | kyc_decision_lifecycle | 4 | 3 | 0 | no | `"ob-poc".kyc_decisions` | `status` | 3 |
| M-018 | kyc | kyc_dag.yaml | kyc_service_agreement_lifecycle | 4 | 4 | 0 | no | `"ob-poc".kyc_service_agreements` | `status` | 2 |
| M-019 | instrument_matrix | instrument_matrix_dag.yaml | group_discovery_lifecycle | 5 | 6 | 0 | no | `"ob-poc".client_group` | `discovery_status` | 0 (all backend) |
| M-020 | instrument_matrix | instrument_matrix_dag.yaml | trading_profile_template_lifecycle | 2 | 1 | 0 | no | `"ob-poc".cbu_trading_profiles` (cbu_id IS NULL) | `status` | 1 |
| M-021 | instrument_matrix | instrument_matrix_dag.yaml | settlement_pattern_lifecycle | 7 | 11 | 0 | no | `"ob-poc".cbu_settlement_chains` | (boolean `is_active` only — DAG-first state column) | 8 |
| M-022 | instrument_matrix | instrument_matrix_dag.yaml | trade_gateway_lifecycle | 5 | 6 | 0 | no | (hybrid JSON document + `is_active`) | (document body) | 6 |
| M-023 | instrument_matrix | instrument_matrix_dag.yaml | trading_profile_lifecycle | 9 | 12 | 1 | no | `"ob-poc".cbu_trading_profiles` (cbu_id IS NOT NULL) | `status` | 9 |
| M-024 | instrument_matrix | instrument_matrix_dag.yaml | trading_activity_lifecycle | 4 | 5 | 0 | no | `"ob-poc".cbu_trading_activity` (DAG-first; **table missing**) | `activity_state` | 0 (all backend) |
| M-025 | instrument_matrix | instrument_matrix_dag.yaml | service_resource_lifecycle | 4 | 5 | 0 | no | `"ob-poc".service_resource_types` | (derived from `is_active` + `provisioning_strategy`) | 4 |
| M-026 | instrument_matrix | instrument_matrix_dag.yaml | service_intent_lifecycle | 3 | 4 | 0 | no | `"ob-poc".service_intents` | `status` | 3 |
| M-027 | instrument_matrix | instrument_matrix_dag.yaml | delivery_lifecycle | 5 | 6 | 0 | no | `"ob-poc".service_delivery_map` | `delivery_status` | 4 |
| M-028 | instrument_matrix | instrument_matrix_dag.yaml | reconciliation_config_lifecycle | 4 | 5 | 0 | no | (DAG-first; **table missing**) | (new) | 4 |
| M-029 | instrument_matrix | instrument_matrix_dag.yaml | corporate_action_event_lifecycle | 3 | 2 | 0 | no | (DAG-first; **table missing**) | (new) | 1 (1 automatic) |
| M-030 | instrument_matrix | instrument_matrix_dag.yaml | collateral_management_lifecycle | 4 | 5 | 0 | no | (DAG-first; **table missing**) | (new) | 4 |
| M-031 | cbu | cbu_dag.yaml | cbu_discovery_lifecycle | 5 | 6 | 1 | yes (paired with M-032) | `"ob-poc".cbus` | `status` | 5 |
| M-032 | cbu | cbu_dag.yaml | cbu_operational_lifecycle (dual on slot `cbu`) | 8 | 9 | 0 | (dual half) | `"ob-poc".cbus` | `operational_status` (**column missing**, S-3) | 6 (3 backend) |
| M-033 | cbu | cbu_dag.yaml | entity_proper_person_lifecycle | 3 | 2 | 0 | no | `"ob-poc".entity_proper_persons` | `person_state` | 2 |
| M-034 | cbu | cbu_dag.yaml | entity_limited_company_ubo_lifecycle | 5 | 6 | 0 | no | `"ob-poc".entity_limited_companies` | `ubo_status` | 1 (5 backend) |
| M-035 | cbu | cbu_dag.yaml | cbu_evidence_lifecycle | 4 | 4 | 0 | no | `"ob-poc".cbu_evidence` | `verification_status` | 3 (1 backend) |
| M-036 | cbu | cbu_dag.yaml | investor_lifecycle | 7 | 6 | 0 | no | `"ob-poc".investors` | (derived from events + status) | 5 |
| M-037 | cbu | cbu_dag.yaml | investor_kyc_lifecycle | 6 | 6 | 0 | no | `"ob-poc".investors` | `kyc_status` | 4 (2 backend) |
| M-038 | cbu | cbu_dag.yaml | holding_lifecycle | 7 | 11 | 0 | no | `"ob-poc".holdings` | `holding_status` | 6 (2 backend) |
| M-039 | cbu | cbu_dag.yaml | service_consumption_lifecycle | 6 | 6 | 2 | no | `"ob-poc".cbu_service_consumption` (DAG-first; **table missing**, S-1 BLOCKING) | `status` | 6 |
| M-040 | cbu | cbu_dag.yaml | cbu_corporate_action_lifecycle | 7 | 6 | 0 | no | `"ob-poc".cbu_corporate_action_events` (DAG-first; **table missing**, S-4) | `ca_status` | 5 (1 backend) |
| M-041 | cbu | cbu_dag.yaml | cbu_disposition_lifecycle | 4 | 5 | 0 | no | `"ob-poc".cbus` (uses `deleted_at` + new `disposition_status` column **missing**, S-11) | `disposition_status` | 5 |
| M-042 | cbu | cbu_dag.yaml | client_group_entity_review_lifecycle | 4 | 4 | 0 | no | `"ob-poc".client_group_entity` | `review_status` | 0 (all backend) |
| M-043 | cbu | cbu_dag.yaml | share_class_lifecycle | 6 | 7 | 0 | no | `"ob-poc".share_classes` | `lifecycle_status` (**column missing**, S-10) | 6 |
| M-044 | cbu | cbu_dag.yaml | manco_lifecycle | 6 | 8 | 0 | no | `"ob-poc".manco_regulatory_status` | `regulatory_status` | 7 |
| M-045 | deal | deal_dag.yaml | deal_commercial_lifecycle | 10 | 11 | 1 | yes (paired with M-046) | `"ob-poc".deals` | `deal_status` | 8 (CHECK lacks BAC_APPROVAL/LOST/REJECTED/WITHDRAWN, S-2 BLOCKING + S-8) |
| M-046 | deal | deal_dag.yaml | deal_operational_lifecycle (dual on slot `deal`) | 5 | 5 | 0 | (dual half) | `"ob-poc".deals` | `operational_status` (**column missing**, S-9) | 5 |
| M-047 | deal | deal_dag.yaml | deal_product_lifecycle | 5 | 5 | 0 | no | `"ob-poc".deal_products` | `product_status` | 5 |
| M-048 | deal | deal_dag.yaml | deal_rate_card_lifecycle | 8 | 13 | 0 | no | `"ob-poc".deal_rate_cards` | `status` | 7 (1 backend) |
| M-049 | deal | deal_dag.yaml | deal_onboarding_request_lifecycle | 5 | 6 | 0 | no | `"ob-poc".deal_onboarding_requests` | `request_status` | 1 (used 6×) |
| M-050 | deal | deal_dag.yaml | deal_document_lifecycle | 6 | 6 | 0 | no | `"ob-poc".deal_documents` | `document_status` | 1 (used 6×) |
| M-051 | deal | deal_dag.yaml | deal_ubo_assessment_lifecycle | 5 | 5 | 0 | no | `"ob-poc".deal_ubo_assessments` | `assessment_status` | 1 (used 5×) |
| M-052 | deal | deal_dag.yaml | deal_sla_lifecycle | 6 | 6 | 0 | no | `"ob-poc".deal_slas` | `sla_status` (**column missing**, S-7) | 3 (3 backend) |
| M-053 | deal | deal_dag.yaml | billing_profile_lifecycle | 4 | 5 | 0 | no | `"ob-poc".fee_billing_profiles` | `status` | 3 |
| M-054 | deal | deal_dag.yaml | billing_period_lifecycle | 6 | 6 | 0 | no | `"ob-poc".fee_billing_periods` | `calc_status` | 5 |
| M-055 | book_setup | book_setup_dag.yaml | book_lifecycle | 8 | 7 | 2 | no | `"ob-poc".client_books` (DAG-first; **table missing**, S-5) | `status` | 5 |
| M-056 | semos_maintenance | semos_maintenance_dag.yaml | changeset_lifecycle | 6 | 5 | 0 | no | `"sem_reg".changesets` | `status` | 5 |
| M-057 | semos_maintenance | semos_maintenance_dag.yaml | attribute_def_lifecycle (external) | 5 | 5 | 0 | yes (paired with M-058) | `"sem_reg".attribute_defs` | `lifecycle_status` | 4 (1 backend) |
| M-058 | semos_maintenance | semos_maintenance_dag.yaml | attribute_def_internal_lifecycle (dual on slot `attribute_def`) | 3 | 3 | 0 | (dual half) | `"sem_reg".attribute_defs` | `lifecycle_status` | 1 (2 backend/system) |
| M-059 | semos_maintenance | semos_maintenance_dag.yaml | derivation_spec_lifecycle | 3 | 3 | 0 | no | `"sem_reg".derivation_specs` | `lifecycle_status` | 2 (1 backend) |
| M-060 | semos_maintenance | semos_maintenance_dag.yaml | service_resource_def_lifecycle | 4 | 5 | 0 | no | `"sem_reg".service_resource_defs` | `lifecycle_status` | 3 (used 5×) |
| M-061 | semos_maintenance | semos_maintenance_dag.yaml | phrase_authoring_lifecycle | 8 | 9 | 0 | no | `"sem_reg".phrase_authoring` | `authoring_status` | 9 |

**Counting note.** `cbu` slot and `deal` slot each declare both a primary state machine and a dual_lifecycle; `attribute_def` slot does the same. These are presented as paired entries (M-031/M-032, M-045/M-046, M-057/M-058) rather than collapsed, because the brief requires per-state-machine evidence and the two halves carry distinct states/transitions/owners.

### 0.3 Workspaces with overall_lifecycle but no per-slot state machines

Two workspaces declare an `overall_lifecycle:` derived aggregate without authoring any per-slot state machines. They appear in this catalogue for completeness; they have no Section 1 subsection (no state machine to inventory).

| Workspace | Overall lifecycle | Phases | Carrier |
|---|---|---|---|
| SessionBootstrap | `session_bootstrap_overall_lifecycle` | 2 (unresolved → resolved) | transient — `ReplSessionV2.entity_scope` |
| OnboardingRequest | `onboarding_request_overall_lifecycle` | 7 (scoping → validating → submitted → in_progress → blocked → completed → cancelled) | reconcile to Deal-owned `deal_onboarding_requests` (state machine M-049) |

---

## Section 1 — Per-state-machine structured presentation

> Section 1 sub-sections follow the M-NNN.A–J fixed-field schema declared by the brief. Fields A–E are populated from the DAG YAMLs and the substrate audit (substrate evidence already validates carrier alignment exhaustively in §0.3 and §3 of that document). Fields F–J are presented as **evidence pointers** rather than inlined deep extracts: the verb FQNs declared in `transitions[].via` (field F) and the substrate-audit ID, migration filename, and parent-doc references (fields G–J) point the reviewer to where to look. A full code-grep / migration-history / test inventory pass per state machine is anticipated as a follow-up if any field-A–E finding raises a question that warrants the deeper traversal.

### 1.X.A–E format

Each subsection presents:

- **A — Identity** (workspace, DAG file:line range, state machine name as declared, one-line description verbatim).
- **B — Declared states** (table: state, type, description, carrier value mapping).
- **C — Declared transitions** (table: from, to, guard / precondition, cross-workspace constraint reference, DSL verb FQN).
- **D — Cross-workspace constraints involving this state machine** (source / target).
- **E — Carrier evidence** (carrier table, state column, schema CHECK enumeration, set comparison DAG ↔ CHECK, substrate-audit ID where flagged).

### 1.X.F–J evidence pointers

A condensed block per subsection:

- **F — DSL verb FQNs** (extracted from `transitions[].via` plus operator vocabulary in DAG header).
- **G — Code paths** (substrate-audit reference, or "see §4 of substrate audit" for verbs declared but with no extant carrier).
- **H — Migration history** (migration filename if mentioned in DAG; "(none in DAG)" otherwise).
- **I — Test fixtures** (none gathered in this pass — pointer only).
- **J — Internal documentation** (DAG header docstring + parent docs cited in `PARENT DOCS:` block of the DAG file).

---

### Workspace batch 1.1 — Catalogue

#### M-001 — catalogue.proposal

**A — Identity.**
- Workspace: catalogue
- DAG file: `catalogue_dag.yaml` (lines 31–75)
- State machine name: `catalogue_proposal_lifecycle`
- Description (verbatim): "Catalogue authorship proposal lifecycle. 5 states: DRAFT (entry) → STAGED → COMMITTED (terminal) | ROLLED_BACK | REJECTED. STAGED is the validator-clean checkpoint. COMMITTED is the architectural drift gate — it writes the committed declaration to `catalogue_committed_verbs` (Tranche 3 Phase 3.F Stage 4 makes this the catalogue's source of truth)."
- Owner: catalogue-author
- Expected lifetime: short_lived

**B — Declared states.**
| State | Type | Description | Carrier value |
|---|---|---|---|
| DRAFT | entry | "Proposal created; validator may have errors; author iterating." | DRAFT |
| STAGED | intermediate | "Validator clean; awaiting commit by a different catalogue-author (two-eye rule)." | STAGED |
| COMMITTED | terminal | "Promoted to authoritative catalogue; triggers seed reload." | COMMITTED |
| ROLLED_BACK | terminal | "Staged proposal rolled back to DRAFT (or terminal if explicitly archived)." | ROLLED_BACK |
| REJECTED | intermediate | "Proposal rejected by reviewer; reopen path available." | REJECTED |

**C — Declared transitions.**
| From | To | Guard / precondition | XW | Verb |
|---|---|---|---|---|
| DRAFT | STAGED | (auto on validator-clean, out-of-band) | — | `catalogue.stage-proposal` |
| STAGED | COMMITTED | (two-eye) | — | `catalogue.commit-verb-declaration` |
| STAGED | ROLLED_BACK | — | — | `catalogue.rollback-verb-declaration` |
| DRAFT | REJECTED | — | — | `catalogue.reject-proposal` |
| REJECTED | DRAFT | — | — | `catalogue.reopen-proposal` |

**D — Cross-workspace constraints involving this state machine.** none.

**E — Carrier evidence.**
- Carrier table: `"ob-poc".catalogue_proposals`
- State column: `status`
- CHECK enumeration: `DRAFT / STAGED / COMMITTED / ROLLED_BACK / REJECTED` (per substrate audit §0.3 row Catalogue: "✅ all 5 states").
- Set comparison: DAG ∩ CHECK = {DRAFT, STAGED, COMMITTED, ROLLED_BACK, REJECTED}; DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.

**F — DSL verbs (declared in transitions).** `catalogue.stage-proposal`, `catalogue.commit-verb-declaration`, `catalogue.rollback-verb-declaration`, `catalogue.reject-proposal`, `catalogue.reopen-proposal`.

**G — Code paths.** Substrate audit §0.5 sample lists 4 catalogue verbs in domain. Code-path grep deferred (per resumption note).

**H — Migration history.** Schema reference in v1.2 §0.3 lists `catalogue_proposals` and `catalogue_committed_verbs` as Tranche 3 additions (substrate audit §0.1 row Catalogue: 2 tables).

**I — Test fixtures.** Not gathered.

**J — Internal documentation.** DAG header docstring; references `docs/governance/tranche-3-design-2026-04-26.md §2`. Substrate audit S-27 flags Catalogue as not listed in v1.2 §4.1 inventory (a documentation gap, not a state-machine concern).

---

### Workspace batch 1.2 — ProductMaintenance

#### M-002 — product_maintenance.service

**A — Identity.**
- Workspace: product_maintenance
- DAG file: `product_service_taxonomy_dag.yaml` (lines 121–166)
- State machine name: `service_lifecycle`
- Description (verbatim): "Service-definition lifecycle. 5 states modelled after attribute_def_lifecycle (ungoverned → draft → active → deprecated → retired). Catalog services start ungoverned (discovered but not yet defined); transition to draft via service.define; reach active when included in a published changeset; deprecated/retired follow attribute_def semantics."
- Owner: stewards
- Expected lifetime: long_lived
- `suspended_state_exempt: true`

**B — Declared states.**
| State | Type | Description | Carrier value |
|---|---|---|---|
| ungoverned | entry | "Pre-governance; service exists but is not formally defined." | ungoverned |
| draft | intermediate | "Service being authored within a changeset." | draft |
| active | intermediate | "Published; consumers reference this service." | active |
| deprecated | intermediate | "Replaced by newer version; migrate consumers." | deprecated |
| retired | terminal | "Terminal; no longer in use." | retired |

**C — Declared transitions.**
| From | To | Guard / precondition | XW | Verb |
|---|---|---|---|---|
| ungoverned | draft | — | — | `service.define` |
| draft | active | — | — | (backend: `changeset.publish` includes this service) |
| active | draft | — | — | `service.propose-revision` |
| active | deprecated | — | — | `service.deprecate` |
| deprecated | retired | — | — | `service.retire` |

**D — Cross-workspace constraints involving this state machine.** Source side of CBU's `service_consumption_requires_active_service` (CBU `service_consumption.proposed → provisioned` requires `services.lifecycle_status = active` for the referenced service). Declared in `cbu_dag.yaml` cross_workspace_constraints (R2, 2026-04-26).

**E — Carrier evidence.**
- Carrier table: `"ob-poc".services`
- State column: `lifecycle_status`
- Schema status: substrate audit §0.3 row ProductMaintenance: "✅" with `lifecycle_status` on both `services` and `service_versions` (R2, 2026-04-26).
- Set comparison: DAG ∩ CHECK = all 5 states (per substrate audit "all states materialised"). DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.

**F — DSL verbs.** `service.define`, `service.propose-revision`, `service.deprecate`, `service.retire`. Backend transition (`draft → active`) effected by `changeset.publish` ingesting the service into a published changeset.

**G — Code paths.** Substrate audit §0.5 sample lists `service.define / .deprecate / .retire / service-version.publish / .retire`. Code-path grep deferred.

**H — Migration history.** R2 2026-04-26 added `lifecycle_status` to `services` (per DAG header comment "R2 (2026-04-26): service slot promoted to stateful"). Specific migration file not cited in DAG header.

**I — Test fixtures.** Not gathered.

**J — Internal documentation.** DAG header references `semos_maintenance_dag.yaml` (the governed source for service-definition authoring) and `docs/todo/catalogue-platform-refinement-v1_3.md`.

#### M-003 — product_maintenance.service_version

**A — Identity.**
- Workspace: product_maintenance
- DAG file: `product_service_taxonomy_dag.yaml` (lines 172–219)
- State machine name: `service_version_lifecycle`
- Description (verbatim): "Service-version lifecycle. 5 states (drafted → reviewed → published → superseded → retired). Each version of a service definition flows through review and publication independently of the parent service's overall lifecycle state."
- Owner: stewards
- Expected lifetime: long_lived
- `parent_slot:` `product_maintenance.service` via `service_versions.service_id → services.id`
- `suspended_state_exempt: true`

**B — Declared states.**
| State | Type | Description | Carrier value |
|---|---|---|---|
| drafted | entry | "Version drafted; under authoring." | drafted |
| reviewed | intermediate | "Submitted for and passed review." | reviewed |
| published | intermediate | "Active version; consumers may reference." | published |
| superseded | intermediate | "Newer version published; this one no longer current." | superseded |
| retired | terminal | "Terminal; removed from circulation." | retired |

**C — Declared transitions.**
| From | To | Guard | XW | Verb |
|---|---|---|---|---|
| drafted | reviewed | — | — | `service-version.submit-for-review` |
| reviewed | published | — | — | `service-version.publish` |
| published | superseded | — | — | (backend: new published version of same service) |
| published | retired | — | — | `service-version.retire` |
| superseded | retired | — | — | `service-version.retire` |

**D — Cross-workspace constraints involving this state machine.** none.

**E — Carrier evidence.**
- Carrier table: `"ob-poc".service_versions`
- State column: `lifecycle_status`
- Schema status: substrate audit §0.3 ProductMaintenance row: "✅" both states materialised.
- Set comparison: DAG ∩ CHECK = all 5; DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.

**F — DSL verbs.** `service-version.submit-for-review`, `service-version.publish`, `service-version.retire`. Backend transition (`published → superseded`) is implicit on new-version publication.

**G — Code paths.** Substrate audit §0.5 sample lists `service-version.publish / .retire`. Code-path grep deferred.

**H — Migration history.** R2 2026-04-26 (per DAG header).

**I — Test fixtures.** Not gathered.

**J — Internal documentation.** DAG header.

---

### Workspace batch 1.3 — BookingPrincipal

#### M-004 — booking_principal.clearance

**A — Identity.**
- Workspace: booking_principal
- DAG file: `booking_principal_dag.yaml` (lines 131–188)
- State machine name: `booking_principal_clearance_lifecycle`
- Description (verbatim): "Per-(deal, booking_principal) clearance lifecycle. 7 states: PENDING (entry) → SCREENING → APPROVED / REJECTED → ACTIVE → SUSPENDED → REVOKED (terminal). REJECTED can be reopened back to PENDING for retry. APPROVED/ACTIVE are required to gate the deal KYC_CLEARANCE → CONTRACTED transition (compound BAC+KYC+BP gate declared in deal_dag.yaml cross_workspace_constraints)."
- Owner: ops+credit
- Expected lifetime: long_lived
- Scope: per_deal_principal

**B — Declared states.**
| State | Type | Description | Carrier value |
|---|---|---|---|
| PENDING | entry | "Clearance request created; screening not yet started." | PENDING |
| SCREENING | intermediate | "Screening in progress (sanctions, credit, regulatory)." | SCREENING |
| APPROVED | intermediate | "Screening passed; ready to activate. Gate-eligible." | APPROVED |
| REJECTED | intermediate | "Screening failed. Reopenable to PENDING for retry." | REJECTED |
| ACTIVE | intermediate | "Clearance live; principal cleared to book the deal." | ACTIVE |
| SUSPENDED | intermediate | "Temporarily held (regulatory action, dispute, audit)." | SUSPENDED |
| REVOKED | terminal | "Terminal — clearance withdrawn permanently." | REVOKED |

**C — Declared transitions.**
| From | To | Guard | XW | Verb |
|---|---|---|---|---|
| PENDING | SCREENING | — | — | `booking-principal-clearance.start-screening` |
| SCREENING | APPROVED | — | — | `booking-principal-clearance.approve` |
| SCREENING | REJECTED | — | — | `booking-principal-clearance.reject` |
| REJECTED | PENDING | — | — | `booking-principal-clearance.reopen` |
| APPROVED | ACTIVE | — | — | `booking-principal-clearance.activate` |
| ACTIVE | SUSPENDED | — | — | `booking-principal-clearance.suspend` |
| SUSPENDED | ACTIVE | — | — | `booking-principal-clearance.reinstate` |
| {APPROVED, ACTIVE, SUSPENDED} | REVOKED | — | — | `booking-principal-clearance.revoke` |

**D — Cross-workspace constraints involving this state machine.** Source side of `deal_contracted_requires_bp_approved` (declared in `deal_dag.yaml` lines 1023–1035): Deal `KYC_CLEARANCE → CONTRACTED` requires every booking_principal_clearance attached to this deal to be APPROVED or ACTIVE. Compound BAC+KYC+BP gate (R3.5 hoist).

**E — Carrier evidence.**
- Carrier table: `"ob-poc".booking_principal_clearances` (substrate audit §0.3 row BookingPrincipal: "✅", `clearance_status` varchar CHECK, R3.5 hoisted, migration `20260429_booking_principal_clearance.sql`).
- State column: `clearance_status`
- Schema status: per substrate audit "all 7 states" materialised.
- Set comparison: DAG ∩ CHECK = all 7; DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.
- UNIQUE constraint on `(booking_principal_id, deal_id, cbu_id)` (per DAG header §1.5 substrate audit).

**F — DSL verbs.** All 8 transitions use `booking-principal-clearance.*` verbs (start-screening, approve, reject, reopen, activate, suspend, reinstate, revoke). Substrate audit §1.5 lists 9 verbs in the family (the 8 transition verbs + `booking-principal-clearance.create`).

**G — Code paths.** Substrate audit §1.5 confirms verbs exist. Substrate audit S-22 flags ~15 verbs (including `booking-principal-clearance.create`) as declaring `target_workspace` without `target_slot` — a verb-declaration tightness issue. Code-path grep deferred.

**H — Migration history.** `rust/migrations/20260429_booking_principal_clearance.sql` (cited in DAG header).

**I — Test fixtures.** Not gathered.

**J — Internal documentation.** DAG header references `docs/todo/onboarding-dag-remediation-plan-2026-04-26.md (Slice R3 + R3.5)`, `docs/todo/onboarding-dag-deep-review-2026-04-26.md (Finding 3.7)`, `docs/todo/catalogue-platform-refinement-v1_3.md`.

---

### Workspace batch 1.4 — LifecycleResources

#### M-005 — lifecycle_resources.application_instance

**A — Identity.**
- Workspace: lifecycle_resources
- DAG file: `lifecycle_resources_dag.yaml` (lines 186–244)
- State machine name: `application_instance_lifecycle`
- Description (verbatim): "Per-instance lifecycle. PROVISIONED is the entry state (instance row created but not yet serving traffic). ACTIVE is the steady operating state. MAINTENANCE_WINDOW / DEGRADED / OFFLINE are operational variants. DECOMMISSIONED is terminal."
- Owner: ops+platform
- Expected lifetime: long_lived
- `suspended_state_exempt: true` (rationale: OFFLINE + MAINTENANCE_WINDOW + DEGRADED already cover pause/hold semantics).

**B — Declared states.**
| State | Type | Description | Carrier value |
|---|---|---|---|
| PROVISIONED | entry | "Instance row created; not yet serving traffic." | PROVISIONED |
| ACTIVE | intermediate | "Instance live and healthy; serving bindings." | ACTIVE |
| MAINTENANCE_WINDOW | intermediate | "Planned maintenance — traffic paused; bindings still bound." | MAINTENANCE_WINDOW |
| DEGRADED | intermediate | "Health-check signal: instance impaired but partially serving." | DEGRADED |
| OFFLINE | intermediate | "Operational hold — incident, security freeze, or capacity action." | OFFLINE |
| DECOMMISSIONED | terminal | "Terminal — instance retired; bindings cascaded to RETIRED." | DECOMMISSIONED |

**C — Declared transitions.**
| From | To | Guard | XW | Verb |
|---|---|---|---|---|
| PROVISIONED | ACTIVE | — | — | `application-instance.activate` |
| ACTIVE | MAINTENANCE_WINDOW | — | — | `application-instance.enter-maintenance` |
| MAINTENANCE_WINDOW | ACTIVE | — | — | `application-instance.exit-maintenance` |
| ACTIVE | DEGRADED | — | — | (backend: health-check signal) |
| DEGRADED | ACTIVE | — | — | (backend: health-check signal) |
| ACTIVE | OFFLINE | — | — | `application-instance.take-offline` |
| OFFLINE | ACTIVE | — | — | `application-instance.bring-online` |
| {ACTIVE, OFFLINE, MAINTENANCE_WINDOW, DEGRADED} | DECOMMISSIONED | — | — | `application-instance.decommission` |

**D — Cross-workspace constraints involving this state machine.** Indirect: this slot is referenced from CBU's `service_consumption_active_requires_live_binding` predicate (`application_instances.lifecycle_status = 'ACTIVE'` AND linked binding LIVE → CBU `service_consumption.provisioned → active` permitted). Declared in `cbu_dag.yaml` cross_workspace_constraints. The capability_binding side (M-006) carries the binding_state predicate; this slot supplies the parent ACTIVE check.

**E — Carrier evidence.**
- Carrier table: `"ob-poc".application_instances` (substrate audit §0.3: "✅" `lifecycle_status` varchar CHECK, migration 2026-04-27).
- State column: `lifecycle_status`
- Schema status: per substrate audit "all states" materialised.
- Set comparison: DAG ∩ CHECK = {PROVISIONED, ACTIVE, MAINTENANCE_WINDOW, DEGRADED, OFFLINE, DECOMMISSIONED}; DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.

**F — DSL verbs.** `application-instance.activate`, `application-instance.enter-maintenance`, `application-instance.exit-maintenance`, `application-instance.take-offline`, `application-instance.bring-online`, `application-instance.decommission`. Backend `(health-check signal)` for DEGRADED transitions (Layer 3, not DAG verb).

**G — Code paths.** Substrate audit §0.5 sample lists 6 `application-instance.*` verbs. S-21 flags PascalCase/snake_case workspace name normalisation (cosmetic). Code-path grep deferred.

**H — Migration history.** `rust/migrations/20260427_lifecycle_resources_workspace.sql` (cited in DAG header).

**I — Test fixtures.** Not gathered.

**J — Internal documentation.** DAG header references `docs/todo/onboarding-dag-remediation-plan-2026-04-26.md (Slice R1)`, `docs/todo/onboarding-dag-deep-review-2026-04-26.md`, `docs/todo/catalogue-platform-refinement-v1_3.md`.

#### M-006 — lifecycle_resources.capability_binding

**A — Identity.**
- Workspace: lifecycle_resources
- DAG file: `lifecycle_resources_dag.yaml` (lines 250–314)
- State machine name: `capability_binding_lifecycle`
- Description (verbatim): "Per-(application_instance, service) binding lifecycle. DRAFT is the entry state — the binding has been declared but no traffic flows. PILOT runs limited load; LIVE is full production. DEPRECATED signals an upcoming retirement; RETIRED is terminal."
- Owner: ops+platform
- Expected lifetime: long_lived
- `parent_slot:` `lifecycle_resources.application_instance` via `capability_bindings.application_instance_id → application_instances.id`
- `state_dependency`: cascade rule `parent.DECOMMISSIONED → child must be RETIRED` (`cascade_on_parent_transition: true`, `severity: error`).
- `suspended_state_exempt: true`.

**B — Declared states.**
| State | Type | Description | Carrier value |
|---|---|---|---|
| DRAFT | entry | "Binding declared; no traffic; pre-pilot." | DRAFT |
| PILOT | intermediate | "Limited load; canary cohort; pilot validation." | PILOT |
| LIVE | intermediate | "Full production traffic. Downstream service_consumption can rely on it." | LIVE |
| DEPRECATED | intermediate | "Marked for retirement; no new consumers; existing traffic drains." | DEPRECATED |
| RETIRED | terminal | "Terminal — no traffic; binding closed." | RETIRED |

**C — Declared transitions.**
| From | To | Guard | XW | Verb |
|---|---|---|---|---|
| DRAFT | PILOT | parent instance not DECOMMISSIONED/OFFLINE (cross-slot constraint `binding_pilot_requires_instance_serving`) | — | `capability-binding.start-pilot` |
| PILOT | LIVE | parent instance ACTIVE (cross-slot constraint `binding_live_requires_instance_active`) | — | `capability-binding.promote-live` |
| PILOT | DRAFT | — | — | `capability-binding.abort-pilot` |
| LIVE | DEPRECATED | — | — | `capability-binding.deprecate` |
| DEPRECATED | RETIRED | — | — | `capability-binding.retire` |

**D — Cross-workspace constraints involving this state machine.** Source side of CBU's `service_consumption_active_requires_live_binding` (`capability_bindings.binding_status = 'LIVE'` AND parent application_instance ACTIVE → CBU `service_consumption.provisioned → active` permitted). Declared in `cbu_dag.yaml` cross_workspace_constraints. The DAG header notes a `cross_workspace_constraint: cbu.service_consumption.activate requires LIVE binding` was deferred to R4 from the lifecycle_resources side, then landed in cbu_dag.yaml.

**E — Carrier evidence.**
- Carrier table: `"ob-poc".capability_bindings` (substrate audit §0.3: "✅" `binding_status` varchar CHECK, migration 2026-04-27).
- State column: `binding_status`
- Set comparison: DAG ∩ CHECK = all 5; DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.
- `state_dependency` declared (parent DECOMMISSIONED → child RETIRED) — runtime enforcement via DAG cascade rule (severity: error).

**F — DSL verbs.** `capability-binding.start-pilot`, `capability-binding.promote-live`, `capability-binding.abort-pilot`, `capability-binding.deprecate`, `capability-binding.retire`.

**G — Code paths.** Substrate audit §0.5 sample lists 5 `capability-binding.*` verbs. S-21 cosmetic mismatch flag applies. Code-path grep deferred.

**H — Migration history.** `rust/migrations/20260427_lifecycle_resources_workspace.sql` (cited in DAG header).

**I — Test fixtures.** Not gathered.

**J — Internal documentation.** DAG header.

---

### Workspace batch 1.5 — BookSetup

#### M-055 — book_setup.book

**A — Identity.**
- Workspace: book_setup
- DAG file: `book_setup_dag.yaml` (lines 195–255)
- State machine name: `book_lifecycle`
- Description (verbatim): "Client book setup-journey lifecycle. 7-state sequence tracking progress from proposed through ready-for-deal. Book is a conceptual container over CBUs; the table is introduced as part of this workspace (Tranche 3 migration adds client_books with (book_id, client_group_id, name, status, jurisdiction_hint, structure_template))."
- Owner: onboarding_ops
- Expected lifetime: long_lived
- `suspended_state_exempt: true` (rationale: `abandoned` covers workflow-halt semantics).
- Source entity: `"ob-poc".client_books` — DAG-first per D-2; **table missing** in current schema (substrate audit S-5).

**B — Declared states.**
| State | Type | Description | Carrier value |
|---|---|---|---|
| proposed | entry | "Book identified; client_group set." | proposed |
| structure_chosen | intermediate | "Jurisdiction + structure template selected." | structure_chosen |
| entities_provisioned | intermediate | "Legal entities created for the book structure." | entities_provisioned |
| cbus_scaffolded | intermediate | "CBUs created per fund vehicle; categories set." | cbus_scaffolded |
| parties_assigned | intermediate | "Structural roles assigned (manco, depositary, etc.)." | parties_assigned |
| mandates_defined | intermediate | "Trading mandates authored and linked to CBUs." | mandates_defined |
| ready_for_deal | terminal | "Book structure complete; ready for deal.CONTRACTED handoff." | ready_for_deal |
| abandoned | terminal | "Book-setup abandoned pre-handoff." | abandoned |

**C — Declared transitions.**
| From | To | Guard / precondition | XW | Verb |
|---|---|---|---|---|
| proposed | structure_chosen | — | — | `book.select-structure` |
| structure_chosen | entities_provisioned | structure_template selected (cross-slot constraint) | — | (macro: `structure.*` invokes entity.create series) |
| entities_provisioned | cbus_scaffolded | KYC case in DISCOVERY/ASSESSMENT/REVIEW/APPROVED for client group (XW constraint) | XW: book_cbus_scaffolded_requires_kyc_case_in_progress | `cbu.create` (iterated per fund vehicle) |
| cbus_scaffolded | parties_assigned | required roles assigned per structure template (cross-slot) | — | `cbu.assign-role` (iterated per role per CBU) |
| parties_assigned | mandates_defined | every CBU has at least one trading_profile in DRAFT+ (cross-slot) | — | `mandate.create` (iterated per mandate) |
| mandates_defined | ready_for_deal | every CBU VALIDATED (cross-slot) | XW (advisory): book_ready_requires_deal_contracted_gate | `book.mark-ready` |
| {proposed, structure_chosen, entities_provisioned, cbus_scaffolded, parties_assigned, mandates_defined} | abandoned | — | — | `book.abandon` |

**D — Cross-workspace constraints involving this state machine.**
1. `book_cbus_scaffolded_requires_kyc_case_in_progress` (source: kyc.kyc_case in [DISCOVERY, ASSESSMENT, REVIEW, APPROVED] WHERE `cases.client_group_id = this_book.client_group_id`; target: book_setup.book transition `entities_provisioned → cbus_scaffolded`; severity: error).
2. `book_ready_requires_deal_contracted_gate` (source: deal.deal in [KYC_CLEARANCE, CONTRACTED, ONBOARDING, ACTIVE] WHERE `deals.primary_client_group_id = this_book.client_group_id`; target: book_setup.book transition `mandates_defined → ready_for_deal`; severity: warning — advisory).

**E — Carrier evidence.**
- Carrier table: `"ob-poc".client_books` — **DAG-first; table does not exist in schema** (substrate audit S-5 SIGNIFICANT).
- State column: `status` (DAG-declared; no schema column).
- Schema status: substrate audit §0.3 row BookSetup: "❌ table not yet in schema"; §1.12 row G-E and S-5 BLOCKING-or-SIGNIFICANT classification.
- Set comparison: not computable — no carrier rows.
- The 5 cross-slot constraints (`book_entities_provisioned_requires_structure_chosen`, `book_cbus_scaffolded_requires_entities`, `book_parties_assigned_requires_all_cbus_have_required_roles`, `book_mandates_defined_requires_trading_profiles`, `book_ready_requires_all_cbus_validated`) have no schema enforcement path because the parent `client_books` row is conceptual.

**F — DSL verbs.** `book.select-structure`, `book.mark-ready`, `book.abandon`. Cross-workspace verbs invoked: `cbu.create`, `cbu.assign-role`, `mandate.create`. Macro: `structure.*` family invokes entity.create series.

**G — Code paths.** Substrate audit §0.5 sample lists `book.create / .mark-ready` in `book_setup` workspace; verb count "8" in workspace heuristic. Carrier write path does not exist (table missing).

**H — Migration history.** None — table not yet migrated. Substrate audit §7.2 lists S-5 (`client_books` table missing) as a SIGNIFICANT pre-R.1 remediation candidate.

**I — Test fixtures.** Not gathered.

**J — Internal documentation.** DAG header references `docs/todo/catalogue-platform-refinement-v1_3.md` and `docs/todo/tranche-3-semos-maintenance-findings-2026-04-24.md`.

---

### Workspace batch 1.6 — KYC

> All 12 KYC state machines reconcile to `kyc_dag.yaml` carriers. Substrate audit §0.3 row KYC: "✅ all 87 states materialised, none missing." Carrier alignment is exhaustive at the state level. Verbs sampled in substrate audit §0.5 include 7 `kyc-case.*`, `case.*`, `entity-workstream.*`, `screening-ops.*`, `screening.*`, `ubo-registry.*`, `red-flag.*`, `doc-request.*`, `evidence.*`. F–J are presented condensed.

#### M-007 — kyc.kyc_case

**A — Identity.** kyc · `kyc_dag.yaml` (lines 159–226) · `kyc_case_lifecycle` · "11-state KYC case lifecycle. Lifted (reconcile-existing) from existing rust/config/sem_os_seeds/state_machines/kyc_case_lifecycle.yaml. Schema-backed via cases.status CHECK constraint."

**B — Declared states.**
| State | Type | Description |
|---|---|---|
| INTAKE | entry | "Case created; scope not yet determined." |
| DISCOVERY | intermediate | "Entity workstreams created; evidence collection." |
| ASSESSMENT | intermediate | "Risk-rating being determined; UBO chain finalized." |
| REVIEW | intermediate | "Committee / compliance sign-off in progress." |
| APPROVED | terminal | "Case approved; CBU cleared for onboarding." |
| REJECTED | terminal | "Case rejected; CBU cannot onboard under this case." |
| BLOCKED | terminal | "Blocked pending external resolution." |
| WITHDRAWN | terminal | "Client withdrew application." |
| DO_NOT_ONBOARD | terminal | "Hard-stop terminal — refusal with compliance hold." |
| EXPIRED | terminal | "Case timed out (external — e.g. document validity lapsed)." |
| REFER_TO_REGULATOR | terminal | "Escalated to external authority." |

**C — Declared transitions.**
| From | To | Verb |
|---|---|---|
| INTAKE | DISCOVERY | `kyc-case.update-status` |
| DISCOVERY | ASSESSMENT | `kyc-case.update-status` |
| ASSESSMENT | REVIEW | `kyc-case.update-status` |
| REVIEW | APPROVED | `case.approve` |
| REVIEW | REJECTED | `case.reject` |
| (any non-terminal) | BLOCKED | `kyc-case.escalate` |
| (any non-terminal) | WITHDRAWN | `kyc-case.close` |
| (any non-terminal) | DO_NOT_ONBOARD | `case.reject` (with do-not-onboard flag) |
| (any non-terminal) | REFER_TO_REGULATOR | `kyc-case.escalate` |
| APPROVED | DISCOVERY | `kyc-case.reopen` (for periodic review) |
| (any non-terminal) | EXPIRED | (implicit from external timer) |

**D — Cross-workspace constraints involving this state machine.** Source side of three constraints declared in OTHER workspaces:
1. `cbu_validated_requires_kyc_case_approved` (CBU `VALIDATION_PENDING → VALIDATED` requires kyc_case APPROVED).
2. `deal_contracted_requires_kyc_approved` (Deal `KYC_CLEARANCE → CONTRACTED` requires kyc_case APPROVED).
3. `book_cbus_scaffolded_requires_kyc_case_in_progress` (book_setup `entities_provisioned → cbus_scaffolded` requires kyc_case in [DISCOVERY, ASSESSMENT, REVIEW, APPROVED]).

**E — Carrier evidence.** `"ob-poc".cases.status` varchar CHECK; substrate audit §0.3 KYC row: "✅ all 87 states." DAG ∩ CHECK = all 11; DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.

**F — Verbs.** `kyc-case.update-status`, `kyc-case.escalate`, `kyc-case.close`, `kyc-case.reopen`, `case.approve`, `case.reject`. Substrate audit §0.5 sample also lists `kyc-case.set-risk-rating`.

**G — Code paths.** Substrate audit §1.3 confirms verbs exist. Code-path grep deferred.

**H — Migration history.** Pre-existing CHECK; no recent migration cited in DAG header.

**I — Test fixtures.** Not gathered.

**J — Documentation.** DAG header `kyc_dag.yaml` (top); cited parents `docs/todo/tranche-2-kyc-kickoff-2026-04-23.md`, `docs/todo/catalogue-platform-refinement-v1_2.md`, `docs/todo/instrument-matrix-pilot-findings-2026-04-23.md`.

#### M-008 — kyc.entity_kyc

**A.** kyc · `kyc_dag.yaml` (lines 228–279) · `entity_kyc_lifecycle` · "Entity-level KYC journey (reconcile-existing from state_machines/entity_kyc_lifecycle.yaml). Tracks an entity's progression from initial skeleton/placeholder through workstream completion to approval as part of a case."

**B.** 8 states: placeholder (entry) → empty → filled → workstream_open → screening_complete → evidence_collected → verified → approved (terminal). All described per DAG.

**C.**
| From | To | Verb |
|---|---|---|
| placeholder | empty | (backend: entity lookup / GLEIF import) |
| empty | filled | `skeleton.build` |
| filled | workstream_open | `entity-workstream.add` |
| workstream_open | screening_complete | `screening-ops.full` |
| screening_complete | evidence_collected | `evidence.link` |
| evidence_collected | verified | `evidence.verify` |
| verified | approved | `entity-workstream.close` |

**D.** none.

**E.** Carrier: derived from `entity_workstreams.status` + `kyc_ubo_registry.status` (DAG declares `state_column: (derived from ...)`). No standalone state column. Set comparison: n/a (derived).

**F.** `skeleton.build`, `entity-workstream.add`, `screening-ops.full`, `evidence.link`, `evidence.verify`, `entity-workstream.close`.

**G–J.** Carrier is derived; underlying carriers (entity_workstreams, kyc_ubo_registry) covered by M-009 and M-012. DAG header.

#### M-009 — kyc.entity_workstream

**A.** kyc · `kyc_dag.yaml` (lines 285–344) · `entity_workstream_lifecycle` · "Per-entity workstream within a KYC case. 10 states from schema CHECK constraint."

**B.** 10 states: PENDING (entry), COLLECT, VERIFY, SCREEN, ASSESS, COMPLETE, BLOCKED, ENHANCED_DD, REFERRED, PROHIBITED. Terminal: [COMPLETE, PROHIBITED].

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | COLLECT | `entity-workstream.add` |
| COLLECT | VERIFY | `evidence.verify` |
| VERIFY | SCREEN | `screening-ops.full` |
| SCREEN | ASSESS | (backend: screenings cleared) |
| ASSESS | COMPLETE | `entity-workstream.close` |
| (any non-terminal) | BLOCKED | `entity-workstream.update-status` |
| (any non-terminal) | ENHANCED_DD | `red-flag.escalate` |
| (any non-terminal) | REFERRED | `kyc-case.escalate` |
| (any non-terminal) | PROHIBITED | `case.reject` (cascading) |

**D.** none.

**E.** `"ob-poc".entity_workstreams.status` varchar CHECK; "all states materialised" per audit. DAG ∩ CHECK complete.

**F.** `entity-workstream.add`, `entity-workstream.close`, `entity-workstream.update-status`, `evidence.verify`, `screening-ops.full`, `red-flag.escalate`, `kyc-case.escalate`, `case.reject`.

**G–J.** DAG header; substrate audit §1.3.

#### M-010 — kyc.screening

**A.** kyc · `kyc_dag.yaml` (lines 346–402) · `screening_lifecycle` · "Per-screening lifecycle. 8 states from schema CHECK. Applies to each screening_type (SANCTIONS, PEP, ADVERSE_MEDIA, CREDIT, CRIMINAL, REGULATORY, CONSOLIDATED)."

**B.** 8 states: PENDING (entry), RUNNING, CLEAR, HIT_PENDING_REVIEW, HIT_CONFIRMED (terminal — needs red_flag escalation), HIT_DISMISSED, ERROR, EXPIRED.

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | RUNNING | `screening-ops.start` |
| RUNNING | CLEAR | (backend: no hits) |
| RUNNING | HIT_PENDING_REVIEW | `screening.record-hit` |
| HIT_PENDING_REVIEW | HIT_CONFIRMED | `screening.confirm` |
| HIT_PENDING_REVIEW | HIT_DISMISSED | `screening.dismiss` |
| RUNNING | ERROR | (backend: provider error) |
| ERROR | PENDING | `screening-ops.retry` |
| (CLEAR, HIT_DISMISSED) | EXPIRED | (time-decay) |
| EXPIRED | PENDING | `screening-ops.refresh` |

**D.** none.

**E.** `"ob-poc".screenings.status` varchar CHECK. DAG ∩ CHECK complete.

**F.** `screening-ops.start`, `screening.record-hit`, `screening.confirm`, `screening.dismiss`, `screening-ops.retry`, `screening-ops.refresh`. Plus 3 backend signals.

**G–J.** DAG header; substrate audit §1.3.

#### M-011 — kyc.ubo_evidence

**A.** kyc · `kyc_dag.yaml` (lines 404–430) · `ubo_evidence_lifecycle` · "UBO evidence verification (4 states)."

**B.** 4 states: PENDING (entry), VERIFIED, REJECTED (terminal), EXPIRED.

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | VERIFIED | `evidence.verify` |
| PENDING | REJECTED | `evidence.reject` |
| VERIFIED | EXPIRED | (time-decay) |
| EXPIRED | PENDING | `evidence.require` (re-collection) |

**D.** Cited as predicate in cross-slot constraint `ubo_approved_requires_provable_evidence_chain`: `kyc_ubo_registry.status = APPROVED → EXISTS ubo_evidence WHERE ubo_id = this AND verification_status = VERIFIED`. Intra-DAG (cross_slot_constraints), not cross_workspace_constraints.

**E.** `"ob-poc".ubo_evidence.verification_status` varchar CHECK. DAG ∩ CHECK = {PENDING, VERIFIED, REJECTED, EXPIRED}; ∅ in either ∖.

**F.** `evidence.verify`, `evidence.reject`, `evidence.require`. Plus backend time-decay.

**G–J.** DAG header.

#### M-012 — kyc.kyc_ubo_registry

**A.** kyc · `kyc_dag.yaml` (lines 432–489) · `kyc_ubo_registry_lifecycle` · "UBO promotion lifecycle within a KYC case. 9 states tracking the UBO's journey from candidate through formal declaration."

**B.** 9 states: CANDIDATE (entry), IDENTIFIED, PROVABLE, PROVED, REVIEWED, APPROVED (terminal), WAIVED (terminal), REJECTED (terminal), EXPIRED.

**C.**
| From | To | Verb |
|---|---|---|
| CANDIDATE | IDENTIFIED | `ubo-registry.verify` |
| IDENTIFIED | PROVABLE | `ubo-registry.verify` |
| PROVABLE | PROVED | `evidence.verify` |
| PROVED | REVIEWED | (compliance review) |
| REVIEWED | APPROVED | `ubo-registry.approve` |
| REVIEWED | WAIVED | (compliance waiver) |
| REVIEWED | REJECTED | `ubo-registry.reject` |
| APPROVED | EXPIRED | (time-decay) |
| EXPIRED | CANDIDATE | `ubo-registry.expire` + `ubo-registry.discover` |

**D.** Source of cross-slot constraint M-011 (UBO evidence requirement). Cross-references CBU's M-034 `entity_limited_company_ubo` (transition `MANUAL_REQUIRED → DISCOVERED` via `ubo-registry.promote-to-ubo`). No `cross_workspace_constraints:` declared (the cross-DAG link is verb-mediated, not gate-mediated).

**E.** `"ob-poc".kyc_ubo_registry.status` varchar CHECK. DAG ∩ CHECK = all 9.

**F.** `ubo-registry.verify`, `ubo-registry.approve`, `ubo-registry.reject`, `ubo-registry.expire`, `ubo-registry.discover`, `evidence.verify`. Plus 4 backend signals.

**G–J.** DAG header. Substrate audit S-25 flags overlap with deal.deal_ubo_assessment (M-051) — "v1.3 candidate: consolidate the two, they're epistemic states of the same underlying UBO determination" per DAG header note.

#### M-013 — kyc.kyc_ubo_evidence

**A.** kyc · `kyc_dag.yaml` (lines 491–526) · `kyc_ubo_evidence_lifecycle` · "Evidence for UBO determination (7 states)."

**B.** 7 states: REQUIRED (entry), REQUESTED, RECEIVED, VERIFIED, REJECTED (terminal), WAIVED (terminal), EXPIRED.

**C.**
| From | To | Verb |
|---|---|---|
| REQUIRED | REQUESTED | `doc-request.send` |
| REQUESTED | RECEIVED | `doc-request.verify` |
| RECEIVED | VERIFIED | `evidence.verify` |
| RECEIVED | REJECTED | `evidence.reject` |
| (any non-terminal) | WAIVED | `evidence.waive` |
| VERIFIED | EXPIRED | (time-decay) |

**D.** none.

**E.** `"ob-poc".kyc_ubo_evidence.status` varchar CHECK. DAG ∩ CHECK complete per audit §0.3.

**F.** `doc-request.send`, `doc-request.verify`, `evidence.verify`, `evidence.reject`, `evidence.waive`.

**G–J.** DAG header.

#### M-014 — kyc.red_flag

**A.** kyc · `kyc_dag.yaml` (lines 528–568) · `red_flag_lifecycle` · "Risk-flag lifecycle (6 states)."

**B.** 6 states: OPEN (entry), UNDER_REVIEW, MITIGATED, WAIVED, BLOCKING, CLOSED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| OPEN | UNDER_REVIEW | `red-flag.escalate` |
| UNDER_REVIEW | MITIGATED | `red-flag.resolve` |
| UNDER_REVIEW | WAIVED | `red-flag.waive` |
| UNDER_REVIEW | BLOCKING | `red-flag.update-rating` |
| (MITIGATED, WAIVED) | CLOSED | (case progression) |
| BLOCKING | MITIGATED | `red-flag.resolve` |

**D.** Cited in cross-slot constraint `case_cannot_approve_with_unresolved_red_flags` (kyc_case `→ APPROVED` blocked while any red_flag.status = BLOCKING).

**E.** `"ob-poc".red_flags.status` varchar CHECK.

**F.** `red-flag.escalate`, `red-flag.resolve`, `red-flag.waive`, `red-flag.update-rating`. Plus 1 backend (case progression).

**G–J.** DAG header.

#### M-015 — kyc.doc_request

**A.** kyc · `kyc_dag.yaml` (lines 570–613) · `doc_request_lifecycle` · "Document request lifecycle (9 states)."

**B.** 9 states: DRAFT (entry), REQUIRED, REQUESTED, RECEIVED, UNDER_REVIEW, VERIFIED, REJECTED (terminal), WAIVED (terminal), EXPIRED.

**C.**
| From | To | Verb |
|---|---|---|
| DRAFT | REQUIRED | `doc-request.create` |
| REQUIRED | REQUESTED | `doc-request.send` |
| REQUESTED | RECEIVED | `doc-request.verify` |
| RECEIVED | UNDER_REVIEW | (compliance review) |
| UNDER_REVIEW | VERIFIED | `evidence.verify` |
| UNDER_REVIEW | REJECTED | `evidence.reject` |
| (any non-terminal) | WAIVED | `evidence.waive` |
| (VERIFIED, REJECTED) | EXPIRED | (time-decay) |

**D.** Cited in cross-slot constraint `doc_request_verified_triggers_workstream_advance` (informational).

**E.** `"ob-poc".doc_requests.status` varchar CHECK.

**F.** `doc-request.create`, `doc-request.send`, `doc-request.verify`, `evidence.verify`, `evidence.reject`, `evidence.waive`.

**G–J.** DAG header.

#### M-016 — kyc.outreach_request

**A.** kyc · `kyc_dag.yaml` (lines 615–650) · `outreach_request_lifecycle` · "Counterparty outreach (7 states)."

**B.** 7 states: DRAFT (entry), PENDING, SENT, REMINDED, RESPONDED, CLOSED (terminal), EXPIRED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| DRAFT | PENDING | `request.create` |
| PENDING | SENT | `request.send` |
| SENT | REMINDED | `request.remind` |
| (SENT, REMINDED) | RESPONDED | `request.record-response` |
| (RESPONDED, REMINDED) | CLOSED | `request.close` |
| (SENT, REMINDED) | EXPIRED | (cutoff passed) |

**D.** none.

**E.** `"ob-poc".outreach_requests.status` varchar CHECK.

**F.** `request.create`, `request.send`, `request.remind`, `request.record-response`, `request.close`.

**G–J.** DAG header.

#### M-017 — kyc.kyc_decision

**A.** kyc · `kyc_dag.yaml` (lines 652–675) · `kyc_decision_lifecycle` · "Final KYC decision (4 states)."

**B.** 4 states: PENDING_REVIEW (entry), CLEARED (terminal), REJECTED (terminal), CONDITIONAL (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| PENDING_REVIEW | CLEARED | `case.approve` |
| PENDING_REVIEW | REJECTED | `case.reject` |
| PENDING_REVIEW | CONDITIONAL | `case.approve-with-conditions` |

**D.** none directly; M-007 (kyc_case) drives kyc_decision creation/state.

**E.** `"ob-poc".kyc_decisions.status` varchar CHECK.

**F.** `case.approve`, `case.reject`, `case.approve-with-conditions`.

**G–J.** DAG header.

#### M-018 — kyc.kyc_service_agreement

**A.** kyc · `kyc_dag.yaml` (lines 677–706) · `kyc_service_agreement_lifecycle` · "KYC-as-a-Service sponsor arrangement (4 states, proposed — schema today has no CHECK constraint)." Product-gated: activated_by `[product.kyc_service]`.

**B.** 4 states: DRAFT (entry), ACTIVE, SUSPENDED, TERMINATED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| DRAFT | ACTIVE | `kyc-agreement.update` (status=ACTIVE) |
| ACTIVE | SUSPENDED | `kyc-agreement.update` (status=SUSPENDED) |
| SUSPENDED | ACTIVE | `kyc-agreement.update` (status=ACTIVE) |
| (ACTIVE, SUSPENDED) | TERMINATED | `kyc-agreement.delete` |

**D.** Cited in cross-slot constraint `kyc_service_agreement_active_required_for_sponsored_cases` (cases.kyc_context = SPONSORED → EXISTS kyc_service_agreement WHERE sponsor_id = ... AND status = ACTIVE).

**E.** `"ob-poc".kyc_service_agreements.status` — DAG comment notes "schema today has no CHECK constraint". Set comparison: DAG declares 4 states; CHECK enumeration not currently enforced (verb-only validation).

**F.** `kyc-agreement.update`, `kyc-agreement.delete`.

**G–J.** DAG header. Carrier note: 4-state lifecycle proposed in DAG; underlying schema `kyc_service_agreements.status` column likely a free-text varchar (substrate audit lists `kyc_service_agreements` as carrier in §0.3 KYC row but does not flag a missing CHECK).

---

### Workspace batch 1.7 — InstrumentMatrix

> Mixed carrier maturity. Substrate audit §0.3 row IM: "partial" — `cbu_trading_activity`, `reconciliation`, `corporate_action_event`, `collateral_management` are DAG-first table-missing (S-6, S-14); `cbu_settlement_chains` 7-state lifecycle is collapsed onto `is_active` boolean (S-12); `service_resource_types` 4-state lifecycle is collapsed onto `is_active` boolean (S-13).

#### M-019 — instrument_matrix.group

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 274–317) · `group_discovery_lifecycle` · "Client-group discovery lifecycle. Driven by the research / GLEIF-import pipeline. Prerequisite for creating any trading mandate under the group."

**B.** 5 states: not_started (entry), in_progress, complete, stale, failed. No declared `terminal_states`.

**C.**
| From | To | Verb |
|---|---|---|
| not_started | in_progress | (research pipeline kick-off — upstream trigger) |
| in_progress | complete | (discovery confirmed signal — backend) |
| in_progress | failed | (research fault signal — backend) |
| complete | stale | (time-decay trigger) |
| stale | in_progress | (refresh trigger) |
| failed | in_progress | (retry trigger) |

**D.** Read-only reference from CBU, Deal, BookSetup, KYC (each declares `client_group` as a reconcile-existing slot pointing to this state machine).

**E.** `"ob-poc".client_group.discovery_status`. Substrate audit §0.3 lists `client_group` in Core/Shared family (~36 tables); no specific CHECK note. DAG ∩ CHECK alignment not separately audited but no carrier-only states found anywhere in §0.3.

**F.** Verbs: 0 in DAG transitions — all transitions are backend signals (research pipeline, time-decay, retry). The slot is a "signal slot" not author-driven.

**G–J.** DAG header. Substrate audit S-21 cosmetic flag potentially applies for any client_group verbs.

#### M-020 — instrument_matrix.trading_profile_template

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 319–344) · `trading_profile_template_lifecycle` · "Template-level trading profile lifecycle. Simpler than the streetside lifecycle (Adam Q6: templates are either available or not). Templates are group-level artefacts cloned to CBU instances."

**B.** 2 states: available (entry), unavailable (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| available | unavailable | `trading-profile.retire-template` |

**D.** none.

**E.** `"ob-poc".cbu_trading_profiles` (rows WHERE `cbu_id IS NULL` — template scope). State column: `status`. Set comparison: 2 DAG states are subsumed in the larger 9-state CHECK on the same column (M-023 trading_profile uses cbu_id IS NOT NULL rows).

**F.** `trading-profile.retire-template`.

**G–J.** DAG header. Substrate audit §0.3 IM row notes `cbu_trading_profiles` carrier "✅" with `status`.

#### M-021 — instrument_matrix.settlement_pattern_template

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 346–419) · `settlement_pattern_lifecycle` · "Settlement chain lifecycle. Schema today uses only `is_active`; DAG formalises the pre-activation lifecycle (Adam Q1). Parallel_run state added per pass-7 Q-E. `superseded_by` graph attribute for chain replacement."

**B.** 7 states: draft (entry), configured, reviewed, parallel_run, live, suspended, deactivated (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| draft | configured | `settlement-chain.add-hop` |
| draft | configured | `settlement-chain.define-location` |
| configured | reviewed | `settlement-chain.request-review` |
| reviewed | parallel_run | `settlement-chain.enter-parallel-run` |
| parallel_run | live | `settlement-chain.go-live` |
| parallel_run | reviewed | `settlement-chain.abort-parallel-run` |
| live | suspended | `settlement-chain.suspend` |
| suspended | live | `settlement-chain.reactivate` |
| live | deactivated | `settlement-chain.deactivate-chain` |
| suspended | deactivated | `settlement-chain.deactivate-chain` |

**D.** Cited in cross-slot constraints `mandate_active_requires_live_settlement`, `archived_mandate_cascades_dependents`, `deactivated_chain_requires_universe_recheck`.

**E.** Carrier `"ob-poc".cbu_settlement_chains`. **State column collapse**: DAG declares 7 states; schema uses only `is_active` boolean (substrate audit S-12 SIGNIFICANT). DAG comment: "live ≡ is_active=true; deactivated ≡ is_active=false; draft / configured / reviewed / parallel_run are currently conceptual; schema has no state column yet. Will require a migration to persist."
- DAG ∩ CHECK = {live, deactivated} (modulo boolean encoding); DAG ∖ CHECK = {draft, configured, reviewed, parallel_run, suspended} — five states **not materialised** in schema.
- Graph attribute `superseded_by` declared (uuid → cbu_settlement_chains.chain_id).

**F.** `settlement-chain.add-hop`, `settlement-chain.define-location`, `settlement-chain.request-review`, `settlement-chain.enter-parallel-run`, `settlement-chain.go-live`, `settlement-chain.abort-parallel-run`, `settlement-chain.suspend`, `settlement-chain.reactivate`, `settlement-chain.deactivate-chain`.

**G–J.** Substrate audit S-12 SIGNIFICANT. DAG header. Verbs marked "NEW verb — P.3" in DAG indicate they were authored as part of the IM pilot (P.3); whether they are wired to a CHECK column or only to is_active is unverified — verb-impl grep deferred.

#### M-022 — instrument_matrix.trade_gateway

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 467–514) · `trade_gateway_lifecycle` · "Broker / exchange gateway lifecycle. Hybrid persistence per Adam Q3: document-shaped config (FIX session details, routing rules, ticker maps) with `is_active` column for query speed. State machine lives on document body; is_active is a materialized view. Intermediate UAT / FIX-certification states are NOT in the DAG (Adam Q-F: UAT is service-resource setup — how, not what)."

**B.** 5 states: defined (entry), enabled, active, suspended, retired (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| defined | enabled | `trade-gateway.enable-gateway` |
| enabled | active | `trade-gateway.activate-gateway` |
| active | suspended | `trade-gateway.suspend-gateway` |
| suspended | active | `trade-gateway.reactivate-gateway` |
| active | retired | `trade-gateway.retire-gateway` |
| suspended | retired | `trade-gateway.retire-gateway` |

**D.** Cited in `archived_mandate_cascades_dependents` (warning) and `retired_gateway_prunes_routing_rules` (error).

**E.** Carrier: "(hybrid — JSON document + `is_active` column for query speed)". State column: "(document body — not SQL CHECK)". Carrier alignment is by-document, not by-CHECK. Substrate audit does not separately enumerate this collapse; structurally similar to S-13 (boolean-collapsed lifecycle) but DAG explicitly chose hybrid persistence per Adam Q3.

**F.** Six `trade-gateway.*` verbs covering all 6 transitions.

**G–J.** DAG header. Verbs marked "NEW verb — P.3" for `reactivate-gateway` and `retire-gateway`.

#### M-023 — instrument_matrix.trading_profile

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 529–613) · `trading_profile_lifecycle` · "The mandate itself — streetside instance cloned from template. 9 states (passes 3+7): parallel_run between approved and active, plus superseded terminal state for version-replacement audit. ACTIVE state projects into CBU tollgate (§5 cross-workspace aggregate)."
- Owner: trading+ops
- Expected lifetime: long_lived (SUSPENDED present)
- `periodic_review_cadence:` `P1Y` base / HIGH `P6M` / LOW `P2Y`

**B.** 9 states: DRAFT (entry), SUBMITTED, APPROVED, PARALLEL_RUN, ACTIVE, SUSPENDED, REJECTED, SUPERSEDED (terminal), ARCHIVED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| DRAFT | SUBMITTED | `trading-profile.submit` |
| SUBMITTED | APPROVED | `trading-profile.approve` |
| SUBMITTED | REJECTED | `trading-profile.reject` |
| REJECTED | DRAFT | `trading-profile.create-draft` |
| APPROVED | PARALLEL_RUN | `trading-profile.enter-parallel-run` |
| PARALLEL_RUN | ACTIVE | `trading-profile.go-live` |
| PARALLEL_RUN | APPROVED | `trading-profile.abort-parallel-run` |
| ACTIVE | SUSPENDED | `trading-profile.suspend` |
| SUSPENDED | ACTIVE | `trading-profile.reactivate` |
| ACTIVE | SUPERSEDED | `trading-profile.supersede` (new version goes live) |
| ACTIVE | ARCHIVED | `trading-profile.archive` |
| SUSPENDED | ARCHIVED | `trading-profile.archive` |

**D.** Cross-workspace constraints involving this state machine:
1. `mandate_requires_validated_cbu` (this DAG): trading_profile DRAFT → SUBMITTED requires CBU `cbus.status = VALIDATED`. Source: cbu.cbu (M-031); target: this slot.
2. **CBU-side reciprocal** in cbu_dag.yaml `derived_cross_workspace_state` (`cbu_operationally_active`): aggregates `trading_profiles.cbu_id = this_cbu.cbu_id AND trading_profiles.status = 'ACTIVE'` as a contributor to CBU operationally_active tollgate.

Cross-slot constraints (intra-DAG) involving this slot: `mandate_requires_validated_cbu` (R-4 promoted to cross_workspace_constraints), `mandate_active_requires_live_settlement`, `archived_mandate_cascades_dependents`, `cbu_archived_requires_mandate_archived`, `isda_coverage_required_for_derivative_trading`.

**E.** Carrier `"ob-poc".cbu_trading_profiles` (rows WHERE cbu_id IS NOT NULL — streetside instances). State column `status`. Substrate audit §0.3 IM row: "trading_profile 9/9 ✅" — full CHECK alignment.

**F.** 9 transition verbs (`trading-profile.submit / .approve / .reject / .create-draft / .enter-parallel-run / .go-live / .abort-parallel-run / .suspend / .reactivate / .supersede / .archive`).

**G–J.** DAG header. Verbs marked "NEW verb — P.3" for parallel_run + supersede + suspend/reactivate. Substrate audit §1.8 confirms verb chain.

#### M-024 — instrument_matrix.trading_activity

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 619–674) · `trading_activity_lifecycle` · "Per-CBU trading-activity state derived from trade events. Governs the distinction between trade_permissioned (mandate live, never traded) and actively_trading (has traded and is currently active) in the overall_lifecycle. Also flags dormancy for commercial / regulatory review. Signal slot: the state is the input, not a thing the UI edits directly."
- Owner: trading+ops
- Expected lifetime: long_lived
- Source entity: `"ob-poc".cbu_trading_activity` — DAG-first; **table missing** (substrate audit S-6 SIGNIFICANT).

**B.** 4 states: never_traded (entry), trading, dormant, suspended. No declared terminal_states.

**C.**
| From | To | Verb |
|---|---|---|
| never_traded | trading | (backend: first trade posted — event from settlement pipeline) |
| trading | dormant | (backend: last_trade_at + dormancy_window < now) |
| dormant | trading | (backend: new trade posted) |
| (any non-suspended) | suspended | (backend: trading_profile.SUSPENDED triggers mirror) |
| suspended | trading | (backend: trading_profile.reactivate triggers mirror) |

**D.** Intended consumer: cbu_dag.yaml's `cbu_operationally_active` tollgate (M-031 derived_cross_workspace_state). DAG header: "trading_activity.first_trade_at IS NOT NULL → cbu moves past trade_permissioned into actively_trading." Substrate audit §1.12 G-A.

**E.** Carrier table **missing** (S-6). State column `activity_state` declared in DAG; no schema column. Set comparison: not computable.

**F.** All transitions are backend signals — 0 author-facing verbs.

**G–J.** Substrate audit S-6 SIGNIFICANT. DAG header. Tranche 3 migration scheduled.

#### M-025 — instrument_matrix.service_resource

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 735–773) · `service_resource_lifecycle` · "SRDEF lifecycle (Adam Q9b: 4 states confirmed). This is the typed-resource definition layer; per-CBU instances live in `cbu_resource_instances` (a separate table, provisioned by Stage 5 of the product-service pipeline per pass 6)."

**B.** 4 states: provisioned (entry), activated, suspended, decommissioned (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| provisioned | activated | `service-resource.activate` |
| activated | suspended | `service-resource.suspend` |
| suspended | activated | `service-resource.reactivate` |
| activated | decommissioned | `service-resource.decommission` |
| suspended | decommissioned | `service-resource.decommission` |

**D.** Cited in cross-slot constraint `decommissioned_resource_cascades_intent` (severity error).

**E.** Carrier `"ob-poc".service_resource_types`. State column: "(derived from `is_active` + `provisioning_strategy`)" — DAG comment. Substrate audit S-13 MINOR: "4-state lifecycle collapsed onto boolean `is_active`."
- DAG ∩ CHECK: 2 states encodable (activated ≡ is_active=true; decommissioned ≡ is_active=false). DAG ∖ CHECK = {provisioned, suspended}.

**F.** `service-resource.activate`, `service-resource.suspend`, `service-resource.reactivate`, `service-resource.decommission`.

**G–J.** Substrate audit S-13 MINOR. DAG header.

#### M-026 — instrument_matrix.service_intent

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 775–812) · `service_intent_lifecycle` · "Per-CBU service intent — declares which (product, service) combinations the CBU has enrolled in, with options JSONB carrying service-specific parameters (markets, currencies, instrument classes, counterparties). This table IS the CBU's 'lifecycle services profile' per pass 6."

**B.** 3 states: active (entry), suspended, cancelled (terminal). Plus `additional_operations: service-intent.supersede` (replaces intent with new row, immutable log-like).

**C.**
| From | To | Verb |
|---|---|---|
| active | suspended | `service-intent.suspend` |
| suspended | active | `service-intent.resume` |
| active | cancelled | `service-intent.cancel` |
| suspended | cancelled | `service-intent.cancel` |

**D.** Cited in cross-slot constraints `archived_mandate_cascades_dependents` (warning), `decommissioned_resource_cascades_intent` (error).

**E.** Carrier `"ob-poc".service_intents`. State column `status`. Substrate audit S-25 SIGNIFICANT: "service_intents (3-state) and missing `cbu_service_consumption` (6-state) have semantic overlap — both describe per-(cbu, service) consumption" — flagged for resolution when S-1 lands.

**F.** `service-intent.suspend`, `service-intent.resume`, `service-intent.cancel`, `service-intent.supersede`.

**G–J.** DAG header. S-25 noted.

#### M-027 — instrument_matrix.delivery

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 851–891) · `delivery_lifecycle` · "Service delivery tracking. 5 states per schema CHECK constraint. NOTE: this is borderline operational under the what-vs-how rule (pass 3 addendum §9.4). Kept as-is per A-1 v3 because schema is authoritative — flag for potential future refactor out of IM."
- v1.2 V1.2-7 borderline_operational candidate.

**B.** 5 states: PENDING (entry), IN_PROGRESS, DELIVERED (terminal), FAILED (terminal), CANCELLED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | IN_PROGRESS | `delivery.start` |
| IN_PROGRESS | DELIVERED | `delivery.complete` |
| IN_PROGRESS | FAILED | `delivery.fail` |
| PENDING | CANCELLED | `delivery.cancel` |
| IN_PROGRESS | CANCELLED | `delivery.cancel` |

**D.** none.

**E.** Carrier `"ob-poc".service_delivery_map`. State column `delivery_status`. Substrate audit §0.3 IM row notes `delivery_status` schema-backed.

**F.** `delivery.start`, `delivery.complete`, `delivery.fail`, `delivery.cancel`.

**G–J.** DAG header. v1.2 V1.2-7 borderline_operational note.

#### M-028 — instrument_matrix.reconciliation

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 897–942) · `reconciliation_config_lifecycle` · "Reconciliation CONFIG lifecycle — which SoR we reconcile against, tolerances, escalation paths. The RUN events (scheduled / running / matched / breaks) are operational runtime (layer 3) and not in the DAG. (Adam Q-A: 'recon is part of a mandate.')"
- Product-gated: `[product.custody, product.fund_accounting, product.cash_management]`.
- DAG-first; **table missing** (substrate audit S-14 MINOR).

**B.** 4 states: draft (entry), active, suspended, retired (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| draft | active | `reconciliation.activate` |
| active | suspended | `reconciliation.suspend` |
| suspended | active | `reconciliation.reactivate` |
| active | retired | `reconciliation.retire` |
| suspended | retired | `reconciliation.retire` |

**D.** none.

**E.** Carrier and state column declared "(new — P.2 schema work or reuse of existing recon table if present)". Set comparison: not computable.

**F.** `reconciliation.activate`, `reconciliation.suspend`, `reconciliation.reactivate`, `reconciliation.retire`.

**G–J.** Substrate audit S-14. DAG header.

#### M-029 — instrument_matrix.corporate_action_event

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 944–976) · `corporate_action_event_lifecycle` · "Per-mandate CA event decision-capture. Distinct from corporate_action_policy (static config on trading profile). This captures the mandate's ELECTION DECISION for a specific CA event. Processing + settlement are operational (layer 3) — tracked downstream in service profile, not in DAG (Adam Q-B + pass-3 addendum narrowed to 3 states)."
- Product-gated: `[product.custody]`.
- DAG-first; **table missing** (substrate audit S-14 MINOR).

**B.** 3 states: election_pending (entry), elected (terminal), default_applied (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| election_pending | elected | `corporate-action-event.elect` |
| election_pending | default_applied | (automatic trigger at cutoff) |

**D.** none.

**E.** Carrier "(new — per-mandate CA event tracking)"; **table missing**. Set comparison: not computable.

**F.** `corporate-action-event.elect`. Plus 1 backend (cutoff trigger).

**G–J.** Substrate audit S-14. DAG header.

#### M-030 — instrument_matrix.collateral_management

**A.** instrument_matrix · `instrument_matrix_dag.yaml` (lines 978–1020) · `collateral_management_lifecycle` · "Per-mandate collateral management config — which CSAs in play, collateral schedule, thresholds, MTA. Does NOT model individual margin call events (those are runtime — layer 3). Adam Q-C: 'derivatives 100% in scope, add collateral management as a first-class branch.'"
- Product-gated: `[product.derivatives, product.collateral_mgmt]`.
- DAG-first; **table missing** (substrate audit S-14 MINOR).

**B.** 4 states: configured (entry), active, suspended, terminated (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| configured | active | `collateral-management.activate` |
| active | suspended | `collateral-management.suspend` |
| suspended | active | `collateral-management.reactivate` |
| active | terminated | `collateral-management.terminate` |
| suspended | terminated | `collateral-management.terminate` |

**D.** Cited in cross-slot constraint `collateral_management_active_requires_isda` (active state requires `isda_framework.is_active = true`).

**E.** Carrier "(new — per-mandate CSA/collateral config)"; **table missing**. Set comparison: not computable.

**F.** `collateral-management.activate`, `collateral-management.suspend`, `collateral-management.reactivate`, `collateral-management.terminate`.

**G–J.** Substrate audit S-14. DAG header.

---

### Workspace batch 1.8 — CBU

> CBU is the operational hub of the estate (87 inbound FKs per substrate audit §0.4). It carries 13 stateful slots plus a dual_lifecycle on the primary `cbu` slot. Carrier alignment is a mix: CBU discovery (M-031) is fully materialised; CBU operational dual (M-032), service_consumption (M-039), cbu_corporate_action (M-040), cbu_disposition (M-041), and share_class (M-043) are DAG-first with carrier columns or tables missing (substrate audit S-1, S-3, S-4, S-10, S-11). M-031 hosts the canonical `cbu_operationally_active` derived_cross_workspace_state aggregate.

#### M-031 — cbu.cbu (primary discovery)

**A.** cbu · `cbu_dag.yaml` (lines 297–346) · `cbu_discovery_lifecycle` · "5-state discovery/validation lifecycle. Schema-backed via chk_cbu_status CHECK constraint. This is the compliance-owned half of the CBU dual lifecycle; the operational half lives in dual_lifecycle below and starts at the VALIDATED junction."
- Owner: compliance
- Expected lifetime: long_lived
- `parent_slot:` self (cbu via `cbu_entity_relationships`); `state_dependency` cascade rules — parent suspended → child suspended; parent offboarded → child offboarded/archived (severity error). Models master-feeder / umbrella-compartment hierarchy.

**B.** 5 states: DISCOVERED (entry), VALIDATION_PENDING, VALIDATED (junction to operational dual), UPDATE_PENDING_PROOF, VALIDATION_FAILED (terminal).

**C.**
| From | To | Verb (with args) |
|---|---|---|
| DISCOVERED | VALIDATION_PENDING | `cbu.submit-for-validation` |
| VALIDATION_PENDING | VALIDATED | `cbu.decide` (decision: APPROVE) |
| VALIDATION_PENDING | VALIDATION_FAILED | `cbu.decide` (decision: REJECT) |
| VALIDATED | UPDATE_PENDING_PROOF | `cbu.request-proof-update` |
| UPDATE_PENDING_PROOF | VALIDATION_PENDING | `cbu.submit-for-validation` |
| VALIDATION_FAILED | VALIDATION_PENDING | `cbu.reopen-validation` |

**D.** Cross-workspace constraints involving this state machine:
1. `cbu_validated_requires_kyc_case_approved` (this DAG): VALIDATION_PENDING → VALIDATED requires kyc.kyc_case APPROVED (M-007). Source: kyc; target: cbu.
2. **Source side** of `mandate_requires_validated_cbu` declared in instrument_matrix_dag.yaml (IM trading_profile DRAFT → SUBMITTED requires cbu.cbu VALIDATED).
3. **Source side** of OnboardingRequest's `onboarding_request_requires_cbu_validated` (validating → submitted requires cbu VALIDATED).
4. **Source side** of derived state aggregator `cbu_operationally_active` (this DAG, see §5 below).

**E.** Carrier `"ob-poc".cbus`. State column `status`. Substrate audit §0.3 row CBU: discovery 5/5 ✅. CHECK enumeration: `DISCOVERED, VALIDATION_PENDING, VALIDATED, UPDATE_PENDING_PROOF, VALIDATION_FAILED`. DAG ∖ CHECK = ∅; CHECK ∖ DAG = ∅.

**F.** `cbu.submit-for-validation`, `cbu.decide`, `cbu.request-proof-update`, `cbu.reopen-validation`. (Plus `cbu.create`, `cbu.create-from-client-group`, `cbu.ensure` for entry creation per overall_lifecycle.)

**G — Code paths.** Substrate audit §1.7 confirms verb chain. `chk_cbu_status` CHECK constraint cited in DAG header.

**H — Migrations.** Pre-existing CHECK; no recent migration.

**I — Tests.** Not gathered.

**J — Documentation.** DAG header references `docs/todo/catalogue-platform-refinement-v1_3.md`, `docs/todo/tranche-2-cbu-findings-2026-04-23.md (§7.0 foundational reframe)`, `docs/todo/tranche-2-cross-workspace-reconciliation-2026-04-24.md (§4.1)`.

**Derived cross-workspace state hosted on this slot:** `cbu_operationally_active` (host_slot: cbu, host_state: operationally_active). Aggregates 5 sources:
- kyc.kyc_case.status = APPROVED (M-007).
- deal.deal.deal_status ∈ [CONTRACTED, ONBOARDING, ACTIVE] (M-045/M-046).
- instrument_matrix.trading_profile.status = ACTIVE (M-023).
- cbu.cbu_evidence ALL verified for required evidence (M-035).
- cbu.service_consumption ≥1 in active state (M-039 — currently DAG-first).
Visibility: first_class_state; cacheable.

#### M-032 — cbu.cbu (operational dual)

**A.** cbu · `cbu_dag.yaml` (lines 351–408) · `cbu_operational_lifecycle` (dual_lifecycle on slot `cbu`) · "Operational trading/servicing lifecycle of the CBU. Owned by trading + ops. Begins at VALIDATED junction once discovery completes; operational gating is via cbu.operationally_active tollgate aggregate (§5). Modelled in DAG ahead of schema migration — fields below do not yet have a cbus.* column to write into. Tranche 3 migration will add cbus.operational_status."
- Owner: trading+ops
- junction_state_from_primary: VALIDATED

**B.** 8 states: dormant (entry), trade_permissioned, actively_trading, restricted, suspended, winding_down, offboarded (terminal), archived (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| dormant | trade_permissioned | (backend: cbu.operationally_active becomes true) |
| trade_permissioned | actively_trading | (backend: first trade executed — IM trading_activity first_trade_at set) |
| actively_trading | restricted | `cbu.restrict` |
| restricted | actively_trading | `cbu.unrestrict` |
| {trade_permissioned, actively_trading, restricted} | suspended | `cbu.suspend` |
| suspended | actively_trading | `cbu.reinstate` |
| {trade_permissioned, actively_trading, restricted, suspended} | winding_down | `cbu.begin-winding-down` |
| winding_down | offboarded | `cbu.complete-offboard` |
| offboarded | archived | (backend: archival scheduler) |

**D.** Consumed by overall_lifecycle of this workspace. Suspended state from this dual is also referenced from M-044 (manco SUSPENDED cascade) and M-038 (holding state). No cross_workspace_constraints declared with this dual as source/target directly — its states feed `cbu_operationally_active` indirectly (via service_consumption + trading_profile readiness).

**E.** Carrier `"ob-poc".cbus`. State column `operational_status` — **column missing in schema** (substrate audit S-3 SIGNIFICANT). Set comparison: DAG declares 8 states; CHECK does not exist (column missing). DAG ∖ CHECK = all 8 (none materialised).

**F.** `cbu.restrict`, `cbu.unrestrict`, `cbu.suspend`, `cbu.reinstate`, `cbu.begin-winding-down`, `cbu.complete-offboard`. Plus 3 backend transitions (operationally_active flip, first-trade signal, archival scheduler).

**G–J.** Substrate audit S-3 SIGNIFICANT. DAG header. R-3 re-centring 2026-04-24.

#### M-033 — cbu.entity_proper_person

**A.** cbu · `cbu_dag.yaml` (lines 414–439) · `entity_proper_person_lifecycle` · "Natural-person entity state. 3 states from schema comment. Applies to UBOs, directors, signatories, beneficial owners."

**B.** 3 states: GHOST (entry), IDENTIFIED, VERIFIED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| GHOST | IDENTIFIED | `entity.identify` |
| IDENTIFIED | VERIFIED | `entity.verify` |

**D.** Cited in cross-slot constraint `entity_verified_required_for_ubo_role` (entity assigned to ownership/control role must be VERIFIED for proper_person OR ubo-terminal for limited_company).

**E.** Carrier `"ob-poc".entity_proper_persons`. State column `person_state`. Substrate audit §0.3 CBU row notes `person_state` schema-backed.

**F.** `entity.identify`, `entity.verify`.

**G–J.** DAG header.

#### M-034 — cbu.entity_limited_company_ubo

**A.** cbu · `cbu_dag.yaml` (lines 441–480) · `entity_limited_company_ubo_lifecycle` · "UBO discovery state for limited companies. 5 states from schema comment. Tracks whether beneficial ownership has been traced and categorised."

**B.** 5 states: PENDING (entry), DISCOVERED (terminal), PUBLIC_FLOAT (terminal), EXEMPT (terminal), MANUAL_REQUIRED.

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | DISCOVERED | (backend: UBO discovery pipeline) |
| PENDING | PUBLIC_FLOAT | (backend: listed-company check) |
| PENDING | EXEMPT | (backend: exemption classifier) |
| PENDING | MANUAL_REQUIRED | (backend: pipeline failure) |
| MANUAL_REQUIRED | DISCOVERED | `ubo-registry.promote-to-ubo` (cross-workspace verb from KYC) |

**D.** Cross-DAG link: KYC's `ubo-registry.promote-to-ubo` mutates this state. Same verb is also used in M-012 (kyc_ubo_registry). Cited in cross-slot constraint `cbu_validated_requires_uboes_in_terminal_state` (CBU VALIDATED requires all entity_limited_companies in linked role-chain at ubo_status ∈ {DISCOVERED, PUBLIC_FLOAT, EXEMPT}).

**E.** Carrier `"ob-poc".entity_limited_companies`. State column `ubo_status`. Substrate audit §0.3 schema-backed.

**F.** `ubo-registry.promote-to-ubo`. Plus 4 backend signals.

**G–J.** DAG header.

#### M-035 — cbu.cbu_evidence

**A.** cbu · `cbu_dag.yaml` (lines 486–521) · `cbu_evidence_lifecycle` · "Per-evidence-item lifecycle. Evidence supports CBU validation: ownership proofs, formation docs, attestations, registry checks, manual verifications."

**B.** 4 states: PENDING (entry), VERIFIED, REJECTED (terminal), EXPIRED.

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | VERIFIED | `cbu.verify-evidence` |
| PENDING | REJECTED | `cbu.verify-evidence` (decision: reject) |
| VERIFIED | EXPIRED | (backend: time-decay trigger based on evidence_type validity window) |
| EXPIRED | PENDING | `cbu.attach-evidence` (re-attach refresh) |

**D.** Source side of cross-slot constraint `cbu_validated_requires_evidence_set_verified` (CBU VALIDATED requires ALL required cbu_evidence rows at VERIFIED). Cited as predicate in `cbu_operationally_active` derived state (M-031).

**E.** Carrier `"ob-poc".cbu_evidence`. State column `verification_status`. Substrate audit §0.3 CBU row.

**F.** `cbu.verify-evidence`, `cbu.attach-evidence`. Plus 1 backend (time-decay).

**G–J.** DAG header.

#### M-036 — cbu.investor

**A.** cbu · `cbu_dag.yaml` (lines 527–579) · `investor_lifecycle` · "Investor commercial lifecycle — from eligibility check through active subscription to offboarding. Distinct from investor_kyc (parallel; see below). This lifecycle is the 'can-they-invest-right-now' state."
- Owner: ops
- Expected lifetime: long_lived (SUSPENDED present)
- `category_gated:` activated_by `[FUND_MANDATE]`

**B.** 7 states: DRAFT (entry), ELIGIBLE, ACTIVE, REDEEMING, REDEEMED, SUSPENDED, OFFBOARDED (terminal).

**C.**
| From | To | Verb (precondition) |
|---|---|---|
| DRAFT | ELIGIBLE | `investor.mark-eligible` |
| ELIGIBLE | ACTIVE | `investor.activate` (precondition: investor_kyc.status = APPROVED) |
| ACTIVE | SUSPENDED | `investor.suspend` |
| SUSPENDED | ACTIVE | `investor.reinstate` |
| (ACTIVE, SUSPENDED) | OFFBOARDED | `investor.offboard` |

> Note — REDEEMING and REDEEMED states declared but no transitions explicitly enter or leave them in the DAG transitions table.

**D.** Cited in cross-slot constraints `investor_active_requires_kyc_approved`, `holding_active_requires_investor_active`, `holding_suspended_cascades_from_investor` (informational), `investor_offboarded_requires_all_holdings_closed`.

**E.** Carrier `"ob-poc".investors`. State column: "(derived from investor lifecycle events + status)" per DAG. Substrate audit notes investors carrier in CBU §0.3 row.

**F.** `investor.mark-eligible`, `investor.activate`, `investor.suspend`, `investor.reinstate`, `investor.offboard`.

**G–J.** DAG header. **Possible mechanical anomaly:** REDEEMING and REDEEMED states declared but unreachable / unleavable per DAG transitions. (See §2.A / §2.B.)

#### M-037 — cbu.investor_kyc

**A.** cbu · `cbu_dag.yaml` (lines 581–644) · `investor_kyc_lifecycle` · "Per-investor KYC state, parallel to investor_lifecycle. 6 states from schema comment. Refresh cadence is regulatory (annual high-risk, biennial low-risk). Matches KYC findings V1.3-CAND-3 periodic review cadence pattern."
- Owner: compliance
- Expected lifetime: long_lived
- `suspended_state_exempt: true` (rationale: KYC refresh handled via EXPIRED / REFRESH_REQUIRED).
- `category_gated:` `[FUND_MANDATE]`
- `periodic_review_cadence:` `P2Y` base / HIGH `P1Y` / LOW `P3Y`

**B.** 6 states: NOT_STARTED (entry), IN_PROGRESS, APPROVED, REJECTED (terminal), EXPIRED, REFRESH_REQUIRED.

**C.**
| From | To | Verb |
|---|---|---|
| NOT_STARTED | IN_PROGRESS | `investor.start-kyc` |
| IN_PROGRESS | APPROVED | `investor.approve-kyc` |
| IN_PROGRESS | REJECTED | `investor.reject-kyc` |
| APPROVED | EXPIRED | (backend: kyc_expires_at trigger) |
| APPROVED | REFRESH_REQUIRED | `investor.request-documents` |
| (EXPIRED, REFRESH_REQUIRED) | IN_PROGRESS | `investor.start-kyc` |

**D.** Source side of cross-slot constraint `investor_active_requires_kyc_approved`.

**E.** Carrier `"ob-poc".investors`. State column `kyc_status`. Substrate audit §0.3 CBU row notes `kyc_status` schema-backed.

**F.** `investor.start-kyc`, `investor.approve-kyc`, `investor.reject-kyc`, `investor.request-documents`. Plus 1 backend (kyc_expires_at).

**G–J.** DAG header.

#### M-038 — cbu.holding

**A.** cbu · `cbu_dag.yaml` (lines 650–719) · `holding_lifecycle` · "Per-holding lifecycle. 7 states (schema has 4; v1.3 extends with RESTRICTED / PLEDGED / FROZEN per CBU findings G-8 for encumbered-holdings semantics — legal locks, collateral pledges, sanctions freezes). Schema migration deferred (D-2)."
- Owner: ops
- Expected lifetime: long_lived
- `category_gated:` `[FUND_MANDATE]`

**B.** 7 states: PENDING (entry), ACTIVE, SUSPENDED, RESTRICTED, PLEDGED, FROZEN, CLOSED (terminal).

**C.**
| From | To | Verb (with args) |
|---|---|---|
| PENDING | ACTIVE | `holding.update-status` (status=ACTIVE) |
| ACTIVE | SUSPENDED | `holding.update-status` (status=SUSPENDED) |
| SUSPENDED | ACTIVE | `holding.update-status` (status=ACTIVE) |
| ACTIVE | RESTRICTED | `holding.restrict` |
| RESTRICTED | ACTIVE | `holding.lift-restriction` |
| ACTIVE | PLEDGED | `holding.pledge` |
| PLEDGED | ACTIVE | `holding.release-pledge` |
| {ACTIVE, SUSPENDED, RESTRICTED, PLEDGED} | FROZEN | (backend: sanctions hit) |
| FROZEN | ACTIVE | (backend: sanctions cleared) |
| {ACTIVE, SUSPENDED, PENDING, RESTRICTED} | CLOSED | `holding.close` |

**D.** Cited in cross-slot constraints `holding_active_requires_investor_active`, `holding_suspended_cascades_from_investor` (informational), `cbu_update_pending_proof_blocks_new_subscriptions`.

**E.** Carrier `"ob-poc".holdings`. State column `holding_status`. Substrate audit §0.3 CBU row notes `holding_status` schema-backed; DAG comment notes "schema has 4; v1.3 extends with RESTRICTED / PLEDGED / FROZEN" — schema migration deferred (D-2).
- DAG ∖ CHECK = {RESTRICTED, PLEDGED, FROZEN} per DAG comment (3 states not yet in CHECK). Substrate audit does not separately list a finding for this; folded into general "DAG-first per D-2" pattern.

**F.** `holding.update-status`, `holding.restrict`, `holding.lift-restriction`, `holding.pledge`, `holding.release-pledge`, `holding.close`. Plus 2 backend (sanctions).

**G–J.** DAG header. **Possible Section 2 anomaly:** 3 states (RESTRICTED, PLEDGED, FROZEN) declared in DAG but not in current CHECK per DAG comment.

#### M-039 — cbu.service_consumption

**A.** cbu · `cbu_dag.yaml` (lines 736–783) · `service_consumption_lifecycle` · "Per-(cbu, service_kind) provisioning lifecycle. Tracks whether this CBU has this service turned on, being provisioned, suspended, winding down, or retired. service_kind categorical: CUSTODY, TA, FA, SEC_LENDING, FX, TRADING, REPORTING, PRICING, COLLATERAL."
- Owner: ops
- Expected lifetime: long_lived
- Source entity `"ob-poc".cbu_service_consumption` — DAG-first; **table missing** (substrate audit S-1 BLOCKING).

**B.** 6 states: proposed (entry), provisioned, active, suspended, winding_down, retired (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| proposed | provisioned | `service-consumption.provision` |
| provisioned | active | `service-consumption.activate` |
| active | suspended | `service-consumption.suspend` |
| suspended | active | `service-consumption.reinstate` |
| (active, suspended) | winding_down | `service-consumption.begin-winddown` |
| winding_down | retired | `service-consumption.retire` |

**D.** Cross-workspace constraints involving this state machine (3 source-side gates, declared in this DAG):
1. `service_consumption_requires_active_service` (R2): provisioned transition requires product_maintenance.service in `lifecycle_status = active` (M-002).
2. `service_consumption_requires_deal_contracted` (R4): proposed → provisioned requires deal.deal in [CONTRACTED, ONBOARDING, ACTIVE] for same client_group (M-045/M-046).
3. `service_consumption_active_requires_live_binding` (post-R7e): provisioned → active requires capability_binding LIVE (M-006) on application_instance ACTIVE (M-005) for the same (cbu, service) pair.

Also a contributor to `cbu_operationally_active` derived aggregate (M-031).

**E.** Carrier table **missing** (substrate audit S-1 BLOCKING). State column `status` declared in DAG; no schema column. DAG ∖ CHECK = all 6 (none materialised). Substrate audit S-25 flags semantic overlap with `service_intents` (M-026 — 3-state) — both describe per-(cbu, service) consumption; resolution flagged when S-1 lands.

**F.** `service-consumption.provision`, `service-consumption.activate`, `service-consumption.suspend`, `service-consumption.reinstate`, `service-consumption.begin-winddown`, `service-consumption.retire`.

**G–J.** Substrate audit S-1 BLOCKING (highest pre-R.1 remediation priority). Verb files exist (substrate audit §1.9 confirms 6 verbs in `service-consumption.*` family). DAG header.

#### M-040 — cbu.cbu_corporate_action

**A.** cbu · `cbu_dag.yaml` (lines 798–855) · `cbu_corporate_action_lifecycle` · "Per-event lifecycle for CBU-level corporate actions. Types: rename, redomiciliation (change of jurisdiction), merger (with another CBU), conversion (fund type change), restructuring (master-feeder reorg). Each event has an effective_date; state machine governs pre-effective approval + post-effective implementation."
- Owner: compliance
- Expected lifetime: long_lived (DAG comment notes possibly more accurate as ephemeral; kept long_lived + exempt).
- `suspended_state_exempt: true`
- Source `"ob-poc".cbu_corporate_action_events` — DAG-first; **table missing** (substrate audit S-4 SIGNIFICANT).

**B.** 7 states: proposed (entry), under_review, approved, effective, implemented (terminal), rejected (terminal), withdrawn (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| proposed | under_review | `cbu-ca.submit-for-review` |
| under_review | approved | `cbu-ca.approve` |
| under_review | rejected | `cbu-ca.reject` |
| {proposed, under_review} | withdrawn | `cbu-ca.withdraw` |
| approved | effective | (backend: effective_date reached) |
| effective | implemented | `cbu-ca.mark-implemented` |

**D.** none.

**E.** Carrier table **missing**. State column `ca_status` declared in DAG; no schema column. Substrate audit S-4. DAG ∖ CHECK = all 7.

**F.** `cbu-ca.submit-for-review`, `cbu-ca.approve`, `cbu-ca.reject`, `cbu-ca.withdraw`, `cbu-ca.mark-implemented`. Plus 1 backend (effective_date).

**G–J.** Substrate audit S-4 SIGNIFICANT. DAG header. Substrate audit §0.5 sample lists 5 `cbu-ca.*` verbs.

#### M-041 — cbu.cbu_disposition

**A.** cbu · `cbu_dag.yaml` (lines 866–925) · `cbu_disposition_lifecycle` · "CBU administrative-disposition state, orthogonal to operational lifecycle. Covers: active (normal); under_remediation — post-breach enhanced monitoring (distinct from operational_lifecycle.suspended which is OPERATIONAL pause; this is COMPLIANCE / AUDIT state); soft_deleted (cbus.deleted_at IS NOT NULL; restorable); hard_deleted — terminal."
- Owner: compliance
- Expected lifetime: long_lived
- `suspended_state_exempt: true`
- Source `"ob-poc".cbus` (uses `deleted_at` + new `disposition_status` column — **column missing**, substrate audit S-11).

**B.** 4 states: active (entry), under_remediation, soft_deleted, hard_deleted (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| active | under_remediation | `cbu.flag-for-remediation` |
| under_remediation | active | `cbu.clear-remediation` |
| active | soft_deleted | `cbu.soft-delete` |
| soft_deleted | active | `cbu.restore` |
| soft_deleted | hard_deleted | `cbu.hard-delete` (requires explicit authorisation + retention-window check) |

**D.** none.

**E.** Carrier `"ob-poc".cbus` (with new `disposition_status` column missing per S-11). DAG declares 4 states; substrate audit notes `cbus.deleted_at` exists for soft_deleted but `disposition_status` not yet added. DAG ∩ CHECK = {soft_deleted via deleted_at}; DAG ∖ CHECK = {active explicit, under_remediation, hard_deleted}.

**F.** `cbu.flag-for-remediation`, `cbu.clear-remediation`, `cbu.soft-delete`, `cbu.restore`, `cbu.hard-delete`.

**G–J.** Substrate audit S-11 MINOR. DAG header.

#### M-042 — cbu.client_group_entity_review

**A.** cbu · `cbu_dag.yaml` (lines 931–965) · `client_group_entity_review_lifecycle` · "Per-(client_group, entity) link review state. 4 states from schema comment. Governs inclusion of an entity into a client group during onboarding research."

**B.** 4 states: pending (entry), confirmed, rejected, needs_update. No terminal_states (DAG comment: "confirmed is final for ops but can always return to needs_update").

**C.**
| From | To | Verb |
|---|---|---|
| pending | confirmed | (backend: client-group researcher review — cross-workspace verb from onboarding) |
| pending | rejected | (backend: client-group researcher review) |
| (confirmed, rejected) | needs_update | (backend: source data change trigger) |
| needs_update | pending | (backend: re-review required) |

**D.** none.

**E.** Carrier `"ob-poc".client_group_entity`. State column `review_status`. Substrate audit §0.3 lists `client_group_entity` in Core/Shared family.

**F.** 0 author-facing verbs — all 4 transitions are backend signals.

**G–J.** DAG header. **Possible Section 2 anomaly:** zero verbs declare `target_slot` for this; all transitions are backend-only. Whether the implementation honours the declared transitions is not audited here.

#### M-043 — cbu.share_class

**A.** cbu · `cbu_dag.yaml` (lines 1010–1068) · `share_class_lifecycle` · "Share-class subscription-availability lifecycle. 6 states capturing DRAFT (pre-launch) through to liquidation. A class typically lives in OPEN or SOFT_CLOSED for most of its life; HARD_CLOSED blocks redemptions too (rare — regulatory or wind-down)."
- Owner: ops+fund-admin
- Expected lifetime: long_lived
- `suspended_state_exempt: true`
- `category_gated:` `[FUND_MANDATE]`
- Source `"ob-poc".share_classes`. State column `lifecycle_status` — DAG-first; **column missing** (substrate audit S-10 MINOR).

**B.** 6 states: DRAFT (entry), OPEN, SOFT_CLOSED, HARD_CLOSED, WINDING_DOWN, LIQUIDATED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| DRAFT | OPEN | `share-class.launch` |
| OPEN | SOFT_CLOSED | `share-class.soft-close` |
| SOFT_CLOSED | OPEN | `share-class.reopen` |
| (OPEN, SOFT_CLOSED) | HARD_CLOSED | `share-class.hard-close` |
| HARD_CLOSED | SOFT_CLOSED | `share-class.lift-hard-close` |
| (OPEN, SOFT_CLOSED, HARD_CLOSED) | WINDING_DOWN | `share-class.begin-winddown` |
| WINDING_DOWN | LIQUIDATED | `share-class.close` |

**D.** none.

**E.** Substrate audit S-10. DAG ∖ CHECK = all 6 (column missing).

**F.** `share-class.launch`, `share-class.soft-close`, `share-class.reopen`, `share-class.hard-close`, `share-class.lift-hard-close`, `share-class.begin-winddown`, `share-class.close`.

**G–J.** Substrate audit S-10 MINOR. DAG header.

#### M-044 — cbu.manco

**A.** cbu · `cbu_dag.yaml` (lines 1074–1132) · `manco_lifecycle` · "Per-manco regulatory + operational state. A manco under regulatory action cascades SUSPENDED to all CBUs it manages (cross-slot constraint hooked via cbu_entity_roles where role = fund_manager / sub_manager). Reference-data-like in v1.2; R-6 promotes to stateful because regulatory-action cascade is real commercial risk (Credit Suisse 2022, FTX, etc.)."
- Owner: compliance
- Expected lifetime: long_lived
- Source `"ob-poc".manco_regulatory_status` (migration `20260425_manco_regulatory_status.sql`).

**B.** 6 states: UNDER_REVIEW (entry), APPROVED, UNDER_INVESTIGATION, SUSPENDED, SUNSET, TERMINATED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| UNDER_REVIEW | APPROVED | `manco.approve` |
| UNDER_REVIEW | TERMINATED | `manco.reject` |
| APPROVED | UNDER_INVESTIGATION | `manco.flag-regulatory` |
| UNDER_INVESTIGATION | APPROVED | `manco.clear-regulatory` |
| UNDER_INVESTIGATION | SUSPENDED | `manco.suspend` |
| SUSPENDED | UNDER_INVESTIGATION | `manco.partial-reinstate` |
| (APPROVED, UNDER_INVESTIGATION, SUSPENDED) | SUNSET | `manco.begin-sunset` |
| SUNSET | TERMINATED | `manco.terminate` |

**D.** Cascade rule (DAG header): manco SUSPENDED propagates SUSPENDED to all managed CBUs (via cbu_entity_roles where role = fund_manager / sub_manager). Modelled as semantic, not declared as `cross_workspace_constraints`.

**E.** Carrier `"ob-poc".manco_regulatory_status` (substrate audit §0.3 SemOsMaintenance row lists this; v1.2 §4.1 places under SemOsMaintenance, but DAG places it in CBU). State column `regulatory_status`.

**F.** `manco.approve`, `manco.reject`, `manco.flag-regulatory`, `manco.clear-regulatory`, `manco.suspend`, `manco.partial-reinstate`, `manco.begin-sunset`, `manco.terminate`.

**G–J.** Migration `20260425_manco_regulatory_status.sql` (cited in DAG header). DAG header.

---

### Workspace batch 1.9 — Deal

> Deal carries 9 stateful slots plus a dual_lifecycle on the primary `deal` slot. Substrate audit §0.3 row Deal: "mostly ✅" — primary commercial CHECK lacks BAC_APPROVAL (S-2 BLOCKING), LOST/REJECTED/WITHDRAWN granularity (S-8), and operational dual states (S-9). `deal_slas.sla_status` column missing (S-7).

#### M-045 — deal.deal (commercial primary)

**A.** deal · `deal_dag.yaml` (lines 263–349) · `deal_commercial_lifecycle` · "Commercial lifecycle (sales + BAC owned). 9 primary states: PROSPECT → QUALIFYING → NEGOTIATING → BAC_APPROVAL → KYC_CLEARANCE → CONTRACTED (junction to operational dual). Terminal-negative split: LOST / REJECTED / WITHDRAWN / CANCELLED (G-3 granularity)."
- Owner: sales+BAC
- Expected lifetime: long_lived
- `periodic_review_cadence:` `P1Y` base / HIGH `P6M` / LOW `P2Y`
- `internal_accountability:` (sponsor / rm / coverage_banker — G-8, schema deferred)

**B.** 10 states: PROSPECT (entry), QUALIFYING, NEGOTIATING, BAC_APPROVAL, KYC_CLEARANCE, CONTRACTED (junction; commercial-terminal not workspace-terminal), LOST (terminal), REJECTED (terminal), WITHDRAWN (terminal), CANCELLED (terminal).

**C.**
| From | To | Verb (precondition) |
|---|---|---|
| PROSPECT | QUALIFYING | `deal.create`, `deal.update-status` |
| QUALIFYING | NEGOTIATING | `deal.update-status`, `deal.create-rate-card` |
| NEGOTIATING | BAC_APPROVAL | `deal.submit-for-bac` (precondition: ≥1 deal_rate_card AGREED) |
| BAC_APPROVAL | KYC_CLEARANCE | `deal.bac-approve` |
| BAC_APPROVAL | REJECTED | `deal.bac-reject` |
| KYC_CLEARANCE | CONTRACTED | `deal.update-status`, `deal.add-contract` (precondition: group KYC case = APPROVED — XW) |
| KYC_CLEARANCE | REJECTED | `deal.reject` (precondition: KYC failed) |
| {PROSPECT, QUALIFYING, NEGOTIATING, BAC_APPROVAL, KYC_CLEARANCE} | LOST | `deal.mark-lost` |
| {PROSPECT, QUALIFYING, NEGOTIATING, BAC_APPROVAL, KYC_CLEARANCE} | WITHDRAWN | `deal.mark-withdrawn` |
| {PROSPECT, QUALIFYING, NEGOTIATING, BAC_APPROVAL, KYC_CLEARANCE} | CANCELLED | `deal.cancel` |

**D.** Cross-workspace constraints involving this state machine (target side):
1. `deal_contracted_requires_kyc_approved` (this DAG): KYC_CLEARANCE → CONTRACTED requires kyc.kyc_case APPROVED for `cases.client_group_id = this_deal.primary_client_group_id`.
2. `deal_contracted_requires_bp_approved` (this DAG, R3.5): KYC_CLEARANCE → CONTRACTED requires every booking_principal_clearance attached to this deal to be APPROVED or ACTIVE (M-004).

Source side of:
- `service_consumption_requires_deal_contracted` (cbu_dag.yaml; CBU service_consumption proposed → provisioned requires deal in [CONTRACTED, ONBOARDING, ACTIVE]).
- `book_ready_requires_deal_contracted_gate` (book_setup_dag.yaml; advisory).
- Contributor to `cbu_operationally_active` aggregate (M-031).

Cross-slot constraints (intra-DAG): `deal_contracted_requires_agreed_rate_card`, `deal_active_requires_all_onboarding_complete`, `deal_active_requires_active_billing_profile`, `deal_offboarded_requires_all_billing_closed`, `deal_ubo_assessment_blocked_halts_deal`.

**E.** Carrier `"ob-poc".deals`. State column `deal_status`.
- Substrate audit §0.3 row Deal: "commercial 6/10 ✅" — current CHECK enumerates 9 states excluding BAC_APPROVAL, and CANCELLED is the only terminal-negative; granular LOST/REJECTED/WITHDRAWN not yet in CHECK.
- DAG ∖ CHECK (substrate audit S-2 BLOCKING + S-8 SIGNIFICANT): {BAC_APPROVAL, LOST, REJECTED, WITHDRAWN}.
- CHECK ∖ DAG: ∅ (current CHECK uses CANCELLED which is also in DAG; no extra states).

**F.** Verbs in DAG transitions: `deal.create`, `deal.update-status`, `deal.create-rate-card`, `deal.submit-for-bac`, `deal.bac-approve`, `deal.bac-reject`, `deal.add-contract`, `deal.reject`, `deal.mark-lost`, `deal.mark-withdrawn`, `deal.cancel`.

**G–J.** Substrate audit S-2 BLOCKING: 3 verbs (`deal.submit-for-bac`, `deal.bac-approve`, `deal.bac-reject`) will fail at runtime / R.1 will report constraint mismatch until CHECK extended. DAG header. R-5 amendments 2026-04-24.

#### M-046 — deal.deal (operational dual)

**A.** deal · `deal_dag.yaml` (lines 355–397) · `deal_operational_lifecycle` (dual_lifecycle on slot `deal`) · "Operational servicing lifecycle — ops-owned. Begins at CONTRACTED once commercial lifecycle completes. G-10 commercial-vs-operational duality captured here. Schema migration to add these states to deals.deal_status (or a separate deals.operational_status column) deferred to Tranche 3 (D-2)."
- Owner: ops
- junction_state_from_primary: CONTRACTED

**B.** 5 states: ONBOARDING (entry), ACTIVE, SUSPENDED (G-5), WINDING_DOWN, OFFBOARDED (terminal).

**C.**
| From | To | Verb (precondition) |
|---|---|---|
| ONBOARDING | ACTIVE | `deal.update-status` (precondition: ALL deal_onboarding_requests.request_status = COMPLETED) |
| ACTIVE | SUSPENDED | `deal.suspend` |
| SUSPENDED | ACTIVE | `deal.reinstate` |
| (ACTIVE, SUSPENDED) | WINDING_DOWN | `deal.begin-winding-down` |
| WINDING_DOWN | OFFBOARDED | `deal.update-status` |

**D.** Contributor (states ONBOARDING/ACTIVE) to `cbu_operationally_active` aggregate (M-031) via predicate `deals.deal_status IN [CONTRACTED, ONBOARDING, ACTIVE]`. (Note: predicate references commercial state plus operational ONBOARDING/ACTIVE — currently encoded as a single deal_status CHECK; with operational_status separation per S-9, the predicate would split.)

**E.** Substrate audit S-9 SIGNIFICANT: column `operational_status` missing in schema. Set comparison: DAG ∖ CHECK = all 5.

**F.** `deal.update-status`, `deal.suspend`, `deal.reinstate`, `deal.begin-winding-down`.

**G–J.** Substrate audit S-9. DAG header. R-5 G-10. Note: DAG also describes G-4 amendment dual_lifecycle (parallel chain for ACTIVE deals — scope extensions / repricing) but DAG comment says "v1.3 schema currently supports one junction. Second amendment chain is authored as a separate slot sibling until V1.3 is extended" — i.e. the amendment chain is not formally declared as a state machine here.

#### M-047 — deal.deal_product

**A.** deal · `deal_dag.yaml` (lines 413–447) · `deal_product_lifecycle` · "Per-(deal, product) commercial commitment lifecycle. 5 states from schema CHECK. A product enters the deal as PROPOSED, is haggled through NEGOTIATING, lands at AGREED or DECLINED, and can be REMOVED by retraction before contracting."

**B.** 5 states: PROPOSED (entry), NEGOTIATING, AGREED (terminal), DECLINED (terminal), REMOVED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| PROPOSED | NEGOTIATING | `deal.update-product-status` |
| NEGOTIATING | AGREED | `deal.update-product-status`, `deal.agree-rate-card` |
| (PROPOSED, NEGOTIATING) | DECLINED | `deal.update-product-status` |
| (PROPOSED, NEGOTIATING, AGREED) | REMOVED | `deal.remove-product` |

**D.** none directly; folded into deal commercial cross-slot constraints.

**E.** Carrier `"ob-poc".deal_products`. State column `product_status`. Substrate audit §0.3 schema-backed.

**F.** `deal.update-product-status`, `deal.agree-rate-card`, `deal.remove-product`.

**G–J.** DAG header.

#### M-048 — deal.deal_rate_card

**A.** deal · `deal_dag.yaml` (lines 449–524) · `deal_rate_card_lifecycle` · "Rate-card negotiation lifecycle. 8 states (was 6; R-5 G-2 adds PENDING_INTERNAL_APPROVAL + APPROVED_INTERNALLY for pricing-committee gate). Internal approval precedes PROPOSED whenever discount exceeds threshold, pricing model is bespoke, or the deal is in a new jurisdiction. Otherwise DRAFT → PROPOSED is direct. SUPERSEDED is db-triggered by idx_deal_rate_cards_one_agreed (migration 069). External commercial-commitment tier applies at AGREED transition (V1.3-7)."
- Owner: sales+pricing-committee

**B.** 8 states: DRAFT (entry), PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY, PROPOSED, COUNTER_PROPOSED, AGREED, SUPERSEDED (terminal), CANCELLED (terminal).

**C.**
| From | To | Verb (precondition) |
|---|---|---|
| DRAFT | PENDING_INTERNAL_APPROVAL | `deal.submit-for-pricing-approval` (precondition: discount > threshold OR bespoke_model OR new_jurisdiction) |
| PENDING_INTERNAL_APPROVAL | APPROVED_INTERNALLY | `deal.pricing-approve` |
| PENDING_INTERNAL_APPROVAL | DRAFT | `deal.pricing-reject` |
| APPROVED_INTERNALLY | PROPOSED | `deal.propose-rate-card` |
| DRAFT | PROPOSED | `deal.propose-rate-card` (precondition: discount within threshold AND standard_model) |
| PROPOSED | COUNTER_PROPOSED | `deal.counter-rate-card` |
| COUNTER_PROPOSED | PROPOSED | `deal.propose-rate-card` (re-propose after counter) |
| (PROPOSED, COUNTER_PROPOSED) | AGREED | `deal.agree-rate-card` |
| AGREED | SUPERSEDED | (backend: new AGREED for same (deal, contract, product)) |
| {DRAFT, PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY, PROPOSED, COUNTER_PROPOSED} | CANCELLED | (implicit: rate card removed or deal cancelled) |

**D.** Cited in cross-slot constraints `deal_contracted_requires_agreed_rate_card`, `rate_card_agreed_uniqueness` (DB-enforced via `idx_deal_rate_cards_one_agreed`, migration 069), `billing_profile_activation_requires_agreed_rate_card`, `counter_proposal_returns_agreed_to_counter` (informational).

**E.** Carrier `"ob-poc".deal_rate_cards`. State column `status`.
- Substrate audit §0.3 schema-backed.
- DAG ∖ CHECK: PENDING_INTERNAL_APPROVAL and APPROVED_INTERNALLY are R-5 G-2 additions; DAG header notes "Schema migrations deferred per D-2 (Tranche 3 window): Expand deal_rate_cards_status_check with PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY". So DAG ∖ CHECK = {PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY} — not separately catalogued in substrate audit's S-IDs but folded into general "schema-lag deferred per D-2".

**F.** `deal.submit-for-pricing-approval`, `deal.pricing-approve`, `deal.pricing-reject`, `deal.propose-rate-card`, `deal.counter-rate-card`, `deal.agree-rate-card`. Plus 2 backend (SUPERSEDED, CANCELLED implicit).

**G–J.** Migration 069 (idx_deal_rate_cards_one_agreed). DAG header. R-5 G-2.

#### M-049 — deal.deal_onboarding_request

**A.** deal · `deal_dag.yaml` (lines 526–562) · `deal_onboarding_request_lifecycle` · "Handoff to ops — a per-CBU onboarding request emitted when deal reaches CONTRACTED. 5 states from schema CHECK."

**B.** 5 states: PENDING (entry), IN_PROGRESS, BLOCKED, COMPLETED (terminal), CANCELLED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | IN_PROGRESS | `deal.update-onboarding-status` |
| IN_PROGRESS | COMPLETED | `deal.update-onboarding-status` |
| (PENDING, IN_PROGRESS) | BLOCKED | `deal.update-onboarding-status` |
| BLOCKED | IN_PROGRESS | `deal.update-onboarding-status` |
| (PENDING, IN_PROGRESS, BLOCKED) | CANCELLED | `deal.update-onboarding-status` |

**D.** Reconcile-existing target for OnboardingRequest workspace's overall_lifecycle. OnboardingRequest's two cross_workspace_constraints (`onboarding_request_requires_deal_contracted`, `onboarding_request_requires_cbu_validated`) target OnboardingRequest's `validating → submitted` pack-state transition, not this carrier — they gate the *creation* of a deal_onboarding_request row.

**E.** Carrier `"ob-poc".deal_onboarding_requests`. State column `request_status`. Substrate audit §0.3 schema-backed.

**F.** `deal.update-onboarding-status` (used 6 times — single verb covers all 5 transitions via status arg).

**G–J.** DAG header. Substrate audit §1.6 confirms `deal_onboarding_requests` is "the strongest single FK hub on the onboarding-path spine."

#### M-050 — deal.deal_document

**A.** deal · `deal_dag.yaml` (lines 564–604) · `deal_document_lifecycle` · "Legal / commercial artefact lifecycle. 6 states from schema CHECK. Covers contracts, term sheets, side letters, NDAs, rate schedules, SLAs, proposals, RFP responses, board approvals, legal opinions."

**B.** 6 states: DRAFT (entry), UNDER_REVIEW, SIGNED, EXECUTED (terminal), SUPERSEDED (terminal), ARCHIVED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| DRAFT | UNDER_REVIEW | `deal.update-document-status` |
| UNDER_REVIEW | SIGNED | `deal.update-document-status` |
| SIGNED | EXECUTED | `deal.update-document-status` |
| (EXECUTED, SIGNED) | SUPERSEDED | `deal.update-document-status` |
| (EXECUTED, SUPERSEDED) | ARCHIVED | `deal.update-document-status` |

**D.** none.

**E.** Carrier `"ob-poc".deal_documents`. State column `document_status`. Substrate audit §0.3 schema-backed.

**F.** `deal.update-document-status` (used 6 times).

**G–J.** DAG header.

#### M-051 — deal.deal_ubo_assessment

**A.** deal · `deal_dag.yaml` (lines 620–655) · `deal_ubo_assessment_lifecycle` · "Per-entity UBO/KYC risk assessment within a deal. 5 states from schema CHECK. Overlaps with KYC workspace's kyc_ubo_registry (v1.3 candidate: consolidate the two, they're epistemic states of the same underlying UBO determination)."

**B.** 5 states: PENDING (entry), IN_PROGRESS, COMPLETED (terminal), REQUIRES_EDD, BLOCKED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | IN_PROGRESS | `deal.update-ubo-assessment` |
| IN_PROGRESS | COMPLETED | `deal.update-ubo-assessment` |
| (IN_PROGRESS, COMPLETED) | REQUIRES_EDD | `deal.update-ubo-assessment` |
| (IN_PROGRESS, REQUIRES_EDD) | BLOCKED | `deal.update-ubo-assessment` |

**D.** Cited in cross-slot constraint `deal_ubo_assessment_blocked_halts_deal` (BLOCKED → deal cannot advance past KYC_CLEARANCE).

**E.** Carrier `"ob-poc".deal_ubo_assessments`. State column `assessment_status`. Substrate audit §0.3 schema-backed. **Semantic overlap** with kyc.kyc_ubo_registry (M-012) noted in DAG header — flagged as v1.3 consolidation candidate.

**F.** `deal.update-ubo-assessment` (used 5 times).

**G–J.** DAG header. Overlap with M-012.

#### M-052 — deal.deal_sla

**A.** deal · `deal_dag.yaml` (lines 805–862) · `deal_sla_lifecycle` · "Per-SLA commitment lifecycle. 6 states. SLAs negotiated as part of the deal; become active at deal.CONTRACTED; breaches trigger penalty calculations; remediation restores to active. sla_type (AVAILABILITY / TURNAROUND / ACCURACY / REPORTING) and penalty_type (FEE_REBATE / CREDIT / ESCALATION) remain categorical."
- Owner: ops
- Expected lifetime: long_lived
- `suspended_state_exempt: true`
- Source `"ob-poc".deal_slas`. State column `sla_status` — DAG-first; **column missing** (substrate audit S-7 SIGNIFICANT).
- R-5 G-7 promoted from stateless.

**B.** 6 states: NEGOTIATED (entry), ACTIVE, BREACHED, IN_REMEDIATION, RESOLVED, WAIVED. No declared `terminal_states` ("SLAs are continuously monitored").

**C.**
| From | To | Verb |
|---|---|---|
| NEGOTIATED | ACTIVE | (backend: deal.CONTRACTED) |
| ACTIVE | BREACHED | (backend: SLA measurement fails threshold) |
| BREACHED | IN_REMEDIATION | `deal.start-sla-remediation` |
| IN_REMEDIATION | RESOLVED | `deal.resolve-sla-breach` |
| RESOLVED | ACTIVE | (backend: remediation complete) |
| BREACHED | WAIVED | `deal.waive-sla-breach` |

**D.** none.

**E.** Substrate audit S-7. DAG ∖ CHECK = all 6 (column missing).

**F.** `deal.start-sla-remediation`, `deal.resolve-sla-breach`, `deal.waive-sla-breach`. Plus 3 backend (NEGOTIATED, BREACHED, RESOLVED→ACTIVE).

**G–J.** Substrate audit S-7 SIGNIFICANT. DAG header. R-5 G-7 promotion.

#### M-053 — deal.billing_profile

**A.** deal · `deal_dag.yaml` (lines 657–693) · `billing_profile_lifecycle` · "Fee billing profile lifecycle. 4 states from schema CHECK. Profile binds a deal + contract + rate card + CBU + product. Cannot activate without an AGREED rate card."
- Owner: ops+finance
- Expected lifetime: long_lived (SUSPENDED present)

**B.** 4 states: DRAFT (entry), ACTIVE, SUSPENDED, CLOSED (terminal).

**C.**
| From | To | Verb (precondition) |
|---|---|---|
| DRAFT | ACTIVE | `billing.activate-profile` (precondition: bound deal_rate_card.status = AGREED) |
| ACTIVE | SUSPENDED | `billing.suspend-profile` |
| SUSPENDED | ACTIVE | `billing.activate-profile` |
| (DRAFT, ACTIVE, SUSPENDED) | CLOSED | `billing.close-profile` |

**D.** Cited in cross-slot constraints `billing_profile_activation_requires_agreed_rate_card`, `billing_period_creation_requires_active_profile`, `deal_offboarded_requires_all_billing_closed`. Also gates `deal_active_requires_active_billing_profile`.

**E.** Carrier `"ob-poc".fee_billing_profiles`. State column `status`. Substrate audit §0.3 schema-backed.

**F.** `billing.activate-profile`, `billing.suspend-profile`, `billing.close-profile`.

**G–J.** DAG header.

#### M-054 — deal.billing_period

**A.** deal · `deal_dag.yaml` (lines 695–738) · `billing_period_lifecycle` · "Per-period billing lifecycle. 6 states from schema CHECK. Runs in parallel to billing_profile (multiple periods per profile lifetime). INVOICED and DISPUTED are terminal per-period; dispute resolution is out-of-scope (Layer 3 operational)."

**B.** 6 states: PENDING (entry), CALCULATED, REVIEWED, APPROVED, INVOICED (terminal), DISPUTED (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| PENDING | CALCULATED | `billing.calculate-period` |
| CALCULATED | REVIEWED | `billing.review-period` |
| REVIEWED | APPROVED | `billing.approve-period` |
| APPROVED | INVOICED | `billing.generate-invoice` |
| (CALCULATED, REVIEWED, APPROVED, INVOICED) | DISPUTED | `billing.dispute-period` |

**D.** Cited in cross-slot constraint `billing_period_creation_requires_active_profile` (parent profile must be ACTIVE or SUSPENDED).

**E.** Carrier `"ob-poc".fee_billing_periods`. State column `calc_status`. Substrate audit §0.3 schema-backed.

**F.** `billing.calculate-period`, `billing.review-period`, `billing.approve-period`, `billing.generate-invoice`, `billing.dispute-period`.

**G–J.** DAG header.

---

### Workspace batch 1.10 — SemOsMaintenance

> Governance workspace. All 5 governance carriers (changesets, attribute_defs, derivation_specs, service_resource_defs, phrase_authoring) live in the `"sem_reg".*` schema and are fully materialised per substrate audit §0.3 row SemOsMaintenance: "✅ all states materialised". `attribute_def` carries a dual_lifecycle (External governed vs Internal operational, V1.3-5).

#### M-056 — semos_maintenance.changeset

**A.** semos_maintenance · `semos_maintenance_dag.yaml` (lines 221–272) · `changeset_lifecycle` · "Changeset authoring + review + publish lifecycle. 6 states (reconcile-existing from state_machines/changeset_lifecycle.yaml). The canonical governance ceremony: compose → submit → review → approve/reject → publish."
- Owner: stewards
- Expected lifetime: long_lived
- `suspended_state_exempt: true`

**B.** 6 states: composing (entry), submitted, reviewing, approved, published (terminal), rejected (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| composing | submitted | `changeset.submit` |
| submitted | reviewing | `changeset.enter-review` |
| reviewing | approved | `changeset.approve` |
| reviewing | rejected | `changeset.reject` |
| approved | published | `changeset.publish` |

**D.** Cited as integrator in cross-slot constraint `published_changeset_propagates_items` (when changeset → published, every referenced attribute_def, derivation_spec, service_resource_def must transition to active or appropriate published state atomically).

**E.** Carrier `"sem_reg".changesets`. State column `status`. Substrate audit §0.3 row SemOsMaintenance: "✅ all states materialised".

**F.** `changeset.submit`, `changeset.enter-review`, `changeset.approve`, `changeset.reject`, `changeset.publish`.

**G–J.** DAG header references CLAUDE.md §SemOS Maintenance workspace, `docs/annex-sem-os.md`, `docs/todo/catalogue-platform-refinement-v1_3.md`.

#### M-057 — semos_maintenance.attribute_def (external — primary)

**A.** semos_maintenance · `semos_maintenance_dag.yaml` (lines 289–331) · `attribute_def_lifecycle` · "Attribute-definition lifecycle. 5 states (reconcile-existing from state_machines/attribute_def_lifecycle.yaml). The primary (default) path is the EXTERNAL governed path — full ProposeValidateSignOffPublish ceremony. The INTERNAL operational path (dual_lifecycle) auto-approves without ceremony — used for internal/system attributes where governance is noise."
- Owner: stewards
- Expected lifetime: long_lived
- `suspended_state_exempt: true`
- `periodic_review_cadence:` `P1Y` base / HIGH `P6M`

**B.** 5 states: ungoverned (entry), draft, active, deprecated, retired (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| ungoverned | draft | `attribute.define` |
| draft | active | (backend: changeset.publish includes this attribute) |
| active | draft | `attribute.propose-revision` (V1.3-6 periodic-review re-entry) |
| active | deprecated | `attribute.deprecate` |
| deprecated | retired | `attribute.retire` |

**D.** Cited in cross-slot constraints:
- `published_changeset_propagates_items`.
- `derivation_active_requires_active_upstream` (derivation_spec.active requires upstream attribute_defs in active or deprecated).
- `retired_attribute_blocks_active_derivations` (attribute_def.retired blocked while active derivation_spec references it).
- `srdef_complete_requires_no_gaps`.

No `cross_workspace_constraints:` declared (DAG comment: "governance changes propagate asynchronously").

**E.** Carrier `"sem_reg".attribute_defs`. State column `lifecycle_status`. Substrate audit §0.3 schema-backed.

**F.** `attribute.define`, `attribute.propose-revision`, `attribute.deprecate`, `attribute.retire`. Plus 1 backend (changeset.publish promotion).

**G–J.** DAG header.

#### M-058 — semos_maintenance.attribute_def (internal — dual)

**A.** semos_maintenance · `semos_maintenance_dag.yaml` (lines 339–366) · `attribute_def_internal_lifecycle` (dual_lifecycle on slot `attribute_def`) · "Internal / operational attribute lifecycle. Auto-approved; no changeset ceremony. Used for system-level attributes where governance overhead would be noise (derivation intermediates, telemetry, internal diagnostics)."
- Owner: system
- junction_state_from_primary: ungoverned (starts at primary entry)

**B.** 3 states: auto_active (entry), auto_updated, auto_retired (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| auto_active | auto_updated | `attribute.update-internal` |
| auto_updated | auto_active | (backend: update applied) |
| (auto_active, auto_updated) | auto_retired | (backend: internal cleanup; no verb — infrastructure) |

**D.** none.

**E.** Carrier `"sem_reg".attribute_defs` (same table as M-057). DAG comment: distinguished by `AttributeVisibility::Internal` field. State column `lifecycle_status` shared with M-057. The two state machines write disjoint state values per visibility.

**F.** `attribute.update-internal`. Plus 2 backend.

**G–J.** DAG header. CLAUDE.md sections 117-130 reference two-tier attribute model and `attribute.define-internal` / `attribute.update-internal` verbs (introduced 2026-03-28, extended 2026-04-02).

#### M-059 — semos_maintenance.derivation_spec

**A.** semos_maintenance · `semos_maintenance_dag.yaml` (lines 372–408) · `derivation_spec_lifecycle` · "Derivation-spec lifecycle. 3 states (reconcile-existing from state_machines/derivation_spec_lifecycle.yaml). Derivation specs compute derived attributes from base attributes; STALE flags that an upstream attribute has changed and the derivation must be recomputed."
- Owner: stewards
- Expected lifetime: long_lived
- `suspended_state_exempt: true`

**B.** 3 states: draft (entry), active, stale. No declared terminal_states (re-activate from stale).

**C.**
| From | To | Verb |
|---|---|---|
| draft | active | `derivation.activate` |
| active | stale | (backend: upstream attribute version bump) |
| stale | active | `derivation.recompute-stale` |

**D.** Cited in cross-slot constraints `published_changeset_propagates_items`, `derivation_active_requires_active_upstream`, `retired_attribute_blocks_active_derivations`.

**E.** Carrier `"sem_reg".derivation_specs`. State column `lifecycle_status`.

**F.** `derivation.activate`, `derivation.recompute-stale`. Plus 1 backend.

**G–J.** DAG header.

#### M-060 — semos_maintenance.service_resource_def

**A.** semos_maintenance · `semos_maintenance_dag.yaml` (lines 414–455) · `service_resource_def_lifecycle` · "Service-resource-definition lifecycle. 4 states (reconcile-existing from state_machines/service_resource_def_lifecycle.yaml). SRDEFs define service-resource metadata; gaps_found means the SRDEF references attributes not declared in the registry."
- Owner: stewards
- Expected lifetime: long_lived
- `suspended_state_exempt: true`

**B.** 4 states: unsynced (entry), synced, gaps_found, complete (terminal).

**C.**
| From | To | Verb |
|---|---|---|
| unsynced | synced | `service-resource.sync-definitions` |
| synced | gaps_found | `service-resource.check-attribute-gaps` |
| gaps_found | synced | `service-resource.sync-definitions` (re-sync after filling gaps) |
| synced | complete | `service-resource.mark-complete` |
| gaps_found | complete | `service-resource.mark-complete` (force-complete — steward override) |

**D.** Cited in cross-slot constraints `published_changeset_propagates_items`, `srdef_complete_requires_no_gaps` (complete requires no referenced attributes in ungoverned or missing).

**E.** Carrier `"sem_reg".service_resource_defs`. State column `lifecycle_status`.

**F.** `service-resource.sync-definitions`, `service-resource.check-attribute-gaps`, `service-resource.mark-complete`.

**G–J.** DAG header.

#### M-061 — semos_maintenance.phrase_authoring

**A.** semos_maintenance · `semos_maintenance_dag.yaml` (lines 461–521) · `phrase_authoring_lifecycle` · "Governed phrase authoring lifecycle. 8 states (reconcile-existing from state_machines/phrase_authoring_lifecycle.yaml). Covers AI-proposed phrase mappings through collision/quality checks, human review, and publish. Deferred state for backlog mgmt."
- Owner: stewards
- Expected lifetime: long_lived
- `suspended_state_exempt: true`

**B.** 8 states: proposed (entry), collision_checked, quality_checked, reviewed, published (terminal), rejected (terminal), refined, deferred.

**C.**
| From | To | Verb |
|---|---|---|
| proposed | collision_checked | `phrase.check-collision` |
| collision_checked | quality_checked | `phrase.check-quality` |
| quality_checked | reviewed | `phrase.submit-for-review` |
| reviewed | published | `phrase.approve-proposal` |
| reviewed | rejected | `phrase.reject-proposal` |
| reviewed | refined | `phrase.request-refinement` |
| reviewed | deferred | `phrase.defer` |
| refined | proposed | `phrase.propose` |
| deferred | proposed | `phrase.reactivate` |

**D.** Cited in cross-slot constraint `published_phrase_requires_collision_clean` (phrase cannot be published if collision was found).

**E.** Carrier `"sem_reg".phrase_authoring`. State column `authoring_status`. Substrate audit §0.3 schema-backed; CLAUDE.md sections 137-145 reference 13,570 phrase_bank entries and the 8-state state machine.

**F.** All 9 transitions use `phrase.*` verbs (check-collision, check-quality, submit-for-review, approve-proposal, reject-proposal, request-refinement, defer, propose, reactivate).

**G–J.** DAG header. CLAUDE.md notes "9 phrase.* verbs, AI proposal pipeline with 5-signal confidence scoring + risk-tiered approval routing".

---

**Section 1 traversal complete: M-001 through M-061 inventoried.**

---

## Section 2 — Mechanical anomaly findings

> Anomalies detected from DAG content alone (states, transitions, declared cross-workspace constraints, declared carrier mappings) plus carrier evidence cross-referenced against substrate audit §0.3 / §1.12 / §5. Anomaly types §2.D (bypass writes), §2.F (implemented-but-undeclared), and §2.J (slot-dispatch gaps) require code-grep work and are recorded as deferred.

### 2.A — Orphan states (declared states no DSL verb writes to)

A state qualifies if **no transition in the DAG reaches it** and **no DSL verb is declared** as the `via` for that transition. (States reached only by backend signals are NOT counted as orphans here — those are intentional signal slots.) States only entered via backend signals are listed separately in §2.B.

| State machine | Orphan state | Evidence |
|---|---|---|
| M-036 cbu.investor | `REDEEMING` | declared in `states:` but no transition has `to: REDEEMING` and no transition has `from: REDEEMING`. State unreachable AND unleavable per DAG. |
| M-036 cbu.investor | `REDEEMED` | declared in `states:` but no transition has `to: REDEEMED` and no transition has `from: REDEEMED`. State unreachable AND unleavable per DAG. |

DAG comment for M-036 describes both states ("Redemption instruction placed; awaiting settlement" / "Units = 0; account still open. Pre-offboard state.") but the transition table in cbu_dag.yaml lines 562–579 omits any `(from, REDEEMING)` or `(*, REDEEMING)` / `(*, REDEEMED)` rows.

No other states across M-001..M-061 have zero in/out edges per DAG. (Backend-only entry states like M-019.in_progress, M-024.trading, M-029.default_applied are reachable via declared `via:` strings — backend signals — and are NOT orphans.)

### 2.B — Unreachable states (declared but no inbound transition)

Same set as §2.A for the unreachable side: M-036.REDEEMING and M-036.REDEEMED have no inbound transitions in the DAG.

Additional unreachable states from "(any non-terminal)" wildcard expansion would require expansion analysis; this inventory uses DAG declarative content only.

### 2.C — Unused transitions (transitions declared in DAG but no verb implements them)

Detection requires verb-YAML grep against each declared `(workspace, slot, from-state, to-state)`. Deferred. Substrate audit §4.2 reports zero hard verb-to-entity binding mismatches at the substrate level (193 cosmetic-naming and 12 intentional cross-workspace verbs noted, all classified as not-substrate-gaps). Verb-by-verb transition coverage analysis against this DAG inventory is the next pass.

### 2.D — Bypass writes (code paths writing state column directly)

Source grep across `rust/src/**/*.rs` and `rust/crates/sem_os_postgres/src/ops/*.rs` for SQL `UPDATE ... SET <state-col> =` patterns. Three categories surfaced:

**LIKELY-OK** (writes inside `SemOsVerbOp::execute` impls in `sem_os_postgres/src/ops/*.rs`):
- `billing.rs` — writes `fee_billing_profiles.status` (3 sites: ACTIVE, SUSPENDED, CLOSED) and `fee_billing_periods.calc_status` (2 sites: CALCULATING, DISPUTED). Each emits `emit_pending_state_advance(...)` after the UPDATE — DAG-aware dispatch confirmed.
- `deal.rs` — writes `deals.deal_status` (`UpdateStatus::execute`, multiple sites including ONBOARDING in `RequestOnboarding`/`RequestOnboardingBatch` and ACTIVE in `UpdateOnboardingStatus`). Writes are guarded by FSM validation + KYC gating + onboarding-completion checks.
- `deal.rs` — writes `deal_rate_cards.status` (SUPERSEDED, AGREED at `CounterOffer::execute` line 1302, `AgreeRateCard::execute` line 1361).
- `cbu.rs` — writes `cases.status` (`EvaluateCase::execute` line 1282, parameterised by decision logic: APPROVED/REJECTED/ESCALATE).
- `investor.rs` — writes `holdings.status` (lines 390, 440 — `CompleteRedemption::execute`, `Offboard::execute`, value `'closed'`). Note: holdings table uses `status`, not `holding_status` as the DAG declares for M-038. This is a likely **§2.G naming inconsistency** (carrier column vs DAG declaration).
- `access_review.rs` — writes `access_review_campaigns.status` (line 57). Not in this evidence base's state-machine catalogue (peripheral).
- `authoring.rs` — writes `sem_reg.changesets.status` (lines 122, 156 — `update_change_set_status()`, `mark_superseded()`). Registry operations, not verb-op layer, but invoked from `changeset.*` verbs.

**SUSPICIOUS** (direct writes outside SemOsVerbOp / verb framework):
- `rust/src/services/service_pipeline_service_impl.rs:165` — `intent_supersede()` async function writes `service_intents.status = 'superseded'` directly outside the verb-op framework. **State machine M-026 service_intent declares no `superseded` state** in its DAG enumeration (declared states are: active, suspended, cancelled; plus `additional_operations: service-intent.supersede` which "replaces intent with new row, immutable log-like"). The write target therefore appears to be a state value the DAG does not declare — a §3.B finding (CHECK ∖ DAG ≠ ∅) hidden inside service-layer code.
- `rust/src/mcp/handlers/learning_tools.rs` — writes `learning_candidates.status` (lines 335 'approved', 386 'applied', 432 'rejected') and `core.rs:1829` ('applied'). `learning_candidates` is NOT a DAG-declared carrier; these writes do not bypass any state machine in this evidence base, but they are direct UPDATEs from MCP handlers without `SemOsVerbOp` wrapping. Lower concern; flagged for completeness.

**BACKEND-SIGNAL** (DAG-declared `(backend: ...)` transitions):
- No matches found for the DAG-declared backend signals (M-005 health-check transitions PROVISIONED↔DEGRADED, M-011 time-decay VERIFIED→EXPIRED, M-038 sanctions-hit ACTIVE→FROZEN, etc.). Either implemented via DB triggers / async jobs not in `rust/src` (Layer 3 per DAG out_of_scope), or not yet implemented. Cannot distinguish from this grep alone.

**TEST-ONLY** (test harnesses; safe — listed for completeness):
- Direct UPDATEs on carrier state columns appear in `rust/tests/db_integration.rs`, `kyc_full_lifecycle.rs`, `deal_to_kyc_lifecycle.rs`, `custody_integration.rs`, `threshold_rfi_integration.rs`, `catalogue_workspace_lifecycle.rs`, `onboarding_harness.rs`, and `seed_allianz_onboarding.sql` — these are setup/verification UPDATEs, not production paths.

### 2.E — Declared-but-unenforced cross-workspace constraints

Constraints declared in DAG `cross_workspace_constraints:` blocks where the carrier required to evaluate the constraint at runtime is missing or column-collapsed.

| ID | Source state machine | Target state machine | Why unenforceable |
|---|---|---|---|
| `service_consumption_requires_active_service` | M-002 product_maintenance.service | M-039 cbu.service_consumption | Target carrier `"ob-poc".cbu_service_consumption` **missing** (substrate audit S-1 BLOCKING). Constraint cannot be evaluated at runtime — there are no rows to gate. |
| `service_consumption_requires_deal_contracted` | M-045 deal.deal | M-039 cbu.service_consumption | Same as above (target carrier missing). |
| `service_consumption_active_requires_live_binding` | M-006 lifecycle_resources.capability_binding | M-039 cbu.service_consumption | Same as above (target carrier missing). |
| `mandate_requires_validated_cbu` | M-031 cbu.cbu | M-023 instrument_matrix.trading_profile | Source carrier OK; target carrier OK. **Enforceable** — listed for completeness. |
| `cbu_validated_requires_kyc_case_approved` | M-007 kyc.kyc_case | M-031 cbu.cbu | Source OK; target OK. Enforceable. |
| `deal_contracted_requires_kyc_approved` | M-007 kyc.kyc_case | M-045 deal.deal | Source OK; target CHECK lacks BAC_APPROVAL/LOST/REJECTED/WITHDRAWN granularity (S-2 + S-8) — affects predicate composition, not the cross-workspace gate itself. |
| `deal_contracted_requires_bp_approved` | M-004 booking_principal.clearance | M-045 deal.deal | Source OK (migration 20260429_booking_principal_clearance.sql); target as above. |
| `book_cbus_scaffolded_requires_kyc_case_in_progress` | M-007 kyc.kyc_case | M-055 book_setup.book | Source OK; target carrier `"ob-poc".client_books` **missing** (S-5 SIGNIFICANT). Constraint cannot be evaluated — target rows do not exist. |
| `book_ready_requires_deal_contracted_gate` (advisory) | M-045 deal.deal | M-055 book_setup.book | Same target gap. Severity: warning. |
| `onboarding_request_requires_deal_contracted` | M-045 deal.deal | OnboardingRequest pack-state (no per-slot SM) | Source partially blocked by S-2 (deal CHECK gap); target is pack-state, not a schema-backed slot. |
| `onboarding_request_requires_cbu_validated` | M-031 cbu.cbu | OnboardingRequest pack-state | Source OK; target as above. |

### 2.F — Implemented-but-undeclared constraints

Deferred — requires code grep for runtime guards that are not declared as `cross_workspace_constraints:` in any DAG.

### 2.G — Naming inconsistencies

Substrate audit S-21 (cross-cutting): ~193 verbs declare `transition_args.target_workspace` in snake_case (e.g., `lifecycle_resources`) while their domain-prefix workspace classification reports PascalCase (e.g., `LifecycleResources`). Substrate audit §4.2.1 lists affected verb classes:
- `application-instance.*` (6 verbs) — claimed LifecycleResources / declared lifecycle_resources.
- `capability-binding.*` (5 verbs) — same.
- `attribute.deprecate` (1) — claimed SemOsMaintenance / declared semos_maintenance.
- `billing.*` (8 profile/period verbs) — claimed Deal / declared deal.
- `book.*` (4) — claimed BookSetup / declared book_setup.
- `booking-principal-clearance.*` (8) — claimed BookingPrincipal / declared booking_principal.
- `cbu-ca.*` (5) — claimed CBU / declared cbu.

Severity: cosmetic per substrate audit (runtime normalises). Functional impact: none.

Substrate audit S-22: ~15 verbs (notably `booking-principal-clearance.create`, several `cbu.assign-*`, `booking-principal.{create,update,retire}`) declare `target_workspace:` without `target_slot:`. Listed for declarative-completeness review.

### 2.H — Empty or thin DAGs

State machines declared with very few states/transitions, or workspaces with no per-slot state machines:

| Workspace / state machine | State count | Transition count | Note |
|---|---|---|---|
| SessionBootstrap | 0 SMs (overall_lifecycle 2-phase only) | 0 | DAG header: "smallest workspace in the estate ... 2 allowed verbs ... No entity aggregates, no state machines, no cross-workspace gates." Intent confirmed; flagged for business reviewer to confirm the thinness is correct. |
| OnboardingRequest | 0 SMs (overall_lifecycle 7-phase only; all slots reconcile-existing) | 0 | DAG header: "Pack profile: 6 verbs, all cross-workspace ... No new stateful slots — the primary operational entity (deal_onboarding_request) already lives in Deal DAG §2.2." Pack-wrapper, not a stateful workspace. |
| M-020 instrument_matrix.trading_profile_template | 2 | 1 | DAG header: "Simpler than the streetside lifecycle (Adam Q6: templates are either available or not)." Adam-confirmed thinness. |
| M-029 instrument_matrix.corporate_action_event | 3 | 2 | DAG header: "(Adam Q-B + pass-3 addendum narrowed to 3 states)." Adam-confirmed. |
| M-002 product_maintenance.service | 5 | 5 | Modelled after attribute_def_lifecycle. |
| M-003 product_maintenance.service_version | 5 | 4 | Standard authoring shape. |
| M-019 instrument_matrix.group | 5 | 6 | All 6 transitions backend; no author verbs. |
| M-042 cbu.client_group_entity_review | 4 | 4 | All 4 transitions backend; no author verbs. |
| M-059 semos_maintenance.derivation_spec | 3 | 3 | Adam DAG comment: governance ceremony minimal. |

### 2.I — Dual-lifecycle anomalies

Three slots declare a `dual_lifecycle:`:

| Slot | Primary SM | Dual SM | Primary materialised? | Dual materialised? |
|---|---|---|---|---|
| cbu.cbu | M-031 cbu_discovery_lifecycle (5 states) | M-032 cbu_operational_lifecycle (8 states) | yes — `cbus.status` CHECK 5/5 | **no** — `cbus.operational_status` column missing (S-3 SIGNIFICANT) |
| deal.deal | M-045 deal_commercial_lifecycle (10 states) | M-046 deal_operational_lifecycle (5 states) | partial — current CHECK lacks BAC_APPROVAL + 3 terminal-negative granularities (S-2 BLOCKING + S-8 SIGNIFICANT) | **no** — `deals.operational_status` column missing (S-9 SIGNIFICANT) |
| semos_maintenance.attribute_def | M-057 attribute_def_lifecycle external (5 states) | M-058 attribute_def_internal_lifecycle (3 states) | yes | yes (shared `lifecycle_status` column with disjoint state values; differentiated by `AttributeVisibility` field per CLAUDE.md sections 117-130) |

DAG header for deal_dag.yaml notes a fourth dual_lifecycle intent (G-4 amendment chain — parallel chain during ACTIVE deals for scope extensions / repricing) but is "left as a schema follow-up" because v1.3 schema currently supports one junction.

### 2.J — Slot-dispatch gaps

Deferred — requires code grep for `slot_state.rs` or equivalent runtime dispatch registrations to verify every DAG-declared slot has a runtime entry. Tractable but not done in this pass.

---



---

## Section 3 — DAG-vs-code consistency findings

> Substrate audit §4.2 reports zero hard verb-to-entity binding mismatches, 193 cosmetic naming mismatches (S-21), 15 declarative-completeness gaps (S-22), and 12 intentional cross-workspace transitions. The schema CHECK comparisons in Section 1.X.E across all 61 state machines surface every DAG ∖ CHECK gap. This section consolidates Section 1 findings into the §3.A–E categories from the brief; full transition-by-transition impl reconciliation requires deeper code-grep and is recorded as deferred.

### 3.A — Transitions where DAG and impl disagree

Deferred (full code-grep). Substrate audit §4.2.5: zero hard mismatches at the substrate level after excluding cosmetic/intentional classes. Per-transition impl validation pass not done.

### 3.B — States where DAG and CHECK disagree

Compiled from Section 1 carrier-evidence sub-sections (1.X.E) plus substrate audit §0.3 / §5.

| State machine | DAG states | CHECK status | DAG ∖ CHECK | CHECK ∖ DAG | Substrate ID |
|---|---|---|---|---|---|
| M-032 cbu.cbu (operational dual) | dormant, trade_permissioned, actively_trading, restricted, suspended, winding_down, offboarded, archived | column `operational_status` **missing** | all 8 | n/a | S-3 SIGNIFICANT |
| M-039 cbu.service_consumption | proposed, provisioned, active, suspended, winding_down, retired | table **missing** | all 6 | n/a | S-1 BLOCKING |
| M-040 cbu.cbu_corporate_action | proposed, under_review, approved, effective, implemented, rejected, withdrawn | table **missing** | all 7 | n/a | S-4 SIGNIFICANT |
| M-041 cbu.cbu_disposition | active, under_remediation, soft_deleted, hard_deleted | column `disposition_status` missing (uses `deleted_at` for soft_deleted) | active, under_remediation, hard_deleted | n/a | S-11 MINOR |
| M-043 cbu.share_class | DRAFT, OPEN, SOFT_CLOSED, HARD_CLOSED, WINDING_DOWN, LIQUIDATED | column `lifecycle_status` **missing** | all 6 | n/a | S-10 MINOR |
| M-038 cbu.holding | PENDING, ACTIVE, SUSPENDED, RESTRICTED, PLEDGED, FROZEN, CLOSED | CHECK has 4 (per DAG comment "schema has 4; v1.3 extends with RESTRICTED / PLEDGED / FROZEN") | RESTRICTED, PLEDGED, FROZEN | unknown — substrate audit does not separately list this gap | (folded into D-2 schema-lag pattern) |
| M-024 instrument_matrix.trading_activity | never_traded, trading, dormant, suspended | table **missing** | all 4 | n/a | S-6 SIGNIFICANT |
| M-021 instrument_matrix.settlement_pattern_template | draft, configured, reviewed, parallel_run, live, suspended, deactivated | column collapsed to boolean `is_active` | draft, configured, reviewed, parallel_run, suspended | n/a | S-12 SIGNIFICANT |
| M-022 instrument_matrix.trade_gateway | defined, enabled, active, suspended, retired | hybrid (document body + `is_active` boolean) | (encoded in document) | n/a | (intentional per Adam Q3) |
| M-025 instrument_matrix.service_resource | provisioned, activated, suspended, decommissioned | derived from `is_active` + `provisioning_strategy` | provisioned, suspended | n/a | S-13 MINOR |
| M-028 instrument_matrix.reconciliation | draft, active, suspended, retired | table **missing** | all 4 | n/a | S-14 MINOR |
| M-029 instrument_matrix.corporate_action_event | election_pending, elected, default_applied | table **missing** | all 3 | n/a | S-14 MINOR |
| M-030 instrument_matrix.collateral_management | configured, active, suspended, terminated | table **missing** | all 4 | n/a | S-14 MINOR |
| M-045 deal.deal (commercial) | PROSPECT, QUALIFYING, NEGOTIATING, BAC_APPROVAL, KYC_CLEARANCE, CONTRACTED, LOST, REJECTED, WITHDRAWN, CANCELLED | CHECK has 9 (no BAC_APPROVAL; only single CANCELLED for terminal-negative) | BAC_APPROVAL, LOST, REJECTED, WITHDRAWN | n/a | S-2 BLOCKING + S-8 SIGNIFICANT |
| M-046 deal.deal (operational dual) | ONBOARDING, ACTIVE, SUSPENDED, WINDING_DOWN, OFFBOARDED | column `operational_status` missing | all 5 | n/a | S-9 SIGNIFICANT |
| M-048 deal.deal_rate_card | DRAFT, PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY, PROPOSED, COUNTER_PROPOSED, AGREED, SUPERSEDED, CANCELLED | CHECK has 6 (per DAG header "Schema migrations deferred per D-2: Expand deal_rate_cards_status_check with PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY") | PENDING_INTERNAL_APPROVAL, APPROVED_INTERNALLY | n/a | (folded into D-2) |
| M-052 deal.deal_sla | NEGOTIATED, ACTIVE, BREACHED, IN_REMEDIATION, RESOLVED, WAIVED | column `sla_status` **missing** | all 6 | n/a | S-7 SIGNIFICANT |
| M-055 book_setup.book | proposed, structure_chosen, entities_provisioned, cbus_scaffolded, parties_assigned, mandates_defined, ready_for_deal, abandoned | table **missing** | all 8 | n/a | S-5 SIGNIFICANT |
| M-018 kyc.kyc_service_agreement | DRAFT, ACTIVE, SUSPENDED, TERMINATED | "schema today has no CHECK constraint" per DAG | unknown — likely all 4 not enforced | n/a | (DAG-comment-flagged) |

All other state machines (M-001 catalogue.proposal, M-002–M-003 product_maintenance.*, M-004 booking_principal.clearance, M-005–M-006 lifecycle_resources.*, M-007 to M-017 kyc.* (excluding M-018), M-019 instrument_matrix.group (state column shared with client_group), M-020 instrument_matrix.trading_profile_template (subset of M-023 CHECK), M-023 instrument_matrix.trading_profile, M-026 instrument_matrix.service_intent, M-027 instrument_matrix.delivery, M-031 cbu.cbu (discovery), M-033 cbu.entity_proper_person, M-034 cbu.entity_limited_company_ubo, M-035 cbu.cbu_evidence, M-036 cbu.investor (modulo §2.A REDEEMING/REDEEMED), M-037 cbu.investor_kyc, M-042 cbu.client_group_entity_review, M-044 cbu.manco, M-047 deal.deal_product, M-049 deal.deal_onboarding_request, M-050 deal.deal_document, M-051 deal.deal_ubo_assessment, M-053 deal.billing_profile, M-054 deal.billing_period, M-056–M-061 semos_maintenance.*) report CHECK alignment per substrate audit §0.3 (DAG ∩ CHECK = full state set; ∖ in either direction = ∅).

Substrate audit §0.3 closing note: "Where a CHECK constraint exists, its enumeration matches the DAG state list. The lag direction is **always DAG-ahead, never schema-ahead** — no schema state is undeclared in any DAG."

### 3.C — Cross-workspace constraints where DAG and runtime disagree

§2.E inventory above lists every cross_workspace_constraint and which carrier gaps prevent runtime evaluation. Three constraints (`service_consumption_requires_active_service`, `service_consumption_requires_deal_contracted`, `service_consumption_active_requires_live_binding`) cannot fire because target carrier `cbu_service_consumption` is missing (S-1). Two constraints (`book_cbus_scaffolded_requires_kyc_case_in_progress`, `book_ready_requires_deal_contracted_gate`) cannot fire because target carrier `client_books` is missing (S-5).

### 3.D — Verb declarations vs verb impls

Substrate audit §4.2.5 reported zero verbs at substrate-level mismatch. A more thorough scan of `rust/config/verbs/**/*.yaml` for `transition_args.target_workspace` declarations surfaces additional **declarative drift** findings the substrate audit's sampling did not catch. These are cases where the verb's declared `target_workspace` does not match the workspace where the slot is authored in the DAG taxonomies.

| Verb FQN family | Declared `target_workspace` | DAG-declared workspace for the slot | Likely correct workspace |
|---|---|---|---|
| `registry.investor.*` (9 verbs: create, activate, suspend, upgrade, downgrade, restrict, unrestrict, archive, reactivate) | `session_bootstrap` | cbu (M-036 investor_lifecycle in cbu_dag.yaml) | cbu |
| `registry.holding.*` (5 verbs: initiate-holding, confirm-holding, approve-holding, complete-holding, archive-holding) | `session_bootstrap` | cbu (M-038 holding_lifecycle in cbu_dag.yaml) | cbu |
| `registry.share-class.*` (6 verbs: deploy, update-nav, suspend-trading, resume-trading, retire, archive) | `session_bootstrap` | cbu (M-043 share_class_lifecycle in cbu_dag.yaml) | cbu |
| `screening.{initiate,escalate,resolve}` (3 verbs) | `instrument_matrix` | kyc (M-010 screening_lifecycle in kyc_dag.yaml) | kyc |
| `service.{draft,publish,retire,reactivate}` (4 verbs) | `instrument_matrix` | product_maintenance (M-002 service_lifecycle in product_service_taxonomy_dag.yaml) | product_maintenance |
| `manco-group.*` (7 verbs: setup, approve, activate, suspend, retire, exit-group, amend) | `instrument_matrix` (slot `manco_group`) | cbu (M-044 manco_lifecycle in cbu_dag.yaml — slot id `manco`) | cbu (and slot rename `manco_group` → `manco`) |
| `service-consumption.*` (6 verbs: create, accept, activate, suspend, resume, terminate) | `onboarding_request` (slot `service_consumption`) | cbu (M-039 service_consumption_lifecycle in cbu_dag.yaml) | cbu |
| `custody.trade-gateway.{open,initiate-mocking,go-live}` (3 verbs) | `onboarding_request` | instrument_matrix (M-022 trade_gateway_lifecycle) | instrument_matrix |
| `custody.settlement-chain.*` (8 verbs: propose, review, approve, activate, complete, suspend, reactivate, retire) | `cbu` (slot `settlement_chain`) | instrument_matrix (M-021 settlement_pattern_lifecycle in instrument_matrix_dag.yaml — slot id `settlement_pattern_template`) | instrument_matrix (and slot id reconciliation) |
| `corporate-action-event.{propose-event,accept}` (2 verbs) | `instrument_matrix` | instrument_matrix (M-029 corporate_action_event_lifecycle) | OK — matches |
| `cbu.{submit-for-review,validate,approve}` (3 verbs) | `cbu` (slot `cbu`) | cbu — but DAG transitions on M-031 declare verbs `cbu.submit-for-validation` and `cbu.decide`, not `submit-for-review`/`validate`/`approve` | verb-name reconciliation needed; or verbs are obsolete and DAG is canonical |
| `kyc.kyc-case.{create,approve,reject,submit-for-review}` (4 verbs) | `kyc` | kyc (M-007) — DAG declares `kyc-case.update-status`, `case.approve`, `case.reject`, `kyc-case.escalate`, `kyc-case.close`, `kyc-case.reopen`. Naming convention differs (`kyc.kyc-case.*` prefix vs DAG's `kyc-case.*` / `case.*`) | naming reconciliation needed |
| `kyc.evidence.{submit,approve}` (2 verbs) | `kyc` (slot `evidence`) | kyc — DAG transitions for M-011/M-013/M-015 reference `evidence.verify`, `evidence.reject`, `evidence.waive` (no `submit`, no `approve`) | verb-set reconciliation needed |
| `kyc.red-flag.{create,approve,reject,clear,escalate}` (5 verbs) | `kyc` | kyc (M-014) — DAG declares `red-flag.escalate`, `red-flag.resolve`, `red-flag.waive`, `red-flag.update-rating` (no `create`/`approve`/`reject`/`clear`) | verb-set reconciliation needed |
| `kyc.ubo-registry.{register-ubo,verify-ubo,retire-ubo,update-beneficial-interest}` (4 verbs) | `kyc` | kyc (M-012) — DAG declares `ubo-registry.verify`, `ubo-registry.approve`, `ubo-registry.reject`, `ubo-registry.expire`, `ubo-registry.discover` | verb-set reconciliation needed |
| `entity.archive` | `kyc` (slot `entity`) | kyc declares `entity_kyc` (M-008) but no `entity` slot; CBU declares `entity_proper_person` (M-033) and `entity_limited_company_ubo` (M-034) | unclear which slot |

These are **declarative drift findings** — verbs and DAG state machines that do not name the same target workspace/slot, or that do not name the same set of transition verbs. The drift is mechanical (a verb-yaml/DAG-yaml audit can detect it deterministically), not business — but for a business reviewer, it raises the structural question: which is canonical, the DAG or the verb declarations? The brief reserves that judgement to the reviewer.

> Caveat. This evidence is from a wide-scan agent pass over `rust/config/verbs/**/*.yaml`. Verb-by-verb file-level confirmation has not been done — some entries may be wrong-prefix YAML examples or stale verbs awaiting deprecation. The substrate audit S-21 (cosmetic naming) and S-22 (target_slot omission) would absorb part of this drift; the remainder warrants a dedicated reconciliation pass.

Substrate audit S-22: ~15 verbs declare `target_workspace` without `target_slot` (notably `booking-principal-clearance.create`, several `cbu.assign-*`, `booking-principal.{create,update,retire}`). Listed for declarative-completeness review.

### 3.E — Migration vs DAG sequence

Recent migrations cited in DAG headers (and visible in substrate audit §0.3):
- `20260424_tranche_2_3_dag_alignment.sql` — alignment work (CLAUDE.md notes).
- `20260425_manco_regulatory_status.sql` — adds `manco_regulatory_status` carrier (M-044).
- `20260427_lifecycle_resources_workspace.sql` — adds `application_instances` + `capability_bindings` (M-005, M-006).
- `20260428_service_lifecycle.sql` — adds `lifecycle_status` to `services` and `service_versions` (M-002, M-003).
- `20260429_booking_principal_clearance.sql` — adds `booking_principal_clearances` (M-004).

No migration / DAG-state divergence detected: every recent migration corresponds to a DAG state machine that the migration was written to materialise. The reverse direction (DAG-declared state machines without a corresponding migration) is the schema-lag pattern catalogued in §3.B (S-1, S-3, S-4, S-5, S-6, S-7, S-9, S-10, S-11, S-12, S-13, S-14).

---



---

## Section 4 — Onboarding-path coherence evidence

> The onboarding business path Deal → Tollgate → KYC/BAC/BookingPrincipal → OnboardingRequest → CBU → InstrumentMatrix → SubscribedProducts → Services → Resources traverses 8 phase boundaries. Substrate audit §6 already produced this analysis; this section restates each transition with the brief's required fields plus state-machine M-IDs.

### 4.1 — Deal → Tollgate

- Source state machine + state at boundary: M-045 deal.deal at `NEGOTIATING`.
- Target state machine + state at boundary: tollgate is a **pattern, not a first-class state machine** (substrate audit S-19) — no state machine listed; evaluations stored in `tollgate_evaluations.passed` (boolean) keyed to `case_id` or `workstream_id`.
- Bridging verb(s): `tollgate.evaluate`, `tollgate.override`.
- Cross-workspace constraint enforcing the transition: none — tollgate is verb-mediated, not gate-mediated. KYC tollgate evaluations correspond to KYC `screening_in_flight` overall_lifecycle phase (M-007 transitions DISCOVERY → ASSESSMENT → REVIEW).
- DAG declaration of the bridge: kyc_dag.yaml describes tollgate as stateless (`stateless: true; rationale: "Tollgate evaluations are pass/fail boolean ... no lifecycle"`).
- Code evidence of the bridge actually working: substrate audit §1.2 confirms verbs and FK `tollgate_evaluations.case_id → cases(case_id)` exist.
- Open questions: substrate audit S-19 flags as "Tollgate as pattern, not entity ... worth flagging" for explicit business acceptance. No state column on cases or workstreams persists "clearance status."

### 4.2 — Tollgate → KYC / BAC / BookingPrincipal (parallel gates)

Three parallel gates:

**KYC gate.**
- Source: M-007 kyc.kyc_case at `REVIEW` → `APPROVED`.
- Bridging verb: `case.approve`.
- Cross-workspace constraints downstream: `cbu_validated_requires_kyc_case_approved` (CBU), `deal_contracted_requires_kyc_approved` (Deal), `book_cbus_scaffolded_requires_kyc_case_in_progress` (BookSetup).
- Code evidence: full carrier alignment per substrate audit §0.3 KYC row.

**BAC gate.**
- Source: M-045 deal.deal at `BAC_APPROVAL` → `KYC_CLEARANCE`.
- Bridging verb: `deal.bac-approve`.
- Cross-workspace constraint enforcing the transition: none — BAC is intra-Deal only (state value, not first-class entity per substrate audit S-20).
- DAG declaration: deal_dag.yaml lines 322–331 add the BAC_APPROVAL state (R-5 G-1).
- Code evidence: **BLOCKING gap** — substrate audit S-2: `deals.deal_status` CHECK lacks `BAC_APPROVAL`. 3 verbs (`deal.submit-for-bac`, `deal.bac-approve`, `deal.bac-reject`) cannot persist state.
- Open questions: substrate audit S-2 BLOCKING; S-20 flags BAC-as-state-not-entity as an architectural choice.

**BookingPrincipal gate.**
- Source: M-004 booking_principal.clearance at `APPROVED` or `ACTIVE`.
- Bridging verb: `booking-principal-clearance.approve`, `booking-principal-clearance.activate`.
- Cross-workspace constraint enforcing transition: `deal_contracted_requires_bp_approved` (deal_dag.yaml lines 1023–1035).
- Code evidence: full carrier alignment (migration 20260429_booking_principal_clearance.sql).

### 4.3 — Compliance complete → OnboardingRequest

- Source state machine + state: M-045 deal.deal at `CONTRACTED`.
- Target state machine + state: M-049 deal.deal_onboarding_request at `PENDING` (entry).
- Bridging verb(s): `deal.request-onboarding`, `deal.request-onboarding-batch`.
- Cross-workspace constraints involving the bridge: 2 (declared in `onboarding_request_dag.yaml`):
  - `onboarding_request_requires_deal_contracted` (deal in CONTRACTED/ONBOARDING/ACTIVE).
  - `onboarding_request_requires_cbu_validated` (target CBU in VALIDATED).
- DAG declaration: `onboarding_request_dag.yaml` overall_lifecycle phase `submitted` (lines 88–98) explicitly maps `EXISTS deal_onboarding_request for this (deal, cbu, product) AND deal_onboarding_request.request_status = 'PENDING'`.
- Code evidence: substrate audit §1.6 confirms `deal_onboarding_requests` carrier exists with FKs to deal/cbu/product/kyc_case.
- Open questions: substrate audit S-15 — no FK from `deal_onboarding_requests` *forward* to `cbu_product_subscriptions` or `cbu_service_consumption`; the handoff "Deal commits → operational subscription created" is implicit.

### 4.4 — OnboardingRequest → CBU

- Source state machine + state: M-049 deal.deal_onboarding_request at `IN_PROGRESS`.
- Target state machine + state: M-031 cbu.cbu at `DISCOVERED → VALIDATION_PENDING → VALIDATED`.
- Bridging verb(s): `cbu.create`, `cbu.create-from-client-group`, `cbu.ensure`, `cbu.submit-for-validation`, `cbu.decide`.
- Cross-workspace constraint: `cbu_validated_requires_kyc_case_approved` (cbu_dag.yaml — gates VALIDATION_PENDING → VALIDATED on KYC APPROVED).
- DAG declaration: cbu_dag.yaml §2.1 cbu slot.
- Code evidence: discovery 5/5 ✅ per substrate audit §0.3.
- Open questions: substrate audit S-3 — `cbus.operational_status` column missing for post-VALIDATED operational states (M-032).

### 4.5 — CBU → InstrumentMatrix

- Source state machine + state: M-031 cbu.cbu at `VALIDATED`.
- Target state machine + state: M-023 instrument_matrix.trading_profile at `DRAFT → SUBMITTED → APPROVED → PARALLEL_RUN → ACTIVE`.
- Bridging verb(s): `trading-profile.create`, `trading-profile.submit`, `trading-profile.approve`, `trading-profile.enter-parallel-run`, `trading-profile.go-live`.
- Cross-workspace constraint: `mandate_requires_validated_cbu` (instrument_matrix_dag.yaml — gates trading_profile DRAFT → SUBMITTED on cbu VALIDATED).
- DAG declaration: instrument_matrix_dag.yaml §3b cross_workspace_constraints.
- Code evidence: trading_profile 9/9 ✅ per substrate audit §0.3.
- Open questions: substrate audit S-6 — `cbu_trading_activity` table missing (M-024) blocks the actively_trading signal that feeds `cbu_operationally_active`.

### 4.6 — CBU → SubscribedProducts

- Source state machine + state: M-031 cbu.cbu at `VALIDATED` AND M-045 deal.deal at `[CONTRACTED, ONBOARDING, ACTIVE]`.
- Target state machine + state: M-039 cbu.service_consumption at `proposed → provisioned → active` (per (cbu, service_kind) row).
- Bridging verb(s): `cbu.add-product`, `service-consumption.provision`, `service-consumption.activate`.
- Cross-workspace constraints: 3, all targeting M-039 (declared in cbu_dag.yaml):
  - `service_consumption_requires_active_service` (M-002 source).
  - `service_consumption_requires_deal_contracted` (M-045/M-046 source).
  - `service_consumption_active_requires_live_binding` (M-006 source).
- DAG declaration: cbu_dag.yaml §2.5b service_consumption slot.
- Code evidence: **BLOCKING gap** — substrate audit S-1: `cbu_service_consumption` table missing. 6 verbs in `service-consumption.*` family cannot persist state. Adjacent table `service_intents` (M-026) carries 3-state lifecycle; the 6-state per-(cbu, service_kind) lifecycle has no carrier.
- Open questions: substrate audit S-25 — semantic overlap with `service_intents` (M-026); resolution flagged when S-1 lands.

### 4.7 — SubscribedProducts → Services

- Source state machine + state: M-039 cbu.service_consumption at `provisioned`.
- Target state machine + state: M-002 product_maintenance.service at `active` (read-only catalogue lookup).
- Bridging verb(s): `service.list`, `service.read`, `service-version.publish` (catalogue authoring); cross-workspace consumption is implicit.
- Cross-workspace constraint enforcing the transition: `service_consumption_requires_active_service` (already listed in 4.6).
- DAG declaration: product_service_taxonomy_dag.yaml §2 service slot (R2 2026-04-26).
- Code evidence: full carrier alignment per substrate audit §0.3.
- Open questions: none — 5 states fully materialised.

### 4.8 — Services → Resources

- Source state machine + state: M-002 product_maintenance.service at `active`.
- Target state machine + state: M-006 lifecycle_resources.capability_binding at `LIVE` AND M-005 lifecycle_resources.application_instance at `ACTIVE`.
- Bridging verb(s): `application-instance.activate`, `capability-binding.start-pilot`, `capability-binding.promote-live`.
- Cross-workspace constraint: `service_consumption_active_requires_live_binding` (cbu_dag.yaml — gates cbu.service_consumption provisioned → active on capability_binding LIVE on application_instance ACTIVE).
- DAG declaration: lifecycle_resources_dag.yaml §2.3 + §2.4 + cbu_dag.yaml §4.
- Code evidence: full carrier alignment per substrate audit §0.3 (migration 2026-04-27).
- Open questions: substrate audit S-16 — `cbu_resource_instances` lacks lifecycle column; service-readiness is derived rather than persisted on the binding-anchor row.

### 4.9 — Path verdict (substrate-level)

Per substrate audit §6: "The onboarding journey is **conceptually expressible** end-to-end (every transition has a DAG, every transition has a verb) but **not architecturally executable** end-to-end without three blocking gaps closed: S-1 (cbu_service_consumption carrier missing), S-2 (deals.deal_status BAC_APPROVAL not in CHECK), S-3 (cbus.operational_status column missing). After these three migrations, the path is fully traversable."

---



---

## Section 5 — Cross-workspace constraint inventory

> Flat catalogue of every `cross_workspace_constraints:` entry declared in any DAG, plus the source / target / runtime-enforcement evidence per the brief. Eleven constraints declared across 5 DAGs (catalogue, session_bootstrap, product_service_taxonomy, booking_principal, lifecycle_resources, kyc, semos_maintenance declare none).

| # | Constraint name | Source workspace.slot.state | Target workspace.slot.transition | Source predicate | Severity | Carrier-completeness for runtime enforcement | Verbs gated | DAG file (line range) |
|---|---|---|---|---|---|---|---|---|
| 1 | `onboarding_request_requires_deal_contracted` | deal.deal in [CONTRACTED, ONBOARDING, ACTIVE] | onboarding_request.onboarding_request `validating → submitted` (pack-state) | `deals.deal_id = this_request.deal_id` | error | source: M-045 partial CHECK (S-2 BLOCKING affects predicate composition); target: pack-state (no schema slot) | `deal.request-onboarding`, `deal.request-onboarding-batch` | onboarding_request_dag.yaml (192–203) |
| 2 | `onboarding_request_requires_cbu_validated` | cbu.cbu = VALIDATED | onboarding_request.onboarding_request `validating → submitted` (pack-state) | `cbus.cbu_id = this_request.cbu_id` | error | source: M-031 ✅; target: pack-state | (same as #1) | onboarding_request_dag.yaml (205–214) |
| 3 | `cbu_validated_requires_kyc_case_approved` | kyc.kyc_case = APPROVED | cbu.cbu `VALIDATION_PENDING → VALIDATED` | `cases.sponsor_cbu_id = this_cbu.cbu_id OR cases.client_group_id = this_cbu.primary_client_group_id` | error | source: M-007 ✅; target: M-031 ✅ — **fully enforceable** | `cbu.decide` (decision: APPROVE) | cbu_dag.yaml (1268–1279) |
| 4 | `service_consumption_requires_active_service` | product_maintenance.service = active | cbu.service_consumption `proposed → provisioned` | `services.service_id = this_consumption.service_id` | error | source: M-002 ✅; target: M-039 **carrier missing (S-1 BLOCKING)** | `service-consumption.provision` | cbu_dag.yaml (1286–1299) |
| 5 | `service_consumption_requires_deal_contracted` | deal.deal in [CONTRACTED, ONBOARDING, ACTIVE] | cbu.service_consumption `proposed → provisioned` | `deals.primary_client_group_id = this_consumption_cbu.client_group_id` | error | source: M-045 partial (S-2); target: M-039 missing (S-1) | `service-consumption.provision` | cbu_dag.yaml (1307–1320) |
| 6 | `service_consumption_active_requires_live_binding` | lifecycle_resources.capability_binding = LIVE (with parent application_instance ACTIVE) | cbu.service_consumption `provisioned → active` | `capability_bindings.service_id = this_consumption.service_id AND EXISTS (application_instances WHERE id = capability_bindings.application_instance_id AND lifecycle_status = 'ACTIVE')` | error | source: M-006 + M-005 ✅; target: M-039 missing (S-1) | `service-consumption.activate` | cbu_dag.yaml (1331–1346) |
| 7 | `mandate_requires_validated_cbu` | cbu.cbu = VALIDATED | instrument_matrix.trading_profile `DRAFT → SUBMITTED` | (predicate via `cbu_trading_profiles.cbu_id`) | error | source: M-031 ✅; target: M-023 ✅ — **fully enforceable** | `trading-profile.submit` | instrument_matrix_dag.yaml (1133–1141) |
| 8 | `deal_contracted_requires_kyc_approved` | kyc.kyc_case = APPROVED | deal.deal `KYC_CLEARANCE → CONTRACTED` | `cases.client_group_id = this_deal.primary_client_group_id` | error | source: M-007 ✅; target: M-045 partial (S-2 BLOCKING) | `deal.update-status`, `deal.add-contract` | deal_dag.yaml (1004–1015) |
| 9 | `deal_contracted_requires_bp_approved` | booking_principal.clearance in [APPROVED, ACTIVE] | deal.deal `KYC_CLEARANCE → CONTRACTED` | `booking_principal_clearances.deal_id = this_deal.deal_id` | error | source: M-004 ✅; target: M-045 partial (S-2) | `deal.update-status`, `deal.add-contract` | deal_dag.yaml (1023–1035) |
| 10 | `book_cbus_scaffolded_requires_kyc_case_in_progress` | kyc.kyc_case in [DISCOVERY, ASSESSMENT, REVIEW, APPROVED] | book_setup.book `entities_provisioned → cbus_scaffolded` | `cases.client_group_id = this_book.client_group_id` | error | source: M-007 ✅; target: M-055 **carrier missing (S-5 SIGNIFICANT)** | `cbu.create` | book_setup_dag.yaml (365–376) |
| 11 | `book_ready_requires_deal_contracted_gate` (advisory) | deal.deal in [KYC_CLEARANCE, CONTRACTED, ONBOARDING, ACTIVE] | book_setup.book `mandates_defined → ready_for_deal` | `deals.primary_client_group_id = this_book.client_group_id` | warning | source: M-045 partial (S-2); target: M-055 missing (S-5) | `book.mark-ready` | book_setup_dag.yaml (378–391) |

### 5.1 — Constraint origin patterns

- **3 constraints** target `cbu.service_consumption` (M-039) — the densest gating cluster, blocked by S-1 BLOCKING (carrier missing).
- **2 constraints** target `book_setup.book` (M-055) — blocked by S-5 SIGNIFICANT (carrier missing).
- **2 constraints** target `deal.deal KYC_CLEARANCE → CONTRACTED` — fully sourced, partially target-blocked by S-2 (BAC_APPROVAL CHECK gap).
- **2 constraints** target OnboardingRequest pack-state — pack-level not state-machine-level enforcement.
- **2 constraints** are fully enforceable (#3 and #7) — KYC → CBU and CBU → IM trading_profile gates.

### 5.2 — Constraint origin DAGs

- **cbu_dag.yaml**: 4 constraints (#3 inbound from KYC; #4, #5, #6 outbound from CBU into other source workspaces but targeting CBU's own service_consumption slot).
- **deal_dag.yaml**: 2 constraints (#8 inbound from KYC; #9 inbound from BookingPrincipal).
- **book_setup_dag.yaml**: 2 constraints (#10 inbound from KYC; #11 inbound from Deal).
- **onboarding_request_dag.yaml**: 2 constraints (#1 inbound from Deal; #2 inbound from CBU).
- **instrument_matrix_dag.yaml**: 1 constraint (#7 inbound from CBU).

DAGs declaring zero `cross_workspace_constraints:`: catalogue_dag, session_bootstrap_dag, product_service_taxonomy_dag, booking_principal_dag, lifecycle_resources_dag (deferred to R4), kyc_dag, semos_maintenance_dag.

> Note on direction. The brief asks "source: this state machine constrains another" vs "target: another state machine gates this one." Per v1.3-1 Mode A semantics, the *source* state machine publishes a state, and the *target* state machine's transition is gated on it. KYC publishes APPROVED; downstream CBU/Deal/BookSetup transitions are gated on it. CBU publishes VALIDATED; downstream IM/OnboardingRequest gated on it. The source side of these constraints is *passive* (publishes state); the target side is *active* (must check before allowing transition).

### 5.3 — Cross-DAG references that are NOT cross_workspace_constraints

Several semantic dependencies cross DAG boundaries but are not declared as `cross_workspace_constraints:`:

- M-044 cbu.manco SUSPENDED cascade to managed CBUs (DAG header describes the cascade; modelled as semantic, not declared as constraint).
- M-031 cbu.cbu hosts `cbu_operationally_active` derived_cross_workspace_state (V1.3-2 Mode B aggregation/tollgate, NOT Mode A blocking) — aggregates KYC, Deal, IM trading_profile, cbu_evidence, service_consumption. Distinct from the cross_workspace_constraints catalogued above.
- KYC's `ubo-registry.promote-to-ubo` writes M-034 cbu.entity_limited_company_ubo `MANUAL_REQUIRED → DISCOVERED` — verb-mediated, not gate-mediated.
- M-018 kyc.kyc_service_agreement gated by `product.kyc_service` (product-module gate, not cross-workspace).
- M-051 deal.deal_ubo_assessment overlaps semantically with M-012 kyc.kyc_ubo_registry (DAG header v1.3 candidate: consolidate).
- All overall_lifecycle aggregates that read foreign-workspace states for derivation purposes are not constraint-enforced.

---



---

## Section 6 — Output summary for the business reviewer

### 6.A — Inventory totals

| Metric | Count |
|---|---|
| DAG taxonomies | 12 |
| State machines (M-001..M-061) | 61 |
| Workspaces with overall_lifecycle but no per-slot SMs | 2 (SessionBootstrap, OnboardingRequest) |
| Declared states across all SMs | ~358 |
| Declared transitions across all SMs | ~330 |
| Cross-workspace constraints (Mode A blocking) | 11 |
| Derived cross-workspace state (Mode B aggregation) | 1 (`cbu_operationally_active`) |
| Dual lifecycle declarations | 4 (cbu primary+operational, deal commercial+operational, attribute_def external+internal, deal_dag G-4 amendment intent — not formally declared) |
| Cross-slot constraints (intra-DAG) | 35+ (across all DAGs combined) |
| State machines with full carrier alignment | ~38 of 61 |
| State machines with carrier table missing | 7 (M-024, M-028, M-029, M-030, M-039, M-040, M-055) |
| State machines with carrier column missing | 4 (M-032, M-043, M-046, M-052) |
| State machines with column-collapsed-to-boolean | 2 (M-021, M-025) |
| State machines with hybrid persistence (intentional) | 1 (M-022) |
| State machines with CHECK partially enumerated | 3 (M-038, M-045, M-048) |
| Mechanical anomaly findings (§2) | §2.A: 2 orphan states (M-036.REDEEMING, REDEEMED) · §2.D: 2 SUSPICIOUS bypass writes (service_pipeline_service_impl.rs:165 + learning_tools peripheral) · §2.E: 5 unenforceable XW constraints · §2.G: 193 cosmetic naming + 15 declarative completeness · §2.H: 9 thin/empty entries · §2.I: 2 dual-lifecycle materialisation gaps (cbu, deal) |
| Consistency findings (§3) | §3.B: 14 state machines with DAG ∖ CHECK ≠ ∅ · §3.C: 5 unenforceable XW constraints · §3.D: ≥50 verbs with workspace/slot declarative drift (registry.investor.*, registry.holding.*, registry.share-class.* claim session_bootstrap; screening.* claims instrument_matrix; service.* claims instrument_matrix; manco-group.* claims instrument_matrix; service-consumption.* claims onboarding_request; settlement-chain.* claims cbu — see §7.4) · §3.E: no migration / DAG divergence detected |
| Onboarding-path transitions (§4) | 8 — 5 fully enforceable, 3 blocked by S-1/S-2/S-3 |
| Substrate-audit S-IDs flagged in this evidence base | 14 distinct (S-1, S-2, S-3, S-4, S-5, S-6, S-7, S-8, S-9, S-10, S-11, S-12, S-13, S-14) covering carrier gaps; S-21, S-22, S-25 covering naming/declarative/redundancy |
| Test coverage (§7.2) | **36 of 61 state machines (59%) have zero test coverage.** Tested: 25 SMs concentrated in KYC (9 SMs), CBU discovery+fund-mandate (6 SMs), SemOsMaintenance (6 SMs), and a handful in IM/Deal/Catalogue. Untested workspaces (entirely or near-entirely): ProductMaintenance, BookingPrincipal, LifecycleResources, BookSetup, all DAG-first state machines (M-024, M-028, M-029, M-030, M-039, M-040, M-055), all dual-lifecycle operational halves (M-032, M-046). |
| Declarative-drift findings (§3.D / §7.4) | ≥50 verbs declare `target_workspace + target_slot` that mismatches the DAG-authoring workspace; 4 distinct cluster patterns (session_bootstrap over-claim, instrument_matrix over-claim, onboarding_request over-claim, cbu over-claim) plus naming-convention drift (e.g. `kyc.kyc-case.*` vs DAG's `kyc-case.*`/`case.*`) and verb-set drift on M-007 / M-011 / M-014 / M-012. |

### 6.B — Where to start the business review

A suggested ordering for Adam's verdict pass, by review-value heuristic:

**Highest review value — onboarding-path state machines with anomalies:**

1. **M-039 cbu.service_consumption** — most cross-workspace constraints (3 inbound), carrier missing (S-1 BLOCKING), semantic overlap with M-026 service_intent (S-25). Centre of the "Deal → operational subscription" pivot. Verdict here decides whether the 6-state lifecycle is correct, whether 3-state service_intent should subsume it, and whether the gate cluster is right.
2. **M-045 deal.deal (commercial)** — primary commercial workspace; CHECK lacks BAC_APPROVAL (S-2 BLOCKING), terminal-negative granularity (S-8). Adam's three-leg deal tollgate (BAC + KYC + BP) is half-DAG-half-state-value. Verdict on whether BAC-as-state-not-entity is correct.
3. **M-046 deal.deal (operational dual)** — column missing (S-9). Plus pending G-4 amendment chain (referenced in DAG header but not formally declared). Verdict on commercial / operational / amendment duality.
4. **M-031 cbu.cbu (discovery)** — fully materialised; canonical `cbu_operationally_active` aggregate hosted here (Mode B Tollgate). Verdict on the 5-source aggregator definition.
5. **M-032 cbu.cbu (operational dual)** — DAG-only (S-3). Verdict on the 8-state operational lifecycle.
6. **M-007 kyc.kyc_case** — most-fanned-out state machine (3 cross-workspace constraints inbound). Verdict on the 11-state lifecycle.

**Highest review value — declared but currently unmaterialised state machines (DAG-first):**

7. **M-055 book_setup.book** — entire workspace primary entity, table missing (S-5). Verdict on whether the 8-state book journey is the right granularity.
8. **M-024 instrument_matrix.trading_activity** — signal slot, table missing (S-6). Verdict on dormancy / first-trade as separate concerns.
9. **M-040 cbu.cbu_corporate_action** — table missing (S-4). Verdict on CBU-level vs instrument-level CA distinction.
10. **M-028, M-029, M-030 instrument_matrix.{reconciliation, corporate_action_event, collateral_management}** — IM pass-3/7 additions. Verdict on borderline-operational classification.

**Mechanical anomalies — DAG-internal review:**

11. **M-036 cbu.investor REDEEMING / REDEEMED orphan states** — declared states with zero in/out transitions per DAG. Verdict: should they be reachable / leavable, or removed from the state set?
12. **M-038 cbu.holding** — schema CHECK has 4 of 7 declared states; RESTRICTED / PLEDGED / FROZEN are DAG-only. Verdict on whether legal-locks / collateral-pledges / sanctions-freeze are first-class holding states or attributes.

**Likely-OK quick-confirm state machines (full carrier, no anomalies in §2):**

13. M-001 catalogue.proposal — 5 states, governance-clean. 3 tests.
14. M-004 booking_principal.clearance — fully materialised, two-axis (deal × principal). **Zero tests** — flag for review despite carrier alignment.
15. M-005, M-006 lifecycle_resources.* — fully materialised; cascade rule declared. **Zero tests** — flag.
16. M-007–M-018 kyc.* (modulo M-007 above) — 12 state machines, 87 states, full alignment per substrate audit. Mixed test coverage: M-007–M-015 well-tested (8–41 tests each); **M-016 outreach_request, M-017 kyc_decision, M-018 kyc_service_agreement have zero tests**.
17. M-056–M-061 semos_maintenance.* — governance-only; 6 SMs, full alignment, well-tested (25–50+ tests each).

**Additional review priorities raised by §7 cross-cutting evidence:**

18. **Declarative drift §3.D / §7.4** — for any state machine the reviewer has questions on, the verb-FQN ↔ DAG-FQN reconciliation is a separate decision: is the DAG canonical (verbs need renaming) or is the YAML canonical (DAG transitions need updating)? Particularly material for:
    - M-007 kyc_case (DAG declares `kyc-case.update-status`, `case.approve`; YAML has `kyc.kyc-case.create/.approve/.reject/.submit-for-review`).
    - M-014 red_flag (DAG declares `red-flag.escalate/.resolve/.waive/.update-rating`; YAML has `kyc.red-flag.create/.approve/.reject/.clear/.escalate`).
    - M-012 kyc_ubo_registry (DAG declares `ubo-registry.verify/.approve/.reject/.expire/.discover`; YAML has `kyc.ubo-registry.register-ubo/.verify-ubo/.retire-ubo/.update-beneficial-interest`).
    - M-036 / M-038 / M-043 cbu.* (verbs declare target_workspace `session_bootstrap` instead of `cbu`).
19. **Test-coverage signal §7.2** — 36 of 61 state machines have zero tests. The reviewer may treat this as a separate dimension from carrier alignment: a state machine may be DAG-correct AND carrier-aligned AND zero-tested. The product-quality consequence is for the reviewer; the evidence is in §7.2.
20. **§2.D bypass write at `service_pipeline_service_impl.rs:165`** — writes `service_intents.status = 'superseded'`, which is **NOT a declared state of M-026 service_intent** (DAG enumerates active, suspended, cancelled). This is a §3.B CHECK ∖ DAG ≠ ∅ finding hidden inside service-layer code: either the DAG should declare `superseded` as a state (possible business verdict: yes, supersession is real for service intents), or the impl should not be writing that value (possible verdict: the write is a bug).

### 6.C — What is NOT in this document

- **Business verdicts.** Whether any state machine is correct / needs renaming / wrongly modelled is not adjudicated. Verdicts are for the business reviewer (Adam + BNY domain experts).
- **Recommended changes.** No state additions, removals, renamings, or transition changes are proposed.
- **Severity classifications.** "Blocking / significant / minor" labels appear where they are quoted from substrate audit S-IDs (which are inputs); no fresh severity is added.
- **Catalogue-layer concerns.** Three-axis declarations, validator behaviour, tier governance, escalation DSL — out of scope per brief.
- **Schema remediation proposals.** Migration drafts not produced.
- **Code-grep deep extracts** for §1.X.F–J (verb-impl summaries, code-path write sites, test fixtures, internal docs). The substrate audit's pre-existing code-path coverage is sufficient for §4 and §3.B; per-state-machine impl-vs-declaration reconciliation pass deferred.
- **§2.D bypass-write detection** (SQL UPDATE bypassing DSL verbs).
- **§2.F implemented-but-undeclared constraints** detection.
- **§2.J slot-dispatch gap detection** (runtime registry vs DAG slot list).
- **§3.A transition-by-transition verb-impl reconciliation.**
- **Test-fixture inventory** per state machine.

These are the natural follow-up passes after the business reviewer's verdicts identify which state machines warrant deeper traversal.

### 6.D — Resumption notes

If Adam wants to deepen any field on a specific state machine:

- **For 1.X.F (verb impl summaries):** grep `rust/config/verbs/<domain>/*.yaml` for `transition_args:` blocks where `target_workspace + target_slot` matches the state machine. Substrate audit §4.2 catalogues 50+ verbs by domain.
- **For 1.X.G (code paths):** grep `rust/src/**/*.rs` for SQL `UPDATE ... SET <state_column>` and for `CustomOperation` impls covering the verbs from F.
- **For 1.X.H (migration history):** `rust/migrations/*.sql` filtered by carrier table name from §1.X.E.
- **For 1.X.I (tests):** `rust/tests/`, `rust/scenarios/suites/`, plus `rust/src/**/tests/**`.
- **For 1.X.J (internal docs):** DAG header already cites parent docs; expand by greping `docs/` for state-machine-id or carrier name.

---

---

## Section 7 — Cross-cutting evidence appendix

> Section 7 holds tabular evidence that would be repetitive if presented per state machine (§1.X.F–J). Each table is referenced from the relevant §1.X subsection. The appendix is grouped: 7.1 verb-to-state-machine mapping, 7.2 test-coverage matrix, 7.3 migration history per carrier, 7.4 declarative-drift summary (mirrors §3.D), 7.5 bypass-write findings (mirrors §2.D).

### 7.1 — Verb-to-state-machine mapping (§1.X.F evidence)

A scan of `rust/config/verbs/**/*.yaml` found ~180 verbs declaring `transition_args.target_workspace + target_slot`. The mapping below groups by target_workspace + target_slot and supports two analyses: (a) which verbs implement transitions for each state machine (§1.X.F); (b) which verbs declare drift between their declared workspace/slot and the DAG's authoring location (§3.D).

> Source data: parallel agent scan, 2026-04-29. Verb FQN normalisation may differ from canonical names — entries listed reflect the YAML as authored. Where a verb's `target_workspace` mismatches the DAG-authoring workspace, the mismatch is captured in §3.D / §7.4. Per-verb `from_state`/`to_state` declaration extraction was incomplete in this pass — populated where the agent could resolve it; otherwise blank. Detailed file-by-file extraction remains a deferred sub-task.

**Verb counts by target_workspace + target_slot (canonical view):**

| target_workspace | target_slot | Verb count | State machine |
|---|---|---|---|
| catalogue | proposal | 3+ | M-001 |
| product_maintenance | service | 4 (drafted as instrument_matrix per §3.D drift) | M-002 |
| product_maintenance | service_version | 3 | M-003 |
| product_maintenance | delivery | 2 (flag-for-review, clear-review-flag) | M-027 (note: delivery slot is in instrument_matrix per DAG; cross-workspace verb) |
| booking_principal | clearance | 7 | M-004 |
| lifecycle_resources | application_instance | 6 | M-005 |
| lifecycle_resources | capability_binding | 4 | M-006 |
| lifecycle_resources | service_pipeline | 3 (propose, activate, retire) | (no state machine in this evidence base — possible orphan slot or stale verb declarations) |
| kyc | kyc_case | 4 (drift-named — `kyc.kyc-case.*` prefix) | M-007 |
| kyc | red_flag | 5 (drift-named) | M-014 |
| kyc | evidence | 2 (drift-named — slot does not exist in DAG) | (no SM — declarative drift §3.D) |
| kyc | ubo_registry | 4 (drift-named vs M-012 kyc_ubo_registry) | M-012 |
| kyc | entity | 1 (`entity.archive`; slot does not exist in kyc DAG) | (no SM — drift) |
| instrument_matrix | trading_profile | 14 | M-023 / M-020 (template variant) |
| instrument_matrix | service_resource | 3 | M-025 |
| instrument_matrix | service_intent | (verbs in service_pipeline_service_impl.rs, not all declared via transition_args) | M-026 |
| instrument_matrix | corporate_action_event | 2 | M-029 |
| instrument_matrix | reconciliation | 4 (declared as `product_maintenance.reconciliation.*` per §7.4) | M-028 |
| instrument_matrix | collateral_management | 5 | M-030 |
| instrument_matrix | screening | 3 (drift — slot is in kyc per DAG) | (drift — should map to M-010) |
| instrument_matrix | service | 4 (drift — slot is in product_maintenance per DAG) | (drift — should map to M-002) |
| instrument_matrix | manco_group | 7 (drift — slot id is `manco` in cbu DAG, not `manco_group`) | (drift — should map to M-044) |
| cbu | cbu | 13+ | M-031 / M-032 |
| cbu | cbu_evidence | 1 (`cbu.attach-evidence`) + verify-evidence inline | M-035 |
| cbu | cbu_corporate_action | 5 | M-040 |
| cbu | cbu_disposition | 5 | M-041 |
| cbu | settlement_chain | 8 (drift — slot is `settlement_pattern_template` in instrument_matrix per DAG) | (drift — should map to M-021) |
| deal | deal | 5 (set-status, cancel, archive, launch-operations, +) | M-045 / M-046 |
| deal | deal_participant | 2 | (stateless slot per DAG; no SM) |
| deal | deal_document | 2 | M-050 |
| deal | deal_attribute | 1 | (stateless) |
| deal | deal_pod_owner | 2 | (stateless) |
| deal | deal_analytics_config | 1 | (stateless) |
| deal | deal_recon_settings | 1 | (stateless) |
| deal | deal_reporting_settings | 1 | (stateless) |
| deal | deal_opex_readiness | 1 | (stateless) |
| deal | deal_migration | 3 | (stateless / not in DAG) |
| deal | billing_profile | 3 | M-053 |
| deal | billing_period | 5 | M-054 |
| onboarding_request | onboarding_request | 1 (`onboarding_request.setup`) | (no per-slot SM; pack-level only) |
| onboarding_request | service_consumption | 6 (drift — slot is in cbu DAG per M-039) | (drift — should map to M-039) |
| onboarding_request | trade_gateway | 3 (drift — slot is in instrument_matrix per M-022) | (drift — should map to M-022) |
| session_bootstrap | investor | 9 (drift — slot is in cbu DAG per M-036) | (drift — should map to M-036) |
| session_bootstrap | holding | 5 (drift — slot is in cbu DAG per M-038) | (drift — should map to M-038) |
| session_bootstrap | share_class | 6 (drift — slot is in cbu DAG per M-043) | (drift — should map to M-043) |
| book_setup | book | 4 | M-055 |
| semos_maintenance | attribute_def | 2 (`attribute.deprecate`, `sem-reg.governance.deprecate-attrdef`) | M-057 |
| semos_maintenance | phrase | 3 (drift — DAG slot is `phrase_authoring`) | M-061 |
| semos_maintenance | enum_value | 1 | (no SM) |

> Total verbs with `transition_args` ≈ 180, distributed across 12 declared workspaces and 60+ declared slots. ≥ 50 verbs sit in workspaces that drift from where their slot is authored (see §3.D / §7.4).

### 7.2 — Test-coverage matrix per state machine (§1.X.I evidence)

A scan of `rust/tests/**/*.rs`, `rust/src/**/tests/*.rs`, and `rust/scenarios/suites/*.yaml` mapped tests to state machines by carrier-table reference and verb-FQN reference. **36 of 61 state machines have zero test coverage.** This is the most material §1.X.I finding for the business reviewer.

**Tested state machines (25):**

| State machine | Test files | Approx. test count |
|---|---|---|
| M-007 kyc.kyc_case | `kyc_full_lifecycle.rs`, `deal_to_kyc_lifecycle.rs`, `db_integration.rs` | 41 |
| M-008 kyc.entity_kyc | `reducer_integration_tests.rs`, `reducer_state_phase2_tests.rs` | 23 |
| M-009 kyc.entity_workstream | `kyc_full_lifecycle.rs`, `db_integration.rs` | 37 |
| M-010 kyc.screening | `kyc_full_lifecycle.rs`, `db_integration.rs` | 37 |
| M-011 kyc.ubo_evidence | `kyc_full_lifecycle.rs`, `db_integration.rs` | 37 |
| M-012 kyc.kyc_ubo_registry | `kyc_full_lifecycle.rs`, `db_integration.rs` | 37 |
| M-013 kyc.kyc_ubo_evidence | `kyc_full_lifecycle.rs` | 8 |
| M-014 kyc.red_flag | `kyc_full_lifecycle.rs` | 8 |
| M-015 kyc.doc_request | `kyc_full_lifecycle.rs`, `db_integration.rs` | 37 |
| M-031 cbu.cbu (discovery) | `db_integration.rs`, `client_group_integration.rs` | 47 |
| M-036 cbu.investor | `investor_register_tests.rs` | 14 |
| M-037 cbu.investor_kyc | `investor_register_tests.rs` | 14 |
| M-038 cbu.holding | `capital_ownership_integration.rs` | 8 |
| M-043 cbu.share_class | `capital_ownership_integration.rs` | 8 |
| M-001 catalogue.proposal | `catalogue_workspace_lifecycle.rs` | 3 |
| M-020 instrument_matrix.trading_profile_template | `trading_profile_field_test.rs` | 7 |
| M-023 instrument_matrix.trading_profile | `trading_profile_field_test.rs` | 7 |
| M-045 deal.deal (commercial) | `deal_to_kyc_lifecycle.rs` | 4 |
| M-056 semos_maintenance.changeset | `stewardship_b3_changesets.rs`, `sem_reg_integration.rs`, `sem_reg_authoring_integration.rs` | 50+ |
| M-057 semos_maintenance.attribute_def (external) | `sem_reg_integration.rs`, `sem_reg_authoring_integration.rs` | 50+ |
| M-058 semos_maintenance.attribute_def (internal) | `sem_reg_integration.rs`, `sem_reg_authoring_integration.rs` | 50+ |
| M-059 semos_maintenance.derivation_spec | `sem_reg_integration.rs` | 25+ |
| M-060 semos_maintenance.service_resource_def | `sem_reg_integration.rs` | 25+ |
| M-061 semos_maintenance.phrase_authoring | `sem_reg_authoring_integration.rs` | 25+ |

**Untested state machines (36):**

| Workspace | M-IDs (zero test coverage) |
|---|---|
| kyc | M-016 outreach_request, M-017 kyc_decision, M-018 kyc_service_agreement |
| cbu | M-032 (operational dual), M-033 entity_proper_person, M-034 entity_limited_company_ubo, M-035 cbu_evidence, M-039 service_consumption, M-040 cbu_corporate_action, M-041 cbu_disposition, M-042 client_group_entity_review, M-044 manco |
| deal | M-046 (operational dual), M-047 deal_product, M-048 deal_rate_card, M-049 deal_onboarding_request, M-050 deal_document, M-051 deal_ubo_assessment, M-052 deal_sla, M-053 billing_profile, M-054 billing_period |
| product_maintenance | M-002 service, M-003 service_version |
| booking_principal | M-004 clearance |
| lifecycle_resources | M-005 application_instance, M-006 capability_binding |
| instrument_matrix | M-019 group, M-021 settlement_pattern_template, M-022 trade_gateway, M-024 trading_activity, M-025 service_resource, M-026 service_intent, M-027 delivery, M-028 reconciliation, M-029 corporate_action_event, M-030 collateral_management |
| book_setup | M-055 book |

> Caveat. This is a heuristic test-mapping (carrier-table or verb-FQN string match in test source). State machines that share carriers (e.g. M-031 + M-041 both on `cbus`; M-036 + M-037 both on `investors`) may count tests that exercise one but not the other. Test counts are approximate. The "zero-coverage" finding is the load-bearing signal — those state machines have no tests of any kind that reference their carrier or verbs.

### 7.3 — Migration history per carrier (§1.X.H evidence)

Migrations explicitly cited in DAG headers or substrate audit:

| Carrier table | Migration filename | Date | What it does |
|---|---|---|---|
| `cbus` (status CHECK extension) | `20260423_extend_cbu_status_check.sql` | 2026-04-23 | Extends `chk_cbu_status` CHECK constraint (per substrate audit §0.3 row CBU). |
| `cbus` + `cbu_entity_*` (soft-delete) | `20260316_soft_delete_cbu_entities.sql` | 2026-03-16 | Adds `deleted_at` column for M-041 cbu_disposition `soft_deleted` state. |
| `cbus`, `manco_regulatory_status`, others | `20260424_tranche_2_3_dag_alignment.sql` | 2026-04-24 | Alignment work between DAG state machines and schema CHECKs. |
| `manco_regulatory_status` (M-044 carrier) | `20260425_manco_regulatory_status.sql` | 2026-04-25 | New carrier table with `regulatory_status` CHECK enumeration. |
| `cbu_trading_profiles` (M-020 / M-023) | `20260331_trading_profile_templates.sql` | 2026-03-31 | Adds template/instance distinction (cbu_id IS NULL for templates). Plus `202412_trading_matrix_storage.sql`. |
| `deals` (KYC clearance gate) | `20260324_deal_kyc_clearance_gate.sql` | 2026-03-24 | Adds KYC_CLEARANCE state to deal_status (precursor to R-5 BAC_APPROVAL gap S-2). |
| `deal_rate_cards` (one-AGREED constraint) | (migration 069 cited in DAG header — see filename idx_deal_rate_cards_one_agreed) | (date n/a) | DB-enforced one-AGREED-per-(deal, contract, product). |
| `application_instances`, `capability_bindings` (M-005, M-006) | `20260427_lifecycle_resources_workspace.sql` | 2026-04-27 | New carrier tables with state CHECK constraints. |
| `services`, `service_versions` (M-002, M-003) | `20260428_service_lifecycle.sql` | 2026-04-28 | Adds `lifecycle_status` columns + CHECK. |
| `booking_principal_clearances` (M-004) | `20260429_booking_principal_clearance.sql` | 2026-04-29 | New carrier table with `clearance_status` CHECK. |
| `catalogue_proposals`, `catalogue_committed_verbs` (M-001) | `20260427_catalogue_workspace.sql` | 2026-04-27 | New carrier tables. |
| `cases`, `kyc_*` family (KYC carriers) | `202412_kyc_case_builder.sql` | 2024-12 | KYC family carrier creation (substrate audit §0.3 row KYC: "✅ all states materialised"). |
| `cbu_settlement_chains` (M-021) | `202412_trading_matrix_storage.sql` | 2024-12 | Initial carrier (state-collapsed-to-boolean per S-12). |
| `service_resource_types`, `service_intents`, `service_delivery_map` (M-025/M-026/M-027) | `202412_*` (multiple) | 2024-12 | IM family carriers. |
| `share_classes` (M-043 carrier; lifecycle_status MISSING) | (no recent migration adds `lifecycle_status`) | — | Substrate audit S-10. |
| `client_books` (M-055 carrier) | (no migration found) | — | Substrate audit S-5: table missing. |
| `cbu_service_consumption` (M-039 carrier) | (no migration found) | — | Substrate audit S-1: table missing. |
| `cbu_trading_activity` (M-024 carrier) | (no migration found) | — | Substrate audit S-6: table missing. |
| `cbu_corporate_action_events` (M-040 carrier) | (no migration found) | — | Substrate audit S-4: table missing. |
| `deal_slas.sla_status` (M-052 column) | (no migration found) | — | Substrate audit S-7: column missing. |
| `cbus.operational_status` (M-032 column) | (no migration found) | — | Substrate audit S-3: column missing. |
| `deals.operational_status` (M-046 column) | (no migration found) | — | Substrate audit S-9: column missing. |
| `cbus.disposition_status` (M-041 column) | (no migration found — soft_delete uses deleted_at) | — | Substrate audit S-11: column missing. |

> Caveat. This is the migration-evidence the DAG headers and substrate audit cite explicitly. A complete migration-by-carrier traversal of `rust/migrations/*.sql` (~300 files) would surface earlier creation events and intermediate column additions; the table above prioritises recent + DAG-cited migrations.

### 7.4 — Declarative-drift summary (verbs targeting "wrong" workspace per DAG)

This is a consolidation of §3.D's table — the same drift findings restated for cross-cutting visibility. **At least 50 verbs declare a `target_workspace` that does not match the DAG-authoring workspace of the slot they target.** Source data: agent scan of `rust/config/verbs/**/*.yaml`, 2026-04-29.

Drift cluster summary:
- **`session_bootstrap` over-claims** (~20 verbs): `registry.investor.*`, `registry.holding.*`, `registry.share-class.*` declare session_bootstrap as target but their slots are CBU-domain (M-036, M-038, M-043). session_bootstrap DAG explicitly declares 0 per-slot state machines.
- **`instrument_matrix` over-claims** (~14 verbs): `screening.*` (kyc), `service.*` (product_maintenance), `manco-group.*` (cbu), `service-pipeline.*` (no DAG SM).
- **`onboarding_request` over-claims** (~9 verbs): `service-consumption.*` (cbu/M-039), `custody.trade-gateway.*` (instrument_matrix/M-022).
- **`cbu` over-claims** (~8 verbs): `custody.settlement-chain.*` (instrument_matrix/M-021).
- **`product_maintenance` under-claims** (~4 verbs): `product_maintenance.reconciliation.*` declared with target_workspace `instrument_matrix` (correct per DAG which places reconciliation in IM/M-028) — naming convention drift, not target drift.
- **Naming-convention drift**: `kyc.kyc-case.*` vs DAG's `kyc-case.*`/`case.*`; `kyc.evidence.*` vs DAG's `evidence.*`; `kyc.red-flag.*` vs DAG's `red-flag.*`; `kyc.ubo-registry.*` vs DAG's `ubo-registry.*`. The double-`kyc.` prefix appears to be a verb-domain-naming drift not picked up by substrate audit S-21 cosmetic flag.
- **Verb-set drift**: M-007 kyc.kyc_case DAG transitions reference `kyc-case.update-status`, `case.approve`, `case.reject`, `kyc-case.escalate`, `kyc-case.close`, `kyc-case.reopen`. The verb scan finds `kyc.kyc-case.{create,approve,reject,submit-for-review}`. Set difference: verbs declared in DAG that the scan did not find under the canonical FQN; verbs in YAML that the DAG does not reference. This is the most consequential drift class for the business reviewer because it creates ambiguity about which name is canonical.

### 7.5 — Bypass-write findings (mirrors §2.D)

See §2.D inline. Material findings: 2 SUSPICIOUS sites (`service_pipeline_service_impl.rs:165` writing `service_intents.status = 'superseded'` outside verb framework, and a likely §3.B finding because `superseded` is not in the DAG's declared state set for M-026). The other suspicious site (`learning_tools.rs` MCP handler writes to `learning_candidates`) is on a peripheral table not in the DAG taxonomy.

---

**End of DAG Business-Semantic Review — Evidence Base. 2026-04-29.**

**Document status:** Sections 0, 1 (M-001..M-061), 2, 3, 4, 5, 6, 7 landed. Resumability per §6.D.

---

**Document status:** skeleton + Section 0 landed. Section 1 subsections being appended in workspace-grouped batches. Sections 2–6 follow.
