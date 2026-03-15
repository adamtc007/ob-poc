BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_structure_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    child_cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    relationship_type VARCHAR(32) NOT NULL,
    relationship_selector VARCHAR(64) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    capital_flow VARCHAR(32),
    effective_from DATE,
    effective_to DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT cbu_structure_links_no_self_link CHECK (parent_cbu_id <> child_cbu_id),
    CONSTRAINT cbu_structure_links_status_check
        CHECK (status IN ('ACTIVE', 'TERMINATED', 'SUSPENDED'))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_cbu_structure_links_active
    ON "ob-poc".cbu_structure_links(parent_cbu_id, child_cbu_id, relationship_type)
    WHERE status = 'ACTIVE';

CREATE INDEX IF NOT EXISTS idx_cbu_structure_links_parent_selector
    ON "ob-poc".cbu_structure_links(parent_cbu_id, relationship_selector)
    WHERE status = 'ACTIVE';

CREATE INDEX IF NOT EXISTS idx_cbu_structure_links_child
    ON "ob-poc".cbu_structure_links(child_cbu_id);

COMMIT;
