//! Raw input events from any source.

use crate::keyboard::{KeyCode, KeyModifiers};
use crate::mouse::{MouseButton, MouseEvent, ScrollDelta};
use esper_snapshot::Vec2;
use serde::{Deserialize, Serialize};

/// Raw input event from any source.
///
/// This is the unified input type that all platform-specific events
/// are converted to before processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawInput {
    // =========================================================================
    // KEYBOARD
    // =========================================================================
    /// Key pressed down.
    KeyDown {
        key: KeyCode,
        modifiers: KeyModifiers,
    },

    /// Key released.
    KeyUp {
        key: KeyCode,
        modifiers: KeyModifiers,
    },

    /// Key held and repeating.
    KeyRepeat {
        key: KeyCode,
        modifiers: KeyModifiers,
    },

    // =========================================================================
    // MOUSE
    // =========================================================================
    /// Mouse event.
    Mouse(MouseEvent),

    // =========================================================================
    // TOUCH
    // =========================================================================
    /// Touch started (finger down).
    TouchStart { id: u64, pos: Vec2 },

    /// Touch moved.
    TouchMove { id: u64, pos: Vec2 },

    /// Touch ended (finger up).
    TouchEnd { id: u64, pos: Vec2 },

    /// Touch cancelled.
    TouchCancel { id: u64 },

    // =========================================================================
    // GAMEPAD
    // =========================================================================
    /// Gamepad D-pad direction.
    GamepadDpad { direction: DpadDirection },

    /// Gamepad analog stick.
    GamepadStick { stick: Stick, x: f32, y: f32 },

    /// Gamepad button pressed.
    GamepadButton { button: GamepadButton },

    /// Gamepad trigger.
    GamepadTrigger { trigger: Trigger, value: f32 },

    // =========================================================================
    // SPECIAL
    // =========================================================================
    /// Focus gained.
    FocusGained,

    /// Focus lost.
    FocusLost,

    /// Window resized.
    Resized { width: f32, height: f32 },
}

impl RawInput {
    /// Create a key down event.
    pub fn key_down(key: KeyCode) -> Self {
        RawInput::KeyDown {
            key,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Create a key down event with modifiers.
    pub fn key_down_with(key: KeyCode, modifiers: KeyModifiers) -> Self {
        RawInput::KeyDown { key, modifiers }
    }

    /// Create a mouse move event.
    pub fn mouse_move(x: f32, y: f32) -> Self {
        RawInput::Mouse(MouseEvent::Move {
            pos: Vec2::new(x, y),
        })
    }

    /// Create a mouse button down event.
    pub fn mouse_down(button: MouseButton, x: f32, y: f32) -> Self {
        RawInput::Mouse(MouseEvent::ButtonDown {
            button,
            pos: Vec2::new(x, y),
        })
    }

    /// Create a mouse button up event.
    pub fn mouse_up(button: MouseButton, x: f32, y: f32) -> Self {
        RawInput::Mouse(MouseEvent::ButtonUp {
            button,
            pos: Vec2::new(x, y),
        })
    }

    /// Create a mouse scroll event.
    pub fn mouse_scroll(delta_y: f32, x: f32, y: f32) -> Self {
        RawInput::Mouse(MouseEvent::Scroll {
            delta: ScrollDelta::Lines { x: 0.0, y: delta_y },
            pos: Vec2::new(x, y),
        })
    }

    /// Check if this is a keyboard event.
    pub fn is_keyboard(&self) -> bool {
        matches!(
            self,
            RawInput::KeyDown { .. } | RawInput::KeyUp { .. } | RawInput::KeyRepeat { .. }
        )
    }

    /// Check if this is a mouse event.
    pub fn is_mouse(&self) -> bool {
        matches!(self, RawInput::Mouse(_))
    }

    /// Check if this is a touch event.
    pub fn is_touch(&self) -> bool {
        matches!(
            self,
            RawInput::TouchStart { .. }
                | RawInput::TouchMove { .. }
                | RawInput::TouchEnd { .. }
                | RawInput::TouchCancel { .. }
        )
    }

    /// Check if this is a gamepad event.
    pub fn is_gamepad(&self) -> bool {
        matches!(
            self,
            RawInput::GamepadDpad { .. }
                | RawInput::GamepadStick { .. }
                | RawInput::GamepadButton { .. }
                | RawInput::GamepadTrigger { .. }
        )
    }
}

/// D-pad direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DpadDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Center,
}

/// Analog stick identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stick {
    Left,
    Right,
}

/// Gamepad button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadButton {
    /// A/Cross button.
    South,
    /// B/Circle button.
    East,
    /// X/Square button.
    West,
    /// Y/Triangle button.
    North,
    /// Left bumper.
    LeftBumper,
    /// Right bumper.
    RightBumper,
    /// Left stick click.
    LeftStick,
    /// Right stick click.
    RightStick,
    /// Start/Options button.
    Start,
    /// Select/Share button.
    Select,
    /// Guide/Home button.
    Guide,
}

/// Trigger identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Trigger {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_input_categories() {
        assert!(RawInput::key_down(KeyCode::A).is_keyboard());
        assert!(!RawInput::key_down(KeyCode::A).is_mouse());

        assert!(RawInput::mouse_move(0.0, 0.0).is_mouse());
        assert!(!RawInput::mouse_move(0.0, 0.0).is_keyboard());

        assert!(RawInput::TouchStart {
            id: 0,
            pos: Vec2::ZERO
        }
        .is_touch());
        assert!(!RawInput::TouchStart {
            id: 0,
            pos: Vec2::ZERO
        }
        .is_gamepad());

        assert!(RawInput::GamepadDpad {
            direction: DpadDirection::Up
        }
        .is_gamepad());
    }

    #[test]
    fn raw_input_constructors() {
        let input = RawInput::key_down_with(KeyCode::S, KeyModifiers::CTRL);
        if let RawInput::KeyDown { key, modifiers } = input {
            assert_eq!(key, KeyCode::S);
            assert!(modifiers.ctrl);
        } else {
            panic!("expected KeyDown");
        }

        let input = RawInput::mouse_down(MouseButton::Primary, 100.0, 200.0);
        if let RawInput::Mouse(MouseEvent::ButtonDown { button, pos }) = input {
            assert_eq!(button, MouseButton::Primary);
            assert_eq!(pos.x, 100.0);
            assert_eq!(pos.y, 200.0);
        } else {
            panic!("expected Mouse ButtonDown");
        }
    }
}
