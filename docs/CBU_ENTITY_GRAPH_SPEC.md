# CBU Entity Graph Visualization Specification

## Technology Decision: egui with Custom Painting

### Why egui (not raw wgpu)

| Factor | egui | wgpu direct |
|--------|------|-------------|
| **Already in stack** | ✓ You use it | New learning |
| **Custom 2D drawing** | `egui::Painter` API | Build from scratch |
| **UI controls** | Built-in panels, buttons | Build from scratch |
| **WASM support** | ✓ eframe | ✓ but more setup |
| **Performance ceiling** | ~5K nodes smooth | ~50K+ nodes |
| **Dev velocity** | Fast iteration | Slower |
| **Text rendering** | Built-in | Complex to add |

### Recommendation

**Start with egui + custom painting.** Graduate to wgpu/bevy only if:
- Node count exceeds 5,000
- Frame rate drops below 30fps
- Need 3D or advanced shaders

### egui Architecture

```rust
// Main visualization widget
pub struct CbuGraphWidget {
    graph: CbuGraph,
    camera: Camera2D,
    interaction: InteractionState,
    render_cache: RenderCache,
}

impl egui::Widget for CbuGraphWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (response, painter) = ui.allocate_painter(
            ui.available_size(),
            egui::Sense::click_and_drag(),
        );
        
        // Handle input
        self.handle_input(&response, ui);
        
        // Transform to world coordinates
        let to_screen = self.camera.to_screen_transform(response.rect);
        
        // Render layers
        self.render_edges(&painter, &to_screen);
        self.render_nodes(&painter, &to_screen);
        self.render_overlay(&painter, &to_screen);
        self.render_focus_card(&painter, ui, &to_screen);
        
        response
    }
}
```

---

## 1. Overview

### 1.1 Purpose

Interactive visualization of a single CBU's entity structure with two switchable perspectives:
- **KYC/UBO View**: Ownership chains, verification status, risk ratings, documents
- **Onboarding View**: Products, services, resources, delivery status

### 1.2 Core Principles

1. **CBU is context, not a node** - It scopes what we see, but isn't visually present
2. **Entities connect to entities** - Graph is entity-role-entity relationships
3. **Template-driven layout** - CBU type (LuxSICAV, Master-Feeder, etc.) provides scaffold
4. **Core graph is stable** - Switching views doesn't move entities
5. **Overlays swap instantly** - One frame KYC, next frame Onboarding

### 1.3 Visual Summary

```
┌─────────────────────────────────────────────────────────────────────────┐
│  [● KYC/UBO]  [○ Onboarding]                    🔍 Search   ⚙ Settings  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│                              [UBO: Person A]                            │
│                                    │                                    │
│                                  25% │                                  │
│                                    ▼                                    │
│      ┌──────────┐            [Trust X]            ┌──────────┐         │
│      │ Depositary│               │                │  Auditor │         │
│      └─────┬────┘          40%   │   60%          └────┬─────┘         │
│            │                 ╲   │   ╱                 │               │
│            │                  ╲  │  ╱                  │               │
│            └───────────────▶ [Fund] ◀─────────────────┘               │
│                                │                                       │
│                    ┌───────────┼───────────┐                          │
│                    │           │           │                          │
│               [Class A]   [Class I]   [Class B]                       │
│                    │           │           │                          │
│              [47 Investors]  [12 Inv]   [3 Inv]                       │
│                                                                         │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  Fund: Acme SICAV  │  Risk: MEDIUM  │  KYC: 34/37 verified  │  ⚠ 2 exp │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Data Model

### 2.1 Graph Structure

```rust
/// Complete CBU visualization data
pub struct CbuGraph {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub cbu_category: CbuCategory,
    pub template: CbuTemplate,
    
    /// Core entities - always visible, stable positions
    pub core: CoreGraph,
    
    /// Investor groups (aggregated)
    pub investor_groups: Vec<InvestorGroup>,
    
    /// Overlays - loaded on demand
    pub kyc_overlay: Option<KycOverlay>,
    pub onboarding_overlay: Option<OnboardingOverlay>,
    
    /// Current view
    pub active_view: ViewMode,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    KycUbo,
    Onboarding,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CbuCategory {
    FundMandate,
    CorporateGroup,
    InstitutionalAccount,
    RetailClient,
    FamilyTrust,
    CorrespondentBank,
}
```

### 2.2 Core Graph

```rust
/// Stable entity structure - positions don't change between views
pub struct CoreGraph {
    pub entities: Vec<CoreEntity>,
    pub ownership_edges: Vec<OwnershipEdge>,
    pub control_edges: Vec<ControlEdge>,
    
    /// Computed layout (entity_id -> position)
    pub layout: HashMap<Uuid, Vec2>,
    
    /// Layout metadata
    pub bounds: Rect,
    pub anchor_entity: Uuid,
}

pub struct CoreEntity {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: EntityType,
    pub jurisdiction: Option<String>,
    
    /// All roles this entity has in this CBU
    pub roles: Vec<EntityRole>,
    
    /// Primary role (highest priority) - determines badge
    pub primary_role: EntityRole,
    
    /// Template slot this entity fills (if any)
    pub template_slot: Option<String>,
    
    /// Is this entity part of an investor group?
    pub investor_group_id: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    NaturalPerson,
    LimitedCompany,
    Partnership,
    Fund,
    Trust,
    Foundation,
    GovernmentBody,
    Other,
}

pub struct EntityRole {
    pub role_code: RoleCode,
    pub target_entity: Option<Uuid>,  // Who they have this role WITH
    pub ownership_pct: Option<f32>,
    pub effective_date: Option<NaiveDate>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RoleCode {
    // Ownership (highest priority)
    UltimateBeneficialOwner = 100,
    Shareholder = 90,
    BeneficialOwner = 85,
    LimitedPartner = 80,
    
    // Control
    Director = 70,
    Officer = 65,
    ConductingOfficer = 64,
    CompanySecretary = 60,
    AuthorizedSignatory = 55,
    
    // Fund-specific
    ManagementCompany = 75,
    InvestmentManager = 74,
    Depositary = 50,
    Administrator = 45,
    TransferAgent = 44,
    Auditor = 40,
    LegalCounsel = 35,
    PrimeBroker = 38,
    
    // Trust-specific
    Settlor = 72,
    Trustee = 71,
    Protector = 68,
    Beneficiary = 30,
    
    // Other
    Investor = 25,
    ServiceProvider = 20,
    Nominee = 15,
    RelatedParty = 10,
    Other = 0,
}

pub struct OwnershipEdge {
    pub from_entity: Uuid,      // Owner
    pub to_entity: Uuid,        // Owned
    pub ownership_pct: f32,
    pub ownership_type: OwnershipType,
    pub share_class: Option<Uuid>,
}

#[derive(Clone, Copy)]
pub enum OwnershipType {
    Direct,
    Indirect,
    Beneficial,
    Nominee,
}

pub struct ControlEdge {
    pub from_entity: Uuid,      // Controller
    pub to_entity: Uuid,        // Controlled
    pub control_type: ControlType,
}

#[derive(Clone, Copy)]
pub enum ControlType {
    Director,
    Officer,
    Signatory,
    Trustee,
    Manager,
}
```

### 2.3 Investor Groups

```rust
/// Aggregated investor group (for retail funds)
pub struct InvestorGroup {
    pub group_id: usize,
    pub share_class_id: Option<Uuid>,
    pub share_class_name: String,
    
    /// Summary stats
    pub count: usize,
    pub total_ownership_pct: f32,
    pub total_value: Option<f64>,
    pub currency: String,
    
    /// Breakdown
    pub by_jurisdiction: HashMap<String, usize>,
    pub by_investor_type: HashMap<InvestorType, usize>,
    pub by_kyc_status: HashMap<KycStatus, usize>,
    
    /// Expansion state
    pub expanded: bool,
    
    /// Individual members (loaded when expanded, or if count <= threshold)
    pub members: Option<Vec<InvestorMember>>,
    
    /// Visual position (relative to share class node)
    pub position: Vec2,
}

pub struct InvestorMember {
    pub entity_id: Uuid,
    pub name: String,
    pub investor_type: InvestorType,
    pub ownership_pct: f32,
    pub value: Option<f64>,
    pub kyc_status: KycStatus,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvestorType {
    Retail,
    Institutional,
    Founder,
    Employee,
    SeedCapital,
}

const INVESTOR_COLLAPSE_THRESHOLD: usize = 5;
```

### 2.4 KYC/UBO Overlay

```rust
/// KYC-specific visualization data
pub struct KycOverlay {
    /// Per-entity KYC decorations
    pub entity_kyc: HashMap<Uuid, EntityKycData>,
    
    /// Document nodes
    pub documents: Vec<DocumentNode>,
    
    /// Entity-document edges
    pub entity_doc_edges: Vec<EntityDocEdge>,
    
