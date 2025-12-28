# Role Taxonomy V2 - Implementation Specification

## Overview

This document specifies the implementation of a comprehensive role taxonomy for CBU/UBO visualization. The goal is to replace the ad-hoc role priority system with a structured taxonomy that:

1. **Separates ownership from control from services** - different layout behaviors
2. **Supports complex structures** - hedge funds, PE, trusts, prime broker chains
3. **Validates role assignments** - prevents invalid combinations
4. **Drives visualization layout** - pyramid for ownership, flat for services

## Files Created

1. `rust/migrations/202501_role_taxonomy_v2.sql` - Database migration with:
   - Extended `roles` table schema
   - `role_categories` reference table
   - `ubo_treatments` reference table  
   - `role_incompatibilities` validation table
   - `role_requirements` dependency table
   - 60+ role definitions across 9 categories
   - Validation functions

2. `rust/config/verbs/cbu-role-v2.yaml` - DSL verbs for role assignment:
   - `assign-ownership` - ownership chain with percentage
   - `assign-control` - control chain (directors, officers)
   - `assign-trust-role` - trust-specific roles
   - `assign-fund-role` - fund structure/management
   - `assign-service-provider` - flat service providers
   - `assign-signatory` - trading/execution roles

## Implementation Tasks

### Task 1: Update `rust/src/graph/types.rs`

Add new role category enum and layout behavior hints:

```rust
/// Role category from taxonomy - determines layout behavior
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoleCategory {
    OwnershipChain,      // Pyramid layout (UBOs at apex)
    ControlChain,        // Overlay on ownership
    FundStructure,       // Tree layout (umbrella → subfund)
    FundManagement,      // Satellite around fund
    TrustRoles,          // Radial around trust
    ServiceProvider,     // Flat bottom row
    TradingExecution,    // Flat right column
    InvestorChain,       // Pyramid down (below fund)
    RelatedParty,        // Peripheral
}

impl RoleCategory {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "OWNERSHIP_CHAIN" => Some(Self::OwnershipChain),
            "CONTROL_CHAIN" => Some(Self::ControlChain),
            "FUND_STRUCTURE" => Some(Self::FundStructure),
            "FUND_MANAGEMENT" => Some(Self::FundManagement),
            "TRUST_ROLES" => Some(Self::TrustRoles),
            "SERVICE_PROVIDER" => Some(Self::ServiceProvider),
            "TRADING_EXECUTION" => Some(Self::TradingExecution),
            "INVESTOR_CHAIN" => Some(Self::InvestorChain),
            "RELATED_PARTY" => Some(Self::RelatedParty),
            _ => None,
        }
    }
    
    pub fn layout_behavior(&self) -> LayoutBehavior {
        match self {
            Self::OwnershipChain => LayoutBehavior::PyramidUp,
            Self::ControlChain => LayoutBehavior::Overlay,
            Self::FundStructure => LayoutBehavior::TreeDown,
            Self::FundManagement => LayoutBehavior::Satellite,
            Self::TrustRoles => LayoutBehavior::Radial,
            Self::ServiceProvider => LayoutBehavior::FlatBottom,
            Self::TradingExecution => LayoutBehavior::FlatRight,
            Self::InvestorChain => LayoutBehavior::PyramidDown,
            Self::RelatedParty => LayoutBehavior::Peripheral,
        }
    }
}

/// Layout behavior hint for positioning
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutBehavior {
    PyramidUp,     // Ownership - UBOs at apex, working down
    PyramidDown,   // Investors - fund at top, investors below
    TreeDown,      // Fund hierarchy - parent to children
    Overlay,       // Control - adjacent to owned entity
    Satellite,     // Management - orbit around fund
    Radial,        // Trust - around central trust entity
    FlatBottom,    // Services - row at bottom
    FlatRight,     // Trading - column at right
    Peripheral,    // Related - outer edges
}
```

Update `GraphNode` struct:

```rust
pub struct GraphNode {
    // ... existing fields ...
    
    /// Primary role category (from taxonomy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role_category: Option<String>,
    
    /// Layout behavior hint (computed from role_category)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_behavior: Option<String>,
    
    /// UBO treatment code (TERMINUS, LOOK_THROUGH, CONTROL_PRONG, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ubo_treatment: Option<String>,
    
    /// KYC obligation level (FULL_KYC, SIMPLIFIED, SCREEN_ONLY, RECORD_ONLY)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyc_obligation: Option<String>,
}
```

