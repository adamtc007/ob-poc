# Database Schema Reference

**Database**: `data_designer` on PostgreSQL 17  
**Schemas**: `ob-poc` (51 tables), `custody` (17 tables), `kyc` (11 tables)  
**Updated**: 2025-12-02

## Overview

This document describes the database schema used by the OB-POC KYC/AML onboarding system. The schema supports:

- **Core KYC/AML**: CBUs, entities, documents, screening, KYC investigations
- **Service Delivery**: Products, services, resource instances
- **Custody & Settlement**: Three-layer model (Universe → SSI → Booking Rules)
- **Investor Registry**: Fund share classes, holdings, and movements (Clearstream-style)
- **Agentic DSL Generation**: The `rust/src/agentic/` module generates DSL that creates records in these tables

## Core Tables

### cbus (Client Business Units)

The central entity representing a client relationship.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| cbu_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | Client name |
| description | text | | | Description |
| nature_purpose | text | | | Nature and purpose of business |
| source_of_funds | text | | | Source of funds |
| client_type | varchar(100) | | | FUND, CORPORATE, INDIVIDUAL, etc. |
| jurisdiction | varchar(50) | | | Primary jurisdiction code |
| risk_context | jsonb | | '{}' | Risk assessment context |
| onboarding_context | jsonb | | '{}' | Onboarding workflow context |
| semantic_context | jsonb | | '{}' | AI/semantic context |
| embedding | vector | | | pgvector embedding |
| commercial_client_entity_id | uuid | YES | | FK to entities - head office that contracted with bank |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entities (Base Entity Table)

Base table for all entity types (Class Table Inheritance pattern).

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| entity_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_type_id | uuid | NOT NULL | | FK to entity_types |
| external_id | varchar(255) | | | External system reference |
| name | varchar(255) | NOT NULL | | Display name |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_types (Entity Type Registry)

Defines available entity types and their extension tables.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| entity_type_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | Display name |
| type_code | varchar(100) | | | Code for DSL verbs (e.g., 'proper_person') |
| table_name | varchar(255) | NOT NULL | | Extension table name |
| description | text | | | |
| parent_type_id | uuid | | | For type hierarchy |
| type_hierarchy_path | text[] | | | Ancestor path |
| semantic_context | jsonb | | '{}' | AI context |
| embedding | vector | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Entity Extension Tables

### entity_proper_persons (Natural Persons)

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| proper_person_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| first_name | varchar(255) | NOT NULL | | |
| last_name | varchar(255) | NOT NULL | | |
| middle_names | varchar(255) | | | |
| date_of_birth | date | | | |
| nationality | varchar(100) | | | |
| residence_address | text | | | |
| id_document_type | varchar(100) | | | |
| id_document_number | varchar(100) | | | |
| search_name | text | | | Computed search field |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_limited_companies (Companies)

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| limited_company_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| company_name | varchar(255) | NOT NULL | | |
| registration_number | varchar(100) | | | |
| jurisdiction | varchar(100) | | | |
| incorporation_date | date | | | |
| registered_address | text | | | |
| business_nature | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_partnerships

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| partnership_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| partnership_name | varchar(255) | NOT NULL | | |
| partnership_type | varchar(100) | | | LP, LLP, GP, etc. |
| jurisdiction | varchar(100) | | | |
| formation_date | date | | | |
| principal_place_business | text | | | |
| partnership_agreement_date | date | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_trusts

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| trust_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| trust_name | varchar(255) | NOT NULL | | |
| trust_type | varchar(100) | | | Discretionary, Fixed, etc. |
| jurisdiction | varchar(100) | NOT NULL | | |
| establishment_date | date | | | |
| trust_deed_date | date | | | |
| trust_purpose | text | | | |
| governing_law | varchar(100) | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Role Management

### roles

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| role_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | DIRECTOR, UBO, SHAREHOLDER, etc. |
| description | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### cbu_entity_roles (CBU-Entity-Role Junction)

Links entities to CBUs with specific roles.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| cbu_entity_role_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| entity_id | uuid | NOT NULL | | FK to entities |
| role_id | uuid | NOT NULL | | FK to roles |
| created_at | timestamptz | | now() | |

## Document Management

