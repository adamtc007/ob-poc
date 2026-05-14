//! ob-poc-taxonomy — Generic taxonomy combinators + view-config loader.
//!
//! ## Capability claim
//!
//! Owns the generic taxonomy combinators (Product / Instrument), the
//! taxonomy stack/builder/rules engine, and the view-config-service
//! loader. Paired because `taxonomy::rules` consumes `view_config_service`
//! directly.
//!
//! ## Anti-charter
//!
//! - NOT domain-specific taxonomy SERVICES (those live in ob-poc).
//! - NOT ontology lifecycle (that's `ob-poc-ontology`).
