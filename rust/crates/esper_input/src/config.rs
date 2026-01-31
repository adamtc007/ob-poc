//! Input configuration and key bindings.

use crate::keyboard::{KeyCode, KeyModifiers};
use crate::mouse::MouseButton;
use crate::raw::{DpadDirection, GamepadButton};
use crate::{
    DEFAULT_DOUBLE_CLICK_MS, DEFAULT_DRAG_THRESHOLD, DEFAULT_REPEAT_DELAY_MS,
    DEFAULT_REPEAT_RATE_MS,
};
use esper_core::Verb;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Key binding entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBinding {
    /// Primary key.
    pub key: KeyCode,
    /// Required modifiers.
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    /// Create a binding with no modifiers.
    pub fn new(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Create a binding with modifiers.
    pub fn with_mods(key: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    /// Create a binding with Ctrl modifier.
    pub fn ctrl(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::CTRL,
        }
    }

    /// Create a binding with Shift modifier.
    pub fn shift(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::SHIFT,
        }
    }

    /// Check if this binding matches the given key and modifiers.
    pub fn matches(&self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

/// Mouse binding entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MouseBinding {
    /// Mouse button.
    pub button: MouseButton,
    /// Required modifiers.
    pub modifiers: KeyModifiers,
    /// Whether this is for click, double-click, or drag.
    pub action: MouseAction,
}

/// Mouse action type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseAction {
    Click,
    DoubleClick,
    Drag,
}

impl MouseBinding {
    /// Create a click binding.
    pub fn click(button: MouseButton) -> Self {
        Self {
            button,
            modifiers: KeyModifiers::NONE,
            action: MouseAction::Click,
        }
    }

    /// Create a double-click binding.
    pub fn double_click(button: MouseButton) -> Self {
        Self {
            button,
            modifiers: KeyModifiers::NONE,
            action: MouseAction::DoubleClick,
        }
    }

    /// Create a drag binding.
    pub fn drag(button: MouseButton) -> Self {
        Self {
            button,
            modifiers: KeyModifiers::NONE,
            action: MouseAction::Drag,
        }
    }

    /// Add modifiers to this binding.
    pub fn with_mods(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

/// Complete input configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    /// Keyboard bindings: KeyBinding → Verb.
    pub keyboard: HashMap<KeyBinding, VerbTemplate>,

    /// Mouse bindings: MouseBinding → Verb.
    pub mouse: HashMap<MouseBinding, VerbTemplate>,

    /// Gamepad D-pad bindings.
    pub dpad: HashMap<DpadDirection, VerbTemplate>,

    /// Gamepad button bindings.
    pub gamepad_buttons: HashMap<GamepadButton, VerbTemplate>,

    /// Key repeat delay (ms).
    pub repeat_delay_ms: u64,

    /// Key repeat rate (ms).
    pub repeat_rate_ms: u64,

    /// Double-click threshold (ms).
    pub double_click_ms: u64,

    /// Drag threshold (pixels).
    pub drag_threshold: f32,

    /// Scroll zoom sensitivity.
    pub scroll_zoom_sensitivity: f32,

    /// Pan speed multiplier.
    pub pan_speed: f32,

    /// Invert Y axis for pan.
    pub invert_y: bool,
}

/// Verb template with optional parameterization.
///
/// Some verbs need runtime parameters (e.g., pan amount depends on drag delta).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerbTemplate {
    /// Fixed verb, no parameters.
    Fixed(Verb),

    /// Pan by mouse delta (filled in at runtime).
    PanByDelta,

    /// Zoom in by scroll delta (filled in at runtime).
    ZoomInByScroll,

    /// Zoom out by scroll delta (filled in at runtime).
    ZoomOutByScroll,

    /// Zoom to mouse position (filled in at runtime).
    ZoomToPoint,

    /// Focus entity under cursor (filled in at runtime).
    FocusUnderCursor,

    /// Select node under cursor (filled in at runtime).
    SelectUnderCursor,
}

impl VerbTemplate {
    /// Check if this template needs runtime parameters.
    pub fn needs_parameters(&self) -> bool {
        !matches!(self, VerbTemplate::Fixed(_))
    }

    /// Check if this is a zoom-in template.
    pub fn is_zoom_in(&self) -> bool {
        matches!(self, VerbTemplate::ZoomInByScroll)
    }

