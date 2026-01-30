-- Migration 066: Placeholder Entities
-- Implements placeholder entity lifecycle for structure macros
-- Design doc: TODO-cbu-structure-macros-v3.md

-- ============================================================================
-- SECTION 1: Placeholder Entity Columns
-- ============================================================================

-- Add placeholder tracking columns to entities table
ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS placeholder_status VARCHAR(20)
CHECK (placeholder_status IS NULL OR placeholder_status IN ('pending', 'resolved', 'verified', 'expired', 'rejected'));

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS placeholder_kind VARCHAR(50);

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS placeholder_created_for UUID REFERENCES "ob-poc".cbus(cbu_id);

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS placeholder_resolved_entity_id UUID REFERENCES "ob-poc".entities(entity_id);

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS placeholder_resolved_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS placeholder_expires_at TIMESTAMPTZ;

ALTER TABLE "ob-poc".entities
ADD COLUMN IF NOT EXISTS placeholder_created_by_macro VARCHAR(100);

COMMENT ON COLUMN "ob-poc".entities.placeholder_status IS
'Placeholder lifecycle state: pending (awaiting resolution), resolved (linked to real entity), verified (KYC complete)';

COMMENT ON COLUMN "ob-poc".entities.placeholder_kind IS
'Service provider role this placeholder represents: depositary, administrator, custodian, etc.';

COMMENT ON COLUMN "ob-poc".entities.placeholder_created_for IS
'CBU that this placeholder was created for (by macro expansion)';

COMMENT ON COLUMN "ob-poc".entities.placeholder_resolved_entity_id IS
'Real entity ID that resolved this placeholder';

-- ============================================================================
-- SECTION 2: Placeholder Indexes
-- ============================================================================

-- Index for finding pending placeholders (dashboard query)
CREATE INDEX IF NOT EXISTS idx_entities_placeholder_pending
ON "ob-poc".entities(placeholder_status, placeholder_created_for)
WHERE placeholder_status = 'pending';

-- Index for finding placeholders by kind
CREATE INDEX IF NOT EXISTS idx_entities_placeholder_kind
ON "ob-poc".entities(placeholder_kind, placeholder_created_for)
WHERE placeholder_status IS NOT NULL;

-- Index for finding placeholders expiring soon
CREATE INDEX IF NOT EXISTS idx_entities_placeholder_expiring
ON "ob-poc".entities(placeholder_expires_at)
WHERE placeholder_status = 'pending' AND placeholder_expires_at IS NOT NULL;

-- ============================================================================
-- SECTION 3: Placeholder Kind Reference Data
-- ============================================================================

-- Reference table for valid placeholder kinds (enables UI dropdowns)
CREATE TABLE IF NOT EXISTS "ob-poc".placeholder_kinds (
    kind VARCHAR(50) PRIMARY KEY,
    display_name VARCHAR(100) NOT NULL,
    description TEXT,
    entity_type VARCHAR(50) NOT NULL DEFAULT 'legal',
    required_for_go_live BOOLEAN NOT NULL DEFAULT true,
    sort_order INT NOT NULL DEFAULT 0
);

COMMENT ON TABLE "ob-poc".placeholder_kinds IS
'Reference data for valid placeholder entity kinds (service provider roles)';

-- Insert standard placeholder kinds
INSERT INTO "ob-poc".placeholder_kinds (kind, display_name, description, entity_type, required_for_go_live, sort_order)
VALUES
    ('depositary', 'Depositary', 'Fund depositary / custodian bank', 'legal', true, 10),
    ('administrator', 'Fund Administrator', 'Fund administration services', 'legal', true, 20),
    ('transfer-agent', 'Transfer Agent', 'Shareholder registry services', 'legal', true, 30),
    ('custodian', 'Custodian', 'Asset safekeeping services', 'legal', true, 40),
    ('auditor', 'Auditor', 'Fund auditor', 'legal', true, 50),
    ('management-company', 'Management Company', 'ManCo / AIFM', 'legal', true, 60),
    ('aifm', 'AIFM', 'Alternative Investment Fund Manager', 'legal', true, 70),
    ('general-partner', 'General Partner', 'GP for LP structures', 'legal', true, 80),
    ('investment-manager', 'Investment Manager', 'Discretionary investment manager', 'legal', false, 90),
    ('prime-broker', 'Prime Broker', 'Prime brokerage services', 'legal', false, 100),
    ('acd', 'Authorised Corporate Director', 'UK OEIC ACD', 'legal', true, 110),
    ('trustee', 'Trustee', 'Unit trust trustee', 'legal', true, 120),
    ('operator', 'ACS Operator', 'UK ACS operator', 'legal', true, 130),
    ('authorized-participant', 'Authorized Participant', 'ETF AP for creation/redemption', 'legal', false, 140),
    ('distributor', 'Distributor', 'Fund distribution services', 'legal', false, 150)
ON CONFLICT (kind) DO UPDATE
SET display_name = EXCLUDED.display_name,
    description = EXCLUDED.description,
    entity_type = EXCLUDED.entity_type,
    required_for_go_live = EXCLUDED.required_for_go_live,
    sort_order = EXCLUDED.sort_order;

-- ============================================================================
-- SECTION 4: Placeholder Functions
-- ============================================================================

