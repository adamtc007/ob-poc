# CBU Hierarchical Visualization - Implementation Plan v2

**Task**: Implement hierarchical visualization with two views  
**Created**: 2025-12-01  
**Status**: READY FOR IMPLEMENTATION  
**Priority**: High

---

## Overview

Replace radial layout with hierarchical tree layout. Two distinct views, not layers of one view.

---

## View 1: Service Delivery Map

**Purpose**: What does BNY provide to this client?

```
                              ┌─────────────────┐
                              │      CBU        │
                              │ Apex Capital    │
                              └────────┬────────┘
                                       │
              ┌────────────────────────┼────────────────────────┐
              │                        │                        │
              ▼                        ▼                        ▼
      ┌───────────────┐       ┌───────────────┐       ┌───────────────┐
      │   Product     │       │   Product     │       │   Product     │
      │ Global Custody│       │ Fund Admin    │       │ Prime Broker  │
      └───────┬───────┘       └───────┬───────┘       └───────────────┘
              │                       │
      ┌───────┴───────┐               │
      │               │               │
      ▼               ▼               ▼
┌───────────┐  ┌───────────┐   ┌───────────┐
│  Service  │  │  Service  │   │  Service  │
│ Safekeep  │  │ Settlement│   │ NAV Calc  │
└─────┬─────┘  └─────┬─────┘   └───────────┘
      │              │
      ▼              ▼
┌───────────┐  ┌───────────┐
│ Resource  │  │ Resource  │
│Account-001│  │ SSI-US-EQ │
└───────────┘  └─────┬─────┘
                     │
              ┌──────┴──────┐
              │             │
              ▼             ▼
        ┌──────────┐ ┌──────────┐
        │ Booking  │ │ Booking  │
        │ Rule-1   │ │ Rule-2   │
        └──────────┘ └──────────┘
```

**Hierarchy**: CBU → Product → Service → Resource → (SSI/BookingRule)

**Layout**: Top-down tree with horizontal spreading at each level

---

## View 2: KYC/UBO Structure

**Purpose**: Who is this client? Who owns/controls it?

```
                     ┌─────────────────────┐
                     │  Commercial Client  │
                     │  Apex Holdings LLC  │
                     │      (US-DE)        │
                     └──────────┬──────────┘
                                │
                           OWNS 100%
                                │
                                ▼
                     ┌─────────────────────┐
                     │       ManCo         │
                     │ Apex Management Ltd │
                     │        (KY)         │
                     └──────────┬──────────┘
                                │
                           MANAGES
                                │
              ┌─────────────────┼─────────────────┐
              │                 │                 │
              ▼                 ▼                 ▼
      ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
      │   Fund LP    │  │   Officers   │  │   Officers   │
      │ Apex Partners│  │ Marcus Chen  │  │Sarah Williams│
      │     (KY)     │  │  DIRECTOR    │  │  DIRECTOR    │
      └──────┬───────┘  │    UBO       │  └──────────────┘
             │          └──────┬───────┘
             │                 │
         ISSUES           OWNS 60%
             │            Class A
             │                 │
      ┌──────┴──────┐         │
      │             │         │
      ▼             ▼         │
┌───────────┐ ┌───────────┐   │
│ Class A   │ │ Class B   │◄──┘
│ Founding  │ │ Institut. │
│   USD     │ │   USD     │
└───────────┘ └───────────┘
```

**Hierarchy**: Commercial Client → ManCo → Fund Entity → Share Classes  
**Plus**: Officers attached to entities, ownership arrows

---

## Trust Variation (No Share Classes at Top)

```
                     ┌─────────────────────┐
                     │      Trustee        │
                     │Jersey Trust Co Ltd  │
                     │        (JE)         │
                     └──────────┬──────────┘
                                │
                           CONTROLS
                           (TRUSTEE)
                                │
                                ▼
                     ┌─────────────────────┐
                     │       Trust         │
                     │Wellington Family    │──────────┬──────────┐
                     │        (JE)         │          │          │
                     └──────────┬──────────┘          │          │
                                │                     │          │
                                │                     ▼          ▼
                                │              ┌───────────┐┌───────────┐
                           SETTLOR             │Beneficiary││Beneficiary│
                                │              │   Emma    ││   James   │
                                ▼              └───────────┘└───────────┘
                     ┌─────────────────────┐
                     │      Settlor        │
                     │  Robert Wellington  │
                     │        (GB)         │
                     └─────────────────────┘
```

