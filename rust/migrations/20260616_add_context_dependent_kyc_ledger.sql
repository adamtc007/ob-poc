-- Migration: 20260616_add_context_dependent_kyc_ledger.sql
-- Description: Create context-dependent kyc_clearance_mandates table and compliance rollup views for SemOS engine gating.

CREATE TABLE IF NOT EXISTS "ob-poc".kyc_clearance_mandates (
    clearance_id UUID PRIMARY KEY DEFAULT uuidv7(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE RESTRICT,
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    role_id VARCHAR(50) NOT NULL,
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id) ON DELETE RESTRICT,
    clearance_status VARCHAR(50) DEFAULT 'IN_PROGRESS' NOT NULL,
    token_id VARCHAR(100) NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    CONSTRAINT uq_entity_role_product UNIQUE (entity_id, role_id, product_id),
    CONSTRAINT chk_clearance_status CHECK (
        clearance_status IN ('NOT_REQUIRED', 'IN_PROGRESS', 'CLEARED', 'FAILED')
    ),
    CONSTRAINT chk_role_id CHECK (
        role_id IN ('CONTRACTING_PARTY', 'GUARANTOR', 'INTRODUCER', 'INVESTMENT_MANAGER', 'FUND_ADMIN')
    )
);

-- Index for quick lookups by CBU (wrapped entity context)
CREATE INDEX IF NOT EXISTS idx_kyc_clearance_cbu 
ON "ob-poc".kyc_clearance_mandates (cbu_id) 
WHERE cbu_id IS NOT NULL;

-- 1. View for gating onboarding requests (per CBU / product / Contracting Party)
CREATE OR REPLACE VIEW "ob-poc".deal_onboarding_request_compliance AS
SELECT
    dor.request_id,
    COALESCE(kcm.clearance_status, 'IN_PROGRESS') AS status
FROM "ob-poc".deal_onboarding_requests dor
LEFT JOIN "ob-poc".kyc_clearance_mandates kcm ON kcm.cbu_id = dor.cbu_id 
                                              AND kcm.product_id = dor.product_id
                                              AND kcm.role_id = 'CONTRACTING_PARTY';

-- 2. View for gating deal contracting (primary signing entity)
CREATE OR REPLACE VIEW "ob-poc".deal_contracting_compliance AS
SELECT
    d.deal_id,
    COALESCE(kcm.clearance_status, 'IN_PROGRESS') AS status
FROM "ob-poc".deals d
JOIN "ob-poc".deal_participants dp ON dp.deal_id = d.deal_id 
                                   AND dp.participant_role = 'CONTRACTING_PARTY' 
                                   AND dp.is_primary = true
JOIN "ob-poc".deal_onboarding_requests dor ON dor.deal_id = d.deal_id
LEFT JOIN "ob-poc".kyc_clearance_mandates kcm ON kcm.entity_id = dp.entity_id
                                              AND kcm.product_id = dor.product_id
                                              AND kcm.role_id = 'CONTRACTING_PARTY';
