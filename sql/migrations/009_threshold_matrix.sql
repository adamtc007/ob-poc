-- Threshold Decision Matrix for KYC Requirements
-- Computes KYC requirements dynamically based on CBU risk factors

-- =============================================================================
-- RISK FACTORS
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".threshold_factors (
    factor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    factor_type VARCHAR(50) NOT NULL,
    factor_code VARCHAR(50) NOT NULL,
    risk_weight INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(factor_type, factor_code)
);

COMMENT ON TABLE "ob-poc".threshold_factors IS 'Risk factors contributing to overall CBU risk score';
COMMENT ON COLUMN "ob-poc".threshold_factors.factor_type IS 'Category: CBU_TYPE, SOURCE_OF_FUNDS, NATURE_PURPOSE, JURISDICTION, PRODUCT_RISK';
COMMENT ON COLUMN "ob-poc".threshold_factors.risk_weight IS 'Contribution to composite risk score (higher = riskier)';

-- =============================================================================
-- RISK BANDS
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".risk_bands (
    band_code VARCHAR(20) PRIMARY KEY,
    min_score INTEGER NOT NULL,
    max_score INTEGER NOT NULL,
    description TEXT,
    escalation_required BOOLEAN DEFAULT false,
    review_frequency_months INTEGER DEFAULT 12,
    CONSTRAINT valid_score_range CHECK (min_score <= max_score)
);

COMMENT ON TABLE "ob-poc".risk_bands IS 'Risk band definitions mapping composite score to risk level';

-- =============================================================================
-- THRESHOLD REQUIREMENTS
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".threshold_requirements (
    requirement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_role VARCHAR(50) NOT NULL,
    risk_band VARCHAR(20) NOT NULL REFERENCES "ob-poc".risk_bands(band_code),
    attribute_code VARCHAR(50) NOT NULL,
    is_required BOOLEAN NOT NULL DEFAULT true,
    confidence_min NUMERIC(3,2) DEFAULT 0.85,
    max_age_days INTEGER,
    must_be_authoritative BOOLEAN DEFAULT false,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE(entity_role, risk_band, attribute_code)
);

COMMENT ON TABLE "ob-poc".threshold_requirements IS 'KYC attribute requirements per entity role and risk band';

-- =============================================================================
-- ACCEPTABLE DOCUMENT TYPES PER REQUIREMENT
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".requirement_acceptable_docs (
    requirement_id UUID REFERENCES "ob-poc".threshold_requirements(requirement_id) ON DELETE CASCADE,
    document_type_code VARCHAR(50) REFERENCES "ob-poc".document_types(type_code),
    priority INTEGER DEFAULT 1,
    PRIMARY KEY (requirement_id, document_type_code)
);

-- =============================================================================
-- SCREENING REQUIREMENTS PER RISK BAND
-- =============================================================================
CREATE TABLE IF NOT EXISTS "ob-poc".screening_requirements (
    risk_band VARCHAR(20) REFERENCES "ob-poc".risk_bands(band_code),
    screening_type VARCHAR(50) NOT NULL,
    is_required BOOLEAN NOT NULL DEFAULT true,
    frequency_months INTEGER DEFAULT 12,
    PRIMARY KEY (risk_band, screening_type)
);

-- =============================================================================
-- INDEXES
-- =============================================================================
CREATE INDEX IF NOT EXISTS idx_threshold_factors_type ON "ob-poc".threshold_factors(factor_type);
CREATE INDEX IF NOT EXISTS idx_threshold_factors_active ON "ob-poc".threshold_factors(is_active) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_threshold_requirements_role ON "ob-poc".threshold_requirements(entity_role);
CREATE INDEX IF NOT EXISTS idx_threshold_requirements_band ON "ob-poc".threshold_requirements(risk_band);

-- =============================================================================
-- HELPER FUNCTION: Compute risk score for a CBU
-- =============================================================================
CREATE OR REPLACE FUNCTION "ob-poc".compute_cbu_risk_score(target_cbu_id UUID)
RETURNS TABLE (
    risk_score INTEGER,
    risk_band VARCHAR(20),
    factors JSONB
) AS $$
DECLARE
    v_score INTEGER := 0;
    v_factors JSONB := '[]'::JSONB;
    v_cbu RECORD;
    v_factor RECORD;
    v_product_risk INTEGER;
BEGIN
    SELECT client_type, jurisdiction, nature_purpose, source_of_funds
    INTO v_cbu
    FROM "ob-poc".cbus
    WHERE cbu_id = target_cbu_id;

    IF NOT FOUND THEN
        RETURN;
    END IF;

    -- CBU type factor
    IF v_cbu.client_type IS NOT NULL THEN
        SELECT * INTO v_factor
        FROM "ob-poc".threshold_factors
        WHERE factor_type = 'CBU_TYPE' AND factor_code = v_cbu.client_type AND is_active = true;
        
        IF FOUND THEN
            v_score := v_score + v_factor.risk_weight;
            v_factors := v_factors || jsonb_build_object('type', v_factor.factor_type, 'code', v_factor.factor_code, 'weight', v_factor.risk_weight);
        END IF;
    END IF;

    -- Source of funds factor
    IF v_cbu.source_of_funds IS NOT NULL THEN
        SELECT * INTO v_factor
        FROM "ob-poc".threshold_factors
        WHERE factor_type = 'SOURCE_OF_FUNDS' AND factor_code = v_cbu.source_of_funds AND is_active = true;
        
        IF FOUND THEN
            v_score := v_score + v_factor.risk_weight;
            v_factors := v_factors || jsonb_build_object('type', v_factor.factor_type, 'code', v_factor.factor_code, 'weight', v_factor.risk_weight);
        END IF;
    END IF;

    -- Nature/purpose factor
    IF v_cbu.nature_purpose IS NOT NULL THEN
        SELECT * INTO v_factor
        FROM "ob-poc".threshold_factors
        WHERE factor_type = 'NATURE_PURPOSE' AND factor_code = v_cbu.nature_purpose AND is_active = true;
        
        IF FOUND THEN
            v_score := v_score + v_factor.risk_weight;
            v_factors := v_factors || jsonb_build_object('type', v_factor.factor_type, 'code', v_factor.factor_code, 'weight', v_factor.risk_weight);
        END IF;
    END IF;

    -- Product risk (MAX from service_delivery_map)
    SELECT COALESCE(MAX(tf.risk_weight), 0) INTO v_product_risk
    FROM "ob-poc".service_delivery_map sdm
    JOIN "ob-poc".products p ON sdm.product_id = p.product_id
    JOIN "ob-poc".threshold_factors tf ON tf.factor_code = p.product_code
    WHERE sdm.cbu_id = target_cbu_id AND tf.factor_type = 'PRODUCT_RISK' AND tf.is_active = true;

    IF v_product_risk > 0 THEN
        v_score := v_score + v_product_risk;
        v_factors := v_factors || jsonb_build_object('type', 'PRODUCT_RISK', 'code', 'MAX_PRODUCT', 'weight', v_product_risk);
    END IF;

    -- Map to risk band
    RETURN QUERY
    SELECT v_score, rb.band_code, v_factors
    FROM "ob-poc".risk_bands rb
    WHERE v_score >= rb.min_score AND v_score <= rb.max_score
    LIMIT 1;
END;
$$ LANGUAGE plpgsql STABLE;
