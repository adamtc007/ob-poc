//! State machine transitions and validation
//!
//! This module defines valid state transitions and provides error types
//! for invalid transitions.

use ob_poc_types::viewport::{
    CbuRef, ConcreteEntityRef, ConfigNodeRef, InstrumentMatrixRef, InstrumentType,
    ProductServiceRef, ViewportFocusState,
};
use thiserror::Error;

/// Error types for invalid state transitions
#[derive(Debug, Error, Clone, PartialEq)]
pub enum TransitionError {
    #[error("Cannot ascend: focus stack is empty")]
    CannotAscend,

    #[error("Cannot descend from None focus state")]
    CannotDescendFromNone,

    #[error("Invalid target for current focus: {0}")]
    InvalidTarget(String),

    #[error("No CBU context for operation")]
    NoCbuContext,

    #[error("No matrix context for instrument type focus")]
    NoMatrixContext,

    #[error("No instrument type context for config node focus")]
    NoInstrumentTypeContext,

    #[error("Enhance level {requested} exceeds maximum {max}")]
    EnhanceLevelExceeded { requested: u8, max: u8 },

    #[error("Cannot reduce enhance level below 0")]
    CannotReduceBelowZero,
}

/// Result type for state transitions
pub type TransitionResult<T> = Result<T, TransitionError>;

/// Validates whether a transition is valid from the current state
pub struct TransitionValidator;

impl TransitionValidator {
    /// Check if we can focus on an entity from the current state
    pub fn can_focus_entity(
        current: &ViewportFocusState,
        _entity: &ConcreteEntityRef,
    ) -> TransitionResult<()> {
        match current {
            ViewportFocusState::None => Err(TransitionError::NoCbuContext),
            ViewportFocusState::CbuContainer { .. } => Ok(()),
            ViewportFocusState::CbuEntity { .. } => Ok(()), // Can switch between entities
            ViewportFocusState::CbuProductService { .. } => Ok(()),
            ViewportFocusState::InstrumentMatrix { .. } => Ok(()),
            ViewportFocusState::InstrumentType { .. } => Ok(()),
            ViewportFocusState::ConfigNode { .. } => Ok(()),
            ViewportFocusState::BoardControl { .. } => Ok(()),
        }
    }

    /// Check if we can focus on a product/service from the current state
    pub fn can_focus_product_service(
        current: &ViewportFocusState,
        _target: &ProductServiceRef,
    ) -> TransitionResult<()> {
        match current {
            ViewportFocusState::None => Err(TransitionError::NoCbuContext),
            ViewportFocusState::CbuContainer { .. } => Ok(()),
            ViewportFocusState::CbuEntity { .. } => Ok(()),
            ViewportFocusState::CbuProductService { .. } => Ok(()), // Can switch
            ViewportFocusState::InstrumentMatrix { .. } => Ok(()),
            ViewportFocusState::InstrumentType { .. } => Ok(()),
            ViewportFocusState::ConfigNode { .. } => Ok(()),
            ViewportFocusState::BoardControl { .. } => Ok(()),
        }
    }

    /// Check if we can focus on the instrument matrix from the current state
    pub fn can_focus_matrix(current: &ViewportFocusState) -> TransitionResult<CbuRef> {
        match current {
            ViewportFocusState::None => Err(TransitionError::NoCbuContext),
            ViewportFocusState::CbuContainer { cbu, .. } => Ok(cbu.clone()),
            ViewportFocusState::CbuEntity { cbu, .. } => Ok(cbu.clone()),
            ViewportFocusState::CbuProductService { cbu, .. } => Ok(cbu.clone()),
            ViewportFocusState::InstrumentMatrix { cbu, .. } => Ok(cbu.clone()),
            ViewportFocusState::InstrumentType { cbu, .. } => Ok(cbu.clone()),
            ViewportFocusState::ConfigNode { cbu, .. } => Ok(cbu.clone()),
            ViewportFocusState::BoardControl { source_cbu, .. } => Ok(source_cbu.clone()),
        }
    }

