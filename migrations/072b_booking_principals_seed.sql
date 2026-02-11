-- Booking Principals — Seed Data (idempotent)
-- Depends on: 072_booking_principals.sql
--
-- Seeds:
--   5 legal entities (BNY operating entities)
--   5 booking locations (LU, GB, KY, SG, US)
--   5 booking principals (one per LE+location)
--   Extend products with product_family
--   Extend services with lifecycle_tags
--   Service availability (three-lane records)
--   Rulesets + rules (regulatory, commercial, operational)
--   Contract packs + templates
--   Client-principal relationships

BEGIN;

-- ============================================================================
-- Legal Entities
-- ============================================================================

INSERT INTO "ob-poc".legal_entity (legal_entity_id, name, incorporation_jurisdiction, lei, status)
VALUES
    ('a1000000-0000-0000-0000-000000000001', 'BNY Mellon SA/NV', 'LU', '549300JB4DQ85LFAM292', 'active'),
    ('a1000000-0000-0000-0000-000000000002', 'BNY Mellon Fund Services (Ireland) DAC', 'GB', '635400QCXIQ5ISK9WS73', 'active'),
    ('a1000000-0000-0000-0000-000000000003', 'BNY Mellon International Ltd (Cayman)', 'KY', NULL, 'active'),
    ('a1000000-0000-0000-0000-000000000004', 'BNY Mellon Singapore Pte Ltd', 'SG', '549300L1KW4QJLKGS237', 'active'),
    ('a1000000-0000-0000-0000-000000000005', 'The Bank of New York Mellon', 'US', 'HPFHU0OQ28E4N0NFVK49', 'active')
ON CONFLICT (legal_entity_id) DO NOTHING;

-- ============================================================================
-- Booking Locations
-- ============================================================================

INSERT INTO "ob-poc".booking_location (booking_location_id, country_code, jurisdiction_code, regulatory_regime_tags)
VALUES
    ('b1000000-0000-0000-0000-000000000001', 'LU', 'LU', ARRAY['MiFID_II', 'UCITS', 'AIFMD', 'CSSF']),
    ('b1000000-0000-0000-0000-000000000002', 'GB', 'GB', ARRAY['FCA', 'PRA', 'MiFID_II']),
    ('b1000000-0000-0000-0000-000000000003', 'KY', 'KY', ARRAY['CIMA', 'SIBL']),
    ('b1000000-0000-0000-0000-000000000004', 'SG', 'SG', ARRAY['MAS', 'SFA']),
    ('b1000000-0000-0000-0000-000000000005', 'US', 'US', ARRAY['SEC', 'CFTC', 'FINRA', 'OCC'])
ON CONFLICT (booking_location_id) DO NOTHING;

-- ============================================================================
-- Booking Principals (LE + Location envelopes)
-- ============================================================================

INSERT INTO "ob-poc".booking_principal
    (booking_principal_id, legal_entity_id, booking_location_id, principal_code, book_code, status)
VALUES
    ('c1000000-0000-0000-0000-000000000001',
     'a1000000-0000-0000-0000-000000000001',
     'b1000000-0000-0000-0000-000000000001',
     'BNYM-LU', 'LU-01', 'active'),
    ('c1000000-0000-0000-0000-000000000002',
     'a1000000-0000-0000-0000-000000000002',
     'b1000000-0000-0000-0000-000000000002',
     'BNYM-GB', 'GB-01', 'active'),
    ('c1000000-0000-0000-0000-000000000003',
     'a1000000-0000-0000-0000-000000000003',
     'b1000000-0000-0000-0000-000000000003',
     'BNYM-KY', 'KY-01', 'active'),
    ('c1000000-0000-0000-0000-000000000004',
     'a1000000-0000-0000-0000-000000000004',
     'b1000000-0000-0000-0000-000000000004',
     'BNYM-SG', 'SG-01', 'active'),
    ('c1000000-0000-0000-0000-000000000005',
     'a1000000-0000-0000-0000-000000000005',
     'b1000000-0000-0000-0000-000000000005',
     'BNYM-US', 'US-01', 'active')
ON CONFLICT (booking_principal_id) DO NOTHING;

-- ============================================================================
-- Extend Products with product_family
-- ============================================================================

UPDATE "ob-poc".products SET product_family = 'custody_services'
WHERE product_code = 'CUSTODY';

UPDATE "ob-poc".products SET product_family = 'fund_services'
WHERE product_code = 'TRANSFER_AGENCY';

