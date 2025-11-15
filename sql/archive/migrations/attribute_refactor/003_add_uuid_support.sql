-- Phase 0: Add UUID Support to Existing Attribute System
-- This migration adds UUID columns without breaking existing string-based IDs
-- Date: 2025-11-14

BEGIN;

-- Enable UUID extension if not already enabled
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================================
-- PART 1: Add UUID column to attribute_registry
-- ============================================================================

ALTER TABLE "ob-poc".attribute_registry 
ADD COLUMN IF NOT EXISTS uuid UUID;

-- Generate deterministic UUIDs from semantic IDs
-- Using UUID v5 with a namespace to ensure consistency
UPDATE "ob-poc".attribute_registry 
SET uuid = uuid_generate_v5(
    'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'::uuid,  -- namespace UUID
    id  -- semantic ID as seed
)
WHERE uuid IS NULL;

-- Make UUID required and unique
ALTER TABLE "ob-poc".attribute_registry 
ALTER COLUMN uuid SET NOT NULL;

ALTER TABLE "ob-poc".attribute_registry 
ADD CONSTRAINT uk_attribute_uuid UNIQUE (uuid);

-- Create indexes for efficient lookups
CREATE INDEX IF NOT EXISTS idx_attr_uuid 
ON "ob-poc".attribute_registry(uuid);

-- ============================================================================
-- PART 2: Add UUID support to attribute_values_typed
-- ============================================================================

ALTER TABLE "ob-poc".attribute_values_typed
ADD COLUMN IF NOT EXISTS attribute_uuid UUID;

-- Populate UUIDs from registry
UPDATE "ob-poc".attribute_values_typed av
SET attribute_uuid = ar.uuid
FROM "ob-poc".attribute_registry ar
WHERE av.attribute_id = ar.id
AND av.attribute_uuid IS NULL;

-- Add foreign key constraint
ALTER TABLE "ob-poc".attribute_values_typed
ADD CONSTRAINT fk_attribute_uuid 
FOREIGN KEY (attribute_uuid) REFERENCES "ob-poc".attribute_registry(uuid);

-- Create index for UUID-based queries
CREATE INDEX IF NOT EXISTS idx_values_attr_uuid 
ON "ob-poc".attribute_values_typed(attribute_uuid);

-- ============================================================================
-- PART 3: Create UUID lookup functions
-- ============================================================================

-- Function to resolve UUID to semantic ID
CREATE OR REPLACE FUNCTION "ob-poc".resolve_uuid_to_semantic(attr_uuid UUID)
RETURNS TEXT AS $$
    SELECT id FROM "ob-poc".attribute_registry WHERE uuid = attr_uuid;
$$ LANGUAGE SQL STABLE;

-- Function to resolve semantic ID to UUID
CREATE OR REPLACE FUNCTION "ob-poc".resolve_semantic_to_uuid(semantic_id TEXT)
RETURNS UUID AS $$
    SELECT uuid FROM "ob-poc".attribute_registry WHERE id = semantic_id;
$$ LANGUAGE SQL STABLE;

-- Create view for easy UUID mapping
CREATE OR REPLACE VIEW "ob-poc".attribute_uuid_map AS
SELECT 
    id as semantic_id,
    uuid,
    display_name,
    category,
    value_type
FROM "ob-poc".attribute_registry
ORDER BY id;

COMMIT;

-- Verify the migration
SELECT 
    'Attributes with UUIDs' as check_name,
    COUNT(*) as count
FROM "ob-poc".attribute_registry
WHERE uuid IS NOT NULL

UNION ALL

SELECT 
    'Unique UUIDs' as check_name,
    COUNT(DISTINCT uuid) as count
FROM "ob-poc".attribute_registry

UNION ALL

SELECT 
    'Values with UUID references' as check_name,
    COUNT(*) as count
FROM "ob-poc".attribute_values_typed
WHERE attribute_uuid IS NOT NULL;
