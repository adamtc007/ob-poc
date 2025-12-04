# KYC/UBO DSL Transition & Implementation Plan

## Goals
- Enforce staged flow for CBU and KYC/UBO: discover → prove → assess → decision.
- Add evidence-aware verbs and guarded transitions (no jumps to APPROVED/COMPLETE without prerequisites).
- Tie UBO discovery/proof to ownership thresholds and documents.
- Roll red-flag severity into case outcomes, including regulator referral / do-not-onboard endpoints.

## Scope Overview
- State machines for `kyc-case`, `entity-workstream`, and `cbu`.
- UBO verification lifecycle (suspected → proven → removed).
- Red-flag aggregation with thresholds that drive decisions.
- Document-request lifecycle guards and defaults.
- Rules/tests to enforce the above.

---

## Gap Analysis (What Exists vs What's Needed)

### Already Implemented ✅

#### KYC Case Management
- `kyc-case` domain with verbs: `create`, `update-status`, `escalate`, `assign`, `set-risk-rating`, `close`, `read`, `list-by-cbu`
- Case statuses: `INTAKE`, `DISCOVERY`, `ASSESSMENT`, `REVIEW`, `APPROVED`, `REJECTED`, `BLOCKED`, `WITHDRAWN`, `EXPIRED`
- Escalation levels: `STANDARD`, `SENIOR_COMPLIANCE`, `EXECUTIVE`, `BOARD`
- Case snapshots table (`kyc.case_snapshots`) with versioning
- Transition validation function `kyc.is_valid_case_transition()`

#### Entity Workstreams
- `entity-workstream` domain with verbs: `create`, `update-status`, `block`, `complete`, `set-enhanced-dd`, `set-ubo`, `list-by-case`, `read`
- Workstream statuses: `PENDING`, `COLLECT`, `VERIFY`, `SCREEN`, `ASSESS`, `COMPLETE`, `BLOCKED`, `ENHANCED_DD`
- Discovery tracking: `discovery_source_workstream_id`, `discovery_reason`, `discovery_depth`
- UBO flags: `is_ubo`, `ownership_percentage`

#### Red Flags
- `red-flag` domain with verbs: `raise`, `mitigate`, `waive`, `dismiss`, `set-blocking`, `list-by-case`, `list-by-workstream`
- Severity levels: `SOFT`, `ESCALATE`, `HARD_STOP`
- Status tracking: `OPEN`, `UNDER_REVIEW`, `MITIGATED`, `WAIVED`, `BLOCKING`, `CLOSED`

#### Document Requests
- `doc-request` domain with verbs: `create`, `mark-requested`, `receive`, `verify`, `reject`, `waive`, `list-by-workstream`
- Statuses: `REQUIRED`, `REQUESTED`, `RECEIVED`, `UNDER_REVIEW`, `VERIFIED`, `REJECTED`, `WAIVED`, `EXPIRED`
- RFI batch generation via `kyc.generate_doc_requests_from_threshold()`

#### UBO Management
- `ubo` domain with verbs: `add-ownership`, `update-ownership`, `end-ownership`, `list-owners`, `list-owned`, `register-ubo`, `verify-ubo`, `list-ubos`, `list-by-subject`, `calculate`, `discover-owner`, `infer-chain`, `trace-chains`, `check-completeness`, `supersede-ubo`, `close-ubo`, `snapshot-cbu`, `compare-snapshot`, `list-snapshots`
- UBO registry with `case_id`, `workstream_id`, `discovery_method`, `superseded_by`, `closed_at`, `closed_reason`
- Ownership chain computation: `compute_ownership_chains()`, `check_ubo_completeness()`, `capture_ubo_snapshot()`

#### Threshold System
- `threshold` domain with verbs: `derive`, `evaluate`, `check-entity`
- Tables: `threshold_factors`, `risk_bands`, `threshold_requirements`, `requirement_acceptable_docs`, `screening_requirements`
- Risk score computation: `compute_cbu_risk_score()`

#### Screenings
- `case-screening` domain with verbs: `run`, `complete`, `review-hit`, `list-by-workstream`
- Types: `SANCTIONS`, `PEP`, `ADVERSE_MEDIA`, `CREDIT`, `CRIMINAL`, `REGULATORY`, `CONSOLIDATED`

### Gaps to Implement 🔴

#### Phase 1: State Machine Guards & New Statuses
| Item | Status | Action Required |
|------|--------|-----------------|
| Case statuses `REFER_TO_REGULATOR`, `DO_NOT_ONBOARD` | ❌ Missing | Add to DB CHECK constraint + verbs.yaml valid_values |
| Workstream statuses `REFERRED`, `PROHIBITED` | ❌ Missing | Add to DB CHECK constraint + verbs.yaml valid_values |
| Transition precondition validation (plugin) | ❌ Missing | Add plugin handler for guarded transitions |
| Doc-request `DRAFT` initial status | ❌ Missing | Add to CHECK constraint, update create verb default |

