//! Graph Query Executor
//!
//! Executes graph.* DSL verbs using the GraphQueryEngine.
//! This module bridges the DSL execution layer with the graph query engine.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::config::types::GraphQueryOperation;
use super::runtime_registry::{RuntimeGraphQueryConfig, RuntimeVerb};
use crate::graph::view_model::{GraphFilter, GraphViewModel};
use crate::graph::{EdgeType, LayerType, NodeType};

#[cfg(feature = "database")]
use crate::graph::query_engine::GraphQueryEngine;

// =============================================================================
// GRAPH QUERY EXECUTOR
// =============================================================================

/// Executor for graph.* DSL verbs
#[cfg(feature = "database")]
pub struct GraphQueryExecutor {
    engine: GraphQueryEngine,
}

#[cfg(feature = "database")]
impl GraphQueryExecutor {
    /// Create a new GraphQueryExecutor
    pub fn new(pool: PgPool) -> Self {
        Self {
            engine: GraphQueryEngine::new(pool),
        }
    }

    /// Execute a graph query verb
    pub async fn execute(
        &self,
        verb: &RuntimeVerb,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        tracing::debug!(
            "GraphQueryExecutor::execute {}.{} operation={:?}",
            verb.domain,
            verb.verb,
            config.operation
        );

        match config.operation {
            GraphQueryOperation::View => self.execute_view(config, args).await,
            GraphQueryOperation::Focus => self.execute_focus(config, args).await,
            GraphQueryOperation::Filter => self.execute_filter(config, args).await,
            GraphQueryOperation::GroupBy => self.execute_group_by(config, args).await,
            GraphQueryOperation::Path => self.execute_path(config, args).await,
            GraphQueryOperation::FindConnected => self.execute_find_connected(config, args).await,
            GraphQueryOperation::Ancestors => self.execute_ancestors(config, args).await,
            GraphQueryOperation::Descendants => self.execute_descendants(config, args).await,
            GraphQueryOperation::Compare => self.execute_compare(config, args).await,
        }
    }

    // =========================================================================
    // OPERATION IMPLEMENTATIONS
    // =========================================================================

    async fn execute_view(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let cbu_id = self.get_uuid_arg(args, "cbu-id")?;
        let view_mode = self.get_string_arg(args, "view-mode").unwrap_or_else(|| {
            config
                .default_view_mode
                .clone()
                .unwrap_or_else(|| "KYC_UBO".to_string())
        });
        let max_depth = self
            .get_int_arg(args, "max-depth")
            .unwrap_or(config.max_depth);

        let model = self
            .engine
            .execute_view(cbu_id, &view_mode, max_depth)
            .await?;

        Ok(GraphQueryResult::Graph(Box::new(model)))
    }

    async fn execute_focus(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let cbu_id = self.get_uuid_arg(args, "cbu-id")?;
        let entity_id = self.get_uuid_arg(args, "entity-id")?;
        let depth = self.get_int_arg(args, "depth").unwrap_or(config.max_depth);

        let model = self.engine.execute_focus(cbu_id, entity_id, depth).await?;

        Ok(GraphQueryResult::Graph(Box::new(model)))
    }

    async fn execute_filter(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let cbu_id = self.get_uuid_arg(args, "cbu-id")?;

        // First get the full view
        let view_mode = config
            .default_view_mode
            .clone()
            .unwrap_or_else(|| "KYC_UBO".to_string());
        let model = self
            .engine
            .execute_view(cbu_id, &view_mode, config.max_depth)
            .await?;

        // Build filter from args
        let filter = self.build_filter_from_args(args)?;

        // Apply filter
        let filtered = self.engine.execute_filter(&model, &filter)?;

        Ok(GraphQueryResult::Graph(Box::new(filtered)))
    }

    async fn execute_group_by(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let cbu_id = self.get_uuid_arg(args, "cbu-id")?;
        let group_by = self
            .get_string_arg(args, "group-by")
            .ok_or_else(|| anyhow!("Missing required argument: group-by"))?;

        // First get the full view
        let view_mode = config
            .default_view_mode
            .clone()
            .unwrap_or_else(|| "KYC_UBO".to_string());
        let model = self
            .engine
            .execute_view(cbu_id, &view_mode, config.max_depth)
            .await?;

        // Apply grouping
        let grouped = self.engine.execute_group_by(&model, &group_by)?;

        Ok(GraphQueryResult::Graph(Box::new(grouped)))
    }

