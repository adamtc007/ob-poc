# OB-POC Database Schema Reference

**Generated:** 2025-11-29
**Database:** data_designer
**Schema:** ob-poc
**Total Tables:** 100
**Total Views:** 12

## Extensions

| Extension | Version | Purpose |
|-----------|---------|---------|
| **pg_trgm** | 1.6 | Trigram fuzzy text search |
| **uuid-ossp** | 1.1 | UUID generation functions |
| **vector** | 0.8.0 | Vector embeddings for RAG |

---

## Quick Reference: Key Tables

| Table | PK Column | PK Type | Notes |
|-------|-----------|---------|-------|
| `cbus` | `cbu_id` | uuid | Client Business Units |
| `entities` | `entity_id` | uuid | All entity types |
| `entity_types` | `entity_type_id` | uuid | Entity type definitions |
| `document_catalog` | `doc_id` | uuid | Document instances |
| `document_types` | `type_id` | uuid | **NOT** document_type_id |
| `attribute_registry` | `id` | text | Also has uuid column |
| `roles` | `role_id` | uuid | Entity roles |
| `kyc_investigations` | `investigation_id` | uuid | KYC investigations |
| `screenings` | `screening_id` | uuid | Screening records |
| `csg_validation_rules` | `rule_id` | uuid | CSG validation rules |
| `dsl_generation_log` | `log_id` | uuid | Agent generation audit trail |

---

### CBU (Client Business Unit) - 3 tables

#### cbus
```
cbu_id                    uuid PK NOT NULL DEFAULT gen_random_uuid()
name                      varchar NOT NULL
description               text
nature_purpose            text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
source_of_funds           text
client_type               varchar
jurisdiction              varchar
risk_context              jsonb DEFAULT {}
onboarding_context        jsonb DEFAULT {}
semantic_context          jsonb DEFAULT {}
embedding                 vector
embedding_model           varchar
embedding_updated_at      timestamptz
```

#### cbu_entity_roles
```
cbu_entity_role_id        uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
entity_id                 uuid NOT NULL FK -> entities(entity_id)
role_id                   uuid NOT NULL FK -> roles(role_id)
created_at                timestamptz DEFAULT NOW()
```

#### cbu_creation_log
```
log_id                    uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
nature_purpose            text
source_of_funds           text
ai_instruction            text
generated_dsl             text
created_at                timestamptz DEFAULT NOW()
```

### Entities - 13 tables

#### entities
```
entity_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
entity_type_id            uuid NOT NULL FK -> entity_types(entity_type_id)
external_id               varchar
name                      varchar NOT NULL
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### entity_types
```
entity_type_id            uuid PK NOT NULL DEFAULT gen_random_uuid()
name                      varchar NOT NULL
description               text
table_name                varchar NOT NULL
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
type_code                 varchar
semantic_context          jsonb DEFAULT {}
parent_type_id            uuid FK -> entity_types(entity_type_id)
type_hierarchy_path       _text
embedding                 vector
embedding_model           varchar
embedding_updated_at      timestamptz
```

#### entity_proper_persons
```
proper_person_id          uuid PK NOT NULL DEFAULT gen_random_uuid()
first_name                varchar NOT NULL
last_name                 varchar NOT NULL
middle_names              varchar
date_of_birth             date
nationality               varchar
residence_address         text
id_document_type          varchar
id_document_number        varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
search_name               text
entity_id                 uuid FK -> entities(entity_id)
```

#### entity_limited_companies
```
limited_company_id        uuid PK NOT NULL DEFAULT gen_random_uuid()
company_name              varchar NOT NULL
registration_number       varchar
jurisdiction              varchar
incorporation_date        date
registered_address        text
business_nature           text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
entity_id                 uuid FK -> entities(entity_id)
```

#### entity_partnerships
```
partnership_id            uuid PK NOT NULL DEFAULT gen_random_uuid()
partnership_name          varchar NOT NULL
partnership_type          varchar
jurisdiction              varchar
formation_date            date
principal_place_business  text
partnership_agreement_date date
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
entity_id                 uuid FK -> entities(entity_id)
```

#### entity_trusts
```
trust_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
trust_name                varchar NOT NULL
trust_type                varchar
jurisdiction              varchar NOT NULL
establishment_date        date
trust_deed_date           date
trust_purpose             text
governing_law             varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
entity_id                 uuid FK -> entities(entity_id)
```

#### entity_lifecycle_status
```
status_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
entity_type               varchar NOT NULL
entity_id                 uuid NOT NULL
status_code               varchar NOT NULL
status_description        varchar
effective_date            date NOT NULL
end_date                  date
reason_code               varchar
notes                     text
created_by                varchar DEFAULT system
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### entity_crud_rules
```
rule_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
entity_table_name         varchar NOT NULL
operation_type            varchar NOT NULL
field_name                varchar
constraint_type           varchar NOT NULL
constraint_description    text NOT NULL
validation_pattern        varchar
error_message             text
is_active                 bool
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### entity_validation_rules
```
rule_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
entity_type               varchar NOT NULL
field_name                varchar NOT NULL
validation_type           varchar NOT NULL
validation_rule           jsonb NOT NULL
error_message             varchar
severity                  varchar DEFAULT ERROR
is_active                 bool
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### entity_role_connections
```
connection_id             uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
entity_id                 uuid NOT NULL FK -> entities(entity_id)
role_id                   uuid NOT NULL
connection_type           varchar NOT NULL
created_at                timestamptz DEFAULT NOW()
```

