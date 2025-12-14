# Trading Profile: EntityRef and CSA Design Decisions

## Issue 1: EntityRef Resolution

### Answer: Where is LEI stored?

**IMPORTANT**: LEI exists in **THREE tables** (Claude Code only found one):

| Table | LEI Column | Use Case |
|-------|------------|----------|
| `custody.entity_settlement_identity` | `lei varchar(20)` | Settlement identities - banks, brokers |
| `ob-poc.entity_funds` | `lei varchar(20)` | Fund entities with LEI |
| `ob-poc.entity_manco` | `lei varchar(20)` | Management companies |

**Note**: `entity_limited_companies` does NOT have LEI - only `registration_number`.

**Resolution must check all three tables** - ISDA counterparties could be:
- Banks/brokers → in `entity_settlement_identity`
- Fund counterparties → in `entity_funds`
- Asset managers → in `entity_manco`

### Answer: Resolution Strategy → Option B (Resolve at Materialize Time)

**Decision:** Keep `{type: LEI, value: "..."}` in the document, resolve during materialization.

**Rationale:**
- Document stays human-readable (people know LEIs, not UUIDs)
- Always uses current entity data
- Matches how booking rules and ISDA references work in production

### Answer: What if Entity Not Found? → FAIL

**Decision:** Materialization fails with clear error message.

**Rationale:**
- Entities must exist before a trading profile can reference them
- Silent skipping would create broken configurations
- Creating placeholders is dangerous (incomplete entity data)

### Implementation: Entity Resolver

Create `rust/src/trading_profile/resolve.rs`:

```rust
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EntityRef {
    pub ref_type: EntityRefType,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityRefType {
    LEI,
    BIC,
    UUID,
    NAME,
}

/// Resolve EntityRef to entity_id UUID
/// Checks multiple tables in priority order
pub async fn resolve_entity_ref(
    pool: &PgPool,
    entity_ref: &EntityRef,
) -> Result<Uuid, ResolveError> {
    match entity_ref.ref_type {
        EntityRefType::UUID => {
            // Direct UUID - just parse and verify exists
            let uuid = Uuid::parse_str(&entity_ref.value)
                .map_err(|_| ResolveError::InvalidUuid(entity_ref.value.clone()))?;
            verify_entity_exists(pool, uuid).await?;
            Ok(uuid)
        }
        EntityRefType::LEI => resolve_by_lei(pool, &entity_ref.value).await,
        EntityRefType::BIC => resolve_by_bic(pool, &entity_ref.value).await,
        EntityRefType::NAME => resolve_by_name(pool, &entity_ref.value).await,
    }
}

async fn resolve_by_lei(pool: &PgPool, lei: &str) -> Result<Uuid, ResolveError> {
    // Check entity_funds first (most common for counterparties)
    let result: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT entity_id FROM "ob-poc".entity_funds WHERE lei = $1
        UNION
        SELECT entity_id FROM "ob-poc".entity_manco WHERE lei = $1
        UNION
        SELECT entity_id FROM custody.entity_settlement_identity WHERE lei = $1
        LIMIT 1
        "#
    )
    .bind(lei)
    .fetch_optional(pool)
    .await?;

    result.ok_or_else(|| ResolveError::NotFound {
        ref_type: "LEI".to_string(),
        value: lei.to_string(),
        hint: "Ensure entity exists in entity_funds, entity_manco, or entity_settlement_identity".to_string(),
    })
}

async fn resolve_by_bic(pool: &PgPool, bic: &str) -> Result<Uuid, ResolveError> {
    let result: Option<Uuid> = sqlx::query_scalar(
        "SELECT entity_id FROM custody.entity_settlement_identity WHERE primary_bic = $1 LIMIT 1"
    )
    .bind(bic)
    .fetch_optional(pool)
    .await?;

    result.ok_or_else(|| ResolveError::NotFound {
        ref_type: "BIC".to_string(),
        value: bic.to_string(),
        hint: "Ensure entity has settlement identity with this BIC".to_string(),
    })
}

async fn resolve_by_name(pool: &PgPool, name: &str) -> Result<Uuid, ResolveError> {
    // Use existing entity search (fuzzy match on search_name)
    let result: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT entity_id FROM "ob-poc".entities WHERE search_name ILIKE $1 LIMIT 1"#
    )
    .bind(format!("%{}%", name))
    .fetch_optional(pool)
    .await?;

    result.ok_or_else(|| ResolveError::NotFound {
        ref_type: "NAME".to_string(),
        value: name.to_string(),
        hint: "Entity not found by name search".to_string(),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),
    
    #[error("Entity not found: {ref_type}={value}. {hint}")]
    NotFound {
        ref_type: String,
        value: String,
        hint: String,
    },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

### Usage in Materialization

```rust
// In materialize.rs
async fn sync_isda_agreements(
    tx: &mut Transaction<'_, Postgres>,
    pool: &PgPool,  // For entity resolution
    cbu_id: Uuid,
    isda_configs: &[ISDAConfig],
) -> Result<usize, MaterializeError> {
    for isda in isda_configs {
        // Resolve counterparty LEI → entity_id
        let counterparty_id = resolve_entity_ref(pool, &isda.counterparty)
            .await
            .map_err(|e| MaterializeError::EntityResolution {
                context: format!("ISDA counterparty for agreement dated {}", isda.agreement_date),
                source: e,
            })?;
        
        // Now insert with resolved UUID
        sqlx::query(...)
            .bind(counterparty_id)
            ...
    }
}
```

---

## Issue 2: CSA SSI → Use Reference Pattern

### Answer: Consolidate to Reference Pattern (Option A)

**Decision:** `standing_instructions` is the single source of truth. CSA references by name.

**Rationale:**
- Matches DB design: `csa_agreements.collateral_ssi_id` is FK to `cbu_ssi`
- Single place to maintain SSI details
- Enables reuse (multiple CSAs can reference same SSI)
- Validation can check ref exists before materialization

### Document Structure Change

**BEFORE (current - duplicated):**
```yaml
isda_agreements:
  - counterparty: {type: LEI, value: "W22LROWP2IHZNBB6K528"}
    csa:
      collateral_ssi:           # INLINE - BAD
        name: GS_COLLATERAL_SSI
        custody_account: "COLL-GS-EUR-001"
        custody_bic: "IRVTDEFX"
        ...

