-- ============================================================================
-- Migration: 002_kyc_screening_decision_monitoring_tables.sql
-- Purpose: Complete persistence layer for DSL verb domains
-- Generated: 2024-11-26
-- 
-- This migration adds 14 missing tables required by the verb schema:
--   - Document domain: document_requests, document_entity_links
--   - Entity domain: ownership_relationships
--   - KYC domain: investigations, risk_assessments, risk_ratings
--   - Screening domain: screening_results, screening_hit_resolutions, screening_batches
--   - Decision domain: decisions, decision_conditions
--   - Monitoring domain: monitoring_reviews, monitoring_cases, monitoring_alert_rules, monitoring_activities
-- ============================================================================

BEGIN;

-- ============================================================================
-- DOCUMENT DOMAIN - Missing Tables
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".document_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    document_type_id UUID REFERENCES "ob-poc".document_types(type_id),
    priority VARCHAR(20) DEFAULT 'NORMAL' 
        CHECK (priority IN ('LOW', 'NORMAL', 'HIGH', 'URGENT')),
    source VARCHAR(30) 
        CHECK (source IN ('REGISTRY', 'CLIENT', 'THIRD_PARTY')),
    due_date DATE,
    status VARCHAR(30) DEFAULT 'PENDING' 
        CHECK (status IN ('PENDING', 'RECEIVED', 'EXPIRED', 'CANCELLED')),
    fulfilled_by_doc_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    notes TEXT,
    requested_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    
    CONSTRAINT document_requests_target_check 
        CHECK (cbu_id IS NOT NULL OR entity_id IS NOT NULL)
);

COMMENT ON TABLE "ob-poc".document_requests IS 
    'Tracks document requests for CBUs or entities during onboarding/KYC';

CREATE INDEX idx_document_requests_cbu ON "ob-poc".document_requests(cbu_id);
CREATE INDEX idx_document_requests_entity ON "ob-poc".document_requests(entity_id);
CREATE INDEX idx_document_requests_status ON "ob-poc".document_requests(status);


CREATE TABLE IF NOT EXISTS "ob-poc".document_entity_links (
    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doc_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    link_type VARCHAR(50) DEFAULT 'EVIDENCE'
        CHECK (link_type IN ('EVIDENCE', 'IDENTITY', 'ADDRESS', 'FINANCIAL', 'REGULATORY', 'OTHER')),
    linked_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    
    UNIQUE(doc_id, entity_id, link_type)
);

COMMENT ON TABLE "ob-poc".document_entity_links IS 
    'Links documents to entities with typed relationship';

CREATE INDEX idx_document_entity_links_doc ON "ob-poc".document_entity_links(doc_id);
CREATE INDEX idx_document_entity_links_entity ON "ob-poc".document_entity_links(entity_id);


-- ============================================================================
-- ENTITY DOMAIN - Missing Tables  
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
    evidence_doc_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
    notes TEXT,
    created_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    
    CONSTRAINT ownership_not_self CHECK (owner_entity_id != owned_entity_id),
    CONSTRAINT ownership_temporal CHECK (effective_to IS NULL OR effective_to > effective_from)
);

COMMENT ON TABLE "ob-poc".ownership_relationships IS 
    'Tracks ownership relationships between entities for UBO analysis';

CREATE INDEX idx_ownership_owner ON "ob-poc".ownership_relationships(owner_entity_id);
CREATE INDEX idx_ownership_owned ON "ob-poc".ownership_relationships(owned_entity_id);
CREATE INDEX idx_ownership_type ON "ob-poc".ownership_relationships(ownership_type);


-- ============================================================================
-- KYC DOMAIN - Missing Tables
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".investigations (
    investigation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_type VARCHAR(30) NOT NULL
        CHECK (investigation_type IN ('STANDARD', 'ENHANCED_DUE_DILIGENCE', 'SIMPLIFIED', 'PERIODIC_REVIEW')),
    status VARCHAR(30) DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'IN_PROGRESS', 'COLLECTING_DOCUMENTS', 'UNDER_REVIEW', 
                          'ESCALATED', 'APPROVED', 'REJECTED', 'CLOSED')),
    risk_rating VARCHAR(20)
        CHECK (risk_rating IN ('LOW', 'MEDIUM', 'MEDIUM_HIGH', 'HIGH', 'VERY_HIGH')),
    ubo_threshold NUMERIC(5,2) DEFAULT 25.0
        CHECK (ubo_threshold >= 0 AND ubo_threshold <= 100),
    deadline DATE,
    outcome VARCHAR(30)
        CHECK (outcome IN ('APPROVED', 'REJECTED', 'CONDITIONALLY_APPROVED')),
    outcome_rationale TEXT,
    assigned_to VARCHAR(255),
    completed_at TIMESTAMPTZ,
    created_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

