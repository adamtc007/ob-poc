//! View Operations
//!
//! These operations manage session view state - the unified "it" that:
//! - IS what the user sees
//! - IS what operations target
//! - IS what agent knows about
//!
//! Session = Intent Scope = Visual State = Operation Target
//!
//! # Rationale for Custom Ops
//!
//! View operations require:
//! - Access to UnifiedSessionContext (not just ExecutionContext)
//! - Taxonomy building from database
//! - Layout computation
//! - Selection state management

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::custom_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::session::{
    ContextMode, DetailLevel, DrillDirection, GapType, IlluminateAspect, RedFlagCategory,
    Refinement, RiskThreshold, TraceMode, ViewState,
};
use crate::taxonomy::{Filter, Metaphor, Status, TaxonomyBuilder, TaxonomyContext};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Extract UUID argument from verb call
fn get_uuid_arg(verb_call: &VerbCall, name: &str, ctx: &ExecutionContext) -> Option<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| {
            if let Some(symbol) = a.value.as_symbol() {
                ctx.resolve(symbol)
            } else {
                a.value.as_uuid()
            }
        })
}

/// Extract string argument from verb call
fn get_string_arg(verb_call: &VerbCall, name: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Extract string list argument from verb call
fn get_string_list_arg(verb_call: &VerbCall, name: &str) -> Option<Vec<String>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| {
            if let Some(list) = a.value.as_list() {
                let strings: Vec<String> = list
                    .iter()
                    .filter_map(|v| v.as_string().map(|s| s.to_string()))
                    .collect();
                if strings.is_empty() {
                    None
                } else {
                    Some(strings)
                }
            } else if let Some(s) = a.value.as_string() {
                Some(vec![s.to_string()])
            } else {
                None
            }
        })
}

/// Extract boolean argument from verb call
fn get_bool_arg(verb_call: &VerbCall, name: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_boolean())
}

/// Extract UUID list argument from verb call
fn get_uuid_list_arg(
    verb_call: &VerbCall,
    name: &str,
    ctx: &ExecutionContext,
) -> Option<Vec<Uuid>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| {
            if let Some(list) = a.value.as_list() {
                let uuids: Vec<Uuid> = list
                    .iter()
                    .filter_map(|v| {
                        if let Some(symbol) = v.as_symbol() {
                            ctx.resolve(symbol)
                        } else {
                            v.as_uuid()
                        }
                    })
                    .collect();
                if uuids.is_empty() {
                    None
                } else {
                    Some(uuids)
                }
            } else {
                None
            }
        })
}

/// Helper to find entry in map by key
fn find_map_entry<'a>(
    entries: &'a [(String, dsl_core::AstNode)],
    key: &str,
) -> Option<&'a dsl_core::AstNode> {
    entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Parse filter from JSON object
fn parse_filter_from_args(verb_call: &VerbCall, arg_name: &str) -> Option<Filter> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            // Try to parse as a filter from the value
            if let Some(obj) = a.value.as_map() {
                // Check for jurisdiction filter
                if let Some(jurisdictions) = find_map_entry(obj, "jurisdiction") {
                    if let Some(list) = jurisdictions.as_list() {
                        let juris: Vec<String> = list
                            .iter()
                            .filter_map(|v| v.as_string().map(|s| s.to_string()))
                            .collect();
                        return Some(Filter::Jurisdiction(juris));
                    }
                }
                // Check for status filter
                if let Some(statuses) = find_map_entry(obj, "status") {
                    if let Some(list) = statuses.as_list() {
                        let stats: Vec<Status> = list
                            .iter()
                            .filter_map(|v| {
                                v.as_string().and_then(|s| match s.to_uppercase().as_str() {
                                    "RED" => Some(Status::Red),
                                    "AMBER" => Some(Status::Amber),
                                    "GREEN" => Some(Status::Green),
                                    _ => None,
                                })
                            })
                            .collect();
                        return Some(Filter::Status(stats));
                    }
                }
                // Check for fund_type filter
                if let Some(types) = find_map_entry(obj, "fund_type") {
                    if let Some(list) = types.as_list() {
                        let fund_types: Vec<String> = list
                            .iter()
                            .filter_map(|v| v.as_string().map(|s| s.to_string()))
                            .collect();
                        return Some(Filter::FundType(fund_types));
                    }
                }
            }
            None
        })
}

// =============================================================================
// VIEW OP RESULT
// =============================================================================

/// Result type for view operations
#[derive(Debug, Clone, Serialize)]
pub struct ViewOpResult {
    /// Context description
    pub context: String,
    /// Total nodes in taxonomy
    pub total_count: usize,
    /// Current selection count
    pub selection_count: usize,
    /// Number of active refinements
    pub refinement_count: usize,
    /// Whether there's a pending operation
    pub has_pending: bool,
    /// Visual metaphor derived from shape
    pub metaphor: String,
    /// Selection IDs for DSL binding
    pub selection_ids: Vec<Uuid>,
}

