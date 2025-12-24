## Phase 1.6: Multi-Regulator Support

### The Problem

Entities can have multiple regulatory registrations:

| Scenario | Example |
|----------|---------|
| **Dual regulation** | UK bank: FCA (conduct) + PRA (prudential) |
| **Multi-jurisdiction** | Global AM: SEC + FCA + MAS + SFC |
| **EU passporting** | Lux fund: CSSF (home) + passported to 27 states |
| **Activity-based** | US firm: SEC (securities) + CFTC (derivatives) + FINRA (broker) |
| **State + Federal** | US: Federal regulator + state regulators |

### Revised Schema

**Migration:** `V0XX__entity_regulatory_registrations.sql`

```sql
-- Replace single-regulator design with multi-regulator
-- Drop old table if exists (or migrate data)
-- DROP TABLE IF EXISTS entity_regulatory_profiles;

CREATE TABLE ob_ref.registration_types (
    registration_type VARCHAR(50) PRIMARY KEY,
    description VARCHAR(255),
    is_primary BOOLEAN DEFAULT FALSE,
    allows_reliance BOOLEAN DEFAULT TRUE
);

INSERT INTO ob_ref.registration_types VALUES
('PRIMARY', 'Primary/home state regulator', TRUE, TRUE),
('DUAL_CONDUCT', 'Dual regulation - conduct authority', FALSE, TRUE),
('DUAL_PRUDENTIAL', 'Dual regulation - prudential authority', FALSE, TRUE),
('PASSPORTED', 'EU/EEA passported registration', FALSE, TRUE),
('BRANCH', 'Branch registration in jurisdiction', FALSE, TRUE),
('SUBSIDIARY', 'Separate subsidiary registration', FALSE, TRUE),
('ADDITIONAL', 'Additional registration (same jurisdiction)', FALSE, TRUE),
('STATE', 'State/provincial registration', FALSE, FALSE),
('SRO', 'Self-regulatory organization', FALSE, TRUE);

CREATE TABLE ob_kyc.entity_regulatory_registrations (
    registration_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(entity_id),
    regulator_code VARCHAR(50) NOT NULL REFERENCES ob_ref.regulators(regulator_code),
    
    -- Registration details
    registration_number VARCHAR(100),
    registration_type VARCHAR(50) REFERENCES ob_ref.registration_types(registration_type),
    activity_scope VARCHAR(500),          -- What activities this covers
    
    -- For passporting
    home_regulator_code VARCHAR(50),      -- If passported, who's the home regulator
    passport_reference VARCHAR(100),      -- Passport notification reference
    
    -- Verification
    registration_verified BOOLEAN DEFAULT FALSE,
    verification_date DATE,
    verification_method VARCHAR(50),
    verification_reference VARCHAR(500),
    verification_expires DATE,            -- When re-verification needed
    
    -- Status
    status VARCHAR(50) DEFAULT 'ACTIVE',  -- ACTIVE, SUSPENDED, WITHDRAWN, EXPIRED
    effective_date DATE,
    expiry_date DATE,
    
    -- Audit
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    created_by UUID,
    
    -- Constraints
    CONSTRAINT uq_entity_regulator UNIQUE (entity_id, regulator_code)
);

CREATE INDEX idx_reg_entity ON ob_kyc.entity_regulatory_registrations(entity_id);
CREATE INDEX idx_reg_regulator ON ob_kyc.entity_regulatory_registrations(regulator_code);
CREATE INDEX idx_reg_status ON ob_kyc.entity_regulatory_registrations(status);
CREATE INDEX idx_reg_type ON ob_kyc.entity_regulatory_registrations(registration_type);

-- View: Entity with all registrations summarized
CREATE VIEW ob_kyc.v_entity_regulatory_summary AS
SELECT 
    e.entity_id,
    e.entity_name,
    COUNT(r.registration_id) as registration_count,
    COUNT(r.registration_id) FILTER (WHERE r.registration_verified) as verified_count,
    BOOL_OR(r.registration_verified AND rt.tier_code = 'EQUIVALENT') as has_equivalent_regulator,
    ARRAY_AGG(DISTINCT r.regulator_code) FILTER (WHERE r.status = 'ACTIVE') as active_regulators,
    MAX(r.verification_date) as last_verified,
    MIN(r.verification_expires) FILTER (WHERE r.verification_expires > CURRENT_DATE) as next_expiry
FROM entities e
LEFT JOIN ob_kyc.entity_regulatory_registrations r ON e.entity_id = r.entity_id
LEFT JOIN ob_ref.regulators reg ON r.regulator_code = reg.regulator_code
LEFT JOIN ob_ref.regulatory_tiers rt ON reg.regulatory_tier = rt.tier_code
GROUP BY e.entity_id, e.entity_name;
```