UPDATE "ob-poc".products SET product_family = 'fund_services'
WHERE product_code = 'FUND_ACCOUNTING';

UPDATE "ob-poc".products SET product_family = 'markets'
WHERE product_code = 'MARKETS_FX';

UPDATE "ob-poc".products SET product_family = 'middle_office'
WHERE product_code = 'MIDDLE_OFFICE';

UPDATE "ob-poc".products SET product_family = 'collateral'
WHERE product_code = 'COLLATERAL_MGMT';

UPDATE "ob-poc".products SET product_family = 'alternatives'
WHERE product_code = 'ALTS';

-- ============================================================================
-- Extend Services with lifecycle_tags
-- ============================================================================

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['core', 'regulatory']
WHERE service_code = 'SAFEKEEPING';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['core']
WHERE service_code = 'CASH_MGMT';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['core']
WHERE service_code = 'SETTLEMENT';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['reporting', 'regulatory']
WHERE service_code = 'REPORTING';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['corporate_actions']
WHERE service_code = 'CORP_ACTIONS';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['core', 'valuation']
WHERE service_code = 'NAV_CALC';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['reporting', 'regulatory']
WHERE service_code = 'REG_REPORTING';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['investor_services']
WHERE service_code = 'INVESTOR_ACCT';

UPDATE "ob-poc".services SET lifecycle_tags = ARRAY['investor_services', 'regulatory']
WHERE service_code = 'INVESTOR_REG';

-- ============================================================================
-- Service Availability (three-lane per principal × service)
-- ============================================================================

-- BNYM-LU: Full custody + fund services
INSERT INTO "ob-poc".service_availability
    (booking_principal_id, service_id, regulatory_status, commercial_status, operational_status, delivery_model)
VALUES
    -- Safekeeping
    ('c1000000-0000-0000-0000-000000000001', '42c49225-1a06-4416-99f0-ae89cebd8f8f',
     'permitted', 'offered', 'supported', 'direct'),
    -- Cash Management
    ('c1000000-0000-0000-0000-000000000001', '22a021c0-e169-46cd-b4da-41c9b2c1cade',
     'permitted', 'offered', 'supported', 'direct'),
    -- Settlement
    ('c1000000-0000-0000-0000-000000000001', 'b1b1cf13-369a-448e-aae6-24dfbb6ed739',
     'permitted', 'offered', 'supported', 'direct'),
    -- NAV Calc
    ('c1000000-0000-0000-0000-000000000001', '7c536c3f-d475-4fa1-b665-f05e3dbd4e45',
     'permitted', 'offered', 'supported', 'direct'),
    -- Investor Accounting
    ('c1000000-0000-0000-0000-000000000001', '71084961-a94f-438f-a5b7-bd7b5a425469',
     'permitted', 'offered', 'supported', 'direct'),
    -- Regulatory Reporting
    ('c1000000-0000-0000-0000-000000000001', 'e75cef50-8f2e-4260-8a18-4f63cab526dc',
     'permitted', 'offered', 'supported', 'direct')
ON CONFLICT DO NOTHING;

-- BNYM-GB: Full custody, limited TA
INSERT INTO "ob-poc".service_availability
    (booking_principal_id, service_id, regulatory_status, commercial_status, operational_status, delivery_model)
VALUES
    ('c1000000-0000-0000-0000-000000000002', '42c49225-1a06-4416-99f0-ae89cebd8f8f',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000002', '22a021c0-e169-46cd-b4da-41c9b2c1cade',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000002', 'b1b1cf13-369a-448e-aae6-24dfbb6ed739',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000002', '71084961-a94f-438f-a5b7-bd7b5a425469',
     'permitted', 'offered', 'limited', 'partner')
ON CONFLICT DO NOTHING;

-- BNYM-KY: Custody only (offshore)
INSERT INTO "ob-poc".service_availability
    (booking_principal_id, service_id, regulatory_status, commercial_status, operational_status, delivery_model)
VALUES
    ('c1000000-0000-0000-0000-000000000003', '42c49225-1a06-4416-99f0-ae89cebd8f8f',
     'permitted', 'offered', 'supported', 'sub_custodian'),
    ('c1000000-0000-0000-0000-000000000003', '22a021c0-e169-46cd-b4da-41c9b2c1cade',
     'permitted', 'offered', 'supported', 'sub_custodian'),
    ('c1000000-0000-0000-0000-000000000003', 'b1b1cf13-369a-448e-aae6-24dfbb6ed739',
     'permitted', 'offered', 'supported', 'partner')
