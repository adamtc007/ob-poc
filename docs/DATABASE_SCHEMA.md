# OB-POC Database Schema Reference

**Generated:** 2025-11-29  
**Last Updated:** 2025-11-29 (Entity search indexes, attribute sources/sinks)  
**Database:** data_designer  
**Schema:** ob-poc  
**Total Tables:** 101  
**PostgreSQL Version:** 14.19

## Extensions

| Extension | Version | Purpose |
|-----------|---------|---------|
| **pgvector** | 0.8.0 | Vector embeddings for RAG |
| **pg_trgm** | - | Trigram fuzzy text search |

---

## Quick Reference: Key Tables

| Table | PK Column | PK Type | Notes |
|-------|-----------|---------|-------|
| `cbus` | `cbu_id` | uuid | Client Business Units |
| `entities` | `entity_id` | uuid | All entity types |
| `entity_types` | `entity_type_id` | uuid | Entity type definitions |
| `document_catalog` | `doc_id` | uuid | Document instances |
| `document_types` | `type_id` | uuid | **NOT** `document_type_id` |
| `attribute_registry` | `id` | text | Also has `uuid` column |
| `dictionary` | `attribute_id` | uuid | Legacy attribute dictionary |
| `roles` | `role_id` | uuid | Entity roles |
| `kyc_investigations` | `investigation_id` | uuid | KYC investigations |
| `kyc_decisions` | `decision_id` | uuid | KYC decisions |
| `screenings` | `screening_id` | uuid | Screening records |
| `csg_validation_rules` | `rule_id` | uuid | CSG validation rules |
| `csg_rule_overrides` | `override_id` | uuid | Per-CBU rule overrides |
| `csg_semantic_similarity_cache` | `cache_id` | uuid | Similarity cache |

---

## Table Categories

### 1. CBU (Client Business Unit) - 4 tables

#### cbus
Primary table for Client Business Units.
```
cbu_id              uuid PK DEFAULT gen_random_uuid()
name                varchar(255) NOT NULL
description         text
nature_purpose      text
source_of_funds     text
client_type         varchar(100)
jurisdiction        varchar(50)
created_at          timestamptz
updated_at          timestamptz
```

#### cbu_entity_roles
Links CBUs to entities with roles.
```
cbu_entity_role_id  uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
entity_id           uuid NOT NULL FK -> entities
role_id             uuid NOT NULL FK -> roles
created_at          timestamptz
```

#### cbu_creation_log
Audit trail for CBU creation.
```
log_id              uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL
nature_purpose      text
source_of_funds     text
ai_instruction      text
generated_dsl       text
created_at          timestamptz
```

#### roles
Business roles for entity-CBU relationships.
```
role_id             uuid PK DEFAULT gen_random_uuid()
name                varchar(255) NOT NULL
description         text
created_at          timestamptz
updated_at          timestamptz
```

---

### 2. Entities - 10 tables

#### entities
Core entity table (all entity types reference this).
```
entity_id           uuid PK DEFAULT gen_random_uuid()
entity_type_id      uuid NOT NULL FK -> entity_types
external_id         varchar(255)
name                varchar(255) NOT NULL
created_at          timestamptz
updated_at          timestamptz
```

#### entity_types
Defines entity type categories.
```
entity_type_id      uuid PK DEFAULT gen_random_uuid()
name                varchar(255) NOT NULL
description         text
table_name          varchar(255) NOT NULL
created_at          timestamptz
updated_at          timestamptz
```

#### entity_proper_persons
Individual persons (13 columns).
```
proper_person_id    uuid PK DEFAULT gen_random_uuid()
first_name          varchar(255) NOT NULL
last_name           varchar(255) NOT NULL
middle_names        varchar(255)
date_of_birth       date
nationality         varchar(100)
residence_address   text
id_document_type    varchar(100)
id_document_number  varchar(100)
search_name         text GENERATED ALWAYS AS (first_name || ' ' || last_name) STORED  -- For trigram search
created_at          timestamptz
updated_at          timestamptz
-- Plus additional columns
```

#### entity_limited_companies
Limited company entities.
```
limited_company_id  uuid PK DEFAULT gen_random_uuid()
company_name        varchar(255) NOT NULL
registration_number varchar(100)
jurisdiction        varchar(100)
incorporation_date  date
registered_address  text
business_nature     text
created_at          timestamptz
updated_at          timestamptz
```

#### entity_partnerships
Partnership entities.
```
partnership_id      uuid PK DEFAULT gen_random_uuid()
partnership_name    varchar(255) NOT NULL
partnership_type    varchar(100)
jurisdiction        varchar(100)
formation_date      date
principal_place_business text
partnership_agreement_date date
created_at          timestamptz
updated_at          timestamptz
```

#### entity_trusts
Trust entities.
```
trust_id            uuid PK DEFAULT gen_random_uuid()
trust_name          varchar(255) NOT NULL
trust_type          varchar(100)
jurisdiction        varchar(100) NOT NULL
establishment_date  date
trust_deed_date     date
trust_purpose       text
governing_law       varchar(100)
created_at          timestamptz
updated_at          timestamptz
```