### document_types

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| type_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| type_code | varchar(100) | NOT NULL | | PASSPORT, CERT_OF_INCORP, etc. |
| display_name | varchar(200) | NOT NULL | | |
| category | varchar(100) | NOT NULL | | IDENTITY, CORPORATE, FINANCIAL |
| domain | varchar(100) | | | |
| description | text | | | |
| required_attributes | jsonb | | '{}' | |
| applicability | jsonb | | '{}' | Entity type applicability |
| semantic_context | jsonb | | '{}' | |
| embedding | vector | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### document_catalog

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| doc_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| document_id | uuid | | gen_random_uuid() | Business ID |
| cbu_id | uuid | | | FK to cbus |
| document_type_id | uuid | | | FK to document_types |
| document_type_code | varchar(100) | | | Denormalized type code |
| document_name | varchar(255) | | | |
| file_hash_sha256 | text | | | |
| storage_key | text | | | S3/storage reference |
| file_size_bytes | bigint | | | |
| mime_type | varchar(100) | | | |
| source_system | varchar(100) | | | |
| status | varchar(50) | | 'active' | |
| extraction_status | varchar(50) | | 'PENDING' | |
| extracted_data | jsonb | | | AI-extracted data |
| extraction_confidence | numeric | | | |
| last_extracted_at | timestamptz | | | |
| metadata | jsonb | | '{}' | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Screening & KYC

### screenings

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| screening_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| investigation_id | uuid | | | FK to kyc_investigations |
| entity_id | uuid | NOT NULL | | FK to entities |
| screening_type | varchar(50) | NOT NULL | | PEP, SANCTIONS, ADVERSE_MEDIA |
| databases | jsonb | | | Databases searched |
| lists | jsonb | | | Specific lists |
| include_rca | boolean | | false | Include relatives/close associates |
| search_depth | varchar(20) | | | |
| languages | jsonb | | | |
| status | varchar(50) | | 'PENDING' | |
| result | varchar(50) | | | CLEAR, HIT, INCONCLUSIVE |
| match_details | jsonb | | | |
| resolution | varchar(50) | | | TRUE_MATCH, FALSE_POSITIVE |
| resolution_rationale | text | | | |
| screened_at | timestamptz | | now() | |
| reviewed_by | varchar(255) | | | |
| resolved_by | varchar(255) | | | |
| resolved_at | timestamptz | | | |

### kyc_investigations

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| investigation_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | | | FK to cbus |
| investigation_type | varchar(50) | NOT NULL | | INITIAL, PERIODIC, TRIGGER |
| risk_rating | varchar(20) | | | LOW, MEDIUM, HIGH |
| regulatory_framework | jsonb | | | |
| ubo_threshold | numeric | | 10.0 | |
| investigation_depth | integer | | 5 | |
| status | varchar(50) | | 'INITIATED' | |
| deadline | date | | | |
| outcome | varchar(50) | | | |
| notes | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |
| completed_at | timestamptz | | | |

### kyc_decisions

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| decision_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| investigation_id | uuid | | | FK to kyc_investigations |
| decision | varchar(50) | NOT NULL | | APPROVE, REJECT, CONDITIONAL |
| decision_authority | varchar(100) | | | |
| rationale | text | | | |
| decided_by | varchar(255) | | | |
| decided_at | timestamptz | | now() | |
| effective_date | date | | CURRENT_DATE | |
| review_date | date | | | |

### entity_kyc_status

Per-entity KYC status within a CBU context.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| status_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | NOT NULL | | FK to entities |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| kyc_status | varchar(50) | NOT NULL | | NOT_STARTED, IN_PROGRESS, PENDING_REVIEW, APPROVED, REJECTED, EXPIRED |
| risk_rating | varchar(20) | | | LOW, MEDIUM, HIGH, PROHIBITED |
| reviewer | varchar(255) | | | Reviewer email/ID |
| notes | text | | | Status notes |
| next_review_date | date | | | Scheduled review date |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

**Unique constraint**: (entity_id, cbu_id)

### control_relationships

Non-ownership control links between entities.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| control_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| controller_entity_id | uuid | NOT NULL | | FK to entities (who controls) |
| controlled_entity_id | uuid | NOT NULL | | FK to entities (who is controlled) |
| control_type | varchar(50) | NOT NULL | | BOARD_CONTROL, VOTING_RIGHTS, VETO_POWER, MANAGEMENT, TRUSTEE, PROTECTOR, OTHER |
| description | text | | | Description of control mechanism |
| effective_from | date | | | Start date |
| effective_to | date | | | End date |
| is_active | boolean | | true | Active record |
| evidence_doc_id | uuid | | | FK to document_catalog |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Products & Services

