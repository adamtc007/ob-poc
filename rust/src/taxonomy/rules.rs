//! Membership rules and taxonomy context
//!
//! Rules define HOW to build a taxonomy - what entities to include,
//! which edges to traverse, how to group children.
//!
//! Context defines WHAT taxonomy to build - universe, book, single CBU, etc.
//!
//! ## Config-Driven Approach
//!
//! Edge types and view applicability are now driven by database configuration
//! in `ob-poc.edge_types` and `ob-poc.view_modes` tables. Use
//! `MembershipRules::from_view_config()` to build rules from database config
//! instead of hardcoded methods like `cbu_ubo()`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::Filter;

#[cfg(feature = "database")]
use crate::database::view_config_service::ViewConfigService;
#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// TAXONOMY CONTEXT - What to build
// =============================================================================

/// Context determines which taxonomy to build
/// This is the "scope" of what the user is looking at
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaxonomyContext {
    /// All CBUs the user can see (filtered by access)
    Universe,

    /// All CBUs for a commercial client (book view)
    Book { client_id: Uuid },

    /// Single CBU - trading/service view (roles, products, services)
    CbuTrading { cbu_id: Uuid },

    /// Single CBU - UBO ownership view (ownership chains to natural persons)
    CbuUbo { cbu_id: Uuid },

    /// Single CBU - KYC case view (workstreams, documents, screenings)
    CbuKyc { cbu_id: Uuid, case_id: Option<Uuid> },

    /// Entity forest - entities matching filters, grouped by type/ownership
    EntityForest { filters: Vec<Filter> },

    /// Custom context with explicit rules
    Custom { rules: Box<MembershipRules> },
}

impl TaxonomyContext {
    /// Build membership rules from this context (sync, uses hardcoded rules)
    ///
    /// **Deprecated**: Use `to_rules_from_config()` instead for config-driven
    /// edge types from database.
    pub fn to_rules(&self) -> MembershipRules {
        match self {
            TaxonomyContext::Universe => MembershipRules::universe(),
            TaxonomyContext::Book { client_id } => MembershipRules::book(*client_id),
            TaxonomyContext::CbuTrading { cbu_id } => MembershipRules::cbu_trading(*cbu_id),
            TaxonomyContext::CbuUbo { cbu_id } => MembershipRules::cbu_ubo(*cbu_id),
            TaxonomyContext::CbuKyc { cbu_id, case_id } => {
                MembershipRules::cbu_kyc(*cbu_id, *case_id)
            }
            TaxonomyContext::EntityForest { filters } => MembershipRules::entity_forest(filters),
            TaxonomyContext::Custom { rules } => (**rules).clone(),
        }
    }

    /// Human-readable description of this context
    pub fn description(&self) -> String {
        match self {
            TaxonomyContext::Universe => "All CBUs".into(),
            TaxonomyContext::Book { .. } => "Client book".into(),
            TaxonomyContext::CbuTrading { .. } => "CBU trading view".into(),
            TaxonomyContext::CbuUbo { .. } => "CBU ownership view".into(),
            TaxonomyContext::CbuKyc { .. } => "CBU KYC view".into(),
            TaxonomyContext::EntityForest { .. } => "Entity forest".into(),
            TaxonomyContext::Custom { .. } => "Custom view".into(),
        }
    }

    /// Get the view mode code for this context
    pub fn view_mode_code(&self) -> &'static str {
        match self {
            TaxonomyContext::Universe => "UNIVERSE",
            TaxonomyContext::Book { .. } => "BOOK",
            TaxonomyContext::CbuTrading { .. } => "TRADING",
            TaxonomyContext::CbuUbo { .. } => "KYC_UBO",
            TaxonomyContext::CbuKyc { .. } => "KYC",
            TaxonomyContext::EntityForest { .. } => "ENTITY_FOREST",
            TaxonomyContext::Custom { .. } => "CUSTOM",
        }
    }

    /// Get the CBU ID if this context is for a single CBU
    pub fn cbu_id(&self) -> Option<Uuid> {
        match self {
            TaxonomyContext::CbuTrading { cbu_id } => Some(*cbu_id),
            TaxonomyContext::CbuUbo { cbu_id } => Some(*cbu_id),
            TaxonomyContext::CbuKyc { cbu_id, .. } => Some(*cbu_id),
            _ => None,
        }
    }
}

