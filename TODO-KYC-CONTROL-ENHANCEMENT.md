# KYC Ownership & Control Model Enhancement

## Overview

Extend the existing KYC platform to support comprehensive ownership and control analysis for UBO discovery. This builds on the Clearstream-style registry infrastructure already in place (`kyc.share_classes`, `kyc.holdings`, `kyc.movements`).

### Core Principles
- **Reconciliation**: Voting shares must sum to 100% at each entity
- **Traversal**: CBU upward to terminus (natural person, listed company, or government)
- **Indirect ownership**: Multiplicative through chain (A owns 50% of B, B owns 60% of C → A owns 30% of C)
- **Two paths to UBO**: Ownership (≥25%) OR Control (board appointment rights)
- **Epistemic model**: DISCOVERED → ALLEGED → EVIDENCED → VERIFIED

---

## Phase A: Reference Data - Missing Roles

### A.1 Add C-Suite & Executive Roles

**File**: Create SQL migration `migrations/YYYYMMDD_add_executive_roles.sql`

```sql
INSERT INTO "ob-poc".roles (role_id, name, description, role_category) VALUES
  (gen_random_uuid(), 'CEO', 'Chief Executive Officer - operational control', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'CFO', 'Chief Financial Officer - financial control', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'CIO', 'Chief Investment Officer - investment decisions', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'COO', 'Chief Operating Officer - operations', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'MANAGING_DIRECTOR', 'Managing Director - senior management', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'CHAIRMAN', 'Board Chairman - board control', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'EXECUTIVE_DIRECTOR', 'Executive board member - management + board', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'NON_EXECUTIVE_DIRECTOR', 'Non-executive board member - oversight only', 'OWNERSHIP_CONTROL');
```

### A.2 Add LLP-Specific Roles

```sql
INSERT INTO "ob-poc".roles (role_id, name, description, role_category) VALUES
  (gen_random_uuid(), 'DESIGNATED_MEMBER', 'LLP designated member - statutory signatory', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'MEMBER', 'LLP member (distinct from partner)', 'OWNERSHIP_CONTROL');
```

### A.3 Add Control-Specific Roles

```sql
INSERT INTO "ob-poc".roles (role_id, name, description, role_category) VALUES
  (gen_random_uuid(), 'CONTROLLER', 'De facto control without ownership (SHA provisions, veto rights)', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'POWER_OF_ATTORNEY', 'Legal representative - can act for entity', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'APPOINTOR', 'Trust role - can appoint/remove trustees', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'ENFORCER', 'Trust role - charitable/purpose trust oversight', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'PORTFOLIO_MANAGER', 'Day-to-day investment decisions', 'OWNERSHIP_CONTROL'),
  (gen_random_uuid(), 'KEY_PERSON', 'Subject to key-man clause provisions', 'OWNERSHIP_CONTROL');
```

### A.4 Add Role-to-Entity-Type Validation Table

**File**: Create SQL migration `migrations/YYYYMMDD_role_entity_type_constraints.sql`

```sql
CREATE TABLE "ob-poc".role_applicable_entity_types (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id),
  entity_type VARCHAR(100) NOT NULL,
  created_at TIMESTAMPTZ DEFAULT now(),
  UNIQUE(role_id, entity_type)
);

COMMENT ON TABLE "ob-poc".role_applicable_entity_types IS 
  'Constrains which roles can be assigned to which entity types (e.g., TRUSTEE only for TRUST_*)';

-- Populate constraints
INSERT INTO "ob-poc".role_applicable_entity_types (role_id, entity_type)
SELECT r.role_id, et.entity_type
FROM "ob-poc".roles r
CROSS JOIN (VALUES 
  ('TRUSTEE', 'TRUST_DISCRETIONARY'),
  ('TRUSTEE', 'TRUST_FIXED_INTEREST'),
  ('TRUSTEE', 'TRUST_UNIT'),
  ('TRUSTEE', 'TRUST_CHARITABLE'),
  ('SETTLOR', 'TRUST_DISCRETIONARY'),
  ('SETTLOR', 'TRUST_FIXED_INTEREST'),
  ('SETTLOR', 'TRUST_UNIT'),
  ('SETTLOR', 'TRUST_CHARITABLE'),
  ('BENEFICIARY', 'TRUST_DISCRETIONARY'),
  ('BENEFICIARY', 'TRUST_FIXED_INTEREST'),
  ('BENEFICIARY', 'TRUST_UNIT'),
  ('BENEFICIARY', 'TRUST_CHARITABLE'),
  ('PROTECTOR', 'TRUST_DISCRETIONARY'),
  ('PROTECTOR', 'TRUST_FIXED_INTEREST'),
  ('PROTECTOR', 'TRUST_UNIT'),
  ('PROTECTOR', 'TRUST_CHARITABLE'),
  ('APPOINTOR', 'TRUST_DISCRETIONARY'),
  ('APPOINTOR', 'TRUST_FIXED_INTEREST'),
  ('APPOINTOR', 'TRUST_UNIT'),
  ('APPOINTOR', 'TRUST_CHARITABLE'),
  ('GENERAL_PARTNER', 'PARTNERSHIP_GENERAL'),
  ('GENERAL_PARTNER', 'PARTNERSHIP_LIMITED'),
  ('LIMITED_PARTNER', 'PARTNERSHIP_LIMITED'),
  ('MANAGING_PARTNER', 'PARTNERSHIP_GENERAL'),
  ('MANAGING_PARTNER', 'PARTNERSHIP_LIMITED'),
  ('MANAGING_PARTNER', 'PARTNERSHIP_LLP'),
  ('DESIGNATED_MEMBER', 'PARTNERSHIP_LLP'),
  ('MEMBER', 'PARTNERSHIP_LLP')
) AS et(role_name, entity_type)
WHERE r.name = et.role_name;
```

---

## Phase B: Schema Migrations - Extend Registry & New Tables

### B.1 Extend share_classes for Capital Structure

**File**: `migrations/YYYYMMDD_extend_share_classes_capital.sql`

```sql
-- Extend existing Clearstream-style share_classes for corporate capital structure
ALTER TABLE kyc.share_classes 
  ADD COLUMN IF NOT EXISTS share_type VARCHAR(50),
  ADD COLUMN IF NOT EXISTS authorized_shares NUMERIC(20,0),
  ADD COLUMN IF NOT EXISTS issued_shares NUMERIC(20,0),
  ADD COLUMN IF NOT EXISTS voting_rights_per_share NUMERIC(10,4) DEFAULT 1.0,
  ADD COLUMN IF NOT EXISTS par_value NUMERIC(20,6),
  ADD COLUMN IF NOT EXISTS dividend_rights BOOLEAN DEFAULT true,
  ADD COLUMN IF NOT EXISTS liquidation_preference NUMERIC(20,2);

COMMENT ON COLUMN kyc.share_classes.share_type IS 'ORDINARY, PREFERENCE_A, PREFERENCE_B, DEFERRED, REDEEMABLE, GROWTH, MANAGEMENT';
COMMENT ON COLUMN kyc.share_classes.authorized_shares IS 'Maximum shares authorized in articles';
COMMENT ON COLUMN kyc.share_classes.issued_shares IS 'Actually issued shares - SUM(holdings.units) must equal this';
COMMENT ON COLUMN kyc.share_classes.voting_rights_per_share IS 'Votes per share (0 for non-voting, >1 for super-voting)';
COMMENT ON COLUMN kyc.share_classes.par_value IS 'Nominal/par value per share';

-- Add constraint for share_type
ALTER TABLE kyc.share_classes 
  ADD CONSTRAINT chk_share_type CHECK (
    share_type IS NULL OR share_type IN (
      'ORDINARY', 'PREFERENCE_A', 'PREFERENCE_B', 'DEFERRED', 
      'REDEEMABLE', 'GROWTH', 'MANAGEMENT', 'CONVERTIBLE'
    )
  );
```

