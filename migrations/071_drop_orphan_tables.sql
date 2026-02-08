-- Migration 071: Drop orphan tables
--
-- These 59 tables are MIGRATION-ONLY: they exist in prior migration DDL
-- but have zero references in verb YAML or Rust code.
-- Cross-referenced against rust/config/verbs/**/*.yaml and rust/**/*.rs.
--
-- Also drops 7 orphan views that depend solely on dropped tables.

BEGIN;

-- ============================================================
-- 1. Drop orphan VIEWS that depend on tables we're dropping
-- ============================================================
DROP VIEW IF EXISTS "ob-poc".dsl_execution_summary CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_cbu_kyc_scope CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_current_session_scope CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_entity_regulatory_status CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_recent_scope_snapshots CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_resolution_method_stats CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_scope_resolution_quality CASCADE;
DROP VIEW IF EXISTS "ob-poc".v_session_resolution_timeline CASCADE;

-- ============================================================
-- 2. Drop FK constraints from KEPT tables that reference drop targets
-- ============================================================

-- kyc.share_class_identifiers → kyc.instrument_identifier_schemes
ALTER TABLE kyc.share_class_identifiers
    DROP CONSTRAINT IF EXISTS share_class_identifiers_scheme_code_fkey;

-- ob-poc.case_evaluation_snapshots → ob-poc.case_decision_thresholds
ALTER TABLE "ob-poc".case_evaluation_snapshots
    DROP CONSTRAINT IF EXISTS case_evaluation_snapshots_matched_threshold_id_fkey;

-- ob_kyc.entity_regulatory_registrations → ob_ref.registration_types
ALTER TABLE ob_kyc.entity_regulatory_registrations
    DROP CONSTRAINT IF EXISTS entity_regulatory_registrations_registration_type_fkey;

-- ob_ref.regulators → ob_ref.regulatory_tiers
ALTER TABLE ob_ref.regulators
    DROP CONSTRAINT IF EXISTS regulators_regulatory_tier_fkey;

-- ============================================================
-- 3. Drop orphan tables — public schema (10)
-- ============================================================
DROP TABLE IF EXISTS public.rule_executions CASCADE;
DROP TABLE IF EXISTS public.rule_versions CASCADE;
DROP TABLE IF EXISTS public.rule_dependencies CASCADE;
DROP TABLE IF EXISTS public.rules CASCADE;
DROP TABLE IF EXISTS public.derived_attributes CASCADE;
DROP TABLE IF EXISTS public.rule_categories CASCADE;
DROP TABLE IF EXISTS public.business_attributes CASCADE;
DROP TABLE IF EXISTS public.attribute_sources CASCADE;
DROP TABLE IF EXISTS public.data_domains CASCADE;
DROP TABLE IF EXISTS public.credentials_vault CASCADE;

-- ============================================================
-- 4. Drop orphan tables — ob-poc schema (33)
-- ============================================================

-- DSL artifacts (superseded by current pipeline)
DROP TABLE IF EXISTS "ob-poc".dsl_execution_log CASCADE;
DROP TABLE IF EXISTS "ob-poc".dsl_versions CASCADE;
DROP TABLE IF EXISTS "ob-poc".dsl_domains CASCADE;
DROP TABLE IF EXISTS "ob-poc".dsl_examples CASCADE;
DROP TABLE IF EXISTS "ob-poc".dsl_graph_contexts CASCADE;

-- Session/scope (superseded by V2 REPL)
DROP TABLE IF EXISTS "ob-poc".resolution_events CASCADE;
DROP TABLE IF EXISTS "ob-poc".scope_snapshots CASCADE;
DROP TABLE IF EXISTS "ob-poc".session_bookmarks CASCADE;
DROP TABLE IF EXISTS "ob-poc".session_scope_history CASCADE;
DROP TABLE IF EXISTS "ob-poc".session_scopes CASCADE;

-- Entity subtypes (never implemented beyond migration)
DROP TABLE IF EXISTS "ob-poc".entity_cooperatives CASCADE;
DROP TABLE IF EXISTS "ob-poc".entity_crud_rules CASCADE;
DROP TABLE IF EXISTS "ob-poc".entity_foundations CASCADE;
DROP TABLE IF EXISTS "ob-poc".entity_government CASCADE;
DROP TABLE IF EXISTS "ob-poc".entity_regulatory_profiles CASCADE;
DROP TABLE IF EXISTS "ob-poc".entity_validation_rules CASCADE;