// =============================================================================
// CONFIG-DRIVEN TAXONOMY CONTEXT (Database Feature)
// =============================================================================

#[cfg(feature = "database")]
impl TaxonomyContext {
    /// Build membership rules from database configuration
    ///
    /// This is the **recommended** way to get rules from a context. It queries
    /// the `edge_types` and `view_modes` tables to determine which edges
    /// to traverse based on the context's view mode.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ctx = TaxonomyContext::CbuUbo { cbu_id };
    /// let rules = ctx.to_rules_from_config(&pool).await?;
    /// ```
    pub async fn to_rules_from_config(&self, pool: &PgPool) -> Result<MembershipRules> {
        match self {
            TaxonomyContext::Custom { rules } => Ok((**rules).clone()),
            _ => {
                let view_mode = self.view_mode_code();
                let cbu_id = self.cbu_id();
                MembershipRules::from_view_config(pool, view_mode, cbu_id).await
            }
        }
    }
}

// =============================================================================
// MEMBERSHIP RULES - How to build
// =============================================================================

/// Membership rules - compiled from context
/// Defines what entities to include and how to structure them
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipRules {
    /// Root entity filter - where to start
    pub root_filter: RootFilter,

    /// Which entities to include as nodes
    pub entity_filter: EntityFilter,

    /// Which relationship types to traverse
    pub edge_types: Vec<EdgeType>,

    /// How to group/nest children
    pub grouping: GroupingStrategy,

    /// Traversal direction
    pub direction: TraversalDirection,

    /// When to stop traversal
    pub terminus: TerminusCondition,

    /// Maximum depth to traverse
    pub max_depth: u32,

    /// Whether to include products/services as nodes
    pub include_products: bool,

    /// Whether to include documents as nodes
    pub include_documents: bool,
}

impl Default for MembershipRules {
    fn default() -> Self {
        Self {
            root_filter: RootFilter::AllCbus,
            entity_filter: EntityFilter::All,
            edge_types: vec![EdgeType::CbuRole],
            grouping: GroupingStrategy::None,
            direction: TraversalDirection::Down,
            terminus: TerminusCondition::MaxDepth,
            max_depth: 5,
            include_products: false,
            include_documents: false,
        }
    }
}

impl MembershipRules {
    /// Universe view - all CBUs grouped by jurisdiction
    pub fn universe() -> Self {
        Self {
            root_filter: RootFilter::AllCbus,
            entity_filter: EntityFilter::CbusOnly,
            edge_types: vec![],
            grouping: GroupingStrategy::ByDimension(Dimension::Jurisdiction),
            direction: TraversalDirection::Down,
            terminus: TerminusCondition::MaxDepth,
            max_depth: 2, // Just clusters and CBUs
            include_products: false,
            include_documents: false,
        }
    }

    /// Book view - all CBUs for a client
    pub fn book(client_id: Uuid) -> Self {
        Self {
            root_filter: RootFilter::Client { client_id },
            entity_filter: EntityFilter::CbusOnly,
            edge_types: vec![],
            grouping: GroupingStrategy::ByDimension(Dimension::FundType),
            direction: TraversalDirection::Down,
            terminus: TerminusCondition::MaxDepth,
            max_depth: 2,
            include_products: true,
            include_documents: false,
        }
    }

