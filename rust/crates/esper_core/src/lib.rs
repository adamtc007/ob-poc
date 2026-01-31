//! ESPER Navigation Core
//!
//! This crate implements the navigation engine - the pure state machine that
//! transforms verbs into effects. It is input-agnostic and rendering-agnostic.
//!
//! # Architecture
//!
//! ```text
//! Input (any source)
//!       │
//!       ▼
//!    ┌──────────┐
//!    │   Verb   │  ← Command to execute
//!    └────┬─────┘
//!         │
//!         ▼
//!    ┌──────────────┐
//!    │  DroneState  │  ← Navigation state machine
//!    │  .execute()  │
//!    └──────┬───────┘
//!           │
//!           ▼
//!    ┌──────────────┐
//!    │  EffectSet   │  ← Bitmask of effects to handle
//!    └──────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use esper_core::{DroneState, Verb, EffectSet};
//! use esper_snapshot::WorldSnapshot;
//!
//! let world = WorldSnapshot::empty(1);
//! let mut state = DroneState::new();
//!
//! // Execute a verb
//! let effects = state.execute(Verb::Next, &world)?;
//!
//! // Handle effects
//! if effects.contains(EffectSet::CAMERA_CHANGED) {
//!     // Update camera animation
//! }
//! ```

mod effect;
mod fault;
mod phase;
mod replay;
mod stack;
mod state;
mod verb;

pub use effect::EffectSet;
pub use fault::Fault;
pub use phase::NavigationPhase;
pub use replay::{NavigationLog, TimestampedVerb};
pub use stack::ContextStack;
pub use state::{CameraState, DroneState, LodState, NavigationMode, TaxonomyState};
pub use verb::Verb;

/// Maximum context stack depth to prevent runaway navigation.
pub const MAX_CONTEXT_DEPTH: usize = 32;

/// Number of ticks for hover dwell detection.
pub const DWELL_TICKS: u64 = 30; // ~0.5s at 60fps

/// Number of ticks for settling phase after navigation.
pub const SETTLE_TICKS: u64 = 12; // ~0.2s at 60fps

/// Camera lerp speed (per second).
pub const CAMERA_LERP_SPEED: f32 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_reasonable() {
        assert!(MAX_CONTEXT_DEPTH > 0);
        assert!(DWELL_TICKS > 0);
        assert!(SETTLE_TICKS > 0);
        assert!(CAMERA_LERP_SPEED > 0.0);
    }
}