### Task 2: Update `rust/src/graph/layout.rs`

Replace the tier-based layout with role-category-based layout:

```rust
/// View mode for role-based layout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    KycUbo,           // Full view with ownership pyramid
    UboOnly,          // Pure ownership/control pyramid
    FundStructure,    // Fund hierarchy tree
    ServiceDelivery,  // Services with trading entities
    ProductsOnly,     // Simple product view
}

impl LayoutEngine {
    /// Main layout function - role-category aware
    pub fn layout(&self, graph: &mut CbuGraph) {
        // Group entities by their primary role category
        let groups = self.group_by_role_category(graph);
        
        // Layout ownership pyramid (top half)
        self.layout_ownership_pyramid(graph, &groups.ownership);
        
        // Layout control overlay (adjacent to owned entities)
        self.layout_control_overlay(graph, &groups.control, &groups.ownership);
        
        // Layout fund structure (tree below ownership)
        self.layout_fund_tree(graph, &groups.fund_structure);
        
        // Layout fund management (satellite around funds)
        self.layout_fund_management_satellite(graph, &groups.fund_management);
        
        // Layout trust roles (radial around trust entities)
        self.layout_trust_radial(graph, &groups.trust);
        
        // Layout service providers (flat bottom)
        self.layout_services_flat(graph, &groups.services);
        
        // Layout trading/execution (flat right)
        self.layout_trading_flat(graph, &groups.trading);
        
        // Layout investor chain (pyramid below fund)
        self.layout_investor_pyramid(graph, &groups.investors);
        
        // Layout related parties (peripheral)
        self.layout_peripheral(graph, &groups.related);
    }
    
    fn group_by_role_category(&self, graph: &CbuGraph) -> RoleCategoryGroups {
        let mut groups = RoleCategoryGroups::default();
        
        for (idx, node) in graph.nodes.iter().enumerate() {
            if node.node_type != NodeType::Entity {
                continue; // Only layout entities by role
            }
            
            // Get primary role category
            let category = node.primary_role_category
                .as_ref()
                .and_then(|c| RoleCategory::from_str(c));
            
            match category {
                Some(RoleCategory::OwnershipChain) => groups.ownership.push(idx),
                Some(RoleCategory::ControlChain) => groups.control.push(idx),
                Some(RoleCategory::FundStructure) => groups.fund_structure.push(idx),
                Some(RoleCategory::FundManagement) => groups.fund_management.push(idx),
                Some(RoleCategory::TrustRoles) => groups.trust.push(idx),
                Some(RoleCategory::ServiceProvider) => groups.services.push(idx),
                Some(RoleCategory::TradingExecution) => groups.trading.push(idx),
                Some(RoleCategory::InvestorChain) => groups.investors.push(idx),
                Some(RoleCategory::RelatedParty) => groups.related.push(idx),
                None => groups.uncategorized.push(idx),
            }
        }
        
        groups
    }
    
    /// Layout ownership chain as upward pyramid
    /// Uses ownership edges to determine layering
    fn layout_ownership_pyramid(
        &self,
        graph: &mut CbuGraph,
        ownership_indices: &[usize],
    ) {
        // Build ownership DAG from edges
        let ownership_edges: Vec<_> = graph.edges.iter()
            .filter(|e| e.edge_type == EdgeType::Owns)
            .collect();
        
        // Find root (commercial client - entity being owned)
        // Walk ownership chain upward to find UBOs at apex
        // Position: root at bottom center, UBOs at top
        
        // Layer 0: Commercial client (bottom)
        // Layer 1: Direct owners
        // Layer 2: Owners of owners
        // Layer N: UBOs (natural persons) at apex
        
        // Within each layer:
        // - Sort by ownership percentage (higher = more central)
        // - PERSON entities to right, SHELL entities to left
        
        // Implementation: BFS from root, tracking depth
        let mut layers: Vec<Vec<usize>> = Vec::new();
        // ... BFS implementation ...
        
        // Position nodes
        for (layer_num, layer_indices) in layers.iter().enumerate() {
            let y = self.config.canvas_height 
                - (layer_num as f32 * self.config.tier_spacing_y)
                - self.config.bottom_margin;
            
            self.layout_layer_centered(graph, layer_indices, y);
        }
    }
    
    /// Layout control chain as overlay on ownership
    fn layout_control_overlay(
        &self,
        graph: &mut CbuGraph,
        control_indices: &[usize],
        ownership_indices: &[usize],
    ) {
        // For each control entity, find the entity it controls
        // Position slightly offset from the controlled entity
        
        let control_edges: Vec<_> = graph.edges.iter()
            .filter(|e| e.edge_type == EdgeType::Controls)
            .collect();
        
        for &idx in control_indices {
            let node_id = &graph.nodes[idx].id;
            
            // Find the entity this one controls
            if let Some(edge) = control_edges.iter().find(|e| &e.source == node_id) {
                // Find position of controlled entity
                if let Some(controlled_idx) = graph.nodes.iter()
                    .position(|n| n.id == edge.target)
                {
                    let controlled = &graph.nodes[controlled_idx];
                    if let (Some(cx), Some(cy)) = (controlled.x, controlled.y) {
                        // Position offset (above-right of controlled entity)
                        graph.nodes[idx].x = Some(cx + 50.0);
                        graph.nodes[idx].y = Some(cy - 30.0);
                    }
                }
            }
        }
    }
    
    /// Layout service providers as flat row at bottom
    fn layout_services_flat(
        &self,
        graph: &mut CbuGraph,
        service_indices: &[usize],
    ) {
        if service_indices.is_empty() {
            return;
        }
        
        let y = self.config.canvas_height - self.config.flat_zone_height;
        let spacing = self.config.node_spacing_x;
        let total_width = service_indices.len() as f32 * spacing;
        let start_x = (self.config.canvas_width - total_width) / 2.0;
        
        for (i, &idx) in service_indices.iter().enumerate() {
            graph.nodes[idx].x = Some(start_x + i as f32 * spacing);
            graph.nodes[idx].y = Some(y);
        }
    }
    
    /// Layout trading/execution as flat column at right
    fn layout_trading_flat(
        &self,
        graph: &mut CbuGraph,
        trading_indices: &[usize],
    ) {
        if trading_indices.is_empty() {
            return;
        }
        
        let x = self.config.canvas_width - self.config.flat_zone_width;
        let spacing = self.config.tier_spacing_y * 0.7; // Tighter vertical
        let total_height = trading_indices.len() as f32 * spacing;
        let start_y = (self.config.canvas_height - total_height) / 2.0;
        
        for (i, &idx) in trading_indices.iter().enumerate() {
            graph.nodes[idx].x = Some(x);
            graph.nodes[idx].y = Some(start_y + i as f32 * spacing);
        }
    }
    
    /// Layout trust roles radially around trust entity
    fn layout_trust_radial(
        &self,
        graph: &mut CbuGraph,
        trust_indices: &[usize],
    ) {
        // Find trust entities (TRUST entity type)
        let trust_entities: Vec<_> = graph.nodes.iter()
            .enumerate()
            .filter(|(_, n)| n.data.get("entity_type")
                .and_then(|v| v.as_str())
                .map(|t| t.contains("TRUST"))
                .unwrap_or(false))
            .collect();
        
        // For each trust, find its related entities and position radially
        for (trust_idx, trust_node) in trust_entities {
            let trust_id = &trust_node.id;
            let (cx, cy) = (
                trust_node.x.unwrap_or(self.config.canvas_width / 2.0),
                trust_node.y.unwrap_or(self.config.canvas_height / 2.0),
            );
            
            // Find entities with trust roles pointing to this trust
            let related: Vec<_> = trust_indices.iter()
                .filter(|&&idx| {
                    graph.edges.iter().any(|e| 
                        &e.source == &graph.nodes[idx].id 
                        && &e.target == trust_id
                        && matches!(e.edge_type, 
                            EdgeType::TrustSettlor 
                            | EdgeType::TrustTrustee 
                            | EdgeType::TrustBeneficiary 
                            | EdgeType::TrustProtector))
                })
                .cloned()
                .collect();
            
            // Position radially
            let radius = self.config.orbit_radius;
            let angle_step = 2.0 * std::f32::consts::PI / related.len().max(1) as f32;
            
            for (i, idx) in related.iter().enumerate() {
                let angle = i as f32 * angle_step - std::f32::consts::PI / 2.0; // Start at top
                graph.nodes[*idx].x = Some(cx + radius * angle.cos());
                graph.nodes[*idx].y = Some(cy + radius * angle.sin());
            }
        }
    }
}

#[derive(Default)]
struct RoleCategoryGroups {
    ownership: Vec<usize>,
    control: Vec<usize>,
    fund_structure: Vec<usize>,
    fund_management: Vec<usize>,
    trust: Vec<usize>,
    services: Vec<usize>,
    trading: Vec<usize>,
    investors: Vec<usize>,
    related: Vec<usize>,
    uncategorized: Vec<usize>,
}
```

