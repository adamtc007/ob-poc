-- Migration: 017_kyc_investigation_tables.sql
-- Description: Add unique constraints for idempotent UPSERT operations and new KYC investigation tables
-- Created: 2025-11-25
-- Part of: KYC/UBO Vocabulary + Idempotent Execution Refactor

-- =============================================================================
-- PART 1: Unique Constraints for Idempotent Operations
-- =============================================================================

-- CBU natural key: name + jurisdiction
-- Allows UPSERT semantics for cbu.ensure
ALTER TABLE "ob-poc".cbus
ADD CONSTRAINT IF NOT EXISTS cbus_natural_key UNIQUE (name, jurisdiction);

-- Ownership edge natural key: source + target + relationship_type
-- Allows UPSERT for entity.ensure-ownership
ALTER TABLE "ob-poc".entity_role_connections
ADD CONSTRAINT IF NOT EXISTS entity_role_connections_natural_key
UNIQUE (source_entity_id, target_entity_id, relationship_type);

-- Limited companies natural key: company_number (when present)
-- Allows UPSERT for entity.ensure-limited-company
CREATE UNIQUE INDEX IF NOT EXISTS limited_companies_company_number_idx
ON "ob-poc".entity_limited_companies (company_number)
WHERE company_number IS NOT NULL;

-- =============================================================================
-- PART 2: KYC Investigation Tables
-- =============================================================================