---

## Implementation

### Step 1: Data Structures

Create `rust/src/visualization/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

/// View mode selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    ServiceDelivery,
    KycUbo,
}

/// Node in the visualization tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: Uuid,
    pub node_type: TreeNodeType,
    pub label: String,
    pub sublabel: Option<String>,
    pub jurisdiction: Option<String>,
    pub children: Vec<TreeNode>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Types of nodes in the tree
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeNodeType {
    // KYC/UBO View
    Cbu,
    CommercialClient,
    ManCo,
    FundEntity,
    TrustEntity,
    Person,
    ShareClass,
    
    // Service Delivery View
    Product,
    Service,
    Resource,
    Ssi,
    BookingRule,
}

/// Edge connecting nodes (for non-hierarchical relationships like ownership)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEdge {
    pub from: Uuid,
    pub to: Uuid,
    pub edge_type: TreeEdgeType,
    pub label: Option<String>,
    pub weight: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeEdgeType {
    // Structural (part of tree)
    ChildOf,
    
    // Overlay (drawn on top of tree)
    Owns,
    Controls,
    Role,
}

/// Complete visualization data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuVisualization {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub view_mode: ViewMode,
    pub root: TreeNode,
    pub overlay_edges: Vec<TreeEdge>,  // Non-hierarchical edges drawn on top
    pub stats: VisualizationStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub max_depth: usize,
    pub entity_count: usize,
    pub person_count: usize,
    pub share_class_count: usize,
}
```

---

### Step 2: KYC/UBO Tree Builder

Create `rust/src/visualization/kyc_builder.rs`:

