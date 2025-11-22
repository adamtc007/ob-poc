-- Minimal CBU attribute dictionary for tests

INSERT INTO "ob-poc".dictionary (attribute_id, name, sink, source)
VALUES
  ('CBU.LEGAL_NAME',           'Legal Name',           '{"sink": ["CBU"]}', '{}' ),
  ('CBU.LEGAL_JURISDICTION',   'Legal Jurisdiction',   '{"sink": ["CBU"]}', '{}' ),
  ('CBU.NATURE_PURPOSE',       'Nature Purpose',       '{"sink": ["CBU"]}', '{}' ),
  ('CBU.REGISTERED_ADDRESS',   'Registered Address',   '{"sink": ["CBU"]}', '{}' ),
  ('CBU.PRIMARY_CONTACT_EMAIL','Primary Contact Email','{"sink": ["CBU"]}', '{}' )
ON CONFLICT (attribute_id) DO NOTHING;