### products

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| product_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | |
| product_code | varchar(50) | | | |
| product_category | varchar(100) | | | |
| regulatory_framework | varchar(100) | | | |
| description | text | | | |
| min_asset_requirement | numeric | | | |
| is_active | boolean | | true | |
| metadata | jsonb | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### services

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| service_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | |
| service_code | varchar(50) | | | |
| service_category | varchar(100) | | | |
| description | text | | | |
| sla_definition | jsonb | | | |
| is_active | boolean | | true | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Resource Instance Taxonomy

### cbu_resource_instances

Delivered resource instances (accounts, connections, etc.).

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| instance_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| product_id | uuid | | | FK to products |
| service_id | uuid | | | FK to services |
| resource_type_id | uuid | | | FK to service_resources |
| instance_url | varchar(1024) | NOT NULL | | Resource locator |
| instance_identifier | varchar(255) | | | External ID |
| instance_name | varchar(255) | | | Display name |
| instance_config | jsonb | | '{}' | Configuration |
| status | varchar(50) | NOT NULL | 'PENDING' | PENDING, ACTIVE, SUSPENDED, DECOMMISSIONED |
| requested_at | timestamptz | | now() | |
| provisioned_at | timestamptz | | | |
| activated_at | timestamptz | | | |
| decommissioned_at | timestamptz | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### resource_instance_attributes

Typed attribute values for resource instances.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| value_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| instance_id | uuid | NOT NULL | | FK to cbu_resource_instances |
| attribute_id | uuid | NOT NULL | | FK to attribute_registry |
| value_text | varchar | | | Text value |
| value_number | numeric | | | Numeric value |
| value_boolean | boolean | | | Boolean value |
| value_date | date | | | Date value |
| value_timestamp | timestamptz | | | Timestamp value |
| value_json | jsonb | | | JSON value |
| state | varchar(50) | | 'proposed' | proposed, confirmed, superseded |
| source | jsonb | | | Source metadata |
| observed_at | timestamptz | | now() | |

### service_delivery_map

Tracks service delivery to CBUs.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| delivery_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| product_id | uuid | NOT NULL | | FK to products |
| service_id | uuid | NOT NULL | | FK to services |
| instance_id | uuid | | | FK to cbu_resource_instances |
| service_config | jsonb | | '{}' | |
| delivery_status | varchar(50) | | 'PENDING' | PENDING, IN_PROGRESS, DELIVERED, FAILED |
| requested_at | timestamptz | | now() | |
| started_at | timestamptz | | | |
| delivered_at | timestamptz | | | |
| failed_at | timestamptz | | | |
| failure_reason | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Reference Data

### master_jurisdictions

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| jurisdiction_code | varchar(10) | NOT NULL | | Primary key (e.g., 'LU', 'IE') |
| jurisdiction_name | varchar(200) | NOT NULL | | |
| country_code | varchar(3) | NOT NULL | | ISO country code |
| region | varchar(100) | | | |
| regulatory_framework | varchar(100) | | | |
| entity_formation_allowed | boolean | | true | |
| offshore_jurisdiction | boolean | | false | |
| regulatory_authority | varchar(300) | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Custody Schema (`custody`)

The custody schema implements a three-layer model for settlement instruction routing.

### Layer 1: Universe Tables

#### cbu_instrument_universe

Defines what instruments a CBU trades.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| universe_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| instrument_class_id | uuid | NOT NULL | | FK to instrument_classes |
| market_id | uuid | | | FK to markets |
| currencies | varchar(3)[] | NOT NULL | '{}' | Supported currencies |
| settlement_types | varchar(10)[] | | '{DVP}' | DVP, FOP, RVP |
| counterparty_entity_id | uuid | | | For OTC counterparty-specific |
| is_held | boolean | | true | Holds positions |
| is_traded | boolean | | true | Actively trades |
| is_active | boolean | | true | Active record |
| effective_date | date | NOT NULL | CURRENT_DATE | |

### Layer 2: SSI Tables

#### cbu_ssi (Standing Settlement Instructions)