### Task 3: Update `rust/src/database/visualization_repository.rs`

Add query for role taxonomy:

```rust
/// Extended entity view with role taxonomy
#[derive(Debug, Clone)]
pub struct GraphEntityViewV2 {
    pub cbu_entity_role_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,
    pub entity_category: Option<String>,
    pub role_name: String,
    pub jurisdiction: Option<String>,
    pub roles: Vec<String>,
    pub role_categories: Vec<String>,
    pub primary_role: Option<String>,
    pub primary_role_category: Option<String>,
    pub primary_layout: Option<String>,
    pub role_priority: Option<i32>,
    pub ubo_treatment: Option<String>,
    pub kyc_obligation: Option<String>,
}

impl VisualizationRepository {
    /// Get entities with full role taxonomy metadata
    pub async fn get_graph_entities_v2(&self, cbu_id: Uuid) -> Result<Vec<GraphEntityViewV2>> {
        let rows = sqlx::query!(
            r#"SELECT
                entity_id as "entity_id!",
                entity_name as "entity_name!",
                entity_type as "entity_type!",
                entity_category,
                jurisdiction,
                roles,
                role_categories,
                primary_role,
                (SELECT role_category FROM "ob-poc".roles WHERE name = primary_role) as primary_role_category,
                (SELECT layout_category FROM "ob-poc".roles WHERE name = primary_role) as primary_layout,
                max_role_priority as role_priority,
                effective_ubo_treatment as ubo_treatment,
                effective_kyc_obligation as kyc_obligation
               FROM "ob-poc".v_cbu_entity_with_roles
               WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| GraphEntityViewV2 {
            cbu_entity_role_id: r.entity_id,
            entity_id: r.entity_id,
            entity_name: r.entity_name,
            entity_type: r.entity_type,
            entity_category: r.entity_category,
            role_name: r.primary_role.clone().unwrap_or_default(),
            jurisdiction: r.jurisdiction,
            roles: r.roles.unwrap_or_default(),
            role_categories: r.role_categories.unwrap_or_default(),
            primary_role: r.primary_role,
            primary_role_category: r.primary_role_category,
            primary_layout: r.primary_layout,
            role_priority: r.role_priority,
            ubo_treatment: r.ubo_treatment,
            kyc_obligation: r.kyc_obligation,
        }).collect())
    }
    
    /// Get role taxonomy for reference data
    pub async fn get_role_taxonomy(&self) -> Result<Vec<RoleTaxonomyView>> {
        sqlx::query_as!(
            RoleTaxonomyView,
            r#"SELECT
                role_id,
                name as role_code,
                description,
                role_category,
                layout_category,
                ubo_treatment,
                display_priority,
                requires_percentage,
                natural_person_only,
                legal_entity_only,
                kyc_obligation
               FROM "ob-poc".v_role_taxonomy
               WHERE is_active = true
               ORDER BY role_category, sort_order"#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }
    
    /// Validate a role assignment before inserting
    pub async fn validate_role_assignment(
        &self,
        entity_id: Uuid,
        role_name: &str,
        cbu_id: Uuid,
    ) -> Result<RoleValidationResult> {
        let row = sqlx::query!(
            r#"SELECT is_valid, error_code, error_message
               FROM "ob-poc".validate_role_assignment($1, $2, $3)"#,
            entity_id,
            role_name,
            cbu_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(RoleValidationResult {
            is_valid: row.is_valid.unwrap_or(false),
            error_code: row.error_code,
            error_message: row.error_message,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RoleTaxonomyView {
    pub role_id: Uuid,
    pub role_code: String,
    pub description: Option<String>,
    pub role_category: Option<String>,
    pub layout_category: Option<String>,
    pub ubo_treatment: Option<String>,
    pub display_priority: Option<i32>,
    pub requires_percentage: Option<bool>,
    pub natural_person_only: Option<bool>,
    pub legal_entity_only: Option<bool>,
    pub kyc_obligation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RoleValidationResult {
    pub is_valid: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
```

