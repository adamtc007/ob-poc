# Entity Type Dependencies Migration

## Overview

This migration creates the unified `entity_type_dependencies` table that replaces the resource-specific `resource_dependencies` table with a generalized dependency model.

## Files

| File | Purpose |
|------|---------|
| `000_pre_migration_check.sql` | Diagnostic - check current state before migration |
| `001_entity_type_dependencies.sql` | Main migration - creates table and seeds data |
| `002_sync_resource_dependencies.sql` | Sync - copies any missing resource deps |
| `003_post_migration_validation.sql` | Validation - verify migration success |

## Execution Order

```bash
# 1. Check current state
psql -d data_designer -f migrations/000_pre_migration_check.sql

# 2. Run main migration (creates table + seeds)
psql -d data_designer -f migrations/001_entity_type_dependencies.sql

# 3. Sync any existing resource_dependencies not in seed data
psql -d data_designer -f migrations/002_sync_resource_dependencies.sql

# 4. Validate everything worked
psql -d data_designer -f migrations/003_post_migration_validation.sql
```

## What Gets Created

### Table: `entity_type_dependencies`

```
┌──────────────────┬──────────────────┬───────────────────────────┬──────────┐
│ from_type        │ from_subtype     │ to_type:to_subtype        │ via_arg  │
├──────────────────┼──────────────────┼───────────────────────────┼──────────┤
│ case             │ NULL             │ cbu                       │ cbu-id   │
│ workstream       │ NULL             │ case                      │ case-id  │
│ kyc_case         │ NULL             │ cbu                       │ cbu-id   │
│ fund             │ NULL             │ cbu                       │ cbu-id   │
│ entity           │ fund_umbrella    │ entity                    │ legal-.. │
│ entity           │ fund_sub         │ entity:fund_umbrella      │ umbrel.. │
│ entity           │ share_class      │ entity:fund_sub           │ sub-f..  │
│ resource_instance│ CUSTODY_ACCT     │ resource_instance:SETTLE  │ settle.. │
│ resource_instance│ SWIFT_CONN       │ resource_instance:CUSTODY │ custod.. │
│ ...              │ ...              │ ...                       │ ...      │
└──────────────────┴──────────────────┴───────────────────────────┴──────────┘
```

### Seeded Dependencies

**Structural (type-level):**
- case → cbu
- workstream → case
- document → entity (optional)
- document → cbu (optional)
- observation → entity
- kyc_case → cbu
- fund → cbu

**Fund Hierarchy (subtype-level):**
- entity:fund_umbrella → entity
- entity:fund_sub → entity:fund_umbrella
- entity:share_class → entity:fund_sub
- entity:fund_master → entity:fund_umbrella
- entity:fund_feeder → entity:fund_master

**Service Resources:**
- resource_instance:CUSTODY_ACCT → resource_instance:SETTLE_ACCT
- resource_instance:SWIFT_CONN → resource_instance:CUSTODY_ACCT
- resource_instance:NAV_ENGINE → resource_instance:CUSTODY_ACCT
- resource_instance:CA_PLATFORM → resource_instance:CUSTODY_ACCT
- resource_instance:REPORTING → resource_instance:CUSTODY_ACCT
- resource_instance:PERF_ANALYTICS → resource_instance:CUSTODY_ACCT
- resource_instance:COLLATERAL_MGMT → resource_instance:CUSTODY_ACCT
- resource_instance:SEC_LENDING → resource_instance:CUSTODY_ACCT

## Integration

After migration, initialize the registry at server startup:

```rust
use ob_poc::dsl_v2::init_entity_deps;

async fn main() {
    let pool = PgPool::connect(&db_url).await?;
    
    // Load entity dependencies into global registry
    init_entity_deps(&pool).await?;
    
    // ... rest of startup
}
```

## Rollback

```sql
DROP TABLE IF EXISTS "ob-poc".entity_type_dependencies CASCADE;
```

The legacy `resource_dependencies` table is preserved and unchanged.

## Adding New Dependencies

No code changes required - just insert rows:

```sql
-- Add new resource type dependency
INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind)
VALUES 
('resource_instance', 'NEW_RESOURCE', 'resource_instance', 'CUSTODY_ACCT', 'custody-url', 'required');

-- Add new entity type dependency
INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind)
VALUES 
('entity', 'fund_etf', 'entity', 'fund_umbrella', 'umbrella-id', 'required');
```

Restart the server (or call `init_entity_deps` again) to pick up changes.
