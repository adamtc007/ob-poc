//! Mouse input types.

use esper_snapshot::Vec2;
use serde::{Deserialize, Serialize};

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    /// Primary button (usually left).
    Primary,
    /// Secondary button (usually right).
    Secondary,
    /// Middle button (wheel click).
    Middle,
    /// Extra button 1 (back).
    Back,
    /// Extra button 2 (forward).
    Forward,
}

impl MouseButton {
    /// Check if this is the primary button.
    pub fn is_primary(&self) -> bool {
        matches!(self, MouseButton::Primary)
    }

    /// Check if this is the secondary button.
    pub fn is_secondary(&self) -> bool {
        matches!(self, MouseButton::Secondary)
    }
}

/// Mouse scroll delta.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScrollDelta {
    /// Line-based scrolling (typical for wheel).
    Lines { x: f32, y: f32 },
    /// Pixel-based scrolling (trackpad).
    Pixels { x: f32, y: f32 },
}

impl ScrollDelta {
    /// Get the vertical scroll amount normalized to lines.
    pub fn y_lines(&self) -> f32 {
        match self {
            ScrollDelta::Lines { y, .. } => *y,
            ScrollDelta::Pixels { y, .. } => *y / 20.0, // Approximate
        }
    }

    /// Get the horizontal scroll amount normalized to lines.
    pub fn x_lines(&self) -> f32 {
        match self {
            ScrollDelta::Lines { x, .. } => *x,
            ScrollDelta::Pixels { x, .. } => *x / 20.0, // Approximate
        }
    }

    /// Check if this is a zoom gesture (Ctrl+scroll or pinch).
    pub fn is_zoom_gesture(&self, ctrl_held: bool) -> bool {
        ctrl_held
            && match self {
                ScrollDelta::Lines { y, .. } | ScrollDelta::Pixels { y, .. } => y.abs() > 0.0,
            }
    }
}

impl Default for ScrollDelta {
    fn default() -> Self {
        ScrollDelta::Lines { x: 0.0, y: 0.0 }
    }
}

/// Mouse event types.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseEvent {
    /// Mouse moved to position.
    Move { pos: Vec2 },

    /// Button pressed.
    ButtonDown { button: MouseButton, pos: Vec2 },

    /// Button released.
    ButtonUp { button: MouseButton, pos: Vec2 },

    /// Mouse wheel scrolled.
    Scroll { delta: ScrollDelta, pos: Vec2 },

    /// Mouse entered the window.
    Enter { pos: Vec2 },

    /// Mouse left the window.
    Leave,
}

impl MouseEvent {
    /// Get the position if this event has one.
    pub fn position(&self) -> Option<Vec2> {
        match self {
            MouseEvent::Move { pos }
            | MouseEvent::ButtonDown { pos, .. }
            | MouseEvent::ButtonUp { pos, .. }
            | MouseEvent::Scroll { pos, .. }
            | MouseEvent::Enter { pos } => Some(*pos),
            MouseEvent::Leave => None,
        }
    }

    /// Check if this is a button down event.
    pub fn is_button_down(&self) -> bool {
        matches!(self, MouseEvent::ButtonDown { .. })
    }

    /// Check if this is a button up event.
    pub fn is_button_up(&self) -> bool {
        matches!(self, MouseEvent::ButtonUp { .. })
    }

    /// Check if this is a scroll event.
    pub fn is_scroll(&self) -> bool {
        matches!(self, MouseEvent::Scroll { .. })
    }
}

/// Tracked mouse state for drag detection and gestures.
#[derive(Debug, Clone, Default)]
pub struct MouseState {
    /// Current mouse position.
    pub position: Vec2,
    /// Whether primary button is held.
    pub primary_down: bool,
    /// Whether secondary button is held.
    pub secondary_down: bool,
    /// Whether middle button is held.
    pub middle_down: bool,
    /// Position where primary button was pressed.
    pub drag_start: Option<Vec2>,
    /// Whether a drag is in progress.
    pub dragging: bool,
    /// Time of last click (for double-click detection).
    pub last_click_time: Option<u64>,
    /// Position of last click.
    pub last_click_pos: Option<Vec2>,
}