#### entity_product_mappings
```
entity_type               varchar NOT NULL
product_id                uuid NOT NULL FK -> products(product_id)
compatible                bool NOT NULL
restrictions              jsonb
required_fields           jsonb
created_at                timestamptz DEFAULT NOW()
```

#### ownership_relationships
```
ownership_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
owner_entity_id           uuid NOT NULL FK -> entities(entity_id)
owned_entity_id           uuid NOT NULL FK -> entities(entity_id)
ownership_type            varchar NOT NULL
ownership_percent         numeric NOT NULL
effective_from            date
effective_to              date
evidence_doc_id           uuid FK -> document_catalog(doc_id)
notes                     text
created_by                varchar DEFAULT system
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### ubo_registry
```
ubo_id                    uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
subject_entity_id         uuid NOT NULL FK -> entities(entity_id)
ubo_proper_person_id      uuid NOT NULL FK -> entities(entity_id)
relationship_type         varchar NOT NULL
qualifying_reason         varchar NOT NULL
ownership_percentage      numeric
control_type              varchar
workflow_type             varchar NOT NULL
regulatory_framework      varchar
verification_status       varchar DEFAULT PENDING
screening_result          varchar DEFAULT PENDING
risk_rating               varchar
identified_at             timestamptz DEFAULT NOW()
verified_at               timestamptz
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

### Documents - 9 tables

#### document_catalog
```
doc_id                    uuid PK NOT NULL DEFAULT gen_random_uuid()
file_hash_sha256          text
storage_key               text
file_size_bytes           int8
mime_type                 varchar
extracted_data            jsonb
extraction_status         varchar DEFAULT PENDING
extraction_confidence     numeric
last_extracted_at         timestamptz
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
cbu_id                    uuid FK -> cbus(cbu_id)
document_type_id          uuid FK -> document_types(type_id)
document_id               uuid DEFAULT gen_random_uuid()
document_type_code        varchar
document_name             varchar
source_system             varchar
status                    varchar DEFAULT active
metadata                  jsonb DEFAULT {}
```

#### document_types
```
type_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
type_code                 varchar NOT NULL
display_name              varchar NOT NULL
category                  varchar NOT NULL
domain                    varchar
description               text
required_attributes       jsonb DEFAULT {}
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
applicability             jsonb DEFAULT {}
semantic_context          jsonb DEFAULT {}
embedding                 vector
embedding_model           varchar
embedding_updated_at      timestamptz
```

#### document_attribute_mappings
```
mapping_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
document_type_id          uuid NOT NULL FK -> document_types(type_id)
attribute_uuid            uuid NOT NULL FK -> attribute_registry(uuid)
extraction_method         varchar NOT NULL
field_location            jsonb
field_name                varchar
confidence_threshold      numeric
is_required               bool
validation_pattern        text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### document_entity_links
```
link_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
doc_id                    uuid NOT NULL FK -> document_catalog(doc_id)
entity_id                 uuid NOT NULL FK -> entities(entity_id)
link_type                 varchar DEFAULT EVIDENCE
linked_by                 varchar DEFAULT system
created_at                timestamptz DEFAULT NOW()
```

#### document_metadata
```
doc_id                    uuid NOT NULL FK -> document_catalog(doc_id)
attribute_id              uuid NOT NULL FK -> dictionary(attribute_id)
value                     jsonb NOT NULL
created_at                timestamptz DEFAULT NOW()
extraction_confidence     numeric
extraction_method         varchar
extracted_at              timestamptz
extraction_metadata       jsonb
```

#### document_relationships
```
relationship_id           uuid PK NOT NULL DEFAULT gen_random_uuid()
primary_doc_id            uuid NOT NULL FK -> document_catalog(doc_id)
related_doc_id            uuid NOT NULL FK -> document_catalog(doc_id)
relationship_type         varchar NOT NULL
created_at                timestamptz DEFAULT NOW()
```

#### document_requests
```
request_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
investigation_id          uuid NOT NULL FK -> kyc_investigations(investigation_id)
document_type             varchar NOT NULL
requested_from_entity_type varchar
requested_from_entity_id  uuid
status                    varchar DEFAULT PENDING
requested_at              timestamptz DEFAULT NOW()
received_at               timestamptz
doc_id                    uuid
notes                     text
```

#### document_verifications
```
verification_id           uuid PK NOT NULL DEFAULT gen_random_uuid()
doc_id                    uuid NOT NULL
verification_method       varchar NOT NULL
verification_status       varchar DEFAULT PENDING
verified_by               varchar
verified_at               timestamptz
confidence_score          numeric
issues_found              jsonb DEFAULT []
created_at                timestamptz DEFAULT NOW()
```

#### document_issuers_backup
```
issuer_id                 uuid
issuer_code               varchar
legal_name                varchar
jurisdiction              varchar
regulatory_type           varchar
official_website          varchar
verification_endpoint     varchar
trust_level               varchar
created_at                timestamptz
updated_at                timestamptz
backup_created_at         timestamptz
```

### Attributes - 5 tables

#### attribute_registry
```
id                        text PK NOT NULL
display_name              text NOT NULL
category                  text NOT NULL
value_type                text NOT NULL
validation_rules          jsonb DEFAULT {}
metadata                  jsonb DEFAULT {}
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
uuid                      uuid NOT NULL
applicability             jsonb DEFAULT {}
embedding                 vector
embedding_model           varchar
embedding_updated_at      timestamptz
```

#### attribute_dictionary
```
attribute_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
attr_id                   varchar NOT NULL
attr_name                 varchar NOT NULL
domain                    varchar NOT NULL
data_type                 varchar NOT NULL DEFAULT STRING
description               text
validation_pattern        varchar
is_required               bool
is_active                 bool
created_at                timestamptz DEFAULT NOW()
```

#### attribute_values
```
av_id                     uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
dsl_ob_id                 uuid FK -> dsl_ob(version_id)
dsl_version               int4 NOT NULL
attribute_id              uuid NOT NULL FK -> dictionary(attribute_id)
value                     jsonb NOT NULL
state                     text NOT NULL DEFAULT resolved
source                    jsonb
observed_at               timestamptz DEFAULT NOW()
```

#### attribute_values_typed
```
id                        int4 PK NOT NULL SERIAL
entity_id                 uuid NOT NULL
attribute_id              text NOT NULL FK -> attribute_registry(id)
value_text                text
value_number              numeric
value_integer             int8
value_boolean             bool
value_date                date
value_datetime            timestamptz
value_json                jsonb
effective_from            timestamptz DEFAULT NOW()
effective_to              timestamptz
source                    jsonb
created_at                timestamptz DEFAULT NOW()
created_by                text DEFAULT system
attribute_uuid            uuid FK -> attribute_registry(uuid)
```

#### dictionary
```
attribute_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
name                      varchar NOT NULL
long_description          text
group_id                  varchar NOT NULL DEFAULT default
mask                      varchar DEFAULT string
domain                    varchar
vector                    text
source                    jsonb
sink                      jsonb
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

