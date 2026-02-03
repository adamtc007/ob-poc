//! Unified input handling for ESPER navigation.
//!
//! This crate provides a unified input model that translates keyboard, mouse,
//! touch, and gamepad events into navigation Verbs. The design ensures:
//!
//! 1. **Input source agnostic** - All input flows through RawInput → Verb
//! 2. **Testable** - Input sequences can be recorded and replayed
//! 3. **Rebindable** - Key bindings are configurable via InputConfig
//! 4. **Debounced** - Rapid inputs are coalesced appropriately
//!
//! # Architecture
//!
//! ```text
//! Keyboard ─┐
//! Mouse    ─┼──► RawInput ──► InputProcessor ──► Verb
//! Touch    ─┤                      │
//! Gamepad  ─┘                      │
//!                                  ▼
//!                            InputConfig (rebindable)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use esper_input::{InputProcessor, RawInput, KeyCode};
//!
//! let mut processor = InputProcessor::new();
//!
//! // Process keyboard input
//! if let Some(verb) = processor.process(RawInput::KeyDown(KeyCode::ArrowUp)) {
//!     state.execute(verb, &world)?;
//! }
//! ```

mod config;
mod error;
mod gesture;
mod keyboard;
mod mouse;
mod processor;
mod raw;

pub use config::{InputConfig, KeyBinding, MouseBinding, VerbTemplate};
pub use error::InputError;
pub use gesture::{Gesture, GestureRecognizer, GestureState};
pub use keyboard::{KeyCode, KeyModifiers};
pub use mouse::{MouseButton, MouseEvent, ScrollDelta};
pub use processor::InputProcessor;
pub use raw::RawInput;

/// Default repeat delay for held keys (milliseconds).
pub const DEFAULT_REPEAT_DELAY_MS: u64 = 500;

/// Default repeat rate for held keys (milliseconds between repeats).
pub const DEFAULT_REPEAT_RATE_MS: u64 = 50;

/// Default double-click threshold (milliseconds).
pub const DEFAULT_DOUBLE_CLICK_MS: u64 = 300;

/// Default drag threshold (pixels).
pub const DEFAULT_DRAG_THRESHOLD: f32 = 5.0;

/// Default pinch threshold for touch zoom.
pub const DEFAULT_PINCH_THRESHOLD: f32 = 10.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn constants_are_reasonable() {
        assert!(DEFAULT_REPEAT_DELAY_MS > 0);
        assert!(DEFAULT_REPEAT_RATE_MS > 0);
        assert!(DEFAULT_REPEAT_DELAY_MS > DEFAULT_REPEAT_RATE_MS);
        assert!(DEFAULT_DOUBLE_CLICK_MS > 0);
        assert!(DEFAULT_DRAG_THRESHOLD > 0.0);
        assert!(DEFAULT_PINCH_THRESHOLD > 0.0);
    }
}
