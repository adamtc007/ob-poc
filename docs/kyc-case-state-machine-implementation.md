# KYC Case State Machine - Full Implementation

**Task**: Implement KYC case management with entity workstreams, red flags, DSL-driven rules engine, and visualization  
**Created**: 2025-12-02  
**Updated**: 2025-12-02 (Enhanced rules engine)  
**Status**: READY FOR IMPLEMENTATION  
**Priority**: CRITICAL

---

## Overview

Implement a complete KYC case management system that handles:
- Case lifecycle with expanding entity discovery
- Per-entity workstreams with individual status tracking
- Red flag detection and escalation
- Document request tracking
- Screening integration
- **DSL-driven rules engine with event-based invocation**
- Case visualization

---

## Part 1: Schema Migration

Create migration file and execute:

```sql
-- =============================================================================
-- KYC CASE STATE MACHINE SCHEMA
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. CASES - The main KYC case for a CBU
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.cases (
    case_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Status
    status VARCHAR(30) NOT NULL DEFAULT 'INTAKE',
    -- INTAKE, DISCOVERY, ASSESSMENT, REVIEW, APPROVED, REJECTED, BLOCKED, WITHDRAWN, EXPIRED
    
    -- Escalation
    escalation_level VARCHAR(30) NOT NULL DEFAULT 'STANDARD',
    -- STANDARD, SENIOR_COMPLIANCE, EXECUTIVE, BOARD
    
    -- Risk
    risk_rating VARCHAR(20),
    -- LOW, MEDIUM, HIGH, VERY_HIGH, PROHIBITED
    
    -- Assignment
    assigned_analyst_id UUID,
    assigned_reviewer_id UUID,
    
    -- Timing
    opened_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at TIMESTAMPTZ,
    sla_deadline TIMESTAMPTZ,
    last_activity_at TIMESTAMPTZ DEFAULT now(),
    
    -- Metadata
    case_type VARCHAR(30) DEFAULT 'NEW_CLIENT',
    -- NEW_CLIENT, PERIODIC_REVIEW, EVENT_DRIVEN, REMEDIATION
    
    notes TEXT,
    
    -- Constraints
    CONSTRAINT chk_case_status CHECK (status IN (
        'INTAKE', 'DISCOVERY', 'ASSESSMENT', 'REVIEW', 
        'APPROVED', 'REJECTED', 'BLOCKED', 'WITHDRAWN', 'EXPIRED'
    )),
    CONSTRAINT chk_escalation_level CHECK (escalation_level IN (
        'STANDARD', 'SENIOR_COMPLIANCE', 'EXECUTIVE', 'BOARD'
    )),
    CONSTRAINT chk_risk_rating CHECK (risk_rating IS NULL OR risk_rating IN (
        'LOW', 'MEDIUM', 'HIGH', 'VERY_HIGH', 'PROHIBITED'
    )),
    CONSTRAINT chk_case_type CHECK (case_type IN (
        'NEW_CLIENT', 'PERIODIC_REVIEW', 'EVENT_DRIVEN', 'REMEDIATION'
    ))
);

CREATE INDEX idx_cases_cbu ON kyc.cases(cbu_id);
CREATE INDEX idx_cases_status ON kyc.cases(status);
CREATE INDEX idx_cases_analyst ON kyc.cases(assigned_analyst_id) WHERE assigned_analyst_id IS NOT NULL;

COMMENT ON TABLE kyc.cases IS 'KYC cases for client onboarding and periodic review';

-- -----------------------------------------------------------------------------
-- 2. ENTITY WORKSTREAMS - Per-entity work within a case
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.entity_workstreams (
    workstream_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Status
    status VARCHAR(30) NOT NULL DEFAULT 'PENDING',
    -- PENDING, COLLECT, VERIFY, SCREEN, ASSESS, COMPLETE, BLOCKED, ENHANCED_DD
    
    -- Discovery chain (how was this entity found?)
    discovery_source_workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id),
    discovery_reason VARCHAR(100),
    -- e.g., "Ownership >25%", "Director", "Trustee", "Beneficial Owner"
    discovery_depth INTEGER DEFAULT 1,
    -- How many levels deep in the ownership chain
    
    -- Risk
    risk_rating VARCHAR(20),
    risk_factors JSONB DEFAULT '[]'::jsonb,
    
    -- Timing
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    blocked_at TIMESTAMPTZ,
    blocked_reason TEXT,
    
    -- Flags
    requires_enhanced_dd BOOLEAN DEFAULT false,
    is_ubo BOOLEAN DEFAULT false,
    ownership_percentage DECIMAL(5,2),
    
    CONSTRAINT chk_workstream_status CHECK (status IN (
        'PENDING', 'COLLECT', 'VERIFY', 'SCREEN', 'ASSESS', 
        'COMPLETE', 'BLOCKED', 'ENHANCED_DD'
    )),
    CONSTRAINT uq_case_entity UNIQUE (case_id, entity_id)
);

CREATE INDEX idx_workstreams_case ON kyc.entity_workstreams(case_id);
CREATE INDEX idx_workstreams_entity ON kyc.entity_workstreams(entity_id);
CREATE INDEX idx_workstreams_status ON kyc.entity_workstreams(status);
CREATE INDEX idx_workstreams_discovery ON kyc.entity_workstreams(discovery_source_workstream_id) 
    WHERE discovery_source_workstream_id IS NOT NULL;

COMMENT ON TABLE kyc.entity_workstreams IS 'Per-entity work items within a KYC case';

-- -----------------------------------------------------------------------------
-- 3. RED FLAGS - Issues requiring attention
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.red_flags (
    red_flag_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
    workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE,
    
    -- Classification
    flag_type VARCHAR(50) NOT NULL,
    -- SANCTIONS_MATCH, PEP_IDENTIFIED, ADVERSE_MEDIA, NOMINEE_UNDISCLOSED,
    -- OPAQUE_STRUCTURE, DOCUMENT_FRAUD, INCONSISTENT_INFO, HIGH_RISK_JURISDICTION,
    -- COMPLEX_STRUCTURE, SOURCE_OF_WEALTH_UNCLEAR, MISSING_UBO, SHELL_COMPANY,
    -- TAX_HAVEN, CASH_INTENSIVE, REGULATORY_ACTION, DOCUMENT_OVERDUE
    
    severity VARCHAR(20) NOT NULL,
    -- SOFT, ESCALATE, HARD_STOP
    
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN',
    -- OPEN, UNDER_REVIEW, MITIGATED, WAIVED, BLOCKING, CLOSED
    
    -- Details
    description TEXT NOT NULL,
    source VARCHAR(50),
    -- SCREENING, DOCUMENT_REVIEW, ANALYST, SYSTEM, RULE_ENGINE, EXTERNAL
    
    source_reference TEXT,
    -- e.g., screening ID, document ID, rule name, external report reference
    
    -- Resolution
    raised_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    raised_by UUID,
    reviewed_at TIMESTAMPTZ,
    reviewed_by UUID,
    resolved_at TIMESTAMPTZ,
    resolved_by UUID,
    resolution_type VARCHAR(30),
    -- MITIGATED, WAIVED, FALSE_POSITIVE, ACCEPTED, REJECTED
    resolution_notes TEXT,
    
    -- Approval for waivers
    waiver_approved_by UUID,
    waiver_justification TEXT,
    
    CONSTRAINT chk_flag_severity CHECK (severity IN ('SOFT', 'ESCALATE', 'HARD_STOP')),
    CONSTRAINT chk_flag_status CHECK (status IN (
        'OPEN', 'UNDER_REVIEW', 'MITIGATED', 'WAIVED', 'BLOCKING', 'CLOSED'
    ))
);

CREATE INDEX idx_red_flags_case ON kyc.red_flags(case_id);
CREATE INDEX idx_red_flags_workstream ON kyc.red_flags(workstream_id) WHERE workstream_id IS NOT NULL;
CREATE INDEX idx_red_flags_status ON kyc.red_flags(status);
CREATE INDEX idx_red_flags_type ON kyc.red_flags(flag_type);
CREATE INDEX idx_red_flags_severity ON kyc.red_flags(severity);

COMMENT ON TABLE kyc.red_flags IS 'Risk indicators and issues found during KYC review';

-- -----------------------------------------------------------------------------
-- 4. DOCUMENT REQUESTS - Required documents per workstream
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.doc_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workstream_id UUID NOT NULL REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE,
    
    -- What document
    doc_type VARCHAR(50) NOT NULL,
    -- PASSPORT, NATIONAL_ID, DRIVERS_LICENSE, PROOF_OF_ADDRESS,
    -- CERT_OF_INCORPORATION, CERT_OF_GOOD_STANDING, MEMORANDUM_ARTICLES,
    -- REGISTER_OF_DIRECTORS, REGISTER_OF_SHAREHOLDERS, ANNUAL_RETURN,
    -- TRUST_DEED, LETTER_OF_WISHES, DEED_OF_APPOINTMENT,
    -- BANK_STATEMENT, SOURCE_OF_WEALTH, SOURCE_OF_FUNDS,
    -- FINANCIAL_STATEMENTS, TAX_RETURN, REGULATORY_LICENSE,
    -- POWER_OF_ATTORNEY, BOARD_RESOLUTION, SIGNATORY_LIST
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'REQUIRED',
    -- REQUIRED, REQUESTED, RECEIVED, UNDER_REVIEW, VERIFIED, REJECTED, WAIVED, EXPIRED
    
    -- Timing
    required_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    requested_at TIMESTAMPTZ,
    due_date DATE,
    received_at TIMESTAMPTZ,
    reviewed_at TIMESTAMPTZ,
    verified_at TIMESTAMPTZ,
    
    -- Document link
    document_id UUID REFERENCES "ob-poc".document_catalog(document_id),
    
    -- Review
    reviewer_id UUID,
    rejection_reason TEXT,
    verification_notes TEXT,
    
    -- Priority
    is_mandatory BOOLEAN DEFAULT true,
    priority VARCHAR(10) DEFAULT 'NORMAL',
    -- LOW, NORMAL, HIGH, URGENT
    
    CONSTRAINT chk_doc_status CHECK (status IN (
        'REQUIRED', 'REQUESTED', 'RECEIVED', 'UNDER_REVIEW', 
        'VERIFIED', 'REJECTED', 'WAIVED', 'EXPIRED'
    ))
);

CREATE INDEX idx_doc_requests_workstream ON kyc.doc_requests(workstream_id);
CREATE INDEX idx_doc_requests_status ON kyc.doc_requests(status);
CREATE INDEX idx_doc_requests_type ON kyc.doc_requests(doc_type);
CREATE INDEX idx_doc_requests_due ON kyc.doc_requests(due_date) WHERE due_date IS NOT NULL;

COMMENT ON TABLE kyc.doc_requests IS 'Document requirements and collection tracking';

-- -----------------------------------------------------------------------------
-- 5. SCREENINGS - Sanctions, PEP, adverse media checks
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.screenings (
    screening_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workstream_id UUID NOT NULL REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE,
    
    -- Type
    screening_type VARCHAR(30) NOT NULL,
    -- SANCTIONS, PEP, ADVERSE_MEDIA, CREDIT, CRIMINAL, REGULATORY, CONSOLIDATED
    
    -- Provider
    provider VARCHAR(50),
    -- REFINITIV, DOW_JONES, LEXISNEXIS, COMPLY_ADVANTAGE, INTERNAL
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    -- PENDING, RUNNING, CLEAR, HIT_PENDING_REVIEW, HIT_CONFIRMED, HIT_DISMISSED, ERROR, EXPIRED
    
    -- Timing
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    
    -- Results
    result_summary VARCHAR(100),
    result_data JSONB,
    match_count INTEGER DEFAULT 0,
    
    -- Review
    reviewed_by UUID,
    reviewed_at TIMESTAMPTZ,
    review_notes TEXT,
    
    -- Link to red flag if hit confirmed
    red_flag_id UUID REFERENCES kyc.red_flags(red_flag_id),
    
    CONSTRAINT chk_screening_type CHECK (screening_type IN (
        'SANCTIONS', 'PEP', 'ADVERSE_MEDIA', 'CREDIT', 'CRIMINAL', 'REGULATORY', 'CONSOLIDATED'
    )),
    CONSTRAINT chk_screening_status CHECK (status IN (
        'PENDING', 'RUNNING', 'CLEAR', 'HIT_PENDING_REVIEW', 
        'HIT_CONFIRMED', 'HIT_DISMISSED', 'ERROR', 'EXPIRED'
    ))
);

CREATE INDEX idx_screenings_workstream ON kyc.screenings(workstream_id);
CREATE INDEX idx_screenings_status ON kyc.screenings(status);
CREATE INDEX idx_screenings_type ON kyc.screenings(screening_type);

COMMENT ON TABLE kyc.screenings IS 'Screening results from various providers';

-- -----------------------------------------------------------------------------
-- 6. CASE EVENTS - Audit trail
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.case_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
    workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE,
    
    -- Event details
    event_type VARCHAR(50) NOT NULL,
    -- CASE_CREATED, CASE_STATUS_CHANGED, CASE_ASSIGNED, CASE_ESCALATED,
    -- WORKSTREAM_CREATED, WORKSTREAM_STATUS_CHANGED, WORKSTREAM_BLOCKED,
    -- RED_FLAG_RAISED, RED_FLAG_RESOLVED, RED_FLAG_WAIVED,
    -- DOC_REQUESTED, DOC_RECEIVED, DOC_VERIFIED, DOC_REJECTED,
    -- SCREENING_RUN, SCREENING_HIT, SCREENING_CLEARED,
    -- ENTITY_DISCOVERED, COMMENT_ADDED, APPROVAL_REQUESTED, APPROVAL_GRANTED,
    -- RULE_TRIGGERED, SLA_WARNING
    
    event_data JSONB DEFAULT '{}'::jsonb,
    
    -- Who / when
    actor_id UUID,
    actor_type VARCHAR(20) DEFAULT 'USER',
    -- USER, SYSTEM, RULE_ENGINE, INTEGRATION
    
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    -- Optional comment
    comment TEXT
);

CREATE INDEX idx_case_events_case ON kyc.case_events(case_id);
CREATE INDEX idx_case_events_workstream ON kyc.case_events(workstream_id) WHERE workstream_id IS NOT NULL;
CREATE INDEX idx_case_events_type ON kyc.case_events(event_type);
CREATE INDEX idx_case_events_time ON kyc.case_events(occurred_at DESC);

COMMENT ON TABLE kyc.case_events IS 'Audit log of all case activities';

-- -----------------------------------------------------------------------------
-- 7. RULE EXECUTION LOG - Track which rules fired
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.rule_executions (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
    workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id) ON DELETE CASCADE,
    
    -- Rule info
    rule_name VARCHAR(100) NOT NULL,
    trigger_event VARCHAR(50) NOT NULL,
    
    -- What happened
    condition_matched BOOLEAN NOT NULL,
    actions_executed JSONB DEFAULT '[]'::jsonb,
    
    -- Context snapshot (for debugging)
    context_snapshot JSONB DEFAULT '{}'::jsonb,
    
    executed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_rule_executions_case ON kyc.rule_executions(case_id);
CREATE INDEX idx_rule_executions_rule ON kyc.rule_executions(rule_name);
CREATE INDEX idx_rule_executions_time ON kyc.rule_executions(executed_at DESC);

COMMENT ON TABLE kyc.rule_executions IS 'Audit trail of rule engine executions';

-- -----------------------------------------------------------------------------
-- 8. APPROVAL REQUESTS - For escalated decisions
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS kyc.approval_requests (
    approval_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
    
    -- What needs approval
    approval_type VARCHAR(30) NOT NULL,
    -- CASE_APPROVAL, RED_FLAG_WAIVER, HIGH_RISK_ONBOARD, ENHANCED_DD_WAIVER
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    -- PENDING, APPROVED, REJECTED, ESCALATED, WITHDRAWN
    
    -- Request
    requested_by UUID NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_notes TEXT,
    
    -- Supporting info
    related_red_flag_id UUID REFERENCES kyc.red_flags(red_flag_id),
    related_workstream_id UUID REFERENCES kyc.entity_workstreams(workstream_id),
    
    -- Decision
    decided_by UUID,
    decided_at TIMESTAMPTZ,
    decision_notes TEXT,
    
    -- Escalation chain
    required_approver_level VARCHAR(30),
    -- SENIOR_ANALYST, TEAM_LEAD, SENIOR_COMPLIANCE, EXECUTIVE, BOARD
    
    CONSTRAINT chk_approval_status CHECK (status IN (
        'PENDING', 'APPROVED', 'REJECTED', 'ESCALATED', 'WITHDRAWN'
    ))
);

CREATE INDEX idx_approvals_case ON kyc.approval_requests(case_id);
CREATE INDEX idx_approvals_status ON kyc.approval_requests(status);

COMMENT ON TABLE kyc.approval_requests IS 'Approval workflow for escalated decisions';

-- -----------------------------------------------------------------------------
-- 9. VIEWS - Useful aggregations
-- -----------------------------------------------------------------------------

-- Case summary view
CREATE OR REPLACE VIEW kyc.v_case_summary AS
SELECT 
    c.case_id,
    c.cbu_id,
    cb.name as cbu_name,
    c.status,
    c.escalation_level,
    c.risk_rating,
    c.case_type,
    c.opened_at,
    c.sla_deadline,
    
    -- Counts
    (SELECT COUNT(*) FROM kyc.entity_workstreams w WHERE w.case_id = c.case_id) as total_workstreams,
    (SELECT COUNT(*) FROM kyc.entity_workstreams w WHERE w.case_id = c.case_id AND w.status = 'COMPLETE') as completed_workstreams,
    (SELECT COUNT(*) FROM kyc.entity_workstreams w WHERE w.case_id = c.case_id AND w.status = 'BLOCKED') as blocked_workstreams,
    
    (SELECT COUNT(*) FROM kyc.red_flags f WHERE f.case_id = c.case_id AND f.status = 'OPEN') as open_red_flags,
    (SELECT COUNT(*) FROM kyc.red_flags f WHERE f.case_id = c.case_id AND f.status = 'BLOCKING') as blocking_red_flags,
    (SELECT COUNT(*) FROM kyc.red_flags f WHERE f.case_id = c.case_id AND f.severity = 'HARD_STOP' AND f.status NOT IN ('MITIGATED', 'WAIVED', 'CLOSED')) as hard_stops,
    
    (SELECT COUNT(*) FROM kyc.doc_requests d 
     JOIN kyc.entity_workstreams w ON w.workstream_id = d.workstream_id
     WHERE w.case_id = c.case_id AND d.status = 'REQUIRED') as pending_docs,
    
    (SELECT COUNT(*) FROM kyc.screenings s
     JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
     WHERE w.case_id = c.case_id AND s.status = 'HIT_PENDING_REVIEW') as pending_screening_reviews,
    
    -- Computed flags for rules
    (SELECT COUNT(*) > 0 FROM kyc.red_flags f WHERE f.case_id = c.case_id AND f.flag_type = 'PEP_IDENTIFIED' AND f.status NOT IN ('CLOSED', 'MITIGATED')) as has_pep,
    (SELECT COUNT(*) > 0 FROM kyc.red_flags f WHERE f.case_id = c.case_id AND f.flag_type = 'ADVERSE_MEDIA' AND f.status NOT IN ('CLOSED', 'MITIGATED')) as has_adverse_media

FROM kyc.cases c
JOIN "ob-poc".cbus cb ON cb.cbu_id = c.cbu_id;

-- Workstream detail view
CREATE OR REPLACE VIEW kyc.v_workstream_detail AS
SELECT 
    w.workstream_id,
    w.case_id,
    w.entity_id,
    e.name as entity_name,
    et.type_code as entity_type,
    w.status,
    w.risk_rating,
    w.requires_enhanced_dd,
    w.is_ubo,
    w.ownership_percentage,
    w.discovery_depth,
    w.created_at,
    w.completed_at,
    
    -- Discovery chain
    w.discovery_source_workstream_id,
    w.discovery_reason,
    pw.entity_id as discovered_from_entity_id,
    
    -- Entity details
    COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
    t.trust_type,
    
    -- Counts
    (SELECT COUNT(*) FROM kyc.doc_requests d WHERE d.workstream_id = w.workstream_id AND d.status NOT IN ('VERIFIED', 'WAIVED')) as pending_docs,
    (SELECT COUNT(*) FROM kyc.screenings s WHERE s.workstream_id = w.workstream_id AND s.status = 'HIT_PENDING_REVIEW') as pending_hits,
    (SELECT COUNT(*) FROM kyc.red_flags f WHERE f.workstream_id = w.workstream_id AND f.status IN ('OPEN', 'BLOCKING')) as active_flags

FROM kyc.entity_workstreams w
JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
LEFT JOIN kyc.entity_workstreams pw ON pw.workstream_id = w.discovery_source_workstream_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_partnerships p ON p.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_trusts t ON t.entity_id = e.entity_id;

-- =============================================================================
-- END MIGRATION
-- =============================================================================
```