---

### Updated DSL Verbs

**File:** `config/verbs/regulatory.yaml` (revised)

```yaml
domain: regulatory

# ═══════════════════════════════════════════════════════════════════════
# Registration management (multi-regulator)
# ═══════════════════════════════════════════════════════════════════════

registration:
  add:
    description: "Add regulatory registration for an entity"
    behavior: crud
    crud:
      operation: insert
      table: entity_regulatory_registrations
      schema: ob_kyc
    args:
      - name: entity-id
        type: uuid
        required: true
        lookup:
          entity_type: entity
      - name: regulator
        type: string
        required: true
        column: regulator_code
        lookup:
          table: regulators
          schema: ob_ref
          search_key: regulator_code
      - name: registration-number
        type: string
        required: false
      - name: registration-type
        type: string
        required: true
        column: registration_type
        enum: [PRIMARY, DUAL_CONDUCT, DUAL_PRUDENTIAL, PASSPORTED, BRANCH, SUBSIDIARY, ADDITIONAL, STATE, SRO]
      - name: activity-scope
        type: string
        required: false
        description: "What activities this registration covers"
      - name: home-regulator
        type: string
        required: false
        column: home_regulator_code
        description: "For passported registrations, the home state regulator"
      - name: effective-date
        type: date
        required: false

  list:
    description: "List all regulatory registrations for an entity"
    behavior: crud
    crud:
      operation: select
      table: entity_regulatory_registrations
      schema: ob_kyc
      multiple: true
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: status
        type: string
        required: false
        default: ACTIVE

  verify:
    description: "Mark registration as verified"
    behavior: plugin
    plugin:
      handler: RegistrationVerifyOp
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: regulator
        type: string
        required: true
        column: regulator_code
      - name: verification-method
        type: string
        required: true
        enum: [MANUAL, REGISTRY_API, DOCUMENT]
      - name: reference
        type: string
        required: false
      - name: expires
        type: date
        required: false
        column: verification_expires
        description: "When re-verification is needed"

  remove:
    description: "Remove/withdraw regulatory registration"
    behavior: crud
    crud:
      operation: update
      table: entity_regulatory_registrations
      schema: ob_kyc
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: regulator
        type: string
        required: true
        column: regulator_code
      - name: reason
        type: string
        required: false
    set:
      status: WITHDRAWN
      expiry_date: CURRENT_DATE

# ═══════════════════════════════════════════════════════════════════════
# Entity-level regulatory status (computed from registrations)
# ═══════════════════════════════════════════════════════════════════════

status:
  check:
    description: "Check entity's overall regulatory status"
    behavior: plugin
    plugin:
      handler: RegulatoryStatusCheckOp
    args:
      - name: entity-id
        type: uuid
        required: true
    returns:
      type: object
      description: |
        {
          "is_regulated": true,
          "registration_count": 3,
          "verified_count": 2,
          "has_equivalent_regulator": true,
          "allows_simplified_dd": true,
          "registrations": [
            {
              "regulator": "FCA",
              "type": "PRIMARY",
              "verified": true,
              "tier": "EQUIVALENT"
            },
            {
              "regulator": "PRA",
              "type": "DUAL_PRUDENTIAL",
              "verified": true,
              "tier": "EQUIVALENT"
            }
          ],
          "next_verification_due": "2025-06-15"
        }

  check-for-activity:
    description: "Check if entity is regulated for specific activity"
    behavior: plugin
    plugin:
      handler: RegulatoryActivityCheckOp
    args:
      - name: entity-id
        type: uuid
        required: true
      - name: activity
        type: string
        required: true
        description: "Activity to check (e.g., ASSET_MANAGEMENT, CUSTODY, DEALING)"
      - name: jurisdiction
        type: string
        required: false
        description: "Specific jurisdiction to check"
    returns:
      type: object
      description: |
        {
          "is_authorized": true,
          "regulator": "FCA",
          "registration_type": "PRIMARY",
          "activity_scope": "Managing investments, arranging deals"
        }
```

