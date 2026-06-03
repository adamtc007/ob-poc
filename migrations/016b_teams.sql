-- =============================================================================
-- TEAMS SCHEMA - Part 1
-- Teams, Access Domains, Memberships
-- =============================================================================

CREATE SCHEMA IF NOT EXISTS teams;

-- -----------------------------------------------------------------------------
-- Access domain reference (for validation)
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.access_domains (
    domain_code VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    visualizer_views TEXT[] NOT NULL DEFAULT '{}',
    is_active BOOLEAN DEFAULT TRUE
);

INSERT INTO teams.access_domains (domain_code, name, description, visualizer_views) VALUES
    ('KYC', 'KYC & Compliance', 'KYC, AML, entity management, screening',
     ARRAY['entity_graph', 'ubo_graph', 'document_catalog', 'case_timeline']),
    ('TRADING', 'Trading Operations', 'Trading, settlement, positions',
     ARRAY['trading_matrix', 'ssi_map', 'position_summary', 'settlement_status']),
    ('ACCOUNTING', 'Accounting & Billing', 'Invoicing, contracts, fees',
     ARRAY['invoice_history', 'fee_schedule', 'contract_summary']),
    ('REPORTING', 'Reporting & Analytics', 'Reports, dashboards, analytics',
     ARRAY['report_catalog', 'dashboard']),
    ('ADMIN', 'Administration', 'System and user administration',
     ARRAY['all'])
ON CONFLICT (domain_code) DO NOTHING;

-- -----------------------------------------------------------------------------
-- Function -> Domain mapping
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.function_domains (
    function_name VARCHAR(100) PRIMARY KEY,
    access_domains VARCHAR(50)[] NOT NULL,
    description TEXT
);

