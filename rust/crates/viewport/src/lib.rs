//! Viewport State Management Crate
//!
//! Provides viewport state management, focus transitions, and enhance level
//! implementations for CBU navigation. This crate extends the core types
//! defined in `ob-poc-types` with runtime behavior.
//!
//! ## Architecture
//!
//! - `ob-poc-types::viewport` - Core types (ViewportState, FocusManager, etc.)
//! - This crate - Runtime context, transitions, concrete Enhanceable impls
//!
//! ## Key Components
//!
//! - `ViewportContext` - Extended wrapper around ViewportState with runtime context
//! - `FocusTransition` - Enum representing all possible focus transitions
//! - `TransitionError` - Error types for invalid state transitions
//! - Enhanceable implementations for CBU, Entity, Matrix, etc.

pub mod enhance;
pub mod executor;
pub mod focus;
pub mod state;
pub mod transitions;

// Re-export core types from ob-poc-types for convenience
pub use ob_poc_types::viewport::{
    CameraState, CbuRef, CbuViewMemory, CbuViewType, ConcreteEntityRef, ConcreteEntityType,
    ConfidenceZone, ConfigNodeRef, EnhanceArg, EnhanceLevelInfo, EnhanceOp, Enhanceable,
    FocusManager, FocusMode, InstrumentMatrixRef, InstrumentType, ProductServiceRef,
    ViewportFilters, ViewportFocusState, ViewportState,
};

// Re-export crate-specific implementations
pub use enhance::{
    CbuEnhanceable, ConcreteEntityEnhanceable, ConfigNodeEnhanceable, InstrumentMatrixEnhanceable,
    InstrumentTypeEnhanceable,
};
pub use executor::{
    ExecutionOutcome, ExecutorError, ExecutorResult, ReferenceResolver, ResolvedSymbol,
    SimpleResolver, ViewportExecutor,
};
pub use focus::FocusTransition;
pub use state::ViewportContext;
pub use transitions::{TransitionError, TransitionResult, TransitionValidator};