ON CONFLICT DO NOTHING;

-- BNYM-SG: Custody + limited services
INSERT INTO "ob-poc".service_availability
    (booking_principal_id, service_id, regulatory_status, commercial_status, operational_status, delivery_model)
VALUES
    ('c1000000-0000-0000-0000-000000000004', '42c49225-1a06-4416-99f0-ae89cebd8f8f',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000004', '22a021c0-e169-46cd-b4da-41c9b2c1cade',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000004', 'b1b1cf13-369a-448e-aae6-24dfbb6ed739',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000004', '7c536c3f-d475-4fa1-b665-f05e3dbd4e45',
     'permitted', 'not_offered', 'not_supported', NULL)
ON CONFLICT DO NOTHING;

-- BNYM-US: Full services
INSERT INTO "ob-poc".service_availability
    (booking_principal_id, service_id, regulatory_status, commercial_status, operational_status, delivery_model)
VALUES
    ('c1000000-0000-0000-0000-000000000005', '42c49225-1a06-4416-99f0-ae89cebd8f8f',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000005', '22a021c0-e169-46cd-b4da-41c9b2c1cade',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000005', 'b1b1cf13-369a-448e-aae6-24dfbb6ed739',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000005', '7c536c3f-d475-4fa1-b665-f05e3dbd4e45',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000005', 'a41be122-5c04-4944-9976-fdc656e25578',
     'permitted', 'offered', 'supported', 'direct'),
    ('c1000000-0000-0000-0000-000000000005', '0e12b362-c4e1-47d2-90f4-51ca0fddc1bf',
     'permitted', 'offered', 'supported', 'direct')
ON CONFLICT DO NOTHING;

-- ============================================================================
-- Rulesets + Rules
-- ============================================================================

-- Global regulatory ruleset: sanctions + classification
INSERT INTO "ob-poc".ruleset
    (ruleset_id, owner_type, owner_id, name, ruleset_boundary, status)
VALUES
    ('d1000000-0000-0000-0000-000000000001', 'global', NULL,
     'Global Regulatory Baseline', 'regulatory', 'active')
ON CONFLICT (ruleset_id) DO NOTHING;

INSERT INTO "ob-poc".rule (ruleset_id, name, kind, priority, when_expr, then_effect, explain)
VALUES
    -- Sanctions deny
    ('d1000000-0000-0000-0000-000000000001', 'Sanctions check',
     'deny', 10,
     '{"field": {"field": "client.risk_flags.sanctions", "op": "eq", "value": true}}',
     '{"action": "deny", "reason_code": "SANCTIONS_HIT", "reason": "Client domicile or entity under active sanctions"}',
     'Clients flagged with sanctions are prohibited from all booking principals'),
    -- Non-professional retail deny
    ('d1000000-0000-0000-0000-000000000001', 'Retail client restriction',
     'deny', 20,
     '{"all": [{"field": {"field": "client.classification.mifid_ii", "op": "eq", "value": "retail_client"}}, {"field": {"field": "client.segment", "op": "neq", "value": "wealth"}}]}',
     '{"action": "deny", "reason_code": "RETAIL_PROHIBITED", "reason": "Non-wealth retail clients cannot be booked"}',
     'Only wealth management retail clients are permitted')
ON CONFLICT DO NOTHING;

-- Custody offering commercial ruleset
INSERT INTO "ob-poc".ruleset
    (ruleset_id, owner_type, owner_id, name, ruleset_boundary, status)
VALUES
    ('d1000000-0000-0000-0000-000000000002', 'offering', '15244192-0e29-4cd4-8d3b-ec19488ad814',
     'Custody Commercial Rules', 'commercial', 'active')
ON CONFLICT (ruleset_id) DO NOTHING;

INSERT INTO "ob-poc".rule (ruleset_id, name, kind, priority, when_expr, then_effect, explain)
VALUES
    -- EU pension gate
    ('d1000000-0000-0000-0000-000000000002', 'EU pension credit approval',
     'require_gate', 50,
     '{"all": [{"field": {"field": "client.segment", "op": "in", "value": ["pension", "sovereign"]}}, {"field": {"field": "client.domicile_country", "op": "in", "value": ["LU", "DE", "FR", "NL", "BE", "IE"]}}]}',
     '{"action": "require_gate", "gate": "credit_approval", "severity": "blocking"}',
     'EU pension/sovereign custody clients require credit committee approval'),
    -- Contract pack selection for EU custody
    ('d1000000-0000-0000-0000-000000000002', 'EU custody contract pack',
     'select_contract', 60,
     '{"field": {"field": "client.domicile_country", "op": "in", "value": ["LU", "DE", "FR", "NL", "BE", "IE", "GB"]}}',
     '{"action": "select_contract", "contract_pack_code": "EU_CUSTODY", "template_types": ["custody_agreement", "sub_custody_schedule"]}',
     'European domiciled clients use EU custody contract pack')
