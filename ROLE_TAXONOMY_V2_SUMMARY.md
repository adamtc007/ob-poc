# Role Taxonomy V2 Implementation Summary

**Commit:** a634cb6

## Database Schema Updates

Added missing columns to:

### cbu_entity_roles
- `target_entity_id` - FK to owned/controlled entity
- `ownership_percentage` - For ownership roles
- `effective_from` / `effective_to` - Role validity dates
- `updated_at` - Timestamp

### entity_relationships
- `trust_interest_type` - fixed, discretionary, contingent
- `updated_at` - Timestamp

## 8 Custom Operation Handlers (cbu_role_ops.rs)

| Handler | Purpose |
|---------|---------|
| `assign-ownership` | Creates ASSET_OWNER role + ownership relationship edge |
| `assign-control` | Creates control roles + control relationship edge |
| `assign-trust` | Creates trust roles + trust relationship edge |
| `assign-simple` | Creates simple roles (no relationship edge) |
| `end-role` | Ends role with effective_to date |
| `update-ownership` | Updates ownership percentage |
| `update-control` | Updates control type |
| `list-by-entity` | Lists all roles for an entity |

## Rust Fixes Applied

- `ExecutionResult::Success` → `ExecutionResult::Record(json)`
- `rust_decimal::Decimal` → `sqlx::types::BigDecimal`
- Fixed date parsing from AstNode (no `as_date()` method)
- Fixed BigDecimal borrow/move issues with `.clone()`

## Files Changed

- `rust/src/dsl_v2/custom_ops/cbu_role_ops.rs` (new)
- `rust/src/dsl_v2/custom_ops/mod.rs`
- `rust/migrations/202501_role_taxonomy_v2_fix.sql` (new)
- `docs/ROLE_TAXONOMY_V2_FIX.md` (new)
- `schema_export.sql`
- `CLAUDE.md`
