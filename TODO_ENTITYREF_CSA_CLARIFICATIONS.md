# Trading Profile: EntityRef and CSA Design Clarifications

## Issue 1: Unify EntityRef Shape Across Verbs and Document

### Current State

**In trading profile YAML document**, entities are referenced as:
```yaml
counterparty:
  type: LEI
  value: "W22LROWP2IHZNBB6K528"

manager:
  type: LEI
  value: "5493001KJTIIGC8Y1R12"

custodian:
  type: LEI
  value: "HPFHU0OQ28E4N0NFVK49"
```

**In DSL verb YAML**, the same fields expect UUIDs:
```yaml
# From isda.yaml
- name: counterparty
  type: uuid
  required: true
  maps_to: counterparty_entity_id

# From cbu-custody.yaml  
- name: counterparty
  type: uuid
  maps_to: counterparty_entity_id
```

**In Rust types** (`trading_profile/types.rs`):
```rust
pub struct EntityRef {
    pub ref_type: EntityRefType,  // LEI, BIC, NAME, UUID
    pub value: String,
}
```

### The Gap

During materialization, when we encounter:
```yaml
counterparty:
  type: LEI
  value: "W22LROWP2IHZNBB6K528"
```

We need to resolve this to a `counterparty_entity_id` UUID to insert into the database. But:

1. **No LEI→entity_id lookup exists** in materialization code
2. **No LEI column in entities table** to look up against
3. The verb expects a UUID but the document provides `{type, value}`

### Investigation Findings

**LEI is stored in `custody.entity_settlement_identity`:**
```sql
                         Table "custody.entity_settlement_identity"
        Column        |           Type           | Nullable |      Default
----------------------+--------------------------+----------+-------------------
 identity_id          | uuid                     | not null | gen_random_uuid()
 entity_id            | uuid                     | not null |  -- FK to entities
 primary_bic          | character varying(11)    | not null |
 lei                  | character varying(20)    |          |  -- LEI stored here
 alert_participant_id | character varying(50)    |          |
 ctm_participant_id   | character varying(50)    |          |
```

This means LEI → entity_id resolution is:
```sql
SELECT entity_id FROM custody.entity_settlement_identity WHERE lei = $1
```

### Questions Needing Clarification

1. **Where is LEI stored?**
   - ✅ **ANSWERED**: LEI is in `custody.entity_settlement_identity.lei`
   - Lookup: `SELECT entity_id FROM custody.entity_settlement_identity WHERE lei = ?`

2. **Resolution strategy - which approach?**
   
   **Option A: Resolve at import time**
   - When importing YAML, immediately look up `{type: LEI, value}` → UUID
   - Store resolved UUIDs in the document
   - Pro: Document is self-contained with resolved IDs
   - Con: Document becomes stale if entity data changes
   
   **Option B: Resolve at materialize time**
   - Keep `{type, value}` in document
   - During materialization, call resolver: `resolve_entity_ref(ref) → Option<Uuid>`
   - Pro: Always uses current entity data
   - Con: Materialization fails if entity doesn't exist
   
   **Option C: Store both**
   ```yaml
   counterparty:
     type: LEI
     value: "W22LROWP2IHZNBB6K528"
     resolved_id: "550e8400-..."  # Cached resolution
   ```
   - Pro: Human-readable + fast execution
   - Con: More complex, potential staleness

3. **What if entity doesn't exist?**
   - Should materialization fail?
   - Should it create a placeholder entity?
   - Should it skip that section with a warning?

4. **Should verbs accept EntityRef directly?**
   - Instead of `type: uuid`, use `type: entity_ref`
   - Verb handler resolves `{type, value}` → UUID internally
   - Would require changes to generic executor

5. **BIC vs LEI vs NAME - resolution priority?**
   - LEI is unique globally but may not be in our DB
   - BIC identifies institutions but isn't entity-specific
   - NAME requires fuzzy matching (EntityGateway)
   - What's the fallback chain?

### Existing Infrastructure to Leverage

- **EntityGateway gRPC** (`rust/crates/entity-gateway/`) - indexes entities for fuzzy search
- **`lookup:` in verb YAML** - already does table lookups for reference data
- **`GatewayResolver`** (`rust/src/dsl_v2/gateway_resolver.rs`) - resolves EntityRefs in CSG linter

### Proposed Investigation

Before planning, need to check:
1. `entity_settlement_identity` table - does it have LEI/BIC columns?
2. How does `isda.create` verb currently work - is it used anywhere?
3. What does GatewayResolver do with EntityRefs today?

---

## Issue 2: Consolidate CSA collateral_ssi to Reference Pattern

### Current State

**Two places define SSI data for the same purpose:**

**Place 1: Inline in CSA** (`isda_agreements[].csa.collateral_ssi`):
```yaml
isda_agreements:
  - counterparty: {...}
    csa:
      csa_type: VM
      collateral_ssi:
        name: GS_COLLATERAL_SSI
        custody_account: "COLL-GS-EUR-001"
        custody_bic: "IRVTDEFX"
        cash_account: "CASH-GS-EUR-001"
        cash_bic: "IRVTDEFX"
```

