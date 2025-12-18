# UBO Architecture Migration - Summary of Changes

**Date**: 2024-12-17  
**Purpose**: Migrate from legacy UBO tables to unified `ubo_edges` graph with clean separation of concerns

## Architecture Decision

### Before (Legacy)
Three separate tables with overlapping concerns:
- `ownership_relationships` - ownership edges (no cbu_id, entity-to-entity only)
- `control_relationships` - control edges (no cbu_id, entity-to-entity only)  
- `ubo_registry` - UBO determinations with KYC workflow state mixed in

### After (Clean Architecture)
Two tables with clear separation:

```
┌─────────────────────────────────────────────────────────────────┐
│                      ubo_edges (ob-poc schema)                   │
│  STRUCTURAL GRAPH - Shared state across all domains              │
│  - Ownership relationships (A owns X% of B)                      │
│  - Control relationships (A controls B via board/voting)         │
│  - Trust roles (settlor, protector, beneficiary)                │
│  - Status workflow: alleged → pending → proven → disputed        │
│  - Used by: UBO, Onboarding, Trading, Visualization             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ UBO verbs update BOTH
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                entity_workstreams (kyc schema)                   │
│  KYC WORKFLOW STATE - Per-entity investigation metadata          │
│  - is_ubo: boolean (derived from graph analysis)                │
│  - ownership_percentage: computed from chains                   │
│  - risk_rating: from screening/assessment                       │
│  - status: PENDING → COLLECT → VERIFY → SCREEN → COMPLETE       │
│  - Used by: KYC Case workflow only                              │
└─────────────────────────────────────────────────────────────────┘
```

## Files Modified

### 1. CLAUDE.md (Documentation)
**Location**: `/CLAUDE.md` (line ~2567)

Added new section "### UBO Graph Architecture" documenting:
- Clean separation between `ubo_edges` (graph) and `entity_workstreams` (KYC)
- Key tables and their purposes
- Convergence model status workflow
- UBO verb pattern example

### 2. SQL Migration
**Location**: `/rust/migrations/202412_ubo_temporal_and_cleanup.sql` (NEW FILE)

Added:
- `effective_from DATE` column to `ubo_edges` for temporal ownership start
- `effective_to DATE` column to `ubo_edges` for temporal ownership end
- Check constraint `chk_ubo_edges_temporal` ensuring valid date ranges
- Index `idx_ubo_edges_temporal` for efficient temporal queries
- View `ubo_edges_current` for non-expired edges only
- Helper function `is_natural_person(entity_id)` for UBO qualification
- View `ubo_candidates` computing natural persons with ≥25% effective ownership
- Deprecation comments on legacy tables (not dropped yet for safety)

**Migration Status**: Successfully applied to database

### 3. UBO Graph Operations
**Location**: `/rust/src/dsl_v2/custom_ops/ubo_graph_ops.rs`

#### Added helper function `sync_ubo_workstream_status()` (lines 699-791)
```rust
/// Sync entity_workstreams.is_ubo based on proven ownership in ubo_edges
///
/// When an edge is verified:
/// 1. Check if from_entity is a natural person (PERSON category)
/// 2. Calculate their total effective ownership percentage across all proven edges
/// 3. If ≥25%, update their entity_workstreams.is_ubo = true
```

This function:
- Checks if entity is a natural person (via `entity_types.entity_category = 'PERSON'`)
- Calculates total proven ownership percentage from `ubo_edges`
- Finds active KYC case for the CBU
- Upserts `entity_workstreams` with `is_ubo` flag and `ownership_percentage`
- Logs the sync operation via tracing

#### Modified `UboVerifyOp::execute()` (line ~942-956)
Added call to `sync_ubo_workstream_status()` after edge is verified as proven:
```rust
// After updating proof status to 'verified':
sync_ubo_workstream_status(
    pool,
    edge.cbu_id,
    edge.from_entity_id,
    proven_percentage,
).await?;
```

## Database State

Current row counts:
| Table | Rows | Status |
|-------|------|--------|
| `ubo_edges` | 12 | Active - use this |
| `ownership_relationships` | 1 | Deprecated |
| `control_relationships` | 0 | Deprecated |
| `ubo_registry` | 0 | Deprecated |

## Remaining Tasks

1. **Update legacy CRUD verbs** - `add-ownership`, `update-ownership`, etc. in `config/verbs/ubo.yaml` need to use `ubo_edges` instead of `ownership_relationships`

2. **Update visualization code** - `graph/builder.rs` `load_ubo_layer()` queries `ubo_registry` instead of `ubo_edges`

3. **Update workflow guards/requirements** - `workflow/guards.rs` and `workflow/requirements.rs` reference legacy tables

4. **Update custom_ops** - `calculate` and `delete-cascade` in `custom_ops/mod.rs` use legacy tables

5. **Update mcp/enrichment.rs** - Entity enrichment queries reference legacy tables

6. **Build and test** - Full compilation and test suite

## Breaking Changes

None yet - legacy tables still exist with deprecation comments. Code changes are additive.

## Rollback Plan

1. Remove temporal columns: `ALTER TABLE "ob-poc".ubo_edges DROP COLUMN effective_from, DROP COLUMN effective_to;`
2. Revert `ubo_graph_ops.rs` changes
3. Remove documentation from CLAUDE.md

## Testing Recommendations

1. Create a test CBU with entities and UBO edges
2. Run `ubo.allege` → `ubo.link-proof` → `ubo.verify` flow
3. Verify `entity_workstreams.is_ubo` is updated correctly
4. Check temporal queries work with `effective_from`/`effective_to`
5. Verify `ubo_candidates` view returns correct UBOs
