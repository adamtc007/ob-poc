# OB-POC — Schema Entity Overview

> **Last reconciled:** 2026-02-11 — against 77 migrations, 57 DSL verb domains, CLAUDE.md
> **Scope:** `"ob-poc"` schema only (226 tables). External schemas (`custody`, `kyc`, `agent`, `teams`) referenced but not detailed.
> **Method:** SQL DDL cross-referenced with DSL verb YAML (`rust/config/verbs/*.yaml`) to validate domain groupings.

---

## Top-Level Domain Map

```mermaid
graph TB
    subgraph "Core Identity"
        ET[entity_types] --> E[entities]
        E --> EN[entity_names]
        E --> EI[entity_identifiers]
        E --> EP[entity_proper_persons]
        E --> EF[entity_funds]
        E --> ESUB[entity_* subtypes]
    end

    subgraph "CBU Aggregate"
        E --> CER[cbu_entity_roles]
        CBU[cbus] --> CER
        CBU --> CGM[cbu_group_members]
        CBU --> CPS[cbu_product_subscriptions]
        CBU --> CTP[cbu_trading_profiles]
        CBU --> CMO[cbu_matrix_product_overlay]
        CBU --> CRI[cbu_resource_instances]
        CBU --> CSR[cbu_service_readiness]
    end

    subgraph "Client Group & UBO"
        CG[client_group] --> CGE[client_group_entity]
        CG --> CGR[client_group_relationship]
        CGE --> E
        ER[entity_relationships] --> E
        UBO[ubo_registry] --> CBU
        UBO --> E
    end

    subgraph "Commercial"
        D[deals] --> CG
        D --> DP[deal_participants]
        D --> DRC[deal_rate_cards]
        D --> DON[deal_onboarding_requests]
        LC[legal_contracts] --> CP[contract_products]
        CP --> CSUB[cbu_subscriptions]
        FBP[fee_billing_profiles] --> D
    end

    subgraph "Product / Service / Resource"
        P[products] --> PS[product_services]
        PS --> S[services]
        S --> SRC[service_resource_capabilities]
        SRC --> SRT[service_resource_types]
    end

    subgraph "Document & Evidence"
        DOC[documents] --> DV[document_versions]
        DR[document_requirements] --> E
        DR --> DOC
    end

    CBU --> CG
    CBU --> E
    CPS --> P
    CSUB --> CP
    DON --> CBU
```

---

## Notation

- **Table names** are shown as in the DDL: `"ob-poc".table_name` (schema prefix omitted for readability).
- **PK / FK** are from `ALTER TABLE ... ADD CONSTRAINT ...` blocks.
- **Verb domain** shows which DSL verb YAML file operates on the table (e.g., `cbu.yaml` → 19 verbs).
- Tables with no verb domain are included only when essential to understanding the data structure.

---

## 1) Core Entity Model

**Verb domains:** `entity` (22 verbs), `identifier` (11), `fund` (20), `bods` (9), `regulatory` (5)

All other aggregates hang off a canonical `entities` table + typed taxonomy in `entity_types`.
Entity subtypes are modelled as separate satellite tables joined by `entity_id`.

```mermaid
erDiagram
    entity_types ||--o{ entity_types : "parent_type_id"
    entity_types ||--o{ entities : "entity_type_id"
    entities ||--o{ entity_names : "entity_id"
    entities ||--o{ entity_identifiers : "entity_id"
    entities ||--o{ entity_proper_persons : "entity_id"
    entities ||--o{ entity_funds : "entity_id"
    entities ||--o{ entity_limited_companies : "entity_id"
    entities ||--o{ entity_trusts : "entity_id"
    entities ||--o{ entity_partnerships : "entity_id"
    entities ||--o{ entity_foundations : "entity_id"
    entities ||--o{ entity_cooperatives : "entity_id"
    entities ||--o{ entity_government : "entity_id"
    entities ||--o{ entity_manco : "entity_id"
    entities ||--o{ entity_addresses : "entity_id"
    entities ||--o{ entity_share_classes : "entity_id"
    entities ||--o{ entity_relationships : "from_entity_id"

    entity_types {
        uuid entity_type_id PK
        varchar name UK
        uuid parent_type_id FK
        varchar type_code
        varchar entity_category
        text[] type_hierarchy_path
    }

    entities {
        uuid entity_id PK
        uuid entity_type_id FK
        varchar name
        varchar external_id
        varchar bods_entity_type
        date founding_date
        date dissolution_date
        boolean is_publicly_listed
        text name_norm
    }

    entity_names {
        uuid name_id PK
        uuid entity_id FK
        varchar name_type
        varchar name_value
    }

    entity_identifiers {
        uuid identifier_id PK
        uuid entity_id FK
        varchar identifier_type
        varchar identifier_value
        varchar issuing_authority
    }

    entity_proper_persons {
        uuid person_id PK
        uuid entity_id FK
        date date_of_birth
        varchar nationality
        varchar country_of_residence
    }

    entity_funds {
        uuid fund_id PK
        uuid entity_id FK
        varchar fund_type
        varchar gleif_category
        varchar domicile
    }

    entity_relationships {
        uuid relationship_id PK
        uuid from_entity_id FK
        uuid to_entity_id FK
        varchar relationship_type
        numeric percentage
        varchar ownership_type
        varchar control_type
        date effective_from
    }
```

