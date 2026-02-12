-- Migration 077: KYC/UBO Computed Architecture
-- Source spec: docs/KYC_UBO_ARCHITECTURE_v0.5.md
-- Creates: graph_import_runs, case_import_runs, ubo_determination_runs,
--          ubo_registry, ubo_evidence, outreach_plans, outreach_items,
--          tollgate_definitions (ob_ref), tollgate_evaluations (kyc),
--          standards_mappings (ob_ref)
-- Alters:  entity_relationships (provenance + natural key),
--          kyc.cases (case_ref, client_group_id, deal_id, etc.),
--          kyc.entity_workstreams (prong booleans)

BEGIN;

-- ============================================================================
-- 0.1  "ob-poc".graph_import_runs
-- ============================================================================

CREATE TABLE "ob-poc".graph_import_runs (
    run_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_kind               VARCHAR(30) NOT NULL DEFAULT 'SKELETON_BUILD',
    scope_root_entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    as_of                  DATE,
    source                 VARCHAR(30) NOT NULL,
    source_query           TEXT,
    source_ref             TEXT,
    payload_hash           VARCHAR(64),
    normalized_hash        VARCHAR(64),
    entities_created       INTEGER DEFAULT 0,
    entities_updated       INTEGER DEFAULT 0,
    edges_created          INTEGER DEFAULT 0,
    status                 VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    superseded_by          UUID REFERENCES "ob-poc".graph_import_runs(run_id),
    superseded_reason      TEXT,
    imported_at            TIMESTAMPTZ DEFAULT NOW(),
    imported_by            VARCHAR(80) NOT NULL DEFAULT 'SYSTEM',
    CONSTRAINT chk_gir_run_kind CHECK (
        run_kind IN ('SKELETON_BUILD','MANUAL_RESEARCH','REFRESH','CORRECTION_REPLAY')
    ),
    CONSTRAINT chk_gir_status CHECK (
        status IN ('ACTIVE','SUPERSEDED','ROLLED_BACK','PARTIAL')
    ),
    CONSTRAINT chk_gir_source CHECK (
        source IN ('GLEIF','COMPANIES_HOUSE','SEC_EDGAR','CLIENT_PROVIDED',
                   'INTERNAL_KYC','BODS','MANUAL','AGENT_DISCOVERED')
    )
);

CREATE INDEX idx_gir_scope_root ON "ob-poc".graph_import_runs(scope_root_entity_id);
CREATE INDEX idx_gir_source_ref ON "ob-poc".graph_import_runs(source, source_ref);
CREATE INDEX idx_gir_status     ON "ob-poc".graph_import_runs(status);

-- ============================================================================
-- 0.2  kyc.case_import_runs (join table)
-- ============================================================================

CREATE TABLE kyc.case_import_runs (
    case_id     UUID NOT NULL REFERENCES kyc.cases(case_id),
    run_id      UUID NOT NULL REFERENCES "ob-poc".graph_import_runs(run_id),
    decision_id UUID REFERENCES kyc.research_decisions(decision_id),
    linked_at   TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (case_id, run_id)
);

CREATE INDEX idx_cir_case ON kyc.case_import_runs(case_id);

-- ============================================================================
-- 0.3  ALTER entity_relationships — provenance + natural key
-- ============================================================================

-- Add provenance columns
ALTER TABLE "ob-poc".entity_relationships
    ADD COLUMN IF NOT EXISTS import_run_id UUID REFERENCES "ob-poc".graph_import_runs(run_id),
    ADD COLUMN IF NOT EXISTS confidence VARCHAR(10),
    ADD COLUMN IF NOT EXISTS evidence_hint TEXT;

-- Backfill source for NULL rows
UPDATE "ob-poc".entity_relationships SET source = 'MANUAL' WHERE source IS NULL;

