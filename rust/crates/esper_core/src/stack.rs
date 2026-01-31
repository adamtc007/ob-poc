//! Context stack for cross-chamber navigation.
//!
//! When diving through doors into nested chambers, we push the current
//! context onto a stack. PullBack pops and restores the previous context.

use crate::verb::ChamberId;
use crate::Fault;
use crate::MAX_CONTEXT_DEPTH;
use esper_snapshot::Vec2;
use serde::{Deserialize, Serialize};

/// A saved navigation context (for stack).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextFrame {
    /// Chamber we came from.
    pub chamber_id: ChamberId,

    /// Camera position in that chamber.
    pub camera_pos: Vec2,

    /// Camera zoom in that chamber.
    pub camera_zoom: f32,

    /// Selected node index (if any).
    pub selection: Option<u32>,
}

impl ContextFrame {
    /// Create a new context frame.
    pub fn new(chamber_id: ChamberId, camera_pos: Vec2, camera_zoom: f32) -> Self {
        Self {
            chamber_id,
            camera_pos,
            camera_zoom,
            selection: None,
        }
    }

    /// Create with selection.
    pub fn with_selection(mut self, selection: Option<u32>) -> Self {
        self.selection = selection;
        self
    }
}

/// Stack of navigation contexts for nested navigation.
///
/// Maximum depth is enforced to prevent runaway navigation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextStack {
    frames: Vec<ContextFrame>,
}

impl ContextStack {
    /// Create an empty context stack.
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    /// Push a context frame onto the stack.
    ///
    /// Returns error if stack is at maximum depth.
    pub fn push(&mut self, frame: ContextFrame) -> Result<(), Fault> {
        if self.frames.len() >= MAX_CONTEXT_DEPTH {
            return Err(Fault::StackOverflow {
                max: MAX_CONTEXT_DEPTH,
            });
        }
        self.frames.push(frame);
        Ok(())
    }

    /// Pop a context frame from the stack.
    ///
    /// Returns error if stack is empty.
    pub fn pop(&mut self) -> Result<ContextFrame, Fault> {
        self.frames.pop().ok_or(Fault::StackUnderflow)
    }

    /// Peek at the top context without removing it.
    pub fn peek(&self) -> Option<&ContextFrame> {
        self.frames.last()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Get the current depth.
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Clear all contexts (return to root).
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Get the root chamber (first pushed, if any).
    pub fn root_chamber(&self) -> Option<ChamberId> {
        self.frames.first().map(|f| f.chamber_id)
    }

    /// Iterate over frames from bottom to top.
    pub fn iter(&self) -> impl Iterator<Item = &ContextFrame> {
        self.frames.iter()
    }

    /// Get breadcrumb path of chamber IDs.
    pub fn breadcrumbs(&self) -> Vec<ChamberId> {
        self.frames.iter().map(|f| f.chamber_id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_push_pop() {
        let mut stack = ContextStack::new();

        let frame1 = ContextFrame::new(1, Vec2::new(10.0, 10.0), 1.0);
        let frame2 = ContextFrame::new(2, Vec2::new(20.0, 20.0), 2.0);

        stack.push(frame1.clone()).unwrap();
        stack.push(frame2.clone()).unwrap();

        assert_eq!(stack.depth(), 2);

        let popped = stack.pop().unwrap();
        assert_eq!(popped.chamber_id, 2);

        let popped = stack.pop().unwrap();
        assert_eq!(popped.chamber_id, 1);

        assert!(stack.is_empty());
    }

    #[test]
    fn stack_underflow() {
        let mut stack = ContextStack::new();
        let result = stack.pop();
        assert!(matches!(result, Err(Fault::StackUnderflow)));
    }

    #[test]
    fn stack_overflow() {
        let mut stack = ContextStack::new();

        for i in 0..MAX_CONTEXT_DEPTH {
            let frame = ContextFrame::new(i as u32, Vec2::ZERO, 1.0);
            stack.push(frame).unwrap();
        }

        let extra = ContextFrame::new(999, Vec2::ZERO, 1.0);
        let result = stack.push(extra);
        assert!(matches!(result, Err(Fault::StackOverflow { .. })));
    }

    #[test]
    fn stack_peek() {
        let mut stack = ContextStack::new();

        assert!(stack.peek().is_none());

        let frame = ContextFrame::new(1, Vec2::new(5.0, 5.0), 1.5);
        stack.push(frame).unwrap();

        let peeked = stack.peek().unwrap();
        assert_eq!(peeked.chamber_id, 1);
        assert_eq!(stack.depth(), 1); // Still there
    }

    #[test]
    fn stack_breadcrumbs() {
        let mut stack = ContextStack::new();

        stack.push(ContextFrame::new(1, Vec2::ZERO, 1.0)).unwrap();
        stack.push(ContextFrame::new(3, Vec2::ZERO, 1.0)).unwrap();
        stack.push(ContextFrame::new(7, Vec2::ZERO, 1.0)).unwrap();

        let crumbs = stack.breadcrumbs();
        assert_eq!(crumbs, vec![1, 3, 7]);
    }

    #[test]
    fn stack_clear() {
        let mut stack = ContextStack::new();

        stack.push(ContextFrame::new(1, Vec2::ZERO, 1.0)).unwrap();
        stack.push(ContextFrame::new(2, Vec2::ZERO, 1.0)).unwrap();

        stack.clear();

        assert!(stack.is_empty());
        assert_eq!(stack.depth(), 0);
    }
}