Account information for settlement.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| ssi_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| ssi_name | varchar(100) | NOT NULL | | Display name |
| ssi_type | varchar(20) | NOT NULL | | SECURITIES, CASH, COLLATERAL |
| safekeeping_account | varchar(35) | | | Securities account |
| safekeeping_bic | varchar(11) | | | Custodian BIC |
| safekeeping_account_name | varchar(100) | | | Account name |
| cash_account | varchar(35) | | | Cash account |
| cash_account_bic | varchar(11) | | | Cash agent BIC |
| cash_currency | varchar(3) | | | Settlement currency |
| pset_bic | varchar(11) | | | Place of settlement BIC |
| status | varchar(20) | | 'PENDING' | PENDING, ACTIVE, SUSPENDED |
| effective_date | date | NOT NULL | | Start date |
| expiry_date | date | | | End date |
| source | varchar(20) | | 'MANUAL' | MANUAL, SWIFT, DTCC |

### Layer 3: Booking Rules

#### ssi_booking_rules

ALERT-style routing rules matching trade characteristics to SSIs.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| rule_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| ssi_id | uuid | NOT NULL | | FK to cbu_ssi |
| rule_name | varchar(100) | NOT NULL | | Display name |
| priority | integer | NOT NULL | 50 | Lower = higher priority |
| instrument_class_id | uuid | | | NULL = any |
| security_type_id | uuid | | | NULL = any |
| market_id | uuid | | | NULL = any |
| currency | varchar(3) | | | NULL = any |
| settlement_type | varchar(10) | | | NULL = any |
| counterparty_entity_id | uuid | | | For OTC |
| specificity_score | integer | | | Generated: counts non-NULL criteria |
| is_active | boolean | | true | |
| effective_date | date | NOT NULL | CURRENT_DATE | |

### Reference Tables

#### instrument_classes

CFI-based instrument classification.

| Column | Type | Description |
|--------|------|-------------|
| class_id | uuid | Primary key |
| class_code | varchar(20) | EQUITY, GOVT_BOND, CORP_BOND, ETF |
| cfi_prefix | varchar(6) | CFI code prefix |
| description | text | |
| smpg_category | varchar(50) | SMPG/ALERT category |

#### markets

ISO 10383 MIC codes.

| Column | Type | Description |
|--------|------|-------------|
| market_id | uuid | Primary key |
| mic | varchar(4) | XNYS, XLON, XNAS |
| market_name | varchar(100) | |
| country_code | varchar(2) | |
| currency | varchar(3) | Primary currency |
| csd_bic | varchar(11) | CSD BIC |

#### security_types

SMPG/ALERT security type taxonomy.

| Column | Type | Description |
|--------|------|-------------|
| security_type_id | uuid | Primary key |
| type_code | varchar(30) | |
| instrument_class_id | uuid | FK to instrument_classes |
| description | text | |
| smpg_code | varchar(10) | |

#### currencies

ISO 4217 currency codes.

| Column | Type | Description |
|--------|------|-------------|
| currency_code | varchar(3) | Primary key (USD, EUR, GBP) |
| currency_name | varchar(50) | |
| decimals | integer | Decimal places |
| is_active | boolean | |

### Supporting Tables

| Table | Purpose |
|-------|---------|
| cbu_ssi_agent_override | Override receiving/delivering agents |
| entity_settlement_identity | BIC/LEI for entity settlement |
| entity_ssi | Entity-level SSIs (vs CBU-level) |
| subcustodian_network | Subcustodian relationships |
| instruction_types | Settlement instruction types |
| instruction_paths | Settlement message routing |
| isda_agreements | ISDA master agreements |
| isda_product_coverage | Products under ISDA |
| isda_product_taxonomy | OTC product classification |
| csa_agreements | Credit support annexes |
| cfi_codes | Full CFI code reference |

## KYC Schema (`kyc`)

The kyc schema implements both KYC case management and a Clearstream-style investor registry.

### KYC Case Management

#### cases

Central table for KYC investigation cases.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| case_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| status | varchar(30) | NOT NULL | 'INTAKE' | INTAKE, DISCOVERY, ASSESSMENT, REVIEW, APPROVED, REJECTED, BLOCKED, WITHDRAWN, EXPIRED |
| escalation_level | varchar(30) | NOT NULL | 'STANDARD' | STANDARD, SENIOR_COMPLIANCE, EXECUTIVE, BOARD |
| risk_rating | varchar(20) | | | LOW, MEDIUM, HIGH, VERY_HIGH, PROHIBITED |
| assigned_analyst_id | uuid | | | Assigned analyst |
| assigned_reviewer_id | uuid | | | Assigned reviewer |
| opened_at | timestamptz | NOT NULL | now() | Case opened timestamp |
| closed_at | timestamptz | | | Case closed timestamp |
| sla_deadline | timestamptz | | | SLA deadline |
| last_activity_at | timestamptz | | now() | Last activity timestamp |
| case_type | varchar(30) | | 'NEW_CLIENT' | NEW_CLIENT, PERIODIC_REVIEW, EVENT_DRIVEN, REMEDIATION |
| notes | text | | | Case notes |

