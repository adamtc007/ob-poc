-- ═══════════════════════════════════════════════════════════════════════════════════════════════
-- Migration: Request Types Reference Table
-- Description: Configuration for different request types (document, information, verification, etc.)
-- ═══════════════════════════════════════════════════════════════════════════════════════════════

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Request Types Table
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS ob_ref.request_types (
    request_type VARCHAR(50) NOT NULL,
    request_subtype VARCHAR(100) NOT NULL,

    -- Configuration
    description VARCHAR(255),
    default_due_days INTEGER DEFAULT 7,
    default_grace_days INTEGER DEFAULT 3,
    max_reminders INTEGER DEFAULT 3,
    blocks_by_default BOOLEAN DEFAULT TRUE,

    -- Who can fulfill?
    fulfillment_sources VARCHAR(50)[] DEFAULT ARRAY['CLIENT', 'USER'],
    auto_fulfill_on_upload BOOLEAN DEFAULT TRUE,  -- Auto-match document uploads

    -- Escalation config
    escalation_enabled BOOLEAN DEFAULT TRUE,
    escalation_after_days INTEGER DEFAULT 10,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (request_type, request_subtype)
);

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Seed Common Request Types
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

-- Documents
INSERT INTO ob_ref.request_types (request_type, request_subtype, description, default_due_days, blocks_by_default) VALUES
('DOCUMENT', 'ID_DOCUMENT', 'Identity document (passport, national ID)', 7, TRUE),
('DOCUMENT', 'PROOF_OF_ADDRESS', 'Proof of address document', 7, TRUE),
('DOCUMENT', 'SOURCE_OF_WEALTH', 'Source of wealth documentation', 14, TRUE),
('DOCUMENT', 'SOURCE_OF_FUNDS', 'Source of funds documentation', 14, TRUE),
('DOCUMENT', 'CERTIFICATE_OF_INCORPORATION', 'Company incorporation certificate', 7, TRUE),
('DOCUMENT', 'ARTICLES_OF_ASSOCIATION', 'Articles/memorandum of association', 7, TRUE),
('DOCUMENT', 'REGISTER_OF_MEMBERS', 'Shareholder register', 7, TRUE),
('DOCUMENT', 'REGISTER_OF_DIRECTORS', 'Directors register', 7, TRUE),
('DOCUMENT', 'FINANCIAL_STATEMENTS', 'Audited financial statements', 14, TRUE),
('DOCUMENT', 'OWNERSHIP_STRUCTURE', 'Ownership structure chart', 7, TRUE),
('DOCUMENT', 'BOARD_RESOLUTION', 'Board resolution', 7, TRUE),
('DOCUMENT', 'POWER_OF_ATTORNEY', 'Power of attorney document', 7, TRUE),
('DOCUMENT', 'TAX_FORMS', 'Tax forms (W-8, W-9, CRS)', 7, TRUE),
('DOCUMENT', 'REGULATORY_LICENSE', 'Regulatory license/authorization', 7, TRUE),
('DOCUMENT', 'OTHER', 'Other document', 7, TRUE)
ON CONFLICT (request_type, request_subtype) DO NOTHING;

-- Information requests
INSERT INTO ob_ref.request_types (request_type, request_subtype, description, default_due_days, blocks_by_default) VALUES
('INFORMATION', 'UBO_DETAILS', 'Ultimate beneficial owner details', 7, TRUE),
('INFORMATION', 'DIRECTOR_DETAILS', 'Director/officer details', 7, TRUE),
('INFORMATION', 'BUSINESS_DESCRIPTION', 'Business description and activities', 7, TRUE),
('INFORMATION', 'CONTACT_DETAILS', 'Contact information', 5, FALSE),
('INFORMATION', 'INVESTMENT_MANDATE', 'Investment mandate/strategy', 7, FALSE),
('INFORMATION', 'TAX_RESIDENCY', 'Tax residency information', 7, TRUE)
ON CONFLICT (request_type, request_subtype) DO NOTHING;

-- Verifications (external)
INSERT INTO ob_ref.request_types (request_type, request_subtype, description, default_due_days, blocks_by_default) VALUES
('VERIFICATION', 'REGISTRY_CHECK', 'Company registry verification', 3, TRUE),
('VERIFICATION', 'REGULATORY_CHECK', 'Regulatory register verification', 3, TRUE),
('VERIFICATION', 'SANCTIONS_SCREENING', 'Sanctions screening', 1, TRUE),
('VERIFICATION', 'PEP_SCREENING', 'PEP screening', 1, TRUE),
('VERIFICATION', 'ADVERSE_MEDIA', 'Adverse media screening', 2, TRUE),
('VERIFICATION', 'ID_VERIFICATION', 'Electronic ID verification', 2, TRUE)
ON CONFLICT (request_type, request_subtype) DO NOTHING;

-- Approvals (internal)
INSERT INTO ob_ref.request_types (request_type, request_subtype, description, default_due_days, blocks_by_default) VALUES
('APPROVAL', 'KYC_REVIEW', 'KYC analyst review', 3, TRUE),
('APPROVAL', 'SENIOR_REVIEW', 'Senior/manager review', 2, TRUE),
('APPROVAL', 'WAIVER_APPROVAL', 'Document waiver approval', 1, TRUE),
('APPROVAL', 'RISK_ACCEPTANCE', 'Risk acceptance approval', 2, TRUE)
ON CONFLICT (request_type, request_subtype) DO NOTHING;

-- Signatures
INSERT INTO ob_ref.request_types (request_type, request_subtype, description, default_due_days, blocks_by_default) VALUES
('SIGNATURE', 'ACCOUNT_OPENING', 'Account opening documents', 14, TRUE),
('SIGNATURE', 'TAX_CERTIFICATION', 'Tax certification signatures', 14, TRUE),
('SIGNATURE', 'AGREEMENT', 'Agreement/contract signatures', 14, TRUE)
ON CONFLICT (request_type, request_subtype) DO NOTHING;

-- ─────────────────────────────────────────────────────────────────────────────────────────────────
-- Comments
-- ─────────────────────────────────────────────────────────────────────────────────────────────────

COMMENT ON TABLE ob_ref.request_types IS 'Configuration for different request types and subtypes';
COMMENT ON COLUMN ob_ref.request_types.fulfillment_sources IS 'Who can fulfill this request: CLIENT, USER, SYSTEM, EXTERNAL_PROVIDER';
COMMENT ON COLUMN ob_ref.request_types.auto_fulfill_on_upload IS 'Whether uploading a matching document auto-fulfills the request';
COMMENT ON COLUMN ob_ref.request_types.escalation_after_days IS 'Days past due date before auto-escalation';