    /// Check if we can focus on an instrument type from the current state
    pub fn can_focus_instrument_type(
        current: &ViewportFocusState,
        _instrument_type: &InstrumentType,
    ) -> TransitionResult<(CbuRef, InstrumentMatrixRef)> {
        match current {
            ViewportFocusState::None => Err(TransitionError::NoCbuContext),
            ViewportFocusState::CbuContainer { .. } => Err(TransitionError::NoMatrixContext),
            ViewportFocusState::CbuEntity { .. } => Err(TransitionError::NoMatrixContext),
            ViewportFocusState::CbuProductService { .. } => Err(TransitionError::NoMatrixContext),
            ViewportFocusState::InstrumentMatrix { cbu, matrix, .. } => {
                Ok((cbu.clone(), matrix.clone()))
            }
            ViewportFocusState::InstrumentType { cbu, matrix, .. } => {
                Ok((cbu.clone(), matrix.clone()))
            }
            ViewportFocusState::ConfigNode { cbu, matrix, .. } => Ok((cbu.clone(), matrix.clone())),
            ViewportFocusState::BoardControl { .. } => Err(TransitionError::NoMatrixContext),
        }
    }

    /// Check if we can focus on a config node from the current state
    pub fn can_focus_config_node(
        current: &ViewportFocusState,
        _config_node: &ConfigNodeRef,
    ) -> TransitionResult<(CbuRef, InstrumentMatrixRef, InstrumentType)> {
        match current {
            ViewportFocusState::None => Err(TransitionError::NoCbuContext),
            ViewportFocusState::CbuContainer { .. } => Err(TransitionError::NoMatrixContext),
            ViewportFocusState::CbuEntity { .. } => Err(TransitionError::NoMatrixContext),
            ViewportFocusState::CbuProductService { .. } => Err(TransitionError::NoMatrixContext),
            ViewportFocusState::InstrumentMatrix { .. } => {
                Err(TransitionError::NoInstrumentTypeContext)
            }
            ViewportFocusState::InstrumentType {
                cbu,
                matrix,
                instrument_type,
                ..
            } => Ok((cbu.clone(), matrix.clone(), instrument_type.clone())),
            ViewportFocusState::ConfigNode {
                cbu,
                matrix,
                instrument_type,
                ..
            } => Ok((cbu.clone(), matrix.clone(), instrument_type.clone())),
            ViewportFocusState::BoardControl { .. } => Err(TransitionError::NoMatrixContext),
        }
    }

    /// Validate enhance level for current focus type
    pub fn validate_enhance_level(
        current: &ViewportFocusState,
        requested: u8,
    ) -> TransitionResult<()> {
        let max = max_enhance_for_state(current);
        if requested > max {
            Err(TransitionError::EnhanceLevelExceeded { requested, max })
        } else {
            Ok(())
        }
    }

    /// Check if ascend is valid
    pub fn can_ascend(stack_depth: usize) -> TransitionResult<()> {
        if stack_depth == 0 {
            Err(TransitionError::CannotAscend)
        } else {
            Ok(())
        }
    }

    /// Check if descend is valid from current state
    pub fn can_descend(current: &ViewportFocusState) -> TransitionResult<()> {
        match current {
            ViewportFocusState::None => Err(TransitionError::CannotDescendFromNone),
            _ => Ok(()),
        }
    }
}

/// Get the maximum enhance level for a focus state
pub fn max_enhance_for_state(state: &ViewportFocusState) -> u8 {
    match state {
        ViewportFocusState::None => 0,
        ViewportFocusState::CbuContainer { .. } => 2,
        ViewportFocusState::CbuEntity { .. } => 4,
        ViewportFocusState::CbuProductService { .. } => 3,
        ViewportFocusState::InstrumentMatrix { .. } => 2,
        ViewportFocusState::InstrumentType { .. } => 3,
        ViewportFocusState::ConfigNode { .. } => 2,
        ViewportFocusState::BoardControl { .. } => 2,
    }
}