**Indexes**: case_id (PK), cbu_id, status, assigned_analyst_id

#### entity_workstreams

Per-entity work items within a case.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| workstream_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| entity_id | uuid | NOT NULL | | FK to entities |
| status | varchar(30) | NOT NULL | 'PENDING' | PENDING, COLLECT, VERIFY, SCREEN, ASSESS, COMPLETE, BLOCKED, ENHANCED_DD |
| discovery_source_workstream_id | uuid | | | FK to self - parent workstream that discovered this entity |
| discovery_reason | varchar(100) | | | Why entity was discovered |
| risk_rating | varchar(20) | | | Entity risk rating |
| risk_factors | jsonb | | '[]' | Array of risk factors |
| created_at | timestamptz | NOT NULL | now() | |
| started_at | timestamptz | | | Work started |
| completed_at | timestamptz | | | Work completed |
| blocked_at | timestamptz | | | When blocked |
| blocked_reason | text | | | Why blocked |
| requires_enhanced_dd | boolean | | false | Enhanced due diligence required |
| is_ubo | boolean | | false | Is this entity a UBO |
| ownership_percentage | numeric(5,2) | | | Ownership percentage if applicable |
| discovery_depth | integer | | 1 | Depth in ownership chain |

**Unique constraint**: (case_id, entity_id)
**Indexes**: case_id, entity_id, status, discovery_source_workstream_id

#### red_flags

Risk indicators raised during KYC.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| red_flag_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| flag_type | varchar(50) | NOT NULL | | Type of red flag |
| severity | varchar(20) | NOT NULL | | SOFT, ESCALATE, HARD_STOP |
| status | varchar(20) | NOT NULL | 'OPEN' | OPEN, UNDER_REVIEW, MITIGATED, WAIVED, BLOCKING, CLOSED |
| description | text | NOT NULL | | Description of the flag |
| source | varchar(50) | | | Source system/rule |
| source_reference | text | | | Reference ID in source |
| raised_at | timestamptz | NOT NULL | now() | When raised |
| raised_by | uuid | | | Who raised it |
| reviewed_at | timestamptz | | | When reviewed |
| reviewed_by | uuid | | | Who reviewed |
| resolved_at | timestamptz | | | When resolved |
| resolved_by | uuid | | | Who resolved |
| resolution_type | varchar(30) | | | How resolved |
| resolution_notes | text | | | Resolution details |
| waiver_approved_by | uuid | | | Who approved waiver |
| waiver_justification | text | | | Waiver justification |

**Indexes**: case_id, workstream_id, flag_type, severity, status

#### doc_requests

Document collection requests per workstream.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| request_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| workstream_id | uuid | NOT NULL | | FK to entity_workstreams |
| doc_type | varchar(50) | NOT NULL | | Document type code |
| status | varchar(20) | NOT NULL | 'REQUIRED' | REQUIRED, REQUESTED, RECEIVED, UNDER_REVIEW, VERIFIED, REJECTED, WAIVED, EXPIRED |
| required_at | timestamptz | NOT NULL | now() | When requirement created |
| requested_at | timestamptz | | | When requested from client |
| due_date | date | | | Due date for document |
| received_at | timestamptz | | | When received |
| reviewed_at | timestamptz | | | When reviewed |
| verified_at | timestamptz | | | When verified |
| document_id | uuid | | | FK to document_catalog |
| reviewer_id | uuid | | | Who reviewed |
| rejection_reason | text | | | Why rejected |
| verification_notes | text | | | Verification notes |
| is_mandatory | boolean | | true | Is document mandatory |
| priority | varchar(10) | | 'NORMAL' | Document priority |

**Indexes**: workstream_id, doc_type, status, due_date

#### screenings

