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
