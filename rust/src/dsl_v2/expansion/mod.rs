//! Template Expansion Module
//!
//! Provides deterministic template expansion with audit trails for DSL execution.
//!
//! ## Overview
//!
//! Templates are macros that expand to multiple DSL statements. This module:
//! - Expands templates deterministically (same input → identical output)
//! - Produces `ExpansionReport` for audit/replay
//! - Derives lock keys from policy + runtime args
//! - Determines batch policy (atomic vs best_effort)
//!
//! ## Pipeline Position
//!
//! ```text
//! Raw DSL → [EXPANSION] → Expanded DSL → Parse → Compile → Execute
//!              │
//!              └── ExpansionReport (audit trail)
//! ```
//!
//! ## Key Concepts
//!
//! - **Expansion is pure**: No database calls, fully deterministic
//! - **Lock derivation**: Extracts entity IDs from args, builds sorted lock set
//! - **Batch policy**: Atomic (all-or-nothing) vs BestEffort (partial success allowed)

mod engine;
mod types;

pub use engine::{expand_templates, expand_templates_simple, ExpansionError, ExpansionOutput};
pub use types::{
    BatchPolicy, ExpansionDiagnostic, ExpansionReport, LockAccess, LockKey, LockMode, LockTarget,
    LockingPolicy, PerItemOrigin, RuntimePolicy, TemplateDigest, TemplateInvocationReport,
    TemplatePolicy,
};
