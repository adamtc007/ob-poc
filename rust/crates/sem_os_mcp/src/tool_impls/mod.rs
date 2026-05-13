//! Concrete `KnowledgeTool` implementations — Phase 4.2b/c.
//!
//! Each submodule defines one tool. Tools delegate to
//! [`crate::bridge::SemOsBridge`] for the actual substrate calls;
//! the binary integrator wires either `StubBridge` (hermetic) or a
//! real `SemOsClient`-backed bridge (Phase 4.3).

pub mod constellation_walk;
pub mod entity_resolve;
pub mod fsm_transitions;
pub mod pack_catalogue;
pub mod verb_surface;

pub use constellation_walk::ConstellationWalkTool;
pub use entity_resolve::EntityResolveTool;
pub use fsm_transitions::FsmTransitionsTool;
pub use pack_catalogue::PackCatalogueTool;
pub use verb_surface::ActiveVerbSurfaceTool;

use std::sync::Arc;

use crate::bridge::SemOsBridge;
use crate::tools::ToolRegistry;

/// Build a [`ToolRegistry`] populated with all five SemOS
/// knowledge tools backed by `bridge`. Used by the binary
/// integrator and by integration tests.
pub fn build_registry(bridge: Arc<dyn SemOsBridge>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EntityResolveTool::new(bridge.clone())));
    registry.register(Arc::new(ActiveVerbSurfaceTool::new(bridge.clone())));
    registry.register(Arc::new(PackCatalogueTool::new(bridge.clone())));
    registry.register(Arc::new(FsmTransitionsTool::new(bridge.clone())));
    registry.register(Arc::new(ConstellationWalkTool::new(bridge)));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::StubBridge;

    #[tokio::test]
    async fn build_registry_registers_five_tools() {
        let registry = build_registry(Arc::new(StubBridge::new()));
        assert_eq!(registry.len(), 5);
        for name in [
            "entity_resolve",
            "active_verb_surface_at_state",
            "pack_catalogue",
            "fsm_transitions",
            "constellation_walk",
        ] {
            assert!(registry.get(name).is_some(), "tool {name} must register");
        }
    }
}