### Task 4: Update `rust/src/graph/builder.rs`

Update the graph builder to use new role taxonomy:

```rust
impl GraphBuilder {
    /// Build graph nodes for entities with role taxonomy
    async fn build_entity_nodes(&mut self, cbu_id: Uuid) -> Result<()> {
        // Use new V2 query that includes taxonomy metadata
        let entities = self.repo.get_graph_entities_v2(cbu_id).await?;
        
        for entity in entities {
            let node = GraphNode {
                id: format!("entity_{}", entity.entity_id),
                node_type: NodeType::Entity,
                layer: LayerType::Ubo,
                label: entity.entity_name.clone(),
                sublabel: entity.jurisdiction.clone(),
                status: NodeStatus::Active,
                data: serde_json::json!({
                    "entity_id": entity.entity_id,
                    "entity_type": entity.entity_type,
                }),
                roles: entity.roles,
                role_categories: entity.role_categories,
                primary_role: entity.primary_role,
                primary_role_category: entity.primary_role_category,
                role_priority: entity.role_priority,
                entity_category: entity.entity_category,
                jurisdiction: entity.jurisdiction,
                
                // New taxonomy fields
                layout_behavior: entity.primary_layout,
                ubo_treatment: entity.ubo_treatment,
                kyc_obligation: entity.kyc_obligation,
                
                ..Default::default()
            };
            
            self.graph.add_node(node);
        }
        
        Ok(())
    }
}
```

