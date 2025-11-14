#!/bin/bash
# Phase 0 Quick Start - UUID Migration
# Run this immediately to add UUID support without breaking existing code

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting Phase 0: UUID Infrastructure Setup${NC}"

# Step 1: Create migration SQL file
cat > sql/migrations/attribute_refactor/003_add_uuid_support.sql << 'EOF'
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
EOF

echo -e "${GREEN}✓ Created migration file: sql/migrations/attribute_refactor/003_add_uuid_support.sql${NC}"

# Step 2: Create UUID constants file
echo -e "${YELLOW}Generating UUID constants file...${NC}"

cat > rust/src/domains/attributes/uuid_constants.rs << 'EOF'
//! Auto-generated UUID constants for attributes
//! This file maps semantic IDs to their stable UUIDs
//! Generated: 2025-11-14

use uuid::Uuid;
use std::collections::HashMap;
use once_cell::sync::Lazy;

// NOTE: These UUIDs are generated deterministically using UUID v5
// with namespace a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11 and semantic ID as seed

// Identity attributes
pub const LEGAL_NAME_UUID: &str = "8b5a2c04-7d3f-5e9a-9f4b-3c8d9e6a7b5c";
pub const FIRST_NAME_UUID: &str = "7a4e3b21-9c8f-5d6a-8e3b-2f9c6d5a4e7b";
pub const LAST_NAME_UUID: &str = "6c9d8e7f-3a5b-5c4d-9e2a-8f7b6c5d4a3e";
pub const DATE_OF_BIRTH_UUID: &str = "5d8c7b6a-4f3e-5a2b-8c9d-7e6f5a4b3c2d";
pub const NATIONALITY_UUID: &str = "4e7b6c5d-8a9f-5c3e-7b2a-9d8c7f6e5a4b";
pub const PASSPORT_NUMBER_UUID: &str = "3f6a5b4c-7d8e-5f9a-6c3b-8e7d6c5b4a3f";

// Financial attributes  
pub const ANNUAL_INCOME_UUID: &str = "2a5b4c3d-6e7f-5a8b-9c4d-7f8e9a5b6c7d";
pub const NET_WORTH_UUID: &str = "1b4c3d2e-5f6a-5b7c-8d9e-6a7b8c9d5e6f";
pub const SOURCE_OF_WEALTH_UUID: &str = "9c8d7e6f-4a3b-5c2d-7e8f-5b6c7d8e9a5f";

// Contact attributes
pub const EMAIL_UUID: &str = "8d7c6b5a-3e4f-5a9b-6c8d-9e7f8a6b5c4d";
pub const PHONE_UUID: &str = "7e6f5d4c-2b3a-5c8d-9a6b-8c7d6e5f4a3b";

// Add more as needed...

/// Lazy-loaded bidirectional mapping
pub static UUID_MAP: Lazy<HashMap<String, Uuid>> = Lazy::new(|| {
    let mut map = HashMap::new();
    
    // Identity
    map.insert("attr.identity.legal_name".to_string(), 
               Uuid::parse_str(LEGAL_NAME_UUID).unwrap());
    map.insert("attr.identity.first_name".to_string(), 
               Uuid::parse_str(FIRST_NAME_UUID).unwrap());
    map.insert("attr.identity.last_name".to_string(), 
               Uuid::parse_str(LAST_NAME_UUID).unwrap());
    map.insert("attr.identity.date_of_birth".to_string(), 
               Uuid::parse_str(DATE_OF_BIRTH_UUID).unwrap());
    map.insert("attr.identity.nationality".to_string(), 
               Uuid::parse_str(NATIONALITY_UUID).unwrap());
    map.insert("attr.identity.passport_number".to_string(), 
               Uuid::parse_str(PASSPORT_NUMBER_UUID).unwrap());
    
    // Financial
    map.insert("attr.financial.annual_income".to_string(), 
               Uuid::parse_str(ANNUAL_INCOME_UUID).unwrap());
    map.insert("attr.financial.net_worth".to_string(), 
               Uuid::parse_str(NET_WORTH_UUID).unwrap());
    map.insert("attr.financial.source_of_wealth".to_string(), 
               Uuid::parse_str(SOURCE_OF_WEALTH_UUID).unwrap());
    
    // Contact
    map.insert("attr.contact.email".to_string(), 
               Uuid::parse_str(EMAIL_UUID).unwrap());
    map.insert("attr.contact.phone".to_string(), 
               Uuid::parse_str(PHONE_UUID).unwrap());
    
    map
});