---

## Part 2: DSL Verbs

Add to `rust/config/verbs.yaml`:

```yaml
# =============================================================================
# KYC CASE MANAGEMENT VERBS
# =============================================================================

# -----------------------------------------------------------------------------
# kyc-case domain
# -----------------------------------------------------------------------------
- domain: kyc-case
  description: "KYC case lifecycle management"
  verbs:
    - name: create
      description: "Create a new KYC case for a CBU"
      behavior: crud
      table: kyc.cases
      operation: insert
      emits_event: case.created
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: case-type
          type: string
          required: false
          maps_to: case_type
          default: "NEW_CLIENT"
          valid_values: [NEW_CLIENT, PERIODIC_REVIEW, EVENT_DRIVEN, REMEDIATION]
        - name: sla-deadline
          type: timestamp
          required: false
          maps_to: sla_deadline
        - name: assigned-analyst-id
          type: uuid
          required: false
          maps_to: assigned_analyst_id
        - name: notes
          type: string
          required: false
          maps_to: notes
      returns:
        - name: case-id
          type: uuid
          maps_to: case_id

    - name: update-status
      description: "Update case status"
      behavior: crud
      table: kyc.cases
      operation: update
      emits_event: case.status-changed
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
          is_key: true
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [INTAKE, DISCOVERY, ASSESSMENT, REVIEW, APPROVED, REJECTED, BLOCKED, WITHDRAWN, EXPIRED]
      returns:
        - name: case-id
          type: uuid
          maps_to: case_id

    - name: escalate
      description: "Escalate case to higher authority"
      behavior: crud
      table: kyc.cases
      operation: update
      emits_event: case.escalated
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
          is_key: true
        - name: escalation-level
          type: string
          required: true
          maps_to: escalation_level
          valid_values: [STANDARD, SENIOR_COMPLIANCE, EXECUTIVE, BOARD]
      returns:
        - name: case-id
          type: uuid
          maps_to: case_id

    - name: assign
      description: "Assign case to analyst and/or reviewer"
      behavior: crud
      table: kyc.cases
      operation: update
      emits_event: case.assigned
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
          is_key: true
        - name: analyst-id
          type: uuid
          required: false
          maps_to: assigned_analyst_id
        - name: reviewer-id
          type: uuid
          required: false
          maps_to: assigned_reviewer_id
      returns:
        - name: case-id
          type: uuid
          maps_to: case_id

    - name: set-risk-rating
      description: "Set case risk rating"
      behavior: crud
      table: kyc.cases
      operation: update
      emits_event: case.risk-rated
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
          is_key: true
        - name: risk-rating
          type: string
          required: true
          maps_to: risk_rating
          valid_values: [LOW, MEDIUM, HIGH, VERY_HIGH, PROHIBITED]
      returns:
        - name: case-id
          type: uuid
          maps_to: case_id

    - name: close
      description: "Close the case"
      behavior: crud
      table: kyc.cases
      operation: update
      emits_event: case.closed
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
          is_key: true
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [APPROVED, REJECTED, WITHDRAWN, EXPIRED]
        - name: notes
          type: string
          required: false
          maps_to: notes
      set_values:
        closed_at: "now()"
      returns:
        - name: case-id
          type: uuid
          maps_to: case_id

    - name: read
      description: "Read case details"
      behavior: crud
      table: kyc.cases
      operation: select
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
      returns:
        - name: case-id
          type: uuid
          maps_to: case_id
        - name: cbu-id
          type: uuid
          maps_to: cbu_id
        - name: status
          type: string
          maps_to: status
        - name: escalation-level
          type: string
          maps_to: escalation_level
        - name: risk-rating
          type: string
          maps_to: risk_rating

# -----------------------------------------------------------------------------
# entity-workstream domain
# -----------------------------------------------------------------------------
- domain: entity-workstream
  description: "Per-entity workstream within a KYC case"
  verbs:
    - name: create
      description: "Create a new entity workstream"
      behavior: crud
      table: kyc.entity_workstreams
      operation: insert
      emits_event: workstream.created
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: discovery-source-id
          type: uuid
          required: false
          maps_to: discovery_source_workstream_id
        - name: discovery-reason
          type: string
          required: false
          maps_to: discovery_reason
        - name: discovery-depth
          type: integer
          required: false
          maps_to: discovery_depth
          default: 1
        - name: ownership-percentage
          type: decimal
          required: false
          maps_to: ownership_percentage
        - name: is-ubo
          type: boolean
          required: false
          maps_to: is_ubo
          default: false
      returns:
        - name: workstream-id
          type: uuid
          maps_to: workstream_id

    - name: update-status
      description: "Update workstream status"
      behavior: crud
      table: kyc.entity_workstreams
      operation: update
      emits_event: workstream.status-changed
      args:
        - name: workstream-id
          type: uuid
          required: true
          maps_to: workstream_id
          is_key: true
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [PENDING, COLLECT, VERIFY, SCREEN, ASSESS, COMPLETE, BLOCKED, ENHANCED_DD]
      returns:
        - name: workstream-id
          type: uuid
          maps_to: workstream_id

    - name: block
      description: "Block workstream with reason"
      behavior: crud
      table: kyc.entity_workstreams
      operation: update
      emits_event: workstream.blocked
      args:
        - name: workstream-id
          type: uuid
          required: true
          maps_to: workstream_id
          is_key: true
        - name: reason
          type: string
          required: true
          maps_to: blocked_reason
      set_values:
        status: "BLOCKED"
        blocked_at: "now()"
      returns:
        - name: workstream-id
          type: uuid
          maps_to: workstream_id

    - name: complete
      description: "Mark workstream as complete"
      behavior: crud
      table: kyc.entity_workstreams
      operation: update
      emits_event: workstream.completed
      args:
        - name: workstream-id
          type: uuid
          required: true
          maps_to: workstream_id
          is_key: true
        - name: risk-rating
          type: string
          required: false
          maps_to: risk_rating
          valid_values: [LOW, MEDIUM, HIGH, VERY_HIGH]
      set_values:
        status: "COMPLETE"
        completed_at: "now()"
      returns:
        - name: workstream-id
          type: uuid
          maps_to: workstream_id

    - name: set-enhanced-dd
      description: "Flag workstream for enhanced due diligence"
      behavior: crud
      table: kyc.entity_workstreams
      operation: update
      emits_event: workstream.enhanced-dd-set
      args:
        - name: workstream-id
          type: uuid
          required: true
          maps_to: workstream_id
          is_key: true
      set_values:
        requires_enhanced_dd: true
        status: "ENHANCED_DD"
      returns:
        - name: workstream-id
          type: uuid
          maps_to: workstream_id

# -----------------------------------------------------------------------------
# red-flag domain
# -----------------------------------------------------------------------------
- domain: red-flag
  description: "Risk indicators and issues"
  verbs:
    - name: raise
      description: "Raise a new red flag"
      behavior: crud
      table: kyc.red_flags
      operation: insert
      emits_event: red-flag.raised
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
        - name: workstream-id
          type: uuid
          required: false
          maps_to: workstream_id
        - name: flag-type
          type: string
          required: true
          maps_to: flag_type
          valid_values: [SANCTIONS_MATCH, PEP_IDENTIFIED, ADVERSE_MEDIA, NOMINEE_UNDISCLOSED, OPAQUE_STRUCTURE, DOCUMENT_FRAUD, INCONSISTENT_INFO, HIGH_RISK_JURISDICTION, COMPLEX_STRUCTURE, SOURCE_OF_WEALTH_UNCLEAR, MISSING_UBO, SHELL_COMPANY, TAX_HAVEN, CASH_INTENSIVE, REGULATORY_ACTION, DOCUMENT_OVERDUE]
        - name: severity
          type: string
          required: true
          maps_to: severity
          valid_values: [SOFT, ESCALATE, HARD_STOP]
        - name: description
          type: string
          required: true
          maps_to: description
        - name: source
          type: string
          required: false
          maps_to: source
          valid_values: [SCREENING, DOCUMENT_REVIEW, ANALYST, SYSTEM, RULE_ENGINE, EXTERNAL]
        - name: source-reference
          type: string
          required: false
          maps_to: source_reference
      returns:
        - name: red-flag-id
          type: uuid
          maps_to: red_flag_id

    - name: mitigate
      description: "Mark red flag as mitigated"
      behavior: crud
      table: kyc.red_flags
      operation: update
      emits_event: red-flag.mitigated
      args:
        - name: red-flag-id
          type: uuid
          required: true
          maps_to: red_flag_id
          is_key: true
        - name: notes
          type: string
          required: true
          maps_to: resolution_notes
      set_values:
        status: "MITIGATED"
        resolution_type: "MITIGATED"
        resolved_at: "now()"
      returns:
        - name: red-flag-id
          type: uuid
          maps_to: red_flag_id

    - name: waive
      description: "Waive red flag with justification"
      behavior: crud
      table: kyc.red_flags
      operation: update
      emits_event: red-flag.waived
      args:
        - name: red-flag-id
          type: uuid
          required: true
          maps_to: red_flag_id
          is_key: true
        - name: justification
          type: string
          required: true
          maps_to: waiver_justification
        - name: approved-by
          type: uuid
          required: true
          maps_to: waiver_approved_by
      set_values:
        status: "WAIVED"
        resolution_type: "WAIVED"
        resolved_at: "now()"
      returns:
        - name: red-flag-id
          type: uuid
          maps_to: red_flag_id

    - name: dismiss
      description: "Dismiss red flag as false positive"
      behavior: crud
      table: kyc.red_flags
      operation: update
      emits_event: red-flag.dismissed
      args:
        - name: red-flag-id
          type: uuid
          required: true
          maps_to: red_flag_id
          is_key: true
        - name: notes
          type: string
          required: true
          maps_to: resolution_notes
      set_values:
        status: "CLOSED"
        resolution_type: "FALSE_POSITIVE"
        resolved_at: "now()"
      returns:
        - name: red-flag-id
          type: uuid
          maps_to: red_flag_id

    - name: set-blocking
      description: "Set red flag as blocking the case"
      behavior: crud
      table: kyc.red_flags
      operation: update
      emits_event: red-flag.blocking
      args:
        - name: red-flag-id
          type: uuid
          required: true
          maps_to: red_flag_id
          is_key: true
      set_values:
        status: "BLOCKING"
      returns:
        - name: red-flag-id
          type: uuid
          maps_to: red_flag_id

# -----------------------------------------------------------------------------
# doc-request domain
# -----------------------------------------------------------------------------
- domain: doc-request
  description: "Document collection and verification"
  verbs:
    - name: create
      description: "Create a document request"
      behavior: crud
      table: kyc.doc_requests
      operation: insert
      emits_event: doc-request.created
      args:
        - name: workstream-id
          type: uuid
          required: true
          maps_to: workstream_id
        - name: doc-type
          type: string
          required: true
          maps_to: doc_type
          valid_values: [PASSPORT, NATIONAL_ID, DRIVERS_LICENSE, PROOF_OF_ADDRESS, CERT_OF_INCORPORATION, CERT_OF_GOOD_STANDING, MEMORANDUM_ARTICLES, REGISTER_OF_DIRECTORS, REGISTER_OF_SHAREHOLDERS, ANNUAL_RETURN, TRUST_DEED, LETTER_OF_WISHES, DEED_OF_APPOINTMENT, BANK_STATEMENT, SOURCE_OF_WEALTH, SOURCE_OF_FUNDS, FINANCIAL_STATEMENTS, TAX_RETURN, REGULATORY_LICENSE, POWER_OF_ATTORNEY, BOARD_RESOLUTION, SIGNATORY_LIST]
        - name: due-date
          type: date
          required: false
          maps_to: due_date
        - name: is-mandatory
          type: boolean
          required: false
          maps_to: is_mandatory
          default: true
        - name: priority
          type: string
          required: false
          maps_to: priority
          default: "NORMAL"
          valid_values: [LOW, NORMAL, HIGH, URGENT]
      returns:
        - name: request-id
          type: uuid
          maps_to: request_id

    - name: mark-requested
      description: "Mark document as formally requested"
      behavior: crud
      table: kyc.doc_requests
      operation: update
      emits_event: doc-request.requested
      args:
        - name: request-id
          type: uuid
          required: true
          maps_to: request_id
          is_key: true
      set_values:
        status: "REQUESTED"
        requested_at: "now()"
      returns:
        - name: request-id
          type: uuid
          maps_to: request_id

    - name: receive
      description: "Record document received"
      behavior: crud
      table: kyc.doc_requests
      operation: update
      emits_event: doc-request.received
      args:
        - name: request-id
          type: uuid
          required: true
          maps_to: request_id
          is_key: true
        - name: document-id
          type: uuid
          required: true
          maps_to: document_id
      set_values:
        status: "RECEIVED"
        received_at: "now()"
      returns:
        - name: request-id
          type: uuid
          maps_to: request_id

    - name: verify
      description: "Verify document as valid"
      behavior: crud
      table: kyc.doc_requests
      operation: update
      emits_event: doc-request.verified
      args:
        - name: request-id
          type: uuid
          required: true
          maps_to: request_id
          is_key: true
        - name: notes
          type: string
          required: false
          maps_to: verification_notes
      set_values:
        status: "VERIFIED"
        verified_at: "now()"
      returns:
        - name: request-id
          type: uuid
          maps_to: request_id

    - name: reject
      description: "Reject document"
      behavior: crud
      table: kyc.doc_requests
      operation: update
      emits_event: doc-request.rejected
      args:
        - name: request-id
          type: uuid
          required: true
          maps_to: request_id
          is_key: true
        - name: reason
          type: string
          required: true
          maps_to: rejection_reason
      set_values:
        status: "REJECTED"
      returns:
        - name: request-id
          type: uuid
          maps_to: request_id

    - name: waive
      description: "Waive document requirement"
      behavior: crud
      table: kyc.doc_requests
      operation: update
      emits_event: doc-request.waived
      args:
        - name: request-id
          type: uuid
          required: true
          maps_to: request_id
          is_key: true
        - name: notes
          type: string
          required: true
          maps_to: verification_notes
      set_values:
        status: "WAIVED"
      returns:
        - name: request-id
          type: uuid
          maps_to: request_id

# -----------------------------------------------------------------------------
# screening domain
# -----------------------------------------------------------------------------
- domain: screening
  description: "Sanctions, PEP, and adverse media screening"
  verbs:
    - name: run
      description: "Initiate a screening"
      behavior: crud
      table: kyc.screenings
      operation: insert
      emits_event: screening.started
      args:
        - name: workstream-id
          type: uuid
          required: true
          maps_to: workstream_id
        - name: screening-type
          type: string
          required: true
          maps_to: screening_type
          valid_values: [SANCTIONS, PEP, ADVERSE_MEDIA, CREDIT, CRIMINAL, REGULATORY, CONSOLIDATED]
        - name: provider
          type: string
          required: false
          maps_to: provider
      returns:
        - name: screening-id
          type: uuid
          maps_to: screening_id

    - name: complete
      description: "Record screening completion"
      behavior: crud
      table: kyc.screenings
      operation: update
      emits_event: screening.completed
      args:
        - name: screening-id
          type: uuid
          required: true
          maps_to: screening_id
          is_key: true
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [CLEAR, HIT_PENDING_REVIEW, ERROR]
        - name: result-summary
          type: string
          required: false
          maps_to: result_summary
        - name: match-count
          type: integer
          required: false
          maps_to: match_count
      set_values:
        completed_at: "now()"
      returns:
        - name: screening-id
          type: uuid
          maps_to: screening_id

    - name: review-hit
      description: "Review a screening hit"
      behavior: crud
      table: kyc.screenings
      operation: update
      emits_event: screening.reviewed
      args:
        - name: screening-id
          type: uuid
          required: true
          maps_to: screening_id
          is_key: true
        - name: status
          type: string
          required: true
          maps_to: status
          valid_values: [HIT_CONFIRMED, HIT_DISMISSED]
        - name: notes
          type: string
          required: true
          maps_to: review_notes
        - name: red-flag-id
          type: uuid
          required: false
          maps_to: red_flag_id
      set_values:
        reviewed_at: "now()"
      returns:
        - name: screening-id
          type: uuid
          maps_to: screening_id

# -----------------------------------------------------------------------------
# holding domain (for ownership events)
# -----------------------------------------------------------------------------
- domain: holding
  description: "Share class holdings"
  verbs:
    - name: create
      description: "Create a holding record"
      behavior: crud
      table: kyc.holdings
      operation: insert
      emits_event: holding.created
      args:
        - name: share-class-id
          type: uuid
          required: true
          maps_to: share_class_id
        - name: holder-entity-id
          type: uuid
          required: true
          maps_to: holder_entity_id
        - name: units
          type: decimal
          required: true
          maps_to: units
        - name: ownership-percentage
          type: decimal
          required: false
          maps_to: ownership_percentage
      returns:
        - name: holding-id
          type: uuid
          maps_to: holding_id

# -----------------------------------------------------------------------------
# case-event domain
# -----------------------------------------------------------------------------
- domain: case-event
  description: "Audit trail for case activities"
  verbs:
    - name: log
      description: "Log a case event"
      behavior: crud
      table: kyc.case_events
      operation: insert
      args:
        - name: case-id
          type: uuid
          required: true
          maps_to: case_id
        - name: workstream-id
          type: uuid
          required: false
          maps_to: workstream_id
        - name: event-type
          type: string
          required: true
          maps_to: event_type
        - name: event-data
          type: json
          required: false
          maps_to: event_data
        - name: actor-id
          type: uuid
          required: false
          maps_to: actor_id
        - name: actor-type
          type: string
          required: false
          maps_to: actor_type
          default: "USER"
          valid_values: [USER, SYSTEM, RULE_ENGINE, INTEGRATION]
        - name: comment
          type: string
          required: false
          maps_to: comment
      returns:
        - name: event-id
          type: uuid
          maps_to: event_id
```

