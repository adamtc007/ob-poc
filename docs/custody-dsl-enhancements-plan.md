# Custody DSL Enhancement Plan

**Document**: `custody-dsl-enhancements-plan.md`  
**Created**: 2025-12-01  
**Status**: PENDING - Execute after agentic DSL generation is tested  
**Priority**: Medium  
**Prerequisite**: Successful end-to-end test of agentic DSL generation

---

## Context

The custody DSL (30 verbs across 7 domains) is functionally complete for agentic generation. This plan addresses gaps identified in review:

- Missing `read` verbs for single-record retrieval
- Lifecycle operations (update, deactivate, terminate)
- Convenience operations (clone, import)

**Do NOT implement until agentic generation is tested end-to-end.**

---

## Phase 1: Must Have (Before Production)

These are basic CRUD gaps that will be needed for any real usage.

### 1.1 Add `cbu-custody.read-ssi`

**Purpose**: Retrieve a single SSI by ID

**File**: `rust/config/verbs.yaml`

**Add under** `cbu-custody.verbs`:

```yaml
read-ssi:
  description: "Read a single SSI by ID"
  behavior: crud
  crud:
    operation: select_by_pk
    table: cbu_ssi
    schema: custody
    pk: ssi_id
  args:
    - name: ssi-id
      type: uuid
      required: true
      maps_to: ssi_id
  returns:
    type: record
```

**Effort**: Small (15 min)

---

### 1.2 Add `cbu-custody.read-booking-rule`

**Purpose**: Retrieve a single booking rule by ID

**Add under** `cbu-custody.verbs`:

```yaml
read-booking-rule:
  description: "Read a single booking rule by ID"
  behavior: crud
  crud:
    operation: select_by_pk
    table: ssi_booking_rules
    schema: custody
    pk: rule_id
  args:
    - name: rule-id
      type: uuid
      required: true
      maps_to: rule_id
  returns:
    type: record
```

**Effort**: Small (15 min)

---

### 1.3 Add `isda.read`

**Purpose**: Retrieve a single ISDA agreement by ID

**Add under** `isda.verbs`:

```yaml
read:
  description: "Read a single ISDA agreement by ID"
  behavior: crud
  crud:
    operation: select_by_pk
    table: isda_agreements
    schema: custody
    pk: isda_id
  args:
    - name: isda-id
      type: uuid
      required: true
      maps_to: isda_id
  returns:
    type: record
```

**Effort**: Small (15 min)

---

### 1.4 Add `isda.list-csa`

**Purpose**: List CSAs for an ISDA agreement

**Add under** `isda.verbs`:

```yaml
list-csa:
  description: "List CSA agreements for an ISDA"
  behavior: crud
  crud:
    operation: list_by_fk
    table: csa_agreements
    schema: custody
    fk_col: isda_id
  args:
    - name: isda-id
      type: uuid
      required: true
      maps_to: isda_id
  returns:
    type: record_set
```

**Effort**: Small (15 min)

---

## Phase 2: Should Have (Production Readiness)

These support real-world lifecycle operations.

### 2.1 Add `cbu-custody.update-ssi`

**Purpose**: Update SSI account details (account numbers, BICs)

**Add under** `cbu-custody.verbs`:

```yaml
update-ssi:
  description: "Update SSI account details"
  behavior: crud
  crud:
    operation: update
    table: cbu_ssi
    schema: custody
    key: ssi_id
  args:
    - name: ssi-id
      type: uuid
      required: true
      maps_to: ssi_id
    - name: name
      type: string
      required: false
      maps_to: ssi_name
    - name: safekeeping-account
      type: string
      required: false
      maps_to: safekeeping_account
    - name: safekeeping-bic
      type: string
      required: false
      maps_to: safekeeping_bic
    - name: cash-account
      type: string
      required: false
      maps_to: cash_account
    - name: cash-bic
      type: string
      required: false
      maps_to: cash_account_bic
    - name: cash-currency
      type: string
      required: false
      maps_to: cash_currency
    - name: pset-bic
      type: string
      required: false
      maps_to: pset_bic
  returns:
    type: affected
```

**Effort**: Small (30 min)

---

### 2.2 Add `cbu-custody.expire-ssi`

**Purpose**: Set expiry date on SSI (cleaner than suspend for planned end)

**Add under** `cbu-custody.verbs`:

```yaml
expire-ssi:
  description: "Set expiry date on an SSI"
  behavior: crud
  crud:
    operation: update
    table: cbu_ssi
    schema: custody
    key: ssi_id
  args:
    - name: ssi-id
      type: uuid
      required: true
      maps_to: ssi_id
    - name: expiry-date
      type: date
      required: true
      maps_to: expiry_date
  returns:
    type: affected
```

**Effort**: Small (15 min)

---

### 2.3 Add `cbu-custody.update-booking-rule`

**Purpose**: Update booking rule criteria (not just priority)

