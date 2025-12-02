//! CBU Visualization module
//!
//! Provides hierarchical tree visualization with multiple views:
//! - KYC/UBO: Who is this client? Who owns/controls it?
//! - Service Delivery: What does BNY provide to this client?
//! - Case: KYC case workstream tree with red flags and stats

pub mod case_builder;
pub mod kyc_builder;
pub mod service_builder;
pub mod types;

pub use case_builder::CaseTreeBuilder;
pub use kyc_builder::KycTreeBuilder;
pub use service_builder::ServiceTreeBuilder;
pub use types::*;
