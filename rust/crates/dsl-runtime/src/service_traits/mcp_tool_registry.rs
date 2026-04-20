//! MCP tool registry access for relocated agent ops.
//!
//! `agent.read-mcp-tools` enumerates the SemReg MCP tool surface so
//! operators can discover what tools the agent can invoke. The concrete
//! registry lives in `ob_poc::sem_reg::agent::mcp_tools`; the trait
//! here lets the relocated `agent_ops` ask for a plane-crossable
//! projection of each spec without dragging the full SemReg surface
//! into dsl-runtime.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Projection of one MCP tool spec to a plane-crossing data shape.
/// Subset of `ob_poc::sem_reg::agent::mcp_tools::SemRegToolSpec` — the
/// consumer op only reads `name` + `description`; `category` is
/// preserved for future filter variants without a trait change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSpec {
    pub name: String,
    pub description: String,
    pub category: String,
}

/// Registry that enumerates MCP tool specs for agent-facing discovery ops.
#[async_trait]
pub trait McpToolRegistry: Send + Sync {
    /// Return all registered tool specs. Production impl delegates to
    /// `ob_poc::sem_reg::agent::mcp_tools::all_tool_specs()` and
    /// projects each entry to [`McpToolSpec`].
    async fn list_specs(&self) -> Vec<McpToolSpec>;
}