```rust
use super::types::*;
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;

pub struct KycTreeBuilder {
    pool: PgPool,
}

impl KycTreeBuilder {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    pub async fn build(&self, cbu_id: Uuid) -> Result<CbuVisualization> {
        // 1. Load CBU
        let cbu = self.load_cbu(cbu_id).await?;
        
        // 2. Determine structure type (fund vs trust vs corporate)
        let client_type = cbu.client_type.as_deref().unwrap_or("UNKNOWN");
        
        // 3. Build tree based on type
        let (root, overlay_edges) = match client_type {
            "HEDGE_FUND" | "40_ACT" | "UCITS" | "PE_FUND" | "VC_FUND" => {
                self.build_fund_tree(cbu_id, &cbu).await?
            }
            "TRUST" => {
                self.build_trust_tree(cbu_id, &cbu).await?
            }
            "CORPORATE" | _ => {
                self.build_corporate_tree(cbu_id, &cbu).await?
            }
        };
        
        let stats = self.calculate_stats(&root, &overlay_edges);
        
        Ok(CbuVisualization {
            cbu_id,
            cbu_name: cbu.name,
            view_mode: ViewMode::KycUbo,
            root,
            overlay_edges,
            stats,
        })
    }
    
    async fn build_fund_tree(&self, cbu_id: Uuid, cbu: &CbuRecord) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();
        
        // Start with commercial client at top
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.load_entity(cc_id).await?;
            Some(TreeNode {
                id: cc_id,
                node_type: TreeNodeType::CommercialClient,
                label: entity.name,
                sublabel: Some("Commercial Client".to_string()),
                jurisdiction: entity.jurisdiction,
                children: vec![],
                metadata: Default::default(),
            })
        } else {
            None
        };
        
        // Load ManCo (entity with PRINCIPAL or MANCO role)
        let manco = self.load_manco(cbu_id).await?;
        
        // Load fund entities (entities that issue FUND share classes)
        let fund_entities = self.load_fund_entities(cbu_id).await?;
        
        // Load share classes per fund entity
        let mut fund_nodes = Vec::new();
        for fund in fund_entities {
            let share_classes = self.load_share_classes_for_entity(fund.id).await?;
            let share_class_nodes: Vec<TreeNode> = share_classes.into_iter().map(|sc| {
                TreeNode {
                    id: sc.id,
                    node_type: TreeNodeType::ShareClass,
                    label: sc.name,
                    sublabel: Some(format!("{} - {}", sc.currency, sc.class_category)),
                    jurisdiction: None,
                    children: vec![],
                    metadata: [
                        ("isin".to_string(), serde_json::json!(sc.isin)),
                        ("nav".to_string(), serde_json::json!(sc.nav_per_share)),
                    ].into_iter().collect(),
                }
            }).collect();
            
            fund_nodes.push(TreeNode {
                id: fund.id,
                node_type: TreeNodeType::FundEntity,
                label: fund.name,
                sublabel: Some("Fund".to_string()),
                jurisdiction: fund.jurisdiction,
                children: share_class_nodes,
                metadata: Default::default(),
            });
        }
        
        // Load officers (persons with roles)
        let officers = self.load_officers(cbu_id).await?;
        let officer_nodes: Vec<TreeNode> = officers.iter().map(|o| {
            TreeNode {
                id: o.entity_id,
                node_type: TreeNodeType::Person,
                label: o.name.clone(),
                sublabel: Some(o.roles.join(", ")),
                jurisdiction: o.nationality.clone(),
                children: vec![],
                metadata: Default::default(),
            }
        }).collect();
        
        // Build ownership overlay edges
        let holdings = self.load_all_holdings(cbu_id).await?;
        for holding in holdings {
            overlay_edges.push(TreeEdge {
                from: holding.investor_entity_id,
                to: holding.share_class_id,
                edge_type: TreeEdgeType::Owns,
                label: Some(format!("{:.0}%", holding.percentage)),
                weight: Some(holding.percentage as f32),
            });
        }
        
        // Assemble tree
        let manco_node = if let Some(m) = manco {
            let mut children = fund_nodes;
            children.extend(officer_nodes);
            
            Some(TreeNode {
                id: m.id,
                node_type: TreeNodeType::ManCo,
                label: m.name,
                sublabel: Some("Management Company".to_string()),
                jurisdiction: m.jurisdiction,
                children,
                metadata: Default::default(),
            })
        } else {
            None
        };
        
        let root = if let Some(mut cc) = commercial_client {
            if let Some(manco) = manco_node {
                cc.children.push(manco);
            }
            cc
        } else if let Some(manco) = manco_node {
            manco
        } else {
            // Fallback: CBU as root
            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: Some(client_type.to_string()),
                jurisdiction: Some(cbu.jurisdiction.clone()),
                children: fund_nodes,
                metadata: Default::default(),
            }
        };
        
        Ok((root, overlay_edges))
    }
    
    async fn build_trust_tree(&self, cbu_id: Uuid, cbu: &CbuRecord) -> Result<(TreeNode, Vec<TreeEdge>)> {
        let mut overlay_edges = Vec::new();
        
        // Load trustee
        let trustee = self.load_entity_by_role(cbu_id, "TRUSTEE").await?;
        
        // Load trust entity (PRINCIPAL)
        let trust = self.load_entity_by_role(cbu_id, "PRINCIPAL").await?;
        
        // Load settlor
        let settlor = self.load_entity_by_role(cbu_id, "SETTLOR").await?;
        
        // Load beneficiaries
        let beneficiaries = self.load_entities_by_role(cbu_id, "BENEFICIARY").await?;
        
        let beneficiary_nodes: Vec<TreeNode> = beneficiaries.into_iter().map(|b| {
            TreeNode {
                id: b.id,
                node_type: TreeNodeType::Person,
                label: b.name,
                sublabel: Some("Beneficiary".to_string()),
                jurisdiction: b.jurisdiction,
                children: vec![],
                metadata: Default::default(),
            }
        }).collect();
        
        let settlor_node = settlor.map(|s| TreeNode {
            id: s.id,
            node_type: TreeNodeType::Person,
            label: s.name,
            sublabel: Some("Settlor".to_string()),
            jurisdiction: s.jurisdiction,
            children: vec![],
            metadata: Default::default(),
        });
        
        let mut trust_children = beneficiary_nodes;
        if let Some(s) = settlor_node {
            trust_children.push(s);
        }
        
        let trust_node = trust.map(|t| TreeNode {
            id: t.id,
            node_type: TreeNodeType::TrustEntity,
            label: t.name,
            sublabel: Some("Trust".to_string()),
            jurisdiction: t.jurisdiction,
            children: trust_children,
            metadata: Default::default(),
        });
        
        // Load control relationships for overlay
        let controls = self.load_control_relationships(cbu_id).await?;
        for ctrl in controls {
            overlay_edges.push(TreeEdge {
                from: ctrl.controller_entity_id,
                to: ctrl.controlled_entity_id,
                edge_type: TreeEdgeType::Controls,
                label: Some(ctrl.control_type),
                weight: None,
            });
        }
        
        let root = if let Some(t) = trustee {
            let mut children = Vec::new();
            if let Some(trust) = trust_node {
                children.push(trust);
            }
            TreeNode {
                id: t.id,
                node_type: TreeNodeType::ManCo, // Trustee acts like ManCo
                label: t.name,
                sublabel: Some("Trustee".to_string()),
                jurisdiction: t.jurisdiction,
                children,
                metadata: Default::default(),
            }
        } else if let Some(trust) = trust_node {
            trust
        } else {
            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: Some("Trust".to_string()),
                jurisdiction: Some(cbu.jurisdiction.clone()),
                children: vec![],
                metadata: Default::default(),
            }
        };
        
        Ok((root, overlay_edges))
    }
    
    async fn build_corporate_tree(&self, cbu_id: Uuid, cbu: &CbuRecord) -> Result<(TreeNode, Vec<TreeEdge>)> {
        // Similar to fund tree but simpler - no ManCo, just parent -> subsidiary
        let mut overlay_edges = Vec::new();
        
        let commercial_client = if let Some(cc_id) = cbu.commercial_client_entity_id {
            let entity = self.load_entity(cc_id).await?;
            Some(TreeNode {
                id: cc_id,
                node_type: TreeNodeType::CommercialClient,
                label: entity.name,
                sublabel: Some("Parent Company".to_string()),
                jurisdiction: entity.jurisdiction,
                children: vec![],
                metadata: Default::default(),
            })
        } else {
            None
        };
        
        let principal = self.load_entity_by_role(cbu_id, "PRINCIPAL").await?;
        let officers = self.load_officers(cbu_id).await?;
        
        let officer_nodes: Vec<TreeNode> = officers.iter().map(|o| {
            TreeNode {
                id: o.entity_id,
                node_type: TreeNodeType::Person,
                label: o.name.clone(),
                sublabel: Some(o.roles.join(", ")),
                jurisdiction: o.nationality.clone(),
                children: vec![],
                metadata: Default::default(),
            }
        }).collect();
        
        // Load corporate share classes
        let share_classes = if let Some(ref p) = principal {
            self.load_share_classes_for_entity(p.id).await?
        } else {
            vec![]
        };
        
        let share_class_nodes: Vec<TreeNode> = share_classes.into_iter().map(|sc| {
            TreeNode {
                id: sc.id,
                node_type: TreeNodeType::ShareClass,
                label: sc.name,
                sublabel: Some(format!("{} - {}", sc.currency, sc.class_category)),
                jurisdiction: None,
                children: vec![],
                metadata: Default::default(),
            }
        }).collect();
        
        // Holdings as overlay
        let holdings = self.load_all_holdings(cbu_id).await?;
        for holding in holdings {
            overlay_edges.push(TreeEdge {
                from: holding.investor_entity_id,
                to: holding.share_class_id,
                edge_type: TreeEdgeType::Owns,
                label: Some(format!("{:.0}%", holding.percentage)),
                weight: Some(holding.percentage as f32),
            });
        }
        
        let principal_node = principal.map(|p| {
            let mut children = share_class_nodes;
            children.extend(officer_nodes);
            TreeNode {
                id: p.id,
                node_type: TreeNodeType::FundEntity, // Generic entity
                label: p.name,
                sublabel: Some("Subsidiary".to_string()),
                jurisdiction: p.jurisdiction,
                children,
                metadata: Default::default(),
            }
        });
        
        let root = if let Some(mut cc) = commercial_client {
            if let Some(p) = principal_node {
                cc.children.push(p);
            }
            cc
        } else if let Some(p) = principal_node {
            p
        } else {
            TreeNode {
                id: cbu_id,
                node_type: TreeNodeType::Cbu,
                label: cbu.name.clone(),
                sublabel: Some("Corporate".to_string()),
                jurisdiction: Some(cbu.jurisdiction.clone()),
                children: vec![],
                metadata: Default::default(),
            }
        };
        
        Ok((root, overlay_edges))
    }
    
    fn calculate_stats(&self, root: &TreeNode, edges: &[TreeEdge]) -> VisualizationStats {
        fn count_nodes(node: &TreeNode, stats: &mut VisualizationStats, depth: usize) {
            stats.total_nodes += 1;
            stats.max_depth = stats.max_depth.max(depth);
            
            match node.node_type {
                TreeNodeType::Person => stats.person_count += 1,
                TreeNodeType::ShareClass => stats.share_class_count += 1,
                TreeNodeType::CommercialClient | TreeNodeType::ManCo | 
                TreeNodeType::FundEntity | TreeNodeType::TrustEntity => stats.entity_count += 1,
                _ => {}
            }
            
            for child in &node.children {
                count_nodes(child, stats, depth + 1);
            }
        }
        
        let mut stats = VisualizationStats {
            total_nodes: 0,
            total_edges: edges.len(),
            max_depth: 0,
            entity_count: 0,
            person_count: 0,
            share_class_count: 0,
        };
        
        count_nodes(root, &mut stats, 0);
        stats
    }
}
```

