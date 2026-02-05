-- Migration 068: Deal/Billing CHECK Constraints and UUID Alignment
-- Addresses review feedback:
-- 1. Add CHECK constraints for status fields (consistency with other tables)
-- 2. Switch from gen_random_uuid() to uuidv7() for time-ordered PKs

-- ============================================================================
-- 1. STATUS CHECK CONSTRAINTS
-- ============================================================================

-- deals.deal_status
ALTER TABLE "ob-poc".deals
ADD CONSTRAINT deals_status_check CHECK (deal_status IN (
    'PROSPECT', 'QUALIFYING', 'NEGOTIATING', 'CONTRACTED',
    'ONBOARDING', 'ACTIVE', 'WINDING_DOWN', 'OFFBOARDED', 'CANCELLED'
));

-- deal_rate_cards.status
ALTER TABLE "ob-poc".deal_rate_cards
ADD CONSTRAINT deal_rate_cards_status_check CHECK (status IN (
    'DRAFT', 'PROPOSED', 'COUNTER_PROPOSED', 'AGREED', 'SUPERSEDED', 'CANCELLED'
));

-- deal_onboarding_requests.request_status
ALTER TABLE "ob-poc".deal_onboarding_requests
ADD CONSTRAINT deal_onboarding_requests_status_check CHECK (request_status IN (
    'PENDING', 'IN_PROGRESS', 'BLOCKED', 'COMPLETED', 'CANCELLED'
));

-- fee_billing_profiles.status
ALTER TABLE "ob-poc".fee_billing_profiles
ADD CONSTRAINT fee_billing_profiles_status_check CHECK (status IN (
    'DRAFT', 'ACTIVE', 'SUSPENDED', 'CLOSED'
));

-- fee_billing_periods.calc_status
ALTER TABLE "ob-poc".fee_billing_periods
ADD CONSTRAINT fee_billing_periods_calc_status_check CHECK (calc_status IN (
    'PENDING', 'CALCULATED', 'REVIEWED', 'APPROVED', 'DISPUTED', 'INVOICED'
));

-- deal_documents.document_status
ALTER TABLE "ob-poc".deal_documents
ADD CONSTRAINT deal_documents_status_check CHECK (document_status IN (
    'DRAFT', 'UNDER_REVIEW', 'SIGNED', 'EXECUTED', 'SUPERSEDED', 'ARCHIVED'
));

-- ============================================================================
-- 2. UUID v7 ALIGNMENT
-- ============================================================================
-- Switch DEFAULT from gen_random_uuid() to uuidv7() for time-ordered PKs
-- uuidv7() is in pg_catalog (public schema), not ob-poc

-- deals
ALTER TABLE "ob-poc".deals
ALTER COLUMN deal_id SET DEFAULT uuidv7();

-- deal_participants (correct column name: deal_participant_id)
ALTER TABLE "ob-poc".deal_participants
ALTER COLUMN deal_participant_id SET DEFAULT uuidv7();

-- deal_rate_cards
ALTER TABLE "ob-poc".deal_rate_cards
ALTER COLUMN rate_card_id SET DEFAULT uuidv7();

-- deal_rate_card_lines
ALTER TABLE "ob-poc".deal_rate_card_lines
ALTER COLUMN line_id SET DEFAULT uuidv7();

-- deal_slas
ALTER TABLE "ob-poc".deal_slas
ALTER COLUMN sla_id SET DEFAULT uuidv7();

-- deal_ubo_assessments
ALTER TABLE "ob-poc".deal_ubo_assessments
ALTER COLUMN assessment_id SET DEFAULT uuidv7();

-- deal_onboarding_requests
ALTER TABLE "ob-poc".deal_onboarding_requests
ALTER COLUMN request_id SET DEFAULT uuidv7();

-- deal_events
ALTER TABLE "ob-poc".deal_events
ALTER COLUMN event_id SET DEFAULT uuidv7();

-- fee_billing_profiles
ALTER TABLE "ob-poc".fee_billing_profiles
ALTER COLUMN profile_id SET DEFAULT uuidv7();

-- fee_billing_account_targets
ALTER TABLE "ob-poc".fee_billing_account_targets
ALTER COLUMN target_id SET DEFAULT uuidv7();

-- fee_billing_periods
ALTER TABLE "ob-poc".fee_billing_periods
ALTER COLUMN period_id SET DEFAULT uuidv7();

-- fee_billing_period_lines (correct column name: period_line_id)
ALTER TABLE "ob-poc".fee_billing_period_lines
ALTER COLUMN period_line_id SET DEFAULT uuidv7();

-- ============================================================================
-- 3. VERIFICATION
-- ============================================================================

DO $$
DECLARE
    constraint_count INTEGER;
    uuid_default_count INTEGER;
BEGIN
    -- Count CHECK constraints added
    SELECT COUNT(*) INTO constraint_count
    FROM information_schema.table_constraints
    WHERE constraint_schema = 'ob-poc'
    AND constraint_type = 'CHECK'
    AND (constraint_name LIKE 'deals_%_check%'
         OR constraint_name LIKE 'deal_%_check%'
         OR constraint_name LIKE 'fee_billing_%_check%');

    RAISE NOTICE 'Deal/billing CHECK constraints: %', constraint_count;

    -- Verify uuidv7 defaults
    SELECT COUNT(*) INTO uuid_default_count
    FROM information_schema.columns
    WHERE table_schema = 'ob-poc'
    AND (table_name LIKE 'deal%' OR table_name LIKE 'fee_billing%')
    AND column_default = 'uuidv7()';

    RAISE NOTICE 'Columns with uuidv7() default: %', uuid_default_count;
END $$;