**Subtype satellite tables** (one per legal form — joined by `entity_id`):

| Table | Legal Form | Key Columns |
|-------|-----------|-------------|
| `entity_proper_persons` | Natural persons | date_of_birth, nationality, country_of_residence |
| `entity_funds` | Funds / vehicles | fund_type, gleif_category, domicile |
| `entity_limited_companies` | Corporates | incorporation_country, share_capital |
| `entity_trusts` | Trusts | trust_type, governing_law |
| `entity_partnerships` | Partnerships | partnership_type |
| `entity_foundations` | Foundations | foundation_purpose |
| `entity_cooperatives` | Cooperatives | cooperative_type |
| `entity_government` | Government bodies | government_level |
| `entity_manco` | Management companies | manco_type, regulated_by |

**Supporting tables:**

| Table | Purpose |
|-------|---------|
| `entity_addresses` | Registered / operational addresses |
| `entity_share_classes` | Share class definitions per entity |
| `entity_parent_relationships` | Direct parent hierarchy (GLEIF-sourced) |
| `entity_lifecycle_events` | Lifecycle events (incorporation, dissolution) |
| `entity_bods_links` | Links to BODS statement IDs |
| `entity_concept_link` | Semantic concept associations (for entity linking) |
| `entity_feature` | Feature flags for ML/entity linking |

---

## 2) CBU Aggregate (Client Business Unit)

**Verb domains:** `cbu` (19 verbs), `cbu-role-v2` (10), `trading-profile` (47), `cash-sweep` (9), `investment-manager` (7), `pricing-config` (14)

CBU is the primary *case container* for onboarding + KYC scope. It carries light scalar fields plus several `jsonb` contexts. Most of the "CBU struct" is expressed via child tables.

```mermaid
erDiagram
    cbus ||--o{ cbu_entity_roles : "cbu_id"
    cbus ||--o{ cbu_group_members : "cbu_id"
    cbus ||--o{ cbu_product_subscriptions : "cbu_id"
    cbus ||--o{ cbu_trading_profiles : "cbu_id"
    cbus ||--o{ cbu_matrix_product_overlay : "cbu_id"
    cbus ||--o{ cbu_resource_instances : "cbu_id"
    cbus ||--o{ cbu_service_readiness : "cbu_id"
    cbus ||--o{ cbu_sla_commitments : "cbu_id"
    cbus ||--o{ cbu_attr_values : "cbu_id"
    cbus ||--o{ cbu_evidence : "cbu_id"
    cbus ||--o{ cbu_lifecycle_instances : "cbu_id"
    cbus ||--o{ cbu_subscriptions : "cbu_id"
    cbus }o--|| entities : "commercial_client_entity_id"
    cbus }o--o| products : "product_id"
    cbu_entity_roles }o--|| entities : "entity_id"
    cbu_entity_roles }o--|| roles : "role_id"
    cbu_group_members }o--|| cbu_groups : "group_id"
    cbu_product_subscriptions }o--|| products : "product_id"

    cbus {
        uuid cbu_id PK
        varchar name
        varchar jurisdiction
        varchar client_type
        varchar cbu_category
        varchar status
        varchar kyc_scope_template
        jsonb risk_context
        jsonb onboarding_context
        jsonb semantic_context
        uuid commercial_client_entity_id FK
        uuid product_id FK
    }

    cbu_entity_roles {
        uuid cbu_entity_role_id PK
        uuid cbu_id FK
        uuid entity_id FK
        uuid role_id FK
        uuid target_entity_id FK
        numeric ownership_percentage
        numeric authority_limit
        date effective_from
        date effective_to
    }

    cbu_trading_profiles {
        uuid profile_id PK
        uuid cbu_id FK
        varchar profile_name
        varchar status
    }

    cbu_product_subscriptions {
        uuid subscription_id PK
        uuid cbu_id FK
        uuid product_id FK
        varchar status
        date effective_from
        jsonb config
    }

    cbu_resource_instances {
        uuid instance_id PK
        uuid cbu_id FK
        uuid product_id FK
        uuid service_id FK
        uuid resource_type_id FK
        varchar instance_url
        varchar status
        uuid market_id FK
        varchar currency
    }

    cbu_groups {
        uuid group_id PK
        uuid manco_entity_id FK
        varchar group_name
        varchar group_type
        uuid ultimate_parent_entity_id FK
    }
```

