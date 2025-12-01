# CBU Seed Data & Visualization Implementation Plan

**Task**: Seed CBU examples and implement visualization views  
**Created**: 2025-12-01  
**Status**: READY FOR IMPLEMENTATION  
**Priority**: High

---

## Part 1: Seed Data for CBU Types

Create seed data covering the main CBU structures. Each should have:
- Commercial client (head office)
- ManCo (where applicable)
- Fund/Trust entity
- Share classes (CORPORATE and FUND)
- Officers (persons with roles)
- Sample ownership chain

---

### 1.1 Hedge Fund CBU

```sql
-- =============================================================================
-- HEDGE FUND: Apex Capital Partners
-- Structure: US Head Office → Cayman ManCo → Cayman Fund → Share Classes
-- =============================================================================

-- Commercial Client (Head Office)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'a1000000-0000-0000-0000-000000000001'::uuid,
    entity_type_id,
    'Apex Capital Holdings LLC'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'a1000000-0000-0000-0000-000000000101'::uuid,
    'a1000000-0000-0000-0000-000000000001'::uuid,
    'Apex Capital Holdings LLC',
    'US-DE',
    'DE-5551234'
);

-- ManCo
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'a1000000-0000-0000-0000-000000000002'::uuid,
    entity_type_id,
    'Apex Capital Management Ltd'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'a1000000-0000-0000-0000-000000000102'::uuid,
    'a1000000-0000-0000-0000-000000000002'::uuid,
    'Apex Capital Management Ltd',
    'KY',
    'KY-MC-98765'
);

-- Fund Entity
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'a1000000-0000-0000-0000-000000000003'::uuid,
    entity_type_id,
    'Apex Capital Partners LP'
FROM "ob-poc".entity_types WHERE type_code = 'partnership';

INSERT INTO "ob-poc".entity_partnerships
(partnership_id, entity_id, partnership_name, partnership_type, jurisdiction)
VALUES (
    'a1000000-0000-0000-0000-000000000103'::uuid,
    'a1000000-0000-0000-0000-000000000003'::uuid,
    'Apex Capital Partners LP',
    'LP',
    'KY'
);

-- CBU
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type, commercial_client_entity_id)
VALUES (
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'Apex Capital Partners',
    'KY',
    'HEDGE_FUND',
    'a1000000-0000-0000-0000-000000000001'::uuid
);

-- ManCo Corporate Shares (owned by Head Office)
INSERT INTO kyc.share_classes (id, cbu_id, entity_id, name, currency, class_category, 
    fund_type, management_fee_bps, performance_fee_bps, redemption_frequency, 
    redemption_notice_days, lock_up_period_months, high_water_mark)
VALUES (
    'a1000000-0000-0000-0000-000000002001'::uuid,
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'a1000000-0000-0000-0000-000000000002'::uuid,
    'ManCo Ordinary Shares',
    'USD',
    'CORPORATE',
    NULL, NULL, NULL, NULL, NULL, NULL, NULL
);

-- Fund Share Classes
INSERT INTO kyc.share_classes (id, cbu_id, entity_id, name, isin, currency, class_category,
    fund_type, fund_structure, investor_eligibility, nav_per_share, nav_frequency,
    management_fee_bps, performance_fee_bps, redemption_frequency, redemption_notice_days,
    lock_up_period_months, gate_percentage, high_water_mark, hurdle_rate, minimum_investment)
VALUES 
(
    'a1000000-0000-0000-0000-000000002002'::uuid,
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'a1000000-0000-0000-0000-000000000003'::uuid,
    'Class A - Founding Partners',
    'KYG0123456789',
    'USD',
    'FUND',
    'HEDGE_FUND', 'OPEN_ENDED', 'QUALIFIED_PURCHASER',
    1000.00, 'MONTHLY',
    150, 2000, 'QUARTERLY', 90,
    24, 25.00, true, 0.08, 5000000.00
),
(
    'a1000000-0000-0000-0000-000000002003'::uuid,
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'a1000000-0000-0000-0000-000000000003'::uuid,
    'Class B - Institutional',
    'KYG0123456790',
    'USD',
    'FUND',
    'HEDGE_FUND', 'OPEN_ENDED', 'QUALIFIED_PURCHASER',
    1000.00, 'MONTHLY',
    200, 2000, 'QUARTERLY', 60,
    12, 25.00, true, 0.08, 1000000.00
);

-- Officers (Persons)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'a1000000-0000-0000-0000-000000000010'::uuid,
    entity_type_id,
    'Marcus Chen'
FROM "ob-poc".entity_types WHERE type_code = 'proper_person';

INSERT INTO "ob-poc".entity_proper_persons
(proper_person_id, entity_id, first_name, last_name, nationality)
VALUES (
    'a1000000-0000-0000-0000-000000000110'::uuid,
    'a1000000-0000-0000-0000-000000000010'::uuid,
    'Marcus', 'Chen', 'US'
);

INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'a1000000-0000-0000-0000-000000000011'::uuid,
    entity_type_id,
    'Sarah Williams'
FROM "ob-poc".entity_types WHERE type_code = 'proper_person';

INSERT INTO "ob-poc".entity_proper_persons
(proper_person_id, entity_id, first_name, last_name, nationality)
VALUES (
    'a1000000-0000-0000-0000-000000000111'::uuid,
    'a1000000-0000-0000-0000-000000000011'::uuid,
    'Sarah', 'Williams', 'US'
);

-- Link persons to CBU with roles
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'a1000000-0000-0000-0000-000000000010'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'DIRECTOR';

INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'a1000000-0000-0000-0000-000000000010'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'UBO';

INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'a1000000-0000-0000-0000-000000000011'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'DIRECTOR';

-- Link entities to CBU
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'a1000000-0000-0000-0000-000000001000'::uuid,
    'a1000000-0000-0000-0000-000000000002'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'PRINCIPAL';

-- Holdings (ownership chain)
-- Head Office owns 100% of ManCo
INSERT INTO kyc.holdings (id, share_class_id, investor_entity_id, units, cost_basis, status)
VALUES (
    'a1000000-0000-0000-0000-000000003001'::uuid,
    'a1000000-0000-0000-0000-000000002001'::uuid,
    'a1000000-0000-0000-0000-000000000001'::uuid,
    1000, 1000000.00, 'active'
);

-- Marcus Chen owns 60% of Class A (founding partner)
INSERT INTO kyc.holdings (id, share_class_id, investor_entity_id, units, cost_basis, status)
VALUES (
    'a1000000-0000-0000-0000-000000003002'::uuid,
    'a1000000-0000-0000-0000-000000002002'::uuid,
    'a1000000-0000-0000-0000-000000000010'::uuid,
    600, 600000.00, 'active'
);
```

