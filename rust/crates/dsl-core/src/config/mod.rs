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
pub mod phrase_gen;
pub mod types;

pub use loader::ConfigLoader;
pub use phrase_gen::generate_phrases;
pub use types::{
    ActionClass, AppliesTo, ArgConfig, ArgType, ArgValidation, ConfirmPolicyConfig, ConstraintRule,
    CrudConfig, CrudOperation, CsgRulesConfig, DomainConfig, DurableConfig, DurableRuntime,
    FuzzyCheckConfig, GraphQueryConfig, GraphQueryOperation, HarmClass, JurisdictionCondition,
    JurisdictionRule, LookupConfig, ResolutionMode, ReturnTypeConfig, ReturnsConfig, RuleCondition,
    RuleRequirement, RuleSeverity, SearchKeyConfig, SlotType, SourceOfTruth, VerbBehavior,
    VerbConfig, VerbConsumes, VerbLifecycle, VerbMetadata, VerbOutputConfig, VerbProduces,
    VerbScope, VerbSentences, VerbStatus, VerbTier, VerbsConfig, WarningRule,
};
