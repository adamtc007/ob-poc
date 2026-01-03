-- Migration 009: KYC Control Enhancement - Schema Extensions
-- Phase B from TODO-KYC-CONTROL-ENHANCEMENT.md
-- Extends share_classes and adds board, trust, partnership, tollgate tables

-- ============================================================================
-- B.1: Extend share_classes for Capital Structure
-- ============================================================================

-- Add corporate capital structure columns to existing share_classes table
ALTER TABLE kyc.share_classes
  ADD COLUMN IF NOT EXISTS share_type VARCHAR(50),
  ADD COLUMN IF NOT EXISTS authorized_shares NUMERIC(20,0),
  ADD COLUMN IF NOT EXISTS issued_shares NUMERIC(20,0),
  ADD COLUMN IF NOT EXISTS voting_rights_per_share NUMERIC(10,4) DEFAULT 1.0,
  ADD COLUMN IF NOT EXISTS par_value NUMERIC(20,6),
  ADD COLUMN IF NOT EXISTS par_value_currency VARCHAR(3),
  ADD COLUMN IF NOT EXISTS dividend_rights BOOLEAN DEFAULT true,
  ADD COLUMN IF NOT EXISTS liquidation_preference NUMERIC(20,2),
  ADD COLUMN IF NOT EXISTS conversion_ratio NUMERIC(10,4),
  ADD COLUMN IF NOT EXISTS is_convertible BOOLEAN DEFAULT false,
  ADD COLUMN IF NOT EXISTS issuer_entity_id UUID REFERENCES "ob-poc".entities(entity_id);

COMMENT ON COLUMN kyc.share_classes.share_type IS 'ORDINARY, PREFERENCE_A, PREFERENCE_B, DEFERRED, REDEEMABLE, GROWTH, MANAGEMENT, CONVERTIBLE';
COMMENT ON COLUMN kyc.share_classes.authorized_shares IS 'Maximum shares authorized in articles/charter';
COMMENT ON COLUMN kyc.share_classes.issued_shares IS 'Actually issued shares - SUM(holdings.units) must equal this for reconciliation';
COMMENT ON COLUMN kyc.share_classes.voting_rights_per_share IS 'Votes per share (0 = non-voting, >1 = super-voting)';
COMMENT ON COLUMN kyc.share_classes.par_value IS 'Nominal/par value per share';
COMMENT ON COLUMN kyc.share_classes.liquidation_preference IS 'Priority claim amount on liquidation (typically for preference shares)';
COMMENT ON COLUMN kyc.share_classes.issuer_entity_id IS 'The entity (company) that issued these shares';

-- Add constraint for share_type
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.check_constraints
    WHERE constraint_name = 'chk_share_type'
    AND constraint_schema = 'kyc'
  ) THEN
    ALTER TABLE kyc.share_classes
      ADD CONSTRAINT chk_share_type CHECK (
        share_type IS NULL OR share_type IN (
          'ORDINARY', 'PREFERENCE_A', 'PREFERENCE_B', 'DEFERRED',
          'REDEEMABLE', 'GROWTH', 'MANAGEMENT', 'CONVERTIBLE',
          'COMMON', 'PREFERRED', 'RESTRICTED', 'FOUNDERS'
        )
      );
  END IF;
END $$;

-- Index for capital structure queries
CREATE INDEX IF NOT EXISTS idx_share_classes_issuer ON kyc.share_classes(issuer_entity_id);
CREATE INDEX IF NOT EXISTS idx_share_classes_type ON kyc.share_classes(share_type) WHERE share_type IS NOT NULL;

-- ============================================================================
-- B.2: Capital Structure View
-- ============================================================================

