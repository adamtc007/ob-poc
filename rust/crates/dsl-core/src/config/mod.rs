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

pub mod dag;
pub mod dag_registry;
pub mod dag_validator;
pub mod escalation;
pub mod green_when_coverage;
pub mod loader;
pub mod pack_loader;
pub mod phrase_gen;
pub mod predicate;
pub mod runbook_composition;
pub mod tier_gate;
pub mod types;
pub mod validator;

pub use dag::{load_dags_from_dir, Dag, LoadedDag};
pub use dag_registry::{DagRegistry, SlotKey, TransitionKey};
pub use dag_validator::{
    entity_kinds_from_taxonomy_yaml, validate_constellation_map_dir_schema_coordination,
    validate_constellation_map_dir_schema_coordination_strict,
    validate_constellation_map_schema_coordination, validate_dags, validate_dags_with_context,
    validate_resolved_template_gate_metadata, DagError, DagValidationContext, DagValidationReport,
    DagWarning, SchemaCoordinationKnownDeferred,
};
pub use green_when_coverage::{
    green_when_coverage_for_dag, green_when_coverage_for_dags, green_when_coverage_summary,
    GreenWhenCoverageRow, GreenWhenCoverageSummary, GreenWhenExclusionReason,
};
pub use pack_loader::{flatten_pack_entries, load_packs_from_dir, LoadedPack};

pub use escalation::{
    compute_effective_tier, compute_effective_tier_with_trace, evaluate_predicate,
    EvaluationContext,
};
pub use loader::ConfigLoader;
pub use phrase_gen::generate_phrases;
pub use runbook_composition::{
    component_a, component_b, component_c, compute_runbook_tier, compute_runbook_tier_with_trace,
    AggregationRule, CrossScopeRule, RunbookStep, RunbookTierTrace,
};
pub use tier_gate::{TierGateAction, TierGateDecision};
pub use types::{
    ActionClass, AppliesTo, ArgConfig, ArgType, ArgValidation, ConfirmPolicyConfig,
    ConsequenceDeclaration, ConsequenceTier, ConstraintRule, CrudConfig, CrudOperation,
    CsgRulesConfig, DomainConfig, DurableConfig, DurableRuntime, EscalationPredicate,
    EscalationRule, ExternalEffect, FuzzyCheckConfig, GraphQueryConfig, GraphQueryOperation,
    HarmClass, JurisdictionCondition, JurisdictionRule, LookupConfig, ResolutionMode,
    ReturnTypeConfig, ReturnsConfig, RuleCondition, RuleRequirement, RuleSeverity, SearchKeyConfig,
    SlotType, SourceOfTruth, StateEffect, ThreeAxisDeclaration, TransitionEdge, VerbBehavior,
    VerbConfig, VerbConsumes, VerbFlavour, VerbLifecycle, VerbMetadata, VerbOutputConfig,
    VerbProduces, VerbRoleGuard, VerbScope, VerbSentences, VerbStatus, VerbTier, VerbTransitions,
    VerbsConfig, WarningRule,
};
pub use validator::{
    collect_declared_fqns, validate_pack_fqns, validate_verb, validate_verbs_config, Location,
    PolicyWarning, StructuralError, ValidationContext, ValidationReport, WellFormednessError,
};