### B.2 Create Capital Structure View

**File**: `migrations/YYYYMMDD_capital_structure_view.sql`

```sql
CREATE OR REPLACE VIEW kyc.capital_structure AS
SELECT 
  sc.id AS share_class_id,
  sc.cbu_id,
  sc.issuer_entity_id,
  sc.name AS share_class_name,
  sc.share_type,
  sc.class_category,
  sc.issued_shares,
  sc.voting_rights_per_share,
  h.id AS holding_id,
  h.investor_entity_id,
  h.units,
  h.status AS holding_status,
  -- Ownership calculation
  CASE 
    WHEN sc.issued_shares > 0 THEN ROUND((h.units / sc.issued_shares) * 100, 4)
    ELSE 0
  END AS ownership_pct,
  -- Voting rights calculation
  h.units * COALESCE(sc.voting_rights_per_share, 1) AS voting_rights,
  -- Total voting rights for this share class
  sc.issued_shares * COALESCE(sc.voting_rights_per_share, 1) AS total_class_voting_rights,
  -- Entity details
  e.name AS investor_name,
  e.entity_type AS investor_entity_type,
  ie.name AS issuer_name,
  ie.entity_type AS issuer_entity_type
FROM kyc.share_classes sc
LEFT JOIN kyc.holdings h ON h.share_class_id = sc.id AND h.status = 'active'
LEFT JOIN "ob-poc".entities e ON e.entity_id = h.investor_entity_id
LEFT JOIN "ob-poc".entities ie ON ie.entity_id = sc.issuer_entity_id
WHERE sc.class_category = 'CORPORATE';

COMMENT ON VIEW kyc.capital_structure IS 
  'Computed ownership and voting percentages from share registry';
```

### B.3 Create Board Composition Table

**File**: `migrations/YYYYMMDD_board_composition.sql`

```sql
CREATE TABLE kyc.board_compositions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  person_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  role_id UUID NOT NULL REFERENCES "ob-poc".roles(role_id),
  appointed_by_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
  appointment_date DATE,
  resignation_date DATE,
  is_active BOOLEAN DEFAULT true,
  appointment_document_id UUID,
  notes TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_dates CHECK (resignation_date IS NULL OR resignation_date >= appointment_date)
);

COMMENT ON TABLE kyc.board_compositions IS 
  'Directors/officers appointed to entity boards with appointment chain';

CREATE INDEX idx_board_entity ON kyc.board_compositions(entity_id) WHERE is_active = true;
CREATE INDEX idx_board_person ON kyc.board_compositions(person_entity_id) WHERE is_active = true;
CREATE INDEX idx_board_appointer ON kyc.board_compositions(appointed_by_entity_id);
```

### B.4 Create Appointment Rights Table

**File**: `migrations/YYYYMMDD_appointment_rights.sql`

```sql
CREATE TABLE kyc.appointment_rights (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  target_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  holder_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  right_type VARCHAR(50) NOT NULL,
  appointable_role_id UUID REFERENCES "ob-poc".roles(role_id),
  max_appointments INTEGER,
  source_document_id UUID,
  source_clause TEXT,
  is_active BOOLEAN DEFAULT true,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_right_type CHECK (right_type IN (
    'APPOINT_DIRECTOR', 'REMOVE_DIRECTOR', 'APPOINT_AND_REMOVE',
    'VETO_APPOINTMENT', 'CONSENT_REQUIRED', 'OBSERVER_SEAT'
  ))
);

COMMENT ON TABLE kyc.appointment_rights IS 
  'SHA/articles provisions granting board appointment/removal rights';
COMMENT ON COLUMN kyc.appointment_rights.source_clause IS 
  'Reference to specific clause in SHA/articles (e.g., "Clause 5.2(a)")';

CREATE INDEX idx_appt_rights_target ON kyc.appointment_rights(target_entity_id) WHERE is_active = true;
CREATE INDEX idx_appt_rights_holder ON kyc.appointment_rights(holder_entity_id) WHERE is_active = true;
```

### B.5 Create Trust Provisions Table

**File**: `migrations/YYYYMMDD_trust_provisions.sql`

```sql
CREATE TABLE kyc.trust_provisions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  trust_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  provision_type VARCHAR(50) NOT NULL,
  holder_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
  beneficiary_class TEXT,
  discretion_level VARCHAR(30),
  vesting_conditions TEXT,
  source_document_id UUID,
  is_active BOOLEAN DEFAULT true,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_provision_type CHECK (provision_type IN (
    'INCOME_BENEFICIARY', 'CAPITAL_BENEFICIARY', 'DISCRETIONARY_BENEFICIARY',
    'REMAINDER_BENEFICIARY', 'APPOINTOR_POWER', 'PROTECTOR_POWER',
    'TRUSTEE_REMOVAL', 'TRUST_VARIATION', 'ACCUMULATION_POWER'
  )),
  CONSTRAINT chk_discretion CHECK (discretion_level IS NULL OR discretion_level IN (
    'ABSOLUTE', 'LIMITED', 'NONE'
  ))
);

COMMENT ON TABLE kyc.trust_provisions IS 
  'Trust deed provisions affecting control and beneficial interest';
COMMENT ON COLUMN kyc.trust_provisions.beneficiary_class IS 
  'Description of class (e.g., "descendants of the settlor")';
COMMENT ON COLUMN kyc.trust_provisions.discretion_level IS 
  'Level of trustee discretion over distributions';

CREATE INDEX idx_trust_prov_trust ON kyc.trust_provisions(trust_entity_id) WHERE is_active = true;
CREATE INDEX idx_trust_prov_holder ON kyc.trust_provisions(holder_entity_id);
```

### B.6 Create Partnership Capital Table

**File**: `migrations/YYYYMMDD_partnership_capital.sql`

