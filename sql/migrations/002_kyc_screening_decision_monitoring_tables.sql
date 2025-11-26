-- ============================================================================
-- Migration: 002_kyc_screening_decision_monitoring_tables.sql
-- Purpose: Complete persistence layer for DSL verb domains
-- Generated: 2024-11-26
-- 
-- NOTE: This migration is ADDITIVE to 017_kyc_investigation_tables.sql
-- It adds missing tables and creates views to bridge naming conventions
-- between the existing schema and the DSL verb crud_asset values.
--
-- Existing tables from 017 that we'll use:
--   - kyc_investigations → bridges to INVESTIGATION crud_asset
--   - screenings → bridges to SCREENING_RESULT crud_asset  
--   - risk_assessments → already matches RISK_ASSESSMENT_CBU
--   - kyc_decisions → bridges to DECISION crud_asset
--   - decision_conditions → already matches DECISION_CONDITION
--   - document_requests → already matches DOCUMENT_REQUEST
--
-- New tables this migration adds:
--   - ownership_relationships (OWNERSHIP)
--   - document_entity_links (DOCUMENT_LINK)
--   - risk_ratings (RISK_RATING)
--   - screening_hit_resolutions (SCREENING_HIT_RESOLUTION)
--   - screening_batches (SCREENING_BATCH)
--   - monitoring_cases (MONITORING_CASE)
--   - monitoring_reviews (MONITORING_REVIEW)
--   - monitoring_alert_rules (MONITORING_ALERT_RULE)
--   - monitoring_activities (MONITORING_ACTIVITY)
-- ============================================================================

BEGIN;

-- ============================================================================
-- ENTITY DOMAIN - ownership_relationships (OWNERSHIP crud_asset)
-- ============================================================================
-- NOTE: This table stores the OWNERSHIP CHAIN (edges in the ownership graph).
-- It is DIFFERENT from ubo_registry which stores the IDENTIFIED UBOs (the result).
-- 
-- Example: A owns 60% of B, B owns 40% of C, Person X owns 80% of A
-- ownership_relationships: 3 rows (A→B, B→C, X→A)
-- ubo_registry: 1 row (X is UBO of C with calculated 19.2%)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".ownership_relationships (
    ownership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    owned_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ownership_type VARCHAR(30) NOT NULL
        CHECK (ownership_type IN ('DIRECT', 'INDIRECT', 'BENEFICIAL')),
    ownership_percent NUMERIC(5,2) NOT NULL
        CHECK (ownership_percent >= 0 AND ownership_percent <= 100),
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,
    evidence_doc_id UUID REFERENCES "ob-poc".document_catalog(document_id),
    notes TEXT,
    created_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    
    CONSTRAINT ownership_not_self CHECK (owner_entity_id != owned_entity_id),
    CONSTRAINT ownership_temporal CHECK (effective_to IS NULL OR effective_to > effective_from)
);

CREATE INDEX IF NOT EXISTS idx_ownership_owner ON "ob-poc".ownership_relationships(owner_entity_id);
CREATE INDEX IF NOT EXISTS idx_ownership_owned ON "ob-poc".ownership_relationships(owned_entity_id);
CREATE INDEX IF NOT EXISTS idx_ownership_type ON "ob-poc".ownership_relationships(ownership_type);

COMMENT ON TABLE "ob-poc".ownership_relationships IS 
    'Tracks ownership relationships between entities for UBO analysis (OWNERSHIP crud_asset)';


-- ============================================================================
-- DOCUMENT DOMAIN - document_entity_links (DOCUMENT_LINK crud_asset)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_entity_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(document_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    link_type VARCHAR(50) DEFAULT 'EVIDENCE'
        CHECK (link_type IN ('EVIDENCE', 'IDENTITY', 'ADDRESS', 'FINANCIAL', 'REGULATORY', 'OTHER')),
    linked_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    
    UNIQUE(document_id, entity_id, link_type)
);

CREATE INDEX IF NOT EXISTS idx_document_entity_links_doc ON "ob-poc".document_entity_links(document_id);
CREATE INDEX IF NOT EXISTS idx_document_entity_links_entity ON "ob-poc".document_entity_links(entity_id);

COMMENT ON TABLE "ob-poc".document_entity_links IS 
    'Links documents to entities with typed relationship (DOCUMENT_LINK crud_asset)';


