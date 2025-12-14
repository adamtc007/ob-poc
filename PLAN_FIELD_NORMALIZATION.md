# Field Normalization Plan: mic vs market

## Problem Statement

Field naming is inconsistent across the trading profile types:

| Location | Field Name | Represents |
|----------|------------|------------|
| `MarketConfig.mic` | `mic` | MIC code (correct) |
| `BookingMatch.market` | `market` | MIC code |
| `StandingInstruction.market` | `market` | MIC code |
| `SubcustodianEntry.market` | `market` | MIC code |
| `ManagerScope.markets` | `markets` | MIC codes |
| `PricingScope.markets` | `markets` | MIC codes |
| `MatchingPlatform.enabled_markets` | `enabled_markets` | MIC codes |

The database uses `market_id` (UUID) as the FK column, with `mic` as the lookup code in `custody.markets`.

## Decision: Normalize to `mic`

**Rationale:**
- MIC (Market Identifier Code) is the ISO 10383 standard term
- `market` is ambiguous - could mean UUID, MIC, or name
- `mic` is already used in `MarketConfig` and database schema
- Verb YAML uses `market` as arg name but resolves via `code_column: mic`

**Scope:**
- Rust types only - no database changes needed
- YAML seed files will use new field names
- Verb YAML unchanged (uses `market` arg name, which is fine as user-facing)

## Changes Required

### 1. Rust Types (`rust/src/trading_profile/types.rs`)

```rust
// BEFORE
pub struct BookingMatch {
    pub market: Option<String>,  // Ambiguous
}

// AFTER  
pub struct BookingMatch {
    pub mic: Option<String>,  // Clear: ISO 10383 code
}
```

**Full list of changes:**

| Struct | Old Field | New Field |
|--------|-----------|-----------|
| `BookingMatch` | `market: Option<String>` | `mic: Option<String>` |
| `StandingInstruction` | `market: Option<String>` | `mic: Option<String>` |
| `SubcustodianEntry` | `market: String` | `mic: String` |
| `ManagerScope` | `markets: Vec<String>` | `mics: Vec<String>` |
| `PricingScope` | `markets: Vec<String>` | `mics: Vec<String>` |
| `MatchingPlatform` | `enabled_markets: Vec<String>` | `enabled_mics: Vec<String>` |

### 2. Materialization Code (`rust/src/dsl_v2/custom_ops/trading_profile.rs`)

Update field access:
- `rule.match_criteria.market` → `rule.match_criteria.mic`
- `ssi.market` → `ssi.mic`
- `market_cfg.mic` (already correct)

### 3. Seed YAML Files

Update `rust/config/seed/trading_profiles/allianzgi_complete.yaml`:
- `standing_instructions.*.market` → `mic`
- `settlement_config.subcustodian_network.*.market` → `mic`
- `booking_rules.*.match.market` → `mic`
- `investment_managers.*.scope.markets` → `mics`
- `pricing_matrix.*.scope.markets` → `mics`
- `settlement_config.matching_platforms.*.enabled_markets` → `enabled_mics`

## Test Harness Design

### Test Strategy

1. **Before changes**: Capture current state as baseline
2. **After changes**: Verify DB<>SQLX<>Rust alignment still works

### Test Harness (`rust/tests/trading_profile_field_test.rs`)

```rust
//! Test harness for field normalization validation
//! Verifies DB schema ↔ SQLX queries ↔ Rust types alignment

#[tokio::test]
async fn test_market_lookup_via_mic() {
    // Verify mic → market_id resolution works
}

#[tokio::test]  
async fn test_ssi_market_field_maps_correctly() {
    // Insert SSI with mic, read back, verify market_id populated
}

#[tokio::test]
async fn test_booking_rule_mic_lookup() {
    // Insert rule with mic filter, verify market_id FK correct
}

#[tokio::test]
async fn test_universe_mic_to_market_id() {
    // Insert universe entry, verify market_id resolved from mic
}

#[tokio::test]
async fn test_full_materialization_idempotent() {
    // Import profile, materialize twice, verify no errors
}

#[tokio::test]
async fn test_seed_yaml_deserializes() {
    // Parse seed YAML with new field names
}
```

### Test Cases

| Test | What it validates |
|------|-------------------|
| `market_lookup_via_mic` | `build_market_map()` returns mic→UUID correctly |
| `ssi_market_field_maps_correctly` | SSI with `mic: "XNYS"` gets correct `market_id` |
| `booking_rule_mic_lookup` | Booking rule with `mic: "XNYS"` resolves to market_id |
| `universe_mic_to_market_id` | Universe entry maps mic→market_id in DB |
| `full_materialization_idempotent` | Complete import→materialize→materialize works |
| `seed_yaml_deserializes` | Updated YAML parses without error |

## Implementation Steps

### Phase 1: Test Harness (Before Changes)

1. Create `rust/tests/trading_profile_field_test.rs`
2. Implement baseline tests using current field names
3. Run tests, ensure green baseline: `cargo test --test trading_profile_field_test`

### Phase 2: Rust Type Changes

1. Update `rust/src/trading_profile/types.rs`:
   - Rename fields as listed above
   - Add `#[serde(alias = "market")]` for backward compat during transition

2. Update `rust/src/dsl_v2/custom_ops/trading_profile.rs`:
   - Change all `.market` field access to `.mic`

3. Compile and fix any errors

### Phase 3: YAML Updates

1. Update seed file: `rust/config/seed/trading_profiles/allianzgi_complete.yaml`
2. Run deserialize test to verify

### Phase 4: Verify

1. Run full test suite: `cargo test --features database`
2. Run field-specific tests: `cargo test --test trading_profile_field_test`
3. Manual verification:
   ```bash
   # Import profile
   ./target/debug/dsl_cli execute -f examples/test.dsl
   
   # Verify data
   psql -d data_designer -c "SELECT ssi_name, market_id FROM custody.cbu_ssi LIMIT 5"
   ```

### Phase 5: Cleanup

1. Remove `#[serde(alias = "market")]` after confirming no old YAMLs in use
2. Update any documentation referencing old field names

## Rollback Plan

If issues found:
1. Revert Rust type changes (git checkout)
2. Keep `#[serde(alias = "...")]` for backward compat
3. Investigate specific failure case

## Files Modified

| File | Type of Change |
|------|----------------|
| `rust/src/trading_profile/types.rs` | Field renames |
| `rust/src/dsl_v2/custom_ops/trading_profile.rs` | Field access updates |
| `rust/config/seed/trading_profiles/allianzgi_complete.yaml` | YAML field names |
| `rust/tests/trading_profile_field_test.rs` | New test file |

## Estimated Effort

- Phase 1 (Test Harness): ~30 min
- Phase 2 (Rust Changes): ~15 min
- Phase 3 (YAML Updates): ~10 min  
- Phase 4 (Verify): ~15 min
- Phase 5 (Cleanup): ~5 min

**Total: ~1.5 hours**

## Success Criteria

1. ✅ All existing tests pass
2. ✅ New field-specific tests pass
3. ✅ `trading-profile.import` works with new YAML
4. ✅ `trading-profile.materialize` is idempotent
5. ✅ SSI/universe/booking rules have correct `market_id` FKs
