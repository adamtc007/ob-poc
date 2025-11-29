-- Seed: 010_csg_document_applicability.sql
-- Purpose: Populate CSG applicability rules for document types
-- Part of CSG Linter implementation (Steps 3-5)

BEGIN;

-- ============================================
-- Identity documents (natural persons only)
-- ============================================

-- PASSPORT - Primary identity for natural persons
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": ["PROPER_PERSON_NATURAL"],
        "excludes": [],
        "requires": [],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Primary identity verification for natural persons",
        "synonyms": ["travel document", "ID document", "identity card"],
        "keywords": ["identity", "government issued", "photo ID", "MRZ"],
        "extraction_hints": {
            "ocr_zones": ["mrz", "photo", "personal_data"],
            "expiry_check": true,
            "mrz_validation": true
        }
    }'::jsonb
WHERE type_code IN ('passport', 'PASSPORT', 'UK-PASSPORT');

-- DRIVERS_LICENSE - Secondary identity for natural persons
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Secondary identity and address verification for natural persons",
        "synonyms": ["driving license", "driver license", "DL"],
        "keywords": ["identity", "government issued", "photo ID", "address"],
        "extraction_hints": {
            "ocr_zones": ["photo", "personal_data", "address"],
            "expiry_check": true
        }
    }'::jsonb
WHERE type_code IN ('DRIVING_LICENSE', 'DRIVERS_LICENSE');

-- NATIONAL_ID - Government-issued identity card
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PROPER_PERSON", "PROPER_PERSON_NATURAL", "PROPER_PERSON_BENEFICIAL_OWNER"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "IDENTITY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Government-issued national identity card",
        "synonyms": ["ID card", "national identity card", "citizen card"],
        "keywords": ["identity", "government issued", "photo ID"],
        "extraction_hints": {
            "ocr_zones": ["photo", "personal_data"],
            "expiry_check": true
        }
    }'::jsonb
WHERE type_code = 'NATIONAL_ID';

-- ============================================
-- Corporate formation documents
-- ============================================

-- CERTIFICATE_OF_INCORPORATION - Corporate formation
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "LLC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LLC"],
        "excludes": [],
        "requires": [],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Official proof of company incorporation and legal existence",
        "synonyms": ["incorporation certificate", "company registration", "formation document"],
        "keywords": ["incorporation", "registered", "company number", "formation date"],
        "extraction_hints": {
            "key_fields": ["company_name", "company_number", "incorporation_date", "jurisdiction"],
            "registry_validation": true
        }
    }'::jsonb
WHERE type_code IN ('CERTIFICATE_OF_INCORPORATION', 'CERT_OF_INCORP');

-- ARTICLES_OF_ASSOCIATION / INCORPORATION - Governance document
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "LLC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": ["LIMITED_COMPANY_PUBLIC"],
        "excludes": [],
        "requires": ["CERTIFICATE_OF_INCORPORATION"],
        "category": "GOVERNANCE"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Company constitutional document defining governance structure",
        "synonyms": ["articles of incorporation", "bylaws", "memorandum of association", "constitution"],
        "keywords": ["governance", "directors", "shareholders", "voting rights", "share classes"],
        "extraction_hints": {
            "key_sections": ["directors", "shareholders", "share_capital", "voting"]
        }
    }'::jsonb
WHERE type_code IN ('ARTICLES_OF_ASSOCIATION', 'ARTICLES_OF_INCORPORATION');

-- BUSINESS_LICENSE - Operating license
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "REGULATORY"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Business operating license from regulatory authority",
        "synonyms": ["operating license", "trade license"],
        "keywords": ["license", "permit", "regulatory", "business"]
    }'::jsonb
WHERE type_code = 'BUSINESS_LICENSE';

-- ============================================
-- Trust documents
-- ============================================

-- TRUST_DEED - Trust formation document
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["TRUST", "TRUST_DISCRETIONARY", "TRUST_FIXED_INTEREST", "TRUST_UNIT"],
        "jurisdictions": [],
        "client_types": ["trust"],
        "required_for": ["TRUST", "TRUST_DISCRETIONARY", "TRUST_FIXED_INTEREST", "TRUST_UNIT"],
        "excludes": [],
        "requires": [],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Legal document establishing trust structure and terms",
        "synonyms": ["trust agreement", "deed of trust", "trust instrument", "settlement deed"],
        "keywords": ["trustee", "settlor", "beneficiary", "trust property", "discretionary"],
        "extraction_hints": {
            "key_parties": ["trustees", "settlor", "beneficiaries", "protector"],
            "key_sections": ["trust_property", "distributions", "powers"]
        }
    }'::jsonb
WHERE type_code = 'TRUST_DEED';

-- ============================================
-- Partnership documents
-- ============================================

-- PARTNERSHIP_AGREEMENT
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["PARTNERSHIP", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": ["PARTNERSHIP", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP"],
        "excludes": [],
        "requires": [],
        "category": "FORMATION"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Agreement establishing partnership terms and partner relationships",
        "synonyms": ["partnership deed", "LLP agreement", "partner agreement"],
        "keywords": ["partners", "profit sharing", "capital contribution", "management"],
        "extraction_hints": {
            "key_parties": ["general_partners", "limited_partners"],
            "key_sections": ["capital", "profit_loss", "management", "dissolution"]
        }
    }'::jsonb
