# KYC & UBO DSL Specification

This document defines the **KYC and UBO grammar** for the OB‑POC DSL, with a focus on:

- Modeling **Client Business Units (CBUs)** as the onboarding anchor
- Managing **KYC cases** and **entity workstreams**
- Representing **ownership chains** and **UBO determinations**
- Supporting **incremental discovery** and **versioned updates** of UBO information

The DSL is executed via the standard pipeline:

1. **Parser (Nom)** → AST (`VerbCall`, `Value`, etc.)
2. **CSG Linter** → context-sensitive validation
3. **Execution Plan** → compiled operations
4. **GenericCrudExecutor + plugins** → DB mutations and domain logic

Configuration is YAML-driven:

- `rust/config/verbs.yaml` — verb definitions and CRUD mappings
- `rust/config/csg_rules.yaml` — CSG validation rules
- `rust/config/rules.yaml` — event-driven rule engine (KYC orchestration)

---

## 1. Core Syntax

The DSL uses an S‑expression syntax:

```clojure
(domain.verb :arg1 value1 :arg2 value2 ... :as @binding)
```

- `domain.verb` — **verb name**; domain and action separated by dot
- `:arg` — **keyword arguments**, mapping to DB columns or plugin inputs
- `@binding` — optional symbol bound to the result for later reference

Example:

```clojure
(cbu.ensure :name "Acme Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)
(entity.create-proper-person :cbu-id @fund :first-name "John" :last-name "Smith" :as @john)
(cbu.assign-role :cbu-id @fund :entity-id @john :role "BENEFICIAL_OWNER" :ownership-percentage 100)
```

---

## 2. Domains Overview

Relevant KYC/UBO-related domains (subset of full DSL):

| Domain          | Purpose                                                              |
|----------------|----------------------------------------------------------------------|
| `cbu`          | Client Business Unit lifecycle (ensure, assign-role, attributes)     |
| `entity`       | Legal/physical entities (companies, funds, individuals, trusts)      |
| `kyc-case`     | KYC case lifecycle (create, status, escalation, risk rating, close)  |
| `entity-workstream` | Per-entity KYC workstreams within a case                        |
| `red-flag`     | Risk indicators and blocking issues                                  |
| `doc-request`  | Document collection / requirements                                   |
| `case-screening` | Screenings within KYC workstreams                                  |
| `allegation`   | Client allegations (unverified claims)                               |
| `observation`  | Attribute observations (evidence from documents/systems)             |
| `discrepancy`  | Conflicts between observations                                       |
| `ubo`          | Ownership relationships and UBO determinations                       |

---

## 3. CBU Grammar

The **CBU (Client Business Unit)** is the central client container for KYC, UBO, and services.

### 3.1 CBU Lifecycle

#### `cbu.ensure`

Upsert a CBU by unique keys (e.g. name + jurisdiction + client_type).

```clojure
(cbu.ensure
  :name "Acme Fund"
  :jurisdiction "LU"
  :client-type "FUND"
  :external-id "ACME-LU-001"      ; optional cross-ref
  :as @fund)
```

- **Behavior**: `crud` (upsert by unique business key)
- **maps_to**: `cbus` table (`name`, `jurisdiction`, `client_type`, `external_id`)

#### `cbu.update`

Update existing CBU attributes.

```clojure
(cbu.update
  :cbu-id @fund
  :name "Acme Growth Fund"
  :client-type "FUND")
```

- **Behavior**: `crud` (UPDATE)
- Used for incremental corrections/changes on CBU metadata.

### 3.2 CBU–Entity Role Grammar

#### `cbu.assign-role`

Assign a role to an entity within a CBU.

```clojure
(cbu.assign-role
  :cbu-id @fund
  :entity-id @john
  :role "BENEFICIAL_OWNER"
  :ownership-percentage 60
  :role-source "CLIENT_DISCLOSURE"
  :as @role1)
```

- **Behavior**: `crud` (INSERT into `cbu_entity_roles`)
- Example fields: `role`, `ownership_percentage`, `role_source`, `effective_from`, `effective_to`.

#### `cbu.update-role`

Update a CBU–entity role (e.g. change ownership percentage).

```clojure
(cbu.update-role
  :role-id @role1
  :ownership-percentage 75
  :effective-from "2025-01-01")
```

- **Behavior**: `crud` (UPDATE)
- Can be used for incremental corrections to **declared** ownership vs **evidenced** ownership.