-- Onboarding pipeline (superseded by deal record)
DROP TABLE IF EXISTS "ob-poc".onboarding_tasks CASCADE;
DROP TABLE IF EXISTS "ob-poc".onboarding_executions CASCADE;
DROP TABLE IF EXISTS "ob-poc".onboarding_products CASCADE;

-- Config / reference data (never wired)
DROP TABLE IF EXISTS "ob-poc".attribute_dictionary CASCADE;
DROP TABLE IF EXISTS "ob-poc".case_decision_thresholds CASCADE;
DROP TABLE IF EXISTS "ob-poc".cbu_entity_roles_history CASCADE;
DROP TABLE IF EXISTS "ob-poc".cbu_service_contexts CASCADE;
DROP TABLE IF EXISTS "ob-poc".client_group_anchor_role CASCADE;
DROP TABLE IF EXISTS "ob-poc".csg_validation_rules CASCADE;
DROP TABLE IF EXISTS "ob-poc".document_validity_rules CASCADE;
DROP TABLE IF EXISTS "ob-poc".fund_investors CASCADE;
DROP TABLE IF EXISTS "ob-poc".kyc_case_sponsor_decisions CASCADE;
DROP TABLE IF EXISTS "ob-poc".market_csd_mappings CASCADE;
DROP TABLE IF EXISTS "ob-poc".master_entity_xref CASCADE;
DROP TABLE IF EXISTS "ob-poc".red_flag_severities CASCADE;
DROP TABLE IF EXISTS "ob-poc".redflag_score_config CASCADE;
DROP TABLE IF EXISTS "ob-poc".regulatory_tiers CASCADE;
DROP TABLE IF EXISTS "ob-poc".resource_profile_sources CASCADE;
DROP TABLE IF EXISTS "ob-poc".role_incompatibilities CASCADE;
DROP TABLE IF EXISTS "ob-poc".role_requirements CASCADE;
DROP TABLE IF EXISTS "ob-poc".schema_changes CASCADE;
DROP TABLE IF EXISTS "ob-poc".service_option_choices CASCADE;
DROP TABLE IF EXISTS "ob-poc".service_option_definitions CASCADE;
DROP TABLE IF EXISTS "ob-poc".taxonomy_crud_log CASCADE;
DROP TABLE IF EXISTS "ob-poc".trading_profile_documents CASCADE;
DROP TABLE IF EXISTS "ob-poc".trading_profile_migration_backup CASCADE;
DROP TABLE IF EXISTS "ob-poc".ubo_treatments CASCADE;
DROP TABLE IF EXISTS "ob-poc".workstream_statuses CASCADE;

-- ============================================================
-- 5. Drop orphan tables — kyc schema (8)
-- ============================================================
DROP TABLE IF EXISTS kyc.approval_requests CASCADE;
DROP TABLE IF EXISTS kyc.bods_right_type_mapping CASCADE;
DROP TABLE IF EXISTS kyc.doc_request_acceptable_types CASCADE;
DROP TABLE IF EXISTS kyc.instrument_identifier_schemes CASCADE;
DROP TABLE IF EXISTS kyc.investor_lifecycle_history CASCADE;
DROP TABLE IF EXISTS kyc.investor_lifecycle_transitions CASCADE;
DROP TABLE IF EXISTS kyc.research_confidence_config CASCADE;
DROP TABLE IF EXISTS kyc.rule_executions CASCADE;

-- ============================================================
-- 6. Drop orphan tables — custody schema (3)
-- ============================================================
DROP TABLE IF EXISTS custody.instruction_paths CASCADE;
DROP TABLE IF EXISTS custody.instruction_types CASCADE;
DROP TABLE IF EXISTS custody.cfi_codes CASCADE;

-- ============================================================
-- 7. Drop orphan tables — agent schema (2)
-- ============================================================
DROP TABLE IF EXISTS agent.esper_aliases CASCADE;
DROP TABLE IF EXISTS agent.stopwords CASCADE;

-- ============================================================
-- 8. Drop orphan tables — ob_ref schema (2)
-- ============================================================
DROP TABLE IF EXISTS ob_ref.registration_types CASCADE;
DROP TABLE IF EXISTS ob_ref.regulatory_tiers CASCADE;

-- ============================================================
-- 9. Drop orphan tables — teams schema (5)
-- ============================================================
DROP TABLE IF EXISTS teams.access_review_log CASCADE;
DROP TABLE IF EXISTS teams.access_domains CASCADE;
DROP TABLE IF EXISTS teams.function_domains CASCADE;
DROP TABLE IF EXISTS teams.membership_audit_log CASCADE;
DROP TABLE IF EXISTS teams.membership_history CASCADE;

COMMIT;