    /// CBU trading view - roles, products, services
    ///
    /// **Deprecated**: Use `from_view_config(pool, "TRADING", cbu_id)` instead
    /// for config-driven edge types from database.
    pub fn cbu_trading(cbu_id: Uuid) -> Self {
        Self {
            root_filter: RootFilter::SingleCbu { cbu_id },
            entity_filter: EntityFilter::All,
            // Config-driven: these should come from edge_types WHERE show_in_trading_view = true
            edge_types: vec![
                EdgeType::CbuRole,
                EdgeType::CbuHasTradingProfile,
                EdgeType::Control,
                EdgeType::BoardMember,
                EdgeType::TrustTrustee,
                EdgeType::FundManagedBy,
                EdgeType::InvestsInVehicle,
                EdgeType::EntityAuthorizesTrading,
                EdgeType::TradingProfileHasMatrix,
            ],
            grouping: GroupingStrategy::ByRole,
            direction: TraversalDirection::Down,
            terminus: TerminusCondition::MaxDepth,
            max_depth: 3,
            include_products: true,
            include_documents: false,
        }
    }

    /// CBU UBO view - ownership chains to natural persons
    ///
    /// **Deprecated**: Use `from_view_config(pool, "UBO", cbu_id)` instead
    /// for config-driven edge types from database.
    pub fn cbu_ubo(cbu_id: Uuid) -> Self {
        Self {
            root_filter: RootFilter::SingleCbu { cbu_id },
            entity_filter: EntityFilter::All,
            // Config-driven: these should come from edge_types WHERE show_in_ubo_view = true
            edge_types: vec![
                EdgeType::Ownership,
                EdgeType::IndirectOwnership,
                EdgeType::Control,
                EdgeType::BoardMember,
                EdgeType::TrustSettlor,
                EdgeType::TrustTrustee,
                EdgeType::TrustBeneficiary,
                EdgeType::TrustProtector,
                EdgeType::CbuRole,
            ],
            grouping: GroupingStrategy::ByOwnership,
            direction: TraversalDirection::Up, // Trace UP to natural persons
            terminus: TerminusCondition::NaturalPerson,
            max_depth: 10, // UBO chains can be deep
            include_products: false,
            include_documents: false,
        }
    }

    /// CBU KYC view - case, workstreams, documents, screenings
    ///
    /// **Deprecated**: Use `from_view_config(pool, "KYC", cbu_id)` instead
    /// for config-driven edge types from database.
    pub fn cbu_kyc(cbu_id: Uuid, _case_id: Option<Uuid>) -> Self {
        Self {
            root_filter: RootFilter::SingleCbu { cbu_id },
            entity_filter: EntityFilter::All,
            edge_types: vec![
                EdgeType::CbuRole,
                EdgeType::HasWorkstream,
                EdgeType::HasDocument,
            ],
            grouping: GroupingStrategy::ByWorkstream,
            direction: TraversalDirection::Down,
            terminus: TerminusCondition::MaxDepth,
            max_depth: 4,
            include_products: false,
            include_documents: true,
        }
    }

    /// Entity forest - entities matching filters
    pub fn entity_forest(filters: &[Filter]) -> Self {
        Self {
            root_filter: RootFilter::Entities {
                filters: filters.to_vec(),
            },
            entity_filter: EntityFilter::ByFilters(filters.to_vec()),
            edge_types: vec![EdgeType::Ownership, EdgeType::Control],
            grouping: GroupingStrategy::ByDimension(Dimension::EntityType),
            direction: TraversalDirection::Both,
            terminus: TerminusCondition::MaxDepth,
            max_depth: 5,
            include_products: false,
            include_documents: false,
        }
    }
}

// =============================================================================
// CONFIG-DRIVEN MEMBERSHIP RULES (Database Feature)
// =============================================================================