#### ubo_registry
Ultimate Beneficial Ownership registry.
```
ubo_id              uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
subject_entity_id   uuid NOT NULL FK -> entities
ubo_proper_person_id uuid NOT NULL FK -> entity_proper_persons
relationship_type   varchar(100) NOT NULL
qualifying_reason   varchar(100) NOT NULL
ownership_percentage numeric(5,2)
control_type        varchar(100)
workflow_type       varchar(100) NOT NULL
regulatory_framework varchar(100)
verification_status varchar(50) DEFAULT 'PENDING'
screening_result    varchar(50) DEFAULT 'PENDING'
risk_rating         varchar(50)
identified_at       timestamptz
verified_at         timestamptz
created_at          timestamptz
updated_at          timestamptz
```

---

### 3. Documents - 9 tables

#### document_catalog
Central document instance table.
```
doc_id              uuid PK DEFAULT gen_random_uuid()
document_id         uuid DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
document_type_id    uuid FK -> document_types
document_type_code  varchar(100)
document_name       varchar(255)
source_system       varchar(100)
status              varchar(50) DEFAULT 'active'
metadata            jsonb DEFAULT '{}'
file_hash_sha256    text
storage_key         text
file_size_bytes     bigint
mime_type           varchar(100)
extracted_data      jsonb
extraction_status   varchar(50) DEFAULT 'PENDING'
extraction_confidence numeric(5,4)
last_extracted_at   timestamptz
created_at          timestamptz
updated_at          timestamptz
```

#### document_types
Document type definitions. **PK is `type_id`, NOT `document_type_id`**.
```
type_id             uuid PK DEFAULT gen_random_uuid()
type_code           varchar(100) NOT NULL UNIQUE
display_name        varchar(200) NOT NULL
category            varchar(100) NOT NULL
domain              varchar(100)
description         text
required_attributes jsonb DEFAULT '{}'
parent_type_code    varchar(100) FK -> document_types(type_code)  -- Hierarchy: PASSPORT_GBR.parent = PASSPORT
created_at          timestamptz
updated_at          timestamptz
```

#### document_attribute_mappings
Maps document types to extractable attributes.
```
mapping_id          uuid PK DEFAULT gen_random_uuid()
document_type_id    uuid NOT NULL FK -> document_types(type_id)
attribute_uuid      uuid NOT NULL FK -> attribute_registry(uuid)
extraction_method   varchar(50) NOT NULL  -- OCR, MRZ, BARCODE, QR_CODE, FORM_FIELD, TABLE, CHECKBOX, SIGNATURE, PHOTO, NLP, AI
field_location      jsonb
field_name          varchar(255)
confidence_threshold numeric(3,2) DEFAULT 0.80
is_required         boolean DEFAULT false
validation_pattern  text
created_at          timestamptz
updated_at          timestamptz
```

#### document_entity_links
Links documents to entities.
```
link_id             uuid PK DEFAULT gen_random_uuid()
doc_id              uuid NOT NULL FK -> document_catalog
entity_id           uuid NOT NULL FK -> entities
link_type           varchar(50) DEFAULT 'EVIDENCE'  -- EVIDENCE, IDENTITY, ADDRESS, FINANCIAL, REGULATORY, OTHER
linked_by           varchar(255) DEFAULT 'system'
created_at          timestamptz
```

#### document_requests
Document request tracking.
```
request_id          uuid PK DEFAULT gen_random_uuid()
investigation_id    uuid NOT NULL FK -> kyc_investigations
document_type       varchar(100) NOT NULL
requested_from_entity_type varchar(50)
requested_from_entity_id uuid
status              varchar(50) DEFAULT 'PENDING'
requested_at        timestamptz
received_at         timestamptz
doc_id              uuid FK -> document_catalog
notes               text
```

#### document_metadata
EAV table for document attributes.
```
doc_id              uuid NOT NULL FK -> document_catalog (composite PK)
attribute_id        uuid NOT NULL FK -> dictionary (composite PK)
value               jsonb NOT NULL
extraction_confidence numeric(3,2)
extraction_method   varchar(50)
extracted_at        timestamptz
extraction_metadata jsonb
created_at          timestamptz
```

#### document_verifications
Document verification records.
```
verification_id     uuid PK DEFAULT gen_random_uuid()
doc_id              uuid NOT NULL FK -> document_catalog
verification_method varchar(100) NOT NULL
verification_status varchar(50) DEFAULT 'PENDING'
verified_by         varchar(255)
verified_at         timestamptz
confidence_score    numeric(5,4)
issues_found        jsonb DEFAULT '[]'
created_at          timestamptz
```

#### document_relationships
Document-to-document relationships.
```
relationship_id     uuid PK DEFAULT gen_random_uuid()
primary_doc_id      uuid NOT NULL FK -> document_catalog
related_doc_id      uuid NOT NULL FK -> document_catalog
relationship_type   varchar(100) NOT NULL
created_at          timestamptz
```

---

### 4. Attributes - 6 tables

