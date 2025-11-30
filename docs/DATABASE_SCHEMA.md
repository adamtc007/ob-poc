# OB-POC Database Schema

## Overview

The `ob-poc` schema contains 112 tables supporting KYC/AML onboarding workflows.

## Core Entity Tables

### entities
Base table for all entity types (Class Table Inheritance pattern).

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| entity_id | uuid | NO | Primary key |
| entity_type_id | uuid | YES | FK to entity_types |
| external_id | varchar | YES | External system reference |
| status | varchar | YES | Entity status |
| created_at | timestamp | YES | Creation timestamp |
| updated_at | timestamp | YES | Last update timestamp |

### entity_types
Defines available entity types.

| Column | Type | Description |
|--------|------|-------------|
| entity_type_id | uuid | Primary key |
| type_code | varchar | Unique code (e.g., PROPER_PERSON_NATURAL) |
| name | varchar | Display name |
| category | varchar | Category (PERSON, COMPANY, etc.) |
| parent_type_id | uuid | FK for type hierarchy |

### entity_proper_persons
Extension table for natural persons.

| Column | Type | Description |
|--------|------|-------------|
| entity_id | uuid | FK to entities |
| first_name | varchar | First name |
| last_name | varchar | Last name |
| middle_names | varchar | Middle names |
| date_of_birth | date | Date of birth |
| nationality | varchar | Nationality code |
| tax_id | varchar | Tax ID |
| residence_address | text | Address |

### entity_limited_companies
Extension table for limited companies.

| Column | Type | Description |
|--------|------|-------------|
| entity_id | uuid | FK to entities |
| company_name | varchar | Company name |
| registration_number | varchar | Registration number |
| jurisdiction | varchar | Jurisdiction code |
| incorporation_date | date | Date of incorporation |
| registered_address | text | Registered address |
| business_nature | text | Nature of business |

### entity_partnerships
Extension table for partnerships.

| Column | Type | Description |
|--------|------|-------------|
| entity_id | uuid | FK to entities |
| partnership_name | varchar | Partnership name |
| partnership_type | varchar | LP, LLP, GP |
| jurisdiction | varchar | Jurisdiction code |
| formation_date | date | Formation date |

### entity_trusts
Extension table for trusts.

| Column | Type | Description |
|--------|------|-------------|
| entity_id | uuid | FK to entities |
| trust_name | varchar | Trust name |
| trust_type | varchar | Discretionary, Fixed Interest, etc. |
| jurisdiction | varchar | Jurisdiction code |
| establishment_date | date | Establishment date |
| governing_law | varchar | Governing law |

## CBU (Client Business Unit) Tables

### cbus
Client Business Units - the main onboarding unit.

| Column | Type | Description |
|--------|------|-------------|
| cbu_id | uuid | Primary key |
| name | varchar | CBU name |
| jurisdiction | varchar | Jurisdiction code |
| client_type | varchar | COMPANY, INDIVIDUAL, TRUST, etc. |
| status | varchar | Onboarding status |
| nature_purpose | text | Nature and purpose |
| risk_rating | varchar | Risk rating |

### cbu_entity_roles
Links entities to CBUs with roles.

| Column | Type | Description |
|--------|------|-------------|
| cbu_entity_role_id | uuid | Primary key |
| cbu_id | uuid | FK to cbus |
| entity_id | uuid | FK to entities |
| role_id | uuid | FK to roles |
| effective_date | date | Role effective date |

### roles
Role definitions.

| Column | Type | Description |
|--------|------|-------------|
| role_id | uuid | Primary key |
| name | varchar | Role name (DIRECTOR, SHAREHOLDER, etc.) |
| description | text | Role description |

## Document Tables

### document_catalog
Document storage and tracking.

| Column | Type | Description |
|--------|------|-------------|
| doc_id | uuid | Primary key |
| document_type_id | uuid | FK to document_types |
| cbu_id | uuid | FK to cbus |
| document_name | varchar | Document name |
| status | varchar | Document status |
| extraction_status | varchar | AI extraction status |
| verification_status | varchar | Verification status |
| file_path | varchar | Storage path |

### document_types
Document type definitions.

| Column | Type | Description |
|--------|------|-------------|
| type_id | uuid | Primary key |
| type_code | varchar | Unique code (PASSPORT, CERT_OF_INCORP, etc.) |
| display_name | varchar | Display name |
| category | varchar | Category |
| valid_for_entity_types | text[] | Applicable entity types |

