//! ob-poc-deal — Deal taxonomy reference data.
//!
//! ## Capability claim
//!
//! Owns the DTO surface for the Deal / Product / RateCard / Participant /
//! Contract / OnboardingRequest / FeeMatrixLine taxonomy. Pure data:
//! `chrono`, `rust_decimal`, `serde`, `uuid`. No DB feature needed — these
//! are wire types; database projections live alongside the repository
//! that produces them, in `ob-poc::database::deal_repository`.
//!
//! ## Anti-charter
//!
//! - NOT the deal repository.
//! - NOT the negotiation workflow.
//! - NOT the billing run logic.
