-- =============================================================================
-- Migration 015: Research Workflows
-- =============================================================================
-- Implements the "Bounded Non-Determinism" pattern for research workflows:
-- - Phase 1 (LLM exploration) decisions are audited in research_decisions
-- - Phase 2 (DSL execution) actions are audited in research_actions
-- - Corrections track when wrong identifiers were selected
-- - Anomalies flag post-import validation issues
-- =============================================================================

-- =============================================================================
-- RESEARCH DECISIONS (Phase 1 Audit Trail)
-- =============================================================================
-- Captures the non-deterministic search and selection process.
-- Every time an LLM picks an identifier (LEI, company number, etc.), log it here.

CREATE TABLE IF NOT EXISTS kyc.research_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What triggered this research
    trigger_id UUID,  -- Optional link to ownership_research_triggers if exists
    target_entity_id UUID REFERENCES "ob-poc".entities(entity_id),

    -- Search context
    search_query TEXT NOT NULL,
    search_context JSONB,  -- {jurisdiction, entity_type, parent_context, etc.}

    -- Source used
    source_provider VARCHAR(30) NOT NULL,  -- gleif, companies_house, sec, orbis, etc.

    -- Candidates found
    candidates_found JSONB NOT NULL DEFAULT '[]',  -- [{key, name, score, metadata}]
    candidates_count INTEGER NOT NULL DEFAULT 0,

    -- Selection
    selected_key VARCHAR(100),  -- LEI, company number, CIK, BvD ID, etc.
    selected_key_type VARCHAR(20),  -- LEI, COMPANY_NUMBER, CIK, BVD_ID
    selection_confidence DECIMAL(3,2),  -- 0.00 - 1.00
    selection_reasoning TEXT NOT NULL,

    -- Decision type
    decision_type VARCHAR(20) NOT NULL,

    -- Verification
    auto_selected BOOLEAN NOT NULL DEFAULT false,
    verified_by UUID,  -- User who confirmed (if not auto)
    verified_at TIMESTAMPTZ,

    -- Link to resulting action
    resulting_action_id UUID,  -- Points to research_actions if executed

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    session_id UUID,

    CONSTRAINT chk_decision_type CHECK (
        decision_type IN (
            'AUTO_SELECTED',     -- High confidence, proceeded automatically
            'USER_SELECTED',     -- User picked from candidates
            'USER_CONFIRMED',    -- Auto-selected but user confirmed
            'NO_MATCH',          -- No suitable candidates found
            'AMBIGUOUS',         -- Multiple candidates, awaiting user input
            'REJECTED'           -- User rejected suggested match
        )
    ),
    CONSTRAINT chk_source_provider CHECK (
        source_provider IN (
            'gleif', 'companies_house', 'sec', 'orbis',
            'open_corporates', 'screening', 'manual', 'document'
        )
    )
);

CREATE INDEX idx_research_decisions_target ON kyc.research_decisions(target_entity_id);
CREATE INDEX idx_research_decisions_type ON kyc.research_decisions(decision_type);
CREATE INDEX idx_research_decisions_source ON kyc.research_decisions(source_provider);
CREATE INDEX idx_research_decisions_created ON kyc.research_decisions(created_at);

COMMENT ON TABLE kyc.research_decisions IS
'Audit trail for Phase 1 (LLM exploration) decisions. Captures the non-deterministic
search and selection process for later review and correction.';

-- =============================================================================
-- RESEARCH ACTIONS (Phase 2 Audit Trail)
-- =============================================================================
-- Every import/update via research verbs is logged here.
-- Links back to the decision that provided the identifier.

CREATE TABLE IF NOT EXISTS kyc.research_actions (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What entity this affects
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Link to decision that triggered this
    decision_id UUID REFERENCES kyc.research_decisions(decision_id),

    -- Action details
    action_type VARCHAR(50) NOT NULL,  -- IMPORT_HIERARCHY, IMPORT_PSC, ENRICH, etc.
    source_provider VARCHAR(30) NOT NULL,
    source_key VARCHAR(100) NOT NULL,  -- The identifier used
    source_key_type VARCHAR(20) NOT NULL,

    -- DSL verb executed
    verb_domain VARCHAR(30) NOT NULL,
    verb_name VARCHAR(50) NOT NULL,
    verb_args JSONB NOT NULL DEFAULT '{}',

    -- Outcome
    success BOOLEAN NOT NULL,

    -- Changes made (if successful)
    entities_created INTEGER DEFAULT 0,
    entities_updated INTEGER DEFAULT 0,
    relationships_created INTEGER DEFAULT 0,
    fields_updated JSONB,  -- [{entity_id, field, old_value, new_value}]

    -- Errors (if failed)
    error_code VARCHAR(50),
    error_message TEXT,

    -- Performance
    duration_ms INTEGER,
    api_calls_made INTEGER,

    -- Audit
    executed_at TIMESTAMPTZ DEFAULT NOW(),
    executed_by UUID,
    session_id UUID,

    -- Rollback support
    is_rolled_back BOOLEAN DEFAULT false,
    rolled_back_at TIMESTAMPTZ,
    rolled_back_by UUID,
    rollback_reason TEXT
);

