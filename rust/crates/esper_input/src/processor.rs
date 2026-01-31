//! Input processor that translates raw input to navigation verbs.

use crate::config::{InputConfig, MouseAction, MouseBinding, VerbTemplate};
use crate::gesture::{Gesture, GestureRecognizer};
use crate::keyboard::KeyModifiers;
use crate::mouse::{MouseButton, MouseEvent, MouseState};
use crate::raw::RawInput;
use esper_core::Verb;
use esper_snapshot::Vec2;

/// Input processor that converts raw input events to navigation verbs.
///
/// This is the main entry point for input handling. It:
/// - Tracks modifier key state
/// - Handles key repeat timing
/// - Recognizes mouse gestures (drag, double-click)
/// - Recognizes touch gestures (pinch, swipe)
/// - Looks up bindings in the configuration
#[derive(Debug)]
pub struct InputProcessor {
    /// Input configuration.
    config: InputConfig,
    /// Current modifier key state.
    modifiers: KeyModifiers,
    /// Mouse state for gesture detection.
    mouse_state: MouseState,
    /// Touch gesture recognizer.
    gesture_recognizer: GestureRecognizer,
    /// Current timestamp (ms).
    timestamp: u64,
    /// Last key pressed (for repeat).
    last_key: Option<crate::keyboard::KeyCode>,
    /// Time of last key press.
    last_key_time: u64,
    /// Whether key is in repeat mode.
    key_repeating: bool,
    /// Viewport size (for coordinate transforms).
    viewport_size: Vec2,
    /// Hit test callback result (entity under cursor).
    hover_entity: Option<u64>,
    /// Hit test callback result (node under cursor).
    hover_node: Option<u32>,
}

impl InputProcessor {
    /// Create a new input processor with default configuration.
    pub fn new() -> Self {
        Self::with_config(InputConfig::default())
    }

    /// Create with custom configuration.
    pub fn with_config(config: InputConfig) -> Self {
        Self {
            config,
            modifiers: KeyModifiers::NONE,
            mouse_state: MouseState::default(),
            gesture_recognizer: GestureRecognizer::new(),
            timestamp: 0,
            last_key: None,
            last_key_time: 0,
            key_repeating: false,
            viewport_size: Vec2::new(800.0, 600.0),
            hover_entity: None,
            hover_node: None,
        }
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &InputConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut InputConfig {
        &mut self.config
    }

    /// Set the viewport size.
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        self.viewport_size = Vec2::new(width, height);
    }

    /// Set the entity currently under the cursor (from hit testing).
    pub fn set_hover_entity(&mut self, entity_id: Option<u64>) {
        self.hover_entity = entity_id;
    }

    /// Set the node currently under the cursor (from hit testing).
    pub fn set_hover_node(&mut self, node_idx: Option<u32>) {
        self.hover_node = node_idx;
    }

