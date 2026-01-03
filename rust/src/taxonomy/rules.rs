//! Membership rules and taxonomy context
//!
//! Rules define HOW to build a taxonomy - what entities to include,
//! which edges to traverse, how to group children.
//!
//! Context defines WHAT taxonomy to build - universe, book, single CBU, etc.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::Filter;

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
    /// Build membership rules from this context
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
            edge_types: vec![EdgeType::HasRole],
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
    pub fn cbu_trading(cbu_id: Uuid) -> Self {
        Self {
            root_filter: RootFilter::SingleCbu { cbu_id },
            entity_filter: EntityFilter::All,
            edge_types: vec![
                EdgeType::HasRole,
                EdgeType::HasProduct,
                EdgeType::HasService,
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
    pub fn cbu_ubo(cbu_id: Uuid) -> Self {
        Self {
            root_filter: RootFilter::SingleCbu { cbu_id },
            entity_filter: EntityFilter::All,
            edge_types: vec![EdgeType::Owns, EdgeType::Controls, EdgeType::TrustRole],
            grouping: GroupingStrategy::ByOwnership,
            direction: TraversalDirection::Up, // Trace UP to natural persons
            terminus: TerminusCondition::NaturalPerson,
            max_depth: 10, // UBO chains can be deep
            include_products: false,
            include_documents: false,
        }
    }

    /// CBU KYC view - case, workstreams, documents, screenings
    pub fn cbu_kyc(cbu_id: Uuid, _case_id: Option<Uuid>) -> Self {
        Self {
            root_filter: RootFilter::SingleCbu { cbu_id },
            entity_filter: EntityFilter::All,
            edge_types: vec![
                EdgeType::HasRole,
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
            edge_types: vec![EdgeType::Owns, EdgeType::Controls],
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    /// CBU -> Entity via role
    HasRole,
    /// Entity -> Entity ownership
    Owns,
    /// Entity -> Entity control
    Controls,
    /// Trust relationships
    TrustRole,
    /// CBU -> Product subscription
    HasProduct,
    /// Product -> Service
    HasService,
    /// Entity -> Workstream
    HasWorkstream,
    /// Workstream/Entity -> Document
    HasDocument,
    /// Entity -> Screening
    HasScreening,
    /// Any relationship
    Any,
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
        assert!(rules.edge_types.contains(&EdgeType::Owns));
    }

    #[test]
    fn test_context_to_rules() {
        let ctx = TaxonomyContext::Universe;
        let rules = ctx.to_rules();
        assert!(matches!(rules.root_filter, RootFilter::AllCbus));
    }
}
