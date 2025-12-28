-- =============================================================================
-- ACCOUNTING SCHEMA - Part 2
-- Service Contracts, Fee Schedules, Invoices
-- =============================================================================

CREATE SCHEMA IF NOT EXISTS accounting;

-- -----------------------------------------------------------------------------
-- Service Contracts
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS accounting.service_contracts (
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

CREATE INDEX IF NOT EXISTS idx_contracts_client ON accounting.service_contracts(client_entity_id);
CREATE INDEX IF NOT EXISTS idx_contracts_status ON accounting.service_contracts(status);

-- -----------------------------------------------------------------------------
-- Fee Schedules
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS accounting.fee_schedules (
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

CREATE INDEX IF NOT EXISTS idx_schedules_contract ON accounting.fee_schedules(contract_id);

-- -----------------------------------------------------------------------------
-- Invoices
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS accounting.invoices (
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

CREATE INDEX IF NOT EXISTS idx_invoice_contract ON accounting.invoices(contract_id);
CREATE INDEX IF NOT EXISTS idx_invoice_entity ON accounting.invoices(billing_entity_id);
CREATE INDEX IF NOT EXISTS idx_invoice_status ON accounting.invoices(status);
CREATE INDEX IF NOT EXISTS idx_invoice_period ON accounting.invoices(period_start, period_end);

-- -----------------------------------------------------------------------------
-- Invoice Lines
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS accounting.invoice_lines (
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

CREATE INDEX IF NOT EXISTS idx_invoice_line_invoice ON accounting.invoice_lines(invoice_id);

-- -----------------------------------------------------------------------------
-- Cost Allocations
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS accounting.cost_allocations (
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

CREATE TABLE IF NOT EXISTS accounting.invoice_contacts (
    contact_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_id UUID NOT NULL REFERENCES accounting.service_contracts(contract_id),

    -- Who
    team_id UUID REFERENCES teams.teams(team_id),
    user_id UUID REFERENCES client_portal.clients(client_id),
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

CREATE INDEX IF NOT EXISTS idx_contacts_contract ON accounting.invoice_contacts(contract_id);

-- -----------------------------------------------------------------------------
-- Triggers for updated_at
-- -----------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION accounting.update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_contracts_updated ON accounting.service_contracts;
CREATE TRIGGER trg_contracts_updated
    BEFORE UPDATE ON accounting.service_contracts
    FOR EACH ROW EXECUTE FUNCTION accounting.update_timestamp();

DROP TRIGGER IF EXISTS trg_invoices_updated ON accounting.invoices;
CREATE TRIGGER trg_invoices_updated
    BEFORE UPDATE ON accounting.invoices
    FOR EACH ROW EXECUTE FUNCTION accounting.update_timestamp();
