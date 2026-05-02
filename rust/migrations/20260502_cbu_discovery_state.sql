-- 20260502_cbu_discovery_state.sql
--
-- Adds the SemOS-visible discovery pipeline slot for per-CBU service
-- resource setup. This is separate from cbus.status, which remains the
-- compliance validation lifecycle.

ALTER TABLE "ob-poc".cbus
    ADD COLUMN IF NOT EXISTS cbu_discovery_state character varying(30) DEFAULT 'PENDING' NOT NULL;

ALTER TABLE "ob-poc".cbus
    DROP CONSTRAINT IF EXISTS chk_cbu_discovery_state;

ALTER TABLE "ob-poc".cbus
    ADD CONSTRAINT chk_cbu_discovery_state CHECK (
        (cbu_discovery_state)::text = ANY (ARRAY[
            'PENDING'::text,
            'DISCOVERING'::text,
            'ROLLUP'::text,
            'POPULATE'::text,
            'PROVISION'::text,
            'READY'::text,
            'FAILED'::text,
            'BLOCKED'::text
        ])
    );

COMMENT ON COLUMN "ob-poc".cbus.cbu_discovery_state IS
    'SemOS-visible service discovery/provisioning pipeline state. PENDING | DISCOVERING | ROLLUP | POPULATE | PROVISION | READY | FAILED | BLOCKED';
