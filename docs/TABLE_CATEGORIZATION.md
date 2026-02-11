# OB-POC Schema Table Categorization

**Total Tables:** 324 (237 base tables + 87 views)

---

## 1. Core Entity Model (15 tables)

Core entity types and their subtypes. Foundation of the entire domain model.

**Base Tables (15):**
- `entities` - Master entity registry
- `entity_types` - Entity type taxonomy
- `entity_type_dependencies` - Type hierarchy rules
- `entity_identifiers` - LEI, DIN, registration numbers
- `entity_names` - Name variants and translations
- `entity_addresses` - Physical and mailing addresses
- `entity_lifecycle_events` - Audit trail of changes
- `entity_funds` - Fund specialization (GLEIF fund category)
- `entity_limited_companies` - Corporate entity subtype
- `entity_partnerships` - Partnership entity subtype
- `entity_trusts` - Trust entity subtype
- `entity_manco` - Management company specialization
- `entity_proper_persons` - Natural person subtype
- `entity_cooperatives` - Cooperative entity subtype (via schema)
- `entity_governments` - Government entity subtype (via schema)

**Related Support:**
- `entity_concept_link` - Semantic concept tagging
- `entity_feature` - ML feature flags

---

## 2. CBU Aggregate (29 tables)

Client Business Unit (trading unit) and all direct child tables.

**Base Tables (29):**
- `cbus` - Master CBU registry
- `cbu_creation_log` - CBU creation audit trail
- `cbu_change_log` - All CBU mutations
- `cbu_entity_roles` - CBU membership (who plays which role)
- `cbu_entity_roles_history` - Role change history
- `cbu_group_members` - CBU group membership
- `cbu_groups` - CBU groups (for batch operations)
- `cbu_control_anchors` - Control hierarchy anchors
- `cbu_board_controller` - Board control specialization
- `cbu_attr_values` - CBU attribute assignments
- `cbu_unified_attr_requirements` - CBU attribute requirements
- `cbu_evidence` - Evidence backing CBU configuration
- `cbu_subscriptions` - CBU → contract + product binding
- `cbu_product_subscriptions` - Product subscriptions per CBU
- `cbu_trading_profiles` - Trading profile instances
- `cbu_matrix_product_overlay` - Matrix customization per CBU
- `cbu_relationship_verification` - Intra-CBU verification
- `cbu_resource_instances` - Service resource allocation
- `cbu_service_readiness` - Service readiness tracking
- `cbu_sla_commitments` - SLA assignments
- `cbu_lifecycle_instances` - Lifecycle stage tracking
- `cbu_layout_overrides` - Visualization layout customization
- `cbu_convergence_status` - Convergence tracking state
- `cbu_ownership_graph` - Ownership structure visualization
- `fund_structure` - Fund specialization
- `fund_investments` - Fund investment tracking
- `onboarding_requests` - CBU creation/onboarding requests
- `onboarding_plans` - Staged onboarding plans

---

## 3. Client Group & Ownership / UBO (28 tables)

Client groups (virtual aggregates), ownership hierarchies, UBO discovery, and control structures.

**Client Group Tables (13):**
- `client_group` - Virtual client group registry
- `client_group_alias` - Searchable aliases
- `client_group_alias_embedding` - Alias semantic embeddings
- `client_group_anchor` - Role-based anchors per jurisdiction
- `client_group_anchor_role` - Anchor role definitions (via schema)
- `client_group_entity` - Entities discovered via research
- `client_group_entity_roles` - Entity roles within group (GLEIF mapping)
- `client_group_entity_tag` - Entity tagging within group
- `client_group_entity_tag_embedding` - Tag semantic embeddings
- `client_group_relationship` - Client relationships
- `client_group_relationship_sources` - Relationship provenance

