//! Gesture recognition for touch and multi-touch input.

use crate::{DEFAULT_DOUBLE_CLICK_MS, DEFAULT_DRAG_THRESHOLD, DEFAULT_PINCH_THRESHOLD};
use esper_snapshot::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Recognized gesture types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Gesture {
    /// Single tap at position.
    Tap { pos: Vec2 },

    /// Double tap at position.
    DoubleTap { pos: Vec2 },

    /// Long press at position.
    LongPress { pos: Vec2 },

    /// Single-finger drag.
    Drag {
        start: Vec2,
        current: Vec2,
        delta: Vec2,
    },

    /// Two-finger pinch (zoom).
    Pinch {
        center: Vec2,
        scale: f32,
        rotation: f32,
    },

    /// Two-finger pan.
    TwoFingerPan { delta: Vec2 },

    /// Swipe in direction.
    Swipe {
        direction: SwipeDirection,
        velocity: f32,
    },

    /// Edge swipe (from screen edge).
    EdgeSwipe { edge: Edge, progress: f32 },
}

/// Swipe direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Screen edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// State of gesture recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum GestureState {
    /// No gesture in progress.
    #[default]
    None,
    /// Gesture is starting/possible.
    Possible,
    /// Gesture recognized and in progress.
    Active,
    /// Gesture completed successfully.
    Completed,
    /// Gesture cancelled.
    Cancelled,
}

/// Touch point tracking.
#[derive(Debug, Clone)]
struct TouchPoint {
    #[allow(dead_code)]
    id: u64,
    start_pos: Vec2,
    current_pos: Vec2,
    start_time: u64,
    last_update: u64,
}

/// Gesture recognizer for touch input.
#[derive(Debug)]
pub struct GestureRecognizer {
    /// Active touch points.
    touches: HashMap<u64, TouchPoint>,
    /// Current gesture state.
    state: GestureState,
    /// Last recognized gesture.
    last_gesture: Option<Gesture>,
    /// Configuration.
    config: GestureConfig,
    /// Initial pinch distance (for scale calculation).
    initial_pinch_distance: Option<f32>,
    /// Initial pinch angle (for rotation calculation).
    initial_pinch_angle: Option<f32>,
    /// Last tap time (for double-tap detection).
    last_tap_time: Option<u64>,
    /// Last tap position.
    last_tap_pos: Option<Vec2>,
}

/// Gesture recognizer configuration.
#[derive(Debug, Clone)]
pub struct GestureConfig {
    /// Minimum drag distance.
    pub drag_threshold: f32,
    /// Minimum pinch distance change.
    pub pinch_threshold: f32,
    /// Double-tap time window (ms).
    pub double_tap_ms: u64,
    /// Long press duration (ms).
    pub long_press_ms: u64,
    /// Swipe velocity threshold.
    pub swipe_velocity: f32,
    /// Edge swipe detection zone (pixels from edge).
    pub edge_zone: f32,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            drag_threshold: DEFAULT_DRAG_THRESHOLD,
            pinch_threshold: DEFAULT_PINCH_THRESHOLD,
            double_tap_ms: DEFAULT_DOUBLE_CLICK_MS,
            long_press_ms: 500,
            swipe_velocity: 500.0,
            edge_zone: 20.0,
        }
    }
}

impl GestureRecognizer {
    /// Create a new gesture recognizer.
    pub fn new() -> Self {
        Self::with_config(GestureConfig::default())
    }

    /// Create with custom configuration.
    pub fn with_config(config: GestureConfig) -> Self {
        Self {
            touches: HashMap::new(),
            state: GestureState::None,
            last_gesture: None,
            config,
            initial_pinch_distance: None,
            initial_pinch_angle: None,
            last_tap_time: None,
            last_tap_pos: None,
        }
    }

    /// Get current gesture state.
    pub fn state(&self) -> GestureState {
        self.state
    }

    /// Get last recognized gesture.
    pub fn gesture(&self) -> Option<&Gesture> {
        self.last_gesture.as_ref()
    }

    /// Get number of active touch points.
    pub fn touch_count(&self) -> usize {
        self.touches.len()
    }

    /// Handle touch start.
    pub fn touch_start(&mut self, id: u64, pos: Vec2, timestamp: u64) -> Option<Gesture> {
        let touch = TouchPoint {
            id,
            start_pos: pos,
            current_pos: pos,
            start_time: timestamp,
            last_update: timestamp,
        };
        self.touches.insert(id, touch);

        if self.touches.len() == 1 {
            self.state = GestureState::Possible;
        } else if self.touches.len() == 2 {
            // Start tracking pinch
            self.initial_pinch_distance = self.two_finger_distance();
            self.initial_pinch_angle = self.two_finger_angle();
        }

        None
    }