#### `cbu.end-role`

End / deactivate a role.

```clojure
(cbu.end-role
  :role-id @role1
  :effective-to "2025-06-30"
  :reason "OWNERSHIP_TRANSFERRED")
```

- **Behavior**: `crud` (UPDATE status/effective_to)
- Maintains timeline of roles for a CBU.

---

## 4. Entity Grammar

Entities represent legal or natural persons attached to CBUs.

### 4.1 Creation

Entities are typically created with dynamic verbs derived from DB `entity_types`.

```clojure
(entity.create-limited-company
  :cbu-id @fund
  :name "Acme Holdings Ltd"
  :jurisdiction "GB"
  :registration-number "123456"
  :as @holdco)

(entity.create-proper-person
  :cbu-id @fund
  :first-name "John"
  :last-name "Smith"
  :date-of-birth "1980-01-15"
  :as @john)
```

- **Behavior**: `crud`
- **maps_to**: `entities` + `entity_attributes` / type-specific tables.

### 4.2 Updates

Standard patterns apply (not exhaustively listed):

```clojure
(entity.update
  :entity-id @john
  :first-name "John A."
  :last-name "Smith"
  :preferred-name "John Smith")
```

---

## 5. KYC Case & Workstream Grammar

### 5.1 Case Lifecycle

#### `kyc-case.create`

```clojure
(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :initiator "ONBOARDING_PORTAL"
  :as @case)
```

#### `kyc-case.update-status`

```clojure
(kyc-case.update-status
  :case-id @case
  :status "ASSESSMENT"
  :reason "INITIAL_SCREENING_COMPLETE")
```

#### Other Verbs

- `kyc-case.escalate` — escalate case to higher level
- `kyc-case.assign` — assign analyst/reviewer
- `kyc-case.set-risk-rating` — set `LOW`/`MEDIUM`/`HIGH` etc.
- `kyc-case.close` — close with status: `APPROVED`, `REJECTED`, `WITHDRAWN`

### 5.2 Entity Workstreams

#### `entity-workstream.create`

```clojure
(entity-workstream.create
  :case-id @case
  :entity-id @john
  :discovery-reason "BENEFICIAL_OWNER"
  :is-ubo true
  :as @ws-john)
```

#### Key verbs

- `entity-workstream.update-status`
- `entity-workstream.block`
- `entity-workstream.complete`
- `entity-workstream.set-enhanced-dd`
- `entity-workstream.set-ubo`

These verbs provide a **process‑level view** on top of the **ownership graph** defined in the `ubo` domain.

---

## 6. Evidence Model: Allegations, Observations, Discrepancies

### 6.1 Allegations

Represents **client-supplied claims**.

```clojure
(allegation.record
  :cbu-id @fund
  :entity-id @john
  :attribute-id "attr.ownership.percentage"
  :value 60
  :display-value "60%"
  :source "KYC_QUESTIONNAIRE"
  :case-id @case
  :as @alleg-ownership)
```

### 6.2 Observations

Represents **observed evidence** from documents or systems.

```clojure
(observation.record-from-document
  :entity-id @john
  :document-id @share-reg
  :attribute "attr.ownership.percentage"
  :value 58
  :extraction-method "OCR"
  :confidence 0.92
  :as @obs-from-reg)
```

### 6.3 Discrepancies

Capture **conflicts** between observations.

```clojure
(discrepancy.record
  :entity-id @john
  :attribute "attr.ownership.percentage"
  :observation-id-1 @obs-from-reg
  :observation-id-2 @other-obs
  :severity "MEDIUM"
  :as @disc1)
```

These domains make UBO determinations **evidence-based** and explainable.

---

## 7. UBO Grammar

The `ubo` domain models:

1. **Ownership relationships** (edges between entities).
2. **UBO determinations** (who is considered a UBO, under which rule).
3. **Verification and lifecycle** of those determinations.

### 7.1 Ownership Relationships

#### `ubo.add-ownership`

```clojure
(ubo.add-ownership
  :owner-entity-id @person
  :owned-entity-id @holdco
  :percentage 100
  :ownership-type "DIRECT"               ; DIRECT / INDIRECT / CONTROL
  :effective-from "2024-01-01"
  :as @own1)
```

#### `ubo.update-ownership`

