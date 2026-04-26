-- Tranche 2 + Tranche 3 DAG schema alignment (2026-04-24).
--
-- Post-R-3 / R-4 / R-5 / R-6 / T3-* migration window per D-2. The DAG
-- taxonomies have been leading the schema since 2026-04-23; this
-- migration brings the schema CHECK constraints + columns + tables
-- up to the states referenced in the DAGs.
--
-- Forward-only. Additive. No data migration needed — existing rows
-- stay valid.
--
-- Parent docs:
--   docs/todo/catalogue-platform-refinement-v1_3.md (spec)
--   docs/todo/tranche-2-cross-workspace-reconciliation-2026-04-24.md (§8 D-2)
--   commits: 9a55413e (R-3 CBU re-centring), f36519b6 (R-4 IM re-anchor),
--            ddd0c27a (R-5 Deal targeted), 643dfafd (R-6 CBU small gaps),
--            48550e2b (T3-B book-setup), 212c0733 (T3-S SemOS)
--
-- Sections:
--   1. CBU: operational_status + disposition_status + new child tables
--   2. Share classes: lifecycle_status
--   3. Deal: expand deals_status_check, deal_rate_cards_status_check;
--            add deal_slas.sla_status; internal accountability columns;
--            parent_deal_id hierarchy
--   4. Book-Setup: new client_books table + cbus.book_id FK
--
-- Deferred (not in this migration):
--   - manco_groups regulatory_status — manco_groups isn't a real table
--     (it's a function-based view); needs design discussion before
--     promoting to a real table
--   - V1.3-1 cross_workspace_constraints runtime enforcement — wiring
--     is validator-level; runtime gate execution is a separate task

BEGIN;

-- =============================================================================
-- 1. CBU: dual_lifecycle operational chain + disposition + new child tables
-- =============================================================================

-- 1.1 cbus.operational_status — the CBU operational lifecycle
-- (post-VALIDATED; per R-3 CBU DAG dual_lifecycle §2.1).
ALTER TABLE "ob-poc".cbus
    ADD COLUMN IF NOT EXISTS operational_status character varying(30);

-- Default is NULL until the CBU reaches VALIDATED and enters operational
-- lifecycle. Once in operational lifecycle, only these 8 states valid.
ALTER TABLE "ob-poc".cbus
    DROP CONSTRAINT IF EXISTS chk_cbu_operational_status;

ALTER TABLE "ob-poc".cbus
    ADD CONSTRAINT chk_cbu_operational_status CHECK (
        (operational_status IS NULL) OR
        ((operational_status)::text = ANY (ARRAY[
            'dormant'::character varying,
            'trade_permissioned'::character varying,
            'actively_trading'::character varying,
            'restricted'::character varying,
            'suspended'::character varying,
            'winding_down'::character varying,
            'offboarded'::character varying,
            'archived'::character varying
        ]::text[]))
    );

COMMENT ON COLUMN "ob-poc".cbus.operational_status IS
    'Post-VALIDATED operational-lifecycle state (R-3 dual_lifecycle). '
    'NULL while CBU is still in discovery lifecycle (cbus.status != VALIDATED). '
    'dormant | trade_permissioned | actively_trading | restricted | suspended | '
    'winding_down | offboarded | archived';

-- 1.2 cbus.disposition_status — administrative disposition (R-6 G-11+G-13)
ALTER TABLE "ob-poc".cbus
    ADD COLUMN IF NOT EXISTS disposition_status character varying(30)
        DEFAULT 'active'::character varying;

ALTER TABLE "ob-poc".cbus
    DROP CONSTRAINT IF EXISTS chk_cbu_disposition_status;

ALTER TABLE "ob-poc".cbus
    ADD CONSTRAINT chk_cbu_disposition_status CHECK (
        (disposition_status)::text = ANY (ARRAY[
            'active'::character varying,
            'under_remediation'::character varying,
            'soft_deleted'::character varying,
            'hard_deleted'::character varying
        ]::text[])
    );

COMMENT ON COLUMN "ob-poc".cbus.disposition_status IS
    'Administrative disposition (R-6 G-11+G-13). Orthogonal to operational_status. '
    'active | under_remediation | soft_deleted | hard_deleted';

-- 1.3 cbu_service_consumption table (R-3 foundational concern #3).
-- Per-(cbu, service_kind) provisioning lifecycle.
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_service_consumption (
    consumption_id uuid DEFAULT uuidv7() NOT NULL,
    cbu_id uuid NOT NULL,
    service_kind character varying(40) NOT NULL,
    status character varying(30) DEFAULT 'proposed'::character varying NOT NULL,
    provisioned_at timestamp with time zone,
    activated_at timestamp with time zone,
    retired_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT cbu_service_consumption_pkey PRIMARY KEY (consumption_id),
    CONSTRAINT cbu_service_consumption_cbu_fk
        FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    CONSTRAINT cbu_service_consumption_status_check CHECK (
        (status)::text = ANY (ARRAY[
            'proposed'::character varying,
            'provisioned'::character varying,
            'active'::character varying,
            'suspended'::character varying,
            'winding_down'::character varying,
            'retired'::character varying
        ]::text[])
    ),
    CONSTRAINT cbu_service_consumption_service_kind_check CHECK (
        (service_kind)::text = ANY (ARRAY[
            'CUSTODY'::character varying,
            'TA'::character varying,
            'FA'::character varying,
            'SEC_LENDING'::character varying,
            'FX'::character varying,
            'TRADING'::character varying,
            'REPORTING'::character varying,
            'PRICING'::character varying,
            'COLLATERAL'::character varying
        ]::text[])
    ),
    CONSTRAINT cbu_service_consumption_unique_kind UNIQUE (cbu_id, service_kind)
);

COMMENT ON TABLE "ob-poc".cbu_service_consumption IS
    'Per-(cbu, service_kind) service provisioning lifecycle (R-3). '
    'CBU consumes services to operate on the street; this table tracks which '
    'services are in which state for each CBU.';

CREATE INDEX IF NOT EXISTS idx_cbu_service_consumption_cbu
    ON "ob-poc".cbu_service_consumption(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_service_consumption_status
    ON "ob-poc".cbu_service_consumption(status);

-- 1.4 cbu_trading_activity table (R-4 IM trading_activity slot).
-- Per-CBU first-trade + dormancy detection. Projects into CBU tollgate.
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_trading_activity (
    cbu_id uuid NOT NULL,
    activity_state character varying(30) DEFAULT 'never_traded'::character varying NOT NULL,
    first_trade_at timestamp with time zone,
    last_trade_at timestamp with time zone,
    dormancy_window_days integer DEFAULT 90,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT cbu_trading_activity_pkey PRIMARY KEY (cbu_id),
    CONSTRAINT cbu_trading_activity_cbu_fk
        FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    CONSTRAINT cbu_trading_activity_state_check CHECK (
        (activity_state)::text = ANY (ARRAY[
            'never_traded'::character varying,
            'trading'::character varying,
            'dormant'::character varying,
            'suspended'::character varying
        ]::text[])
    )
);

COMMENT ON TABLE "ob-poc".cbu_trading_activity IS
    'Per-CBU trading-activity signal (R-4 IM slot trading_activity). '
    'Drives overall_lifecycle.actively_trading phase and CBU operationally_active tollgate. '
    'Populated by settlement pipeline / trade-posting events.';

-- 1.5 cbu_corporate_action_events table (R-6 G-9).
-- CBU-level CAs: rename, redomiciliation, merger, fund-type conversion.
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_corporate_action_events (
    event_id uuid DEFAULT uuidv7() NOT NULL,
    cbu_id uuid NOT NULL,
    event_type character varying(40) NOT NULL,
    ca_status character varying(30) DEFAULT 'proposed'::character varying NOT NULL,
    effective_date date,
    description text,
    proposed_by uuid,
    approved_by uuid,
    rejected_reason text,
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT cbu_corporate_action_events_pkey PRIMARY KEY (event_id),
    CONSTRAINT cbu_ca_events_cbu_fk
        FOREIGN KEY (cbu_id) REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    CONSTRAINT cbu_ca_events_status_check CHECK (
        (ca_status)::text = ANY (ARRAY[
            'proposed'::character varying,
            'under_review'::character varying,
            'approved'::character varying,
            'effective'::character varying,
            'implemented'::character varying,
            'rejected'::character varying,
            'withdrawn'::character varying
        ]::text[])
    ),
    CONSTRAINT cbu_ca_events_type_check CHECK (
        (event_type)::text = ANY (ARRAY[
            'rename'::character varying,
            'redomiciliation'::character varying,
            'merger'::character varying,
            'conversion'::character varying,
            'restructuring'::character varying
        ]::text[])
    )
);

COMMENT ON TABLE "ob-poc".cbu_corporate_action_events IS
    'CBU-level corporate-action events (R-6 G-9). Rename, redomiciliation, '
    'merger with another CBU, fund-type conversion, restructuring. '
    'Distinct from instrument-level CAs (IM workspace corporate_action_event slot).';

CREATE INDEX IF NOT EXISTS idx_cbu_ca_events_cbu
    ON "ob-poc".cbu_corporate_action_events(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_ca_events_status
    ON "ob-poc".cbu_corporate_action_events(ca_status);

-- =============================================================================
-- 2. SHARE CLASSES: lifecycle_status column + CHECK (R-6 G-7)
-- =============================================================================

ALTER TABLE "ob-poc".share_classes
    ADD COLUMN IF NOT EXISTS lifecycle_status character varying(30)
        DEFAULT 'DRAFT'::character varying;

ALTER TABLE "ob-poc".share_classes
    DROP CONSTRAINT IF EXISTS share_classes_lifecycle_status_check;

ALTER TABLE "ob-poc".share_classes
    ADD CONSTRAINT share_classes_lifecycle_status_check CHECK (
        (lifecycle_status)::text = ANY (ARRAY[
            'DRAFT'::character varying,
            'OPEN'::character varying,
            'SOFT_CLOSED'::character varying,
            'HARD_CLOSED'::character varying,
            'WINDING_DOWN'::character varying,
            'LIQUIDATED'::character varying
        ]::text[])
    );

COMMENT ON COLUMN "ob-poc".share_classes.lifecycle_status IS
    'Share-class subscription-availability lifecycle (R-6 G-7). '
    'DRAFT (pre-launch) | OPEN (accepting subs) | SOFT_CLOSED (no new subs) | '
    'HARD_CLOSED (no subs or redemptions) | WINDING_DOWN | LIQUIDATED';

-- =============================================================================
-- 3. DEAL: expand status enums, SLA lifecycle, internal accountability, hierarchy
-- =============================================================================

-- 3.1 Expand deals_status_check (R-5 G-1 + G-3)
ALTER TABLE "ob-poc".deals
    DROP CONSTRAINT IF EXISTS deals_status_check;

ALTER TABLE "ob-poc".deals
    ADD CONSTRAINT deals_status_check CHECK (
        (deal_status)::text = ANY (ARRAY[
            'PROSPECT'::character varying,
            'QUALIFYING'::character varying,
            'NEGOTIATING'::character varying,
            'BAC_APPROVAL'::character varying,          -- R-5 G-1 BAC gate
            'KYC_CLEARANCE'::character varying,
            'CONTRACTED'::character varying,
            'ONBOARDING'::character varying,
            'ACTIVE'::character varying,
            'SUSPENDED'::character varying,              -- R-5 G-5 SUSPENDED
            'WINDING_DOWN'::character varying,
            'OFFBOARDED'::character varying,
            'CANCELLED'::character varying,
            'LOST'::character varying,                   -- R-5 G-3 terminal granularity
            'REJECTED'::character varying,
            'WITHDRAWN'::character varying
        ]::text[])
    );

-- 3.2 Expand deal_rate_cards_status_check (R-5 G-2 pricing approval)
ALTER TABLE "ob-poc".deal_rate_cards
    DROP CONSTRAINT IF EXISTS deal_rate_cards_status_check;

ALTER TABLE "ob-poc".deal_rate_cards
    ADD CONSTRAINT deal_rate_cards_status_check CHECK (
        (status)::text = ANY (ARRAY[
            'DRAFT'::character varying,
            'PENDING_INTERNAL_APPROVAL'::character varying,  -- R-5 G-2
            'APPROVED_INTERNALLY'::character varying,
            'PROPOSED'::character varying,
            'COUNTER_PROPOSED'::character varying,
            'AGREED'::character varying,
            'SUPERSEDED'::character varying,
            'CANCELLED'::character varying
        ]::text[])
    );

-- 3.3 deal_slas.sla_status column + CHECK (R-5 G-7)
ALTER TABLE "ob-poc".deal_slas
    ADD COLUMN IF NOT EXISTS sla_status character varying(30)
        DEFAULT 'NEGOTIATED'::character varying NOT NULL;

ALTER TABLE "ob-poc".deal_slas
    DROP CONSTRAINT IF EXISTS deal_slas_sla_status_check;

ALTER TABLE "ob-poc".deal_slas
    ADD CONSTRAINT deal_slas_sla_status_check CHECK (
        (sla_status)::text = ANY (ARRAY[
            'NEGOTIATED'::character varying,
            'ACTIVE'::character varying,
            'BREACHED'::character varying,
            'IN_REMEDIATION'::character varying,
            'RESOLVED'::character varying,
            'WAIVED'::character varying
        ]::text[])
    );

COMMENT ON COLUMN "ob-poc".deal_slas.sla_status IS
    'SLA commitment lifecycle (R-5 G-7). NEGOTIATED (pre-contract) → '
    'ACTIVE (measured) → BREACHED → IN_REMEDIATION → RESOLVED (+ WAIVED).';

-- 3.4 Deal internal accountability columns (R-5 G-8)
ALTER TABLE "ob-poc".deals
    ADD COLUMN IF NOT EXISTS sponsor_entity_id uuid,
    ADD COLUMN IF NOT EXISTS rm_entity_id uuid,
    ADD COLUMN IF NOT EXISTS coverage_banker_entity_id uuid;

COMMENT ON COLUMN "ob-poc".deals.sponsor_entity_id IS
    'Internal deal sponsor — commercial owner on our side. R-5 G-8.';
COMMENT ON COLUMN "ob-poc".deals.rm_entity_id IS
    'Relationship manager — owns client relationship. R-5 G-8.';
COMMENT ON COLUMN "ob-poc".deals.coverage_banker_entity_id IS
    'Coverage banker — cross-sell owner. R-5 G-8.';

-- 3.5 Deal hierarchy: parent_deal_id FK (G-9 partial)
-- Allows master/schedule/addendum deal hierarchy.
ALTER TABLE "ob-poc".deals
    ADD COLUMN IF NOT EXISTS parent_deal_id uuid;

ALTER TABLE "ob-poc".deals
    DROP CONSTRAINT IF EXISTS deals_parent_deal_fk;

ALTER TABLE "ob-poc".deals
    ADD CONSTRAINT deals_parent_deal_fk
        FOREIGN KEY (parent_deal_id) REFERENCES "ob-poc".deals(deal_id);

COMMENT ON COLUMN "ob-poc".deals.parent_deal_id IS
    'Parent deal (master agreement) for schedule/addendum/side-letter deals. '
    'Child deal state must be consistent with parent per V1.3-3 state_dependency.';

CREATE INDEX IF NOT EXISTS idx_deals_parent_deal
    ON "ob-poc".deals(parent_deal_id) WHERE parent_deal_id IS NOT NULL;

-- =============================================================================
-- 4. BOOK-SETUP: client_books table + cbus.book_id FK (T3-B)
-- =============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".client_books (
    book_id uuid DEFAULT uuidv7() NOT NULL,
    client_group_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    status character varying(30) DEFAULT 'proposed'::character varying NOT NULL,
    jurisdiction_hint character varying(50),
    structure_template character varying(100),
    created_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    updated_at timestamp with time zone DEFAULT (now() AT TIME ZONE 'utc'::text),
    CONSTRAINT client_books_pkey PRIMARY KEY (book_id),
    CONSTRAINT client_books_status_check CHECK (
        (status)::text = ANY (ARRAY[
            'proposed'::character varying,
            'structure_chosen'::character varying,
            'entities_provisioned'::character varying,
            'cbus_scaffolded'::character varying,
            'parties_assigned'::character varying,
            'mandates_defined'::character varying,
            'ready_for_deal'::character varying,
            'abandoned'::character varying
        ]::text[])
    )
);

COMMENT ON TABLE "ob-poc".client_books IS
    'Client book — grouping of CBUs under a single commercial client (T3-B). '
    'Book-setup journey lifecycle: proposed → structure_chosen → '
    'entities_provisioned → cbus_scaffolded → parties_assigned → '
    'mandates_defined → ready_for_deal (+ abandoned terminal).';

CREATE INDEX IF NOT EXISTS idx_client_books_client_group
    ON "ob-poc".client_books(client_group_id);
CREATE INDEX IF NOT EXISTS idx_client_books_status
    ON "ob-poc".client_books(status);

-- cbus.book_id FK — link CBUs to their book
ALTER TABLE "ob-poc".cbus
    ADD COLUMN IF NOT EXISTS book_id uuid;

ALTER TABLE "ob-poc".cbus
    DROP CONSTRAINT IF EXISTS cbus_book_id_fk;

ALTER TABLE "ob-poc".cbus
    ADD CONSTRAINT cbus_book_id_fk
        FOREIGN KEY (book_id) REFERENCES "ob-poc".client_books(book_id);

CREATE INDEX IF NOT EXISTS idx_cbus_book
    ON "ob-poc".cbus(book_id) WHERE book_id IS NOT NULL;

COMMENT ON COLUMN "ob-poc".cbus.book_id IS
    'Parent client_book (T3-B). Groups CBUs under one commercial client. '
    'NULL for CBUs predating book-setup workspace introduction.';

COMMIT;

-- =============================================================================
-- Verification (run manually after migration):
--
--   -- CBU operational + disposition
--   SELECT column_name, data_type, column_default
--     FROM information_schema.columns
--     WHERE table_schema='ob-poc' AND table_name='cbus'
--       AND column_name IN ('operational_status','disposition_status','book_id');
--
--   -- New tables
--   SELECT table_name FROM information_schema.tables
--     WHERE table_schema='ob-poc'
--       AND table_name IN ('cbu_service_consumption','cbu_trading_activity',
--                           'cbu_corporate_action_events','client_books');
--
--   -- Check constraints expanded
--   SELECT conname, pg_get_constraintdef(oid)
--     FROM pg_constraint
--     WHERE conname IN ('deals_status_check','deal_rate_cards_status_check',
--                        'share_classes_lifecycle_status_check',
--                        'deal_slas_sla_status_check');
--
--   -- Deal internal accountability columns
--   SELECT column_name FROM information_schema.columns
--     WHERE table_schema='ob-poc' AND table_name='deals'
--       AND column_name IN ('sponsor_entity_id','rm_entity_id',
--                            'coverage_banker_entity_id','parent_deal_id');
-- =============================================================================
