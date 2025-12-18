# TODO: Entity Relationship Graph Consolidation

## ✅ COMPLETED - 2024-12-18

This consolidation has been successfully implemented. The migration from the old tables (`ownership_relationships`, `control_relationships`, `ubo_edges`) to the new unified schema (`entity_relationships`, `cbu_relationship_verification`) is complete.

---

## Summary of Changes

### Database Changes

**New Tables (created earlier):**
- `entity_relationships` - Single source of truth for structural facts (CBU-agnostic)
  - `from_entity_id`, `to_entity_id`, `relationship_type`, `percentage`, `effective_to`
- `cbu_relationship_verification` - CBU-specific verification workflow state
  - `status`, `alleged_percentage`, `observed_percentage`, `proof_document_id`

**Dropped Tables:**
- `ownership_relationships` - migrated to `entity_relationships`
- `control_relationships` - migrated to `entity_relationships`
- `ubo_edges` - migrated to `entity_relationships` + `cbu_relationship_verification`
- `ubo_observations` - dependency of `ubo_edges`, dropped with cascade

**Updated SQL Functions:**
- `compute_ownership_chains` - rewritten to use `entity_relationships` table

**Recreated Views:**
- `ubo_convergence_status` - uses `cbu_relationship_verification`
- `ubo_missing_proofs` - joins `cbu_relationship_verification` with `entity_relationships`
- `ubo_expired_proofs` - joins verification with document_catalog

**Cascade-Dropped Views (were dependent on ubo_edges):**
- `ubo_edges_current`
- `ubo_candidates`

### Rust Code Updated

| File | Changes |
|------|---------|
| `rust/src/workflow/requirements.rs` | 5 queries updated to use `entity_relationships` |
| `rust/src/workflow/guards.rs` | 1 query updated |
| `rust/src/mcp/enrichment.rs` | 1 query updated |
| `rust/src/dsl_v2/custom_ops/ubo_analysis.rs` | 2 queries updated |
| `rust/src/dsl_v2/custom_ops/mod.rs` | Delete cascade now uses single `entity_relationships` table |
| `rust/src/dsl_v2/custom_ops/ubo_graph_ops.rs` | 3 operations updated (`UboUpdateAllegationOp`, `UboRemoveEdgeOp`, `UboMarkDirtyOp`) |
| `rust/src/dsl_v2/ubo_structure_builder.rs` | **REMOVED** - dead code that was never used |
| `rust/src/dsl_v2/mod.rs` | Removed module export for deleted file |

### Key Column Name Changes

| Old | New |
|-----|-----|
| `owner_entity_id` / `controller_entity_id` | `from_entity_id` |
| `owned_entity_id` / `controlled_entity_id` | `to_entity_id` |
| `ownership_percent` / `percentage` | `percentage` |
| `ended_at` | `effective_to` |
| `edge_id` | `relationship_id` |
| `edge_type` | `relationship_type` |

### Relationship Type Values

All lowercase in `entity_relationships.relationship_type`:
- `'ownership'` - A owns X% of B
- `'control'` - A controls B (board, voting, executive)
- `'trust_role'` - A has role in trust B

---

## Architecture (Final State)

```
┌─────────────────────────────────────────────────────────────────┐
│                    ENTITY_RELATIONSHIPS                         │
│                    (Single Source of Truth)                     │
├─────────────────────────────────────────────────────────────────┤
│  Structural edges - exist independent of any CBU:               │
│  - "Allianz GI GmbH" owns 100% of "Allianz Dynamic Fund"        │
│  - "Sarah Chen" controls "Apex Capital LLP" as managing_partner │
│                                                                 │
│  These relationships are FACTS about the world.                 │
│  They don't belong to a CBU - CBUs reference them.              │
└─────────────────────────────────────────────────────────────────┘
                              │
            ┌─────────────────┼─────────────────┐
            │                 │                 │
            ▼                 ▼                 ▼
┌───────────────────┐ ┌───────────────┐ ┌───────────────────────┐
│ CBU_RELATIONSHIP_ │ │ VISUALIZATION │ │ ONBOARDING            │
│ VERIFICATION      │ │ (reads graph) │ │ (creates edges)       │
│ (KYC workflow)    │ │               │ │                       │
├───────────────────┤ └───────────────┘ └───────────────────────┘
│ cbu_id            │
│ relationship_id   │→ FK to entity_relationships
│ alleged_percentage│
│ proof_document_id │
│ status: unverified│
│   → alleged →     │
│   pending → proven│
└───────────────────┘
```

---

## Key Principle

**Relationships are facts about the world. Verification is CBU-specific.**

- "Allianz GI owns 100% of Fund X" is a fact (lives in `entity_relationships`)
- "We verified this for CBU Y using document Z" is workflow state (lives in `cbu_relationship_verification`)

This separation enables:
- Same relationship, different verification status per CBU
- Visualization/onboarding can query graph without KYC clutter
- KYC workflow is isolated to its own table
- No duplication of structural data

---

## Verification

All 370 unit tests pass after the consolidation.

```bash
cargo test --lib  # All pass
cargo build       # Compiles successfully
cargo sqlx prepare # Query cache regenerated
```

---

*Migration completed 2024-12-18*