### Task 5: Implement Custom Verb Handlers

Create `rust/src/dsl_v2/custom_ops/cbu_role_ops.rs`:

```rust
//! Custom operation handlers for CBU role verbs
//!
//! These handlers implement the validation and complex logic
//! for the cbu-role-v2.yaml verb definitions.

use crate::database::VisualizationRepository;
use anyhow::Result;
use uuid::Uuid;

/// Handler for cbu.role.assign-ownership verb
pub async fn handle_assign_ownership(
    repo: &VisualizationRepository,
    cbu_id: Uuid,
    owner_entity_id: Uuid,
    owned_entity_id: Uuid,
    percentage: f64,
    ownership_type: &str,
    role: &str,
) -> Result<(Uuid, Uuid)> {
    // 1. Validate the role assignment
    let validation = repo.validate_role_assignment(owner_entity_id, role, cbu_id).await?;
    if !validation.is_valid {
        anyhow::bail!(
            "Role assignment validation failed: {} - {}",
            validation.error_code.unwrap_or_default(),
            validation.error_message.unwrap_or_default()
        );
    }
    
    // 2. Create the ownership relationship edge
    let relationship_id = repo.create_entity_relationship(
        owner_entity_id,
        owned_entity_id,
        "ownership",
        Some(percentage),
        Some(ownership_type),
        None, // control_type
        None, // trust_role
    ).await?;
    
    // 3. Create the CBU relationship verification record
    repo.create_cbu_relationship_verification(
        cbu_id,
        relationship_id,
        "unverified",
        Some(percentage), // alleged_percentage
    ).await?;
    
    // 4. Assign the role to the owner entity
    let role_id = repo.assign_role_to_entity(cbu_id, owner_entity_id, role).await?;
    
    Ok((role_id, relationship_id))
}

/// Handler for cbu.role.assign-trust-role verb
pub async fn handle_assign_trust_role(
    repo: &VisualizationRepository,
    cbu_id: Uuid,
    trust_entity_id: Uuid,
    participant_entity_id: Uuid,
    role: &str,
    interest_percentage: Option<f64>,
    interest_type: Option<&str>,
) -> Result<(Uuid, Uuid)> {
    // Validate
    let validation = repo.validate_role_assignment(participant_entity_id, role, cbu_id).await?;
    if !validation.is_valid {
        anyhow::bail!("Trust role validation failed: {}", validation.error_message.unwrap_or_default());
    }
    
    // Map role to trust_role value
    let trust_role = match role {
        "SETTLOR" => "settlor",
        "TRUSTEE" => "trustee",
        "PROTECTOR" => "protector",
        "BENEFICIARY_FIXED" | "BENEFICIARY_DISCRETIONARY" | "BENEFICIARY_CONTINGENT" => "beneficiary",
        "ENFORCER" => "enforcer",
        "APPOINTOR" => "appointor",
        _ => role.to_lowercase().as_str(),
    };
    
    // Create trust relationship
    let relationship_id = repo.create_entity_relationship(
        participant_entity_id,
        trust_entity_id,
        "trust_role",
        interest_percentage,
        None,
        None,
        Some(trust_role),
    ).await?;
    
    // Create verification record
    repo.create_cbu_relationship_verification(cbu_id, relationship_id, "unverified", None).await?;
    
    // Assign role
    let role_id = repo.assign_role_to_entity(cbu_id, participant_entity_id, role).await?;
    
    Ok((role_id, relationship_id))
}
```