-- ============================================================================
-- KYC DOMAIN - risk_ratings (RISK_RATING crud_asset)
-- ============================================================================
-- NOTE: This tracks CBU-LEVEL risk ratings with full history.
-- ubo_registry.risk_rating tracks UBO-LEVEL risk (individual beneficial owners).
-- Both are needed: CBU risk aggregates all factors, UBO risk is per-person.
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".risk_ratings (
    rating_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    rating VARCHAR(20) NOT NULL
        CHECK (rating IN ('LOW', 'MEDIUM', 'MEDIUM_HIGH', 'HIGH', 'VERY_HIGH', 'PROHIBITED')),
    previous_rating VARCHAR(20)
        CHECK (previous_rating IN ('LOW', 'MEDIUM', 'MEDIUM_HIGH', 'HIGH', 'VERY_HIGH', 'PROHIBITED')),
    rationale TEXT,
    assessment_id UUID REFERENCES "ob-poc".risk_assessments(assessment_id),
    effective_from TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    effective_to TIMESTAMPTZ,
    set_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_risk_ratings_cbu ON "ob-poc".risk_ratings(cbu_id);
CREATE INDEX IF NOT EXISTS idx_risk_ratings_rating ON "ob-poc".risk_ratings(rating);
CREATE INDEX IF NOT EXISTS idx_risk_ratings_current ON "ob-poc".risk_ratings(cbu_id, effective_to) 
    WHERE effective_to IS NULL;

COMMENT ON TABLE "ob-poc".risk_ratings IS 
    'Historical record of risk ratings assigned to CBUs (RISK_RATING crud_asset)';


-- ============================================================================
-- SCREENING DOMAIN - screening_hit_resolutions (SCREENING_HIT_RESOLUTION crud_asset)
-- ============================================================================
-- NOTE: This provides DETAILED resolution workflow for screening hits.
-- ubo_registry.screening_result only stores final status ('PENDING'/'CLEAR'/'HIT').
-- This table stores the full resolution history with rationale and evidence.
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".screening_hit_resolutions (
    resolution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    screening_id UUID NOT NULL REFERENCES "ob-poc".screenings(screening_id),
    hit_reference VARCHAR(255),
    ubo_id UUID REFERENCES "ob-poc".ubo_registry(ubo_id),  -- Link to UBO if screening was for UBO identification
    resolution VARCHAR(30) NOT NULL
        CHECK (resolution IN ('TRUE_MATCH', 'FALSE_POSITIVE', 'INCONCLUSIVE', 'ESCALATE')),
    dismiss_reason VARCHAR(30)
        CHECK (dismiss_reason IN ('NAME_ONLY_MATCH', 'DIFFERENT_DOB', 'DIFFERENT_NATIONALITY', 
                                   'DIFFERENT_JURISDICTION', 'DECEASED', 'DELISTED', 'OTHER')),
    rationale TEXT NOT NULL,
    evidence_refs JSONB DEFAULT '[]'::jsonb,
    notes TEXT,
    resolved_by VARCHAR(255) NOT NULL,
    resolved_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    reviewed_by VARCHAR(255),
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_screening_hit_resolutions_screening ON "ob-poc".screening_hit_resolutions(screening_id);
CREATE INDEX IF NOT EXISTS idx_screening_hit_resolutions_resolution ON "ob-poc".screening_hit_resolutions(resolution);

COMMENT ON TABLE "ob-poc".screening_hit_resolutions IS 
    'Resolution decisions for screening hits (SCREENING_HIT_RESOLUTION crud_asset)';


-- ============================================================================
-- SCREENING DOMAIN - screening_batches (SCREENING_BATCH crud_asset)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".screening_batches (
    batch_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id),
    screen_types JSONB NOT NULL DEFAULT '["PEP", "SANCTIONS"]'::jsonb,
    entity_count INTEGER DEFAULT 0,
    completed_count INTEGER DEFAULT 0,
    hit_count INTEGER DEFAULT 0,
    status VARCHAR(30) DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'IN_PROGRESS', 'COMPLETED', 'FAILED', 'CANCELLED')),
    match_threshold NUMERIC(5,2) DEFAULT 85.0,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    created_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_screening_batches_cbu ON "ob-poc".screening_batches(cbu_id);
