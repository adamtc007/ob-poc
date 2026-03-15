//! Constellation map loader, hydration, and action-surface utilities.

pub mod action_surface;
pub mod builtin;
pub mod error;
pub mod hydrated;
pub mod hydration;
pub mod map_def;
pub mod map_loader;
pub mod normalize;
pub mod query_plan;
pub mod summary;
pub mod validate;
pub mod verbs;

pub use action_surface::compute_action_surface;
pub use builtin::load_builtin_constellation_map;
pub use error::{ConstellationError, ConstellationResult};
pub use hydrated::{
    HydratedConstellation, HydratedGraphEdge, HydratedGraphNode, HydratedSlot,
};
pub use hydration::{
    discover_state_machine_slot_contexts, hydrate_constellation, hydrate_constellation_summary,
    ConstellationSlotContext, RawGraphEdge, RawHydrationData, RawOverlayRow, RawSlotRow,
};
pub use map_def::{
    Cardinality, ConstellationMapDef, DependencyEntry, JoinDef, SlotDef, SlotType,
    VerbAvailability, VerbPaletteEntry,
};
pub use map_loader::{compute_map_revision, load_constellation_map};
pub use normalize::normalize_slots;
pub use query_plan::{compile_query_plan, HydrationQueryPlan, QueryLevel, QueryType, SlotQuery};
pub use summary::{compute_summary, ConstellationSummary};
pub use validate::{validate_constellation_map, ResolvedSlot, ValidatedConstellationMap};
pub use verbs::{handle_constellation_hydrate, handle_constellation_summary};