---

### Step 3: Tree Layout Algorithm

Create `rust/src/visualization/layout.rs`:

```rust
use super::types::*;
use std::collections::HashMap;

/// Position of a node in 2D space
#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Layout configuration
pub struct LayoutConfig {
    pub node_width: f32,
    pub node_height: f32,
    pub horizontal_gap: f32,
    pub vertical_gap: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            node_width: 180.0,
            node_height: 60.0,
            horizontal_gap: 40.0,
            vertical_gap: 80.0,
        }
    }
}

/// Computed layout for all nodes
pub struct TreeLayout {
    pub positions: HashMap<Uuid, Position>,
    pub bounds: (f32, f32, f32, f32), // min_x, min_y, max_x, max_y
}

pub fn compute_layout(root: &TreeNode, config: &LayoutConfig) -> TreeLayout {
    let mut positions = HashMap::new();
    
    // First pass: compute subtree widths
    let widths = compute_subtree_widths(root, config);
    
    // Second pass: assign positions top-down
    assign_positions(root, 0.0, 0.0, &widths, config, &mut positions);
    
    // Calculate bounds
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    
    for pos in positions.values() {
        min_x = min_x.min(pos.x);
        min_y = min_y.min(pos.y);
        max_x = max_x.max(pos.x + config.node_width);
        max_y = max_y.max(pos.y + config.node_height);
    }
    
    TreeLayout {
        positions,
        bounds: (min_x, min_y, max_x, max_y),
    }
}

fn compute_subtree_widths(node: &TreeNode, config: &LayoutConfig) -> HashMap<Uuid, f32> {
    let mut widths = HashMap::new();
    compute_width_recursive(node, config, &mut widths);
    widths
}

fn compute_width_recursive(node: &TreeNode, config: &LayoutConfig, widths: &mut HashMap<Uuid, f32>) -> f32 {
    if node.children.is_empty() {
        let width = config.node_width;
        widths.insert(node.id, width);
        width
    } else {
        let children_width: f32 = node.children.iter()
            .map(|child| compute_width_recursive(child, config, widths))
            .sum::<f32>() + (config.horizontal_gap * (node.children.len() - 1) as f32);
        
        let width = children_width.max(config.node_width);
        widths.insert(node.id, width);
        width
    }
}

fn assign_positions(
    node: &TreeNode,
    x: f32,
    y: f32,
    widths: &HashMap<Uuid, f32>,
    config: &LayoutConfig,
    positions: &mut HashMap<Uuid, Position>,
) {
    let subtree_width = widths.get(&node.id).copied().unwrap_or(config.node_width);
    
    // Center this node in its subtree width
    let node_x = x + (subtree_width - config.node_width) / 2.0;
    positions.insert(node.id, Position { x: node_x, y });
    
    // Position children
    if !node.children.is_empty() {
        let child_y = y + config.node_height + config.vertical_gap;
        let mut child_x = x;
        
        for child in &node.children {
            let child_width = widths.get(&child.id).copied().unwrap_or(config.node_width);
            assign_positions(child, child_x, child_y, widths, config, positions);
            child_x += child_width + config.horizontal_gap;
        }
    }
}
```