    /// Handle touch move.
    pub fn touch_move(&mut self, id: u64, pos: Vec2, timestamp: u64) -> Option<Gesture> {
        if let Some(touch) = self.touches.get_mut(&id) {
            touch.current_pos = pos;
            touch.last_update = timestamp;
        }

        match self.touches.len() {
            1 => self.check_drag(),
            2 => self.check_pinch_or_pan(),
            _ => None,
        }
    }

    /// Handle touch end.
    pub fn touch_end(&mut self, id: u64, pos: Vec2, timestamp: u64) -> Option<Gesture> {
        let touch = self.touches.remove(&id);

        if self.touches.is_empty() {
            self.state = GestureState::None;

            if let Some(t) = touch {
                let dist = distance(t.start_pos, pos);
                let duration = timestamp.saturating_sub(t.start_time);

                // Check for tap
                if dist < self.config.drag_threshold && duration < 200 {
                    // Check for double tap
                    if let (Some(last_time), Some(last_pos)) =
                        (self.last_tap_time, self.last_tap_pos)
                    {
                        let tap_interval = timestamp.saturating_sub(last_time);
                        let tap_distance = distance(pos, last_pos);

                        if tap_interval < self.config.double_tap_ms
                            && tap_distance < self.config.drag_threshold * 2.0
                        {
                            self.last_tap_time = None;
                            self.last_tap_pos = None;
                            self.state = GestureState::Completed;
                            let gesture = Gesture::DoubleTap { pos };
                            self.last_gesture = Some(gesture.clone());
                            return Some(gesture);
                        }
                    }

                    self.last_tap_time = Some(timestamp);
                    self.last_tap_pos = Some(pos);
                    self.state = GestureState::Completed;
                    let gesture = Gesture::Tap { pos };
                    self.last_gesture = Some(gesture.clone());
                    return Some(gesture);
                }

                // Check for swipe
                if let Some(gesture) = self.check_swipe(&t, pos, timestamp) {
                    return Some(gesture);
                }
            }
        } else if self.touches.len() == 1 {
            // Reset pinch tracking
            self.initial_pinch_distance = None;
            self.initial_pinch_angle = None;
        }

        None
    }

    /// Handle touch cancel.
    pub fn touch_cancel(&mut self, id: u64) {
        self.touches.remove(&id);
        if self.touches.is_empty() {
            self.state = GestureState::Cancelled;
        }
    }

    /// Reset the recognizer.
    pub fn reset(&mut self) {
        self.touches.clear();
        self.state = GestureState::None;
        self.last_gesture = None;
        self.initial_pinch_distance = None;
        self.initial_pinch_angle = None;
    }

    // =========================================================================
    // INTERNAL HELPERS
    // =========================================================================

    fn check_drag(&mut self) -> Option<Gesture> {
        let touch = self.touches.values().next()?;
        let delta = Vec2 {
            x: touch.current_pos.x - touch.start_pos.x,
            y: touch.current_pos.y - touch.start_pos.y,
        };
        let dist = distance(touch.start_pos, touch.current_pos);

        if dist > self.config.drag_threshold {
            self.state = GestureState::Active;
            let gesture = Gesture::Drag {
                start: touch.start_pos,
                current: touch.current_pos,
                delta,
            };
            self.last_gesture = Some(gesture.clone());
            Some(gesture)
        } else {
            None
        }
    }

    fn check_pinch_or_pan(&mut self) -> Option<Gesture> {
        let touches: Vec<_> = self.touches.values().collect();
        if touches.len() != 2 {
            return None;
        }

        let current_dist = self.two_finger_distance()?;
        let initial_dist = self.initial_pinch_distance?;

        let scale = current_dist / initial_dist;
        let scale_change = (scale - 1.0).abs();

        if scale_change > self.config.pinch_threshold / initial_dist {
            self.state = GestureState::Active;

            let center = Vec2 {
                x: (touches[0].current_pos.x + touches[1].current_pos.x) / 2.0,
                y: (touches[0].current_pos.y + touches[1].current_pos.y) / 2.0,
            };

            let current_angle = self.two_finger_angle().unwrap_or(0.0);
            let initial_angle = self.initial_pinch_angle.unwrap_or(0.0);
            let rotation = current_angle - initial_angle;

            let gesture = Gesture::Pinch {
                center,
                scale,
                rotation,
            };
            self.last_gesture = Some(gesture.clone());
            Some(gesture)
        } else {
            // Check for two-finger pan
            let delta = Vec2 {
                x: (touches[0].current_pos.x - touches[0].start_pos.x + touches[1].current_pos.x
                    - touches[1].start_pos.x)
                    / 2.0,
                y: (touches[0].current_pos.y - touches[0].start_pos.y + touches[1].current_pos.y
                    - touches[1].start_pos.y)
                    / 2.0,
            };

            if delta.x.hypot(delta.y) > self.config.drag_threshold {
                self.state = GestureState::Active;
                let gesture = Gesture::TwoFingerPan { delta };
                self.last_gesture = Some(gesture.clone());
                Some(gesture)
            } else {
                None
            }
        }
    }