---

## Part 3: DSL Rules Engine

The rules engine is the core intelligence. Rules are defined in YAML with a condition language that supports AND/OR/NOT logic, operators, and variable interpolation.

### File: `rust/config/rules.yaml`

```yaml
# =============================================================================
# KYC RISK RULES - DSL DEFINITION
# =============================================================================
#
# Rules are evaluated automatically when events occur.
# Each rule has:
#   - name: unique identifier
#   - description: human readable
#   - priority: lower = higher priority (1 = first)
#   - trigger: which event activates this rule
#   - condition: when to fire (supports all/any/not logic)
#   - actions: what to do when condition matches
#
# Condition Operators:
#   equals, not_equals, in, not_in, contains, starts_with,
#   gt, gte, lt, lte, is_null, is_not_null, matches (regex)
#
# Logic:
#   all: [conditions] - AND
#   any: [conditions] - OR
#   not: {condition}  - NOT
#
# Variables:
#   ${entity.jurisdiction} - field from context
#   ${now} - current timestamp
#   ${now + 3 days} - future timestamp
# =============================================================================

rules:

  # ===========================================================================
  # JURISDICTION RULES
  # ===========================================================================
  
  - name: high-risk-jurisdiction-cayman
    description: "Flag entities in Cayman Islands"
    priority: 10
    trigger:
      event: workstream.created
    condition:
      all:
        - field: entity.jurisdiction
          in: [KY]
    actions:
      - type: raise-red-flag
        params:
          flag-type: HIGH_RISK_JURISDICTION
          severity: ESCALATE
          description: "Entity registered in Cayman Islands (${entity.jurisdiction})"
          source: RULE_ENGINE
          source-reference: high-risk-jurisdiction-cayman
      - type: require-document
        params:
          doc-type: SOURCE_OF_FUNDS
          priority: HIGH

  - name: high-risk-jurisdiction-bvi
    description: "Flag entities in BVI"
    priority: 10
    trigger:
      event: workstream.created
    condition:
      all:
        - field: entity.jurisdiction
          in: [VG, BVI]
    actions:
      - type: raise-red-flag
        params:
          flag-type: HIGH_RISK_JURISDICTION
          severity: ESCALATE
          description: "Entity registered in British Virgin Islands"
          source: RULE_ENGINE
          source-reference: high-risk-jurisdiction-bvi
      - type: require-document
        params:
          doc-type: SOURCE_OF_FUNDS
          priority: HIGH

  - name: tax-haven-jurisdiction
    description: "Flag entities in tax haven jurisdictions"
    priority: 20
    trigger:
      event: workstream.created
    condition:
      all:
        - field: entity.jurisdiction
          in: [PA, JE, GG, IM, MH, BM, AN, TC, VU]
    actions:
      - type: raise-red-flag
        params:
          flag-type: TAX_HAVEN
          severity: SOFT
          description: "Entity in tax haven jurisdiction: ${entity.jurisdiction}"
          source: RULE_ENGINE
          source-reference: tax-haven-jurisdiction

  # ===========================================================================
  # SCREENING RULES
  # ===========================================================================
  
  - name: sanctions-hard-stop
    description: "Sanctions match is immediate hard stop"
    priority: 1
    trigger:
      event: screening.completed
    condition:
      all:
        - field: screening.type
          equals: SANCTIONS
        - field: screening.status
          in: [HIT_PENDING_REVIEW, HIT_CONFIRMED]
    actions:
      - type: raise-red-flag
        params:
          flag-type: SANCTIONS_MATCH
          severity: HARD_STOP
          description: "Sanctions screening match detected"
          source: SCREENING
          source-reference: "${screening.id}"
      - type: block-workstream
        params:
          reason: "Sanctions match - cannot proceed without clearance"
      - type: update-case-status
        params:
          status: BLOCKED
      - type: escalate-case
        params:
          level: EXECUTIVE

  - name: pep-enhanced-dd
    description: "PEP requires enhanced due diligence"
    priority: 5
    trigger:
      event: screening.completed
    condition:
      all:
        - field: screening.type
          equals: PEP
        - field: screening.status
          in: [HIT_PENDING_REVIEW, HIT_CONFIRMED]
    actions:
      - type: raise-red-flag
        params:
          flag-type: PEP_IDENTIFIED
          severity: ESCALATE
          description: "Politically Exposed Person identified"
          source: SCREENING
          source-reference: "${screening.id}"
      - type: set-enhanced-dd
      - type: require-document
        params:
          doc-type: SOURCE_OF_WEALTH
          priority: URGENT
      - type: require-document
        params:
          doc-type: SOURCE_OF_FUNDS
          priority: URGENT
      - type: escalate-case
        params:
          level: SENIOR_COMPLIANCE

  - name: adverse-media-escalate
    description: "Adverse media hits require review"
    priority: 15
    trigger:
      event: screening.completed
    condition:
      all:
        - field: screening.type
          equals: ADVERSE_MEDIA
        - field: screening.status
          equals: HIT_PENDING_REVIEW
    actions:
      - type: raise-red-flag
        params:
          flag-type: ADVERSE_MEDIA
          severity: ESCALATE
          description: "Adverse media coverage detected (${screening.match_count} matches)"
          source: SCREENING
          source-reference: "${screening.id}"
      - type: escalate-case
        params:
          level: SENIOR_COMPLIANCE

  # ===========================================================================
  # ENTITY TYPE RULES
  # ===========================================================================
  
  - name: discretionary-trust
    description: "Discretionary trusts are opaque structures"
    priority: 20
    trigger:
      event: workstream.created
    condition:
      all:
        - field: entity.type
          equals: trust
        - field: entity.trust_type
          equals: DISCRETIONARY
    actions:
      - type: raise-red-flag
        params:
          flag-type: OPAQUE_STRUCTURE
          severity: ESCALATE
          description: "Discretionary trust with no fixed beneficiaries"
          source: RULE_ENGINE
          source-reference: discretionary-trust
      - type: set-enhanced-dd
      - type: require-document
        params:
          doc-type: TRUST_DEED
          priority: HIGH
      - type: require-document
        params:
          doc-type: LETTER_OF_WISHES
          priority: HIGH
      - type: require-document
        params:
          doc-type: DEED_OF_APPOINTMENT
          priority: NORMAL

  - name: nominee-detection
    description: "Nominee arrangements require principal disclosure"
    priority: 5
    trigger:
      event: workstream.created
    condition:
      any:
        - field: entity.name
          contains: nominee
        - field: entity.name
          contains: Nominee
        - field: entity.name
          contains: NOMINEE
        - field: entity.name
          contains: Designated
        - field: entity.name
          contains: DESIGNATED
    actions:
      - type: raise-red-flag
        params:
          flag-type: NOMINEE_UNDISCLOSED
          severity: HARD_STOP
          description: "Nominee arrangement detected - principal disclosure required"
          source: RULE_ENGINE
          source-reference: nominee-detection
      - type: block-workstream
        params:
          reason: "Nominee principal must be disclosed before proceeding"

  - name: shell-company-indicators
    description: "Detect potential shell company characteristics"
    priority: 25
    trigger:
      event: workstream.created
    condition:
      all:
        - field: entity.type
          equals: limited_company
        - any:
            - field: entity.name
              contains: Holdings
            - field: entity.name
              contains: Investments
            - field: entity.name
              contains: International
        - field: entity.jurisdiction
          in: [KY, VG, BVI, PA, BM]
    actions:
      - type: raise-red-flag
        params:
          flag-type: SHELL_COMPANY
          severity: SOFT
          description: "Potential shell company indicators detected"
          source: RULE_ENGINE
          source-reference: shell-company-indicators
      - type: require-document
        params:
          doc-type: FINANCIAL_STATEMENTS
          priority: HIGH
      - type: require-document
        params:
          doc-type: SOURCE_OF_WEALTH
          priority: HIGH

  # ===========================================================================
  # OWNERSHIP RULES
  # ===========================================================================
  
  - name: ubo-threshold-25
    description: "25%+ ownership qualifies as UBO"
    priority: 50
    trigger:
      event: holding.created
    condition:
      all:
        - field: holding.ownership_percentage
          gte: 25
    actions:
      - type: set-ubo
      - type: require-document
        params:
          doc-type: PASSPORT
          priority: HIGH
      - type: require-document
        params:
          doc-type: PROOF_OF_ADDRESS
          priority: HIGH

  - name: controlling-ownership
    description: "Majority ownership requires extra scrutiny"
    priority: 45
    trigger:
      event: holding.created
    condition:
      all:
        - field: holding.ownership_percentage
          gt: 50
    actions:
      - type: set-ubo
      - type: require-document
        params:
          doc-type: SOURCE_OF_WEALTH
          priority: HIGH

  - name: complex-ownership-structure
    description: "Flag structures with 4+ layers"
    priority: 30
    trigger:
      event: workstream.created
    condition:
      all:
        - field: workstream.discovery_depth
          gte: 4
    actions:
      - type: raise-red-flag
        params:
          flag-type: COMPLEX_STRUCTURE
          severity: ESCALATE
          description: "Complex ownership structure with ${workstream.discovery_depth} layers"
          source: RULE_ENGINE
          source-reference: complex-ownership-structure
      - type: set-enhanced-dd
      - type: escalate-case
        params:
          level: SENIOR_COMPLIANCE

  # ===========================================================================
  # COMPOUND / COMBINATION RULES
  # ===========================================================================
  
  - name: high-risk-combination
    description: "Multiple risk factors trigger very high risk rating"
    priority: 15
    trigger:
      event: red-flag.raised
    condition:
      all:
        - field: case.open_red_flags_count
          gte: 3
        - any:
            - field: case.has_pep
              equals: true
            - field: case.has_adverse_media
              equals: true
    actions:
      - type: set-case-risk
        params:
          rating: VERY_HIGH
      - type: escalate-case
        params:
          level: EXECUTIVE
      - type: log-event
        params:
          event-type: HIGH_RISK_COMBINATION
          comment: "Multiple risk factors detected - case marked VERY_HIGH risk"

  - name: pep-plus-jurisdiction
    description: "PEP in high risk jurisdiction is executive escalation"
    priority: 12
    trigger:
      event: red-flag.raised
    condition:
      all:
        - field: case.has_pep
          equals: true
        - field: entity.jurisdiction
          in: [KY, VG, BVI, PA, RU, CN, IR]
    actions:
      - type: set-case-risk
        params:
          rating: VERY_HIGH
      - type: escalate-case
        params:
          level: EXECUTIVE
      - type: log-event
        params:
          event-type: PEP_HIGH_RISK_JURISDICTION
          comment: "PEP identified in high-risk jurisdiction - executive review required"

  # ===========================================================================
  # TEMPORAL / SLA RULES (scheduled, not event-driven)
  # ===========================================================================
  
  - name: document-overdue
    description: "Flag overdue document requests"
    priority: 100
    trigger:
      event: scheduled
      schedule: daily
    condition:
      all:
        - field: doc_request.status
          in: [REQUIRED, REQUESTED]
        - field: doc_request.due_date
          lt: "${now}"
    actions:
      - type: raise-red-flag
        params:
          flag-type: DOCUMENT_OVERDUE
          severity: SOFT
          description: "Document ${doc_request.doc_type} overdue since ${doc_request.due_date}"
          source: RULE_ENGINE
          source-reference: document-overdue
      - type: log-event
        params:
          event-type: DOCUMENT_OVERDUE
          comment: "Document request past due date"

  - name: case-sla-warning
    description: "Warn when case approaching SLA deadline"
    priority: 100
    trigger:
      event: scheduled
      schedule: daily
    condition:
      all:
        - field: case.status
          not_in: [APPROVED, REJECTED, WITHDRAWN, EXPIRED]
        - field: case.sla_deadline
          lt: "${now + 3 days}"
        - field: case.sla_deadline
          gte: "${now}"
    actions:
      - type: escalate-case
        params:
          level: TEAM_LEAD
      - type: log-event
        params:
          event-type: SLA_WARNING
          comment: "Case approaching SLA deadline - ${case.sla_deadline}"

  - name: case-sla-breach
    description: "Handle SLA breach"
    priority: 99
    trigger:
      event: scheduled
      schedule: daily
    condition:
      all:
        - field: case.status
          not_in: [APPROVED, REJECTED, WITHDRAWN, EXPIRED]
        - field: case.sla_deadline
          lt: "${now}"
    actions:
      - type: raise-red-flag
        params:
          flag-type: SLA_BREACH
          severity: ESCALATE
          description: "Case SLA breached - deadline was ${case.sla_deadline}"
          source: RULE_ENGINE
          source-reference: case-sla-breach
      - type: escalate-case
        params:
          level: SENIOR_COMPLIANCE
      - type: log-event
        params:
          event-type: SLA_BREACH
          comment: "Case has exceeded SLA deadline"

  # ===========================================================================
  # WORKFLOW AUTOMATION RULES
  # ===========================================================================
  
  - name: auto-advance-to-verify
    description: "Auto-advance workstream when all docs received"
    priority: 200
    trigger:
      event: doc-request.received
    condition:
      all:
        - field: workstream.status
          equals: COLLECT
        - field: workstream.pending_docs
          equals: 0
    actions:
      - type: update-workstream-status
        params:
          status: VERIFY
      - type: log-event
        params:
          event-type: WORKSTREAM_AUTO_ADVANCED
          comment: "All documents received - advanced to VERIFY"

  - name: auto-advance-to-screen
    description: "Auto-advance workstream when all docs verified"
    priority: 200
    trigger:
      event: doc-request.verified
    condition:
      all:
        - field: workstream.status
          equals: VERIFY
        - field: workstream.pending_docs
          equals: 0
    actions:
      - type: update-workstream-status
        params:
          status: SCREEN
      - type: auto-run-screenings
      - type: log-event
        params:
          event-type: WORKSTREAM_AUTO_ADVANCED
          comment: "All documents verified - advanced to SCREEN, screenings initiated"
```