---

### 1.2 US 40-Act Mutual Fund CBU

```sql
-- =============================================================================
-- 40-ACT MUTUAL FUND: Pacific Growth Fund
-- Structure: US Asset Manager → US Fund → Share Classes (Retail accessible)
-- =============================================================================

-- Commercial Client (Asset Manager)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'b1000000-0000-0000-0000-000000000001'::uuid,
    entity_type_id,
    'Pacific Asset Management Inc'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'b1000000-0000-0000-0000-000000000101'::uuid,
    'b1000000-0000-0000-0000-000000000001'::uuid,
    'Pacific Asset Management Inc',
    'US-DE',
    'DE-7771234'
);

-- Fund Entity (40-Act registered)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'b1000000-0000-0000-0000-000000000002'::uuid,
    entity_type_id,
    'Pacific Growth Fund'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'b1000000-0000-0000-0000-000000000102'::uuid,
    'b1000000-0000-0000-0000-000000000002'::uuid,
    'Pacific Growth Fund',
    'US-MD',
    'SEC-811-12345'
);

-- CBU
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type, commercial_client_entity_id)
VALUES (
    'b1000000-0000-0000-0000-000000001000'::uuid,
    'Pacific Growth Fund',
    'US',
    '40_ACT',
    'b1000000-0000-0000-0000-000000000001'::uuid
);

-- Fund Share Classes (Retail accessible, daily liquidity)
INSERT INTO kyc.share_classes (id, cbu_id, entity_id, name, isin, currency, class_category,
    fund_type, fund_structure, investor_eligibility, nav_per_share, nav_frequency,
    management_fee_bps, subscription_frequency, redemption_frequency, minimum_investment)
VALUES 
(
    'b1000000-0000-0000-0000-000000002001'::uuid,
    'b1000000-0000-0000-0000-000000001000'::uuid,
    'b1000000-0000-0000-0000-000000000002'::uuid,
    'Class A - Retail',
    'US7654321098',
    'USD',
    'FUND',
    '40_ACT', 'OPEN_ENDED', 'RETAIL',
    25.43, 'DAILY',
    75, 'DAILY', 'DAILY', 1000.00
),
(
    'b1000000-0000-0000-0000-000000002002'::uuid,
    'b1000000-0000-0000-0000-000000001000'::uuid,
    'b1000000-0000-0000-0000-000000000002'::uuid,
    'Class I - Institutional',
    'US7654321099',
    'USD',
    'FUND',
    '40_ACT', 'OPEN_ENDED', 'PROFESSIONAL',
    25.50, 'DAILY',
    45, 'DAILY', 'DAILY', 1000000.00
);

-- Officers
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'b1000000-0000-0000-0000-000000000010'::uuid,
    entity_type_id,
    'Jennifer Park'
FROM "ob-poc".entity_types WHERE type_code = 'proper_person';

INSERT INTO "ob-poc".entity_proper_persons
(proper_person_id, entity_id, first_name, last_name, nationality)
VALUES (
    'b1000000-0000-0000-0000-000000000110'::uuid,
    'b1000000-0000-0000-0000-000000000010'::uuid,
    'Jennifer', 'Park', 'US'
);

-- Link to CBU
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'b1000000-0000-0000-0000-000000001000'::uuid,
    'b1000000-0000-0000-0000-000000000010'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'DIRECTOR';

INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'b1000000-0000-0000-0000-000000001000'::uuid,
    'b1000000-0000-0000-0000-000000000002'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'PRINCIPAL';
```

