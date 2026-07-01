//! ob-poc compiler — stub (Phase 3 CR A4)
//!
//! The `VerbHandler` and Op-based `compile_to_ops` path has been removed.
//! All compilation now goes through `dsl_core::compiler::compile_to_steps`.
//! This crate is retained as a workspace member to avoid breaking Cargo.toml
//! dependency declarations; it re-exports nothing and will be removed in a
//! follow-on cleanup CR when dependent crates' Cargo.toml files are updated.
#![deny(unreachable_pub)]