**CBU child tables:**

| Table | Verb Domain | Purpose |
|-------|------------|---------|
| `cbu_entity_roles` | `cbu-role-v2` | Entity-to-CBU role assignments (depositary, IM, etc.) |
| `cbu_entity_roles_history` | — | Audit trail of role changes |
| `cbu_group_members` | `manco-group` | CBU membership in governance groups |
| `cbu_groups` | `manco-group` | Governance book groups (ManCo, apex parent) |
| `cbu_product_subscriptions` | `matrix-overlay` | Product subscriptions per CBU |
| `cbu_trading_profiles` | `trading-profile` | Trading mandate profiles (47 verbs) |
| `cbu_matrix_product_overlay` | `matrix-overlay` | Per-cell instrument/market/currency config |
| `cbu_resource_instances` | `service-resource` | Provisioned resource instances |
| `cbu_service_readiness` | — | Computed service readiness status |
| `cbu_sla_commitments` | `sla` | SLA commitments per CBU |
| `cbu_lifecycle_instances` | `lifecycle` | Active lifecycle instances |
| `cbu_subscriptions` | `contract` | Contract+product subscription (onboarding gate) |
| `cbu_attr_values` | — | Attribute dictionary values per CBU |
| `cbu_evidence` | `cbu` | Document/attestation evidence links |
| `cbu_pricing_config` | `pricing-config` | NAV pricing configuration |
| `cbu_change_log` | — | CBU change audit trail |
| `cbu_creation_log` | — | CBU creation audit trail |

---

## 3) Client Group + Ownership Graph + UBO

**Verb domains:** `client-group` (23 verbs), `ubo` (22), `ownership` (16), `control` (15), `manco-group` (16), `gleif` (16)

Two layers:
1. **Client Group discovery/curation** — candidate entities and proposed relationships (review workflow)
2. **Canonical entity relationships + UBO registry** — promoted graph and per-CBU/case UBO assertions

```mermaid
erDiagram
    client_group ||--o{ client_group_entity : "group_id"
    client_group ||--o{ client_group_relationship : "group_id"
    client_group ||--o{ client_group_alias : "group_id"
    client_group ||--o{ client_group_anchor : "group_id"
    client_group_entity }o--|| entities : "entity_id"
    client_group_entity }o--o| cbus : "cbu_id"
    client_group_entity ||--o{ client_group_entity_roles : "id"
    client_group_relationship }o--|| entities : "parent_entity_id"
    client_group_relationship }o--|| entities : "child_entity_id"
    client_group_relationship }o--o| entity_relationships : "promoted_to_relationship_id"
    entity_relationships }o--|| entities : "from_entity_id"
    entity_relationships }o--|| entities : "to_entity_id"
    ubo_registry }o--|| cbus : "cbu_id"
    ubo_registry }o--|| entities : "subject_entity_id"
    ubo_registry }o--|| entities : "ubo_proper_person_id"
    ubo_registry ||--o{ ubo_evidence : "ubo_id"
    ubo_snapshots }o--|| cbus : "cbu_id"
    ubo_snapshot_comparisons }o--|| ubo_snapshots : "baseline_snapshot_id"
    ubo_snapshot_comparisons }o--|| ubo_snapshots : "current_snapshot_id"

    client_group {
        uuid id PK
        text canonical_name
        text short_code UK
        varchar discovery_status
        varchar discovery_root_lei
        integer entity_count
        integer pending_review_count
    }

    client_group_entity {
        uuid id PK
        uuid group_id FK
        uuid entity_id FK
        uuid cbu_id FK
        text membership_type
        varchar review_status
    }

    client_group_relationship {
        uuid id PK
        uuid group_id FK
        uuid parent_entity_id FK
        uuid child_entity_id FK
        varchar relationship_kind
        varchar review_status
        uuid promoted_to_relationship_id FK
    }

    ubo_registry {
        uuid ubo_id PK
        uuid cbu_id FK
        uuid subject_entity_id FK
        uuid ubo_proper_person_id FK
        varchar relationship_type
        varchar qualifying_reason
        numeric ownership_percentage
        varchar verification_status
        varchar screening_result
    }

    ubo_snapshots {
        uuid snapshot_id PK
        uuid cbu_id FK
        uuid case_id FK
        jsonb ubos
        jsonb ownership_chains
        jsonb control_relationships
        boolean has_gaps
    }
```