-- Make source NOT NULL (it's currently nullable VARCHAR(100))
ALTER TABLE "ob-poc".entity_relationships
    ALTER COLUMN source SET NOT NULL;

-- Backfill confidence for existing rows (source-dependent defaults per spec §2A.6)
UPDATE "ob-poc".entity_relationships SET confidence = 'MEDIUM' WHERE confidence IS NULL;

-- Make confidence NOT NULL
ALTER TABLE "ob-poc".entity_relationships
    ALTER COLUMN confidence SET NOT NULL;

-- Drop old unique constraint (without effective_from)
ALTER TABLE "ob-poc".entity_relationships
    DROP CONSTRAINT IF EXISTS uq_entity_relationship;

-- Drop old partial unique indexes
DROP INDEX IF EXISTS "ob-poc".idx_entity_rel_unique_active;
DROP INDEX IF EXISTS "ob-poc".idx_entity_rel_unique_historical;

-- Add new natural key constraint including effective_from
-- Uses a partial unique index because effective_from can be NULL
CREATE UNIQUE INDEX uq_entity_rel_natural_key
    ON "ob-poc".entity_relationships (from_entity_id, to_entity_id, relationship_type, effective_from)
    WHERE effective_from IS NOT NULL;

-- For rows where effective_from IS NULL, keep uniqueness on (from, to, type) with only active
CREATE UNIQUE INDEX uq_entity_rel_natural_key_null_from
    ON "ob-poc".entity_relationships (from_entity_id, to_entity_id, relationship_type)
    WHERE effective_from IS NULL AND effective_to IS NULL;

-- Index for import run lookups
CREATE INDEX idx_er_import_run ON "ob-poc".entity_relationships(import_run_id)
    WHERE import_run_id IS NOT NULL;

-- ============================================================================
-- 0.4  ALTER kyc.cases — add missing columns
-- ============================================================================

ALTER TABLE kyc.cases
    ADD COLUMN IF NOT EXISTS case_ref VARCHAR(30),
    ADD COLUMN IF NOT EXISTS client_group_id UUID,
    ADD COLUMN IF NOT EXISTS deal_id UUID,
    ADD COLUMN IF NOT EXISTS priority VARCHAR(10) DEFAULT 'NORMAL',
    ADD COLUMN IF NOT EXISTS due_date DATE,
    ADD COLUMN IF NOT EXISTS escalation_date DATE;

-- Generate case_ref for existing rows (CTE required — window functions not allowed in UPDATE)
WITH numbered AS (
    SELECT case_id, opened_at,
           ROW_NUMBER() OVER (ORDER BY opened_at) AS rn
    FROM kyc.cases
    WHERE case_ref IS NULL
)
UPDATE kyc.cases c
SET case_ref = 'KYC-' || EXTRACT(YEAR FROM n.opened_at)::TEXT || '-' || LPAD(n.rn::TEXT, 4, '0')
FROM numbered n
WHERE c.case_id = n.case_id;

-- Now make it NOT NULL + UNIQUE
ALTER TABLE kyc.cases
    ALTER COLUMN case_ref SET NOT NULL;

-- Add unique constraint (may fail if backfill produced duplicates — handled by LPAD)
ALTER TABLE kyc.cases
    ADD CONSTRAINT uq_case_ref UNIQUE (case_ref);

-- Add FK for deal_id (deals table exists per migration 067)
ALTER TABLE kyc.cases
    ADD CONSTRAINT fk_cases_deal FOREIGN KEY (deal_id) REFERENCES "ob-poc".deals(deal_id);

-- Add CHECK on status matching spec state machine
-- Drop any existing constraint first
ALTER TABLE kyc.cases DROP CONSTRAINT IF EXISTS chk_case_status;
ALTER TABLE kyc.cases ADD CONSTRAINT chk_case_status CHECK (
    status IN ('INTAKE','DISCOVERY','ASSESSMENT','REVIEW',
               'APPROVED','REJECTED','BLOCKED','WITHDRAWN','DO_NOT_ONBOARD')
);

-- Sequence for case_ref generation
CREATE SEQUENCE IF NOT EXISTS kyc.case_ref_seq START WITH 200;

-- ============================================================================
-- 0.5  ALTER kyc.entity_workstreams — add prong booleans
-- ============================================================================

ALTER TABLE kyc.entity_workstreams
    ADD COLUMN IF NOT EXISTS inclusion_reason VARCHAR(30),
    ADD COLUMN IF NOT EXISTS identity_verified BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS ownership_proved BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS screening_cleared BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS evidence_complete BOOLEAN DEFAULT FALSE;

-- Ensure UNIQUE on (case_id, entity_id)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_workstream_case_entity'
          AND conrelid = 'kyc.entity_workstreams'::regclass
    ) THEN
        ALTER TABLE kyc.entity_workstreams
            ADD CONSTRAINT uq_workstream_case_entity UNIQUE (case_id, entity_id);
    END IF;
END $$;

-- ============================================================================
-- 0.6  kyc.ubo_determination_runs
-- ============================================================================