    async fn execute_path(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let cbu_id = self.get_uuid_arg(args, "cbu-id")?;
        let from_id = self.get_uuid_arg(args, "from-entity-id")?;
        let to_id = self.get_uuid_arg(args, "to-entity-id")?;

        // Get full view first
        let view_mode = config
            .default_view_mode
            .clone()
            .unwrap_or_else(|| "KYC_UBO".to_string());
        let model = self
            .engine
            .execute_view(cbu_id, &view_mode, config.max_depth)
            .await?;

        // Find path
        let paths = self.engine.execute_path(
            &model,
            &from_id.to_string(),
            &to_id.to_string(),
            None, // TODO: parse edge-types from args
        )?;

        // Return model with paths
        let mut result = model;
        result.paths = paths;
        result.compute_stats();
        result.stats.path_count = result.paths.len();

        Ok(GraphQueryResult::Graph(Box::new(result)))
    }

    async fn execute_find_connected(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let cbu_id = self.get_uuid_arg(args, "cbu-id")?;
        let entity_id = self.get_uuid_arg(args, "entity-id")?;
        let max_depth = self
            .get_int_arg(args, "max-depth")
            .unwrap_or(config.max_depth);

        // Get full view first
        let view_mode = config
            .default_view_mode
            .clone()
            .unwrap_or_else(|| "KYC_UBO".to_string());
        let model = self
            .engine
            .execute_view(cbu_id, &view_mode, config.max_depth)
            .await?;

        // Find connected
        let connected = self.engine.execute_find_connected(
            &model,
            &entity_id.to_string(),
            max_depth,
            None, // TODO: parse edge-types from args
        )?;

        Ok(GraphQueryResult::Graph(Box::new(connected)))
    }

    async fn execute_ancestors(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let entity_id = self.get_uuid_arg(args, "entity-id")?;
        let max_depth = self
            .get_int_arg(args, "max-depth")
            .unwrap_or(config.max_depth);

        let model = self.engine.execute_ancestors(entity_id, max_depth).await?;

        Ok(GraphQueryResult::Graph(Box::new(model)))
    }

    async fn execute_descendants(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let entity_id = self.get_uuid_arg(args, "entity-id")?;
        let max_depth = self
            .get_int_arg(args, "max-depth")
            .unwrap_or(config.max_depth);

        let model = self
            .engine
            .execute_descendants(entity_id, max_depth)
            .await?;

        Ok(GraphQueryResult::Graph(Box::new(model)))
    }

