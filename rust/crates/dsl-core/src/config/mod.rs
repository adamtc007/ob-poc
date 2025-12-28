//! YAML-driven DSL configuration
//!
//! This module provides runtime configuration loading for DSL verbs,
//! allowing verb definitions to be modified without recompiling Rust code.
//!
//! # Architecture
//!
//! ```text
//! config/verbs.yaml → ConfigLoader → VerbsConfig → RuntimeVerbRegistry
//! config/csg_rules.yaml → ConfigLoader → CsgRulesConfig → CSG Linter
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use crate::dsl_v2::config::ConfigLoader;
//!
//! let loader = ConfigLoader::from_env();
//! let verbs = loader.load_verbs()?;
//! let csg_rules = loader.load_csg_rules()?;
//! ```

pub mod loader;
pub mod types;

pub use loader::ConfigLoader;
pub use types::*;