**Ownership / UBO Tables (15):**
- `entity_parent_relationships` - Corporate parent-subsidiary edges
- `entity_relationships` - General relationships (current snapshot)
- `entity_relationships_current` - View: current relationships only
- `entity_relationships_history` - Temporal relationship changes
- `entity_ubos` - Ultimate Beneficial Owner registry
- `ubo_registry` - UBO operational tracking
- `ubo_snapshots` - Point-in-time UBO snapshots
- `ubo_snapshot_comparisons` - Snapshot deltas
- `ubo_evidence` - UBO proof and documentation
- `ubo_assertion_log` - UBO claim audit trail
- `ubo_convergence_status` - Convergence tracking
- `ubo_expired_proofs` - Expired UBO documentation
- `ubo_missing_proofs` - Gaps in UBO documentation
- `control_edges` - Control relationship edges
- `board_control_evidence` - Board control proof

**BODS Integration (5):**
- `bods_entity_statements` - BODS entity statements
- `bods_person_statements` - BODS person statements
- `bods_ownership_statements` - BODS ownership statements
- `bods_entity_types` - BODS entity type mappings
- `bods_interest_types` - BODS interest type catalog
- `entity_bods_links` - Mapping ob-poc entities to BODS

---

## 4. Product / Service / Resource (46 tables)

Products, services, and service delivery infrastructure.

**Core Product/Service (7):**
- `products` - Product catalog
- `services` - Service catalog
- `product_services` - Product → service binding

**Service Resource Configuration (17):**
- `service_resource_types` - Resource type taxonomy
- `service_resource_capabilities` - Capability definitions
- `lifecycle_resource_types` - Lifecycle-specific resources
- `lifecycle_resource_capabilities` - Capability by lifecycle
- `resource_attribute_requirements` - Attribute requirements
- `resource_dependencies` - Inter-resource dependencies
- `resource_instance_attributes` - Instance attribute values
- `resource_instance_dependencies` - Runtime dependencies
- `requirement_acceptable_docs` - Document acceptability per requirement
- `service_delivery_map` - Service → resource allocation
- `service_intents` - Service intent tracking
- `service_availability` - Service availability matrix
- `srdef_discovery_reasons` - Service discovery metadata

**Lifecycle Management (11):**
- `lifecycles` - Lifecycle definitions
- `instrument_lifecycles` - Instrument lifecycle tracking
- `entity_lifecycle_events` - Entity state changes
- `provisioning_requests` - Resource provisioning requests
- `provisioning_events` - Provisioning audit trail
- `provisioning_events` - Event log

**SLA / Measurement (8):**
- `sla_templates` - SLA templates
- `sla_measurements` - SLA metric values
- `sla_metric_types` - Metric type taxonomy
- `sla_breaches` - SLA breach records
- `threshold_requirements` - Threshold definitions
- `threshold_factors` - Threshold factors
- `eligibility_evaluation` - Eligibility assessment

---

## 5. Instrument Matrix (7 tables)

Trading matrix: instruments × markets × currencies with policies and settlements.

**Base Tables (7):**
- `trading_profile_materializations` - Materialized trading policies
- `cbu_matrix_product_overlay` - Per-CBU matrix customization
- `cbu_trading_profiles` - Trading profile instances
- `cbu_attr_values` - Related attribute values
- `instrument_lifecycles` - Instrument state tracking
- `settlement_types` - Settlement method taxonomy

---

## 6. Attribute Dictionary & Evidence (24 tables)

Attributes, documents, evidence, and proof systems.

**Attribute System (7):**
- `attribute_registry` - Attribute definitions
- `attribute_observations` - Attribute measurement instances
- `attribute_values_typed` - Typed attribute values
- `cbu_attr_values` - CBU-level attribute assignments
- `cbu_unified_attr_requirements` - Attribute requirement rules
- `resource_attribute_requirements` - Service resource attributes

**Document & Proof System (17):**
- `documents` - Master document registry
- `document_versions` - Document submission versions (immutable)
- `document_types` - Document type taxonomy
- `document_catalog` - Document indexing
- `document_bundles` - Bundle specifications
- `document_events` - Document state changes
- `bundle_documents` - Bundle membership
- `document_attribute_links` - Document → attribute linkage
- `document_attribute_mappings` - Data extraction mappings
- `proofs` - Proof of assertion (evidence)
- `observation_discrepancies` - Data discrepancies
- `verification_challenges` - Verification problems
- `verification_escalations` - Escalated verification
- `cbu_evidence` - CBU evidence backing
- `ubo_evidence` - UBO proof and documentation
- `board_control_evidence` - Board control evidence

