-- Minimal CBU attribute dictionary for tests
-- Schema: "ob-poc".dictionary (name is the lookup key)

INSERT INTO "ob-poc".dictionary (name, long_description, group_id, mask, domain, sink, source)
VALUES
  ('cbu-name',            'Legal name of the CBU',           'cbu', 'string', 'CBU', '{"tables": ["cbus"]}', '{}'),
  ('jurisdiction',        'Legal jurisdiction code',         'cbu', 'string', 'CBU', '{"tables": ["cbus"]}', '{}'),
  ('nature-purpose',      'Nature and purpose of business',  'cbu', 'string', 'CBU', '{"tables": ["cbus"]}', '{}'),
  ('client-type',         'Entity type classification',      'cbu', 'string', 'CBU', '{"tables": ["cbus"]}', '{}'),
  ('registered-address',  'Registered business address',     'cbu', 'string', 'CBU', '{"tables": ["cbus"]}', '{}'),
  ('primary-contact-email','Primary contact email',          'cbu', 'string', 'CBU', '{"tables": ["cbus"]}', '{}')
ON CONFLICT (name) DO UPDATE SET 
  long_description = EXCLUDED.long_description,
  domain = EXCLUDED.domain;
