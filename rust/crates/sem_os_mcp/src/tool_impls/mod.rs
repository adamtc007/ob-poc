//! Concrete `KnowledgeTool` implementations — Phase 4.2b/c.
//!
//! Each submodule defines one tool. Tools delegate to
//! [`crate::bridge::SemOsBridge`] for the actual substrate calls;
//! the binary integrator wires either `StubBridge` (hermetic) or a
//! real `SemOsClient`-backed bridge (Phase 4.3).

pub mod entity_resolve;
pub mod verb_surface;

pub use entity_resolve::EntityResolveTool;
pub use verb_surface::ActiveVerbSurfaceTool;