## Screening Tables

### screenings
Screening records for PEP, sanctions, adverse media.

| Column | Type | Description |
|--------|------|-------------|
| screening_id | uuid | Primary key |
| entity_id | uuid | FK to entities |
| screening_type | varchar | PEP, SANCTIONS, ADVERSE_MEDIA |
| status | varchar | PENDING, COMPLETED, FAILED |
| result | varchar | CLEAR, MATCH, POSSIBLE_MATCH |
| match_count | integer | Number of matches |

## Investigation Tables

### investigations
KYC investigation records.

| Column | Type | Description |
|--------|------|-------------|
| investigation_id | uuid | Primary key |
| cbu_id | uuid | FK to cbus |
| investigation_type | varchar | Investigation type |
| status | varchar | Status |
| outcome | varchar | Outcome |
| risk_rating | varchar | Assessed risk |
| assigned_to | varchar | Assignee |

### decisions
Onboarding decisions.

| Column | Type | Description |
|--------|------|-------------|
| decision_id | uuid | Primary key |
| cbu_id | uuid | FK to cbus |
| investigation_id | uuid | FK to investigations |
| decision | varchar | APPROVE, REJECT, CONDITIONAL |
| decision_authority | varchar | Authority level |
| rationale | text | Decision rationale |

## DSL Tables

### dsl_generation_log
Logs DSL generation attempts for training/audit.

| Column | Type | Description |
|--------|------|-------------|
| log_id | uuid | Primary key |
| user_intent | text | Original user instruction |
| source | varchar | session, mcp, api |
| session_id | uuid | Session reference |
| final_dsl | text | Final generated DSL |
| executed | boolean | Was it executed |
| execution_success | boolean | Execution result |

### dsl_instances
Persisted DSL for CBUs.

| Column | Type | Description |
|--------|------|-------------|
| instance_id | uuid | Primary key |
| business_reference | varchar | Business reference |
| cbu_id | uuid | FK to cbus |
| dsl_source | text | DSL source code |
| version | integer | Version number |
| status | varchar | Status |

## Ownership Tables

### ownership_relationships
Tracks ownership chains for UBO calculation.

| Column | Type | Description |
|--------|------|-------------|
| relationship_id | uuid | Primary key |
| owner_entity_id | uuid | FK to entities (owner) |
| owned_entity_id | uuid | FK to entities (owned) |
| ownership_type | varchar | DIRECT, INDIRECT, BENEFICIAL |
| ownership_percent | numeric | Ownership percentage |
| effective_from | date | Effective date |
| effective_to | date | End date (null = current) |

## Attribute Tables

### attribute_dictionary
Master attribute definitions.

| Column | Type | Description |
|--------|------|-------------|
| attr_uuid | uuid | Primary key |
| attr_id | varchar | Attribute ID (attr.identity.first_name) |
| display_name | varchar | Display name |
| data_type | varchar | STRING, DATE, UUID, etc. |
| validation_pattern | varchar | Regex pattern |

### attribute_values_typed
Strongly-typed attribute values.

| Column | Type | Description |
|--------|------|-------------|
| value_id | uuid | Primary key |
| attr_uuid | uuid | FK to attribute_dictionary |
| entity_id | uuid | FK to entities |
| document_id | uuid | FK to document_catalog |
| string_value | varchar | String value |
| date_value | date | Date value |
| uuid_value | uuid | UUID value |
| decimal_value | numeric | Decimal value |
| boolean_value | boolean | Boolean value |

## Key Relationships

```
cbus ←── cbu_entity_roles ──→ entities ──→ entity_types
              │                    │
              ▼                    ▼
            roles          entity_proper_persons
                           entity_limited_companies
                           entity_partnerships
                           entity_trusts

document_catalog ──→ document_types
       │
       ▼
   cbus (cbu_id)

screenings ──→ entities
investigations ──→ cbus
decisions ──→ investigations

ownership_relationships ──→ entities (owner)
                       ──→ entities (owned)
```

## Table Count by Category

| Category | Tables |
|----------|--------|
| Entity | 10 |
| CBU | 3 |
| Document | 8 |
| Screening | 6 |
| Investigation | 4 |
| Decision | 3 |
| DSL | 10 |
| Attribute | 5 |
| Monitoring | 8 |
| Orchestration | 5 |
| Other | 50 |
| **Total** | **112** |