#### attribute_registry
Type-safe attribute registry with dual-key pattern.
```
id                  text PK  -- e.g., 'attr.identity.first_name'
uuid                uuid NOT NULL UNIQUE  -- For FK relationships
display_name        text NOT NULL
category            text NOT NULL  -- identity, financial, compliance, document, risk, contact, address, tax, employment, product, entity, ubo, isda
value_type          text NOT NULL  -- string, integer, number, boolean, date, datetime, email, phone, address, currency, percentage, tax_id, json
validation_rules    jsonb DEFAULT '{}'
metadata            jsonb DEFAULT '{}'
created_at          timestamptz
updated_at          timestamptz
```

#### attribute_values
Attribute values linked to CBU and DSL version.
```
av_id               uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
dsl_ob_id           uuid
dsl_version         integer NOT NULL
attribute_id        uuid NOT NULL FK -> dictionary
value               jsonb NOT NULL
state               text NOT NULL DEFAULT 'resolved'
source              jsonb
observed_at         timestamptz
```

#### attribute_values_typed
Type-safe attribute storage with temporal validity.
```
id                  serial PK
entity_id           uuid NOT NULL
attribute_id        text NOT NULL FK -> attribute_registry(id)
attribute_uuid      uuid FK -> attribute_registry(uuid)
value_text          text
value_number        numeric
value_integer       bigint
value_boolean       boolean
value_date          date
value_datetime      timestamptz
value_json          jsonb
effective_from      timestamptz
effective_to        timestamptz
source              jsonb
created_at          timestamptz
created_by          text DEFAULT 'system'
-- CHECK: exactly one value column must be populated
```

#### dictionary
Legacy universal attribute dictionary.
```
attribute_id        uuid PK DEFAULT gen_random_uuid()
name                varchar(255) NOT NULL
long_description    text
group_id            varchar(100) NOT NULL DEFAULT 'default'
mask                varchar(50) DEFAULT 'string'
domain              varchar(100)
vector              text
source              jsonb
sink                jsonb
created_at          timestamptz
updated_at          timestamptz
```

#### attribute_sources
Sparse matrix: defines where each attribute CAN come from.
```
source_id           uuid PK DEFAULT gen_random_uuid()
attribute_id        uuid NOT NULL FK -> dictionary
source_type         varchar(50) NOT NULL  -- document, api, manual, derived, user_input, registry_lookup
source_config       jsonb  -- Contains document_category, field_hints, validation rules
priority            integer DEFAULT 5  -- Lower = preferred source
confidence_weight   numeric(3,2) DEFAULT 1.0
is_authoritative    boolean DEFAULT false
created_at          timestamptz
```

#### attribute_sinks
Defines where attribute values GO (KYC reports, screening, filings).
```
sink_id             uuid PK DEFAULT gen_random_uuid()
attribute_id        uuid NOT NULL FK -> dictionary
sink_type           varchar(50) NOT NULL  -- kyc_report, sanctions_screening, pep_screening, regulatory_filing, account_opening
sink_config         jsonb  -- Contains section, field, match rules, etc.
is_required         boolean DEFAULT false
transform_rule      jsonb
created_at          timestamptz
```

---

### 5. KYC & Investigations - 4 tables

#### kyc_investigations
KYC investigation records.
```
investigation_id    uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
investigation_type  varchar(50) NOT NULL
risk_rating         varchar(20)
regulatory_framework jsonb
ubo_threshold       numeric(5,2) DEFAULT 10.0
investigation_depth integer DEFAULT 5
status              varchar(50) DEFAULT 'INITIATED'
deadline            date
outcome             varchar(50)
notes               text
created_at          timestamptz
updated_at          timestamptz
completed_at        timestamptz
```

#### kyc_decisions
KYC decision records.
```
decision_id         uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
investigation_id    uuid FK -> kyc_investigations
decision            varchar(50) NOT NULL  -- ACCEPT, REJECT, CONDITIONAL_ACCEPTANCE, ESCALATE
decision_authority  varchar(100)
rationale           text
decided_by          varchar(255)
decided_at          timestamptz
effective_date      date
review_date         date
```

#### decision_conditions
Conditions attached to decisions.
```
condition_id        uuid PK DEFAULT gen_random_uuid()
decision_id         uuid NOT NULL FK -> kyc_decisions
condition_type      varchar(50) NOT NULL
description         text
frequency           varchar(50)
due_date            date
threshold           numeric(20,2)
currency            varchar(3)
assigned_to         varchar(255)
status              varchar(50) DEFAULT 'PENDING'
satisfied_by        varchar(255)
satisfied_at        timestamptz
satisfaction_evidence text
created_at          timestamptz
```

#### investigation_assignments
Investigation team assignments.
```
assignment_id       uuid PK DEFAULT gen_random_uuid()
investigation_id    uuid NOT NULL FK -> kyc_investigations
assigned_to         varchar(255) NOT NULL
assigned_at         timestamptz
role                varchar(50)
```

---

### 6. Screening - 6 tables

#### screenings
Screening execution records.
```
screening_id        uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
entity_id           uuid FK -> entities
screening_type      varchar(50) NOT NULL  -- PEP, SANCTIONS, ADVERSE_MEDIA, WATCHLIST
provider            varchar(100)
request_payload     jsonb
response_payload    jsonb
match_count         integer DEFAULT 0
highest_match_score numeric(5,2)
status              varchar(50) DEFAULT 'PENDING'
result              varchar(50)
screened_at         timestamptz
expires_at          timestamptz
notes               text
created_at          timestamptz
updated_at          timestamptz
-- Plus additional columns
```