---

### Step 4: egui Rendering

Create `rust/src/visualization/render.rs`:

```rust
use super::types::*;
use super::layout::*;
use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use std::collections::HashMap;

pub struct TreeRenderer {
    config: LayoutConfig,
}

impl TreeRenderer {
    pub fn new() -> Self {
        Self {
            config: LayoutConfig::default(),
        }
    }
    
    pub fn render(&self, ui: &mut Ui, viz: &CbuVisualization) {
        let layout = compute_layout(&viz.root, &self.config);
        
        // Get available rect and center the tree
        let available = ui.available_rect_before_wrap();
        let (min_x, min_y, max_x, max_y) = layout.bounds;
        let tree_width = max_x - min_x;
        let tree_height = max_y - min_y;
        
        let offset_x = available.min.x + (available.width() - tree_width) / 2.0 - min_x;
        let offset_y = available.min.y + 20.0 - min_y;  // Top padding
        
        let painter = ui.painter();
        
        // Draw edges first (underneath nodes)
        self.draw_tree_edges(&viz.root, &layout.positions, offset_x, offset_y, painter);
        
        // Draw overlay edges (ownership, control)
        self.draw_overlay_edges(&viz.overlay_edges, &layout.positions, offset_x, offset_y, painter);
        
        // Draw nodes
        self.draw_tree_nodes(&viz.root, &layout.positions, offset_x, offset_y, ui);
        
        // Reserve space
        ui.allocate_space(Vec2::new(tree_width, tree_height + 40.0));
    }
    
    fn draw_tree_edges(
        &self,
        node: &TreeNode,
        positions: &HashMap<Uuid, Position>,
        offset_x: f32,
        offset_y: f32,
        painter: &egui::Painter,
    ) {
        let Some(parent_pos) = positions.get(&node.id) else { return };
        
        let parent_bottom = Pos2::new(
            parent_pos.x + offset_x + self.config.node_width / 2.0,
            parent_pos.y + offset_y + self.config.node_height,
        );
        
        for child in &node.children {
            if let Some(child_pos) = positions.get(&child.id) {
                let child_top = Pos2::new(
                    child_pos.x + offset_x + self.config.node_width / 2.0,
                    child_pos.y + offset_y,
                );
                
                // Draw elbow connector
                let mid_y = (parent_bottom.y + child_top.y) / 2.0;
                let stroke = Stroke::new(2.0, Color32::from_rgb(150, 150, 150));
                
                painter.line_segment([parent_bottom, Pos2::new(parent_bottom.x, mid_y)], stroke);
                painter.line_segment([Pos2::new(parent_bottom.x, mid_y), Pos2::new(child_top.x, mid_y)], stroke);
                painter.line_segment([Pos2::new(child_top.x, mid_y), child_top], stroke);
                
                // Recurse
                self.draw_tree_edges(child, positions, offset_x, offset_y, painter);
            }
        }
    }
    
    fn draw_overlay_edges(
        &self,
        edges: &[TreeEdge],
        positions: &HashMap<Uuid, Position>,
        offset_x: f32,
        offset_y: f32,
        painter: &egui::Painter,
    ) {
        for edge in edges {
            let Some(from_pos) = positions.get(&edge.from) else { continue };
            let Some(to_pos) = positions.get(&edge.to) else { continue };
            
            let from_center = Pos2::new(
                from_pos.x + offset_x + self.config.node_width / 2.0,
                from_pos.y + offset_y + self.config.node_height / 2.0,
            );
            let to_center = Pos2::new(
                to_pos.x + offset_x + self.config.node_width / 2.0,
                to_pos.y + offset_y + self.config.node_height / 2.0,
            );
            
            let (color, dashed) = match edge.edge_type {
                TreeEdgeType::Owns => (Color32::from_rgb(66, 133, 244), false),      // Blue
                TreeEdgeType::Controls => (Color32::from_rgb(234, 67, 53), true),    // Red dashed
                TreeEdgeType::Role => (Color32::from_rgb(100, 100, 100), true),      // Gray dashed
                TreeEdgeType::ChildOf => continue,  // Already drawn as tree edge
            };
            
            let stroke = Stroke::new(2.0, color);
            
            if dashed {
                // Draw dashed line
                draw_dashed_line(painter, from_center, to_center, stroke, 8.0, 4.0);
            } else {
                painter.line_segment([from_center, to_center], stroke);
            }
            
            // Draw label if present
            if let Some(label) = &edge.label {
                let mid = Pos2::new(
                    (from_center.x + to_center.x) / 2.0,
                    (from_center.y + to_center.y) / 2.0,
                );
                painter.text(
                    mid,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    color,
                );
            }
            
            // Draw arrow head at target
            draw_arrow_head(painter, from_center, to_center, color);
        }
    }
    
    fn draw_tree_nodes(
        &self,
        node: &TreeNode,
        positions: &HashMap<Uuid, Position>,
        offset_x: f32,
        offset_y: f32,
        ui: &mut Ui,
    ) {
        let Some(pos) = positions.get(&node.id) else { return };
        
        let rect = Rect::from_min_size(
            Pos2::new(pos.x + offset_x, pos.y + offset_y),
            Vec2::new(self.config.node_width, self.config.node_height),
        );
        
        let (bg_color, border_color, icon) = get_node_style(&node.node_type);
        
        let painter = ui.painter();
        
        // Background with rounded corners
        painter.rect_filled(rect, 8.0, bg_color);
        painter.rect_stroke(rect, 8.0, Stroke::new(2.0, border_color));
        
        // Icon
        painter.text(
            Pos2::new(rect.min.x + 12.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(16.0),
            Color32::WHITE,
        );
        
        // Label
        painter.text(
            Pos2::new(rect.min.x + 32.0, rect.min.y + 18.0),
            egui::Align2::LEFT_CENTER,
            &node.label,
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
        
        // Sublabel
        if let Some(sublabel) = &node.sublabel {
            painter.text(
                Pos2::new(rect.min.x + 32.0, rect.min.y + 38.0),
                egui::Align2::LEFT_CENTER,
                sublabel,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(200, 200, 200),
            );
        }
        
        // Jurisdiction badge
        if let Some(jurisdiction) = &node.jurisdiction {
            let badge_rect = Rect::from_min_size(
                Pos2::new(rect.max.x - 35.0, rect.min.y + 5.0),
                Vec2::new(30.0, 16.0),
            );
            painter.rect_filled(badge_rect, 4.0, Color32::from_rgb(80, 80, 80));
            painter.text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                jurisdiction,
                egui::FontId::proportional(9.0),
                Color32::from_rgb(200, 200, 200),
            );
        }
        
        // Recurse for children
        for child in &node.children {
            self.draw_tree_nodes(child, positions, offset_x, offset_y, ui);
        }
    }
}

fn get_node_style(node_type: &TreeNodeType) -> (Color32, Color32, &'static str) {
    match node_type {
        TreeNodeType::Cbu => (
            Color32::from_rgb(30, 136, 229),  // Blue
            Color32::from_rgb(25, 118, 210),
            "🏢",
        ),
        TreeNodeType::CommercialClient => (
            Color32::from_rgb(67, 160, 71),   // Green
            Color32::from_rgb(56, 142, 60),
            "🏛️",
        ),
        TreeNodeType::ManCo => (
            Color32::from_rgb(94, 53, 177),   // Purple
            Color32::from_rgb(81, 45, 168),
            "🏛️",
        ),
        TreeNodeType::FundEntity => (
            Color32::from_rgb(0, 137, 123),   // Teal
            Color32::from_rgb(0, 121, 107),
            "📊",
        ),
        TreeNodeType::TrustEntity => (
            Color32::from_rgb(121, 85, 72),   // Brown
            Color32::from_rgb(109, 76, 65),
            "🏛️",
        ),
        TreeNodeType::Person => (
            Color32::from_rgb(84, 110, 122),  // Blue-gray
            Color32::from_rgb(69, 90, 100),
            "👤",
        ),
        TreeNodeType::ShareClass => (
            Color32::from_rgb(255, 152, 0),   // Orange
            Color32::from_rgb(245, 124, 0),
            "📄",
        ),
        TreeNodeType::Product => (
            Color32::from_rgb(76, 175, 80),   // Green
            Color32::from_rgb(67, 160, 71),
            "📦",
        ),
        TreeNodeType::Service => (
            Color32::from_rgb(33, 150, 243),  // Blue
            Color32::from_rgb(30, 136, 229),
            "⚙️",
        ),
        TreeNodeType::Resource => (
            Color32::from_rgb(156, 39, 176),  // Purple
            Color32::from_rgb(142, 36, 170),
            "🔧",
        ),
        TreeNodeType::Ssi => (
            Color32::from_rgb(0, 150, 136),   // Teal
            Color32::from_rgb(0, 137, 123),
            "🏦",
        ),
        TreeNodeType::BookingRule => (
            Color32::from_rgb(121, 134, 203), // Indigo
            Color32::from_rgb(92, 107, 192),
            "📋",
        ),
    }
}

fn draw_dashed_line(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke, dash: f32, gap: f32) {
    let dir = (to - from).normalized();
    let len = (to - from).length();
    let mut dist = 0.0;
    
    while dist < len {
        let start = from + dir * dist;
        let end_dist = (dist + dash).min(len);
        let end = from + dir * end_dist;
        painter.line_segment([start, end], stroke);
        dist += dash + gap;
    }
}

fn draw_arrow_head(painter: &egui::Painter, from: Pos2, to: Pos2, color: Color32) {
    let dir = (to - from).normalized();
    let perp = Vec2::new(-dir.y, dir.x);
    
    let arrow_size = 8.0;
    let tip = to;
    let left = tip - dir * arrow_size + perp * (arrow_size / 2.0);
    let right = tip - dir * arrow_size - perp * (arrow_size / 2.0);
    
    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        color,
        Stroke::NONE,
    ));
}
```