**Add under** `cbu-custody.verbs`:

```yaml
update-booking-rule:
  description: "Update booking rule criteria"
  behavior: crud
  crud:
    operation: update
    table: ssi_booking_rules
    schema: custody
    key: rule_id
  args:
    - name: rule-id
      type: uuid
      required: true
      maps_to: rule_id
    - name: name
      type: string
      required: false
      maps_to: rule_name
    - name: priority
      type: integer
      required: false
      maps_to: priority
    - name: ssi-id
      type: uuid
      required: false
      maps_to: ssi_id
    - name: instrument-class
      type: lookup
      required: false
      maps_to: instrument_class_id
      lookup:
        table: instrument_classes
        schema: custody
        code_column: code
        id_column: class_id
    - name: market
      type: lookup
      required: false
      maps_to: market_id
      lookup:
        table: markets
        schema: custody
        code_column: mic
        id_column: market_id
    - name: currency
      type: string
      required: false
      maps_to: currency
    - name: settlement-type
      type: string
      required: false
      maps_to: settlement_type
  returns:
    type: affected
```

**Effort**: Small (30 min)

---

### 2.4 Add `cbu-custody.reactivate-rule`

**Purpose**: Undo rule deactivation

**Add under** `cbu-custody.verbs`:

```yaml
reactivate-rule:
  description: "Reactivate a deactivated booking rule"
  behavior: crud
  crud:
    operation: update
    table: ssi_booking_rules
    schema: custody
    key: rule_id
    set_values:
      is_active: true
  args:
    - name: rule-id
      type: uuid
      required: true
      maps_to: rule_id
  returns:
    type: affected
```

**Effort**: Small (15 min)

---

### 2.5 Add `cbu-custody.deactivate-universe`

**Purpose**: Mark universe entry as inactive (client stops trading)

**Add under** `cbu-custody.verbs`:

```yaml
deactivate-universe:
  description: "Deactivate a universe entry (client stops trading this combination)"
  behavior: crud
  crud:
    operation: update
    table: cbu_instrument_universe
    schema: custody
    key: universe_id
    set_values:
      is_active: false
  args:
    - name: universe-id
      type: uuid
      required: true
      maps_to: universe_id
  returns:
    type: affected
```

**Note**: Requires adding `universe_id` capture to `add-universe`. Update:

```yaml
add-universe:
  # ... existing config ...
  returns:
    type: uuid
    name: universe_id
    capture: true  # Change from false to true
```

**Effort**: Small (30 min)

---

### 2.6 Add `isda.terminate`

**Purpose**: Terminate an ISDA agreement (end relationship)

**Add under** `isda.verbs`:

```yaml
terminate:
  description: "Terminate an ISDA agreement"
  behavior: crud
  crud:
    operation: update
    table: isda_agreements
    schema: custody
    key: isda_id
  args:
    - name: isda-id
      type: uuid
      required: true
      maps_to: isda_id
    - name: termination-date
      type: date
      required: true
      maps_to: termination_date
    - name: termination-reason
      type: string
      required: false
      maps_to: termination_reason
  returns:
    type: affected
```

**Database**: Add columns to `custody.isda_agreements`:

```sql
ALTER TABLE custody.isda_agreements 
ADD COLUMN termination_date DATE,
ADD COLUMN termination_reason TEXT;
```

**Effort**: Medium (45 min - includes schema change)

---

## Phase 3: Nice to Have (Future)

These are convenience/efficiency features for mature usage.

### 3.1 Add `cbu-custody.clone-booking-rules`

**Purpose**: Copy all booking rules from one CBU to another (onboard similar clients)

**This requires a plugin** - cannot be done with simple CRUD.

**Add to** `plugins` section:

```yaml
cbu-custody.clone-booking-rules:
  description: "Clone booking rules from source CBU to target CBU"
  handler: clone_booking_rules
  args:
    - name: source-cbu-id
      type: uuid
      required: true
    - name: target-cbu-id
      type: uuid
      required: true
    - name: ssi-mapping
      type: json
      required: true
      description: "Map source SSI IDs to target SSI IDs: {\"source-uuid\": \"target-uuid\"}"
  returns:
    type: record
    description: "Count of rules cloned"
```

**Plugin Implementation** (`rust/src/dsl_v2/custom_ops/custody_ops.rs`):