```sql
CREATE TABLE kyc.partnership_capital (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  partnership_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  partner_entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
  partner_type VARCHAR(30) NOT NULL,
  capital_commitment NUMERIC(20,2),
  capital_contributed NUMERIC(20,2) DEFAULT 0,
  unfunded_commitment NUMERIC(20,2) GENERATED ALWAYS AS (capital_commitment - capital_contributed) STORED,
  profit_share_pct NUMERIC(8,4),
  loss_share_pct NUMERIC(8,4),
  management_rights BOOLEAN DEFAULT false,
  voting_pct NUMERIC(8,4),
  admission_date DATE,
  withdrawal_date DATE,
  is_active BOOLEAN DEFAULT true,
  source_document_id UUID,
  created_at TIMESTAMPTZ DEFAULT now(),
  updated_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_partner_type CHECK (partner_type IN ('GP', 'LP', 'MEMBER')),
  CONSTRAINT chk_capital CHECK (capital_contributed <= capital_commitment),
  UNIQUE(partnership_entity_id, partner_entity_id)
);

COMMENT ON TABLE kyc.partnership_capital IS 
  'Partnership capital accounts and profit/loss allocation';
COMMENT ON COLUMN kyc.partnership_capital.management_rights IS 
  'Whether partner has management rights (always true for GP)';

CREATE INDEX idx_partnership_cap_partnership ON kyc.partnership_capital(partnership_entity_id) WHERE is_active = true;
CREATE INDEX idx_partnership_cap_partner ON kyc.partnership_capital(partner_entity_id) WHERE is_active = true;
```

### B.7 Create Tollgate Evaluation Tables

**File**: `migrations/YYYYMMDD_tollgate_tables.sql`

```sql
-- Tollgate threshold configuration
CREATE TABLE kyc.tollgate_thresholds (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  threshold_name VARCHAR(100) NOT NULL UNIQUE,
  metric_type VARCHAR(50) NOT NULL,
  min_value NUMERIC(10,4),
  max_value NUMERIC(10,4),
  weight NUMERIC(5,2) DEFAULT 1.0,
  is_blocking BOOLEAN DEFAULT false,
  description TEXT,
  created_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT chk_metric_type CHECK (metric_type IN (
    'OWNERSHIP_VERIFIED_PCT', 'CONTROL_VERIFIED_PCT', 'UBO_COVERAGE_PCT',
    'DOC_COMPLETENESS_PCT', 'SCREENING_CLEAR_PCT', 'RED_FLAG_COUNT',
    'ALLEGATION_UNRESOLVED_COUNT', 'DAYS_SINCE_REFRESH'
  ))
);

COMMENT ON TABLE kyc.tollgate_thresholds IS 
  'Configurable thresholds for tollgate pass/fail decisions';

-- Populate default thresholds
INSERT INTO kyc.tollgate_thresholds (threshold_name, metric_type, min_value, is_blocking, description) VALUES
  ('ownership_minimum', 'OWNERSHIP_VERIFIED_PCT', 95.0, true, 'Minimum verified ownership percentage'),
  ('control_minimum', 'CONTROL_VERIFIED_PCT', 100.0, true, 'All control vectors must be verified'),
  ('ubo_coverage', 'UBO_COVERAGE_PCT', 100.0, true, 'All UBOs must be identified'),
  ('doc_completeness', 'DOC_COMPLETENESS_PCT', 90.0, false, 'Target document collection rate'),
  ('screening_clear', 'SCREENING_CLEAR_PCT', 100.0, true, 'All screenings must be clear or escalated'),
  ('max_red_flags', 'RED_FLAG_COUNT', NULL, false, 'Red flags must be addressed'),
  ('max_unresolved_allegations', 'ALLEGATION_UNRESOLVED_COUNT', NULL, true, 'No unresolved ownership allegations');

INSERT INTO kyc.tollgate_thresholds (threshold_name, metric_type, max_value, is_blocking, description) VALUES
  ('red_flag_limit', 'RED_FLAG_COUNT', 0, false, 'Maximum unaddressed red flags'),
  ('allegation_limit', 'ALLEGATION_UNRESOLVED_COUNT', 0, true, 'Maximum unresolved allegations');

-- Tollgate evaluations
CREATE TABLE kyc.tollgate_evaluations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  case_id UUID NOT NULL REFERENCES kyc.cases(case_id),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  evaluation_type VARCHAR(30) NOT NULL,
  evaluated_at TIMESTAMPTZ DEFAULT now(),
  evaluated_by VARCHAR(100),
  overall_result VARCHAR(20) NOT NULL,
  score NUMERIC(5,2),
  metrics JSONB NOT NULL DEFAULT '{}',
  blocking_failures TEXT[],
  warnings TEXT[],
  notes TEXT,
  CONSTRAINT chk_eval_type CHECK (evaluation_type IN (
    'DISCOVERY_COMPLETE', 'EVIDENCE_COMPLETE', 'VERIFICATION_COMPLETE', 
    'DECISION_READY', 'PERIODIC_REVIEW'
  )),
  CONSTRAINT chk_result CHECK (overall_result IN ('PASS', 'FAIL', 'CONDITIONAL', 'OVERRIDE'))
);

COMMENT ON TABLE kyc.tollgate_evaluations IS 
  'Point-in-time tollgate evaluation results';
COMMENT ON COLUMN kyc.tollgate_evaluations.metrics IS 
  'JSON object with all computed metrics at evaluation time';
COMMENT ON COLUMN kyc.tollgate_evaluations.blocking_failures IS 
  'Array of threshold names that caused blocking failure';

CREATE INDEX idx_tollgate_case ON kyc.tollgate_evaluations(case_id);
CREATE INDEX idx_tollgate_cbu ON kyc.tollgate_evaluations(cbu_id);
CREATE INDEX idx_tollgate_result ON kyc.tollgate_evaluations(overall_result);
```

---

## Phase C: Verb YAML Files

### C.1 Capital Domain Verbs

**File**: `rust/config/verbs/kyc/capital.yaml`

