//! Token System
//!
//! YAML-driven visual configuration for entity types.
//! This is the 2D/egui version - no 3D, no GPU instancing.
//!
//! ## Overview
//!
//! Tokens bridge data entities to visual representation:
//! - Colors, icons, sizes
//! - Click/hover behaviors
//! - Detail view templates
//! - Context menu actions
//!
//! ## Usage
//!
//! ```rust,ignore
//! let registry = TokenRegistry::load_defaults()?;
//! let token = registry.get("share_class");
//! let color = token.visual.status_color32(Some("ACTIVE"));
//! ```

mod registry;
mod types;

pub use registry::{TokenConfig, TokenRegistry, DEFAULT_TOKEN_CONFIG};
pub use types::{
    ActionStyle, ClickBehavior, ContextAction, DetailAction, DetailField, DetailSection,
    DetailTemplate, FieldFormat, HoverBehavior, IconDef, InteractionRules, TokenCategory,
    TokenDefinition, TokenTypeId, TokenVisual,
};