CREATE INDEX IF NOT EXISTS idx_screening_batches_status ON "ob-poc".screening_batches(status);

COMMENT ON TABLE "ob-poc".screening_batches IS 
    'Batch screening jobs for multiple entities (SCREENING_BATCH crud_asset)';

-- Link table for batch -> individual screening results
CREATE TABLE IF NOT EXISTS "ob-poc".screening_batch_results (
    batch_id UUID NOT NULL REFERENCES "ob-poc".screening_batches(batch_id),
    screening_id UUID NOT NULL REFERENCES "ob-poc".screenings(screening_id),
    PRIMARY KEY (batch_id, screening_id)
);


-- ============================================================================
-- MONITORING DOMAIN - monitoring_cases (MONITORING_CASE crud_asset)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_cases (
    case_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_type VARCHAR(30) NOT NULL
        CHECK (case_type IN ('ONGOING_MONITORING', 'TRIGGERED_REVIEW', 'PERIODIC_REVIEW')),
    status VARCHAR(30) DEFAULT 'OPEN'
        CHECK (status IN ('OPEN', 'UNDER_REVIEW', 'ESCALATED', 'CLOSED')),
    close_reason VARCHAR(40)
        CHECK (close_reason IN ('ACCOUNT_CLOSED', 'CLIENT_EXITED', 'RELATIONSHIP_TERMINATED',
                                 'MERGED_WITH_OTHER', 'REGULATORY_ORDER', 'OTHER')),
    close_notes TEXT,
    retention_period_years INTEGER DEFAULT 7
        CHECK (retention_period_years >= 5 AND retention_period_years <= 25),
    closed_at TIMESTAMPTZ,
    closed_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_monitoring_cases_cbu ON "ob-poc".monitoring_cases(cbu_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_cases_status ON "ob-poc".monitoring_cases(status);

COMMENT ON TABLE "ob-poc".monitoring_cases IS 
    'Ongoing monitoring cases for CBUs (MONITORING_CASE crud_asset)';


-- ============================================================================
-- MONITORING DOMAIN - monitoring_reviews (MONITORING_REVIEW crud_asset)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_reviews (
    review_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES "ob-poc".monitoring_cases(case_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    review_type VARCHAR(30) NOT NULL
        CHECK (review_type IN ('PERIODIC', 'ANNUAL', 'ENHANCED_PERIODIC', 'SIMPLIFIED_PERIODIC')),
    trigger_type VARCHAR(30)
        CHECK (trigger_type IN ('ADVERSE_MEDIA', 'SANCTIONS_ALERT', 'TRANSACTION_ALERT',
                                 'OWNERSHIP_CHANGE', 'REGULATORY_CHANGE', 'CLIENT_REQUEST',
                                 'INTERNAL_REFERRAL', 'SCREENING_HIT', 'OTHER')),
    trigger_reference_id VARCHAR(255),
    due_date DATE NOT NULL,
    risk_based_frequency VARCHAR(20)
        CHECK (risk_based_frequency IN ('ANNUAL', 'BIANNUAL', 'QUARTERLY', 'MONTHLY')),
    scope JSONB DEFAULT '["FULL"]'::jsonb,
    status VARCHAR(30) DEFAULT 'SCHEDULED'
        CHECK (status IN ('SCHEDULED', 'IN_PROGRESS', 'COMPLETED', 'OVERDUE', 'CANCELLED')),
    outcome VARCHAR(30)
        CHECK (outcome IN ('NO_CHANGE', 'RISK_INCREASED', 'RISK_DECREASED', 
                           'ESCALATED', 'EXIT_RECOMMENDED', 'ENHANCED_MONITORING')),
    findings TEXT,
    next_review_date DATE,
    actions JSONB DEFAULT '[]'::jsonb,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    completed_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_monitoring_reviews_case ON "ob-poc".monitoring_reviews(case_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_reviews_cbu ON "ob-poc".monitoring_reviews(cbu_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_reviews_due ON "ob-poc".monitoring_reviews(due_date) 
    WHERE status IN ('SCHEDULED', 'OVERDUE');

COMMENT ON TABLE "ob-poc".monitoring_reviews IS 
    'Periodic and triggered reviews for ongoing monitoring (MONITORING_REVIEW crud_asset)';


-- ============================================================================
-- MONITORING DOMAIN - monitoring_alert_rules (MONITORING_ALERT_RULE crud_asset)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_alert_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_id UUID REFERENCES "ob-poc".monitoring_cases(case_id),
    rule_type VARCHAR(30) NOT NULL
        CHECK (rule_type IN ('TRANSACTION_VOLUME', 'TRANSACTION_VALUE', 'JURISDICTION_ACTIVITY',
                              'COUNTERPARTY_TYPE', 'PATTERN_DEVIATION', 'CUSTOM')),
    rule_name VARCHAR(255) NOT NULL,
    description TEXT,
    threshold JSONB NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    last_triggered_at TIMESTAMPTZ,
    trigger_count INTEGER DEFAULT 0,
    created_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_monitoring_alert_rules_cbu ON "ob-poc".monitoring_alert_rules(cbu_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_alert_rules_active ON "ob-poc".monitoring_alert_rules(cbu_id, is_active) 
    WHERE is_active = TRUE;

COMMENT ON TABLE "ob-poc".monitoring_alert_rules IS 
    'Custom alert rules for ongoing monitoring (MONITORING_ALERT_RULE crud_asset)';


-- ============================================================================
-- MONITORING DOMAIN - monitoring_activities (MONITORING_ACTIVITY crud_asset)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_activities (
    activity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES "ob-poc".monitoring_cases(case_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    activity_type VARCHAR(30) NOT NULL
        CHECK (activity_type IN ('CLIENT_CONTACT', 'DOCUMENT_UPDATE', 'SCREENING_RUN',
                                  'TRANSACTION_REVIEW', 'RISK_ASSESSMENT', 'INTERNAL_NOTE', 'OTHER')),
    description TEXT NOT NULL,
    reference_id VARCHAR(255),
    reference_type VARCHAR(50),
    recorded_by VARCHAR(255) NOT NULL,
    recorded_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_monitoring_activities_case ON "ob-poc".monitoring_activities(case_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_activities_cbu ON "ob-poc".monitoring_activities(cbu_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_activities_type ON "ob-poc".monitoring_activities(activity_type);

COMMENT ON TABLE "ob-poc".monitoring_activities IS 
    'Activity log for monitoring cases (MONITORING_ACTIVITY crud_asset)';


-- ============================================================================
-- RISK RATING CHANGE TRACKING (for monitoring.update-risk verb)
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".risk_rating_changes (
    change_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_id UUID REFERENCES "ob-poc".monitoring_cases(case_id),
    review_id UUID REFERENCES "ob-poc".monitoring_reviews(review_id),
    previous_rating VARCHAR(20)
        CHECK (previous_rating IN ('LOW', 'MEDIUM', 'MEDIUM_HIGH', 'HIGH', 'VERY_HIGH', 'PROHIBITED')),
    new_rating VARCHAR(20) NOT NULL
        CHECK (new_rating IN ('LOW', 'MEDIUM', 'MEDIUM_HIGH', 'HIGH', 'VERY_HIGH', 'PROHIBITED')),
    change_reason VARCHAR(30) NOT NULL
        CHECK (change_reason IN ('PERIODIC_REVIEW', 'TRIGGER_EVENT', 'OWNERSHIP_CHANGE',
                                  'JURISDICTION_CHANGE', 'PRODUCT_CHANGE', 'SCREENING_RESULT',
                                  'TRANSACTION_PATTERN', 'REGULATORY_CHANGE', 'OTHER')),
    rationale TEXT NOT NULL,
    effective_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    changed_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

CREATE INDEX IF NOT EXISTS idx_risk_rating_changes_cbu ON "ob-poc".risk_rating_changes(cbu_id);

COMMENT ON TABLE "ob-poc".risk_rating_changes IS 
    'Audit trail of risk rating changes during monitoring';


-- ============================================================================
-- BRIDGE VIEWS - Map existing 017 tables to DSL crud_asset naming
-- ============================================================================

-- INVESTIGATION crud_asset -> kyc_investigations
CREATE OR REPLACE VIEW "ob-poc".investigations AS
SELECT 
    investigation_id,
    cbu_id,
    investigation_type,
    status,
    risk_rating,
    ubo_threshold,
    deadline,
    outcome,
    outcome AS outcome_rationale,  -- Bridge column
    notes AS assigned_to,          -- Bridge column
    created_at,
    updated_at,
    completed_at
FROM "ob-poc".kyc_investigations;

COMMENT ON VIEW "ob-poc".investigations IS 
    'Bridge view: maps kyc_investigations to INVESTIGATION crud_asset';


-- SCREENING_RESULT crud_asset -> screenings
CREATE OR REPLACE VIEW "ob-poc".screening_results AS
SELECT 
    screening_id AS result_id,
    entity_id,
    screening_type AS screen_type,
    CASE 
        WHEN databases IS NOT NULL THEN databases->>0 
        ELSE 'INTERNAL' 
    END AS provider,
    85.0 AS match_threshold,
    CASE WHEN result = 'MATCH' THEN 1 
         WHEN result = 'POTENTIAL_MATCH' THEN 1 
         ELSE 0 END AS hit_count,
    NULL::NUMERIC AS highest_match_score,
    match_details AS raw_response,
    '[]'::jsonb AS categories,
    NULL::INTEGER AS lookback_months,
    screened_at,
    NULL::TIMESTAMPTZ AS expires_at,
    screened_at AS created_at
FROM "ob-poc".screenings;

COMMENT ON VIEW "ob-poc".screening_results IS 
    'Bridge view: maps screenings to SCREENING_RESULT crud_asset';


-- DECISION crud_asset -> kyc_decisions
CREATE OR REPLACE VIEW "ob-poc".decisions AS
SELECT 
    decision_id,
    investigation_id,
    cbu_id,
    CASE 
        WHEN decision = 'ACCEPT' THEN 'APPROVE'
        WHEN decision = 'CONDITIONAL_ACCEPTANCE' THEN 'CONDITIONAL_APPROVE'
        WHEN decision = 'REJECT' THEN 'REJECT'
        WHEN decision = 'ESCALATE' THEN 'ESCALATE'
        ELSE decision
    END AS decision_type,
    rationale,
    decided_at::DATE AS decision_date,
    decision_authority AS approval_level,
    review_date AS next_review_date,
    NULL::VARCHAR AS reason_code,
    FALSE AS is_permanent,
    NULL::DATE AS reapply_after,
    NULL::VARCHAR AS escalate_to,
    NULL::VARCHAR AS escalation_reason,
    NULL::VARCHAR AS escalation_priority,
    NULL::DATE AS escalation_due_date,
    rationale AS case_summary,
    NULL::DATE AS defer_until,
    '[]'::jsonb AS pending_items,
    decided_by,
    decided_at AS created_at,
    decided_at AS updated_at
FROM "ob-poc".kyc_decisions;

COMMENT ON VIEW "ob-poc".decisions IS 
    'Bridge view: maps kyc_decisions to DECISION crud_asset';


-- ============================================================================
-- UPDATE CRUD_OPERATIONS CONSTRAINTS FOR NEW ASSET TYPES (if table exists)
-- ============================================================================

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_tables WHERE schemaname = 'ob-poc' AND tablename = 'crud_operations') THEN
        ALTER TABLE "ob-poc".crud_operations 
            DROP CONSTRAINT IF EXISTS crud_operations_asset_type_check;

        ALTER TABLE "ob-poc".crud_operations 
            ADD CONSTRAINT crud_operations_asset_type_check 
            CHECK ((asset_type)::text = ANY (ARRAY[
                'CBU', 'ENTITY', 'PARTNERSHIP', 'LIMITED_COMPANY', 'PROPER_PERSON', 
                'TRUST', 'ATTRIBUTE', 'DOCUMENT',
                'CBU_ENTITY_ROLE', 'OWNERSHIP', 'DOCUMENT_REQUEST', 'DOCUMENT_LINK',
                'INVESTIGATION', 'RISK_ASSESSMENT_CBU', 'RISK_RATING',
                'SCREENING_RESULT', 'SCREENING_HIT_RESOLUTION', 'SCREENING_BATCH',
                'DECISION', 'DECISION_CONDITION',
                'MONITORING_CASE', 'MONITORING_REVIEW', 'MONITORING_ALERT_RULE', 
                'MONITORING_ACTIVITY', 'ATTRIBUTE_VALUE', 'ATTRIBUTE_VALIDATION'
            ]::text[]));
        RAISE NOTICE 'Updated crud_operations constraint';
    ELSE
        RAISE NOTICE 'crud_operations table does not exist, skipping constraint update';
    END IF;
END $$;


-- ============================================================================
-- UPDATE DSL_EXAMPLES CONSTRAINTS FOR NEW ASSET TYPES (if table exists)
-- ============================================================================

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_tables WHERE schemaname = 'ob-poc' AND tablename = 'dsl_examples') THEN
        ALTER TABLE "ob-poc".dsl_examples 
            DROP CONSTRAINT IF EXISTS dsl_examples_asset_type_check;

        ALTER TABLE "ob-poc".dsl_examples 
            ADD CONSTRAINT dsl_examples_asset_type_check 
            CHECK ((asset_type)::text = ANY (ARRAY[
                'CBU', 'ENTITY', 'PARTNERSHIP', 'LIMITED_COMPANY', 'PROPER_PERSON', 
                'TRUST', 'ATTRIBUTE', 'DOCUMENT',
                'CBU_ENTITY_ROLE', 'OWNERSHIP', 'DOCUMENT_REQUEST', 'DOCUMENT_LINK',
                'INVESTIGATION', 'RISK_ASSESSMENT_CBU', 'RISK_RATING',
                'SCREENING_RESULT', 'SCREENING_HIT_RESOLUTION', 'SCREENING_BATCH',
                'DECISION', 'DECISION_CONDITION',
                'MONITORING_CASE', 'MONITORING_REVIEW', 'MONITORING_ALERT_RULE', 
                'MONITORING_ACTIVITY', 'ATTRIBUTE_VALUE', 'ATTRIBUTE_VALIDATION'
            ]::text[]));
        RAISE NOTICE 'Updated dsl_examples constraint';
    ELSE
        RAISE NOTICE 'dsl_examples table does not exist, skipping constraint update';
    END IF;
END $$;


-- ============================================================================
-- HELPER VIEWS
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".active_investigations AS
SELECT 
    i.investigation_id,
    i.cbu_id,
    c.name as cbu_name,
    i.investigation_type,
    i.status,
    i.risk_rating,
    i.deadline,
    i.created_at,
    EXTRACT(DAY FROM NOW() - i.created_at) as days_open
FROM "ob-poc".kyc_investigations i
JOIN "ob-poc".cbus c ON i.cbu_id = c.cbu_id
WHERE i.status NOT IN ('COMPLETE');

CREATE OR REPLACE VIEW "ob-poc".overdue_reviews AS
SELECT 
    mr.review_id,
    mr.cbu_id,
    c.name as cbu_name,
    mr.review_type,
    mr.due_date,
    mr.status,
    CURRENT_DATE - mr.due_date as days_overdue
FROM "ob-poc".monitoring_reviews mr
JOIN "ob-poc".cbus c ON mr.cbu_id = c.cbu_id
WHERE mr.due_date < CURRENT_DATE
  AND mr.status IN ('SCHEDULED', 'IN_PROGRESS');

CREATE OR REPLACE VIEW "ob-poc".blocking_conditions AS
SELECT 
    dc.condition_id,
    dc.decision_id,
    kd.cbu_id,
    c.name as cbu_name,
    dc.condition_type,
    dc.description,
    dc.due_date,
    dc.status,
    CASE 
        WHEN dc.due_date < CURRENT_DATE THEN 'OVERDUE'
        WHEN dc.due_date = CURRENT_DATE THEN 'DUE_TODAY'
        ELSE 'PENDING'
    END as urgency
FROM "ob-poc".decision_conditions dc
JOIN "ob-poc".kyc_decisions kd ON dc.decision_id = kd.decision_id
JOIN "ob-poc".cbus c ON kd.cbu_id = c.cbu_id
WHERE dc.status = 'PENDING';


COMMIT;

-- ============================================================================
-- POST-MIGRATION VERIFICATION
-- ============================================================================
-- 
-- SELECT tablename FROM pg_tables WHERE schemaname = 'ob-poc' 
-- AND tablename IN ('ownership_relationships', 'document_entity_links', 
--                   'risk_ratings', 'screening_hit_resolutions', 
--                   'screening_batches', 'monitoring_cases', 
--                   'monitoring_reviews', 'monitoring_alert_rules', 
--                   'monitoring_activities')
-- ORDER BY tablename;
--
-- Expected: 9 new tables
-- ============================================================================