### KYC & Investigations - 4 tables

#### kyc_investigations
```
investigation_id          uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid FK -> cbus(cbu_id)
investigation_type        varchar NOT NULL
risk_rating               varchar
regulatory_framework      jsonb
ubo_threshold             numeric
investigation_depth       int4
status                    varchar DEFAULT INITIATED
deadline                  date
outcome                   varchar
notes                     text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
completed_at              timestamptz
```

#### kyc_decisions
```
decision_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
investigation_id          uuid FK -> kyc_investigations(investigation_id)
decision                  varchar NOT NULL
decision_authority        varchar
rationale                 text
decided_by                varchar
decided_at                timestamptz DEFAULT NOW()
effective_date            date
review_date               date
```

#### decision_conditions
```
condition_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
decision_id               uuid NOT NULL FK -> kyc_decisions(decision_id)
condition_type            varchar NOT NULL
description               text
frequency                 varchar
due_date                  date
threshold                 numeric
currency                  varchar
assigned_to               varchar
status                    varchar DEFAULT PENDING
satisfied_by              varchar
satisfied_at              timestamptz
satisfaction_evidence     text
created_at                timestamptz DEFAULT NOW()
```

#### investigation_assignments
```
assignment_id             uuid PK NOT NULL DEFAULT gen_random_uuid()
investigation_id          uuid NOT NULL FK -> kyc_investigations(investigation_id)
assignee                  varchar NOT NULL
role                      varchar
assigned_at               timestamptz DEFAULT NOW()
```

### Screening - 5 tables

#### screenings
```
screening_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
investigation_id          uuid FK -> kyc_investigations(investigation_id)
entity_id                 uuid NOT NULL FK -> entities(entity_id)
screening_type            varchar NOT NULL
databases                 jsonb
lists                     jsonb
include_rca               bool
search_depth              varchar
languages                 jsonb
status                    varchar DEFAULT PENDING
result                    varchar
match_details             jsonb
resolution                varchar
resolution_rationale      text
screened_at               timestamptz DEFAULT NOW()
reviewed_by               varchar
resolved_by               varchar
resolved_at               timestamptz
```

#### screening_batches
```
batch_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid FK -> cbus(cbu_id)
investigation_id          uuid FK -> kyc_investigations(investigation_id)
screen_types              jsonb NOT NULL DEFAULT ["PEP", "SANCTIONS"]
entity_count              int4
completed_count           int4
hit_count                 int4
status                    varchar DEFAULT PENDING
match_threshold           numeric
started_at                timestamptz
completed_at              timestamptz
error_message             text
created_by                varchar DEFAULT system
created_at                timestamptz DEFAULT NOW()
```

#### screening_batch_results
```
batch_id                  uuid NOT NULL FK -> screening_batches(batch_id)
screening_id              uuid NOT NULL FK -> screenings(screening_id)
```

