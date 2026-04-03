-- Cross-Workspace State Consistency: Phase 8 — Provider Capabilities
-- Reference data classifying third-party provider operations for replay behaviour.
-- See: docs/architecture/cross-workspace-state-consistency-v0.4.md §6.7

CREATE TABLE IF NOT EXISTS "ob-poc".provider_capabilities (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider        TEXT NOT NULL,
    operation       TEXT NOT NULL,
    capability      TEXT NOT NULL,
    amend_details   JSONB,
    notes           TEXT,

    CONSTRAINT chk_provider_capability
        CHECK (capability IN ('amendable', 'cancel_and_recreate', 'immutable', 'manual'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_capabilities_unique
    ON "ob-poc".provider_capabilities (provider, operation);

-- Seed initial provider capabilities
INSERT INTO "ob-poc".provider_capabilities (provider, operation, capability, notes) VALUES
    ('gleif', 'lei_lookup', 'amendable', 'GLEIF API supports re-query with updated parameters'),
    ('gleif', 'hierarchy_import', 'amendable', 'Re-import overwrites prior hierarchy data'),
    ('screening', 'sanctions_check', 'amendable', 'Re-screen with updated entity data'),
    ('screening', 'pep_check', 'amendable', 'Re-screen with updated entity data'),
    ('settlement', 'instruction_registration', 'cancel_and_recreate', 'Cancel prior instruction, create new'),
    ('custody', 'account_opening', 'amendable', 'Most modern custody APIs support PUT/amend'),
    ('regulatory', 'filing_submission', 'immutable', 'Submit correction referencing original filing'),
    ('legacy_custodian', 'account_opening', 'manual', 'Phone/email correction required')
ON CONFLICT (provider, operation) DO NOTHING;

COMMENT ON TABLE "ob-poc".provider_capabilities IS
    'Reference data: per-provider, per-operation correction classification for replay behaviour (amendable, cancel_and_recreate, immutable, manual).';