**Client Group supporting tables:**

| Table | Purpose |
|-------|---------|
| `client_group_alias` | Searchable aliases with embeddings (for "allianz" → Allianz GI resolution) |
| `client_group_alias_embedding` | Versioned embeddings per alias |
| `client_group_anchor` | Role-based anchors per jurisdiction (governance_controller, ultimate_parent) |
| `client_group_anchor_role` | Anchor role types |
| `client_group_entity_roles` | GLEIF roles (SUBSIDIARY, ULTIMATE_PARENT) from research phase |
| `client_group_entity_tag` | Entity classification tags |
| `client_group_relationship_sources` | Source provenance for relationships |

**UBO/Ownership supporting tables:**

| Table | Purpose |
|-------|---------|
| `ubo_evidence` | Document/attestation evidence for UBO assertions |
| `ubo_assertion_log` | Audit log of assertion results per CBU/case |
| `ubo_snapshot_comparisons` | Diff between two UBO snapshots (added/removed/changed) |
| `entity_ubos` | Legacy BODS-style UBO records per entity |
| `control_edges` | Control relationship edges (board, voting) |
| `entity_relationships_history` | Temporal history of relationship changes |

**External schema reference:** `kyc.cases` (case_id) — KYC cases link to UBO registry and snapshots.

---

## 4) Product / Service / Resource Model

**Verb domains:** `product` (2 verbs), `service` (3), `service-resource` (10), `service-pipeline` (14), `delivery` (3), `lifecycle` (16), `sla` (17)

This cluster models what a CBU subscribes to (Products), what those products compose (Services), and the resource catalog + provisioning instances required to deliver services.

```mermaid
erDiagram
    products ||--o{ product_services : "product_id"
    product_services }o--|| services : "service_id"
    services ||--o{ service_resource_capabilities : "service_id"
    service_resource_capabilities }o--|| service_resource_types : "resource_id"
    service_resource_types ||--o{ resource_dependencies : "resource_type_id"
    service_resource_types ||--o{ resource_attribute_requirements : "resource_id"
    cbus ||--o{ cbu_product_subscriptions : "cbu_id"
    cbu_product_subscriptions }o--|| products : "product_id"
    cbus ||--o{ service_intents : "cbu_id"
    service_intents }o--|| services : "service_id"
    cbus ||--o{ service_delivery_map : "cbu_id"
    service_delivery_map }o--|| services : "service_id"
    cbus ||--o{ cbu_resource_instances : "cbu_id"
    cbu_resource_instances }o--|| service_resource_types : "resource_type_id"
    cbu_resource_instances ||--o{ resource_instance_dependencies : "instance_id"
    cbu_resource_instances ||--o{ resource_instance_attributes : "instance_id"
    lifecycles ||--o{ lifecycle_resource_capabilities : "lifecycle_id"
    lifecycle_resource_capabilities }o--|| lifecycle_resource_types : "resource_type_id"
    instrument_lifecycles }o--|| lifecycles : "lifecycle_id"

    products {
        uuid product_id PK
        varchar name UK
        varchar product_code UK
        varchar product_category
        boolean requires_kyc
        jsonb metadata
    }

    services {
        uuid service_id PK
        varchar name UK
        varchar service_code UK
        varchar service_category
        jsonb sla_definition
    }

    service_resource_types {
        uuid resource_id PK
        varchar name UK
        varchar resource_code UK
        varchar owner
        boolean per_market
        boolean per_currency
        boolean per_counterparty
        varchar provisioning_verb
        jsonb capabilities
    }

    service_delivery_map {
        uuid delivery_id PK
        uuid cbu_id FK
        uuid product_id FK
        uuid service_id FK
        uuid instance_id FK
        varchar delivery_status
    }

    lifecycles {
        uuid lifecycle_id PK
        varchar code UK
        varchar name
        varchar category
        varchar owner
    }
```

