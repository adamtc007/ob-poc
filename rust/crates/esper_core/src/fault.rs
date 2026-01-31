//! Fault types for navigation errors.
//!
//! Faults are recoverable errors that occur during navigation.
//! They don't crash the system but indicate the verb couldn't be executed.

use crate::verb::{ChamberId, DoorId, EntityId, NodeIdx};
use thiserror::Error;

/// Navigation fault - a recoverable error during verb execution.
#[derive(Debug, Clone, Error)]
pub enum Fault {
    /// Attempted to pop from empty context stack.
    #[error("Context stack underflow: no context to pop")]
    StackUnderflow,

    /// Context stack at maximum depth.
    #[error("Context stack overflow: max depth {max} reached")]
    StackOverflow { max: usize },

    /// Referenced door doesn't exist.
    #[error("Door {0} not found")]
    DoorNotFound(DoorId),

    /// Referenced chamber doesn't exist.
    #[error("Chamber {0} not found")]
    ChamberNotFound(ChamberId),

    /// Referenced entity doesn't exist.
    #[error("Entity {0} not found")]
    EntityNotFound(EntityId),

    /// Referenced node doesn't exist.
    #[error("Node {0} not found")]
    NodeNotFound(NodeIdx),

    /// Navigation would create a cycle (door points back to current).
    #[error("Cyclic reference detected: chamber {0}")]
    CyclicReference(ChamberId),

    /// Verb requires a selection but none exists.
    #[error("No selection: verb requires a selected node")]
    NoSelection,

    /// No next sibling exists.
    #[error("No next sibling at node {0}")]
    NoNextSibling(NodeIdx),

    /// No previous sibling exists.
    #[error("No previous sibling at node {0}")]
    NoPrevSibling(NodeIdx),

    /// No children exist to descend into.
    #[error("No children at node {0}")]
    NoChildren(NodeIdx),

    /// No parent exists to ascend to.
    #[error("No parent: already at root")]
    NoParent,

    /// Empty world (no chambers).
    #[error("Empty world: no chambers to navigate")]
    EmptyWorld,

    /// Maximum navigation depth exceeded.
    #[error("Max depth {max} exceeded")]
    MaxDepthExceeded { max: usize },
}

impl Fault {
    /// Check if this fault is recoverable by trying a different action.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Fault::NoNextSibling(_)
                | Fault::NoPrevSibling(_)
                | Fault::NoChildren(_)
                | Fault::NoParent
                | Fault::NoSelection
        )
    }

    /// Check if this fault indicates a programming error.
    pub fn is_bug(&self) -> bool {
        matches!(
            self,
            Fault::DoorNotFound(_)
                | Fault::ChamberNotFound(_)
                | Fault::EntityNotFound(_)
                | Fault::NodeNotFound(_)
                | Fault::CyclicReference(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fault_is_recoverable() {
        assert!(Fault::NoNextSibling(0).is_recoverable());
        assert!(Fault::NoParent.is_recoverable());
        assert!(!Fault::DoorNotFound(1).is_recoverable());
        assert!(!Fault::ChamberNotFound(1).is_recoverable());
    }

    #[test]
    fn fault_is_bug() {
        assert!(Fault::DoorNotFound(1).is_bug());
        assert!(Fault::CyclicReference(1).is_bug());
        assert!(!Fault::NoNextSibling(0).is_bug());
        assert!(!Fault::NoParent.is_bug());
    }

    #[test]
    fn fault_display() {
        let fault = Fault::StackUnderflow;
        assert!(fault.to_string().contains("underflow"));

        let fault = Fault::DoorNotFound(42);
        assert!(fault.to_string().contains("42"));
    }
}
