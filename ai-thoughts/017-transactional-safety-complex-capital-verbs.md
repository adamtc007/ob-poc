# Research: Transactional Safety for Complex Capital Verbs

> **Status:** Research / Design
> **Priority:** High - Prerequisite for 016 implementation
> **Created:** 2026-01-10
> **Scope:** `capital.split` and `capital.dilution.exercise` verb handlers

---

## Problem Statement

Two verbs in the capital structure model require multi-table updates that must be atomic:

```
capital.split (e.g., 2:1 stock split)
├─► INSERT issuance_events
├─► UPDATE share_class_supply
├─► UPDATE kyc.holdings (ALL holders of this class)
└─► UPDATE dilution_instruments (ALL instruments converting to this class)

capital.dilution.exercise
├─► UPDATE dilution_instruments (decrement outstanding)
├─► INSERT dilution_exercise_events (audit)
├─► INSERT kyc.holdings (new position)
└─► INSERT kyc.movements (conversion movement)
```

**Failure scenarios we must prevent:**

1. **Partial split:** Holdings updated but dilution instruments not → ownership math breaks
2. **Partial exercise:** Instrument decremented but holding not created → shares vanish
3. **Concurrent split:** Two splits execute simultaneously → double adjustment
4. **Exercise during split:** Exercise uses pre-split conversion ratio mid-split → wrong shares issued

---

## MANDATORY: Read Before Implementing

**Claude MUST read these before implementing the complex handlers:**

```bash
# 1. Existing advisory lock patterns in ob-poc
view /Users/adamtc007/Developer/ob-poc/rust/src/db/locks.rs

# 2. Existing transactional patterns
grep -r "begin_transaction\|sqlx::Transaction" /Users/adamtc007/Developer/ob-poc/rust/src/

# 3. PostgreSQL isolation levels
# https://www.postgresql.org/docs/current/transaction-iso.html

# 4. Existing movement/holding patterns
view /Users/adamtc007/Developer/ob-poc/rust/src/dsl_v2/custom_ops/investor_ops.rs
```

---

## Research Areas

### 1. PostgreSQL Transaction Isolation Levels

| Level | Dirty Read | Non-Repeatable Read | Phantom Read | Use Case |
|-------|------------|---------------------|--------------|----------|
| READ COMMITTED | No | Yes | Yes | Default, fine for single-row ops |
| REPEATABLE READ | No | No | Yes | Consistent reads within txn |
| SERIALIZABLE | No | No | No | **Required for splits** |

**Recommendation:** `capital.split` should use SERIALIZABLE isolation to prevent concurrent modifications.

```sql
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;
-- all split operations
COMMIT;
```

### 2. Advisory Locks (Entity-Level)

ob-poc already uses PostgreSQL advisory locks. Pattern:

```rust
// Existing pattern from ob-poc
pub async fn with_entity_lock<F, T>(
    pool: &PgPool,
    entity_id: Uuid,
    f: F,
) -> Result<T>
where
    F: FnOnce() -> Future<Output = Result<T>>,
{
    // pg_advisory_xact_lock takes a bigint
    let lock_id = entity_id_to_lock_id(entity_id);
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_id)
        .execute(pool)
        .await?;
    
    f().await
    // Lock released on transaction commit/rollback
}
```

**For splits:** Lock the share_class_id (issuer-level lock may be too broad)

**For exercise:** Lock the instrument_id

### 3. Idempotency Keys

Prevent duplicate operations from retries:

```sql
-- Add to issuance_events
idempotency_key VARCHAR(100) UNIQUE,

-- Add to dilution_exercise_events  
idempotency_key VARCHAR(100) UNIQUE,
```

Handler generates key from inputs:
```rust
let idempotency_key = format!(
    "split:{}:{}:{}:{}",
    share_class_id, ratio_from, ratio_to, effective_date
);
```

If key exists → return existing event_id, don't re-execute.

### 4. Optimistic vs Pessimistic Locking

**Pessimistic (advisory locks):**
- Lock acquired at start
- Others wait
- Good for: splits (rare, high-impact)