#### screening_hit_resolutions
```
resolution_id             uuid PK NOT NULL DEFAULT gen_random_uuid()
screening_id              uuid NOT NULL FK -> screenings(screening_id)
hit_reference             varchar
ubo_id                    uuid FK -> ubo_registry(ubo_id)
resolution                varchar NOT NULL
dismiss_reason            varchar
rationale                 text NOT NULL
evidence_refs             jsonb DEFAULT []
notes                     text
resolved_by               varchar NOT NULL
resolved_at               timestamptz DEFAULT NOW()
reviewed_by               varchar
reviewed_at               timestamptz
created_at                timestamptz DEFAULT NOW()
```

#### screening_lists
```
screening_list_id         uuid PK NOT NULL DEFAULT gen_random_uuid()
list_code                 varchar NOT NULL
list_name                 varchar NOT NULL
list_type                 varchar NOT NULL
provider                  varchar
description               text
is_active                 bool
created_at                timestamptz DEFAULT NOW()
```

### Monitoring - 7 tables

#### monitoring_cases
```
case_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
case_type                 varchar NOT NULL
status                    varchar DEFAULT OPEN
close_reason              varchar
close_notes               text
retention_period_years    int4
closed_at                 timestamptz
closed_by                 varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### monitoring_reviews
```
review_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
case_id                   uuid NOT NULL FK -> monitoring_cases(case_id)
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
review_type               varchar NOT NULL
trigger_type              varchar
trigger_reference_id      varchar
due_date                  date NOT NULL
risk_based_frequency      varchar
scope                     jsonb DEFAULT ["FULL"]
status                    varchar DEFAULT SCHEDULED
outcome                   varchar
findings                  text
next_review_date          date
actions                   jsonb DEFAULT []
started_at                timestamptz
completed_at              timestamptz
completed_by              varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### monitoring_events
```
event_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
event_type                varchar NOT NULL
description               text
severity                  varchar
requires_review           bool
reviewed_by               varchar
reviewed_at               timestamptz
review_outcome            varchar
review_notes              text
created_at                timestamptz DEFAULT NOW()
```

#### monitoring_alert_rules
```
rule_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
case_id                   uuid FK -> monitoring_cases(case_id)
rule_type                 varchar NOT NULL
rule_name                 varchar NOT NULL
description               text
threshold                 jsonb NOT NULL
is_active                 bool
last_triggered_at         timestamptz
trigger_count             int4
created_by                varchar DEFAULT system
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### monitoring_activities
```
activity_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
case_id                   uuid NOT NULL FK -> monitoring_cases(case_id)
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
activity_type             varchar NOT NULL
description               text NOT NULL
reference_id              varchar
reference_type            varchar
recorded_by               varchar NOT NULL
recorded_at               timestamptz DEFAULT NOW()
created_at                timestamptz DEFAULT NOW()
```

#### monitoring_setup
```
setup_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
monitoring_level          varchar NOT NULL
components                jsonb
active                    bool
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### scheduled_reviews
```
review_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
review_type               varchar NOT NULL
due_date                  date NOT NULL
assigned_to               varchar
status                    varchar DEFAULT SCHEDULED
completed_by              varchar
completed_at              timestamptz
completion_notes          text
next_review_id            uuid
created_at                timestamptz DEFAULT NOW()
```

### Risk - 4 tables

#### risk_assessments
```
assessment_id             uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid FK -> cbus(cbu_id)
entity_id                 uuid FK -> entities(entity_id)
investigation_id          uuid FK -> kyc_investigations(investigation_id)
assessment_type           varchar NOT NULL
rating                    varchar
factors                   jsonb
methodology               varchar
rationale                 text
assessed_by               varchar
assessed_at               timestamptz DEFAULT NOW()
```

#### risk_ratings
```
rating_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
rating                    varchar NOT NULL
previous_rating           varchar
rationale                 text
assessment_id             uuid FK -> risk_assessments(assessment_id)
effective_from            timestamptz DEFAULT NOW()
effective_to              timestamptz
set_by                    varchar DEFAULT system
created_at                timestamptz DEFAULT NOW()
```

#### risk_rating_changes
```
change_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
case_id                   uuid FK -> monitoring_cases(case_id)
review_id                 uuid FK -> monitoring_reviews(review_id)
previous_rating           varchar
new_rating                varchar NOT NULL
change_reason             varchar NOT NULL
rationale                 text NOT NULL
effective_at              timestamptz DEFAULT NOW()
changed_by                varchar NOT NULL
created_at                timestamptz DEFAULT NOW()
```

#### risk_flags
```
flag_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid FK -> cbus(cbu_id)
entity_id                 uuid FK -> entities(entity_id)
investigation_id          uuid FK -> kyc_investigations(investigation_id)
flag_type                 varchar NOT NULL
description               text
status                    varchar DEFAULT ACTIVE
flagged_by                varchar
flagged_at                timestamptz DEFAULT NOW()
resolved_by               varchar
resolved_at               timestamptz
resolution_notes          text
```

### DSL Management - 9 tables

#### dsl_instances
```
id                        int4 PK NOT NULL SERIAL
case_id                   varchar
dsl_content               text
domain                    varchar
operation_type            varchar
status                    varchar DEFAULT PROCESSED
processing_time_ms        int8
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
instance_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
domain_name               varchar
business_reference        varchar NOT NULL
current_version           int4
```

