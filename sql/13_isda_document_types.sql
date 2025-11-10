-- 13_isda_document_types.sql
-- ISDA Document Types - Phase 2 Task 1.2
--
-- This script creates ISDA document types with proper AttributeID references
-- using the corrected UUID format from the previous task

-- ============================================================================
-- ISDA DOCUMENT TYPES WITH ATTRIBUTEID LINKAGE
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
 ARRAY['aaaa0001-0000-0000-0000-000000000001'::uuid, 'aaaa0001-0000-0000-0000-000000000002'::uuid, 'aaaa0001-0000-0000-0000-000000000003'::uuid, 'aaaa0001-0000-0000-0000-000000000004'::uuid, 'aaaa0001-0000-0000-0000-000000000005'::uuid],
 ARRAY['aaaa0001-0000-0000-0000-000000000002'::uuid, 'aaaa0001-0000-0000-0000-000000000003'::uuid, 'aaaa0001-0000-0000-0000-000000000004'::uuid, 'aaaa0001-0000-0000-0000-000000000005'::uuid],
 'The foundational legal agreement that governs all derivative transactions between two parties. Contains standard terms and conditions, events of default, termination provisions, and dispute resolution mechanisms.',
 'Legal framework for derivatives trading including: party identification, governing law, events of default, termination events, close-out netting provisions, credit support arrangements, dispute resolution procedures',
 ARRAY['DERIVATIVES_TRADING', 'PRIME_BROKERAGE'],
 ARRAY['EMIR', 'Dodd-Frank', 'MiFID II']),

-- Credit Support Annex
('isda_csa', 'Credit Support Annex (CSA)', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Credit Support Annex defining collateral arrangements and margin requirements for derivative transactions',
 ARRAY['law_firm', 'financial_institution'],
 ARRAY['aaaa0002-0000-0000-0000-000000000001'::uuid, 'aaaa0002-0000-0000-0000-000000000002'::uuid, 'aaaa0002-0000-0000-0000-000000000003'::uuid, 'aaaa0002-0000-0000-0000-000000000004'::uuid, 'aaaa0002-0000-0000-0000-000000000006'::uuid, 'aaaa0002-0000-0000-0000-000000000008'::uuid],
 ARRAY['aaaa0002-0000-0000-0000-000000000002'::uuid, 'aaaa0002-0000-0000-0000-000000000003'::uuid, 'aaaa0002-0000-0000-0000-000000000004'::uuid, 'aaaa0002-0000-0000-0000-000000000006'::uuid],
 'Legal document defining how collateral will be posted and managed to secure derivative exposures between parties.',
 'Collateral management framework including: threshold amounts, minimum transfer amounts, eligible collateral types, valuation methodology, margin call procedures, dispute resolution for collateral',
 ARRAY['DERIVATIVES_TRADING', 'PRIME_BROKERAGE'],
 ARRAY['EMIR', 'Basel III', 'UMR']),

