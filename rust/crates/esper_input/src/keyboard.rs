//! Keyboard input types.

use serde::{Deserialize, Serialize};

/// Key codes for keyboard input.
///
/// This enum covers the common keys used for navigation.
/// Platform-specific key codes should be mapped to these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    // Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Navigation
    Home,
    End,
    PageUp,
    PageDown,

    // Actions
    Enter,
    Space,
    Escape,
    Tab,
    Backspace,
    Delete,

    // Letters (for shortcuts)
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numbers
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Symbols
    Plus,
    Minus,
    Equals,
    BracketLeft,
    BracketRight,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backslash,
    Backtick,
}

impl KeyCode {
    /// Check if this is an arrow key.
    pub fn is_arrow(&self) -> bool {
        matches!(
            self,
            KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::ArrowLeft | KeyCode::ArrowRight
        )
    }

    /// Check if this is a letter key.
    pub fn is_letter(&self) -> bool {
        matches!(
            self,
            KeyCode::A
                | KeyCode::B
                | KeyCode::C
                | KeyCode::D
                | KeyCode::E
                | KeyCode::F
                | KeyCode::G
                | KeyCode::H
                | KeyCode::I
                | KeyCode::J
                | KeyCode::K
                | KeyCode::L
                | KeyCode::M
                | KeyCode::N
                | KeyCode::O
                | KeyCode::P
                | KeyCode::Q
                | KeyCode::R
                | KeyCode::S
                | KeyCode::T
                | KeyCode::U
                | KeyCode::V
                | KeyCode::W
                | KeyCode::X
                | KeyCode::Y
                | KeyCode::Z
        )
    }

    /// Check if this is a digit key.
    pub fn is_digit(&self) -> bool {
        matches!(
            self,
            KeyCode::Digit0
                | KeyCode::Digit1
                | KeyCode::Digit2
                | KeyCode::Digit3
                | KeyCode::Digit4
                | KeyCode::Digit5
                | KeyCode::Digit6
                | KeyCode::Digit7
                | KeyCode::Digit8
                | KeyCode::Digit9
        )
    }

    /// Check if this is a function key.
    pub fn is_function(&self) -> bool {
        matches!(
            self,
            KeyCode::F1
                | KeyCode::F2
                | KeyCode::F3
                | KeyCode::F4
                | KeyCode::F5
                | KeyCode::F6
                | KeyCode::F7
                | KeyCode::F8
                | KeyCode::F9
                | KeyCode::F10
                | KeyCode::F11
                | KeyCode::F12
        )
    }
}

/// Keyboard modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct KeyModifiers {
    /// Shift key is held.
    pub shift: bool,
    /// Control key is held (Cmd on Mac).
    pub ctrl: bool,
    /// Alt key is held (Option on Mac).
    pub alt: bool,
    /// Meta/Super key is held (Windows key, Cmd on Mac).
    pub meta: bool,
}

impl KeyModifiers {
    /// No modifiers.
    pub const NONE: KeyModifiers = KeyModifiers {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };

    /// Shift only.
    pub const SHIFT: KeyModifiers = KeyModifiers {
        shift: true,
        ctrl: false,
        alt: false,
        meta: false,
    };

    /// Ctrl only.
    pub const CTRL: KeyModifiers = KeyModifiers {
        shift: false,
        ctrl: true,
        alt: false,
        meta: false,
    };

    /// Alt only.
    pub const ALT: KeyModifiers = KeyModifiers {
        shift: false,
        ctrl: false,
        alt: true,
        meta: false,
    };

    /// Check if any modifier is held.
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }

    /// Check if no modifiers are held.
    pub fn none(&self) -> bool {
        !self.any()
    }

    /// Create modifiers with shift.
    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// Create modifiers with ctrl.
    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    /// Create modifiers with alt.
    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Create modifiers with meta.
    pub fn with_meta(mut self) -> Self {
        self.meta = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycode_categories() {
        assert!(KeyCode::ArrowUp.is_arrow());
        assert!(!KeyCode::A.is_arrow());

        assert!(KeyCode::A.is_letter());
        assert!(KeyCode::Z.is_letter());
        assert!(!KeyCode::Digit0.is_letter());

        assert!(KeyCode::Digit0.is_digit());
        assert!(KeyCode::Digit9.is_digit());
        assert!(!KeyCode::A.is_digit());

        assert!(KeyCode::F1.is_function());
        assert!(KeyCode::F12.is_function());
        assert!(!KeyCode::A.is_function());
    }

    #[test]
    fn modifier_helpers() {
        assert!(KeyModifiers::NONE.none());
        assert!(!KeyModifiers::NONE.any());

        assert!(KeyModifiers::SHIFT.any());
        assert!(!KeyModifiers::SHIFT.none());

        let mods = KeyModifiers::NONE.with_shift().with_ctrl();
        assert!(mods.shift);
        assert!(mods.ctrl);
        assert!(!mods.alt);
    }
}