#### dsl_instance_versions
```
version_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
instance_id               uuid NOT NULL FK -> dsl_instances(instance_id)
version_number            int4 NOT NULL
dsl_content               text NOT NULL
operation_type            varchar NOT NULL
compilation_status        varchar DEFAULT COMPILED
ast_json                  jsonb
created_at                timestamptz DEFAULT NOW()
```

#### dsl_domains
```
domain_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
domain_name               varchar NOT NULL
description               text
base_grammar_version      varchar DEFAULT 1.0.0
vocabulary_version        varchar DEFAULT 1.0.0
active                    bool
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### dsl_ob
```
version_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
dsl_text                  text NOT NULL
created_at                timestamptz DEFAULT NOW()
```

#### dsl_versions
```
version_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
domain_id                 uuid NOT NULL FK -> dsl_domains(domain_id)
version_number            int4 NOT NULL
functional_state          varchar
dsl_source_code           text NOT NULL
compilation_status        varchar DEFAULT DRAFT
change_description        text
parent_version_id         uuid FK -> dsl_versions(version_id)
created_by                varchar
created_at                timestamptz DEFAULT NOW()
compiled_at               timestamptz
activated_at              timestamptz
```

#### dsl_examples
```
example_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
title                     varchar NOT NULL
description               text
operation_type            varchar NOT NULL
asset_type                varchar NOT NULL
entity_table_name         varchar
natural_language_input    text NOT NULL
example_dsl               text NOT NULL
expected_outcome          text
tags                      _text DEFAULT ARRAY[]
complexity_level          varchar DEFAULT MEDIUM
success_rate              numeric
usage_count               int4
last_used_at              timestamptz
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
created_by                varchar DEFAULT system
```

#### dsl_execution_log
```
execution_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
version_id                uuid NOT NULL FK -> dsl_versions(version_id)
cbu_id                    varchar
execution_phase           varchar NOT NULL
status                    varchar NOT NULL
result_data               jsonb
error_details             jsonb
performance_metrics       jsonb
executed_by               varchar
started_at                timestamptz DEFAULT NOW()
completed_at              timestamptz
duration_ms               int4
```

#### dsl_generation_log
Captures agent DSL generation iterations for training data extraction and audit trail.
```
log_id                    uuid PK NOT NULL DEFAULT gen_random_uuid()
instance_id               uuid FK -> dsl_instances(instance_id)
user_intent               text NOT NULL           -- Natural language input (training pair input)
final_valid_dsl           text                    -- Successfully validated DSL (training pair output)
iterations                jsonb NOT NULL DEFAULT '[]'  -- Array of generation attempts
domain_name               varchar(50) NOT NULL    -- Primary domain: cbu, entity, document
session_id                uuid                    -- Link to agent session
cbu_id                    uuid                    -- Target CBU if applicable
model_used                varchar(100)            -- LLM model identifier
total_attempts            int4 NOT NULL DEFAULT 1
success                   bool NOT NULL DEFAULT false
total_latency_ms          int4                    -- Sum of all attempt latencies
total_input_tokens        int4
total_output_tokens       int4
created_at                timestamptz DEFAULT NOW()
completed_at              timestamptz
```
**Iterations JSONB Structure:**
```json
[{
  "attempt": 1,
  "timestamp": "2025-01-15T10:30:00Z",
  "prompt_template": "cbu_create_v2",
  "prompt_text": "Given the vocabulary...",
  "raw_response": "I'll create a CBU...",
  "extracted_dsl": "(cbu.create :name ...)",
  "parse_result": {"success": true, "error": null},
  "lint_result": {"valid": false, "errors": ["Unknown verb"], "warnings": []},
  "compile_result": {"success": false, "error": "Unknown verb", "step_count": 0},
  "latency_ms": 1500,
  "input_tokens": 500,
  "output_tokens": 200
}]
```
**Indexes:** success (partial), domain_name, created_at DESC, instance_id (partial), session_id (partial), iterations (GIN), user_intent (GIN trigram)

#### parsed_asts
```
ast_id                    uuid PK NOT NULL DEFAULT gen_random_uuid()
version_id                uuid NOT NULL FK -> dsl_versions(version_id)
ast_json                  jsonb NOT NULL
parse_metadata            jsonb
grammar_version           varchar NOT NULL
parser_version            varchar NOT NULL
ast_hash                  varchar
node_count                int4
complexity_score          numeric
parsed_at                 timestamptz DEFAULT NOW()
invalidated_at            timestamptz
```

### Vocabulary & Grammar - 4 tables

#### domain_vocabularies
```
vocab_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
domain                    varchar NOT NULL
verb                      varchar NOT NULL
category                  varchar
description               text
parameters                jsonb
examples                  jsonb
phase                     varchar
active                    bool
version                   varchar DEFAULT 1.0.0
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### verb_registry
```
verb                      varchar NOT NULL
primary_domain            varchar NOT NULL
shared                    bool
deprecated                bool
replacement_verb          varchar
description               text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### grammar_rules
```
rule_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
rule_name                 varchar NOT NULL
rule_definition           text NOT NULL
rule_type                 varchar NOT NULL DEFAULT production
domain                    varchar
version                   varchar DEFAULT 1.0.0
active                    bool
description               text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### vocabulary_audit
```
audit_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
domain                    varchar NOT NULL
verb                      varchar NOT NULL
change_type               varchar NOT NULL
old_definition            jsonb
new_definition            jsonb
changed_by                varchar
change_reason             text
created_at                timestamptz DEFAULT NOW()
```