    async fn execute_compare(
        &self,
        config: &RuntimeGraphQueryConfig,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphQueryResult> {
        let cbu_id = self.get_uuid_arg(args, "cbu-id")?;
        let _left_snapshot_id = self.get_uuid_arg(args, "left-snapshot-id")?;
        let _right_snapshot_id = self.get_uuid_arg(args, "right-snapshot-id")?;

        // TODO: Implement snapshot loading from database
        // For now, return a placeholder showing the feature exists
        let view_mode = config
            .default_view_mode
            .clone()
            .unwrap_or_else(|| "KYC_UBO".to_string());
        let model = self
            .engine
            .execute_view(cbu_id, &view_mode, config.max_depth)
            .await?;

        // Compare with self (no changes) for now
        let comparison = self.engine.execute_compare(&model, &model)?;

        Ok(GraphQueryResult::Comparison(Box::new(comparison)))
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    fn get_uuid_arg(&self, args: &HashMap<String, serde_json::Value>, name: &str) -> Result<Uuid> {
        args.get(name)
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required argument: {}", name))
            .and_then(|s| {
                Uuid::parse_str(s).map_err(|e| anyhow!("Invalid UUID for {}: {}", name, e))
            })
    }

    fn get_string_arg(
        &self,
        args: &HashMap<String, serde_json::Value>,
        name: &str,
    ) -> Option<String> {
        args.get(name)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn get_int_arg(&self, args: &HashMap<String, serde_json::Value>, name: &str) -> Option<u32> {
        args.get(name).and_then(|v| v.as_u64()).map(|n| n as u32)
    }

    fn get_string_list_arg(
        &self,
        args: &HashMap<String, serde_json::Value>,
        name: &str,
    ) -> Vec<String> {
        args.get(name)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn build_filter_from_args(
        &self,
        args: &HashMap<String, serde_json::Value>,
    ) -> Result<GraphFilter> {
        let mut filter = GraphFilter::default();

        // Parse node-types
        let node_type_strs = self.get_string_list_arg(args, "node-types");
        for s in node_type_strs {
            if let Some(nt) = Self::parse_node_type(&s) {
                filter.node_types.push(nt);
            }
        }

        // Parse layers
        let layer_strs = self.get_string_list_arg(args, "layers");
        for s in layer_strs {
            if let Some(lt) = Self::parse_layer_type(&s) {
                filter.layers.push(lt);
            }
        }

        // Parse edge-types
        let edge_type_strs = self.get_string_list_arg(args, "edge-types");
        for s in edge_type_strs {
            if let Some(et) = Self::parse_edge_type(&s) {
                filter.edge_types.push(et);
            }
        }

        // Simple string filters
        filter.role = self.get_string_arg(args, "role");
        filter.jurisdiction = self.get_string_arg(args, "jurisdiction");
        filter.search = self.get_string_arg(args, "search");

        Ok(filter)
    }

    fn parse_node_type(s: &str) -> Option<NodeType> {
        match s.to_lowercase().as_str() {
            "cbu" => Some(NodeType::Cbu),
            "entity" => Some(NodeType::Entity),
            "product" => Some(NodeType::Product),
            "service" => Some(NodeType::Service),
            "resource" => Some(NodeType::Resource),
            "document" => Some(NodeType::Document),
            "ssi" => Some(NodeType::Ssi),
            "market" => Some(NodeType::Market),
            "isda" => Some(NodeType::Isda),
            _ => None,
        }
    }

    fn parse_layer_type(s: &str) -> Option<LayerType> {
        match s.to_lowercase().as_str() {
            "core" => Some(LayerType::Core),
            "ubo" => Some(LayerType::Ubo),
            "kyc" => Some(LayerType::Kyc),
            "services" => Some(LayerType::Services),
            "custody" => Some(LayerType::Custody),
            _ => None,
        }
    }

    fn parse_edge_type(s: &str) -> Option<EdgeType> {
        match s.to_lowercase().as_str() {
            "owns" => Some(EdgeType::Owns),
            "controls" => Some(EdgeType::Controls),
            "has_role" | "hasrole" => Some(EdgeType::HasRole),
            "delivers" => Some(EdgeType::Delivers),
            "routes_to" | "routesto" => Some(EdgeType::RoutesTo),
            "uses_product" | "usesproduct" => Some(EdgeType::UsesProduct),
            _ => None,
        }
    }
}

// =============================================================================
// GRAPH QUERY RESULT
// =============================================================================

/// Result of a graph query execution
#[derive(Debug, Clone)]
pub enum GraphQueryResult {
    /// Standard graph result
    Graph(Box<GraphViewModel>),
    /// Comparison result
    Comparison(Box<crate::graph::view_model::GraphComparison>),
}

impl GraphQueryResult {
    /// Convert to JSON value for DSL result
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            GraphQueryResult::Graph(model) => {
                serde_json::to_value(model).unwrap_or(serde_json::Value::Null)
            }
            GraphQueryResult::Comparison(comp) => {
                serde_json::to_value(comp).unwrap_or(serde_json::Value::Null)
            }
        }
    }

    /// Get as GraphViewModel if this is a Graph result
    pub fn as_graph(&self) -> Option<&GraphViewModel> {
        match self {
            GraphQueryResult::Graph(model) => Some(model),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_node_type() {
        assert_eq!(
            GraphQueryExecutor::parse_node_type("entity"),
            Some(NodeType::Entity)
        );
        assert_eq!(
            GraphQueryExecutor::parse_node_type("cbu"),
            Some(NodeType::Cbu)
        );
        assert_eq!(GraphQueryExecutor::parse_node_type("unknown"), None);
    }

    #[test]
    fn test_parse_layer_type() {
        assert_eq!(
            GraphQueryExecutor::parse_layer_type("core"),
            Some(LayerType::Core)
        );
        assert_eq!(
            GraphQueryExecutor::parse_layer_type("UBO"),
            Some(LayerType::Ubo)
        );
    }

    #[test]
    fn test_parse_edge_type() {
        assert_eq!(
            GraphQueryExecutor::parse_edge_type("owns"),
            Some(EdgeType::Owns)
        );
        assert_eq!(
            GraphQueryExecutor::parse_edge_type("has_role"),
            Some(EdgeType::HasRole)
        );
    }
}