    /// Allegations/observations with issues
    pub alerts: Vec<KycAlert>,
}

pub struct EntityKycData {
    pub kyc_status: KycStatus,
    pub risk_rating: RiskRating,
    pub verification_state: VerificationState,
    pub next_review_date: Option<NaiveDate>,
    pub is_ubo: bool,
    pub aggregate_ownership_pct: Option<f32>,  // Through all chains
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum KycStatus {
    NotStarted,
    InProgress,
    PendingDocuments,
    PendingVerification,
    Verified,
    Rejected,
    Expired,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RiskRating {
    Unrated = 0,
    Standard = 1,
    Low = 2,
    Medium = 3,
    High = 4,
    Prohibited = 5,
}

#[derive(Clone, Copy)]
pub enum VerificationState {
    AllegationOnly,     // Client claimed, not verified
    PartiallyVerified,  // Some docs verified
    FullyVerified,      // All required docs verified
    Contradicted,       // Observations conflict
}

pub struct DocumentNode {
    pub document_id: Uuid,
    pub document_type: String,
    pub category: DocumentCategory,
    pub entity_id: Uuid,           // Which entity this doc belongs to
    pub status: DocumentStatus,
    pub expiry_date: Option<NaiveDate>,
    pub validity_status: ValidityStatus,
    
    /// Position (calculated relative to entity)
    pub position: Vec2,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentCategory {
    Identity,
    Corporate,
    Financial,
    Tax,
    Address,
    Regulatory,
    Ubo,
    Trust,
    Fund,
    Isda,
    Other,
}

#[derive(Clone, Copy)]
pub enum ValidityStatus {
    Valid,
    ExpiringSoon,  // Within 30 days
    Expired,
    Missing,       // Required but not present
    Pending,       // Received, not verified
}

pub struct EntityDocEdge {
    pub entity_id: Uuid,
    pub document_id: Uuid,
    pub relationship: DocRelationship,
}

#[derive(Clone, Copy)]
pub enum DocRelationship {
    ProvesIdentity,
    ProvesAddress,
    ProvesOwnership,
    ProvesAuthority,
    Supporting,
}

pub struct KycAlert {
    pub entity_id: Uuid,
    pub alert_type: AlertType,
    pub message: String,
    pub severity: AlertSeverity,
}

#[derive(Clone, Copy)]
pub enum AlertType {
    DocumentExpiring,
    DocumentExpired,
    DocumentMissing,
    VerificationFailed,
    ObservationConflict,
    ReviewOverdue,
    HighRiskIndicator,
}
```

### 2.5 Onboarding Overlay

```rust
/// Onboarding/service-specific visualization data
pub struct OnboardingOverlay {
    /// Product-Service-Resource tree
    pub products: Vec<ProductNode>,
    pub services: Vec<ServiceNode>,
    pub resources: Vec<ResourceNode>,
    
    /// Edges
    pub product_service_edges: Vec<ProductServiceEdge>,
    pub service_resource_edges: Vec<ServiceResourceEdge>,
    pub entity_resource_edges: Vec<EntityResourceEdge>,
    
    /// Per-entity onboarding role
    pub entity_roles: HashMap<Uuid, OnboardingEntityRole>,
}

pub struct ProductNode {
    pub product_id: Uuid,
    pub product_name: String,
    pub product_type: String,
    pub position: Vec2,
}

pub struct ServiceNode {
    pub service_id: Uuid,
    pub service_name: String,
    pub service_type: String,
    pub delivery_status: DeliveryStatus,
    pub product_id: Uuid,
    pub position: Vec2,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DeliveryStatus {
    NotStarted,
    Pending,
    InProgress,
    Delivered,
    Failed,
    Suspended,
}

pub struct ResourceNode {
    pub instance_id: Uuid,
    pub resource_type: String,
    pub resource_name: String,
    pub external_id: Option<String>,  // Account number, SWIFT BIC, etc.
    pub status: ResourceStatus,
    pub service_id: Uuid,
    pub position: Vec2,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ResourceStatus {
    Pending,
    Active,
    Suspended,
    Decommissioned,
}

pub struct EntityResourceEdge {
    pub entity_id: Uuid,
    pub resource_id: Uuid,
    pub relationship: ResourceRelationship,
}

#[derive(Clone, Copy)]
pub enum ResourceRelationship {
    AccountHolder,
    Signatory,
    Beneficiary,
    Controller,
}

pub struct OnboardingEntityRole {
    pub is_account_holder: bool,
    pub is_signatory: bool,
    pub setup_complete: bool,
}
```

---

## 3. CBU Type Templates

### 3.1 Template Structure

```rust
pub struct CbuTemplate {
    pub template_id: String,
    pub name: String,
    pub description: String,
    pub layout_type: LayoutType,
    pub flow_direction: FlowDirection,
    pub slots: Vec<TemplateSlot>,
    pub expected_flows: Vec<ExpectedFlow>,
}

#[derive(Clone, Copy)]
pub enum LayoutType {
    Hierarchical,       // Top-down or bottom-up tree
    RadialHierarchical, // Center-out with rings
    ForceDirected,      // Pure physics (fallback)
}

#[derive(Clone, Copy)]
pub enum FlowDirection {
    TopDown,
    BottomUp,
    LeftRight,
    CenterOut,
}

pub struct TemplateSlot {
    pub slot_id: String,
    pub label: String,
    pub expected_roles: Vec<RoleCode>,
    pub position: SlotPosition,
    pub layout: SlotLayout,
    pub required: bool,
    pub is_anchor: bool,
}

pub struct SlotPosition {
    pub anchor: PositionAnchor,
    pub offset: Vec2,
}

#[derive(Clone, Copy)]
pub enum PositionAnchor {
    TopCenter,
    TopLeft,
    TopRight,
    Center,
    CenterLeft,
    CenterRight,
    BottomCenter,
    BottomLeft,
    BottomRight,
    BelowSlot(usize),   // Below another slot
    LeftOfSlot(usize),
    RightOfSlot(usize),
}

#[derive(Clone, Copy)]
pub enum SlotLayout {
    Single,              // One entity expected
    VerticalStack,       // Multiple entities stacked vertically
    HorizontalSpread,    // Multiple entities spread horizontally
    ClusteredByParent,   // Cluster under parent slot entities
    Traced,              // Position based on ownership chain (UBOs)
}

pub struct ExpectedFlow {
    pub from_slot: String,
    pub to_slot: String,
    pub relationship: String,
}
```

### 3.2 Template: Luxembourg SICAV

```yaml
template_id: LUX_SICAV
name: "Luxembourg SICAV"
description: "Luxembourg SICAV (UCITS or Part II fund)"
layout_type: Hierarchical
flow_direction: TopDown

slots:
  - slot_id: manco
    label: "Management Company"
    expected_roles: [ManagementCompany, AIFM]
    position:
      anchor: TopCenter
      offset: { x: 0, y: 0 }
    layout: Single
    required: true
    is_anchor: false

  - slot_id: depositary
    label: "Depositary"
    expected_roles: [Depositary, Custodian]
    position:
      anchor: TopLeft
      offset: { x: -200, y: 50 }
    layout: Single
    required: true

  - slot_id: auditor
    label: "Auditor"
    expected_roles: [Auditor]
    position:
      anchor: TopRight
      offset: { x: 200, y: 50 }
    layout: Single
    required: true

  - slot_id: admin
    label: "Administrator"
    expected_roles: [Administrator, FundAdmin, TransferAgent]
    position:
      anchor: CenterLeft
      offset: { x: -250, y: 0 }
    layout: VerticalStack
    required: false

  - slot_id: fund_vehicle
    label: "Fund Vehicle"
    expected_roles: [Principal, FundVehicle, Issuer]
    position:
      anchor: Center
      offset: { x: 0, y: 0 }
    layout: Single
    required: true
    is_anchor: true

  - slot_id: directors
    label: "Directors"
    expected_roles: [Director, ConductingOfficer]
    position:
      anchor: LeftOfSlot
      parent_slot: fund_vehicle
      offset: { x: -150, y: 0 }
    layout: VerticalStack
    required: true

  - slot_id: share_classes
    label: "Share Classes"
    expected_roles: [ShareClass]
    position:
      anchor: BelowSlot
      parent_slot: fund_vehicle
      offset: { x: 0, y: 100 }
    layout: HorizontalSpread
    spacing: 120
    required: false

  - slot_id: investors
    label: "Investors"
    expected_roles: [Investor, Shareholder, LimitedPartner]
    position:
      anchor: BelowSlot
      parent_slot: share_classes
      offset: { x: 0, y: 80 }
    layout: ClusteredByParent
    required: false

  - slot_id: ubos
    label: "UBOs"
    expected_roles: [UltimateBeneficialOwner, BeneficialOwner]
    position:
      anchor: TopCenter
      offset: { x: 0, y: -150 }
    layout: Traced
    required: false
    highlight: true

expected_flows:
  - from_slot: manco
    to_slot: fund_vehicle
    relationship: MANAGES

  - from_slot: depositary
    to_slot: fund_vehicle
    relationship: SAFEKEEPS

  - from_slot: investors
    to_slot: share_classes
    relationship: OWNS

  - from_slot: share_classes
    to_slot: fund_vehicle
    relationship: PART_OF
```

### 3.3 Template: Cayman Master-Feeder

```yaml
template_id: CAYMAN_MASTER_FEEDER
name: "Cayman Master-Feeder"
description: "Cayman master-feeder hedge fund structure"
layout_type: Hierarchical
flow_direction: TopDown

slots:
  - slot_id: investment_manager
    label: "Investment Manager"
    expected_roles: [InvestmentManager, InvestmentAdviser]
    position:
      anchor: TopCenter
      offset: { x: 0, y: 0 }
    layout: Single
    required: true

  - slot_id: master_fund
    label: "Master Fund"
    expected_roles: [MasterFund, Principal]
    position:
      anchor: Center
      offset: { x: 0, y: 0 }
    layout: Single
    required: true
    is_anchor: true

  - slot_id: prime_broker
    label: "Prime Broker"
    expected_roles: [PrimeBroker]
    position:
      anchor: CenterRight
      offset: { x: 220, y: 0 }
    layout: VerticalStack
    required: false

  - slot_id: administrator
    label: "Administrator"
    expected_roles: [Administrator, FundAdmin]
    position:
      anchor: CenterLeft
      offset: { x: -220, y: 0 }
    layout: Single
    required: true

  - slot_id: feeders
    label: "Feeder Funds"
    expected_roles: [FeederFund, Subsidiary]
    position:
      anchor: BelowSlot
      parent_slot: master_fund
      offset: { x: 0, y: 120 }
    layout: HorizontalSpread
    spacing: 250
    required: false

  - slot_id: feeder_investors
    label: "Feeder Investors"
    expected_roles: [Investor, LimitedPartner]
    position:
      anchor: BelowSlot
      parent_slot: feeders
      offset: { x: 0, y: 100 }
    layout: ClusteredByParent
    required: false

  - slot_id: directors
    label: "Directors"
    expected_roles: [Director]
    position:
      anchor: TopLeft
      offset: { x: -180, y: 30 }
    layout: VerticalStack
    required: true

  - slot_id: ubos
    label: "UBOs"
    expected_roles: [UltimateBeneficialOwner]
    position:
      anchor: TopCenter
      offset: { x: 0, y: -120 }
    layout: Traced
    highlight: true
```

### 3.4 Template: Family Trust

```yaml
template_id: FAMILY_TRUST
name: "Family Trust"
description: "Discretionary or fixed family trust"
layout_type: RadialHierarchical
flow_direction: CenterOut

slots:
  - slot_id: trust
    label: "Trust"
    expected_roles: [Principal, TrustVehicle]
    position:
      anchor: Center
      offset: { x: 0, y: 0 }
    layout: Single
    required: true
    is_anchor: true

  - slot_id: settlor
    label: "Settlor"
    expected_roles: [Settlor]
    position:
      anchor: TopCenter
      offset: { x: 0, y: -150 }
    layout: Single
    required: true

  - slot_id: trustees
    label: "Trustees"
    expected_roles: [Trustee, CorporateTrustee]
    position:
      anchor: CenterLeft
      offset: { x: -180, y: 0 }
    layout: VerticalStack
    required: true

  - slot_id: protector
    label: "Protector"
    expected_roles: [Protector]
    position:
      anchor: CenterRight
      offset: { x: 180, y: 0 }
    layout: Single
    required: false

  - slot_id: beneficiaries
    label: "Beneficiaries"
    expected_roles: [Beneficiary, DiscretionaryBeneficiary]
    position:
      anchor: BottomCenter
      offset: { x: 0, y: 150 }
    layout: HorizontalSpread
    spacing: 100

  - slot_id: underlying
    label: "Underlying Assets"
    expected_roles: [UnderlyingCompany, Asset]
    position:
      anchor: BottomLeft
      offset: { x: -150, y: 120 }
    layout: VerticalStack

  - slot_id: ubos
    label: "UBOs"
    expected_roles: [UltimateBeneficialOwner]
    position:
      anchor: TopCenter
      offset: { x: 0, y: -250 }
    layout: Traced
    highlight: true
```

---

## 4. Layout Algorithm

### 4.1 Layout Pipeline

```rust
impl CbuGraph {
    pub fn compute_layout(&mut self) {
        // Step 1: Match entities to template slots
        let slot_assignments = self.assign_entities_to_slots();
        
        // Step 2: Position slotted entities
        self.position_slotted_entities(&slot_assignments);
        
        // Step 3: Position unslotted entities (force-directed within constraints)
        self.position_unslotted_entities(&slot_assignments);
        
        // Step 4: Compute UBO chain positions (traced layout)
        self.position_ubo_chains();
        
        // Step 5: Position investor groups
        self.position_investor_groups();
        
        // Step 6: Apply clustering forces
        self.apply_clustering_refinement();
        
        // Step 7: Compute bounding box
        self.core.bounds = self.compute_bounds();
    }
}
```

### 4.2 Slot Assignment

```rust
fn assign_entities_to_slots(&self) -> HashMap<Uuid, String> {
    let mut assignments: HashMap<Uuid, String> = HashMap::new();
    let mut slot_fills: HashMap<String, Vec<Uuid>> = HashMap::new();
    
    // Sort entities by role priority (highest first)
    let mut entities: Vec<_> = self.core.entities.iter().collect();
    entities.sort_by_key(|e| std::cmp::Reverse(e.primary_role as u32));
    
    for entity in entities {
        // Find best matching slot
        for slot in &self.template.slots {
            let matches = entity.roles.iter()
                .any(|r| slot.expected_roles.contains(&r.role_code));
            
            if matches {
                // Check slot capacity
                let fills = slot_fills.entry(slot.slot_id.clone()).or_default();
                
                let can_fill = match slot.layout {
                    SlotLayout::Single => fills.is_empty(),
                    _ => true,  // Multi-entity slots
                };
                
                if can_fill {
                    fills.push(entity.entity_id);
                    assignments.insert(entity.entity_id, slot.slot_id.clone());
                    break;
                }
            }
        }
    }
    
    assignments
}
```

### 4.3 Position Calculation

```rust
fn position_slotted_entities(&mut self, assignments: &HashMap<Uuid, String>) {
    // Group by slot
    let mut by_slot: HashMap<String, Vec<Uuid>> = HashMap::new();
    for (entity_id, slot_id) in assignments {
        by_slot.entry(slot_id.clone()).or_default().push(*entity_id);
    }
    
    // Find anchor position
    let anchor_slot = self.template.slots.iter()
        .find(|s| s.is_anchor)
        .expect("Template must have anchor slot");
    let anchor_pos = Vec2::ZERO;  // Anchor at origin
    
    // Position each slot's entities
    for slot in &self.template.slots {
        let entities = match by_slot.get(&slot.slot_id) {
            Some(e) => e,
            None => continue,
        };
        
        // Calculate slot base position
        let base_pos = self.calculate_slot_position(slot, anchor_pos);
        
        // Distribute entities within slot
        let positions = match slot.layout {
            SlotLayout::Single => {
                vec![base_pos]
            }
            SlotLayout::VerticalStack => {
                self.vertical_stack(base_pos, entities.len(), 60.0)
            }
            SlotLayout::HorizontalSpread => {
                self.horizontal_spread(base_pos, entities.len(), slot.spacing.unwrap_or(100.0))
            }
            SlotLayout::ClusteredByParent => {
                self.cluster_by_parent(entities, slot)
            }
            SlotLayout::Traced => {
                // Handled separately in position_ubo_chains
                continue;
            }
        };
        
        // Assign positions
        for (entity_id, pos) in entities.iter().zip(positions) {
            self.core.layout.insert(*entity_id, pos);
        }
    }
}

fn calculate_slot_position(&self, slot: &TemplateSlot, anchor_pos: Vec2) -> Vec2 {
    let base = match slot.position.anchor {
        PositionAnchor::TopCenter => Vec2::new(0.0, -200.0),
        PositionAnchor::TopLeft => Vec2::new(-200.0, -200.0),
        PositionAnchor::TopRight => Vec2::new(200.0, -200.0),
        PositionAnchor::Center => Vec2::ZERO,
        PositionAnchor::CenterLeft => Vec2::new(-200.0, 0.0),
        PositionAnchor::CenterRight => Vec2::new(200.0, 0.0),
        PositionAnchor::BottomCenter => Vec2::new(0.0, 200.0),
        PositionAnchor::BottomLeft => Vec2::new(-200.0, 200.0),
        PositionAnchor::BottomRight => Vec2::new(200.0, 200.0),
        PositionAnchor::BelowSlot(parent_idx) => {
            // Get parent slot position
            let parent_slot = &self.template.slots[parent_idx];
            self.calculate_slot_position(parent_slot, anchor_pos) + Vec2::new(0.0, 100.0)
        }
        // ... other anchors
    };
    
    anchor_pos + base + slot.position.offset
}

fn vertical_stack(&self, base: Vec2, count: usize, spacing: f32) -> Vec<Vec2> {
    let total_height = (count - 1) as f32 * spacing;
    let start_y = base.y - total_height / 2.0;
    
    (0..count)
        .map(|i| Vec2::new(base.x, start_y + i as f32 * spacing))
        .collect()
}

fn horizontal_spread(&self, base: Vec2, count: usize, spacing: f32) -> Vec<Vec2> {
    let total_width = (count - 1) as f32 * spacing;
    let start_x = base.x - total_width / 2.0;
    
    (0..count)
        .map(|i| Vec2::new(start_x + i as f32 * spacing, base.y))
        .collect()
}
```

### 4.4 UBO Chain Positioning

```rust
fn position_ubo_chains(&mut self) {
    // Find all UBO entities
    let ubos: Vec<_> = self.core.entities.iter()
        .filter(|e| e.roles.iter().any(|r| r.role_code == RoleCode::UltimateBeneficialOwner))
        .map(|e| e.entity_id)
        .collect();
    
    // Trace ownership chains from each UBO down to anchor
    for ubo_id in ubos {
        let chain = self.trace_ownership_chain(ubo_id, self.core.anchor_entity);
        
        // Position UBO at top, based on chain
        let chain_index = self.get_ubo_chain_index(ubo_id);
        let x_offset = (chain_index as f32 - (self.ubo_count() as f32 / 2.0)) * 150.0;
        
        self.core.layout.insert(ubo_id, Vec2::new(x_offset, -300.0));
        
        // Position intermediate entities in chain
        for (depth, entity_id) in chain.iter().enumerate() {
            if !self.core.layout.contains_key(entity_id) {
                let y = -300.0 + (depth as f32 + 1.0) * 80.0;
                self.core.layout.insert(*entity_id, Vec2::new(x_offset, y));
            }
        }
    }
}

fn trace_ownership_chain(&self, from: Uuid, to: Uuid) -> Vec<Uuid> {
    // BFS to find path from UBO to target
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut parent: HashMap<Uuid, Uuid> = HashMap::new();
    
    queue.push_back(from);
    visited.insert(from);
    
    while let Some(current) = queue.pop_front() {
        if current == to {
            // Reconstruct path
            let mut path = vec![];
            let mut node = to;
            while node != from {
                path.push(node);
                node = parent[&node];
            }
            path.reverse();
            return path;
        }
        
        // Follow ownership edges
        for edge in &self.core.ownership_edges {
            if edge.from_entity == current && !visited.contains(&edge.to_entity) {
                visited.insert(edge.to_entity);
                parent.insert(edge.to_entity, current);
                queue.push_back(edge.to_entity);
            }
        }
    }
    
    vec![]
}
```

### 4.5 Clustering Refinement

```rust
struct ClusteringConfig {
    /// Same entity type attracts
    type_clustering_strength: f32,
    
    /// Co-shareholders cluster
    coshareholder_strength: f32,
    
    /// Family members cluster tightly
    family_strength: f32,
    
    /// Employees cluster with employer
    employment_strength: f32,
    
    /// Service providers form outer ring
    service_provider_distance: f32,
    
    /// Collision avoidance radius
    min_node_distance: f32,
}

impl Default for ClusteringConfig {
    fn default() -> Self {
        Self {
            type_clustering_strength: 0.3,
            coshareholder_strength: 0.5,
            family_strength: 0.9,
            employment_strength: 0.8,
            service_provider_distance: 250.0,
            min_node_distance: 50.0,
        }
    }
}

fn apply_clustering_refinement(&mut self) {
    let config = ClusteringConfig::default();
    
    // Run a few iterations of force refinement
    for _ in 0..50 {
        let mut forces: HashMap<Uuid, Vec2> = HashMap::new();
        
        // Initialize with zero force
        for entity in &self.core.entities {
            forces.insert(entity.entity_id, Vec2::ZERO);
        }
        
        // Type clustering: same types attract horizontally
        self.apply_type_clustering(&mut forces, &config);
        
        // Co-shareholder clustering
        self.apply_coshareholder_clustering(&mut forces, &config);
        
        // Collision avoidance
        self.apply_collision_avoidance(&mut forces, &config);
        
        // Apply forces (with damping)
        for (entity_id, force) in &forces {
            if let Some(pos) = self.core.layout.get_mut(entity_id) {
                *pos += *force * 0.1;  // Damping
            }
        }
    }
}

fn apply_collision_avoidance(&self, forces: &mut HashMap<Uuid, Vec2>, config: &ClusteringConfig) {
    let entities: Vec<_> = self.core.layout.iter().collect();
    
    for i in 0..entities.len() {
        for j in (i + 1)..entities.len() {
            let (id_a, pos_a) = entities[i];
            let (id_b, pos_b) = entities[j];
            
            let delta = *pos_b - *pos_a;
            let distance = delta.length();
            
            if distance < config.min_node_distance && distance > 0.001 {
                // Repulsion force
                let repulsion = delta.normalize() * (config.min_node_distance - distance);
                
                *forces.get_mut(id_a).unwrap() -= repulsion * 0.5;
                *forces.get_mut(id_b).unwrap() += repulsion * 0.5;
            }
        }
    }
}
```

---

## 5. Visual Encoding

### 5.1 Entity Nodes

```rust
impl CoreEntity {
    fn shape(&self) -> Shape {
        match self.entity_type {
            EntityType::NaturalPerson => Shape::Circle,
            EntityType::LimitedCompany => Shape::RoundedRect { corner_radius: 8.0 },
            EntityType::Partnership => Shape::RoundedRect { corner_radius: 4.0 },
            EntityType::Fund => Shape::Diamond,
            EntityType::Trust => Shape::Hexagon,
            EntityType::Foundation => Shape::Octagon,
            EntityType::GovernmentBody => Shape::Rect,
            EntityType::Other => Shape::Circle,
        }
    }
    
    fn base_size(&self) -> f32 {
        // Size based on role importance
        match self.primary_role {
            RoleCode::UltimateBeneficialOwner => 40.0,
            RoleCode::Shareholder => 35.0,
            RoleCode::Director => 30.0,
            RoleCode::ManagementCompany => 35.0,
            RoleCode::InvestmentManager => 35.0,
            RoleCode::Depositary => 28.0,
            RoleCode::Administrator => 25.0,
            RoleCode::Investor => 22.0,
            _ => 24.0,
        }
    }
}
```

### 5.2 Color Palettes

```rust
mod colors {
    use egui::Color32;
    
    // Risk rating colors (KYC view)
    pub fn risk_color(rating: RiskRating) -> Color32 {
        match rating {
            RiskRating::Unrated => Color32::from_rgb(158, 158, 158),    // Grey
            RiskRating::Standard => Color32::from_rgb(76, 175, 80),    // Green
            RiskRating::Low => Color32::from_rgb(139, 195, 74),        // Light green
            RiskRating::Medium => Color32::from_rgb(255, 193, 7),      // Amber
            RiskRating::High => Color32::from_rgb(255, 87, 34),        // Deep orange
            RiskRating::Prohibited => Color32::from_rgb(33, 33, 33),   // Near black
        }
    }
    
    // KYC status border colors
    pub fn kyc_status_color(status: KycStatus) -> Color32 {
        match status {
            KycStatus::Verified => Color32::from_rgb(76, 175, 80),     // Green
            KycStatus::InProgress => Color32::from_rgb(33, 150, 243),  // Blue
            KycStatus::PendingDocuments => Color32::from_rgb(255, 193, 7), // Amber
            KycStatus::PendingVerification => Color32::from_rgb(255, 152, 0), // Orange
            KycStatus::NotStarted => Color32::from_rgb(158, 158, 158), // Grey
            KycStatus::Rejected => Color32::from_rgb(244, 67, 54),     // Red
            KycStatus::Expired => Color32::from_rgb(121, 85, 72),      // Brown
        }
    }
    
    // Entity type colors (onboarding view - neutral)
    pub fn entity_type_color(entity_type: EntityType) -> Color32 {
        match entity_type {
            EntityType::NaturalPerson => Color32::from_rgb(100, 181, 246),  // Light blue
            EntityType::LimitedCompany => Color32::from_rgb(144, 164, 174), // Blue grey
            EntityType::Fund => Color32::from_rgb(178, 223, 219),          // Teal light
            EntityType::Trust => Color32::from_rgb(206, 147, 216),         // Purple light
            _ => Color32::from_rgb(176, 190, 197),                         // Grey blue
        }
    }
    
    // Document category colors
    pub fn doc_category_color(category: DocumentCategory) -> Color32 {
        match category {
            DocumentCategory::Identity => Color32::from_rgb(66, 165, 245),
            DocumentCategory::Corporate => Color32::from_rgb(102, 187, 106),
            DocumentCategory::Financial => Color32::from_rgb(255, 202, 40),
            DocumentCategory::Tax => Color32::from_rgb(239, 83, 80),
            DocumentCategory::Address => Color32::from_rgb(171, 71, 188),
            DocumentCategory::Regulatory => Color32::from_rgb(255, 112, 67),
            DocumentCategory::Ubo => Color32::from_rgb(255, 167, 38),
            _ => Color32::from_rgb(158, 158, 158),
        }
    }
    
    // Delivery status colors
    pub fn delivery_status_color(status: DeliveryStatus) -> Color32 {
        match status {
            DeliveryStatus::Delivered => Color32::from_rgb(76, 175, 80),
            DeliveryStatus::InProgress => Color32::from_rgb(33, 150, 243),
            DeliveryStatus::Pending => Color32::from_rgb(255, 193, 7),
            DeliveryStatus::NotStarted => Color32::from_rgb(158, 158, 158),
            DeliveryStatus::Failed => Color32::from_rgb(244, 67, 54),
            DeliveryStatus::Suspended => Color32::from_rgb(121, 85, 72),
        }
    }
}
```

### 5.3 Level of Detail

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum DetailLevel {
    Micro,      // Colored dot only (< 8px screen radius)
    Icon,       // Shape + status indicator (8-20px)
    Compact,    // Shape + truncated name (20-40px)
    Standard,   // Shape + full name + badge (40-80px)
    Expanded,   // All inline details (80px+)
    Focused,    // Full card overlay
}

impl DetailLevel {
    pub fn from_screen_size(screen_radius: f32, is_focused: bool) -> Self {
        if is_focused {
            return DetailLevel::Focused;
        }
        
        match screen_radius {
            r if r < 8.0 => DetailLevel::Micro,
            r if r < 20.0 => DetailLevel::Icon,
            r if r < 40.0 => DetailLevel::Compact,
            r if r < 80.0 => DetailLevel::Standard,
            _ => DetailLevel::Expanded,
        }
    }
}
```

---

## 6. Edge Routing

### 6.1 Bezier Curves

```rust
pub struct EdgeCurve {
    pub from: Vec2,
    pub to: Vec2,
    pub control: Vec2,      // Quadratic bezier control point
    pub hops: Vec<f32>,     // t-values where to draw hop arcs
}

impl EdgeCurve {
    pub fn new(from: Vec2, to: Vec2, curve_strength: f32) -> Self {
        let delta = to - from;
        let distance = delta.length();
        
        // Perpendicular offset for control point
        let perpendicular = Vec2::new(-delta.y, delta.x).normalize();
        let offset = perpendicular * distance * curve_strength;
        
        let control = (from + to) / 2.0 + offset;
        
        Self {
            from,
            to,
            control,
            hops: vec![],
        }
    }
    
    pub fn point_at(&self, t: f32) -> Vec2 {
        // Quadratic bezier: B(t) = (1-t)²P0 + 2(1-t)tP1 + t²P2
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        
        self.from * mt2 + self.control * (2.0 * mt * t) + self.to * t2
    }
    
    pub fn tangent_at(&self, t: f32) -> Vec2 {
        // Derivative: B'(t) = 2(1-t)(P1-P0) + 2t(P2-P1)
        let mt = 1.0 - t;
        
        (self.control - self.from) * (2.0 * mt) + (self.to - self.control) * (2.0 * t)
    }
}
```

### 6.2 Curve Strength by Edge Type

```rust
fn curve_strength_for_edge(edge_type: EdgeType) -> f32 {
    match edge_type {
        EdgeType::Ownership => 0.25,     // Gentle curve
        EdgeType::Control => 0.15,       // Subtle
        EdgeType::Service => 0.35,       // More pronounced
        EdgeType::Document => 0.10,      // Almost straight
        EdgeType::Resource => 0.30,
    }
}
```

### 6.3 Intersection Detection

```rust
fn find_curve_intersections(curves: &[EdgeCurve]) -> Vec<(usize, usize, f32, f32)> {
    let mut intersections = vec![];
    
    for i in 0..curves.len() {
        for j in (i + 1)..curves.len() {
            // Sample curves and check for crossings
            if let Some((t_i, t_j)) = find_intersection(&curves[i], &curves[j]) {
                intersections.push((i, j, t_i, t_j));
            }
        }
    }
    
    intersections
}

fn find_intersection(a: &EdgeCurve, b: &EdgeCurve) -> Option<(f32, f32)> {
    // Numerical approach: sample both curves and check for crossings
    const SAMPLES: usize = 20;
    
    for i in 0..SAMPLES {
        let t_a1 = i as f32 / SAMPLES as f32;
        let t_a2 = (i + 1) as f32 / SAMPLES as f32;
        let a1 = a.point_at(t_a1);
        let a2 = a.point_at(t_a2);
        
        for j in 0..SAMPLES {
            let t_b1 = j as f32 / SAMPLES as f32;
            let t_b2 = (j + 1) as f32 / SAMPLES as f32;
            let b1 = b.point_at(t_b1);
            let b2 = b.point_at(t_b2);
            
            // Check if line segments intersect
            if let Some((u, v)) = line_intersection(a1, a2, b1, b2) {
                let t_a = t_a1 + u * (t_a2 - t_a1);
                let t_b = t_b1 + v * (t_b2 - t_b1);
                return Some((t_a, t_b));
            }
        }
    }
    
    None
}
```

### 6.4 Hop Arc Rendering

```rust
fn render_edge_with_hops(
    painter: &egui::Painter,
    curve: &EdgeCurve,
    style: &EdgeStyle,
    to_screen: &Transform,
) {
    const HOP_RADIUS: f32 = 6.0;
    const HOP_GAP: f32 = 0.03;  // t-space gap around hop
    
    let mut sorted_hops = curve.hops.clone();
    sorted_hops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    // Draw segments between hops
    let mut segments: Vec<(f32, f32)> = vec![];
    let mut last_t = 0.0;
    
    for hop_t in &sorted_hops {
        if last_t < hop_t - HOP_GAP {
            segments.push((last_t, hop_t - HOP_GAP));
        }
        last_t = hop_t + HOP_GAP;
    }
    if last_t < 1.0 {
        segments.push((last_t, 1.0));
    }
    
    // Draw curve segments
    for (start_t, end_t) in segments {
        let points: Vec<egui::Pos2> = (0..=20)
            .map(|i| {
                let t = start_t + (end_t - start_t) * (i as f32 / 20.0);
                to_screen.transform_point(curve.point_at(t))
            })
            .collect();
        
        painter.add(egui::Shape::line(points, style.stroke));
    }
    
    // Draw hop arcs
    for hop_t in &sorted_hops {
        let hop_point = curve.point_at(*hop_t);
        let tangent = curve.tangent_at(*hop_t).normalize();
        let normal = Vec2::new(-tangent.y, tangent.x);
        
        let arc_center = hop_point + normal * HOP_RADIUS;
        let screen_center = to_screen.transform_point(arc_center);
        let screen_radius = HOP_RADIUS * to_screen.scale;
        
        // Draw arc (semi-circle)
        let angle = tangent.y.atan2(tangent.x);
        let arc_points: Vec<egui::Pos2> = (0..=10)
            .map(|i| {
                let a = angle - std::f32::consts::PI / 2.0 
                      + std::f32::consts::PI * (i as f32 / 10.0);
                egui::pos2(
                    screen_center.x + screen_radius * a.cos(),
                    screen_center.y + screen_radius * a.sin(),
                )
            })
            .collect();
        
        painter.add(egui::Shape::line(arc_points, style.stroke));
    }
}
```

### 6.5 Edge Priority (Who Hops)

```rust
fn edge_priority(role: RoleCode) -> u32 {
    // Higher priority = drawn on top = others hop over this
    match role {
        RoleCode::UltimateBeneficialOwner => 100,
        RoleCode::Shareholder => 90,
        RoleCode::BeneficialOwner => 85,
        RoleCode::Director => 70,
        RoleCode::ManagementCompany => 75,
        RoleCode::Trustee => 72,
        RoleCode::Depositary => 50,
        RoleCode::Administrator => 45,
        RoleCode::ServiceProvider => 20,
        _ => 40,
    }
}

fn assign_hops(curves: &mut [EdgeCurve], edges: &[Edge]) {
    let intersections = find_curve_intersections(curves);
    
    for (i, j, t_i, t_j) in intersections {
        let priority_i = edge_priority(edges[i].role);
        let priority_j = edge_priority(edges[j].role);
        
        // Lower priority edge gets the hop
        if priority_i < priority_j {
            curves[i].hops.push(t_i);
        } else {
            curves[j].hops.push(t_j);
        }
    }
}
```

### 6.6 Edge Styles

```rust
pub struct EdgeStyle {
    pub stroke: egui::Stroke,
    pub show_label: bool,
    pub show_arrow: bool,
    pub dash_pattern: Option<Vec<f32>>,
}

fn edge_style(role: RoleCode, ownership_pct: Option<f32>, view_mode: ViewMode) -> EdgeStyle {
    let (color, width) = match role {
        RoleCode::UltimateBeneficialOwner | RoleCode::Shareholder | RoleCode::BeneficialOwner => {
            (Color32::from_rgb(25, 118, 210), 3.0)  // Blue, thick
        }
        RoleCode::Director | RoleCode::Officer => {
            (Color32::from_rgb(56, 142, 60), 2.0)   // Green
        }
        RoleCode::Trustee | RoleCode::Settlor | RoleCode::Protector => {
            (Color32::from_rgb(123, 31, 162), 2.0)  // Purple
        }
        RoleCode::Depositary | RoleCode::Administrator | RoleCode::Auditor => {
            (Color32::from_rgb(117, 117, 117), 1.5) // Grey
        }
        _ => {
            (Color32::from_rgb(158, 158, 158), 1.0) // Light grey
        }
    };
    
    EdgeStyle {
        stroke: egui::Stroke::new(width, color),
        show_label: ownership_pct.is_some(),
        show_arrow: matches!(role, 
            RoleCode::Shareholder | RoleCode::BeneficialOwner | 
            RoleCode::UltimateBeneficialOwner | RoleCode::Director
        ),
        dash_pattern: if matches!(role, RoleCode::Nominee | RoleCode::BeneficialOwner) {
            Some(vec![8.0, 4.0])
        } else {
            None
        },
    }
}
```

---

## 7. Investor Aggregation

### 7.1 Collapse Logic

```rust
const INVESTOR_COLLAPSE_THRESHOLD: usize = 5;
const INVESTOR_DETAIL_ZOOM: f32 = 2.0;

impl InvestorGroup {
    pub fn visibility(&self, zoom: f32) -> InvestorVisibility {
        if self.count == 0 {
            return InvestorVisibility::Hidden;
        }
        
        if zoom < 0.5 {
            InvestorVisibility::Hidden
        } else if zoom < 1.0 {
            InvestorVisibility::CollapsedSummary
        } else if zoom < INVESTOR_DETAIL_ZOOM || !self.expanded {
            InvestorVisibility::CollapsedClickable
        } else {
            InvestorVisibility::Individual
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum InvestorVisibility {
    Hidden,
    CollapsedSummary,      // Just a count badge
    CollapsedClickable,    // Summary node, can click to expand
    Individual,            // Show individual investors
}
```

### 7.2 Collapsed Node Rendering

```rust
fn render_investor_group_collapsed(
    painter: &egui::Painter,
    group: &InvestorGroup,
    position: egui::Pos2,
    screen_scale: f32,
) {
    let node_size = Vec2::new(140.0, 80.0) * screen_scale;
    let rect = egui::Rect::from_center_size(position, node_size);
    
    // Background
    painter.rect_filled(rect, 8.0, Color32::from_rgb(240, 240, 240));
    painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.0, Color32::from_rgb(189, 189, 189)));
    
    // Icon and count
    let icon = "👥";
    let count_text = format!("{} Investors", group.count);
    
    painter.text(
        rect.center_top() + Vec2::new(0.0, 15.0 * screen_scale),
        egui::Align2::CENTER_CENTER,
        format!("{} {}", icon, count_text),
        egui::FontId::proportional(14.0 * screen_scale),
        Color32::from_rgb(66, 66, 66),
    );
    
    // Summary line
    let summary = format!("{}: {}", group.share_class_name, format_currency(group.total_value));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        summary,
        egui::FontId::proportional(11.0 * screen_scale),
        Color32::from_rgb(97, 97, 97),
    );
    
    // KYC breakdown
    let verified = group.by_kyc_status.get(&KycStatus::Verified).unwrap_or(&0);
    let pending = group.count - verified;
    let kyc_text = format!("KYC: {}✓ {}⚠", verified, pending);
    painter.text(
        rect.center_bottom() - Vec2::new(0.0, 15.0 * screen_scale),
        egui::Align2::CENTER_CENTER,
        kyc_text,
        egui::FontId::proportional(10.0 * screen_scale),
        Color32::from_rgb(117, 117, 117),
    );
    
    // Expand hint
    if screen_scale > 0.8 {
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, 3.0),
            egui::Align2::CENTER_BOTTOM,
            "[Click to expand]",
            egui::FontId::proportional(9.0),
            Color32::from_rgb(150, 150, 150),
        );
    }
}
```

### 7.3 Expanded View

```rust
fn render_investor_group_expanded(
    painter: &egui::Painter,
    group: &InvestorGroup,
    base_position: Vec2,
    to_screen: &Transform,
    interaction: &mut InteractionState,
) {
    let members = match &group.members {
        Some(m) => m,
        None => return,  // Not loaded yet
    };
    
    // Container box
    let container_width = 400.0;
    let row_height = 40.0;
    let container_height = 60.0 + (members.len().min(10) as f32) * row_height;
    
    let screen_pos = to_screen.transform_point(base_position);
    let rect = egui::Rect::from_min_size(
        screen_pos - Vec2::new(container_width / 2.0, 0.0),
        Vec2::new(container_width, container_height),
    );
    
    // Background with shadow
    painter.rect_filled(rect.expand(2.0), 8.0, Color32::from_rgba_unmultiplied(0, 0, 0, 30));
    painter.rect_filled(rect, 8.0, Color32::WHITE);
    painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.0, Color32::from_rgb(200, 200, 200)));
    
    // Header
    let header_rect = egui::Rect::from_min_max(rect.min, rect.min + Vec2::new(container_width, 40.0));
    painter.rect_filled(header_rect, egui::Rounding { nw: 8.0, ne: 8.0, sw: 0.0, se: 0.0 }, 
                       Color32::from_rgb(250, 250, 250));
    
    painter.text(
        header_rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{} - {} Investors", group.share_class_name, group.count),
        egui::FontId::proportional(13.0),
        Color32::from_rgb(33, 33, 33),
    );
    
    // Collapse button
    // ... (button in top-right)
    
    // Investor rows
    let visible_count = members.len().min(10);
    for (i, investor) in members.iter().take(visible_count).enumerate() {
        let row_y = rect.min.y + 50.0 + (i as f32) * row_height;
        let row_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + 10.0, row_y),
            Vec2::new(container_width - 20.0, row_height - 5.0),
        );
        
        // Entity shape (small)
        let shape_pos = row_rect.left_center() + Vec2::new(15.0, 0.0);
        render_entity_shape_micro(painter, shape_pos, investor.entity_type(), investor.kyc_status);
        
        // Name
        painter.text(
            row_rect.left_center() + Vec2::new(35.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &investor.name,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(33, 33, 33),
        );
        
        // Ownership %
        painter.text(
            row_rect.right_center() - Vec2::new(80.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            format!("{:.2}%", investor.ownership_pct),
            egui::FontId::proportional(11.0),
            Color32::from_rgb(97, 97, 97),
        );
        
        // Value
        if let Some(value) = investor.value {
            painter.text(
                row_rect.right_center() - Vec2::new(10.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                format_currency_short(value, &group.currency),
                egui::FontId::proportional(11.0),
                Color32::from_rgb(97, 97, 97),
            );
        }
    }
    
    // "More" indicator
    if members.len() > 10 {
        let more_text = format!("... +{} more", members.len() - 10);
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, 15.0),
            egui::Align2::CENTER_CENTER,
            more_text,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(100, 100, 100),
        );
    }
}
```

---

## 8. Focus Mode

### 8.1 Focus State

```rust
pub struct FocusState {
    pub focused_entity: Option<Uuid>,
    pub highlight_set: HashSet<Uuid>,
    pub transition_progress: f32,  // 0.0 = unfocused, 1.0 = fully focused
}

impl FocusState {
    pub fn set_focus(&mut self, entity_id: Uuid, graph: &CbuGraph) {
        self.focused_entity = Some(entity_id);
        self.highlight_set.clear();
        self.highlight_set.insert(entity_id);
        
        // Add directly connected entities
        for edge in &graph.core.ownership_edges {
            if edge.from_entity == entity_id {
                self.highlight_set.insert(edge.to_entity);
            }
            if edge.to_entity == entity_id {
                self.highlight_set.insert(edge.from_entity);
            }
        }
        for edge in &graph.core.control_edges {
            if edge.from_entity == entity_id {
                self.highlight_set.insert(edge.to_entity);
            }
            if edge.to_entity == entity_id {
                self.highlight_set.insert(edge.from_entity);
            }
        }
        
        self.transition_progress = 0.0;
    }
    
    pub fn clear_focus(&mut self) {
        self.focused_entity = None;
        self.highlight_set.clear();
        self.transition_progress = 1.0;  // Will animate to 0
    }
    
    pub fn update(&mut self, dt: f32) {
        let target = if self.focused_entity.is_some() { 1.0 } else { 0.0 };
        let speed = 5.0;  // Units per second
        
        if self.transition_progress < target {
            self.transition_progress = (self.transition_progress + speed * dt).min(target);
        } else if self.transition_progress > target {
            self.transition_progress = (self.transition_progress - speed * dt).max(target);
        }
    }
    
    pub fn blur_amount(&self) -> f32 {
        self.transition_progress * 0.7  // Max 70% dimmed
    }
    
    pub fn is_highlighted(&self, entity_id: Uuid) -> bool {
        self.highlight_set.contains(&entity_id)
    }
}
```

### 8.2 Blurred Rendering

```rust
fn render_with_focus(
    painter: &egui::Painter,
    graph: &CbuGraph,
    focus: &FocusState,
    to_screen: &Transform,
) {
    let blur = focus.blur_amount();
    
    if blur > 0.001 {
        // Render dimmed background entities
        for entity in &graph.core.entities {
            if !focus.is_highlighted(entity.entity_id) {
                let opacity = (1.0 - blur) as u8 * 255 / 100;
                render_entity_dimmed(painter, entity, to_screen, opacity);
            }
        }
        
        // Render dimmed background edges
        for edge in &graph.core.ownership_edges {
            if !focus.is_highlighted(edge.from_entity) || !focus.is_highlighted(edge.to_entity) {
                let opacity = (1.0 - blur) as u8 * 255 / 100;
                render_edge_dimmed(painter, edge, graph, to_screen, opacity);
            }
        }
    }
    
    // Render highlighted entities at full opacity
    for entity_id in &focus.highlight_set {
        if let Some(entity) = graph.get_entity(*entity_id) {
            render_entity(painter, entity, graph, to_screen, DetailLevel::Standard);
        }
    }
    
    // Render highlighted edges
    for edge in &graph.core.ownership_edges {
        if focus.is_highlighted(edge.from_entity) && focus.is_highlighted(edge.to_entity) {
            render_edge(painter, edge, graph, to_screen);
        }
    }
    
    // Render focus card
    if let Some(focused_id) = focus.focused_entity {
        if let Some(entity) = graph.get_entity(focused_id) {
            render_focus_card(painter, entity, graph, to_screen);
        }
    }
}
```

### 8.3 Focus Card

```rust
fn render_focus_card(
    ui: &mut egui::Ui,
    entity: &CoreEntity,
    graph: &CbuGraph,
) {
    let card_width = 320.0;
    
    egui::Window::new("Entity Details")
        .id(egui::Id::new("focus_card"))
        .fixed_size([card_width, 400.0])
        .anchor(egui::Align2::RIGHT_CENTER, [-20.0, 0.0])
        .frame(egui::Frame::window(ui.style()).shadow(egui::epaint::Shadow {
            extrusion: 8.0,
            color: Color32::from_black_alpha(40),
        }))
        .show(ui.ctx(), |ui| {
            // Header
            ui.horizontal(|ui| {
                render_entity_shape_medium(ui, entity);
                ui.vertical(|ui| {
                    ui.heading(&entity.name);
                    ui.label(format!("{:?}", entity.entity_type));
                });
            });
            
            ui.separator();
            
            // Roles section
            ui.collapsing("Roles", |ui| {
                for role in &entity.roles {
                    ui.horizontal(|ui| {
                        ui.label(format!("• {:?}", role.role_code));
                        if let Some(target) = role.target_entity {
                            if let Some(target_entity) = graph.get_entity(target) {
                                ui.label(format!("→ {}", target_entity.name));
                            }
                        }
                        if let Some(pct) = role.ownership_pct {
                            ui.label(format!("({}%)", pct));
                        }
                    });
                }
            });
            
            // KYC section (if KYC view)
            if let Some(kyc_overlay) = &graph.kyc_overlay {
                if let Some(kyc_data) = kyc_overlay.entity_kyc.get(&entity.entity_id) {
                    ui.separator();
                    ui.collapsing("KYC Status", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Status:");
                            ui.colored_label(
                                colors::kyc_status_color(kyc_data.kyc_status),
                                format!("{:?}", kyc_data.kyc_status),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Risk:");
                            ui.colored_label(
                                colors::risk_color(kyc_data.risk_rating),
                                format!("{:?}", kyc_data.risk_rating),
                            );
                        });
                        if kyc_data.is_ubo {
                            ui.horizontal(|ui| {
                                ui.label("🎯 UBO");
                                if let Some(pct) = kyc_data.aggregate_ownership_pct {
                                    ui.label(format!("({}% aggregate)", pct));
                                }
                            });
                        }
                        if let Some(date) = kyc_data.next_review_date {
                            ui.label(format!("Next review: {}", date));
                        }
                    });
                }
            }
            
            // Documents section
            if let Some(kyc_overlay) = &graph.kyc_overlay {
                let entity_docs: Vec<_> = kyc_overlay.documents.iter()
                    .filter(|d| d.entity_id == entity.entity_id)
                    .collect();
                
                if !entity_docs.is_empty() {
                    ui.separator();
                    ui.collapsing(format!("Documents ({})", entity_docs.len()), |ui| {
                        for doc in entity_docs {
                            ui.horizontal(|ui| {
                                let icon = match doc.validity_status {
                                    ValidityStatus::Valid => "✓",
                                    ValidityStatus::ExpiringSoon => "⚠",
                                    ValidityStatus::Expired => "✗",
                                    ValidityStatus::Missing => "❌",
                                    ValidityStatus::Pending => "○",
                                };
                                ui.label(icon);
                                ui.label(&doc.document_type);
                                if let Some(date) = doc.expiry_date {
                                    ui.label(format!("(exp: {})", date));
                                }
                            });
                        }
                    });
                }
            }
            
            // Actions
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("View Full Details").clicked() {
                    // Navigate to entity detail page
                }
                if ui.button("Start Review").clicked() {
                    // Trigger KYC review workflow
                }
            });
        });
}
```

---

## 9. Camera and Interaction

### 9.1 Camera

```rust
pub struct Camera2D {
    pub center: Vec2,
    pub zoom: f32,
    pub target_center: Vec2,
    pub target_zoom: f32,
    pub animation_speed: f32,
}

impl Camera2D {
    pub fn new() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            target_center: Vec2::ZERO,
            target_zoom: 1.0,
            animation_speed: 5.0,
        }
    }
    
    pub fn update(&mut self, dt: f32) {
        // Smooth interpolation to target
        let t = (self.animation_speed * dt).min(1.0);
        self.center = self.center.lerp(self.target_center, t);
        self.zoom = self.zoom + (self.target_zoom - self.zoom) * t;
    }
    
    pub fn to_screen_transform(&self, viewport: egui::Rect) -> Transform {
        Transform {
            offset: viewport.center().to_vec2() - self.center * self.zoom,
            scale: self.zoom,
        }
    }
    
    pub fn fit_to_bounds(&mut self, bounds: Rect, viewport_size: Vec2, padding: f32) {
        self.target_center = bounds.center();
        
        let padded_bounds = bounds.expand(bounds.width() * padding);
        self.target_zoom = (viewport_size.x / padded_bounds.width())
            .min(viewport_size.y / padded_bounds.height());
    }
    
    pub fn pan(&mut self, delta: Vec2) {
        self.target_center -= delta / self.zoom;
    }
    
    pub fn zoom_at(&mut self, cursor_world: Vec2, factor: f32) {
        let new_zoom = (self.target_zoom * factor).clamp(0.1, 10.0);
        
        // Zoom toward cursor
        let cursor_offset = cursor_world - self.target_center;
        self.target_center += cursor_offset * (1.0 - self.target_zoom / new_zoom);
        self.target_zoom = new_zoom;
    }
}

pub struct Transform {
    pub offset: Vec2,
    pub scale: f32,
}

impl Transform {
    pub fn transform_point(&self, world: Vec2) -> egui::Pos2 {
        egui::pos2(
            world.x * self.scale + self.offset.x,
            world.y * self.scale + self.offset.y,
        )
    }
    
    pub fn inverse_transform_point(&self, screen: egui::Pos2) -> Vec2 {
        Vec2::new(
            (screen.x - self.offset.x) / self.scale,
            (screen.y - self.offset.y) / self.scale,
        )
    }
}
```

### 9.2 Interaction Handling

```rust
pub struct InteractionState {
    pub hovered_entity: Option<Uuid>,
    pub selected_entity: Option<Uuid>,
    pub dragging: Option<DragState>,
    pub focus: FocusState,
}

pub struct DragState {
    pub start_pos: Vec2,
    pub entity_id: Option<Uuid>,  // None = panning
}

impl CbuGraphWidget {
    fn handle_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
        let to_screen = self.camera.to_screen_transform(response.rect);
        
        // Mouse position in world coordinates
        let mouse_world = ui.input(|i| {
            i.pointer.hover_pos()
                .map(|p| to_screen.inverse_transform_point(p))
        });
        
        // Hover detection
        self.interaction.hovered_entity = mouse_world.and_then(|pos| {
            self.find_entity_at(pos)
        });
        
        // Click handling
        if response.clicked() {
            if let Some(entity_id) = self.interaction.hovered_entity {
                self.interaction.focus.set_focus(entity_id, &self.graph);
            } else {
                self.interaction.focus.clear_focus();
            }
        }
        
        // Double-click for investor expansion
        if response.double_clicked() {
            if let Some(group_idx) = self.find_investor_group_at(mouse_world.unwrap_or(Vec2::ZERO)) {
                self.toggle_investor_group(group_idx);
            }
        }
        
        // Drag handling (pan)
        if response.dragged() {
            let delta = response.drag_delta();
            self.camera.pan(egui::vec2(delta.x, delta.y).into());
        }
        
        // Scroll handling (zoom)
        let scroll = ui.input(|i| i.scroll_delta.y);
        if scroll.abs() > 0.0 {
            let factor = 1.0 + scroll * 0.001;
            if let Some(cursor) = mouse_world {
                self.camera.zoom_at(cursor, factor);
            }
        }
        
        // Keyboard shortcuts
        ui.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                self.interaction.focus.clear_focus();
            }
            if i.key_pressed(egui::Key::F) {
                // Fit to view
                self.camera.fit_to_bounds(
                    self.graph.core.bounds,
                    response.rect.size().into(),
                    0.1,
                );
            }
            if i.key_pressed(egui::Key::Num1) {
                self.graph.active_view = ViewMode::KycUbo;
            }
            if i.key_pressed(egui::Key::Num2) {
                self.graph.active_view = ViewMode::Onboarding;
            }
        });
    }
    
    fn find_entity_at(&self, world_pos: Vec2) -> Option<Uuid> {
        for entity in &self.graph.core.entities {
            if let Some(pos) = self.graph.core.layout.get(&entity.entity_id) {
                let distance = (*pos - world_pos).length();
                if distance < entity.base_size() {
                    return Some(entity.entity_id);
                }
            }
        }
        None
    }
}
```

---

## 10. View Toggle UI

### 10.1 Top Bar

```rust
fn render_top_bar(ui: &mut egui::Ui, graph: &mut CbuGraph) {
    ui.horizontal(|ui| {
        // View mode toggle
        ui.label("View:");
        
        let kyc_selected = graph.active_view == ViewMode::KycUbo;
        if ui.selectable_label(kyc_selected, "● KYC/UBO").clicked() {
            graph.active_view = ViewMode::KycUbo;
        }
        
        let onb_selected = graph.active_view == ViewMode::Onboarding;
        if ui.selectable_label(onb_selected, "○ Onboarding").clicked() {
            graph.active_view = ViewMode::Onboarding;
        }
        
        ui.separator();
        
        // CBU info
        ui.label(&graph.cbu_name);
        ui.label(format!("({:?})", graph.cbu_category));
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Settings button
            if ui.button("⚙").clicked() {
                // Open settings
            }
            
            // Search
            ui.text_edit_singleline(&mut String::new());
            ui.label("🔍");
        });
    });
}
```

### 10.2 Status Bar

```rust
fn render_status_bar(ui: &mut egui::Ui, graph: &CbuGraph) {
    ui.horizontal(|ui| {
        // CBU name
        ui.label(&graph.cbu_name);
        
        ui.separator();
        
        // View-specific summary
        match graph.active_view {
            ViewMode::KycUbo => {
                if let Some(kyc) = &graph.kyc_overlay {
                    let verified = kyc.entity_kyc.values()
                        .filter(|k| k.kyc_status == KycStatus::Verified)
                        .count();
                    let total = kyc.entity_kyc.len();
                    
                    ui.label(format!("KYC: {}/{} verified", verified, total));
                    
                    let expiring = kyc.documents.iter()
                        .filter(|d| d.validity_status == ValidityStatus::ExpiringSoon)
                        .count();
                    if expiring > 0 {
                        ui.colored_label(
                            Color32::from_rgb(255, 152, 0),
                            format!("⚠ {} expiring", expiring),
                        );
                    }
                    
                    // Risk summary
                    let high_risk = kyc.entity_kyc.values()
                        .filter(|k| k.risk_rating >= RiskRating::High)
                        .count();
                    if high_risk > 0 {
                        ui.colored_label(
                            Color32::from_rgb(244, 67, 54),
                            format!("🔴 {} high risk", high_risk),
                        );
                    }
                }
            }
            ViewMode::Onboarding => {
                if let Some(onb) = &graph.onboarding_overlay {
                    let delivered = onb.services.iter()
                        .filter(|s| s.delivery_status == DeliveryStatus::Delivered)
                        .count();
                    let total = onb.services.len();
                    
                    ui.label(format!("Services: {}/{} delivered", delivered, total));
                    
                    let active = onb.resources.iter()
                        .filter(|r| r.status == ResourceStatus::Active)
                        .count();
                    ui.label(format!("Resources: {} active", active));
                    
                    let pending = onb.services.iter()
                        .filter(|s| s.delivery_status == DeliveryStatus::Pending)
                        .count();
                    if pending > 0 {
                        ui.colored_label(
                            Color32::from_rgb(255, 193, 7),
                            format!("⏳ {} pending", pending),
                        );
                    }
                }
            }
        }
    });
}
```

---

## 11. Data Loading

### 11.1 Database Queries

```sql
-- Core graph: entities and their roles in this CBU
CREATE OR REPLACE VIEW "ob-poc".v_cbu_graph_entities AS
SELECT 
    cer.cbu_id,
    e.entity_id,
    e.name,
    et.type_code as entity_type,
    e.jurisdiction,
    array_agg(DISTINCT er.role_code) as roles,
    (array_agg(er.role_code ORDER BY 
        CASE er.role_code
            WHEN 'UBO' THEN 100
            WHEN 'SHAREHOLDER' THEN 90
            WHEN 'DIRECTOR' THEN 70
            ELSE 50
        END DESC
    ))[1] as primary_role,
    c.commercial_client_entity_id = e.entity_id as is_commercial_client
FROM "ob-poc".cbu_entity_roles cer
JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
JOIN "ob-poc".entity_roles er ON cer.role_id = er.role_id
JOIN "ob-poc".cbus c ON cer.cbu_id = c.cbu_id
GROUP BY cer.cbu_id, e.entity_id, e.name, et.type_code, e.jurisdiction, 
         c.commercial_client_entity_id;

-- Ownership edges
CREATE OR REPLACE VIEW "ob-poc".v_cbu_graph_ownership AS
SELECT 
    c.cbu_id,
    ol.parent_entity_id as from_entity,
    ol.child_entity_id as to_entity,
    ol.ownership_percentage,
    ol.ownership_type,
    ol.share_class_id
FROM "ob-poc".ownership_links ol
JOIN "ob-poc".cbu_entity_roles cer_from ON ol.parent_entity_id = cer_from.entity_id
JOIN "ob-poc".cbu_entity_roles cer_to ON ol.child_entity_id = cer_to.entity_id
JOIN "ob-poc".cbus c ON cer_from.cbu_id = c.cbu_id AND cer_to.cbu_id = c.cbu_id;

-- KYC overlay data
CREATE OR REPLACE VIEW "ob-poc".v_cbu_kyc_overlay AS
SELECT 
    eks.cbu_id,
    eks.entity_id,
    eks.kyc_status,
    eks.risk_rating,
    eks.last_verified_at,
    eks.next_review_date,
    COALESCE(ubo.is_ubo, false) as is_ubo,
    ubo.aggregate_pct
FROM "ob-poc".entity_kyc_status eks
LEFT JOIN (
    SELECT entity_id, true as is_ubo, SUM(ownership_percentage) as aggregate_pct
    FROM "ob-poc".ownership_links
    WHERE ownership_type = 'BENEFICIAL'
    GROUP BY entity_id
    HAVING SUM(ownership_percentage) >= 25
) ubo ON eks.entity_id = ubo.entity_id;

-- Documents for KYC overlay
CREATE OR REPLACE VIEW "ob-poc".v_cbu_documents_overlay AS
SELECT 
    dc.cbu_id,
    dc.document_id,
    dc.entity_id,
    dt.type_code as document_type,
    dt.category,
    dc.status,
    dc.expiry_date,
    CASE 
        WHEN dc.expiry_date < CURRENT_DATE THEN 'EXPIRED'
        WHEN dc.expiry_date < CURRENT_DATE + INTERVAL '30 days' THEN 'EXPIRING_SOON'
        WHEN dc.status = 'VERIFIED' THEN 'VALID'
        ELSE 'PENDING'
    END as validity_status
FROM "ob-poc".document_catalog dc
JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id;

-- Onboarding overlay: services
CREATE OR REPLACE VIEW "ob-poc".v_cbu_services_overlay AS
SELECT 
    sdm.cbu_id,
    sdm.service_id,
    s.service_name,
    s.service_type,
    sdm.product_id,
    p.product_name,
    sdm.delivery_status
FROM "ob-poc".service_delivery_map sdm
JOIN "ob-poc".services s ON sdm.service_id = s.service_id
JOIN "ob-poc".products p ON sdm.product_id = p.product_id;

-- Onboarding overlay: resources
CREATE OR REPLACE VIEW "ob-poc".v_cbu_resources_overlay AS
SELECT 
    cri.cbu_id,
    cri.instance_id,
    rt.type_name as resource_type,
    cri.external_id,
    cri.status,
    cri.service_id
FROM "ob-poc".cbu_resource_instances cri
JOIN "ob-poc".resource_types rt ON cri.resource_type_id = rt.type_id;

-- Investor aggregation
CREATE OR REPLACE VIEW "ob-poc".v_cbu_investor_summary AS
SELECT 
    cer.cbu_id,
    sc.class_id as share_class_id,
    sc.class_name as share_class_name,
    COUNT(DISTINCT cer.entity_id) as investor_count,
    SUM(ol.ownership_percentage) as total_ownership_pct,
    jsonb_object_agg(
        COALESCE(e.jurisdiction, 'UNKNOWN'),
        COUNT(*)
    ) as by_jurisdiction,
    jsonb_object_agg(
        COALESCE(eks.kyc_status, 'NOT_STARTED'),
        COUNT(*)
    ) as by_kyc_status
FROM "ob-poc".cbu_entity_roles cer
JOIN "ob-poc".entity_roles er ON cer.role_id = er.role_id
JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
LEFT JOIN "ob-poc".ownership_links ol ON cer.entity_id = ol.parent_entity_id
LEFT JOIN "ob-poc".share_classes sc ON ol.share_class_id = sc.class_id
LEFT JOIN "ob-poc".entity_kyc_status eks ON cer.entity_id = eks.entity_id 
    AND cer.cbu_id = eks.cbu_id
WHERE er.role_code = 'INVESTOR'
GROUP BY cer.cbu_id, sc.class_id, sc.class_name;
```

### 11.2 Rust Data Loading

```rust
impl CbuGraph {
    pub async fn load(cbu_id: Uuid, pool: &PgPool) -> Result<Self, Error> {
        // Load CBU metadata
        let cbu = sqlx::query_as!(CbuRow,
            "SELECT cbu_id, name, cbu_category, client_type, jurisdiction 
             FROM \"ob-poc\".cbus WHERE cbu_id = $1",
            cbu_id
        ).fetch_one(pool).await?;
        
        // Load template
        let template = CbuTemplate::for_category(cbu.cbu_category);
        
        // Load core entities
        let entities = sqlx::query_as!(EntityRow,
            "SELECT * FROM \"ob-poc\".v_cbu_graph_entities WHERE cbu_id = $1",
            cbu_id
        ).fetch_all(pool).await?;
        
        // Load ownership edges
        let ownership_edges = sqlx::query_as!(OwnershipRow,
            "SELECT * FROM \"ob-poc\".v_cbu_graph_ownership WHERE cbu_id = $1",
            cbu_id
        ).fetch_all(pool).await?;
        
        // Load investor groups
        let investor_groups = sqlx::query_as!(InvestorGroupRow,
            "SELECT * FROM \"ob-poc\".v_cbu_investor_summary WHERE cbu_id = $1",
            cbu_id
        ).fetch_all(pool).await?;
        
        // Build graph
        let mut graph = Self {
            cbu_id,
            cbu_name: cbu.name,
            cbu_category: cbu.cbu_category.parse()?,
            template,
            core: CoreGraph::from_rows(entities, ownership_edges),
            investor_groups: investor_groups.into_iter().map(Into::into).collect(),
            kyc_overlay: None,
            onboarding_overlay: None,
            active_view: ViewMode::KycUbo,
        };
        
        // Compute layout
        graph.compute_layout();
        
        // Load default overlay
        graph.kyc_overlay = Some(KycOverlay::load(cbu_id, pool).await?);
        
        Ok(graph)
    }
}

impl KycOverlay {
    pub async fn load(cbu_id: Uuid, pool: &PgPool) -> Result<Self, Error> {
        let entity_kyc = sqlx::query_as!(KycRow,
            "SELECT * FROM \"ob-poc\".v_cbu_kyc_overlay WHERE cbu_id = $1",
            cbu_id
        ).fetch_all(pool).await?;
        
        let documents = sqlx::query_as!(DocRow,
            "SELECT * FROM \"ob-poc\".v_cbu_documents_overlay WHERE cbu_id = $1",
            cbu_id
        ).fetch_all(pool).await?;
        
        Ok(Self {
            entity_kyc: entity_kyc.into_iter()
                .map(|r| (r.entity_id, r.into()))
                .collect(),
            documents: documents.into_iter().map(Into::into).collect(),
            entity_doc_edges: vec![],  // Build from documents
            alerts: vec![],
        })
    }
}
```

---

## 12. wgpu Graduation Path

If egui's `Painter` becomes a bottleneck (>5K nodes, <30fps):

### 12.1 When to Graduate

| Symptom | Threshold | Action |
|---------|-----------|--------|
| Frame time | >33ms (30fps) | Profile first |
| Node count | >5,000 | Consider wgpu |
| Edge count | >10,000 | Consider wgpu |
| Zoom lag | Noticeable | Batch rendering |

### 12.2 Migration Path

```rust
// Step 1: Abstract rendering behind trait
pub trait GraphRenderer {
    fn render_node(&mut self, pos: Vec2, shape: Shape, style: &NodeStyle);
    fn render_edge(&mut self, curve: &BezierCurve, style: &EdgeStyle);
    fn render_text(&mut self, pos: Vec2, text: &str, style: &TextStyle);
}

// Step 2: egui implementation (current)
pub struct EguiRenderer<'a> {
    painter: &'a egui::Painter,
    to_screen: Transform,
}

impl GraphRenderer for EguiRenderer<'_> {
    fn render_node(&mut self, pos: Vec2, shape: Shape, style: &NodeStyle) {
        // Current implementation
    }
}

// Step 3: wgpu implementation (future)
pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    node_pipeline: wgpu::RenderPipeline,
    edge_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
}

impl GraphRenderer for WgpuRenderer {
    fn render_node(&mut self, pos: Vec2, shape: Shape, style: &NodeStyle) {
        // Add to instance buffer, batch render
    }
}
```

### 12.3 wgpu Benefits

- **Instanced rendering**: 10K nodes in 1 draw call
- **GPU edge rendering**: Bezier curves in shader
- **MSAA**: Smooth curves without CPU overdraw
- **Compute shaders**: Force simulation on GPU

---

## 13. File Structure

```
src/
├── visualization/
│   ├── mod.rs
│   ├── graph/
│   │   ├── mod.rs
│   │   ├── data.rs           // CbuGraph, CoreGraph, overlays
│   │   ├── layout.rs         // Layout algorithm
│   │   ├── template.rs       // CbuTemplate, slots
│   │   └── clustering.rs     // Clustering forces
│   ├── render/
│   │   ├── mod.rs
│   │   ├── node.rs           // Entity rendering
│   │   ├── edge.rs           // Edge routing and rendering
│   │   ├── overlay_kyc.rs    // KYC view rendering
│   │   ├── overlay_onb.rs    // Onboarding view rendering
│   │   ├── investor.rs       // Investor group rendering
│   │   └── focus.rs          // Focus card rendering
│   ├── interaction/
│   │   ├── mod.rs
│   │   ├── camera.rs         // Camera2D, zoom, pan
│   │   ├── input.rs          // Mouse, keyboard handling
│   │   └── focus.rs          // Focus state management
│   ├── widget.rs             // CbuGraphWidget
│   └── colors.rs             // Color palette
├── db/
│   └── visualization_queries.rs  // SQL queries
└── templates/
    ├── lux_sicav.yaml
    ├── cayman_master_feeder.yaml
    └── family_trust.yaml
```

---

## 14. Summary

| Aspect | Decision |
|--------|----------|
| **Technology** | egui + custom Painter (graduate to wgpu if needed) |
| **Views** | Two overlays (KYC/UBO, Onboarding) on shared core graph |
| **View switching** | Instant (one frame), core entities don't move |
| **Layout** | Template-driven slots + force refinement |
| **Investors** | Aggregated by share class, expand on click/zoom |
| **Edges** | Bezier curves, hop over intersections |
| **LOD** | 6 levels from Micro to Focused |
| **Focus** | Click entity → blur background → show card |
| **Data** | PostgreSQL views, async Rust loading |
