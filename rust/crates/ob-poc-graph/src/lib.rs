//! OB-POC Graph Widget
//!
//! This crate contains ONLY the graph widget - no API, no app shell.
//! The widget is used by ob-poc-ui which owns the API and app lifecycle.

pub mod graph;

pub use graph::{
    entity_matches_type,
    get_entities_for_type,
    // Trading Matrix (hierarchical custody config browser)
    render_node_detail_panel,
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
    GalaxyAction,
    GalaxyView,
    GraphEdgeData,
    GraphNodeData,
    NavigationEntry,
    RiskSummary,
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
    TradingMatrixNodeType,
    TradingMatrixState,
    TransitionAction,
    TypeBrowserAction,
    TypeNode,
    ViewMode,
    ViewTransition,
};
