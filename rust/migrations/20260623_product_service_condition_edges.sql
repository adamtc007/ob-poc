-- Product-service conditional edge support for pure Resolve(cbu, products).
--
-- product_service_conditions originally held reusable option predicates only.
-- Phase 1 resolve needs the same governed predicate table to also represent
-- conditional product-service membership, while preserving the existing option
-- condition reference surface.

BEGIN;

ALTER TABLE "ob-poc".product_service_conditions
    ALTER COLUMN predicate DROP NOT NULL,
    ADD COLUMN IF NOT EXISTS product_id uuid REFERENCES "ob-poc".products(product_id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS service_id uuid REFERENCES "ob-poc".services(service_id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS is_mandatory boolean DEFAULT false,
    ADD COLUMN IF NOT EXISTS is_default boolean DEFAULT false,
    ADD COLUMN IF NOT EXISTS display_order integer,
    ADD COLUMN IF NOT EXISTS configuration jsonb DEFAULT '{}'::jsonb;

CREATE UNIQUE INDEX IF NOT EXISTS idx_product_service_conditions_edge_key
    ON "ob-poc".product_service_conditions(product_id, service_id, condition_key)
    WHERE product_id IS NOT NULL AND service_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_product_service_conditions_active_edge
    ON "ob-poc".product_service_conditions(product_id, lifecycle_status)
    WHERE product_id IS NOT NULL AND service_id IS NOT NULL;

COMMENT ON COLUMN "ob-poc".product_service_conditions.product_id IS
    'Optional product key when the condition represents conditional service membership.';

COMMENT ON COLUMN "ob-poc".product_service_conditions.service_id IS
    'Optional service key when the condition represents conditional service membership.';

COMMIT;