#### screening_batches
Batch screening operations.
```
batch_id            uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
batch_type          varchar(50) NOT NULL
entity_count        integer
screening_types     text[]
status              varchar(50) DEFAULT 'PENDING'
initiated_by        varchar(255)
initiated_at        timestamptz
completed_at        timestamptz
total_hits          integer
high_risk_count     integer
notes               text
created_at          timestamptz
updated_at          timestamptz
```

#### screening_hit_resolutions
Resolution of screening hits.
```
resolution_id       uuid PK DEFAULT gen_random_uuid()
screening_id        uuid NOT NULL FK -> screenings
hit_reference       varchar(255)
match_score         numeric(5,2)
match_type          varchar(50)
matched_data        jsonb
resolution          varchar(50) NOT NULL  -- TRUE_POSITIVE, FALSE_POSITIVE, ESCALATE
resolution_rationale text
resolved_by         varchar(255)
resolved_at         timestamptz
reviewed_by         varchar(255)
reviewed_at         timestamptz
created_at          timestamptz
updated_at          timestamptz
```

#### screening_lists
Screening list definitions.
```
list_id             uuid PK DEFAULT gen_random_uuid()
list_code           varchar(100) NOT NULL
list_name           varchar(255) NOT NULL
list_type           varchar(50) NOT NULL
provider            varchar(100)
last_updated        timestamptz
is_active           boolean DEFAULT true
created_at          timestamptz
```

---

### 7. Monitoring - 7 tables

#### monitoring_cases
Ongoing monitoring cases.
```
case_id             uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
case_type           varchar(50) NOT NULL
trigger_event       varchar(100)
priority            varchar(20)
status              varchar(50) DEFAULT 'OPEN'
assigned_to         varchar(255)
opened_at           timestamptz
closed_at           timestamptz
created_at          timestamptz
updated_at          timestamptz
```

#### monitoring_reviews
Periodic review records.
```
review_id           uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
review_type         varchar(50) NOT NULL
due_date            date
completed_date      date
reviewer            varchar(255)
outcome             varchar(50)
risk_rating_before  varchar(20)
risk_rating_after   varchar(20)
findings            text
recommendations     text
next_review_date    date
status              varchar(50) DEFAULT 'PENDING'
-- Plus additional columns (19 total)
```

#### monitoring_events
Monitoring event log.
```
event_id            uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
entity_id           uuid FK -> entities
event_type          varchar(100) NOT NULL
event_source        varchar(100)
event_data          jsonb
severity            varchar(20)
processed           boolean DEFAULT false
processed_at        timestamptz
created_at          timestamptz
updated_at          timestamptz
```

#### monitoring_alert_rules
Alert rule definitions.
```
rule_id             uuid PK DEFAULT gen_random_uuid()
rule_name           varchar(255) NOT NULL
rule_type           varchar(50) NOT NULL
trigger_conditions  jsonb NOT NULL
actions             jsonb NOT NULL
severity            varchar(20)
is_active           boolean DEFAULT true
applies_to_cbu_types text[]
applies_to_risk_levels text[]
cooldown_hours      integer
last_triggered      timestamptz
created_at          timestamptz
updated_at          timestamptz
```

#### monitoring_activities
Monitoring activity log.
```
activity_id         uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
activity_type       varchar(100) NOT NULL
description         text
performed_by        varchar(255)
performed_at        timestamptz
related_case_id     uuid FK -> monitoring_cases
related_review_id   uuid FK -> monitoring_reviews
metadata            jsonb
created_at          timestamptz
```

---

### 8. Risk - 4 tables

#### risk_assessments
Risk assessment records.
```
assessment_id       uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
assessment_type     varchar(50) NOT NULL
methodology         varchar(100)
inherent_risk       varchar(20)
residual_risk       varchar(20)
control_effectiveness varchar(20)
assessor            varchar(255)
assessed_at         timestamptz
valid_until         date
created_at          timestamptz
```

#### risk_ratings
Current risk ratings.
```
rating_id           uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
rating_type         varchar(50) NOT NULL
current_rating      varchar(20) NOT NULL
rating_score        numeric(5,2)
factors             jsonb
rated_by            varchar(255)
rated_at            timestamptz
effective_from      date
created_at          timestamptz
```

#### risk_rating_changes
Risk rating change history.
```
change_id           uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid NOT NULL FK -> cbus
rating_type         varchar(50) NOT NULL
previous_rating     varchar(20)
new_rating          varchar(20) NOT NULL
change_reason       text
changed_by          varchar(255)
changed_at          timestamptz
approved_by         varchar(255)
approved_at         timestamptz
created_at          timestamptz
```

#### risk_flags
Active risk flags.
```
flag_id             uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
entity_id           uuid FK -> entities
flag_type           varchar(100) NOT NULL
flag_reason         text
severity            varchar(20)
raised_by           varchar(255)
raised_at           timestamptz
resolved_by         varchar(255)
resolved_at         timestamptz
resolution_notes    text
created_at          timestamptz
```