ON CONFLICT DO NOTHING;

-- BNYM-LU operational ruleset
INSERT INTO "ob-poc".ruleset
    (ruleset_id, owner_type, owner_id, name, ruleset_boundary, status)
VALUES
    ('d1000000-0000-0000-0000-000000000003', 'principal', 'c1000000-0000-0000-0000-000000000001',
     'BNYM-LU Operational Rules', 'operational', 'active')
ON CONFLICT (ruleset_id) DO NOTHING;

INSERT INTO "ob-poc".rule (ruleset_id, name, kind, priority, when_expr, then_effect, explain)
VALUES
    -- KYC gate for non-EU domiciled
    ('d1000000-0000-0000-0000-000000000003', 'Non-EU KYC enhanced',
     'require_gate', 70,
     '{"field": {"field": "client.domicile_country", "op": "not_in", "value": ["LU", "DE", "FR", "NL", "BE", "IE", "GB", "IT", "ES", "PT", "AT"]}}',
     '{"action": "require_gate", "gate": "enhanced_due_diligence", "severity": "blocking"}',
     'Non-EU clients booked in Luxembourg require enhanced KYC due diligence')
ON CONFLICT DO NOTHING;

-- Transfer Agency offering commercial ruleset
INSERT INTO "ob-poc".ruleset
    (ruleset_id, owner_type, owner_id, name, ruleset_boundary, status)
VALUES
    ('d1000000-0000-0000-0000-000000000004', 'offering', '3e027380-ca07-41bf-a9c8-66606f338065',
     'Transfer Agency Commercial Rules', 'commercial', 'active')
ON CONFLICT (ruleset_id) DO NOTHING;

INSERT INTO "ob-poc".rule (ruleset_id, name, kind, priority, when_expr, then_effect, explain)
VALUES
    ('d1000000-0000-0000-0000-000000000004', 'TA contract pack',
     'select_contract', 50,
     '{"field": {"field": "client.domicile_country", "op": "in", "value": ["LU", "DE", "FR", "NL", "BE", "IE"]}}',
     '{"action": "select_contract", "contract_pack_code": "EU_TA", "template_types": ["ta_agreement", "registrar_schedule"]}',
     'European domiciled clients use EU TA contract pack'),
    ('d1000000-0000-0000-0000-000000000004', 'Non-fund entity restriction',
     'deny', 30,
     '{"field": {"field": "client.entity_types", "op": "contains", "value": "individual"}}',
     '{"action": "deny", "reason_code": "TA_ENTITY_TYPE", "reason": "Transfer Agency not available for individual entities"}',
     'TA services are only for fund/institutional entities')
ON CONFLICT DO NOTHING;

-- Global commercial: PEP advisory gate
INSERT INTO "ob-poc".ruleset
    (ruleset_id, owner_type, owner_id, name, ruleset_boundary, status)
VALUES
    ('d1000000-0000-0000-0000-000000000005', 'global', NULL,
     'Global AML/KYC Gates', 'commercial', 'active')
ON CONFLICT (ruleset_id) DO NOTHING;

INSERT INTO "ob-poc".rule (ruleset_id, name, kind, priority, when_expr, then_effect, explain)
VALUES
    ('d1000000-0000-0000-0000-000000000005', 'PEP advisory gate',
     'require_gate', 40,
     '{"field": {"field": "client.risk_flags.pep", "op": "eq", "value": true}}',
     '{"action": "require_gate", "gate": "pep_review", "severity": "advisory"}',
     'Politically exposed person flag triggers advisory review gate')
ON CONFLICT DO NOTHING;

-- ============================================================================
-- Contract Packs + Templates
-- ============================================================================