```rust
pub async fn clone_booking_rules(
    pool: &PgPool,
    args: &HashMap<String, Value>,
) -> Result<Value> {
    let source_cbu = args.get("source-cbu-id").and_then(|v| v.as_uuid())?;
    let target_cbu = args.get("target-cbu-id").and_then(|v| v.as_uuid())?;
    let ssi_mapping: HashMap<Uuid, Uuid> = args.get("ssi-mapping")
        .and_then(|v| serde_json::from_value(v.clone()).ok())?;
    
    // Fetch source rules
    let source_rules = sqlx::query_as!(
        BookingRule,
        "SELECT * FROM custody.ssi_booking_rules WHERE cbu_id = $1 AND is_active = true",
        source_cbu
    ).fetch_all(pool).await?;
    
    let mut cloned = 0;
    for rule in source_rules {
        // Map SSI ID
        let target_ssi = ssi_mapping.get(&rule.ssi_id)
            .ok_or_else(|| anyhow!("No mapping for SSI {}", rule.ssi_id))?;
        
        // Insert cloned rule
        sqlx::query!(
            r#"INSERT INTO custody.ssi_booking_rules 
               (cbu_id, ssi_id, rule_name, priority, instrument_class_id, 
                security_type_id, market_id, currency, settlement_type,
                counterparty_entity_id, effective_date)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, CURRENT_DATE)"#,
            target_cbu,
            target_ssi,
            format!("{} (cloned)", rule.rule_name),
            rule.priority,
            rule.instrument_class_id,
            rule.security_type_id,
            rule.market_id,
            rule.currency,
            rule.settlement_type,
            rule.counterparty_entity_id,
        ).execute(pool).await?;
        
        cloned += 1;
    }
    
    Ok(json!({ "cloned": cloned }))
}
```

**Effort**: Large (2-3 hours)

---

### 3.2 Add `cbu-custody.clone-ssi-set`

**Purpose**: Clone all SSIs from one CBU to another

**Similar plugin pattern to clone-booking-rules**

**Effort**: Large (2-3 hours)

---

### 3.3 Add `isda.amend`

**Purpose**: Record ISDA amendments (legal events with dates)

**Database**: Add table `custody.isda_amendments`:

```sql
CREATE TABLE custody.isda_amendments (
    amendment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    isda_id UUID NOT NULL REFERENCES custody.isda_agreements(isda_id),
    amendment_date DATE NOT NULL,
    amendment_type VARCHAR(50),  -- 'SCHEDULE', 'CSA', 'OTHER'
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);
```

**Add to** `isda.verbs`:

```yaml
amend:
  description: "Record an ISDA amendment"
  behavior: crud
  crud:
    operation: insert
    table: isda_amendments
    schema: custody
    returning: amendment_id
  args:
    - name: isda-id
      type: uuid
      required: true
      maps_to: isda_id
    - name: amendment-date
      type: date
      required: true
      maps_to: amendment_date
    - name: amendment-type
      type: string
      required: false
      maps_to: amendment_type
      valid_values: [SCHEDULE, CSA, OTHER]
    - name: description
      type: string
      required: false
      maps_to: description
  returns:
    type: uuid
    name: amendment_id
    capture: true
```

**Effort**: Medium (1 hour - includes schema)

---

### 3.4 Add `cbu-custody.import-from-alert`

**Purpose**: Import SSI/booking rule data from DTCC ALERT format

**This is a complex plugin** requiring:
- ALERT message parsing
- SSI creation from ALERT fields
- Booking rule derivation

**Defer to Phase 2 of product roadmap**

**Effort**: Very Large (1-2 days)

---

## Summary

### Phase 1: Must Have (Total: ~1 hour)

| Verb | Domain | Effort |
|------|--------|--------|
| read-ssi | cbu-custody | 15 min |
| read-booking-rule | cbu-custody | 15 min |
| read | isda | 15 min |
| list-csa | isda | 15 min |

### Phase 2: Should Have (Total: ~3 hours)

| Verb | Domain | Effort |
|------|--------|--------|
| update-ssi | cbu-custody | 30 min |
| expire-ssi | cbu-custody | 15 min |
| update-booking-rule | cbu-custody | 30 min |
| reactivate-rule | cbu-custody | 15 min |
| deactivate-universe | cbu-custody | 30 min |
| terminate | isda | 45 min |

### Phase 3: Nice to Have (Total: ~6-8 hours)

| Verb | Domain | Effort |
|------|--------|--------|
| clone-booking-rules | cbu-custody | 2-3 hours |
| clone-ssi-set | cbu-custody | 2-3 hours |
| amend | isda | 1 hour |
| import-from-alert | cbu-custody | 1-2 days |

---

## Execution Notes

1. **Test agentic generation first** - Don't add verbs until current 30 are proven
2. **Phase 1 is YAML-only** - No Rust code changes, just verbs.yaml
3. **Phase 2 has one schema change** - isda.terminate needs columns
4. **Phase 3 needs plugins** - clone operations require Rust handlers
5. **Update CLAUDE.md** after adding verbs
6. **Update examples** in agentic prompts if adding verbs used in generation

---

## Verification Before Implementation

Before adding any verbs, confirm:

1. [ ] Agentic DSL generation passes end-to-end test
2. [ ] CLI `dsl_cli custody -i "..."` works correctly
3. [ ] Generated DSL validates without errors
4. [ ] Executed DSL creates correct database records
5. [ ] Feedback loop (retry on validation failure) works

---

*End of Enhancement Plan*
