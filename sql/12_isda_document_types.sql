-- 12_isda_document_types.sql
-- ISDA Domain Document Types and Schema Extensions
--
-- This script adds ISDA-specific document types, issuers, and extensions
-- to support comprehensive ISDA Master Agreement workflows and derivatives documentation.

-- ============================================================================
-- ISDA DOCUMENT TYPES
-- ============================================================================

-- ISDA Master Agreement and related documents
INSERT INTO "ob-poc".document_types (type_code, display_name, category, domain, description, typical_issuers, validity_period_days, expected_fields, ai_description, common_contents, key_data_points) VALUES

-- Master Agreement
('isda_master_agreement', 'ISDA Master Agreement', 'legal', 'isda', 'International Swaps and Derivatives Association Master Agreement governing OTC derivative transactions between parties',
 ARRAY['isda_inc', 'law_firm', 'financial_institution'],
 NULL, -- No standard expiry
 '{
   "agreement_date": "date",
   "party_a": "string",
   "party_b": "string",
   "governing_law": "string",
   "version": "string",
   "multicurrency": "boolean",
   "cross_default": "boolean",
   "credit_support_provider": "string",
   "specified_entities": "array",
   "threshold_amount": "number",
   "minimum_transfer_amount": "number",
   "termination_currency": "string"
 }',
 'The foundational legal agreement that governs all derivative transactions between two parties. Contains standard terms and conditions, events of default, termination provisions, and dispute resolution mechanisms.',
 'Legal framework for derivatives trading including: party identification, governing law, events of default, termination events, close-out netting provisions, credit support arrangements, dispute resolution procedures',
 ARRAY['parties', 'governing_law', 'version', 'termination_events', 'credit_support', 'netting_provisions', 'dispute_resolution']),

-- Credit Support Annex
('isda_csa', 'Credit Support Annex (CSA)', 'legal', 'isda', 'Credit Support Annex defining collateral arrangements and margin requirements for derivative transactions',
 ARRAY['law_firm', 'financial_institution'],
 NULL,
 '{
   "base_currency": "string",
   "threshold_amount_party_a": "number",
   "threshold_amount_party_b": "number",
   "minimum_transfer_amount": "number",
   "rounding_amount": "number",
   "eligible_collateral": "array",
   "valuation_percentage": "map",
   "notification_time": "string",
   "margin_approach": "string",
   "dispute_resolution": "string"
 }',
 'Legal document defining how collateral will be posted and managed to secure derivative exposures between parties.',
 'Collateral management framework including: threshold amounts, minimum transfer amounts, eligible collateral types, valuation methodology, margin call procedures, dispute resolution for collateral',
 ARRAY['threshold_amounts', 'eligible_collateral', 'valuation_methodology', 'margin_procedures', 'dispute_resolution']),

-- Schedule to Master Agreement
('isda_schedule', 'Schedule to ISDA Master Agreement', 'legal', 'isda', 'Schedule containing party-specific elections and modifications to the standard ISDA Master Agreement terms',
 ARRAY['law_firm', 'financial_institution'],
 NULL,
 '{
   "termination_events": "array",
   "additional_termination_events": "array",
   "credit_event_upon_merger": "boolean",
   "automatic_early_termination": "boolean",
   "payments_on_early_termination": "string",
   "credit_support_provider": "string",
   "specified_entities": "array",
   "cross_default_threshold": "number",
   "governing_law_elections": "string"
 }',
 'Customized terms and elections that modify the standard ISDA Master Agreement to reflect the specific relationship between the parties.',
 'Party-specific modifications including: termination events, credit support arrangements, governing law elections, cross-default provisions, merger events, early termination procedures',
 ARRAY['termination_events', 'credit_support', 'governing_law', 'cross_default', 'early_termination']),