impl ViewOpResult {
    pub fn from_view_state(view: &ViewState) -> Self {
        Self {
            context: format!("{:?}", view.context),
            total_count: view.taxonomy.descendant_count,
            selection_count: view.selection.len(),
            refinement_count: view.refinements.len(),
            has_pending: view.pending.is_some(),
            metaphor: format!("{:?}", view.metaphor()),
            selection_ids: view.selection.clone(),
        }
    }
}

// =============================================================================
// VIEW.UNIVERSE - View all CBUs with optional filters
// =============================================================================

/// view.universe handler - View all CBUs with optional filters
pub struct ViewUniverseOp;

#[async_trait]
impl CustomOperation for ViewUniverseOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "universe"
    }

    fn rationale(&self) -> &'static str {
        "Requires taxonomy building from database and session state management"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Build context from args
        let taxonomy_ctx = if let Some(client_id) = get_uuid_arg(verb_call, "client", ctx) {
            TaxonomyContext::Book { client_id }
        } else {
            TaxonomyContext::Universe
        };

        // Build taxonomy from database using config-driven rules
        let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
        let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;

        // Create view state
        let mut view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

        // Apply any filters as refinements
        if let Some(jurisdictions) = get_string_list_arg(verb_call, "jurisdiction") {
            view.refine(Refinement::Include {
                filter: Filter::Jurisdiction(jurisdictions),
            });
        }

        if let Some(statuses) = get_string_list_arg(verb_call, "status") {
            let status_enums: Vec<Status> = statuses
                .iter()
                .filter_map(|s| match s.to_uppercase().as_str() {
                    "RED" => Some(Status::Red),
                    "AMBER" => Some(Status::Amber),
                    "GREEN" => Some(Status::Green),
                    _ => None,
                })
                .collect();
            if !status_enums.is_empty() {
                view.refine(Refinement::Include {
                    filter: Filter::Status(status_enums),
                });
            }
        }

        if let Some(fund_types) = get_string_list_arg(verb_call, "fund-type") {
            view.refine(Refinement::Include {
                filter: Filter::FundType(fund_types),
            });
        }

        // Bind selection to execution context for DSL access
        ctx.set_selection(view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        // This fixes the "session state side door" - ViewState was previously discarded
        ctx.set_pending_view_state(view);

        // Return as JSON
        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.universe requires database feature"))
    }
}

// =============================================================================
// VIEW.BOOK - View all CBUs for a commercial client
// =============================================================================

/// view.book handler - View all CBUs for a commercial client
pub struct ViewBookOp;

#[async_trait]
impl CustomOperation for ViewBookOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "book"
    }

    fn rationale(&self) -> &'static str {
        "Requires taxonomy building scoped to a client entity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client_id = get_uuid_arg(verb_call, "client", ctx)
            .ok_or_else(|| anyhow::anyhow!("client argument is required"))?;

        let taxonomy_ctx = TaxonomyContext::Book { client_id };
        let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
        let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;

        let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

        // Bind selection to execution context
        ctx.set_selection(view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        ctx.set_pending_view_state(view);

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.book requires database feature"))
    }
}

// =============================================================================
// VIEW.CBU - Focus on a single CBU
// =============================================================================

/// view.cbu handler - Focus on a single CBU with specified view mode
pub struct ViewCbuOp;

#[async_trait]
impl CustomOperation for ViewCbuOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "cbu"
    }

    fn rationale(&self) -> &'static str {
        "Requires CBU-specific taxonomy building with trading or UBO view modes"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_uuid_arg(verb_call, "cbu-id", ctx)
            .ok_or_else(|| anyhow::anyhow!("cbu-id argument is required"))?;

        let mode = get_string_arg(verb_call, "mode").unwrap_or_else(|| "trading".to_string());

        let taxonomy_ctx = match mode.as_str() {
            "ubo" => TaxonomyContext::CbuUbo { cbu_id },
            _ => TaxonomyContext::CbuTrading { cbu_id },
        };

        let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
        let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;

        let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

        // Bind selection to execution context
        ctx.set_selection(view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        ctx.set_pending_view_state(view);

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.cbu requires database feature"))
    }
}

// =============================================================================
// VIEW.ENTITY-FOREST - View entities by type/ownership filters
// =============================================================================

/// view.entity-forest handler - View entities filtered by type, jurisdiction, role
pub struct ViewEntityForestOp;

