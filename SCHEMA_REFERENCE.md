# OB-POC Database Schema Reference

**Database**: data_designer (PostgreSQL 17)  
**Generated**: 2025-12-09  
**Total Tables**: 112 (ob-poc: 83, custody: 17, kyc: 12)

## Overview

This document provides a complete reference of the database schema for the OB-POC KYC/AML onboarding system.

### Schema Organization

| Schema | Purpose |
|--------|---------|
| ob-poc | Core business entities, products, services, documents, attributes |
| custody | Settlement instructions, booking rules, instrument reference data |
| kyc | KYC case management, workstreams, investor registry |

---


## Core Schema - Business Entities & Services

### ob-poc.attribute_dictionary

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **attribute_id** (PK) | uuid | NO | gen_random_uuid() |  |
| attr_id | character varying(100) | NO |  |  |
| attr_name | character varying(255) | NO |  |  |
| domain | character varying(50) | NO |  |  |
| data_type | character varying(50) | NO | 'STRING'::character varying |  |
| description | text | YES |  |  |
| validation_pattern | character varying(255) | YES |  |  |
| is_required | boolean | YES | false |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.attribute_observations

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **observation_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| attribute_id | uuid | NO |  | ob-poc.attribute_registry.uuid |
| value_text | text | YES |  |  |
| value_number | numeric | YES |  |  |
| value_boolean | boolean | YES |  |  |
| value_date | date | YES |  |  |
| value_datetime | timestamp with time zone | YES |  |  |
| value_json | jsonb | YES |  |  |
| source_type | character varying(30) | NO |  |  |
| source_document_id | uuid | YES |  | ob-poc.document_catalog.doc_id |
| source_workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| source_screening_id | uuid | YES |  | kyc.screenings.screening_id |
| source_reference | text | YES |  |  |
| source_metadata | jsonb | YES | '{}'::jsonb |  |
| confidence | numeric | YES | 0.50 |  |
| is_authoritative | boolean | YES | false |  |
| extraction_method | character varying(50) | YES |  |  |
| observed_at | timestamp with time zone | NO | now() |  |
| observed_by | text | YES |  |  |
| status | character varying(30) | YES | 'ACTIVE'::character varying |  |
| superseded_by | uuid | YES |  | ob-poc.attribute_observations.observation_id |
| superseded_at | timestamp with time zone | YES |  |  |
| effective_from | date | YES |  |  |
| effective_to | date | YES |  |  |
| created_at | timestamp with time zone | NO | now() |  |
| updated_at | timestamp with time zone | NO | now() |  |

### ob-poc.attribute_registry

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **id** (PK) | text | NO |  |  |
| display_name | text | NO |  |  |
| category | text | NO |  |  |
| value_type | text | NO |  |  |
| validation_rules | jsonb | YES | '{}'::jsonb |  |
| metadata | jsonb | YES | '{}'::jsonb |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| uuid | uuid | NO |  |  |
| applicability | jsonb | YES | '{}'::jsonb |  |
| embedding | USER-DEFINED | YES |  |  |
| embedding_model | character varying(100) | YES |  |  |
| embedding_updated_at | timestamp with time zone | YES |  |  |
| domain | character varying(100) | YES |  |  |
| is_required | boolean | YES | false |  |
| default_value | text | YES |  |  |
| group_id | character varying(100) | YES |  |  |
| reconciliation_rules | jsonb | YES | '{}'::jsonb |  |
| acceptable_variation_threshold | numeric | YES |  |  |
| requires_authoritative_source | boolean | YES | false |  |