#### Phase 2: CBU Evidence Lifecycle
| Item | Status | Action Required |
|------|--------|-----------------|
| CBU `status` column | ❌ Missing | Add column to `cbus` table |
| CBU statuses (DISCOVERED, VALIDATION_PENDING, etc.) | ❌ Missing | Add CHECK constraint |
| `cbu.set-status` verb | ❌ Missing | Add to verbs.yaml with transition validation |
| `cbu.attach-evidence` verb | ❌ Missing | Add verb + junction table `cbu_evidence` |
| `cbu.log-change` verb | ❌ Missing | Add verb + audit table `cbu_change_log` |

#### Phase 3: UBO Discovery & Proof
| Item | Status | Action Required |
|------|--------|-----------------|
| UBO `verification_status` values for SUSPECTED/PROVEN/REMOVED | ⚠️ Partial | Current: PENDING/VERIFIED/FAILED/DISPUTED. Add SUSPECTED, PROVEN, REMOVED |
| `ubo.assert` verb | ❌ Missing | Add verb (creates SUSPECTED UBO) |
| `ubo.prove` verb | ❌ Missing | Add verb (transitions to PROVEN, requires evidence) |
| `ubo.remove` verb | ⚠️ Partial | `close-ubo` exists but needs reason/replacement linkage |
| Evidence doc linking for UBO proof | ❌ Missing | Add `evidence_doc_ids` array or junction table |

#### Phase 4: Red-Flag Aggregation
| Item | Status | Action Required |
|------|--------|-----------------|
| `red-flag.aggregate` verb | ❌ Missing | Add plugin verb to compute case-level scores |
| `kyc-case.apply-decision` verb | ❌ Missing | Add verb to map evaluation → status transition |
| Red-flag score thresholds config | ❌ Missing | Add to `threshold_factors` or new config table |
| Auto-escalation rules | ⚠️ Partial | Rule engine exists but needs red-flag score triggers |

---

## Revised Implementation Plan

### Phase 1: State Machine Guards & New Statuses

**SQL Migration (`012_state_machine_guards.sql`):**
```sql
-- Add new case statuses
ALTER TABLE kyc.cases DROP CONSTRAINT IF EXISTS chk_case_status;
ALTER TABLE kyc.cases ADD CONSTRAINT chk_case_status CHECK (
  status IN ('INTAKE', 'DISCOVERY', 'ASSESSMENT', 'REVIEW', 
             'APPROVED', 'REJECTED', 'BLOCKED', 'WITHDRAWN', 'EXPIRED',
             'REFER_TO_REGULATOR', 'DO_NOT_ONBOARD')
);

-- Add new workstream statuses  
ALTER TABLE kyc.entity_workstreams DROP CONSTRAINT IF EXISTS chk_workstream_status;
ALTER TABLE kyc.entity_workstreams ADD CONSTRAINT chk_workstream_status CHECK (
  status IN ('PENDING', 'COLLECT', 'VERIFY', 'SCREEN', 'ASSESS', 'COMPLETE',
             'BLOCKED', 'ENHANCED_DD', 'REFERRED', 'PROHIBITED')
);

-- Add DRAFT to doc_requests
ALTER TABLE kyc.doc_requests DROP CONSTRAINT IF EXISTS chk_doc_status;
ALTER TABLE kyc.doc_requests ADD CONSTRAINT chk_doc_status CHECK (
  status IN ('DRAFT', 'REQUIRED', 'REQUESTED', 'RECEIVED', 'UNDER_REVIEW',
             'VERIFIED', 'REJECTED', 'WAIVED', 'EXPIRED')
);

-- Update transition validation function
CREATE OR REPLACE FUNCTION kyc.is_valid_case_transition(...) -- add new statuses
```

**verbs.yaml updates:**
- Add `REFER_TO_REGULATOR`, `DO_NOT_ONBOARD` to `kyc-case.update-status` and `kyc-case.close` valid_values
- Add `REFERRED`, `PROHIBITED` to `entity-workstream.update-status` valid_values
- Add `DRAFT` to `doc-request.create` valid_values, make it default

**Plugin handler:**
- Add `transition_validator` plugin for precondition checks before status updates

### Phase 2: CBU Evidence Lifecycle