**Delivery & provisioning tables:**

| Table | Purpose |
|-------|---------|
| `service_intents` | What service a CBU desires (intent → delivery) |
| `service_delivery_map` | Actual delivery tracking (status, timeline) |
| `service_availability` | Service availability windows |
| `provisioning_requests` | Resource provisioning request tracking |
| `provisioning_events` | Provisioning event audit trail |
| `resource_dependencies` | Type-level resource dependency graph |
| `resource_instance_dependencies` | Instance-level dependency edges |
| `resource_instance_attributes` | Attribute values on provisioned instances |
| `resource_attribute_requirements` | Required attributes per resource type |

**SLA tables:**

| Table | Purpose |
|-------|---------|
| `sla_templates` | SLA definition templates |
| `sla_measurements` | Measured SLA metrics |
| `sla_breaches` | Recorded SLA breaches |
| `sla_metric_types` | Metric type definitions |
| `cbu_sla_commitments` | SLA commitments per CBU |

---

## 5) Instrument Matrix & Trading Profile

**Verb domains:** `matrix-overlay` (14 verbs), `trading-profile` (47), `pricing-config` (14), `capital` (21)

The instrument matrix is a CBU-scoped overlay keyed by instrument class + market + currency + counterparty. Trading profiles define the mandate; pricing config controls NAV/valuation.

```mermaid
erDiagram
    cbus ||--o{ cbu_matrix_product_overlay : "cbu_id"
    cbu_matrix_product_overlay }o--o| cbu_product_subscriptions : "subscription_id"
    cbus ||--o{ cbu_trading_profiles : "cbu_id"
    cbus ||--o{ cbu_pricing_config : "cbu_id"
    instrument_lifecycles }o--|| lifecycles : "lifecycle_id"

    cbu_matrix_product_overlay {
        uuid overlay_id PK
        uuid cbu_id FK
        uuid instrument_class_id FK
        uuid market_id FK
        varchar currency
        uuid counterparty_entity_id FK
        uuid subscription_id FK
        jsonb additional_services
        jsonb additional_slas
        jsonb additional_resources
        jsonb product_specific_config
        varchar status
    }

    cbu_trading_profiles {
        uuid profile_id PK
        uuid cbu_id FK
        varchar profile_name
        varchar status
    }
```

**External schema references:** `custody.instrument_classes` (class_id), `custody.markets` (market_id) — referenced by overlay FKs.

**Supporting tables:**

| Table | Purpose |
|-------|---------|
| `instrument_lifecycles` | Links instrument classes to lifecycle processes |
| `trading_profile_materializations` | Materialized trading profile snapshots |
| `cbu_pricing_config` | NAV pricing rules per CBU |
| `cbu_pricing_fallback_chains` | Pricing fallback chain ordering |
| `cbu_stale_price_policies` | Stale price handling policies |
| `cbu_nav_impact_thresholds` | NAV impact thresholds |
| `cbu_valuation_schedule` | Valuation schedule config |
| `fund_structure` | Fund hierarchy (umbrella → sub-funds) |
| `fund_investments` | Fund investment allocations |
| `fund_metadata` | Additional fund metadata |

---

## 6) Legal Contracts & Onboarding Gate

**Verb domains:** `contract` (14 verbs), `contract-pack` (2)

CBU onboarding requires a contract+product subscription. No contract = no onboarding. The `cbu_subscriptions` table is the **gate**.

```mermaid
erDiagram
    legal_contracts ||--o{ contract_products : "contract_id"
    contract_products ||--o{ cbu_subscriptions : "contract_id, product_code"
    cbu_subscriptions }o--|| cbus : "cbu_id"
    contract_products }o--o| rate_cards : "rate_card_id"

    legal_contracts {
        uuid contract_id PK
        varchar client_label
        varchar contract_reference
        date effective_date
        date termination_date
        varchar status
    }

    contract_products {
        uuid contract_id PK_FK
        varchar product_code PK
        uuid rate_card_id FK
    }

    cbu_subscriptions {
        uuid cbu_id PK_FK
        uuid contract_id PK_FK
        varchar product_code PK_FK
        varchar status
    }

    rate_cards {
        uuid rate_card_id PK
        varchar name
        varchar currency
    }
```

