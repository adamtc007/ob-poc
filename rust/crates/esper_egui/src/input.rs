//! Input bridge between egui and esper_input.
//!
//! Translates egui input events into Verb commands.

use egui::{InputState, Key, PointerButton, Pos2};
use esper_core::Verb;
use esper_input::{GestureState, InputConfig, KeyCode, KeyModifiers, VerbTemplate};

/// Bridge between egui input and ESPER input system.
#[derive(Debug)]
pub struct InputBridge {
    /// Input configuration.
    config: InputConfig,
    /// Previous pointer position for delta calculation.
    prev_pointer: Option<Pos2>,
    /// Whether primary button is held.
    primary_held: bool,
    /// Whether we're currently dragging.
    is_dragging: bool,
    /// Drag start position.
    drag_start: Option<Pos2>,
}

impl Default for InputBridge {
    fn default() -> Self {
        Self::new(InputConfig::default())
    }
}

impl InputBridge {
    /// Create a new input bridge with the given configuration.
    pub fn new(config: InputConfig) -> Self {
        Self {
            config,
            prev_pointer: None,
            primary_held: false,
            is_dragging: false,
            drag_start: None,
        }
    }

    /// Process egui input and return verbs to execute.
    pub fn process(&mut self, input: &InputState) -> Vec<Verb> {
        let mut verbs = Vec::new();

        // Process keyboard
        verbs.extend(self.process_keyboard(input));

        // Process pointer/mouse
        verbs.extend(self.process_pointer(input));

        // Process scroll
        verbs.extend(self.process_scroll(input));

        verbs
    }

    /// Process keyboard input.
    fn process_keyboard(&self, input: &InputState) -> Vec<Verb> {
        let mut verbs = Vec::new();
        let modifiers = convert_modifiers(input);

        // Check for pressed keys and look up in config
        for key in &[
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::Enter,
            Key::Escape,
            Key::Home,
            Key::End,
            Key::Tab,
            Key::Space,
        ] {
            if input.key_pressed(*key) {
                if let Some(code) = convert_key(*key) {
                    if let Some(template) = self.config.lookup_key(code, modifiers) {
                        if let Some(verb) = template_to_verb(template) {
                            verbs.push(verb);
                        }
                    }
                }
            }
        }

        // Check for +/- zoom keys
        if input.key_pressed(Key::Plus) || input.key_pressed(Key::Equals) {
            if let Some(template) = self.config.lookup_key(KeyCode::Plus, modifiers) {
                if let Some(verb) = template_to_verb(template) {
                    verbs.push(verb);
                }
            }
        }

        if input.key_pressed(Key::Minus) {
            if let Some(template) = self.config.lookup_key(KeyCode::Minus, modifiers) {
                if let Some(verb) = template_to_verb(template) {
                    verbs.push(verb);
                }
            }
        }

        verbs
    }

    /// Process pointer/mouse input.
    fn process_pointer(&mut self, input: &InputState) -> Vec<Verb> {
        let mut verbs = Vec::new();

        let pointer = &input.pointer;

        // Track button state
        let primary_pressed = pointer.button_pressed(PointerButton::Primary);
        let primary_released = pointer.button_released(PointerButton::Primary);

        // Handle primary button press
        if primary_pressed {
            self.primary_held = true;
            self.drag_start = pointer.hover_pos();
        }

        // Handle primary button release
        if primary_released {
            if self.is_dragging {
                // End of drag - no additional verb needed
                self.is_dragging = false;
            }
            self.primary_held = false;
            self.drag_start = None;
        }

        // Handle drag
        if self.primary_held {
            if let (Some(current), Some(prev)) = (pointer.hover_pos(), self.prev_pointer) {
                let delta = current - prev;
                if delta.length() > 1.0 {
                    self.is_dragging = true;
                    // Pan camera by drag delta
                    verbs.push(Verb::PanBy {
                        dx: -delta.x,
                        dy: -delta.y,
                    });
                }
            }
        }

        // Update previous pointer position
        self.prev_pointer = pointer.hover_pos();

        verbs
    }

    /// Process scroll input for zooming.
    fn process_scroll(&self, input: &InputState) -> Vec<Verb> {
        let mut verbs = Vec::new();

        let scroll = input.raw_scroll_delta;
        if scroll.y.abs() > 0.1 {
            // Convert scroll to zoom factor
            let zoom_factor = 1.0 + scroll.y * 0.001;
            verbs.push(Verb::Zoom(zoom_factor));
        }

        verbs
    }

    /// Get the current gesture state.
    pub fn gesture_state(&self) -> GestureState {
        if self.is_dragging {
            GestureState::Active
        } else {
            GestureState::None
        }
    }

    /// Check if currently dragging.
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }
}

/// Convert a VerbTemplate to a Verb (for Fixed templates only).
fn template_to_verb(template: &VerbTemplate) -> Option<Verb> {
    match template {
        VerbTemplate::Fixed(verb) => Some(*verb),
        // Other templates need runtime parameters
        _ => None,
    }
}

/// Convert egui key to esper_input KeyCode.
fn convert_key(key: Key) -> Option<KeyCode> {
    match key {
        Key::ArrowUp => Some(KeyCode::ArrowUp),
        Key::ArrowDown => Some(KeyCode::ArrowDown),
        Key::ArrowLeft => Some(KeyCode::ArrowLeft),
        Key::ArrowRight => Some(KeyCode::ArrowRight),
        Key::Enter => Some(KeyCode::Enter),
        Key::Escape => Some(KeyCode::Escape),
        Key::Home => Some(KeyCode::Home),
        Key::End => Some(KeyCode::End),
        Key::Tab => Some(KeyCode::Tab),
        Key::Space => Some(KeyCode::Space),
        Key::Plus | Key::Equals => Some(KeyCode::Plus),
        Key::Minus => Some(KeyCode::Minus),
        _ => None,
    }
}

/// Convert egui modifiers to esper_input KeyModifiers.
fn convert_modifiers(input: &InputState) -> KeyModifiers {
    let m = &input.modifiers;
    KeyModifiers {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt,
        meta: m.mac_cmd || m.command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_bridge_creation() {
        let bridge = InputBridge::default();
        assert!(!bridge.is_dragging());
    }

    #[test]
    fn convert_modifiers_test() {
        let mods = KeyModifiers {
            shift: true,
            ctrl: false,
            alt: true,
            meta: false,
        };
        assert!(mods.shift);
        assert!(!mods.ctrl);
    }

    #[test]
    fn key_conversion() {
        assert_eq!(convert_key(Key::ArrowUp), Some(KeyCode::ArrowUp));
        assert_eq!(convert_key(Key::Enter), Some(KeyCode::Enter));
        assert!(convert_key(Key::A).is_none()); // Letter keys not mapped
    }
}
