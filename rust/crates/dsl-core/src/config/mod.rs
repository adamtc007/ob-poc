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

pub mod escalation;
pub mod loader;
pub mod phrase_gen;
pub mod runbook_composition;
pub mod types;
pub mod validator;

pub use escalation::{
    compute_effective_tier, compute_effective_tier_with_trace, evaluate_predicate,
    EvaluationContext,
};
pub use runbook_composition::{
    component_a, component_b, component_c, compute_runbook_tier,
    compute_runbook_tier_with_trace, AggregationRule, CrossScopeRule, RunbookStep,
    RunbookTierTrace,
};
pub use validator::{
    validate_verb, validate_verbs_config, Location, PolicyWarning, StructuralError,
    ValidationContext, ValidationReport, WellFormednessError,
};
pub use loader::ConfigLoader;
pub use phrase_gen::generate_phrases;
pub use types::{
    ActionClass, AppliesTo, ArgConfig, ArgType, ArgValidation, ConfirmPolicyConfig, ConsequenceDeclaration,
    ConsequenceTier, ConstraintRule, CrudConfig, CrudOperation, CsgRulesConfig, DomainConfig,
    DurableConfig, DurableRuntime, EscalationPredicate, EscalationRule, ExternalEffect, FuzzyCheckConfig,
    GraphQueryConfig, GraphQueryOperation, HarmClass, JurisdictionCondition, JurisdictionRule,
    LookupConfig, ResolutionMode, ReturnTypeConfig, ReturnsConfig, RuleCondition, RuleRequirement,
    RuleSeverity, SearchKeyConfig, SlotType, SourceOfTruth, StateEffect, ThreeAxisDeclaration,
    TransitionEdge, VerbBehavior, VerbConfig, VerbConsumes, VerbLifecycle, VerbMetadata,
    VerbOutputConfig, VerbProduces, VerbScope, VerbSentences, VerbStatus, VerbTier, VerbTransitions,
    VerbsConfig, WarningRule,
};