impl MouseState {
    /// Update state from a mouse event.
    pub fn update(&mut self, event: &MouseEvent, timestamp: u64, drag_threshold: f32) {
        if let Some(pos) = event.position() {
            self.position = pos;
        }

        match event {
            MouseEvent::ButtonDown { button, pos } => {
                if button.is_primary() {
                    self.primary_down = true;
                    self.drag_start = Some(*pos);
                    self.dragging = false;
                } else if button.is_secondary() {
                    self.secondary_down = true;
                } else if matches!(button, MouseButton::Middle) {
                    self.middle_down = true;
                }
            }
            MouseEvent::ButtonUp { button, pos } => {
                if button.is_primary() {
                    self.primary_down = false;
                    self.drag_start = None;
                    self.dragging = false;

                    // Track click for double-click detection
                    self.last_click_time = Some(timestamp);
                    self.last_click_pos = Some(*pos);
                } else if button.is_secondary() {
                    self.secondary_down = false;
                } else if matches!(button, MouseButton::Middle) {
                    self.middle_down = false;
                }
            }
            MouseEvent::Move { pos } => {
                // Check for drag start
                if self.primary_down && !self.dragging {
                    if let Some(start) = self.drag_start {
                        let dist = (pos.x - start.x).hypot(pos.y - start.y);
                        if dist > drag_threshold {
                            self.dragging = true;
                        }
                    }
                }
            }
            MouseEvent::Leave => {
                // Reset button states on leave
                self.primary_down = false;
                self.secondary_down = false;
                self.middle_down = false;
                self.dragging = false;
                self.drag_start = None;
            }
            _ => {}
        }
    }

    /// Check if this is a double-click.
    pub fn is_double_click(&self, timestamp: u64, pos: Vec2, threshold_ms: u64) -> bool {
        if let (Some(last_time), Some(last_pos)) = (self.last_click_time, self.last_click_pos) {
            let time_ok = timestamp.saturating_sub(last_time) < threshold_ms;
            let pos_ok = (pos.x - last_pos.x).hypot(pos.y - last_pos.y) < 5.0;
            time_ok && pos_ok
        } else {
            false
        }
    }

    /// Get drag delta if currently dragging.
    pub fn drag_delta(&self) -> Option<Vec2> {
        if self.dragging {
            self.drag_start.map(|start| Vec2 {
                x: self.position.x - start.x,
                y: self.position.y - start.y,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_delta_lines() {
        let scroll = ScrollDelta::Lines { x: 1.0, y: -2.0 };
        assert_eq!(scroll.x_lines(), 1.0);
        assert_eq!(scroll.y_lines(), -2.0);

        let scroll = ScrollDelta::Pixels { x: 20.0, y: -40.0 };
        assert_eq!(scroll.x_lines(), 1.0);
        assert_eq!(scroll.y_lines(), -2.0);
    }

    #[test]
    fn mouse_state_drag() {
        let mut state = MouseState::default();
        let threshold = 5.0;

        // Press button
        state.update(
            &MouseEvent::ButtonDown {
                button: MouseButton::Primary,
                pos: Vec2::new(10.0, 10.0),
            },
            0,
            threshold,
        );
        assert!(state.primary_down);
        assert!(!state.dragging);

        // Move small amount (no drag)
        state.update(
            &MouseEvent::Move {
                pos: Vec2::new(12.0, 10.0),
            },
            1,
            threshold,
        );
        assert!(!state.dragging);

        // Move more (start drag)
        state.update(
            &MouseEvent::Move {
                pos: Vec2::new(20.0, 10.0),
            },
            2,
            threshold,
        );
        assert!(state.dragging);

        // Check drag delta
        let delta = state.drag_delta().unwrap();
        assert_eq!(delta.x, 10.0);
        assert_eq!(delta.y, 0.0);
    }

    #[test]
    fn double_click_detection() {
        let mut state = MouseState::default();
        let threshold = 5.0;

        // First click
        state.update(
            &MouseEvent::ButtonDown {
                button: MouseButton::Primary,
                pos: Vec2::new(10.0, 10.0),
            },
            0,
            threshold,
        );
        state.update(
            &MouseEvent::ButtonUp {
                button: MouseButton::Primary,
                pos: Vec2::new(10.0, 10.0),
            },
            50,
            threshold,
        );

        // Second click (within threshold)
        assert!(state.is_double_click(200, Vec2::new(10.0, 10.0), 300));

        // Not a double click (too far apart in time)
        assert!(!state.is_double_click(500, Vec2::new(10.0, 10.0), 300));
    }
}