COMMENT ON TABLE "ob-poc".investigations IS 
    'KYC investigations for client business units';

CREATE INDEX idx_investigations_cbu ON "ob-poc".investigations(cbu_id);
CREATE INDEX idx_investigations_status ON "ob-poc".investigations(status);
CREATE INDEX idx_investigations_type ON "ob-poc".investigations(investigation_type);


CREATE TABLE IF NOT EXISTS "ob-poc".risk_assessments (
    assessment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_id UUID REFERENCES "ob-poc".investigations(investigation_id),
    methodology VARCHAR(30) NOT NULL
        CHECK (methodology IN ('FACTOR_WEIGHTED', 'HIGHEST_RISK', 'CUMULATIVE')),
    overall_score NUMERIC(5,2),
    factor_scores JSONB DEFAULT '{}'::jsonb,
    risk_factors JSONB DEFAULT '[]'::jsonb,
    mitigating_factors JSONB DEFAULT '[]'::jsonb,
    assessed_by VARCHAR(255) DEFAULT 'system',
    assessed_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

COMMENT ON TABLE "ob-poc".risk_assessments IS 
    'Risk assessment results for CBUs using configured methodology';

CREATE INDEX idx_risk_assessments_cbu ON "ob-poc".risk_assessments(cbu_id);
CREATE INDEX idx_risk_assessments_investigation ON "ob-poc".risk_assessments(investigation_id);


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

COMMENT ON TABLE "ob-poc".risk_ratings IS 
    'Historical record of risk ratings assigned to CBUs';

CREATE INDEX idx_risk_ratings_cbu ON "ob-poc".risk_ratings(cbu_id);
CREATE INDEX idx_risk_ratings_rating ON "ob-poc".risk_ratings(rating);
CREATE INDEX idx_risk_ratings_current ON "ob-poc".risk_ratings(cbu_id, effective_to) 
    WHERE effective_to IS NULL;


-- ============================================================================
-- SCREENING DOMAIN - Missing Tables
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".screening_results (
    result_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    screen_type VARCHAR(30) NOT NULL
        CHECK (screen_type IN ('PEP', 'SANCTIONS', 'ADVERSE_MEDIA')),
    provider VARCHAR(30)
        CHECK (provider IN ('REFINITIV', 'DOW_JONES', 'LEXISNEXIS', 'INTERNAL')),
    match_threshold NUMERIC(5,2) DEFAULT 85.0,
    hit_count INTEGER DEFAULT 0,
    highest_match_score NUMERIC(5,2),
    raw_response JSONB,
    categories JSONB DEFAULT '[]'::jsonb,  -- For adverse media categories
    lookback_months INTEGER,                -- For adverse media
    screened_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

COMMENT ON TABLE "ob-poc".screening_results IS 
    'Results from PEP, sanctions, and adverse media screening';

CREATE INDEX idx_screening_results_entity ON "ob-poc".screening_results(entity_id);
CREATE INDEX idx_screening_results_type ON "ob-poc".screening_results(screen_type);
CREATE INDEX idx_screening_results_hits ON "ob-poc".screening_results(entity_id, hit_count) 
    WHERE hit_count > 0;


CREATE TABLE IF NOT EXISTS "ob-poc".screening_hit_resolutions (
    resolution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    result_id UUID NOT NULL REFERENCES "ob-poc".screening_results(result_id),
    hit_reference VARCHAR(255),  -- External hit ID from provider
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

COMMENT ON TABLE "ob-poc".screening_hit_resolutions IS 
    'Resolution decisions for screening hits';

CREATE INDEX idx_screening_hit_resolutions_result ON "ob-poc".screening_hit_resolutions(result_id);
CREATE INDEX idx_screening_hit_resolutions_resolution ON "ob-poc".screening_hit_resolutions(resolution);


CREATE TABLE IF NOT EXISTS "ob-poc".screening_batches (
    batch_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
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

COMMENT ON TABLE "ob-poc".screening_batches IS 
    'Batch screening jobs for multiple entities';

CREATE INDEX idx_screening_batches_cbu ON "ob-poc".screening_batches(cbu_id);
CREATE INDEX idx_screening_batches_status ON "ob-poc".screening_batches(status);


-- Link table for batch -> individual results
CREATE TABLE IF NOT EXISTS "ob-poc".screening_batch_results (
    batch_id UUID NOT NULL REFERENCES "ob-poc".screening_batches(batch_id),
    result_id UUID NOT NULL REFERENCES "ob-poc".screening_results(result_id),
    PRIMARY KEY (batch_id, result_id)
);


-- ============================================================================
-- DECISION DOMAIN - Missing Tables
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID REFERENCES "ob-poc".investigations(investigation_id),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    decision_type VARCHAR(30) NOT NULL
        CHECK (decision_type IN ('APPROVE', 'REJECT', 'ESCALATE', 'DEFER', 'CONDITIONAL_APPROVE')),
    rationale TEXT NOT NULL,
    decision_date DATE DEFAULT CURRENT_DATE,
    
    -- Approval specific
    approval_level VARCHAR(20)
        CHECK (approval_level IN ('ANALYST', 'SENIOR_ANALYST', 'MANAGER', 'DIRECTOR', 'COMMITTEE')),
    next_review_date DATE,
    
    -- Rejection specific
    reason_code VARCHAR(40)
        CHECK (reason_code IN ('SANCTIONS_HIT', 'PEP_UNRESOLVED', 'SOURCE_OF_FUNDS', 'ADVERSE_MEDIA',
                               'DOCUMENTATION_INCOMPLETE', 'HIGH_RISK_JURISDICTION', 
                               'BENEFICIAL_OWNER_UNVERIFIED', 'REGULATORY_PROHIBITION', 'OTHER')),
    is_permanent BOOLEAN DEFAULT FALSE,
    reapply_after DATE,
    
    -- Escalation specific
    escalate_to VARCHAR(20)
        CHECK (escalate_to IN ('SENIOR_ANALYST', 'MANAGER', 'DIRECTOR', 'COMMITTEE', 'MLRO', 'LEGAL')),
    escalation_reason VARCHAR(40)
        CHECK (escalation_reason IN ('HIGH_RISK', 'PEP_INVOLVEMENT', 'SANCTIONS_HIT', 'COMPLEX_STRUCTURE',
                                      'UNUSUAL_ACTIVITY', 'POLICY_EXCEPTION', 'SENIOR_APPROVAL_REQUIRED', 'OTHER')),
    escalation_priority VARCHAR(20)
        CHECK (escalation_priority IN ('LOW', 'NORMAL', 'HIGH', 'URGENT')),
    escalation_due_date DATE,
    case_summary TEXT,
    
    -- Defer specific
    defer_until DATE,
    pending_items JSONB DEFAULT '[]'::jsonb,
    
    decided_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

COMMENT ON TABLE "ob-poc".decisions IS 
    'KYC/onboarding decisions including approvals, rejections, escalations, and deferrals';

CREATE INDEX idx_decisions_investigation ON "ob-poc".decisions(investigation_id);
CREATE INDEX idx_decisions_cbu ON "ob-poc".decisions(cbu_id);
CREATE INDEX idx_decisions_type ON "ob-poc".decisions(decision_type);
CREATE INDEX idx_decisions_escalation ON "ob-poc".decisions(escalate_to) 
    WHERE decision_type = 'ESCALATE';


CREATE TABLE IF NOT EXISTS "ob-poc".decision_conditions (
    condition_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id UUID NOT NULL REFERENCES "ob-poc".decisions(decision_id),
    condition_type VARCHAR(30) NOT NULL
        CHECK (condition_type IN ('DOCUMENT_REQUIRED', 'ENHANCED_MONITORING', 'TRANSACTION_LIMIT',
                                   'PERIODIC_REVIEW', 'SENIOR_SIGN_OFF', 'REGULATORY_APPROVAL', 'OTHER')),
    description TEXT NOT NULL,
    due_date DATE,
    is_blocking BOOLEAN DEFAULT TRUE,
    status VARCHAR(20) DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'SATISFIED', 'WAIVED', 'EXPIRED')),
    satisfied_at TIMESTAMPTZ,
    satisfied_by VARCHAR(255),
    evidence_ref TEXT,
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

COMMENT ON TABLE "ob-poc".decision_conditions IS 
    'Conditions attached to conditional approvals';

CREATE INDEX idx_decision_conditions_decision ON "ob-poc".decision_conditions(decision_id);
CREATE INDEX idx_decision_conditions_status ON "ob-poc".decision_conditions(status);
CREATE INDEX idx_decision_conditions_blocking ON "ob-poc".decision_conditions(decision_id, is_blocking) 
    WHERE status = 'PENDING';


-- ============================================================================
-- MONITORING DOMAIN - Missing Tables
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

COMMENT ON TABLE "ob-poc".monitoring_cases IS 
    'Ongoing monitoring cases for CBUs';

CREATE INDEX idx_monitoring_cases_cbu ON "ob-poc".monitoring_cases(cbu_id);
CREATE INDEX idx_monitoring_cases_status ON "ob-poc".monitoring_cases(status);


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

COMMENT ON TABLE "ob-poc".monitoring_reviews IS 
    'Periodic and triggered reviews for ongoing monitoring';

CREATE INDEX idx_monitoring_reviews_case ON "ob-poc".monitoring_reviews(case_id);
CREATE INDEX idx_monitoring_reviews_cbu ON "ob-poc".monitoring_reviews(cbu_id);
CREATE INDEX idx_monitoring_reviews_due ON "ob-poc".monitoring_reviews(due_date) 
    WHERE status IN ('SCHEDULED', 'OVERDUE');
CREATE INDEX idx_monitoring_reviews_status ON "ob-poc".monitoring_reviews(status);


CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_alert_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    case_id UUID REFERENCES "ob-poc".monitoring_cases(case_id),
    rule_type VARCHAR(30) NOT NULL
        CHECK (rule_type IN ('TRANSACTION_VOLUME', 'TRANSACTION_VALUE', 'JURISDICTION_ACTIVITY',
                              'COUNTERPARTY_TYPE', 'PATTERN_DEVIATION', 'CUSTOM')),
    rule_name VARCHAR(255) NOT NULL,
    description TEXT,
    threshold JSONB NOT NULL,  -- Flexible threshold definition
    is_active BOOLEAN DEFAULT TRUE,
    last_triggered_at TIMESTAMPTZ,
    trigger_count INTEGER DEFAULT 0,
    created_by VARCHAR(255) DEFAULT 'system',
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

COMMENT ON TABLE "ob-poc".monitoring_alert_rules IS 
    'Custom alert rules for ongoing monitoring';

CREATE INDEX idx_monitoring_alert_rules_cbu ON "ob-poc".monitoring_alert_rules(cbu_id);
CREATE INDEX idx_monitoring_alert_rules_active ON "ob-poc".monitoring_alert_rules(cbu_id, is_active) 
    WHERE is_active = TRUE;


CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_activities (
    activity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES "ob-poc".monitoring_cases(case_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    activity_type VARCHAR(30) NOT NULL
        CHECK (activity_type IN ('CLIENT_CONTACT', 'DOCUMENT_UPDATE', 'SCREENING_RUN',
                                  'TRANSACTION_REVIEW', 'RISK_ASSESSMENT', 'INTERNAL_NOTE', 'OTHER')),
    description TEXT NOT NULL,
    reference_id VARCHAR(255),  -- Link to related record
    reference_type VARCHAR(50), -- Type of related record
    recorded_by VARCHAR(255) NOT NULL,
    recorded_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc'),
    created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'utc')
);

COMMENT ON TABLE "ob-poc".monitoring_activities IS 
    'Activity log for monitoring cases';

CREATE INDEX idx_monitoring_activities_case ON "ob-poc".monitoring_activities(case_id);
CREATE INDEX idx_monitoring_activities_cbu ON "ob-poc".monitoring_activities(cbu_id);
CREATE INDEX idx_monitoring_activities_type ON "ob-poc".monitoring_activities(activity_type);
CREATE INDEX idx_monitoring_activities_recorded ON "ob-poc".monitoring_activities(recorded_at DESC);


-- ============================================================================
-- RISK UPDATE TRACKING (for monitoring.update-risk verb)
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

COMMENT ON TABLE "ob-poc".risk_rating_changes IS 
    'Audit trail of risk rating changes during monitoring';

CREATE INDEX idx_risk_rating_changes_cbu ON "ob-poc".risk_rating_changes(cbu_id);
CREATE INDEX idx_risk_rating_changes_case ON "ob-poc".risk_rating_changes(case_id);


-- ============================================================================
-- UPDATE CRUD_OPERATIONS CONSTRAINTS FOR NEW ASSET TYPES
-- ============================================================================

-- Drop and recreate the constraint with expanded asset types
ALTER TABLE "ob-poc".crud_operations 
    DROP CONSTRAINT IF EXISTS crud_operations_asset_type_check;

ALTER TABLE "ob-poc".crud_operations 
    ADD CONSTRAINT crud_operations_asset_type_check 
    CHECK ((asset_type)::text = ANY (ARRAY[
        -- Original types
        'CBU', 'ENTITY', 'PARTNERSHIP', 'LIMITED_COMPANY', 'PROPER_PERSON', 
        'TRUST', 'ATTRIBUTE', 'DOCUMENT',
        -- New types from verb schema
        'CBU_ENTITY_ROLE', 'OWNERSHIP', 'DOCUMENT_REQUEST', 'DOCUMENT_LINK',
        'INVESTIGATION', 'RISK_ASSESSMENT_CBU', 'RISK_RATING',
        'SCREENING_RESULT', 'SCREENING_HIT_RESOLUTION', 'SCREENING_BATCH',
        'DECISION', 'DECISION_CONDITION',
        'MONITORING_CASE', 'MONITORING_REVIEW', 'MONITORING_ALERT_RULE', 
        'MONITORING_ACTIVITY', 'ATTRIBUTE_VALUE', 'ATTRIBUTE_VALIDATION'
    ]::text[]));


-- ============================================================================
-- UPDATE DSL_EXAMPLES CONSTRAINTS FOR NEW ASSET TYPES
-- ============================================================================

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
    i.assigned_to,
    i.created_at,
    EXTRACT(DAY FROM NOW() - i.created_at) as days_open
FROM "ob-poc".investigations i
JOIN "ob-poc".cbus c ON i.cbu_id = c.cbu_id
WHERE i.status NOT IN ('APPROVED', 'REJECTED', 'CLOSED');

COMMENT ON VIEW "ob-poc".active_investigations IS 
    'Currently active KYC investigations with CBU details';


CREATE OR REPLACE VIEW "ob-poc".pending_screening_hits AS
SELECT 
    sr.result_id,
    sr.entity_id,
    e.name as entity_name,
    sr.screen_type,
    sr.hit_count,
    sr.highest_match_score,
    sr.screened_at,
    COUNT(shr.resolution_id) as resolved_count,
    sr.hit_count - COUNT(shr.resolution_id) as unresolved_count
FROM "ob-poc".screening_results sr
JOIN "ob-poc".entities e ON sr.entity_id = e.entity_id
LEFT JOIN "ob-poc".screening_hit_resolutions shr ON sr.result_id = shr.result_id
WHERE sr.hit_count > 0
GROUP BY sr.result_id, sr.entity_id, e.name, sr.screen_type, 
         sr.hit_count, sr.highest_match_score, sr.screened_at
HAVING sr.hit_count > COUNT(shr.resolution_id);

COMMENT ON VIEW "ob-poc".pending_screening_hits IS 
    'Screening results with unresolved hits requiring attention';


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

COMMENT ON VIEW "ob-poc".overdue_reviews IS 
    'Monitoring reviews past their due date';


CREATE OR REPLACE VIEW "ob-poc".blocking_conditions AS
SELECT 
    dc.condition_id,
    dc.decision_id,
    d.cbu_id,
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
JOIN "ob-poc".decisions d ON dc.decision_id = d.decision_id
JOIN "ob-poc".cbus c ON d.cbu_id = c.cbu_id
WHERE dc.is_blocking = TRUE
  AND dc.status = 'PENDING';

COMMENT ON VIEW "ob-poc".blocking_conditions IS 
    'Blocking conditions that must be satisfied for conditional approvals';


COMMIT;

-- ============================================================================
-- POST-MIGRATION VERIFICATION
-- ============================================================================
-- Run these queries to verify the migration:
--
-- SELECT tablename FROM pg_tables WHERE schemaname = 'ob-poc' ORDER BY tablename;
--
-- SELECT 
--     t.table_name,
--     (SELECT count(*) FROM information_schema.columns c 
--      WHERE c.table_schema = t.table_schema AND c.table_name = t.table_name) as column_count
-- FROM information_schema.tables t
-- WHERE t.table_schema = 'ob-poc'
-- AND t.table_name IN (
--     'document_requests', 'document_entity_links', 'ownership_relationships',
--     'investigations', 'risk_assessments', 'risk_ratings',
--     'screening_results', 'screening_hit_resolutions', 'screening_batches',
--     'decisions', 'decision_conditions',
--     'monitoring_cases', 'monitoring_reviews', 'monitoring_alert_rules', 'monitoring_activities'
-- )
-- ORDER BY t.table_name;
-- ============================================================================