---

## 7. Legal Contracts & Onboarding Gate (10 tables)

Legal contracts, rate cards, product subscriptions, and CBU onboarding gate.

**Base Tables (10):**
- `legal_contracts` - Master contract registry
- `legal_entity` - Legal entity specialization (via schema)
- `contract_products` - Contracted products
- `contract_template` - Contract templates
- `contract_pack` - Contract packaging/bundling
- `rate_cards` - Rate card definitions
- `cbu_subscriptions` - CBU → contract + product binding (onboarding gate)
- `cbu_product_subscriptions` - Product-level subscriptions
- `v_active_rate_cards` - View: currently active rate cards
- `v_contract_summary` - View: contract metadata

---

## 8. Deal Record & Fee Billing (15 tables)

Commercial deal hub and fee billing infrastructure.

**Deal Record (10):**
- `deals` - Master deal registry
- `deal_participants` - Deal team members
- `deal_products` - Deal product scope
- `deal_contracts` - Contract linkage
- `deal_rate_cards` - Rate card negotiation
- `deal_rate_card_lines` - Fee line items
- `deal_slas` - SLA commitments
- `deal_documents` - Attached documents
- `deal_ubo_assessments` - KYC references
- `deal_onboarding_requests` - Onboarding handoff
- `deal_events` - Deal audit trail

**Fee Billing (5):**
- `fee_billing_profiles` - Billing configuration
- `fee_billing_account_targets` - CBU billing scope
- `fee_billing_periods` - Billing cycles
- `fee_billing_period_lines` - Calculated fees
- `v_billing_profile_summary` - View: billing state

---

## 9. Workflow / Task Queue (15 tables)

Workflow orchestration, task queue, staged execution, and rejection codes.

**Workflow (8):**
- `workflow_definitions` - Workflow definitions
- `workflow_instances` - Running workflow instances
- `workflow_pending_tasks` - Outbound task tracking
- `workflow_task_events` - Task audit trail
- `workflow_audit_log` - Workflow operations log
- `task_result_queue` - Inbound task completion queue
- `task_result_dlq` - Dead letter queue
- `dsl_workflow_phases` - DSL phase assignments

**Staged Runbook (4):**
- `staged_runbook` - Session-scoped runbook
- `staged_command` - Individual DSL commands
- `staged_command_entity` - Entity footprint
- `staged_command_candidate` - Picker candidates

**Support (3):**
- `rejection_reason_codes` - Document rejection codes
- `crud_operations` - CRUD audit trail
- `sheet_execution_audit` - Execution ledger

---

## 10. DSL / Verb / Session / REPL (38 tables)

DSL definitions, verb registry, session management, REPL state, and semantic matching.

**DSL Definitions & Verbs (11):**
- `dsl_verbs` - Master verb registry
- `dsl_verb_categories` - Verb classification
- `dsl_verb_sync_log` - Verb synchronization audit
- `dsl_ob` - DSL observation log
- `dsl_view_state_changes` - View state transitions
- `dsl_generation_log` - DSL generation audit
- `dsl_idempotency` - Idempotency tracking
- `dsl_workflow_phases` - Phase assignments
- `verb_pattern_embeddings` - Semantic embeddings
- `verb_centroids` - Verb semantic centroids
- `v_verb_intent_patterns` - View: flattened intent patterns
- `v_verb_discovery` - View: verb search index
- `v_verb_embedding_stats` - View: embedding coverage

**Session & REPL (10):**
- `sessions` - V1 agent sessions (legacy)
- `dsl_sessions` - V1 DSL sessions (legacy)
- `dsl_session_events` - Session events
- `dsl_session_locks` - Session locking
- `repl_sessions_v2` - V2 REPL sessions (active)
- `repl_invocation_records` - REPL invocation audit
- `session_scopes` - Session scope tracking
- `session_scope_history` - Scope change history
- `session_bookmarks` - Session waypoints