### Products & Services - 12 tables

#### products
```
product_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
name                      varchar NOT NULL
description               text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
product_code              varchar
product_category          varchar
regulatory_framework      varchar
min_asset_requirement     numeric
is_active                 bool
metadata                  jsonb
```

#### services
```
service_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
name                      varchar NOT NULL
description               text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
service_code              varchar
service_category          varchar
sla_definition            jsonb
is_active                 bool
```

#### product_services
```
product_id                uuid NOT NULL FK -> products(product_id)
service_id                uuid NOT NULL FK -> services(service_id)
is_mandatory              bool
is_default                bool
display_order             int4
configuration             jsonb
```

#### product_requirements
```
product_id                uuid NOT NULL FK -> products(product_id)
entity_types              jsonb NOT NULL
required_dsl              jsonb NOT NULL
attributes                jsonb NOT NULL
compliance                jsonb NOT NULL
prerequisites             jsonb NOT NULL
conditional_rules         jsonb NOT NULL
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### product_workflows
```
workflow_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
product_id                uuid NOT NULL FK -> products(product_id)
entity_type               varchar NOT NULL
required_dsl              jsonb NOT NULL
generated_dsl             text NOT NULL
compliance_rules          jsonb NOT NULL
status                    varchar NOT NULL DEFAULT PENDING
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### service_option_definitions
```
option_def_id             uuid PK NOT NULL DEFAULT gen_random_uuid()
service_id                uuid NOT NULL FK -> services(service_id)
option_key                varchar NOT NULL
option_label              varchar
option_type               varchar NOT NULL
validation_rules          jsonb
is_required               bool
display_order             int4
help_text                 text
```

#### service_option_choices
```
choice_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
option_def_id             uuid NOT NULL FK -> service_option_definitions(option_def_id)
choice_value              varchar NOT NULL
choice_label              varchar
choice_metadata           jsonb
is_default                bool
is_active                 bool
display_order             int4
requires_options          jsonb
excludes_options          jsonb
```

#### service_resources
```
service_id                uuid NOT NULL FK -> services(service_id)
resource_id               uuid NOT NULL FK -> prod_resources(resource_id)
```

#### service_resource_capabilities
```
capability_id             uuid PK NOT NULL DEFAULT gen_random_uuid()
service_id                uuid NOT NULL FK -> services(service_id)
resource_id               uuid NOT NULL FK -> prod_resources(resource_id)
supported_options         jsonb NOT NULL
priority                  int4
cost_factor               numeric
performance_rating        int4
resource_config           jsonb
is_active                 bool
```

#### service_discovery_cache
```
discovery_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
product_id                uuid FK -> products(product_id)
discovered_at             timestamptz DEFAULT NOW()
services_available        jsonb
resource_availability     jsonb
ttl_seconds               int4
```

#### prod_resources
```
resource_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
name                      varchar NOT NULL
description               text
owner                     varchar NOT NULL
dictionary_group          varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
resource_code             varchar
resource_type             varchar
vendor                    varchar
version                   varchar
api_endpoint              text
api_version               varchar
authentication_method     varchar
authentication_config     jsonb
capabilities              jsonb
capacity_limits           jsonb
maintenance_windows       jsonb
is_active                 bool
```

#### resource_attribute_requirements
```
requirement_id            uuid PK NOT NULL DEFAULT gen_random_uuid()
resource_id               uuid NOT NULL FK -> prod_resources(resource_id)
attribute_id              uuid NOT NULL FK -> dictionary(attribute_id)
resource_field_name       varchar
is_mandatory              bool
transformation_rule       jsonb
validation_override       jsonb
```

### Onboarding - 4 tables

#### onboarding_requests
```
request_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
request_state             varchar NOT NULL DEFAULT draft
dsl_draft                 text
dsl_version               int4
current_phase             varchar
phase_metadata            jsonb
validation_errors         jsonb
created_by                varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
completed_at              timestamptz
```

#### onboarding_products
```
onboarding_product_id     uuid PK NOT NULL DEFAULT gen_random_uuid()
request_id                uuid NOT NULL FK -> onboarding_requests(request_id)
product_id                uuid NOT NULL FK -> products(product_id)
selection_order           int4
selected_at               timestamptz DEFAULT NOW()
```

#### onboarding_resource_allocations
```
allocation_id             uuid PK NOT NULL DEFAULT gen_random_uuid()
request_id                uuid NOT NULL FK -> onboarding_requests(request_id)
service_id                uuid NOT NULL FK -> services(service_id)
resource_id               uuid NOT NULL FK -> prod_resources(resource_id)
handles_options           jsonb
required_attributes       _uuid
allocation_status         varchar DEFAULT pending
allocated_at              timestamptz DEFAULT NOW()
```

#### onboarding_service_configs
```
config_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
request_id                uuid NOT NULL FK -> onboarding_requests(request_id)
service_id                uuid NOT NULL FK -> services(service_id)
option_selections         jsonb NOT NULL
is_valid                  bool
validation_messages       jsonb
configured_at             timestamptz DEFAULT NOW()
```