-- Trade Confirmation
('isda_confirmation', 'Trade Confirmation', 'financial', 'isda', 'Legal confirmation of specific derivative transaction terms executed under ISDA Master Agreement',
 ARRAY['financial_institution', 'broker_dealer'],
 NULL, -- Confirmation is valid for life of trade
 '{
   "trade_date": "date",
   "effective_date": "date",
   "termination_date": "date",
   "notional_amount": "number",
   "currency": "string",
   "product_type": "string",
   "counterparty_a": "string",
   "counterparty_b": "string",
   "underlying": "string",
   "settlement_terms": "map",
   "calculation_agent": "string",
   "business_day_convention": "string"
 }',
 'Legal documentation of a specific derivative trade including all economic and operational terms.',
 'Complete trade specification including: parties, trade date, notional amount, underlying reference, payment terms, settlement procedures, calculation methodology',
 ARRAY['trade_details', 'economic_terms', 'settlement_terms', 'calculation_methodology', 'business_days']),

-- Amendment Letters
('isda_amendment', 'ISDA Amendment Letter', 'legal', 'isda', 'Amendment to existing ISDA Master Agreement or related documentation',
 ARRAY['law_firm', 'financial_institution'],
 NULL,
 '{
   "original_agreement_date": "date",
   "amendment_date": "date",
   "amendment_type": "string",
   "sections_amended": "array",
   "effective_date": "date",
   "party_a_signatory": "string",
   "party_b_signatory": "string",
   "amendment_description": "text"
 }',
 'Legal document modifying terms of existing ISDA Master Agreement or related documents.',
 'Modifications to existing agreements including: sections being amended, effective dates, new terms, signatory information',
 ARRAY['amendment_type', 'sections_amended', 'effective_date', 'new_terms', 'signatories']),

-- Netting Opinions
('isda_netting_opinion', 'ISDA Netting Opinion', 'legal', 'isda', 'Legal opinion on enforceability of close-out netting provisions under local law',
 ARRAY['law_firm', 'isda_inc'],
 1825, -- Typically valid for 5 years
 '{
   "jurisdiction": "string",
   "opinion_date": "date",
   "law_firm": "string",
   "opinion_type": "string",
   "entity_types_covered": "array",
   "limitations": "array",
   "assumptions": "array",
   "enforceability_conclusion": "string"
 }',
 'Legal opinion confirming that close-out netting provisions in ISDA agreements will be enforceable under local law.',
 'Legal analysis of netting enforceability including: jurisdictional scope, entity types covered, legal limitations, assumptions, enforceability conclusions',
 ARRAY['jurisdiction', 'enforceability', 'entity_types', 'limitations', 'legal_assumptions']),

-- Definitions Booklet
('isda_definitions', 'ISDA Definitions', 'legal', 'isda', 'Standardized definitions for derivative transaction terms published by ISDA',
 ARRAY['isda_inc'],
 NULL, -- Definitions are evergreen until superseded
 '{
   "definitions_type": "string",
   "publication_date": "date",
   "version": "string",
   "product_coverage": "array",
   "key_definitions": "array",
   "calculation_methodology": "map"
 }',
 'Standardized dictionary of terms and calculation methodologies used in derivative transactions.',
 'Standard definitions including: product terminology, calculation methodologies, business day conventions, settlement procedures, events of default definitions',
 ARRAY['product_terms', 'calculation_methods', 'business_day_conventions', 'settlement_definitions']),

-- Novation Agreement
('isda_novation', 'ISDA Novation Agreement', 'legal', 'isda', 'Agreement transferring rights and obligations of derivative transactions to new counterparty',
 ARRAY['financial_institution', 'law_firm'],
 NULL,
 '{
   "novation_date": "date",
   "original_transaction": "string",
   "transferor": "string",
   "transferee": "string",
   "remaining_party": "string",
   "consent_required": "boolean",
   "effective_date": "date",
   "novated_transactions": "array"
 }',
 'Legal mechanism for transferring derivative positions from one party to another.',
 'Transaction transfer documentation including: parties involved, transactions being transferred, effective dates, consent requirements',
 ARRAY['transfer_parties', 'novated_transactions', 'effective_date', 'consent_requirements']),

