//! Modal Dialogs Module
//!
//! Contains modal dialog implementations:
//! - Entity Finder: Search and resolve EntityRefs
//! - CBU Picker: Search and select CBU to work with

pub mod cbu_picker;
pub mod entity_finder;

pub use cbu_picker::{CbuPickerModal, CbuPickerResult};
pub use entity_finder::{EntityFinderModal, EntityFinderResult, ResolveContext};
