//! Agent Control Plane — Phase 8 of the Semantic Registry.
//!
//! Provides types and database operations for agent plans, decisions,
//! escalation records, and disambiguation prompts. All records are
//! immutable (INSERT-only) with snapshot provenance chains.
//!
//! ## Module Structure
//!
//! - `plans` — AgentPlan + PlanStep types and DB operations
//! - `decisions` — DecisionRecord with snapshot_manifest provenance
//! - `escalation` — DisambiguationPrompt + EscalationRecord

pub mod decisions;
pub mod escalation;
pub mod mcp_tools;
pub mod plans;

// Re-export primary types
#[cfg(test)]
pub(crate) use decisions::{DecisionRecord, DecisionStore};
pub use mcp_tools::all_tool_specs;
#[cfg(test)]
pub(crate) use plans::{AgentPlan, AgentPlanStatus, PlanStep, PlanStepStatus, PlanStore};