**Optimistic (version columns):**
- Read version, write with WHERE version = X
- Retry on conflict
- Good for: exercises (frequent, localized)

**Recommendation:** 
- `capital.split` → Pessimistic (advisory lock on share_class_id)
- `capital.dilution.exercise` → Optimistic with retry

### 5. Existing ob-poc Patterns to Leverage

Check what patterns already exist:

```bash
# Advisory locks
grep -r "advisory_lock\|pg_advisory" /Users/adamtc007/Developer/ob-poc/rust/src/

# Transaction handling
grep -r "Transaction\|begin\|commit\|rollback" /Users/adamtc007/Developer/ob-poc/rust/src/

# Existing multi-table ops
grep -r "multi_table\|atomic\|transactional" /Users/adamtc007/Developer/ob-poc/rust/src/
```

---

## Proposed Patterns

### Pattern A: Split Operation

```rust
pub struct CapitalSplitOp;

#[async_trait]
impl CustomOperation for CapitalSplitOp {
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = ctx.resolve_uuid_arg(verb_call, "share-class-id")?;
        let ratio_from = ctx.resolve_int_arg(verb_call, "ratio-from")?;
        let ratio_to = ctx.resolve_int_arg(verb_call, "ratio-to")?;
        let effective_date = ctx.resolve_date_arg_or(verb_call, "effective-date", today())?;
        
        // Generate idempotency key
        let idempotency_key = format!(
            "split:{}:{}:{}:{}",
            share_class_id, ratio_from, ratio_to, effective_date
        );
        
        // Check for existing (idempotent)
        if let Some(existing) = check_idempotency_key(pool, &idempotency_key).await? {
            return Ok(ExecutionResult::Uuid(existing));
        }
        
        // Start SERIALIZABLE transaction
        let mut tx = pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *tx)
            .await?;
        
        // Advisory lock on share class
        let lock_id = share_class_id_to_lock_id(share_class_id);
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_id)
            .execute(&mut *tx)
            .await?;
        
        // 1. Validate current state
        let current_supply = get_current_supply(&mut tx, share_class_id).await?;
        if current_supply.issued_units == 0 {
            return Err(anyhow!("Cannot split share class with no issued units"));
        }
        
        // 2. Calculate new values
        let multiplier = Decimal::from(ratio_to) / Decimal::from(ratio_from);
        let new_issued = current_supply.issued_units * multiplier;
        
        // 3. Insert issuance event (with idempotency key)
        let event_id = insert_issuance_event(
            &mut tx,
            share_class_id,
            "STOCK_SPLIT",
            new_issued - current_supply.issued_units,
            ratio_from,
            ratio_to,
            effective_date,
            &idempotency_key,
        ).await?;
        
        // 4. Update supply
        update_supply(&mut tx, share_class_id, new_issued, effective_date, event_id).await?;
        
        // 5. Bulk update ALL holdings for this share class
        let holdings_updated = sqlx::query(
            r#"
            UPDATE kyc.holdings
            SET units = units * $1,
                cost_basis = CASE WHEN cost_basis IS NOT NULL 
                             THEN cost_basis / $1 ELSE NULL END,
                updated_at = now()
            WHERE share_class_id = $2
              AND status = 'active'
            "#
        )
        .bind(multiplier)
        .bind(share_class_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        
        // 6. Bulk update ALL dilution instruments converting to this class
        let instruments_updated = sqlx::query(
            r#"
            UPDATE kyc.dilution_instruments
            SET conversion_ratio = conversion_ratio * $1,
                exercise_price = CASE WHEN exercise_price IS NOT NULL 
                                 THEN exercise_price / $1 ELSE NULL END,
                updated_at = now()
            WHERE converts_to_share_class_id = $2
              AND status = 'ACTIVE'
            "#
        )
        .bind(multiplier)
        .bind(share_class_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        
        // 7. Commit (releases advisory lock)
        tx.commit().await?;
        
        tracing::info!(
            "capital.split: {}:{} on {} - {} holdings, {} instruments adjusted",
            ratio_from, ratio_to, share_class_id, holdings_updated, instruments_updated
        );
        
        Ok(ExecutionResult::Uuid(event_id))
    }
}
```