---

### 1.3 UCITS Fund CBU

```sql
-- =============================================================================
-- UCITS FUND: Europa Equity UCITS
-- Structure: UK Asset Manager → Irish ManCo → Irish ICAV → Share Classes
-- =============================================================================

-- Commercial Client (UK Head Office)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'c1000000-0000-0000-0000-000000000001'::uuid,
    entity_type_id,
    'Europa Asset Management PLC'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'c1000000-0000-0000-0000-000000000101'::uuid,
    'c1000000-0000-0000-0000-000000000001'::uuid,
    'Europa Asset Management PLC',
    'GB',
    'UK-12345678'
);

-- ManCo (Irish regulated)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'c1000000-0000-0000-0000-000000000002'::uuid,
    entity_type_id,
    'Europa Fund Management Ireland Ltd'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'c1000000-0000-0000-0000-000000000102'::uuid,
    'c1000000-0000-0000-0000-000000000002'::uuid,
    'Europa Fund Management Ireland Ltd',
    'IE',
    'IE-567890'
);

-- Fund Entity (ICAV)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'c1000000-0000-0000-0000-000000000003'::uuid,
    entity_type_id,
    'Europa Equity UCITS ICAV'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'c1000000-0000-0000-0000-000000000103'::uuid,
    'c1000000-0000-0000-0000-000000000003'::uuid,
    'Europa Equity UCITS ICAV',
    'IE',
    'IE-ICAV-1234'
);

-- CBU
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type, commercial_client_entity_id)
VALUES (
    'c1000000-0000-0000-0000-000000001000'::uuid,
    'Europa Equity UCITS',
    'IE',
    'UCITS',
    'c1000000-0000-0000-0000-000000000001'::uuid
);

-- ManCo Corporate Shares
INSERT INTO kyc.share_classes (id, cbu_id, entity_id, name, currency, class_category)
VALUES (
    'c1000000-0000-0000-0000-000000002001'::uuid,
    'c1000000-0000-0000-0000-000000001000'::uuid,
    'c1000000-0000-0000-0000-000000000002'::uuid,
    'ManCo Ordinary Shares',
    'EUR',
    'CORPORATE'
);

-- Fund Share Classes (multi-currency, UCITS compliant)
INSERT INTO kyc.share_classes (id, cbu_id, entity_id, name, isin, currency, class_category,
    fund_type, fund_structure, investor_eligibility, nav_per_share, nav_frequency,
    management_fee_bps, subscription_frequency, redemption_frequency, redemption_notice_days,
    minimum_investment)
VALUES 
(
    'c1000000-0000-0000-0000-000000002002'::uuid,
    'c1000000-0000-0000-0000-000000001000'::uuid,
    'c1000000-0000-0000-0000-000000000003'::uuid,
    'Class A EUR Retail',
    'IE00B1234567',
    'EUR',
    'FUND',
    'UCITS', 'OPEN_ENDED', 'RETAIL',
    100.00, 'DAILY',
    150, 'DAILY', 'DAILY', 0, 500.00
),
(
    'c1000000-0000-0000-0000-000000002003'::uuid,
    'c1000000-0000-0000-0000-000000001000'::uuid,
    'c1000000-0000-0000-0000-000000000003'::uuid,
    'Class A GBP Retail',
    'IE00B1234568',
    'GBP',
    'FUND',
    'UCITS', 'OPEN_ENDED', 'RETAIL',
    100.00, 'DAILY',
    150, 'DAILY', 'DAILY', 0, 500.00
),
(
    'c1000000-0000-0000-0000-000000002004'::uuid,
    'c1000000-0000-0000-0000-000000001000'::uuid,
    'c1000000-0000-0000-0000-000000000003'::uuid,
    'Class I EUR Institutional',
    'IE00B1234569',
    'EUR',
    'FUND',
    'UCITS', 'OPEN_ENDED', 'PROFESSIONAL',
    1000.00, 'DAILY',
    75, 'DAILY', 'DAILY', 0, 100000.00
);

-- Ownership: UK Head Office owns 100% of Irish ManCo
INSERT INTO kyc.holdings (id, share_class_id, investor_entity_id, units, cost_basis, status)
VALUES (
    'c1000000-0000-0000-0000-000000003001'::uuid,
    'c1000000-0000-0000-0000-000000002001'::uuid,
    'c1000000-0000-0000-0000-000000000001'::uuid,
    1000, 100000.00, 'active'
);

-- Link entities to CBU
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'c1000000-0000-0000-0000-000000001000'::uuid,
    'c1000000-0000-0000-0000-000000000002'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'PRINCIPAL';
```