-- Closeout Amount Statement
('isda_closeout_statement', 'Close-out Amount Statement', 'financial', 'isda', 'Statement calculating close-out amounts upon early termination of ISDA agreement',
 ARRAY['financial_institution'],
 90, -- Typically must be provided within 90 days
 '{
   "calculation_date": "date",
   "early_termination_date": "date",
   "calculation_agent": "string",
   "terminated_transactions": "array",
   "market_quotations": "array",
   "loss_calculation_method": "string",
   "total_closeout_amount": "number",
   "currency": "string"
 }',
 'Financial statement showing amounts owed upon early termination of derivative transactions.',
 'Termination valuation including: terminated transactions, market quotations, loss calculations, net settlement amounts',
 ARRAY['terminated_transactions', 'market_valuations', 'loss_calculations', 'net_amounts']);

-- ============================================================================
-- ISDA-SPECIFIC ISSUING AUTHORITIES
-- ============================================================================

INSERT INTO "ob-poc".document_issuers (issuer_code, legal_name, jurisdiction, regulatory_type, authority_level, document_types_issued, official_website, api_integration_available, reliability_score) VALUES

-- ISDA Inc
('isda_inc', 'International Swaps and Derivatives Association, Inc.', 'US', 'trade_association', 'industry',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_definitions', 'isda_netting_opinion'],
 'https://www.isda.org', false, 1.0),

-- Major Financial Institutions (examples)
('jpmorgan_chase', 'JPMorgan Chase & Co.', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 'https://www.jpmorganchase.com', false, 0.95),

('goldman_sachs', 'The Goldman Sachs Group, Inc.', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 'https://www.goldmansachs.com', false, 0.95),

('morgan_stanley', 'Morgan Stanley', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 'https://www.morganstanley.com', false, 0.95),

-- International Banks
('deutsche_bank', 'Deutsche Bank AG', 'DE', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment'],
 'https://www.db.com', false, 0.9),

('ubs', 'UBS Group AG', 'CH', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment'],
 'https://www.ubs.com', false, 0.9),

-- Law Firms (examples)
('allen_overy', 'Allen & Overy LLP', 'GB', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_amendment', 'isda_netting_opinion'],
 'https://www.allenovery.com', false, 0.95),

('cleary_gottlieb', 'Cleary Gottlieb Steen & Hamilton LLP', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_amendment', 'isda_netting_opinion'],
 'https://www.clearygottlieb.com', false, 0.95);

-- ============================================================================
-- ISDA DOCUMENT TEMPLATES
-- ============================================================================

-- Templates for common ISDA documents
INSERT INTO "ob-poc".document_templates (document_type_id, template_name, template_version, template_structure, required_fields, ai_prompts) VALUES

-- ISDA Master Agreement Template
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'isda_master_agreement'),
 'ISDA 2002 Master Agreement Template', '2002.1',
 '{
   "header": {"parties": ["string", "string"], "date": "date"},
   "part1": {"termination_events": "array", "credit_events": "array"},
   "part2": {"tax_representations": "object", "miscellaneous": "object"},
   "schedule": {"elections": "object", "definitions": "object"},
   "signatures": {"party_a": "string", "party_b": "string", "date": "date"}
 }',
 '{
   "party_a": {"required": true, "type": "string", "description": "Full legal name of Party A"},
   "party_b": {"required": true, "type": "string", "description": "Full legal name of Party B"},
   "governing_law": {"required": true, "type": "string", "description": "Governing law jurisdiction"},
   "agreement_date": {"required": true, "type": "date", "description": "Date of agreement execution"}
 }',
 '{
   "extraction_prompt": "Extract the following key information from this ISDA Master Agreement: party names, governing law, agreement date, termination events, and credit support provisions.",
   "validation_prompt": "Verify that this document contains the standard ISDA Master Agreement sections and all required party information is complete.",
   "summary_prompt": "Provide a business summary of this ISDA Master Agreement including the parties, key terms, and governing provisions."
 }'),