**Key safety features:**
1. SERIALIZABLE isolation - no concurrent reads see stale data
2. Advisory lock on share_class_id - serializes all splits on same class
3. Idempotency key - prevents duplicate application
4. Single transaction - all-or-nothing
5. Logs affected counts - audit trail

### Pattern B: Exercise Operation

```rust
pub struct CapitalDilutionExerciseOp;

#[async_trait]
impl CustomOperation for CapitalDilutionExerciseOp {
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let instrument_id = ctx.resolve_uuid_arg(verb_call, "instrument-id")?;
        let units_to_exercise = ctx.resolve_decimal_arg(verb_call, "units")?;
        let exercise_date = ctx.resolve_date_arg_or(verb_call, "exercise-date", today())?;
        let is_cashless = ctx.resolve_bool_arg_or(verb_call, "is-cashless", false)?;
        
        // Idempotency key
        let idempotency_key = format!(
            "exercise:{}:{}:{}",
            instrument_id, units_to_exercise, exercise_date
        );
        
        if let Some(existing) = check_idempotency_key(pool, &idempotency_key).await? {
            return Ok(ExecutionResult::Uuid(existing));
        }
        
        // Retry loop for optimistic locking
        let max_retries = 3;
        for attempt in 0..max_retries {
            match try_exercise(
                pool,
                instrument_id,
                units_to_exercise,
                exercise_date,
                is_cashless,
                &idempotency_key,
            ).await {
                Ok(result) => return Ok(result),
                Err(e) if is_serialization_error(&e) && attempt < max_retries - 1 => {
                    tracing::warn!("Exercise retry {} due to serialization conflict", attempt + 1);
                    tokio::time::sleep(Duration::from_millis(50 * (attempt + 1) as u64)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        Err(anyhow!("Exercise failed after {} retries", max_retries))
    }
}

async fn try_exercise(
    pool: &PgPool,
    instrument_id: Uuid,
    units_to_exercise: Decimal,
    exercise_date: NaiveDate,
    is_cashless: bool,
    idempotency_key: &str,
) -> Result<ExecutionResult> {
    let mut tx = pool.begin().await?;
    
    // 1. Lock and fetch instrument with version check
    let instrument = sqlx::query_as::<_, DilutionInstrument>(
        r#"
        SELECT * FROM kyc.dilution_instruments
        WHERE instrument_id = $1
        FOR UPDATE  -- Row-level lock
        "#
    )
    .bind(instrument_id)
    .fetch_one(&mut *tx)
    .await?;
    
    // 2. Validate
    if instrument.status != "ACTIVE" {
        return Err(anyhow!("Instrument {} is not active (status={})", 
            instrument_id, instrument.status));
    }
    if !instrument.is_exercisable {
        return Err(anyhow!("Instrument {} is not currently exercisable", instrument_id));
    }
    if units_to_exercise > instrument.units_outstanding {
        return Err(anyhow!(
            "Cannot exercise {} units, only {} outstanding",
            units_to_exercise, instrument.units_outstanding
        ));
    }
    
    // 3. Calculate shares to issue
    let shares_to_issue = units_to_exercise * instrument.conversion_ratio;
    let shares_after_tax = if is_cashless {
        // Simplified: withhold 40% for taxes
        shares_to_issue * Decimal::from_str("0.6")?
    } else {
        shares_to_issue
    };
    
    // 4. Update instrument
    let rows = sqlx::query(
        r#"
        UPDATE kyc.dilution_instruments
        SET units_exercised = units_exercised + $1,
            status = CASE 
                WHEN units_exercised + $1 >= units_granted - units_forfeited 
                THEN 'EXERCISED' ELSE status END,
            updated_at = now()
        WHERE instrument_id = $2
          AND units_outstanding >= $1  -- Optimistic check
        "#
    )
    .bind(units_to_exercise)
    .bind(instrument_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    
    if rows == 0 {
        // Concurrent modification - will retry
        return Err(anyhow!("Concurrent modification detected").context("SERIALIZATION"));
    }
    
    // 5. Create holding (or update existing)
    let holding_id = upsert_holding(
        &mut tx,
        instrument.holder_entity_id,
        instrument.converts_to_share_class_id,
        shares_after_tax,
        instrument.exercise_price,
        exercise_date,
    ).await?;
    
    // 6. Create movement record
    let movement_id = insert_movement(
        &mut tx,
        holding_id,
        "conversion",
        shares_after_tax,
        instrument.exercise_price,
        exercise_date,
    ).await?;
    
    // 7. Create exercise event (audit)
    let exercise_id = sqlx::query_scalar(
        r#"
        INSERT INTO kyc.dilution_exercise_events (
            instrument_id, units_exercised, exercise_date, 
            exercise_price_paid, shares_issued, resulting_holding_id,
            is_cashless, shares_withheld_for_tax, idempotency_key
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING exercise_id
        "#
    )
    .bind(instrument_id)
    .bind(units_to_exercise)
    .bind(exercise_date)
    .bind(instrument.exercise_price)
    .bind(shares_to_issue)
    .bind(holding_id)
    .bind(is_cashless)
    .bind(if is_cashless { Some(shares_to_issue - shares_after_tax) } else { None })
    .bind(idempotency_key)
    .fetch_one(&mut *tx)
    .await?;
    
    // 8. Commit
    tx.commit().await?;
    
    tracing::info!(
        "capital.dilution.exercise: {} units of {} → {} shares in holding {}",
        units_to_exercise, instrument_id, shares_after_tax, holding_id
    );
    
    Ok(ExecutionResult::Uuid(exercise_id))
}
```

