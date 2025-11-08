-- Migration 004: Rename individuals/persons to "Proper Person" terminology
-- This migration renames all references to individuals/persons to use "Proper Person" terminology

-- 1. Rename the entity_proper_persons table to entity_proper_persons
ALTER TABLE "ob-poc".entity_proper_persons RENAME TO entity_proper_persons;

-- 2. Rename the primary key column
ALTER TABLE "ob-poc".entity_proper_persons RENAME COLUMN proper_person_id TO proper_person_id;

-- 3. Update entity_types table reference
UPDATE "ob-poc".entity_types
SET
    name = 'PROPER_PERSON',
    description = 'Natural Person/Proper Person',
    table_name = 'entity_proper_persons'
WHERE name = 'PROPER_PERSON';

-- 4. Update entity_product_mappings table enum values
-- Note: This would require application-level changes for enum handling
-- For now, we'll document that PROPER_PERSON should be treated as PROPER_PERSON

-- 5. Rename indexes
ALTER INDEX IF EXISTS "ob-poc".idx_proper_persons_full_name RENAME TO idx_proper_persons_full_name;
ALTER INDEX IF EXISTS "ob-poc".idx_proper_persons_nationality RENAME TO idx_proper_persons_nationality;
ALTER INDEX IF EXISTS "ob-poc".idx_proper_persons_id_document RENAME TO idx_proper_persons_id_document;

-- 6. Update trust_parties table enum values
-- Update party_type enum values from PROPER_PERSON to PROPER_PERSON
UPDATE "ob-poc".trust_parties
SET party_type = 'PROPER_PERSON'
WHERE party_type = 'PROPER_PERSON';

-- 7. Update dictionary seed data KYC attributes
UPDATE "ob-poc".dictionary
SET
    name = REPLACE(name, 'kyc.proper_person.', 'kyc.proper_person.'),
    long_description = REPLACE(long_description, 'individual', 'proper person'),
    long_description = REPLACE(long_description, 'Proper Person', 'Proper Person')
WHERE name LIKE 'kyc.proper_person.%';

-- 8. Update entity type references in dictionary (step by step to avoid multiple assignments)
UPDATE "ob-poc".dictionary
SET long_description = REPLACE(long_description, 'individual', 'proper person')
WHERE long_description LIKE '%individual%';

UPDATE "ob-poc".dictionary
SET long_description = REPLACE(long_description, 'PROPER_PERSON', 'PROPER_PERSON')
WHERE long_description LIKE '%PROPER_PERSON%';

UPDATE "ob-poc".dictionary
SET source = REPLACE(source::text, 'kyc_proper_person', 'kyc_proper_person')::jsonb
WHERE source::text LIKE '%kyc_proper_person%';

UPDATE "ob-poc".dictionary
SET sink = REPLACE(sink::text, 'kyc_proper_person', 'kyc_proper_person')::jsonb
WHERE sink::text LIKE '%kyc_proper_person%';

-- 9. Update any remaining person/people references in comments and descriptions (step by step)
UPDATE "ob-poc".dictionary
SET long_description = REPLACE(long_description, ' person', ' proper person')
WHERE long_description LIKE '% person%' AND name NOT LIKE '%person%';

UPDATE "ob-poc".dictionary
SET long_description = REPLACE(long_description, 'person ', 'proper person ')
WHERE long_description LIKE '%person %' AND name NOT LIKE '%person%';

UPDATE "ob-poc".dictionary
SET long_description = REPLACE(long_description, 'Person', 'Proper Person')
WHERE long_description LIKE '%Person%' AND name NOT LIKE '%person%';

-- 10. Update UBO registry column comment (metadata only)
COMMENT ON COLUMN "ob-poc".ubo_registry.ubo_proper_person_id IS 'References the proper person entity who is the UBO';

-- 11. Create a view for backward compatibility (temporary)
CREATE OR REPLACE VIEW "ob-poc".entity_proper_persons AS
SELECT
    proper_person_id as proper_person_id,
    first_name,
    last_name,
    middle_names,
    date_of_birth,
    nationality,
    residence_address,
    id_document_type,
    id_document_number,
    created_at,
    updated_at
FROM "ob-poc".entity_proper_persons;

-- 12. Create schema_migrations table if it doesn't exist
CREATE TABLE IF NOT EXISTS "ob-poc".schema_migrations (
    version VARCHAR(10) PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);

-- 13. Add migration tracking
INSERT INTO "ob-poc".schema_migrations (version, description, applied_at)
VALUES ('004', 'Rename individuals to proper persons', NOW())
ON CONFLICT (version) DO NOTHING;

-- Note: This migration requires corresponding changes in the Go codebase:
-- 1. Struct names: Proper Person -> ProperPerson
-- 2. Field names: proper_person_id -> proper_person_id
-- 3. Constants: PROPER_PERSON -> PROPER_PERSON
-- 4. Function names and comments
-- 5. Test data and mock files
-- 6. Documentation updates