### Orchestration - 4 tables

#### orchestration_sessions
```
session_id                uuid PK NOT NULL DEFAULT gen_random_uuid()
primary_domain            varchar NOT NULL
cbu_id                    uuid FK -> cbus(cbu_id)
entity_type               varchar
entity_name               text
jurisdiction              varchar
products                  _text
services                  _text
workflow_type             varchar DEFAULT ONBOARDING
current_state             varchar DEFAULT CREATED
version_number            int4
unified_dsl               text
shared_context            jsonb
execution_plan            jsonb
entity_refs               jsonb
attribute_refs            jsonb
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
last_used                 timestamptz DEFAULT NOW()
expires_at                timestamptz DEFAULT NOW()
```

#### orchestration_domain_sessions
```
id                        uuid PK NOT NULL DEFAULT gen_random_uuid()
orchestration_session_id  uuid NOT NULL FK -> orchestration_sessions(session_id)
domain_name               varchar NOT NULL
domain_session_id         uuid NOT NULL
state                     varchar DEFAULT CREATED
contributed_dsl           text
domain_context            jsonb
dependencies              _text
last_activity             timestamptz DEFAULT NOW()
created_at                timestamptz DEFAULT NOW()
```

#### orchestration_tasks
```
task_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
orchestration_session_id  uuid NOT NULL FK -> orchestration_sessions(session_id)
domain_name               varchar NOT NULL
verb                      varchar NOT NULL
parameters                jsonb
dependencies              _text
status                    varchar DEFAULT PENDING
generated_dsl             text
error_message             text
scheduled_at              timestamptz DEFAULT NOW()
started_at                timestamptz
completed_at              timestamptz
created_at                timestamptz DEFAULT NOW()
```

#### orchestration_state_history
```
id                        uuid PK NOT NULL DEFAULT gen_random_uuid()
orchestration_session_id  uuid NOT NULL FK -> orchestration_sessions(session_id)
from_state                varchar
to_state                  varchar NOT NULL
domain_name               varchar
reason                    text
generated_by              varchar
version_number            int4
metadata                  jsonb
created_at                timestamptz DEFAULT NOW()
```

### Reference Data - 4 tables

#### master_jurisdictions
```
jurisdiction_code         varchar NOT NULL
jurisdiction_name         varchar NOT NULL
country_code              varchar NOT NULL
region                    varchar
regulatory_framework      varchar
entity_formation_allowed  bool
offshore_jurisdiction     bool
regulatory_authority      varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### master_entity_xref
```
xref_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
entity_type               varchar NOT NULL
entity_id                 uuid NOT NULL
entity_name               varchar NOT NULL
jurisdiction_code         varchar FK -> master_jurisdictions(jurisdiction_code)
entity_status             varchar DEFAULT ACTIVE
business_purpose          text
primary_contact_person    uuid
regulatory_numbers        jsonb DEFAULT {}
additional_metadata       jsonb DEFAULT {}
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### currencies
```
currency_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
iso_code                  varchar NOT NULL
name                      varchar NOT NULL
symbol                    varchar
decimal_places            int4
is_active                 bool
created_at                timestamptz DEFAULT NOW()
```

#### roles
```
role_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
name                      varchar NOT NULL
description               text
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

### CSG Linter - 3 tables

#### csg_validation_rules
```
rule_id                   uuid PK NOT NULL DEFAULT gen_random_uuid()
rule_code                 varchar NOT NULL
rule_name                 varchar NOT NULL
rule_version              int4
target_type               varchar NOT NULL
target_code               varchar
rule_type                 varchar NOT NULL
rule_params               jsonb NOT NULL
error_code                varchar NOT NULL
error_message_template    text NOT NULL
suggestion_template       text
severity                  varchar DEFAULT error
description               text
rationale                 text
documentation_url         text
is_active                 bool
effective_from            timestamptz DEFAULT NOW()
effective_until           timestamptz
created_by                varchar
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### csg_rule_overrides
```
override_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
rule_id                   uuid NOT NULL FK -> csg_validation_rules(rule_id)
cbu_id                    uuid NOT NULL FK -> cbus(cbu_id)
override_type             varchar NOT NULL
override_params           jsonb
approved_by               varchar
approval_reason           text NOT NULL
approved_at               timestamptz
expires_at                timestamptz
created_by                varchar
created_at                timestamptz DEFAULT NOW()
```

#### csg_semantic_similarity_cache
```
cache_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
source_type               varchar NOT NULL
source_code               varchar NOT NULL
target_type               varchar NOT NULL
target_code               varchar NOT NULL
cosine_similarity         float8 NOT NULL
levenshtein_distance      int4
semantic_relatedness      float8
relationship_type         varchar
computed_at               timestamptz DEFAULT NOW()
expires_at                timestamptz DEFAULT NOW()
```

### Trusts & Partnerships - 5 tables