**Supporting tables:**

| Table | Purpose |
|-------|---------|
| `contract_pack` | Grouped contract packages |
| `contract_template` | Contract template definitions |

---

## 7) Deal Record & Fee Billing

**Verb domains:** `deal` (42 verbs), `billing` (17)

Deal Record is the commercial origination hub linking sales → contracting → onboarding → servicing → billing.

```mermaid
erDiagram
    deals ||--o{ deal_participants : "deal_id"
    deals ||--o{ deal_contracts : "deal_id"
    deals ||--o{ deal_rate_cards : "deal_id"
    deals ||--o{ deal_slas : "deal_id"
    deals ||--o{ deal_documents : "deal_id"
    deals ||--o{ deal_ubo_assessments : "deal_id"
    deals ||--o{ deal_onboarding_requests : "deal_id"
    deals ||--o{ deal_events : "deal_id"
    deals ||--o{ deal_products : "deal_id"
    deals }o--|| client_group : "primary_client_group_id"
    deal_participants }o--|| entities : "entity_id"
    deal_rate_cards ||--o{ deal_rate_card_lines : "rate_card_id"
    deal_onboarding_requests }o--o| cbus : "cbu_id"
    fee_billing_profiles }o--|| deals : "deal_id"
    fee_billing_profiles ||--o{ fee_billing_account_targets : "profile_id"
    fee_billing_profiles ||--o{ fee_billing_periods : "profile_id"
    fee_billing_periods ||--o{ fee_billing_period_lines : "period_id"
    fee_billing_account_targets }o--|| cbus : "cbu_id"

    deals {
        uuid deal_id PK
        varchar deal_name
        varchar deal_reference UK
        uuid primary_client_group_id FK
        varchar sales_owner
        varchar deal_status
        numeric estimated_revenue
        varchar currency_code
    }

    deal_participants {
        uuid deal_participant_id PK
        uuid deal_id FK
        uuid entity_id FK
        varchar participant_role
    }

    deal_rate_cards {
        uuid rate_card_id PK
        uuid deal_id FK
        varchar version
        varchar status
    }

    deal_rate_card_lines {
        uuid line_id PK
        uuid rate_card_id FK
        varchar fee_type
        varchar pricing_model
        numeric rate_value
        varchar currency_code
    }

    deal_onboarding_requests {
        uuid request_id PK
        uuid deal_id FK
        uuid cbu_id FK
        varchar status
    }

    fee_billing_profiles {
        uuid profile_id PK
        uuid deal_id FK
        varchar billing_frequency
        varchar invoice_currency
    }

    fee_billing_periods {
        uuid period_id PK
        uuid profile_id FK
        date period_start
        date period_end
        varchar status
    }

    fee_billing_period_lines {
        uuid line_id PK
        uuid period_id FK
        varchar fee_type
        numeric calculated_amount
    }
```

**Deal status state machine:** `PROSPECT → QUALIFYING → NEGOTIATING → CONTRACTED → ONBOARDING → ACTIVE → WINDING_DOWN → OFFBOARDED` (any → `CANCELLED`)

**Rate card status:** `DRAFT → PROPOSED → COUNTER_OFFERED ↔ REVISED → AGREED → SUPERSEDED`

**Pricing models:** `BPS` (basis points on AUM), `FLAT` (fixed fee), `TIERED` (volume-based), `PER_TRANSACTION`

---

## 8) Document & Evidence Model

**Verb domains:** `document` (13 verbs), `requirement` (10), `attribute` (11), `docs-bundle` (3)

Two document models coexist:
- **Legacy:** `document_catalog` + `document_types` — flat catalog with extraction status
- **V2 (049):** `documents` → `document_versions` — three-layer model (requirement → document → version)