---

### 1.4 Trust CBU (Family Office)

```sql
-- =============================================================================
-- TRUST: Wellington Family Trust
-- Structure: Trustee → Trust → Beneficiaries
-- =============================================================================

-- Trustee (Corporate)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'd1000000-0000-0000-0000-000000000001'::uuid,
    entity_type_id,
    'Jersey Trust Company Ltd'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'd1000000-0000-0000-0000-000000000101'::uuid,
    'd1000000-0000-0000-0000-000000000001'::uuid,
    'Jersey Trust Company Ltd',
    'JE',
    'JE-TC-9999'
);

-- Trust Entity
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'd1000000-0000-0000-0000-000000000002'::uuid,
    entity_type_id,
    'Wellington Family Trust'
FROM "ob-poc".entity_types WHERE type_code = 'trust';

INSERT INTO "ob-poc".entity_trusts
(trust_id, entity_id, trust_name, trust_type, jurisdiction, governing_law)
VALUES (
    'd1000000-0000-0000-0000-000000000102'::uuid,
    'd1000000-0000-0000-0000-000000000002'::uuid,
    'Wellington Family Trust',
    'DISCRETIONARY',
    'JE',
    'Jersey'
);

-- Settlor (Person who created the trust)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'd1000000-0000-0000-0000-000000000010'::uuid,
    entity_type_id,
    'Robert Wellington'
FROM "ob-poc".entity_types WHERE type_code = 'proper_person';

INSERT INTO "ob-poc".entity_proper_persons
(proper_person_id, entity_id, first_name, last_name, nationality)
VALUES (
    'd1000000-0000-0000-0000-000000000110'::uuid,
    'd1000000-0000-0000-0000-000000000010'::uuid,
    'Robert', 'Wellington', 'GB'
);

-- Beneficiaries
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'd1000000-0000-0000-0000-000000000011'::uuid,
    entity_type_id,
    'Emma Wellington'
FROM "ob-poc".entity_types WHERE type_code = 'proper_person';

INSERT INTO "ob-poc".entity_proper_persons
(proper_person_id, entity_id, first_name, last_name, nationality)
VALUES (
    'd1000000-0000-0000-0000-000000000111'::uuid,
    'd1000000-0000-0000-0000-000000000011'::uuid,
    'Emma', 'Wellington', 'GB'
);

INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'd1000000-0000-0000-0000-000000000012'::uuid,
    entity_type_id,
    'James Wellington'
FROM "ob-poc".entity_types WHERE type_code = 'proper_person';

INSERT INTO "ob-poc".entity_proper_persons
(proper_person_id, entity_id, first_name, last_name, nationality)
VALUES (
    'd1000000-0000-0000-0000-000000000112'::uuid,
    'd1000000-0000-0000-0000-000000000012'::uuid,
    'James', 'Wellington', 'GB'
);

-- CBU (Trust as client - no commercial client, trust IS the client)
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
VALUES (
    'd1000000-0000-0000-0000-000000001000'::uuid,
    'Wellington Family Trust',
    'JE',
    'TRUST'
);

-- Link roles
-- Trustee
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'd1000000-0000-0000-0000-000000001000'::uuid,
    'd1000000-0000-0000-0000-000000000001'::uuid,
    role_id
FROM "ob-poc".roles WHERE name IN ('TRUSTEE');

-- Trust as Principal
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'd1000000-0000-0000-0000-000000001000'::uuid,
    'd1000000-0000-0000-0000-000000000002'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'PRINCIPAL';

-- Settlor
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'd1000000-0000-0000-0000-000000001000'::uuid,
    'd1000000-0000-0000-0000-000000000010'::uuid,
    role_id
FROM "ob-poc".roles WHERE name IN ('SETTLOR', 'UBO');

-- Beneficiaries
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'd1000000-0000-0000-0000-000000001000'::uuid,
    'd1000000-0000-0000-0000-000000000011'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'BENEFICIARY';

INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'd1000000-0000-0000-0000-000000001000'::uuid,
    'd1000000-0000-0000-0000-000000000012'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'BENEFICIARY';

-- Control relationship: Trustee controls Trust
INSERT INTO "ob-poc".control_relationships 
(control_id, controller_entity_id, controlled_entity_id, control_type, description, is_active)
VALUES (
    'd1000000-0000-0000-0000-000000004001'::uuid,
    'd1000000-0000-0000-0000-000000000001'::uuid,
    'd1000000-0000-0000-0000-000000000002'::uuid,
    'TRUSTEE',
    'Corporate trustee with full discretionary powers',
    true
);
```

