# TODO: Teams, Roles & Access Domains

## Overview

Implement organizational identity layer between users and CBU access:

```
User → Team Membership (with role) → Team → CBU Access + Service Entitlements
                                       ↓
                              Delegating Entity (ManCo, IM, Fund)
```

**Key Concepts:**
- **Team**: Organizational unit with delegated authority from an entity
- **Role Key**: `{team_type}.{function}:{level}` - e.g., `fund-ops.settlement:approver`
- **Access Domain**: KYC, TRADING, ACCOUNTING, REPORTING - derived from function
- **CBU Access**: Explicit list, by ManCo, by IM, or by filter

---

## Part 1: Schema

### File: `rust/migrations/202412_teams.sql`

```sql
-- =============================================================================
-- TEAMS SCHEMA
-- =============================================================================

CREATE SCHEMA IF NOT EXISTS teams;

-- -----------------------------------------------------------------------------
-- Access domain reference (for validation)
-- -----------------------------------------------------------------------------

CREATE TABLE teams.access_domains (
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
     ARRAY['all']);

-- -----------------------------------------------------------------------------
-- Function → Domain mapping
-- -----------------------------------------------------------------------------

CREATE TABLE teams.function_domains (
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
    ('user-admin', ARRAY['ADMIN'], 'User administration');

-- -----------------------------------------------------------------------------
-- Teams
-- -----------------------------------------------------------------------------

CREATE TABLE teams.teams (
    team_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    
    -- Type
    team_type VARCHAR(50) NOT NULL,
    CONSTRAINT chk_team_type CHECK (team_type IN (
        'fund-ops', 'manco-oversight', 'im-trading', 'spv-admin', 
        'client-service', 'accounting', 'reporting'
    )),
    
    -- Authority delegation
    delegating_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    authority_type VARCHAR(50) NOT NULL,
    CONSTRAINT chk_authority_type CHECK (authority_type IN (
        'operational', 'oversight', 'trading', 'administrative'
    )),
    authority_scope JSONB DEFAULT '{}',
    -- { jurisdictions: ["LU", "IE"], fund_types: ["UCITS"], products: [] }
    
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

CREATE INDEX idx_teams_entity ON teams.teams(delegating_entity_id);
CREATE INDEX idx_teams_type ON teams.teams(team_type);
CREATE INDEX idx_teams_active ON teams.teams(is_active) WHERE is_active = TRUE;

-- -----------------------------------------------------------------------------
-- Users (rename from client_portal.clients)
-- -----------------------------------------------------------------------------

-- First rename the table
ALTER TABLE client_portal.clients RENAME TO users;

-- Add new columns
ALTER TABLE client_portal.users
    ADD COLUMN IF NOT EXISTS employer_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    ADD COLUMN IF NOT EXISTS identity_provider VARCHAR(50) DEFAULT 'local',
    ADD COLUMN IF NOT EXISTS status VARCHAR(50) DEFAULT 'ACTIVE',
    ADD COLUMN IF NOT EXISTS offboarded_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS offboard_reason VARCHAR(50);

-- Drop old column (CBU access now via teams)
ALTER TABLE client_portal.users DROP COLUMN IF EXISTS accessible_cbus;

-- Add constraints
ALTER TABLE client_portal.users
    ADD CONSTRAINT chk_user_status CHECK (status IN ('ACTIVE', 'SUSPENDED', 'OFFBOARDED')),
    ADD CONSTRAINT chk_identity_provider CHECK (identity_provider IN ('local', 'saml', 'oidc')),
    ADD CONSTRAINT chk_offboard_reason CHECK (offboard_reason IS NULL OR offboard_reason IN (
        'resigned', 'terminated', 'retired', 'deceased', 'other'
    ));

-- Rename client_id to user_id for clarity
ALTER TABLE client_portal.users RENAME COLUMN client_id TO user_id;

-- -----------------------------------------------------------------------------
-- Team Memberships
-- -----------------------------------------------------------------------------

CREATE TABLE teams.memberships (
    membership_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams.teams(team_id),
    user_id UUID NOT NULL REFERENCES client_portal.users(user_id),
    
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
    
    -- Audit
    delegated_by_user_id UUID,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- User can have multiple roles in same team
    UNIQUE(team_id, user_id, role_key)
);

CREATE INDEX idx_membership_team ON teams.memberships(team_id);
CREATE INDEX idx_membership_user ON teams.memberships(user_id);
CREATE INDEX idx_membership_type ON teams.memberships(team_type);
CREATE INDEX idx_membership_function ON teams.memberships(function_name);
CREATE INDEX idx_membership_active ON teams.memberships(effective_from, effective_to)
    WHERE effective_to IS NULL OR effective_to >= CURRENT_DATE;

-- -----------------------------------------------------------------------------
-- Explicit CBU Access (for explicit access mode)
-- -----------------------------------------------------------------------------

CREATE TABLE teams.team_cbu_access (
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

CREATE TABLE teams.membership_history (
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

CREATE INDEX idx_membership_history_user ON teams.membership_history(user_id);
CREATE INDEX idx_membership_history_team ON teams.membership_history(team_id);

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
    u.name as user_name,
    u.email as user_email,
    fd.access_domains
FROM teams.memberships m
JOIN teams.teams t ON m.team_id = t.team_id
JOIN "ob-poc".entities e ON t.delegating_entity_id = e.entity_id
JOIN client_portal.users u ON m.user_id = u.user_id
LEFT JOIN teams.function_domains fd ON m.function_name = fd.function_name
WHERE t.is_active = TRUE
  AND u.status = 'ACTIVE'
  AND m.effective_from <= CURRENT_DATE
  AND (m.effective_to IS NULL OR m.effective_to >= CURRENT_DATE);

-- User's resolved CBU access
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
    WHERE ut.access_mode = 'explicit'
    
    UNION ALL
    
    -- By ManCo mode
    SELECT ut.user_id, ut.team_id, ut.team_name, ut.role_key, ut.access_domains,
           c.cbu_id
    FROM user_teams ut
    JOIN "ob-poc".cbus c ON c.manco_entity_id = ut.delegating_entity_id
    WHERE ut.access_mode = 'by-manco'
    
    UNION ALL
    
    -- By IM mode
    SELECT ut.user_id, ut.team_id, ut.team_name, ut.role_key, ut.access_domains,
           a.cbu_id
    FROM user_teams ut
    JOIN "ob-poc".cbu_im_assignments a ON a.im_entity_id = ut.delegating_entity_id
    WHERE ut.access_mode = 'by-im'
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
CROSS JOIN LATERAL unnest(rc.access_domains) as unnest_domain
GROUP BY rc.user_id, rc.cbu_id, c.name;

-- -----------------------------------------------------------------------------
-- Functions
-- -----------------------------------------------------------------------------

-- Get user's access domains
CREATE OR REPLACE FUNCTION teams.get_user_access_domains(p_user_id UUID)
RETURNS VARCHAR(50)[] AS $$
    SELECT array_agg(DISTINCT unnest_domain)
    FROM teams.v_effective_memberships m
    CROSS JOIN LATERAL unnest(m.access_domains) as unnest_domain
    WHERE m.user_id = p_user_id;
$$ LANGUAGE SQL STABLE;

-- Check if user has specific access domain
CREATE OR REPLACE FUNCTION teams.user_has_domain(p_user_id UUID, p_domain VARCHAR(50))
RETURNS BOOLEAN AS $$
    SELECT p_domain = ANY(teams.get_user_access_domains(p_user_id));
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

CREATE TRIGGER trg_teams_updated
    BEFORE UPDATE ON teams.teams
    FOR EACH ROW EXECUTE FUNCTION teams.update_timestamp();

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

CREATE TRIGGER trg_membership_history
    AFTER INSERT OR UPDATE ON teams.memberships
    FOR EACH ROW EXECUTE FUNCTION teams.log_membership_change();
```

---

## Part 2: Accounting Schema

### File: `rust/migrations/202412_accounting.sql`