```yaml
domains:
  capital:
    description: |
      Corporate capital structure management - extends Clearstream-style registry.
      Wraps share-class.* and holding.* verbs with corporate-specific semantics.
      Enforces reconciliation: SUM(holdings) = issued_shares.
    
    verbs:
      define-share-class:
        description: Define a corporate share class (ordinary, preference, etc.)
        behavior: plugin
        plugin:
          handler: CapitalDefineShareClassOp
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: issuer-entity-id
            type: uuid
            required: true
            lookup:
              table: entities
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: name
            type: string
            required: true
            description: "Share class name (e.g., 'Ordinary Shares', 'A Preference')"
          - name: share-type
            type: string
            required: true
            valid_values: [ORDINARY, PREFERENCE_A, PREFERENCE_B, DEFERRED, REDEEMABLE, GROWTH, MANAGEMENT, CONVERTIBLE]
          - name: authorized-shares
            type: integer
            required: false
            description: "Maximum authorized in articles"
          - name: issued-shares
            type: integer
            required: true
            description: "Actually issued shares"
          - name: voting-rights-per-share
            type: decimal
            required: false
            default: 1.0
            description: "0 = non-voting, >1 = super-voting"
          - name: par-value
            type: decimal
            required: false
          - name: currency
            type: string
            required: false
            default: "GBP"
        returns:
          type: uuid
          name: share_class_id
          capture: true

      allocate:
        description: |
          Allocate shares to a shareholder. Creates holding record.
          Validates total allocations don't exceed issued_shares.
        behavior: plugin
        plugin:
          handler: CapitalAllocateOp
        args:
          - name: share-class-id
            type: uuid
            required: true
            lookup:
              table: share_classes
              schema: kyc
              search_key: name
              primary_key: id
          - name: shareholder-entity-id
            type: uuid
            required: true
            lookup:
              table: entities
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: units
            type: decimal
            required: true
          - name: acquisition-date
            type: date
            required: false
          - name: cost-basis
            type: decimal
            required: false
        returns:
          type: uuid
          name: holding_id
          capture: true

      transfer:
        description: Transfer shares between shareholders (creates movement records)
        behavior: plugin
        plugin:
          handler: CapitalTransferOp
        args:
          - name: share-class-id
            type: uuid
            required: true
          - name: from-entity-id
            type: uuid
            required: true
          - name: to-entity-id
            type: uuid
            required: true
          - name: units
            type: decimal
            required: true
          - name: transfer-date
            type: date
            required: true
          - name: reference
            type: string
            required: true
        returns:
          type: uuid
          name: transfer_id

      reconcile:
        description: |
          Verify capital structure reconciliation:
          - SUM(holdings.units) = issued_shares
          - All shareholders have verified identity
          Returns ownership% and voting% for each holder.
        behavior: plugin
        plugin:
          handler: CapitalReconcileOp
        args:
          - name: entity-id
            type: uuid
            required: true
            description: "The issuing entity to reconcile"
            lookup:
              table: entities
              schema: ob-poc
              search_key: name
              primary_key: entity_id
        returns:
          type: record
          fields:
            - name: is_reconciled
              type: boolean
            - name: issued_shares
              type: integer
            - name: allocated_shares
              type: integer
            - name: unallocated_shares
              type: integer
            - name: shareholders
              type: array
              items:
                - entity_id
                - units
                - ownership_pct
                - voting_pct
                - is_verified

      get-ownership-chain:
        description: |
          Traverse ownership upward from entity to all terminus points.
          Computes indirect ownership percentages multiplicatively.
        behavior: plugin
        plugin:
          handler: CapitalOwnershipChainOp
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: min-ownership-pct
            type: decimal
            required: false
            default: 0.01
            description: "Minimum ownership % to include in chain"
        returns:
          type: record_set
          description: "Array of ownership paths with computed indirect %"

      issue-shares:
        description: Issue additional shares (increases issued_shares)
        behavior: plugin
        plugin:
          handler: CapitalIssueSharesOp
        args:
          - name: share-class-id
            type: uuid
            required: true
          - name: additional-shares
            type: integer
            required: true
          - name: issue-date
            type: date
            required: true
          - name: reason
            type: string
            required: false
        returns:
          type: affected

      cancel-shares:
        description: Cancel/buyback shares (decreases issued_shares)
        behavior: plugin
        plugin:
          handler: CapitalCancelSharesOp
        args:
          - name: share-class-id
            type: uuid
            required: true
          - name: shares-to-cancel
            type: integer
            required: true
          - name: cancel-date
            type: date
            required: true
          - name: reason
            type: string
            required: false
        returns:
          type: affected
```

### C.2 Board Domain Verbs

**File**: `rust/config/verbs/kyc/board.yaml`

```yaml
domains:
  board:
    description: |
      Board composition and appointment rights management.
      Tracks who sits on boards and who has power to appoint/remove.
    
    verbs:
      appoint:
        description: Appoint a person to a board position
        behavior: crud
        crud:
          operation: upsert
          table: board_compositions
          schema: kyc
          returning: id
          conflict_keys:
            - entity_id
            - person_entity_id
            - role_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: entity-id
            type: uuid
            required: true
            maps_to: entity_id
            description: "The entity whose board this is"
          - name: person-entity-id
            type: uuid
            required: true
            maps_to: person_entity_id
            description: "The person being appointed"
          - name: role-id
            type: uuid
            required: true
            maps_to: role_id
            lookup:
              table: roles
              schema: ob-poc
              search_key: name
              primary_key: role_id
          - name: appointed-by-entity-id
            type: uuid
            required: false
            maps_to: appointed_by_entity_id
            description: "Entity exercising appointment right"
          - name: appointment-date
            type: date
            required: false
            maps_to: appointment_date
          - name: appointment-document-id
            type: uuid
            required: false
            maps_to: appointment_document_id
        returns:
          type: uuid
          name: board_appointment_id
          capture: true

      resign:
        description: Record resignation/removal from board
        behavior: crud
        crud:
          operation: update
          table: board_compositions
          schema: kyc
          key: id
        args:
          - name: appointment-id
            type: uuid
            required: true
            maps_to: id
          - name: resignation-date
            type: date
            required: true
            maps_to: resignation_date
          - name: notes
            type: string
            required: false
            maps_to: notes
        set_values:
          is_active: false
        returns:
          type: affected

      list-by-entity:
        description: List current board composition for an entity
        behavior: crud
        crud:
          operation: list_by_fk
          table: board_compositions
          schema: kyc
          fk_col: entity_id
        args:
          - name: entity-id
            type: uuid
            required: true
          - name: include-inactive
            type: boolean
            required: false
            default: false
        returns:
          type: record_set

      list-by-person:
        description: List all board positions held by a person
        behavior: crud
        crud:
          operation: list_by_fk
          table: board_compositions
          schema: kyc
          fk_col: person_entity_id
        args:
          - name: person-entity-id
            type: uuid
            required: true
          - name: include-inactive
            type: boolean
            required: false
            default: false
        returns:
          type: record_set

      grant-appointment-right:
        description: Record an appointment right from SHA/articles
        behavior: crud
        crud:
          operation: upsert
          table: appointment_rights
          schema: kyc
          returning: id
          conflict_keys:
            - target_entity_id
            - holder_entity_id
            - right_type
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: target-entity-id
            type: uuid
            required: true
            maps_to: target_entity_id
            description: "Entity whose board can be affected"
          - name: holder-entity-id
            type: uuid
            required: true
            maps_to: holder_entity_id
            description: "Entity holding the right"
          - name: right-type
            type: string
            required: true
            maps_to: right_type
            valid_values: [APPOINT_DIRECTOR, REMOVE_DIRECTOR, APPOINT_AND_REMOVE, VETO_APPOINTMENT, CONSENT_REQUIRED, OBSERVER_SEAT]
          - name: appointable-role-id
            type: uuid
            required: false
            maps_to: appointable_role_id
          - name: max-appointments
            type: integer
            required: false
            maps_to: max_appointments
          - name: source-document-id
            type: uuid
            required: false
            maps_to: source_document_id
          - name: source-clause
            type: string
            required: false
            maps_to: source_clause
        returns:
          type: uuid
          name: appointment_right_id
          capture: true

      list-appointment-rights:
        description: List who has appointment rights over an entity's board
        behavior: crud
        crud:
          operation: list_by_fk
          table: appointment_rights
          schema: kyc
          fk_col: target_entity_id
        args:
          - name: target-entity-id
            type: uuid
            required: true
        returns:
          type: record_set

      analyze-control:
        description: |
          Analyze board control - who controls the board through:
          - Majority of appointments
          - Appointment/removal rights
          - Veto powers
        behavior: plugin
        plugin:
          handler: BoardAnalyzeControlOp
        args:
          - name: entity-id
            type: uuid
            required: true
        returns:
          type: record
          fields:
            - name: total_seats
              type: integer
            - name: controllers
              type: array
              items:
                - entity_id
                - seats_appointed
                - appointment_rights
                - is_majority_controller
```

### C.3 Trust Domain Verbs