---

### File: `rust/src/kyc/rules/mod.rs`

```rust
//! DSL-driven KYC rules engine
//!
//! Rules are defined in YAML and evaluated automatically when events occur.
//! The engine supports:
//! - Complex conditions (AND/OR/NOT)
//! - Multiple operators (equals, in, contains, gte, etc.)
//! - Variable interpolation
//! - Multiple actions per rule
//! - Scheduled (temporal) rules

pub mod parser;
pub mod evaluator;
pub mod executor;
pub mod context;
pub mod event_bus;
pub mod scheduler;

pub use parser::{Rule, RulesConfig, load_rules};
pub use evaluator::RuleEvaluator;
pub use executor::ActionExecutor;
pub use context::RuleContext;
pub use event_bus::{KycEvent, KycEventBus};
pub use scheduler::RuleScheduler;
```

---

### File: `rust/src/kyc/rules/parser.rs`

```rust
//! Parse rules from YAML configuration

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RulesConfig {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub name: String,
    pub description: String,
    pub priority: i32,
    pub trigger: Trigger,
    pub condition: Condition,
    pub actions: Vec<Action>,
    
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl Rule {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Trigger {
    pub event: String,
    #[serde(default)]
    pub schedule: Option<String>,
}

impl Trigger {
    pub fn is_scheduled(&self) -> bool {
        self.event == "scheduled"
    }
    
    pub fn matches_event(&self, event_name: &str) -> bool {
        self.event == event_name
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Condition {
    All { all: Vec<Condition> },
    Any { any: Vec<Condition> },
    Not { not: Box<Condition> },
    Leaf(LeafCondition),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeafCondition {
    pub field: String,
    #[serde(flatten)]
    pub operator: Operator,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Equals(Value),
    NotEquals(Value),
    In(Vec<Value>),
    NotIn(Vec<Value>),
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    Gt(f64),
    Gte(f64),
    Lt(f64),
    Lte(f64),
    IsNull(bool),
    IsNotNull(bool),
    Matches(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Action {
    #[serde(rename = "type")]
    pub action_type: String,
    
    #[serde(default)]
    pub params: HashMap<String, Value>,
}

/// Load rules from a YAML file
pub fn load_rules(path: &Path) -> Result<Vec<Rule>> {
    let content = std::fs::read_to_string(path)?;
    let config: RulesConfig = serde_yaml::from_str(&content)?;
    
    // Filter to enabled rules only
    let rules: Vec<Rule> = config.rules
        .into_iter()
        .filter(|r| r.is_enabled())
        .collect();
    
    tracing::info!("Loaded {} rules from {:?}", rules.len(), path);
    
    Ok(rules)
}

/// Load rules from a string (for testing)
pub fn load_rules_from_str(yaml: &str) -> Result<Vec<Rule>> {
    let config: RulesConfig = serde_yaml::from_str(yaml)?;
    Ok(config.rules.into_iter().filter(|r| r.is_enabled()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_rule() {
        let yaml = r#"
rules:
  - name: test-rule
    description: "Test rule"
    priority: 10
    trigger:
      event: workstream.created
    condition:
      field: entity.jurisdiction
      equals: KY
    actions:
      - type: raise-red-flag
        params:
          flag-type: HIGH_RISK_JURISDICTION
          severity: ESCALATE
"#;
        
        let rules = load_rules_from_str(yaml).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "test-rule");
    }
    
    #[test]
    fn test_parse_compound_condition() {
        let yaml = r#"
rules:
  - name: compound-rule
    description: "Compound condition"
    priority: 10
    trigger:
      event: workstream.created
    condition:
      all:
        - field: entity.type
          equals: trust
        - any:
            - field: entity.jurisdiction
              in: [KY, VG]
            - field: entity.name
              contains: nominee
    actions:
      - type: raise-red-flag
        params:
          flag-type: COMPLEX
          severity: ESCALATE
"#;
        
        let rules = load_rules_from_str(yaml).unwrap();
        assert_eq!(rules.len(), 1);
        
        match &rules[0].condition {
            Condition::All { all } => assert_eq!(all.len(), 2),
            _ => panic!("Expected All condition"),
        }
    }
}
```

