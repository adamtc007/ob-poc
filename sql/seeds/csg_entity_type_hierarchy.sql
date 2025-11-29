-- Seed: csg_entity_type_hierarchy.sql
-- Purpose: Populate CSG hierarchy paths and semantic context for entity types
-- Part of CSG Linter implementation

BEGIN;

-- ============================================
-- Set up hierarchy paths for entity types
-- ============================================

-- Proper Person types
UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON', 'PROPER_PERSON_NATURAL'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "typical_documents": ["PASSPORT", "DRIVING_LICENSE", "NATIONAL_ID", "UTILITY_BILL"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality"],
        "synonyms": ["individual", "natural person", "person"]
    }'::jsonb
WHERE type_code = 'PROPER_PERSON_NATURAL';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'PROPER_PERSON', 'PROPER_PERSON_BENEFICIAL_OWNER'],
    semantic_context = '{
        "category": "NATURAL_PERSON",
        "typical_documents": ["PASSPORT", "DRIVING_LICENSE", "NATIONAL_ID", "PROOF_OF_FUNDS"],
        "typical_attributes": ["first_name", "last_name", "date_of_birth", "nationality", "ownership_percentage"],
        "synonyms": ["UBO", "beneficial owner", "ultimate beneficial owner"]
    }'::jsonb
WHERE type_code = 'PROPER_PERSON_BENEFICIAL_OWNER';

-- Limited Company types
UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_PRIVATE'],
    semantic_context = '{
        "category": "CORPORATE",
        "typical_documents": ["ARTICLES_OF_INCORPORATION", "FINANCIAL_STATEMENT", "BUSINESS_LICENSE"],
        "typical_attributes": ["company_name", "registration_number", "incorporation_date", "jurisdiction"],
        "synonyms": ["private limited company", "Ltd", "private company"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_PRIVATE';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_PUBLIC'],
    semantic_context = '{
        "category": "CORPORATE",
        "typical_documents": ["ARTICLES_OF_INCORPORATION", "FINANCIAL_STATEMENT", "BUSINESS_LICENSE"],
        "typical_attributes": ["company_name", "registration_number", "incorporation_date", "jurisdiction", "stock_exchange"],
        "synonyms": ["public limited company", "PLC", "publicly traded company"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_PUBLIC';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'LIMITED_COMPANY', 'LIMITED_COMPANY_UNLIMITED'],
    semantic_context = '{
        "category": "CORPORATE",
        "typical_documents": ["ARTICLES_OF_INCORPORATION", "FINANCIAL_STATEMENT"],
        "typical_attributes": ["company_name", "registration_number", "incorporation_date", "jurisdiction"],
        "synonyms": ["unlimited company", "unlimited liability company"]
    }'::jsonb
WHERE type_code = 'LIMITED_COMPANY_UNLIMITED';

-- Partnership types
UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_GENERAL'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "typical_documents": ["FINANCIAL_STATEMENT", "BUSINESS_LICENSE"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction"],
        "synonyms": ["general partnership", "GP"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_GENERAL';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_LIMITED'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "typical_documents": ["FINANCIAL_STATEMENT", "BUSINESS_LICENSE"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction"],
        "synonyms": ["limited partnership", "LP"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_LIMITED';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'PARTNERSHIP', 'PARTNERSHIP_LLP'],
    semantic_context = '{
        "category": "PARTNERSHIP",
        "typical_documents": ["FINANCIAL_STATEMENT", "BUSINESS_LICENSE"],
        "typical_attributes": ["partnership_name", "formation_date", "jurisdiction"],
        "synonyms": ["limited liability partnership", "LLP"]
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_LLP';

-- Trust types
UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_DISCRETIONARY'],
    semantic_context = '{
        "category": "TRUST",
        "typical_documents": ["FINANCIAL_STATEMENT"],
        "typical_attributes": ["trust_name", "formation_date", "governing_law"],
        "synonyms": ["discretionary trust", "family trust"]
    }'::jsonb
WHERE type_code = 'TRUST_DISCRETIONARY';

UPDATE "ob-poc".entity_types SET
    type_hierarchy_path = ARRAY['ENTITY', 'LEGAL_ENTITY', 'TRUST', 'TRUST_FIXED_INTEREST'],
    semantic_context = '{
        "category": "TRUST",
        "typical_documents": ["FINANCIAL_STATEMENT"],
        "typical_attributes": ["trust_name", "formation_date", "governing_law"],
        "synonyms": ["fixed interest trust", "unit trust"]
    }'::jsonb
WHERE type_code = 'TRUST_FIXED_INTEREST';

COMMIT;
