-- Migration 008: KYC Control Enhancement - Roles and Validation
-- Phase A from TODO-KYC-CONTROL-ENHANCEMENT.md
-- Adds executive, LLP, and control-specific roles plus role-entity-type constraints

-- ============================================================================
-- A.1: Add C-Suite & Executive Roles
-- ============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category, layout_category, ubo_treatment, natural_person_only, display_priority, sort_order)
VALUES
  ('CEO', 'Chief Executive Officer - operational control', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 10, 10),
  ('CFO', 'Chief Financial Officer - financial control', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 11, 11),
  ('CIO', 'Chief Investment Officer - investment decisions', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 12, 12),
  ('COO', 'Chief Operating Officer - operations', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 13, 13),
  ('CRO', 'Chief Risk Officer - risk management', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 14, 14),
  ('CCO', 'Chief Compliance Officer - compliance oversight', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 15, 15),
  ('MANAGING_DIRECTOR', 'Managing Director - senior management', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 20, 20),
  ('CHAIRMAN', 'Board Chairman - board control', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 5, 5),
  ('EXECUTIVE_DIRECTOR', 'Executive board member - management + board', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 25, 25),
  ('NON_EXECUTIVE_DIRECTOR', 'Non-executive board member - oversight only', 'CONTROL_CHAIN', 'Overlay', 'OVERSIGHT', true, 26, 26)
ON CONFLICT (name) DO UPDATE SET
  description = EXCLUDED.description,
  role_category = EXCLUDED.role_category,
  layout_category = EXCLUDED.layout_category,
  ubo_treatment = EXCLUDED.ubo_treatment,
  natural_person_only = EXCLUDED.natural_person_only,
  display_priority = EXCLUDED.display_priority,
  sort_order = EXCLUDED.sort_order,
  updated_at = now();

-- ============================================================================
-- A.2: Add LLP-Specific Roles
-- ============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category, layout_category, ubo_treatment, requires_percentage, display_priority, sort_order)
VALUES
  ('DESIGNATED_MEMBER', 'LLP designated member - statutory signatory with filing duties', 'OWNERSHIP_CHAIN', 'PyramidUp', 'BENEFICIAL_OWNER', true, 30, 30),
  ('MEMBER', 'LLP member - ownership interest without designated status', 'OWNERSHIP_CHAIN', 'PyramidUp', 'BENEFICIAL_OWNER', true, 31, 31)
ON CONFLICT (name) DO UPDATE SET
  description = EXCLUDED.description,
  role_category = EXCLUDED.role_category,
  layout_category = EXCLUDED.layout_category,
  ubo_treatment = EXCLUDED.ubo_treatment,
  requires_percentage = EXCLUDED.requires_percentage,
  display_priority = EXCLUDED.display_priority,
  sort_order = EXCLUDED.sort_order,
  updated_at = now();

-- ============================================================================
-- A.3: Add Control-Specific Roles
-- ============================================================================

INSERT INTO "ob-poc".roles (name, description, role_category, layout_category, ubo_treatment, natural_person_only, display_priority, sort_order)
VALUES
  ('CONTROLLER', 'De facto control without ownership (SHA provisions, veto rights)', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', false, 35, 35),
  ('POWER_OF_ATTORNEY', 'Legal representative - can act for entity', 'CONTROL_CHAIN', 'Overlay', 'CONTROL_PERSON', true, 36, 36),
  ('APPOINTOR', 'Trust role - can appoint/remove trustees', 'TRUST_ROLES', 'Radial', 'CONTROL_PERSON', false, 40, 40),
  ('ENFORCER', 'Trust role - charitable/purpose trust oversight', 'TRUST_ROLES', 'Radial', 'OVERSIGHT', false, 41, 41),
  ('PORTFOLIO_MANAGER', 'Day-to-day investment decisions', 'FUND_MANAGEMENT', 'Overlay', 'CONTROL_PERSON', true, 45, 45),
  ('KEY_PERSON', 'Subject to key-man clause provisions', 'FUND_MANAGEMENT', 'Overlay', 'KEY_PERSON', true, 46, 46),
  ('INVESTMENT_COMMITTEE_MEMBER', 'Member of investment committee', 'FUND_MANAGEMENT', 'Overlay', 'OVERSIGHT', true, 47, 47),
  ('CONDUCTING_OFFICER', 'CSSF conducting officer (Luxembourg funds)', 'FUND_MANAGEMENT', 'Overlay', 'CONTROL_PERSON', true, 48, 48)
ON CONFLICT (name) DO UPDATE SET
  description = EXCLUDED.description,
  role_category = EXCLUDED.role_category,
  layout_category = EXCLUDED.layout_category,
  ubo_treatment = EXCLUDED.ubo_treatment,
  natural_person_only = EXCLUDED.natural_person_only,
  display_priority = EXCLUDED.display_priority,
  sort_order = EXCLUDED.sort_order,
  updated_at = now();

-- ============================================================================
-- A.4: Role-to-Entity-Type Validation Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS "ob-poc".role_applicable_entity_types (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id) ON DELETE CASCADE,
  entity_type_code VARCHAR(100) NOT NULL,
  is_required BOOLEAN DEFAULT false,
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  UNIQUE(role_id, entity_type_code)
);

