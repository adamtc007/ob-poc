# TODO: CBU Connection & Entity Tree Review

## Objective
Review and refine all connections from CBU node through the entity tree for proper visualization layout.

## Current State
- Products/services now show at top under CBU node
- Entities grouped by role with OWNERSHIP_CONTROL at top, TRADING_EXECUTION at bottom
- Role priority driven by `role_category` in database view `v_cbu_entity_with_roles`

## Connection Types to Review

### 1. CBU → Products (service layer)
- [x] Products show linked to CBU
- [ ] Review product → service → resource hierarchy
- [ ] Consider collapsing service details under product

### 2. CBU → Entities (core layer)
- [x] Entities grouped by role
- [x] Sorted by role_category (ownership top, trading bottom)
- [ ] Review all role types and ensure correct categorization
- [ ] Consider sub-grouping within categories

### 3. Entity → Entity Relationships
- [ ] Ownership chains (UBO layer)
- [ ] Control relationships (non-ownership control)
- [ ] Parent/subsidiary relationships

### 4. Entity Role Categories

#### OWNERSHIP_CONTROL (top of tree)
- BENEFICIAL_OWNER
- SHAREHOLDER
- LIMITED_PARTNER / GENERAL_PARTNER
- DIRECTOR
- TRUSTEE / SETTLOR / PROTECTOR
- AUTHORIZED_SIGNATORY

#### BOTH (middle)
- PRINCIPAL
- COMMERCIAL_CLIENT
- SERVICE_PROVIDER

#### TRADING_EXECUTION (bottom of tree)
- ASSET_OWNER
- INVESTMENT_MANAGER
- CUSTODIAN
- ADMINISTRATOR
- PRIME_BROKER
- AUDITOR / LEGAL_COUNSEL

### 5. Layout Rules to Define
- [ ] Left-to-right hierarchy: CBU → intermediate nodes → leaf entities
- [ ] Vertical ordering within groups
- [ ] Edge labels (role names on connections)
- [ ] Node sizing based on importance
- [ ] Color coding by role category

### 6. Data Sources
- `v_cbu_entity_with_roles` - aggregated entity/role view
- `CbuGraphBuilder` - builds graph from VisualizationRepository
- `/api/cbu/:id/graph` - graph endpoint
- `renderCbuTree()` in app.ts - UI rendering

### 7. Files to Review
- `rust/src/database/visualization_repository.rs` - DB queries
- `rust/src/graph/builder.rs` - graph construction
- `rust/src/graph/types.rs` - node/edge types
- `go/cmd/web/static/app.ts` - UI rendering (mapGraphToCbuData, renderCbuTree)
- SQL view: `"ob-poc".v_cbu_entity_with_roles`

## Next Steps
1. Map all entity relationship types in the database
2. Define layout taxonomy rules for each relationship type
3. Update graph builder to include all relationship edges
4. Refine UI rendering to match layout rules
