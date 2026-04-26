-- Booking Principal clearance lifecycle (R3 — 2026-04-26).
--
-- Adam's three deal tollgates are BAC + KYC + Booking Principals. BAC and
-- KYC are state-machined (deal.deal_status BAC_APPROVAL / cases.status);
-- this migration adds the third (BP clearance) as a per-deal-per-principal
-- state carrier.
--
-- Decision: per-deal-per-principal scope (Adam option (a)) — NOT
-- per-principal-global. UNIQUE (booking_principal_id, deal_id, cbu_id)
-- enforces the triple. Each deal must clear each contributing booking
-- principal independently.
--
-- Backfill: every existing CONTRACTED deal gets one ACTIVE clearance row
-- per booking_principal that has an active client_principal_relationship
-- with the deal's primary_client_group_id. Assumption: there is no
-- direct deals→booking_principal FK in the current schema; the only
-- declared link path is (deals.primary_client_group_id) →
-- (client_principal_relationship.client_group_id) →
-- (client_principal_relationship.booking_principal_id). If a CONTRACTED
-- deal has no matching CPR row, no backfill row is created (the deal is
-- already past the gate; new clearances can be authored manually post
-- hoc if needed).
--
-- Forward-only. The DAG side is wired in deal_dag.yaml §2.2.1 +
-- cross_workspace_constraints.deal_contracted_requires_bp_approved.
--
-- Parent docs:
--   docs/todo/onboarding-dag-remediation-plan-2026-04-26.md §"Slice R3"

BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".booking_principal_clearances (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    booking_principal_id uuid NOT NULL,
    deal_id uuid,
    cbu_id uuid,
    clearance_status varchar(20) NOT NULL DEFAULT 'PENDING',
    screening_started_at timestamp with time zone,
    approved_at timestamp with time zone,
    rejected_at timestamp with time zone,
    rejection_reason text,
    activated_at timestamp with time zone,
    suspended_at timestamp with time zone,
    suspension_reason text,
    revoked_at timestamp with time zone,
    notes text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT booking_principal_clearances_status_check CHECK (
        clearance_status IN (
            'PENDING','SCREENING','APPROVED','REJECTED',
            'ACTIVE','SUSPENDED','REVOKED'
        )
    ),
    CONSTRAINT booking_principal_clearances_bp_fk
        FOREIGN KEY (booking_principal_id)
        REFERENCES "ob-poc".booking_principal(booking_principal_id)
        ON DELETE CASCADE,
    CONSTRAINT booking_principal_clearances_deal_fk
        FOREIGN KEY (deal_id)
        REFERENCES "ob-poc".deals(deal_id)
        ON DELETE CASCADE,
    CONSTRAINT booking_principal_clearances_cbu_fk
        FOREIGN KEY (cbu_id)
        REFERENCES "ob-poc".cbus(cbu_id)
        ON DELETE SET NULL,
    -- Per-deal-per-principal scope (Adam option (a)). NULLs are treated
    -- as distinct by Postgres so deals with no CBU pin can still hold a
    -- clearance row per (bp, deal).
    CONSTRAINT booking_principal_clearances_triple_unique
        UNIQUE (booking_principal_id, deal_id, cbu_id)
);

COMMENT ON TABLE "ob-poc".booking_principal_clearances IS
    'Per-(deal, booking_principal) clearance lifecycle (R3, 2026-04-26). '
    'Third leg of Adam''s deal tollgate triad: BAC + KYC + BP. '
    'States: PENDING → SCREENING → APPROVED/REJECTED → ACTIVE → SUSPENDED → REVOKED. '
    'APPROVED or ACTIVE required to gate deal KYC_CLEARANCE → CONTRACTED.';

COMMENT ON COLUMN "ob-poc".booking_principal_clearances.clearance_status IS
    'BP clearance lifecycle: PENDING (entry) → SCREENING → APPROVED → '
    'ACTIVE | REJECTED (reopenable) | SUSPENDED ↔ ACTIVE | REVOKED (terminal).';

CREATE INDEX IF NOT EXISTS idx_bp_clearance_status
    ON "ob-poc".booking_principal_clearances(clearance_status);

CREATE INDEX IF NOT EXISTS idx_bp_clearance_deal
    ON "ob-poc".booking_principal_clearances(deal_id)
    WHERE deal_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bp_clearance_cbu
    ON "ob-poc".booking_principal_clearances(cbu_id)
    WHERE cbu_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bp_clearance_principal
    ON "ob-poc".booking_principal_clearances(booking_principal_id);

-- ---------------------------------------------------------------------------
-- Backfill: existing CONTRACTED (or further-progressed) deals are assumed
-- to have implicit historical BP clearance. Insert one ACTIVE clearance
-- row per (deal, booking_principal) pair derived from
-- client_principal_relationship for the deal's primary_client_group_id.
--
-- Assumption (documented above): no direct deals.booking_principal_id FK
-- exists; the linkage is via primary_client_group_id ↔ CPR. If a
-- CONTRACTED deal has no matching CPR row, no backfill is attempted —
-- such deals already passed the gate historically and need no carrier.
-- ---------------------------------------------------------------------------
INSERT INTO "ob-poc".booking_principal_clearances (
    booking_principal_id, deal_id, cbu_id, clearance_status,
    activated_at, notes
)
SELECT DISTINCT
    cpr.booking_principal_id,
    d.deal_id,
    NULL::uuid AS cbu_id,
    'ACTIVE',
    COALESCE(d.contracted_at, d.created_at, now()),
    'Backfilled by migration 20260429 — historical clearance for pre-existing CONTRACTED+ deal.'
FROM "ob-poc".deals d
JOIN "ob-poc".client_principal_relationship cpr
    ON cpr.client_group_id = d.primary_client_group_id
   AND cpr.relationship_status = 'active'
WHERE d.deal_status IN ('CONTRACTED','ONBOARDING','ACTIVE','WINDING_DOWN','OFFBOARDED')
ON CONFLICT (booking_principal_id, deal_id, cbu_id) DO NOTHING;

COMMIT;

-- Verification (run manually after migration):
--   SELECT table_name FROM information_schema.tables
--     WHERE table_schema = 'ob-poc'
--       AND table_name = 'booking_principal_clearances';
--
--   SELECT clearance_status, COUNT(*)
--     FROM "ob-poc".booking_principal_clearances
--     GROUP BY clearance_status;
--
--   SELECT conname, pg_get_constraintdef(oid)
--     FROM pg_constraint
--     WHERE conname LIKE 'booking_principal_clearances%';
