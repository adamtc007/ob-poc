-- Migration: 024_csg_validation_rules_table.sql
-- Purpose: Create centralized CSG validation rules table
-- Part of CSG Linter implementation for business rule validation

BEGIN;

-- ============================================
-- CSG_VALIDATION_RULES: Centralized Rule Store
-- ============================================
-- This table allows rules to be managed independently of the entities they govern.
-- Rules can be versioned, A/B tested, and overridden per-CBU.

CREATE TABLE IF NOT EXISTS "ob-poc".csg_validation_rules (
    rule_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Rule identification
    rule_code VARCHAR(100) UNIQUE NOT NULL,
    rule_name VARCHAR(255) NOT NULL,
    rule_version INTEGER DEFAULT 1,

    -- What this rule applies to
    target_type VARCHAR(50) NOT NULL CHECK (target_type IN (
        'document_type',      -- Rule about document types
        'attribute',          -- Rule about attributes
        'entity_type',        -- Rule about entity types
        'verb',               -- Rule about DSL verbs
        'cross_reference'     -- Rule about relationships
    )),
    target_code VARCHAR(100),  -- Specific target (e.g., "PASSPORT") or NULL for all

    -- The rule definition
    rule_type VARCHAR(50) NOT NULL CHECK (rule_type IN (
        'entity_type_constraint',     -- Allowed entity types
        'jurisdiction_constraint',    -- Allowed jurisdictions
        'client_type_constraint',     -- Allowed client types
        'prerequisite',               -- Required prior operations
        'exclusion',                  -- Mutually exclusive items
        'co_occurrence',              -- Must appear together
        'sequence',                   -- Must appear in order
        'cardinality',                -- Min/max occurrences
        'custom'                      -- Custom validation function
    )),

    -- Rule parameters (the actual constraints)
    rule_params JSONB NOT NULL,

    -- Error handling
    error_code VARCHAR(10) NOT NULL,   -- e.g., "C001"
    error_message_template TEXT NOT NULL,
    suggestion_template TEXT,
    severity VARCHAR(20) DEFAULT 'error' CHECK (severity IN ('error', 'warning', 'info')),

    -- Metadata
    description TEXT,
    rationale TEXT,
    documentation_url TEXT,

    -- Lifecycle
    is_active BOOLEAN DEFAULT true,
    effective_from TIMESTAMPTZ DEFAULT NOW(),
    effective_until TIMESTAMPTZ,

    -- Audit
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_csg_rules_target
ON "ob-poc".csg_validation_rules(target_type, target_code);

CREATE INDEX IF NOT EXISTS idx_csg_rules_type
ON "ob-poc".csg_validation_rules(rule_type);

CREATE INDEX IF NOT EXISTS idx_csg_rules_active
ON "ob-poc".csg_validation_rules(is_active) WHERE is_active = true;

CREATE INDEX IF NOT EXISTS idx_csg_rules_params
ON "ob-poc".csg_validation_rules USING GIN (rule_params);

-- ============================================
-- CSG_RULE_OVERRIDES: Per-CBU Rule Overrides
-- ============================================
-- Allows specific CBUs to have custom rule behavior

CREATE TABLE IF NOT EXISTS "ob-poc".csg_rule_overrides (
    override_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID NOT NULL REFERENCES "ob-poc".csg_validation_rules(rule_id) ON DELETE CASCADE,
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,

    -- Override behavior
    override_type VARCHAR(50) NOT NULL CHECK (override_type IN (
        'disable',            -- Completely disable this rule for this CBU
        'downgrade',          -- Change error to warning
        'modify_params',      -- Use different parameters
        'add_exception'       -- Add specific exception values
    )),
    override_params JSONB,

    -- Approval workflow
    approved_by VARCHAR(255),
    approval_reason TEXT NOT NULL,
    approved_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,

    -- Audit
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(rule_id, cbu_id)
);

CREATE INDEX IF NOT EXISTS idx_csg_overrides_cbu
ON "ob-poc".csg_rule_overrides(cbu_id);

CREATE INDEX IF NOT EXISTS idx_csg_overrides_rule
ON "ob-poc".csg_rule_overrides(rule_id);

COMMIT;