-- Main investigation table - wraps entire KYC workflow for one CBU
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_investigations (
    investigation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    investigation_type VARCHAR(50) NOT NULL,  -- STANDARD, ENHANCED_DUE_DILIGENCE, SIMPLIFIED
    risk_rating VARCHAR(20),  -- LOW, MEDIUM, MEDIUM_HIGH, HIGH, PROHIBITED
    regulatory_framework JSONB,  -- ["EU_5MLD", "US_BSA_AML", ...]
    ubo_threshold NUMERIC(5,2) DEFAULT 10.0,  -- Ownership % threshold to track
    investigation_depth INTEGER DEFAULT 5,  -- Max levels to traverse
    status VARCHAR(50) DEFAULT 'INITIATED',  -- INITIATED, COLLECTING_DOCUMENTS, ANALYZING, PENDING_REVIEW, COMPLETE
    deadline DATE,
    outcome VARCHAR(50),  -- NULL until complete: APPROVED, REJECTED, CONDITIONAL, ESCALATED
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_investigations_cbu ON "ob-poc".kyc_investigations(cbu_id);
CREATE INDEX IF NOT EXISTS idx_investigations_status ON "ob-poc".kyc_investigations(status);

-- Investigation assignments (analysts working on cases)
CREATE TABLE IF NOT EXISTS "ob-poc".investigation_assignments (
    assignment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID NOT NULL REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    assignee VARCHAR(255) NOT NULL,
    role VARCHAR(50),  -- PRIMARY_ANALYST, REVIEWER, APPROVER
    assigned_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(investigation_id, assignee, role)
);

-- =============================================================================
-- PART 3: Document Collection Tables
-- =============================================================================

-- Document requests (track what documents are needed)
CREATE TABLE IF NOT EXISTS "ob-poc".document_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    document_type_code VARCHAR(100) NOT NULL,
    source VARCHAR(50),  -- REGISTRY, CLIENT, THIRD_PARTY
    priority VARCHAR(20) DEFAULT 'NORMAL',  -- LOW, NORMAL, HIGH, URGENT
    status VARCHAR(50) DEFAULT 'REQUESTED',  -- REQUESTED, RECEIVED, VERIFIED, REJECTED
    due_date DATE,
    received_document_id UUID REFERENCES "ob-poc".document_catalog(document_id),
    received_from VARCHAR(255),
    received_date TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CHECK ((entity_id IS NOT NULL) OR (cbu_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_doc_requests_investigation ON "ob-poc".document_requests(investigation_id);
CREATE INDEX IF NOT EXISTS idx_doc_requests_entity ON "ob-poc".document_requests(entity_id);
CREATE INDEX IF NOT EXISTS idx_doc_requests_status ON "ob-poc".document_requests(status);

-- Document verifications (audit trail for verification)
CREATE TABLE IF NOT EXISTS "ob-poc".document_verifications (
    verification_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(document_id) ON DELETE CASCADE,
    verification_method VARCHAR(100),  -- REGISTRY_CHECK, MANUAL_REVIEW, OCR, BIOMETRIC
    verification_status VARCHAR(50),  -- VERIFIED, FAILED, PENDING
    verified_by VARCHAR(255),
    verification_date TIMESTAMPTZ DEFAULT NOW(),
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_doc_verifications_document ON "ob-poc".document_verifications(document_id);

-- =============================================================================
-- PART 4: Screening Tables
-- =============================================================================

-- Screening records (PEP, Sanctions, Adverse Media)
CREATE TABLE IF NOT EXISTS "ob-poc".screenings (
    screening_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    screening_type VARCHAR(50) NOT NULL,  -- PEP, SANCTIONS, ADVERSE_MEDIA
    databases JSONB,  -- ["WORLD_CHECK", "REFINITIV", "DOW_JONES", ...]
    lists JSONB,  -- ["OFAC_SDN", "EU_SANCTIONS", "UK_HMT", "UN_SANCTIONS"]
    include_rca BOOLEAN DEFAULT FALSE,  -- Include Relatives and Close Associates (for PEP)
    search_depth VARCHAR(20),  -- QUICK, STANDARD, DEEP (for adverse media)
    languages JSONB,  -- ["EN", "DE", "FR"]
    status VARCHAR(50) DEFAULT 'PENDING',  -- PENDING, COMPLETED, REVIEW_REQUIRED
    result VARCHAR(50),  -- NO_MATCH, POTENTIAL_MATCH, MATCH
    match_details JSONB,  -- Full match information
    resolution VARCHAR(50),  -- FALSE_POSITIVE, TRUE_HIT, ESCALATE
    resolution_rationale TEXT,
    screened_at TIMESTAMPTZ DEFAULT NOW(),
    reviewed_by VARCHAR(255),
    resolved_by VARCHAR(255),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_screenings_entity ON "ob-poc".screenings(entity_id);
CREATE INDEX IF NOT EXISTS idx_screenings_investigation ON "ob-poc".screenings(investigation_id);
CREATE INDEX IF NOT EXISTS idx_screenings_type ON "ob-poc".screenings(screening_type);
CREATE INDEX IF NOT EXISTS idx_screenings_result ON "ob-poc".screenings(result);

-- =============================================================================
-- PART 5: Risk Assessment Tables
-- =============================================================================

-- Risk assessments (for CBUs and entities)
CREATE TABLE IF NOT EXISTS "ob-poc".risk_assessments (
    assessment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    assessment_type VARCHAR(50) NOT NULL,  -- CBU, ENTITY
    rating VARCHAR(20),  -- LOW, MEDIUM, MEDIUM_HIGH, HIGH, PROHIBITED
    factors JSONB,  -- [{"factor": "PEP_EXPOSURE", "rating": "HIGH", "weight": 0.3}, ...]
    methodology VARCHAR(50),  -- FACTOR_WEIGHTED, HIGHEST_RISK, CUMULATIVE
    rationale TEXT,
    assessed_by VARCHAR(255),
    assessed_at TIMESTAMPTZ DEFAULT NOW(),
    CHECK ((cbu_id IS NOT NULL) OR (entity_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_risk_assessments_cbu ON "ob-poc".risk_assessments(cbu_id);
CREATE INDEX IF NOT EXISTS idx_risk_assessments_entity ON "ob-poc".risk_assessments(entity_id);
CREATE INDEX IF NOT EXISTS idx_risk_assessments_rating ON "ob-poc".risk_assessments(rating);

-- Risk flags (red flags, amber flags, notes)
CREATE TABLE IF NOT EXISTS "ob-poc".risk_flags (
    flag_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    flag_type VARCHAR(50) NOT NULL,  -- RED_FLAG, AMBER_FLAG, NOTE
    description TEXT,
    status VARCHAR(50) DEFAULT 'ACTIVE',  -- ACTIVE, RESOLVED, SUPERSEDED
    flagged_by VARCHAR(255),
    flagged_at TIMESTAMPTZ DEFAULT NOW(),
    resolved_by VARCHAR(255),
    resolved_at TIMESTAMPTZ,
    resolution_notes TEXT,
    CHECK ((cbu_id IS NOT NULL) OR (entity_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_risk_flags_cbu ON "ob-poc".risk_flags(cbu_id);
CREATE INDEX IF NOT EXISTS idx_risk_flags_entity ON "ob-poc".risk_flags(entity_id);
CREATE INDEX IF NOT EXISTS idx_risk_flags_status ON "ob-poc".risk_flags(status);

-- =============================================================================
-- PART 6: Decision Tables
-- =============================================================================

-- KYC decisions (onboarding decisions)
CREATE TABLE IF NOT EXISTS "ob-poc".kyc_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    decision VARCHAR(50) NOT NULL,  -- ACCEPT, CONDITIONAL_ACCEPTANCE, REJECT, ESCALATE
    decision_authority VARCHAR(100),  -- ANALYST, SENIOR_MANAGEMENT, BOARD
    rationale TEXT,
    decided_by VARCHAR(255),
    decided_at TIMESTAMPTZ DEFAULT NOW(),
    effective_date DATE DEFAULT CURRENT_DATE,
    review_date DATE  -- Next scheduled review
);

CREATE INDEX IF NOT EXISTS idx_decisions_cbu ON "ob-poc".kyc_decisions(cbu_id);
CREATE INDEX IF NOT EXISTS idx_decisions_investigation ON "ob-poc".kyc_decisions(investigation_id);
CREATE INDEX IF NOT EXISTS idx_decisions_decision ON "ob-poc".kyc_decisions(decision);

-- Decision conditions (for conditional acceptance)
CREATE TABLE IF NOT EXISTS "ob-poc".decision_conditions (
    condition_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id UUID NOT NULL REFERENCES "ob-poc".kyc_decisions(decision_id) ON DELETE CASCADE,
    condition_type VARCHAR(50) NOT NULL,  -- ENHANCED_MONITORING, TRANSACTION_LIMIT, DOCUMENT_REQUIRED, TRANSACTION_APPROVAL
    description TEXT,
    frequency VARCHAR(50),  -- ONE_TIME, QUARTERLY, ANNUAL
    due_date DATE,
    threshold NUMERIC(20,2),  -- For transaction limits
    currency VARCHAR(3),  -- For transaction limits (EUR, USD, GBP)
    assigned_to VARCHAR(255),
    status VARCHAR(50) DEFAULT 'PENDING',  -- PENDING, SATISFIED, OVERDUE, WAIVED
    satisfied_by VARCHAR(255),
    satisfied_at TIMESTAMPTZ,
    satisfaction_evidence TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_conditions_decision ON "ob-poc".decision_conditions(decision_id);
CREATE INDEX IF NOT EXISTS idx_conditions_status ON "ob-poc".decision_conditions(status);

-- =============================================================================
-- PART 7: Monitoring Tables
-- =============================================================================

-- Monitoring setup (ongoing monitoring configuration)
CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_setup (
    setup_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    monitoring_level VARCHAR(50) NOT NULL,  -- STANDARD, ENHANCED
    components JSONB,  -- [{"type": "TRANSACTION_MONITORING", "frequency": "REAL_TIME"}, ...]
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(cbu_id)  -- One setup per CBU
);

CREATE INDEX IF NOT EXISTS idx_monitoring_setup_cbu ON "ob-poc".monitoring_setup(cbu_id);

-- Monitoring events (alerts, transactions, etc.)
CREATE TABLE IF NOT EXISTS "ob-poc".monitoring_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    event_type VARCHAR(50) NOT NULL,  -- TRANSACTION_ALERT, SCREENING_ALERT, KYC_REFRESH_DUE
    description TEXT,
    severity VARCHAR(20),  -- LOW, MEDIUM, HIGH, CRITICAL
    requires_review BOOLEAN DEFAULT FALSE,
    reviewed_by VARCHAR(255),
    reviewed_at TIMESTAMPTZ,
    review_outcome VARCHAR(50),  -- CLEARED, ESCALATED, SAR_FILED
    review_notes TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_monitoring_events_cbu ON "ob-poc".monitoring_events(cbu_id);
CREATE INDEX IF NOT EXISTS idx_monitoring_events_type ON "ob-poc".monitoring_events(event_type);
CREATE INDEX IF NOT EXISTS idx_monitoring_events_review ON "ob-poc".monitoring_events(requires_review) WHERE requires_review = TRUE;

-- Scheduled reviews (KYC refresh, periodic reviews)
CREATE TABLE IF NOT EXISTS "ob-poc".scheduled_reviews (
    review_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    review_type VARCHAR(50) NOT NULL,  -- ANNUAL_KYC_REFRESH, QUARTERLY_REVIEW, TRIGGERED_REVIEW
    due_date DATE NOT NULL,
    assigned_to VARCHAR(255),
    status VARCHAR(50) DEFAULT 'SCHEDULED',  -- SCHEDULED, IN_PROGRESS, COMPLETED, OVERDUE
    completed_by VARCHAR(255),
    completed_at TIMESTAMPTZ,
    completion_notes TEXT,
    next_review_id UUID,  -- Links to successor review after completion
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_scheduled_reviews_cbu ON "ob-poc".scheduled_reviews(cbu_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_reviews_due ON "ob-poc".scheduled_reviews(due_date);
CREATE INDEX IF NOT EXISTS idx_scheduled_reviews_status ON "ob-poc".scheduled_reviews(status);

-- =============================================================================
-- PART 8: UBO Registry Table (for calculated UBOs)
-- =============================================================================

-- UBO registry (calculated beneficial owners)
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_registry (
    ubo_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ownership_percent NUMERIC(10,4),  -- Calculated total ownership
    is_direct BOOLEAN DEFAULT FALSE,  -- Direct ownership vs. indirect
    calculation_method VARCHAR(50),  -- RECURSIVE_MULTIPLY, CUMULATIVE
    ownership_chain JSONB,  -- Path through ownership graph
    flag_reason VARCHAR(50),  -- MATERIAL_INFLUENCE, CONTROL_WITHOUT_OWNERSHIP, PEP (for manual flags)
    flagged_by VARCHAR(255),
    verified BOOLEAN DEFAULT FALSE,
    verified_by VARCHAR(255),
    verified_at TIMESTAMPTZ,
    verification_method VARCHAR(100),
    verification_notes TEXT,
    status VARCHAR(50) DEFAULT 'ACTIVE',  -- ACTIVE, CLEARED
    cleared_at TIMESTAMPTZ,
    cleared_reason VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(cbu_id, entity_id)  -- One UBO entry per entity per CBU
);

CREATE INDEX IF NOT EXISTS idx_ubo_registry_cbu ON "ob-poc".ubo_registry(cbu_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_entity ON "ob-poc".ubo_registry(entity_id);
CREATE INDEX IF NOT EXISTS idx_ubo_registry_status ON "ob-poc".ubo_registry(status);
