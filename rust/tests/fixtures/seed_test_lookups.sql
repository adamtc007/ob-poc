-- Seed lookup tables for onboarding harness tests

-- Roles
INSERT INTO "ob-poc".roles (role_id, name, description)
VALUES
    (gen_random_uuid(), 'InvestmentManager', 'Manages investments for the fund'),
    (gen_random_uuid(), 'BeneficialOwner', 'Ultimate beneficial owner (>25% ownership)'),
    (gen_random_uuid(), 'Director', 'Board member or director'),
    (gen_random_uuid(), 'AuthorizedSignatory', 'Authorized to sign on behalf of entity'),
    (gen_random_uuid(), 'Shareholder', 'Shareholder of the entity')
ON CONFLICT (name) DO NOTHING;

-- Document types
INSERT INTO "ob-poc".document_types (document_type_id, type_code, type_name, category)
VALUES
    (gen_random_uuid(), 'CERT_OF_INCORP', 'Certificate of Incorporation', 'CORPORATE'),
    (gen_random_uuid(), 'PASSPORT', 'Passport', 'IDENTITY'),
    (gen_random_uuid(), 'UTILITY_BILL', 'Utility Bill', 'ADDRESS'),
    (gen_random_uuid(), 'ARTICLES_OF_ASSOC', 'Articles of Association', 'CORPORATE'),
    (gen_random_uuid(), 'SHAREHOLDER_REG', 'Shareholder Register', 'CORPORATE')
ON CONFLICT (type_code) DO NOTHING;

-- Jurisdictions
INSERT INTO "ob-poc".jurisdictions (jurisdiction_id, iso_code, name, region)
VALUES
    (gen_random_uuid(), 'LU', 'Luxembourg', 'EU'),
    (gen_random_uuid(), 'GB', 'United Kingdom', 'EU'),
    (gen_random_uuid(), 'US', 'United States', 'NA'),
    (gen_random_uuid(), 'IE', 'Ireland', 'EU'),
    (gen_random_uuid(), 'CH', 'Switzerland', 'EU')
ON CONFLICT (iso_code) DO NOTHING;