---

### File: `rust/src/kyc/rules/context.rs`

```rust
//! Build rule evaluation context from database and events

use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use chrono::{Utc, Duration};

/// Context for rule evaluation containing all field values
#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    values: HashMap<String, Value>,
}

impl RuleContext {
    pub fn new() -> Self {
        Self { values: HashMap::new() }
    }
    
    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.values.insert(key.to_string(), value.into());
    }
    
    pub fn set_opt<T: Into<Value>>(&mut self, key: &str, value: Option<T>) {
        if let Some(v) = value {
            self.values.insert(key.to_string(), v.into());
        }
    }
    
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }
    
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }
    
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.values.get(key).and_then(|v| v.as_f64())
    }
    
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| v.as_i64())
    }
    
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| v.as_bool())
    }
    
    pub fn get_uuid(&self, key: &str) -> Option<Uuid> {
        self.get_string(key).and_then(|s| Uuid::parse_str(s).ok())
    }
    
    /// Expand variables in a template string
    /// e.g., "Entity in ${entity.jurisdiction}" -> "Entity in KY"
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = template.to_string();
        
        // Handle time expressions like ${now} and ${now + 3 days}
        result = self.interpolate_time_expressions(&result);
        
        // Handle field references like ${entity.jurisdiction}
        for (key, value) in &self.values {
            let placeholder = format!("${{{}}}", key);
            let replacement = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => value.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
        
        result
    }
    
    fn interpolate_time_expressions(&self, template: &str) -> String {
        let mut result = template.to_string();
        let now = Utc::now();
        
        // Replace ${now}
        result = result.replace("${now}", &now.to_rfc3339());
        
        // Replace ${now + N days}
        let re = regex::Regex::new(r"\$\{now\s*\+\s*(\d+)\s*days?\}").unwrap();
        result = re.replace_all(&result, |caps: &regex::Captures| {
            let days: i64 = caps[1].parse().unwrap_or(0);
            let future = now + Duration::days(days);
            future.to_rfc3339()
        }).to_string();
        
        // Replace ${now - N days}
        let re = regex::Regex::new(r"\$\{now\s*-\s*(\d+)\s*days?\}").unwrap();
        result = re.replace_all(&result, |caps: &regex::Captures| {
            let days: i64 = caps[1].parse().unwrap_or(0);
            let past = now - Duration::days(days);
            past.to_rfc3339()
        }).to_string();
        
        result
    }
    
    /// Snapshot context for audit logging
    pub fn snapshot(&self) -> Value {
        Value::Object(
            self.values
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        )
    }
}

/// Build context for workstream.created event
pub async fn build_workstream_created_context(
    pool: &PgPool,
    case_id: Uuid,
    workstream_id: Uuid,
    entity_id: Uuid,
) -> Result<RuleContext> {
    let mut ctx = RuleContext::new();
    
    // Core IDs
    ctx.set("case.id", case_id.to_string());
    ctx.set("workstream.id", workstream_id.to_string());
    ctx.set("entity.id", entity_id.to_string());
    
    // Load entity details
    let entity = sqlx::query!(
        r#"
        SELECT 
            e.name,
            et.type_code as entity_type,
            COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
            t.trust_type
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
        LEFT JOIN "ob-poc".entity_partnerships p ON p.entity_id = e.entity_id
        LEFT JOIN "ob-poc".entity_trusts t ON t.entity_id = e.entity_id
        WHERE e.entity_id = $1
        "#,
        entity_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("entity.name", entity.name);
    ctx.set("entity.type", entity.entity_type);
    ctx.set_opt("entity.jurisdiction", entity.jurisdiction);
    ctx.set_opt("entity.trust_type", entity.trust_type);
    
    // Load workstream details
    let ws = sqlx::query!(
        r#"
        SELECT 
            status,
            discovery_depth,
            ownership_percentage,
            is_ubo,
            requires_enhanced_dd
        FROM kyc.entity_workstreams
        WHERE workstream_id = $1
        "#,
        workstream_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("workstream.status", ws.status);
    ctx.set("workstream.discovery_depth", ws.discovery_depth.unwrap_or(1) as i64);
    ctx.set("workstream.is_ubo", ws.is_ubo);
    ctx.set("workstream.requires_enhanced_dd", ws.requires_enhanced_dd);
    if let Some(pct) = ws.ownership_percentage {
        ctx.set("workstream.ownership_percentage", pct.to_string().parse::<f64>().unwrap_or(0.0));
    }
    
    // Load case summary
    load_case_context(pool, case_id, &mut ctx).await?;
    
    // Load workstream doc/screening stats
    load_workstream_stats(pool, workstream_id, &mut ctx).await?;
    
    Ok(ctx)
}

/// Build context for screening.completed event
pub async fn build_screening_completed_context(
    pool: &PgPool,
    case_id: Uuid,
    workstream_id: Uuid,
    screening_id: Uuid,
) -> Result<RuleContext> {
    let mut ctx = RuleContext::new();
    
    ctx.set("case.id", case_id.to_string());
    ctx.set("workstream.id", workstream_id.to_string());
    ctx.set("screening.id", screening_id.to_string());
    
    // Load screening details
    let screening = sqlx::query!(
        r#"
        SELECT 
            screening_type,
            status,
            match_count,
            result_summary
        FROM kyc.screenings
        WHERE screening_id = $1
        "#,
        screening_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("screening.type", screening.screening_type);
    ctx.set("screening.status", screening.status);
    ctx.set("screening.match_count", screening.match_count.unwrap_or(0) as i64);
    ctx.set_opt("screening.result_summary", screening.result_summary);
    
    // Load entity for this workstream
    let ws = sqlx::query!(
        "SELECT entity_id FROM kyc.entity_workstreams WHERE workstream_id = $1",
        workstream_id
    )
    .fetch_one(pool)
    .await?;
    
    load_entity_context(pool, ws.entity_id, &mut ctx).await?;
    load_case_context(pool, case_id, &mut ctx).await?;
    
    Ok(ctx)
}

/// Build context for holding.created event
pub async fn build_holding_created_context(
    pool: &PgPool,
    case_id: Uuid,
    workstream_id: Uuid,
    holding_id: Uuid,
) -> Result<RuleContext> {
    let mut ctx = RuleContext::new();
    
    ctx.set("case.id", case_id.to_string());
    ctx.set("workstream.id", workstream_id.to_string());
    ctx.set("holding.id", holding_id.to_string());
    
    // Load holding details
    let holding = sqlx::query!(
        r#"
        SELECT 
            ownership_percentage,
            units
        FROM kyc.holdings
        WHERE holding_id = $1
        "#,
        holding_id
    )
    .fetch_one(pool)
    .await?;
    
    if let Some(pct) = holding.ownership_percentage {
        ctx.set("holding.ownership_percentage", pct.to_string().parse::<f64>().unwrap_or(0.0));
    }
    
    load_case_context(pool, case_id, &mut ctx).await?;
    
    Ok(ctx)
}

/// Build context for red-flag.raised event
pub async fn build_red_flag_raised_context(
    pool: &PgPool,
    case_id: Uuid,
    red_flag_id: Uuid,
) -> Result<RuleContext> {
    let mut ctx = RuleContext::new();
    
    ctx.set("case.id", case_id.to_string());
    ctx.set("red_flag.id", red_flag_id.to_string());
    
    // Load red flag details
    let rf = sqlx::query!(
        r#"
        SELECT 
            flag_type,
            severity,
            status,
            workstream_id
        FROM kyc.red_flags
        WHERE red_flag_id = $1
        "#,
        red_flag_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("red_flag.type", rf.flag_type);
    ctx.set("red_flag.severity", rf.severity);
    ctx.set("red_flag.status", rf.status);
    
    if let Some(ws_id) = rf.workstream_id {
        ctx.set("workstream.id", ws_id.to_string());
        
        let ws = sqlx::query!(
            "SELECT entity_id FROM kyc.entity_workstreams WHERE workstream_id = $1",
            ws_id
        )
        .fetch_one(pool)
        .await?;
        
        load_entity_context(pool, ws.entity_id, &mut ctx).await?;
    }
    
    load_case_context(pool, case_id, &mut ctx).await?;
    
    Ok(ctx)
}

/// Build context for doc-request events
pub async fn build_doc_request_context(
    pool: &PgPool,
    case_id: Uuid,
    workstream_id: Uuid,
    request_id: Uuid,
) -> Result<RuleContext> {
    let mut ctx = RuleContext::new();
    
    ctx.set("case.id", case_id.to_string());
    ctx.set("workstream.id", workstream_id.to_string());
    ctx.set("doc_request.id", request_id.to_string());
    
    // Load doc request
    let doc = sqlx::query!(
        r#"
        SELECT 
            doc_type,
            status,
            due_date,
            priority
        FROM kyc.doc_requests
        WHERE request_id = $1
        "#,
        request_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("doc_request.doc_type", doc.doc_type);
    ctx.set("doc_request.status", doc.status);
    ctx.set_opt("doc_request.priority", doc.priority);
    if let Some(due) = doc.due_date {
        ctx.set("doc_request.due_date", due.to_string());
    }
    
    load_case_context(pool, case_id, &mut ctx).await?;
    load_workstream_stats(pool, workstream_id, &mut ctx).await?;
    
    Ok(ctx)
}

// Helper functions

async fn load_entity_context(pool: &PgPool, entity_id: Uuid, ctx: &mut RuleContext) -> Result<()> {
    let entity = sqlx::query!(
        r#"
        SELECT 
            e.name,
            et.type_code as entity_type,
            COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
            t.trust_type
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
        LEFT JOIN "ob-poc".entity_partnerships p ON p.entity_id = e.entity_id
        LEFT JOIN "ob-poc".entity_trusts t ON t.entity_id = e.entity_id
        WHERE e.entity_id = $1
        "#,
        entity_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("entity.id", entity_id.to_string());
    ctx.set("entity.name", entity.name);
    ctx.set("entity.type", entity.entity_type);
    ctx.set_opt("entity.jurisdiction", entity.jurisdiction);
    ctx.set_opt("entity.trust_type", entity.trust_type);
    
    Ok(())
}

async fn load_case_context(pool: &PgPool, case_id: Uuid, ctx: &mut RuleContext) -> Result<()> {
    let case_summary = sqlx::query!(
        r#"
        SELECT 
            status,
            escalation_level,
            risk_rating,
            sla_deadline,
            open_red_flags as "open_red_flags!",
            has_pep as "has_pep!",
            has_adverse_media as "has_adverse_media!"
        FROM kyc.v_case_summary
        WHERE case_id = $1
        "#,
        case_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("case.status", case_summary.status);
    ctx.set("case.escalation_level", case_summary.escalation_level);
    ctx.set_opt("case.risk_rating", case_summary.risk_rating);
    ctx.set("case.open_red_flags_count", case_summary.open_red_flags);
    ctx.set("case.has_pep", case_summary.has_pep);
    ctx.set("case.has_adverse_media", case_summary.has_adverse_media);
    
    if let Some(sla) = case_summary.sla_deadline {
        ctx.set("case.sla_deadline", sla.to_rfc3339());
    }
    
    Ok(())
}

async fn load_workstream_stats(pool: &PgPool, workstream_id: Uuid, ctx: &mut RuleContext) -> Result<()> {
    let stats = sqlx::query!(
        r#"
        SELECT 
            (SELECT COUNT(*) FROM kyc.doc_requests WHERE workstream_id = $1 AND status NOT IN ('VERIFIED', 'WAIVED')) as pending_docs,
            (SELECT COUNT(*) FROM kyc.screenings WHERE workstream_id = $1 AND status = 'HIT_PENDING_REVIEW') as pending_hits
        "#,
        workstream_id
    )
    .fetch_one(pool)
    .await?;
    
    ctx.set("workstream.pending_docs", stats.pending_docs.unwrap_or(0));
    ctx.set("workstream.pending_hits", stats.pending_hits.unwrap_or(0));
    
    Ok(())
}
```

---

### File: `rust/src/kyc/rules/evaluator.rs`