**File**: `rust/config/verbs/kyc/trust.yaml`

```yaml
domains:
  trust:
    description: |
      Trust-specific control and beneficial interest management.
      Maps trust deed provisions to control vectors.
    
    verbs:
      record-provision:
        description: Record a provision from the trust deed affecting control or benefit
        behavior: crud
        crud:
          operation: upsert
          table: trust_provisions
          schema: kyc
          returning: id
          conflict_keys:
            - trust_entity_id
            - provision_type
            - holder_entity_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: trust-entity-id
            type: uuid
            required: true
            maps_to: trust_entity_id
          - name: provision-type
            type: string
            required: true
            maps_to: provision_type
            valid_values:
              - INCOME_BENEFICIARY
              - CAPITAL_BENEFICIARY
              - DISCRETIONARY_BENEFICIARY
              - REMAINDER_BENEFICIARY
              - APPOINTOR_POWER
              - PROTECTOR_POWER
              - TRUSTEE_REMOVAL
              - TRUST_VARIATION
              - ACCUMULATION_POWER
          - name: holder-entity-id
            type: uuid
            required: false
            maps_to: holder_entity_id
            description: "Entity holding this provision/right (null for class beneficiaries)"
          - name: beneficiary-class
            type: string
            required: false
            maps_to: beneficiary_class
            description: "Description of beneficiary class if not specific entity"
          - name: discretion-level
            type: string
            required: false
            maps_to: discretion_level
            valid_values: [ABSOLUTE, LIMITED, NONE]
          - name: vesting-conditions
            type: string
            required: false
            maps_to: vesting_conditions
          - name: source-document-id
            type: uuid
            required: false
            maps_to: source_document_id
        returns:
          type: uuid
          name: provision_id
          capture: true

      list-provisions:
        description: List all provisions for a trust
        behavior: crud
        crud:
          operation: list_by_fk
          table: trust_provisions
          schema: kyc
          fk_col: trust_entity_id
        args:
          - name: trust-entity-id
            type: uuid
            required: true
          - name: provision-type
            type: string
            required: false
            maps_to: provision_type
        returns:
          type: record_set

      analyze-control:
        description: |
          Analyze trust control vectors:
          - Trustee with absolute discretion = control
          - Appointor with trustee removal = control
          - Protector with veto = influence
        behavior: plugin
        plugin:
          handler: TrustAnalyzeControlOp
        args:
          - name: trust-entity-id
            type: uuid
            required: true
        returns:
          type: record
          fields:
            - name: controlling_parties
              type: array
              items:
                - entity_id
                - role
                - control_vector
                - control_strength
            - name: beneficial_parties
              type: array
              items:
                - entity_id
                - benefit_type
                - is_fixed_interest
            - name: trust_type_inference
              type: string
              description: "DISCRETIONARY, FIXED_INTEREST, BARE"

      identify-ubos:
        description: |
          Identify UBOs from trust structure:
          - Fixed interest beneficiaries ≥25%
          - Settlor (if reserved powers)
          - Trustee with absolute discretion
          - Appointor/Protector with removal power
        behavior: plugin
        plugin:
          handler: TrustIdentifyUbosOp
        args:
          - name: trust-entity-id
            type: uuid
            required: true
        returns:
          type: record_set
          description: "Array of UBO candidates with control/ownership vector"
```

### C.4 Partnership Domain Verbs

**File**: `rust/config/verbs/kyc/partnership.yaml`

```yaml
domains:
  partnership:
    description: |
      Partnership capital accounts and control structure.
      Models GP/LP dynamics and profit/loss allocation.
    
    verbs:
      add-partner:
        description: Add a partner to the partnership with capital commitment
        behavior: crud
        crud:
          operation: upsert
          table: partnership_capital
          schema: kyc
          returning: id
          conflict_keys:
            - partnership_entity_id
            - partner_entity_id
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: partnership-entity-id
            type: uuid
            required: true
            maps_to: partnership_entity_id
          - name: partner-entity-id
            type: uuid
            required: true
            maps_to: partner_entity_id
          - name: partner-type
            type: string
            required: true
            maps_to: partner_type
            valid_values: [GP, LP, MEMBER]
          - name: capital-commitment
            type: decimal
            required: true
            maps_to: capital_commitment
          - name: profit-share-pct
            type: decimal
            required: true
            maps_to: profit_share_pct
          - name: loss-share-pct
            type: decimal
            required: false
            maps_to: loss_share_pct
            description: "Defaults to profit-share-pct if not specified"
          - name: voting-pct
            type: decimal
            required: false
            maps_to: voting_pct
          - name: management-rights
            type: boolean
            required: false
            maps_to: management_rights
            description: "Auto-true for GP, default false for LP"
          - name: admission-date
            type: date
            required: false
            maps_to: admission_date
          - name: source-document-id
            type: uuid
            required: false
            maps_to: source_document_id
        returns:
          type: uuid
          name: partner_capital_id
          capture: true

      record-contribution:
        description: Record capital contribution (drawdown)
        behavior: plugin
        plugin:
          handler: PartnershipContributionOp
        args:
          - name: partnership-entity-id
            type: uuid
            required: true
          - name: partner-entity-id
            type: uuid
            required: true
          - name: amount
            type: decimal
            required: true
          - name: contribution-date
            type: date
            required: true
          - name: reference
            type: string
            required: false
        returns:
          type: uuid
          name: contribution_id

      record-distribution:
        description: Record distribution to partner
        behavior: plugin
        plugin:
          handler: PartnershipDistributionOp
        args:
          - name: partnership-entity-id
            type: uuid
            required: true
          - name: partner-entity-id
            type: uuid
            required: true
          - name: amount
            type: decimal
            required: true
          - name: distribution-type
            type: string
            required: true
            valid_values: [PROFIT, RETURN_OF_CAPITAL, LIQUIDATION]
          - name: distribution-date
            type: date
            required: true
        returns:
          type: uuid
          name: distribution_id

      withdraw-partner:
        description: Record partner withdrawal/exit
        behavior: crud
        crud:
          operation: update
          table: partnership_capital
          schema: kyc
        args:
          - name: partnership-entity-id
            type: uuid
            required: true
          - name: partner-entity-id
            type: uuid
            required: true
          - name: withdrawal-date
            type: date
            required: true
            maps_to: withdrawal_date
        set_values:
          is_active: false
        returns:
          type: affected

      list-partners:
        description: List all partners in a partnership
        behavior: crud
        crud:
          operation: list_by_fk
          table: partnership_capital
          schema: kyc
          fk_col: partnership_entity_id
        args:
          - name: partnership-entity-id
            type: uuid
            required: true
          - name: partner-type
            type: string
            required: false
            maps_to: partner_type
          - name: include-inactive
            type: boolean
            required: false
            default: false
        returns:
          type: record_set

      reconcile:
        description: |
          Reconcile partnership capital:
          - Verify profit shares sum to 100%
          - Calculate total committed/contributed/unfunded
          - Identify control (GP or majority LP)
        behavior: plugin
        plugin:
          handler: PartnershipReconcileOp
        args:
          - name: partnership-entity-id
            type: uuid
            required: true
        returns:
          type: record
          fields:
            - name: is_reconciled
              type: boolean
            - name: total_commitment
              type: decimal
            - name: total_contributed
              type: decimal
            - name: total_unfunded
              type: decimal
            - name: profit_share_total
              type: decimal
            - name: controlling_partners
              type: array
              items:
                - entity_id
                - partner_type
                - control_basis
```