CREATE INDEX idx_research_actions_target ON kyc.research_actions(target_entity_id);
CREATE INDEX idx_research_actions_decision ON kyc.research_actions(decision_id);
CREATE INDEX idx_research_actions_verb ON kyc.research_actions(verb_domain, verb_name);
CREATE INDEX idx_research_actions_executed ON kyc.research_actions(executed_at);
CREATE INDEX idx_research_actions_success ON kyc.research_actions(success);

COMMENT ON TABLE kyc.research_actions IS
'Audit trail for Phase 2 (DSL execution). Every import/update via research verbs
is logged here with full details for reproducibility and rollback.';

-- Update research_decisions with resulting action link
-- (Can't add FK constraint due to circular reference, but we track the link)

-- =============================================================================
-- RESEARCH CORRECTIONS
-- =============================================================================
-- Tracks when a wrong identifier was selected and corrected.

CREATE TABLE IF NOT EXISTS kyc.research_corrections (
    correction_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What's being corrected
    original_decision_id UUID NOT NULL REFERENCES kyc.research_decisions(decision_id),
    original_action_id UUID REFERENCES kyc.research_actions(action_id),

    -- Correction details
    correction_type VARCHAR(20) NOT NULL,

    -- Wrong selection
    wrong_key VARCHAR(100),
    wrong_key_type VARCHAR(20),

    -- Correct selection
    correct_key VARCHAR(100),
    correct_key_type VARCHAR(20),

    -- New action (if re-imported)
    new_action_id UUID REFERENCES kyc.research_actions(action_id),

    -- Why
    correction_reason TEXT NOT NULL,

    -- Who/when
    corrected_at TIMESTAMPTZ DEFAULT NOW(),
    corrected_by UUID NOT NULL,

    CONSTRAINT chk_correction_type CHECK (
        correction_type IN (
            'WRONG_ENTITY',       -- Selected wrong entity entirely
            'WRONG_JURISDICTION', -- Right name, wrong jurisdiction
            'STALE_DATA',         -- Data was outdated
            'MERGE_REQUIRED',     -- Need to merge with existing
            'UNLINK'              -- Remove incorrect link
        )
    )
);

CREATE INDEX idx_corrections_decision ON kyc.research_corrections(original_decision_id);
CREATE INDEX idx_corrections_corrected ON kyc.research_corrections(corrected_at);

COMMENT ON TABLE kyc.research_corrections IS
'Tracks corrections when Phase 1 selected the wrong identifier.
Supports learning and audit trail for regulatory inquiries.';

-- =============================================================================
-- RESEARCH ANOMALIES
-- =============================================================================
-- Post-import validation flags anomalies for review.

CREATE TABLE IF NOT EXISTS kyc.research_anomalies (
    anomaly_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- What action triggered this
    action_id UUID NOT NULL REFERENCES kyc.research_actions(action_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Anomaly details
    rule_code VARCHAR(50) NOT NULL,
    severity VARCHAR(10) NOT NULL,
    description TEXT NOT NULL,

    -- Context
    expected_value TEXT,
    actual_value TEXT,

    -- Resolution
    status VARCHAR(20) DEFAULT 'OPEN',
    resolution TEXT,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,

    -- Audit
    detected_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT chk_anomaly_severity CHECK (severity IN ('ERROR', 'WARNING', 'INFO')),
    CONSTRAINT chk_anomaly_status CHECK (status IN ('OPEN', 'ACKNOWLEDGED', 'RESOLVED', 'FALSE_POSITIVE'))
);

CREATE INDEX idx_anomalies_action ON kyc.research_anomalies(action_id);
CREATE INDEX idx_anomalies_entity ON kyc.research_anomalies(entity_id);
CREATE INDEX idx_anomalies_status ON kyc.research_anomalies(status);

COMMENT ON TABLE kyc.research_anomalies IS
'Post-import validation anomalies. Flags issues like jurisdiction mismatch,
circular ownership, or duplicate entities for human review.';

-- =============================================================================
-- RESEARCH CONFIDENCE CONFIG
-- =============================================================================
-- Configurable thresholds for auto-proceed vs ambiguous vs reject.

CREATE TABLE IF NOT EXISTS kyc.research_confidence_config (
    config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope (global or per-source)
    source_provider VARCHAR(30),  -- NULL = global default

    -- Thresholds
    auto_proceed_threshold DECIMAL(3,2) DEFAULT 0.90,
    ambiguous_threshold DECIMAL(3,2) DEFAULT 0.70,
    reject_threshold DECIMAL(3,2) DEFAULT 0.50,

    -- Behavior
    require_human_checkpoint BOOLEAN DEFAULT false,
    checkpoint_contexts TEXT[],  -- {'NEW_CLIENT', 'MATERIAL_HOLDING', 'HIGH_RISK'}

    -- Limits
    max_auto_imports_per_session INTEGER DEFAULT 50,
    max_chain_depth INTEGER DEFAULT 10,

    -- Active
    effective_from DATE DEFAULT CURRENT_DATE,
    effective_to DATE,

    CONSTRAINT uq_confidence_source UNIQUE (source_provider, effective_from)
);

-- Seed defaults
INSERT INTO kyc.research_confidence_config (
    source_provider, auto_proceed_threshold, ambiguous_threshold, require_human_checkpoint
) VALUES
    (NULL, 0.90, 0.70, false),           -- Global default
    ('gleif', 0.92, 0.75, false),        -- GLEIF is authoritative, high bar
    ('companies_house', 0.88, 0.70, false),
    ('orbis', 0.85, 0.65, false),        -- Commercial, slightly lower
    ('screening', 0.00, 0.00, true)      -- Always human checkpoint for screening
ON CONFLICT DO NOTHING;

COMMENT ON TABLE kyc.research_confidence_config IS
'Configurable thresholds for confidence-based routing. Determines when to
auto-proceed, ask user to disambiguate, or reject matches.';

-- =============================================================================
-- OUTREACH REQUESTS
-- =============================================================================
-- Track requests sent to counterparties for information.

CREATE TABLE IF NOT EXISTS kyc.outreach_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Target
    target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Request type
    request_type VARCHAR(30) NOT NULL,

    -- Recipient
    recipient_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    recipient_email VARCHAR(255),
    recipient_name VARCHAR(255),

    -- Status
    status VARCHAR(20) DEFAULT 'DRAFT',

    -- Timing
    deadline_date DATE,
    sent_at TIMESTAMPTZ,
    reminder_sent_at TIMESTAMPTZ,

    -- Response
    response_type VARCHAR(30),
    response_received_at TIMESTAMPTZ,
    response_document_id UUID,

    -- Notes
    request_notes TEXT,
    resolution_notes TEXT,

    -- Link to research trigger
    trigger_id UUID,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID,
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT chk_request_type CHECK (
        request_type IN (
            'NOMINEE_DISCLOSURE',
            'UBO_DECLARATION',
            'SHARE_REGISTER',
            'BOARD_COMPOSITION',
            'BENEFICIAL_OWNERSHIP',
            'GENERAL_INQUIRY'
        )
    ),
    CONSTRAINT chk_request_status CHECK (
        status IN ('DRAFT', 'PENDING', 'SENT', 'REMINDED', 'RESPONDED', 'CLOSED', 'EXPIRED')
    ),
    CONSTRAINT chk_response_type CHECK (
        response_type IS NULL OR response_type IN (
            'FULL_DISCLOSURE', 'PARTIAL_DISCLOSURE', 'DECLINED', 'NO_RESPONSE'
        )
    )
);

CREATE INDEX idx_outreach_target ON kyc.outreach_requests(target_entity_id);
CREATE INDEX idx_outreach_status ON kyc.outreach_requests(status);
CREATE INDEX idx_outreach_deadline ON kyc.outreach_requests(deadline_date);

COMMENT ON TABLE kyc.outreach_requests IS
'Tracks outreach requests to counterparties for ownership disclosures,
UBO declarations, and other information gathering.';

-- =============================================================================
-- VIEWS FOR REPORTING
-- =============================================================================

-- Research activity summary by entity
CREATE OR REPLACE VIEW kyc.v_research_activity AS
SELECT
    e.entity_id,
    e.name AS entity_name,
    COUNT(DISTINCT d.decision_id) AS decision_count,
    COUNT(DISTINCT a.action_id) AS action_count,
    COUNT(DISTINCT c.correction_id) AS correction_count,
    COUNT(DISTINCT an.anomaly_id) FILTER (WHERE an.status = 'OPEN') AS open_anomalies,
    MAX(a.executed_at) AS last_research_action
FROM "ob-poc".entities e
LEFT JOIN kyc.research_decisions d ON d.target_entity_id = e.entity_id
LEFT JOIN kyc.research_actions a ON a.target_entity_id = e.entity_id
LEFT JOIN kyc.research_corrections c ON c.original_decision_id = d.decision_id
LEFT JOIN kyc.research_anomalies an ON an.entity_id = e.entity_id
GROUP BY e.entity_id, e.name;

COMMENT ON VIEW kyc.v_research_activity IS
'Summary of research activity per entity for monitoring and reporting.';

-- Pending decisions requiring user input
CREATE OR REPLACE VIEW kyc.v_pending_decisions AS
SELECT
    d.decision_id,
    d.target_entity_id,
    e.name AS entity_name,
    d.search_query,
    d.source_provider,
    d.candidates_count,
    d.candidates_found,
    d.decision_type,
    d.created_at
FROM kyc.research_decisions d
JOIN "ob-poc".entities e ON e.entity_id = d.target_entity_id
WHERE d.decision_type = 'AMBIGUOUS'
  AND d.verified_by IS NULL
ORDER BY d.created_at;

COMMENT ON VIEW kyc.v_pending_decisions IS
'Decisions awaiting user disambiguation or confirmation.';
