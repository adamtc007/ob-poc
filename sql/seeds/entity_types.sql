-- Entity Types Seed Data
-- Run: psql $DATABASE_URL -f sql/seeds/entity_types.sql

INSERT INTO "ob-poc".entity_types (entity_type_id, name, table_name, description)
VALUES
  (gen_random_uuid(), 'PROPER_PERSON', 'entity_proper_persons', 'Natural person / individual'),
  (gen_random_uuid(), 'LIMITED_COMPANY', 'entity_limited_companies', 'Limited liability company'),
  (gen_random_uuid(), 'PARTNERSHIP', 'entity_partnerships', 'Partnership (LP, LLP, GP)'),
  (gen_random_uuid(), 'TRUST', 'entity_trusts', 'Trust structure'),
  (gen_random_uuid(), 'SICAV', 'entity_sicavs', 'Societe d''investissement a capital variable'),
  (gen_random_uuid(), 'SPV', 'entity_spvs', 'Special purpose vehicle'),
  (gen_random_uuid(), 'FUND', 'entity_funds', 'Investment fund'),
  (gen_random_uuid(), 'SOVEREIGN_WEALTH_FUND', 'entity_sovereign_wealth_funds', 'Government investment fund')
ON CONFLICT (name) DO NOTHING;