    /// Check if this is a zoom-out template.
    pub fn is_zoom_out(&self) -> bool {
        matches!(self, VerbTemplate::ZoomOutByScroll)
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        let mut config = Self {
            keyboard: HashMap::new(),
            mouse: HashMap::new(),
            dpad: HashMap::new(),
            gamepad_buttons: HashMap::new(),
            repeat_delay_ms: DEFAULT_REPEAT_DELAY_MS,
            repeat_rate_ms: DEFAULT_REPEAT_RATE_MS,
            double_click_ms: DEFAULT_DOUBLE_CLICK_MS,
            drag_threshold: DEFAULT_DRAG_THRESHOLD,
            scroll_zoom_sensitivity: 0.1,
            pan_speed: 1.0,
            invert_y: false,
        };

        config.setup_default_bindings();
        config
    }
}

impl InputConfig {
    /// Create empty config with no bindings.
    pub fn empty() -> Self {
        Self {
            keyboard: HashMap::new(),
            mouse: HashMap::new(),
            dpad: HashMap::new(),
            gamepad_buttons: HashMap::new(),
            repeat_delay_ms: DEFAULT_REPEAT_DELAY_MS,
            repeat_rate_ms: DEFAULT_REPEAT_RATE_MS,
            double_click_ms: DEFAULT_DOUBLE_CLICK_MS,
            drag_threshold: DEFAULT_DRAG_THRESHOLD,
            scroll_zoom_sensitivity: 0.1,
            pan_speed: 1.0,
            invert_y: false,
        }
    }