#[cfg(feature = "database")]
impl MembershipRules {
    /// Build membership rules from database view configuration
    ///
    /// This is the **recommended** way to create MembershipRules. It queries
    /// the `edge_types` and `view_modes` tables to determine which edges
    /// to traverse based on the view mode.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    /// * `view_mode` - View mode code (e.g., "UBO", "TRADING", "SERVICE")
    /// * `cbu_id` - CBU ID for single-CBU views
    ///
    /// # Example
    ///
    /// ```ignore
    /// let rules = MembershipRules::from_view_config(&pool, "UBO", Some(cbu_id)).await?;
    /// ```
    pub async fn from_view_config(
        pool: &PgPool,
        view_mode: &str,
        cbu_id: Option<Uuid>,
    ) -> Result<Self> {
        // Get edge types applicable to this view mode
        let edge_configs = ViewConfigService::get_view_edge_types(pool, view_mode).await?;

        // Convert to EdgeType enum, filtering out unknown codes
        let edge_types: Vec<EdgeType> = edge_configs
            .iter()
            .filter_map(|ec| EdgeType::from_code(&ec.edge_type_code))
            .collect();

        // Get view mode configuration for traversal direction and other settings
        let view_config = ViewConfigService::get_view_mode_config(pool, view_mode).await?;

        // Determine traversal direction from config
        let direction = view_config
            .as_ref()
            .and_then(|vc| vc.primary_traversal_direction.as_ref())
            .map(|d| match d.as_str() {
                "UP" => TraversalDirection::Up,
                "DOWN" => TraversalDirection::Down,
                "BOTH" => TraversalDirection::Both,
                _ => TraversalDirection::Down,
            })
            .unwrap_or(TraversalDirection::Down);

        // Determine root filter based on cbu_id
        let root_filter = match cbu_id {
            Some(id) => RootFilter::SingleCbu { cbu_id: id },
            None => RootFilter::AllCbus,
        };

        // Determine grouping and terminus based on view mode
        let (grouping, terminus, max_depth, include_products, include_documents) = match view_mode {
            "UBO" | "KYC_UBO" => (
                GroupingStrategy::ByOwnership,
                TerminusCondition::NaturalPerson,
                10,
                false,
                false,
            ),
            "TRADING" => (
                GroupingStrategy::ByRole,
                TerminusCondition::MaxDepth,
                5,
                true,
                false,
            ),
            "SERVICE" | "SERVICE_DELIVERY" => (
                GroupingStrategy::None,
                TerminusCondition::MaxDepth,
                4,
                true,
                false,
            ),
            "PRODUCT" | "PRODUCTS_ONLY" => (
                GroupingStrategy::None,
                TerminusCondition::MaxDepth,
                3,
                true,
                false,
            ),
            "FUND_STRUCTURE" => (
                GroupingStrategy::ByDimension(Dimension::FundType),
                TerminusCondition::MaxDepth,
                5,
                false,
                false,
            ),
            "KYC" => (
                GroupingStrategy::ByWorkstream,
                TerminusCondition::MaxDepth,
                4,
                false,
                true,
            ),
            _ => (
                GroupingStrategy::None,
                TerminusCondition::MaxDepth,
                5,
                false,
                false,
            ),
        };

        Ok(Self {
            root_filter,
            entity_filter: EntityFilter::All,
            edge_types,
            grouping,
            direction,
            terminus,
            max_depth,
            include_products,
            include_documents,
        })
    }

    /// Build membership rules from database config with full customization
    ///
    /// This method allows overriding specific settings while still using
    /// database-driven edge types.
    pub async fn from_view_config_with_overrides(
        pool: &PgPool,
        view_mode: &str,
        cbu_id: Option<Uuid>,
        max_depth_override: Option<u32>,
        include_products_override: Option<bool>,
        include_documents_override: Option<bool>,
    ) -> Result<Self> {
        let mut rules = Self::from_view_config(pool, view_mode, cbu_id).await?;

        if let Some(depth) = max_depth_override {
            rules.max_depth = depth;
        }
        if let Some(products) = include_products_override {
            rules.include_products = products;
        }
        if let Some(documents) = include_documents_override {
            rules.include_documents = documents;
        }

        Ok(rules)
    }

    /// Get edge types from database for a view mode
    ///
    /// Utility method to just get the edge types without building full rules.
    pub async fn get_edge_types_for_view(pool: &PgPool, view_mode: &str) -> Result<Vec<EdgeType>> {
        let edge_configs = ViewConfigService::get_view_edge_types(pool, view_mode).await?;
        let edge_types: Vec<EdgeType> = edge_configs
            .iter()
            .filter_map(|ec| EdgeType::from_code(&ec.edge_type_code))
            .collect();
        Ok(edge_types)
    }
}

// =============================================================================
// SUPPORTING TYPES
// =============================================================================