**DSL Instances & Snapshots (8):**
- `dsl_instances` - DSL statement tracking
- `dsl_instance_versions` - Instance versioning
- `dsl_snapshots` - Point-in-time snapshots
- `dsl_idempotency` - Idempotency deduplication
- `v_execution_audit_with_view` - View: full execution trace
- `v_request_execution_trace` - View: request tracing
- `v_staged_runbook` - View: runbook state

**Semantic Matching & Learning (9):**
- `verb_pattern_embeddings` - Phrase semantic embeddings
- `semantic_match_cache` - Match result cache
- `intent_feedback` - User feedback signals
- `intent_feedback_analysis` - Feedback analysis
- `detected_patterns` - Pattern learning candidates
- `v_learning_feedback` - View: learning metrics
- `v_learning_stats` - View: learning statistics
- `v_verb_intent_patterns` - View: verb phrase index

---

## 11. BPMN Integration (4 tables)

BPMN workflow orchestration.

**Base Tables (4):**
- `bpmn_correlations` - Process instance ↔ session linking
- `bpmn_job_frames` - Job dedupe/dequeue tracking
- `bpmn_parked_tokens` - Waiting REPL entries
- `bpmn_pending_dispatches` - Resilient dispatch queue
- `expansion_reports` - DSL expansion audit trail

---

## 12. KYC & Screening (19 tables)

Know-Your-Customer, due diligence, screening, and sanctions checks.

**KYC Cases (5):**
- `kyc_decisions` - KYC approval decisions
- `kyc_service_agreements` - KYC service contracts
- `case_evaluation_snapshots` - KYC case snapshots
- `case_types` - Case type taxonomy
- `v_case_redflag_summary` - View: KYC red flags

**Screening & PEP (7):**
- `screening_lists` - Sanctions list subscriptions
- `screening_types` - Screening type taxonomy
- `screening_requirements` - Screening requirement rules
- `person_pep_status` - Politically exposed person flags
- `v_blocking_reasons_expanded` - View: blocking reasons

**Verification & Challenges (7):**
- `verification_challenges` - Verification blockers
- `verification_escalations` - Escalated verifications
- `cbu_relationship_verification` - Relationship verification
- `threshold_requirements` - Risk threshold definitions
- `risk_ratings` - Risk assessment scores
- `risk_bands` - Risk band definitions
- `v_cbu_kyc_summary` - View: KYC status

---

## 13. GLEIF Integration (3 tables)

Legal Entity Identifier (LEI) data and corporate hierarchy import.

**Base Tables (3):**
- `gleif_relationships` - GLEIF hierarchy relationships
- `gleif_sync_log` - Synchronization audit
- `entity_bods_links` - GLEIF ↔ BODS mapping
- `v_gleif_hierarchy` - View: hierarchy traversal
- `v_entities_with_lei` - View: LEI-linked entities

---

## 14. Client Profile & Classification (11 tables)

Client profiling, classification, and booking details.

**Client Profile (7):**
- `client_profile` - Client master profile
- `client_types` - Client type taxonomy
- `client_classification` - Client classification/rating
- `client_allegations` - Allegation tracking
- `client_principal_relationship` - Client ↔ principal linkage
- `delegation_relationships` - Delegation structure
- `trust_parties` - Trust party relationships
- `v_allegation_summary` - View: allegations
- `v_cgr_canonical` - View: canonical relationship

**Booking Details (4):**
- `booking_principal` - Booking principal entity
- `booking_location` - Booking location
- `v_open_discrepancies` - View: discrepancy tracking
- `v_cgr_discrepancies` - View: relationship discrepancies

---

## 15. Reference / Taxonomy (52 tables)

Reference data, enumerations, and system taxonomies.

**Role & Category Taxonomies (5):**
- `roles` - Role definitions
- `role_types` - Role type taxonomy
- `role_categories` - Role grouping
- `role_applicable_entity_types` - Role × entity type matrix