    /// Set up default key bindings.
    fn setup_default_bindings(&mut self) {
        // Arrow keys for navigation
        self.keyboard.insert(
            KeyBinding::new(KeyCode::ArrowUp),
            VerbTemplate::Fixed(Verb::Ascend),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::ArrowDown),
            VerbTemplate::Fixed(Verb::Descend),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::ArrowLeft),
            VerbTemplate::Fixed(Verb::Prev),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::ArrowRight),
            VerbTemplate::Fixed(Verb::Next),
        );

        // Vim-style (HJKL)
        self.keyboard
            .insert(KeyBinding::new(KeyCode::H), VerbTemplate::Fixed(Verb::Prev));
        self.keyboard.insert(
            KeyBinding::new(KeyCode::J),
            VerbTemplate::Fixed(Verb::Descend),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::K),
            VerbTemplate::Fixed(Verb::Ascend),
        );
        self.keyboard
            .insert(KeyBinding::new(KeyCode::L), VerbTemplate::Fixed(Verb::Next));

        // WASD for pan
        self.keyboard.insert(
            KeyBinding::new(KeyCode::W),
            VerbTemplate::Fixed(Verb::PanBy { dx: 0.0, dy: -50.0 }),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::A),
            VerbTemplate::Fixed(Verb::PanBy { dx: -50.0, dy: 0.0 }),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::S),
            VerbTemplate::Fixed(Verb::PanBy { dx: 0.0, dy: 50.0 }),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::D),
            VerbTemplate::Fixed(Verb::PanBy { dx: 50.0, dy: 0.0 }),
        );

        // Zoom
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Plus),
            VerbTemplate::Fixed(Verb::Zoom(1.2)),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Equals),
            VerbTemplate::Fixed(Verb::Zoom(1.2)),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Minus),
            VerbTemplate::Fixed(Verb::Zoom(1.0 / 1.2)),
        );

        // Home/End for first/last
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Home),
            VerbTemplate::Fixed(Verb::First),
        );
        self.keyboard.insert(
            KeyBinding::new(KeyCode::End),
            VerbTemplate::Fixed(Verb::Last),
        );

        // Space for expand/collapse toggle
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Space),
            VerbTemplate::Fixed(Verb::Expand),
        );
        self.keyboard.insert(
            KeyBinding::shift(KeyCode::Space),
            VerbTemplate::Fixed(Verb::Collapse),
        );

        // Enter for focus/select
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Enter),
            VerbTemplate::SelectUnderCursor,
        );

        // Escape for root/clear
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Escape),
            VerbTemplate::Fixed(Verb::Root),
        );

        // Tab for mode toggle
        self.keyboard.insert(
            KeyBinding::new(KeyCode::Tab),
            VerbTemplate::Fixed(Verb::ModeToggle),
        );

        // Undo/Redo
        self.keyboard.insert(
            KeyBinding::ctrl(KeyCode::Z),
            VerbTemplate::Fixed(Verb::Noop),
        ); // Undo would be external
        self.keyboard.insert(
            KeyBinding::with_mods(KeyCode::Z, KeyModifiers::CTRL.with_shift()),
            VerbTemplate::Fixed(Verb::Noop),
        ); // Redo would be external

        // Mouse bindings
        self.mouse.insert(
            MouseBinding::click(MouseButton::Primary),
            VerbTemplate::SelectUnderCursor,
        );
        self.mouse.insert(
            MouseBinding::double_click(MouseButton::Primary),
            VerbTemplate::FocusUnderCursor,
        );
        self.mouse.insert(
            MouseBinding::drag(MouseButton::Primary),
            VerbTemplate::PanByDelta,
        );
        self.mouse.insert(
            MouseBinding::drag(MouseButton::Middle),
            VerbTemplate::PanByDelta,
        );

        // Gamepad D-pad
        self.dpad
            .insert(DpadDirection::Up, VerbTemplate::Fixed(Verb::Ascend));
        self.dpad
            .insert(DpadDirection::Down, VerbTemplate::Fixed(Verb::Descend));
        self.dpad
            .insert(DpadDirection::Left, VerbTemplate::Fixed(Verb::Prev));
        self.dpad
            .insert(DpadDirection::Right, VerbTemplate::Fixed(Verb::Next));

        // Gamepad buttons
        self.gamepad_buttons
            .insert(GamepadButton::South, VerbTemplate::Fixed(Verb::Expand));
        self.gamepad_buttons
            .insert(GamepadButton::East, VerbTemplate::Fixed(Verb::Ascend));
        self.gamepad_buttons
            .insert(GamepadButton::West, VerbTemplate::Fixed(Verb::Collapse));
        self.gamepad_buttons
            .insert(GamepadButton::North, VerbTemplate::Fixed(Verb::Root));
    }

    /// Look up verb for a key press.
    pub fn lookup_key(&self, key: KeyCode, modifiers: KeyModifiers) -> Option<&VerbTemplate> {
        let binding = KeyBinding { key, modifiers };
        self.keyboard.get(&binding)
    }

    /// Look up verb for a D-pad direction.
    pub fn lookup_dpad(&self, direction: DpadDirection) -> Option<&VerbTemplate> {
        self.dpad.get(&direction)
    }

    /// Look up verb for a gamepad button.
    pub fn lookup_gamepad_button(&self, button: GamepadButton) -> Option<&VerbTemplate> {
        self.gamepad_buttons.get(&button)
    }

    /// Bind a key to a verb.
    pub fn bind_key(&mut self, binding: KeyBinding, verb: VerbTemplate) {
        self.keyboard.insert(binding, verb);
    }

    /// Unbind a key.
    pub fn unbind_key(&mut self, binding: &KeyBinding) {
        self.keyboard.remove(binding);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_bindings() {
        let config = InputConfig::default();
        assert!(!config.keyboard.is_empty());
        assert!(!config.mouse.is_empty());
        assert!(!config.dpad.is_empty());
    }

    #[test]
    fn key_lookup() {
        let config = InputConfig::default();

        // Arrow up should map to Ascend
        let verb = config.lookup_key(KeyCode::ArrowUp, KeyModifiers::NONE);
        assert!(matches!(verb, Some(VerbTemplate::Fixed(Verb::Ascend))));

        // Ctrl+Z should exist
        let verb = config.lookup_key(KeyCode::Z, KeyModifiers::CTRL);
        assert!(verb.is_some());

        // Random unbound key
        let verb = config.lookup_key(KeyCode::F12, KeyModifiers::NONE);
        assert!(verb.is_none());
    }

    #[test]
    fn custom_binding() {
        let mut config = InputConfig::empty();

        config.bind_key(KeyBinding::new(KeyCode::Q), VerbTemplate::Fixed(Verb::Root));

        let verb = config.lookup_key(KeyCode::Q, KeyModifiers::NONE);
        assert!(matches!(verb, Some(VerbTemplate::Fixed(Verb::Root))));
    }

    #[test]
    fn binding_with_modifiers() {
        let binding = KeyBinding::with_mods(KeyCode::S, KeyModifiers::CTRL);
        assert!(binding.matches(KeyCode::S, KeyModifiers::CTRL));
        assert!(!binding.matches(KeyCode::S, KeyModifiers::NONE));
        assert!(!binding.matches(KeyCode::S, KeyModifiers::SHIFT));
    }
}
