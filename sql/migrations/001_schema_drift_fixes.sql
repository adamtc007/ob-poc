-- ============================================================================
-- Pre-flight Fix: 001_schema_drift_fixes.sql
-- Purpose: Fix column mismatches between Rust code expectations and live DB
-- Run BEFORE: 002_kyc_screening_decision_monitoring_tables.sql
-- 
-- These fixes address SQLX compile-time errors where Rust code expects
-- columns that may be missing from the live database.
-- ============================================================================

BEGIN;

-- ============================================================================
-- 1. document_metadata - Ensure doc_id column exists and has FK
-- ============================================================================
-- The Rust code at crud_executor.rs:878 expects doc_id in document_metadata

DO $$
BEGIN
    -- Check if doc_id column exists
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'document_metadata' 
        AND column_name = 'doc_id'
    ) THEN
        -- Add the column if missing
        ALTER TABLE "ob-poc".document_metadata 
        ADD COLUMN doc_id uuid REFERENCES "ob-poc".document_catalog(doc_id);
        
        RAISE NOTICE 'Added doc_id column to document_metadata';
    ELSE
        RAISE NOTICE 'doc_id column already exists in document_metadata';
    END IF;
END $$;


-- ============================================================================
-- 2. document_attribute_mappings - Ensure document_type_id has proper FK
-- ============================================================================
-- The Rust code at document.rs:77 joins on dam.document_type_id

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'document_attribute_mappings' 
        AND column_name = 'document_type_id'
    ) THEN
        ALTER TABLE "ob-poc".document_attribute_mappings 
        ADD COLUMN document_type_id uuid REFERENCES "ob-poc".document_types(type_id);
        
        RAISE NOTICE 'Added document_type_id column to document_attribute_mappings';
    ELSE
        RAISE NOTICE 'document_type_id column already exists in document_attribute_mappings';
    END IF;
END $$;


-- ============================================================================
-- 3. attribute_values_typed - Ensure all typed columns exist
-- ============================================================================
-- The Rust code at source_executor.rs:120 expects value_text, value_number, etc.

DO $$
BEGIN
    -- Check entity_id
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'attribute_values_typed' 
        AND column_name = 'entity_id'
    ) THEN
        ALTER TABLE "ob-poc".attribute_values_typed 
        ADD COLUMN entity_id uuid NOT NULL REFERENCES "ob-poc".entities(entity_id);
        RAISE NOTICE 'Added entity_id column to attribute_values_typed';
    END IF;

    -- Check value_text
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'attribute_values_typed' 
        AND column_name = 'value_text'
    ) THEN
        ALTER TABLE "ob-poc".attribute_values_typed ADD COLUMN value_text text;
        RAISE NOTICE 'Added value_text column to attribute_values_typed';
    END IF;

    -- Check value_number
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'attribute_values_typed' 
        AND column_name = 'value_number'
    ) THEN
        ALTER TABLE "ob-poc".attribute_values_typed ADD COLUMN value_number numeric;
        RAISE NOTICE 'Added value_number column to attribute_values_typed';
    END IF;

    -- Check value_integer
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'attribute_values_typed' 
        AND column_name = 'value_integer'
    ) THEN
        ALTER TABLE "ob-poc".attribute_values_typed ADD COLUMN value_integer bigint;
        RAISE NOTICE 'Added value_integer column to attribute_values_typed';
    END IF;

    -- Check value_boolean
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'attribute_values_typed' 
        AND column_name = 'value_boolean'
    ) THEN
        ALTER TABLE "ob-poc".attribute_values_typed ADD COLUMN value_boolean boolean;
        RAISE NOTICE 'Added value_boolean column to attribute_values_typed';
    END IF;

    -- Check value_json
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'attribute_values_typed' 
        AND column_name = 'value_json'
    ) THEN
        ALTER TABLE "ob-poc".attribute_values_typed ADD COLUMN value_json jsonb;
        RAISE NOTICE 'Added value_json column to attribute_values_typed';
    END IF;

    -- Check attribute_uuid
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'attribute_values_typed' 
        AND column_name = 'attribute_uuid'
    ) THEN
        ALTER TABLE "ob-poc".attribute_values_typed ADD COLUMN attribute_uuid uuid;
        RAISE NOTICE 'Added attribute_uuid column to attribute_values_typed';
    END IF;
END $$;


-- ============================================================================
-- 4. document_catalog - Ensure document_type_id exists for joins
-- ============================================================================
-- The Rust code at document.rs:120 joins dc.document_type_id to dt.type_id

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'ob-poc' 
        AND table_name = 'document_catalog' 
        AND column_name = 'document_type_id'
    ) THEN
        ALTER TABLE "ob-poc".document_catalog 
        ADD COLUMN document_type_id uuid REFERENCES "ob-poc".document_types(type_id);
        
        RAISE NOTICE 'Added document_type_id column to document_catalog';
    ELSE
        RAISE NOTICE 'document_type_id column already exists in document_catalog';
    END IF;
END $$;


-- ============================================================================
-- 5. Verify schema state
-- ============================================================================

-- Show current state of critical tables
DO $$
DECLARE
    col_count INTEGER;
BEGIN
    -- document_metadata columns
    SELECT COUNT(*) INTO col_count
    FROM information_schema.columns 
    WHERE table_schema = 'ob-poc' AND table_name = 'document_metadata';
    RAISE NOTICE 'document_metadata has % columns', col_count;

    -- attribute_values_typed columns
    SELECT COUNT(*) INTO col_count
    FROM information_schema.columns 
    WHERE table_schema = 'ob-poc' AND table_name = 'attribute_values_typed';
    RAISE NOTICE 'attribute_values_typed has % columns', col_count;

    -- document_catalog columns
    SELECT COUNT(*) INTO col_count
    FROM information_schema.columns 
    WHERE table_schema = 'ob-poc' AND table_name = 'document_catalog';
    RAISE NOTICE 'document_catalog has % columns', col_count;
END $$;

COMMIT;

-- ============================================================================
-- Verification queries (run manually after migration)
-- ============================================================================
-- 
-- \d "ob-poc".document_metadata
-- \d "ob-poc".attribute_values_typed  
-- \d "ob-poc".document_catalog
-- \d "ob-poc".document_attribute_mappings
--
-- ============================================================================