**Entity & Relationship Taxonomies (6):**
- `entity_types` - Entity type taxonomy
- `entity_type_dependencies` - Type hierarchy
- `entity_share_classes` - Share class definitions
- `edge_types` - Relationship type taxonomy
- `node_types` - Graph node type taxonomy
- `delegation_relationships` - Delegation types

**Operational Taxonomies (6):**
- `case_types` - Workflow case types
- `document_types` - Document classification
- `settlement_types` - Settlement method taxonomy
- `ssi_types` - Settlement identifier types
- `screening_types` - Screening type taxonomy
- `sla_metric_types` - SLA metric types

**Geographic & Regulatory (4):**
- `master_jurisdictions` - Jurisdiction list
- `jurisdictions` - Jurisdiction reference (active)
- `regulators` - Regulatory authority registry
- `v_regulatory_gaps` - View: regulatory compliance gaps

**System & Lookup (8):**
- `dictionary` - Text lookup dictionary
- `rule` - Rule definitions
- `rule_field_dictionary` - Rule field catalog
- `ruleset` - Rule grouping
- `placeholder_kinds` - Placeholder entity types
- `view_modes` - View mode enum
- `currencies` - Currency catalog
- `srdef_discovery_reasons` - Service discovery reasons

**Layout & Visualization (3):**
- `layout_cache` - Cached layout computations
- `layout_config` - Layout configuration
- `view_modes` - Visualization modes

---

## View Tables (87 Total)

Materialized or computed views for reporting, analytics, and optimized queries.

### CBU Views (24)
- `v_cbus_by_manco` - CBUs grouped by management company
- `v_cbu_lifecycle` - CBU lifecycle status
- `v_cbu_lifecycle_coverage` - Lifecycle completeness
- `v_cbu_lifecycle_gaps` - Missing lifecycle steps
- `v_cbu_products` - CBU product inventory
- `v_cbu_subscriptions` - CBU contract subscriptions
- `v_cbu_attr_summary` - Attribute status overview
- `v_cbu_attr_gaps` - Missing attributes
- `v_cbu_unified_gaps` - Unified attribute gaps
- `v_cbu_entity_graph` - Entity relationship visualization
- `v_cbu_entity_with_roles` - Entity + role assignments
- `v_cbu_investor_details` - Investor information
- `v_cbu_investor_groups` - Investor grouping
- `v_cbu_matrix_effective` - Materialized matrix
- `v_cbu_kyc_summary` - KYC completion status
- `v_cbu_readiness_summary` - Onboarding readiness
- `v_cbu_service_gaps` - Service delivery gaps
- `v_cbu_validation_summary` - Data quality summary
- `v_service_readiness_dashboard` - Service readiness rollup

### Client & Group Views (9)
- `v_client_group_aliases` - Client alias lookup
- `v_client_group_anchors` - Role-based anchor lookup
- `v_client_group_entity_search` - Entity discovery
- `v_client_entity_tags` - Entity tagging
- `v_entity_aliases` - Entity name variants
- `v_cgr_canonical` - Canonical relationship view
- `v_cgr_discrepancies` - Relationship discrepancies
- `v_cgr_unverified_allegations` - Unverified allegations
- `v_manco_group_summary` - ManCo group rollup

### Entity Views (5)
- `entity_search_view` - Full-text entity search
- `entity_relationships_current` - Current relationships only
- `v_entities_with_lei` - LEI-linked entities
- `v_entity_linking_data` - Entity linking snapshot
- `v_entity_linking_stats` - Entity linking statistics

### UBO Views (5)
- `v_ubo_candidates` - UBO candidate pool
- `v_ubo_evidence_summary` - Evidence completeness
- `v_ubo_interests` - Interest holdings

### Document & Workflow Views (10)
- `v_document_extraction_map` - Document data mapping
- `v_staged_runbook` - Runbook visualization
- `v_workflow_summary` - Workflow status overview
- `v_request_execution_trace` - Execution tracing
- `v_execution_audit_with_view` - Full audit trail
- `v_runbook_summary` - Runbook ledger