```rust
//! Evaluate rule conditions against context

use super::parser::{Condition, LeafCondition, Operator};
use super::context::RuleContext;
use serde_json::Value;
use chrono::{DateTime, Utc};

pub struct RuleEvaluator;

impl RuleEvaluator {
    pub fn new() -> Self {
        Self
    }
    
    /// Evaluate a condition against the context
    pub fn evaluate(&self, condition: &Condition, context: &RuleContext) -> bool {
        match condition {
            Condition::All { all } => {
                all.iter().all(|c| self.evaluate(c, context))
            }
            Condition::Any { any } => {
                any.iter().any(|c| self.evaluate(c, context))
            }
            Condition::Not { not } => {
                !self.evaluate(not, context)
            }
            Condition::Leaf(leaf) => {
                self.evaluate_leaf(leaf, context)
            }
        }
    }
    
    fn evaluate_leaf(&self, leaf: &LeafCondition, context: &RuleContext) -> bool {
        let value = context.get(&leaf.field);
        
        // Handle time expression comparisons
        if self.is_time_comparison(&leaf.operator) {
            return self.evaluate_time_comparison(&leaf.operator, value, context);
        }
        
        match (&leaf.operator, value) {
            // Equals
            (Operator::Equals(expected), Some(actual)) => {
                self.values_equal(expected, actual)
            }
            (Operator::Equals(_), None) => false,
            
            // Not Equals
            (Operator::NotEquals(expected), Some(actual)) => {
                !self.values_equal(expected, actual)
            }
            (Operator::NotEquals(_), None) => true,
            
            // In
            (Operator::In(list), Some(actual)) => {
                list.iter().any(|item| self.values_equal(item, actual))
            }
            (Operator::In(_), None) => false,
            
            // Not In
            (Operator::NotIn(list), Some(actual)) => {
                !list.iter().any(|item| self.values_equal(item, actual))
            }
            (Operator::NotIn(_), None) => true,
            
            // Contains (string)
            (Operator::Contains(substr), Some(Value::String(s))) => {
                s.to_lowercase().contains(&substr.to_lowercase())
            }
            (Operator::Contains(_), _) => false,
            
            // StartsWith
            (Operator::StartsWith(prefix), Some(Value::String(s))) => {
                s.to_lowercase().starts_with(&prefix.to_lowercase())
            }
            (Operator::StartsWith(_), _) => false,
            
            // EndsWith
            (Operator::EndsWith(suffix), Some(Value::String(s))) => {
                s.to_lowercase().ends_with(&suffix.to_lowercase())
            }
            (Operator::EndsWith(_), _) => false,
            
            // Numeric comparisons
            (Operator::Gt(threshold), Some(actual)) => {
                self.get_number(actual).map(|n| n > *threshold).unwrap_or(false)
            }
            (Operator::Gte(threshold), Some(actual)) => {
                self.get_number(actual).map(|n| n >= *threshold).unwrap_or(false)
            }
            (Operator::Lt(threshold), Some(actual)) => {
                self.get_number(actual).map(|n| n < *threshold).unwrap_or(false)
            }
            (Operator::Lte(threshold), Some(actual)) => {
                self.get_number(actual).map(|n| n <= *threshold).unwrap_or(false)
            }
            (Operator::Gt(_) | Operator::Gte(_) | Operator::Lt(_) | Operator::Lte(_), None) => false,
            
            // Null checks
            (Operator::IsNull(expected), v) => {
                (v.is_none() || v == Some(&Value::Null)) == *expected
            }
            (Operator::IsNotNull(expected), v) => {
                (v.is_some() && v != Some(&Value::Null)) == *expected
            }
            
            // Regex
            (Operator::Matches(pattern), Some(Value::String(s))) => {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(s))
                    .unwrap_or(false)
            }
            (Operator::Matches(_), _) => false,
        }
    }
    
    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::String(s1), Value::String(s2)) => s1.to_lowercase() == s2.to_lowercase(),
            (Value::Number(n1), Value::Number(n2)) => n1.as_f64() == n2.as_f64(),
            (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
            (Value::Null, Value::Null) => true,
            _ => a == b,
        }
    }
    
    fn get_number(&self, value: &Value) -> Option<f64> {
        match value {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse().ok(),
            _ => None,
        }
    }
    
    fn is_time_comparison(&self, operator: &Operator) -> bool {
        match operator {
            Operator::Lt(v) | Operator::Lte(v) | Operator::Gt(v) | Operator::Gte(v) => {
                // Check if the value looks like it might be a time expression result
                // This is a heuristic - time expressions are interpolated before evaluation
                false // Numbers are handled normally
            }
            _ => false,
        }
    }
    
    fn evaluate_time_comparison(&self, operator: &Operator, value: Option<&Value>, context: &RuleContext) -> bool {
        // For time comparisons, we compare ISO timestamps
        let actual_time = value
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        
        let Some(actual) = actual_time else { return false };
        
        // The threshold should already be interpolated as a timestamp
        match operator {
            Operator::Lt(threshold) => {
                // Treat threshold as epoch timestamp if numeric
                let threshold_time = DateTime::from_timestamp(*threshold as i64, 0);
                threshold_time.map(|t| actual < t).unwrap_or(false)
            }
            // ... similar for other operators
            _ => false,
        }
    }
}

impl Default for RuleEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_equals() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "KY");
        
        let condition = Condition::Leaf(LeafCondition {
            field: "entity.jurisdiction".to_string(),
            operator: Operator::Equals(json!("KY")),
        });
        
        assert!(evaluator.evaluate(&condition, &ctx));
    }
    
    #[test]
    fn test_in_operator() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "VG");
        
        let condition = Condition::Leaf(LeafCondition {
            field: "entity.jurisdiction".to_string(),
            operator: Operator::In(vec![json!("KY"), json!("VG"), json!("BVI")]),
        });
        
        assert!(evaluator.evaluate(&condition, &ctx));
    }
    
    #[test]
    fn test_all_condition() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.type", "trust");
        ctx.set("entity.trust_type", "DISCRETIONARY");
        
        let condition = Condition::All {
            all: vec![
                Condition::Leaf(LeafCondition {
                    field: "entity.type".to_string(),
                    operator: Operator::Equals(json!("trust")),
                }),
                Condition::Leaf(LeafCondition {
                    field: "entity.trust_type".to_string(),
                    operator: Operator::Equals(json!("DISCRETIONARY")),
                }),
            ],
        };
        
        assert!(evaluator.evaluate(&condition, &ctx));
    }
    
    #[test]
    fn test_gte_operator() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("holding.ownership_percentage", 30.0);
        
        let condition = Condition::Leaf(LeafCondition {
            field: "holding.ownership_percentage".to_string(),
            operator: Operator::Gte(25.0),
        });
        
        assert!(evaluator.evaluate(&condition, &ctx));
    }
    
    #[test]
    fn test_contains_case_insensitive() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.name", "ABC Nominee Services Ltd");
        
        let condition = Condition::Leaf(LeafCondition {
            field: "entity.name".to_string(),
            operator: Operator::Contains("nominee".to_string()),
        });
        
        assert!(evaluator.evaluate(&condition, &ctx));
    }
}
```

---

### File: `rust/src/kyc/rules/executor.rs`

```rust
//! Execute rule actions

use super::parser::Action;
use super::context::RuleContext;
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;
use serde_json::Value;

pub struct ActionExecutor {
    pool: PgPool,
}

impl ActionExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Execute a single action
    pub async fn execute(&self, action: &Action, context: &RuleContext, rule_name: &str) -> Result<()> {
        tracing::debug!(action = %action.action_type, rule = %rule_name, "Executing action");
        
        match action.action_type.as_str() {
            "raise-red-flag" => self.raise_red_flag(action, context, rule_name).await,
            "block-workstream" => self.block_workstream(action, context).await,
            "update-case-status" => self.update_case_status(action, context).await,
            "update-workstream-status" => self.update_workstream_status(action, context).await,
            "escalate-case" => self.escalate_case(action, context).await,
            "set-enhanced-dd" => self.set_enhanced_dd(context).await,
            "set-ubo" => self.set_ubo(context).await,
            "set-case-risk" => self.set_case_risk(action, context).await,
            "require-document" => self.require_document(action, context).await,
            "log-event" => self.log_event(action, context, rule_name).await,
            "auto-run-screenings" => self.auto_run_screenings(context).await,
            _ => {
                tracing::warn!(action = %action.action_type, "Unknown action type");
                Ok(())
            }
        }
    }
    
    async fn raise_red_flag(&self, action: &Action, context: &RuleContext, rule_name: &str) -> Result<()> {
        let flag_type = self.get_param_string(action, "flag-type", context)?;
        let severity = self.get_param_string(action, "severity", context)?;
        let description = self.get_param_string(action, "description", context)?;
        let source = action.params.get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("RULE_ENGINE");
        let source_reference = action.params.get("source-reference")
            .and_then(|v| v.as_str())
            .map(|s| context.interpolate(s))
            .unwrap_or_else(|| rule_name.to_string());
        
        let case_id = context.get_uuid("case.id")
            .ok_or_else(|| anyhow::anyhow!("Missing case.id in context"))?;
        let workstream_id = context.get_uuid("workstream.id");
        
        sqlx::query!(
            r#"
            INSERT INTO kyc.red_flags (case_id, workstream_id, flag_type, severity, description, source, source_reference)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            case_id,
            workstream_id,
            flag_type,
            severity,
            description,
            source,
            source_reference
        )
        .execute(&self.pool)
        .await?;
        
        tracing::info!(flag_type = %flag_type, severity = %severity, "Raised red flag");
        
        Ok(())
    }
    
    async fn block_workstream(&self, action: &Action, context: &RuleContext) -> Result<()> {
        let reason = self.get_param_string(action, "reason", context)?;
        let workstream_id = context.get_uuid("workstream.id")
            .ok_or_else(|| anyhow::anyhow!("Missing workstream.id in context"))?;
        
        sqlx::query!(
            r#"
            UPDATE kyc.entity_workstreams 
            SET status = 'BLOCKED', blocked_at = now(), blocked_reason = $1 
            WHERE workstream_id = $2
            "#,
            reason,
            workstream_id
        )
        .execute(&self.pool)
        .await?;
        
        tracing::info!(workstream_id = %workstream_id, "Blocked workstream");
        
        Ok(())
    }
    
    async fn update_case_status(&self, action: &Action, context: &RuleContext) -> Result<()> {
        let status = self.get_param_string(action, "status", context)?;
        let case_id = context.get_uuid("case.id")
            .ok_or_else(|| anyhow::anyhow!("Missing case.id in context"))?;
        
        sqlx::query!(
            "UPDATE kyc.cases SET status = $1, last_activity_at = now() WHERE case_id = $2",
            status,
            case_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn update_workstream_status(&self, action: &Action, context: &RuleContext) -> Result<()> {
        let status = self.get_param_string(action, "status", context)?;
        let workstream_id = context.get_uuid("workstream.id")
            .ok_or_else(|| anyhow::anyhow!("Missing workstream.id in context"))?;
        
        sqlx::query!(
            "UPDATE kyc.entity_workstreams SET status = $1 WHERE workstream_id = $2",
            status,
            workstream_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn escalate_case(&self, action: &Action, context: &RuleContext) -> Result<()> {
        let level = self.get_param_string(action, "level", context)?;
        let case_id = context.get_uuid("case.id")
            .ok_or_else(|| anyhow::anyhow!("Missing case.id in context"))?;
        
        // Only escalate if current level is lower
        sqlx::query!(
            r#"
            UPDATE kyc.cases 
            SET escalation_level = $1, last_activity_at = now()
            WHERE case_id = $2
            AND CASE escalation_level
                WHEN 'STANDARD' THEN 1
                WHEN 'TEAM_LEAD' THEN 2
                WHEN 'SENIOR_COMPLIANCE' THEN 3
                WHEN 'EXECUTIVE' THEN 4
                WHEN 'BOARD' THEN 5
                ELSE 0
            END < CASE $1
                WHEN 'STANDARD' THEN 1
                WHEN 'TEAM_LEAD' THEN 2
                WHEN 'SENIOR_COMPLIANCE' THEN 3
                WHEN 'EXECUTIVE' THEN 4
                WHEN 'BOARD' THEN 5
                ELSE 0
            END
            "#,
            level,
            case_id
        )
        .execute(&self.pool)
        .await?;
        
        tracing::info!(case_id = %case_id, level = %level, "Escalated case");
        
        Ok(())
    }
    
    async fn set_enhanced_dd(&self, context: &RuleContext) -> Result<()> {
        let workstream_id = context.get_uuid("workstream.id")
            .ok_or_else(|| anyhow::anyhow!("Missing workstream.id in context"))?;
        
        sqlx::query!(
            r#"
            UPDATE kyc.entity_workstreams 
            SET requires_enhanced_dd = true, status = 'ENHANCED_DD' 
            WHERE workstream_id = $1
            "#,
            workstream_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn set_ubo(&self, context: &RuleContext) -> Result<()> {
        let workstream_id = context.get_uuid("workstream.id")
            .ok_or_else(|| anyhow::anyhow!("Missing workstream.id in context"))?;
        
        sqlx::query!(
            "UPDATE kyc.entity_workstreams SET is_ubo = true WHERE workstream_id = $1",
            workstream_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn set_case_risk(&self, action: &Action, context: &RuleContext) -> Result<()> {
        let rating = self.get_param_string(action, "rating", context)?;
        let case_id = context.get_uuid("case.id")
            .ok_or_else(|| anyhow::anyhow!("Missing case.id in context"))?;
        
        sqlx::query!(
            "UPDATE kyc.cases SET risk_rating = $1, last_activity_at = now() WHERE case_id = $2",
            rating,
            case_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn require_document(&self, action: &Action, context: &RuleContext) -> Result<()> {
        let doc_type = self.get_param_string(action, "doc-type", context)?;
        let priority = action.params.get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("NORMAL");
        
        let workstream_id = context.get_uuid("workstream.id")
            .ok_or_else(|| anyhow::anyhow!("Missing workstream.id in context"))?;
        
        // Use ON CONFLICT to avoid duplicates
        sqlx::query!(
            r#"
            INSERT INTO kyc.doc_requests (workstream_id, doc_type, priority)
            VALUES ($1, $2, $3)
            ON CONFLICT (workstream_id, doc_type) DO UPDATE SET priority = 
                CASE WHEN EXCLUDED.priority = 'URGENT' THEN 'URGENT'
                     WHEN EXCLUDED.priority = 'HIGH' AND kyc.doc_requests.priority NOT IN ('URGENT') THEN 'HIGH'
                     ELSE kyc.doc_requests.priority
                END
            "#,
            workstream_id,
            doc_type,
            priority
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn log_event(&self, action: &Action, context: &RuleContext, rule_name: &str) -> Result<()> {
        let event_type = self.get_param_string(action, "event-type", context)?;
        let comment = action.params.get("comment")
            .and_then(|v| v.as_str())
            .map(|s| context.interpolate(s));
        
        let case_id = context.get_uuid("case.id")
            .ok_or_else(|| anyhow::anyhow!("Missing case.id in context"))?;
        let workstream_id = context.get_uuid("workstream.id");
        
        sqlx::query!(
            r#"
            INSERT INTO kyc.case_events (case_id, workstream_id, event_type, actor_type, comment, event_data)
            VALUES ($1, $2, $3, 'RULE_ENGINE', $4, $5)
            "#,
            case_id,
            workstream_id,
            event_type,
            comment,
            serde_json::json!({"rule": rule_name})
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn auto_run_screenings(&self, context: &RuleContext) -> Result<()> {
        let workstream_id = context.get_uuid("workstream.id")
            .ok_or_else(|| anyhow::anyhow!("Missing workstream.id in context"))?;
        
        // Create screening requests for standard types
        for screening_type in ["SANCTIONS", "PEP", "ADVERSE_MEDIA"] {
            sqlx::query!(
                r#"
                INSERT INTO kyc.screenings (workstream_id, screening_type, status)
                VALUES ($1, $2, 'PENDING')
                ON CONFLICT DO NOTHING
                "#,
                workstream_id,
                screening_type
            )
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
    
    // Helper to get interpolated string param
    fn get_param_string(&self, action: &Action, key: &str, context: &RuleContext) -> Result<String> {
        let value = action.params.get(key)
            .ok_or_else(|| anyhow::anyhow!("Missing param: {}", key))?
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Param {} must be string", key))?;
        
        Ok(context.interpolate(value))
    }
}
```