### C.5 Tollgate Domain Verbs

**File**: `rust/config/verbs/kyc/tollgate.yaml`

```yaml
domains:
  tollgate:
    description: |
      Decision engine for KYC workflow gates.
      Evaluates verification completeness against configurable thresholds.
    
    verbs:
      evaluate:
        description: |
          Run tollgate evaluation for a case.
          Computes all metrics and compares against thresholds.
        behavior: plugin
        plugin:
          handler: TollgateEvaluateOp
        args:
          - name: case-id
            type: uuid
            required: true
            lookup:
              table: cases
              schema: kyc
              search_key: case_id
              primary_key: case_id
          - name: evaluation-type
            type: string
            required: true
            valid_values:
              - DISCOVERY_COMPLETE
              - EVIDENCE_COMPLETE
              - VERIFICATION_COMPLETE
              - DECISION_READY
              - PERIODIC_REVIEW
          - name: evaluated-by
            type: string
            required: false
            description: "User/system performing evaluation"
        returns:
          type: record
          fields:
            - name: evaluation_id
              type: uuid
            - name: overall_result
              type: string
            - name: score
              type: decimal
            - name: metrics
              type: json
            - name: blocking_failures
              type: array
            - name: warnings
              type: array

      get-metrics:
        description: Compute current metrics for a CBU without recording evaluation
        behavior: plugin
        plugin:
          handler: TollgateGetMetricsOp
        args:
          - name: cbu-id
            type: uuid
            required: true
        returns:
          type: record
          fields:
            - name: ownership_verified_pct
              type: decimal
            - name: control_verified_pct
              type: decimal
            - name: ubo_coverage_pct
              type: decimal
            - name: doc_completeness_pct
              type: decimal
            - name: screening_clear_pct
              type: decimal
            - name: red_flag_count
              type: integer
            - name: allegation_unresolved_count
              type: integer
            - name: days_since_last_refresh
              type: integer

      set-threshold:
        description: Update a tollgate threshold
        behavior: crud
        crud:
          operation: update
          table: tollgate_thresholds
          schema: kyc
          key: id
        args:
          - name: threshold-name
            type: string
            required: true
            maps_to: threshold_name
          - name: min-value
            type: decimal
            required: false
            maps_to: min_value
          - name: max-value
            type: decimal
            required: false
            maps_to: max_value
          - name: is-blocking
            type: boolean
            required: false
            maps_to: is_blocking
        returns:
          type: affected

      override:
        description: Record management override of tollgate failure
        behavior: plugin
        plugin:
          handler: TollgateOverrideOp
        args:
          - name: evaluation-id
            type: uuid
            required: true
          - name: override-reason
            type: string
            required: true
          - name: approved-by
            type: string
            required: true
          - name: conditions
            type: string
            required: false
            description: "Any conditions attached to override"
        returns:
          type: uuid
          name: override_id

      list-evaluations:
        description: List tollgate evaluations for a case
        behavior: crud
        crud:
          operation: list_by_fk
          table: tollgate_evaluations
          schema: kyc
          fk_col: case_id
        args:
          - name: case-id
            type: uuid
            required: true
          - name: evaluation-type
            type: string
            required: false
            maps_to: evaluation_type
        returns:
          type: record_set

      get-decision-readiness:
        description: |
          Summary view of decision readiness:
          - What's blocking?
          - What's complete?
          - What needs attention?
        behavior: plugin
        plugin:
          handler: TollgateDecisionReadinessOp
        args:
          - name: case-id
            type: uuid
            required: true
        returns:
          type: record
          fields:
            - name: is_decision_ready
              type: boolean
            - name: blocking_issues
              type: array
            - name: completion_summary
              type: json
            - name: recommended_actions
              type: array
```

### C.6 Control Domain Verbs (Unified Analysis)

**File**: `rust/config/verbs/kyc/control.yaml`

Note: A stub of this file may already exist. This should be merged/extended.

```yaml
domains:
  control:
    description: |
      Unified control analysis across all entity types.
      Aggregates ownership, board, trust, and partnership control vectors.
    
    verbs:
      analyze:
        description: |
          Comprehensive control analysis for any entity type.
          Routes to appropriate analyzer based on entity_type.
        behavior: plugin
        plugin:
          handler: ControlAnalyzeOp
        args:
          - name: entity-id
            type: uuid
            required: true
            lookup:
              table: entities
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: include-indirect
            type: boolean
            required: false
            default: true
            description: "Include indirect control through chains"
        returns:
          type: record
          fields:
            - name: entity_type
              type: string
            - name: ownership_controllers
              type: array
              description: "Entities with ≥25% ownership"
            - name: board_controllers
              type: array
              description: "Entities with board control"
            - name: other_controllers
              type: array
              description: "SHA rights, trust powers, etc."
            - name: ubo_candidates
              type: array
              description: "Natural persons identified as potential UBOs"

      build-graph:
        description: |
          Build full control graph for a CBU.
          Nodes: entities, persons, positions
          Edges: OWNS, CONTROLS, APPOINTS, INFLUENCES, HOLDS_POSITION
        behavior: plugin
        plugin:
          handler: ControlBuildGraphOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: depth
            type: integer
            required: false
            default: 10
            description: "Maximum traversal depth"
        returns:
          type: record
          fields:
            - name: nodes
              type: array
            - name: edges
              type: array
            - name: terminus_entities
              type: array
              description: "Entities with no further upstream owners"

      identify-ubos:
        description: |
          Identify all UBOs for a CBU across all control vectors:
          - Ownership ≥25% (direct or indirect)
          - Board majority control
          - Trust control (trustee discretion, appointor power)
          - Partnership GP control
        behavior: plugin
        plugin:
          handler: ControlIdentifyUbosOp
        args:
          - name: cbu-id
            type: uuid
            required: true
        returns:
          type: record_set
          fields:
            - name: person_entity_id
              type: uuid
            - name: control_vectors
              type: array
              description: "Array of {type, path, percentage}"
            - name: verification_status
              type: string
            - name: evidence_documents
              type: array
```

---

## Phase D: Test Scenarios

### D.1 Capital Structure Test

**File**: `rust/tests/scenarios/valid/11_capital_structure.dsl`