CREATE TABLE kyc.ubo_determination_runs (
    run_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    case_id             UUID REFERENCES kyc.cases(case_id),
    as_of               DATE NOT NULL,
    config_version      VARCHAR(20) NOT NULL,
    threshold_pct       DECIMAL(5,2) NOT NULL,
    code_hash           VARCHAR(64),
    candidates_found    INTEGER NOT NULL DEFAULT 0,
    output_snapshot     JSONB NOT NULL,
    chains_snapshot     JSONB,
    coverage_snapshot   JSONB,
    computed_at         TIMESTAMPTZ DEFAULT NOW(),
    computed_by         VARCHAR(50) DEFAULT 'SYSTEM',
    computation_ms      INTEGER
);

CREATE INDEX idx_udr_case ON kyc.ubo_determination_runs(case_id);
CREATE INDEX idx_udr_subject ON kyc.ubo_determination_runs(subject_entity_id);

-- ============================================================================
-- 0.7  kyc.ubo_registry
-- ============================================================================

CREATE TABLE kyc.ubo_registry (
    ubo_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id             UUID NOT NULL REFERENCES kyc.cases(case_id),
    workstream_id       UUID NOT NULL REFERENCES kyc.entity_workstreams(workstream_id),
    subject_entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ubo_person_id       UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ubo_type            VARCHAR(20) NOT NULL,
    status              VARCHAR(20) NOT NULL DEFAULT 'CANDIDATE',
    determination_run_id UUID REFERENCES kyc.ubo_determination_runs(run_id),
    computed_percentage  DECIMAL(7,4),
    chain_description    TEXT,
    waiver_reason       TEXT,
    waiver_authority     VARCHAR(50),
    waiver_expiry        DATE,
    risk_flags          JSONB DEFAULT '[]',
    identified_at       TIMESTAMPTZ,
    proved_at           TIMESTAMPTZ,
    reviewed_at         TIMESTAMPTZ,
    approved_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_ubo_type CHECK (
        ubo_type IN ('OWNERSHIP','CONTROL','TRUST_ROLE','SMO_FALLBACK','NOMINEE_BENEFICIARY')
    ),
    CONSTRAINT chk_ubo_status CHECK (
        status IN ('CANDIDATE','IDENTIFIED','PROVABLE','PROVED','REVIEWED','APPROVED',
                   'WAIVED','REJECTED','EXPIRED')
    )
);

CREATE INDEX idx_ubr_case ON kyc.ubo_registry(case_id);
CREATE INDEX idx_ubr_person ON kyc.ubo_registry(ubo_person_id);
CREATE INDEX idx_ubr_status ON kyc.ubo_registry(status);

-- ============================================================================
-- 0.8  kyc.ubo_evidence
-- ============================================================================

CREATE TABLE kyc.ubo_evidence (
    evidence_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ubo_id              UUID NOT NULL REFERENCES kyc.ubo_registry(ubo_id),
    evidence_type       VARCHAR(30) NOT NULL,
    document_id         UUID,
    screening_id        UUID,
    relationship_id     UUID REFERENCES "ob-poc".entity_relationships(relationship_id),
    determination_run_id UUID,
    status              VARCHAR(20) DEFAULT 'REQUIRED',
    verified_at         TIMESTAMPTZ,
    verified_by         UUID,
    notes               TEXT,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_evidence_type CHECK (
        evidence_type IN ('IDENTITY_DOC','OWNERSHIP_REGISTER','BOARD_RESOLUTION','TRUST_DEED',
                          'PARTNERSHIP_AGREEMENT','SCREENING_CLEAR','SPECIAL_RIGHTS_DOC',
                          'ANNUAL_RETURN','SHARE_CERTIFICATE','CHAIN_PROOF')
    ),
    CONSTRAINT chk_evidence_status CHECK (
        status IN ('REQUIRED','REQUESTED','RECEIVED','VERIFIED','REJECTED','WAIVED','EXPIRED')
    )
);

CREATE INDEX idx_ube_ubo ON kyc.ubo_evidence(ubo_id);
CREATE INDEX idx_ube_status ON kyc.ubo_evidence(status);

-- ============================================================================
-- 0.9  kyc.outreach_plans + kyc.outreach_items
-- ============================================================================

CREATE TABLE kyc.outreach_plans (
    plan_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id             UUID NOT NULL REFERENCES kyc.cases(case_id),
    workstream_id       UUID REFERENCES kyc.entity_workstreams(workstream_id),
    determination_run_id UUID REFERENCES kyc.ubo_determination_runs(run_id),
    generated_at        TIMESTAMPTZ DEFAULT NOW(),
    status              VARCHAR(20) DEFAULT 'DRAFT',
    total_items         INTEGER NOT NULL DEFAULT 0,
    items_responded     INTEGER DEFAULT 0,
    CONSTRAINT chk_op_status CHECK (
        status IN ('DRAFT','APPROVED','SENT','PARTIALLY_RESPONDED','CLOSED')
    )
);