---

### File: `rust/src/kyc/rules/event_bus.rs`

```rust
//! Event bus for routing KYC events to rules engine

use super::{Rule, RuleEvaluator, ActionExecutor, RuleContext};
use super::context::*;
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;
use std::sync::Arc;

/// KYC events that can trigger rules
#[derive(Debug, Clone)]
pub enum KycEvent {
    // Workstream events
    WorkstreamCreated { case_id: Uuid, workstream_id: Uuid, entity_id: Uuid },
    WorkstreamStatusChanged { case_id: Uuid, workstream_id: Uuid, old_status: String, new_status: String },
    WorkstreamBlocked { case_id: Uuid, workstream_id: Uuid },
    WorkstreamCompleted { case_id: Uuid, workstream_id: Uuid },
    
    // Screening events
    ScreeningStarted { case_id: Uuid, workstream_id: Uuid, screening_id: Uuid },
    ScreeningCompleted { case_id: Uuid, workstream_id: Uuid, screening_id: Uuid },
    ScreeningReviewed { case_id: Uuid, workstream_id: Uuid, screening_id: Uuid },
    
    // Document events
    DocRequestCreated { case_id: Uuid, workstream_id: Uuid, request_id: Uuid },
    DocRequestReceived { case_id: Uuid, workstream_id: Uuid, request_id: Uuid },
    DocRequestVerified { case_id: Uuid, workstream_id: Uuid, request_id: Uuid },
    DocRequestRejected { case_id: Uuid, workstream_id: Uuid, request_id: Uuid },
    
    // Red flag events
    RedFlagRaised { case_id: Uuid, red_flag_id: Uuid },
    RedFlagMitigated { case_id: Uuid, red_flag_id: Uuid },
    RedFlagWaived { case_id: Uuid, red_flag_id: Uuid },
    
    // Holding events
    HoldingCreated { case_id: Uuid, workstream_id: Uuid, holding_id: Uuid },
    
    // Case events
    CaseCreated { case_id: Uuid },
    CaseStatusChanged { case_id: Uuid, old_status: String, new_status: String },
    CaseEscalated { case_id: Uuid, level: String },
}

impl KycEvent {
    /// Get the event name for rule matching
    pub fn event_name(&self) -> &'static str {
        match self {
            KycEvent::WorkstreamCreated { .. } => "workstream.created",
            KycEvent::WorkstreamStatusChanged { .. } => "workstream.status-changed",
            KycEvent::WorkstreamBlocked { .. } => "workstream.blocked",
            KycEvent::WorkstreamCompleted { .. } => "workstream.completed",
            KycEvent::ScreeningStarted { .. } => "screening.started",
            KycEvent::ScreeningCompleted { .. } => "screening.completed",
            KycEvent::ScreeningReviewed { .. } => "screening.reviewed",
            KycEvent::DocRequestCreated { .. } => "doc-request.created",
            KycEvent::DocRequestReceived { .. } => "doc-request.received",
            KycEvent::DocRequestVerified { .. } => "doc-request.verified",
            KycEvent::DocRequestRejected { .. } => "doc-request.rejected",
            KycEvent::RedFlagRaised { .. } => "red-flag.raised",
            KycEvent::RedFlagMitigated { .. } => "red-flag.mitigated",
            KycEvent::RedFlagWaived { .. } => "red-flag.waived",
            KycEvent::HoldingCreated { .. } => "holding.created",
            KycEvent::CaseCreated { .. } => "case.created",
            KycEvent::CaseStatusChanged { .. } => "case.status-changed",
            KycEvent::CaseEscalated { .. } => "case.escalated",
        }
    }
    
    pub fn case_id(&self) -> Uuid {
        match self {
            KycEvent::WorkstreamCreated { case_id, .. } => *case_id,
            KycEvent::WorkstreamStatusChanged { case_id, .. } => *case_id,
            KycEvent::WorkstreamBlocked { case_id, .. } => *case_id,
            KycEvent::WorkstreamCompleted { case_id, .. } => *case_id,
            KycEvent::ScreeningStarted { case_id, .. } => *case_id,
            KycEvent::ScreeningCompleted { case_id, .. } => *case_id,
            KycEvent::ScreeningReviewed { case_id, .. } => *case_id,
            KycEvent::DocRequestCreated { case_id, .. } => *case_id,
            KycEvent::DocRequestReceived { case_id, .. } => *case_id,
            KycEvent::DocRequestVerified { case_id, .. } => *case_id,
            KycEvent::DocRequestRejected { case_id, .. } => *case_id,
            KycEvent::RedFlagRaised { case_id, .. } => *case_id,
            KycEvent::RedFlagMitigated { case_id, .. } => *case_id,
            KycEvent::RedFlagWaived { case_id, .. } => *case_id,
            KycEvent::HoldingCreated { case_id, .. } => *case_id,
            KycEvent::CaseCreated { case_id } => *case_id,
            KycEvent::CaseStatusChanged { case_id, .. } => *case_id,
            KycEvent::CaseEscalated { case_id, .. } => *case_id,
        }
    }
}

/// Event bus that routes events to the rules engine
pub struct KycEventBus {
    pool: PgPool,
    rules: Vec<Rule>,
    evaluator: RuleEvaluator,
}

impl KycEventBus {
    pub fn new(pool: PgPool, rules: Vec<Rule>) -> Self {
        Self {
            pool,
            rules,
            evaluator: RuleEvaluator::new(),
        }
    }
    
    /// Publish an event and evaluate matching rules
    pub async fn publish(&self, event: KycEvent) -> Result<()> {
        let event_name = event.event_name();
        tracing::debug!(event = %event_name, case_id = %event.case_id(), "Processing event");
        
        // Build context for this event
        let context = self.build_context(&event).await?;
        
        // Find matching rules
        let matching_rules: Vec<&Rule> = self.rules
            .iter()
            .filter(|r| r.trigger.matches_event(event_name))
            .collect();
        
        if matching_rules.is_empty() {
            return Ok(());
        }
        
        tracing::debug!(count = matching_rules.len(), "Found matching rules");
        
        // Sort by priority
        let mut sorted_rules = matching_rules;
        sorted_rules.sort_by_key(|r| r.priority);
        
        // Evaluate and execute
        let executor = ActionExecutor::new(self.pool.clone());
        
        for rule in sorted_rules {
            let matched = self.evaluator.evaluate(&rule.condition, &context);
            
            // Log rule execution
            self.log_rule_execution(&event, rule, matched, &context).await?;
            
            if matched {
                tracing::info!(rule = %rule.name, "Rule matched, executing actions");
                
                for action in &rule.actions {
                    if let Err(e) = executor.execute(action, &context, &rule.name).await {
                        tracing::error!(rule = %rule.name, action = %action.action_type, error = %e, "Action failed");
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn build_context(&self, event: &KycEvent) -> Result<RuleContext> {
        match event {
            KycEvent::WorkstreamCreated { case_id, workstream_id, entity_id } => {
                build_workstream_created_context(&self.pool, *case_id, *workstream_id, *entity_id).await
            }
            KycEvent::ScreeningCompleted { case_id, workstream_id, screening_id } => {
                build_screening_completed_context(&self.pool, *case_id, *workstream_id, *screening_id).await
            }
            KycEvent::HoldingCreated { case_id, workstream_id, holding_id } => {
                build_holding_created_context(&self.pool, *case_id, *workstream_id, *holding_id).await
            }
            KycEvent::RedFlagRaised { case_id, red_flag_id } => {
                build_red_flag_raised_context(&self.pool, *case_id, *red_flag_id).await
            }
            KycEvent::DocRequestReceived { case_id, workstream_id, request_id } |
            KycEvent::DocRequestVerified { case_id, workstream_id, request_id } => {
                build_doc_request_context(&self.pool, *case_id, *workstream_id, *request_id).await
            }
            // For other events, build a basic context
            _ => {
                let mut ctx = RuleContext::new();
                ctx.set("case.id", event.case_id().to_string());
                Ok(ctx)
            }
        }
    }
    
    async fn log_rule_execution(&self, event: &KycEvent, rule: &Rule, matched: bool, context: &RuleContext) -> Result<()> {
        let case_id = event.case_id();
        let workstream_id = context.get_uuid("workstream.id");
        
        sqlx::query!(
            r#"
            INSERT INTO kyc.rule_executions (case_id, workstream_id, rule_name, trigger_event, condition_matched, context_snapshot)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            case_id,
            workstream_id,
            rule.name,
            event.event_name(),
            matched,
            context.snapshot()
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

---

### File: `rust/src/kyc/rules/scheduler.rs`

```rust
//! Scheduler for temporal rules (SLA checks, overdue documents, etc.)

use super::{Rule, RuleEvaluator, ActionExecutor, RuleContext};
use sqlx::PgPool;
use anyhow::Result;
use chrono::Utc;

pub struct RuleScheduler {
    pool: PgPool,
    rules: Vec<Rule>,
    evaluator: RuleEvaluator,
}

impl RuleScheduler {
    pub fn new(pool: PgPool, rules: Vec<Rule>) -> Self {
        // Filter to scheduled rules only
        let scheduled_rules: Vec<Rule> = rules
            .into_iter()
            .filter(|r| r.trigger.is_scheduled())
            .collect();
        
        Self {
            pool,
            rules: scheduled_rules,
            evaluator: RuleEvaluator::new(),
        }
    }
    
    /// Run all daily scheduled rules
    pub async fn run_daily(&self) -> Result<()> {
        tracing::info!("Running daily scheduled rules");
        
        let daily_rules: Vec<&Rule> = self.rules
            .iter()
            .filter(|r| r.trigger.schedule.as_deref() == Some("daily"))
            .collect();
        
        for rule in daily_rules {
            if let Err(e) = self.run_rule(rule).await {
                tracing::error!(rule = %rule.name, error = %e, "Scheduled rule failed");
            }
        }
        
        Ok(())
    }
    
    /// Run all hourly scheduled rules
    pub async fn run_hourly(&self) -> Result<()> {
        tracing::info!("Running hourly scheduled rules");
        
        let hourly_rules: Vec<&Rule> = self.rules
            .iter()
            .filter(|r| r.trigger.schedule.as_deref() == Some("hourly"))
            .collect();
        
        for rule in hourly_rules {
            if let Err(e) = self.run_rule(rule).await {
                tracing::error!(rule = %rule.name, error = %e, "Scheduled rule failed");
            }
        }
        
        Ok(())
    }
    
    async fn run_rule(&self, rule: &Rule) -> Result<()> {
        tracing::debug!(rule = %rule.name, "Running scheduled rule");
        
        let executor = ActionExecutor::new(self.pool.clone());
        
        match rule.name.as_str() {
            "document-overdue" => {
                self.check_overdue_documents(rule, &executor).await?;
            }
            "case-sla-warning" => {
                self.check_sla_warning(rule, &executor).await?;
            }
            "case-sla-breach" => {
                self.check_sla_breach(rule, &executor).await?;
            }
            _ => {
                tracing::warn!(rule = %rule.name, "Unknown scheduled rule");
            }
        }
        
        Ok(())
    }
    