standing_instructions:
  OTC_COLLATERAL:
    - name: GS_OTC_SSI          # DIFFERENT NAME - CONFUSING
      ...
```

**AFTER (reference pattern):**
```yaml
isda_agreements:
  - counterparty: {type: LEI, value: "W22LROWP2IHZNBB6K528"}
    csa:
      collateral_ssi_ref: "GS_COLLATERAL_SSI"  # REFERENCE ONLY

standing_instructions:
  OTC_COLLATERAL:
    - name: GS_COLLATERAL_SSI   # SINGLE SOURCE OF TRUTH
      counterparty_lei: "W22LROWP2IHZNBB6K528"
      currency: USD
      custody_account: "COLL-GS-EUR-001"
      custody_bic: "IRVTDEFX"
      cash_account: "CASH-GS-EUR-001"
      cash_bic: "IRVTDEFX"
```

### Type Changes

```rust
// BEFORE
pub struct CSAConfig {
    pub collateral_ssi: SSI,  // Full inline SSI
    ...
}

// AFTER
pub struct CSAConfig {
    pub collateral_ssi_ref: String,  // Just the name
    ...
}
```

### Validation

Add to `trading_profile/validate.rs`:

```rust
pub fn validate_csa_ssi_refs(doc: &TradingProfileDoc) -> Vec<ValidationError> {
    // Collect all SSI names from standing_instructions
    let mut ssi_names: HashSet<&str> = HashSet::new();
    
    if let Some(ref si) = doc.standing_instructions {
        for (category, ssis) in si.iter() {
            for ssi in ssis {
                ssi_names.insert(&ssi.name);
            }
        }
    }
    
    // Check all CSA collateral_ssi_ref values exist
    let mut errors = vec![];
    
    if let Some(ref isda_agreements) = doc.isda_agreements {
        for isda in isda_agreements {
            if let Some(ref csa) = isda.csa {
                if !ssi_names.contains(csa.collateral_ssi_ref.as_str()) {
                    errors.push(ValidationError::MissingReference {
                        field: "csa.collateral_ssi_ref".to_string(),
                        value: csa.collateral_ssi_ref.clone(),
                        expected_in: "standing_instructions".to_string(),
                        context: format!(
                            "ISDA with counterparty {}",
                            isda.counterparty.value
                        ),
                    });
                }
            }
        }
    }
    
    errors
}
```

### Materialization Order

```rust
pub async fn materialize(pool: &PgPool, cbu_id: Uuid, doc: &TradingProfileDoc) -> Result<...> {
    let mut tx = pool.begin().await?;
    
    // 1. SSIs FIRST (so we can look up by name later)
    let ssi_map = sync_standing_instructions(&mut tx, cbu_id, &doc.standing_instructions).await?;
    // Returns HashMap<String, Uuid> mapping name → ssi_id
    
    // 2. ISDA/CSA (references SSIs by name)
    sync_isda_agreements(&mut tx, pool, cbu_id, &doc.isda_agreements, &ssi_map).await?;
    
    // ... rest of materialization
    
    tx.commit().await?;
}