```sql
-- =============================================================================
-- ACCOUNTING SCHEMA
-- =============================================================================

CREATE SCHEMA IF NOT EXISTS accounting;

-- -----------------------------------------------------------------------------
-- Service Contracts
-- -----------------------------------------------------------------------------

CREATE TABLE accounting.service_contracts (
    contract_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Parties
    client_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Contract details
    contract_reference VARCHAR(100),
    contract_type VARCHAR(50) NOT NULL DEFAULT 'MASTER',
    CONSTRAINT chk_contract_type CHECK (contract_type IN ('MASTER', 'ADDENDUM', 'SCHEDULE')),
    parent_contract_id UUID REFERENCES accounting.service_contracts(contract_id),
    
    -- Scope - which CBUs covered
    scope_type VARCHAR(50) NOT NULL DEFAULT 'ALL_CBUS',
    CONSTRAINT chk_scope_type CHECK (scope_type IN ('ALL_CBUS', 'EXPLICIT', 'BY_FILTER')),
    explicit_cbus UUID[],
    scope_filter JSONB,
    
    -- Dates
    effective_from DATE NOT NULL,
    effective_to DATE,
    
    -- Status
    status VARCHAR(50) DEFAULT 'ACTIVE',
    CONSTRAINT chk_status CHECK (status IN ('DRAFT', 'ACTIVE', 'SUSPENDED', 'TERMINATED')),
    
    -- Documents
    document_id UUID,  -- Link to document_catalog
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- -----------------------------------------------------------------------------
-- Fee Schedules
-- -----------------------------------------------------------------------------

CREATE TABLE accounting.fee_schedules (
    schedule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_id UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),
    
    -- What's being priced
    fee_type VARCHAR(100) NOT NULL,
    fee_category VARCHAR(50),
    
    -- Pricing model
    pricing_model VARCHAR(50) NOT NULL,
    CONSTRAINT chk_pricing_model CHECK (pricing_model IN (
        'FIXED', 'BPS_AUM', 'BPS_NAV', 'TIERED', 'PER_TRANSACTION', 'PER_ACCOUNT'
    )),
    pricing_config JSONB NOT NULL,
    
    -- Scope within contract
    applies_to_cbus UUID[],
    applies_to_products VARCHAR[],
    applies_to_instrument_classes VARCHAR[],
    
    -- Dates
    effective_from DATE NOT NULL,
    effective_to DATE,
    
    -- Minimums/maximums
    minimum_fee DECIMAL(18,2),
    maximum_fee DECIMAL(18,2),
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- -----------------------------------------------------------------------------
-- Invoices
-- -----------------------------------------------------------------------------

CREATE TABLE accounting.invoices (
    invoice_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Links
    contract_id UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),
    billing_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    
    -- Invoice details
    invoice_number VARCHAR(100) NOT NULL UNIQUE,
    invoice_date DATE NOT NULL,
    due_date DATE NOT NULL,
    
    -- Period
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    
    -- Currency and amounts
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    subtotal DECIMAL(18,2) NOT NULL,
    tax_amount DECIMAL(18,2) DEFAULT 0,
    total_amount DECIMAL(18,2) NOT NULL,
    
    -- Status
    status VARCHAR(50) DEFAULT 'DRAFT',
    CONSTRAINT chk_invoice_status CHECK (status IN (
        'DRAFT', 'PENDING_APPROVAL', 'ISSUED', 'SENT', 'PAID', 
        'PARTIALLY_PAID', 'OVERDUE', 'DISPUTED', 'CANCELLED', 'WRITTEN_OFF'
    )),
    
    -- Payment tracking
    paid_amount DECIMAL(18,2) DEFAULT 0,
    paid_date DATE,
    payment_reference VARCHAR(255),
    
    -- Document
    document_id UUID,
    
    -- Approval
    approved_by_user_id UUID,
    approved_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_invoice_contract ON accounting.invoices(contract_id);
CREATE INDEX idx_invoice_entity ON accounting.invoices(billing_entity_id);
CREATE INDEX idx_invoice_status ON accounting.invoices(status);
CREATE INDEX idx_invoice_period ON accounting.invoices(period_start, period_end);

-- -----------------------------------------------------------------------------
-- Invoice Lines
-- -----------------------------------------------------------------------------

CREATE TABLE accounting.invoice_lines (
    line_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id UUID NOT NULL REFERENCES accounting.invoices(invoice_id) ON DELETE CASCADE,
    
    -- What
    fee_schedule_id UUID REFERENCES accounting.fee_schedules(schedule_id),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    
    -- Description
    description TEXT NOT NULL,
    fee_type VARCHAR(100),
    fee_category VARCHAR(50),
    
    -- Amounts
    quantity DECIMAL(18,4),
    unit_price DECIMAL(18,6),
    amount DECIMAL(18,2) NOT NULL,
    
    -- Calculation details (for audit)
    calculation_basis JSONB,
    
    line_order INTEGER DEFAULT 0
);

CREATE INDEX idx_invoice_line_invoice ON accounting.invoice_lines(invoice_id);

-- -----------------------------------------------------------------------------
-- Cost Allocations
-- -----------------------------------------------------------------------------

CREATE TABLE accounting.cost_allocations (
    allocation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Source
    invoice_id UUID REFERENCES accounting.invoices(invoice_id),
    invoice_line_id UUID REFERENCES accounting.invoice_lines(line_id),
    
    -- Target
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    
    -- Amount
    allocated_amount DECIMAL(18,2) NOT NULL,
    allocation_percentage DECIMAL(8,4),
    allocation_method VARCHAR(50),
    
    -- Period
    period_start DATE,
    period_end DATE,
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- -----------------------------------------------------------------------------
-- Invoice Contacts
-- -----------------------------------------------------------------------------

CREATE TABLE accounting.invoice_contacts (
    contact_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_id UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),
    
    -- Who
    team_id UUID REFERENCES teams.teams(team_id),
    user_id UUID REFERENCES client_portal.users(user_id),
    email VARCHAR(255),
    
    -- Role
    contact_role VARCHAR(50) DEFAULT 'RECIPIENT',
    CONSTRAINT chk_contact_role CHECK (contact_role IN (
        'PRIMARY', 'RECIPIENT', 'CC', 'APPROVER', 'ESCALATION'
    )),
    
    -- What they receive
    receives_invoices BOOLEAN DEFAULT TRUE,
    receives_statements BOOLEAN DEFAULT FALSE,
    receives_reminders BOOLEAN DEFAULT TRUE,
    
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Part 3: Verbs

### File: `rust/config/verbs/team.yaml`

```yaml
domains:
  team:
    description: "Team and organizational unit management"
    
    verbs:
      # =========================================================================
      # TEAM LIFECYCLE
      # =========================================================================
      
      create:
        description: "Create a new team with delegated authority"
        behavior: crud
        crud:
          operation: insert
          table: teams
          schema: teams
          returning: team_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: team-type
            type: string
            required: true
            maps_to: team_type
            valid_values: [fund-ops, manco-oversight, im-trading, spv-admin, client-service, accounting, reporting]
          - name: delegating-entity
            type: uuid
            required: true
            maps_to: delegating_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: authority-type
            type: string
            required: true
            maps_to: authority_type
            valid_values: [operational, oversight, trading, administrative]
          - name: access-mode
            type: string
            required: true
            maps_to: access_mode
            valid_values: [explicit, by-manco, by-im, by-filter]
          - name: authority-scope
            type: json
            required: false
            maps_to: authority_scope
        produces:
          binding: team
          type: uuid

      archive:
        description: "Archive team (soft delete)"
        behavior: crud
        crud:
          operation: update
          table: teams
          schema: teams
          key: team_id
          set_values:
            is_active: false
            archived_at: now()
        args:
          - name: team
            type: uuid
            required: true
            maps_to: team_id
            lookup:
              table: teams
              entity_type: team
              schema: teams
              search_key: name
              primary_key: team_id
          - name: reason
            type: string
            required: true
            maps_to: archive_reason
        effects:
          - "Ends all active memberships"
          - "Revokes active sessions"

      # =========================================================================
      # MEMBERSHIP
      # =========================================================================

      add-member:
        description: "Add user to team with role"
        behavior: crud
        crud:
          operation: upsert
          table: memberships
          schema: teams
          conflict_keys: [team_id, user_id, role_key]
          returning: membership_id
        args:
          - name: team
            type: uuid
            required: true
            maps_to: team_id
            lookup:
              table: teams
              entity_type: team
              schema: teams
              search_key: name
              primary_key: team_id
          - name: user
            type: uuid
            required: true
            maps_to: user_id
            lookup:
              table: users
              entity_type: user
              schema: client_portal
              search_key: email
              primary_key: user_id
          - name: role
            type: string
            required: true
            maps_to: role_key
            description: "Role key: {team-type}.{function}:{level}"
          - name: effective-from
            type: date
            required: false
            maps_to: effective_from
          - name: effective-to
            type: date
            required: false
            maps_to: effective_to
        produces:
          binding: membership
          type: uuid

      remove-member:
        description: "Remove user from team (or specific role)"
        behavior: plugin
        handler: team_remove_member
        args:
          - name: team
            type: uuid
            required: true
            lookup:
              table: teams
              entity_type: team
              schema: teams
              search_key: name
              primary_key: team_id
          - name: user
            type: uuid
            required: true
            lookup:
              table: users
              entity_type: user
              schema: client_portal
              search_key: email
              primary_key: user_id
          - name: role
            type: string
            required: false
            description: "If omitted, removes ALL roles in team"
          - name: reason
            type: string
            required: false
        effects:
          - "Sets effective_to = today on matching memberships"
          - "Logs to membership_history"

      update-member:
        description: "Update member's role"
        behavior: plugin
        handler: team_update_member
        args:
          - name: team
            type: uuid
            required: true
          - name: user
            type: uuid
            required: true
          - name: old-role
            type: string
            required: false
          - name: new-role
            type: string
            required: true
          - name: reason
            type: string
            required: false

      transfer-member:
        description: "Move user from one team to another"
        behavior: plugin
        handler: team_transfer_member
        args:
          - name: from-team
            type: uuid
            required: true
          - name: to-team
            type: uuid
            required: true
          - name: user
            type: uuid
            required: true
          - name: new-role
            type: string
            required: true
          - name: reason
            type: string
            required: false
        effects:
          - "Atomic: removes from source, adds to target"

      # =========================================================================
      # CBU ACCESS
      # =========================================================================

      add-cbu-access:
        description: "Add CBU to team's explicit access list"
        behavior: crud
        crud:
          operation: upsert
          table: team_cbu_access
          schema: teams
          conflict_keys: [team_id, cbu_id]
          returning: access_id
        args:
          - name: team
            type: uuid
            required: true
            maps_to: team_id
          - name: cbu
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id

      remove-cbu-access:
        description: "Remove CBU from team's access"
        behavior: crud
        crud:
          operation: delete
          table: team_cbu_access
          schema: teams
        args:
          - name: team
            type: uuid
            required: true
            maps_to: team_id
          - name: cbu
            type: uuid
            required: true
            maps_to: cbu_id

      # =========================================================================
      # ENTITLEMENTS
      # =========================================================================

      grant-service:
        description: "Grant service entitlement to team"
        behavior: plugin
        handler: team_grant_service
        args:
          - name: team
            type: uuid
            required: true
          - name: service
            type: string
            required: true
            valid_values: [client-portal, reporting-portal, repair-queue, trade-instruction]
          - name: config
            type: json
            required: false

      revoke-service:
        description: "Revoke service entitlement"
        behavior: plugin
        handler: team_revoke_service
        args:
          - name: team
            type: uuid
            required: true
          - name: service
            type: string
            required: true

      # =========================================================================
      # QUERIES
      # =========================================================================

      list-members:
        description: "List team members"
        behavior: crud
        crud:
          operation: list_by_fk
          table: v_effective_memberships
          schema: teams
          fk_col: team_id
        args:
          - name: team
            type: uuid
            required: true
          - name: role-pattern
            type: string
            required: false
          - name: include-inactive
            type: boolean
            default: false

      list-cbus:
        description: "List CBUs team can access"
        behavior: plugin
        handler: team_list_cbus
        args:
          - name: team
            type: uuid
            required: true

  # ===========================================================================
  # USER DOMAIN
  # ===========================================================================
  
  user:
    description: "User lifecycle management"
    
    verbs:
      create:
        description: "Create portal user"
        behavior: crud
        crud:
          operation: insert
          table: users
          schema: client_portal
          returning: user_id
        args:
          - name: email
            type: string
            required: true
            maps_to: email
          - name: name
            type: string
            required: true
            maps_to: name
          - name: employer
            type: uuid
            required: false
            maps_to: employer_entity_id
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: identity-provider
            type: string
            required: false
            maps_to: identity_provider
            default: local
        produces:
          binding: user
          type: uuid

      suspend:
        description: "Suspend user access"
        behavior: crud
        crud:
          operation: update
          table: users
          schema: client_portal
          key: user_id
          set_values:
            status: SUSPENDED
        args:
          - name: user
            type: uuid
            required: true
            maps_to: user_id

      reactivate:
        description: "Reactivate suspended user"
        behavior: crud
        crud:
          operation: update
          table: users
          schema: client_portal
          key: user_id
          set_values:
            status: ACTIVE
        args:
          - name: user
            type: uuid
            required: true
            maps_to: user_id

      offboard:
        description: "Offboard user completely (left company)"
        behavior: plugin
        handler: user_offboard
        args:
          - name: user
            type: uuid
            required: true
            lookup:
              table: users
              entity_type: user
              schema: client_portal
              search_key: email
              primary_key: user_id
          - name: reason
            type: string
            required: true
            valid_values: [resigned, terminated, retired, deceased, other]
          - name: notes
            type: string
            required: false
        effects:
          - "Sets status = OFFBOARDED"
          - "Ends ALL team memberships"
          - "Revokes all active sessions"
          - "Logs full audit trail"

      list-teams:
        description: "List user's team memberships"
        behavior: crud
        crud:
          operation: list_by_fk
          table: v_effective_memberships
          schema: teams
          fk_col: user_id
        args:
          - name: user
            type: uuid
            required: true

      list-cbus:
        description: "List all CBUs user can access"
        behavior: plugin
        handler: user_list_cbus
        args:
          - name: user
            type: uuid
            required: true

      check-access:
        description: "Check user access to specific CBU"
        behavior: plugin
        handler: user_check_access
        args:
          - name: user
            type: uuid
            required: true
          - name: cbu
            type: uuid
            required: true
        returns:
          type: object
          description: "{ has_access, via_teams, roles, access_domains }"
