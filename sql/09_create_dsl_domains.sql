-- 09_create_dsl_domains.sql
-- Create DSL Domain Registry Table
--
-- This script creates the dsl_domains table that is referenced by
-- the document library and ISDA domain registration scripts.

-- ============================================================================
-- DSL DOMAIN REGISTRY TABLE
-- ============================================================================

-- Create the dsl_domains table if it doesn't exist
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_domains (
    domain_id SERIAL PRIMARY KEY,
    domain_name VARCHAR(50) UNIQUE NOT NULL,
    description TEXT NOT NULL,
    base_grammar_version VARCHAR(20) NOT NULL DEFAULT '3.0.0',
    vocabulary_version VARCHAR(20) NOT NULL DEFAULT '1.0.0',
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,

    -- Ensure domain names follow naming conventions
    CONSTRAINT valid_domain_name CHECK (domain_name ~ '^[A-Z][a-zA-Z_]*$')
);

-- Create index for fast domain lookups
CREATE INDEX IF NOT EXISTS idx_dsl_domains_name ON "ob-poc".dsl_domains (domain_name);
CREATE INDEX IF NOT EXISTS idx_dsl_domains_active ON "ob-poc".dsl_domains (active);

-- Create updated_at trigger
CREATE OR REPLACE FUNCTION "ob-poc".update_dsl_domains_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER trigger_dsl_domains_updated_at
    BEFORE UPDATE ON "ob-poc".dsl_domains
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_dsl_domains_updated_at();

-- Add some initial core domains
INSERT INTO "ob-poc".dsl_domains (domain_name, description, base_grammar_version, vocabulary_version) VALUES
('KYC', 'Know Your Customer workflows and compliance checks', '3.0.0', '1.0.0'),
('UBO', 'Ultimate Beneficial Ownership discovery and validation', '3.0.0', '1.0.0'),
('Onboarding', 'Client onboarding and account setup workflows', '3.0.0', '1.0.0'),
('Compliance', 'Regulatory compliance and risk management', '3.0.0', '1.0.0'),
('Graph', 'Entity relationship and ownership graph operations', '3.0.0', '1.0.0')
ON CONFLICT (domain_name) DO NOTHING;

-- Verify the table was created successfully
SELECT
    domain_name,
    description,
    base_grammar_version,
    vocabulary_version,
    active,
    created_at
FROM "ob-poc".dsl_domains
ORDER BY domain_name;