-- Function to create a placeholder entity
CREATE OR REPLACE FUNCTION "ob-poc".create_placeholder_entity(
    p_kind VARCHAR(50),
    p_cbu_id UUID,
    p_macro_id VARCHAR(100) DEFAULT NULL,
    p_expires_in_days INT DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    v_placeholder_id UUID;
    v_display_name VARCHAR(200);
    v_kind_info RECORD;
BEGIN
    -- Validate kind
    SELECT * INTO v_kind_info
    FROM "ob-poc".placeholder_kinds
    WHERE kind = p_kind;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Invalid placeholder kind: %', p_kind;
    END IF;

    -- Generate display name
    v_display_name := '[Pending ' || v_kind_info.display_name || ']';

    -- Create placeholder entity
    INSERT INTO "ob-poc".entities (
        name,
        entity_type,
        placeholder_status,
        placeholder_kind,
        placeholder_created_for,
        placeholder_created_by_macro,
        placeholder_expires_at
    ) VALUES (
        v_display_name,
        v_kind_info.entity_type,
        'pending',
        p_kind,
        p_cbu_id,
        p_macro_id,
        CASE WHEN p_expires_in_days IS NOT NULL
             THEN NOW() + (p_expires_in_days || ' days')::INTERVAL
             ELSE NULL
        END
    )
    RETURNING entity_id INTO v_placeholder_id;

    RETURN v_placeholder_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".create_placeholder_entity IS
'Creates a placeholder entity for a service provider role. Returns the placeholder entity_id.';

-- Function to resolve a placeholder to a real entity
CREATE OR REPLACE FUNCTION "ob-poc".resolve_placeholder(
    p_placeholder_id UUID,
    p_real_entity_id UUID
)
RETURNS VOID AS $$
DECLARE
    v_placeholder RECORD;
    v_real_entity RECORD;
BEGIN
    -- Get placeholder info
    SELECT * INTO v_placeholder
    FROM "ob-poc".entities
    WHERE entity_id = p_placeholder_id
      AND placeholder_status = 'pending';

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Placeholder not found or not in pending state: %', p_placeholder_id;
    END IF;

    -- Get real entity info
    SELECT * INTO v_real_entity
    FROM "ob-poc".entities
    WHERE entity_id = p_real_entity_id
      AND placeholder_status IS NULL;  -- Must not be a placeholder itself

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Real entity not found or is itself a placeholder: %', p_real_entity_id;
    END IF;

    -- Update placeholder status
    UPDATE "ob-poc".entities
    SET placeholder_status = 'resolved',
        placeholder_resolved_entity_id = p_real_entity_id,
        placeholder_resolved_at = NOW()
    WHERE entity_id = p_placeholder_id;

    -- Update all role edges pointing to placeholder to point to real entity
    UPDATE "ob-poc".entity_roles
    SET entity_id = p_real_entity_id
    WHERE entity_id = p_placeholder_id;

    UPDATE "ob-poc".cbu_entity_roles
    SET entity_id = p_real_entity_id
    WHERE entity_id = p_placeholder_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION "ob-poc".resolve_placeholder IS
'Resolves a placeholder entity to a real entity, updating all role edges.';

-- ============================================================================
-- SECTION 5: Views for Placeholder Management
-- ============================================================================

-- View of pending placeholders with context
CREATE OR REPLACE VIEW "ob-poc".v_pending_placeholders AS
SELECT
    e.entity_id,
    e.name AS placeholder_name,
    e.placeholder_kind,
    pk.display_name AS kind_display_name,
    pk.required_for_go_live,
    e.placeholder_created_for AS cbu_id,
    c.name AS cbu_name,
    e.placeholder_created_by_macro AS macro_id,
    e.placeholder_expires_at,
    CASE
        WHEN e.placeholder_expires_at IS NOT NULL
             AND e.placeholder_expires_at < NOW() + INTERVAL '7 days'
        THEN true
        ELSE false
    END AS expiring_soon,
    e.created_at
FROM "ob-poc".entities e
JOIN "ob-poc".placeholder_kinds pk ON pk.kind = e.placeholder_kind
LEFT JOIN "ob-poc".cbus c ON c.cbu_id = e.placeholder_created_for
WHERE e.placeholder_status = 'pending'
ORDER BY
    e.placeholder_expires_at NULLS LAST,
    pk.sort_order;

COMMENT ON VIEW "ob-poc".v_pending_placeholders IS
'Dashboard view of pending placeholder entities with CBU context and expiry warnings';

-- View of placeholder resolution statistics
CREATE OR REPLACE VIEW "ob-poc".v_placeholder_stats AS
SELECT
    COALESCE(c.name, 'Unassigned') AS cbu_name,
    e.placeholder_created_for AS cbu_id,
    COUNT(*) FILTER (WHERE e.placeholder_status = 'pending') AS pending_count,
    COUNT(*) FILTER (WHERE e.placeholder_status = 'resolved') AS resolved_count,
    COUNT(*) FILTER (WHERE e.placeholder_status = 'verified') AS verified_count,
    COUNT(*) FILTER (WHERE e.placeholder_status = 'pending'
                      AND e.placeholder_expires_at < NOW()) AS expired_count,
    COUNT(*) FILTER (WHERE e.placeholder_status = 'pending'
                      AND pk.required_for_go_live = true) AS blocking_go_live
FROM "ob-poc".entities e
LEFT JOIN "ob-poc".cbus c ON c.cbu_id = e.placeholder_created_for
LEFT JOIN "ob-poc".placeholder_kinds pk ON pk.kind = e.placeholder_kind
WHERE e.placeholder_status IS NOT NULL
GROUP BY c.name, e.placeholder_created_for;

COMMENT ON VIEW "ob-poc".v_placeholder_stats IS
'Statistics on placeholder resolution by CBU';