```

---

## Part 4: Plugin Handlers

### File: `rust/src/dsl_v2/custom_ops/team_ops.rs`

```rust
//! Team and User management operations

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;

use crate::dsl_v2::custom_ops::{CustomOp, CustomOpContext, CustomOpResult};

/// Remove member from team
pub struct TeamRemoveMember;

#[async_trait]
impl CustomOp for TeamRemoveMember {
    fn name(&self) -> &'static str { "team_remove_member" }
    
    async fn execute(&self, ctx: &mut CustomOpContext) -> CustomOpResult {
        let team_id: Uuid = ctx.require_arg("team")?;
        let user_id: Uuid = ctx.require_arg("user")?;
        let role: Option<String> = ctx.get_arg("role")?;
        let reason: Option<String> = ctx.get_arg("reason")?;
        
        let today = Utc::now().date_naive();
        
        // End matching memberships
        let affected = if let Some(role_key) = role {
            sqlx::query!(
                r#"
                UPDATE teams.memberships 
                SET effective_to = $1, updated_at = NOW()
                WHERE team_id = $2 AND user_id = $3 AND role_key = $4
                  AND (effective_to IS NULL OR effective_to > $1)
                "#,
                today, team_id, user_id, role_key
            )
            .execute(&ctx.pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query!(
                r#"
                UPDATE teams.memberships 
                SET effective_to = $1, updated_at = NOW()
                WHERE team_id = $2 AND user_id = $3
                  AND (effective_to IS NULL OR effective_to > $1)
                "#,
                today, team_id, user_id
            )
            .execute(&ctx.pool)
            .await?
            .rows_affected()
        };
        
        // Log history
        sqlx::query!(
            r#"
            INSERT INTO teams.membership_history 
                (membership_id, team_id, user_id, action, reason, changed_at)
            SELECT membership_id, team_id, user_id, 'REMOVED', $4, NOW()
            FROM teams.memberships
            WHERE team_id = $1 AND user_id = $2 AND effective_to = $3
            "#,
            team_id, user_id, today, reason
        )
        .execute(&ctx.pool)
        .await?;
        
        Ok(serde_json::json!({
            "affected": affected,
            "team_id": team_id,
            "user_id": user_id
        }))
    }
}

/// Transfer member between teams
pub struct TeamTransferMember;

#[async_trait]
impl CustomOp for TeamTransferMember {
    fn name(&self) -> &'static str { "team_transfer_member" }
    
    async fn execute(&self, ctx: &mut CustomOpContext) -> CustomOpResult {
        let from_team: Uuid = ctx.require_arg("from-team")?;
        let to_team: Uuid = ctx.require_arg("to-team")?;
        let user_id: Uuid = ctx.require_arg("user")?;
        let new_role: String = ctx.require_arg("new-role")?;
        let reason: Option<String> = ctx.get_arg("reason")?;
        
        let today = Utc::now().date_naive();
        
        // Atomic transaction
        let mut tx = ctx.pool.begin().await?;
        
        // 1. End all memberships in source team
        sqlx::query!(
            r#"
            UPDATE teams.memberships 
            SET effective_to = $1, updated_at = NOW()
            WHERE team_id = $2 AND user_id = $3
              AND (effective_to IS NULL OR effective_to > $1)
            "#,
            today, from_team, user_id
        )
        .execute(&mut *tx)
        .await?;
        
        // 2. Add to target team
        let membership_id = sqlx::query_scalar!(
            r#"
            INSERT INTO teams.memberships (team_id, user_id, role_key, effective_from)
            VALUES ($1, $2, $3, $4)
            RETURNING membership_id
            "#,
            to_team, user_id, new_role, today
        )
        .fetch_one(&mut *tx)
        .await?;
        
        // 3. Log transfer
        sqlx::query!(
            r#"
            INSERT INTO teams.membership_history 
                (membership_id, team_id, user_id, action, new_role_key, reason, changed_at)
            VALUES ($1, $2, $3, 'TRANSFERRED', $4, $5, NOW())
            "#,
            membership_id, to_team, user_id, new_role, reason
        )
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        Ok(serde_json::json!({
            "membership_id": membership_id,
            "from_team": from_team,
            "to_team": to_team,
            "new_role": new_role
        }))
    }
}

/// Offboard user completely
pub struct UserOffboard;

#[async_trait]
impl CustomOp for UserOffboard {
    fn name(&self) -> &'static str { "user_offboard" }
    
    async fn execute(&self, ctx: &mut CustomOpContext) -> CustomOpResult {
        let user_id: Uuid = ctx.require_arg("user")?;
        let reason: String = ctx.require_arg("reason")?;
        let notes: Option<String> = ctx.get_arg("notes")?;
        
        let today = Utc::now().date_naive();
        let now = Utc::now();
        
        let mut tx = ctx.pool.begin().await?;
        
        // 1. Update user status
        sqlx::query!(
            r#"
            UPDATE client_portal.users 
            SET status = 'OFFBOARDED', 
                offboarded_at = $2,
                offboard_reason = $3,
                updated_at = NOW()
            WHERE user_id = $1
            "#,
            user_id, now, reason
        )
        .execute(&mut *tx)
        .await?;
        
        // 2. End ALL team memberships
        let affected = sqlx::query!(
            r#"
            UPDATE teams.memberships 
            SET effective_to = $1, updated_at = NOW()
            WHERE user_id = $2
              AND (effective_to IS NULL OR effective_to > $1)
            "#,
            today, user_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        
        // 3. Log to history
        sqlx::query!(
            r#"
            INSERT INTO teams.membership_history 
                (membership_id, team_id, user_id, action, reason, changed_at)
            SELECT membership_id, team_id, user_id, 'REMOVED', $2, NOW()
            FROM teams.memberships
            WHERE user_id = $1 AND effective_to = $3
            "#,
            user_id, 
            format!("OFFBOARDED: {} - {}", reason, notes.unwrap_or_default()),
            today
        )
        .execute(&mut *tx)
        .await?;
        
        // 4. Revoke sessions (if session table exists)
        let _ = sqlx::query!(
            "DELETE FROM client_portal.sessions WHERE client_id = $1",
            user_id
        )
        .execute(&mut *tx)
        .await;
        
        tx.commit().await?;
        
        Ok(serde_json::json!({
            "user_id": user_id,
            "status": "OFFBOARDED",
            "memberships_ended": affected,
            "reason": reason
        }))
    }
}

/// Get user's CBU access with domains
pub struct UserListCbus;

#[async_trait]
impl CustomOp for UserListCbus {
    fn name(&self) -> &'static str { "user_list_cbus" }
    