INSERT INTO "ob-poc".contract_pack (contract_pack_id, code, name, description)
VALUES
    ('e1000000-0000-0000-0000-000000000001', 'EU_CUSTODY',
     'European Custody Pack', 'Standard contract set for EU custody clients'),
    ('e1000000-0000-0000-0000-000000000002', 'EU_TA',
     'European Transfer Agency Pack', 'Standard TA contract set for EU fund clients'),
    ('e1000000-0000-0000-0000-000000000003', 'ISDA_CSA',
     'ISDA Master + CSA', 'OTC derivatives documentation pack'),
    ('e1000000-0000-0000-0000-000000000004', 'US_CUSTODY',
     'US Custody Pack', 'Standard contract set for US custody clients')
ON CONFLICT (contract_pack_id) DO NOTHING;

INSERT INTO "ob-poc".contract_template (contract_pack_id, template_type, template_ref)
VALUES
    ('e1000000-0000-0000-0000-000000000001', 'custody_agreement', 'TPL-EU-CUST-001'),
    ('e1000000-0000-0000-0000-000000000001', 'sub_custody_schedule', 'TPL-EU-SUBCUST-001'),
    ('e1000000-0000-0000-0000-000000000001', 'fee_schedule', 'TPL-EU-FEE-001'),
    ('e1000000-0000-0000-0000-000000000002', 'ta_agreement', 'TPL-EU-TA-001'),
    ('e1000000-0000-0000-0000-000000000002', 'registrar_schedule', 'TPL-EU-REG-001'),
    ('e1000000-0000-0000-0000-000000000003', 'isda_master', 'TPL-ISDA-2002'),
    ('e1000000-0000-0000-0000-000000000003', 'csa', 'TPL-CSA-VM-001'),
    ('e1000000-0000-0000-0000-000000000004', 'custody_agreement', 'TPL-US-CUST-001'),
    ('e1000000-0000-0000-0000-000000000004', 'fee_schedule', 'TPL-US-FEE-001')
ON CONFLICT DO NOTHING;

-- ============================================================================
-- Client-Principal Relationships (link to existing client groups)
-- ============================================================================

-- Allianz → BNYM-LU (Custody)
INSERT INTO "ob-poc".client_principal_relationship
    (client_group_id, booking_principal_id, product_offering_id, contract_ref, onboarded_at)
VALUES
    ('11111111-1111-1111-1111-111111111111',
     'c1000000-0000-0000-0000-000000000001',
     '15244192-0e29-4cd4-8d3b-ec19488ad814',
     'MSA-ALZ-LU-2023', '2023-06-15')
ON CONFLICT DO NOTHING;

-- Allianz → BNYM-LU (Fund Accounting)
INSERT INTO "ob-poc".client_principal_relationship
    (client_group_id, booking_principal_id, product_offering_id, contract_ref, onboarded_at)
VALUES
    ('11111111-1111-1111-1111-111111111111',
     'c1000000-0000-0000-0000-000000000001',
     '7d263b9a-8918-47d4-b469-c1d4fc84b529',
     'MSA-ALZ-LU-2023', '2023-06-15')
ON CONFLICT DO NOTHING;

-- Aviva → BNYM-GB (Custody)
INSERT INTO "ob-poc".client_principal_relationship
    (client_group_id, booking_principal_id, product_offering_id, contract_ref, onboarded_at)
VALUES
    ('22222222-2222-2222-2222-222222222222',
     'c1000000-0000-0000-0000-000000000002',
     '15244192-0e29-4cd4-8d3b-ec19488ad814',
     'MSA-AVI-GB-2022', '2022-09-01')
ON CONFLICT DO NOTHING;

-- BlackRock → BNYM-US (Custody)
INSERT INTO "ob-poc".client_principal_relationship
    (client_group_id, booking_principal_id, product_offering_id, contract_ref, onboarded_at)
VALUES
    ('33333333-3333-3333-3333-333333333333',
     'c1000000-0000-0000-0000-000000000005',
     '15244192-0e29-4cd4-8d3b-ec19488ad814',
     'MSA-BLK-US-2021', '2021-01-10')
ON CONFLICT DO NOTHING;

-- BlackRock → BNYM-LU (Transfer Agency)
INSERT INTO "ob-poc".client_principal_relationship
    (client_group_id, booking_principal_id, product_offering_id, contract_ref, onboarded_at)
VALUES
    ('33333333-3333-3333-3333-333333333333',
     'c1000000-0000-0000-0000-000000000001',
     '3e027380-ca07-41bf-a9c8-66606f338065',
     'MSA-BLK-LU-2022', '2022-03-20')
ON CONFLICT DO NOTHING;

COMMIT;
