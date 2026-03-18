import yaml
import re

# Table list from migration files
migration_tables = [
    "accounting.cost_allocations", "accounting.fee_schedules", "accounting.invoice_contacts",
    "accounting.invoice_lines", "accounting.invoices", "accounting.service_contracts",
    "client_portal.clients", "client_portal.commitments", "client_portal.credentials",
    "client_portal.escalations", "client_portal.sessions", "client_portal.submissions",
    "custody.ca_event_types", "custody.cbu_ca_instruction_windows", "custody.cbu_ca_preferences",
    "custody.cbu_ca_ssi_mappings", "custody.cbu_cash_sweep_config", "custody.cbu_cross_border_config",
    "custody.cbu_gateway_connectivity", "custody.cbu_gateway_fallbacks", "custody.cbu_gateway_routing",
    "custody.cbu_im_assignments", "custody.cbu_instruction_assignments", "custody.cbu_instruction_field_overrides",
    "custody.cbu_nav_impact_thresholds", "custody.cbu_pricing_config", "custody.cbu_pricing_fallback_chains",
    "custody.cbu_settlement_chains", "custody.cbu_settlement_location_preferences", "custody.cbu_stale_price_policies",
    "custody.cbu_tax_reclaim_config", "custody.cbu_tax_reporting", "custody.cbu_tax_status",
    "custody.cbu_valuation_schedule", "custody.instruction_message_types", "custody.instruction_templates",
    "custody.settlement_chain_hops", "custody.settlement_locations", "custody.tax_jurisdictions",
    "custody.tax_treaty_rates", "custody.trade_gateways", "kyc.outstanding_requests",
    "ob_kyc.entity_regulatory_registrations", "ob_ref.registration_types", "ob_ref.regulators",
    "ob_ref.regulatory_tiers", "ob_ref.request_types", "ob_ref.role_types",
    "ob-poc.cbu_entity_roles_history", "ob-poc.cbu_lifecycle_instances", "ob-poc.cbu_matrix_product_overlay",
    "ob-poc.cbu_product_subscriptions", "ob-poc.cbu_relationship_verification", "ob-poc.cbu_service_contexts",
    "ob-poc.cbu_sla_commitments", "ob-poc.cbu_structure_links", "ob-poc.dsl_graph_contexts",
    "ob-poc.dsl_verb_categories", "ob-poc.dsl_verb_sync_log", "ob-poc.dsl_verbs",
    "ob-poc.dsl_view_state_changes", "ob-poc.dsl_workflow_phases", "ob-poc.edge_types",
    "ob-poc.entity_concept_link", "ob-poc.entity_feature", "ob-poc.entity_regulatory_profiles",
    "ob-poc.entity_relationships", "ob-poc.extraction_jobs", "ob-poc.fund_investors",
    "ob-poc.instrument_lifecycles", "ob-poc.intent_feedback", "ob-poc.intent_feedback_analysis",
    "ob-poc.kyc_case_sponsor_decisions", "ob-poc.kyc_decisions", "ob-poc.kyc_service_agreements",
    "ob-poc.layout_cache", "ob-poc.layout_config", "ob-poc.layout_overrides",
    "ob-poc.lifecycle_resource_capabilities", "ob-poc.lifecycle_resource_types", "ob-poc.lifecycles",
    "ob-poc.market_csd_mappings", "ob-poc.node_types", "ob-poc.proofs", "ob-poc.regulators",
    "ob-poc.regulatory_tiers", "ob-poc.resource_profile_sources", "ob-poc.role_categories",
    "ob-poc.role_incompatibilities", "ob-poc.role_requirements", "ob-poc.role_types",
    "ob-poc.semantic_match_cache", "ob-poc.sla_breaches", "ob-poc.sla_measurements",
    "ob-poc.sla_metric_types", "ob-poc.sla_templates", "ob-poc.state_overrides",
    "ob-poc.trading_profile_documents", "ob-poc.trading_profile_migration_backup", "ob-poc.ubo_assertion_log",
    "ob-poc.ubo_edges", "ob-poc.ubo_observations", "ob-poc.ubo_treatments",
    "ob-poc.verb_pattern_embeddings", "ob-poc.view_modes", "ob-poc.workflow_audit_log",
    "ob-poc.workflow_definitions", "ob-poc.workflow_instances", "sem_reg.reducer_states",
    "teams.access_attestations", "teams.access_domains", "teams.access_review_campaigns",
    "teams.access_review_items", "teams.access_review_log", "teams.function_domains",
    "teams.membership_audit_log", "teams.membership_history", "teams.memberships",
    "teams.team_cbu_access", "teams.team_service_entitlements", "teams.teams"
]

def normalize_table_name(name):
    if "." in name:
        schema, table = name.split(".", 1)
        if schema == "ob-poc":
            return table
        return name
    return name

with open("config/sem_os_seeds/domain_metadata.yaml", "r") as f:
    metadata = yaml.safe_load(f)

catalogued_tables = set()
for domain_name, domain_data in metadata.get("domains", {}).items():
    for table_name in domain_data.get("tables", {}).keys():
        catalogued_tables.add(table_name)

missing_in_metadata = []
for mt in migration_tables:
    norm_mt = normalize_table_name(mt)
    if norm_mt not in catalogued_tables:
        missing_in_metadata.append(mt)

print(f"Total migration tables: {len(migration_tables)}")
print(f"Total catalogued tables in metadata: {len(catalogued_tables)}")
print(f"Missing in metadata: {len(missing_in_metadata)}")
for mt in sorted(missing_in_metadata):
    print(f"  - {mt}")
