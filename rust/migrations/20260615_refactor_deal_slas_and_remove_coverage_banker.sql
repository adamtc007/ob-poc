-- Migration to refactor deal_slas to reference service attributes directly (service_attribute_id)
-- and remove coverage_banker_entity_id from ob-poc.deals.

BEGIN;

-- 1. Remove coverage_banker_entity_id column from ob-poc.deals
ALTER TABLE "ob-poc".deals DROP COLUMN IF EXISTS coverage_banker_entity_id;

-- 2. Refactor deal_slas table: rename service_id -> service_attribute_id.
-- (Fixed pre-apply, Phase 0 state-graph remediation: this migration originally
-- attempted DROP+ADD with `REFERENCES "ob-poc".attribute_registry(uuid)`, but
-- attribute_registry's PK is `id text`, not a uuid column -- that FK target
-- does not exist. The column is a plain FK to services(service_id); a rename
-- preserves the existing FK constraint and matches the Rust binding
-- (sem_os_postgres::ops::deal::AddSla binds it as a plain Uuid).
ALTER TABLE "ob-poc".deal_slas RENAME COLUMN service_id TO service_attribute_id;

COMMIT;