Screening requests and results per workstream.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| screening_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| workstream_id | uuid | NOT NULL | | FK to entity_workstreams |
| screening_type | varchar(30) | NOT NULL | | SANCTIONS, PEP, ADVERSE_MEDIA, CREDIT, CRIMINAL, REGULATORY, CONSOLIDATED |
| provider | varchar(50) | | | Screening provider |
| status | varchar(20) | NOT NULL | 'PENDING' | PENDING, RUNNING, CLEAR, HIT_PENDING_REVIEW, HIT_CONFIRMED, HIT_DISMISSED, ERROR, EXPIRED |
| requested_at | timestamptz | NOT NULL | now() | When requested |
| completed_at | timestamptz | | | When completed |
| expires_at | timestamptz | | | When expires |
| result_summary | varchar(100) | | | Brief result |
| result_data | jsonb | | | Full result data |
| match_count | integer | | 0 | Number of matches |
| reviewed_by | uuid | | | Who reviewed |
| reviewed_at | timestamptz | | | When reviewed |
| review_notes | text | | | Review notes |
| red_flag_id | uuid | | | FK to red_flags if hit raised flag |

**Indexes**: workstream_id, screening_type, status

#### case_events

Audit trail for case activities.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| event_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| event_type | varchar(50) | NOT NULL | | Event type |
| event_data | jsonb | | '{}' | Event payload |
| actor_id | uuid | | | Who performed action |
| actor_type | varchar(20) | | 'USER' | USER, SYSTEM, RULE |
| occurred_at | timestamptz | NOT NULL | now() | When occurred |
| comment | text | | | Optional comment |

**Indexes**: case_id, workstream_id, event_type, occurred_at DESC

#### rule_executions

Audit log for rules engine executions.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| execution_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| rule_name | varchar(100) | NOT NULL | | Rule that was evaluated |
| trigger_event | varchar(50) | NOT NULL | | Event that triggered rule |
| condition_matched | boolean | NOT NULL | | Whether conditions matched |
| actions_executed | jsonb | | '[]' | Actions that were executed |
| context_snapshot | jsonb | | '{}' | Context at time of execution |
| executed_at | timestamptz | NOT NULL | now() | When executed |

#### approval_requests

Escalation and approval workflow.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| approval_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| request_type | varchar(50) | NOT NULL | | Type of approval needed |
| requested_by | varchar(255) | | | Who requested |
| requested_at | timestamptz | NOT NULL | now() | When requested |
| approver | varchar(255) | | | Who approved/rejected |
| decision | varchar(20) | | | APPROVED, REJECTED, PENDING |
| decision_at | timestamptz | | | When decided |
| comments | text | | | Decision comments |

### KYC Case Views

#### v_case_summary

Aggregated case view with counts.

```sql
SELECT c.*, 
       COUNT(DISTINCT w.workstream_id) as workstream_count,
       COUNT(DISTINCT r.red_flag_id) FILTER (WHERE r.status = 'OPEN') as open_flags,
       MIN(c.sla_deadline) as next_deadline
FROM kyc.cases c
LEFT JOIN kyc.entity_workstreams w ON c.case_id = w.case_id
LEFT JOIN kyc.red_flags r ON c.case_id = r.case_id
GROUP BY c.case_id
```

#### v_workstream_detail

Workstream view with entity details.

```sql
SELECT w.*, e.name as entity_name, et.name as entity_type
FROM kyc.entity_workstreams w
JOIN entities e ON w.entity_id = e.entity_id
JOIN entity_types et ON e.entity_type_id = et.entity_type_id
```

### Investor Registry

### share_classes

Fund share class master data.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| id | uuid | NOT NULL | uuid_generate_v4() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus (the fund) |
| entity_id | uuid | YES | | FK to entities - legal entity that issues this share class |
| name | varchar(255) | NOT NULL | | Share class name (e.g., "Class A EUR") |
| isin | varchar(12) | | | ISIN code |
| currency | char(3) | NOT NULL | 'EUR' | Share class currency |
| class_category | varchar(20) | NO | 'FUND' | CORPORATE = company ownership, FUND = investment fund |
| fund_type | varchar(50) | | | HEDGE_FUND, UCITS, AIFMD, PRIVATE_EQUITY, REIT |
| fund_structure | varchar(50) | | | OPEN_ENDED, CLOSED_ENDED |
| investor_eligibility | varchar(50) | | | RETAIL, PROFESSIONAL, QUALIFIED |
| nav_per_share | numeric(20,6) | | | Current NAV |
| nav_date | date | | | NAV valuation date |
| management_fee_bps | integer | | | Management fee in basis points |
| performance_fee_bps | integer | | | Performance fee in basis points |
| high_water_mark | boolean | | false | Performance fee uses high water mark |
| hurdle_rate | numeric(5,2) | | | Hurdle rate for performance fee |
| subscription_frequency | varchar(50) | | | Daily, Weekly, Monthly |
| redemption_frequency | varchar(50) | | | Daily, Weekly, Monthly |
| redemption_notice_days | integer | | | Notice period for redemptions |
| lock_up_period_months | integer | | | Lock-up period for hedge funds |
| gate_percentage | numeric(5,2) | | | Redemption gate percentage |
| minimum_investment | numeric(20,2) | | | Minimum investment amount |
| status | varchar(50) | NOT NULL | 'active' | active, closed |
| created_at | timestamptz | NOT NULL | now() | |
| updated_at | timestamptz | NOT NULL | now() | |

