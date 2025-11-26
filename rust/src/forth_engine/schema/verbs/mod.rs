//! Verb definitions organized by domain.

pub mod cbu;
pub mod entity;
pub mod document;
pub mod kyc;
pub mod screening;
pub mod decision;
pub mod monitoring;
pub mod attribute;

pub use cbu::*;
pub use entity::*;
pub use document::*;
pub use kyc::*;
pub use screening::*;
pub use decision::*;
pub use monitoring::*;
pub use attribute::*;
