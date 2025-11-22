-- Entity types for CBU CRUD tests
-- Schema: "ob-poc".entity_types (requires name and table_name)

INSERT INTO "ob-poc".entity_types (name, description, table_name)
VALUES
  ('PROPER_PERSON', 'Natural person entity', 'proper_persons'),
  ('COMPANY',       'Corporate entity',      'companies')
ON CONFLICT (name) DO UPDATE SET
  description = EXCLUDED.description;

-- Note: roles table may not exist in current schema
-- If needed, create it or use cbu_entity_roles table instead