/// Where to start building the tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RootFilter {
    /// All CBUs (filtered by user access)
    AllCbus,
    /// CBUs for a specific client
    Client { client_id: Uuid },
    /// A single CBU
    SingleCbu { cbu_id: Uuid },
    /// Entities matching filters
    Entities { filters: Vec<Filter> },
}

/// Which entities to include
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityFilter {
    /// All entities
    All,
    /// Only CBUs (no child entities)
    CbusOnly,
    /// Entities matching filters
    ByFilters(Vec<Filter>),
    /// Specific entity types
    ByType(Vec<String>),
}

/// Relationship types to traverse
///
/// These map to `edge_type_code` values in the `ob-poc.edge_types` table.
/// Use `EdgeType::from_code()` to convert database codes to enum variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    // =========================================================================
    // Ownership & Control (UBO view)
    // =========================================================================
    /// Direct ownership relationship (OWNERSHIP)
    Ownership,
    /// Indirect/beneficial ownership (INDIRECT_OWNERSHIP)
    IndirectOwnership,
    /// Control without ownership (CONTROL)
    Control,
    /// Board member relationship (BOARD_MEMBER)
    BoardMember,

    // =========================================================================
    // Trust Relationships (UBO view)
    // =========================================================================
    /// Trust settlor (TRUST_SETTLOR)
    TrustSettlor,
    /// Trust trustee (TRUST_TRUSTEE)
    TrustTrustee,
    /// Trust beneficiary (TRUST_BENEFICIARY)
    TrustBeneficiary,
    /// Trust protector (TRUST_PROTECTOR)
    TrustProtector,

    // =========================================================================
    // CBU Relationships
    // =========================================================================
    /// CBU -> Entity via role (CBU_ROLE)
    CbuRole,
    /// CBU -> Product (CBU_USES_PRODUCT)
    CbuUsesProduct,
    /// CBU -> Trading Profile (CBU_HAS_TRADING_PROFILE)
    CbuHasTradingProfile,

    // =========================================================================
    // Fund Structure
    // =========================================================================
    /// Umbrella -> Subfund (UMBRELLA_CONTAINS_SUBFUND)
    UmbrellaContainsSubfund,
    /// Fund -> Share Class (FUND_HAS_SHARE_CLASS)
    FundHasShareClass,
    /// Feeder -> Master (FEEDER_TO_MASTER)
    FeederToMaster,
    /// Fund -> Management Company (FUND_MANAGED_BY)
    FundManagedBy,
    /// Investment vehicle relationship (INVESTS_IN_VEHICLE)
    InvestsInVehicle,

    // =========================================================================
    // Trading (Trading view)
    // =========================================================================
    /// Entity authorizes trading (ENTITY_AUTHORIZES_TRADING)
    EntityAuthorizesTrading,
    /// Trading profile -> matrix (TRADING_PROFILE_HAS_MATRIX)
    TradingProfileHasMatrix,

    // =========================================================================
    // Service Delivery (Service view)
    // =========================================================================
    /// Product -> Service (PRODUCT_PROVIDES_SERVICE)
    ProductProvidesService,
    /// Service -> Resource (SERVICE_USES_RESOURCE)
    ServiceUsesResource,

    // =========================================================================
    // KYC/Documents (legacy - for backward compat)
    // =========================================================================
    /// Entity -> Workstream
    HasWorkstream,
    /// Workstream/Entity -> Document
    HasDocument,
    /// Entity -> Screening
    HasScreening,

    // =========================================================================
    // Wildcards
    // =========================================================================
    /// Any relationship (used for filters)
    Any,
}

impl EdgeType {
    /// Convert database edge_type_code to EdgeType enum
    ///
    /// Returns None for unrecognized codes.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            // Ownership & Control
            "OWNERSHIP" => Some(Self::Ownership),
            "INDIRECT_OWNERSHIP" => Some(Self::IndirectOwnership),
            "CONTROL" => Some(Self::Control),
            "BOARD_MEMBER" => Some(Self::BoardMember),

