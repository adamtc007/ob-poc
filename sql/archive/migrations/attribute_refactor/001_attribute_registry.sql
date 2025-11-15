-- Attribute Dictionary Refactoring - Phase 1: Foundation
-- Migration: 001_attribute_registry.sql
-- Purpose: Create new type-safe attribute registry system
-- Date: 2025-11-14
-- Version: 1.0

-- ============================================================================
-- PART 1: CREATE NEW ATTRIBUTE REGISTRY TABLE
-- ============================================================================

-- Create the new attribute registry table with string-based IDs
-- This replaces the UUID-based attribute system with typed string identifiers
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_registry (
    -- String-based attribute identifier (e.g., "attr.identity.first_name")
    id TEXT PRIMARY KEY,

    -- Human-readable display name
    display_name TEXT NOT NULL,

    -- Category for grouping (identity, financial, compliance, etc.)
    category TEXT NOT NULL,

    -- Value type for storage (string, number, integer, boolean, date, etc.)
    value_type TEXT NOT NULL,

    -- Validation rules as JSON
    validation_rules JSONB DEFAULT '{}'::jsonb,

    -- Additional metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Audit fields
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),

    -- Constraints
    CONSTRAINT check_category CHECK (category IN (
        'identity', 'financial', 'compliance', 'document', 'risk',
        'contact', 'address', 'tax', 'employment', 'product',
        'entity', 'ubo', 'isda'
    )),

    CONSTRAINT check_value_type CHECK (value_type IN (
        'string', 'integer', 'number', 'boolean', 'date', 'datetime',
        'email', 'phone', 'address', 'currency', 'percentage',
        'tax_id', 'json'
    ))
);

-- Create indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_attribute_registry_category
    ON "ob-poc".attribute_registry(category);

CREATE INDEX IF NOT EXISTS idx_attribute_registry_value_type
    ON "ob-poc".attribute_registry(value_type);

-- Add comment to table
COMMENT ON TABLE "ob-poc".attribute_registry IS
    'Type-safe attribute registry with string-based identifiers following the AttributeID-as-Type pattern';

COMMENT ON COLUMN "ob-poc".attribute_registry.id IS
    'Attribute identifier in format attr.{category}.{name} (e.g., attr.identity.first_name)';

COMMENT ON COLUMN "ob-poc".attribute_registry.validation_rules IS
    'JSON object containing validation rules: {required, min_value, max_value, min_length, max_length, pattern, allowed_values}';

-- ============================================================================
-- PART 2: CREATE NEW ATTRIBUTE VALUES TABLE
-- ============================================================================

-- Create the new attribute values table with proper typing
CREATE TABLE IF NOT EXISTS "ob-poc".attribute_values_typed (
    -- Primary key
    id SERIAL PRIMARY KEY,

    -- Entity this attribute belongs to
    entity_id UUID NOT NULL,

    -- Attribute reference (string-based ID)
    attribute_id TEXT NOT NULL REFERENCES "ob-poc".attribute_registry(id),

    -- Type-specific value columns (only one should be populated)
    value_text TEXT,
    value_number NUMERIC,
    value_integer BIGINT,
    value_boolean BOOLEAN,
    value_date DATE,
    value_datetime TIMESTAMPTZ,
    value_json JSONB,

    -- Temporal validity
    effective_from TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    effective_to TIMESTAMPTZ,

    -- Source information
    source JSONB,

    -- Audit trail
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    created_by TEXT DEFAULT 'system',

    -- Ensure only one value column is populated
    CONSTRAINT check_single_value CHECK (
        (
            (value_text IS NOT NULL)::int +
            (value_number IS NOT NULL)::int +
            (value_integer IS NOT NULL)::int +
            (value_boolean IS NOT NULL)::int +
            (value_date IS NOT NULL)::int +
            (value_datetime IS NOT NULL)::int +
            (value_json IS NOT NULL)::int
        ) = 1
    ),

    -- Ensure effective_to is after effective_from
    CONSTRAINT check_temporal_validity CHECK (
        effective_to IS NULL OR effective_to > effective_from
    )
);

-- Create indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_attribute_values_typed_entity
    ON "ob-poc".attribute_values_typed(entity_id);

CREATE INDEX IF NOT EXISTS idx_attribute_values_typed_attribute
    ON "ob-poc".attribute_values_typed(attribute_id);

CREATE INDEX IF NOT EXISTS idx_attribute_values_typed_entity_attribute
    ON "ob-poc".attribute_values_typed(entity_id, attribute_id);

CREATE INDEX IF NOT EXISTS idx_attribute_values_typed_effective
    ON "ob-poc".attribute_values_typed(effective_from, effective_to)
    WHERE effective_to IS NULL;

-- Add comments
COMMENT ON TABLE "ob-poc".attribute_values_typed IS
    'Type-safe attribute values with proper column typing based on value_type';

COMMENT ON CONSTRAINT check_single_value ON "ob-poc".attribute_values_typed IS
    'Ensures exactly one value column is populated per row';