**Key safety features:**
1. `FOR UPDATE` row lock - prevents concurrent exercise of same instrument
2. Optimistic check in UPDATE WHERE clause
3. Retry loop for serialization conflicts
4. Idempotency key - prevents double exercise
5. Single transaction - all-or-nothing
6. Detailed audit event

---

## Error Recovery

### What if transaction fails mid-way?

PostgreSQL guarantees atomicity - partial commits are impossible. But we should log failures:

```rust
async fn execute(...) -> Result<ExecutionResult> {
    let result = do_transaction(...).await;
    
    match &result {
        Err(e) => {
            tracing::error!(
                verb = "capital.split",
                share_class_id = %share_class_id,
                error = %e,
                "Split transaction failed - no changes applied"
            );
            // Optionally: record in a failed_operations table for review
        }
        Ok(_) => {}
    }
    
    result
}
```

### What if the app crashes after commit but before response?

Idempotency key handles this - client retries, we return existing result.

### What if we need to reverse a split?

Splits are **irreversible** by design. To undo:
1. Execute reverse split (e.g., 1:2 after a 2:1)
2. Creates new issuance_event with type `CONSOLIDATION`
3. Audit trail preserved

---

## Testing Requirements

### Unit Tests

```rust
#[tokio::test]
async fn test_split_atomicity() {
    // Setup: share class with holdings + dilution instruments
    // Execute split
    // Verify: ALL holdings adjusted OR NONE adjusted
}

#[tokio::test]
async fn test_split_idempotency() {
    // Execute same split twice
    // Verify: second call returns same event_id, no double adjustment
}

#[tokio::test]
async fn test_split_concurrent_block() {
    // Start two splits simultaneously
    // Verify: one waits for the other (serialized)
}

#[tokio::test]
async fn test_exercise_concurrent_same_instrument() {
    // Two users exercise same instrument simultaneously
    // Verify: total exercised <= outstanding
}

#[tokio::test]
async fn test_exercise_during_split() {
    // Start exercise, start split, complete both
    // Verify: shares issued use correct (post-split) ratio
}
```

### Integration Tests

```sql
-- Simulate crash mid-split (connection kill)
-- Verify: no partial state

-- Verify holding sum equals issued after split
SELECT 
    scs.issued_units,
    SUM(h.units) as holdings_sum,
    scs.issued_units - SUM(h.units) as variance
FROM kyc.share_class_supply scs
JOIN kyc.holdings h ON h.share_class_id = scs.share_class_id
WHERE h.status = 'active'
GROUP BY scs.share_class_id, scs.issued_units
HAVING ABS(scs.issued_units - SUM(h.units)) > 0.01;
-- Should return 0 rows
```

