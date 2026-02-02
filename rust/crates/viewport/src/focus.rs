//! Focus management and navigation
//!
//! This module provides focus transition logic for navigating the CBU hierarchy.
//!
//! Note: FocusManager is defined in ob-poc-types and has its own methods.
//! This module provides the FocusTransition enum for declarative transitions
//! and free functions that work with FocusManager.

use ob_poc_types::viewport::{
    CbuRef, ConcreteEntityRef, ConfigNodeRef, FocusManager, FocusMode, InstrumentMatrixRef,
    InstrumentType, ProductServiceRef, ViewportFocusState,
};

/// Describes a focus transition operation
#[derive(Debug, Clone, PartialEq)]
pub enum FocusTransition {
    /// Clear all focus
    Clear,

    /// Focus on a CBU container
    FocusCbu { cbu: CbuRef, enhance_level: u8 },

    /// Focus on an entity within a CBU
    FocusEntity {
        cbu: CbuRef,
        entity: ConcreteEntityRef,
        entity_enhance: u8,
        container_enhance: u8,
    },

    /// Focus on a product/service within a CBU
    FocusProductService {
        cbu: CbuRef,
        target: ProductServiceRef,
        target_enhance: u8,
        container_enhance: u8,
    },

    /// Focus on the instrument matrix
    FocusMatrix {
        cbu: CbuRef,
        matrix: InstrumentMatrixRef,
        matrix_enhance: u8,
        container_enhance: u8,
    },

    /// Focus on an instrument type within the matrix
    FocusInstrumentType {
        cbu: CbuRef,
        matrix: InstrumentMatrixRef,
        instrument_type: InstrumentType,
        type_enhance: u8,
        matrix_enhance: u8,
        container_enhance: u8,
    },

    /// Focus on a config node (MIC, BIC, Pricing)
    FocusConfigNode {
        cbu: CbuRef,
        matrix: InstrumentMatrixRef,
        instrument_type: InstrumentType,
        config_node: ConfigNodeRef,
        node_enhance: u8,
        type_enhance: u8,
        matrix_enhance: u8,
        container_enhance: u8,
    },

    /// Ascend to parent focus (pop stack)
    Ascend,

    /// Descend into child (push current to stack)
    Descend(Box<FocusTransition>),

    /// Change enhance level without changing focus target
    Enhance { delta: i8 },

    /// Set enhance level to specific value
    EnhanceSet { level: u8 },

    /// Set enhance to maximum for current focus type
    EnhanceMax,

    /// Reset enhance to minimum (L0)
    EnhanceReset,
}

/// Apply a focus transition to a FocusManager
///
/// Returns a reference to the new state after applying the transition.
pub fn apply_transition(
    manager: &mut FocusManager,
    transition: FocusTransition,
) -> &ViewportFocusState {
    match transition {
        FocusTransition::Clear => {
            manager.focus_stack.clear();
            manager.state = ViewportFocusState::None;
        }

        FocusTransition::FocusCbu { cbu, enhance_level } => {
            manager.state = ViewportFocusState::CbuContainer { cbu, enhance_level };
        }

        FocusTransition::FocusEntity {
            cbu,
            entity,
            entity_enhance,
            container_enhance,
        } => {
            manager.state = ViewportFocusState::CbuEntity {
                cbu,
                entity,
                entity_enhance,
                container_enhance,
            };
        }

        FocusTransition::FocusProductService {
            cbu,
            target,
            target_enhance,
            container_enhance,
        } => {
            manager.state = ViewportFocusState::CbuProductService {
                cbu,
                target,
                target_enhance,
                container_enhance,
            };
        }

        FocusTransition::FocusMatrix {
            cbu,
            matrix,
            matrix_enhance,
            container_enhance,
        } => {
            manager.state = ViewportFocusState::InstrumentMatrix {
                cbu,
                matrix,
                matrix_enhance,
                container_enhance,
            };
        }

        FocusTransition::FocusInstrumentType {
            cbu,
            matrix,
            instrument_type,
            type_enhance,
            matrix_enhance,
            container_enhance,
        } => {
            manager.state = ViewportFocusState::InstrumentType {
                cbu,
                matrix,
                instrument_type,
                type_enhance,
                matrix_enhance,
                container_enhance,
            };
        }

        FocusTransition::FocusConfigNode {
            cbu,
            matrix,
            instrument_type,
            config_node,
            node_enhance,
            type_enhance,
            matrix_enhance,
            container_enhance,
        } => {
            manager.state = ViewportFocusState::ConfigNode {
                cbu,
                matrix,
                instrument_type,
                config_node,
                node_enhance,
                type_enhance,
                matrix_enhance,
                container_enhance,
            };
        }

        FocusTransition::Ascend => {
            manager.ascend();
        }

        FocusTransition::Descend(child_transition) => {
            // Push current state to stack
            manager.focus_stack.push(manager.state.clone());
            // Apply the child transition
            apply_transition(manager, *child_transition);
        }

        FocusTransition::Enhance { delta } => {
            adjust_enhance_level(manager, delta);
        }

        FocusTransition::EnhanceSet { level } => {
            set_enhance_level(manager, level);
        }

        FocusTransition::EnhanceMax => {
            let max = max_enhance_level(&manager.state);
            set_enhance_level(manager, max);
        }

        FocusTransition::EnhanceReset => {
            set_enhance_level(manager, 0);
        }
    }
    &manager.state
}