            // Trust
            "TRUST_SETTLOR" => Some(Self::TrustSettlor),
            "TRUST_TRUSTEE" => Some(Self::TrustTrustee),
            "TRUST_BENEFICIARY" => Some(Self::TrustBeneficiary),
            "TRUST_PROTECTOR" => Some(Self::TrustProtector),

            // CBU
            "CBU_ROLE" => Some(Self::CbuRole),
            "CBU_USES_PRODUCT" => Some(Self::CbuUsesProduct),
            "CBU_HAS_TRADING_PROFILE" => Some(Self::CbuHasTradingProfile),

            // Fund Structure
            "UMBRELLA_CONTAINS_SUBFUND" => Some(Self::UmbrellaContainsSubfund),
            "FUND_HAS_SHARE_CLASS" => Some(Self::FundHasShareClass),
            "FEEDER_TO_MASTER" => Some(Self::FeederToMaster),
            "FUND_MANAGED_BY" => Some(Self::FundManagedBy),
            "INVESTS_IN_VEHICLE" => Some(Self::InvestsInVehicle),

            // Trading
            "ENTITY_AUTHORIZES_TRADING" => Some(Self::EntityAuthorizesTrading),
            "TRADING_PROFILE_HAS_MATRIX" => Some(Self::TradingProfileHasMatrix),

            // Service Delivery
            "PRODUCT_PROVIDES_SERVICE" => Some(Self::ProductProvidesService),
            "SERVICE_USES_RESOURCE" => Some(Self::ServiceUsesResource),

            // Unknown
            _ => None,
        }
    }

    /// Convert EdgeType enum to database edge_type_code
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::Ownership => "OWNERSHIP",
            Self::IndirectOwnership => "INDIRECT_OWNERSHIP",
            Self::Control => "CONTROL",
            Self::BoardMember => "BOARD_MEMBER",
            Self::TrustSettlor => "TRUST_SETTLOR",
            Self::TrustTrustee => "TRUST_TRUSTEE",
            Self::TrustBeneficiary => "TRUST_BENEFICIARY",
            Self::TrustProtector => "TRUST_PROTECTOR",
            Self::CbuRole => "CBU_ROLE",
            Self::CbuUsesProduct => "CBU_USES_PRODUCT",
            Self::CbuHasTradingProfile => "CBU_HAS_TRADING_PROFILE",
            Self::UmbrellaContainsSubfund => "UMBRELLA_CONTAINS_SUBFUND",
            Self::FundHasShareClass => "FUND_HAS_SHARE_CLASS",
            Self::FeederToMaster => "FEEDER_TO_MASTER",
            Self::FundManagedBy => "FUND_MANAGED_BY",
            Self::InvestsInVehicle => "INVESTS_IN_VEHICLE",
            Self::EntityAuthorizesTrading => "ENTITY_AUTHORIZES_TRADING",
            Self::TradingProfileHasMatrix => "TRADING_PROFILE_HAS_MATRIX",
            Self::ProductProvidesService => "PRODUCT_PROVIDES_SERVICE",
            Self::ServiceUsesResource => "SERVICE_USES_RESOURCE",
            // Legacy KYC edges don't have database codes yet
            Self::HasWorkstream => "HAS_WORKSTREAM",
            Self::HasDocument => "HAS_DOCUMENT",
            Self::HasScreening => "HAS_SCREENING",
            Self::Any => "ANY",
        }
    }

    /// Check if this edge type represents ownership
    pub fn is_ownership(&self) -> bool {
        matches!(self, Self::Ownership | Self::IndirectOwnership)
    }

    /// Check if this edge type represents control
    pub fn is_control(&self) -> bool {
        matches!(self, Self::Control | Self::BoardMember)
    }

    /// Check if this edge type is a trust relationship
    pub fn is_trust(&self) -> bool {
        matches!(
            self,
            Self::TrustSettlor | Self::TrustTrustee | Self::TrustBeneficiary | Self::TrustProtector
        )
    }
}