---

### 1.5 Corporate Treasury CBU

```sql
-- =============================================================================
-- CORPORATE: TechCorp Global Treasury
-- Structure: US Parent → Treasury Entity (not a fund, corporate client)
-- =============================================================================

-- Parent Company (Commercial Client)
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'e1000000-0000-0000-0000-000000000001'::uuid,
    entity_type_id,
    'TechCorp Inc'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'e1000000-0000-0000-0000-000000000101'::uuid,
    'e1000000-0000-0000-0000-000000000001'::uuid,
    'TechCorp Inc',
    'US-DE',
    'DE-TECH-5555'
);

-- Treasury Subsidiary
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'e1000000-0000-0000-0000-000000000002'::uuid,
    entity_type_id,
    'TechCorp Treasury Ltd'
FROM "ob-poc".entity_types WHERE type_code = 'limited_company';

INSERT INTO "ob-poc".entity_limited_companies 
(limited_company_id, entity_id, company_name, jurisdiction, registration_number)
VALUES (
    'e1000000-0000-0000-0000-000000000102'::uuid,
    'e1000000-0000-0000-0000-000000000002'::uuid,
    'TechCorp Treasury Ltd',
    'IE',
    'IE-TREAS-1234'
);

-- CBU
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type, commercial_client_entity_id)
VALUES (
    'e1000000-0000-0000-0000-000000001000'::uuid,
    'TechCorp Global Treasury',
    'IE',
    'CORPORATE',
    'e1000000-0000-0000-0000-000000000001'::uuid
);

-- Treasury Corporate Shares (100% owned by parent)
INSERT INTO kyc.share_classes (id, cbu_id, entity_id, name, currency, class_category)
VALUES (
    'e1000000-0000-0000-0000-000000002001'::uuid,
    'e1000000-0000-0000-0000-000000001000'::uuid,
    'e1000000-0000-0000-0000-000000000002'::uuid,
    'Treasury Ordinary Shares',
    'EUR',
    'CORPORATE'
);

-- Ownership
INSERT INTO kyc.holdings (id, share_class_id, investor_entity_id, units, cost_basis, status)
VALUES (
    'e1000000-0000-0000-0000-000000003001'::uuid,
    'e1000000-0000-0000-0000-000000002001'::uuid,
    'e1000000-0000-0000-0000-000000000001'::uuid,
    1000, 1000000.00, 'active'
);

-- Officers
INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
SELECT 
    'e1000000-0000-0000-0000-000000000010'::uuid,
    entity_type_id,
    'Michael Torres'
FROM "ob-poc".entity_types WHERE type_code = 'proper_person';

INSERT INTO "ob-poc".entity_proper_persons
(proper_person_id, entity_id, first_name, last_name, nationality)
VALUES (
    'e1000000-0000-0000-0000-000000000110'::uuid,
    'e1000000-0000-0000-0000-000000000010'::uuid,
    'Michael', 'Torres', 'US'
);

-- Link
INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'e1000000-0000-0000-0000-000000001000'::uuid,
    'e1000000-0000-0000-0000-000000000010'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'DIRECTOR';

INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
SELECT 
    'e1000000-0000-0000-0000-000000001000'::uuid,
    'e1000000-0000-0000-0000-000000000002'::uuid,
    role_id
FROM "ob-poc".roles WHERE name = 'PRINCIPAL';
```

---

### 1.6 Ensure Required Roles Exist

```sql
-- Ensure all required roles exist
INSERT INTO "ob-poc".roles (name, description)
VALUES 
    ('TRUSTEE', 'Trustee of a trust'),
    ('SETTLOR', 'Settlor who created a trust'),
    ('BENEFICIARY', 'Beneficiary of a trust'),
    ('PROTECTOR', 'Protector with oversight powers'),
    ('MANCO', 'Management company')
ON CONFLICT (name) DO NOTHING;
```

---

## Part 2: Visualization Implementation

### 2.1 Graph Types Update

Update `rust/src/graph/types.rs` to add layer support:

```rust
/// Layer types for visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    /// Ownership, control, roles - KYC/UBO view
    Structure,
    /// Products, services, resources - Service Delivery view
    Delivery,
    /// Share classes (as investment), holdings - Investor Registry
    Registry,
    /// Document requirements and submissions
    Documents,
}

/// Edge types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    // Structure layer
    Owns,           // Entity/Person owns shares
    Controls,       // Non-ownership control
    Role,           // Person has role at entity/CBU
    Issues,         // Entity issues share class
    
    // Delivery layer  
    SubscribedTo,   // CBU subscribed to product
    Activated,      // Product activated service
    Provisioned,    // Service provisioned resource
    RoutesTo,       // Booking rule routes to SSI
    
    // Registry layer
    Holds,          // Investor holds position
    
    // Documents layer
    Requires,       // Entity requires document
    Submitted,      // Document submitted for entity
}

/// Node types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    // Core
    Cbu,
    
    // Structure layer
    Entity,         // Any legal entity (company, partnership, trust)
    Person,         // Natural person
    ShareClass,     // Share class (appears in Structure and Registry)
    
    // Delivery layer
    Product,
    Service,
    Resource,
    Ssi,
    BookingRule,
    
    // Documents layer
    Document,
    DocumentRequirement,
}

/// Graph node with layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: Uuid,
    pub node_type: NodeType,
    pub layer: LayerType,
    pub label: String,
    pub sublabel: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Graph edge with layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: Uuid,
    pub to: Uuid,
    pub edge_type: EdgeType,
    pub layer: LayerType,
    pub label: Option<String>,
    pub weight: Option<f32>,  // For ownership percentage
}

/// Complete graph for a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuGraph {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub layer_stats: HashMap<LayerType, LayerStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerStats {
    pub node_count: usize,
    pub edge_count: usize,
}
```

---

### 2.2 Graph Builder Update

Update `rust/src/graph/builder.rs` to build layered graph:

```rust
impl CbuGraphBuilder {
    pub async fn build(&self, cbu_id: Uuid, layers: &[LayerType]) -> Result<CbuGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        
        // Always add CBU as center node
        let cbu = self.load_cbu(cbu_id).await?;
        nodes.push(GraphNode {
            id: cbu_id,
            node_type: NodeType::Cbu,
            layer: LayerType::Structure, // CBU belongs to Structure
            label: cbu.name.clone(),
            sublabel: Some(cbu.client_type.unwrap_or_default()),
            metadata: HashMap::new(),
        });
        
        // Load requested layers
        if layers.contains(&LayerType::Structure) {
            self.load_structure_layer(cbu_id, &mut nodes, &mut edges).await?;
        }
        
        if layers.contains(&LayerType::Delivery) {
            self.load_delivery_layer(cbu_id, &mut nodes, &mut edges).await?;
        }
        
        if layers.contains(&LayerType::Registry) {
            self.load_registry_layer(cbu_id, &mut nodes, &mut edges).await?;
        }
        
        if layers.contains(&LayerType::Documents) {
            self.load_documents_layer(cbu_id, &mut nodes, &mut edges).await?;
        }
        
        // Calculate stats per layer
        let layer_stats = self.calculate_layer_stats(&nodes, &edges);
        
        Ok(CbuGraph {
            cbu_id,
            cbu_name: cbu.name,
            nodes,
            edges,
            layer_stats,
        })
    }
    
    async fn load_structure_layer(&self, cbu_id: Uuid, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) -> Result<()> {
        // 1. Load commercial client if set
        // 2. Load entities linked via cbu_entity_roles
        // 3. Load persons linked via cbu_entity_roles  
        // 4. Load share classes (CORPORATE category)
        // 5. Load holdings (ownership chain)
        // 6. Load control relationships
        // 7. Build edges for all relationships
        
        // Commercial client
        if let Some(commercial_client_id) = self.get_commercial_client(cbu_id).await? {
            let entity = self.load_entity(commercial_client_id).await?;
            nodes.push(GraphNode {
                id: commercial_client_id,
                node_type: NodeType::Entity,
                layer: LayerType::Structure,
                label: entity.name,
                sublabel: Some("Commercial Client".to_string()),
                metadata: HashMap::new(),
            });
            edges.push(GraphEdge {
                from: commercial_client_id,
                to: cbu_id,
                edge_type: EdgeType::Role,
                layer: LayerType::Structure,
                label: Some("CLIENT_OF".to_string()),
                weight: None,
            });
        }
        
        // Entities and roles
        let roles = self.load_cbu_entity_roles(cbu_id).await?;
        for role in roles {
            // Add entity/person node if not already present
            // Add role edge
        }
        
        // Share classes (CORPORATE)
        let share_classes = self.load_share_classes(cbu_id, Some("CORPORATE")).await?;
        for sc in share_classes {
            nodes.push(GraphNode {
                id: sc.id,
                node_type: NodeType::ShareClass,
                layer: LayerType::Structure,
                label: sc.name,
                sublabel: Some(format!("{} - {}", sc.currency, sc.class_category)),
                metadata: HashMap::new(),
            });
            // ISSUES edge from entity to share class
            if let Some(entity_id) = sc.entity_id {
                edges.push(GraphEdge {
                    from: entity_id,
                    to: sc.id,
                    edge_type: EdgeType::Issues,
                    layer: LayerType::Structure,
                    label: None,
                    weight: None,
                });
            }
        }
        
        // Holdings (ownership)
        let holdings = self.load_holdings_for_share_classes(&share_classes).await?;
        for holding in holdings {
            edges.push(GraphEdge {
                from: holding.investor_entity_id,
                to: holding.share_class_id,
                edge_type: EdgeType::Owns,
                layer: LayerType::Structure,
                label: Some(format!("{:.1}%", holding.percentage)),
                weight: Some(holding.percentage as f32),
            });
        }
        
        // Control relationships
        let controls = self.load_control_relationships(cbu_id).await?;
        for ctrl in controls {
            edges.push(GraphEdge {
                from: ctrl.controller_entity_id,
                to: ctrl.controlled_entity_id,
                edge_type: EdgeType::Controls,
                layer: LayerType::Structure,
                label: Some(ctrl.control_type),
                weight: None,
            });
        }
        
        Ok(())
    }
    
    async fn load_delivery_layer(&self, cbu_id: Uuid, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) -> Result<()> {
        // 1. Load products subscribed
        // 2. Load services activated
        // 3. Load resource instances provisioned
        // 4. Load SSIs
        // 5. Load booking rules
        // Build Product -> Service -> Resource -> SSI -> Rules edges
        
        // ... implementation
        Ok(())
    }
    
    async fn load_registry_layer(&self, cbu_id: Uuid, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) -> Result<()> {
        // 1. Load share classes (FUND category)
        // 2. Load holdings with counts (don't load 10,000 investor nodes)
        // 3. Create summary nodes for large holder counts
        
        let share_classes = self.load_share_classes(cbu_id, Some("FUND")).await?;
        for sc in share_classes {
            let holder_count = self.count_holdings(sc.id).await?;
            let total_aum = self.sum_holdings_value(sc.id).await?;
            
            nodes.push(GraphNode {
                id: sc.id,
                node_type: NodeType::ShareClass,
                layer: LayerType::Registry,
                label: sc.name,
                sublabel: Some(format!("{} holders, {} AUM", holder_count, total_aum)),
                metadata: [
                    ("holder_count".to_string(), json!(holder_count)),
                    ("total_aum".to_string(), json!(total_aum)),
                ].into_iter().collect(),
            });
            
            // ISSUES edge
            if let Some(entity_id) = sc.entity_id {
                edges.push(GraphEdge {
                    from: entity_id,
                    to: sc.id,
                    edge_type: EdgeType::Issues,
                    layer: LayerType::Registry,
                    label: None,
                    weight: None,
                });
            }
        }
        
        Ok(())
    }
    
    async fn load_documents_layer(&self, cbu_id: Uuid, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) -> Result<()> {
        // 1. Load document requirements for CBU entities
        // 2. Load submitted documents
        // 3. Create edges: Entity -> Requires -> DocType, Entity -> Submitted -> Doc
        
        // ... implementation
        Ok(())
    }
}
```