COMMENT ON TABLE "ob-poc".role_applicable_entity_types IS
  'Constrains which roles can be assigned to which entity types. E.g., TRUSTEE only for trust entities, GP only for partnerships.';
COMMENT ON COLUMN "ob-poc".role_applicable_entity_types.is_required IS
  'If true, this role MUST be assigned to an entity of this type (e.g., every trust must have a TRUSTEE)';

-- Index for quick lookup by entity type
CREATE INDEX IF NOT EXISTS idx_role_applicable_entity_type
  ON "ob-poc".role_applicable_entity_types(entity_type_code);

-- ============================================================================
-- Populate role-entity-type constraints
-- ============================================================================

-- Trust roles - only applicable to trust entities
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('TRUSTEE', 'TRUST_DISCRETIONARY', true, 'Every discretionary trust requires at least one trustee'),
  ('TRUSTEE', 'TRUST_FIXED_INTEREST', true, 'Every fixed interest trust requires at least one trustee'),
  ('TRUSTEE', 'TRUST_UNIT', true, 'Every unit trust requires at least one trustee'),
  ('TRUSTEE', 'TRUST_CHARITABLE', true, 'Every charitable trust requires at least one trustee'),
  ('SETTLOR', 'TRUST_DISCRETIONARY', false, NULL),
  ('SETTLOR', 'TRUST_FIXED_INTEREST', false, NULL),
  ('SETTLOR', 'TRUST_UNIT', false, NULL),
  ('SETTLOR', 'TRUST_CHARITABLE', false, NULL),
  ('BENEFICIARY', 'TRUST_DISCRETIONARY', false, NULL),
  ('BENEFICIARY', 'TRUST_FIXED_INTEREST', false, NULL),
  ('BENEFICIARY', 'TRUST_UNIT', false, NULL),
  ('BENEFICIARY', 'TRUST_CHARITABLE', false, NULL),
  ('PROTECTOR', 'TRUST_DISCRETIONARY', false, NULL),
  ('PROTECTOR', 'TRUST_FIXED_INTEREST', false, NULL),
  ('PROTECTOR', 'TRUST_UNIT', false, NULL),
  ('PROTECTOR', 'TRUST_CHARITABLE', false, NULL),
  ('APPOINTOR', 'TRUST_DISCRETIONARY', false, NULL),
  ('APPOINTOR', 'TRUST_FIXED_INTEREST', false, NULL),
  ('APPOINTOR', 'TRUST_UNIT', false, NULL),
  ('APPOINTOR', 'TRUST_CHARITABLE', false, NULL),
  ('ENFORCER', 'TRUST_CHARITABLE', false, 'Enforcer role specific to charitable/purpose trusts')
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- Partnership roles - only applicable to partnership entities
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('GENERAL_PARTNER', 'PARTNERSHIP_GENERAL', true, 'GP required for general partnership'),
  ('GENERAL_PARTNER', 'PARTNERSHIP_LIMITED', true, 'GP required for limited partnership'),
  ('LIMITED_PARTNER', 'PARTNERSHIP_LIMITED', false, NULL),
  ('MANAGING_PARTNER', 'PARTNERSHIP_GENERAL', false, NULL),
  ('MANAGING_PARTNER', 'PARTNERSHIP_LIMITED', false, NULL),
  ('MANAGING_PARTNER', 'PARTNERSHIP_LLP', false, NULL),
  ('DESIGNATED_MEMBER', 'PARTNERSHIP_LLP', true, 'LLP requires at least two designated members'),
  ('MEMBER', 'PARTNERSHIP_LLP', false, NULL)
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- Corporate roles - applicable to companies
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('DIRECTOR', 'LIMITED_COMPANY', true, 'Company requires at least one director'),
  ('DIRECTOR', 'PRIVATE_LIMITED_COMPANY', true, NULL),
  ('DIRECTOR', 'PUBLIC_LIMITED_COMPANY', true, NULL),
  ('CHAIRMAN', 'LIMITED_COMPANY', false, NULL),
  ('CHAIRMAN', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('CHAIRMAN', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('EXECUTIVE_DIRECTOR', 'LIMITED_COMPANY', false, NULL),
  ('EXECUTIVE_DIRECTOR', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('EXECUTIVE_DIRECTOR', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('NON_EXECUTIVE_DIRECTOR', 'LIMITED_COMPANY', false, NULL),
  ('NON_EXECUTIVE_DIRECTOR', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('NON_EXECUTIVE_DIRECTOR', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('CEO', 'LIMITED_COMPANY', false, NULL),
  ('CEO', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('CEO', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('CFO', 'LIMITED_COMPANY', false, NULL),
  ('CFO', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('CFO', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('SHAREHOLDER', 'LIMITED_COMPANY', false, NULL),
  ('SHAREHOLDER', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('SHAREHOLDER', 'PUBLIC_LIMITED_COMPANY', false, NULL),
  ('COMPANY_SECRETARY', 'LIMITED_COMPANY', false, NULL),
  ('COMPANY_SECRETARY', 'PRIVATE_LIMITED_COMPANY', false, NULL),
  ('COMPANY_SECRETARY', 'PUBLIC_LIMITED_COMPANY', true, 'Public companies require a company secretary')
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- Fund roles - applicable to fund entities
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type_code, is_required, notes)
SELECT r.role_id, et.entity_type_code, et.is_required, et.notes
FROM "ob-poc".roles r
CROSS JOIN (VALUES
  ('INVESTMENT_MANAGER', 'FUND_SICAV', false, NULL),
  ('INVESTMENT_MANAGER', 'FUND_ICAV', false, NULL),
  ('INVESTMENT_MANAGER', 'FUND_OEIC', false, NULL),
  ('INVESTMENT_MANAGER', 'FUND_FCP', false, NULL),
  ('MANAGEMENT_COMPANY', 'FUND_SICAV', true, 'UCITS/AIF requires authorized ManCo'),
  ('MANAGEMENT_COMPANY', 'FUND_ICAV', true, NULL),
  ('MANAGEMENT_COMPANY', 'FUND_OEIC', true, NULL),
  ('MANAGEMENT_COMPANY', 'FUND_FCP', true, NULL),
  ('DEPOSITARY', 'FUND_SICAV', true, 'Regulated funds require depositary'),
  ('DEPOSITARY', 'FUND_ICAV', true, NULL),
  ('DEPOSITARY', 'FUND_OEIC', true, NULL),
  ('DEPOSITARY', 'FUND_FCP', true, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_SICAV', false, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_ICAV', false, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_OEIC', false, NULL),
  ('PORTFOLIO_MANAGER', 'FUND_FCP', false, NULL),
  ('CONDUCTING_OFFICER', 'FUND_SICAV', false, 'Luxembourg CSSF requirement'),
  ('CONDUCTING_OFFICER', 'FUND_FCP', false, NULL),
  ('KEY_PERSON', 'FUND_SICAV', false, NULL),
  ('KEY_PERSON', 'FUND_ICAV', false, NULL),
  ('KEY_PERSON', 'FUND_OEIC', false, NULL),
  ('KEY_PERSON', 'FUND_FCP', false, NULL)
) AS et(role_name, entity_type_code, is_required, notes)
WHERE r.name = et.role_name
ON CONFLICT (role_id, entity_type_code) DO UPDATE SET
  is_required = EXCLUDED.is_required,
  notes = EXCLUDED.notes;

-- ============================================================================
-- Create view for role applicability lookup
-- ============================================================================

CREATE OR REPLACE VIEW "ob-poc".v_role_applicability AS
SELECT
  r.role_id,
  r.name AS role_name,
  r.description AS role_description,
  r.role_category,
  r.ubo_treatment,
  r.natural_person_only,
  r.legal_entity_only,
  COALESCE(
    array_agg(DISTINCT raet.entity_type_code) FILTER (WHERE raet.entity_type_code IS NOT NULL),
    ARRAY[]::varchar[]
  ) AS applicable_entity_types,
  COALESCE(
    array_agg(DISTINCT raet.entity_type_code) FILTER (WHERE raet.is_required = true),
    ARRAY[]::varchar[]
  ) AS required_for_entity_types
FROM "ob-poc".roles r
LEFT JOIN "ob-poc".role_applicable_entity_types raet ON r.role_id = raet.role_id
WHERE r.is_active = true
GROUP BY r.role_id, r.name, r.description, r.role_category, r.ubo_treatment, r.natural_person_only, r.legal_entity_only;

COMMENT ON VIEW "ob-poc".v_role_applicability IS
  'Aggregated view of roles with their applicable entity types for validation';

-- ============================================================================
-- Create function to validate role assignment
-- ============================================================================

CREATE OR REPLACE FUNCTION "ob-poc".validate_role_for_entity_type(
  p_role_id UUID,
  p_entity_type_code VARCHAR
) RETURNS BOOLEAN AS $$
DECLARE
  v_has_constraints BOOLEAN;
  v_is_applicable BOOLEAN;
BEGIN
  -- Check if this role has any entity type constraints
  SELECT EXISTS(
    SELECT 1 FROM "ob-poc".role_applicable_entity_types
    WHERE role_id = p_role_id
  ) INTO v_has_constraints;

  -- If no constraints, role is applicable to all entity types
  IF NOT v_has_constraints THEN
    RETURN true;
  END IF;

  -- Check if entity type is in the allowed list
  SELECT EXISTS(
    SELECT 1 FROM "ob-poc".role_applicable_entity_types
    WHERE role_id = p_role_id
    AND entity_type_code = p_entity_type_code
  ) INTO v_is_applicable;

  RETURN v_is_applicable;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION "ob-poc".validate_role_for_entity_type IS
  'Validates whether a role can be assigned to an entity of the given type. Returns true if allowed.';
