-- Migration to refactor deal_slas to reference service attributes directly (service_attribute_id)
-- and remove coverage_banker_entity_id from ob-poc.deals.

BEGIN;

-- 1. Remove coverage_banker_entity_id column from ob-poc.deals
ALTER TABLE "ob-poc".deals DROP COLUMN IF EXISTS coverage_banker_entity_id;

-- 2. Refactor deal_slas table: remove service_id, add service_attribute_id
ALTER TABLE "ob-poc".deal_slas DROP COLUMN IF EXISTS service_id;
ALTER TABLE "ob-poc".deal_slas ADD COLUMN service_attribute_id UUID REFERENCES "ob-poc".attribute_registry(uuid);

COMMIT;
