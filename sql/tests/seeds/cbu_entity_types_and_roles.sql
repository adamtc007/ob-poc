-- Entity types (just enough for CBU CRUD tests)

INSERT INTO "ob-poc".entity_types (entity_type_id, type_name)
VALUES
  (gen_random_uuid(), 'PROPER_PERSON'),
  (gen_random_uuid(), 'COMPANY')
ON CONFLICT (type_name) DO NOTHING;

-- Roles (e.g. Beneficial Owner)

INSERT INTO "ob-poc".roles (role_id, role_name)
VALUES
  (gen_random_uuid(), 'BeneficialOwner')
ON CONFLICT (role_name) DO NOTHING;
