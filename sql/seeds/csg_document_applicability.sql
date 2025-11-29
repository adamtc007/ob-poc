-- Seed: csg_document_applicability.sql
-- Purpose: Populate CSG applicability rules for document types
-- Part of CSG Linter implementation

BEGIN;

-- ============================================
-- Identity documents (person only)
-- ============================================

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Verify identity of natural person",
        "synonyms": ["ID", "identification document"],
        "keywords": ["identity", "photo", "government"]
    }'::jsonb
WHERE type_code IN ('passport', 'PASSPORT', 'UK-PASSPORT');

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Verify identity via driving license",
        "synonyms": ["driving licence", "license", "DL"],
        "keywords": ["identity", "photo", "driving"]
    }'::jsonb
WHERE type_code = 'DRIVING_LICENSE';

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Verify identity via national ID card",
        "synonyms": ["national identity card", "ID card"],
        "keywords": ["identity", "photo", "government", "national"]
    }'::jsonb
WHERE type_code = 'NATIONAL_ID';

-- ============================================
-- Corporate formation documents
-- ============================================

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED"],
        "required_for": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Define company governance rules and structure",
        "synonyms": ["certificate of incorporation", "memorandum", "constitution"],
        "keywords": ["incorporation", "formation", "company", "legal"]
    }'::jsonb
WHERE type_code = 'ARTICLES_OF_INCORPORATION';

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"],
        "category": "CORPORATE"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Business operating license from regulatory authority",
        "synonyms": ["operating license", "trade license"],
        "keywords": ["license", "permit", "regulatory", "business"]
    }'::jsonb
WHERE type_code = 'BUSINESS_LICENSE';

-- ============================================
-- Financial documents (entities only)
-- ============================================

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP", "TRUST_DISCRETIONARY", "TRUST_FIXED_INTEREST"],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Show financial position and health of entity",
        "synonyms": ["accounts", "annual report", "audited accounts"],
        "keywords": ["financial", "statements", "audit", "accounts", "balance sheet"]
    }'::jsonb
WHERE type_code = 'FINANCIAL_STATEMENT';

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP", "PROPER_PERSON_NATURAL"],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Demonstrate source of funds for compliance",
        "synonyms": ["source of funds", "funds verification"],
        "keywords": ["funds", "source", "wealth", "compliance"]
    }'::jsonb
WHERE type_code = 'PROOF_OF_FUNDS';

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PROPER_PERSON_NATURAL"],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Show banking history and transactions",
        "synonyms": ["account statement", "bank records"],
        "keywords": ["bank", "statement", "transactions", "account"]
    }'::jsonb
WHERE type_code = 'BANK_STATEMENT';

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PROPER_PERSON_NATURAL"],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Tax compliance documentation",
        "synonyms": ["tax filing", "tax records"],
        "keywords": ["tax", "return", "income", "compliance"]
    }'::jsonb
WHERE type_code = 'TAX_RETURN';

-- ============================================
-- Address verification (universal)
-- ============================================

UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
        "category": "ADDRESS"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Verify residential or business address",
        "synonyms": ["address verification", "residence proof", "proof of address"],
        "keywords": ["address", "residence", "proof", "utility"]
    }'::jsonb
WHERE type_code = 'UTILITY_BILL';

COMMIT;