---

### 9. DSL Management - 8 tables

#### dsl_instances
DSL instance registry.
```
instance_id         uuid PK DEFAULT gen_random_uuid()
domain_name         varchar(100) NOT NULL
business_reference  varchar(255) NOT NULL
case_id             varchar(255)
entity_id           uuid
current_version     integer DEFAULT 1
status              varchar(50) DEFAULT 'CREATED'
created_at          timestamptz
updated_at          timestamptz
metadata            jsonb
-- Plus additional columns (13 total)
UNIQUE(domain_name, business_reference)
```

#### dsl_instance_versions
Version history for DSL instances.
```
version_id          uuid PK DEFAULT gen_random_uuid()
instance_id         uuid NOT NULL FK -> dsl_instances
version_number      integer NOT NULL
dsl_content         text NOT NULL
operation_type      varchar(50) NOT NULL
compilation_status  varchar(50) DEFAULT 'PENDING'
ast_json            jsonb
created_at          timestamptz
UNIQUE(instance_id, version_number)
```

#### dsl_domains
Domain definitions.
```
domain_id           uuid PK DEFAULT gen_random_uuid()
domain_name         varchar(100) NOT NULL UNIQUE
description         text
base_grammar_version varchar(20) DEFAULT '1.0.0'
vocabulary_version  varchar(20) DEFAULT '1.0.0'
active              boolean DEFAULT true
created_at          timestamptz
updated_at          timestamptz
```

#### dsl_ob
DSL objects per CBU.
```
version_id          uuid PK DEFAULT gen_random_uuid()
cbu_id              varchar(255) NOT NULL
dsl_text            text NOT NULL
created_at          timestamptz
```

#### dsl_versions
DSL version control.
```
version_id          uuid PK DEFAULT gen_random_uuid()
domain_id           uuid FK -> dsl_domains
version_number      integer NOT NULL
dsl_source_code     text NOT NULL
description         text
author              varchar(255)
status              varchar(50) DEFAULT 'DRAFT'
published_at        timestamptz
deprecated_at       timestamptz
created_at          timestamptz
updated_at          timestamptz
parent_version_id   uuid
```

#### parsed_asts
Compiled AST storage.
```
ast_id              uuid PK DEFAULT gen_random_uuid()
version_id          uuid FK -> dsl_instance_versions
case_id             varchar(255)
dsl_text            text
ast_json            jsonb
word_count          integer
complexity_score    numeric(5,2)
parsed_at           timestamptz
invalidated_at      timestamptz
created_at          timestamptz
updated_at          timestamptz
```

#### dsl_execution_log
DSL execution history.
```
execution_id        uuid PK DEFAULT gen_random_uuid()
version_id          uuid FK -> dsl_instance_versions
case_id             varchar(255)
execution_status    varchar(50)
execution_start     timestamptz
execution_end       timestamptz
error_message       text
context_snapshot    jsonb
results             jsonb
executed_by         varchar(255)
created_at          timestamptz
updated_at          timestamptz
```

#### dsl_examples
Example DSL snippets for AI training.
```
example_id          uuid PK DEFAULT gen_random_uuid()
title               varchar(255) NOT NULL
description         text
operation_type      varchar(20) NOT NULL  -- CREATE, READ, UPDATE, DELETE
asset_type          varchar(50) NOT NULL
entity_table_name   varchar(100)
natural_language_input text NOT NULL
example_dsl         text NOT NULL
expected_outcome    text
tags                text[]
complexity_level    varchar(20) DEFAULT 'MEDIUM'
success_rate        numeric(3,2) DEFAULT 1.0
usage_count         integer DEFAULT 0
last_used_at        timestamptz
created_at          timestamptz
updated_at          timestamptz
created_by          varchar(255) DEFAULT 'system'
```

---

### 10. Vocabulary & Grammar - 4 tables

#### domain_vocabularies
Domain verb definitions.
```
vocab_id            uuid PK DEFAULT gen_random_uuid()
domain              varchar(100) NOT NULL
verb                varchar(100) NOT NULL
category            varchar(50)
description         text
parameters          jsonb
examples            jsonb
phase               varchar(20)
active              boolean DEFAULT true
version             varchar(20) DEFAULT '1.0.0'
created_at          timestamptz
updated_at          timestamptz
UNIQUE(domain, verb)
```

#### verb_registry
Verb registry for DSL.
```
verb_id             uuid PK DEFAULT gen_random_uuid()
domain              varchar(100) NOT NULL
verb_name           varchar(100) NOT NULL
description         text
parameters_schema   jsonb
return_type         varchar(50)
created_at          timestamptz
updated_at          timestamptz
UNIQUE(domain, verb_name)
```

#### grammar_rules
EBNF grammar rules.
```
rule_id             uuid PK DEFAULT gen_random_uuid()
rule_name           varchar(100) NOT NULL
rule_definition     text NOT NULL
rule_type           varchar(50) DEFAULT 'production'
domain              varchar(100)
version             varchar(20) DEFAULT '1.0.0'
active              boolean DEFAULT true
description         text
created_at          timestamptz
updated_at          timestamptz
```