    async fn check_overdue_documents(&self, rule: &Rule, executor: &ActionExecutor) -> Result<()> {
        // Find overdue document requests
        let overdue_docs = sqlx::query!(
            r#"
            SELECT 
                dr.request_id,
                dr.workstream_id,
                dr.doc_type,
                dr.due_date,
                ew.case_id
            FROM kyc.doc_requests dr
            JOIN kyc.entity_workstreams ew ON ew.workstream_id = dr.workstream_id
            WHERE dr.status IN ('REQUIRED', 'REQUESTED')
            AND dr.due_date < CURRENT_DATE
            AND NOT EXISTS (
                SELECT 1 FROM kyc.red_flags rf 
                WHERE rf.workstream_id = dr.workstream_id 
                AND rf.flag_type = 'DOCUMENT_OVERDUE'
                AND rf.source_reference = dr.request_id::text
                AND rf.status NOT IN ('CLOSED', 'MITIGATED')
            )
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        for doc in overdue_docs {
            let mut context = RuleContext::new();
            context.set("case.id", doc.case_id.to_string());
            context.set("workstream.id", doc.workstream_id.to_string());
            context.set("doc_request.id", doc.request_id.to_string());
            context.set("doc_request.doc_type", doc.doc_type);
            if let Some(due) = doc.due_date {
                context.set("doc_request.due_date", due.to_string());
            }
            
            // Execute actions
            for action in &rule.actions {
                executor.execute(action, &context, &rule.name).await?;
            }
        }
        
        Ok(())
    }
    
    async fn check_sla_warning(&self, rule: &Rule, executor: &ActionExecutor) -> Result<()> {
        // Find cases approaching SLA
        let warning_cases = sqlx::query!(
            r#"
            SELECT 
                case_id,
                sla_deadline,
                status,
                escalation_level
            FROM kyc.cases
            WHERE status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'EXPIRED')
            AND sla_deadline IS NOT NULL
            AND sla_deadline < CURRENT_TIMESTAMP + INTERVAL '3 days'
            AND sla_deadline >= CURRENT_TIMESTAMP
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        for case in warning_cases {
            let mut context = RuleContext::new();
            context.set("case.id", case.case_id.to_string());
            context.set("case.status", case.status);
            context.set("case.escalation_level", case.escalation_level);
            if let Some(sla) = case.sla_deadline {
                context.set("case.sla_deadline", sla.to_rfc3339());
            }
            
            for action in &rule.actions {
                executor.execute(action, &context, &rule.name).await?;
            }
        }
        
        Ok(())
    }
    
    async fn check_sla_breach(&self, rule: &Rule, executor: &ActionExecutor) -> Result<()> {
        // Find cases that have breached SLA
        let breached_cases = sqlx::query!(
            r#"
            SELECT 
                case_id,
                sla_deadline,
                status,
                escalation_level
            FROM kyc.cases
            WHERE status NOT IN ('APPROVED', 'REJECTED', 'WITHDRAWN', 'EXPIRED')
            AND sla_deadline IS NOT NULL
            AND sla_deadline < CURRENT_TIMESTAMP
            AND NOT EXISTS (
                SELECT 1 FROM kyc.red_flags rf 
                WHERE rf.case_id = cases.case_id 
                AND rf.flag_type = 'SLA_BREACH'
                AND rf.status NOT IN ('CLOSED', 'MITIGATED')
            )
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        for case in breached_cases {
            let mut context = RuleContext::new();
            context.set("case.id", case.case_id.to_string());
            context.set("case.status", case.status);
            context.set("case.escalation_level", case.escalation_level);
            if let Some(sla) = case.sla_deadline {
                context.set("case.sla_deadline", sla.to_rfc3339());
            }
            
            for action in &rule.actions {
                executor.execute(action, &context, &rule.name).await?;
            }
        }
        
        Ok(())
    }
}
```

---

### File: `rust/src/kyc/mod.rs`

```rust
//! KYC case management module

pub mod rules;

pub use rules::{
    Rule, RulesConfig, load_rules,
    RuleEvaluator, ActionExecutor, RuleContext,
    KycEvent, KycEventBus, RuleScheduler,
};
```

---

## Part 4: Integration with DSL Executor

Update `rust/src/dsl/generic_executor.rs` to emit events after successful operations.

Add near the end of the execute function, after successful database operation:

```rust
// After successful operation, emit event for rules engine
if let Some(event_bus) = &self.event_bus {
    let event = self.build_event(&domain, &verb, &result)?;
    if let Some(evt) = event {
        event_bus.publish(evt).await?;
    }
}
```

Add helper method:

```rust
fn build_event(&self, domain: &str, verb: &str, result: &ExecutionResult) -> Result<Option<KycEvent>> {
    // Map domain.verb to KycEvent
    let event = match (domain, verb) {
        ("entity-workstream", "create") => {
            let case_id = result.get_uuid("case_id")?;
            let workstream_id = result.get_uuid("workstream_id")?;
            let entity_id = result.get_uuid("entity_id")?;
            Some(KycEvent::WorkstreamCreated { case_id, workstream_id, entity_id })
        }
        ("screening", "complete") => {
            // Need to look up case_id from workstream
            let screening_id = result.get_uuid("screening_id")?;
            let workstream_id = result.get_uuid("workstream_id")?;
            let case_id = self.lookup_case_id_from_workstream(workstream_id).await?;
            Some(KycEvent::ScreeningCompleted { case_id, workstream_id, screening_id })
        }
        ("red-flag", "raise") => {
            let case_id = result.get_uuid("case_id")?;
            let red_flag_id = result.get_uuid("red_flag_id")?;
            Some(KycEvent::RedFlagRaised { case_id, red_flag_id })
        }
        ("holding", "create") => {
            let holding_id = result.get_uuid("holding_id")?;
            // Need to look up case_id and workstream_id
            // This depends on how holdings are linked to workstreams
            None // TODO: implement when holding model is clearer
        }
        ("doc-request", "receive") => {
            let request_id = result.get_uuid("request_id")?;
            let workstream_id = result.get_uuid("workstream_id")?;
            let case_id = self.lookup_case_id_from_workstream(workstream_id).await?;
            Some(KycEvent::DocRequestReceived { case_id, workstream_id, request_id })
        }
        ("doc-request", "verify") => {
            let request_id = result.get_uuid("request_id")?;
            let workstream_id = result.get_uuid("workstream_id")?;
            let case_id = self.lookup_case_id_from_workstream(workstream_id).await?;
            Some(KycEvent::DocRequestVerified { case_id, workstream_id, request_id })
        }
        _ => None,
    };
    
    Ok(event)
}
```

---

## Part 5: Case Visualization

Create `rust/src/visualization/case_builder.rs`:

```rust
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;

/// Builds a visualization tree for a KYC case showing workstream graph
pub struct CaseTreeBuilder {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseVisualization {
    pub case_id: Uuid,
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub status: String,
    pub escalation_level: String,
    pub risk_rating: Option<String>,
    pub workstream_tree: Vec<WorkstreamNode>,
    pub case_red_flags: Vec<RedFlagInfo>,
    pub stats: CaseStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkstreamNode {
    pub workstream_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub status: String,
    pub risk_rating: Option<String>,
    pub is_ubo: bool,
    pub ownership_percentage: Option<f64>,
    pub requires_enhanced_dd: bool,
    pub discovery_reason: Option<String>,
    pub discovery_depth: i32,
    pub red_flags: Vec<RedFlagInfo>,
    pub doc_stats: DocStats,
    pub screening_stats: ScreeningStats,
    pub children: Vec<WorkstreamNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedFlagInfo {
    pub red_flag_id: Uuid,
    pub flag_type: String,
    pub severity: String,
    pub status: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DocStats {
    pub pending: i64,
    pub received: i64,
    pub verified: i64,
    pub rejected: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ScreeningStats {
    pub clear: i64,
    pub pending_review: i64,
    pub confirmed_hits: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseStats {
    pub total_workstreams: usize,
    pub completed_workstreams: usize,
    pub blocked_workstreams: usize,
    pub open_red_flags: usize,
    pub hard_stops: usize,
}

impl CaseTreeBuilder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    pub async fn build(&self, case_id: Uuid) -> Result<CaseVisualization> {
        // Load case info
        let case_info = sqlx::query!(
            r#"
            SELECT 
                c.case_id,
                c.cbu_id,
                cb.name as cbu_name,
                c.status,
                c.escalation_level,
                c.risk_rating
            FROM kyc.cases c
            JOIN "ob-poc".cbus cb ON cb.cbu_id = c.cbu_id
            WHERE c.case_id = $1
            "#,
            case_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        // Load all workstreams
        let workstreams = sqlx::query!(
            r#"
            SELECT 
                w.workstream_id,
                w.entity_id,
                e.name as entity_name,
                et.type_code as entity_type,
                COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction,
                w.status,
                w.risk_rating,
                w.is_ubo,
                w.ownership_percentage,
                w.requires_enhanced_dd,
                w.discovery_reason,
                w.discovery_depth,
                w.discovery_source_workstream_id
            FROM kyc.entity_workstreams w
            JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
            JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_partnerships p ON p.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_trusts t ON t.entity_id = e.entity_id
            WHERE w.case_id = $1
            ORDER BY w.discovery_depth, w.created_at
            "#,
            case_id
        )
        .fetch_all(&self.pool)
        .await?;
        
        // Load all red flags
        let red_flags = sqlx::query_as!(
            RedFlagInfo,
            r#"
            SELECT 
                red_flag_id,
                flag_type,
                severity,
                status,
                description
            FROM kyc.red_flags
            WHERE case_id = $1
            ORDER BY raised_at DESC
            "#,
            case_id
        )
        .fetch_all(&self.pool)
        .await?;
        
        // Build workstream nodes
        let mut nodes: HashMap<Uuid, WorkstreamNode> = HashMap::new();
        let mut root_ids: Vec<Uuid> = Vec::new();
        
        for ws in &workstreams {
            // Get doc stats
            let doc_stats = sqlx::query!(
                r#"
                SELECT 
                    COUNT(*) FILTER (WHERE status IN ('REQUIRED', 'REQUESTED')) as pending,
                    COUNT(*) FILTER (WHERE status IN ('RECEIVED', 'UNDER_REVIEW')) as received,
                    COUNT(*) FILTER (WHERE status = 'VERIFIED') as verified,
                    COUNT(*) FILTER (WHERE status = 'REJECTED') as rejected
                FROM kyc.doc_requests
                WHERE workstream_id = $1
                "#,
                ws.workstream_id
            )
            .fetch_one(&self.pool)
            .await?;
            
            // Get screening stats
            let screening_stats = sqlx::query!(
                r#"
                SELECT 
                    COUNT(*) FILTER (WHERE status = 'CLEAR') as clear,
                    COUNT(*) FILTER (WHERE status = 'HIT_PENDING_REVIEW') as pending_review,
                    COUNT(*) FILTER (WHERE status = 'HIT_CONFIRMED') as confirmed_hits
                FROM kyc.screenings
                WHERE workstream_id = $1
                "#,
                ws.workstream_id
            )
            .fetch_one(&self.pool)
            .await?;
            
            // Get red flags for this workstream
            let ws_flags: Vec<RedFlagInfo> = red_flags
                .iter()
                .filter(|rf| {
                    // Check if this flag belongs to this workstream
                    // Would need workstream_id in RedFlagInfo for this
                    false // Simplified - would need join
                })
                .cloned()
                .collect();
            
            let node = WorkstreamNode {
                workstream_id: ws.workstream_id,
                entity_id: ws.entity_id,
                entity_name: ws.entity_name.clone(),
                entity_type: ws.entity_type.clone(),
                jurisdiction: ws.jurisdiction.clone(),
                status: ws.status.clone(),
                risk_rating: ws.risk_rating.clone(),
                is_ubo: ws.is_ubo,
                ownership_percentage: ws.ownership_percentage.map(|d| d.to_string().parse().unwrap_or(0.0)),
                requires_enhanced_dd: ws.requires_enhanced_dd,
                discovery_reason: ws.discovery_reason.clone(),
                discovery_depth: ws.discovery_depth.unwrap_or(1),
                red_flags: ws_flags,
                doc_stats: DocStats {
                    pending: doc_stats.pending.unwrap_or(0),
                    received: doc_stats.received.unwrap_or(0),
                    verified: doc_stats.verified.unwrap_or(0),
                    rejected: doc_stats.rejected.unwrap_or(0),
                },
                screening_stats: ScreeningStats {
                    clear: screening_stats.clear.unwrap_or(0),
                    pending_review: screening_stats.pending_review.unwrap_or(0),
                    confirmed_hits: screening_stats.confirmed_hits.unwrap_or(0),
                },
                children: vec![],
            };
            
            nodes.insert(ws.workstream_id, node);
            
            if ws.discovery_source_workstream_id.is_none() {
                root_ids.push(ws.workstream_id);
            }
        }
        
        // Build tree by linking children to parents
        for ws in &workstreams {
            if let Some(parent_id) = ws.discovery_source_workstream_id {
                if let Some(child_node) = nodes.get(&ws.workstream_id).cloned() {
                    if let Some(parent_node) = nodes.get_mut(&parent_id) {
                        parent_node.children.push(child_node);
                    }
                }
            }
        }
        
        // Get root nodes
        let workstream_tree: Vec<WorkstreamNode> = root_ids
            .iter()
            .filter_map(|id| nodes.get(id).cloned())
            .collect();
        
        // Case-level red flags (not tied to workstream)
        let case_red_flags: Vec<RedFlagInfo> = red_flags.clone(); // Simplified
        
        // Stats
        let stats = CaseStats {
            total_workstreams: workstreams.len(),
            completed_workstreams: workstreams.iter().filter(|w| w.status == "COMPLETE").count(),
            blocked_workstreams: workstreams.iter().filter(|w| w.status == "BLOCKED").count(),
            open_red_flags: red_flags.iter().filter(|r| r.status == "OPEN" || r.status == "BLOCKING").count(),
            hard_stops: red_flags.iter().filter(|r| r.severity == "HARD_STOP" && r.status != "MITIGATED" && r.status != "WAIVED" && r.status != "CLOSED").count(),
        };
        
        Ok(CaseVisualization {
            case_id,
            cbu_id: case_info.cbu_id,
            cbu_name: case_info.cbu_name,
            status: case_info.status,
            escalation_level: case_info.escalation_level,
            risk_rating: case_info.risk_rating,
            workstream_tree,
            case_red_flags,
            stats,
        })
    }
}
```

---

## Part 6: Startup and Initialization

Add to your server startup:

```rust
// Load rules
let rules_path = Path::new("rust/config/rules.yaml");
let rules = kyc::load_rules(rules_path)?;
tracing::info!("Loaded {} KYC rules", rules.len());

// Create event bus
let event_bus = Arc::new(KycEventBus::new(pool.clone(), rules.clone()));

// Create scheduler
let scheduler = Arc::new(RuleScheduler::new(pool.clone(), rules));

// Start daily scheduler (run once per day)
let scheduler_clone = scheduler.clone();
tokio::spawn(async move {
    loop {
        // Wait until midnight or next scheduled time
        tokio::time::sleep(tokio::time::Duration::from_secs(86400)).await;
        if let Err(e) = scheduler_clone.run_daily().await {
            tracing::error!(error = %e, "Daily rule scheduler failed");
        }
    }
});

// Pass event_bus to executor
let executor = GenericExecutor::new(pool.clone(), Some(event_bus));
```

---

## Summary

| Component | File | Purpose |
|-----------|------|---------|
| Schema | Part 1 SQL | 9 tables including rule_executions |
| DSL Verbs | Part 2 YAML | 6 domains, 25+ verbs with emits_event |
| Rules YAML | rules.yaml | All rules in DSL format |
| Parser | parser.rs | YAML → Rule structs |
| Context | context.rs | Build evaluation context from DB |
| Evaluator | evaluator.rs | Evaluate conditions (AND/OR/NOT) |
| Executor | executor.rs | Execute rule actions |
| Event Bus | event_bus.rs | Route events to rules |
| Scheduler | scheduler.rs | Run temporal rules |
| Integration | generic_executor.rs | Emit events after DSL ops |
| Visualization | case_builder.rs | Case workstream tree |

---

## Verification Steps

1. **Run migration**
2. **Load rules**: `cargo test --package ob-poc -- kyc::rules::parser`
3. **Create test case via DSL**
4. **Create workstream → rules should fire automatically**
5. **Check kyc.rule_executions table for audit trail**
6. **Check kyc.red_flags for auto-generated flags**

---

*No sleep till it compiles.* 🔥
