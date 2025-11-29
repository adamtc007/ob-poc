-- Seed: 011_csg_entity_type_hierarchy.sql
-- Purpose: Populate CSG hierarchy paths and semantic context for entity types
-- Part of CSG Linter implementation (Steps 3-5)

BEGIN;

-- ============================================
-- First, populate type_code from name if missing
-- ============================================

UPDATE "ob-poc".entity_types
SET type_code = UPPER(REPLACE(REPLACE(name, ' ', '_'), '-', '_'))
WHERE type_code IS NULL AND name IS NOT NULL;

-- ============================================
-- Root abstract types
-- ============================================

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY'],
    semantic_context = '{
        "category": "ROOT",
        "is_abstract": true,
        "typical_documents": [],
        "typical_attributes": []
    }'::jsonb
WHERE type_code = 'ENTITY';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY'],
    semantic_context = '{
        "category": "LEGAL_ENTITY",
        "is_abstract": true,
        "typical_documents": [],
        "typical_attributes": []
    }'::jsonb
WHERE type_code = 'LEGAL_ENTITY';

-- ============================================
-- Proper Person types (natural persons)
-- ============================================

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "is_abstract": false,
        "typical_documents": ["PASSPORT", "DRIVERS_LICENSE", "NATIONAL_ID", "PROOF_OF_ADDRESS"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality", "residential_address"],
        "synonyms": ["individual", "natural person", "person"]
    }'::jsonb
WHERE type_code = 'PROPER_PERSON';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON', 'PROPER_PERSON_NATURAL'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "is_abstract": false,
        "typical_documents": ["PASSPORT", "DRIVERS_LICENSE", "NATIONAL_ID", "PROOF_OF_ADDRESS"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality", "residential_address", "tax_id"],
        "synonyms": ["individual", "natural person", "person"]
    }'::jsonb
WHERE type_code = 'PROPER_PERSON_NATURAL';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON', 'PROPER_PERSON_BENEFICIAL_OWNER'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "is_abstract": false,
        "typical_documents": ["PASSPORT", "DRIVERS_LICENSE", "NATIONAL_ID", "PROOF_OF_ADDRESS", "PROOF_OF_FUNDS"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality", "ownership_percentage", "control_type"],
        "synonyms": ["UBO", "beneficial owner", "ultimate beneficial owner"]
    }'::jsonb
WHERE type_code IN ('PROPER_PERSON_BENEFICIAL_OWNER', 'BENEFICIAL_OWNER');

-- ============================================
-- Limited Company hierarchy
-- ============================================

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION", "REGISTER_OF_MEMBERS", "REGISTER_OF_DIRECTORS", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["company_name", "company_number", "incorporation_date", "jurisdiction", "registered_address"],
        "synonyms": ["company", "corporation", "limited company"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_PRIVATE'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION", "REGISTER_OF_MEMBERS", "REGISTER_OF_DIRECTORS"],
        "typical_attributes": ["company_name", "company_number", "incorporation_date", "jurisdiction", "registered_address"],
        "synonyms": ["private limited company", "Ltd", "private company", "Pty Ltd"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_PRIVATE';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_PUBLIC'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION", "REGISTER_OF_MEMBERS", "REGISTER_OF_DIRECTORS", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["company_name", "company_number", "incorporation_date", "jurisdiction", "registered_address", "stock_exchange", "ticker_symbol"],
        "synonyms": ["public limited company", "PLC", "publicly traded company", "listed company"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_PUBLIC';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_UNLIMITED'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["company_name", "company_number", "incorporation_date", "jurisdiction", "registered_address"],
        "synonyms": ["unlimited company", "unlimited liability company"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_UNLIMITED';

-- LLC
UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LLC'],
    semantic_context = '{
        "category": "CORPORATE",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "ARTICLES_OF_ASSOCIATION"],
        "typical_attributes": ["company_name", "company_number", "formation_date", "jurisdiction", "registered_address"],
        "synonyms": ["limited liability company", "LLC"]
    }'::jsonb
WHERE type_code = 'LLC';

-- ============================================
-- Partnership hierarchy
-- ============================================

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction"],
        "synonyms": ["partnership"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_GENERAL'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction", "general_partners"],
        "synonyms": ["general partnership", "GP"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_GENERAL';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_LIMITED'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction", "general_partners", "limited_partners"],
        "synonyms": ["limited partnership", "LP"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_LIMITED';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_LLP'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "is_abstract": false,
        "typical_documents": ["PARTNERSHIP_AGREEMENT", "CERTIFICATE_OF_INCORPORATION"],
        "typical_attributes": ["partnership_name", "registration_number", "formation_date", "jurisdiction"],
        "synonyms": ["limited liability partnership", "LLP"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_LLP';

-- ============================================
-- Trust hierarchy
-- ============================================

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "settlor"],
        "synonyms": ["trust"]
    }'::jsonb
WHERE type_code = 'TRUST';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_DISCRETIONARY'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "settlor", "beneficiary_classes"],
        "synonyms": ["discretionary trust", "family trust"]
    }'::jsonb
WHERE type_code = 'TRUST_DISCRETIONARY';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_FIXED_INTEREST'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "settlor", "named_beneficiaries"],
        "synonyms": ["fixed interest trust"]
    }'::jsonb
WHERE type_code = 'TRUST_FIXED_INTEREST';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_UNIT'],
    semantic_context = '{
        "category": "TRUST",
        "is_abstract": false,
        "typical_documents": ["TRUST_DEED", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["trust_name", "establishment_date", "jurisdiction", "trustees", "unit_holders"],
        "synonyms": ["unit trust", "mutual fund"]
    }'::jsonb
WHERE type_code = 'TRUST_UNIT';

-- ============================================
-- Fund types
-- ============================================

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'FUND'],
    semantic_context = '{
        "category": "FUND",
        "is_abstract": false,
        "typical_documents": ["CERTIFICATE_OF_INCORPORATION", "FINANCIAL_STATEMENTS"],
        "typical_attributes": ["fund_name", "formation_date", "jurisdiction", "fund_manager", "investment_strategy"],
        "synonyms": ["investment fund", "fund"]
    }'::jsonb
WHERE type_code = 'FUND';

COMMIT;