/// Get the current enhance level from a focus state
pub fn current_enhance_level(state: &ViewportFocusState) -> u8 {
    match state {
        ViewportFocusState::None => 0,
        ViewportFocusState::CbuContainer { enhance_level, .. } => *enhance_level,
        ViewportFocusState::CbuEntity { entity_enhance, .. } => *entity_enhance,
        ViewportFocusState::CbuProductService { target_enhance, .. } => *target_enhance,
        ViewportFocusState::InstrumentMatrix { matrix_enhance, .. } => *matrix_enhance,
        ViewportFocusState::InstrumentType { type_enhance, .. } => *type_enhance,
        ViewportFocusState::ConfigNode { node_enhance, .. } => *node_enhance,
        ViewportFocusState::BoardControl { enhance_level, .. } => *enhance_level,
    }
}

/// Extract the CBU reference from any focus state
pub fn extract_cbu(state: &ViewportFocusState) -> Option<&CbuRef> {
    match state {
        ViewportFocusState::None => None,
        ViewportFocusState::CbuContainer { cbu, .. } => Some(cbu),
        ViewportFocusState::CbuEntity { cbu, .. } => Some(cbu),
        ViewportFocusState::CbuProductService { cbu, .. } => Some(cbu),
        ViewportFocusState::InstrumentMatrix { cbu, .. } => Some(cbu),
        ViewportFocusState::InstrumentType { cbu, .. } => Some(cbu),
        ViewportFocusState::ConfigNode { cbu, .. } => Some(cbu),
        ViewportFocusState::BoardControl { source_cbu, .. } => Some(source_cbu),
    }
}

/// Extract the matrix reference if in matrix context
pub fn extract_matrix(state: &ViewportFocusState) -> Option<&InstrumentMatrixRef> {
    match state {
        ViewportFocusState::InstrumentMatrix { matrix, .. } => Some(matrix),
        ViewportFocusState::InstrumentType { matrix, .. } => Some(matrix),
        ViewportFocusState::ConfigNode { matrix, .. } => Some(matrix),
        _ => None,
    }
}

/// Extract the instrument type if in instrument type context
pub fn extract_instrument_type(state: &ViewportFocusState) -> Option<&InstrumentType> {
    match state {
        ViewportFocusState::InstrumentType {
            instrument_type, ..
        } => Some(instrument_type),
        ViewportFocusState::ConfigNode {
            instrument_type, ..
        } => Some(instrument_type),
        _ => None,
    }
}