CREATE OR REPLACE VIEW kyc.v_capital_structure AS
SELECT
  sc.id AS share_class_id,
  sc.cbu_id,
  sc.issuer_entity_id,
  sc.name AS share_class_name,
  sc.share_type,
  sc.class_category,
  sc.authorized_shares,
  sc.issued_shares,
  sc.voting_rights_per_share,
  sc.par_value,
  sc.par_value_currency,
  sc.dividend_rights,
  sc.liquidation_preference,
  h.id AS holding_id,
  h.investor_entity_id,
  h.units,
  h.cost_basis,
  h.status AS holding_status,
  -- Ownership calculation
  CASE
    WHEN sc.issued_shares > 0 AND sc.issued_shares IS NOT NULL
    THEN ROUND((h.units / sc.issued_shares) * 100, 4)
    ELSE 0
  END AS ownership_pct,
  -- Voting rights calculation
  h.units * COALESCE(sc.voting_rights_per_share, 1) AS holder_voting_rights,
  -- Total voting rights for this share class
  sc.issued_shares * COALESCE(sc.voting_rights_per_share, 1) AS total_class_voting_rights,
  -- Voting percentage
  CASE
    WHEN sc.issued_shares > 0 AND sc.issued_shares IS NOT NULL AND sc.voting_rights_per_share > 0
    THEN ROUND((h.units / sc.issued_shares) * 100, 4)
    ELSE 0
  END AS voting_pct,
  -- Entity details
  e.name AS investor_name,
  et.type_code AS investor_entity_type,
  ie.name AS issuer_name,
  iet.type_code AS issuer_entity_type
FROM kyc.share_classes sc
LEFT JOIN kyc.holdings h ON h.share_class_id = sc.id AND h.status = 'active'
LEFT JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
LEFT JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
LEFT JOIN "ob-poc".entities ie ON ie.entity_id = sc.issuer_entity_id
LEFT JOIN "ob-poc".entity_types iet ON ie.entity_type_id = iet.entity_type_id
WHERE sc.class_category = 'CORPORATE' OR sc.share_type IS NOT NULL;

COMMENT ON VIEW kyc.v_capital_structure IS
  'Computed ownership and voting percentages from corporate share registry. Join with share_classes and holdings.';

-- ============================================================================
-- B.3: Board Compositions Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.board_compositions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  person_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id),
  appointed_by_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
  appointment_date DATE,
  resignation_date DATE,
  is_active BOOLEAN DEFAULT true,
  appointment_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  appointment_source VARCHAR(50),
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_board_dates CHECK (resignation_date IS NULL OR resignation_date >= appointment_date),
  CONSTRAINT chk_appointment_source CHECK (
    appointment_source IS NULL OR appointment_source IN (
      'ARTICLES', 'SHA', 'BOARD_RESOLUTION', 'SHAREHOLDER_RESOLUTION',
      'REGULATOR_APPROVAL', 'COURT_ORDER', 'OTHER'
    )
  )
);

COMMENT ON TABLE kyc.board_compositions IS
  'Directors and officers appointed to entity boards with appointment chain for control analysis';
COMMENT ON COLUMN kyc.board_compositions.entity_id IS 'The entity (company/fund) whose board this is';
COMMENT ON COLUMN kyc.board_compositions.person_entity_id IS 'The person appointed to the board position';
COMMENT ON COLUMN kyc.board_compositions.appointed_by_entity_id IS 'Entity that exercised appointment right (for SHA-based appointments)';
COMMENT ON COLUMN kyc.board_compositions.appointment_source IS 'Legal basis for the appointment';

