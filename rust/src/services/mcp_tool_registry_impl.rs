//! ob-poc impl of [`dsl_runtime::service_traits::McpToolRegistry`].
//!
//! Bridges the trait to `crate::sem_reg::agent::mcp_tools::all_tool_specs()`.
//! Projects the internal `SemRegToolSpec` to the plane-crossing
//! `McpToolSpec` shape.

use async_trait::async_trait;

use dsl_runtime::service_traits::{McpToolRegistry, McpToolSpec};

use crate::sem_reg::agent::mcp_tools::all_tool_specs;

pub struct ObPocMcpToolRegistry;

impl ObPocMcpToolRegistry {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocMcpToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpToolRegistry for ObPocMcpToolRegistry {
    async fn list_specs(&self) -> Vec<McpToolSpec> {
        all_tool_specs()
            .into_iter()
            .map(|spec| McpToolSpec {
                name: spec.name,
                description: spec.description,
                category: spec.category,
            })
            .collect()
    }
}
