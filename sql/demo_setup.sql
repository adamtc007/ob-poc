-- Demo Setup SQL for Real Database End-to-End Operations
-- This script sets up the minimal required schema for the end-to-end demo

-- Create the ob-poc schema if it doesn't exist
CREATE SCHEMA IF NOT EXISTS "ob-poc";

-- Set search path to use the schema
SET search_path TO "ob-poc";

-- CBUs (Client Business Units) Table
CREATE TABLE IF NOT EXISTS "ob-poc".cbus (
    cbu_id VARCHAR(255) PRIMARY KEY,
    client_name VARCHAR(500),
    client_type VARCHAR(100),
    jurisdiction VARCHAR(10),
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Dictionary Table for AttributeIDs
CREATE TABLE IF NOT EXISTS "ob-poc".dictionary (
    attribute_id UUID PRIMARY KEY,
    attribute_name VARCHAR(255) NOT NULL,
    data_type VARCHAR(100),
    domain VARCHAR(100),
    description TEXT,
    privacy_classification VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Attribute Values Table
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_values (
    id SERIAL PRIMARY KEY,
    attribute_id UUID REFERENCES "ob-poc".dictionary(attribute_id),
    entity_id VARCHAR(255),
    attribute_value TEXT,
    value_type VARCHAR(100),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Entities Table
CREATE TABLE IF NOT EXISTS "ob-poc".entities (
    entity_id VARCHAR(255) PRIMARY KEY,
    entity_name VARCHAR(500),
    entity_type VARCHAR(100),
    jurisdiction VARCHAR(10),
    incorporation_date DATE,
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- DSL Instances Table
CREATE TABLE IF NOT EXISTS "ob-poc".dsl_instances (
    id SERIAL PRIMARY KEY,
    case_id VARCHAR(255) NOT NULL,
    dsl_content TEXT NOT NULL,
    domain VARCHAR(100),
    operation_type VARCHAR(100),
    status VARCHAR(50) DEFAULT 'PROCESSED',
    processing_time_ms BIGINT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Parsed ASTs Table
CREATE TABLE IF NOT EXISTS "ob-poc".parsed_asts (
    id SERIAL PRIMARY KEY,
    case_id VARCHAR(255) NOT NULL,
    ast_json JSONB,
    ast_format_version VARCHAR(50) DEFAULT '3.1',
    compression_type VARCHAR(50),
    parse_time_ms BIGINT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- UBO Registry Table
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_registry (
    id SERIAL PRIMARY KEY,
    entity_id VARCHAR(255) REFERENCES "ob-poc".entities(entity_id),
    ubo_name VARCHAR(500),
    ownership_percentage DECIMAL(5,2),
    control_type VARCHAR(100),
    verification_status VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Document Catalog Table
CREATE TABLE IF NOT EXISTS "ob-poc".document_catalog (
    id SERIAL PRIMARY KEY,
    document_id VARCHAR(255) UNIQUE,
    document_type VARCHAR(100),
    entity_id VARCHAR(255),
    case_id VARCHAR(255),
    document_status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for better performance
CREATE INDEX IF NOT EXISTS idx_cbus_client_name ON "ob-poc".cbus(client_name);
CREATE INDEX IF NOT EXISTS idx_entities_entity_type ON "ob-poc".entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_dsl_instances_case_id ON "ob-poc".dsl_instances(case_id);
CREATE INDEX IF NOT EXISTS idx_parsed_asts_case_id ON "ob-poc".parsed_asts(case_id);
CREATE INDEX IF NOT EXISTS idx_attribute_values_entity_id ON "ob-poc".attribute_values(entity_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_entity_id ON "ob-poc".ubo_registry(entity_id);

-- Insert some basic dictionary attributes for the demo
INSERT INTO "ob-poc".dictionary (attribute_id, attribute_name, data_type, domain, description, privacy_classification)
VALUES
    ('550e8400-e29b-41d4-a716-446655440001', 'customer-name', 'STRING', 'kyc', 'Customer full name', 'PII'),
    ('550e8400-e29b-41d4-a716-446655440002', 'customer-type', 'ENUM', 'kyc', 'Type of customer (INDIVIDUAL/CORPORATE)', 'PUBLIC'),
    ('550e8400-e29b-41d4-a716-446655440003', 'jurisdiction', 'STRING', 'general', 'Legal jurisdiction code', 'PUBLIC'),
    ('550e8400-e29b-41d4-a716-446655440004', 'customer-id', 'UUID', 'kyc', 'Unique customer identifier', 'INTERNAL'),
    ('550e8400-e29b-41d4-a716-446655440005', 'entity-name', 'STRING', 'ubo', 'Legal entity name', 'PUBLIC'),
    ('550e8400-e29b-41d4-a716-446655440006', 'entity-type', 'ENUM', 'ubo', 'Type of legal entity', 'PUBLIC'),
    ('550e8400-e29b-41d4-a716-446655440007', 'entity-id', 'UUID', 'ubo', 'Unique entity identifier', 'INTERNAL'),
    ('550e8400-e29b-41d4-a716-446655440008', 'incorporation-date', 'DATE', 'ubo', 'Date of incorporation', 'PUBLIC'),
    ('550e8400-e29b-41d4-a716-446655440009', 'document-number', 'STRING', 'kyc', 'Identity document number', 'PII'),
    ('550e8400-e29b-41d4-a716-446655440010', 'risk-score', 'DECIMAL', 'compliance', 'Risk assessment score', 'INTERNAL')
ON CONFLICT (attribute_id) DO NOTHING;

-- Insert sample CBU data for testing
INSERT INTO "ob-poc".cbus (cbu_id, client_name, client_type, jurisdiction, status)
VALUES
    ('CBU-DEMO-001', 'Demo Financial Institution', 'BANK', 'US', 'ACTIVE'),
    ('CBU-DEMO-002', 'Demo Investment Firm', 'ASSET_MANAGER', 'GB', 'ACTIVE'),
    ('CBU-DEMO-003', 'Demo Hedge Fund', 'HEDGE_FUND', 'KY', 'ACTIVE')
ON CONFLICT (cbu_id) DO NOTHING;

-- Create a function to clean up demo data (useful for testing)
CREATE OR REPLACE FUNCTION "ob-poc".cleanup_demo_data()
RETURNS VOID AS $$
BEGIN
    DELETE FROM "ob-poc".attribute_values WHERE entity_id LIKE 'DEMO-%' OR entity_id LIKE 'CASE-%';
    DELETE FROM "ob-poc".parsed_asts WHERE case_id LIKE 'CASE-%';
    DELETE FROM "ob-poc".dsl_instances WHERE case_id LIKE 'CASE-%';
    DELETE FROM "ob-poc".ubo_registry WHERE entity_id LIKE 'DEMO-%';
    DELETE FROM "ob-poc".entities WHERE entity_id LIKE 'DEMO-%';
    DELETE FROM "ob-poc".document_catalog WHERE case_id LIKE 'CASE-%';

    RAISE NOTICE 'Demo data cleanup completed';
END;
$$ LANGUAGE plpgsql;

-- Grant permissions (adjust as needed for your setup)
GRANT USAGE ON SCHEMA "ob-poc" TO PUBLIC;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA "ob-poc" TO PUBLIC;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA "ob-poc" TO PUBLIC;

-- Show setup completion
DO $$
BEGIN
    RAISE NOTICE 'Database setup for End-to-End Demo completed successfully!';
    RAISE NOTICE 'Schema: ob-poc';
    RAISE NOTICE 'Tables created: cbus, dictionary, attribute_values, entities, dsl_instances, parsed_asts, ubo_registry, document_catalog';
    RAISE NOTICE 'Sample data inserted: % dictionary attributes, % CBU records',
        (SELECT COUNT(*) FROM "ob-poc".dictionary),
        (SELECT COUNT(*) FROM "ob-poc".cbus);
    RAISE NOTICE 'Ready for real database end-to-end demo!';
END;
$$;
