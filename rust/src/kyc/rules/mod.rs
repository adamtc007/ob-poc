//! DSL-driven KYC rules engine
//!
//! Rules are defined in YAML and evaluated automatically when events occur.
//! The engine supports:
//! - Complex conditions (AND/OR/NOT)
//! - Multiple operators (equals, in, contains, gte, etc.)
//! - Variable interpolation
//! - Multiple actions per rule
//! - Scheduled (temporal) rules

mod context;
mod evaluator;
mod event_bus;
mod parser;
mod scheduler;

pub use context::RuleContext;
pub use evaluator::RuleEvaluator;
pub use event_bus::{KycEvent, KycEventBus};
pub use parser::{
    load_rules, Action, Condition, LeafCondition, Operator, Rule, RulesConfig, Trigger,
};
pub use scheduler::RuleScheduler;