INSERT INTO teams.function_domains (function_name, access_domains, description) VALUES
    -- Fund Ops functions
    ('settlement', ARRAY['TRADING'], 'DVP, FOP, matching'),
    ('cash-management', ARRAY['TRADING'], 'Sweeps, funding, FX'),
    ('reconciliation', ARRAY['TRADING', 'ACCOUNTING'], 'Breaks, resolution'),
    ('corporate-actions', ARRAY['TRADING'], 'CA processing'),
    ('pricing-nav', ARRAY['TRADING', 'REPORTING'], 'NAV calculation'),
    ('kyc-onboarding', ARRAY['KYC'], 'Client onboarding'),

    -- ManCo Oversight functions
    ('compliance', ARRAY['KYC', 'REPORTING'], 'Regulatory compliance'),
    ('board', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Board-level oversight'),
    ('conducting', ARRAY['KYC'], 'Conducting officer duties'),
    ('risk', ARRAY['TRADING', 'REPORTING'], 'Risk oversight'),

    -- IM Trading functions
    ('portfolio', ARRAY['TRADING'], 'Portfolio management'),
    ('dealing', ARRAY['TRADING'], 'Execution'),
    ('middle-office', ARRAY['TRADING'], 'Confirmation, allocation'),

    -- SPV Admin functions
    ('trustee', ARRAY['KYC', 'TRADING'], 'Trustee responsibilities'),
    ('servicer', ARRAY['TRADING', 'ACCOUNTING'], 'Loan servicing'),
    ('waterfall', ARRAY['TRADING', 'ACCOUNTING'], 'Payment waterfall'),

    -- Client Service functions
    ('relationship', ARRAY['KYC', 'TRADING', 'ACCOUNTING'], 'RM - sees all'),
    ('onboarding', ARRAY['KYC'], 'Onboarding support'),
    ('support', ARRAY['KYC', 'TRADING'], 'Day-to-day support'),

    -- Accounting functions
    ('accounts-payable', ARRAY['ACCOUNTING'], 'Invoice processing'),
    ('billing', ARRAY['ACCOUNTING'], 'Billing management'),
    ('contract-admin', ARRAY['ACCOUNTING'], 'Contract administration'),
    ('cost-allocation', ARRAY['ACCOUNTING'], 'Cost allocation'),

    -- Reporting functions
    ('investor-reporting', ARRAY['REPORTING'], 'Investor reports'),
    ('regulatory-reporting', ARRAY['KYC', 'REPORTING'], 'Regulatory filings'),

    -- Admin functions
    ('team-admin', ARRAY['ADMIN'], 'Team administration'),
    ('user-admin', ARRAY['ADMIN'], 'User administration'),

    -- Governance functions (Part 6)
    ('board-secretary', ARRAY['KYC', 'REPORTING'], 'Board administration'),
    ('investment-decision', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Investment approval authority'),
    ('risk-oversight', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Risk function oversight'),
    ('compliance-oversight', ARRAY['KYC', 'REPORTING'], 'Compliance function oversight'),
    ('cio', ARRAY['TRADING', 'REPORTING'], 'Chief Investment Officer'),
    ('coo', ARRAY['KYC', 'TRADING', 'ACCOUNTING', 'REPORTING'], 'Chief Operating Officer'),
    ('cfo', ARRAY['ACCOUNTING', 'REPORTING'], 'Chief Financial Officer'),
    ('cro', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Chief Risk Officer')
ON CONFLICT (function_name) DO NOTHING;

-- -----------------------------------------------------------------------------
-- Teams
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.teams (
    team_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,

    -- Type (includes governance types from Part 6)
    team_type VARCHAR(50) NOT NULL,
    CONSTRAINT chk_team_type CHECK (team_type IN (
        'fund-ops', 'manco-oversight', 'im-trading', 'spv-admin',
        'client-service', 'accounting', 'reporting',
        'board', 'investment-committee', 'conducting-officers', 'executive'
    )),

    -- Authority delegation
    delegating_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    authority_type VARCHAR(50) NOT NULL,
    CONSTRAINT chk_authority_type CHECK (authority_type IN (
        'operational', 'oversight', 'trading', 'administrative', 'governance'
    )),
    authority_scope JSONB DEFAULT '{}',

    -- CBU access mode
    access_mode VARCHAR(50) NOT NULL,
    CONSTRAINT chk_access_mode CHECK (access_mode IN (
        'explicit', 'by-manco', 'by-im', 'by-filter'
    )),
    explicit_cbus UUID[],
    scope_filter JSONB,

    -- Service entitlements
    service_entitlements JSONB DEFAULT '{}',

    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    archived_at TIMESTAMPTZ,
    archive_reason TEXT,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    created_by_user_id UUID
);

CREATE INDEX IF NOT EXISTS idx_teams_entity ON teams.teams(delegating_entity_id);
CREATE INDEX IF NOT EXISTS idx_teams_type ON teams.teams(team_type);
CREATE INDEX IF NOT EXISTS idx_teams_active ON teams.teams(is_active) WHERE is_active = TRUE;

-- -----------------------------------------------------------------------------
-- Users (evolved from client_portal.clients)
-- We keep the existing table and add columns
-- -----------------------------------------------------------------------------

-- Add new columns to existing clients table
ALTER TABLE client_portal.clients
    ADD COLUMN IF NOT EXISTS employer_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    ADD COLUMN IF NOT EXISTS identity_provider VARCHAR(50) DEFAULT 'local',
    ADD COLUMN IF NOT EXISTS status VARCHAR(50) DEFAULT 'ACTIVE',
    ADD COLUMN IF NOT EXISTS offboarded_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS offboard_reason VARCHAR(50);

-- Add constraints (ignore errors if they exist)
DO $$
BEGIN
    ALTER TABLE client_portal.clients
        ADD CONSTRAINT chk_user_status CHECK (status IN ('ACTIVE', 'SUSPENDED', 'OFFBOARDED'));
EXCEPTION WHEN duplicate_object THEN
    NULL;
END $$;

DO $$
BEGIN
    ALTER TABLE client_portal.clients
        ADD CONSTRAINT chk_identity_provider CHECK (identity_provider IN ('local', 'saml', 'oidc'));
EXCEPTION WHEN duplicate_object THEN
    NULL;
END $$;

DO $$
BEGIN
    ALTER TABLE client_portal.clients
        ADD CONSTRAINT chk_offboard_reason CHECK (offboard_reason IS NULL OR offboard_reason IN (
            'resigned', 'terminated', 'retired', 'deceased', 'other'
        ));
EXCEPTION WHEN duplicate_object THEN
    NULL;
END $$;

-- Create a view that renames client_id to user_id for new code
CREATE OR REPLACE VIEW client_portal.users AS
SELECT
    client_id AS user_id,
    name,
    email,
    accessible_cbus,
    is_active,
    created_at,
    updated_at,
    last_login_at,
    employer_entity_id,
    identity_provider,
    status,
    offboarded_at,
    offboard_reason
FROM client_portal.clients;

-- -----------------------------------------------------------------------------
-- Team Memberships
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.memberships (
    membership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams.teams(team_id),
    user_id UUID NOT NULL REFERENCES client_portal.clients(client_id),

    -- The composite role key: {team_type}.{function}:{level}
    role_key VARCHAR(100) NOT NULL,

    -- Parsed components (generated for efficient querying)
    team_type VARCHAR(50) GENERATED ALWAYS AS (split_part(role_key, '.', 1)) STORED,
    function_name VARCHAR(50) GENERATED ALWAYS AS (
        split_part(split_part(role_key, '.', 2), ':', 1)
    ) STORED,
    role_level VARCHAR(50) GENERATED ALWAYS AS (split_part(role_key, ':', 2)) STORED,

    -- Validity period
    effective_from DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to DATE,

    -- Permission overrides (fine-grained)
    permission_overrides JSONB DEFAULT '{}',

    -- Legal appointment linkage (Part 6: Governance)
    legal_appointment_id UUID,
    requires_legal_appointment BOOLEAN DEFAULT FALSE,

    -- Audit
    delegated_by_user_id UUID,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- User can have multiple roles in same team
    UNIQUE(team_id, user_id, role_key)
);

CREATE INDEX IF NOT EXISTS idx_membership_team ON teams.memberships(team_id);
CREATE INDEX IF NOT EXISTS idx_membership_user ON teams.memberships(user_id);
CREATE INDEX IF NOT EXISTS idx_membership_type ON teams.memberships(team_type);
CREATE INDEX IF NOT EXISTS idx_membership_function ON teams.memberships(function_name);
-- Note: partial index on active memberships uses NULL check only
-- (CURRENT_DATE is not immutable so can't be used in index predicate)
CREATE INDEX IF NOT EXISTS idx_membership_active ON teams.memberships(effective_from, effective_to)
    WHERE effective_to IS NULL;

COMMENT ON COLUMN teams.memberships.legal_appointment_id IS
    'Links portal access to legal appointment (DIRECTOR, CONDUCTING_OFFICER, etc.)';
COMMENT ON COLUMN teams.memberships.requires_legal_appointment IS
    'If true, warns when no legal appointment linked';

-- -----------------------------------------------------------------------------
-- Explicit CBU Access (for explicit access mode)
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.team_cbu_access (
    access_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams.teams(team_id),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),

    -- Optional restrictions within the CBU
    access_restrictions JSONB DEFAULT '{}',

    granted_at TIMESTAMPTZ DEFAULT NOW(),
    granted_by_user_id UUID,

    UNIQUE(team_id, cbu_id)
);

-- -----------------------------------------------------------------------------
-- Membership History (audit trail)
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.membership_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    membership_id UUID NOT NULL,
    team_id UUID NOT NULL,
    user_id UUID NOT NULL,

    -- What changed
    action VARCHAR(50) NOT NULL,  -- ADDED, UPDATED, REMOVED, TRANSFERRED
    old_role_key VARCHAR(100),
    new_role_key VARCHAR(100),

    -- Why
    reason TEXT,

    -- Who/when
    changed_by_user_id UUID,
    changed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_membership_history_user ON teams.membership_history(user_id);
CREATE INDEX IF NOT EXISTS idx_membership_history_team ON teams.membership_history(team_id);

-- -----------------------------------------------------------------------------
-- Views
-- -----------------------------------------------------------------------------

-- Effective memberships (only active)
CREATE OR REPLACE VIEW teams.v_effective_memberships AS
SELECT
    m.*,
    t.name as team_name,
    t.delegating_entity_id,
    e.name as delegating_entity_name,
    c.name as user_name,
    c.email as user_email,
    fd.access_domains
FROM teams.memberships m
JOIN teams.teams t ON m.team_id = t.team_id
JOIN "ob-poc".entities e ON t.delegating_entity_id = e.entity_id
JOIN client_portal.clients c ON m.user_id = c.client_id
LEFT JOIN teams.function_domains fd ON m.function_name = fd.function_name
WHERE t.is_active = TRUE
  AND c.status = 'ACTIVE'
  AND m.effective_from <= CURRENT_DATE
  AND (m.effective_to IS NULL OR m.effective_to >= CURRENT_DATE);

-- User's resolved CBU access
-- Note: adapted to use manager_entity_id from custody.cbu_im_assignments
-- and we don't have manco_entity_id yet, so by-manco uses cbu_entity_roles
CREATE OR REPLACE VIEW teams.v_user_cbu_access AS
WITH user_teams AS (
    SELECT
        m.user_id,
        t.team_id,
        t.name as team_name,
        m.role_key,
        fd.access_domains,
        t.access_mode,
        t.explicit_cbus,
        t.delegating_entity_id,
        t.scope_filter,
        t.authority_scope
    FROM teams.v_effective_memberships m
    JOIN teams.teams t ON m.team_id = t.team_id
    LEFT JOIN teams.function_domains fd ON m.function_name = fd.function_name
),
resolved_cbus AS (
    -- Explicit mode
    SELECT ut.user_id, ut.team_id, ut.team_name, ut.role_key, ut.access_domains,
           unnest(ut.explicit_cbus) as cbu_id
    FROM user_teams ut
    WHERE ut.access_mode = 'explicit' AND ut.explicit_cbus IS NOT NULL

    UNION ALL

    -- By ManCo mode: CBUs where the delegating entity has MANAGEMENT_COMPANY role
    SELECT ut.user_id, ut.team_id, ut.team_name, ut.role_key, ut.access_domains,
           cer.cbu_id
    FROM user_teams ut
    JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = ut.delegating_entity_id
    JOIN "ob-poc".roles r ON cer.role_id = r.role_id AND r.name = 'MANAGEMENT_COMPANY'
    WHERE ut.access_mode = 'by-manco'

    UNION ALL

    -- By IM mode: CBUs where delegating entity is an investment manager
    SELECT ut.user_id, ut.team_id, ut.team_name, ut.role_key, ut.access_domains,
           a.cbu_id
    FROM user_teams ut
    JOIN custody.cbu_im_assignments a ON a.manager_entity_id = ut.delegating_entity_id
    WHERE ut.access_mode = 'by-im' AND a.status = 'ACTIVE'
)
SELECT
    rc.user_id,
    rc.cbu_id,
    c.name as cbu_name,
    array_agg(DISTINCT rc.team_id) as via_teams,
    array_agg(DISTINCT rc.role_key) as roles,
    array_agg(DISTINCT unnest_domain) as access_domains
FROM resolved_cbus rc
JOIN "ob-poc".cbus c ON rc.cbu_id = c.cbu_id
CROSS JOIN LATERAL unnest(COALESCE(rc.access_domains, ARRAY[]::varchar[])) as unnest_domain
GROUP BY rc.user_id, rc.cbu_id, c.name;

-- Governance access view (Part 6)
CREATE OR REPLACE VIEW teams.v_governance_access AS
SELECT
    m.membership_id,
    m.user_id,
    c.name as user_name,
    c.email as user_email,
    m.team_id,
    t.name as team_name,
    t.team_type,
    m.role_key,
    m.function_name,
    m.role_level,
    fd.access_domains,
    -- Legal appointment details (if cbu_entity_roles exists)
    m.legal_appointment_id,
    -- Warning flag
    CASE
        WHEN m.requires_legal_appointment AND m.legal_appointment_id IS NULL
        THEN TRUE
        ELSE FALSE
    END as missing_legal_appointment
FROM teams.memberships m
JOIN teams.teams t ON m.team_id = t.team_id
JOIN client_portal.clients c ON m.user_id = c.client_id
LEFT JOIN teams.function_domains fd ON m.function_name = fd.function_name
WHERE t.team_type IN ('board', 'investment-committee', 'conducting-officers', 'executive')
  AND t.is_active = TRUE
  AND c.status = 'ACTIVE'
  AND m.effective_from <= CURRENT_DATE
  AND (m.effective_to IS NULL OR m.effective_to >= CURRENT_DATE);

-- -----------------------------------------------------------------------------
-- Functions
-- -----------------------------------------------------------------------------

-- Get user's access domains
CREATE OR REPLACE FUNCTION teams.get_user_access_domains(p_user_id UUID)
RETURNS VARCHAR(50)[] AS $$
    SELECT array_agg(DISTINCT unnest_domain)
    FROM teams.v_effective_memberships m
    CROSS JOIN LATERAL unnest(COALESCE(m.access_domains, ARRAY[]::varchar[])) as unnest_domain
    WHERE m.user_id = p_user_id;
$$ LANGUAGE SQL STABLE;

-- Check if user has specific access domain
CREATE OR REPLACE FUNCTION teams.user_has_domain(p_user_id UUID, p_domain VARCHAR(50))
RETURNS BOOLEAN AS $$
    SELECT p_domain = ANY(COALESCE(teams.get_user_access_domains(p_user_id), ARRAY[]::varchar[]));
$$ LANGUAGE SQL STABLE;

-- Check if user can access specific CBU
CREATE OR REPLACE FUNCTION teams.user_can_access_cbu(p_user_id UUID, p_cbu_id UUID)
RETURNS BOOLEAN AS $$
    SELECT EXISTS (
        SELECT 1 FROM teams.v_user_cbu_access
        WHERE user_id = p_user_id AND cbu_id = p_cbu_id
    );
$$ LANGUAGE SQL STABLE;

-- Get user's CBU access with domains
CREATE OR REPLACE FUNCTION teams.get_user_cbu_access(p_user_id UUID)
RETURNS TABLE (
    cbu_id UUID,
    cbu_name VARCHAR,
    access_domains VARCHAR[],
    via_teams UUID[],
    roles VARCHAR[]
) AS $$
    SELECT cbu_id, cbu_name, access_domains, via_teams, roles
    FROM teams.v_user_cbu_access
    WHERE user_id = p_user_id;
$$ LANGUAGE SQL STABLE;

-- -----------------------------------------------------------------------------
-- Triggers
-- -----------------------------------------------------------------------------

-- Update timestamp trigger
CREATE OR REPLACE FUNCTION teams.update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_teams_updated ON teams.teams;
CREATE TRIGGER trg_teams_updated
    BEFORE UPDATE ON teams.teams
    FOR EACH ROW EXECUTE FUNCTION teams.update_timestamp();

DROP TRIGGER IF EXISTS trg_memberships_updated ON teams.memberships;
CREATE TRIGGER trg_memberships_updated
    BEFORE UPDATE ON teams.memberships
    FOR EACH ROW EXECUTE FUNCTION teams.update_timestamp();

-- Membership history trigger
CREATE OR REPLACE FUNCTION teams.log_membership_change()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO teams.membership_history
            (membership_id, team_id, user_id, action, new_role_key, changed_by_user_id)
        VALUES
            (NEW.membership_id, NEW.team_id, NEW.user_id, 'ADDED', NEW.role_key, NEW.delegated_by_user_id);
    ELSIF TG_OP = 'UPDATE' THEN
        IF OLD.role_key != NEW.role_key THEN
            INSERT INTO teams.membership_history
                (membership_id, team_id, user_id, action, old_role_key, new_role_key)
            VALUES
                (NEW.membership_id, NEW.team_id, NEW.user_id, 'UPDATED', OLD.role_key, NEW.role_key);
        END IF;
        IF NEW.effective_to IS NOT NULL AND OLD.effective_to IS NULL THEN
            INSERT INTO teams.membership_history
                (membership_id, team_id, user_id, action, old_role_key)
            VALUES
                (NEW.membership_id, NEW.team_id, NEW.user_id, 'REMOVED', OLD.role_key);
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_membership_history ON teams.memberships;
CREATE TRIGGER trg_membership_history
    AFTER INSERT OR UPDATE ON teams.memberships
    FOR EACH ROW EXECUTE FUNCTION teams.log_membership_change();

-- -----------------------------------------------------------------------------
-- Team Service Entitlements
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.team_service_entitlements (
    entitlement_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams.teams(team_id),
    service_code VARCHAR(100) NOT NULL,
    config JSONB DEFAULT '{}',
    granted_at TIMESTAMPTZ DEFAULT NOW(),
    granted_by_user_id UUID,
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(team_id, service_code)
);

-- -----------------------------------------------------------------------------
-- Membership Audit Log (for plugin operations)
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.membership_audit_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams.teams(team_id),
    user_id UUID NOT NULL,
    action VARCHAR(50) NOT NULL,
    reason TEXT,
    performed_at TIMESTAMPTZ DEFAULT NOW(),
    performed_by_user_id UUID
);

CREATE INDEX IF NOT EXISTS idx_membership_audit_team ON teams.membership_audit_log(team_id);
CREATE INDEX IF NOT EXISTS idx_membership_audit_user ON teams.membership_audit_log(user_id);

-- =============================================================================
-- ACCESS REVIEW AUTOMATION - Part 7
-- Periodic access reviews with detection, workflow, attestation, and enforcement
-- =============================================================================

-- -----------------------------------------------------------------------------
-- Review Campaigns (quarterly, annual, triggered)
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.access_review_campaigns (
    campaign_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Campaign details
    name VARCHAR(255) NOT NULL,
    review_type VARCHAR(50) NOT NULL,
    CONSTRAINT chk_review_type CHECK (review_type IN (
        'QUARTERLY', 'ANNUAL', 'TRIGGERED', 'JOINER_MOVER_LEAVER'
    )),

    -- Scope
    scope_type VARCHAR(50) NOT NULL,
    CONSTRAINT chk_scope_type CHECK (scope_type IN (
        'ALL', 'BY_TEAM_TYPE', 'BY_DELEGATING_ENTITY', 'SPECIFIC_TEAMS', 'GOVERNANCE_ONLY'
    )),
    scope_filter JSONB,

    -- Timing
    review_period_start DATE NOT NULL DEFAULT CURRENT_DATE,
    review_period_end DATE NOT NULL DEFAULT CURRENT_DATE,
    deadline DATE NOT NULL,
    reminder_days INTEGER[] DEFAULT ARRAY[7, 3, 1],

    -- Status
    status VARCHAR(50) DEFAULT 'DRAFT',
    CONSTRAINT chk_campaign_status CHECK (status IN (
        'DRAFT', 'POPULATING', 'ACTIVE', 'IN_REVIEW', 'PAST_DEADLINE',
        'COMPLETED', 'CANCELLED'
    )),

    -- Stats (denormalized for dashboard)
    total_items INTEGER DEFAULT 0,
    reviewed_items INTEGER DEFAULT 0,
    confirmed_items INTEGER DEFAULT 0,
    revoked_items INTEGER DEFAULT 0,
    extended_items INTEGER DEFAULT 0,
    escalated_items INTEGER DEFAULT 0,
    pending_items INTEGER DEFAULT 0,

    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by_user_id UUID,
    launched_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_campaigns_status ON teams.access_review_campaigns(status);
CREATE INDEX IF NOT EXISTS idx_campaigns_deadline ON teams.access_review_campaigns(deadline);

-- -----------------------------------------------------------------------------
-- Individual Review Items
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.access_review_items (
    item_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL REFERENCES teams.access_review_campaigns(campaign_id),

    -- What's being reviewed (snapshot at campaign creation)
    membership_id UUID NOT NULL REFERENCES teams.memberships(membership_id),
    user_id UUID NOT NULL,
    team_id UUID NOT NULL,
    role_key VARCHAR(100) NOT NULL,

    -- Context snapshot for reviewer
    user_name VARCHAR(255),
    user_email VARCHAR(255),
    user_employer VARCHAR(255),
    team_name VARCHAR(255),
    team_type VARCHAR(50),
    delegating_entity_name VARCHAR(255),
    access_domains VARCHAR(50)[],

    -- Legal appointment context
    legal_appointment_id UUID,
    legal_position VARCHAR(100),
    legal_entity_name VARCHAR(255),
    legal_effective_from DATE,
    legal_effective_to DATE,

    -- Activity context
    last_login_at TIMESTAMPTZ,
    days_since_login INTEGER,
    membership_created_at TIMESTAMPTZ,
    membership_age_days INTEGER,

    -- Flags (auto-detected issues)
    flag_no_legal_link BOOLEAN DEFAULT FALSE,
    flag_legal_expired BOOLEAN DEFAULT FALSE,
    flag_legal_expiring_soon BOOLEAN DEFAULT FALSE,
    flag_dormant_account BOOLEAN DEFAULT FALSE,
    flag_never_logged_in BOOLEAN DEFAULT FALSE,
    flag_role_mismatch BOOLEAN DEFAULT FALSE,
    flag_orphaned_membership BOOLEAN DEFAULT FALSE,
    flags_json JSONB DEFAULT '{}',

    -- Recommendation
    recommendation VARCHAR(50),
    CONSTRAINT chk_recommendation CHECK (recommendation IN (
        'CONFIRM', 'REVOKE', 'EXTEND', 'REVIEW', 'ESCALATE'
    )),
    recommendation_reason TEXT,
    risk_score INTEGER DEFAULT 0,

    -- Assignment
    reviewer_user_id UUID,
    reviewer_email VARCHAR(255),
    reviewer_name VARCHAR(255),

    -- Review outcome
    status VARCHAR(50) DEFAULT 'PENDING',
    CONSTRAINT chk_item_status CHECK (status IN (
        'PENDING', 'CONFIRMED', 'REVOKED', 'EXTENDED', 'ESCALATED',
        'AUTO_SUSPENDED', 'SKIPPED'
    )),
    reviewed_at TIMESTAMPTZ,
    reviewer_notes TEXT,

    -- If extended
    extended_to DATE,
    extension_reason TEXT,

    -- If escalated
    escalated_to_user_id UUID,
    escalation_reason TEXT,
    escalated_at TIMESTAMPTZ,

    -- If auto-actioned
    auto_action_at TIMESTAMPTZ,
    auto_action_reason TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_review_items_campaign ON teams.access_review_items(campaign_id);
CREATE INDEX IF NOT EXISTS idx_review_items_reviewer ON teams.access_review_items(reviewer_user_id);
CREATE INDEX IF NOT EXISTS idx_review_items_status ON teams.access_review_items(status);
CREATE INDEX IF NOT EXISTS idx_review_items_membership ON teams.access_review_items(membership_id);
CREATE INDEX IF NOT EXISTS idx_review_items_pending ON teams.access_review_items(campaign_id, status)
    WHERE status = 'PENDING';
CREATE INDEX IF NOT EXISTS idx_review_items_flagged ON teams.access_review_items(campaign_id)
    WHERE flag_no_legal_link OR flag_legal_expired OR flag_dormant_account;

-- -----------------------------------------------------------------------------
-- Attestations
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.access_attestations (
    attestation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL REFERENCES teams.access_review_campaigns(campaign_id),

    -- Who attested
    attester_user_id UUID NOT NULL,
    attester_name VARCHAR(255) NOT NULL,
    attester_email VARCHAR(255) NOT NULL,
    attester_role VARCHAR(100),

    -- Scope
    attestation_scope VARCHAR(50) NOT NULL,
    CONSTRAINT chk_attestation_scope CHECK (attestation_scope IN (
        'FULL_CAMPAIGN', 'MY_REVIEWS', 'SPECIFIC_TEAM', 'SPECIFIC_ITEMS'
    )),
    team_id UUID,
    item_ids UUID[],
    items_count INTEGER NOT NULL,

    -- Attestation content
    attestation_text TEXT NOT NULL,
    attestation_version VARCHAR(20) DEFAULT 'v1',

    -- Signature
    attested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    signature_hash TEXT NOT NULL,
    signature_input TEXT,

    -- Context
    ip_address INET,
    user_agent TEXT,
    session_id UUID
);

CREATE INDEX IF NOT EXISTS idx_attestations_campaign ON teams.access_attestations(campaign_id);
CREATE INDEX IF NOT EXISTS idx_attestations_attester ON teams.access_attestations(attester_user_id);

-- -----------------------------------------------------------------------------
-- Review Activity Log (detailed audit trail)
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS teams.access_review_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    campaign_id UUID REFERENCES teams.access_review_campaigns(campaign_id),
    item_id UUID REFERENCES teams.access_review_items(item_id),

    -- Action
    action VARCHAR(50) NOT NULL,
    action_detail JSONB,

    -- Who
    actor_user_id UUID,
    actor_email VARCHAR(255),
    actor_type VARCHAR(50),

    -- When
    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- Context
    ip_address INET
);

CREATE INDEX IF NOT EXISTS idx_review_log_campaign ON teams.access_review_log(campaign_id);
CREATE INDEX IF NOT EXISTS idx_review_log_item ON teams.access_review_log(item_id);
CREATE INDEX IF NOT EXISTS idx_review_log_time ON teams.access_review_log(created_at);

-- -----------------------------------------------------------------------------
-- Access Review Views
-- -----------------------------------------------------------------------------

-- Campaign dashboard view
CREATE OR REPLACE VIEW teams.v_campaign_dashboard AS
SELECT
    c.*,
    ROUND(c.reviewed_items::numeric / NULLIF(c.total_items, 0) * 100, 1) as progress_percent,
    c.deadline - CURRENT_DATE as days_until_deadline,
    CASE
        WHEN c.status = 'COMPLETED' THEN 'COMPLETED'
        WHEN CURRENT_DATE > c.deadline THEN 'OVERDUE'
        WHEN c.deadline - CURRENT_DATE <= 3 THEN 'URGENT'
        WHEN c.deadline - CURRENT_DATE <= 7 THEN 'DUE_SOON'
        ELSE 'ON_TRACK'
    END as urgency
FROM teams.access_review_campaigns c;

-- Reviewer workload view
CREATE OR REPLACE VIEW teams.v_reviewer_workload AS
SELECT
    i.campaign_id,
    i.reviewer_user_id,
    i.reviewer_email,
    i.reviewer_name,
    COUNT(*) as total_items,
    COUNT(*) FILTER (WHERE i.status = 'PENDING') as pending_items,
    COUNT(*) FILTER (WHERE i.status = 'CONFIRMED') as confirmed_items,
    COUNT(*) FILTER (WHERE i.status = 'REVOKED') as revoked_items,
    COUNT(*) FILTER (WHERE i.flag_legal_expired OR i.flag_no_legal_link OR i.flag_dormant_account) as flagged_items,
    EXISTS (
        SELECT 1 FROM teams.access_attestations a
        WHERE a.campaign_id = i.campaign_id
          AND a.attester_user_id = i.reviewer_user_id
    ) as has_attested
FROM teams.access_review_items i
GROUP BY i.campaign_id, i.reviewer_user_id, i.reviewer_email, i.reviewer_name;

-- Flagged items summary
CREATE OR REPLACE VIEW teams.v_flagged_items_summary AS
SELECT
    campaign_id,
    COUNT(*) FILTER (WHERE flag_legal_expired) as legal_expired_count,
    COUNT(*) FILTER (WHERE flag_legal_expiring_soon) as legal_expiring_count,
    COUNT(*) FILTER (WHERE flag_no_legal_link) as no_legal_link_count,
    COUNT(*) FILTER (WHERE flag_dormant_account) as dormant_count,
    COUNT(*) FILTER (WHERE flag_never_logged_in) as never_logged_in_count,
    COUNT(*) FILTER (WHERE flag_role_mismatch) as role_mismatch_count,
    COUNT(*) FILTER (WHERE risk_score >= 70) as high_risk_count,
    COUNT(*) FILTER (WHERE risk_score BETWEEN 40 AND 69) as medium_risk_count,
    COUNT(*) FILTER (WHERE risk_score < 40) as low_risk_count
FROM teams.access_review_items
GROUP BY campaign_id;

-- -----------------------------------------------------------------------------
-- Access Review Functions
-- -----------------------------------------------------------------------------

-- Generate attestation signature
CREATE OR REPLACE FUNCTION teams.generate_attestation_signature(
    p_attester_id UUID,
    p_campaign_id UUID,
    p_item_ids UUID[],
    p_attestation_text TEXT,
    p_timestamp TIMESTAMPTZ
)
RETURNS TEXT AS $$
DECLARE
    v_input TEXT;
BEGIN
    v_input := p_attester_id::text || '|' ||
               p_campaign_id::text || '|' ||
               array_to_string(p_item_ids, ',') || '|' ||
               p_attestation_text || '|' ||
               p_timestamp::text;

    RETURN 'sha256:' || encode(sha256(v_input::bytea), 'hex');
END;
$$ LANGUAGE plpgsql IMMUTABLE;