---

### Step 5: Integration

Update `rust/src/visualization/mod.rs`:

```rust
pub mod types;
pub mod kyc_builder;
pub mod service_builder;
pub mod layout;
pub mod render;

pub use types::*;
pub use kyc_builder::KycTreeBuilder;
pub use layout::compute_layout;
pub use render::TreeRenderer;
```

Update CLI or app to use new visualization:

```rust
// In app.rs or wherever the CBU viewer is

use visualization::{KycTreeBuilder, TreeRenderer, ViewMode};

async fn show_cbu_view(ui: &mut Ui, pool: &PgPool, cbu_id: Uuid, view_mode: ViewMode) {
    match view_mode {
        ViewMode::KycUbo => {
            let builder = KycTreeBuilder::new(pool.clone());
            if let Ok(viz) = builder.build(cbu_id).await {
                let renderer = TreeRenderer::new();
                renderer.render(ui, &viz);
            }
        }
        ViewMode::ServiceDelivery => {
            // TODO: Implement ServiceDeliveryBuilder
        }
    }
}
```

---

## Verification

Test with each seeded CBU:

| CBU | Expected Structure |
|-----|-------------------|
| Apex Capital (HEDGE_FUND) | CommercialClient → ManCo → Fund LP → Share Classes, Officers |
| Pacific Growth (40_ACT) | CommercialClient → Fund → Share Classes, Officers |
| Europa Equity (UCITS) | CommercialClient → ManCo → ICAV → Share Classes |
| Wellington Trust (TRUST) | Trustee → Trust → Settlor, Beneficiaries |
| TechCorp Treasury (CORPORATE) | Parent → Subsidiary → Share Classes, Officers |

---

## Summary

| Component | File | Purpose |
|-----------|------|---------|
| Types | `visualization/types.rs` | TreeNode, TreeEdge, ViewMode |
| KYC Builder | `visualization/kyc_builder.rs` | Build tree from DB for KYC/UBO view |
| Layout | `visualization/layout.rs` | Compute node positions (top-down tree) |
| Render | `visualization/render.rs` | Draw tree in egui |

---

*End of Plan*