async fn sync_isda_agreements(
    tx: &mut Transaction<'_, Postgres>,
    pool: &PgPool,
    cbu_id: Uuid,
    isda_configs: &[ISDAConfig],
    ssi_map: &HashMap<String, Uuid>,  // SSI name → ssi_id
) -> Result<usize> {
    for isda in isda_configs {
        let counterparty_id = resolve_entity_ref(pool, &isda.counterparty).await?;
        
        let isda_id = sqlx::query_scalar(...)
            .bind(counterparty_id)
            .fetch_one(&mut **tx)
            .await?;
        
        if let Some(ref csa) = isda.csa {
            // Look up SSI by name
            let ssi_id = ssi_map.get(&csa.collateral_ssi_ref)
                .ok_or_else(|| MaterializeError::MissingSSI(csa.collateral_ssi_ref.clone()))?;
            
            sqlx::query(r#"
                INSERT INTO custody.csa_agreements 
                (isda_id, csa_type, threshold_amount, ..., collateral_ssi_id, ...)
                VALUES ($1, $2, $3, ..., $4, ...)
            "#)
            .bind(isda_id)
            .bind(&csa.csa_type)
            .bind(&csa.threshold_amount)
            .bind(ssi_id)  // Resolved from ssi_map
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(isda_configs.len())
}
```

---

## Updated Seed File Section

Fix the `allianzgi_complete.yaml` to use reference pattern:

```yaml
# ==============================================================================
# ISDA AGREEMENTS - CSA now uses collateral_ssi_ref (not inline)
# ==============================================================================
isda_agreements:
  - counterparty:
      type: LEI
      value: "W22LROWP2IHZNBB6K528"
    agreement_date: "2020-03-15"
    governing_law: ENGLISH
    effective_date: "2020-04-01"
    
    product_coverage:
      - asset_class: RATES
        base_products: [SWAP, SWAPTION, CAP_FLOOR, FRA]
      - asset_class: FX
        base_products: [FORWARD, OPTION, SWAP, NDF]
    
    csa:
      csa_type: VM
      threshold_amount: 0
      threshold_currency: USD
      minimum_transfer_amount: 500000
      rounding_amount: 10000
      
      eligible_collateral:
        - type: CASH
          currencies: [USD, EUR, GBP]
          haircut_pct: 0
        - type: GOVT_BOND
          issuers: [US, DE, GB, FR]
          min_rating: AA-
          haircut_pct: 2.0
      
      # CHANGED: Reference instead of inline
      collateral_ssi_ref: "GS_COLLATERAL_SSI"
      
      valuation_time: "16:00"
      valuation_timezone: America/New_York
      notification_time: "18:00"
      settlement_days: 1
      dispute_resolution: CALCULATION_AGENT

  - counterparty:
      type: LEI
      value: "8IE5DZWZ7BX5LA5DJB03"
    agreement_date: "2019-06-01"
    governing_law: NEW_YORK
    effective_date: "2019-07-01"
    
    product_coverage:
      - asset_class: CREDIT
        base_products: [CDS, CDX, TRANCHE, LCDS]
    
    csa:
      csa_type: VM_IM
      threshold_amount: 0
      threshold_currency: USD
      minimum_transfer_amount: 1000000
      rounding_amount: 50000
      
      eligible_collateral:
        - type: CASH
          currencies: [USD]
          haircut_pct: 0
        - type: GOVT_BOND
          issuers: [US]
          min_rating: AA
          haircut_pct: 2.0
      
      initial_margin:
        calculation_method: ISDA_SIMM
        posting_frequency: DAILY
        segregation_required: true
        custodian:
          type: LEI
          value: "HPFHU0OQ28E4N0NFVK49"
      
      # CHANGED: Reference instead of inline
      collateral_ssi_ref: "JPM_COLLATERAL_SSI"
      
      valuation_time: "15:00"
      valuation_timezone: America/New_York
      settlement_days: 1

  - counterparty:
      type: LEI
      value: "213800LBQA1Y9L22JB70"
    agreement_date: "2021-01-15"
    governing_law: ENGLISH
    effective_date: "2021-02-01"
    
    product_coverage:
      - asset_class: RATES
        base_products: [SWAP, SWAPTION]
      - asset_class: FX
        base_products: [FORWARD, OPTION]
    
    csa:
      csa_type: VM
      threshold_amount: 0
      threshold_currency: GBP
      minimum_transfer_amount: 250000
      
      eligible_collateral:
        - type: CASH
          currencies: [GBP, EUR, USD]
          haircut_pct: 0
      
      # CHANGED: Reference instead of inline
      collateral_ssi_ref: "BARC_COLLATERAL_SSI"
      
      valuation_time: "16:00"
      valuation_timezone: Europe/London
      settlement_days: 1

# ==============================================================================
# STANDING INSTRUCTIONS - OTC_COLLATERAL is now the single source
# ==============================================================================
standing_instructions:
  # ... CUSTODY section unchanged ...

  OTC_COLLATERAL:
    # Goldman Sachs collateral SSI (referenced by CSA above)
    - name: GS_COLLATERAL_SSI
      counterparty:
        type: LEI
        value: "W22LROWP2IHZNBB6K528"
      currency: USD
      custody_account: "COLL-GS-EUR-001"
      custody_bic: "IRVTDEFX"
      cash_account: "CASH-GS-EUR-001"
      cash_bic: "IRVTDEFX"

    # JP Morgan collateral SSI
    - name: JPM_COLLATERAL_SSI
      counterparty:
        type: LEI
        value: "8IE5DZWZ7BX5LA5DJB03"
      currency: USD
      custody_account: "COLL-JPM-USD-001"
      custody_bic: "IRVTUS3N"
      cash_account: "CASH-JPM-USD-001"
      cash_bic: "IRVTUS3N"

    # Barclays collateral SSI
    - name: BARC_COLLATERAL_SSI
      counterparty:
        type: LEI
        value: "213800LBQA1Y9L22JB70"
      currency: GBP
      custody_account: "COLL-BARC-GBP-001"
      custody_bic: "IRVTGB2X"
      cash_account: "CASH-BARC-GBP-001"
      cash_bic: "IRVTGB2X"
```

---

## Summary of Decisions

| Issue | Decision | Implementation |
|-------|----------|----------------|
| EntityRef storage | LEI in `entity_funds`, `entity_manco`, `entity_settlement_identity` | Union query across all three |
| EntityRef resolution | **Option B** - Resolve at materialize time | `resolve_entity_ref()` function |
| Entity not found | **Fail** with clear error | `ResolveError::NotFound` with hint |
| CSA SSI | **Reference pattern** - `collateral_ssi_ref: "NAME"` | String field, validate exists |
| SSI source of truth | `standing_instructions.OTC_COLLATERAL` | Single definition, CSA references |
| Materialization order | SSIs first, then ISDA/CSA | Returns `HashMap<name, ssi_id>` for lookup |

---

## Files to Update

1. **`rust/src/trading_profile/types.rs`**
   - Change `CSAConfig.collateral_ssi: SSI` → `collateral_ssi_ref: String`

2. **`rust/src/trading_profile/resolve.rs`** (NEW)
   - `resolve_entity_ref()` function
   - `resolve_by_lei()`, `resolve_by_bic()`, `resolve_by_name()`

3. **`rust/src/trading_profile/validate.rs`**
   - Add `validate_csa_ssi_refs()`

4. **`rust/src/trading_profile/materialize.rs`**
   - Use `resolve_entity_ref()` for counterparties
   - Return `ssi_map` from SSI sync
   - Look up `collateral_ssi_ref` in `ssi_map`

5. **`rust/config/seed/trading_profiles/allianzgi_complete.yaml`**
   - Change all `collateral_ssi:` to `collateral_ssi_ref:`
   - Consolidate SSI definitions in `standing_instructions.OTC_COLLATERAL`