```clojure
(ubo.update-ownership
  :ownership-id @own1
  :percentage 80
  :effective-from "2025-01-01")
```

#### `ubo.end-ownership`

```clojure
(ubo.end-ownership
  :ownership-id @own1
  :effective-to "2025-06-30"
  :reason "SHARES_SOLD")
```

#### Listing

```clojure
(ubo.list-owners
  :entity-id @fund-entity
  :as @owners-of-fund)

(ubo.list-owned
  :entity-id @person
  :as @entities-owned-by-person)
```

### 7.2 UBO Determinations

#### `ubo.register-ubo`

Registers a **UBO determination** for a CBU + subject entity.

```clojure
(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @fund-entity
  :ubo-person-id @person
  :relationship-type "OWNER"              ; OWNER / CONTROLLER / SIGNATORY
  :qualifying-reason "OWNERSHIP_25PCT"    ; regulatory rationale
  :ownership-percentage 60
  :case-id @case                          ; tie to KYC case
  :workstream-id @ws-ubo                  ; tie to specific workstream
  :workflow-type "ONBOARDING"
  :as @ubo1)
```

#### `ubo.verify-ubo`

```clojure
(ubo.verify-ubo
  :ubo-id @ubo1
  :verification-status "VERIFIED"         ; VERIFIED / TENTATIVE / REJECTED
  :risk-rating "LOW"
  :verification-method "DOCUMENTARY_EVIDENCE"
  :verified-by "ANALYST123")
```

#### `ubo.list-ubos` / `ubo.list-by-subject`

```clojure
(ubo.list-ubos
  :cbu-id @fund
  :as @fund-ubos)

(ubo.list-by-subject
  :subject-entity-id @fund-entity
  :as @entity-ubos)
```

---

## 8. New Discovery & Versioning Verbs

To make **incremental discovery** and **UBO versioning** first‑class, we introduce **new verbs**, implemented as **YAML‑defined plugin behaviors**.

### 8.1 Discovery Verbs

#### `ubo.discover-owner` (Plugin)

Used when an owner is **discovered** (e.g. via registry search), not declared.

```clojure
(ubo.discover-owner
  :cbu-id @fund
  :subject-entity-id @fund-entity
  :owner-name "Global Holdings Ltd"
  :jurisdiction "KY"
  :discovery-method "REGISTRY_SEARCH"
  :case-id @case
  :workstream-id @ws-company
  :as @discovery1)
```

**Behavior:**

- **Plugin**:
  - Tries to **find or create** an entity for the owner.
  - Creates `ubo.add-ownership` record(s) according to rules (e.g. 100% if registry says so).
  - Optionally records an **allegation** or **observation** for the ownership percentage.
  - Returns:
    - `owner_entity_id`
    - `ownership_id`
    - optionally `allegation_id` / `observation_id`.

Suggested `verbs.yaml` extract:

```yaml
- name: ubo.discover-owner
  behavior: plugin
  plugin: ubo_discover_owner
  args:
    - name: cbu-id
      type: uuid
    - name: subject-entity-id
      type: uuid
    - name: owner-name
      type: string
    - name: jurisdiction
      type: string
    - name: discovery-method
      type: string
    - name: case-id
      type: uuid?
    - name: workstream-id
      type: uuid?
```

#### `ubo.infer-chain` (Plugin)

Performs **graph analysis** to propose or materialize ownership chains for a CBU.

```clojure
(ubo.infer-chain
  :cbu-id @fund
  :subject-entity-id @fund-entity
  :threshold 25.0
  :max-depth 5
  :case-id @case
  :as @inferred-chain)
```

**Behavior:**

- Plugin queries ownership graph, calculates effective ownership.
- Returns:
  - `proposed_ubos` with computed percentages.
- Optionally:
  - Writes to `ubo` tables (materializing `ubo.register-ubo` records).
  - Or stores results in a staging table for review.

### 8.2 UBO Versioning Verbs

#### `ubo.supersede-ubo`

Marks a UBO determination as superseded by a new one.

```clojure
(ubo.supersede-ubo
  :previous-ubo-id @ubo1
  :new-ubo-id @ubo2
  :reason "OWNERSHIP_CHANGED"
  :case-id @case2)
```

- **Behavior**: `plugin` or `crud` wrapping two updates:
  - Old UBO status → `SUPERSEDED`.
  - New UBO status → `ACTIVE`.

#### `ubo.close-ubo`