    fn check_swipe(
        &mut self,
        touch: &TouchPoint,
        end_pos: Vec2,
        timestamp: u64,
    ) -> Option<Gesture> {
        let delta = Vec2 {
            x: end_pos.x - touch.start_pos.x,
            y: end_pos.y - touch.start_pos.y,
        };
        let dist = distance(touch.start_pos, end_pos);
        let duration_s = (timestamp.saturating_sub(touch.start_time)) as f32 / 1000.0;

        if duration_s < 0.001 {
            return None;
        }

        let velocity = dist / duration_s;

        if velocity > self.config.swipe_velocity && dist > self.config.drag_threshold * 3.0 {
            let direction = if delta.x.abs() > delta.y.abs() {
                if delta.x > 0.0 {
                    SwipeDirection::Right
                } else {
                    SwipeDirection::Left
                }
            } else if delta.y > 0.0 {
                SwipeDirection::Down
            } else {
                SwipeDirection::Up
            };

            self.state = GestureState::Completed;
            let gesture = Gesture::Swipe {
                direction,
                velocity,
            };
            self.last_gesture = Some(gesture.clone());
            Some(gesture)
        } else {
            None
        }
    }

    fn two_finger_distance(&self) -> Option<f32> {
        let touches: Vec<_> = self.touches.values().collect();
        if touches.len() != 2 {
            return None;
        }
        Some(distance(touches[0].current_pos, touches[1].current_pos))
    }

    fn two_finger_angle(&self) -> Option<f32> {
        let touches: Vec<_> = self.touches.values().collect();
        if touches.len() != 2 {
            return None;
        }
        let dx = touches[1].current_pos.x - touches[0].current_pos.x;
        let dy = touches[1].current_pos.y - touches[0].current_pos.y;
        Some(dy.atan2(dx))
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

fn distance(a: Vec2, b: Vec2) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_tap() {
        let mut recognizer = GestureRecognizer::new();

        recognizer.touch_start(1, Vec2::new(100.0, 100.0), 0);
        let gesture = recognizer.touch_end(1, Vec2::new(100.0, 100.0), 100);

        assert!(matches!(gesture, Some(Gesture::Tap { .. })));
        assert_eq!(recognizer.state(), GestureState::Completed);
    }

    #[test]
    fn double_tap() {
        let mut recognizer = GestureRecognizer::new();

        // First tap
        recognizer.touch_start(1, Vec2::new(100.0, 100.0), 0);
        let _ = recognizer.touch_end(1, Vec2::new(100.0, 100.0), 100);

        // Second tap
        recognizer.touch_start(2, Vec2::new(100.0, 100.0), 150);
        let gesture = recognizer.touch_end(2, Vec2::new(100.0, 100.0), 250);

        assert!(matches!(gesture, Some(Gesture::DoubleTap { .. })));
    }

    #[test]
    fn drag_gesture() {
        let mut recognizer = GestureRecognizer::new();

        recognizer.touch_start(1, Vec2::new(100.0, 100.0), 0);

        // Move beyond threshold
        let gesture = recognizer.touch_move(1, Vec2::new(150.0, 100.0), 100);

        assert!(matches!(gesture, Some(Gesture::Drag { .. })));
        assert_eq!(recognizer.state(), GestureState::Active);
    }

    #[test]
    fn pinch_gesture() {
        let mut recognizer = GestureRecognizer::new();

        // Two fingers down
        recognizer.touch_start(1, Vec2::new(100.0, 100.0), 0);
        recognizer.touch_start(2, Vec2::new(200.0, 100.0), 0);

        // Move fingers apart (zoom in)
        recognizer.touch_move(1, Vec2::new(50.0, 100.0), 100);
        let gesture = recognizer.touch_move(2, Vec2::new(250.0, 100.0), 100);

        assert!(matches!(gesture, Some(Gesture::Pinch { scale, .. }) if scale > 1.0));
    }

    #[test]
    fn swipe_gesture() {
        let mut recognizer = GestureRecognizer::new();

        recognizer.touch_start(1, Vec2::new(100.0, 100.0), 0);
        // Fast horizontal movement
        let gesture = recognizer.touch_end(1, Vec2::new(400.0, 100.0), 200);

        assert!(matches!(
            gesture,
            Some(Gesture::Swipe {
                direction: SwipeDirection::Right,
                ..
            })
        ));
    }
}