```mermaid
erDiagram
    document_requirements }o--|| entities : "entity_id"
    document_requirements }o--o| documents : "document_id"
    documents ||--o{ document_versions : "document_id"
    documents }o--o| entities : "entity_id"
    documents }o--o| cbus : "cbu_id"
    attribute_registry ||--o{ document_attribute_links : "attribute_id"
    document_attribute_links }o--|| document_types : "document_type_id"
    attribute_registry ||--o{ attribute_observations : "attribute_id"
    attribute_observations }o--|| entities : "entity_id"
    attribute_registry ||--o{ attribute_values_typed : "attribute_id"
    attribute_registry ||--o{ cbu_attr_values : "attr_id"

    document_requirements {
        uuid requirement_id PK
        uuid entity_id FK
        uuid document_id FK
        varchar doc_type
        varchar status
        varchar min_state
    }

    documents {
        uuid document_id PK
        uuid entity_id FK
        uuid cbu_id FK
        varchar document_type
        varchar status
    }

    document_versions {
        uuid version_id PK
        uuid document_id FK
        varchar storage_key
        varchar verification_status
        varchar rejection_code
    }

    attribute_registry {
        text id PK
        uuid uuid UK
        text display_name
        text category
        text value_type
        jsonb validation_rules
        jsonb applicability
    }

    attribute_observations {
        uuid observation_id PK
        uuid entity_id FK
        uuid attribute_id FK
        varchar source_type
        numeric confidence
        boolean is_authoritative
        varchar status
    }
```

**Requirement state machine:** `missing → requested → received → in_qa → verified` (also: `rejected → retry`, `waived`, `expired`)

**Rejection reason codes** (`rejection_reason_codes` table): Standardized codes for document QA — categories: quality, mismatch, validity, data, format, authenticity.

**Attribute dictionary tables:**

| Table | Purpose |
|-------|---------|
| `attribute_registry` | Central attribute definitions (type, validation, applicability) |
| `attribute_values_typed` | Typed attribute values per entity (denormalized columns) |
| `attribute_observations` | Observed values with source, confidence, supersession chain |
| `cbu_attr_values` | Attribute values per CBU (with evidence refs) |
| `document_attribute_links` | Policy-style proof links (document type → attribute) |
| `document_attribute_mappings` | Extraction-oriented field mappings |
| `observation_discrepancies` | Detected discrepancies between observation sources |

**Legacy document tables:**

| Table | Purpose |
|-------|---------|
| `document_catalog` | Flat document catalog (pre-V2) |
| `document_types` | Document type definitions with required attributes |
| `document_bundles` | Grouped document sets |
| `document_events` | Document lifecycle event log |

---

## 9) Workflow Task Queue

**Verb domains:** `runbook` (7 verbs) — most workflow tables are infrastructure for the BPMN/task engine

The task queue provides an async return path for long-running operations (document solicitation, human approvals).

```mermaid
erDiagram
    workflow_pending_tasks ||--o{ task_result_queue : "task_id"
    workflow_pending_tasks }o--|| workflow_instances : "workflow_instance_id"
    task_result_dlq }o--|| workflow_pending_tasks : "task_id"
    workflow_task_events }o--|| workflow_pending_tasks : "task_id"

    workflow_pending_tasks {
        uuid task_id PK
        uuid workflow_instance_id FK
        varchar task_type
        varchar status
        jsonb payload
        varchar callback_url
    }

    task_result_queue {
        uuid result_id PK
        uuid task_id FK
        varchar status
        jsonb items
        varchar idempotency_key
    }
```

| Table | Purpose |
|-------|---------|
| `workflow_pending_tasks` | Outbound task tracking (emitted by workflows) |
| `task_result_queue` | Inbound results (ephemeral, deleted after processing) |
| `task_result_dlq` | Dead letter queue for failed processing |
| `workflow_task_events` | Permanent audit trail |
| `workflow_instances` | Active workflow instances |
| `workflow_definitions` | Workflow definitions |
| `workflow_audit_log` | Workflow execution audit |
| `staged_runbook` | Staged REPL runbook container |
| `staged_command` | Individual staged DSL commands |
| `staged_command_entity` | Resolved entity footprint per command |
| `staged_command_candidate` | Picker candidates for ambiguous resolution |
| `rejection_reason_codes` | Reference data for document QA rejection reasons |

---

## 10) Screening & KYC Support Tables (in ob-poc)

**Verb domains:** `screening` (3 verbs), `kyc-agreement` (4)

The main KYC tables live in the `kyc` schema. These `ob-poc` tables support KYC integration.

| Table | Purpose |
|-------|---------|
| `kyc_service_agreements` | KYC service agreements between CBU and provider |
| `kyc_decisions` | KYC decision records |
| `screening_lists` | Screening list definitions (sanctions, PEP) |
| `screening_requirements` | Screening requirements per entity type |
| `screening_types` | Screening type taxonomy |
| `person_pep_status` | PEP status records per person entity |
| `verification_challenges` | Verification challenge questions |
| `verification_escalations` | Escalated verification cases |
| `risk_ratings` | Risk rating definitions |
| `risk_bands` | Risk band thresholds |
| `case_types` | Case type taxonomy (NEW_CLIENT, PERIODIC_REVIEW, etc.) |
| `case_evaluation_snapshots` | Point-in-time case evaluation captures |

