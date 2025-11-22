-- Document types for CBU model + CRUD DSLs
-- Schema: "ob-poc".document_types

INSERT INTO "ob-poc".document_types (type_code, display_name, category, description)
VALUES
  ('DSL.CBU.MODEL',           'CBU Model DSL',          'DSL', 'CBU Model specification defining states, transitions, and attribute chunks'),
  ('DSL.CRUD.CBU.TEMPLATE',   'CBU CRUD Template DSL',  'DSL', 'Parametrized CBU CRUD recipe with placeholders'),
  ('DSL.CRUD.CBU',            'CBU CRUD Instance DSL',  'DSL', 'Concrete CBU CRUD execution document with values')
ON CONFLICT (type_code) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  description = EXCLUDED.description;
