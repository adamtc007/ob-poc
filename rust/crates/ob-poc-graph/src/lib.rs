//! OB-POC Graph Widget
//!
//! This crate contains ONLY the graph widget - no API, no app shell.
//! Graph data structures and utilities for visualization.

pub mod config;
pub mod graph;

#[allow(deprecated)]
pub use graph::{
    // Cluster view (ManCo center + CBU orbital rings)
    cluster::ClusterCbuData,
    cluster::ClusterView,
    cluster::ManCoData,
    entity_matches_type,
    get_entities_for_type,
    // Trading Matrix (hierarchical custody config browser)
    render_node_detail_panel,
    // Service Taxonomy (Product → Service → Resource browser)
    render_service_detail_panel,
    render_service_taxonomy,
    render_trading_matrix_browser,
    render_type_browser,
    // Astronomy view (universe/solar system transitions)
    AstronomyView,
    // Core graph types
    CbuGraphData,
    CbuGraphWidget,
    // Galaxy view (cluster visualization with force simulation)
    ClusterData,
    ClusterNode,
    ClusterType,
    // Ontology (type hierarchy browser)
    EntityTypeOntology,
    ForceConfig,
    ForceSimulation,
    GalaxyAction, // deprecated - use NavigationAction from ob_poc_types::galaxy instead
    GalaxyView,
    GraphEdgeData,
    GraphNodeData,
    // Navigation actions
    GraphWidgetAction,
    IntentData,
    NavigateBackAction,
    NavigationEntry,
    ProductData,
    ResourceData,
    RiskSummary,
    ServiceData,
    ServiceStatus,
    ServiceTaxonomy,
    ServiceTaxonomyAction,
    ServiceTaxonomyNode,
    ServiceTaxonomyNodeId,
    ServiceTaxonomyNodeType,
    ServiceTaxonomyState,
    ServiceTaxonomyStats,
    // Animation
    SpringConfig,
    SpringF32,
    SpringVec2,
    TaxonomyState,
    TradingMatrix,
    TradingMatrixAction,
    TradingMatrixMetadata,
    TradingMatrixNode,
    TradingMatrixNodeId,
    TradingMatrixNodeIdExt,
    TradingMatrixNodeType,
    TradingMatrixResponse,
    TradingMatrixState,
    TransitionAction,
    TypeBrowserAction,
    TypeNode,
    ViewMode,
    ViewTransition,
};

// Re-export galaxy types from ob-poc-types for convenience
pub use ob_poc_types::galaxy::AgentMode;

// Re-export investor register types from ob-poc-types for convenience
pub use ob_poc_types::investor_register::{
    AggregateBreakdown, AggregateInvestorsNode, BreakdownDimension, ControlHolderNode, ControlTier,
    InvestorFilters, InvestorListItem, InvestorListResponse, InvestorRegisterView,
    IssuerSummary as InvestorIssuerSummary, PaginationInfo, ThresholdConfig,
};

// Re-export Esper render state types for NavigationVerb handlers
pub use graph::viewport::{EsperRenderState, GapType, IlluminateAspect, RedFlagCategory};