```lisp
;; Capital Structure Test
;; Tests corporate share registry and ownership reconciliation

(cbu.create
    :name "Capital Structure Test Co"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

;; Create the company
(entity.create-limited-company
    :cbu-id @cbu
    :name "Test Holdings Ltd"
    :company-number "UK999888"
    :jurisdiction "GB"
    :as @company)

;; Create shareholders
(entity.create-limited-company
    :cbu-id @cbu
    :name "Majority Investor Ltd"
    :company-number "UK999777"
    :jurisdiction "GB"
    :as @majority-holder)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "John"
    :last-name "Founder"
    :date-of-birth "1970-01-01"
    :nationality "GB"
    :as @founder)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Jane"
    :last-name "Angel"
    :date-of-birth "1980-05-15"
    :nationality "US"
    :as @angel)

;; Define share classes
(capital.define-share-class
    :cbu-id @cbu
    :issuer-entity-id @company
    :name "Ordinary Shares"
    :share-type "ORDINARY"
    :issued-shares 1000000
    :voting-rights-per-share 1.0
    :par-value 0.01
    :as @ordinary)

(capital.define-share-class
    :cbu-id @cbu
    :issuer-entity-id @company
    :name "A Preference Shares"
    :share-type "PREFERENCE_A"
    :issued-shares 500000
    :voting-rights-per-share 0
    :par-value 1.00
    :as @pref-a)

;; Allocate shares
(capital.allocate
    :share-class-id @ordinary
    :shareholder-entity-id @majority-holder
    :units 600000
    :acquisition-date "2020-01-01")

(capital.allocate
    :share-class-id @ordinary
    :shareholder-entity-id @founder
    :units 300000
    :acquisition-date "2020-01-01")

(capital.allocate
    :share-class-id @ordinary
    :shareholder-entity-id @angel
    :units 100000
    :acquisition-date "2021-06-15")

(capital.allocate
    :share-class-id @pref-a
    :shareholder-entity-id @majority-holder
    :units 500000
    :acquisition-date "2022-03-01")

;; Reconcile
(capital.reconcile
    :entity-id @company
    :as @reconciliation)

;; Verify ownership chain
(capital.get-ownership-chain
    :entity-id @company
    :as @chain)
```

### D.2 Board Control Test

**File**: `rust/tests/scenarios/valid/12_board_control.dsl`

```lisp
;; Board Control Test
;; Tests board composition and appointment rights

(cbu.create
    :name "Board Control Test"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Board Test Ltd"
    :company-number "UK888777"
    :jurisdiction "GB"
    :as @company)

;; Create investors with appointment rights
(entity.create-limited-company
    :cbu-id @cbu
    :name "PE Fund I LP"
    :company-number "KY12345"
    :jurisdiction "KY"
    :as @pe-fund)

;; Create directors
(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Alice"
    :last-name "Chair"
    :date-of-birth "1965-03-20"
    :nationality "GB"
    :as @alice)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Bob"
    :last-name "Director"
    :date-of-birth "1975-08-10"
    :nationality "GB"
    :as @bob)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Carol"
    :last-name "Nominee"
    :date-of-birth "1980-12-01"
    :nationality "US"
    :as @carol)

;; Grant appointment rights (from SHA)
(board.grant-appointment-right
    :cbu-id @cbu
    :target-entity-id @company
    :holder-entity-id @pe-fund
    :right-type "APPOINT_AND_REMOVE"
    :max-appointments 2
    :source-clause "Clause 5.2(a)")

;; Make appointments
(board.appoint
    :cbu-id @cbu
    :entity-id @company
    :person-entity-id @alice
    :role-id "CHAIRMAN"
    :appointment-date "2020-01-15")

(board.appoint
    :cbu-id @cbu
    :entity-id @company
    :person-entity-id @bob
    :role-id "EXECUTIVE_DIRECTOR"
    :appointment-date "2020-01-15")

(board.appoint
    :cbu-id @cbu
    :entity-id @company
    :person-entity-id @carol
    :role-id "NON_EXECUTIVE_DIRECTOR"
    :appointed-by-entity-id @pe-fund
    :appointment-date "2021-06-01")

;; Analyze board control
(board.analyze-control
    :entity-id @company
    :as @control-analysis)
```

### D.3 Trust Structure Test

**File**: `rust/tests/scenarios/valid/13_trust_control.dsl`

```lisp
;; Trust Control Test
;; Tests trust provisions and UBO identification

(cbu.create
    :name "Trust Structure Test"
    :client-type "corporate"
    :jurisdiction "JE"
    :as @cbu)

;; Create the trust
(entity.create-trust
    :cbu-id @cbu
    :name "Smith Family Trust"
    :trust-type "DISCRETIONARY"
    :jurisdiction "JE"
    :as @trust)

;; Create trust parties
(entity.create-proper-person
    :cbu-id @cbu
    :first-name "William"
    :last-name "Smith"
    :date-of-birth "1950-06-15"
    :nationality "GB"
    :as @settlor)

(entity.create-limited-company
    :cbu-id @cbu
    :name "ABC Trustees Ltd"
    :company-number "JE98765"
    :jurisdiction "JE"
    :as @trustee-co)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "James"
    :last-name "Smith"
    :date-of-birth "1975-09-20"
    :nationality "GB"
    :as @beneficiary-1)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Emma"
    :last-name "Smith"
    :date-of-birth "1978-04-10"
    :nationality "GB"
    :as @beneficiary-2)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Richard"
    :last-name "Protector"
    :date-of-birth "1960-11-30"
    :nationality "GB"
    :as @protector)

;; Record trust provisions
(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "DISCRETIONARY_BENEFICIARY"
    :holder-entity-id @beneficiary-1
    :discretion-level "ABSOLUTE")

(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "DISCRETIONARY_BENEFICIARY"
    :holder-entity-id @beneficiary-2
    :discretion-level "ABSOLUTE")

(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "PROTECTOR_POWER"
    :holder-entity-id @protector)

(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "TRUSTEE_REMOVAL"
    :holder-entity-id @protector)

;; Assign roles
(cbu.assign-role
    :cbu-id @cbu
    :entity-id @settlor
    :role "SETTLOR"
    :target-entity-id @trust)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @trustee-co
    :role "TRUSTEE"
    :target-entity-id @trust)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @protector
    :role "PROTECTOR"
    :target-entity-id @trust)

;; Analyze trust control
(trust.analyze-control
    :trust-entity-id @trust
    :as @trust-control)

;; Identify UBOs
(trust.identify-ubos
    :trust-entity-id @trust
    :as @trust-ubos)
```

### D.4 Partnership Test

**File**: `rust/tests/scenarios/valid/14_partnership_capital.dsl`

```lisp
;; Partnership Capital Test
;; Tests LP/GP structure and capital accounts

(cbu.create
    :name "Partnership Test"
    :client-type "fund"
    :jurisdiction "KY"
    :as @cbu)

;; Create the LP
(entity.create-partnership
    :cbu-id @cbu
    :name "Growth Fund I LP"
    :partnership-type "LIMITED"
    :jurisdiction "KY"
    :as @fund-lp)

;; Create GP
(entity.create-limited-company
    :cbu-id @cbu
    :name "Growth GP Ltd"
    :company-number "KY54321"
    :jurisdiction "KY"
    :as @gp)

;; Create LPs
(entity.create-limited-company
    :cbu-id @cbu
    :name "Pension Fund A"
    :company-number "US11111"
    :jurisdiction "US"
    :as @lp-pension)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Sovereign Wealth Fund B"
    :company-number "AE22222"
    :jurisdiction "AE"
    :as @lp-swf)

;; Add partners
(partnership.add-partner
    :cbu-id @cbu
    :partnership-entity-id @fund-lp
    :partner-entity-id @gp
    :partner-type "GP"
    :capital-commitment 1000000
    :profit-share-pct 20.0
    :management-rights true
    :admission-date "2023-01-01")

(partnership.add-partner
    :cbu-id @cbu
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-pension
    :partner-type "LP"
    :capital-commitment 50000000
    :profit-share-pct 50.0
    :admission-date "2023-01-01")

(partnership.add-partner
    :cbu-id @cbu
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-swf
    :partner-type "LP"
    :capital-commitment 30000000
    :profit-share-pct 30.0
    :admission-date "2023-03-15")

;; Record contributions
(partnership.record-contribution
    :partnership-entity-id @fund-lp
    :partner-entity-id @gp
    :amount 200000
    :contribution-date "2023-02-01"
    :reference "GP-DRAW-001")

(partnership.record-contribution
    :partnership-entity-id @fund-lp
    :partner-entity-id @lp-pension
    :amount 10000000
    :contribution-date "2023-02-01"
    :reference "LP-DRAW-001")

;; Reconcile
(partnership.reconcile
    :partnership-entity-id @fund-lp
    :as @partnership-recon)
```