**Unique constraint**: (cbu_id, isin)

### holdings

Investor positions in share classes.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| id | uuid | NOT NULL | uuid_generate_v4() | Primary key |
| share_class_id | uuid | NOT NULL | | FK to share_classes |
| investor_entity_id | uuid | NOT NULL | | FK to entities (the investor) |
| units | numeric(20,6) | NOT NULL | 0 | Number of units held |
| cost_basis | numeric(20,2) | | | Total cost basis |
| acquisition_date | date | | | Initial acquisition date |
| status | varchar(50) | NOT NULL | 'active' | active, closed |
| created_at | timestamptz | NOT NULL | now() | |
| updated_at | timestamptz | NOT NULL | now() | |

**Unique constraint**: (share_class_id, investor_entity_id)

### movements

Subscription, redemption, and transfer transactions.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| id | uuid | NOT NULL | uuid_generate_v4() | Primary key |
| holding_id | uuid | NOT NULL | | FK to holdings |
| movement_type | varchar(50) | NOT NULL | | subscription, redemption, transfer_in, transfer_out, dividend, adjustment |
| units | numeric(20,6) | NOT NULL | | Number of units |
| price_per_unit | numeric(20,6) | | | Price at transaction |
| amount | numeric(20,2) | | | Total amount |
| currency | char(3) | NOT NULL | 'EUR' | Transaction currency |
| trade_date | date | NOT NULL | | Trade date |
| settlement_date | date | | | Settlement date |
| status | varchar(50) | NOT NULL | 'pending' | pending, confirmed, settled, cancelled, failed |
| reference | varchar(100) | | | External reference |
| notes | text | | | Transaction notes |
| created_at | timestamptz | NOT NULL | now() | |
| updated_at | timestamptz | NOT NULL | now() | |

**Check constraints**:
- movement_type IN ('subscription', 'redemption', 'transfer_in', 'transfer_out', 'dividend', 'adjustment')
- status IN ('pending', 'confirmed', 'settled', 'cancelled', 'failed')


## Table Count by Category

| Category | Tables | Examples |
|----------|--------|----------|
| Core | 5 | cbus, entities, entity_types, roles, cbu_entity_roles |
| Entity Extensions | 4 | entity_proper_persons, entity_limited_companies, entity_partnerships, entity_trusts |
| Documents | 3 | document_catalog, document_types, document_attribute_mappings |
| Products/Services | 8 | products, services, service_delivery_map, cbu_resource_instances |
| Reference Data | 4 | master_jurisdictions, currencies, roles, dictionary |
| DSL/Execution | 6 | dsl_instances, dsl_instance_versions, dsl_execution_log, dsl_domains, dsl_examples |
| Onboarding | 4 | onboarding_requests, onboarding_products, service_option_definitions, service_option_choices |
| Attributes | 4 | attribute_registry, attribute_values_typed, attribute_dictionary, resource_attribute_requirements |
| Other | 13 | Various support tables |
| **ob-poc Total** | **51** | |
| **Custody** | **17** | cbu_instrument_universe, cbu_ssi, ssi_booking_rules, isda_agreements, csa_agreements |
| **KYC** | **11** | cases, entity_workstreams, red_flags, doc_requests, screenings, share_classes, holdings, movements |
| **Grand Total** | **79** | |

## Rebuilding the Schema

```bash
# Full schema rebuild
psql -d data_designer -f schema_export.sql

# Seed data only
psql -d data_designer -f sql/seeds/all_seeds.sql
```
