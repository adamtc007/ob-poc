-- Manco regulatory-status table promotion (2026-04-25).
--
-- Backfills the schema for the manco state machine declared in
-- cbu_dag.yaml §2.5 (R-6 G-3) and the 8 manco.* verbs declared in
-- manco-group.yaml (commit 114ccdca).
--
-- Context.
--   Pre-R-6: manco identity was tracked through entities + cbu_entity_roles
--   (role = management-company). manco_groups was a function-based view
--   (fn_get_manco_group_cbus, fn_manco_group_control_chain) — not a real
--   table, no per-manco state.
--
--   R-6 G-3 introduced a regulatory-action cascade: when a manco enters
--   UNDER_INVESTIGATION or SUSPENDED, all CBUs it manages should propagate
--   to operational SUSPENDED. That cascade requires a per-manco state to
--   read from. This migration adds the carrier table.
--
-- Design.
--   manco_regulatory_status is keyed by manco_entity_id (FK → entities)
--   rather than promoting manco_groups itself to a table. Rationale:
--   - mancos ARE entities (with role = management-company); they don't
--     have separate identity
--   - the regulatory STATE is a property of the manco entity, scoped to
--     when it acts as a manco
--   - lazy-create rows: a row exists only once a manco enters UNDER_REVIEW
--     (i.e. has been formally onboarded as a manco we're tracking)
--
-- Forward-only. No data migration — existing rows in cbu_entity_roles
-- with role = management-company are unaffected; manco state defaults
-- to "no row → not formally tracked".
--
-- Parent docs:
--   docs/todo/tranche-2-cbu-findings-2026-04-23.md §7.2 G-3
--   commits: 643dfafd (R-6 DAG), 114ccdca (8 manco.* verbs)

BEGIN;

CREATE TABLE IF NOT EXISTS "ob-poc".manco_regulatory_status (
    manco_entity_id uuid NOT NULL,
    regulatory_status character varying(30)
        DEFAULT 'UNDER_REVIEW'::character varying NOT NULL,
    flagged_reason text,
    flagged_at timestamp with time zone,
    cleared_at timestamp with time zone,
    sunset_started_at timestamp with time zone,
    terminated_at timestamp with time zone,
    notes text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT manco_regulatory_status_pkey PRIMARY KEY (manco_entity_id),
    CONSTRAINT manco_regulatory_status_entity_fk
        FOREIGN KEY (manco_entity_id)
        REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    CONSTRAINT manco_regulatory_status_check CHECK (
        (regulatory_status)::text = ANY (ARRAY[
            'UNDER_REVIEW'::character varying,
            'APPROVED'::character varying,
            'UNDER_INVESTIGATION'::character varying,
            'SUSPENDED'::character varying,
            'SUNSET'::character varying,
            'TERMINATED'::character varying
        ]::text[])
    )
);

COMMENT ON TABLE "ob-poc".manco_regulatory_status IS
    'Per-manco regulatory + operational state (R-6 G-3). Keyed by '
    'manco_entity_id (entities row with role = management-company). '
    'Lazy-create: row exists once manco is formally tracked. Cascade: '
    'UNDER_INVESTIGATION / SUSPENDED on a manco propagates SUSPENDED to '
    'all CBUs that reference it via cbu_entity_roles where role = '
    'management-company / sub-manager.';

COMMENT ON COLUMN "ob-poc".manco_regulatory_status.regulatory_status IS
    'Manco lifecycle state: UNDER_REVIEW (onboarding) → APPROVED → '
    'UNDER_INVESTIGATION (regulatory flag) → SUSPENDED (full hold) → '
    'SUNSET (no new mandates; existing run to exit) → TERMINATED.';

CREATE INDEX IF NOT EXISTS idx_manco_regulatory_status_status
    ON "ob-poc".manco_regulatory_status(regulatory_status);

-- Helper view: manco_status_with_cbu_count joins manco regulatory status
-- with the count of CBUs managed (using existing fn_get_manco_group_cbus).
-- Read-only convenience for ops dashboards / cascade planning.
CREATE OR REPLACE VIEW "ob-poc".v_manco_regulatory_status_summary AS
SELECT
    mrs.manco_entity_id,
    e.name AS manco_name,
    mrs.regulatory_status,
    mrs.flagged_at,
    mrs.flagged_reason,
    (
        SELECT COUNT(*)
        FROM "ob-poc".fn_get_manco_group_cbus(mrs.manco_entity_id)
    ) AS managed_cbu_count,
    mrs.updated_at
FROM "ob-poc".manco_regulatory_status mrs
JOIN "ob-poc".entities e ON e.entity_id = mrs.manco_entity_id;

COMMENT ON VIEW "ob-poc".v_manco_regulatory_status_summary IS
    'Per-manco regulatory state + count of managed CBUs. Read-only '
    'convenience view for ops dashboards. Cascade target preview.';

COMMIT;

-- Verification (run manually after migration):
--   SELECT table_name FROM information_schema.tables
--     WHERE table_schema = 'ob-poc'
--       AND table_name IN ('manco_regulatory_status');
--
--   SELECT conname, pg_get_constraintdef(oid)
--     FROM pg_constraint
--     WHERE conname = 'manco_regulatory_status_check';
--
--   SELECT * FROM "ob-poc".v_manco_regulatory_status_summary LIMIT 5;