#### trust_parties
```
trust_party_id            uuid PK NOT NULL DEFAULT gen_random_uuid()
trust_id                  uuid NOT NULL FK -> entity_trusts(trust_id)
entity_id                 uuid NOT NULL FK -> entities(entity_id)
party_role                varchar NOT NULL
party_type                varchar NOT NULL
appointment_date          date
resignation_date          date
is_active                 bool
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### trust_beneficiary_classes
```
beneficiary_class_id      uuid PK NOT NULL DEFAULT gen_random_uuid()
trust_id                  uuid NOT NULL FK -> entity_trusts(trust_id)
class_name                varchar NOT NULL
class_definition          text
class_type                varchar
monitoring_required       bool
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### trust_protector_powers
```
protector_power_id        uuid PK NOT NULL DEFAULT gen_random_uuid()
trust_party_id            uuid NOT NULL FK -> trust_parties(trust_party_id)
power_type                varchar NOT NULL
power_description         text
is_active                 bool
created_at                timestamptz DEFAULT NOW()
```

#### partnership_interests
```
interest_id               uuid PK NOT NULL DEFAULT gen_random_uuid()
partnership_id            uuid NOT NULL FK -> entity_partnerships(partnership_id)
entity_id                 uuid NOT NULL FK -> entities(entity_id)
partner_type              varchar NOT NULL
capital_commitment        numeric
ownership_percentage      numeric
voting_rights             numeric
profit_sharing_percentage numeric
admission_date            date
withdrawal_date           date
is_active                 bool
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### partnership_control_mechanisms
```
control_mechanism_id      uuid PK NOT NULL DEFAULT gen_random_uuid()
partnership_id            uuid NOT NULL FK -> entity_partnerships(partnership_id)
entity_id                 uuid NOT NULL FK -> entities(entity_id)
control_type              varchar NOT NULL
control_description       text
effective_date            date
termination_date          date
is_active                 bool
created_at                timestamptz DEFAULT NOW()
```

### Audit & Operations - 5 tables

#### crud_operations
```
operation_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
operation_type            varchar NOT NULL
asset_type                varchar NOT NULL
entity_table_name         varchar
generated_dsl             text NOT NULL
ai_instruction            text NOT NULL
affected_records          jsonb NOT NULL DEFAULT []
execution_status          varchar NOT NULL DEFAULT PENDING
ai_confidence             numeric
ai_provider               varchar
ai_model                  varchar
execution_time_ms         int4
error_message             text
created_by                varchar DEFAULT agentic_system
created_at                timestamptz DEFAULT NOW()
completed_at              timestamptz
rows_affected             int4
transaction_id            uuid
parent_operation_id       uuid FK -> crud_operations(operation_id)
```

#### rag_embeddings
```
embedding_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
content_type              varchar NOT NULL
content_text              text NOT NULL
embedding_data            jsonb
metadata                  jsonb NOT NULL DEFAULT {}
source_table              varchar
asset_type                varchar
relevance_score           numeric
usage_count               int4
last_used_at              timestamptz
created_at                timestamptz DEFAULT NOW()
updated_at                timestamptz DEFAULT NOW()
```

#### schema_changes
```
change_id                 uuid PK NOT NULL DEFAULT gen_random_uuid()
change_type               varchar NOT NULL
description               text NOT NULL
script_name               varchar
applied_at                timestamptz DEFAULT NOW()
applied_by                varchar
```

#### taxonomy_audit_log
```
audit_id                  uuid PK NOT NULL DEFAULT gen_random_uuid()
operation                 varchar NOT NULL
entity_type               varchar NOT NULL
entity_id                 uuid NOT NULL
user_id                   varchar NOT NULL
before_state              jsonb
after_state               jsonb
metadata                  jsonb
success                   bool NOT NULL
error_message             text
created_at                timestamptz NOT NULL DEFAULT NOW()
```

#### taxonomy_crud_log
```
operation_id              uuid PK NOT NULL DEFAULT gen_random_uuid()
operation_type            varchar NOT NULL
entity_type               varchar NOT NULL
entity_id                 uuid
natural_language_input    text
parsed_dsl                text
execution_result          jsonb
success                   bool
error_message             text
user_id                   varchar
created_at                timestamptz DEFAULT NOW()
execution_time_ms         int4
```

---

## Views

| View | Purpose |
|------|---------|
| `active_investigations` | Active (non-complete) investigations with CBU info |
| `attribute_uuid_map` | Maps semantic IDs to UUIDs |
| `blocking_conditions` | Pending conditions with urgency status |
| `decisions` | Bridge view mapping kyc_decisions |
| `dsl_execution_summary` | DSL execution statistics |
| `dsl_latest_versions` | Latest DSL versions per domain |
| `entity_search_view` | Unified cross-type entity search |
| `investigations` | Investigation summary view |
| `jurisdictions` | Jurisdiction lookup view |
| `overdue_reviews` | Reviews past due date |
| `referential_integrity_check` | FK integrity validation |
| `screening_results` | Screening results summary |

---

## Important Notes

1. **document_types.type_id** - The PK is `type_id`, not `document_type_id`
2. **attribute_registry dual-key** - Has both `id` (text PK) and `uuid` (unique) columns
3. **Vector embeddings** - Tables with `embedding` column use pgvector `vector` type
4. **CSG columns** - `applicability`, `semantic_context`, `embedding` added for context-sensitive validation
5. **Temporal attributes** - `attribute_values_typed` uses `effective_from`/`effective_to`
