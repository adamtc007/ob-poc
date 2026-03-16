# P1-B: Entity Lifecycle FSM Audit

**Review Session:** P1-B
**Date:** 2026-03-16
**Scope:** Status/state columns across `ob-poc` schema, FSM transition graph structure, enforcement layer (DB vs Rust), registry coverage, and correctness gaps.

---

## Executive Summary

The codebase contains **~45+ status enums** across Rust code and PostgreSQL CHECK constraints. Only **3 entities** have fully enforced FSM transition validation in Rust (KYC Case, Deal, and ChangeSet). The remaining entities rely solely on DB CHECK constraints for valid-value enforcement with **no transition validation** -- any status can jump to any other valid status.

A **CRITICAL** drift exists between the KYC Case YAML taxonomy definition (7 states) and the hardcoded `CASE_TRANSITIONS` (11 states) in `kyc_case_ops.rs`. These are completely incompatible state sets.

**Severity distribution:** 2 CRITICAL, 8 FLAG, 12 MINOR, 6 CLEAN

---

## Table of Contents

1. [FSM Implementation Patterns](#1-fsm-implementation-patterns)
2. [YAML-Driven Entity Lifecycles (Ontology)](#2-yaml-driven-entity-lifecycles-ontology)
3. [Hardcoded Rust FSMs](#3-hardcoded-rust-fsms)
4. [Semantic OS Status Enums](#4-semantic-os-status-enums)
5. [BPMN Integration Status Enums](#5-bpmn-integration-status-enums)
6. [Session / REPL Status Enums](#6-session--repl-status-enums)
7. [DB-Only CHECK Constraint FSMs](#7-db-only-check-constraint-fsms)
8. [Cross-Cutting Analysis](#8-cross-cutting-analysis)
9. [Findings Summary](#9-findings-summary)
10. [Recommendations](#10-recommendations)

---

## 1. FSM Implementation Patterns

Three distinct patterns exist:

| Pattern | Enforcement | Transition Validation | Examples |
|---------|-------------|----------------------|----------|
| **A: Hardcoded transition map** | Rust function + DB CHECK | Explicit `is_valid_transition()` | KYC Case, Deal |
| **B: YAML-driven EntityLifecycle** | YAML config + `ontology::lifecycle` module | Available but **not wired** into domain ops (except test utilities) | CBU, Entity, Document, etc. (13 entities) |
| **C: DB CHECK only** | PostgreSQL CHECK constraint | **None** -- any valid value reachable from any other | Legal contracts, capital events, subscriptions, etc. |

**Key finding:** Pattern B defines a reusable FSM framework (`ontology/lifecycle.rs`) with `is_valid_transition()`, `terminal_states()`, and `LifecycleValidation` -- but the `domain_ops/` handlers **do not call it**. The lifecycle.rs functions are marked `#[allow(dead_code)]` and only used in tests. The 13 YAML-defined lifecycles are metadata-only with no runtime enforcement.

**Files:**
- `rust/src/ontology/lifecycle.rs` -- Generic FSM framework (dead code in production)
- `rust/src/ontology/types.rs` -- `EntityLifecycle`, `StateTransition` types
- `rust/config/ontology/entity_taxonomy.yaml` -- 13 entity lifecycle definitions
- `rust/src/domain_ops/kyc_case_ops.rs` -- Hardcoded Pattern A
- `rust/src/domain_ops/deal_ops.rs` -- Hardcoded Pattern A

---

## 2. YAML-Driven Entity Lifecycles (Ontology)

Source: `rust/config/ontology/entity_taxonomy.yaml`

### 2.1 CBU (Client Business Unit)

**Severity: FLAG** -- YAML lifecycle defined but not enforced at runtime.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| DISCOVERED | DRAFT | No |
| DRAFT | PENDING_VALIDATION | No |
| PENDING_VALIDATION | VALIDATED, DRAFT | No |
| VALIDATED | ACTIVE | No |
| ACTIVE | SUSPENDED, TERMINATED | No |
| SUSPENDED | ACTIVE, TERMINATED | No |
| TERMINATED | *(none)* | **Yes** |

- **Initial state:** DISCOVERED
- **Terminal states:** TERMINATED
- **Status column:** `status`
- **Gap:** No Rust-side transition enforcement. A CBU could jump from DISCOVERED to TERMINATED via raw SQL or CRUD verb.
- **SUSPENDED handling:** Correct -- bidirectional with ACTIVE, forward-only to TERMINATED.

### 2.2 Entity (Person/Company)

**Severity: MINOR** -- Simple 4-state lifecycle, low risk.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| DRAFT | ACTIVE | No |
| ACTIVE | INACTIVE, MERGED | No |
| INACTIVE | ACTIVE, MERGED | No |
| MERGED | *(none)* | **Yes** |

- **Initial state:** DRAFT
- **Terminal states:** MERGED
- **Note:** INACTIVE is reversible (reactivation via INACTIVE->ACTIVE). No SUSPENDED state.
- **Subtypes:** proper_person, limited_company, trust, partnership inherit this lifecycle via `parent_type: entity`.

### 2.3 KYC Case (YAML Definition)

**Severity: CRITICAL** -- Incompatible with hardcoded `CASE_TRANSITIONS`. See Section 3.1.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| OPEN | IN_PROGRESS | No |
| IN_PROGRESS | PENDING_DECISION | No |
| PENDING_DECISION | APPROVED, REJECTED | No |
| APPROVED | CLOSED | No |
| REJECTED | CLOSED | No |
| CLOSED | *(none)* | **Yes** |
| CANCELLED | *(none)* | **Yes** |

- **Initial state:** OPEN
- **Terminal states:** CLOSED, CANCELLED
- **7 states, linear flow** -- dramatically simpler than the 11-state hardcoded version.
- **Missing from YAML:** INTAKE, DISCOVERY, ASSESSMENT, REVIEW, BLOCKED, WITHDRAWN, EXPIRED, REFER_TO_REGULATOR, DO_NOT_ONBOARD

### 2.4 KYC Workstream

**Severity: MINOR** -- Clean 5-state machine.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| PENDING | IN_PROGRESS, WAIVED | No |
| IN_PROGRESS | COMPLETE, BLOCKED | No |
| COMPLETE | *(none)* | **Yes** |
| BLOCKED | IN_PROGRESS | No |
| WAIVED | *(none)* | **Yes** |

- **Initial state:** PENDING
- **Terminal states:** COMPLETE, WAIVED
- **BLOCKED handling:** Correct -- can recover to IN_PROGRESS only.

### 2.5 Document

**Severity: FLAG** -- Re-request loops need careful guard attention.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| PENDING | REQUESTED | No |
| REQUESTED | RECEIVED | No |
| RECEIVED | UNDER_REVIEW | No |
| UNDER_REVIEW | VERIFIED, REJECTED | No |
| VERIFIED | EXPIRED | No |
| REJECTED | REQUESTED | No |
| EXPIRED | REQUESTED | No |

- **Initial state:** PENDING
- **Terminal states:** **None** -- VERIFIED and EXPIRED both have outbound transitions. This means documents can cycle indefinitely.
- **Gap:** No true terminal state. A VERIFIED document can EXPIRE and be re-REQUESTED. Operationally correct for document lifecycle but unusual for FSM design.
- **REJECTED loop:** REJECTED->REQUESTED allows re-solicitation.

### 2.6 Doc Request

**Severity: CLEAN** -- Well-defined with clear terminals.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| PENDING | REQUESTED | No |
| REQUESTED | RECEIVED, WAIVED | No |
| RECEIVED | ACCEPTED, REJECTED | No |
| ACCEPTED | *(none)* | **Yes** |
| REJECTED | REQUESTED | No |
| WAIVED | *(none)* | **Yes** |

- **Initial state:** PENDING
- **Terminal states:** ACCEPTED, WAIVED
- **REJECTED loop:** Same re-request pattern as Document.

### 2.7 UBO Record

**Severity: MINOR** -- Dispute recovery path is unusual.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| DECLARED | EVIDENCED | No |
| EVIDENCED | VERIFIED, DISPUTED | No |
| VERIFIED | SUPERSEDED | No |
| DISPUTED | EVIDENCED, DECLARED | No |
| SUPERSEDED | *(none)* | **Yes** |

- **Initial state:** DECLARED
- **Terminal states:** SUPERSEDED
- **DISPUTED handling:** Can regress to EVIDENCED (re-evidence) or DECLARED (full reset). This allows potentially infinite loops: DECLARED->EVIDENCED->DISPUTED->DECLARED->...

### 2.8 Screening

**Severity: FLAG** -- MATCH_FOUND has a dead-end path concern.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| PENDING | IN_PROGRESS | No |
| IN_PROGRESS | COMPLETED, MATCH_FOUND | No |
| COMPLETED | *(none)* | **Yes** |
| MATCH_FOUND | CLEARED, ESCALATED | No |
| CLEARED | *(none)* | **Yes** |
| ESCALATED | *(none)* | **Yes** |

- **Initial state:** PENDING
- **Terminal states:** COMPLETED, CLEARED, ESCALATED
- **Gap:** ESCALATED is terminal with no recovery path. Once escalated, a new screening must be created. Operationally this may be correct but should be documented.

### 2.9 Red Flag

**Severity: CLEAN** -- Simple and correct.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| OPEN | UNDER_REVIEW | No |
| UNDER_REVIEW | MITIGATED, WAIVED, ESCALATED | No |
| MITIGATED | *(none)* | **Yes** |
| WAIVED | *(none)* | **Yes** |
| ESCALATED | *(none)* | **Yes** |

- **Initial state:** OPEN
- **Terminal states:** MITIGATED, WAIVED, ESCALATED

### 2.10 CBU Resource Instance

**Severity: MINOR** -- Clean with SUSPENDED support.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| PENDING | PROVISIONING | No |
| PROVISIONING | PROVISIONED, PENDING | No |
| PROVISIONED | ACTIVE | No |
| ACTIVE | SUSPENDED, DECOMMISSIONED | No |
| SUSPENDED | ACTIVE, DECOMMISSIONED | No |
| DECOMMISSIONED | *(none)* | **Yes** |

- **Initial state:** PENDING
- **Terminal states:** DECOMMISSIONED
- **PROVISIONING->PENDING:** Allows retry on provisioning failure.
- **SUSPENDED handling:** Bidirectional with ACTIVE, forward to DECOMMISSIONED. Consistent with CBU pattern.

### 2.11 Service Delivery

**Severity: CLEAN** -- Clean with SUSPENDED.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| NOT_STARTED | IN_PROGRESS | No |
| IN_PROGRESS | READY | No |
| READY | ACTIVE | No |
| ACTIVE | SUSPENDED, TERMINATED | No |
| SUSPENDED | ACTIVE, TERMINATED | No |
| TERMINATED | *(none)* | **Yes** |

- **Initial state:** NOT_STARTED
- **Terminal states:** TERMINATED
- **SUSPENDED handling:** Identical pattern to CBU. Consistent.

### 2.12 Monitoring Event

**Severity: MINOR** -- ESCALATED is a dead-end.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| DETECTED | ACKNOWLEDGED | No |
| ACKNOWLEDGED | INVESTIGATING | No |
| INVESTIGATING | RESOLVED, ESCALATED | No |
| RESOLVED | *(none)* | **Yes** |
| ESCALATED | *(none)* | **Yes** |

- **Initial state:** DETECTED
- **Terminal states:** RESOLVED, ESCALATED
- **Same ESCALATED dead-end pattern** as Screening.

### 2.13 Entities Without Lifecycles

The following entities in the taxonomy have `lifecycle: null` or no lifecycle field:

- **product, service, resource_type** -- Reference data, no status lifecycle needed
- **ownership_edge** -- Relationship type, no lifecycle
- **investigation** -- Alias for `kyc_case`
- **decision** -- No lifecycle defined

---

## 3. Hardcoded Rust FSMs

### 3.1 KYC Case (Hardcoded CASE_TRANSITIONS)

**Severity: CRITICAL** -- Two completely incompatible FSM definitions for the same entity.

**Source:** `rust/src/domain_ops/kyc_case_ops.rs`

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| INTAKE | DISCOVERY, WITHDRAWN | No |
| DISCOVERY | ASSESSMENT, BLOCKED, WITHDRAWN | No |
| ASSESSMENT | REVIEW, BLOCKED, WITHDRAWN | No |
| REVIEW | APPROVED, REJECTED, REFER_TO_REGULATOR, DO_NOT_ONBOARD, BLOCKED, WITHDRAWN | No |
| BLOCKED | DISCOVERY, ASSESSMENT, REVIEW, WITHDRAWN | No |
| APPROVED | *(none)* | **Yes** |
| REJECTED | *(none)* | **Yes** |
| WITHDRAWN | *(none)* | **Yes** |
| EXPIRED | *(none)* | **Yes** |
| REFER_TO_REGULATOR | *(none)* | **Yes** |
| DO_NOT_ONBOARD | *(none)* | **Yes** |

- **11 states** vs YAML's 7 states
- **6 terminal states** (`CLOSE_STATUSES`): APPROVED, REJECTED, WITHDRAWN, EXPIRED, REFER_TO_REGULATOR, DO_NOT_ONBOARD
- **Enforcement:** `KycCaseUpdateStatusOp` validates transitions via `is_valid_transition()`. `KycCaseCloseOp` validates close status separately.
- **BLOCKED handling:** Can recover to DISCOVERY, ASSESSMENT, or REVIEW (context-dependent regression).
- **WITHDRAWN:** Reachable from all non-terminal states (universal escape hatch).
- **EXPIRED:** Terminal but not reachable from any state in the transition map. Presumably set by a background timer process. No inbound transition guard.

**Drift Analysis:**

| YAML States | Hardcoded States | Match? |
|-------------|-----------------|--------|
| OPEN | INTAKE | Renamed |
| IN_PROGRESS | DISCOVERY, ASSESSMENT | Split into 2 |
| PENDING_DECISION | REVIEW | Renamed |
| APPROVED | APPROVED | Match |
| REJECTED | REJECTED | Match |
| CLOSED | *(missing)* | Not in hardcoded |
| CANCELLED | WITHDRAWN | Renamed |
| *(missing)* | BLOCKED | Not in YAML |
| *(missing)* | EXPIRED | Not in YAML |
| *(missing)* | REFER_TO_REGULATOR | Not in YAML |
| *(missing)* | DO_NOT_ONBOARD | Not in YAML |

**Root cause:** The YAML taxonomy was written as an idealized model. The hardcoded transitions reflect actual business requirements (multi-phase discovery, regulatory outcomes). The YAML was never updated to match. The hardcoded version is the source of truth at runtime, but the YAML version is what the ontology service, ECIR noun index, and EntityLifecycle framework see.

### 3.2 Deal Record

**Severity: CLEAN** -- Well-implemented FSM with transition validation.

**Source:** `rust/src/domain_ops/deal_ops.rs`

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| PROSPECT | QUALIFYING, CANCELLED | No |
| QUALIFYING | NEGOTIATING, CANCELLED | No |
| NEGOTIATING | CONTRACTED, QUALIFYING, CANCELLED | No |
| CONTRACTED | ONBOARDING, CANCELLED | No |
| ONBOARDING | ACTIVE, CANCELLED | No |
| ACTIVE | WINDING_DOWN | No |
| WINDING_DOWN | OFFBOARDED | No |
| OFFBOARDED | *(none)* | **Yes** |
| CANCELLED | *(none)* | **Yes** |

- **9 states**, linear pipeline with CANCELLED escape hatch
- **Terminal states:** OFFBOARDED, CANCELLED
- **Enforcement:** `is_valid_deal_status_transition()` is a `matches!` macro. `DealUpdateStatusOp` validates before updating. `DealCancelOp` has additional guards (cannot cancel ACTIVE/WINDING_DOWN/OFFBOARDED/CANCELLED).
- **Regression path:** NEGOTIATING->QUALIFYING (deal regression to earlier phase). Intentional.
- **Auto-transition:** `DealCreateOnboardingRequestOp` automatically transitions CONTRACTED->ONBOARDING. `DealCompleteOnboardingOp` auto-transitions ONBOARDING->ACTIVE when all requests complete.
- **Gap:** No DB CHECK constraint on `deal_status` column found in migrations. Validation is Rust-only. A raw SQL UPDATE could set an invalid status.

### 3.3 Rate Card (Implicit FSM)

**Severity: FLAG** -- No explicit transition map, transitions enforced per-verb.

Rate card status values from code inspection: DRAFT, PROPOSED, COUNTER_OFFERED, REVISED, AGREED, SUPERSEDED, CANCELLED, REJECTED.

Transitions are enforced implicitly by individual verb handlers:
- `DealCounterOfferRateCardOp`: Sets old card to SUPERSEDED, creates new card as COUNTER_OFFERED
- `DealAgreeRateCardOp`: Validates status is PROPOSED or COUNTER_OFFERED, then sets AGREED
- No explicit `is_valid_rate_card_transition()` function exists

---

## 4. Semantic OS Status Enums

### 4.1 SnapshotStatus

**Severity: CLEAN** -- Immutable snapshots with well-defined lifecycle.

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| Draft | Active | No |
| Active | Deprecated | No |
| Deprecated | Retired | No |
| Retired | *(none)* | **Yes** |

- **Enforcement:** Publish gates evaluate before `Active` transition. `supersede_snapshot()` handles Active->Deprecated atomically.
- **Strum derives:** `Display`, `EnumString`, `AsRefStr` -- consistent pattern.

### 4.2 ChangeSetStatus

**Severity: CLEAN** -- Most rigorous FSM in the codebase.

**Source:** `rust/crates/sem_os_core/src/authoring/types.rs`

| State | Valid Transitions To | Terminal? |
|-------|---------------------|-----------|
| Draft | UnderReview, Validated, Rejected | No |
| UnderReview | Approved, Rejected | No |
| Approved | Validated, Rejected | No |
| Validated | DryRunPassed, DryRunFailed | No |
| DryRunPassed | Published | No |
| DryRunFailed | *(none)* | **Yes** |
| Published | Superseded | **Yes** (effectively) |
| Rejected | *(none)* | **Yes** |
| Superseded | *(none)* | **Yes** |

- **9 states** with explicit `is_terminal()` and `is_intermediate()` methods
- **Strum derives** for consistent serialization
- **DB CHECK constraint** in migration 099/101 covers all 9 values
- **No explicit transition map** function -- transitions enforced by `GovernanceVerbService` method-level logic. Each method checks preconditions.

---

## 5. BPMN Integration Status Enums

**Source:** `rust/src/bpmn_integration/types.rs`

### 5.1 CorrelationStatus

**Severity: MINOR** -- No transition validation. Low risk (controlled by integration layer).

States: `Active`, `Completed`, `Failed`, `Cancelled`
Terminal: Completed, Failed, Cancelled

### 5.2 JobFrameStatus

**Severity: MINOR** -- Same pattern.

States: `Active`, `Completed`, `Failed`, `DeadLettered`
Terminal: Completed, Failed, DeadLettered

### 5.3 ParkedTokenStatus

**Severity: MINOR** -- Same pattern.

States: `Waiting`, `Resolved`, `TimedOut`, `Cancelled`
Terminal: Resolved, TimedOut, Cancelled

### 5.4 PendingDispatchStatus

**Severity: MINOR** -- Same pattern.

States: `Pending`, `Dispatched`, `FailedPermanent`
Terminal: Dispatched, FailedPermanent

**Pattern inconsistency:** All four use manual `as_str()`/`parse()` instead of strum derives. DB CHECK constraints exist in migrations 073/075/076 and match the Rust enums exactly.

---

## 6. Session / REPL Status Enums

### 6.1 DslStatus

**Severity: MINOR** -- Has `is_runnable()` but no transition guard.

**Source:** `rust/src/api/session.rs`

States: `Draft`, `Ready`, `Executed`, `Cancelled`, `Failed`

- `is_runnable()` method returns true for Draft and Ready only.
- `RunSheet.runnable_dsl()` filters entries by this predicate to prevent re-execution.
- No explicit transition map.

### 6.2 ReplStateV2

**Severity: CLEAN** -- Not a status column FSM; it is an in-memory tagged enum with embedded context data per variant. Transitions are enforced by the orchestrator state machine (`orchestrator_v2.rs`). Not persisted as a status column.

States (tagged enum variants): `ScopeGate`, `JourneySelection`, `InPack`, `Clarifying`, `SentencePlayback`, `RunbookEditing`, `Executing`

### 6.3 RunbookStatus / EntryStatus / InvocationStatus

**Severity: FLAG** -- Three related status enums with no explicit transition maps.

**Source:** `rust/src/repl/runbook.rs`

- **RunbookStatus:** Draft, Building, Ready, Executing, Completed, Parked, Aborted
- **EntryStatus:** Proposed, Confirmed, Resolved, Executing, Completed, Failed, Parked, Disabled
- **InvocationStatus:** Active, Completed, TimedOut, Cancelled

No `is_valid_transition()` functions found for any of these. Status changes are made directly by the orchestrator without formal transition validation.

---

## 7. DB-Only CHECK Constraint FSMs (Pattern C)

These have PostgreSQL CHECK constraints defining valid values but **no Rust-side transition enforcement**:

| Entity / Table | Migration | Valid Values | Rust Enum? |
|---------------|-----------|-------------|------------|
| `legal_contracts` | 045 | DRAFT, ACTIVE, TERMINATED, EXPIRED | No |
| `cbu_subscriptions` | 045 | PENDING, ACTIVE, SUSPENDED, TERMINATED | No |
| `capital_events` | 013 | DRAFT, PENDING_APPROVAL, EFFECTIVE, REVERSED, CANCELLED | No |
| `dilution_instruments` | 013 | ACTIVE, EXERCISED, EXPIRED, FORFEITED, CANCELLED | No |
| `reconciliation` | 013 | RUNNING, COMPLETED, FAILED, CANCELLED | No |
| `resolution` | 013 | OPEN, ACKNOWLEDGED, INVESTIGATING, RESOLVED, FALSE_POSITIVE | No |
| `detected_patterns` | add_verification | DETECTED, INVESTIGATING, RESOLVED, FALSE_POSITIVE | No |
| `verification_challenges` | add_verification | OPEN, RESPONDED, RESOLVED, ESCALATED | No |
| `verification_escalations` | add_verification | PENDING, UNDER_REVIEW, DECIDED | No |
| `anomaly` | 015 | OPEN, ACKNOWLEDGED, RESOLVED, FALSE_POSITIVE | No |
| `document_instances` | 090 | pending, received, verified, rejected, expired | No |
| `cbu_resource_instances` | PRODUCTS_SERVICES_RESOURCES | PENDING, PROVISIONING, ACTIVE, SUSPENDED, DECOMMISSIONED | No |
| `placeholder_entities` | 066 | pending, resolved, verified, expired, rejected | No |
| `provisioning_ledger` | 026 | queued, sent, ack, completed, failed, cancelled | No |
| `service_intents` | 024 | active, suspended, cancelled | No |
| `rulesets` | PRODUCTS_SERVICES_RESOURCES | draft, active, retired | No |
| `agent_plans` | 085 | draft, active, completed, failed, cancelled | No |
| `plan_steps` | 085 | pending, running, completed, failed, skipped | No |
| `client_group_entity` (review_status) | 055 | pending, confirmed, rejected, needs_update | No |
| `client_group_research` (discovery_status) | 055 | not_started, in_progress, complete, stale, failed | No |

**Severity: FLAG (aggregate)** -- All of these can have any valid value set via any path. No guard prevents `legal_contracts` jumping from DRAFT directly to EXPIRED, for example.

---

## 8. Cross-Cutting Analysis

### 8.1 SUSPENDED State Consistency

Entities with SUSPENDED states:

| Entity | SUSPENDED Behavior | Can Reactivate? | Forward to Terminal? |
|--------|-------------------|-----------------|---------------------|
| CBU | ACTIVE<->SUSPENDED, SUSPENDED->TERMINATED | Yes | Yes |
| CBU Resource Instance | ACTIVE<->SUSPENDED, SUSPENDED->DECOMMISSIONED | Yes | Yes |
| Service Delivery | ACTIVE<->SUSPENDED, SUSPENDED->TERMINATED | Yes | Yes |
| CBU Subscriptions | *(DB CHECK only)* | Unknown | Unknown |
| Service Intents | *(DB CHECK only)* | Unknown | Unknown |

**Finding:** The three YAML-defined SUSPENDED patterns are **perfectly consistent**: bidirectional with ACTIVE, forward-only to the terminal state. The two DB-only entities (subscriptions, service intents) have no defined behavior.

### 8.2 Terminal State Handling

| Pattern | Entities | Method |
|---------|----------|--------|
| Explicit terminal list | KYC Case (hardcoded) | `CLOSE_STATUSES` constant + `is_terminal_status()` |
| Inferred (no outbound transitions) | All YAML entities | `terminal_states()` in lifecycle.rs (unused in prod) |
| Explicit `is_terminal()` method | ChangeSetStatus | Method on enum |
| None | All Pattern C entities | No terminal detection |

### 8.3 Enum Implementation Inconsistency

| Pattern | Modules | Derives |
|---------|---------|---------|
| Strum-based | sem_os_core types (SnapshotStatus, GovernanceTier, TrustClass, ChangeType, ObjectType, ChangeSetStatus) | `Display`, `EnumString`, `AsRefStr`, `#[strum(serialize_all = "snake_case")]` |
| Manual as_str/parse | BPMN integration (4 enums) | Hand-written `as_str()`, `parse()` methods |
| Serde-only | Session types (DslStatus, SessionMode) | `Serialize`, `Deserialize` with `#[serde(rename_all)]` |
| String constants | KYC Case, Deal | `&str` values, no enum type at all |

### 8.4 DB CHECK vs YAML vs Rust Alignment

| Entity | DB CHECK | YAML Lifecycle | Rust Enum | Rust Transition Validation | Aligned? |
|--------|----------|---------------|-----------|---------------------------|----------|
| KYC Case | *(not found)* | 7 states | *(string constants)* | Yes (hardcoded, 11 states) | **NO -- CRITICAL drift** |
| CBU | *(not found)* | 7 states | No | No | Partial (YAML only) |
| Entity | *(not found)* | 4 states | No | No | Partial (YAML only) |
| Deal | *(not found)* | *(none)* | *(string constants)* | Yes | No DB CHECK |
| Document | *(not found)* | 7 states | No | No | Partial (YAML only) |
| Legal Contract | Yes (4 values) | *(none)* | No | No | DB only |
| CBU Subscription | Yes (4 values) | *(none)* | No | No | DB only |
| SnapshotStatus | *(PG enum type)* | *(none)* | Yes (strum) | Yes (publish gates) | Aligned |
| ChangeSetStatus | Yes (9 values) | *(none)* | Yes (strum) | Yes (governance verbs) | Aligned |
| BPMN Correlation | Yes (4 values) | *(none)* | Yes (manual) | No | DB + Rust enum aligned, no guards |

### 8.5 Document Lifecycle: Migration 049 vs YAML Taxonomy

Migration 049 defines the **requirement** state machine: `missing -> requested -> received -> in_qa -> verified` (plus `rejected`, `waived`, `expired`).

The YAML taxonomy defines a **document** lifecycle: `PENDING -> REQUESTED -> RECEIVED -> UNDER_REVIEW -> VERIFIED -> EXPIRED` (plus `REJECTED`).

These represent different entities (requirement vs document) but describe overlapping concerns. The state names are similar but not identical (`in_qa` vs `UNDER_REVIEW`). This is likely intentional (requirements track what's needed, documents track what's received) but should be explicitly documented to prevent confusion.

---

## 9. Findings Summary

### CRITICAL

| ID | Entity | Finding |
|----|--------|---------|
| **C-1** | KYC Case | Two incompatible FSM definitions: YAML taxonomy (7 states: OPEN/IN_PROGRESS/PENDING_DECISION/APPROVED/REJECTED/CLOSED/CANCELLED) vs hardcoded CASE_TRANSITIONS (11 states: INTAKE/DISCOVERY/ASSESSMENT/REVIEW/BLOCKED/APPROVED/REJECTED/WITHDRAWN/EXPIRED/REFER_TO_REGULATOR/DO_NOT_ONBOARD). The hardcoded version is the runtime truth. YAML is stale. Any code relying on the ontology EntityLifecycle for KYC cases will get wrong answers. |
| **C-2** | KYC Case (EXPIRED) | EXPIRED is a terminal state in CASE_TRANSITIONS but has no inbound transition in the transition map. It appears to be set by an external process (timer/background job). There is no guard preventing a manual `kyc.update-status` to EXPIRED from a non-terminal state, since `is_terminal_status()` only blocks transitions *from* terminal states, not *to* them via the update-status verb. The close verb does validate. |

### FLAG

| ID | Entity | Finding |
|----|--------|---------|
| **F-1** | All YAML entities (13) | `ontology/lifecycle.rs` provides `is_valid_transition()`, `terminal_states()`, `validate_transition()` but these are marked `#[allow(dead_code)]` and never called from domain_ops handlers. The YAML-defined FSMs are metadata-only with no runtime enforcement. |
| **F-2** | Deal | No DB CHECK constraint on `deal_status` column. Transition validation is Rust-only. A raw SQL UPDATE or CRUD verb could set an invalid status value. |
| **F-3** | Rate Card | No explicit transition map function. Transitions enforced per-verb with ad-hoc status checks. Missing DRAFT->PROPOSED transition guard (not found in code). |
| **F-4** | Document (YAML) | No terminal states in the document lifecycle. VERIFIED->EXPIRED->REQUESTED creates an infinite loop possibility. Operationally correct but atypical FSM. |
| **F-5** | Runbook/Entry/Invocation | Three related status enums (RunbookStatus 7-state, EntryStatus 8-state, InvocationStatus 4-state) with no formal transition validation. Status set directly by orchestrator. |
| **F-6** | DB-only entities (20+) | All Pattern C entities have valid-value enforcement only. Any valid status can be set from any other valid status. No transition ordering enforced. |
| **F-7** | UBO Record | DISPUTED->DECLARED regression allows full state reset. Combined with DECLARED->EVIDENCED->DISPUTED loop, creates potential infinite cycling with no guard or counter. |
| **F-8** | Screening/Monitoring | ESCALATED is a terminal dead-end in both entities. Once escalated, the only option is to create a new record. This should be documented as intentional or considered for a recovery path. |

### MINOR

| ID | Entity | Finding |
|----|--------|---------|
| **M-1** | BPMN enums (4) | Manual `as_str()`/`parse()` instead of strum derives. Inconsistent with sem_os_core pattern. |
| **M-2** | DslStatus | Has `is_runnable()` but no transition guard. Low risk -- session entries are short-lived. |
| **M-3** | Entity lifecycle | INACTIVE is reversible (reactivation). No business rule check on INACTIVE->ACTIVE. |
| **M-4** | Document vs Requirement | Similar but non-identical state names (`in_qa` vs `UNDER_REVIEW`) for overlapping document QA concerns. |
| **M-5** | CBU lifecycle | 7 states with DISCOVERED->DRAFT->PENDING_VALIDATION->VALIDATED->ACTIVE -- multi-step activation may be over-engineered for the current CRUD-based operations. No evidence of PENDING_VALIDATION or VALIDATED being used in domain ops. |
| **M-6** | Agent plans/steps | DB CHECK constraints (5 states each) but no Rust enum. Status updates in `sem_reg/agent/plans.rs`. |
| **M-7** | Capital events | 5-state FSM (DRAFT->PENDING_APPROVAL->EFFECTIVE, with REVERSED/CANCELLED) -- DB CHECK only, no Rust enforcement. |
| **M-8** | Client group research | 3 different status columns (review_status, verification_status, discovery_status) with DB CHECK only. |
| **M-9** | Placeholder entities | 5-state lifecycle (pending->resolved->verified, with expired/rejected) with DB CHECK only. |
| **M-10** | Provisioning ledger | 6-state lifecycle (queued->sent->ack->completed, with failed/cancelled) with DB CHECK only. |
| **M-11** | Rulesets | 3-state lifecycle (draft->active->retired) with DB CHECK only. |
| **M-12** | Legal contracts | 4-state lifecycle (DRAFT->ACTIVE->TERMINATED/EXPIRED) with DB CHECK only. No Rust enum or transition validation. |

### CLEAN

| ID | Entity | Notes |
|----|--------|-------|
| **OK-1** | SnapshotStatus | Full alignment: PG enum, strum-derived Rust enum, publish gate enforcement. |
| **OK-2** | ChangeSetStatus | 9-state with `is_terminal()`, strum derives, DB CHECK, governance verb enforcement. |
| **OK-3** | Deal (transitions) | `is_valid_deal_status_transition()` covers all valid paths. Auto-transitions well-guarded. |
| **OK-4** | ReplStateV2 | Tagged enum with embedded data -- not a status column FSM. Correct pattern for in-memory state machine. |
| **OK-5** | SUSPENDED consistency | CBU, CBU Resource Instance, Service Delivery all use identical SUSPENDED pattern (bidirectional ACTIVE, forward to terminal). |
| **OK-6** | Doc Request | Well-defined 6-state FSM with clear terminals. |

---

## 10. Recommendations

### Immediate (P0)

1. **Reconcile KYC Case FSMs (C-1):** Update `entity_taxonomy.yaml` to match the 11-state `CASE_TRANSITIONS` that is actually enforced at runtime, OR refactor `kyc_case_ops.rs` to use the ontology `EntityLifecycle` framework. The current dual-definition state is dangerous -- any code path that relies on `entity_taxonomy.yaml` for KYC case lifecycle validation will produce wrong results.

2. **Guard EXPIRED transition (C-2):** Add an explicit guard in `KycCaseUpdateStatusOp` that prevents manual transition to EXPIRED. If EXPIRED is only reachable via background timer, the update-status verb should reject it.

### Short-Term (P1)

3. **Wire YAML lifecycles to domain ops (F-1):** The `ontology/lifecycle.rs` framework is complete and tested. Wire `is_valid_transition()` into the CRUD executor's update path so that all 13 YAML-defined entities get transition validation at runtime. Remove `#[allow(dead_code)]`.

4. **Add DB CHECK for deal_status (F-2):** Add a CHECK constraint on `"ob-poc".deals.deal_status` to match the Rust validation.

5. **Create explicit rate card transition map (F-3):** Extract an `is_valid_rate_card_transition()` function analogous to the deal status validator.

### Medium-Term (P2)

6. **Standardize enum patterns:** Migrate BPMN integration enums (M-1) to strum derives. Consider creating a `StatusEnum` trait with `as_str()`, `parse()`, `is_terminal()` methods.

7. **Document terminal-state-as-dead-end pattern (F-8):** If ESCALATED being a dead-end is intentional across Screening and Monitoring Event, document this as a design decision.

8. **Add transition validation to Pattern C entities (F-6):** Prioritize entities with business-critical state machines (legal_contracts, capital_events, cbu_subscriptions) for Rust-side transition enforcement.