-- ============================================================================
-- PART 3: CREATE HELPER FUNCTIONS
-- ============================================================================

-- Function to get current attribute value for an entity
CREATE OR REPLACE FUNCTION "ob-poc".get_attribute_value(
    p_entity_id UUID,
    p_attribute_id TEXT
)
RETURNS TABLE (
    value_text TEXT,
    value_number NUMERIC,
    value_integer BIGINT,
    value_boolean BOOLEAN,
    value_date DATE,
    value_datetime TIMESTAMPTZ,
    value_json JSONB
)
LANGUAGE SQL
STABLE
AS $$
    SELECT
        value_text,
        value_number,
        value_integer,
        value_boolean,
        value_date,
        value_datetime,
        value_json
    FROM "ob-poc".attribute_values_typed
    WHERE entity_id = p_entity_id
      AND attribute_id = p_attribute_id
      AND effective_to IS NULL
    ORDER BY effective_from DESC
    LIMIT 1;
$$;

-- Function to set attribute value (creates new version, expires old)
CREATE OR REPLACE FUNCTION "ob-poc".set_attribute_value(
    p_entity_id UUID,
    p_attribute_id TEXT,
    p_value_text TEXT DEFAULT NULL,
    p_value_number NUMERIC DEFAULT NULL,
    p_value_integer BIGINT DEFAULT NULL,
    p_value_boolean BOOLEAN DEFAULT NULL,
    p_value_date DATE DEFAULT NULL,
    p_value_datetime TIMESTAMPTZ DEFAULT NULL,
    p_value_json JSONB DEFAULT NULL,
    p_created_by TEXT DEFAULT 'system'
)
RETURNS BIGINT
LANGUAGE PLPGSQL
AS $$
DECLARE
    v_new_id BIGINT;
BEGIN
    -- Expire any existing active values
    UPDATE "ob-poc".attribute_values_typed
    SET effective_to = NOW() AT TIME ZONE 'utc'
    WHERE entity_id = p_entity_id
      AND attribute_id = p_attribute_id
      AND effective_to IS NULL;

    -- Insert new value
    INSERT INTO "ob-poc".attribute_values_typed (
        entity_id,
        attribute_id,
        value_text,
        value_number,
        value_integer,
        value_boolean,
        value_date,
        value_datetime,
        value_json,
        created_by
    )
    VALUES (
        p_entity_id,
        p_attribute_id,
        p_value_text,
        p_value_number,
        p_value_integer,
        p_value_boolean,
        p_value_date,
        p_value_datetime,
        p_value_json,
        p_created_by
    )
    RETURNING id INTO v_new_id;

    RETURN v_new_id;
END;
$$;

-- ============================================================================
-- PART 4: CREATE UPDATE TRIGGER
-- ============================================================================

-- Function to update the updated_at timestamp
CREATE OR REPLACE FUNCTION "ob-poc".update_attribute_registry_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW() AT TIME ZONE 'utc';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for attribute registry updates
DROP TRIGGER IF EXISTS trigger_update_attribute_registry_timestamp
    ON "ob-poc".attribute_registry;

CREATE TRIGGER trigger_update_attribute_registry_timestamp
    BEFORE UPDATE ON "ob-poc".attribute_registry
    FOR EACH ROW
    EXECUTE FUNCTION "ob-poc".update_attribute_registry_timestamp();

-- ============================================================================
-- PART 5: GRANT PERMISSIONS
-- ============================================================================

-- Grant appropriate permissions (adjust user/role as needed)
-- GRANT SELECT, INSERT, UPDATE ON "ob-poc".attribute_registry TO your_app_user;
-- GRANT SELECT, INSERT, UPDATE ON "ob-poc".attribute_values_typed TO your_app_user;
-- GRANT USAGE, SELECT ON SEQUENCE "ob-poc".attribute_values_typed_id_seq TO your_app_user;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- Verify tables were created
DO $$
BEGIN
    RAISE NOTICE 'Verifying attribute_registry table...';
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'ob-poc'
        AND table_name = 'attribute_registry'
    ) THEN
        RAISE NOTICE '✓ attribute_registry table created successfully';
    ELSE
        RAISE EXCEPTION '✗ attribute_registry table NOT created';
    END IF;

    RAISE NOTICE 'Verifying attribute_values_typed table...';
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'ob-poc'
        AND table_name = 'attribute_values_typed'
    ) THEN
        RAISE NOTICE '✓ attribute_values_typed table created successfully';
    ELSE
        RAISE EXCEPTION '✗ attribute_values_typed table NOT created';
    END IF;

    RAISE NOTICE '';
    RAISE NOTICE 'Migration 001_attribute_registry.sql completed successfully!';
    RAISE NOTICE 'Next steps:';
    RAISE NOTICE '  1. Run 002_seed_attribute_registry.sql to populate the registry';
    RAISE NOTICE '  2. Run 003_migrate_existing_data.sql to migrate old data (if needed)';
END $$;