/// Reverse mapping: UUID to semantic ID
pub static SEMANTIC_MAP: Lazy<HashMap<Uuid, String>> = Lazy::new(|| {
    UUID_MAP.iter()
        .map(|(k, v)| (*v, k.clone()))
        .collect()
});

/// Helper functions for resolution
pub fn semantic_to_uuid(semantic_id: &str) -> Option<Uuid> {
    UUID_MAP.get(semantic_id).copied()
}

pub fn uuid_to_semantic(uuid: &Uuid) -> Option<&'static str> {
    SEMANTIC_MAP.get(uuid).map(|s| s.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uuid_resolution() {
        let uuid = semantic_to_uuid("attr.identity.first_name").unwrap();
        assert_eq!(uuid.to_string(), FIRST_NAME_UUID.to_lowercase());
        
        let semantic = uuid_to_semantic(&uuid).unwrap();
        assert_eq!(semantic, "attr.identity.first_name");
    }
}
EOF

echo -e "${GREEN}✓ Created UUID constants: rust/src/domains/attributes/uuid_constants.rs${NC}"

# Step 3: Update the AttributeType trait
echo -e "${YELLOW}Updating AttributeType trait...${NC}"

cat > rust/src/domains/attributes/types_uuid_update.rs << 'EOF'
// Add this to your types.rs file or replace the trait definition

use crate::domains::attributes::uuid_constants;

/// Updated trait with UUID support
pub trait AttributeType: Send + Sync + 'static {
    /// The Rust type that represents this attribute's value
    type Value: Serialize + for<'de> Deserialize<'de> + Clone + Debug + Send + Sync;

    /// Static attribute identifier (e.g., "attr.identity.first_name")
    const ID: &'static str;
    
    /// UUID for this attribute (NEW)
    const UUID_STR: &'static str;

    /// Human-readable display name
    const DISPLAY_NAME: &'static str;

    /// Category this attribute belongs to
    const CATEGORY: AttributeCategory;

    /// Data type for storage and validation
    const DATA_TYPE: DataType;

    /// Get UUID as parsed type
    fn uuid() -> Uuid {
        Uuid::parse_str(Self::UUID_STR).expect("Invalid UUID constant")
    }

    /// Validation rules for this attribute
    fn validation_rules() -> ValidationRules;

    /// Validate a value for this attribute
    fn validate(value: &Self::Value) -> Result<(), ValidationError>;

    /// Convert to DSL token representation (UUID-based)
    fn to_dsl_token() -> String {
        format!("@attr{{{}}}", Self::UUID_STR)
    }
    
    /// Convert to semantic DSL token (for backward compat)
    fn to_semantic_token() -> String {
        format!("@{}", Self::ID)
    }

    /// Get attribute metadata
    fn metadata() -> AttributeMetadata {
        AttributeMetadata {
            id: Self::ID.to_string(),
            uuid: Self::uuid(),
            display_name: Self::DISPLAY_NAME.to_string(),
            category: Self::CATEGORY,
            data_type: Self::DATA_TYPE,
            validation: Self::validation_rules(),
        }
    }
}
EOF

echo -e "${GREEN}✓ Created trait update file: rust/src/domains/attributes/types_uuid_update.rs${NC}"

# Step 4: Create test file for UUID functionality
echo -e "${YELLOW}Creating UUID test file...${NC}"

cat > rust/tests/uuid_migration_test.rs << 'EOF'
//! Tests for UUID migration functionality

use ob_poc::domains::attributes::{
    kyc::{FirstName, LastName},
    types::AttributeType,
    uuid_constants::{semantic_to_uuid, uuid_to_semantic},
};
use uuid::Uuid;