#[async_trait]
impl CustomOperation for ViewEntityForestOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "entity-forest"
    }

    fn rationale(&self) -> &'static str {
        "Requires entity forest taxonomy building with multiple filter types"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mut filters = Vec::new();

        if let Some(jurisdictions) = get_string_list_arg(verb_call, "jurisdiction") {
            filters.push(Filter::Jurisdiction(jurisdictions));
        }

        // entity-type and role filters would need corresponding Filter variants
        // For now, we'll use the existing filters

        let taxonomy_ctx = TaxonomyContext::EntityForest {
            filters: filters.clone(),
        };
        let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
        let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;

        let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

        // Bind selection to execution context
        ctx.set_selection(view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        ctx.set_pending_view_state(view);

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "view.entity-forest requires database feature"
        ))
    }
}

// =============================================================================
// VIEW.REFINE - Refine current view with additional filter
// =============================================================================

/// view.refine handler - Apply refinement to current view
pub struct ViewRefineOp;

#[async_trait]
impl CustomOperation for ViewRefineOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "refine"
    }

    fn rationale(&self) -> &'static str {
        "Modifies session view state with refinements"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Get current selection from context
        let current_selection = ctx.get_selection().cloned().unwrap_or_default();

        if current_selection.is_empty() {
            return Err(anyhow::anyhow!(
                "No active view. Use view.universe, view.book, or view.cbu first."
            ));
        }

        let mut new_selection = current_selection.clone();

        // Apply include filter
        if let Some(filter) = parse_filter_from_args(verb_call, "include") {
            // This would normally filter against taxonomy nodes
            // For now, we just note that refinement was requested
            tracing::debug!(?filter, "Applying include filter");
        }

        // Apply exclude filter
        if let Some(filter) = parse_filter_from_args(verb_call, "exclude") {
            tracing::debug!(?filter, "Applying exclude filter");
        }

        // Add specific IDs
        if let Some(add_ids) = get_uuid_list_arg(verb_call, "add", ctx) {
            for id in add_ids {
                if !new_selection.contains(&id) {
                    new_selection.push(id);
                }
            }
        }

        // Remove specific IDs
        if let Some(remove_ids) = get_uuid_list_arg(verb_call, "remove", ctx) {
            new_selection.retain(|id| !remove_ids.contains(id));
        }

        // Update selection in context
        ctx.set_selection(new_selection.clone());

        Ok(ExecutionResult::Record(json!({
            "selection_count": new_selection.len(),
            "selection_ids": new_selection,
            "message": "View refined successfully"
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.refine requires database feature"))
    }
}

// =============================================================================
// VIEW.CLEAR - Clear refinements
// =============================================================================

/// view.clear handler - Clear all refinements, return to base view
pub struct ViewClearOp;

#[async_trait]
impl CustomOperation for ViewClearOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "clear"
    }

    fn rationale(&self) -> &'static str {
        "Clears refinements from session view state"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Clear selection
        ctx.clear_selection();

        Ok(ExecutionResult::Record(json!({
            "message": "View cleared. Use view.universe, view.book, or view.cbu to set a new view."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.clear requires database feature"))
    }
}

// =============================================================================
// VIEW.SELECT - Explicitly set selection
// =============================================================================

/// view.select handler - Explicitly set selection within current view
pub struct ViewSelectOp;

#[async_trait]
impl CustomOperation for ViewSelectOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "select"
    }

    fn rationale(&self) -> &'static str {
        "Directly manipulates selection state"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Handle :none flag
        if get_bool_arg(verb_call, "none").unwrap_or(false) {
            ctx.clear_selection();
            return Ok(ExecutionResult::Record(json!({
                "selection_count": 0,
                "message": "Selection cleared"
            })));
        }

        // Handle :all flag - keep current selection as-is (it's already "all" from the view)
        if get_bool_arg(verb_call, "all").unwrap_or(false) {
            let count = ctx.selection_count();
            return Ok(ExecutionResult::Record(json!({
                "selection_count": count,
                "message": "All items selected"
            })));
        }

        // Handle explicit IDs
        if let Some(ids) = get_uuid_list_arg(verb_call, "ids", ctx) {
            ctx.set_selection(ids.clone());
            return Ok(ExecutionResult::Record(json!({
                "selection_count": ids.len(),
                "selection_ids": ids,
                "message": "Selection set explicitly"
            })));
        }

        Ok(ExecutionResult::Record(json!({
            "message": "No selection change. Use :ids, :all, or :none."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.select requires database feature"))
    }
}

// =============================================================================
// VIEW.LAYOUT - Change layout strategy
// =============================================================================

/// view.layout handler - Change layout strategy for current view
pub struct ViewLayoutOp;

#[async_trait]
impl CustomOperation for ViewLayoutOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "layout"
    }

    fn rationale(&self) -> &'static str {
        "Configures layout algorithm for visualization"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mode = get_string_arg(verb_call, "mode").unwrap_or_else(|| "auto".to_string());
        let primary_axis = get_string_arg(verb_call, "primary-axis");
        let size_by = get_string_arg(verb_call, "size-by");
        let color_by = get_string_arg(verb_call, "color-by");

        // Determine metaphor from mode
        let metaphor = match mode.as_str() {
            "galaxy" => Metaphor::Galaxy,
            "grid" => Metaphor::Tree, // Grid uses tree layout
            "tree" => Metaphor::Tree,
            "network" => Metaphor::Network,
            "pyramid" => Metaphor::Pyramid,
            _ => Metaphor::Tree, // auto derives from shape
        };

        Ok(ExecutionResult::Record(json!({
            "layout_mode": mode,
            "metaphor": format!("{:?}", metaphor),
            "primary_axis": primary_axis,
            "size_by": size_by,
            "color_by": color_by,
            "message": "Layout configuration updated"
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.layout requires database feature"))
    }
}

// =============================================================================
// VIEW.STATUS - Get current view state summary
// =============================================================================

/// view.status handler - Get current view state summary
pub struct ViewStatusOp;

#[async_trait]
impl CustomOperation for ViewStatusOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "status"
    }

    fn rationale(&self) -> &'static str {
        "Reports on current session view state"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let selection = ctx.get_selection();
        let has_selection = ctx.has_selection();
        let count = ctx.selection_count();

        Ok(ExecutionResult::Record(json!({
            "has_view": has_selection,
            "selection_count": count,
            "selection_ids": selection,
            "message": if has_selection {
                format!("View active with {} items selected", count)
            } else {
                "No active view. Use view.universe, view.book, or view.cbu to set one.".to_string()
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.status requires database feature"))
    }
}

// =============================================================================
// VIEW.SELECTION-INFO - Get detailed info about current selection
// =============================================================================

/// view.selection-info handler - Get detailed info about current selection
pub struct ViewSelectionInfoOp;

#[async_trait]
impl CustomOperation for ViewSelectionInfoOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "selection-info"
    }

    fn rationale(&self) -> &'static str {
        "Provides detailed information about selected items"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let selection = ctx.get_selection().cloned().unwrap_or_default();

        if selection.is_empty() {
            return Ok(ExecutionResult::Record(json!({
                "message": "No items selected",
                "count": 0,
                "ids": []
            })));
        }

        // TODO: Query database for detailed info on each selected item
        // For now, just return the IDs

        Ok(ExecutionResult::Record(json!({
            "count": selection.len(),
            "ids": selection,
            "message": format!("{} items in current selection", selection.len())
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "view.selection-info requires database feature"
        ))
    }
}

// =============================================================================
// VIEW.ZOOM-IN - Zoom into a node, expanding it into its child taxonomy
// =============================================================================

/// view.zoom-in handler - Zoom into a node using its expansion rule
pub struct ViewZoomInOp;

#[async_trait]
impl CustomOperation for ViewZoomInOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "zoom-in"
    }

    fn rationale(&self) -> &'static str {
        "Navigates into a node's child taxonomy using its expansion rule"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let node_id = get_uuid_arg(verb_call, "node-id", ctx)
            .ok_or_else(|| anyhow::anyhow!("node-id argument is required"))?;

        // Get current view from session context
        // Note: This operation needs session context, not just execution context
        // For now, we return a message indicating the zoom action
        // The actual zoom is performed by the session layer

        Ok(ExecutionResult::Record(json!({
            "action": "zoom-in",
            "node_id": node_id.to_string(),
            "message": format!("Zoom into node {}. Use session.zoom_in() to execute.", node_id)
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.zoom-in requires database feature"))
    }
}

// =============================================================================
// VIEW.ZOOM-OUT - Zoom out to the parent taxonomy
// =============================================================================

/// view.zoom-out handler - Pop the current frame and return to parent view
pub struct ViewZoomOutOp;

#[async_trait]
impl CustomOperation for ViewZoomOutOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "zoom-out"
    }

    fn rationale(&self) -> &'static str {
        "Navigates back to the parent taxonomy by popping the current frame"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Note: This operation needs session context, not just execution context
        // For now, we return a message indicating the zoom action
        // The actual zoom is performed by the session layer

        Ok(ExecutionResult::Record(json!({
            "action": "zoom-out",
            "message": "Zoom out to parent taxonomy. Use session.zoom_out() to execute."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.zoom-out requires database feature"))
    }
}

// =============================================================================
// VIEW.BACK-TO - Jump back to a specific breadcrumb level
// =============================================================================

/// view.back-to handler - Pop frames until reaching a target level
pub struct ViewBackToOp;

#[async_trait]
impl CustomOperation for ViewBackToOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "back-to"
    }

    fn rationale(&self) -> &'static str {
        "Navigates to a specific breadcrumb level by popping frames"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let depth = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depth")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as usize);

        let frame_id = get_uuid_arg(verb_call, "frame-id", ctx);

        // Note: This operation needs session context, not just execution context
        // For now, we return a message indicating the navigation action

        Ok(ExecutionResult::Record(json!({
            "action": "back-to",
            "depth": depth,
            "frame_id": frame_id.map(|id| id.to_string()),
            "message": "Navigate to breadcrumb level. Use session.back_to() to execute."
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.back-to requires database feature"))
    }
}

// =============================================================================
// VIEW.BREADCRUMBS - Get current navigation breadcrumbs
// =============================================================================

/// view.breadcrumbs handler - Returns the current navigation path
pub struct ViewBreadcrumbsOp;

#[async_trait]
impl CustomOperation for ViewBreadcrumbsOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "breadcrumbs"
    }

    fn rationale(&self) -> &'static str {
        "Reports on the current navigation stack for breadcrumb display"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Note: This operation needs session context to access the stack
        // For now, we return a placeholder indicating breadcrumbs should be fetched from session

        Ok(ExecutionResult::Record(json!({
            "action": "breadcrumbs",
            "message": "Get breadcrumbs from session.breadcrumbs() or session.breadcrumbs_with_ids()"
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "view.breadcrumbs requires database feature"
        ))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.DRILL
// =============================================================================

/// view.drill handler - Drill into an entity (subsidiaries or parent chain)
pub struct ViewDrillOp;

#[async_trait]
impl CustomOperation for ViewDrillOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "drill"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: drill into entity hierarchy with scale level advancement"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(verb_call, "entity-id", ctx)
            .ok_or_else(|| anyhow::anyhow!("entity-id argument is required"))?;

        let direction_str =
            get_string_arg(verb_call, "direction").unwrap_or_else(|| "down".to_string());
        let direction = match direction_str.as_str() {
            "up" => DrillDirection::Up,
            _ => DrillDirection::Down,
        };

        let depth = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depth")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(1) as u8;

        // Get current view and apply drill operation
        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;
            view.drill_into(entity_id, direction);
            view.visible_depth = depth;

            let result = ViewOpResult::from_view_state(&view);
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "drill",
                "entity_id": entity_id.to_string(),
                "direction": format!("{:?}", direction),
                "depth": depth,
                "scale_level": format!("{:?}", result.context),
                "message": format!("Drilled {} into entity {}", direction_str, entity_id)
            })))
        } else {
            // Create minimal view state with drill info
            let mut view = ViewState::empty();
            view.drill_into(entity_id, direction);
            view.visible_depth = depth;
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "drill",
                "entity_id": entity_id.to_string(),
                "direction": format!("{:?}", direction),
                "depth": depth,
                "message": format!("Drilled {} into entity {}", direction_str, entity_id)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.drill requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.SURFACE
// =============================================================================

/// view.surface handler - Surface up from drill (pop navigation stack)
pub struct ViewSurfaceOp;

#[async_trait]
impl CustomOperation for ViewSurfaceOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "surface"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: surface up from drill by popping navigation stack"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let levels = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "levels")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(1) as usize;

        let to_universe = get_bool_arg(verb_call, "to-universe").unwrap_or(false);

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;

            if to_universe {
                view.surface_to_universe();
            } else {
                for _ in 0..levels {
                    if !view.surface_up() {
                        break; // Stack is empty
                    }
                }
            }

            let scale = format!("{:?}", view.scale_level);
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "surface",
                "levels": levels,
                "to_universe": to_universe,
                "scale_level": scale,
                "message": if to_universe {
                    "Surfaced to universe view".to_string()
                } else {
                    format!("Surfaced {} level(s)", levels)
                }
            })))
        } else {
            Ok(ExecutionResult::Record(json!({
                "action": "surface",
                "message": "No active view to surface from"
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.surface requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.TRACE
// =============================================================================

/// view.trace handler - Follow money, control, or risk threads
pub struct ViewTraceOp;

#[async_trait]
impl CustomOperation for ViewTraceOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "trace"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: follow investigation threads (money, control, risk)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mode_str = get_string_arg(verb_call, "mode")
            .ok_or_else(|| anyhow::anyhow!("mode argument is required"))?;

        let trace_mode = match mode_str.to_lowercase().as_str() {
            "money" => TraceMode::Money,
            "control" => TraceMode::Control,
            "risk" => TraceMode::Risk,
            "documents" => TraceMode::Documents,
            "alerts" => TraceMode::Alerts,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid trace mode: {}. Valid: money, control, risk, documents, alerts",
                    mode_str
                ))
            }
        };

        let from_entity = get_uuid_arg(verb_call, "from-entity", ctx);
        let depth = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depth")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(5) as u8;

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;
            view.start_trace(trace_mode, from_entity);
            view.trace_depth = depth;
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "trace",
                "mode": format!("{:?}", trace_mode),
                "from_entity": from_entity.map(|id| id.to_string()),
                "depth": depth,
                "message": format!("Started {:?} trace", trace_mode)
            })))
        } else {
            let mut view = ViewState::empty();
            view.start_trace(trace_mode, from_entity);
            view.trace_depth = depth;
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "trace",
                "mode": format!("{:?}", trace_mode),
                "from_entity": from_entity.map(|id| id.to_string()),
                "depth": depth,
                "message": format!("Started {:?} trace", trace_mode)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.trace requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.XRAY
// =============================================================================

/// view.xray handler - Show hidden layers transparently
pub struct ViewXrayOp;

#[async_trait]
impl CustomOperation for ViewXrayOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "xray"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: x-ray mode to show hidden layers transparently"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let off = get_bool_arg(verb_call, "off").unwrap_or(false);
        let layers = get_string_list_arg(verb_call, "layers")
            .unwrap_or_else(|| vec!["custody".to_string(), "ubo".to_string()]);

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;

            if off {
                view.disable_xray();
            } else {
                view.enable_xray(layers.clone());
            }

            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "xray",
                "enabled": !off,
                "layers": layers,
                "message": if off { "X-ray mode disabled" } else { "X-ray mode enabled" }
            })))
        } else {
            let mut view = ViewState::empty();
            if !off {
                view.enable_xray(layers.clone());
            }
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "xray",
                "enabled": !off,
                "layers": layers,
                "message": if off { "X-ray mode disabled" } else { "X-ray mode enabled" }
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.xray requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.PEEL
// =============================================================================

/// view.peel handler - Remove outer layer to reveal inner structure
pub struct ViewPeelOp;

#[async_trait]
impl CustomOperation for ViewPeelOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "peel"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: peel away layers to reveal inner structure"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let layer = get_string_arg(verb_call, "layer");
        let reset = get_bool_arg(verb_call, "reset").unwrap_or(false);

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;

            if reset {
                view.unpeel_all();
            } else if let Some(layer_name) = &layer {
                view.peel_layer(layer_name.clone());
            } else {
                // Default peel when no layer specified
                view.peel_layer("outer".to_string());
            }

            let peel_depth = view.peel_depth;
            let peeled = view.peeled_layers.clone();
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "peel",
                "layer": layer,
                "reset": reset,
                "peel_depth": peel_depth,
                "peeled_layers": peeled,
                "message": if reset {
                    "Peel reset".to_string()
                } else {
                    format!("Peeled layer (depth: {})", peel_depth)
                }
            })))
        } else {
            let mut view = ViewState::empty();
            if !reset {
                if let Some(layer_name) = &layer {
                    view.peel_layer(layer_name.clone());
                } else {
                    // Default peel when no layer specified
                    view.peel_layer("outer".to_string());
                }
            }
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "peel",
                "message": "Peeled layer"
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.peel requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.ILLUMINATE
// =============================================================================

/// view.illuminate handler - Highlight specific aspect across all entities
pub struct ViewIlluminateOp;

#[async_trait]
impl CustomOperation for ViewIlluminateOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "illuminate"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: illuminate a specific aspect across all visible entities"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let aspect_str = get_string_arg(verb_call, "aspect")
            .ok_or_else(|| anyhow::anyhow!("aspect argument is required"))?;

        let aspect = match aspect_str.to_lowercase().as_str() {
            "risks" => IlluminateAspect::Risks,
            "documents" => IlluminateAspect::Documents,
            "screenings" => IlluminateAspect::Screenings,
            "gaps" => IlluminateAspect::Gaps,
            "pending" => IlluminateAspect::Pending,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid aspect: {}. Valid: risks, documents, screenings, gaps, pending",
                    aspect_str
                ))
            }
        };

        let off = get_bool_arg(verb_call, "off").unwrap_or(false);

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;

            if off {
                view.illuminate_aspect = None;
            } else {
                view.illuminate(aspect);
            }

            ctx.set_pending_view_state(view);

            let msg = if off {
                "Illumination cleared".to_string()
            } else {
                format!("Illuminating {:?}", aspect)
            };

            Ok(ExecutionResult::Record(json!({
                "action": "illuminate",
                "aspect": format!("{:?}", aspect),
                "enabled": !off,
                "message": msg
            })))
        } else {
            let mut view = ViewState::empty();
            if !off {
                view.illuminate(aspect);
            }
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "illuminate",
                "aspect": format!("{:?}", aspect),
                "enabled": !off,
                "message": format!("Illuminating {:?}", aspect)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.illuminate requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.SHADOW
// =============================================================================

/// view.shadow handler - Dim non-risk items to emphasize risk
pub struct ViewShadowOp;

#[async_trait]
impl CustomOperation for ViewShadowOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "shadow"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: shadow mode dims non-risk items to emphasize risks"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let threshold_str =
            get_string_arg(verb_call, "threshold").unwrap_or_else(|| "any".to_string());
        let threshold = match threshold_str.to_lowercase().as_str() {
            "high" => RiskThreshold::High,
            "medium" => RiskThreshold::Medium,
            _ => RiskThreshold::Any,
        };

        let off = get_bool_arg(verb_call, "off").unwrap_or(false);

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;

            if off {
                view.disable_shadow();
            } else {
                view.enable_shadow(threshold);
            }

            ctx.set_pending_view_state(view);

            let msg = if off {
                "Shadow mode disabled".to_string()
            } else {
                format!("Shadow mode: {:?} threshold", threshold)
            };

            Ok(ExecutionResult::Record(json!({
                "action": "shadow",
                "threshold": format!("{:?}", threshold),
                "enabled": !off,
                "message": msg
            })))
        } else {
            let mut view = ViewState::empty();
            if !off {
                view.enable_shadow(threshold);
            }
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "shadow",
                "threshold": format!("{:?}", threshold),
                "enabled": !off,
                "message": format!("Shadow mode: {:?} threshold", threshold)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.shadow requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.RED-FLAG
// =============================================================================

/// view.red-flag handler - Scan and highlight red flags
pub struct ViewRedFlagOp;

#[async_trait]
impl CustomOperation for ViewRedFlagOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "red-flag"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: red flag scan mode to highlight risk indicators"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let category_str =
            get_string_arg(verb_call, "category").unwrap_or_else(|| "all".to_string());
        let category = match category_str.to_lowercase().as_str() {
            "pep" => RedFlagCategory::Pep,
            "sanctions" => RedFlagCategory::Sanctions,
            "adverse-media" | "adversemedia" => RedFlagCategory::AdverseMedia,
            _ => RedFlagCategory::All,
        };

        let off = get_bool_arg(verb_call, "off").unwrap_or(false);

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;

            if off {
                view.stop_red_flag_scan();
            } else {
                view.start_red_flag_scan(Some(category));
            }

            ctx.set_pending_view_state(view);

            let msg = if off {
                "Red flag scan disabled".to_string()
            } else {
                format!("Red flag scan: {:?}", category)
            };

            Ok(ExecutionResult::Record(json!({
                "action": "red-flag",
                "category": format!("{:?}", category),
                "enabled": !off,
                "message": msg
            })))
        } else {
            let mut view = ViewState::empty();
            if !off {
                view.start_red_flag_scan(Some(category));
            }
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "red-flag",
                "category": format!("{:?}", category),
                "enabled": !off,
                "message": format!("Red flag scan: {:?}", category)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.red-flag requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.BLACK-HOLES
// =============================================================================

/// view.black-holes handler - Show data gaps as 'black holes'
pub struct ViewBlackHolesOp;

#[async_trait]
impl CustomOperation for ViewBlackHolesOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "black-holes"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: black hole mode visualizes data gaps"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let gap_type_str = get_string_arg(verb_call, "type").unwrap_or_else(|| "all".to_string());
        let gap_type = match gap_type_str.to_lowercase().as_str() {
            "ownership" => GapType::Ownership,
            "documents" => GapType::Documents,
            "screening" => GapType::Screening,
            _ => GapType::All,
        };

        let off = get_bool_arg(verb_call, "off").unwrap_or(false);

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;

            if off {
                view.disable_black_holes();
            } else {
                view.enable_black_holes(Some(gap_type));
            }

            ctx.set_pending_view_state(view);

            let msg = if off {
                "Black hole mode disabled".to_string()
            } else {
                format!("Black hole mode: {:?}", gap_type)
            };

            Ok(ExecutionResult::Record(json!({
                "action": "black-holes",
                "type": format!("{:?}", gap_type),
                "enabled": !off,
                "message": msg
            })))
        } else {
            let mut view = ViewState::empty();
            if !off {
                view.enable_black_holes(Some(gap_type));
            }
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "black-holes",
                "type": format!("{:?}", gap_type),
                "enabled": !off,
                "message": format!("Black hole mode: {:?}", gap_type)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "view.black-holes requires database feature"
        ))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.DETAIL