### D.5 Tollgate Workflow Test

**File**: `rust/tests/scenarios/valid/15_tollgate_workflow.dsl`

```lisp
;; Tollgate Workflow Test
;; Tests end-to-end KYC with tollgate evaluation

(cbu.create
    :name "Tollgate Test Client"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

;; Create corporate structure
(entity.create-limited-company
    :cbu-id @cbu
    :name "Tollgate Test Ltd"
    :company-number "UK777666"
    :jurisdiction "GB"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Test"
    :last-name "UBO"
    :date-of-birth "1970-01-01"
    :nationality "GB"
    :as @ubo)

;; Create KYC case
(kyc-case.create
    :cbu-id @cbu
    :case-type "NEW_CLIENT"
    :as @case)

;; Workstreams
(entity-workstream.create
    :case-id @case
    :entity-id @company
    :as @ws-company)

(entity-workstream.create
    :case-id @case
    :entity-id @ubo
    :discovery-reason "BENEFICIAL_OWNER"
    :ownership-percentage 100
    :is-ubo true
    :as @ws-ubo)

;; Documents
(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "CERTIFICATE_OF_INCORPORATION"
    :as @cert)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "REGISTER_OF_SHAREHOLDERS"
    :as @register)

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PASSPORT"
    :as @passport)

;; Capital structure
(capital.define-share-class
    :cbu-id @cbu
    :issuer-entity-id @company
    :name "Ordinary Shares"
    :share-type "ORDINARY"
    :issued-shares 100
    :voting-rights-per-share 1.0
    :as @shares)

(capital.allocate
    :share-class-id @shares
    :shareholder-entity-id @ubo
    :units 100)

;; Screenings
(case-screening.run
    :workstream-id @ws-ubo
    :screening-type "PEP"
    :as @pep)

(case-screening.run
    :workstream-id @ws-ubo
    :screening-type "SANCTIONS"
    :as @sanctions)

;; Record allegation and proof for ownership
(allegation.record
    :cbu-id @cbu
    :subject-entity-id @ubo
    :allegation-type "OWNERSHIP"
    :target-entity-id @company
    :percentage 100.0
    :source "REGISTER_OF_SHAREHOLDERS"
    :as @ownership-allegation)

;; First tollgate - should fail (evidence not complete)
(tollgate.evaluate
    :case-id @case
    :evaluation-type "EVIDENCE_COMPLETE"
    :as @eval-1)

;; Submit proof
(proof.submit
    :edge-id @ownership-allegation
    :document-id @register
    :proof-type "REGISTER_EXTRACT"
    :verified-value 100.0)

;; Second tollgate - should pass
(tollgate.evaluate
    :case-id @case
    :evaluation-type "EVIDENCE_COMPLETE"
    :as @eval-2)

;; Get decision readiness
(tollgate.get-decision-readiness
    :case-id @case
    :as @readiness)
```

### D.6 Integrate into KYC Convergence Tests

**File**: Extend `rust/tests/kyc_convergence_integration.rs`

Add test functions that:
1. Execute the new DSL scenarios
2. Verify capital reconciliation works
3. Verify board control analysis
4. Verify trust UBO identification
5. Verify tollgate pass/fail logic

---

## Phase E: Documentation

### E.1 Update CLAUDE.md

Add section covering:
- New verb domains (capital, board, trust, partnership, tollgate)
- Control graph model
- Tollgate evaluation flow
- Test scenario locations

### E.2 Create Domain Documentation

**File**: `docs/kyc-control-model.md`

Document:
- Ownership vs Control paths to UBO
- Epistemic model (allegation → proof → verification)
- Entity type decision matrix
- Role-to-entity-type constraints
- Tollgate thresholds and decision logic

---

## Implementation Notes

### Plugin Handlers Required

The following Rust plugin handlers need implementation:

```
src/dsl_v2/plugins/
├── capital/
│   ├── mod.rs
│   ├── define_share_class.rs
│   ├── allocate.rs
│   ├── transfer.rs
│   ├── reconcile.rs
│   └── ownership_chain.rs
├── board/
│   ├── mod.rs
│   └── analyze_control.rs
├── trust/
│   ├── mod.rs
│   ├── analyze_control.rs
│   └── identify_ubos.rs
├── partnership/
│   ├── mod.rs
│   ├── contribution.rs
│   ├── distribution.rs
│   └── reconcile.rs
├── tollgate/
│   ├── mod.rs
│   ├── evaluate.rs
│   ├── get_metrics.rs
│   ├── override.rs
│   └── decision_readiness.rs
└── control/
    ├── mod.rs
    ├── analyze.rs
    ├── build_graph.rs
    └── identify_ubos.rs
```

### GLEIF Integration (Future)

The `gleif.import-tree` verb should be enhanced to emit allegations:

```yaml
# Future enhancement - macro-style integration
gleif.import-tree:
  post_actions:
    - verb: allegation.record
      for_each: discovered_ownership_edge
      args:
        allegation-type: OWNERSHIP
        source: GLEIF
        status: DISCOVERED
```

This is noted as a future enhancement, keeping GLEIF verbs separate for now.

---

## Execution Order

1. **A.1-A.4**: Role reference data (can run immediately)
2. **B.1-B.2**: Extend share_classes + view (depends on nothing)
3. **B.3-B.6**: New tables (depends on nothing)
4. **B.7**: Tollgate tables (depends on nothing)
5. **C.1-C.6**: Verb YAML files (depends on B.*)
6. **Plugin implementations**: Rust code (depends on C.*)
7. **D.1-D.6**: Test scenarios (depends on all above)
8. **E.1-E.2**: Documentation (can run in parallel)

---

## Validation Checklist

- [ ] All migrations run without error
- [ ] `cargo build` succeeds with new plugin stubs
- [ ] Each test scenario parses without error
- [ ] `capital.reconcile` correctly validates SUM(holdings) = issued_shares
- [ ] `board.analyze-control` identifies majority controller
- [ ] `trust.identify-ubos` finds all control vectors
- [ ] `partnership.reconcile` validates profit shares sum to 100%
- [ ] `tollgate.evaluate` returns correct pass/fail
- [ ] Existing KYC tests still pass
- [ ] New tests integrated into `rust/tests/scenarios/run_tests.sh`