WHERE type_code = 'PARTNERSHIP_AGREEMENT';

-- ============================================
-- Financial documents
-- ============================================

-- FINANCIAL_STATEMENT - Financial statements
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "PARTNERSHIP", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "TRUST"],
        "jurisdictions": [],
        "client_types": ["corporate", "trust"],
        "required_for": ["LIMITED_COMPANY_PUBLIC"],
        "excludes": [],
        "requires": [],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Audited or management financial statements for due diligence",
        "synonyms": ["accounts", "annual report", "financial report", "audited accounts"],
        "keywords": ["balance sheet", "income statement", "cash flow", "audit", "assets", "liabilities"],
        "extraction_hints": {
            "key_sections": ["balance_sheet", "income_statement", "cash_flow", "notes"],
            "audit_check": true
        }
    }'::jsonb
WHERE type_code IN ('FINANCIAL_STATEMENT', 'FINANCIAL_STATEMENTS');

-- PROOF_OF_FUNDS
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "LIMITED_COMPANY_UNLIMITED", "PARTNERSHIP_GENERAL", "PARTNERSHIP_LIMITED", "PARTNERSHIP_LLP", "PROPER_PERSON_NATURAL"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Demonstrate source of funds for compliance",
        "synonyms": ["source of funds", "funds verification"],
        "keywords": ["funds", "source", "wealth", "compliance"]
    }'::jsonb
WHERE type_code = 'PROOF_OF_FUNDS';

-- BANK_STATEMENT
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PROPER_PERSON_NATURAL"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "FINANCIAL"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Show banking history and transactions",
        "synonyms": ["account statement", "bank records"],
        "keywords": ["bank", "statement", "transactions", "account"]
    }'::jsonb
WHERE type_code = 'BANK_STATEMENT';

-- TAX_RETURN
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "PROPER_PERSON_NATURAL"],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
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

-- PROOF_OF_ADDRESS / UTILITY_BILL
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": [],
        "jurisdictions": [],
        "client_types": [],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "ADDRESS"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Verification of residential or business address",
        "synonyms": ["utility bill", "bank statement", "address verification"],
        "keywords": ["address", "residence", "utility", "recent"],
        "extraction_hints": {
            "key_fields": ["name", "address", "date"],
            "recency_check": true,
            "max_age_months": 3
        }
    }'::jsonb
WHERE type_code IN ('PROOF_OF_ADDRESS', 'UTILITY_BILL');

-- ============================================
-- Compliance documents
-- ============================================

-- BENEFICIAL_OWNERSHIP_DECLARATION
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC", "TRUST", "PARTNERSHIP", "LLC"],
        "jurisdictions": [],
        "client_types": ["corporate", "trust"],
        "required_for": ["LIMITED_COMPANY", "TRUST", "PARTNERSHIP"],
        "excludes": [],
        "requires": [],
        "category": "COMPLIANCE"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Declaration of ultimate beneficial owners for AML compliance",
        "synonyms": ["UBO declaration", "beneficial owner form", "ownership declaration"],
        "keywords": ["beneficial owner", "UBO", "ownership", "control", "25%", "threshold"],
        "extraction_hints": {
            "key_fields": ["beneficial_owners", "ownership_percentage", "control_type"],
            "threshold_check": true
        }
    }'::jsonb
WHERE type_code = 'BENEFICIAL_OWNERSHIP_DECLARATION';

-- ============================================
-- Corporate registers
-- ============================================

-- REGISTER_OF_MEMBERS
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "REGISTER"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Official register of company shareholders/members",
        "synonyms": ["shareholder register", "member register", "share register"],
        "keywords": ["shareholders", "members", "shares", "ownership"],
        "extraction_hints": {
            "key_fields": ["shareholders", "share_class", "number_of_shares", "percentage"]
        }
    }'::jsonb
WHERE type_code = 'REGISTER_OF_MEMBERS';

-- REGISTER_OF_DIRECTORS
UPDATE "ob-poc".document_types SET
    applicability = '{
        "entity_types": ["LIMITED_COMPANY", "LIMITED_COMPANY_PRIVATE", "LIMITED_COMPANY_PUBLIC"],
        "jurisdictions": [],
        "client_types": ["corporate"],
        "required_for": [],
        "excludes": [],
        "requires": [],
        "category": "REGISTER"
    }'::jsonb,
    semantic_context = '{
        "purpose": "Official register of company directors and officers",
        "synonyms": ["director register", "officer register", "board register"],
        "keywords": ["directors", "officers", "board", "appointment", "resignation"],
        "extraction_hints": {
            "key_fields": ["directors", "appointment_date", "role", "nationality"]
        }
    }'::jsonb
WHERE type_code = 'REGISTER_OF_DIRECTORS';

COMMIT;