-- CSA Template
((SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'isda_csa'),
 'ISDA Credit Support Annex Template', '1995.1',
 '{
   "paragraph_11": {"base_currency": "string", "eligible_collateral": "array"},
   "paragraph_12": {"valuation_and_margin": "object"},
   "paragraph_13": {"conditions_precedent": "array"},
   "elections": {"threshold_amounts": "object", "minimum_transfer": "number"}
 }',
 '{
   "base_currency": {"required": true, "type": "string", "description": "Base currency for calculations"},
   "threshold_amount_party_a": {"required": true, "type": "number", "description": "Threshold amount for Party A"},
   "threshold_amount_party_b": {"required": true, "type": "number", "description": "Threshold amount for Party B"}
 }',
 '{
   "extraction_prompt": "Extract collateral terms from this CSA including threshold amounts, eligible collateral, and valuation methodology.",
   "validation_prompt": "Confirm this CSA contains proper threshold amounts, eligible collateral definitions, and margin call procedures.",
   "summary_prompt": "Summarize the collateral arrangement including threshold amounts and eligible collateral types."
 }');

-- ============================================================================
-- ISDA DOCUMENT RELATIONSHIPS SETUP
-- ============================================================================

-- Common document relationships for ISDA documentation
-- Note: Specific relationships will be created via document.link verb in workflows

-- ============================================================================
-- SAMPLE DOCUMENT CATALOG ENTRIES
-- ============================================================================

-- Sample ISDA documents for testing and demonstration
INSERT INTO "ob-poc".document_catalog (
    document_code, document_type_id, issuer_id, title, description,
    issue_date, language, related_entities, tags, confidentiality_level
) VALUES

-- Sample ISDA Master Agreement
('doc-isda-master-zenith-001',
 (SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'isda_master_agreement'),
 (SELECT issuer_id FROM "ob-poc".document_issuers WHERE issuer_code = 'jpmorgan_chase'),
 'ISDA Master Agreement - Zenith Capital Partners LP and JPMorgan Chase Bank, N.A.',
 'ISDA 2002 Master Agreement governing derivative transactions between Zenith Capital and JPMorgan',
 '2023-01-15',
 'en',
 ARRAY['company-zenith-spv-001', 'jpmorgan-chase-entity'],
 ARRAY['isda', 'master_agreement', 'derivatives', 'otc', 'zenith', 'jpmorgan'],
 'confidential'),

-- Sample CSA
('doc-isda-csa-zenith-001',
 (SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'isda_csa'),
 (SELECT issuer_id FROM "ob-poc".document_issuers WHERE issuer_code = 'jpmorgan_chase'),
 'Credit Support Annex - Zenith Capital Partners LP and JPMorgan Chase Bank, N.A.',
 'Credit Support Annex defining collateral arrangements for derivative exposures',
 '2023-01-15',
 'en',
 ARRAY['company-zenith-spv-001', 'jpmorgan-chase-entity'],
 ARRAY['isda', 'csa', 'collateral', 'margin', 'zenith', 'jpmorgan'],
 'confidential'),

-- Sample Trade Confirmation
('doc-isda-confirm-zenith-001',
 (SELECT type_id FROM "ob-poc".document_types WHERE type_code = 'isda_confirmation'),
 (SELECT issuer_id FROM "ob-poc".document_issuers WHERE issuer_code = 'jpmorgan_chase'),
 'Interest Rate Swap Confirmation - Trade Date 2024-03-15',
 '5-year USD interest rate swap confirmation under ISDA Master Agreement',
 '2024-03-15',
 'en',
 ARRAY['company-zenith-spv-001', 'jpmorgan-chase-entity'],
 ARRAY['isda', 'confirmation', 'irs', 'interest_rate_swap', 'usd', 'zenith'],
 'confidential');

-- Create document relationships
INSERT INTO "ob-poc".document_relationships (source_document_id, target_document_id, relationship_type, description) VALUES

-- CSA supports Master Agreement
((SELECT document_id FROM "ob-poc".document_catalog WHERE document_code = 'doc-isda-csa-zenith-001'),
 (SELECT document_id FROM "ob-poc".document_catalog WHERE document_code = 'doc-isda-master-zenith-001'),
 'supports',
 'CSA provides collateral framework for Master Agreement'),

-- Trade Confirmation references Master Agreement
((SELECT document_id FROM "ob-poc".document_catalog WHERE document_code = 'doc-isda-confirm-zenith-001'),
 (SELECT document_id FROM "ob-poc".document_catalog WHERE document_code = 'doc-isda-master-zenith-001'),
 'references',
 'Trade confirmation executed under Master Agreement framework');

-- ISDA document types and infrastructure are now ready for DSL workflows