**Place 2: In standing_instructions** (`standing_instructions.OTC_COLLATERAL`):
```yaml
standing_instructions:
  OTC_COLLATERAL:
    - name: GS_OTC_SSI
      counterparty_lei: "W22LROWP2IHZNBB6K528"
      currency: USD
      custody_account: "OTC-GS-COLL-001"
      custody_bic: "IRVTUS3N"
      cash_account: "OTC-GS-CASH-001"
      cash_bic: "IRVTUS3N"
```

### The Problem

1. **Duplication** - Same SSI data in two places
2. **Inconsistent names** - `GS_COLLATERAL_SSI` vs `GS_OTC_SSI` (are these the same?)
3. **Validation gap** - No check that CSA's collateral_ssi exists in standing_instructions
4. **Materialization confusion** - Which one gets written to `cbu_ssi` table?

### Questions Needing Clarification

1. **Are these the same SSI or different?**
   - `GS_COLLATERAL_SSI` (in CSA) vs `GS_OTC_SSI` (in standing_instructions)
   - If same: consolidate. If different: clarify purpose of each.

2. **What's the canonical source?**
   
   **Option A: standing_instructions is the source**
   ```yaml
   csa:
     collateral_ssi_ref: "GS_COLLATERAL_SSI"  # Just a reference
   
   standing_instructions:
     OTC_COLLATERAL:
       - name: GS_COLLATERAL_SSI  # Full definition here
         custody_account: "..."
   ```
   - Pro: Single place to define SSIs, reusable
   - Pro: Validation can check ref exists
   - Con: Requires name uniqueness across categories
   
   **Option B: CSA owns collateral SSI inline**
   ```yaml
   csa:
     collateral_ssi:
       name: GS_COLLATERAL_SSI
       custody_account: "..."
   
   # No OTC_COLLATERAL section in standing_instructions
   ```
   - Pro: Self-contained CSA definition
   - Con: Can't reuse SSI across multiple CSAs
   - Con: Separate from other SSI management

3. **Database FK relationship?**
   - ✅ **CONFIRMED**: `custody.csa_agreements` has `collateral_ssi_id uuid REFERENCES custody.cbu_ssi(ssi_id)`
   - This confirms CSA should reference an existing SSI, not define inline
   - **Strongly supports Option A (reference pattern)**

4. **Materialization order dependency?**
   - If CSA references SSI by name, SSI must be materialized first
   - Current code: SSIs materialized before other sections (correct order)
   - But need to add CSA materialization that looks up SSI by name

### Proposed Design (Pending Confirmation)

```yaml
# BEFORE (current - duplicated)
csa:
  collateral_ssi:
    name: GS_COLLATERAL_SSI
    custody_account: "COLL-GS-EUR-001"
    ...

standing_instructions:
  OTC_COLLATERAL:
    - name: GS_OTC_SSI
      ...

# AFTER (reference pattern)
csa:
  collateral_ssi_ref: "GS_COLLATERAL_SSI"  # Just the name

standing_instructions:
  OTC_COLLATERAL:
    - name: GS_COLLATERAL_SSI  # Single source of truth
      counterparty_lei: "W22LROWP2IHZNBB6K528"
      custody_account: "COLL-GS-EUR-001"
      custody_bic: "IRVTDEFX"
      cash_account: "CASH-GS-EUR-001"
      cash_bic: "IRVTDEFX"
```

### Validation to Add

```rust
// In trading-profile.validate or trading-profile.import
fn validate_csa_ssi_refs(doc: &TradingProfileDocument) -> Vec<String> {
    let ssi_names: HashSet<_> = doc.standing_instructions
        .values()
        .flatten()
        .map(|s| &s.name)
        .collect();
    
    let mut errors = vec![];
    for isda in &doc.isda_agreements {
        if let Some(csa) = &isda.csa {
            if let Some(ref_name) = &csa.collateral_ssi_ref {
                if !ssi_names.contains(ref_name) {
                    errors.push(format!(
                        "CSA for {} references undefined SSI '{}'",
                        isda.counterparty.value, ref_name
                    ));
                }
            }
        }
    }
    errors
}
```

---

## Summary: What I Need to Proceed

| Issue | Key Question | Status |
|-------|--------------|--------|
| EntityRef | Where is LEI stored in DB? | ✅ `custody.entity_settlement_identity.lei` |
| EntityRef | Resolve at import or materialize time? | **Need input: Option A, B, or C** |
| EntityRef | What if entity not found? | **Need input: Fail / Skip / Create** |
| CSA SSI | Database FK confirms reference pattern? | ✅ Yes - `collateral_ssi_id` FK to `cbu_ssi` |
| CSA SSI | Are inline and standing_instructions SSIs the same? | **Need input: Yes / No / Depends** |
| CSA SSI | Confirm reference pattern is correct approach? | **Need input: Yes / Alternative** |

### Recommended Approach (Based on Investigation)

**EntityRef**: Option B (resolve at materialize time) seems best because:
- LEI lookup is simple: `SELECT entity_id FROM custody.entity_settlement_identity WHERE lei = $1`
- Keeps document human-readable with LEI values
- Always uses current entity data

**CSA SSI**: Option A (reference pattern) is clearly correct because:
- Database schema already has `collateral_ssi_id` FK to `cbu_ssi`
- SSIs should be defined once in `standing_instructions`
- CSA just references by name, materialization looks up `ssi_id`

Once you confirm these approaches, I can produce detailed implementation plans with test harnesses.