#### vocabulary_audit
Vocabulary change tracking.
```
audit_id            uuid PK DEFAULT gen_random_uuid()
vocab_id            uuid FK -> domain_vocabularies
change_type         varchar(20) NOT NULL
old_value           jsonb
new_value           jsonb
changed_by          varchar(255)
changed_at          timestamptz
reason              text
created_at          timestamptz
```

---

### 11. Products & Services - 10 tables

#### products
Product catalog.
```
product_id          uuid PK DEFAULT gen_random_uuid()
name                varchar(255) NOT NULL
code                varchar(50)
description         text
category            varchar(100)
status              varchar(50) DEFAULT 'ACTIVE'
launch_date         date
sunset_date         date
created_at          timestamptz
updated_at          timestamptz
metadata            jsonb
```

#### services
Service catalog.
```
service_id          uuid PK DEFAULT gen_random_uuid()
name                varchar(255) NOT NULL
code                varchar(50)
description         text
category            varchar(100)
status              varchar(50) DEFAULT 'ACTIVE'
created_at          timestamptz
updated_at          timestamptz
metadata            jsonb
```

#### product_services
Product-service mappings.
```
product_id          uuid NOT NULL FK -> products (composite PK)
service_id          uuid NOT NULL FK -> services (composite PK)
is_mandatory        boolean DEFAULT false
configuration       jsonb
created_at          timestamptz
updated_at          timestamptz
```

#### prod_resources
Production resources.
```
resource_id         uuid PK DEFAULT gen_random_uuid()
name                varchar(255) NOT NULL
description         text
owner               varchar(255) NOT NULL
-- Plus additional columns (19 total)
```

#### service_option_definitions
Service option definitions.
```
option_id           uuid PK DEFAULT gen_random_uuid()
service_id          uuid NOT NULL FK -> services
option_name         varchar(255) NOT NULL
option_type         varchar(50) NOT NULL
default_value       text
allowed_values      jsonb
is_required         boolean DEFAULT false
created_at          timestamptz
updated_at          timestamptz
```

---

### 12. Onboarding - 5 tables

#### onboarding_requests
Onboarding workflow state.
```
request_id          uuid PK DEFAULT gen_random_uuid()
cbu_id              uuid FK -> cbus
entity_type         varchar(100)
entity_name         varchar(255)
jurisdiction        varchar(50)
products            text[]
status              varchar(50) DEFAULT 'INITIATED'
initiated_by        varchar(255)
initiated_at        timestamptz
completed_at        timestamptz
created_at          timestamptz
updated_at          timestamptz
```

#### onboarding_products
Product selections for onboarding.
```
id                  uuid PK DEFAULT gen_random_uuid()
request_id          uuid NOT NULL FK -> onboarding_requests
product_id          uuid NOT NULL FK -> products
status              varchar(50) DEFAULT 'PENDING'
created_at          timestamptz
```

---

### 13. Orchestration - 4 tables

#### orchestration_sessions
Multi-domain orchestration sessions.
```
session_id          uuid PK DEFAULT gen_random_uuid()
primary_domain      varchar(100) NOT NULL
cbu_id              uuid FK -> cbus
entity_type         varchar(50)
entity_name         text
jurisdiction        varchar(10)
products            text[]
services            text[]
workflow_type       varchar(50) DEFAULT 'ONBOARDING'
current_state       varchar(50) DEFAULT 'CREATED'
version_number      integer DEFAULT 0
unified_dsl         text
shared_context      jsonb
execution_plan      jsonb
entity_refs         jsonb
attribute_refs      jsonb
created_at          timestamptz
updated_at          timestamptz
last_used           timestamptz
expires_at          timestamptz
```

#### orchestration_domain_sessions
Domain-specific execution within orchestration.
```
id                  uuid PK DEFAULT gen_random_uuid()
orchestration_session_id uuid NOT NULL FK -> orchestration_sessions
domain_name         varchar(100) NOT NULL
domain_session_id   uuid NOT NULL
state               varchar(50) DEFAULT 'CREATED'
contributed_dsl     text
domain_context      jsonb
dependencies        text[]
last_activity       timestamptz
created_at          timestamptz
```

#### orchestration_tasks
Task queue for orchestration.
```
task_id             uuid PK DEFAULT gen_random_uuid()
orchestration_session_id uuid NOT NULL FK -> orchestration_sessions
domain_name         varchar(100) NOT NULL
verb                varchar(200) NOT NULL
parameters          jsonb
dependencies        text[]
status              varchar(50) DEFAULT 'PENDING'
generated_dsl       text
error_message       text
scheduled_at        timestamptz
started_at          timestamptz
completed_at        timestamptz
created_at          timestamptz
```

#### orchestration_state_history
State transition history.
```
id                  uuid PK DEFAULT gen_random_uuid()
orchestration_session_id uuid NOT NULL FK -> orchestration_sessions
from_state          varchar(50)
to_state            varchar(50) NOT NULL
domain_name         varchar(100)
reason              text
generated_by        varchar(100)
version_number      integer
metadata            jsonb
created_at          timestamptz
```

---