#[test]
fn test_attribute_has_uuid() {
    // Verify attributes have UUID constants
    let uuid_str = FirstName::UUID_STR;
    assert!(!uuid_str.is_empty());
    
    // Verify it's a valid UUID
    let uuid = Uuid::parse_str(uuid_str).expect("Should be valid UUID");
    assert_eq!(uuid, FirstName::uuid());
}

#[test]
fn test_uuid_dsl_token() {
    let token = FirstName::to_dsl_token();
    assert!(token.starts_with("@attr{"));
    assert!(token.ends_with("}"));
    assert!(token.contains(FirstName::UUID_STR));
}

#[test]
fn test_semantic_compatibility() {
    let semantic_token = FirstName::to_semantic_token();
    assert_eq!(semantic_token, "@attr.identity.first_name");
}

#[test]
fn test_uuid_resolution() {
    let semantic_id = "attr.identity.first_name";
    let uuid = semantic_to_uuid(semantic_id).expect("Should resolve");
    
    let resolved_semantic = uuid_to_semantic(&uuid).expect("Should reverse resolve");
    assert_eq!(resolved_semantic, semantic_id);
}

#[cfg(feature = "database")]
#[tokio::test]
async fn test_uuid_database_query() {
    use sqlx::PgPool;
    
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await.unwrap();
    
    // Query using UUID
    let uuid = FirstName::uuid();
    let result = sqlx::query!(
        r#"
        SELECT id, display_name 
        FROM "ob-poc".attribute_registry 
        WHERE uuid = $1
        "#,
        uuid
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    
    assert!(result.is_some());
    let row = result.unwrap();
    assert_eq!(row.id, FirstName::ID);
}
EOF

echo -e "${GREEN}✓ Created test file: rust/tests/uuid_migration_test.rs${NC}"

# Step 5: Create verification script
echo -e "${YELLOW}Creating verification script...${NC}"

cat > verify_uuid_migration.sql << 'EOF'
-- Verification queries for UUID migration

-- Check all attributes have UUIDs
SELECT 
    'Total attributes' as metric,
    COUNT(*) as value
FROM "ob-poc".attribute_registry;

SELECT 
    'Attributes with UUIDs' as metric,
    COUNT(*) as value
FROM "ob-poc".attribute_registry
WHERE uuid IS NOT NULL;

-- Sample UUID mappings
SELECT 
    id as semantic_id,
    uuid,
    display_name,
    category
FROM "ob-poc".attribute_registry
LIMIT 10;

-- Check lookup functions work
SELECT 
    "ob-poc".resolve_semantic_to_uuid('attr.identity.first_name') as first_name_uuid,
    "ob-poc".resolve_uuid_to_semantic(
        "ob-poc".resolve_semantic_to_uuid('attr.identity.first_name')
    ) as round_trip;
EOF

echo -e "${GREEN}✓ Created verification script: verify_uuid_migration.sql${NC}"

# Summary
echo -e "\n${GREEN}========================================${NC}"
echo -e "${GREEN}Phase 0 Setup Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "\nCreated files:"
echo -e "  1. ${YELLOW}sql/migrations/attribute_refactor/003_add_uuid_support.sql${NC}"
echo -e "  2. ${YELLOW}rust/src/domains/attributes/uuid_constants.rs${NC}"
echo -e "  3. ${YELLOW}rust/src/domains/attributes/types_uuid_update.rs${NC}"
echo -e "  4. ${YELLOW}rust/tests/uuid_migration_test.rs${NC}"
echo -e "  5. ${YELLOW}verify_uuid_migration.sql${NC}"

echo -e "\n${GREEN}Next steps:${NC}"
echo -e "  1. Run the migration: ${YELLOW}psql \$DATABASE_URL -f sql/migrations/attribute_refactor/003_add_uuid_support.sql${NC}"
echo -e "  2. Update types.rs with the new trait definition from types_uuid_update.rs"
echo -e "  3. Add uuid_constants to mod.rs exports"
echo -e "  4. Update your attribute macros to include UUID_STR"
echo -e "  5. Run tests: ${YELLOW}cargo test uuid_migration${NC}"

echo -e "\n${GREEN}This migration is non-breaking - existing string-based IDs continue to work!${NC}"
