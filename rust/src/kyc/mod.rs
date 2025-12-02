//! KYC case management module
//!
//! This module provides:
//! - Case lifecycle management
//! - Entity workstream tracking
//! - Red flag detection and escalation
//! - DSL-driven rules engine

pub mod rules;

pub use rules::{
    load_rules, Action, Condition, KycEvent, KycEventBus, LeafCondition, Operator, Rule,
    RuleContext, RuleEvaluator, RuleScheduler, RulesConfig,
};