**SQL Migration (`013_cbu_evidence.sql`):**
```sql
-- Add status column to CBUs
ALTER TABLE "ob-poc".cbus ADD COLUMN IF NOT EXISTS status VARCHAR(30) DEFAULT 'DISCOVERED';
ALTER TABLE "ob-poc".cbus ADD CONSTRAINT chk_cbu_status CHECK (
  status IN ('DISCOVERED', 'VALIDATION_PENDING', 'VALIDATED', 
             'UPDATE_PENDING_PROOF', 'VALIDATION_FAILED')
);

-- CBU evidence junction
CREATE TABLE "ob-poc".cbu_evidence (
  evidence_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  document_id UUID REFERENCES "ob-poc".document_catalog(doc_id),
  attestation_ref VARCHAR(255),
  evidence_type VARCHAR(50) NOT NULL,
  attached_at TIMESTAMPTZ DEFAULT now(),
  attached_by VARCHAR(255)
);

-- CBU change audit log
CREATE TABLE "ob-poc".cbu_change_log (
  log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
  change_type VARCHAR(50) NOT NULL,
  old_value JSONB,
  new_value JSONB,
  evidence_ids UUID[],
  changed_at TIMESTAMPTZ DEFAULT now(),
  changed_by VARCHAR(255),
  reason TEXT
);
```

**verbs.yaml additions:**
- `cbu.set-status` - guarded status transitions
- `cbu.attach-evidence` - link documents/attestations
- `cbu.log-change` - audit trail

### Phase 3: UBO Discovery & Proof

**SQL Migration (`014_ubo_proof.sql`):**
```sql
-- Update verification_status values
ALTER TABLE "ob-poc".ubo_registry DROP CONSTRAINT IF EXISTS chk_ubo_verification_status;
ALTER TABLE "ob-poc".ubo_registry ADD CONSTRAINT chk_ubo_verification_status CHECK (
  verification_status IN ('SUSPECTED', 'PENDING', 'PROVEN', 'VERIFIED', 
                          'FAILED', 'DISPUTED', 'REMOVED')
);

-- Add evidence linking
ALTER TABLE "ob-poc".ubo_registry ADD COLUMN IF NOT EXISTS evidence_doc_ids UUID[];
ALTER TABLE "ob-poc".ubo_registry ADD COLUMN IF NOT EXISTS proof_date TIMESTAMPTZ;
ALTER TABLE "ob-poc".ubo_registry ADD COLUMN IF NOT EXISTS replacement_ubo_id UUID;
```

**verbs.yaml additions:**
- `ubo.assert` - create SUSPECTED UBO with discovery source
- `ubo.prove` - transition to PROVEN with evidence requirements
- Update `ubo.close-ubo` to support replacement linkage

### Phase 4: Red-Flag Aggregation

**SQL Migration (`015_redflag_aggregation.sql`):**
```sql
-- Red-flag score configuration
CREATE TABLE "ob-poc".redflag_score_config (
  config_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  severity VARCHAR(20) NOT NULL,
  weight INTEGER NOT NULL,
  is_blocking BOOLEAN DEFAULT false,
  UNIQUE(severity)
);

INSERT INTO "ob-poc".redflag_score_config VALUES
  (gen_random_uuid(), 'SOFT', 1, false),
  (gen_random_uuid(), 'ESCALATE', 2, false),
  (gen_random_uuid(), 'HARD_STOP', 1000, true);

-- Case evaluation snapshots
CREATE TABLE "ob-poc".case_evaluation_snapshots (
  snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  case_id UUID NOT NULL REFERENCES kyc.cases(case_id),
  soft_score INTEGER NOT NULL,
  escalate_score INTEGER NOT NULL,
  has_hard_stop BOOLEAN NOT NULL,
  total_score INTEGER NOT NULL,
  recommended_action VARCHAR(50),
  evaluated_at TIMESTAMPTZ DEFAULT now(),
  evaluated_by VARCHAR(255)
);

-- Aggregation function
CREATE OR REPLACE FUNCTION "ob-poc".compute_case_redflag_score(p_case_id UUID) ...
```

**verbs.yaml additions:**
- `red-flag.aggregate` - compute and store case scores
- `kyc-case.apply-decision` - map evaluation to status transition

---

## Execution Order

1. **Phase 1** - State machine guards (foundation for all other phases)
2. **Phase 2** - CBU evidence lifecycle
3. **Phase 3** - UBO discovery & proof
4. **Phase 4** - Red-flag aggregation
5. **Phase 5** - Integration testing & DSL scenarios

## Open Decisions (Defaults)
- Soft score referral threshold: **4** (2x ESCALATE flags)
- Ownership % UBO trigger: **25%** (standard regulatory threshold)
- `REFER_TO_REGULATOR` is **non-terminal** (can return to REVIEW after regulator response)
- `ENHANCED_DD` workstreams trigger case-level **ASSESSMENT** status hold
