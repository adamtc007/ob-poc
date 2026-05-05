-- Service-resource data dictionary tranche 4.
--
-- Separates SRDEF governance state from coverage/materialization state. The
-- existing lifecycle_status column remains the coverage/materialization status
-- used by gap checks; governance_status tracks catalogue stewardship.

ALTER TABLE "ob-poc".service_resource_types
ADD COLUMN IF NOT EXISTS governance_status TEXT NOT NULL DEFAULT 'active'
    CHECK (governance_status IN ('draft', 'active', 'deprecated', 'retired'));

CREATE INDEX IF NOT EXISTS idx_service_resource_types_governance_status
    ON "ob-poc".service_resource_types(governance_status);

COMMENT ON COLUMN "ob-poc".service_resource_types.governance_status IS
    'Catalogue governance lifecycle for the SRDEF object: draft, active, deprecated, retired.';

COMMENT ON COLUMN "ob-poc".service_resource_types.lifecycle_status IS
    'Coverage/materialization lifecycle for SRDEF sync and attribute-gap readiness: unsynced, synced, gaps_found, complete.';
