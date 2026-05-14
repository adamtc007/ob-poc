//! ob-poc-booking-principal — Booking principal taxonomy reference data.
//!
//! ## Capability claim
//!
//! Owns the DTO surface for the booking-principal capability: legal
//! entity, booking location, booking principal, ruleset, rule, service
//! availability, gap report, delivery plan, eligibility evaluation
//! result, contract pack. Pure data shapes consumed by both API handlers
//! and the rule evaluator.
//!
//! ## Anti-charter
//!
//! - NOT the rule evaluator engine.
//! - NOT the booking-principal repository.
//! - NOT governance or materialization.
