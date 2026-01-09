//! View management module for density-based mode switching and layout transitions
//!
//! This module provides:
//! - `DensityRule`: Rules for switching view modes based on visible entity counts
//! - `VisibleEntities`: Counting entities in viewport for density calculation
//! - `ViewModeController`: Debounced mode switching with smooth layout transitions
//! - Layout interpolation utilities for animating between graph layouts

pub mod density;
pub mod transition;

pub use density::{
    evaluate_density_rules, DensityRule, DensityThreshold, NodeRenderMode, ViewMode,
    VisibleEntities,
};
pub use transition::{
    ease_in_out_cubic, ease_out_cubic, ease_out_expo, ease_out_quad, interpolate_layouts, lerp_f32,
    lerp_pos2, lerp_vec2, linear, suggest_transition_params, EasingFn, EsperTransition,
    EsperTransitionState, InterpolatedNode, LayoutSnapshot, LayoutTransition, NodeSnapshot,
    SpringConfig, SpringF32, SpringVec2, TransitionParams,
};