---

### 2.3 API Endpoint Update

Update the graph API to accept layer selection:

```rust
// GET /api/cbu/{id}/graph?layers=structure,delivery
#[derive(Deserialize)]
pub struct GraphQuery {
    #[serde(default = "default_layers")]
    layers: String,  // Comma-separated: "structure,delivery,registry,documents"
}

fn default_layers() -> String {
    "structure,delivery".to_string()
}

pub async fn get_cbu_graph(
    Path(cbu_id): Path<Uuid>,
    Query(query): Query<GraphQuery>,
    State(state): State<AppState>,
) -> Result<Json<CbuGraph>, ApiError> {
    let layers: Vec<LayerType> = query.layers
        .split(',')
        .filter_map(|s| match s.trim() {
            "structure" => Some(LayerType::Structure),
            "delivery" => Some(LayerType::Delivery),
            "registry" => Some(LayerType::Registry),
            "documents" => Some(LayerType::Documents),
            _ => None,
        })
        .collect();
    
    let builder = CbuGraphBuilder::new(state.pool.clone());
    let graph = builder.build(cbu_id, &layers).await?;
    
    Ok(Json(graph))
}
```

---

### 2.4 UI Layer Toggles

Update the egui UI to add layer toggle panel:

```rust
// In graph_view.rs or app.rs

pub struct LayerState {
    pub structure: bool,
    pub delivery: bool,
    pub registry: bool,
    pub documents: bool,
}

impl Default for LayerState {
    fn default() -> Self {
        Self {
            structure: true,
            delivery: true,
            registry: false,
            documents: false,
        }
    }
}

fn render_layer_panel(ui: &mut Ui, layers: &mut LayerState, stats: &HashMap<LayerType, LayerStats>) {
    ui.heading("Layers");
    ui.separator();
    
    let structure_count = stats.get(&LayerType::Structure).map(|s| s.node_count).unwrap_or(0);
    ui.checkbox(&mut layers.structure, format!("Structure ({} nodes)", structure_count));
    
    let delivery_count = stats.get(&LayerType::Delivery).map(|s| s.node_count).unwrap_or(0);
    ui.checkbox(&mut layers.delivery, format!("Delivery ({} nodes)", delivery_count));
    
    let registry_count = stats.get(&LayerType::Registry).map(|s| s.node_count).unwrap_or(0);
    ui.checkbox(&mut layers.registry, format!("Registry ({} nodes)", registry_count));
    
    let documents_count = stats.get(&LayerType::Documents).map(|s| s.node_count).unwrap_or(0);
    ui.checkbox(&mut layers.documents, format!("Documents ({} nodes)", documents_count));
}

fn filter_visible_nodes(graph: &CbuGraph, layers: &LayerState) -> Vec<&GraphNode> {
    graph.nodes.iter().filter(|n| {
        match n.layer {
            LayerType::Structure => layers.structure,
            LayerType::Delivery => layers.delivery,
            LayerType::Registry => layers.registry,
            LayerType::Documents => layers.documents,
        }
    }).collect()
}

fn filter_visible_edges(graph: &CbuGraph, layers: &LayerState) -> Vec<&GraphEdge> {
    graph.edges.iter().filter(|e| {
        match e.layer {
            LayerType::Structure => layers.structure,
            LayerType::Delivery => layers.delivery,
            LayerType::Registry => layers.registry,
            LayerType::Documents => layers.documents,
        }
    }).collect()
}
```