CREATE INDEX idx_op_case ON kyc.outreach_plans(case_id);

CREATE TABLE kyc.outreach_items (
    item_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id             UUID NOT NULL REFERENCES kyc.outreach_plans(plan_id),
    prong               VARCHAR(30) NOT NULL,
    target_entity_id    UUID REFERENCES "ob-poc".entities(entity_id),
    gap_description     TEXT NOT NULL,
    request_text        TEXT NOT NULL,
    doc_type_requested  VARCHAR(50),
    priority            INTEGER DEFAULT 5,
    closes_gap_ref      TEXT,
    status              VARCHAR(20) DEFAULT 'PENDING',
    responded_at        TIMESTAMPTZ,
    document_id         UUID,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_oi_status CHECK (
        status IN ('PENDING','SENT','RESPONDED','VERIFIED','WAIVED')
    )
);

CREATE INDEX idx_oi_plan ON kyc.outreach_items(plan_id);

-- ============================================================================
-- 0.10  ob_ref.tollgate_definitions + kyc.tollgate_evaluations
-- ============================================================================

CREATE TABLE ob_ref.tollgate_definitions (
    tollgate_id         VARCHAR(30) PRIMARY KEY,
    display_name        VARCHAR(100) NOT NULL,
    description         TEXT,
    applies_to          VARCHAR(20) NOT NULL,
    required_status     VARCHAR(20),
    default_thresholds  JSONB NOT NULL,
    override_permitted  BOOLEAN DEFAULT TRUE,
    override_authority  VARCHAR(30),
    override_max_days   INTEGER,
    CONSTRAINT chk_td_applies CHECK (applies_to IN ('CASE','WORKSTREAM'))
);

-- Seed tollgate definitions
INSERT INTO ob_ref.tollgate_definitions (tollgate_id, display_name, applies_to, required_status, default_thresholds) VALUES
('SKELETON_READY', 'Skeleton Build Complete', 'CASE', 'DISCOVERY', '{
    "ownership_coverage_pct": 70,
    "minimum_sources_consulted": 1,
    "cycle_anomalies_acknowledged": true,
    "high_severity_conflicts_resolved": true
}'::jsonb),
('EVIDENCE_COMPLETE', 'Evidence Collection Complete', 'CASE', 'ASSESSMENT', '{
    "ownership_coverage_pct": 95,
    "identity_docs_verified_pct": 100,
    "screening_cleared_pct": 100,
    "outreach_plan_items_max": 0
}'::jsonb),
('REVIEW_COMPLETE', 'Review Complete', 'CASE', 'REVIEW', '{
    "all_ubos_approved": true,
    "all_workstreams_closed": true,
    "no_open_discrepancies": true
}'::jsonb);

CREATE TABLE kyc.tollgate_evaluations (
    evaluation_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id             UUID NOT NULL REFERENCES kyc.cases(case_id),
    workstream_id       UUID REFERENCES kyc.entity_workstreams(workstream_id),
    tollgate_id         VARCHAR(30) NOT NULL REFERENCES ob_ref.tollgate_definitions(tollgate_id),
    passed              BOOLEAN NOT NULL,
    evaluation_detail   JSONB NOT NULL,
    gaps                JSONB,
    overridden          BOOLEAN DEFAULT FALSE,
    override_by         UUID,
    override_reason     TEXT,
    override_expiry     DATE,
    evaluated_at        TIMESTAMPTZ DEFAULT NOW(),
    config_version      VARCHAR(20) NOT NULL
);

CREATE INDEX idx_te_case ON kyc.tollgate_evaluations(case_id);
CREATE INDEX idx_te_gate ON kyc.tollgate_evaluations(tollgate_id);

-- ============================================================================
-- ob_ref.standards_mappings (reference data)
-- ============================================================================

CREATE TABLE ob_ref.standards_mappings (
    mapping_id          SERIAL PRIMARY KEY,
    standard            VARCHAR(20) NOT NULL,
    our_value           VARCHAR(50) NOT NULL,
    standard_value      VARCHAR(100) NOT NULL,
    standard_version    VARCHAR(20) NOT NULL,
    notes               TEXT,
    effective_from      DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to        DATE
);

CREATE INDEX idx_sm_standard ON ob_ref.standards_mappings(standard, our_value);

COMMIT;