## Visualization Layout Diagram

```
┌────────────────────────────────────────────────────────────────────────────┐
│                           CANVAS (1200 x 900)                              │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│     ┌──────────┐                                  ┌──────────────────────┐ │
│     │   UBO    │  ← OWNERSHIP_CHAIN apex          │  TRADING_EXECUTION   │ │
│     │ (Person) │                                  │  ┌────────────────┐  │ │
│     └────┬─────┘                                  │  │ Auth Signatory │  │ │
│          │ owns 60%                               │  ├────────────────┤  │ │
│     ┌────┴─────┐                                  │  │ Auth Trader    │  │ │
│     │ HoldCo A │                                  │  ├────────────────┤  │ │
│     └────┬─────┘                                  │  │ Settlement     │  │ │
│          │ owns 100%                              │  └────────────────┘  │ │
│     ┌────┴───────────┐        ┌──────────┐       └──────────────────────┘ │
│     │ COMMERCIAL     │◄───────│ Director │  ← CONTROL_CHAIN overlay       │
│     │ CLIENT         │        └──────────┘                                │
│     │ (Operating Co) │                                                    │
│     └────────────────┘                                                    │
│            │                                                              │
│            │ managed_by                                                   │
│     ┌──────┴──────┐                                                       │
│     │   ManCo     │  ← FUND_MANAGEMENT satellite                         │
│     └──────┬──────┘                                                       │
│            │                                                              │
│     ┌──────┴──────────────────────────────┐                              │
│     │         FUND (SICAV)                │  ← FUND_STRUCTURE tree       │
│     │  ┌─────────┬──────────┬──────────┐  │                              │
│     │  │SubFund A│ SubFund B│ SubFund C│  │                              │
│     │  └─────────┴──────────┴──────────┘  │                              │
│     └─────────────────────────────────────┘                              │
│                      │                                                    │
│                      │ investors                                          │
│              ┌───────┴───────┐  ← INVESTOR_CHAIN pyramid down            │
│              │  Pension Fund │                                            │
│              └───────────────┘                                            │
│                                                                           │
├────────────────────────────────────────────────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  ← SERVICE_PROVIDER  │
│ │Depositary│ │ Custodian│ │  Admin   │ │  Auditor │     flat bottom      │
│ └──────────┘ └──────────┘ └──────────┘ └──────────┘                      │
└────────────────────────────────────────────────────────────────────────────┘
```

## Testing Checklist

1. [ ] Run migration on dev database
2. [ ] Verify role taxonomy view returns all 60+ roles
3. [ ] Test role validation function with valid/invalid combinations
4. [ ] Create test CBU with:
   - [ ] Ownership chain (3 levels to UBO)
   - [ ] Control overlay (director on operating company)
   - [ ] Fund structure (umbrella + 2 subfunds)
   - [ ] Service providers (depositary, custodian, admin)
   - [ ] Trading execution (2 signatories)
5. [ ] Verify graph layout positions entities correctly by category
6. [ ] Test role incompatibility rules (GP + LP on same entity should fail)
7. [ ] Test role requirement rules (subfund without umbrella should warn)

## Migration Steps

1. Apply SQL migration: `psql -f rust/migrations/202501_role_taxonomy_v2.sql`
2. Update Rust code per tasks above
3. Run `cargo build` and fix any compilation errors
4. Run test suite: `cargo test`
5. Test with real data via API or DSL REPL