---

### 2.5 Visual Styling by Layer

```rust
fn get_layer_color(layer: LayerType) -> Color32 {
    match layer {
        LayerType::Structure => Color32::from_rgb(66, 133, 244),   // Blue
        LayerType::Delivery => Color32::from_rgb(52, 168, 83),    // Green  
        LayerType::Registry => Color32::from_rgb(251, 188, 4),    // Orange
        LayerType::Documents => Color32::from_rgb(154, 160, 166), // Gray
    }
}

fn get_edge_style(edge_type: &EdgeType) -> (Color32, f32, bool) {
    // Returns (color, thickness, is_dashed)
    match edge_type {
        EdgeType::Owns => (Color32::from_rgb(66, 133, 244), 2.0, false),
        EdgeType::Controls => (Color32::from_rgb(66, 133, 244), 1.5, true),
        EdgeType::Role => (Color32::from_rgb(100, 100, 100), 1.0, false),
        EdgeType::Issues => (Color32::from_rgb(66, 133, 244), 1.5, false),
        EdgeType::SubscribedTo => (Color32::from_rgb(52, 168, 83), 2.0, false),
        EdgeType::Activated => (Color32::from_rgb(52, 168, 83), 1.5, false),
        EdgeType::Provisioned => (Color32::from_rgb(52, 168, 83), 1.5, false),
        EdgeType::RoutesTo => (Color32::from_rgb(52, 168, 83), 1.0, true),
        EdgeType::Holds => (Color32::from_rgb(251, 188, 4), 1.0, false),
        EdgeType::Requires => (Color32::from_rgb(154, 160, 166), 1.0, true),
        EdgeType::Submitted => (Color32::from_rgb(154, 160, 166), 1.5, false),
    }
}

fn get_node_icon(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Cbu => "🏢",
        NodeType::Entity => "🏛️",
        NodeType::Person => "👤",
        NodeType::ShareClass => "📊",
        NodeType::Product => "📦",
        NodeType::Service => "⚙️",
        NodeType::Resource => "🔧",
        NodeType::Ssi => "🏦",
        NodeType::BookingRule => "📋",
        NodeType::Document => "📄",
        NodeType::DocumentRequirement => "📝",
    }
}
```

---

## Verification

After implementation:

1. **Seed data**: Run the SQL to populate 5 CBU examples
2. **API**: Test `GET /api/cbu/{id}/graph?layers=structure,delivery`
3. **UI**: Toggle layers on/off, verify correct nodes show/hide
4. **Styles**: Verify colors and edge styles per layer

---

## Summary

| CBU Type | ID Prefix | Client Type | Key Features |
|----------|-----------|-------------|--------------|
| Hedge Fund | a1... | HEDGE_FUND | ManCo, LP, lock-up, HWM, performance fee |
| 40-Act | b1... | 40_ACT | Daily NAV, retail accessible |
| UCITS | c1... | UCITS | Multi-currency, Irish ICAV, EU retail |
| Trust | d1... | TRUST | Trustee, settlor, beneficiaries, no shares |
| Corporate | e1... | CORPORATE | Treasury, simple corporate structure |

---

*End of Plan*
