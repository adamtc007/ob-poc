//! OB-POC Graph Widget
//!
//! This crate contains ONLY the graph widget - no API, no app shell.
//! The widget is used by ob-poc-ui which owns the API and app lifecycle.

pub mod graph;

pub use graph::{
    entity_matches_type,
    get_entities_for_type,
    render_type_browser,
    // Astronomy view (universe/solar system transitions)
    AstronomyView,
    // Core graph types
    CbuGraphData,
    CbuGraphWidget,
    // Ontology (type hierarchy browser)
    EntityTypeOntology,
    GraphEdgeData,
    GraphNodeData,
    NavigationEntry,
    // Animation
    SpringConfig,
    SpringF32,
    SpringVec2,
    TaxonomyState,
    TransitionAction,
    TypeBrowserAction,
    TypeNode,
    ViewMode,
    ViewTransition,
};