---

## Schema Additions Required

Add idempotency columns to:

```sql
-- In issuance_events (already in 016 migration)
ALTER TABLE kyc.issuance_events
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(100) UNIQUE;

-- In dilution_exercise_events (already in 016 migration)  
ALTER TABLE kyc.dilution_exercise_events
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(100) UNIQUE;
```

---

## Summary: Implementation Checklist for Complex Verbs

### `capital.split`
- [ ] SERIALIZABLE transaction isolation
- [ ] Advisory lock on share_class_id
- [ ] Idempotency key check at start
- [ ] Bulk UPDATE holdings with multiplier
- [ ] Bulk UPDATE dilution_instruments with multiplier
- [ ] Adjust exercise_price inversely
- [ ] Log affected row counts
- [ ] Test concurrent split blocking
- [ ] Test idempotency on retry

### `capital.dilution.exercise`
- [ ] FOR UPDATE row lock on instrument
- [ ] Optimistic WHERE clause on units_outstanding
- [ ] Retry loop (3 attempts) for serialization conflicts
- [ ] Idempotency key check at start
- [ ] Create holding via upsert pattern
- [ ] Create movement record
- [ ] Create audit event
- [ ] Handle cashless exercise tax withholding
- [ ] Test concurrent exercise same instrument
- [ ] Test exercise during split

---

## Rust Pattern: Parameter Structs for Complex Operations

When a transactional function has many parameters (>5-6), use a **parameter struct** instead of individual arguments. This pattern provides:

1. **Self-documentation** - Field names and doc comments describe each parameter
2. **Optional handling** - `Option<T>` fields with sensible defaults
3. **Validation methods** - Add `fn validate(&self) -> Result<()>` to the struct
4. **Cleaner call sites** - Named fields instead of positional arguments
5. **Clippy compliance** - Avoids `too_many_arguments` lint

### Example: ExerciseParams

```rust
/// Parameters for exercise operation
#[cfg(feature = "database")]
struct ExerciseParams {
    /// The instrument to exercise
    instrument_id: Uuid,
    /// Number of units to exercise
    units_to_exercise: rust_decimal::Decimal,
    /// Date of exercise (defaults to today)
    exercise_date: NaiveDate,
    /// Override the instrument's exercise price (None = use instrument's price)
    exercise_price_override: Option<rust_decimal::Decimal>,
    /// Cashless exercise withholds shares for tax
    is_cashless: bool,
    /// Idempotency key to prevent duplicate operations
    idempotency_key: String,
}

impl ExerciseParams {
    /// Validate parameters before execution
    fn validate(&self) -> Result<()> {
        if self.units_to_exercise <= Decimal::ZERO {
            return Err(anyhow!("units must be positive"));
        }
        Ok(())
    }
}
```

### Usage Pattern

```rust
// In execute() - build params from verb_call
let params = ExerciseParams {
    instrument_id,
    units_to_exercise,
    exercise_date,
    exercise_price_override,
    is_cashless,
    idempotency_key,
};

// Retry loop with clean call
for attempt in 0..max_retries {
    match self.try_exercise(pool, &params).await {
        Ok(result) => return Ok(result),
        Err(e) if is_serialization_error(&e) => continue,
        Err(e) => return Err(e),
    }
}

// In try_exercise() - access via params.field_name
async fn try_exercise(&self, pool: &PgPool, params: &ExerciseParams) -> Result<...> {
    // Use params.instrument_id, params.units_to_exercise, etc.
}
```

### When to Use This Pattern

- **Always** for functions with 6+ parameters
- **Recommended** for functions with 4-5 parameters if they're related
- **Consider** when you want to add validation or defaults

---

## References

- PostgreSQL Transaction Isolation: https://www.postgresql.org/docs/current/transaction-iso.html
- PostgreSQL Advisory Locks: https://www.postgresql.org/docs/current/explicit-locking.html#ADVISORY-LOCKS
- Idempotency Patterns: https://stripe.com/docs/api/idempotent_requests
- Existing ob-poc lock patterns: `/rust/src/db/locks.rs`