CREATE INDEX IF NOT EXISTS idx_board_entity ON kyc.board_compositions(entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_board_person ON kyc.board_compositions(person_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_board_appointer ON kyc.board_compositions(appointed_by_entity_id) WHERE appointed_by_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_board_cbu ON kyc.board_compositions(cbu_id);

-- Unique constraint: same person can't hold same role on same entity board twice (while active)
CREATE UNIQUE INDEX IF NOT EXISTS idx_board_unique_active
  ON kyc.board_compositions(entity_id, person_entity_id, role_id)
  WHERE is_active = true;

-- ============================================================================
-- B.4: Appointment Rights Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.appointment_rights (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  right_type VARCHAR(50) NOT NULL,
  appointable_role_id UUID REFERENCES "ob-poc".roles(role_id),
  max_appointments INTEGER,
  current_appointments INTEGER DEFAULT 0,
  source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  source_clause TEXT,
  effective_from DATE,
  effective_to DATE,
  is_active BOOLEAN DEFAULT true,
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_right_type CHECK (right_type IN (
    'APPOINT_DIRECTOR', 'REMOVE_DIRECTOR', 'APPOINT_AND_REMOVE',
    'VETO_APPOINTMENT', 'CONSENT_REQUIRED', 'OBSERVER_SEAT',
    'APPOINT_CHAIRMAN', 'APPOINT_CEO', 'APPOINT_AUDITOR'
  )),
  CONSTRAINT chk_appointments CHECK (
    max_appointments IS NULL OR current_appointments <= max_appointments
  )
);

COMMENT ON TABLE kyc.appointment_rights IS
  'SHA/articles provisions granting board appointment/removal rights - key for control analysis';
COMMENT ON COLUMN kyc.appointment_rights.target_entity_id IS 'Entity whose board can be affected by this right';
COMMENT ON COLUMN kyc.appointment_rights.holder_entity_id IS 'Entity holding/exercising the appointment right';
COMMENT ON COLUMN kyc.appointment_rights.source_clause IS 'Reference to specific clause in SHA/articles (e.g., "Clause 5.2(a)")';
COMMENT ON COLUMN kyc.appointment_rights.max_appointments IS 'Maximum number of directors this right allows appointing';

CREATE INDEX IF NOT EXISTS idx_appt_rights_target ON kyc.appointment_rights(target_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_appt_rights_holder ON kyc.appointment_rights(holder_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_appt_rights_cbu ON kyc.appointment_rights(cbu_id);

-- Unique constraint: one right type per holder-target pair
CREATE UNIQUE INDEX IF NOT EXISTS idx_appt_rights_unique
  ON kyc.appointment_rights(target_entity_id, holder_entity_id, right_type)
  WHERE is_active = true;

-- ============================================================================
-- B.5: Trust Provisions Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.trust_provisions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  trust_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  provision_type VARCHAR(50) NOT NULL,
  holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
  beneficiary_class TEXT,
  interest_percentage NUMERIC(8,4),
  discretion_level VARCHAR(30),
  vesting_conditions TEXT,
  vesting_date DATE,
  source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  source_clause TEXT,
  is_active BOOLEAN DEFAULT true,
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_provision_type CHECK (provision_type IN (
    'INCOME_BENEFICIARY', 'CAPITAL_BENEFICIARY', 'DISCRETIONARY_BENEFICIARY',
    'REMAINDER_BENEFICIARY', 'CONTINGENT_BENEFICIARY', 'DEFAULT_BENEFICIARY',
    'APPOINTOR_POWER', 'PROTECTOR_POWER', 'TRUSTEE_REMOVAL',
    'TRUST_VARIATION', 'ACCUMULATION_POWER', 'ADVANCEMENT_POWER',
    'INVESTMENT_DIRECTION', 'DISTRIBUTION_DIRECTION', 'ADD_BENEFICIARY',
    'EXCLUDE_BENEFICIARY', 'RESERVED_POWER'
  )),
  CONSTRAINT chk_discretion CHECK (discretion_level IS NULL OR discretion_level IN (
    'ABSOLUTE', 'LIMITED', 'NONE', 'FETTERED'
  )),
  CONSTRAINT chk_interest_pct CHECK (
    interest_percentage IS NULL OR (interest_percentage >= 0 AND interest_percentage <= 100)
  )
);

COMMENT ON TABLE kyc.trust_provisions IS
  'Trust deed provisions affecting control and beneficial interest - key for trust UBO analysis';
COMMENT ON COLUMN kyc.trust_provisions.trust_entity_id IS 'The trust entity this provision belongs to';
COMMENT ON COLUMN kyc.trust_provisions.holder_entity_id IS 'Entity holding this provision/right (NULL for class beneficiaries)';
COMMENT ON COLUMN kyc.trust_provisions.beneficiary_class IS 'Description of beneficiary class if not specific entity (e.g., "descendants of the settlor")';
COMMENT ON COLUMN kyc.trust_provisions.discretion_level IS 'Level of trustee discretion: ABSOLUTE=full, LIMITED=within parameters, NONE=mandatory, FETTERED=restricted';
COMMENT ON COLUMN kyc.trust_provisions.interest_percentage IS 'Fixed interest percentage for fixed interest trusts';

CREATE INDEX IF NOT EXISTS idx_trust_prov_trust ON kyc.trust_provisions(trust_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_trust_prov_holder ON kyc.trust_provisions(holder_entity_id) WHERE holder_entity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trust_prov_cbu ON kyc.trust_provisions(cbu_id);
CREATE INDEX IF NOT EXISTS idx_trust_prov_type ON kyc.trust_provisions(provision_type);

-- ============================================================================
-- B.6: Partnership Capital Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS kyc.partnership_capital (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
  partnership_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  partner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  partner_type VARCHAR(30) NOT NULL,
  capital_commitment NUMERIC(20,2),
  capital_contributed NUMERIC(20,2) DEFAULT 0,
  capital_returned NUMERIC(20,2) DEFAULT 0,
  unfunded_commitment NUMERIC(20,2) GENERATED ALWAYS AS (
    COALESCE(capital_commitment, 0) - COALESCE(capital_contributed, 0) + COALESCE(capital_returned, 0)
  ) STORED,
  profit_share_pct NUMERIC(8,4),
  loss_share_pct NUMERIC(8,4),
  management_fee_share_pct NUMERIC(8,4),
  carried_interest_pct NUMERIC(8,4),
  management_rights BOOLEAN DEFAULT false,
  voting_pct NUMERIC(8,4),
  admission_date DATE,
  withdrawal_date DATE,
  is_active BOOLEAN DEFAULT true,
  source_document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_partner_type CHECK (partner_type IN ('GP', 'LP', 'MEMBER', 'FOUNDING_PARTNER', 'SPECIAL_LP')),
  CONSTRAINT chk_capital CHECK (
    capital_contributed IS NULL OR capital_commitment IS NULL OR
    capital_contributed <= capital_commitment
  ),
  CONSTRAINT chk_percentages CHECK (
    (profit_share_pct IS NULL OR (profit_share_pct >= 0 AND profit_share_pct <= 100)) AND
    (loss_share_pct IS NULL OR (loss_share_pct >= 0 AND loss_share_pct <= 100)) AND
    (voting_pct IS NULL OR (voting_pct >= 0 AND voting_pct <= 100))
  ),
  UNIQUE(partnership_entity_id, partner_entity_id)
);

COMMENT ON TABLE kyc.partnership_capital IS
  'Partnership capital accounts and profit/loss allocation for LP/GP structures';
COMMENT ON COLUMN kyc.partnership_capital.partner_type IS 'GP=General Partner (control+liability), LP=Limited Partner (passive), MEMBER=LLP member';
COMMENT ON COLUMN kyc.partnership_capital.management_rights IS 'Whether partner has management rights (always true for GP)';
COMMENT ON COLUMN kyc.partnership_capital.unfunded_commitment IS 'Computed: commitment - contributed + returned';
COMMENT ON COLUMN kyc.partnership_capital.carried_interest_pct IS 'GP carried interest percentage (typically 20%)';

CREATE INDEX IF NOT EXISTS idx_partnership_cap_partnership ON kyc.partnership_capital(partnership_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_partnership_cap_partner ON kyc.partnership_capital(partner_entity_id) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_partnership_cap_cbu ON kyc.partnership_capital(cbu_id);
CREATE INDEX IF NOT EXISTS idx_partnership_cap_type ON kyc.partnership_capital(partner_type);

-- ============================================================================
-- B.7: Tollgate Evaluation Tables
-- ============================================================================

-- Tollgate threshold configuration
CREATE TABLE IF NOT EXISTS kyc.tollgate_thresholds (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  threshold_name VARCHAR(100) NOT NULL UNIQUE,
  metric_type VARCHAR(50) NOT NULL,
  comparison VARCHAR(10) NOT NULL DEFAULT 'GTE',
  threshold_value NUMERIC(10,4),
  weight NUMERIC(5,2) DEFAULT 1.0,
  is_blocking BOOLEAN DEFAULT false,
  applies_to_case_types VARCHAR(50)[] DEFAULT ARRAY['NEW_CLIENT', 'PERIODIC_REVIEW', 'EVENT_DRIVEN'],
  description TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_metric_type CHECK (metric_type IN (
    'OWNERSHIP_VERIFIED_PCT', 'CONTROL_VERIFIED_PCT', 'UBO_COVERAGE_PCT',
    'DOC_COMPLETENESS_PCT', 'SCREENING_CLEAR_PCT', 'RED_FLAG_COUNT',
    'ALLEGATION_UNRESOLVED_COUNT', 'DAYS_SINCE_REFRESH', 'ENTITY_KYC_COMPLETE_PCT',
    'HIGH_RISK_ENTITY_COUNT', 'OPEN_DISCREPANCY_COUNT', 'EVIDENCE_FRESHNESS_DAYS'
  )),
  CONSTRAINT chk_comparison CHECK (comparison IN ('GT', 'GTE', 'LT', 'LTE', 'EQ', 'NEQ'))
);

COMMENT ON TABLE kyc.tollgate_thresholds IS
  'Configurable thresholds for tollgate pass/fail decisions in KYC workflow';
COMMENT ON COLUMN kyc.tollgate_thresholds.comparison IS 'Comparison operator: GT(>), GTE(>=), LT(<), LTE(<=), EQ(=), NEQ(!=)';
COMMENT ON COLUMN kyc.tollgate_thresholds.is_blocking IS 'If true, failing this threshold blocks case progression';
COMMENT ON COLUMN kyc.tollgate_thresholds.applies_to_case_types IS 'Case types this threshold applies to';

-- Populate default thresholds
INSERT INTO kyc.tollgate_thresholds (threshold_name, metric_type, comparison, threshold_value, is_blocking, weight, description)
VALUES
  ('ownership_minimum', 'OWNERSHIP_VERIFIED_PCT', 'GTE', 95.0, true, 1.0, 'Minimum verified ownership percentage'),
  ('control_minimum', 'CONTROL_VERIFIED_PCT', 'GTE', 100.0, true, 1.0, 'All control vectors must be verified'),
  ('ubo_coverage', 'UBO_COVERAGE_PCT', 'GTE', 100.0, true, 1.0, 'All UBOs must be identified'),
  ('doc_completeness', 'DOC_COMPLETENESS_PCT', 'GTE', 90.0, false, 0.8, 'Target document collection rate'),
  ('entity_kyc_complete', 'ENTITY_KYC_COMPLETE_PCT', 'GTE', 100.0, true, 1.0, 'All entities must complete KYC'),
  ('screening_clear', 'SCREENING_CLEAR_PCT', 'GTE', 100.0, true, 1.0, 'All screenings must be clear or escalated'),
  ('red_flag_limit', 'RED_FLAG_COUNT', 'LTE', 0, false, 0.9, 'Maximum unaddressed red flags'),
  ('allegation_limit', 'ALLEGATION_UNRESOLVED_COUNT', 'LTE', 0, true, 1.0, 'No unresolved ownership allegations'),
  ('discrepancy_limit', 'OPEN_DISCREPANCY_COUNT', 'LTE', 0, false, 0.7, 'No open discrepancies'),
  ('high_risk_limit', 'HIGH_RISK_ENTITY_COUNT', 'LTE', 0, false, 0.8, 'High risk entities require escalation'),
  ('evidence_freshness', 'EVIDENCE_FRESHNESS_DAYS', 'LTE', 365, false, 0.5, 'Evidence should be less than 1 year old')
ON CONFLICT (threshold_name) DO UPDATE SET
  metric_type = EXCLUDED.metric_type,
  comparison = EXCLUDED.comparison,
  threshold_value = EXCLUDED.threshold_value,
  is_blocking = EXCLUDED.is_blocking,
  weight = EXCLUDED.weight,
  description = EXCLUDED.description,
  updated_at = now();

-- Tollgate evaluations
CREATE TABLE IF NOT EXISTS kyc.tollgate_evaluations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  case_id UUID NOT NULL REFERENCES kyc.cases(case_id) ON DELETE CASCADE,
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  evaluation_type VARCHAR(30) NOT NULL,
  evaluated_at TIMESTAMPTZ DEFAULT now(),
  evaluated_by VARCHAR(100),
  overall_result VARCHAR(20) NOT NULL,
  score NUMERIC(5,2),
  metrics JSONB NOT NULL DEFAULT '{}',
  threshold_results JSONB NOT NULL DEFAULT '{}',
  blocking_failures TEXT[],
  warnings TEXT[],
  override_id UUID,
  notes TEXT,
  CONSTRAINT chk_eval_type CHECK (evaluation_type IN (
    'DISCOVERY_COMPLETE', 'EVIDENCE_COMPLETE', 'VERIFICATION_COMPLETE',
    'DECISION_READY', 'PERIODIC_REVIEW', 'EVENT_TRIGGERED'
  )),
  CONSTRAINT chk_result CHECK (overall_result IN ('PASS', 'FAIL', 'CONDITIONAL', 'OVERRIDE', 'PENDING'))
);

COMMENT ON TABLE kyc.tollgate_evaluations IS
  'Point-in-time tollgate evaluation results for KYC cases';
COMMENT ON COLUMN kyc.tollgate_evaluations.metrics IS
  'JSON object with all computed metrics at evaluation time';
COMMENT ON COLUMN kyc.tollgate_evaluations.threshold_results IS
  'JSON object with per-threshold pass/fail results';
COMMENT ON COLUMN kyc.tollgate_evaluations.blocking_failures IS
  'Array of threshold names that caused blocking failure';

CREATE INDEX IF NOT EXISTS idx_tollgate_case ON kyc.tollgate_evaluations(case_id);
CREATE INDEX IF NOT EXISTS idx_tollgate_cbu ON kyc.tollgate_evaluations(cbu_id);
CREATE INDEX IF NOT EXISTS idx_tollgate_result ON kyc.tollgate_evaluations(overall_result);
CREATE INDEX IF NOT EXISTS idx_tollgate_type ON kyc.tollgate_evaluations(evaluation_type);
CREATE INDEX IF NOT EXISTS idx_tollgate_date ON kyc.tollgate_evaluations(evaluated_at DESC);

-- Tollgate overrides (management override of failed tollgate)
CREATE TABLE IF NOT EXISTS kyc.tollgate_overrides (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  evaluation_id UUID NOT NULL REFERENCES kyc.tollgate_evaluations(id) ON DELETE CASCADE,
  override_reason TEXT NOT NULL,
  approved_by VARCHAR(100) NOT NULL,
  approved_at TIMESTAMPTZ DEFAULT now(),
  approval_authority VARCHAR(50) NOT NULL,
  conditions TEXT,
  expiry_date DATE,
  is_active BOOLEAN DEFAULT true,
  review_required_by DATE,
  created_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_authority CHECK (approval_authority IN (
    'ANALYST', 'SENIOR_ANALYST', 'TEAM_LEAD', 'COMPLIANCE_OFFICER',
    'SENIOR_COMPLIANCE', 'MLRO', 'EXECUTIVE', 'BOARD'
  ))
);

COMMENT ON TABLE kyc.tollgate_overrides IS
  'Management overrides for tollgate failures with audit trail';
COMMENT ON COLUMN kyc.tollgate_overrides.approval_authority IS 'Level of authority that approved the override';
COMMENT ON COLUMN kyc.tollgate_overrides.conditions IS 'Any conditions attached to the override';
COMMENT ON COLUMN kyc.tollgate_overrides.review_required_by IS 'Date by which the override must be reviewed';

CREATE INDEX IF NOT EXISTS idx_override_eval ON kyc.tollgate_overrides(evaluation_id);
CREATE INDEX IF NOT EXISTS idx_override_active ON kyc.tollgate_overrides(is_active) WHERE is_active = true;