/// How to group children
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GroupingStrategy {
    /// No grouping - flat children
    None,
    /// Group by dimension (jurisdiction, fund_type, etc.)
    ByDimension(Dimension),
    /// Group by role category
    ByRole,
    /// Group by ownership chain
    ByOwnership,
    /// Group by workstream
    ByWorkstream,
}

/// Dimensions for grouping
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Dimension {
    Jurisdiction,
    FundType,
    ClientType,
    EntityType,
    Status,
    RoleCategory,
}

/// Traversal direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraversalDirection {
    /// From root to leaves (normal tree)
    Down,
    /// From target to root (UBO chain)
    Up,
    /// Both directions
    Both,
}

/// When to stop traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminusCondition {
    /// Stop at max_depth
    MaxDepth,
    /// Stop at natural persons (for UBO)
    NaturalPerson,
    /// Stop at public companies
    PublicCompany,
    /// Stop when no more parents
    NoMoreOwners,
    /// Custom predicate
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_universe_rules() {
        let rules = MembershipRules::universe();
        assert!(matches!(rules.root_filter, RootFilter::AllCbus));
        assert!(matches!(
            rules.grouping,
            GroupingStrategy::ByDimension(Dimension::Jurisdiction)
        ));
    }

    #[test]
    fn test_ubo_rules() {
        let rules = MembershipRules::cbu_ubo(Uuid::new_v4());
        assert_eq!(rules.direction, TraversalDirection::Up);
        assert!(matches!(rules.terminus, TerminusCondition::NaturalPerson));
        assert!(rules.edge_types.contains(&EdgeType::Ownership));
        assert!(rules.edge_types.contains(&EdgeType::Control));
        assert!(rules.edge_types.contains(&EdgeType::TrustBeneficiary));
    }

    #[test]
    fn test_trading_rules() {
        let rules = MembershipRules::cbu_trading(Uuid::new_v4());
        assert_eq!(rules.direction, TraversalDirection::Down);
        assert!(rules.edge_types.contains(&EdgeType::CbuRole));
        assert!(rules.edge_types.contains(&EdgeType::CbuHasTradingProfile));
        assert!(rules.include_products);
    }

    #[test]
    fn test_context_to_rules() {
        let ctx = TaxonomyContext::Universe;
        let rules = ctx.to_rules();
        assert!(matches!(rules.root_filter, RootFilter::AllCbus));
    }

    #[test]
    fn test_edge_type_from_code() {
        assert_eq!(EdgeType::from_code("OWNERSHIP"), Some(EdgeType::Ownership));
        assert_eq!(EdgeType::from_code("CONTROL"), Some(EdgeType::Control));
        assert_eq!(
            EdgeType::from_code("TRUST_BENEFICIARY"),
            Some(EdgeType::TrustBeneficiary)
        );
        assert_eq!(EdgeType::from_code("CBU_ROLE"), Some(EdgeType::CbuRole));
        assert_eq!(EdgeType::from_code("UNKNOWN"), None);
    }

    #[test]
    fn test_edge_type_to_code() {
        assert_eq!(EdgeType::Ownership.to_code(), "OWNERSHIP");
        assert_eq!(EdgeType::Control.to_code(), "CONTROL");
        assert_eq!(EdgeType::TrustBeneficiary.to_code(), "TRUST_BENEFICIARY");
        assert_eq!(EdgeType::CbuRole.to_code(), "CBU_ROLE");
    }

    #[test]
    fn test_edge_type_predicates() {
        assert!(EdgeType::Ownership.is_ownership());
        assert!(EdgeType::IndirectOwnership.is_ownership());
        assert!(!EdgeType::Control.is_ownership());

        assert!(EdgeType::Control.is_control());
        assert!(EdgeType::BoardMember.is_control());
        assert!(!EdgeType::Ownership.is_control());

        assert!(EdgeType::TrustSettlor.is_trust());
        assert!(EdgeType::TrustTrustee.is_trust());
        assert!(EdgeType::TrustBeneficiary.is_trust());
        assert!(EdgeType::TrustProtector.is_trust());
        assert!(!EdgeType::Ownership.is_trust());
    }
}