    /// Get current modifier state.
    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    /// Check if a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.mouse_state.dragging
    }

    /// Get current mouse position.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_state.position
    }

    /// Process a raw input event and return a verb if one should be executed.
    ///
    /// Returns `None` if the input doesn't map to any action.
    pub fn process(&mut self, input: RawInput, timestamp: u64) -> Option<Verb> {
        self.timestamp = timestamp;

        match input {
            // Keyboard
            RawInput::KeyDown { key, modifiers } => {
                self.modifiers = modifiers;
                self.last_key = Some(key);
                self.last_key_time = timestamp;
                self.key_repeating = false;

                self.process_key(key, modifiers)
            }
            RawInput::KeyUp { key, modifiers } => {
                self.modifiers = modifiers;
                if self.last_key == Some(key) {
                    self.last_key = None;
                    self.key_repeating = false;
                }
                None
            }
            RawInput::KeyRepeat { key, modifiers } => {
                self.modifiers = modifiers;
                self.key_repeating = true;
                self.process_key(key, modifiers)
            }

            // Mouse
            RawInput::Mouse(event) => self.process_mouse(event, timestamp),

            // Touch
            RawInput::TouchStart { id, pos } => {
                self.gesture_recognizer.touch_start(id, pos, timestamp);
                None
            }
            RawInput::TouchMove { id, pos } => {
                if let Some(gesture) = self.gesture_recognizer.touch_move(id, pos, timestamp) {
                    self.gesture_to_verb(gesture)
                } else {
                    None
                }
            }
            RawInput::TouchEnd { id, pos } => {
                if let Some(gesture) = self.gesture_recognizer.touch_end(id, pos, timestamp) {
                    self.gesture_to_verb(gesture)
                } else {
                    None
                }
            }
            RawInput::TouchCancel { id } => {
                self.gesture_recognizer.touch_cancel(id);
                None
            }

            // Gamepad
            RawInput::GamepadDpad { direction } => self
                .config
                .lookup_dpad(direction)
                .and_then(|t| self.resolve_template(t)),
            RawInput::GamepadButton { button } => self
                .config
                .lookup_gamepad_button(button)
                .and_then(|t| self.resolve_template(t)),
            RawInput::GamepadStick { stick, x, y } => {
                // Map stick to pan (with deadzone)
                const DEADZONE: f32 = 0.15;
                if x.abs() > DEADZONE || y.abs() > DEADZONE {
                    let dx = x * 10.0 * self.config.pan_speed;
                    let dy =
                        if self.config.invert_y { -y } else { y } * 10.0 * self.config.pan_speed;
                    // Only left stick pans by default
                    if matches!(stick, crate::raw::Stick::Left) {
                        Some(Verb::PanBy { dx, dy })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            RawInput::GamepadTrigger { trigger, value } => {
                // Map triggers to zoom (with threshold)
                const THRESHOLD: f32 = 0.1;
                if value > THRESHOLD {
                    let factor = 1.0 + value * 0.5;
                    match trigger {
                        crate::raw::Trigger::Right => Some(Verb::Zoom(factor)),
                        crate::raw::Trigger::Left => Some(Verb::Zoom(1.0 / factor)),
                    }
                } else {
                    None
                }
            }

            // Special
            RawInput::FocusLost => {
                // Reset state on focus loss
                self.modifiers = KeyModifiers::NONE;
                self.last_key = None;
                self.key_repeating = false;
                self.mouse_state = MouseState::default();
                self.gesture_recognizer.reset();
                None
            }
            RawInput::FocusGained => None,
            RawInput::Resized { width, height } => {
                self.viewport_size = Vec2::new(width, height);
                None
            }
        }
    }

    /// Update timing (call each frame to handle key repeat).
    pub fn update(&mut self, timestamp: u64) -> Option<Verb> {
        self.timestamp = timestamp;

        // Handle key repeat
        if let Some(key) = self.last_key {
            let elapsed = timestamp.saturating_sub(self.last_key_time);

            if self.key_repeating {
                // Already repeating, check repeat rate
                if elapsed >= self.config.repeat_rate_ms {
                    self.last_key_time = timestamp;
                    return self.process_key(key, self.modifiers);
                }
            } else {
                // Not yet repeating, check initial delay
                if elapsed >= self.config.repeat_delay_ms {
                    self.key_repeating = true;
                    self.last_key_time = timestamp;
                    return self.process_key(key, self.modifiers);
                }
            }
        }

        None
    }

    // =========================================================================
    // INTERNAL HELPERS
    // =========================================================================

    fn process_key(&self, key: crate::keyboard::KeyCode, modifiers: KeyModifiers) -> Option<Verb> {
        self.config
            .lookup_key(key, modifiers)
            .and_then(|t| self.resolve_template(t))
    }

    fn process_mouse(&mut self, event: MouseEvent, timestamp: u64) -> Option<Verb> {
        // Update mouse state
        self.mouse_state
            .update(&event, timestamp, self.config.drag_threshold);

        match event {
            MouseEvent::ButtonDown { button, pos } => {
                // Check for double-click
                if button == MouseButton::Primary
                    && self
                        .mouse_state
                        .is_double_click(timestamp, pos, self.config.double_click_ms)
                {
                    let binding = MouseBinding {
                        button,
                        modifiers: self.modifiers,
                        action: MouseAction::DoubleClick,
                    };
                    if let Some(template) = self.config.mouse.get(&binding) {
                        return self.resolve_template(template);
                    }
                }
                None
            }
            MouseEvent::ButtonUp { button, pos } => {
                // Click (if not dragging)
                if !self.mouse_state.dragging {
                    let binding = MouseBinding {
                        button,
                        modifiers: self.modifiers,
                        action: MouseAction::Click,
                    };
                    if let Some(template) = self.config.mouse.get(&binding) {
                        return self.resolve_template_with_pos(template, pos);
                    }
                }
                None
            }
            MouseEvent::Move { pos: _ } => {
                // Drag
                if self.mouse_state.dragging {
                    if let Some(delta) = self.mouse_state.drag_delta() {
                        // For drag, we want incremental delta, not total
                        // This is a simplification - real impl would track last position
                        let binding = MouseBinding {
                            button: MouseButton::Primary,
                            modifiers: self.modifiers,
                            action: MouseAction::Drag,
                        };
                        if let Some(template) = self.config.mouse.get(&binding) {
                            if matches!(template, VerbTemplate::PanByDelta) {
                                // Invert for natural pan feel
                                let dx = -delta.x * self.config.pan_speed;
                                let dy = if self.config.invert_y {
                                    delta.y
                                } else {
                                    -delta.y
                                } * self.config.pan_speed;
                                // Reset drag start to current for incremental updates
                                // (This is a simplification)
                                return Some(Verb::PanBy {
                                    dx: dx * 0.1,
                                    dy: dy * 0.1,
                                });
                            }
                        }
                    }
                }
                // Update preview on hover
                if let Some(node) = self.hover_node {
                    return Some(Verb::Preview(node));
                }
                None
            }
            MouseEvent::Scroll { delta, pos: _ } => {
                // Ctrl+scroll = zoom
                if self.modifiers.ctrl || delta.is_zoom_gesture(self.modifiers.ctrl) {
                    let y = delta.y_lines();
                    if y.abs() > 0.01 {
                        let factor = 1.0 + y.abs() * self.config.scroll_zoom_sensitivity;
                        if y > 0.0 {
                            return Some(Verb::Zoom(factor));
                        } else {
                            return Some(Verb::Zoom(1.0 / factor));
                        }
                    }
                } else {
                    // Regular scroll = pan
                    let dx = delta.x_lines() * 20.0 * self.config.pan_speed;
                    let dy = delta.y_lines() * 20.0 * self.config.pan_speed;
                    if dx.abs() > 0.01 || dy.abs() > 0.01 {
                        let dy = if self.config.invert_y { -dy } else { dy };
                        return Some(Verb::PanBy { dx, dy });
                    }
                }
                None
            }
            MouseEvent::Leave => Some(Verb::ClearPreview),
            MouseEvent::Enter { .. } => None,
        }
    }

    fn gesture_to_verb(&self, gesture: Gesture) -> Option<Verb> {
        match gesture {
            Gesture::Tap { pos: _ } => {
                // Tap = select
                self.hover_node.map(Verb::Select)
            }
            Gesture::DoubleTap { pos: _ } => {
                // Double tap = focus
                self.hover_entity.map(Verb::Focus)
            }
            Gesture::LongPress { pos: _ } => {
                // Long press = context menu (no verb)
                None
            }
            Gesture::Drag { delta, .. } => {
                let dx = -delta.x * self.config.pan_speed;
                let dy = if self.config.invert_y {
                    delta.y
                } else {
                    -delta.y
                } * self.config.pan_speed;
                Some(Verb::PanBy {
                    dx: dx * 0.1,
                    dy: dy * 0.1,
                })
            }
            Gesture::Pinch { scale, .. } => {
                // scale > 1.0 = zoom in, scale < 1.0 = zoom out
                Some(Verb::Zoom(scale))
            }
            Gesture::TwoFingerPan { delta } => {
                let dx = -delta.x * self.config.pan_speed;
                let dy = if self.config.invert_y {
                    delta.y
                } else {
                    -delta.y
                } * self.config.pan_speed;
                Some(Verb::PanBy { dx, dy })
            }
            Gesture::Swipe { direction, .. } => {
                use crate::gesture::SwipeDirection;
                match direction {
                    SwipeDirection::Left => Some(Verb::Next),
                    SwipeDirection::Right => Some(Verb::Prev),
                    SwipeDirection::Up => Some(Verb::Ascend),
                    SwipeDirection::Down => Some(Verb::Descend),
                }
            }
            Gesture::EdgeSwipe { .. } => {
                // Edge swipes could trigger navigation (no verb for now)
                None
            }
        }
    }

    fn resolve_template(&self, template: &VerbTemplate) -> Option<Verb> {
        match template {
            VerbTemplate::Fixed(verb) => Some(*verb),
            VerbTemplate::PanByDelta => None, // Needs position data
            VerbTemplate::ZoomInByScroll => None, // Needs scroll data
            VerbTemplate::ZoomOutByScroll => None, // Needs scroll data
            VerbTemplate::ZoomToPoint => None, // Needs position data
            VerbTemplate::FocusUnderCursor => self.hover_entity.map(Verb::Focus),
            VerbTemplate::SelectUnderCursor => self.hover_node.map(Verb::Select),
        }
    }

    fn resolve_template_with_pos(&self, template: &VerbTemplate, _pos: Vec2) -> Option<Verb> {
        match template {
            VerbTemplate::Fixed(verb) => Some(*verb),
            VerbTemplate::FocusUnderCursor => self.hover_entity.map(Verb::Focus),
            VerbTemplate::SelectUnderCursor => self.hover_node.map(Verb::Select),
            _ => None,
        }
    }
}

impl Default for InputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::KeyCode;

    #[test]
    fn process_key_down() {
        let mut processor = InputProcessor::new();

        // Arrow up should produce Ascend
        let verb = processor.process(RawInput::key_down(KeyCode::ArrowUp), 0);
        assert!(matches!(verb, Some(Verb::Ascend)));
    }

    #[test]
    fn process_key_with_modifiers() {
        let mut processor = InputProcessor::new();

        // Shift+Space should produce Collapse
        let verb = processor.process(
            RawInput::key_down_with(KeyCode::Space, KeyModifiers::SHIFT),
            0,
        );
        assert!(matches!(verb, Some(Verb::Collapse)));
    }

    #[test]
    fn process_scroll_zoom() {
        use crate::mouse::ScrollDelta;

        let mut processor = InputProcessor::new();

        // Ctrl+scroll should zoom
        processor.modifiers = KeyModifiers::CTRL;

        // The scroll processing checks modifiers internally
        let verb = processor.process_mouse(
            MouseEvent::Scroll {
                delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
                pos: Vec2::new(100.0, 100.0),
            },
            0,
        );
        assert!(matches!(verb, Some(Verb::Zoom(_))));
    }

    #[test]
    fn hover_entity_focus() {
        let mut processor = InputProcessor::new();
        processor.set_hover_entity(Some(42));

        // Double-click should focus the hover entity
        let template = VerbTemplate::FocusUnderCursor;
        let verb = processor.resolve_template(&template);
        assert!(matches!(verb, Some(Verb::Focus(42))));
    }

    #[test]
    fn key_repeat_timing() {
        let mut processor = InputProcessor::new();

        // Press key
        let _ = processor.process(RawInput::key_down(KeyCode::ArrowUp), 0);

        // Before repeat delay - no repeat
        let verb = processor.update(100);
        assert!(verb.is_none());

        // After repeat delay - should repeat
        let verb = processor.update(600);
        assert!(matches!(verb, Some(Verb::Ascend)));
    }
}