**External schema:** `kyc.cases` is the primary KYC case table (referenced by `ubo_registry.case_id`).

---

## 11) BODS (Beneficial Ownership Data Standard)

**Verb domain:** `bods` (9 verbs)

BODS tables store structured beneficial ownership statements per the Open Ownership standard.

| Table | Purpose |
|-------|---------|
| `bods_entity_types` | BODS entity type taxonomy |
| `bods_interest_types` | BODS interest type taxonomy (ownership, control, trust) |
| `bods_entity_statements` | Entity statements (legal entities in ownership chains) |
| `bods_person_statements` | Person statements (natural persons in ownership chains) |
| `bods_ownership_statements` | Ownership/control statements linking persons to entities |

---

## 12) GLEIF Integration

**Verb domain:** `gleif` (16 verbs)

| Table | Purpose |
|-------|---------|
| `gleif_lei_records` | Cached LEI records from GLEIF API |
| `gleif_relationships` | Cached GLEIF relationship records (parent/child) |
| `gleif_sync_log` | GLEIF sync operation audit log |

---

## 13) Booking & Client Principal

**Verb domains:** `booking-location` (3 verbs), `booking-principal` (9), `client-principal-relationship` (4)

| Table | Purpose |
|-------|---------|
| `booking_location` | Booking location definitions |
| `booking_principal` | Booking principal entities |
| `client_principal_relationship` | Links clients to principals |

---

## 14) Reference Taxonomies

These tables are small but load-bearing — they drive interpretation, UI grouping, and rule selection.

| Table | Verb Domain | Purpose |
|-------|------------|---------|
| `roles` | `cbu-role-v2` | Role taxonomy (depositary, IM, director, etc.) |
| `role_types` | — | Role type classification |
| `role_categories` | — | Role category grouping |
| `role_applicable_entity_types` | — | Which entity types can hold which roles |
| `currencies` | — | Currency reference data |
| `master_jurisdictions` | `fund` | Jurisdiction definitions |
| `settlement_types` | — | Settlement type taxonomy |
| `ssi_types` | — | SSI type taxonomy |
| `view_modes` | — | View mode definitions |
| `edge_types` | — | Edge type taxonomy (for graph rendering) |
| `node_types` | — | Node type taxonomy (for graph rendering) |
| `regulators` | `regulatory` | Regulatory body definitions |
| `placeholder_kinds` | — | Placeholder entity kinds |
| `client_types` | — | Client type taxonomy |
| `dictionary` | — | General-purpose dictionary entries |
| `rule` | `rule` (3 verbs) | Business rule definitions |
| `ruleset` | `ruleset` (3 verbs) | Rule set groupings |
| `rule_field_dictionary` | — | Rule field definitions |

---

## Schema Statistics

| Metric | Count |
|--------|-------|
| Total `ob-poc` tables | 226 |
| Tables with DSL verb domains | ~85 |
| Tables in this document | ~150 (essential to data model) |
| Tables omitted (DSL engine, REPL, semantic search, layout cache) | ~76 |
| DSL verb domains | 57 |
| Total verb count | ~750+ |
| Migrations | 77 (001–077 + 072b) |

**Omitted infrastructure tables** (no verb domains, not essential to data model):
- DSL engine: `dsl_verbs`, `dsl_sessions`, `dsl_instances`, `dsl_snapshots`, `dsl_*` (14 tables)
- Semantic search: `verb_pattern_embeddings`, `verb_centroids`, `semantic_match_cache`, `detected_patterns`, `intent_feedback*`
- REPL: `repl_sessions_v2`, `repl_invocation_records`
- BPMN integration: `bpmn_correlations`, `bpmn_job_frames`, `bpmn_parked_tokens`, `bpmn_pending_dispatches`, `expansion_reports`
- Session/layout: `sessions`, `session_scopes`, `session_scope_history`, `session_bookmarks`, `layout_cache`, `layout_config`
- Audit: `sheet_execution_audit`, `cbu_board_controller`, `board_control_evidence`, `cbu_control_anchors`