Closes a UBO determination when no longer applicable.

```clojure
(ubo.close-ubo
  :ubo-id @ubo1
  :closure-reason "CBU_TERMINATED"
  :effective-to "2025-12-31")
```

- **Behavior**: `crud` (UPDATE).

### 8.3 Snapshot Verbs

#### `ubo.snapshot-cbu` (Plugin)

Stores a **point-in-time UBO snapshot** for a CBU.

```clojure
(ubo.snapshot-cbu
  :cbu-id @fund
  :snapshot-type "REGULATORY"
  :case-id @case
  :as @snapshot1)
```

- Behavior:
  - Plugin reads current UBOs, ownerships, statuses.
  - Writes a snapshot record (e.g. `ubo_snapshots` + `ubo_snapshot_lines`).
  - Enables future comparison.

#### `ubo.compare-snapshots` (Plugin)

Compares two snapshots.

```clojure
(ubo.compare-snapshots
  :snapshot-id-1 @snapshot1
  :snapshot-id-2 @snapshot2
  :as @diff)
```

- Returns structure describing:
  - Added/removed UBOs.
  - Ownership percentage changes.
  - Changes in risk/verification status.

---

## 9. End-to-End Incremental UBO Discovery Example

```clojure
;; 1. Ensure CBU and baseline entity
(cbu.ensure
  :name "Acme Fund"
  :jurisdiction "LU"
  :client-type "FUND"
  :as @fund)

(entity.create-limited-company
  :cbu-id @fund
  :name "Acme Holdings Ltd"
  :jurisdiction "GB"
  :as @holdco)

(entity.create-proper-person
  :cbu-id @fund
  :first-name "John"
  :last-name "Smith"
  :as @john)

;; 2. Create KYC case and workstreams
(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :as @case)

(entity-workstream.create
  :case-id @case
  :entity-id @holdco
  :discovery-reason "CLIENT_PRINCIPAL"
  :as @ws-company)

(entity-workstream.create
  :case-id @case
  :entity-id @john
  :discovery-reason "BENEFICIAL_OWNER"
  :is-ubo true
  :as @ws-ubo)

;; 3. Discover ownership via registry (plugin)
(ubo.discover-owner
  :cbu-id @fund
  :subject-entity-id @fund-entity      ; fund legal entity
  :owner-name "Acme Holdings Ltd"
  :jurisdiction "GB"
  :discovery-method "REGISTRY_SEARCH"
  :case-id @case
  :workstream-id @ws-company
  :as @disc-own1)

;; 4. Register UBO determination based on discovered chain
(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @fund-entity
  :ubo-person-id @john
  :relationship-type "OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 60
  :case-id @case
  :workstream-id @ws-ubo
  :workflow-type "ONBOARDING"
  :as @ubo1)

(ubo.verify-ubo
  :ubo-id @ubo1
  :verification-status "VERIFIED"
  :risk-rating "LOW"
  :verification-method "DOCUMENTARY_EVIDENCE")

;; 5. Snapshot the UBO state at approval
(ubo.snapshot-cbu
  :cbu-id @fund
  :snapshot-type "ONBOARDING_APPROVAL"
  :case-id @case
  :as @snapshot-onb)

(kyc-case.update-status
  :case-id @case
  :status "APPROVED")
```

---

## 10. Implementation Notes

- **Generic CRUD**:
  - Many verbs above (`cbu.ensure`, `cbu.assign-role`, `ubo.add-ownership`, etc.) are `behavior: crud` with `maps_to` definitions in `verbs.yaml`.
- **Plugins**:
  - Discovery/inference/versioning verbs (`ubo.discover-owner`, `ubo.infer-chain`, `ubo.snapshot-cbu`, `ubo.compare-snapshots`, `ubo.supersede-ubo`) are `behavior: plugin` and implemented in `rust/src/dsl_v2/custom_ops/ubo_*`.
- **CSG Rules**:
  - Add validations in `csg_rules.yaml`:
    - Ensure `:case-id`/`:workstream-id` are consistent with referenced records.
    - Enforce constraints like UBO percentage thresholds and required attributes.

This specification should be treated as the **authoritative definition** of the KYC & UBO DSL vocabulary and its semantics. Changes to the KYC/UBO model should be reflected here and then implemented via `verbs.yaml`, `rules.yaml`, and the corresponding Rust plugin modules.