### 14. Reference Data - 2 tables

#### master_jurisdictions
Jurisdiction reference data.
```
jurisdiction_id     uuid PK DEFAULT gen_random_uuid()
code                varchar(10) NOT NULL UNIQUE
name                varchar(255) NOT NULL
region              varchar(100)
regulatory_body     varchar(255)
risk_level          varchar(20)
is_fatf_member      boolean
is_eu_member        boolean
timezone            varchar(50)
created_at          timestamptz
updated_at          timestamptz
```

#### master_entity_xref
External ID cross-reference.
```
xref_id             uuid PK DEFAULT gen_random_uuid()
entity_id           uuid NOT NULL FK -> entities
external_system     varchar(100) NOT NULL
external_id         varchar(255) NOT NULL
external_type       varchar(100)
sync_status         varchar(50) DEFAULT 'ACTIVE'
last_synced_at      timestamptz
created_at          timestamptz
updated_at          timestamptz
-- Plus additional columns (12 total)
UNIQUE(external_system, external_id)
```

---

### 15. Audit & Operations - 5 tables

#### crud_operations
CRUD operation audit log.
```
operation_id        uuid PK DEFAULT gen_random_uuid()
operation_type      varchar(20) NOT NULL  -- CREATE, READ, UPDATE, DELETE
asset_type          varchar(50) NOT NULL  -- CBU, ENTITY, DOCUMENT, etc.
entity_table_name   varchar(100)
generated_dsl       text NOT NULL
ai_instruction      text NOT NULL
affected_records    jsonb NOT NULL DEFAULT '[]'
execution_status    varchar(20) NOT NULL DEFAULT 'PENDING'
ai_confidence       numeric(3,2)  -- 0.0 to 1.0
ai_provider         varchar(50)
ai_model            varchar(100)
execution_time_ms   integer
error_message       text
created_by          varchar(255) DEFAULT 'agentic_system'
created_at          timestamptz
completed_at        timestamptz
rows_affected       integer DEFAULT 0
transaction_id      uuid
parent_operation_id uuid
```

#### rag_embeddings
Vector embeddings for RAG.
```
embedding_id        uuid PK DEFAULT gen_random_uuid()
source_type         varchar(50) NOT NULL
source_id           uuid NOT NULL
chunk_index         integer
chunk_text          text NOT NULL
embedding           vector(1536)  -- pgvector
model               varchar(100)
metadata            jsonb
created_at          timestamptz
updated_at          timestamptz
-- Plus additional columns (12 total)
```

#### schema_changes
Schema change log.
```
change_id           uuid PK DEFAULT gen_random_uuid()
table_name          varchar(100) NOT NULL
change_type         varchar(50) NOT NULL
change_description  text
applied_at          timestamptz
applied_by          varchar(255)
```

#### taxonomy_crud_log / taxonomy_audit_log
Taxonomy operation logging.

---

## Views

The schema includes several views for convenience:

| View | Purpose |
|------|---------|
| `active_investigations` | Active (non-complete) investigations with CBU info |
| `blocking_conditions` | Pending conditions with urgency status |
| `decisions` | Bridge view mapping kyc_decisions to DECISION crud_asset |
| `attribute_uuid_map` | Maps semantic IDs to UUIDs in attribute_registry |
| `entity_search_view` | Unified cross-type entity search with trigram support |

#### entity_search_view
Unified view for cross-entity-type fuzzy search.
```sql
-- Returns: id, entity_type, display_name, subtitle_1, subtitle_2, search_text
-- Entity types: PERSON, COMPANY, CBU, TRUST
-- Usage: WHERE search_text % 'query' ORDER BY similarity(search_text, 'query') DESC
```

---

## Indexes

### Trigram Indexes (pg_trgm)
For fuzzy text search and typeahead/autocomplete.

| Index | Table | Column | Type |
|-------|-------|--------|------|
| `idx_persons_search_name_trgm` | entity_proper_persons | search_name | GIN |
| `idx_persons_first_name_trgm` | entity_proper_persons | first_name | GIN |
| `idx_persons_last_name_trgm` | entity_proper_persons | last_name | GIN |
| `idx_companies_name_trgm` | entity_limited_companies | company_name | GIN |
| `idx_companies_reg_number` | entity_limited_companies | registration_number | B-tree |
| `idx_cbu_name_trgm` | cbus | name | GIN |
| `idx_trusts_name_trgm` | entity_trusts | trust_name | GIN |

---

## Functions

Key database functions:

| Function | Purpose |
|----------|---------|
| `get_attribute_value(entity_id, attribute_id)` | Get typed attribute value |
| `set_attribute_value(...)` | Set attribute value with temporal tracking |
| `resolve_semantic_to_uuid(text)` | Convert semantic ID to UUID |
| `resolve_uuid_to_semantic(uuid)` | Convert UUID to semantic ID |
| `get_next_version_number(domain_name)` | Get next DSL version number |
| `invalidate_ast_cache()` | Trigger to invalidate AST on DSL change |
| `refresh_document_type_similarities()` | Refresh CSG similarity cache for document types |

---

### 16. CSG (Context-Sensitive Grammar) Linter - 3 tables

