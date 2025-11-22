-- Document types for CBU model + CRUD DSLs

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category)
VALUES
  (gen_random_uuid(), 'DSL.CBU.MODEL',           'CBU Model DSL',          'DSL'),
  (gen_random_uuid(), 'DSL.CRUD.CBU.TEMPLATE',   'CBU CRUD Template DSL',  'DSL'),
  (gen_random_uuid(), 'DSL.CRUD.CBU',            'CBU CRUD Instance DSL',  'DSL')
ON CONFLICT (type_code) DO NOTHING;