// =============================================================================

/// view.detail handler - Change detail level for current focus
pub struct ViewDetailOp;

#[async_trait]
impl CustomOperation for ViewDetailOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "detail"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: change detail level (graph, attributes, raw)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let level_str = get_string_arg(verb_call, "level")
            .ok_or_else(|| anyhow::anyhow!("level argument is required"))?;

        let level = match level_str.to_lowercase().as_str() {
            "graph" => DetailLevel::Graph,
            "attributes" => DetailLevel::Attributes,
            "raw" => DetailLevel::Raw,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid detail level: {}. Valid: graph, attributes, raw",
                    level_str
                ))
            }
        };

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;
            view.set_detail_level(level);
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "detail",
                "level": format!("{:?}", level),
                "message": format!("Detail level set to {:?}", level)
            })))
        } else {
            let mut view = ViewState::empty();
            view.set_detail_level(level);
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "detail",
                "level": format!("{:?}", level),
                "message": format!("Detail level set to {:?}", level)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.detail requires database feature"))
    }
}

// =============================================================================
// ESPER NAVIGATION - VIEW.CONTEXT
// =============================================================================

/// view.context handler - Set UI context mode
pub struct ViewContextOp;

#[async_trait]
impl CustomOperation for ViewContextOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "context"
    }

    fn rationale(&self) -> &'static str {
        "Esper navigation: set UI context mode for workflow adaptation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mode_str = get_string_arg(verb_call, "mode")
            .ok_or_else(|| anyhow::anyhow!("mode argument is required"))?;

        let mode = match mode_str.to_lowercase().as_str() {
            "onboarding" => ContextMode::Onboarding,
            "review" => ContextMode::Review,
            "investigation" => ContextMode::Investigation,
            "monitoring" => ContextMode::Monitoring,
            "remediation" => ContextMode::Remediation,
            _ => return Err(anyhow::anyhow!("Invalid context mode: {}. Valid: onboarding, review, investigation, monitoring, remediation", mode_str)),
        };

        if let Some(view) = ctx.take_pending_view_state() {
            let mut view = view;
            view.set_context_mode(mode);
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "context",
                "mode": format!("{:?}", mode),
                "message": format!("Context mode set to {:?}", mode)
            })))
        } else {
            let mut view = ViewState::empty();
            view.set_context_mode(mode);
            ctx.set_pending_view_state(view);

            Ok(ExecutionResult::Record(json!({
                "action": "context",
                "mode": format!("{:?}", mode),
                "message": format!("Context mode set to {:?}", mode)
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("view.context requires database feature"))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_op_result_serialization() {
        let result = ViewOpResult {
            context: "Universe".to_string(),
            total_count: 100,
            selection_count: 50,
            refinement_count: 2,
            has_pending: false,
            metaphor: "Galaxy".to_string(),
            selection_ids: vec![Uuid::new_v4()],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Universe"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_operation_domains() {
        assert_eq!(ViewUniverseOp.domain(), "view");
        assert_eq!(ViewBookOp.domain(), "view");
        assert_eq!(ViewCbuOp.domain(), "view");
        assert_eq!(ViewRefineOp.domain(), "view");
        assert_eq!(ViewClearOp.domain(), "view");
        assert_eq!(ViewSelectOp.domain(), "view");
        assert_eq!(ViewLayoutOp.domain(), "view");
        assert_eq!(ViewStatusOp.domain(), "view");
        assert_eq!(ViewSelectionInfoOp.domain(), "view");
    }

    #[test]
    fn test_operation_verbs() {
        assert_eq!(ViewUniverseOp.verb(), "universe");
        assert_eq!(ViewBookOp.verb(), "book");
        assert_eq!(ViewCbuOp.verb(), "cbu");
        assert_eq!(ViewEntityForestOp.verb(), "entity-forest");
        assert_eq!(ViewRefineOp.verb(), "refine");
        assert_eq!(ViewClearOp.verb(), "clear");
        assert_eq!(ViewSelectOp.verb(), "select");
        assert_eq!(ViewLayoutOp.verb(), "layout");
        assert_eq!(ViewStatusOp.verb(), "status");
        assert_eq!(ViewSelectionInfoOp.verb(), "selection-info");
    }

    #[test]
    fn test_zoom_operation_verbs() {
        assert_eq!(ViewZoomInOp.verb(), "zoom-in");
        assert_eq!(ViewZoomOutOp.verb(), "zoom-out");
        assert_eq!(ViewBackToOp.verb(), "back-to");
        assert_eq!(ViewBreadcrumbsOp.verb(), "breadcrumbs");
    }

    #[test]
    fn test_zoom_operation_domains() {
        assert_eq!(ViewZoomInOp.domain(), "view");
        assert_eq!(ViewZoomOutOp.domain(), "view");
        assert_eq!(ViewBackToOp.domain(), "view");
        assert_eq!(ViewBreadcrumbsOp.domain(), "view");
    }
}