The CSG Linter validates DSL programs against business rules that depend on runtime context.

#### csg_validation_rules
Centralized rule store for CSG validation.
```
rule_id             uuid PK DEFAULT gen_random_uuid()
rule_code           varchar(100) NOT NULL UNIQUE
rule_name           varchar(255) NOT NULL
rule_version        integer DEFAULT 1
target_type         varchar(50) NOT NULL  -- document_type, attribute, entity_type, verb, cross_reference
target_code         varchar(100)
rule_type           varchar(50) NOT NULL  -- entity_type_constraint, jurisdiction_constraint, client_type_constraint, prerequisite, exclusion, co_occurrence, sequence, cardinality, custom
rule_params         jsonb NOT NULL
error_code          varchar(10) NOT NULL  -- e.g., "C001"
error_message_template text NOT NULL
suggestion_template text
severity            varchar(20) DEFAULT 'error'  -- error, warning, info
description         text
rationale           text
documentation_url   text
is_active           boolean DEFAULT true
effective_from      timestamptz
effective_until     timestamptz
created_by          varchar(255)
created_at          timestamptz
updated_at          timestamptz
```

#### csg_rule_overrides
Per-CBU rule overrides for custom behavior.
```
override_id         uuid PK DEFAULT gen_random_uuid()
rule_id             uuid NOT NULL FK -> csg_validation_rules
cbu_id              uuid NOT NULL FK -> cbus
override_type       varchar(50) NOT NULL  -- disable, downgrade, modify_params, add_exception
override_params     jsonb
approved_by         varchar(255)
approval_reason     text NOT NULL
approved_at         timestamptz
expires_at          timestamptz
created_by          varchar(255)
created_at          timestamptz
UNIQUE(rule_id, cbu_id)
```

#### csg_semantic_similarity_cache
Pre-computed similarity scores for fast suggestions.
```
cache_id            uuid PK DEFAULT gen_random_uuid()
source_type         varchar(50) NOT NULL  -- document_type, attribute, entity_type
source_code         varchar(100) NOT NULL
target_type         varchar(50) NOT NULL
target_code         varchar(100) NOT NULL
cosine_similarity   float NOT NULL
levenshtein_distance integer
semantic_relatedness float
relationship_type   varchar(50)  -- alternative, complement, parent, child
computed_at         timestamptz
expires_at          timestamptz
UNIQUE(source_type, source_code, target_type, target_code)
```

---

### CSG Metadata Columns (added to existing tables)

The following columns were added to support CSG linting:

#### document_types (additional columns)
```
applicability       jsonb DEFAULT '{}'  -- CSG rules: entity_types[], jurisdictions[], client_types[], required_for[], excludes[]
semantic_context    jsonb DEFAULT '{}'  -- Rich metadata: purpose, synonyms[], keywords[]
embedding           vector(1536)        -- Vector embedding for similarity search
embedding_model     varchar(100)
embedding_updated_at timestamptz
```

#### attribute_registry (additional columns)
```
applicability       jsonb DEFAULT '{}'  -- CSG rules: entity_types[], required_for[], source_documents[], depends_on[]
embedding           vector(1536)
embedding_model     varchar(100)
embedding_updated_at timestamptz
```

#### entity_types (additional columns)
```
type_code           varchar(100) UNIQUE -- Normalized identifier (e.g., "LIMITED_COMPANY_PRIVATE")
semantic_context    jsonb DEFAULT '{}'  -- Rich metadata: category, synonyms[], typical_documents[]
parent_type_id      uuid FK -> entity_types  -- Type hierarchy
type_hierarchy_path text[]              -- Materialized path, e.g., ["ENTITY", "LEGAL_ENTITY", "LIMITED_COMPANY"]
embedding           vector(1536)
embedding_model     varchar(100)
embedding_updated_at timestamptz
```

#### cbus (additional columns)
```
risk_context        jsonb DEFAULT '{}'  -- Risk metadata: risk_rating, pep_exposure, sanctions_exposure
onboarding_context  jsonb DEFAULT '{}'  -- State: stage, completed_steps[], pending_requirements[]
semantic_context    jsonb DEFAULT '{}'  -- Rich metadata: business_description, industry_keywords[]
embedding           vector(1536)
embedding_model     varchar(100)
embedding_updated_at timestamptz
```

---

## Important Notes

1. **document_types.type_id** - The PK is `type_id`, not `document_type_id`
2. **attribute_registry dual-key** - Has both `id` (text PK) and `uuid` (unique) columns
3. **Temporal attributes** - `attribute_values_typed` uses `effective_from`/`effective_to` for temporal validity
4. **Entity types** - All entities link to `entity_types` via `entity_type_id`, specific data in type-specific tables
5. **pgvector enabled** - Use `vector(1536)` type for embeddings in `rag_embeddings`
6. **CSG applicability rules** - `document_types.applicability` and `attribute_registry.applicability` contain JSONB with `entity_types[]` arrays for context-sensitive validation
7. **Entity type hierarchy** - `entity_types.type_hierarchy_path` contains materialized path arrays for wildcard matching (e.g., `PROPER_PERSON_*` matches `PROPER_PERSON_NATURAL`)