-- Schedule to Master Agreement
('isda_schedule', 'Schedule to ISDA Master Agreement', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Schedule containing party-specific elections and modifications to the standard ISDA Master Agreement terms',
 ARRAY['law_firm', 'financial_institution'],
 ARRAY['aaaa0004-0000-0000-0000-000000000001'::uuid, 'aaaa0004-0000-0000-0000-000000000002'::uuid, 'aaaa0004-0000-0000-0000-000000000003'::uuid, 'aaaa0004-0000-0000-0000-000000000004'::uuid, 'aaaa0004-0000-0000-0000-000000000005'::uuid],
 ARRAY['aaaa0004-0000-0000-0000-000000000001'::uuid, 'aaaa0004-0000-0000-0000-000000000005'::uuid],
 'Customized terms and elections that modify the standard ISDA Master Agreement to reflect the specific relationship between the parties.',
 'Party-specific modifications including: termination events, credit support arrangements, governing law elections, cross-default provisions, merger events, early termination procedures',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank']),

-- Trade Confirmation
('isda_confirmation', 'Trade Confirmation', 'financial', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Legal confirmation of specific derivative transaction terms executed under ISDA Master Agreement',
 ARRAY['financial_institution', 'broker_dealer'],
 ARRAY['aaaa0003-0000-0000-0000-000000000001'::uuid, 'aaaa0003-0000-0000-0000-000000000002'::uuid, 'aaaa0003-0000-0000-0000-000000000003'::uuid, 'aaaa0003-0000-0000-0000-000000000004'::uuid, 'aaaa0003-0000-0000-0000-000000000005'::uuid, 'aaaa0003-0000-0000-0000-000000000006'::uuid, 'aaaa0003-0000-0000-0000-000000000007'::uuid, 'aaaa0003-0000-0000-0000-000000000008'::uuid],
 ARRAY['aaaa0003-0000-0000-0000-000000000001'::uuid, 'aaaa0003-0000-0000-0000-000000000004'::uuid, 'aaaa0003-0000-0000-0000-000000000005'::uuid, 'aaaa0003-0000-0000-0000-000000000006'::uuid],
 'Legal documentation of a specific derivative trade including all economic and operational terms.',
 'Complete trade specification including: parties, trade date, notional amount, underlying reference, payment terms, settlement procedures, calculation methodology',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank', 'MiFID II']),

-- Amendment Letters
('isda_amendment', 'ISDA Amendment Letter', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Amendment to existing ISDA Master Agreement or related documentation',
 ARRAY['law_firm', 'financial_institution'],
 ARRAY['aaaa0005-0000-0000-0000-000000000001'::uuid, 'aaaa0005-0000-0000-0000-000000000002'::uuid, 'aaaa0005-0000-0000-0000-000000000003'::uuid, 'aaaa0005-0000-0000-0000-000000000004'::uuid, 'aaaa0005-0000-0000-0000-000000000005'::uuid],
 ARRAY['aaaa0005-0000-0000-0000-000000000003'::uuid, 'aaaa0005-0000-0000-0000-000000000004'::uuid, 'aaaa0005-0000-0000-0000-000000000005'::uuid],
 'Legal document modifying terms of existing ISDA Master Agreement or related documents.',
 'Modifications to existing agreements including: sections being amended, effective dates, new terms, signatory information',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank']),

-- Netting Opinions
('isda_netting_opinion', 'ISDA Netting Opinion', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Legal opinion on enforceability of close-out netting provisions under local law',
 ARRAY['law_firm', 'isda_inc'],
 ARRAY['aaaa0006-0000-0000-0000-000000000001'::uuid, 'aaaa0006-0000-0000-0000-000000000002'::uuid, 'aaaa0006-0000-0000-0000-000000000003'::uuid, 'aaaa0006-0000-0000-0000-000000000004'::uuid, 'aaaa0006-0000-0000-0000-000000000005'::uuid],
 ARRAY['aaaa0006-0000-0000-0000-000000000001'::uuid, 'aaaa0006-0000-0000-0000-000000000005'::uuid],
 'Legal opinion confirming that close-out netting provisions in ISDA agreements will be enforceable under local law.',
 'Legal analysis of netting enforceability including: jurisdictional scope, entity types covered, legal limitations, assumptions, enforceability conclusions',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['Basel III', 'Local Banking Regulations']),

-- Close-out Amount Statement
('isda_closeout_statement', 'Close-out Amount Statement', 'financial', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Statement calculating close-out amounts upon early termination of ISDA agreement',
 ARRAY['financial_institution'],
 ARRAY['aaaa0007-0000-0000-0000-000000000001'::uuid, 'aaaa0007-0000-0000-0000-000000000002'::uuid, 'aaaa0007-0000-0000-0000-000000000003'::uuid, 'aaaa0007-0000-0000-0000-000000000004'::uuid, 'aaaa0007-0000-0000-0000-000000000005'::uuid, 'aaaa0007-0000-0000-0000-000000000006'::uuid],
 ARRAY['aaaa0007-0000-0000-0000-000000000002'::uuid, 'aaaa0007-0000-0000-0000-000000000004'::uuid, 'aaaa0007-0000-0000-0000-000000000006'::uuid],
 'Financial statement showing amounts owed upon early termination of derivative transactions.',
 'Termination valuation including: terminated transactions, market quotations, loss calculations, net settlement amounts',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank']),

-- Novation Agreement
('isda_novation', 'ISDA Novation Agreement', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Agreement transferring rights and obligations of derivative transactions to new counterparty',
 ARRAY['financial_institution', 'law_firm'],
 ARRAY['aaaa0008-0000-0000-0000-000000000001'::uuid, 'aaaa0008-0000-0000-0000-000000000002'::uuid, 'aaaa0008-0000-0000-0000-000000000003'::uuid, 'aaaa0008-0000-0000-0000-000000000004'::uuid, 'aaaa0008-0000-0000-0000-000000000005'::uuid],
 ARRAY['aaaa0008-0000-0000-0000-000000000002'::uuid, 'aaaa0008-0000-0000-0000-000000000003'::uuid, 'aaaa0008-0000-0000-0000-000000000004'::uuid],
 'Legal mechanism for transferring derivative positions from one party to another.',
 'Transaction transfer documentation including: parties involved, transactions being transferred, effective dates, consent requirements',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['EMIR', 'Dodd-Frank']),

-- ISDA Definitions
('isda_definitions', 'ISDA Definitions', 'legal', 'isda',
 'd0c00002-0000-0000-0000-000000000002'::uuid,
 'Standardized definitions for derivative transaction terms published by ISDA',
 ARRAY['isda_inc'],
 ARRAY['aaaa0009-0000-0000-0000-000000000001'::uuid, 'aaaa0009-0000-0000-0000-000000000002'::uuid, 'aaaa0009-0000-0000-0000-000000000003'::uuid, 'aaaa0009-0000-0000-0000-000000000004'::uuid],
 ARRAY['aaaa0009-0000-0000-0000-000000000001'::uuid, 'aaaa0009-0000-0000-0000-000000000003'::uuid, 'aaaa0009-0000-0000-0000-000000000004'::uuid],
 'Standardized dictionary of terms and calculation methodologies used in derivative transactions.',
 'Standard definitions including: product terminology, calculation methodologies, business day conventions, settlement procedures, events of default definitions',
 ARRAY['DERIVATIVES_TRADING'],
 ARRAY['ISDA Protocol', 'Industry Standards'])

ON CONFLICT (type_code) DO NOTHING;

-- ============================================================================
-- ISDA ISSUING AUTHORITIES
-- ============================================================================

-- Insert ISDA-specific issuing authorities
INSERT INTO "ob-poc".document_issuers (issuer_code, legal_name, jurisdiction, regulatory_type, authority_level, document_types_issued, official_website, api_integration_available, reliability_score) VALUES

-- ISDA Inc
('isda_inc', 'International Swaps and Derivatives Association, Inc.', 'US', 'trade_association', 'industry',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_definitions', 'isda_netting_opinion'],
 'https://www.isda.org', false, 1.0),

-- Major Financial Institutions
('jpmorgan_chase_isda', 'JPMorgan Chase & Co.', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 'https://www.jpmorganchase.com', false, 0.95),

('goldman_sachs_isda', 'The Goldman Sachs Group, Inc.', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 'https://www.goldmansachs.com', false, 0.95),

('morgan_stanley_isda', 'Morgan Stanley', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment', 'isda_closeout_statement'],
 'https://www.morganstanley.com', false, 0.95),

-- International Banks
('deutsche_bank_isda', 'Deutsche Bank AG', 'DE', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment'],
 'https://www.db.com', false, 0.9),

('ubs_isda', 'UBS Group AG', 'CH', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_confirmation', 'isda_amendment'],
 'https://www.ubs.com', false, 0.9),

-- Law Firms
('allen_overy_isda', 'Allen & Overy LLP', 'GB', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_amendment', 'isda_netting_opinion'],
 'https://www.allenovery.com', false, 0.95),

('cleary_gottlieb_isda', 'Cleary Gottlieb Steen & Hamilton LLP', 'US', 'private', 'private',
 ARRAY['isda_master_agreement', 'isda_csa', 'isda_schedule', 'isda_amendment', 'isda_netting_opinion'],
 'https://www.clearygottlieb.com', false, 0.95)

ON CONFLICT (issuer_code) DO NOTHING;

-- Phase 2 Task 1.2 Complete: ISDA Document Types Created
-- - 9 ISDA document types with proper AttributeID linkage
-- - 8 ISDA document issuers (ISDA Inc, banks, law firms)
-- - All with proper referential integrity and compliance framework mapping
-- - Ready for sample document creation and ISDA DSL verbs
