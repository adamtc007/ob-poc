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
pub use decisions::{DecisionRecord, DecisionStore};
pub use escalation::{AgentDisambiguationPrompt, AgentEscalationRecord, EscalationStore};
pub use mcp_tools::{
    all_tool_specs, dispatch_tool, GroundingContext, SemRegToolContext, SemRegToolResult,
};
pub use plans::{AgentPlan, AgentPlanStatus, PlanStep, PlanStepStatus, PlanStore};
