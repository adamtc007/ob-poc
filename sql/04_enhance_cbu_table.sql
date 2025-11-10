-- Enhancement to CBU table: Add source_of_funds and improve structure
-- This migration adds essential fields for onboarding workflows

-- Add source_of_funds column to existing cbus table
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS source_of_funds TEXT,
ADD COLUMN IF NOT EXISTS customer_type VARCHAR(100),
ADD COLUMN IF NOT EXISTS jurisdiction VARCHAR(10),
ADD COLUMN IF NOT EXISTS channel VARCHAR(100),
ADD COLUMN IF NOT EXISTS risk_rating VARCHAR(20),
ADD COLUMN IF NOT EXISTS status VARCHAR(50) DEFAULT 'ACTIVE';

-- Create indexes for new fields
CREATE INDEX IF NOT EXISTS idx_cbus_customer_type ON "ob-poc".cbus (customer_type);
CREATE INDEX IF NOT EXISTS idx_cbus_jurisdiction ON "ob-poc".cbus (jurisdiction);
CREATE INDEX IF NOT EXISTS idx_cbus_risk_rating ON "ob-poc".cbus (risk_rating);
CREATE INDEX IF NOT EXISTS idx_cbus_status ON "ob-poc".cbus (status);

-- Create CBU relationships table for complex ownership structures
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_relationships (
    relationship_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_cbu_id UUID NOT NULL,
    child_cbu_id UUID NOT NULL,
    relationship_type VARCHAR(100) NOT NULL, -- SUBSIDIARY, BRANCH, AFFILIATE, FUND_SERIES
    ownership_percentage DECIMAL(5,2),
    effective_from DATE NOT NULL,
    effective_to DATE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    CONSTRAINT fk_parent_cbu FOREIGN KEY (parent_cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT fk_child_cbu FOREIGN KEY (child_cbu_id) REFERENCES "ob-poc".cbus(cbu_id),
    CONSTRAINT chk_ownership_percentage CHECK (ownership_percentage >= 0 AND ownership_percentage <= 100),
    CONSTRAINT chk_different_cbus CHECK (parent_cbu_id != child_cbu_id)
);

CREATE INDEX IF NOT EXISTS idx_cbu_relationships_parent ON "ob-poc".cbu_relationships (parent_cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_relationships_child ON "ob-poc".cbu_relationships (child_cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_relationships_type ON "ob-poc".cbu_relationships (relationship_type);

-- Create CBU contact information table
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_contacts (
    contact_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    contact_type VARCHAR(50) NOT NULL, -- PRIMARY, SECONDARY, COMPLIANCE, OPERATIONS, RELATIONSHIP_MANAGER
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    title VARCHAR(100),
    email VARCHAR(255),
    phone VARCHAR(50),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state_province VARCHAR(100),
    postal_code VARCHAR(20),
    country VARCHAR(10),
    is_primary BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_cbu_contacts_cbu_id ON "ob-poc".cbu_contacts (cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_contacts_type ON "ob-poc".cbu_contacts (contact_type);
CREATE INDEX IF NOT EXISTS idx_cbu_contacts_primary ON "ob-poc".cbu_contacts (cbu_id, is_primary) WHERE is_primary = TRUE;

-- Create CBU documents table for onboarding document tracking
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    document_type VARCHAR(100) NOT NULL, -- INCORPORATION_CERT, OFFERING_MEMORANDUM, etc.
    document_name VARCHAR(255) NOT NULL,
    file_path VARCHAR(500),
    document_status VARCHAR(50) NOT NULL DEFAULT 'REQUIRED', -- REQUIRED, RECEIVED, VERIFIED, EXPIRED
    received_date DATE,
    expiry_date DATE,
    verified_by VARCHAR(255),
    verified_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

CREATE INDEX IF NOT EXISTS idx_cbu_documents_cbu_id ON "ob-poc".cbu_documents (cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_documents_type ON "ob-poc".cbu_documents (document_type);
CREATE INDEX IF NOT EXISTS idx_cbu_documents_status ON "ob-poc".cbu_documents (document_status);
CREATE INDEX IF NOT EXISTS idx_cbu_documents_expiry ON "ob-poc".cbu_documents (expiry_date) WHERE expiry_date IS NOT NULL;

-- Create CBU attributes table for flexible key-value storage
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_attributes (
    attribute_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    attribute_key VARCHAR(255) NOT NULL,
    attribute_value TEXT,
    attribute_type VARCHAR(50) NOT NULL DEFAULT 'string', -- string, number, boolean, date, json
    is_sensitive BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc'),

    CONSTRAINT uk_cbu_attribute_key UNIQUE (cbu_id, attribute_key)
);

CREATE INDEX IF NOT EXISTS idx_cbu_attributes_cbu_id ON "ob-poc".cbu_attributes (cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_attributes_key ON "ob-poc".cbu_attributes (attribute_key);

-- Create a view for comprehensive CBU information
CREATE OR REPLACE VIEW "ob-poc".cbu_summary AS
SELECT
    c.cbu_id,
    c.name,
    c.description,
    c.nature_purpose,
    c.source_of_funds,
    c.customer_type,
    c.jurisdiction,
    c.channel,
    c.risk_rating,
    c.status,
    c.created_at,
    c.updated_at,

    -- Contact information (primary contact)
    cc.first_name || ' ' || cc.last_name AS primary_contact_name,
    cc.email AS primary_contact_email,
    cc.phone AS primary_contact_phone,

    -- Document counts
    COUNT(DISTINCT cd.document_id) AS total_documents,
    COUNT(DISTINCT cd.document_id) FILTER (WHERE cd.document_status = 'VERIFIED') AS verified_documents,
    COUNT(DISTINCT cd.document_id) FILTER (WHERE cd.document_status = 'REQUIRED') AS required_documents,

    -- Relationship counts
    COUNT(DISTINCT cr_parent.relationship_id) AS child_relationships,
    COUNT(DISTINCT cr_child.relationship_id) AS parent_relationships

FROM "ob-poc".cbus c
LEFT JOIN "ob-poc".cbu_contacts cc ON c.cbu_id = cc.cbu_id AND cc.is_primary = TRUE
LEFT JOIN "ob-poc".cbu_documents cd ON c.cbu_id = cd.cbu_id
LEFT JOIN "ob-poc".cbu_relationships cr_parent ON c.cbu_id = cr_parent.parent_cbu_id
LEFT JOIN "ob-poc".cbu_relationships cr_child ON c.cbu_id = cr_child.child_cbu_id

GROUP BY
    c.cbu_id, c.name, c.description, c.nature_purpose, c.source_of_funds,
    c.customer_type, c.jurisdiction, c.channel, c.risk_rating, c.status,
    c.created_at, c.updated_at,
    cc.first_name, cc.last_name, cc.email, cc.phone;

-- Insert some reference data for customer types
CREATE TABLE IF NOT EXISTS "ob-poc".customer_types (
    customer_type VARCHAR(100) PRIMARY KEY,
    description TEXT NOT NULL,
    regulatory_category VARCHAR(100),
    typical_products JSONB,
    required_documents JSONB,
    risk_category VARCHAR(20),
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- Insert standard customer types
INSERT INTO "ob-poc".customer_types (customer_type, description, regulatory_category, typical_products, required_documents, risk_category) VALUES
('HEDGE_FUND', 'Hedge fund or alternative investment fund', 'PROFESSIONAL_INVESTOR', '["CUSTODY", "FUND_ACCOUNTING", "MIDDLE_OFFICE"]', '["INCORPORATION_CERT", "OFFERING_MEMORANDUM", "IMA"]', 'HIGH'),
('UCITS_FUND', 'UCITS compliant mutual fund', 'RETAIL_FUND', '["CUSTODY", "FUND_ACCOUNTING"]', '["PROSPECTUS", "KIID", "FUND_RULES"]', 'LOW'),
('PRIVATE_EQUITY', 'Private equity or venture capital fund', 'PROFESSIONAL_INVESTOR', '["CUSTODY", "FUND_ACCOUNTING", "CAPITAL_CALL"]', '["INCORPORATION_CERT", "LPA", "PLACEMENT_MEMORANDUM"]', 'MEDIUM'),
('FAMILY_OFFICE', 'Single or multi-family office', 'PRIVATE_WEALTH', '["CUSTODY", "DISCRETIONARY_MANAGEMENT", "ADVISORY_SERVICES"]', '["FAMILY_CONSTITUTION", "INVESTMENT_POLICY"]', 'MEDIUM'),
('CORPORATE', 'Corporate treasury client', 'CORPORATE', '["CUSTODY", "CASH_MANAGEMENT", "FX_SERVICES"]', '["INCORPORATION_CERT", "BOARD_RESOLUTION", "FINANCIAL_STATEMENTS"]', 'LOW'),
('PENSION_FUND', 'Pension fund or retirement plan', 'INSTITUTIONAL', '["CUSTODY", "FUND_ACCOUNTING", "PERFORMANCE_MEASUREMENT"]', '["TRUST_DEED", "INVESTMENT_MANDATE"]', 'LOW'),
('INSURANCE', 'Insurance company or reinsurer', 'INSTITUTIONAL', '["CUSTODY", "FUND_ACCOUNTING", "REGULATORY_REPORTING"]', '["INSURANCE_LICENSE", "SOLVENCY_REPORT"]', 'MEDIUM')
ON CONFLICT (customer_type) DO UPDATE SET
    description = EXCLUDED.description,
    regulatory_category = EXCLUDED.regulatory_category,
    typical_products = EXCLUDED.typical_products,
    required_documents = EXCLUDED.required_documents,
    risk_category = EXCLUDED.risk_category;

-- Create function to validate CBU data integrity
CREATE OR REPLACE FUNCTION "ob-poc".validate_cbu_integrity(p_cbu_id UUID)
RETURNS TABLE(
    check_name TEXT,
    status TEXT,
    details TEXT
) AS $$
BEGIN
    -- Check if CBU has primary contact
    RETURN QUERY
    SELECT
        'primary_contact'::TEXT,
        CASE WHEN EXISTS(SELECT 1 FROM "ob-poc".cbu_contacts WHERE cbu_id = p_cbu_id AND is_primary = TRUE)
             THEN 'PASS' ELSE 'FAIL' END::TEXT,
        'CBU must have a primary contact'::TEXT;

    -- Check if required fields are populated
    RETURN QUERY
    SELECT
        'required_fields'::TEXT,
        CASE WHEN c.nature_purpose IS NOT NULL AND c.source_of_funds IS NOT NULL AND c.customer_type IS NOT NULL
             THEN 'PASS' ELSE 'FAIL' END::TEXT,
        'Nature & purpose, source of funds, and customer type are required'::TEXT
    FROM "ob-poc".cbus c WHERE c.cbu_id = p_cbu_id;

    -- Check document requirements based on customer type
    RETURN QUERY
    SELECT
        'document_requirements'::TEXT,
        CASE WHEN (
            SELECT COUNT(DISTINCT cd.document_type)
            FROM "ob-poc".cbu_documents cd
            WHERE cd.cbu_id = p_cbu_id AND cd.document_status = 'VERIFIED'
        ) >= 3 THEN 'PASS' ELSE 'WARN' END::TEXT,
        'At least 3 key documents should be verified'::TEXT;
END;
$$ LANGUAGE plpgsql;

COMMENT ON TABLE "ob-poc".cbus IS 'Enhanced Customer Business Unit definitions with comprehensive onboarding support';
COMMENT ON COLUMN "ob-poc".cbus.nature_purpose IS 'Business nature and purpose statement required for compliance';
COMMENT ON COLUMN "ob-poc".cbus.source_of_funds IS 'Source of investment funds for AML/KYC compliance';
COMMENT ON COLUMN "ob-poc".cbus.customer_type IS 'Type of customer (HEDGE_FUND, UCITS_FUND, etc.)';
COMMENT ON COLUMN "ob-poc".cbus.jurisdiction IS 'Primary regulatory jurisdiction (ISO 3166-1 alpha-2)';
COMMENT ON COLUMN "ob-poc".cbus.channel IS 'Onboarding channel (RELATIONSHIP_MANAGER, DIRECT_DIGITAL, etc.)';
COMMENT ON COLUMN "ob-poc".cbus.risk_rating IS 'Risk assessment rating (LOW, MEDIUM, HIGH)';

COMMENT ON TABLE "ob-poc".cbu_relationships IS 'Hierarchical relationships between CBUs for complex structures';
COMMENT ON TABLE "ob-poc".cbu_contacts IS 'Contact persons associated with each CBU';
COMMENT ON TABLE "ob-poc".cbu_documents IS 'Document tracking for onboarding and ongoing compliance';
COMMENT ON TABLE "ob-poc".cbu_attributes IS 'Flexible key-value attributes for CBU-specific data';
COMMENT ON VIEW "ob-poc".cbu_summary IS 'Comprehensive view of CBU data with related information';
