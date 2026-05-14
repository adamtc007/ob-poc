//! ob-poc-bods — BODS 0.4 / LEI spine reference data.
//!
//! ## Capability claim
//!
//! Owns the DTO surface for Beneficial Ownership Data Standard (BODS) 0.4
//! entities and the LEI (Legal Entity Identifier) spine: EntityIdentifier,
//! EntityWithLei, GleifHierarchyEntry, GleifHierarchyRelationship,
//! PersonPepStatus, UboInterest, plus the BodsInterestType / EntityType
//! enums. All shapes are `sqlx::FromRow` + serde, suitable as both wire
//! types and database projections.
//!
//! ## Anti-charter
//!
//! - NOT a GLEIF API client.
//! - NOT the ownership-graph pipeline.
//! - NOT governance — these are read shapes, not authoring.
//!
//! ## Dependency discipline
//!
//! Depends only on primitives. DB-coupled sqlx + rust_decimal gated
//! behind the `database` feature.