    async fn execute(&self, ctx: &mut CustomOpContext) -> CustomOpResult {
        let user_id: Uuid = ctx.require_arg("user")?;
        
        let cbus = sqlx::query!(
            r#"
            SELECT 
                cbu_id,
                cbu_name,
                access_domains,
                via_teams,
                roles
            FROM teams.v_user_cbu_access
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(&ctx.pool)
        .await?;
        
        Ok(serde_json::json!({
            "user_id": user_id,
            "cbu_count": cbus.len(),
            "cbus": cbus.into_iter().map(|c| serde_json::json!({
                "cbu_id": c.cbu_id,
                "cbu_name": c.cbu_name,
                "access_domains": c.access_domains,
                "via_teams": c.via_teams,
                "roles": c.roles
            })).collect::<Vec<_>>()
        }))
    }
}

/// Check user access to specific CBU
pub struct UserCheckAccess;

#[async_trait]
impl CustomOp for UserCheckAccess {
    fn name(&self) -> &'static str { "user_check_access" }
    
    async fn execute(&self, ctx: &mut CustomOpContext) -> CustomOpResult {
        let user_id: Uuid = ctx.require_arg("user")?;
        let cbu_id: Uuid = ctx.require_arg("cbu")?;
        
        let access = sqlx::query!(
            r#"
            SELECT 
                access_domains,
                via_teams,
                roles
            FROM teams.v_user_cbu_access
            WHERE user_id = $1 AND cbu_id = $2
            "#,
            user_id, cbu_id
        )
        .fetch_optional(&ctx.pool)
        .await?;
        
        Ok(match access {
            Some(a) => serde_json::json!({
                "has_access": true,
                "cbu_id": cbu_id,
                "access_domains": a.access_domains,
                "via_teams": a.via_teams,
                "roles": a.roles
            }),
            None => serde_json::json!({
                "has_access": false,
                "cbu_id": cbu_id
            })
        })
    }
}
```

---

## Part 5: Visualizer Integration

### Updates needed in visualizer:

```rust
// In visualizer/src/app.rs or similar

/// Filter graph based on user's access domains
fn filter_for_access_domains(
    graph: &EntityGraph, 
    domains: &[AccessDomain]
) -> EntityGraph {
    graph.filter(|node| {
        match node.category {
            // KYC nodes
            NodeCategory::Entity | 
            NodeCategory::Ownership | 
            NodeCategory::Document |
            NodeCategory::KycCase => domains.contains(&AccessDomain::KYC),
            
            // Trading nodes
            NodeCategory::TradingProfile |
            NodeCategory::Ssi |
            NodeCategory::Isda |
            NodeCategory::Position => domains.contains(&AccessDomain::TRADING),
            
            // Accounting nodes
            NodeCategory::Contract |
            NodeCategory::Invoice |
            NodeCategory::FeeSchedule => domains.contains(&AccessDomain::ACCOUNTING),
            
            // Shared (CBU visible to all with any access)
            NodeCategory::Cbu => true,
        }
    })
}

/// Determine available views for user
fn available_views(domains: &[AccessDomain]) -> Vec<ViewType> {
    let mut views = vec![ViewType::Overview];  // Everyone gets overview
    
    if domains.contains(&AccessDomain::KYC) {
        views.extend([
            ViewType::EntityGraph,
            ViewType::UboGraph,
            ViewType::DocumentCatalog,
            ViewType::CaseTimeline,
        ]);
    }
    
    if domains.contains(&AccessDomain::TRADING) {
        views.extend([
            ViewType::TradingMatrix,
            ViewType::SsiMap,
            ViewType::PositionSummary,
        ]);
    }
    
    if domains.contains(&AccessDomain::ACCOUNTING) {
        views.extend([
            ViewType::InvoiceHistory,
            ViewType::ContractSummary,
        ]);
    }
    
    views
}
```

---

## Verification

```bash
# Run migrations
cd /Users/adamtc007/Developer/ob-poc/rust
psql -d ob-poc -f migrations/202412_teams.sql
psql -d ob-poc -f migrations/202412_accounting.sql

# Compile
cargo check --lib

# Test
cargo test team_ops -- --nocapture
cargo test user_ops -- --nocapture
```

---

## Summary

| Component | Purpose |
|-----------|---------|
| **teams.teams** | Organizational units with delegated authority |
| **teams.memberships** | User role assignments with composite keys |
| **teams.function_domains** | Maps function → access domains |
| **accounting.*** | Invoicing, contracts, fee schedules |
| **Access Domains** | KYC, TRADING, ACCOUNTING, REPORTING |
| **Role Key** | `{team_type}.{function}:{level}` |

**Key Flows:**
- User login → resolve team memberships → derive access domains → filter views
- Hans leaves → `user.offboard` → all memberships ended → sessions revoked
- New team member → `team.add-member :role "fund-ops.settlement:operator"`
- Visualizer checks `access_domains` before showing Trading Matrix vs UBO Graph


---

## Part 6: Governance Teams & Legal Appointment Linkage

### Overview

Governance teams (Board, IC, Conducting Officers) require linkage to legal appointments for audit purposes. Two distinct records:

1. **Legal Position** - Filed with regulator, fiduciary responsibility
2. **Portal Access** - What they can see/do, audit trail

```
┌─────────────────────────────────────────────────────────────────┐
│  LEGAL POSITION                    PORTAL ACCESS                 │
│  (cbu.assign-role)                 (team.add-member)            │
│                                                                  │
│  Entity: John Smith Ltd     →      User: john@example.com       │
│  Role: DIRECTOR                    Role: governance.board:member│
│  CBU: Apex Fund                    Team: Apex Fund Board        │
│  Effective: 2024-01-01             Linked to legal appointment  │
│                                                                  │
│  Regulatory record                 Access control + audit       │
└─────────────────────────────────────────────────────────────────┘
```

### Extended Team Types

Add to `teams.teams.team_type` constraint:

```sql
ALTER TABLE teams.teams 
DROP CONSTRAINT IF EXISTS chk_team_type;

ALTER TABLE teams.teams 
ADD CONSTRAINT chk_team_type CHECK (team_type IN (
    -- Operational
    'fund-ops', 
    'manco-oversight', 
    'im-trading', 
    'spv-admin', 
    'client-service', 
    'accounting', 
    'reporting',
    -- Governance
    'board',
    'investment-committee',
    'conducting-officers',
    'executive'
));
```

### Governance Functions

Add to `teams.function_domains`:

```sql
INSERT INTO teams.function_domains (function_name, access_domains, description) VALUES
    -- Board functions
    ('board', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Board-level oversight'),
    ('board-secretary', ARRAY['KYC', 'REPORTING'], 'Board administration'),
    
    -- Investment Committee
    ('investment-decision', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Investment approval authority'),
    
    -- Conducting Officers (ManCo)
    ('conducting', ARRAY['KYC', 'REPORTING'], 'UCITS/AIFMD designated person'),
    ('risk-oversight', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Risk function oversight'),
    ('compliance-oversight', ARRAY['KYC', 'REPORTING'], 'Compliance function oversight'),
    
    -- Executive
    ('cio', ARRAY['TRADING', 'REPORTING'], 'Chief Investment Officer'),
    ('coo', ARRAY['KYC', 'TRADING', 'ACCOUNTING', 'REPORTING'], 'Chief Operating Officer'),
    ('cfo', ARRAY['ACCOUNTING', 'REPORTING'], 'Chief Financial Officer'),
    ('cro', ARRAY['KYC', 'TRADING', 'REPORTING'], 'Chief Risk Officer');
```

### Governance Levels

Standard levels for governance roles:

| Level | Description |
|-------|-------------|
| `chair` | Leads the body (Board Chair, IC Chair) |
| `member` | Voting member |
| `observer` | Non-voting observer |
| `delegate` | Acting on behalf of (delegated authority) |
| `secretary` | Admin/minutes, no vote |

### Legal Appointment Linkage

#### Schema Changes

```sql
-- Add legal appointment linkage to memberships
ALTER TABLE teams.memberships
    ADD COLUMN legal_appointment_id UUID,
    ADD COLUMN requires_legal_appointment BOOLEAN DEFAULT FALSE;

-- Reference to cbu_roles (the legal appointment record)
-- Note: Can't add FK if cbu_roles doesn't exist yet, add after
-- ALTER TABLE teams.memberships 
--     ADD CONSTRAINT fk_legal_appointment 
--     FOREIGN KEY (legal_appointment_id) 
--     REFERENCES "ob-poc".cbu_roles(role_id);

-- Governance teams should flag if legal appointment required
COMMENT ON COLUMN teams.memberships.legal_appointment_id IS 
    'Links portal access to legal appointment (DIRECTOR, CONDUCTING_OFFICER, etc.)';
COMMENT ON COLUMN teams.memberships.requires_legal_appointment IS 
    'If true, warns when no legal appointment linked';

-- View to show governance access with legal positions
CREATE OR REPLACE VIEW teams.v_governance_access AS
SELECT 
    m.membership_id,
    m.user_id,
    u.name as user_name,
    u.email as user_email,
    m.team_id,
    t.name as team_name,
    t.team_type,
    m.role_key,
    m.function_name,
    m.role_level,
    fd.access_domains,
    -- Legal appointment details
    m.legal_appointment_id,
    cr.role_type as legal_position,
    cr.effective_from as legal_effective_from,
    cr.effective_to as legal_effective_to,
    e.name as legal_entity_name,
    -- Warning flag
    CASE 
        WHEN m.requires_legal_appointment AND m.legal_appointment_id IS NULL 
        THEN TRUE 
        ELSE FALSE 
    END as missing_legal_appointment
FROM teams.memberships m
JOIN teams.teams t ON m.team_id = t.team_id
JOIN client_portal.users u ON m.user_id = u.user_id
LEFT JOIN teams.function_domains fd ON m.function_name = fd.function_name
LEFT JOIN "ob-poc".cbu_roles cr ON m.legal_appointment_id = cr.role_id
LEFT JOIN "ob-poc".entities e ON cr.entity_id = e.entity_id
WHERE t.team_type IN ('board', 'investment-committee', 'conducting-officers', 'executive')
  AND t.is_active = TRUE
  AND u.status = 'ACTIVE'
  AND m.effective_from <= CURRENT_DATE
  AND (m.effective_to IS NULL OR m.effective_to >= CURRENT_DATE);
```

### Governance Verb Extensions

Add to `rust/config/verbs/team.yaml`:

```yaml
      # =========================================================================
      # GOVERNANCE MEMBERSHIP (with legal linkage)
      # =========================================================================

      add-governance-member:
        description: "Add governance member with optional legal appointment link"
        behavior: plugin
        handler: team_add_governance_member
        args:
          - name: team
            type: uuid
            required: true
            lookup:
              table: teams
              entity_type: team
              schema: teams
              search_key: name
              primary_key: team_id
          - name: user
            type: uuid
            required: true
            lookup:
              table: users
              entity_type: user
              schema: client_portal
              search_key: email
              primary_key: user_id
          - name: role
            type: string
            required: true
            description: "Governance role key: governance.{function}:{level}"
          - name: legal-appointment
            type: uuid
            required: false
            description: "Link to cbu_roles record (DIRECTOR, CONDUCTING_OFFICER, etc.)"
          - name: require-legal-link
            type: boolean
            default: true
            description: "Warn if no legal appointment provided"
        effects:
          - "Creates membership with legal_appointment_id"
          - "Validates legal appointment is active and for correct CBU"
          - "Warns if require-legal-link=true and no appointment provided"
        produces:
          binding: membership
          type: uuid

      verify-governance-access:
        description: "Audit governance access against legal appointments"
        behavior: plugin
        handler: team_verify_governance_access
        args:
          - name: team
            type: uuid
            required: false
            description: "Specific team (optional - all governance teams if omitted)"
          - name: as-of-date
            type: date
            required: false
            description: "Point-in-time check (default: today)"
        returns:
          type: object
          description: "{ valid: [...], warnings: [...], expired_appointments: [...] }"
```

### Example Role Keys

```
# Board
governance.board:chair              # Board chair
governance.board:member             # Board director
governance.board:observer           # Non-voting observer
governance.board-secretary:member   # Board secretary

# Investment Committee
governance.investment-decision:chair    # IC chair
governance.investment-decision:member   # IC voting member
governance.investment-decision:observer # IC observer

# Conducting Officers (ManCo)
governance.conducting:member            # Designated conducting officer
governance.risk-oversight:member        # Risk officer
governance.compliance-oversight:member  # Compliance officer

# Executive
governance.cio:member    # Chief Investment Officer
governance.coo:member    # Chief Operating Officer
governance.cfo:member    # Chief Financial Officer
governance.cro:member    # Chief Risk Officer
```

### Example Flows

```clojure
;; ============================================================
;; SCENARIO: Appoint John Smith as Apex Fund director
;; ============================================================

;; 1. Legal appointment (regulatory record)
(cbu.assign-role 
  :cbu-id @apex-fund 
  :entity-id @john-smith-entity  ;; John's legal entity
  :role "DIRECTOR"
  :effective-from "2024-01-01"
  :appointed-by "Board Resolution 2024-001")
→ @john-director-appointment

;; 2. Portal access (linked to legal)
(team.add-governance-member 
  :team @apex-board 
  :user @john-smith-user  ;; John's login
  :role "governance.board:member"
  :legal-appointment @john-director-appointment)
→ @john-board-membership

;; ============================================================
;; SCENARIO: John resigns from board
;; ============================================================

;; 1. End legal appointment
(cbu.end-role 
  :role-assignment @john-director-appointment
  :effective-to "2024-12-31"
  :reason "Resignation")

;; 2. End portal access
(team.remove-member 
  :team @apex-board 
  :user @john-smith-user
  :reason "Director resignation")

;; ============================================================
;; SCENARIO: Audit governance access
;; ============================================================

(team.verify-governance-access :as-of-date "2024-09-30")
→ {
    "valid": [
      { "user": "john@example.com", "role": "governance.board:member", 
        "legal_position": "DIRECTOR", "cbu": "Apex Fund" }
    ],
    "warnings": [
      { "user": "temp@example.com", "role": "governance.board:observer",
        "issue": "No legal appointment linked" }
    ],
    "expired_appointments": [
      { "user": "former@example.com", "role": "governance.board:member",
        "legal_expired": "2024-06-30", "access_still_active": true }
    ]
  }
```

### Audit Query

When regulator asks: "Who had access to Apex Fund data in Q3 2024 and what was their legal authority?"

```sql
SELECT 
    u.name as user_name,
    u.email,
    m.role_key as portal_role,
    t.name as team_name,
    cr.role_type as legal_position,
    e.name as legal_entity,
    cr.effective_from as legal_from,
    cr.effective_to as legal_to,
    m.effective_from as access_from,
    m.effective_to as access_to,
    fd.access_domains,
    -- Flag any issues
    CASE 
        WHEN m.legal_appointment_id IS NULL THEN 'NO_LEGAL_LINK'
        WHEN cr.effective_to < '2024-07-01' THEN 'LEGAL_EXPIRED_BEFORE_PERIOD'
        ELSE 'OK'
    END as audit_status
FROM teams.memberships m
JOIN teams.teams t ON m.team_id = t.team_id
JOIN client_portal.users u ON m.user_id = u.user_id
LEFT JOIN teams.function_domains fd ON m.function_name = fd.function_name
LEFT JOIN "ob-poc".cbu_roles cr ON m.legal_appointment_id = cr.role_id
LEFT JOIN "ob-poc".entities e ON cr.entity_id = e.entity_id
WHERE t.delegating_entity_id = (SELECT entity_id FROM "ob-poc".cbus WHERE name = 'Apex Fund')
  AND t.team_type IN ('board', 'investment-committee', 'conducting-officers', 'executive')
  -- Active during Q3 2024
  AND m.effective_from <= '2024-09-30'
  AND (m.effective_to IS NULL OR m.effective_to >= '2024-07-01')
ORDER BY t.team_type, m.role_key, u.name;
```

### Summary: Governance Model

| Concept | Purpose |
|---------|---------|
| **Legal Appointment** | Regulatory record (cbu_roles) - "John is a Director" |
| **Portal Access** | Access control (memberships) - "John can view board materials" |
| **Legal Link** | Proves access derived from legal authority |
| **Audit View** | Shows who had what access with what authority |
| **Verification** | Flags expired appointments, missing links |

**Team Types:**
- `board` - Fund/ManCo directors
- `investment-committee` - IC members  
- `conducting-officers` - ManCo designated persons
- `executive` - CIO/COO/CFO/CRO

**Levels:**
- `chair` - Leads body
- `member` - Voting member
- `observer` - Non-voting
- `delegate` - Acting on behalf of
- `secretary` - Admin

**Security Gate:** Every governance access is traceable to legal authority. Regulator can see exactly who had access, when, and by what legal right.


---

## Part 7: Access Review Automation

### Overview

Periodic access reviews are regulatory requirement but admin nightmare. Automate detection, workflow, attestation, and enforcement.

```
┌─────────────────────────────────────────────────────────────────┐
│                    AUTOMATED ACCESS REVIEW                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. DETECT     → Legal expired, dormant, no-link, mismatches   │
│  2. CAMPAIGN   → Group by reviewer, pre-populate recommendations│
│  3. REVIEW     → Confirm / Revoke / Extend (bulk or individual)│
│  4. ATTEST     → Digital signature on reviewed items            │
│  5. ENFORCE    → Auto-suspend unreviewed past deadline          │
│  6. AUDIT      → Full trail for regulator                       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Schema

```sql
-- =============================================================================
-- ACCESS REVIEW SCHEMA
-- =============================================================================

-- Review campaigns (quarterly, annual, triggered)
CREATE TABLE teams.access_review_campaigns (
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
    review_period_start DATE NOT NULL,
    review_period_end DATE NOT NULL,
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

-- Individual review items
CREATE TABLE teams.access_review_items (
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
    risk_score INTEGER DEFAULT 0,  -- 0-100, higher = more attention needed
    
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

CREATE INDEX idx_review_items_campaign ON teams.access_review_items(campaign_id);
CREATE INDEX idx_review_items_reviewer ON teams.access_review_items(reviewer_user_id);
CREATE INDEX idx_review_items_status ON teams.access_review_items(status);
CREATE INDEX idx_review_items_membership ON teams.access_review_items(membership_id);
CREATE INDEX idx_review_items_pending ON teams.access_review_items(campaign_id, status) 
    WHERE status = 'PENDING';
CREATE INDEX idx_review_items_flagged ON teams.access_review_items(campaign_id) 
    WHERE flag_no_legal_link OR flag_legal_expired OR flag_dormant_account;

-- Attestations
CREATE TABLE teams.access_attestations (
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
    signature_input TEXT,  -- What was hashed (for verification)
    
    -- Context
    ip_address INET,
    user_agent TEXT,
    session_id UUID
);

CREATE INDEX idx_attestations_campaign ON teams.access_attestations(campaign_id);
CREATE INDEX idx_attestations_attester ON teams.access_attestations(attester_user_id);

-- Review activity log (detailed audit trail)
CREATE TABLE teams.access_review_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    campaign_id UUID REFERENCES teams.access_review_campaigns(campaign_id),
    item_id UUID REFERENCES teams.access_review_items(item_id),
    
    -- Action
    action VARCHAR(50) NOT NULL,
    action_detail JSONB,
    
    -- Who
    actor_user_id UUID,
    actor_email VARCHAR(255),
    actor_type VARCHAR(50),  -- USER, SYSTEM, SCHEDULER
    
    -- When
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Context
    ip_address INET
);

CREATE INDEX idx_review_log_campaign ON teams.access_review_log(campaign_id);
CREATE INDEX idx_review_log_item ON teams.access_review_log(item_id);
CREATE INDEX idx_review_log_time ON teams.access_review_log(created_at);

-- =============================================================================
-- VIEWS
-- =============================================================================

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

-- =============================================================================
-- FUNCTIONS
-- =============================================================================

-- Populate campaign with review items
CREATE OR REPLACE FUNCTION teams.populate_review_campaign(p_campaign_id UUID)
RETURNS JSONB AS $$
DECLARE
    v_campaign RECORD;
    v_inserted INTEGER := 0;
    v_flagged INTEGER := 0;
BEGIN
    SELECT * INTO v_campaign FROM teams.access_review_campaigns WHERE campaign_id = p_campaign_id;
    
    -- Insert review items from effective memberships
    INSERT INTO teams.access_review_items (
        campaign_id, membership_id, user_id, team_id, role_key,
        user_name, user_email, team_name, team_type, delegating_entity_name,
        access_domains, legal_appointment_id, legal_position, legal_entity_name,
        legal_effective_from, legal_effective_to,
        last_login_at, days_since_login, membership_created_at, membership_age_days,
        flag_no_legal_link, flag_legal_expired, flag_legal_expiring_soon,
        flag_dormant_account, flag_never_logged_in,
        recommendation, recommendation_reason, risk_score,
        reviewer_user_id, reviewer_email, reviewer_name
    )
    SELECT 
        p_campaign_id,
        m.membership_id,
        m.user_id,
        m.team_id,
        m.role_key,
        u.name,
        u.email,
        t.name,
        t.team_type,
        e.name,
        fd.access_domains,
        m.legal_appointment_id,
        cr.role_type,
        le.name,
        cr.effective_from,
        cr.effective_to,
        u.last_login_at,
        EXTRACT(DAY FROM NOW() - u.last_login_at)::INTEGER,
        m.created_at,
        EXTRACT(DAY FROM NOW() - m.created_at)::INTEGER,
        -- Flags
        (t.team_type IN ('board', 'investment-committee', 'conducting-officers') 
            AND m.legal_appointment_id IS NULL),
        (cr.effective_to IS NOT NULL AND cr.effective_to < CURRENT_DATE),
        (cr.effective_to IS NOT NULL AND cr.effective_to BETWEEN CURRENT_DATE AND CURRENT_DATE + 30),
        (u.last_login_at < CURRENT_DATE - 90),
        (u.last_login_at IS NULL),
        -- Recommendation
        CASE 
            WHEN cr.effective_to < CURRENT_DATE THEN 'REVOKE'
            WHEN u.last_login_at < CURRENT_DATE - 90 THEN 'REVOKE'
            WHEN cr.effective_to BETWEEN CURRENT_DATE AND CURRENT_DATE + 30 THEN 'EXTEND'
            WHEN t.team_type IN ('board', 'investment-committee', 'conducting-officers') 
                 AND m.legal_appointment_id IS NULL THEN 'REVIEW'
            ELSE 'CONFIRM'
        END,
        -- Recommendation reason
        CASE 
            WHEN cr.effective_to < CURRENT_DATE THEN 'Legal appointment expired'
            WHEN u.last_login_at < CURRENT_DATE - 90 THEN 'No login in 90+ days'
            WHEN cr.effective_to BETWEEN CURRENT_DATE AND CURRENT_DATE + 30 THEN 'Legal appointment expiring soon'
            WHEN t.team_type IN ('board', 'investment-committee', 'conducting-officers') 
                 AND m.legal_appointment_id IS NULL THEN 'Governance role without legal appointment'
            ELSE 'No issues detected'
        END,
        -- Risk score
        CASE 
            WHEN cr.effective_to < CURRENT_DATE THEN 90
            WHEN u.last_login_at < CURRENT_DATE - 90 THEN 70
            WHEN t.team_type IN ('board', 'investment-committee', 'conducting-officers') 
                 AND m.legal_appointment_id IS NULL THEN 80
            WHEN cr.effective_to BETWEEN CURRENT_DATE AND CURRENT_DATE + 30 THEN 50
            ELSE 10
        END,
        -- Assign reviewer (team admin or delegating entity contact)
        -- Simplified: assign to first team admin found
        (SELECT tm.user_id FROM teams.memberships tm 
         WHERE tm.team_id = t.team_id AND tm.role_level = 'admin' 
         LIMIT 1),
        NULL, NULL
    FROM teams.v_effective_memberships m
    JOIN teams.teams t ON m.team_id = t.team_id
    JOIN client_portal.users u ON m.user_id = u.user_id
    JOIN "ob-poc".entities e ON t.delegating_entity_id = e.entity_id
    LEFT JOIN teams.function_domains fd ON m.function_name = fd.function_name
    LEFT JOIN "ob-poc".cbu_roles cr ON m.legal_appointment_id = cr.role_id
    LEFT JOIN "ob-poc".entities le ON cr.entity_id = le.entity_id
    WHERE 
        -- Apply scope filter
        (v_campaign.scope_type = 'ALL')
        OR (v_campaign.scope_type = 'GOVERNANCE_ONLY' 
            AND t.team_type IN ('board', 'investment-committee', 'conducting-officers', 'executive'))
        OR (v_campaign.scope_type = 'BY_TEAM_TYPE' 
            AND t.team_type = ANY(ARRAY(SELECT jsonb_array_elements_text(v_campaign.scope_filter->'team_types'))))
        OR (v_campaign.scope_type = 'SPECIFIC_TEAMS'
            AND t.team_id = ANY(ARRAY(SELECT (jsonb_array_elements_text(v_campaign.scope_filter->'team_ids'))::uuid)));
    
    GET DIAGNOSTICS v_inserted = ROW_COUNT;
    
    -- Count flagged
    SELECT COUNT(*) INTO v_flagged 
    FROM teams.access_review_items 
    WHERE campaign_id = p_campaign_id
      AND (flag_legal_expired OR flag_legal_expiring_soon OR flag_no_legal_link 
           OR flag_dormant_account OR flag_never_logged_in);
    
    -- Update campaign stats
    UPDATE teams.access_review_campaigns
    SET total_items = v_inserted,
        pending_items = v_inserted,
        status = 'ACTIVE'
    WHERE campaign_id = p_campaign_id;
    
    RETURN jsonb_build_object(
        'total_items', v_inserted,
        'flagged_items', v_flagged
    );
END;
$$ LANGUAGE plpgsql;

-- Process items past deadline
CREATE OR REPLACE FUNCTION teams.process_review_deadline(
    p_campaign_id UUID,
    p_action VARCHAR(50) DEFAULT 'SUSPEND'
)
RETURNS JSONB AS $$
DECLARE
    v_affected INTEGER := 0;
BEGIN
    IF p_action = 'SUSPEND' THEN
        -- Suspend unreviewed memberships
        UPDATE teams.memberships m
        SET effective_to = CURRENT_DATE,
            updated_at = NOW()
        FROM teams.access_review_items i
        WHERE i.membership_id = m.membership_id
          AND i.campaign_id = p_campaign_id
          AND i.status = 'PENDING';
        
        GET DIAGNOSTICS v_affected = ROW_COUNT;
        
        -- Mark items as auto-suspended
        UPDATE teams.access_review_items
        SET status = 'AUTO_SUSPENDED',
            auto_action_at = NOW(),
            auto_action_reason = 'Unreviewed past deadline - auto suspended'
        WHERE campaign_id = p_campaign_id
          AND status = 'PENDING';
    END IF;
    
    -- Update campaign
    UPDATE teams.access_review_campaigns
    SET status = 'COMPLETED',
        completed_at = NOW()
    WHERE campaign_id = p_campaign_id;
    
    RETURN jsonb_build_object(
        'action', p_action,
        'affected', v_affected
    );
END;
$$ LANGUAGE plpgsql;

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
    -- Build deterministic input string
    v_input := p_attester_id::text || '|' ||
               p_campaign_id::text || '|' ||
               array_to_string(p_item_ids, ',') || '|' ||
               p_attestation_text || '|' ||
               p_timestamp::text;
    
    -- Return SHA-256 hash
    RETURN 'sha256:' || encode(sha256(v_input::bytea), 'hex');
END;
$$ LANGUAGE plpgsql IMMUTABLE;
```

### Verbs

Add to `rust/config/verbs/access-review.yaml`:

```yaml
domains:
  access-review:
    description: "Periodic access review and attestation"
    
    verbs:
      # =========================================================================
      # CAMPAIGN LIFECYCLE
      # =========================================================================
      
      create-campaign:
        description: "Create access review campaign"
        behavior: crud
        crud:
          operation: insert
          table: access_review_campaigns
          schema: teams
          returning: campaign_id
        args:
          - name: name
            type: string
            required: true
            maps_to: name
          - name: review-type
            type: string
            required: true
            maps_to: review_type
            valid_values: [QUARTERLY, ANNUAL, TRIGGERED, JOINER_MOVER_LEAVER]
          - name: scope-type
            type: string
            required: true
            maps_to: scope_type
            valid_values: [ALL, BY_TEAM_TYPE, BY_DELEGATING_ENTITY, SPECIFIC_TEAMS, GOVERNANCE_ONLY]
          - name: scope-filter
            type: json
            required: false
            maps_to: scope_filter
          - name: deadline
            type: date
            required: true
            maps_to: deadline
          - name: review-period-start
            type: date
            required: false
            maps_to: review_period_start
          - name: review-period-end
            type: date
            required: false
            maps_to: review_period_end
        produces:
          binding: campaign
          type: uuid

      populate-campaign:
        description: "Populate campaign with review items"
        behavior: plugin
        handler: access_review_populate
        args:
          - name: campaign
            type: uuid
            required: true
        effects:
          - "Creates review items for in-scope memberships"
          - "Auto-detects flags and recommendations"
          - "Assigns reviewers"
        returns:
          type: object
          description: "{ total_items, flagged_items, by_flag: {...} }"

      launch-campaign:
        description: "Launch campaign and notify reviewers"
        behavior: plugin
        handler: access_review_launch
        args:
          - name: campaign
            type: uuid
            required: true
        effects:
          - "Sets status = ACTIVE"
          - "Sends email notifications"
          - "Schedules reminders"

      # =========================================================================
      # REVIEW ACTIONS
      # =========================================================================

      confirm-access:
        description: "Confirm access is appropriate"
        behavior: crud
        crud:
          operation: update
          table: access_review_items
          schema: teams
          key: item_id
          set_values:
            status: CONFIRMED
            reviewed_at: now()
        args:
          - name: item
            type: uuid
            required: true
            maps_to: item_id
          - name: notes
            type: string
            required: false
            maps_to: reviewer_notes

      revoke-access:
        description: "Revoke access"
        behavior: plugin
        handler: access_review_revoke
        args:
          - name: item
            type: uuid
            required: true
          - name: reason
            type: string
            required: true
          - name: effective-date
            type: date
            required: false
        effects:
          - "Marks item as REVOKED"
          - "Ends membership"
          - "Logs to history"

      extend-access:
        description: "Extend access with new end date"
        behavior: crud
        crud:
          operation: update
          table: access_review_items
          schema: teams
          key: item_id
          set_values:
            status: EXTENDED
            reviewed_at: now()
        args:
          - name: item
            type: uuid
            required: true
            maps_to: item_id
          - name: extend-to
            type: date
            required: true
            maps_to: extended_to
          - name: reason
            type: string
            required: true
            maps_to: extension_reason

      escalate-item:
        description: "Escalate for senior review"
        behavior: crud
        crud:
          operation: update
          table: access_review_items
          schema: teams
          key: item_id
          set_values:
            status: ESCALATED
            escalated_at: now()
        args:
          - name: item
            type: uuid
            required: true
            maps_to: item_id
          - name: escalate-to
            type: uuid
            required: false
            maps_to: escalated_to_user_id
          - name: reason
            type: string
            required: true
            maps_to: escalation_reason

      # =========================================================================
      # BULK OPERATIONS
      # =========================================================================

      bulk-confirm:
        description: "Confirm multiple items"
        behavior: plugin
        handler: access_review_bulk_confirm
        args:
          - name: items
            type: uuid_list
            required: true
          - name: notes
            type: string
            required: false
        returns:
          type: object
          description: "{ confirmed: N }"

      confirm-all-clean:
        description: "Confirm all unflagged items"
        behavior: plugin
        handler: access_review_confirm_clean
        args:
          - name: campaign
            type: uuid
            required: true
          - name: reviewer
            type: uuid
            required: false
        returns:
          type: object
          description: "{ confirmed: N, remaining: N }"

      # =========================================================================
      # ATTESTATION
      # =========================================================================

      attest:
        description: "Formal attestation of reviews"
        behavior: plugin
        handler: access_review_attest
        args:
          - name: campaign
            type: uuid
            required: true
          - name: scope
            type: string
            required: true
            valid_values: [FULL_CAMPAIGN, MY_REVIEWS, SPECIFIC_TEAM, SPECIFIC_ITEMS]
          - name: team
            type: uuid
            required: false
          - name: items
            type: uuid_list
            required: false
          - name: attestation-text
            type: string
            required: false
        effects:
          - "Creates attestation record"
          - "Generates signature hash"
          - "Logs IP/device"
        returns:
          type: object
          description: "{ attestation_id, items_count, signature }"

      # =========================================================================
      # DEADLINE PROCESSING
      # =========================================================================

      process-deadline:
        description: "Handle items past deadline"
        behavior: plugin
        handler: access_review_process_deadline
        args:
          - name: campaign
            type: uuid
            required: true
          - name: action
            type: string
            required: true
            valid_values: [suspend, escalate, report-only]
        effects:
          - "Suspends unreviewed if action=suspend"
          - "Notifies compliance"

      send-reminders:
        description: "Send reminder emails to reviewers with pending items"
        behavior: plugin
        handler: access_review_send_reminders
        args:
          - name: campaign
            type: uuid
            required: true

      # =========================================================================
      # QUERIES
      # =========================================================================

      my-pending:
        description: "Get my pending review items"
        behavior: plugin
        handler: access_review_my_pending
        args:
          - name: campaign
            type: uuid
            required: false
        returns:
          type: record_set

      campaign-status:
        description: "Campaign dashboard"
        behavior: plugin
        handler: access_review_status
        args:
          - name: campaign
            type: uuid
            required: true
        returns:
          type: object

      audit-report:
        description: "Generate audit report"
        behavior: plugin
        handler: access_review_audit_report
        args:
          - name: campaign
            type: uuid
            required: true
          - name: format
            type: string
            valid_values: [json, csv, pdf]
        returns:
          type: document
```

### Detection Rules

Configured rules that run during `populate-campaign`:

```yaml
# config/access_review_rules.yaml

detection_rules:
  - id: legal_expired
    name: "Legal Appointment Expired"
    description: "Legal appointment has ended but portal access continues"
    severity: CRITICAL
    flag_field: flag_legal_expired
    condition: |
      legal_effective_to IS NOT NULL 
      AND legal_effective_to < CURRENT_DATE
    recommendation: REVOKE
    risk_score: 90

  - id: legal_expiring_soon
    name: "Legal Appointment Expiring Soon"
    description: "Legal appointment expires within 30 days"
    severity: MEDIUM
    flag_field: flag_legal_expiring_soon
    condition: |
      legal_effective_to IS NOT NULL 
      AND legal_effective_to BETWEEN CURRENT_DATE AND CURRENT_DATE + 30
    recommendation: EXTEND
    risk_score: 50

  - id: no_legal_link
    name: "Governance Without Legal Link"
    description: "Governance role without linked legal appointment"
    severity: HIGH
    flag_field: flag_no_legal_link
    condition: |
      team_type IN ('board', 'investment-committee', 'conducting-officers')
      AND legal_appointment_id IS NULL
    recommendation: REVIEW
    risk_score: 80

  - id: dormant_90_days
    name: "Dormant Account (90+ days)"
    description: "No login activity in 90 days"
    severity: MEDIUM
    flag_field: flag_dormant_account
    condition: |
      last_login_at < CURRENT_DATE - 90
    recommendation: REVOKE
    risk_score: 70

  - id: never_logged_in
    name: "Never Logged In"
    description: "Account created but never accessed"
    severity: LOW
    flag_field: flag_never_logged_in
    condition: |
      last_login_at IS NULL
      AND membership_age_days > 30
    recommendation: REVIEW
    risk_score: 40

  - id: excessive_access
    name: "Excessive Access Breadth"
    description: "User has access to unusually many CBUs"
    severity: LOW
    flag_field: null
    flags_json_key: excessive_access
    condition: |
      (SELECT COUNT(DISTINCT cbu_id) 
       FROM teams.v_user_cbu_access 
       WHERE user_id = membership.user_id) > 20
    recommendation: REVIEW
    risk_score: 30
```

### Standard Attestation Text

```yaml
attestation_templates:
  standard:
    text: |
      I, {attester_name}, hereby attest that I have reviewed the access rights 
      listed in this campaign and confirm that:
      
      1. Each confirmed access is necessary for the user's current role
      2. Each confirmed access is appropriate given the user's legal authority
      3. I have revoked or flagged any access that is no longer required
      4. To the best of my knowledge, the reviewed access rights comply with 
         applicable policies and regulations
      
      Date: {date}
      Items reviewed: {items_count}
      Campaign: {campaign_name}
    
  governance:
    text: |
      I, {attester_name}, in my capacity as {attester_role}, hereby attest that 
      I have reviewed the governance access rights listed in this campaign and confirm that:
      
      1. Each confirmed access is linked to a valid legal appointment
      2. Each user's portal access is consistent with their fiduciary duties
      3. Access for expired appointments has been revoked
      4. The access rights comply with UCITS/AIFMD requirements as applicable
      
      Date: {date}
      Items reviewed: {items_count}
```

### Example Review Flow

```clojure
;; ============================================================
;; QUARTERLY ACCESS REVIEW - FULL FLOW
;; ============================================================

;; 1. Create campaign (usually scheduled job or compliance officer)
(access-review.create-campaign
  :name "Q1 2025 Quarterly Review"
  :review-type "QUARTERLY"
  :scope-type "ALL"
  :review-period-start "2025-01-01"
  :review-period-end "2025-01-31"
  :deadline "2025-02-15")
→ @q1-campaign

;; 2. Populate with memberships + auto-detect issues
(access-review.populate-campaign :campaign @q1-campaign)
→ {
    "total_items": 523,
    "flagged_items": 31,
    "by_flag": {
      "legal_expired": 3,
      "legal_expiring_soon": 12,
      "no_legal_link": 4,
      "dormant_account": 9,
      "never_logged_in": 3
    },
    "by_recommendation": {
      "CONFIRM": 478,
      "REVOKE": 12,
      "EXTEND": 12,
      "REVIEW": 21
    }
  }

;; 3. Launch campaign (sends notifications)
(access-review.launch-campaign :campaign @q1-campaign)
→ { "notifications_sent": 23, "reviewers": 23 }

;; ============================================================
;; REVIEWER WORKFLOW
;; ============================================================

;; Maria logs in, sees her items
(access-review.my-pending :campaign @q1-campaign)
→ [
    { "item_id": @item-1, "user": "hans@allianz.com", 
      "role": "fund-ops.settlement:operator",
      "flags": [], "recommendation": "CONFIRM", "risk_score": 10 },
    { "item_id": @item-2, "user": "former@allianz.com",
      "role": "governance.board:member", 
      "flags": ["legal_expired"], "recommendation": "REVOKE", "risk_score": 90 },
    { "item_id": @item-3, "user": "new@allianz.com",
      "role": "fund-ops.reconciliation:viewer",
      "flags": ["never_logged_in"], "recommendation": "REVIEW", "risk_score": 40 },
    ...
  ]

;; Bulk confirm all clean items (no flags, low risk)
(access-review.confirm-all-clean :campaign @q1-campaign)
→ { "confirmed": 42, "remaining": 6 }

;; Handle flagged items individually
(access-review.revoke-access
  :item @item-2
  :reason "Board appointment ended 2024-11-30, confirmed with legal")

(access-review.confirm-access
  :item @item-3
  :notes "New starter, confirmed with HR - legitimate access")

;; Attest completed reviews
(access-review.attest
  :campaign @q1-campaign
  :scope "MY_REVIEWS")
→ {
    "attestation_id": @att-1,
    "items_count": 48,
    "signature": "sha256:a1b2c3d4e5..."
  }

;; ============================================================
;; DEADLINE PROCESSING (automated job or manual trigger)
;; ============================================================

;; Day after deadline - process stragglers
(access-review.process-deadline
  :campaign @q1-campaign
  :action "suspend")
→ {
    "suspended": 7,
    "reviewers_notified": ["hans@allianz.com", "thomas@allianz.com"],
    "compliance_notified": true
  }

;; ============================================================
;; AUDIT REPORT
;; ============================================================

(access-review.audit-report
  :campaign @q1-campaign
  :format "pdf")
→ @audit-report-doc

;; Report contains:
;; - Campaign summary (dates, scope, stats)
;; - All review decisions with timestamps
;; - Attestations with signatures
;; - Flagged items and resolutions
;; - Auto-suspended items
;; - Reviewer completion rates
```

### Dashboard Mockup

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Q1 2025 ACCESS REVIEW                                   Deadline: Feb 15   │
│  Status: IN_REVIEW                                       Days left: 8       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PROGRESS                                                                    │
│  ████████████████████████████░░░░░░░░  78% complete (408/523)              │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ SUMMARY                                                                │ │
│  ├──────────────┬─────────┬─────────────────────────────────────────────┤ │
│  │ ✓ Confirmed  │   389   │ ████████████████████████████████████        │ │
│  │ ✗ Revoked    │    14   │ ██                                          │ │
│  │ ↻ Extended   │     5   │ █                                           │ │
│  │ ⚠ Escalated  │     3   │ █                                           │ │
│  │ ⏳ Pending   │   112   │ ████████████                                │ │
│  └──────────────┴─────────┴─────────────────────────────────────────────┘ │
│                                                                              │
│  FLAGS REQUIRING ATTENTION                                                   │
│  ┌────────────────────────────────┬───────┬────────────┬─────────────────┐ │
│  │ Flag                           │ Total │ Resolved   │ Pending         │ │
│  ├────────────────────────────────┼───────┼────────────┼─────────────────┤ │
│  │ 🔴 Legal Expired               │     3 │     3      │     0           │ │
│  │ 🟠 Legal Expiring (<30d)       │    12 │     8      │     4           │ │
│  │ 🟠 No Legal Link (Governance)  │     4 │     2      │     2           │ │
│  │ 🟡 Dormant (90+ days)          │     9 │     7      │     2           │ │
│  │ 🟡 Never Logged In             │     3 │     1      │     2           │ │
│  └────────────────────────────────┴───────┴────────────┴─────────────────┘ │
│                                                                              │
│  REVIEWER STATUS                                                             │
│  ┌────────────────────────────┬───────┬────────┬──────────┬──────────────┐ │
│  │ Reviewer                   │ Total │ Done   │ Pending  │ Attested     │ │
│  ├────────────────────────────┼───────┼────────┼──────────┼──────────────┤ │
│  │ maria.schmidt@allianz.com  │    48 │    48  │     0    │ ✓ Feb 8      │ │
│  │ thomas.mueller@allianz.com │    52 │    52  │     0    │ ✓ Feb 9      │ │
│  │ hans.weber@allianz.com     │    67 │    34  │    33    │ -            │ │
│  │ anna.koch@allianz.com      │    41 │    41  │     0    │ ✓ Feb 10     │ │
│  │ peter.braun@allianz.com    │    38 │     0  │    38    │ -            │ │
│  │ ...                        │       │        │          │              │ │
│  └────────────────────────────┴───────┴────────┴──────────┴──────────────┘ │
│                                                                              │
│  [Send Reminders (3)]  [Export Report]  [Process Deadline]                  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Scheduled Jobs

```yaml
# Cron jobs for access review automation
scheduled_jobs:
  
  # Create quarterly campaigns automatically
  - name: create_quarterly_campaign
    cron: "0 0 1 1,4,7,10 *"  # First day of each quarter
    action: |
      (access-review.create-campaign
        :name "Q{quarter} {year} Quarterly Access Review"
        :review-type "QUARTERLY"
        :scope-type "ALL"
        :deadline "{quarter_end + 15 days}")
      (access-review.populate-campaign :campaign @new-campaign)
      (access-review.launch-campaign :campaign @new-campaign)
  
  # Send reminders
  - name: send_review_reminders
    cron: "0 9 * * 1-5"  # 9am weekdays
    action: |
      FOR campaign IN (SELECT * FROM active_campaigns WHERE deadline - TODAY <= 7)
        (access-review.send-reminders :campaign campaign.id)
  
  # Process deadlines
  - name: process_review_deadlines
    cron: "0 0 * * *"  # Midnight daily
    action: |
      FOR campaign IN (SELECT * FROM active_campaigns WHERE deadline < TODAY)
        (access-review.process-deadline :campaign campaign.id :action "suspend")
```

### Summary

| Phase | Manual Effort | Automated |
|-------|---------------|-----------|
| **Create campaign** | Define scope, deadline | Scheduled quarterly |
| **Populate** | Export users to spreadsheet | One click, auto-flags |
| **Assign reviewers** | Email managers | Auto-assign by team |
| **Review** | Reply to emails | In-system bulk actions |
| **Chase** | 3 weeks of emails | Auto-reminders |
| **Attest** | "Per email from John" | Digital signature |
| **Enforce** | Manual revocation | Auto-suspend |
| **Audit** | Compile evidence | One-click report |

**Time savings:** 2-3 weeks → 2-3 days
**Audit quality:** "Best effort" → Complete trail
**Compliance posture:** Reactive → Proactive

The admin nightmare becomes a managed process. Still work, but defensible work.
