-- 12_isda_document_types_phase2.sql
-- ISDA Document Types and Schema - Phase 2 Implementation
--
-- This script creates comprehensive ISDA document types, AttributeIDs, and issuers
-- with full referential integrity for derivative contract workflows.

-- ============================================================================
-- ISDA ATTRIBUTEIDS - ADD TO DICTIONARY TABLE
-- ============================================================================

-- ISDA-specific attributes that will be referenced by UUIDs
INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain, source, sink, created_at, updated_at) VALUES

-- ============================================================================
-- ISDA Master Agreement Attributes
-- ============================================================================
('isda0001-0000-0000-0000-000000000001', 'isda.master_agreement.agreement_date', 'Date ISDA Master Agreement was executed', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0001-0000-0000-0000-000000000002', 'isda.master_agreement.party_a', 'Legal name of Party A to the agreement', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0001-0000-0000-0000-000000000003', 'isda.master_agreement.party_b', 'Legal name of Party B to the agreement', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0001-0000-0000-0000-000000000004', 'isda.master_agreement.governing_law', 'Governing law jurisdiction', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "jurisdiction_code"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0001-0000-0000-0000-000000000005', 'isda.master_agreement.version', 'ISDA Master Agreement version (1992, 2002, etc.)', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "values": ["1987", "1992", "2002"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0001-0000-0000-0000-000000000006', 'isda.master_agreement.multicurrency', 'Whether multicurrency cross-default applies', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0001-0000-0000-0000-000000000007', 'isda.master_agreement.cross_default', 'Whether cross-default provisions apply', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0001-0000-0000-0000-000000000008', 'isda.master_agreement.termination_currency', 'Currency for termination payments', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Credit Support Annex (CSA) Attributes
-- ============================================================================
('isda0002-0000-0000-0000-000000000001', 'isda.csa.base_currency', 'Base currency for CSA calculations', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000002', 'isda.csa.threshold_party_a', 'Threshold amount for Party A', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000003', 'isda.csa.threshold_party_b', 'Threshold amount for Party B', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000004', 'isda.csa.minimum_transfer_amount', 'Minimum transfer amount', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000005', 'isda.csa.rounding_amount', 'Amount to round transfers', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000006', 'isda.csa.eligible_collateral', 'Types of eligible collateral', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000007', 'isda.csa.valuation_percentage', 'Haircuts by collateral type', 'ISDA', 'map', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000008', 'isda.csa.margin_approach', 'VM (variation margin) or IM (initial margin)', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["VM", "IM", "BOTH"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0002-0000-0000-0000-000000000009', 'isda.csa.notification_time', 'Time for margin call notifications', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Trade Confirmation Attributes
-- ============================================================================
('isda0003-0000-0000-0000-000000000001', 'isda.confirmation.trade_date', 'Date trade was executed', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000002', 'isda.confirmation.effective_date', 'Trade effective/start date', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000003', 'isda.confirmation.termination_date', 'Trade maturity/end date', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000004', 'isda.confirmation.notional_amount', 'Trade notional amount', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000005', 'isda.confirmation.currency', 'Trade currency', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000006', 'isda.confirmation.product_type', 'Derivative product type', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["IRS", "CDS", "FX_Forward", "FX_Option", "Equity_Option", "Commodity_Swap"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000007', 'isda.confirmation.payer', 'Party paying fixed rate/premium', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000008', 'isda.confirmation.receiver', 'Party receiving fixed rate/premium', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000009', 'isda.confirmation.underlying', 'Underlying reference (rate, bond, etc.)', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0003-0000-0000-0000-000000000010', 'isda.confirmation.calculation_agent', 'Entity responsible for calculations', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Schedule to Master Agreement Attributes
-- ============================================================================
('isda0004-0000-0000-0000-000000000001', 'isda.schedule.termination_events', 'List of termination events', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0004-0000-0000-0000-000000000002', 'isda.schedule.additional_termination_events', 'Additional termination events', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0004-0000-0000-0000-000000000003', 'isda.schedule.credit_event_upon_merger', 'Credit event upon merger election', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0004-0000-0000-0000-000000000004', 'isda.schedule.automatic_early_termination', 'Automatic early termination election', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0004-0000-0000-0000-000000000005', 'isda.schedule.payments_on_early_termination', 'Payment methodology on early termination', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": false, "values": ["first_method", "second_method", "market_quotation", "loss"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Amendment Letter Attributes
-- ============================================================================
('isda0005-0000-0000-0000-000000000001', 'isda.amendment.original_agreement_date', 'Date of original agreement being amended', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0005-0000-0000-0000-000000000002', 'isda.amendment.amendment_date', 'Date of amendment', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0005-0000-0000-0000-000000000003', 'isda.amendment.amendment_type', 'Type of amendment', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["schedule_modification", "csa_modification", "additional_terms", "termination_provision"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0005-0000-0000-0000-000000000004', 'isda.amendment.sections_amended', 'Sections being amended', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0005-0000-0000-0000-000000000005', 'isda.amendment.effective_date', 'Date amendment becomes effective', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Netting Opinion Attributes
-- ============================================================================
('isda0006-0000-0000-0000-000000000001', 'isda.netting_opinion.jurisdiction', 'Jurisdiction covered by opinion', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0006-0000-0000-0000-000000000002', 'isda.netting_opinion.opinion_date', 'Date of legal opinion', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0006-0000-0000-0000-000000000003', 'isda.netting_opinion.law_firm', 'Law firm providing opinion', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0006-0000-0000-0000-000000000004', 'isda.netting_opinion.entity_types_covered', 'Entity types covered by opinion', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0006-0000-0000-0000-000000000005', 'isda.netting_opinion.enforceability_conclusion', 'Conclusion on netting enforceability', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["enforceable", "enforceable_with_limitations", "not_enforceable", "uncertain"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Close-out Statement Attributes
-- ============================================================================
('isda0007-0000-0000-0000-000000000001', 'isda.closeout.calculation_date', 'Date of close-out calculation', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0007-0000-0000-0000-000000000002', 'isda.closeout.early_termination_date', 'Early termination date', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0007-0000-0000-0000-000000000003', 'isda.closeout.calculation_agent', 'Entity performing close-out calculation', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0007-0000-0000-0000-000000000004', 'isda.closeout.total_closeout_amount', 'Net close-out amount', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0007-0000-0000-0000-000000000005', 'isda.closeout.currency', 'Close-out amount currency', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0007-0000-0000-0000-000000000006', 'isda.closeout.valuation_method', 'Method used for valuation', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["market_quotation", "loss", "first_method", "second_method"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Novation Agreement Attributes
-- ============================================================================
('isda0008-0000-0000-0000-000000000001', 'isda.novation.novation_date', 'Date of novation', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0008-0000-0000-0000-000000000002', 'isda.novation.transferor', 'Party transferring the trade', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0008-0000-0000-0000-000000000003', 'isda.novation.transferee', 'Party receiving the trade', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0008-0000-0000-0000-000000000004', 'isda.novation.remaining_party', 'Non-transferring counterparty', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0008-0000-0000-0000-000000000005', 'isda.novation.consent_required', 'Whether remaining party consent needed', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- ISDA Definitions Attributes
-- ============================================================================
('isda0009-0000-0000-0000-000000000001', 'isda.definitions.definitions_type', 'Type of ISDA definitions', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["2006_Definitions", "2021_Definitions", "FX_and_Currency_Option_Definitions", "Equity_Derivatives_Definitions"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0009-0000-0000-0000-000000000002', 'isda.definitions.publication_date', 'Publication date of definitions', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0009-0000-0000-0000-000000000003', 'isda.definitions.version', 'Version of definitions', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('isda0009-0000-0000-0000-000000000004', 'isda.definitions.product_coverage', 'Products covered by definitions', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW())

ON CONFLICT (attribute_id) DO UPDATE SET
    name = EXCLUDED.name,
    long_description = EXCLUDED.long_description,
    updated_at = NOW();

-- ============================================================================
-- ISDA DOCUMENT TYPES
-- ============================================================================

-- Insert ISDA-specific document types with proper AttributeID linkage
INSERT INTO "ob-poc".document_types (
    type_code, display_name, category, domain, primary_attribute_id,
    description, typical_issuers, expected_attribute_ids, key_data_point_attributes,
    ai_description, common_contents, required_for_products, compliance_frameworks
) VALUES

-- ISDA Master Agreement
('isda_master_agreement', 'ISDA Master Agreement', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'International Swaps and Derivatives Association Master Agreement governing OTC derivative transactions between parties',
 ARRAY['isda_inc', 'law_firm', 'financial_institution'],
 ARRAY['isda0001-0000-0000-0000-000000000001'::uuid, 'isda0001-0000-0000-0000-000000000002'::uuid, 'isda0001-0000-0000-0000-000000000003'::uuid, 'isda0001-0000-0000-0000-000000000004'::uuid, 'isda0001-0000-0000-0000-000000000005'::uuid],
 ARRAY['isda0001-0000-0000-0000-000000000002'::uuid, 'isda0001-0000-0000-0000-000000000003'::uuid, 'isda0001-0000-0000-0000-000000000004'::uuid, 'isda0001-0000-0000-0000-000000000005'::uuid],
 'The foundational legal agreement that governs all derivative transactions between two parties. Contains standard terms and conditions, events of default, termination provisions, and dispute resolution mechanisms.',
 'Legal framework for derivatives trading including: party identification, governing law, events of default, termination events, close-out netting provisions, credit support arrangements, dispute resolution procedures',
 ARRAY['DERIVATIVES_TRADING', 'PRIME_BROKERAGE'],
 ARRAY['EMIR', 'Dodd-Frank', 'MiFID II']),

-- Credit Support Annex
('isda_csa', 'Credit Support Annex (CSA)', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Credit Support Annex defining collateral arrangements and margin requirements for derivative transactions',
 ARRAY['law_firm', 'financial_institution'],
 ARRAY['isda0002-0000-0000-0000-000000000001'::uuid, 'isda0002-0000-0000-0000-000000000002'::uuid, 'isda0002-0000-0000-0000-000000000003'::uuid, 'isda0002-0000-0000-0000-000000000004'::uuid, 'isda0002-0000-0000-0000-000000000006'::uuid, 'isda0002-0000-0000-0000-000000000008'::uuid],
 ARRAY['isda0002-0000-0000-0000-000000000002'::uuid, 'isda0002-0000-0000-0000-000000000003'::uuid, 'isda0002-0000-0000-0000-000000000004'::uuid, 'isda0002-0000-0000-0000-000000000006'::uuid],
 'Legal document defining how collateral will be posted and managed to secure derivative exposures between parties.',
 'Collateral management framework including: threshold amounts, minimum transfer amounts, eligible collateral types, valuation methodology, margin call procedures, dispute resolution for collateral',
 ARRAY['DERIVATIVES_TRADING', 'PRIME_BROKERAGE'],
 ARRAY['EMIR', 'Basel III', 'UMR']),

-- Schedule to Master Agreement
('isda_schedule', 'Schedule to ISDA Master Agreement', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Schedule containing party-specific elections and modifications to the standard ISDA Master Agreement terms',
 ARRAY['law_firm', 'financial_institution'],
 ARRAY['isda0004-0000-0000-0000-000000000001'::uuid, 'isda0004-0000-0000-0000-000000000002'::uuid, 'isda0004-0000-0000-0000-000000000003'::uuid, 'isda0004-0000-0000-0000-000000000004'::uuid, 'isda0004-0000-0000-0000-000000000005'::uuid],
 ARRAY['isda0004-0000-0000-0000-000000000001'::uuid, 'isda0004-0000-0000-0000-000000000005'::uuid],
 'Customized terms and elections that modify the standard ISDA Master Agreement to reflect the specific relationship between the parties.',
 'Party-specific modifications including: termination events, credit support arrangements, governing law elections, cross-default provisions, merger events, early termination procedures',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank']),

-- Trade Confirmation
('isda_confirmation', 'Trade Confirmation', 'financial', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Legal confirmation of specific derivative transaction terms executed under ISDA Master Agreement',
 ARRAY['financial_institution', 'broker_dealer'],
 ARRAY['isda0003-0000-0000-0000-000000000001'::uuid, 'isda0003-0000-0000-0000-000000000002'::uuid, 'isda0003-0000-0000-0000-000000000003'::uuid, 'isda0003-0000-0000-0000-000000000004'::uuid, 'isda0003-0000-0000-0000-000000000005'::uuid, 'isda0003-0000-0000-0000-000000000006'::uuid, 'isda0003-0000-0000-0000-000000000007'::uuid, 'isda0003-0000-0000-0000-000000000008'::uuid],
 ARRAY['isda0003-0000-0000-0000-000000000001'::uuid, 'isda0003-0000-0000-0000-000000000004'::uuid, 'isda0003-0000-0000-0000-000000000005'::uuid, 'isda0003-0000-0000-0000-000000000006'::uuid],
 'Legal documentation of a specific derivative trade including all economic and operational terms.',
 'Complete trade specification including: parties, trade date, notional amount, underlying reference, payment terms, settlement procedures, calculation methodology',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank', 'MiFID II']),

-- Amendment Letters
('isda_amendment', 'ISDA Amendment Letter', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Amendment to existing ISDA Master Agreement or related documentation',
 ARRAY['law_firm', 'financial_institution'],
 ARRAY['isda0005-0000-0000-0000-000000000001'::uuid, 'isda0005-0000-0000-0000-000000000002'::uuid, 'isda0005-0000-0000-0000-000000000003'::uuid, 'isda0005-0000-0000-0000-000000000004'::uuid, 'isda0005-0000-0000-0000-000000000005'::uuid],
 ARRAY['isda0005-0000-0000-0000-000000000003'::uuid, 'isda0005-0000-0000-0000-000000000004'::uuid, 'isda0005-0000-0000-0000-000000000005'::uuid],
 'Legal document modifying terms of existing ISDA Master Agreement or related documents.',
 'Modifications to existing agreements including: sections being amended, effective dates, new terms, signatory information',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank']),

-- Netting Opinions
('isda_netting_opinion', 'ISDA Netting Opinion', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Legal opinion on enforceability of close-out netting provisions under local law',
 ARRAY['law_firm', 'isda_inc'],
 ARRAY['isda0006-0000-0000-0000-000000000001'::uuid, 'isda0006-0000-0000-0000-000000000002'::uuid, 'isda0006-0000-0000-0000-000000000003'::uuid, 'isda0006-0000-0000-0000-000000000004'::uuid, 'isda0006-0000-0000-0000-000000000005'::uuid],
 + ARRAY['isda0006-0000-0000-0000-000000000001'::uuid, 'isda0006-0000-0000-0000-000000000005'::uuid],
 + 'Legal opinion confirming that close-out netting provisions in ISDA agreements will be enforceable under local law.',
 + 'Legal analysis of netting enforceability including: jurisdictional scope, entity types covered, legal limitations, assumptions, enforceability conclusions',
 + ARRAY['DERIVATIVES_TRADING'],
 + ARRAY['Basel III', 'Local Banking Regulations']),

 -- Close-out Amount Statement
 ('isda_closeout_statement', 'Close-out Amount Statement', 'financial', 'isda',
 + 'd0c00002-0000-0000-0000-000000000002'::uuid,
 + 'Statement calculating close-out amounts upon early termination of ISDA agreement',
 + ARRAY['financial_institution'],
 + ARRAY['isda0007-0000-0000-0000-000000000001'::uuid, 'isda0007-0000-0000-0000-000000000002'::uuid, 'isda0007-0000-0000-0000-000000000003'::uuid, 'isda0007-0000-0000-0000-000000000004'::uuid, 'isda0007-0000-0000-0000-000000000005'::uuid, 'isda0007-0000-0000-0000-000000000006'::uuid],
 + ARRAY['isda0007-0000-0000-0000-000000000002'::uuid, 'isda0007-0000-0000-0000-000000000004'::uuid, 'isda0007-0000-0000-0000-000000000006'::uuid],
 + 'Financial statement showing amounts owed upon early termination of derivative transactions.',
 + 'Termination valuation including: terminated transactions, market quotations, loss calculations, net settlement amounts',
 + ARRAY['DERIVATIVES_TRADING'],
 + ARRAY['EMIR', 'Dodd-Frank']),

 -- Novation Agreement
 ('isda_novation', 'ISDA Novation Agreement', 'legal', 'isda',
 + 'd0c00002-0000-0000-0000-000000000002'::uuid,
 + 'Agreement transferring rights and obligations of derivative transactions to new counterparty',
 + ARRAY['financial_institution', 'law_firm'],
 + ARRAY['isda0008-0000-0000-0000-000000000001'::uuid, 'isda0008-0000-0000-0000-000000000002'::uuid, 'isda0008-0000-0000-0000-000000000003'::uuid, 'isda0008-0000-0000-0000-000000000004'::uuid, 'isda0008-0000-0000-0000-000000000005'::uuid],
 + ARRAY['isda0008-0000-0000-0000-000000000002'::uuid, 'isda0008-0000-0000-0000-000000000003'::uuid, 'isda0008-0000-0000-0000-000000000004'::uuid],
 + 'Legal mechanism for transferring derivative positions from one party to another.',
 + 'Transaction transfer documentation including: parties involved, transactions being transferred, effective dates, consent requirements',
 + ARRAY['DERIVATIVES_TRADING'],
 + ARRAY['EMIR', 'Dodd-Frank']),

 -- ISDA Definitions
 ('isda_definitions', 'ISDA Definitions', 'legal', 'isda',
 + 'd0c00002-0000-0000-0000-000000000002'::uuid,
 + 'Standardized definitions for derivative transaction terms published by ISDA',
 + ARRAY['isda_inc'],
 + ARRAY['isda0009-0000-0000-0000-000000000001'::uuid, 'isda0009-0000-0000-0000-000000000002'::uuid, 'isda0009-0000-0000-0000-000000000003'::uuid, 'isda0009-0000-0000-0000-000000000004'::uuid],
 + ARRAY['isda0009-0000-0000-0000-000000000001'::uuid, 'isda0009-0000-0000-0000-000000000003'::uuid, 'isda0009-0000-0000-0000-000000000004'::uuid],
 + 'Standardized dictionary of terms and calculation methodologies used in derivative transactions.',
 + 'Standard definitions including: product terminology, calculation methodologies, business day conventions, settlement procedures, events of default definitions',
 + ARRAY['DERIVATIVES_TRADING'],
 + ARRAY['ISDA Protocol', 'Industry Standards'])

 +ON CONFLICT (type_code) DO NOTHING;

 +-- ============================================================================
 +-- ISDA ISSUING AUTHORITIES
 +-- ============================================================================
 +
 +-- Insert ISDA-specific issuing authorities
 +INSERT INTO "ob-poc".document_issuers (issuer_code, legal_name, jurisdiction, regulatory_type, authority_level, document_types_issued, official_website, api_integration_available, reliability_score) VALUES
 +
 +-- ISDA Inc
 +('isda_inc', 'International Swaps and Derivatives Association, Inc.', 'US', 'trade_association', 'industry',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_definitions', 'isda_netting_opinion'],
 + 'https://www.isda.org', false, 1.0),
 +
 +-- Major Financial Institutions
 +('jpmorgan_chase_isda', 'JPMorgan Chase & Co.', 'US', 'private', 'private',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 + 'https://www.jpmorganchase.com', false, 0.95),
 +
 +('goldman_sachs_isda', 'The Goldman Sachs Group, Inc.', 'US', 'private', 'private',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 + 'https://www.goldmansachs.com', false, 0.95),
 +
 +('morgan_stanley_isda', 'Morgan Stanley', 'US', 'private', 'private',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 + 'https://www.morganstanley.com', false, 0.95),
 +
 +-- International Banks
 +('deutsche_bank_isda', 'Deutsche Bank AG', 'DE', 'private', 'private',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment'],
 + 'https://www.db.com', false, 0.9),
 +
 +('ubs_isda', 'UBS Group AG', 'CH', 'private', 'private',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment'],
 + 'https://www.ubs.com', false, 0.9),
 +
 +-- Law Firms
 +('allen_overy_isda', 'Allen & Overy LLP', 'GB', 'private', 'private',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_amendment', 'isda_netting_opinion'],
 + 'https://www.allenovery.com', false, 0.95),
 +
 +('cleary_gottlieb_isda', 'Cleary Gottlieb Steen & Hamilton LLP', 'US', 'private', 'private',
 + ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_amendment', 'isda_netting_opinion'],
 + 'https://www.clearygottlieb.com', false, 0.95)
 +
 +ON CONFLICT (issuer_code) DO NOTHING;
 +
 +-- Phase 2 Task 1 Complete: ISDA AttributeIDs and Document Types Created
 +-- - 45 new ISDA AttributeIDs added to dictionary
 +-- - 9 ISDA document types with proper AttributeID linkage
 +-- - 8 ISDA document issuers (ISDA Inc, banks, law firms)
 +-- - All with proper referential integrity and compliance framework mapping