/// Get the maximum enhance level for the current focus type
pub fn max_enhance_level(state: &ViewportFocusState) -> u8 {
    state.max_enhance_level()
}

/// Get the current enhance level
pub fn current_enhance_level(state: &ViewportFocusState) -> u8 {
    state.primary_enhance_level()
}

/// Adjust the enhance level by a delta
fn adjust_enhance_level(manager: &mut FocusManager, delta: i8) {
    let current = manager.state.primary_enhance_level() as i8;
    let max = manager.state.max_enhance_level() as i8;
    let new_level = (current + delta).clamp(0, max) as u8;
    set_enhance_level(manager, new_level);
}

/// Set the enhance level directly
fn set_enhance_level(manager: &mut FocusManager, level: u8) {
    let max = manager.state.max_enhance_level();
    let clamped = level.min(max);

    match &mut manager.state {
        ViewportFocusState::None => {}
        ViewportFocusState::CbuContainer { enhance_level, .. } => {
            *enhance_level = clamped;
        }
        ViewportFocusState::CbuEntity { entity_enhance, .. } => {
            *entity_enhance = clamped;
        }
        ViewportFocusState::CbuProductService { target_enhance, .. } => {
            *target_enhance = clamped;
        }
        ViewportFocusState::InstrumentMatrix { matrix_enhance, .. } => {
            *matrix_enhance = clamped;
        }
        ViewportFocusState::InstrumentType { type_enhance, .. } => {
            *type_enhance = clamped;
        }
        ViewportFocusState::ConfigNode { node_enhance, .. } => {
            *node_enhance = clamped;
        }
        ViewportFocusState::BoardControl { enhance_level, .. } => {
            *enhance_level = clamped;
        }
    }
}

/// Clear the focus stack without changing current focus
pub fn clear_stack(manager: &mut FocusManager) {
    manager.focus_stack.clear();
}

/// Set the focus mode
pub fn set_mode(manager: &mut FocusManager, mode: FocusMode) {
    manager.focus_mode = mode;
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_cbu() -> CbuRef {
        CbuRef(Uuid::now_v7())
    }

    #[test]
    fn test_focus_cbu_transition() {
        let mut manager = FocusManager::default();

        apply_transition(
            &mut manager,
            FocusTransition::FocusCbu {
                cbu: test_cbu(),
                enhance_level: 0,
            },
        );

        assert!(matches!(
            manager.state,
            ViewportFocusState::CbuContainer { .. }
        ));
        assert_eq!(current_enhance_level(&manager.state), 0);
        assert_eq!(max_enhance_level(&manager.state), 2);
    }

    #[test]
    fn test_enhance_increment() {
        let mut manager = FocusManager::default();

        apply_transition(
            &mut manager,
            FocusTransition::FocusCbu {
                cbu: test_cbu(),
                enhance_level: 0,
            },
        );

        apply_transition(&mut manager, FocusTransition::Enhance { delta: 1 });
        assert_eq!(current_enhance_level(&manager.state), 1);

        apply_transition(&mut manager, FocusTransition::Enhance { delta: 1 });
        assert_eq!(current_enhance_level(&manager.state), 2);

        // Should cap at max
        apply_transition(&mut manager, FocusTransition::Enhance { delta: 1 });
        assert_eq!(current_enhance_level(&manager.state), 2);
    }

    #[test]
    fn test_enhance_max() {
        let mut manager = FocusManager::default();

        apply_transition(
            &mut manager,
            FocusTransition::FocusCbu {
                cbu: test_cbu(),
                enhance_level: 0,
            },
        );

        apply_transition(&mut manager, FocusTransition::EnhanceMax);
        assert_eq!(current_enhance_level(&manager.state), 2);
    }

    #[test]
    fn test_descend_and_ascend() {
        let mut manager = FocusManager::default();
        let cbu = test_cbu();

        // Focus CBU
        apply_transition(
            &mut manager,
            FocusTransition::FocusCbu {
                cbu: cbu.clone(),
                enhance_level: 1,
            },
        );

        // Descend into matrix
        apply_transition(
            &mut manager,
            FocusTransition::Descend(Box::new(FocusTransition::FocusMatrix {
                cbu: cbu.clone(),
                matrix: InstrumentMatrixRef(Uuid::now_v7()),
                matrix_enhance: 0,
                container_enhance: 1,
            })),
        );

        assert!(matches!(
            manager.state,
            ViewportFocusState::InstrumentMatrix { .. }
        ));
        assert!(manager.can_ascend());
        assert_eq!(manager.stack_depth(), 1);

        // Ascend back to CBU
        apply_transition(&mut manager, FocusTransition::Ascend);

        assert!(matches!(
            manager.state,
            ViewportFocusState::CbuContainer { .. }
        ));
        // Stack is empty after ascending, but can_ascend() returns true
        // because state != None (CbuContainer is a valid focus state)
        assert_eq!(manager.stack_depth(), 0);
        // Note: can_ascend() is true because we're in a non-None state,
        // but ascending would pop nothing (stack is empty) and state remains
    }

    #[test]
    fn test_clear_transition() {
        let mut manager = FocusManager::default();

        apply_transition(
            &mut manager,
            FocusTransition::FocusCbu {
                cbu: test_cbu(),
                enhance_level: 1,
            },
        );

        apply_transition(&mut manager, FocusTransition::Clear);

        assert!(matches!(manager.state, ViewportFocusState::None));
        assert!(!manager.can_ascend());
    }
}