---

### DSL Usage Examples

**UK Bank (dual regulated):**
```lisp
; Add primary FCA registration
(regulatory.registration.add 
  entity-id:@barclays 
  regulator:FCA 
  registration-number:"122702"
  registration-type:PRIMARY
  activity-scope:"Accepting deposits, consumer credit, insurance mediation")

; Add PRA dual regulation
(regulatory.registration.add 
  entity-id:@barclays 
  regulator:PRA 
  registration-number:"122702"
  registration-type:DUAL_PRUDENTIAL
  activity-scope:"Prudential supervision")

; Verify both
(regulatory.registration.verify entity-id:@barclays regulator:FCA verification-method:REGISTRY_API)
(regulatory.registration.verify entity-id:@barclays regulator:PRA verification-method:REGISTRY_API)
```

**EU Fund Manager (passported):**
```lisp
; Primary registration in Luxembourg
(regulatory.registration.add
  entity-id:@allianz-gi-lux
  regulator:CSSF
  registration-number:"S00001234"
  registration-type:PRIMARY
  activity-scope:"UCITS management, AIF management")

; Passported to Germany
(regulatory.registration.add
  entity-id:@allianz-gi-lux
  regulator:BaFin
  registration-type:PASSPORTED
  home-regulator:CSSF
  activity-scope:"Marketing UCITS to German investors")

; Passported to France
(regulatory.registration.add
  entity-id:@allianz-gi-lux
  regulator:AMF
  registration-type:PASSPORTED
  home-regulator:CSSF)
```

**US Broker-Dealer:**
```lisp
; SEC registration
(regulatory.registration.add
  entity-id:@goldman-securities
  regulator:SEC
  registration-number:"8-12345"
  registration-type:PRIMARY
  activity-scope:"Broker-dealer")

; FINRA membership
(regulatory.registration.add
  entity-id:@goldman-securities
  regulator:FINRA
  registration-number:"123"
  registration-type:SRO
  activity-scope:"Broker-dealer")

; State registration (NY)
(regulatory.registration.add
  entity-id:@goldman-securities
  regulator:NYDFS
  registration-type:STATE)
```

**Check overall status:**
```lisp
(regulatory.status.check entity-id:@barclays)

; Returns:
; {
;   "is_regulated": true,
;   "registration_count": 2,
;   "has_equivalent_regulator": true,
;   "allows_simplified_dd": true,
;   "registrations": [
;     {"regulator": "FCA", "type": "PRIMARY", "verified": true},
;     {"regulator": "PRA", "type": "DUAL_PRUDENTIAL", "verified": true}
;   ]
; }
```

**Check for specific activity:**
```lisp
(regulatory.status.check-for-activity 
  entity-id:@allianz-gi-lux 
  activity:UCITS_MANAGEMENT 
  jurisdiction:DE)

; Returns:
; {
;   "is_authorized": true,
;   "regulator": "BaFin",
;   "registration_type": "PASSPORTED",
;   "home_regulator": "CSSF"
; }
```

---

### Impact on KYC Scoping

The `determine_obligation` function now checks:

```rust
fn determine_obligation(role: &RoleType, entity_id: Uuid, pool: &PgPool) -> KycObligation {
    if role.check_regulatory_status {
        // Check if entity has ANY verified EQUIVALENT registration
        let status = regulatory_status_check(entity_id, pool).await?;
        
        if status.has_equivalent_regulator && status.verified_count > 0 {
            return role.if_regulated_obligation.unwrap_or(KycObligation::Simplified);
        }
    }
    
    // Fall through to full KYC
    if role.triggers_full_kyc {
        return KycObligation::FullKyc;
    }
    // ... etc
}
```

The key question for simplified DD: **Does the entity have at least one verified registration with an EQUIVALENT tier regulator?**

Not: "Which single regulator?" but "Any qualifying regulator?"

---

### Additional Tasks for Phase 1

- [ ] Update migration to use multi-regulator schema
- [ ] Create `registration_types` reference table
- [ ] Update `regulatory.yaml` verbs for multi-regulator
- [ ] Implement `RegulatoryStatusCheckOp` plugin
- [ ] Implement `RegulatoryActivityCheckOp` plugin
- [ ] Update KYC scope logic to use new status check
- [ ] Add state/provincial regulators to seed data (optional)
- [ ] Add self-regulatory orgs to seed data (FINRA, etc.)