/// Describe the current focus state for display
/// Note: CbuRef is a newtype (Uuid), so we display the UUID
pub fn describe_focus(state: &ViewportFocusState) -> String {
    match state {
        ViewportFocusState::None => "No focus".to_string(),
        ViewportFocusState::CbuContainer { cbu, enhance_level } => {
            format!("CBU: {} (L{})", cbu.0, enhance_level)
        }
        ViewportFocusState::CbuEntity {
            cbu,
            entity,
            entity_enhance,
            ..
        } => {
            format!(
                "Entity: {:?} in CBU {} (L{})",
                entity.entity_type, cbu.0, entity_enhance
            )
        }
        ViewportFocusState::CbuProductService {
            cbu,
            target,
            target_enhance,
            ..
        } => {
            let target_desc = match target {
                ProductServiceRef::Product { id } => format!("Product {}", id),
                ProductServiceRef::Service { id } => format!("Service {}", id),
                ProductServiceRef::ServiceResource { id } => format!("Resource {}", id),
            };
            format!("{} in CBU {} (L{})", target_desc, cbu.0, target_enhance)
        }
        ViewportFocusState::InstrumentMatrix {
            cbu,
            matrix_enhance,
            ..
        } => {
            format!("Instrument Matrix for CBU {} (L{})", cbu.0, matrix_enhance)
        }
        ViewportFocusState::InstrumentType {
            cbu,
            instrument_type,
            type_enhance,
            ..
        } => {
            format!(
                "{:?} for CBU {} (L{})",
                instrument_type, cbu.0, type_enhance
            )
        }
        ViewportFocusState::ConfigNode {
            cbu,
            config_node,
            node_enhance,
            ..
        } => {
            let node_desc = match config_node {
                ConfigNodeRef::Mic { code } => format!("MIC {}", code),
                ConfigNodeRef::Bic { code } => format!("BIC {}", code),
                ConfigNodeRef::Pricing { id } => format!("Pricing {}", id),
                ConfigNodeRef::Restrictions { id } => format!("Restrictions {}", id),
            };
            format!("Config {} for CBU {} (L{})", node_desc, cbu.0, node_enhance)
        }
        ViewportFocusState::BoardControl {
            source_cbu,
            anchor_entity_name,
            enhance_level,
            ..
        } => {
            format!(
                "Board Control: {} from CBU {} (L{})",
                anchor_entity_name, source_cbu.0, enhance_level
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_cbu() -> CbuRef {
        CbuRef(Uuid::new_v4())
    }

    fn test_matrix() -> InstrumentMatrixRef {
        InstrumentMatrixRef(Uuid::new_v4())
    }

    #[test]
    fn test_can_focus_entity_requires_cbu() {
        let state = ViewportFocusState::None;
        let entity = ConcreteEntityRef {
            id: Uuid::new_v4(),
            entity_type: ob_poc_types::viewport::ConcreteEntityType::Company,
        };

        let result = TransitionValidator::can_focus_entity(&state, &entity);
        assert!(matches!(result, Err(TransitionError::NoCbuContext)));
    }

    #[test]
    fn test_can_focus_entity_from_cbu() {
        let state = ViewportFocusState::CbuContainer {
            cbu: test_cbu(),
            enhance_level: 1,
        };
        let entity = ConcreteEntityRef {
            id: Uuid::new_v4(),
            entity_type: ob_poc_types::viewport::ConcreteEntityType::Company,
        };

        let result = TransitionValidator::can_focus_entity(&state, &entity);
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_focus_instrument_type_requires_matrix() {
        let state = ViewportFocusState::CbuContainer {
            cbu: test_cbu(),
            enhance_level: 1,
        };

        let result =
            TransitionValidator::can_focus_instrument_type(&state, &InstrumentType::Equity);
        assert!(matches!(result, Err(TransitionError::NoMatrixContext)));
    }

    #[test]
    fn test_can_focus_instrument_type_from_matrix() {
        let cbu = test_cbu();
        let matrix = test_matrix();

        let state = ViewportFocusState::InstrumentMatrix {
            cbu: cbu.clone(),
            matrix: matrix.clone(),
            matrix_enhance: 1,
            container_enhance: 1,
        };

        let result =
            TransitionValidator::can_focus_instrument_type(&state, &InstrumentType::Equity);
        assert!(result.is_ok());
        let (result_cbu, result_matrix) = result.unwrap();
        assert_eq!(result_cbu.0, cbu.0);
        assert_eq!(result_matrix.0, matrix.0);
    }

    #[test]
    fn test_enhance_level_validation() {
        let state = ViewportFocusState::CbuContainer {
            cbu: test_cbu(),
            enhance_level: 1,
        };

        // Valid levels
        assert!(TransitionValidator::validate_enhance_level(&state, 0).is_ok());
        assert!(TransitionValidator::validate_enhance_level(&state, 1).is_ok());
        assert!(TransitionValidator::validate_enhance_level(&state, 2).is_ok());

        // Invalid level
        let result = TransitionValidator::validate_enhance_level(&state, 3);
        assert!(matches!(
            result,
            Err(TransitionError::EnhanceLevelExceeded {
                requested: 3,
                max: 2
            })
        ));
    }

    #[test]
    fn test_describe_focus() {
        let cbu = test_cbu();
        let state = ViewportFocusState::CbuContainer {
            cbu: cbu.clone(),
            enhance_level: 1,
        };

        let desc = describe_focus(&state);
        assert!(desc.contains(&cbu.0.to_string()));
        assert!(desc.contains("L1"));
    }
}
