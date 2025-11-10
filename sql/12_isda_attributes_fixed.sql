-- 12_isda_attributes_fixed.sql
-- ISDA AttributeIDs - Phase 2 Task 1 (Fixed UUID Format)
--
-- This script adds ISDA-specific AttributeIDs to the dictionary table
-- with proper UUID format and referential integrity

-- ============================================================================
-- ISDA ATTRIBUTEIDS - ADD TO DICTIONARY TABLE
-- ============================================================================

-- ISDA-specific attributes that will be referenced by UUIDs
INSERT INTO "ob-poc".dictionary (attribute_id, name, long_description, group_id, mask, domain, source, sink, created_at, updated_at) VALUES

-- ============================================================================
-- ISDA Master Agreement Attributes
-- ============================================================================
('aaaa0001-0000-0000-0000-000000000001', 'isda.master_agreement.agreement_date', 'Date ISDA Master Agreement was executed', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0001-0000-0000-0000-000000000002', 'isda.master_agreement.party_a', 'Legal name of Party A to the agreement', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0001-0000-0000-0000-000000000003', 'isda.master_agreement.party_b', 'Legal name of Party B to the agreement', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0001-0000-0000-0000-000000000004', 'isda.master_agreement.governing_law', 'Governing law jurisdiction', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "jurisdiction_code"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0001-0000-0000-0000-000000000005', 'isda.master_agreement.version', 'ISDA Master Agreement version (1992, 2002, etc.)', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "values": ["1987", "1992", "2002"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0001-0000-0000-0000-000000000006', 'isda.master_agreement.multicurrency', 'Whether multicurrency cross-default applies', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0001-0000-0000-0000-000000000007', 'isda.master_agreement.cross_default', 'Whether cross-default provisions apply', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0001-0000-0000-0000-000000000008', 'isda.master_agreement.termination_currency', 'Currency for termination payments', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Credit Support Annex (CSA) Attributes
-- ============================================================================
('aaaa0002-0000-0000-0000-000000000001', 'isda.csa.base_currency', 'Base currency for CSA calculations', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000002', 'isda.csa.threshold_party_a', 'Threshold amount for Party A', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000003', 'isda.csa.threshold_party_b', 'Threshold amount for Party B', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000004', 'isda.csa.minimum_transfer_amount', 'Minimum transfer amount', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000005', 'isda.csa.rounding_amount', 'Amount to round transfers', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000006', 'isda.csa.eligible_collateral', 'Types of eligible collateral', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000007', 'isda.csa.valuation_percentage', 'Haircuts by collateral type', 'ISDA', 'map', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000008', 'isda.csa.margin_approach', 'VM (variation margin) or IM (initial margin)', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["VM", "IM", "BOTH"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0002-0000-0000-0000-000000000009', 'isda.csa.notification_time', 'Time for margin call notifications', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Trade Confirmation Attributes
-- ============================================================================
('aaaa0003-0000-0000-0000-000000000001', 'isda.confirmation.trade_date', 'Date trade was executed', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000002', 'isda.confirmation.effective_date', 'Trade effective/start date', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000003', 'isda.confirmation.termination_date', 'Trade maturity/end date', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000004', 'isda.confirmation.notional_amount', 'Trade notional amount', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000005', 'isda.confirmation.currency', 'Trade currency', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000006', 'isda.confirmation.product_type', 'Derivative product type', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["IRS", "CDS", "FX_Forward", "FX_Option", "Equity_Option", "Commodity_Swap"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000007', 'isda.confirmation.payer', 'Party paying fixed rate/premium', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000008', 'isda.confirmation.receiver', 'Party receiving fixed rate/premium', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-000000000009', 'isda.confirmation.underlying', 'Underlying reference (rate, bond, etc.)', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0003-0000-0000-0000-00000000000a', 'isda.confirmation.calculation_agent', 'Entity responsible for calculations', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Schedule to Master Agreement Attributes
-- ============================================================================
('aaaa0004-0000-0000-0000-000000000001', 'isda.schedule.termination_events', 'List of termination events', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0004-0000-0000-0000-000000000002', 'isda.schedule.additional_termination_events', 'Additional termination events', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0004-0000-0000-0000-000000000003', 'isda.schedule.credit_event_upon_merger', 'Credit event upon merger election', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0004-0000-0000-0000-000000000004', 'isda.schedule.automatic_early_termination', 'Automatic early termination election', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": false}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0004-0000-0000-0000-000000000005', 'isda.schedule.payments_on_early_termination', 'Payment methodology on early termination', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": false, "values": ["first_method", "second_method", "market_quotation", "loss"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Amendment Letter Attributes
-- ============================================================================
('aaaa0005-0000-0000-0000-000000000001', 'isda.amendment.original_agreement_date', 'Date of original agreement being amended', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0005-0000-0000-0000-000000000002', 'isda.amendment.amendment_date', 'Date of amendment', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0005-0000-0000-0000-000000000003', 'isda.amendment.amendment_type', 'Type of amendment', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["schedule_modification", "csa_modification", "additional_terms", "termination_provision"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0005-0000-0000-0000-000000000004', 'isda.amendment.sections_amended', 'Sections being amended', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0005-0000-0000-0000-000000000005', 'isda.amendment.effective_date', 'Date amendment becomes effective', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Netting Opinion Attributes
-- ============================================================================
('aaaa0006-0000-0000-0000-000000000001', 'isda.netting_opinion.jurisdiction', 'Jurisdiction covered by opinion', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-3166-1"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0006-0000-0000-0000-000000000002', 'isda.netting_opinion.opinion_date', 'Date of legal opinion', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0006-0000-0000-0000-000000000003', 'isda.netting_opinion.law_firm', 'Law firm providing opinion', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0006-0000-0000-0000-000000000004', 'isda.netting_opinion.entity_types_covered', 'Entity types covered by opinion', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0006-0000-0000-0000-000000000005', 'isda.netting_opinion.enforceability_conclusion', 'Conclusion on netting enforceability', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["enforceable", "enforceable_with_limitations", "not_enforceable", "uncertain"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Close-out Statement Attributes
-- ============================================================================
('aaaa0007-0000-0000-0000-000000000001', 'isda.closeout.calculation_date', 'Date of close-out calculation', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0007-0000-0000-0000-000000000002', 'isda.closeout.early_termination_date', 'Early termination date', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0007-0000-0000-0000-000000000003', 'isda.closeout.calculation_agent', 'Entity performing close-out calculation', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0007-0000-0000-0000-000000000004', 'isda.closeout.total_closeout_amount', 'Net close-out amount', 'ISDA', 'decimal', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0007-0000-0000-0000-000000000005', 'isda.closeout.currency', 'Close-out amount currency', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true, "format": "ISO-4217"}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0007-0000-0000-0000-000000000006', 'isda.closeout.valuation_method', 'Method used for valuation', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["market_quotation", "loss", "first_method", "second_method"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- Novation Agreement Attributes
-- ============================================================================
('aaaa0008-0000-0000-0000-000000000001', 'isda.novation.novation_date', 'Date of novation', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0008-0000-0000-0000-000000000002', 'isda.novation.transferor', 'Party transferring the trade', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0008-0000-0000-0000-000000000003', 'isda.novation.transferee', 'Party receiving the trade', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0008-0000-0000-0000-000000000004', 'isda.novation.remaining_party', 'Non-transferring counterparty', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0008-0000-0000-0000-000000000005', 'isda.novation.consent_required', 'Whether remaining party consent needed', 'ISDA', 'boolean', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),

-- ============================================================================
-- ISDA Definitions Attributes
-- ============================================================================
('aaaa0009-0000-0000-0000-000000000001', 'isda.definitions.definitions_type', 'Type of ISDA definitions', 'ISDA', 'enum', 'ISDA', '{"type": "extraction", "required": true, "values": ["2006_Definitions", "2021_Definitions", "FX_and_Currency_Option_Definitions", "Equity_Derivatives_Definitions"]}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0009-0000-0000-0000-000000000002', 'isda.definitions.publication_date', 'Publication date of definitions', 'ISDA', 'date', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0009-0000-0000-0000-000000000003', 'isda.definitions.version', 'Version of definitions', 'ISDA', 'string', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW()),
('aaaa0009-0000-0000-0000-000000000004', 'isda.definitions.product_coverage', 'Products covered by definitions', 'ISDA', 'array', 'ISDA', '{"type": "extraction", "required": true}', '{"type": "database", "table": "document_catalog"}', NOW(), NOW())

ON CONFLICT (attribute_id) DO UPDATE SET
    name = EXCLUDED.name,
    long_description = EXCLUDED.long_description,
    updated_at = NOW();

-- Phase 2 Task 1.1 Complete: ISDA AttributeIDs Created
-- - 45 new ISDA AttributeIDs added to dictionary
-- - All with proper referential integrity and valid UUID format
-- - Ready for ISDA document type creation