### Product & Service Views (8)
- `v_active_trading_profiles` - Active profiles
- `v_active_rate_cards` - Active rate cards
- `v_rate_card_history` - Rate card versioning
- `v_service_intents_active` - Active service intents
- `v_provisioning_pending` - Pending provisioning
- `v_offerings_without_rules` - Incomplete offerings
- `v_operational_gaps` - Operational readiness gaps
- `v_commercial_gaps` - Commercial setup gaps

### Session & REPL Views (4)
- `v_session_view_history` - Session navigation history
- `v_staging_summary` - Staging status (if exists)

### Verb & Learning Views (6)
- `v_verb_discovery` - Verb search index
- `v_verb_intent_patterns` - Intent phrase flattening
- `v_verb_embedding_stats` - Embedding coverage
- `v_verbs_needing_recompile` - Stale verb definitions
- `v_learning_feedback` - Learning signal metrics
- `v_learning_stats` - Learning statistics

### Support Views (11)
- `v_attribute_current` - Current attribute values
- `v_contract_summary` - Contract metadata
- `v_deal_summary` - Deal overview
- `v_billing_profile_summary` - Billing state
- `v_case_redflag_summary` - KYC flags
- `v_blocking_reasons_expanded` - Blocking reason details
- `v_allegation_summary` - Allegation tracking
- `v_commercial_gaps` - Commercial compliance gaps
- `v_regulatory_gaps` - Regulatory compliance gaps
- `v_gleif_hierarchy` - GLEIF hierarchy traversal

---

## Summary by Category

| Category | Base Tables | Views | Total |
|----------|------------|-------|-------|
| 1. Core Entity Model | 15 | 5 | 20 |
| 2. CBU Aggregate | 29 | 19 | 48 |
| 3. Client Group & UBO | 28 | 9 | 37 |
| 4. Product / Service / Resource | 46 | 8 | 54 |
| 5. Instrument Matrix | 7 | 1 | 8 |
| 6. Attribute Dictionary & Evidence | 24 | 2 | 26 |
| 7. Legal Contracts & Onboarding | 10 | 2 | 12 |
| 8. Deal Record & Fee Billing | 10 | 1 | 11 |
| 9. Workflow / Task Queue | 15 | 3 | 18 |
| 10. DSL / Verb / Session / REPL | 38 | 10 | 48 |
| 11. BPMN Integration | 5 | 0 | 5 |
| 12. KYC & Screening | 19 | 5 | 24 |
| 13. GLEIF Integration | 3 | 2 | 5 |
| 14. Client Profile & Classification | 11 | 4 | 15 |
| 15. Reference / Taxonomy | 52 | 0 | 52 |
| **TOTAL** | **237** | **87** | **324** |

---

## Design Principles

### Logical Grouping
Each category represents a cohesive domain with clear boundaries and minimal cross-category dependencies.

### Naming Conventions
- **Base tables:** lowercase with underscores (`cbu_entity_roles`)
- **Views:** `v_` prefix for readability (`v_cbu_attr_summary`)
- **Enums/Reference:** singular forms (`role`, `entity_type`, `case_type`)

### Referential Integrity
Foreign keys respect domain boundaries:
- CBU Aggregate depends on Core Entity Model (entities are members of CBUs)
- Client Group & UBO depends on Core Entity Model (entities have ownership/control)
- All operational tables depend on Reference/Taxonomy (roles, types, jurisdictions)

### Performance Considerations
- High-cardinality queries use `semantic_match_cache` and `layout_cache`
- Large result sets have materialized views (`v_cbu_matrix_effective`, `v_active_trading_profiles`)
- Frequently joined tables are denormalized where appropriate (e.g., `cbu_entity_roles` includes entity metadata)

### Audit & Compliance
Every domain has audit/history tables where required:
- `*_log` tables for mutations (`cbu_change_log`, `dsl_generation_log`)
- `*_history` tables for temporal tracking (`entity_relationships_history`, `cbu_entity_roles_history`)
- `*_events` tables for lifecycle tracking (`workflow_task_events`, `deal_events`)