### ob-poc.attribute_values_typed

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **id** (PK) | integer | NO | nextval('"ob-poc".attribute_values_typed |  |
| entity_id | uuid | NO |  |  |
| attribute_id | text | NO |  | ob-poc.attribute_registry.id |
| value_text | text | YES |  |  |
| value_number | numeric | YES |  |  |
| value_integer | bigint | YES |  |  |
| value_boolean | boolean | YES |  |  |
| value_date | date | YES |  |  |
| value_datetime | timestamp with time zone | YES |  |  |
| value_json | jsonb | YES |  |  |
| effective_from | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| effective_to | timestamp with time zone | YES |  |  |
| source | jsonb | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| created_by | text | YES | 'system'::text |  |
| attribute_uuid | uuid | YES |  | ob-poc.attribute_registry.uuid |

### ob-poc.case_decision_thresholds

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **threshold_id** (PK) | uuid | NO | gen_random_uuid() |  |
| threshold_name | character varying(100) | NO |  |  |
| min_score | integer | YES |  |  |
| max_score | integer | YES |  |  |
| has_hard_stop | boolean | YES | false |  |
| escalation_level | character varying(30) | YES |  |  |
| recommended_action | character varying(50) | NO |  |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.case_evaluation_snapshots

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **snapshot_id** (PK) | uuid | NO | gen_random_uuid() |  |
| case_id | uuid | NO |  | kyc.cases.case_id |
| soft_count | integer | NO | 0 |  |
| escalate_count | integer | NO | 0 |  |
| hard_stop_count | integer | NO | 0 |  |
| soft_score | integer | NO | 0 |  |
| escalate_score | integer | NO | 0 |  |
| has_hard_stop | boolean | NO | false |  |
| total_score | integer | NO | 0 |  |
| open_flags | integer | NO | 0 |  |
| mitigated_flags | integer | NO | 0 |  |
| waived_flags | integer | NO | 0 |  |
| matched_threshold_id | uuid | YES |  | ob-poc.case_decision_thresholds.threshold_id |
| recommended_action | character varying(50) | YES |  |  |
| required_escalation_level | character varying(30) | YES |  |  |
| evaluated_at | timestamp with time zone | NO | now() |  |
| evaluated_by | character varying(255) | YES |  |  |
| notes | text | YES |  |  |
| decision_made | character varying(50) | YES |  |  |
| decision_made_at | timestamp with time zone | YES |  |  |
| decision_made_by | character varying(255) | YES |  |  |
| decision_notes | text | YES |  |  |

### ob-poc.case_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(50) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |

### ob-poc.cbu_change_log

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **log_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| change_type | character varying(50) | NO |  |  |
| field_name | character varying(100) | YES |  |  |
| old_value | jsonb | YES |  |  |
| new_value | jsonb | YES |  |  |
| evidence_ids | ARRAY | YES |  |  |
| changed_at | timestamp with time zone | YES | now() |  |
| changed_by | character varying(255) | YES |  |  |
| reason | text | YES |  |  |
| case_id | uuid | YES |  |  |

### ob-poc.cbu_creation_log

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **log_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| nature_purpose | text | YES |  |  |
| source_of_funds | text | YES |  |  |
| ai_instruction | text | YES |  |  |
| generated_dsl | text | YES |  |  |
| created_at | timestamp with time zone | YES | CURRENT_TIMESTAMP |  |

### ob-poc.cbu_entity_roles

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **cbu_entity_role_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| role_id | uuid | NO |  | ob-poc.roles.role_id |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.cbu_evidence

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **evidence_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| document_id | uuid | YES |  | ob-poc.document_catalog.doc_id |
| attestation_ref | character varying(255) | YES |  |  |
| evidence_type | character varying(50) | NO |  |  |
| evidence_category | character varying(50) | YES |  |  |
| description | text | YES |  |  |
| attached_at | timestamp with time zone | YES | now() |  |
| attached_by | character varying(255) | YES |  |  |
| verified_at | timestamp with time zone | YES |  |  |
| verified_by | character varying(255) | YES |  |  |
| verification_status | character varying(30) | YES | 'PENDING'::character varying |  |
| verification_notes | text | YES |  |  |

### ob-poc.cbu_layout_overrides

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **cbu_id** (PK) | uuid | NO |  |  |
| **user_id** (PK) | uuid | NO |  |  |
| **view_mode** (PK) | text | NO |  |  |
| positions | jsonb | NO | '[]'::jsonb |  |
| sizes | jsonb | NO | '[]'::jsonb |  |
| updated_at | timestamp with time zone | NO | now() |  |

### ob-poc.cbu_resource_instances

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **instance_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| product_id | uuid | YES |  | ob-poc.products.product_id |
| service_id | uuid | YES |  | ob-poc.services.service_id |
| resource_type_id | uuid | YES |  | ob-poc.service_resource_types.resource_id |
| instance_url | character varying(1024) | NO |  |  |
| instance_identifier | character varying(255) | YES |  |  |
| instance_name | character varying(255) | YES |  |  |
| instance_config | jsonb | YES | '{}'::jsonb |  |
| status | character varying(50) | NO | 'PENDING'::character varying |  |
| requested_at | timestamp with time zone | YES | now() |  |
| provisioned_at | timestamp with time zone | YES |  |  |
| activated_at | timestamp with time zone | YES |  |  |
| decommissioned_at | timestamp with time zone | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.cbus

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **cbu_id** (PK) | uuid | NO | gen_random_uuid() |  |
| name | character varying(255) | NO |  |  |
| description | text | YES |  |  |
| nature_purpose | text | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| source_of_funds | text | YES |  |  |
| client_type | character varying(100) | YES |  |  |
| jurisdiction | character varying(50) | YES |  |  |
| risk_context | jsonb | YES | '{}'::jsonb |  |
| onboarding_context | jsonb | YES | '{}'::jsonb |  |
| semantic_context | jsonb | YES | '{}'::jsonb |  |
| embedding | USER-DEFINED | YES |  |  |
| embedding_model | character varying(100) | YES |  |  |
| embedding_updated_at | timestamp with time zone | YES |  |  |
| commercial_client_entity_id | uuid | YES |  | ob-poc.entities.entity_id |
| cbu_category | character varying(50) | YES |  |  |
| product_id | uuid | YES |  | ob-poc.products.product_id |
| status | character varying(30) | YES | 'DISCOVERED'::character varying |  |

### ob-poc.client_allegations

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **allegation_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| case_id | uuid | YES |  | kyc.cases.case_id |
| workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| attribute_id | uuid | NO |  | ob-poc.attribute_registry.uuid |
| alleged_value | jsonb | NO |  |  |
| alleged_value_display | text | YES |  |  |
| alleged_at | timestamp with time zone | NO | now() |  |
| alleged_by | text | YES |  |  |
| allegation_source | character varying(50) | NO |  |  |
| allegation_reference | text | YES |  |  |
| verification_status | character varying(30) | YES | 'PENDING'::character varying |  |
| verified_by_observation_id | uuid | YES |  | ob-poc.attribute_observations.observation_id |
| verification_result | character varying(30) | YES |  |  |
| verification_notes | text | YES |  |  |
| verified_at | timestamp with time zone | YES |  |  |
| verified_by | text | YES |  |  |
| created_at | timestamp with time zone | NO | now() |  |
| updated_at | timestamp with time zone | NO | now() |  |

### ob-poc.client_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(50) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |

### ob-poc.crud_operations

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **operation_id** (PK) | uuid | NO | gen_random_uuid() |  |
| operation_type | character varying(20) | NO |  |  |
| asset_type | character varying(50) | NO |  |  |
| entity_table_name | character varying(100) | YES |  |  |
| generated_dsl | text | NO |  |  |
| ai_instruction | text | NO |  |  |
| affected_records | jsonb | NO | '[]'::jsonb |  |
| execution_status | character varying(20) | NO | 'PENDING'::character varying |  |
| ai_confidence | numeric | YES |  |  |
| ai_provider | character varying(50) | YES |  |  |
| ai_model | character varying(100) | YES |  |  |
| execution_time_ms | integer | YES |  |  |
| error_message | text | YES |  |  |
| created_by | character varying(255) | YES | 'agentic_system'::character varying |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| completed_at | timestamp with time zone | YES |  |  |
| rows_affected | integer | YES | 0 |  |
| transaction_id | uuid | YES |  |  |
| parent_operation_id | uuid | YES |  | ob-poc.crud_operations.operation_id |

### ob-poc.csg_validation_rules

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **rule_id** (PK) | uuid | NO | gen_random_uuid() |  |
| rule_code | character varying(100) | NO |  |  |
| rule_name | character varying(255) | NO |  |  |
| rule_version | integer | YES | 1 |  |
| target_type | character varying(50) | NO |  |  |
| target_code | character varying(100) | YES |  |  |
| rule_type | character varying(50) | NO |  |  |
| rule_params | jsonb | NO |  |  |
| error_code | character varying(10) | NO |  |  |
| error_message_template | text | NO |  |  |
| suggestion_template | text | YES |  |  |
| severity | character varying(20) | YES | 'error'::character varying |  |
| description | text | YES |  |  |
| rationale | text | YES |  |  |
| documentation_url | text | YES |  |  |
| is_active | boolean | YES | true |  |
| effective_from | timestamp with time zone | YES | now() |  |
| effective_until | timestamp with time zone | YES |  |  |
| created_by | character varying(255) | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.currencies

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **currency_id** (PK) | uuid | NO | gen_random_uuid() |  |
| iso_code | character varying(3) | NO |  |  |
| name | character varying(100) | NO |  |  |
| symbol | character varying(10) | YES |  |  |
| decimal_places | integer | YES | 2 |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.dictionary

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **attribute_id** (PK) | uuid | NO | gen_random_uuid() |  |
| name | character varying(255) | NO |  |  |
| long_description | text | YES |  |  |
| group_id | character varying(100) | NO | 'default'::character varying |  |
| mask | character varying(50) | YES | 'string'::character varying |  |
| domain | character varying(100) | YES |  |  |
| vector | text | YES |  |  |
| source | jsonb | YES |  |  |
| sink | jsonb | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.document_attribute_links

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **link_id** (PK) | uuid | NO | gen_random_uuid() |  |
| document_type_id | uuid | NO |  | ob-poc.document_types.type_id |
| attribute_id | uuid | NO |  | ob-poc.attribute_registry.uuid |
| direction | character varying(10) | NO |  |  |
| extraction_method | character varying(50) | YES |  |  |
| extraction_field_path | jsonb | YES |  |  |
| extraction_confidence_default | numeric | YES | 0.80 |  |
| extraction_hints | jsonb | YES | '{}'::jsonb |  |
| is_authoritative | boolean | YES | false |  |
| proof_strength | character varying(20) | YES |  |  |
| alternative_doc_types | ARRAY | YES |  |  |
| entity_types | ARRAY | YES |  |  |
| jurisdictions | ARRAY | YES |  |  |
| client_types | ARRAY | YES |  |  |
| is_active | boolean | YES | true |  |
| notes | text | YES |  |  |
| created_at | timestamp with time zone | NO | now() |  |
| updated_at | timestamp with time zone | NO | now() |  |

### ob-poc.document_attribute_mappings

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **mapping_id** (PK) | uuid | NO | gen_random_uuid() |  |
| document_type_id | uuid | NO |  | ob-poc.document_types.type_id |
| attribute_uuid | uuid | NO |  | ob-poc.attribute_registry.uuid |
| extraction_method | character varying(50) | NO |  |  |
| field_location | jsonb | YES |  |  |
| field_name | character varying(255) | YES |  |  |
| confidence_threshold | numeric | YES | 0.80 |  |
| is_required | boolean | YES | false |  |
| validation_pattern | text | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.document_catalog

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **doc_id** (PK) | uuid | NO | gen_random_uuid() |  |
| file_hash_sha256 | text | YES |  |  |
| storage_key | text | YES |  |  |
| file_size_bytes | bigint | YES |  |  |
| mime_type | character varying(100) | YES |  |  |
| extracted_data | jsonb | YES |  |  |
| extraction_status | character varying(50) | YES | 'PENDING'::character varying |  |
| extraction_confidence | numeric | YES |  |  |
| last_extracted_at | timestamp with time zone | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| cbu_id | uuid | YES |  | ob-poc.cbus.cbu_id |
| document_type_id | uuid | YES |  | ob-poc.document_types.type_id |
| document_id | uuid | YES | gen_random_uuid() |  |
| document_type_code | character varying(100) | YES |  |  |
| document_name | character varying(255) | YES |  |  |
| source_system | character varying(100) | YES |  |  |
| status | character varying(50) | YES | 'active'::character varying |  |
| metadata | jsonb | YES | '{}'::jsonb |  |
| entity_id | uuid | YES |  | ob-poc.entities.entity_id |

### ob-poc.document_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **type_id** (PK) | uuid | NO | gen_random_uuid() |  |
| type_code | character varying(100) | NO |  |  |
| display_name | character varying(200) | NO |  |  |
| category | character varying(100) | NO |  |  |
| domain | character varying(100) | YES |  |  |
| description | text | YES |  |  |
| required_attributes | jsonb | YES | '{}'::jsonb |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |
| applicability | jsonb | YES | '{}'::jsonb |  |
| semantic_context | jsonb | YES | '{}'::jsonb |  |
| embedding | USER-DEFINED | YES |  |  |
| embedding_model | character varying(100) | YES |  |  |
| embedding_updated_at | timestamp with time zone | YES |  |  |

### ob-poc.document_validity_rules

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **rule_id** (PK) | uuid | NO | gen_random_uuid() |  |
| document_type_id | uuid | NO |  | ob-poc.document_types.type_id |
| rule_type | character varying(50) | NO |  |  |
| rule_value | integer | YES |  |  |
| rule_unit | character varying(20) | YES |  |  |
| rule_parameters | jsonb | YES |  |  |
| applies_to_jurisdictions | ARRAY | YES |  |  |
| applies_to_entity_types | ARRAY | YES |  |  |
| warning_days | integer | YES | 30 |  |
| is_hard_requirement | boolean | YES | true |  |
| regulatory_source | character varying(200) | YES |  |  |
| notes | text | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.dsl_domains

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **domain_id** (PK) | uuid | NO | gen_random_uuid() |  |
| domain_name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| base_grammar_version | character varying(20) | YES | '1.0.0'::character varying |  |
| vocabulary_version | character varying(20) | YES | '1.0.0'::character varying |  |
| active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.dsl_examples

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **example_id** (PK) | uuid | NO | gen_random_uuid() |  |
| title | character varying(255) | NO |  |  |
| description | text | YES |  |  |
| operation_type | character varying(20) | NO |  |  |
| asset_type | character varying(50) | NO |  |  |
| entity_table_name | character varying(100) | YES |  |  |
| natural_language_input | text | NO |  |  |
| example_dsl | text | NO |  |  |
| expected_outcome | text | YES |  |  |
| tags | ARRAY | YES | ARRAY[]::text[] |  |
| complexity_level | character varying(20) | YES | 'MEDIUM'::character varying |  |
| success_rate | numeric | YES | 1.0 |  |
| usage_count | integer | YES | 0 |  |
| last_used_at | timestamp with time zone | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| created_by | character varying(255) | YES | 'system'::character varying |  |

### ob-poc.dsl_execution_log

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **execution_id** (PK) | uuid | NO | gen_random_uuid() |  |
| version_id | uuid | NO |  | ob-poc.dsl_versions.version_id |
| cbu_id | character varying(255) | YES |  |  |
| execution_phase | character varying(50) | NO |  |  |
| status | character varying(50) | NO |  |  |
| result_data | jsonb | YES |  |  |
| error_details | jsonb | YES |  |  |
| performance_metrics | jsonb | YES |  |  |
| executed_by | character varying(255) | YES |  |  |
| started_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| completed_at | timestamp with time zone | YES |  |  |
| duration_ms | integer | YES |  |  |

### ob-poc.dsl_generation_log

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **log_id** (PK) | uuid | NO | gen_random_uuid() |  |
| instance_id | uuid | YES |  | ob-poc.dsl_instances.instance_id |
| user_intent | text | NO |  |  |
| final_valid_dsl | text | YES |  |  |
| iterations | jsonb | NO | '[]'::jsonb |  |
| domain_name | character varying(50) | NO |  |  |
| session_id | uuid | YES |  |  |
| cbu_id | uuid | YES |  |  |
| model_used | character varying(100) | YES |  |  |
| total_attempts | integer | NO | 1 |  |
| success | boolean | NO | false |  |
| total_latency_ms | integer | YES |  |  |
| total_input_tokens | integer | YES |  |  |
| total_output_tokens | integer | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| completed_at | timestamp with time zone | YES |  |  |

### ob-poc.dsl_idempotency

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **idempotency_key** (PK) | text | NO |  |  |
| execution_id | uuid | NO |  |  |
| statement_index | integer | NO |  |  |
| verb | text | NO |  |  |
| args_hash | text | NO |  |  |
| result_type | text | NO |  |  |
| result_id | uuid | YES |  |  |
| result_json | jsonb | YES |  |  |
| result_affected | bigint | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.dsl_instance_versions

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **version_id** (PK) | uuid | NO | gen_random_uuid() |  |
| instance_id | uuid | NO |  | ob-poc.dsl_instances.instance_id |
| version_number | integer | NO |  |  |
| dsl_content | text | NO |  |  |
| operation_type | character varying(100) | NO |  |  |
| compilation_status | character varying(50) | YES | 'COMPILED'::character varying |  |
| ast_json | jsonb | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| unresolved_count | integer | YES | 0 |  |
| total_refs | integer | YES | 0 |  |

### ob-poc.dsl_instances

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **id** (PK) | integer | NO | nextval('"ob-poc".dsl_instances_id_seq': |  |
| case_id | character varying(255) | YES |  |  |
| dsl_content | text | YES |  |  |
| domain | character varying(100) | YES |  |  |
| operation_type | character varying(100) | YES |  |  |
| status | character varying(50) | YES | 'PROCESSED'::character varying |  |
| processing_time_ms | bigint | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |
| instance_id | uuid | NO | gen_random_uuid() |  |
| domain_name | character varying(100) | YES |  |  |
| business_reference | character varying(255) | NO |  |  |
| current_version | integer | YES | 1 |  |

### ob-poc.dsl_ob

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **version_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| dsl_text | text | NO |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.dsl_session_events

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **event_id** (PK) | uuid | NO | gen_random_uuid() |  |
| session_id | uuid | NO |  | ob-poc.dsl_sessions.session_id |
| event_type | character varying(30) | NO |  |  |
| dsl_source | text | YES |  |  |
| error_message | text | YES |  |  |
| metadata | jsonb | NO | '{}'::jsonb |  |
| occurred_at | timestamp with time zone | NO | now() |  |

### ob-poc.dsl_session_locks

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **session_id** (PK) | uuid | NO |  | ob-poc.dsl_sessions.session_id |
| locked_at | timestamp with time zone | NO | now() |  |
| lock_timeout_at | timestamp with time zone | NO | (now() + '00:00:30'::interval) |  |
| operation | character varying(50) | NO |  |  |

### ob-poc.dsl_sessions

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **session_id** (PK) | uuid | NO | gen_random_uuid() |  |
| status | character varying(20) | NO | 'active'::character varying |  |
| primary_domain | character varying(30) | YES |  |  |
| cbu_id | uuid | YES |  | ob-poc.cbus.cbu_id |
| kyc_case_id | uuid | YES |  | kyc.cases.case_id |
| onboarding_request_id | uuid | YES |  | ob-poc.onboarding_requests.request_id |
| named_refs | jsonb | NO | '{}'::jsonb |  |
| client_type | character varying(50) | YES |  |  |
| jurisdiction | character varying(10) | YES |  |  |
| created_at | timestamp with time zone | NO | now() |  |
| last_activity_at | timestamp with time zone | NO | now() |  |
| expires_at | timestamp with time zone | NO | (now() + '24:00:00'::interval) |  |
| completed_at | timestamp with time zone | YES |  |  |
| error_count | integer | NO | 0 |  |
| last_error | text | YES |  |  |
| last_error_at | timestamp with time zone | YES |  |  |

### ob-poc.dsl_snapshots

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **snapshot_id** (PK) | uuid | NO | gen_random_uuid() |  |
| session_id | uuid | NO |  | ob-poc.dsl_sessions.session_id |
| version | integer | NO |  |  |
| dsl_source | text | NO |  |  |
| dsl_checksum | character varying(64) | NO |  |  |
| success | boolean | NO | true |  |
| bindings_captured | jsonb | NO | '{}'::jsonb |  |
| entities_created | jsonb | NO | '[]'::jsonb |  |
| domains_used | ARRAY | NO | '{}'::text[] |  |
| executed_at | timestamp with time zone | NO | now() |  |
| execution_ms | integer | YES |  |  |

### ob-poc.dsl_versions

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **version_id** (PK) | uuid | NO | gen_random_uuid() |  |
| domain_id | uuid | NO |  | ob-poc.dsl_domains.domain_id |
| version_number | integer | NO |  |  |
| functional_state | character varying(100) | YES |  |  |
| dsl_source_code | text | NO |  |  |
| compilation_status | character varying(50) | YES | 'DRAFT'::character varying |  |
| change_description | text | YES |  |  |
| parent_version_id | uuid | YES |  | ob-poc.dsl_versions.version_id |
| created_by | character varying(255) | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| compiled_at | timestamp with time zone | YES |  |  |
| activated_at | timestamp with time zone | YES |  |  |

### ob-poc.entities

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **entity_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_type_id | uuid | NO |  | ob-poc.entity_types.entity_type_id |
| external_id | character varying(255) | YES |  |  |
| name | character varying(255) | NO |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.entity_crud_rules

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **rule_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_table_name | character varying(100) | NO |  |  |
| operation_type | character varying(20) | NO |  |  |
| field_name | character varying(100) | YES |  |  |
| constraint_type | character varying(50) | NO |  |  |
| constraint_description | text | NO |  |  |
| validation_pattern | character varying(500) | YES |  |  |
| error_message | text | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.entity_limited_companies

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **limited_company_id** (PK) | uuid | NO | gen_random_uuid() |  |
| company_name | character varying(255) | NO |  |  |
| registration_number | character varying(100) | YES |  |  |
| jurisdiction | character varying(100) | YES |  |  |
| incorporation_date | date | YES |  |  |
| registered_address | text | YES |  |  |
| business_nature | text | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| entity_id | uuid | YES |  | ob-poc.entities.entity_id |

### ob-poc.entity_partnerships

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **partnership_id** (PK) | uuid | NO | gen_random_uuid() |  |
| partnership_name | character varying(255) | NO |  |  |
| partnership_type | character varying(100) | YES |  |  |
| jurisdiction | character varying(100) | YES |  |  |
| formation_date | date | YES |  |  |
| principal_place_business | text | YES |  |  |
| partnership_agreement_date | date | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| entity_id | uuid | YES |  | ob-poc.entities.entity_id |

### ob-poc.entity_proper_persons

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **proper_person_id** (PK) | uuid | NO | gen_random_uuid() |  |
| first_name | character varying(255) | NO |  |  |
| last_name | character varying(255) | NO |  |  |
| middle_names | character varying(255) | YES |  |  |
| date_of_birth | date | YES |  |  |
| nationality | character varying(100) | YES |  |  |
| residence_address | text | YES |  |  |
| id_document_type | character varying(100) | YES |  |  |
| id_document_number | character varying(100) | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| search_name | text | YES |  |  |
| entity_id | uuid | YES |  | ob-poc.entities.entity_id |

### ob-poc.entity_trusts

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **trust_id** (PK) | uuid | NO | gen_random_uuid() |  |
| trust_name | character varying(255) | NO |  |  |
| trust_type | character varying(100) | YES |  |  |
| jurisdiction | character varying(100) | NO |  |  |
| establishment_date | date | YES |  |  |
| trust_deed_date | date | YES |  |  |
| trust_purpose | text | YES |  |  |
| governing_law | character varying(100) | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| entity_id | uuid | YES |  | ob-poc.entities.entity_id |

### ob-poc.entity_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **entity_type_id** (PK) | uuid | NO | gen_random_uuid() |  |
| name | character varying(255) | NO |  |  |
| description | text | YES |  |  |
| table_name | character varying(255) | NO |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| type_code | character varying(100) | YES |  |  |
| semantic_context | jsonb | YES | '{}'::jsonb |  |
| parent_type_id | uuid | YES |  | ob-poc.entity_types.entity_type_id |
| type_hierarchy_path | ARRAY | YES |  |  |
| embedding | USER-DEFINED | YES |  |  |
| embedding_model | character varying(100) | YES |  |  |
| embedding_updated_at | timestamp with time zone | YES |  |  |

### ob-poc.entity_validation_rules

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **rule_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_type | character varying(50) | NO |  |  |
| field_name | character varying(100) | NO |  |  |
| validation_type | character varying(50) | NO |  |  |
| validation_rule | jsonb | NO |  |  |
| error_message | character varying(500) | YES |  |  |
| severity | character varying(20) | YES | 'ERROR'::character varying |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.master_entity_xref

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **xref_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_type | character varying(50) | NO |  |  |
| entity_id | uuid | NO |  |  |
| entity_name | character varying(500) | NO |  |  |
| jurisdiction_code | character varying(10) | YES |  | ob-poc.master_jurisdictions.jurisdiction_code |
| entity_status | character varying(50) | YES | 'ACTIVE'::character varying |  |
| business_purpose | text | YES |  |  |
| primary_contact_person | uuid | YES |  |  |
| regulatory_numbers | jsonb | YES | '{}'::jsonb |  |
| additional_metadata | jsonb | YES | '{}'::jsonb |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.master_jurisdictions

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **jurisdiction_code** (PK) | character varying(10) | NO |  |  |
| jurisdiction_name | character varying(200) | NO |  |  |
| country_code | character varying(3) | NO |  |  |
| region | character varying(100) | YES |  |  |
| regulatory_framework | character varying(100) | YES |  |  |
| entity_formation_allowed | boolean | YES | true |  |
| offshore_jurisdiction | boolean | YES | false |  |
| regulatory_authority | character varying(300) | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.observation_discrepancies

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **discrepancy_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| attribute_id | uuid | NO |  | ob-poc.attribute_registry.uuid |
| case_id | uuid | YES |  | kyc.cases.case_id |
| workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| observation_1_id | uuid | NO |  | ob-poc.attribute_observations.observation_id |
| observation_2_id | uuid | NO |  | ob-poc.attribute_observations.observation_id |
| discrepancy_type | character varying(30) | NO |  |  |
| severity | character varying(20) | NO |  |  |
| description | text | NO |  |  |
| value_1_display | text | YES |  |  |
| value_2_display | text | YES |  |  |
| resolution_status | character varying(30) | YES | 'OPEN'::character varying |  |
| resolution_type | character varying(30) | YES |  |  |
| resolution_notes | text | YES |  |  |
| resolved_at | timestamp with time zone | YES |  |  |
| resolved_by | text | YES |  |  |
| accepted_observation_id | uuid | YES |  | ob-poc.attribute_observations.observation_id |
| red_flag_id | uuid | YES |  | kyc.red_flags.red_flag_id |
| detected_at | timestamp with time zone | NO | now() |  |
| detected_by | text | YES | 'SYSTEM'::text |  |
| created_at | timestamp with time zone | NO | now() |  |
| updated_at | timestamp with time zone | NO | now() |  |

### ob-poc.onboarding_products

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **onboarding_product_id** (PK) | uuid | NO | gen_random_uuid() |  |
| request_id | uuid | NO |  | ob-poc.onboarding_requests.request_id |
| product_id | uuid | NO |  | ob-poc.products.product_id |
| selection_order | integer | YES |  |  |
| selected_at | timestamp with time zone | YES | now() |  |

### ob-poc.onboarding_requests

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **request_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| request_state | character varying(50) | NO | 'draft'::character varying |  |
| dsl_draft | text | YES |  |  |
| dsl_version | integer | YES | 1 |  |
| current_phase | character varying(100) | YES |  |  |
| phase_metadata | jsonb | YES |  |  |
| validation_errors | jsonb | YES |  |  |
| created_by | character varying(255) | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |
| completed_at | timestamp with time zone | YES |  |  |

### ob-poc.ownership_relationships

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **ownership_id** (PK) | uuid | NO | gen_random_uuid() |  |
| owner_entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| owned_entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| ownership_type | character varying(30) | NO |  |  |
| ownership_percent | numeric | NO |  |  |
| effective_from | date | YES | CURRENT_DATE |  |
| effective_to | date | YES |  |  |
| evidence_doc_id | uuid | YES |  | ob-poc.document_catalog.doc_id |
| notes | text | YES |  |  |
| created_by | character varying(255) | YES | 'system'::character varying |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.product_services

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **product_id** (PK) | uuid | NO |  | ob-poc.products.product_id |
| **service_id** (PK) | uuid | NO |  | ob-poc.services.service_id |
| is_mandatory | boolean | YES | false |  |
| is_default | boolean | YES | false |  |
| display_order | integer | YES |  |  |
| configuration | jsonb | YES |  |  |

### ob-poc.products

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **product_id** (PK) | uuid | NO | gen_random_uuid() |  |
| name | character varying(255) | NO |  |  |
| description | text | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| product_code | character varying(50) | YES |  |  |
| product_category | character varying(100) | YES |  |  |
| regulatory_framework | character varying(100) | YES |  |  |
| min_asset_requirement | numeric | YES |  |  |
| is_active | boolean | YES | true |  |
| metadata | jsonb | YES |  |  |

### ob-poc.red_flag_severities

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(50) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| is_blocking | boolean | YES | false |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |

### ob-poc.redflag_score_config

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **config_id** (PK) | uuid | NO | gen_random_uuid() |  |
| severity | character varying(20) | NO |  |  |
| weight | integer | NO |  |  |
| is_blocking | boolean | YES | false |  |
| description | text | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.requirement_acceptable_docs

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **requirement_id** (PK) | uuid | NO |  | ob-poc.threshold_requirements.requirement_id |
| **document_type_code** (PK) | character varying(50) | NO |  | ob-poc.document_types.type_code |
| priority | integer | YES | 1 |  |

### ob-poc.resource_attribute_requirements

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **requirement_id** (PK) | uuid | NO | gen_random_uuid() |  |
| resource_id | uuid | NO |  | ob-poc.service_resource_types.resource_id |
| attribute_id | uuid | NO |  | ob-poc.attribute_registry.uuid |
| resource_field_name | character varying(255) | YES |  |  |
| is_mandatory | boolean | YES | true |  |
| transformation_rule | jsonb | YES |  |  |
| validation_override | jsonb | YES |  |  |
| default_value | text | YES |  |  |
| display_order | integer | YES | 0 |  |

### ob-poc.resource_instance_attributes

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **value_id** (PK) | uuid | NO | gen_random_uuid() |  |
| instance_id | uuid | NO |  | ob-poc.cbu_resource_instances.instance_id |
| attribute_id | uuid | NO |  | ob-poc.attribute_registry.uuid |
| value_text | character varying | YES |  |  |
| value_number | numeric | YES |  |  |
| value_boolean | boolean | YES |  |  |
| value_date | date | YES |  |  |
| value_timestamp | timestamp with time zone | YES |  |  |
| value_json | jsonb | YES |  |  |
| state | character varying(50) | YES | 'proposed'::character varying |  |
| source | jsonb | YES |  |  |
| observed_at | timestamp with time zone | YES | now() |  |

### ob-poc.risk_bands

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **band_code** (PK) | character varying(20) | NO |  |  |
| min_score | integer | NO |  |  |
| max_score | integer | NO |  |  |
| description | text | YES |  |  |
| escalation_required | boolean | YES | false |  |
| review_frequency_months | integer | YES | 12 |  |

### ob-poc.risk_ratings

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(50) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| severity_level | integer | YES | 0 |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |

### ob-poc.roles

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **role_id** (PK) | uuid | NO | gen_random_uuid() |  |
| name | character varying(255) | NO |  |  |
| description | text | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| role_category | character varying(30) | YES |  |  |

### ob-poc.schema_changes

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **change_id** (PK) | uuid | NO | gen_random_uuid() |  |
| change_type | character varying(50) | NO |  |  |
| description | text | NO |  |  |
| script_name | character varying(255) | YES |  |  |
| applied_at | timestamp with time zone | YES | now() |  |
| applied_by | character varying(100) | YES | CURRENT_USER |  |

### ob-poc.screening_lists

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **screening_list_id** (PK) | uuid | NO | gen_random_uuid() |  |
| list_code | character varying(50) | NO |  |  |
| list_name | character varying(255) | NO |  |  |
| list_type | character varying(50) | NO |  |  |
| provider | character varying(100) | YES |  |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.screening_requirements

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **risk_band** (PK) | character varying(20) | NO |  | ob-poc.risk_bands.band_code |
| **screening_type** (PK) | character varying(50) | NO |  |  |
| is_required | boolean | NO | true |  |
| frequency_months | integer | YES | 12 |  |

### ob-poc.screening_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(50) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |

### ob-poc.service_delivery_map

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **delivery_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| product_id | uuid | NO |  | ob-poc.products.product_id |
| service_id | uuid | NO |  | ob-poc.services.service_id |
| instance_id | uuid | YES |  | ob-poc.cbu_resource_instances.instance_id |
| service_config | jsonb | YES | '{}'::jsonb |  |
| delivery_status | character varying(50) | YES | 'PENDING'::character varying |  |
| requested_at | timestamp with time zone | YES | now() |  |
| started_at | timestamp with time zone | YES |  |  |
| delivered_at | timestamp with time zone | YES |  |  |
| failed_at | timestamp with time zone | YES |  |  |
| failure_reason | text | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### ob-poc.service_option_choices

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **choice_id** (PK) | uuid | NO | gen_random_uuid() |  |
| option_def_id | uuid | NO |  | ob-poc.service_option_definitions.option_def_id |
| choice_value | character varying(255) | NO |  |  |
| choice_label | character varying(255) | YES |  |  |
| choice_metadata | jsonb | YES |  |  |
| is_default | boolean | YES | false |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES |  |  |
| requires_options | jsonb | YES |  |  |
| excludes_options | jsonb | YES |  |  |

### ob-poc.service_option_definitions

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **option_def_id** (PK) | uuid | NO | gen_random_uuid() |  |
| service_id | uuid | NO |  | ob-poc.services.service_id |
| option_key | character varying(100) | NO |  |  |
| option_label | character varying(255) | YES |  |  |
| option_type | character varying(50) | NO |  |  |
| validation_rules | jsonb | YES |  |  |
| is_required | boolean | YES | false |  |
| display_order | integer | YES |  |  |
| help_text | text | YES |  |  |

### ob-poc.service_resource_capabilities

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **capability_id** (PK) | uuid | NO | gen_random_uuid() |  |
| service_id | uuid | NO |  | ob-poc.services.service_id |
| resource_id | uuid | NO |  | ob-poc.service_resource_types.resource_id |
| supported_options | jsonb | NO |  |  |
| priority | integer | YES | 100 |  |
| cost_factor | numeric | YES | 1.0 |  |
| performance_rating | integer | YES |  |  |
| resource_config | jsonb | YES |  |  |
| is_active | boolean | YES | true |  |

### ob-poc.service_resource_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **resource_id** (PK) | uuid | NO | gen_random_uuid() |  |
| name | character varying(255) | NO |  |  |
| description | text | YES |  |  |
| owner | character varying(255) | NO |  |  |
| dictionary_group | character varying(100) | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| resource_code | character varying(50) | YES |  |  |
| resource_type | character varying(100) | YES |  |  |
| vendor | character varying(255) | YES |  |  |
| version | character varying(50) | YES |  |  |
| api_endpoint | text | YES |  |  |
| api_version | character varying(20) | YES |  |  |
| authentication_method | character varying(50) | YES |  |  |
| authentication_config | jsonb | YES |  |  |
| capabilities | jsonb | YES |  |  |
| capacity_limits | jsonb | YES |  |  |
| maintenance_windows | jsonb | YES |  |  |
| is_active | boolean | YES | true |  |

### ob-poc.services

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **service_id** (PK) | uuid | NO | gen_random_uuid() |  |
| name | character varying(255) | NO |  |  |
| description | text | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| service_code | character varying(50) | YES |  |  |
| service_category | character varying(100) | YES |  |  |
| sla_definition | jsonb | YES |  |  |
| is_active | boolean | YES | true |  |

### ob-poc.settlement_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(20) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |

### ob-poc.ssi_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(50) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |

### ob-poc.taxonomy_crud_log

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **operation_id** (PK) | uuid | NO | gen_random_uuid() |  |
| operation_type | character varying(20) | NO |  |  |
| entity_type | character varying(50) | NO |  |  |
| entity_id | uuid | YES |  |  |
| natural_language_input | text | YES |  |  |
| parsed_dsl | text | YES |  |  |
| execution_result | jsonb | YES |  |  |
| success | boolean | YES | false |  |
| error_message | text | YES |  |  |
| user_id | character varying(255) | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| execution_time_ms | integer | YES |  |  |

### ob-poc.threshold_factors

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **factor_id** (PK) | uuid | NO | gen_random_uuid() |  |
| factor_type | character varying(50) | NO |  |  |
| factor_code | character varying(50) | NO |  |  |
| risk_weight | integer | NO | 1 |  |
| description | text | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.threshold_requirements

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **requirement_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_role | character varying(50) | NO |  |  |
| risk_band | character varying(20) | NO |  | ob-poc.risk_bands.band_code |
| attribute_code | character varying(50) | NO |  |  |
| is_required | boolean | NO | true |  |
| confidence_min | numeric | YES | 0.85 |  |
| max_age_days | integer | YES |  |  |
| must_be_authoritative | boolean | YES | false |  |
| notes | text | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.trust_parties

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **trust_party_id** (PK) | uuid | NO | gen_random_uuid() |  |
| trust_id | uuid | NO |  | ob-poc.entity_trusts.trust_id |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| party_role | character varying(100) | NO |  |  |
| party_type | character varying(100) | NO |  |  |
| appointment_date | date | YES |  |  |
| resignation_date | date | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |

### ob-poc.ubo_evidence

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **ubo_evidence_id** (PK) | uuid | NO | gen_random_uuid() |  |
| ubo_id | uuid | NO |  | ob-poc.ubo_registry.ubo_id |
| document_id | uuid | YES |  | ob-poc.document_catalog.doc_id |
| attestation_ref | character varying(255) | YES |  |  |
| evidence_type | character varying(50) | NO |  |  |
| evidence_role | character varying(50) | NO |  |  |
| description | text | YES |  |  |
| attached_at | timestamp with time zone | YES | now() |  |
| attached_by | character varying(255) | YES |  |  |
| verified_at | timestamp with time zone | YES |  |  |
| verified_by | character varying(255) | YES |  |  |
| verification_status | character varying(30) | YES | 'PENDING'::character varying |  |
| verification_notes | text | YES |  |  |

### ob-poc.ubo_registry

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **ubo_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| subject_entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| ubo_proper_person_id | uuid | NO |  | ob-poc.entities.entity_id |
| relationship_type | character varying(100) | NO |  |  |
| qualifying_reason | character varying(100) | NO |  |  |
| ownership_percentage | numeric | YES |  |  |
| control_type | character varying(100) | YES |  |  |
| workflow_type | character varying(100) | NO |  |  |
| regulatory_framework | character varying(100) | YES |  |  |
| verification_status | character varying(50) | YES | 'PENDING'::character varying |  |
| screening_result | character varying(50) | YES | 'PENDING'::character varying |  |
| risk_rating | character varying(50) | YES |  |  |
| identified_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| verified_at | timestamp with time zone | YES |  |  |
| created_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| updated_at | timestamp with time zone | YES | (now() AT TIME ZONE 'utc'::text) |  |
| case_id | uuid | YES |  | kyc.cases.case_id |
| workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| discovery_method | character varying(30) | YES | 'MANUAL'::character varying |  |
| superseded_by | uuid | YES |  | ob-poc.ubo_registry.ubo_id |
| superseded_at | timestamp with time zone | YES |  |  |
| closed_at | timestamp with time zone | YES |  |  |
| closed_reason | character varying(100) | YES |  |  |
| evidence_doc_ids | ARRAY | YES |  |  |
| proof_date | timestamp with time zone | YES |  |  |
| proof_method | character varying(50) | YES |  |  |
| proof_notes | text | YES |  |  |
| replacement_ubo_id | uuid | YES |  | ob-poc.ubo_registry.ubo_id |
| removal_reason | character varying(100) | YES |  |  |

### ob-poc.ubo_snapshot_comparisons

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **comparison_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| baseline_snapshot_id | uuid | NO |  | ob-poc.ubo_snapshots.snapshot_id |
| current_snapshot_id | uuid | NO |  | ob-poc.ubo_snapshots.snapshot_id |
| has_changes | boolean | NO | false |  |
| change_summary | jsonb | NO | '{}'::jsonb |  |
| added_ubos | jsonb | YES | '[]'::jsonb |  |
| removed_ubos | jsonb | YES | '[]'::jsonb |  |
| changed_ubos | jsonb | YES | '[]'::jsonb |  |
| ownership_changes | jsonb | YES | '[]'::jsonb |  |
| control_changes | jsonb | YES | '[]'::jsonb |  |
| compared_at | timestamp with time zone | NO | now() |  |
| compared_by | character varying(255) | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.ubo_snapshots

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **snapshot_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| case_id | uuid | YES |  | kyc.cases.case_id |
| snapshot_type | character varying(30) | NO | 'MANUAL'::character varying |  |
| snapshot_reason | character varying(100) | YES |  |  |
| ubos | jsonb | NO | '[]'::jsonb |  |
| ownership_chains | jsonb | NO | '[]'::jsonb |  |
| control_relationships | jsonb | NO | '[]'::jsonb |  |
| total_identified_ownership | numeric | YES |  |  |
| has_gaps | boolean | YES | false |  |
| gap_summary | text | YES |  |  |
| captured_at | timestamp with time zone | NO | now() |  |
| captured_by | character varying(255) | YES |  |  |
| notes | text | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |

### ob-poc.workstream_statuses

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **code** (PK) | character varying(50) | NO |  |  |
| name | character varying(100) | NO |  |  |
| description | text | YES |  |  |
| is_terminal | boolean | YES | false |  |
| is_active | boolean | YES | true |  |
| display_order | integer | YES | 0 |  |


## Custody Schema - Settlement & Reference Data

### custody.cbu_instrument_universe

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **universe_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| instrument_class_id | uuid | NO |  | custody.instrument_classes.class_id |
| market_id | uuid | YES |  | custody.markets.market_id |
| currencies | ARRAY | NO | '{}'::character varying[] |  |
| settlement_types | ARRAY | YES | '{DVP}'::character varying[] |  |
| counterparty_entity_id | uuid | YES |  | ob-poc.entities.entity_id |
| is_held | boolean | YES | true |  |
| is_traded | boolean | YES | true |  |
| is_active | boolean | YES | true |  |
| effective_date | date | NO | CURRENT_DATE |  |
| created_at | timestamp with time zone | YES | now() |  |

### custody.cbu_ssi

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **ssi_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| ssi_name | character varying(100) | NO |  |  |
| ssi_type | character varying(20) | NO |  |  |
| safekeeping_account | character varying(35) | YES |  |  |
| safekeeping_bic | character varying(11) | YES |  |  |
| safekeeping_account_name | character varying(100) | YES |  |  |
| cash_account | character varying(35) | YES |  |  |
| cash_account_bic | character varying(11) | YES |  |  |
| cash_currency | character varying(3) | YES |  |  |
| collateral_account | character varying(35) | YES |  |  |
| collateral_account_bic | character varying(11) | YES |  |  |
| pset_bic | character varying(11) | YES |  |  |
| receiving_agent_bic | character varying(11) | YES |  |  |
| delivering_agent_bic | character varying(11) | YES |  |  |
| status | character varying(20) | YES | 'PENDING'::character varying |  |
| effective_date | date | NO |  |  |
| expiry_date | date | YES |  |  |
| source | character varying(20) | YES | 'MANUAL'::character varying |  |
| source_reference | character varying(100) | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |
| created_by | character varying(100) | YES |  |  |
| market_id | uuid | YES |  | custody.markets.market_id |

### custody.cbu_ssi_agent_override

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **override_id** (PK) | uuid | NO | gen_random_uuid() |  |
| ssi_id | uuid | NO |  | custody.cbu_ssi.ssi_id |
| agent_role | character varying(10) | NO |  |  |
| agent_bic | character varying(11) | NO |  |  |
| agent_account | character varying(35) | YES |  |  |
| agent_name | character varying(100) | YES |  |  |
| sequence_order | integer | NO |  |  |
| reason | character varying(255) | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### custody.cfi_codes

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **cfi_code** (PK) | character(6) | NO |  |  |
| category | character(1) | NO |  |  |
| category_name | character varying(50) | YES |  |  |
| group_code | character(2) | NO |  |  |
| group_name | character varying(50) | YES |  |  |
| attribute_1 | character(1) | YES |  |  |
| attribute_2 | character(1) | YES |  |  |
| attribute_3 | character(1) | YES |  |  |
| attribute_4 | character(1) | YES |  |  |
| class_id | uuid | YES |  | custody.instrument_classes.class_id |
| security_type_id | uuid | YES |  | custody.security_types.security_type_id |
| created_at | timestamp with time zone | YES | now() |  |

### custody.csa_agreements

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **csa_id** (PK) | uuid | NO | gen_random_uuid() |  |
| isda_id | uuid | NO |  | custody.isda_agreements.isda_id |
| csa_type | character varying(20) | NO |  |  |
| threshold_amount | numeric | YES |  |  |
| threshold_currency | character varying(3) | YES |  |  |
| minimum_transfer_amount | numeric | YES |  |  |
| rounding_amount | numeric | YES |  |  |
| collateral_ssi_id | uuid | YES |  | custody.cbu_ssi.ssi_id |
| is_active | boolean | YES | true |  |
| effective_date | date | NO |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.entity_settlement_identity

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **identity_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| primary_bic | character varying(11) | NO |  |  |
| lei | character varying(20) | YES |  |  |
| alert_participant_id | character varying(50) | YES |  |  |
| ctm_participant_id | character varying(50) | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.entity_ssi

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **entity_ssi_id** (PK) | uuid | NO | gen_random_uuid() |  |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| instrument_class_id | uuid | YES |  | custody.instrument_classes.class_id |
| security_type_id | uuid | YES |  | custody.security_types.security_type_id |
| market_id | uuid | YES |  | custody.markets.market_id |
| currency | character varying(3) | YES |  |  |
| counterparty_bic | character varying(11) | NO |  |  |
| safekeeping_account | character varying(35) | YES |  |  |
| source | character varying(20) | YES | 'ALERT'::character varying |  |
| source_reference | character varying(100) | YES |  |  |
| status | character varying(20) | YES | 'ACTIVE'::character varying |  |
| effective_date | date | NO |  |  |
| expiry_date | date | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.instruction_paths

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **path_id** (PK) | uuid | NO | gen_random_uuid() |  |
| instrument_class_id | uuid | YES |  | custody.instrument_classes.class_id |
| market_id | uuid | YES |  | custody.markets.market_id |
| currency | character varying(3) | YES |  |  |
| instruction_type_id | uuid | NO |  | custody.instruction_types.type_id |
| resource_id | uuid | NO |  | ob-poc.service_resource_types.resource_id |
| routing_priority | integer | YES | 1 |  |
| enrichment_sources | jsonb | YES | '["SUBCUST_NETWORK", "CLIENT_SSI"]'::jso |  |
| validation_rules | jsonb | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.instruction_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **type_id** (PK) | uuid | NO | gen_random_uuid() |  |
| type_code | character varying(30) | NO |  |  |
| name | character varying(100) | NO |  |  |
| direction | character varying(10) | NO |  |  |
| payment_type | character varying(10) | NO |  |  |
| swift_mt_code | character varying(10) | YES |  |  |
| iso20022_msg_type | character varying(50) | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### custody.instrument_classes

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **class_id** (PK) | uuid | NO | gen_random_uuid() |  |
| code | character varying(20) | NO |  |  |
| name | character varying(100) | NO |  |  |
| default_settlement_cycle | character varying(10) | NO |  |  |
| swift_message_family | character varying(10) | YES |  |  |
| requires_isda | boolean | YES | false |  |
| requires_collateral | boolean | YES | false |  |
| cfi_category | character(1) | YES |  |  |
| cfi_group | character(2) | YES |  |  |
| smpg_group | character varying(20) | YES |  |  |
| isda_asset_class | character varying(30) | YES |  |  |
| parent_class_id | uuid | YES |  | custody.instrument_classes.class_id |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.isda_agreements

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **isda_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| counterparty_entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| agreement_date | date | NO |  |  |
| governing_law | character varying(20) | YES |  |  |
| is_active | boolean | YES | true |  |
| effective_date | date | NO |  |  |
| termination_date | date | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.isda_product_coverage

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **coverage_id** (PK) | uuid | NO | gen_random_uuid() |  |
| isda_id | uuid | NO |  | custody.isda_agreements.isda_id |
| instrument_class_id | uuid | NO |  | custody.instrument_classes.class_id |
| isda_taxonomy_id | uuid | YES |  | custody.isda_product_taxonomy.taxonomy_id |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### custody.isda_product_taxonomy

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **taxonomy_id** (PK) | uuid | NO | gen_random_uuid() |  |
| asset_class | character varying(30) | NO |  |  |
| base_product | character varying(50) | YES |  |  |
| sub_product | character varying(50) | YES |  |  |
| taxonomy_code | character varying(100) | NO |  |  |
| upi_template | character varying(50) | YES |  |  |
| class_id | uuid | YES |  | custody.instrument_classes.class_id |
| cfi_pattern | character varying(6) | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### custody.markets

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **market_id** (PK) | uuid | NO | gen_random_uuid() |  |
| mic | character varying(4) | NO |  |  |
| name | character varying(255) | NO |  |  |
| country_code | character varying(2) | NO |  |  |
| operating_mic | character varying(4) | YES |  |  |
| primary_currency | character varying(3) | NO |  |  |
| supported_currencies | ARRAY | YES | '{}'::character varying[] |  |
| csd_bic | character varying(11) | YES |  |  |
| timezone | character varying(50) | NO |  |  |
| cut_off_time | time without time zone | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.security_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **security_type_id** (PK) | uuid | NO | gen_random_uuid() |  |
| class_id | uuid | NO |  | custody.instrument_classes.class_id |
| code | character varying(4) | NO |  |  |
| name | character varying(100) | NO |  |  |
| cfi_pattern | character varying(6) | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |

### custody.ssi_booking_rules

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **rule_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| ssi_id | uuid | NO |  | custody.cbu_ssi.ssi_id |
| rule_name | character varying(100) | NO |  |  |
| priority | integer | NO | 50 |  |
| instrument_class_id | uuid | YES |  | custody.instrument_classes.class_id |
| security_type_id | uuid | YES |  | custody.security_types.security_type_id |
| market_id | uuid | YES |  | custody.markets.market_id |
| currency | character varying(3) | YES |  |  |
| settlement_type | character varying(10) | YES |  |  |
| counterparty_entity_id | uuid | YES |  | ob-poc.entities.entity_id |
| isda_asset_class | character varying(30) | YES |  |  |
| isda_base_product | character varying(50) | YES |  |  |
| specificity_score | integer | YES |  |  |
| is_active | boolean | YES | true |  |
| effective_date | date | NO | CURRENT_DATE |  |
| expiry_date | date | YES |  |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |

### custody.subcustodian_network

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **network_id** (PK) | uuid | NO | gen_random_uuid() |  |
| market_id | uuid | NO |  | custody.markets.market_id |
| currency | character varying(3) | NO |  |  |
| subcustodian_bic | character varying(11) | NO |  |  |
| subcustodian_name | character varying(255) | YES |  |  |
| local_agent_bic | character varying(11) | YES |  |  |
| local_agent_name | character varying(255) | YES |  |  |
| local_agent_account | character varying(35) | YES |  |  |
| csd_participant_id | character varying(35) | YES |  |  |
| place_of_settlement_bic | character varying(11) | NO |  |  |
| is_primary | boolean | YES | true |  |
| effective_date | date | NO |  |  |
| expiry_date | date | YES |  |  |
| is_active | boolean | YES | true |  |
| created_at | timestamp with time zone | YES | now() |  |
| updated_at | timestamp with time zone | YES | now() |  |


## KYC Schema - Case Management & Investor Registry

### kyc.approval_requests

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **approval_id** (PK) | uuid | NO | gen_random_uuid() |  |
| case_id | uuid | NO |  | kyc.cases.case_id |
| workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| request_type | character varying(50) | NO |  |  |
| requested_by | character varying(255) | YES |  |  |
| requested_at | timestamp with time zone | NO | now() |  |
| approver | character varying(255) | YES |  |  |
| decision | character varying(20) | YES |  |  |
| decision_at | timestamp with time zone | YES |  |  |
| comments | text | YES |  |  |

### kyc.case_events

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **event_id** (PK) | uuid | NO | gen_random_uuid() |  |
| case_id | uuid | NO |  | kyc.cases.case_id |
| workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| event_type | character varying(50) | NO |  |  |
| event_data | jsonb | YES | '{}'::jsonb |  |
| actor_id | uuid | YES |  |  |
| actor_type | character varying(20) | YES | 'USER'::character varying |  |
| occurred_at | timestamp with time zone | NO | now() |  |
| comment | text | YES |  |  |

### kyc.cases

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **case_id** (PK) | uuid | NO | gen_random_uuid() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| status | character varying(30) | NO | 'INTAKE'::character varying |  |
| escalation_level | character varying(30) | NO | 'STANDARD'::character varying |  |
| risk_rating | character varying(20) | YES |  |  |
| assigned_analyst_id | uuid | YES |  |  |
| assigned_reviewer_id | uuid | YES |  |  |
| opened_at | timestamp with time zone | NO | now() |  |
| closed_at | timestamp with time zone | YES |  |  |
| sla_deadline | timestamp with time zone | YES |  |  |
| last_activity_at | timestamp with time zone | YES | now() |  |
| case_type | character varying(30) | YES | 'NEW_CLIENT'::character varying |  |
| notes | text | YES |  |  |
| updated_at | timestamp with time zone | YES | now() |  |

### kyc.doc_request_acceptable_types

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **link_id** (PK) | uuid | NO | gen_random_uuid() |  |
| request_id | uuid | NO |  | kyc.doc_requests.request_id |
| document_type_id | uuid | NO |  | ob-poc.document_types.type_id |
| created_at | timestamp with time zone | YES | now() |  |

### kyc.doc_requests

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **request_id** (PK) | uuid | NO | gen_random_uuid() |  |
| workstream_id | uuid | NO |  | kyc.entity_workstreams.workstream_id |
| doc_type | character varying(50) | NO |  |  |
| status | character varying(20) | NO | 'REQUIRED'::character varying |  |
| required_at | timestamp with time zone | NO | now() |  |
| requested_at | timestamp with time zone | YES |  |  |
| due_date | date | YES |  |  |
| received_at | timestamp with time zone | YES |  |  |
| reviewed_at | timestamp with time zone | YES |  |  |
| verified_at | timestamp with time zone | YES |  |  |
| document_id | uuid | YES |  |  |
| reviewer_id | uuid | YES |  |  |
| rejection_reason | text | YES |  |  |
| verification_notes | text | YES |  |  |
| is_mandatory | boolean | YES | true |  |
| priority | character varying(10) | YES | 'NORMAL'::character varying |  |
| batch_id | uuid | YES |  |  |
| batch_reference | character varying(50) | YES |  |  |
| generation_source | character varying(30) | YES | 'MANUAL'::character varying |  |

### kyc.entity_workstreams

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **workstream_id** (PK) | uuid | NO | gen_random_uuid() |  |
| case_id | uuid | NO |  | kyc.cases.case_id |
| entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| status | character varying(30) | NO | 'PENDING'::character varying |  |
| discovery_source_workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| discovery_reason | character varying(100) | YES |  |  |
| risk_rating | character varying(20) | YES |  |  |
| risk_factors | jsonb | YES | '[]'::jsonb |  |
| created_at | timestamp with time zone | NO | now() |  |
| started_at | timestamp with time zone | YES |  |  |
| completed_at | timestamp with time zone | YES |  |  |
| blocked_at | timestamp with time zone | YES |  |  |
| blocked_reason | text | YES |  |  |
| requires_enhanced_dd | boolean | YES | false |  |
| is_ubo | boolean | YES | false |  |
| ownership_percentage | numeric | YES |  |  |
| discovery_depth | integer | YES | 1 |  |
| updated_at | timestamp with time zone | YES | now() |  |

### kyc.holdings

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **id** (PK) | uuid | NO | uuid_generate_v4() |  |
| share_class_id | uuid | NO |  | kyc.share_classes.id |
| investor_entity_id | uuid | NO |  | ob-poc.entities.entity_id |
| units | numeric | NO | 0 |  |
| cost_basis | numeric | YES |  |  |
| acquisition_date | date | YES |  |  |
| status | character varying(50) | NO | 'active'::character varying |  |
| created_at | timestamp with time zone | NO | now() |  |
| updated_at | timestamp with time zone | NO | now() |  |

### kyc.movements

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **id** (PK) | uuid | NO | uuid_generate_v4() |  |
| holding_id | uuid | NO |  | kyc.holdings.id |
| movement_type | character varying(50) | NO |  |  |
| units | numeric | NO |  |  |
| price_per_unit | numeric | YES |  |  |
| amount | numeric | YES |  |  |
| currency | character(3) | NO | 'EUR'::bpchar |  |
| trade_date | date | NO |  |  |
| settlement_date | date | YES |  |  |
| status | character varying(50) | NO | 'pending'::character varying |  |
| reference | character varying(100) | YES |  |  |
| notes | text | YES |  |  |
| created_at | timestamp with time zone | NO | now() |  |
| updated_at | timestamp with time zone | NO | now() |  |

### kyc.red_flags

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **red_flag_id** (PK) | uuid | NO | gen_random_uuid() |  |
| case_id | uuid | NO |  | kyc.cases.case_id |
| workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| flag_type | character varying(50) | NO |  |  |
| severity | character varying(20) | NO |  |  |
| status | character varying(20) | NO | 'OPEN'::character varying |  |
| description | text | NO |  |  |
| source | character varying(50) | YES |  |  |
| source_reference | text | YES |  |  |
| raised_at | timestamp with time zone | NO | now() |  |
| raised_by | uuid | YES |  |  |
| reviewed_at | timestamp with time zone | YES |  |  |
| reviewed_by | uuid | YES |  |  |
| resolved_at | timestamp with time zone | YES |  |  |
| resolved_by | uuid | YES |  |  |
| resolution_type | character varying(30) | YES |  |  |
| resolution_notes | text | YES |  |  |
| waiver_approved_by | uuid | YES |  |  |
| waiver_justification | text | YES |  |  |

### kyc.rule_executions

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **execution_id** (PK) | uuid | NO | gen_random_uuid() |  |
| case_id | uuid | NO |  | kyc.cases.case_id |
| workstream_id | uuid | YES |  | kyc.entity_workstreams.workstream_id |
| rule_name | character varying(100) | NO |  |  |
| trigger_event | character varying(50) | NO |  |  |
| condition_matched | boolean | NO |  |  |
| actions_executed | jsonb | YES | '[]'::jsonb |  |
| context_snapshot | jsonb | YES | '{}'::jsonb |  |
| executed_at | timestamp with time zone | NO | now() |  |

### kyc.screenings

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **screening_id** (PK) | uuid | NO | gen_random_uuid() |  |
| workstream_id | uuid | NO |  | kyc.entity_workstreams.workstream_id |
| screening_type | character varying(30) | NO |  |  |
| provider | character varying(50) | YES |  |  |
| status | character varying(20) | NO | 'PENDING'::character varying |  |
| requested_at | timestamp with time zone | NO | now() |  |
| completed_at | timestamp with time zone | YES |  |  |
| expires_at | timestamp with time zone | YES |  |  |
| result_summary | character varying(100) | YES |  |  |
| result_data | jsonb | YES |  |  |
| match_count | integer | YES | 0 |  |
| reviewed_by | uuid | YES |  |  |
| reviewed_at | timestamp with time zone | YES |  |  |
| review_notes | text | YES |  |  |
| red_flag_id | uuid | YES |  | kyc.red_flags.red_flag_id |

### kyc.share_classes

| Column | Type | Nullable | Default | FK |
|--------|------|----------|---------|-----|
| **id** (PK) | uuid | NO | uuid_generate_v4() |  |
| cbu_id | uuid | NO |  | ob-poc.cbus.cbu_id |
| name | character varying(255) | NO |  |  |
| isin | character varying(12) | YES |  |  |
| currency | character(3) | NO | 'EUR'::bpchar |  |
| nav_per_share | numeric | YES |  |  |
| nav_date | date | YES |  |  |
| management_fee_bps | integer | YES |  |  |
| performance_fee_bps | integer | YES |  |  |
| subscription_frequency | character varying(50) | YES |  |  |
| redemption_frequency | character varying(50) | YES |  |  |
| redemption_notice_days | integer | YES |  |  |
| minimum_investment | numeric | YES |  |  |
| status | character varying(50) | NO | 'active'::character varying |  |
| created_at | timestamp with time zone | NO | now() |  |
| updated_at | timestamp with time zone | NO | now() |  |
| fund_type | character varying(50) | YES |  |  |
| fund_structure | character varying(50) | YES |  |  |
| investor_eligibility | character varying(50) | YES |  |  |
| lock_up_period_months | integer | YES |  |  |
| gate_percentage | numeric | YES |  |  |
| high_water_mark | boolean | YES | false |  |
| hurdle_rate | numeric | YES |  |  |
| entity_id | uuid | YES |  | ob-poc.entities.entity_id |
| class_category | character varying(20) | YES | 'FUND'::character varying |  |

