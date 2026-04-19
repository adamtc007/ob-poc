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
use dsl_runtime_macros::register_custom_op;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain_ops::CustomOperation;
use crate::session::{Refinement, ViewState};
use crate::taxonomy::{Filter, Metaphor, Status, TaxonomyBuilder, TaxonomyContext};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// LOCAL HELPERS — selection state transport via ctx.extensions
// =============================================================================
//
// `VerbExecutionContext` has no selection API. These helpers carry the view
// selection across native op calls through `ctx.extensions["_selection"]`,
// mirroring the legacy `ExecutionContext::{set,get,clear}_selection` +
// `bind_json("_selection", ...)` pattern.

const EXT_KEY_SELECTION: &str = "_selection";

fn ext_obj_mut(
    ctx: &mut dsl_runtime::VerbExecutionContext,
) -> &mut serde_json::Map<String, serde_json::Value> {
    if !ctx.extensions.is_object() {
        ctx.extensions = serde_json::Value::Object(serde_json::Map::new());
    }
    ctx.extensions.as_object_mut().unwrap()
}

fn set_selection(ctx: &mut dsl_runtime::VerbExecutionContext, selection: Vec<Uuid>) {
    if let Ok(v) = serde_json::to_value(&selection) {
        ext_obj_mut(ctx).insert(EXT_KEY_SELECTION.to_string(), v);
    }
}

fn get_selection(ctx: &dsl_runtime::VerbExecutionContext) -> Option<Vec<Uuid>> {
    let v = ctx.extensions.as_object()?.get(EXT_KEY_SELECTION)?;
    serde_json::from_value(v.clone()).ok()
}

fn has_selection(ctx: &dsl_runtime::VerbExecutionContext) -> bool {
    get_selection(ctx).is_some_and(|s| !s.is_empty())
}

fn selection_count(ctx: &dsl_runtime::VerbExecutionContext) -> usize {
    get_selection(ctx).map(|s| s.len()).unwrap_or(0)
}

fn clear_selection(ctx: &mut dsl_runtime::VerbExecutionContext) {
    if let Some(obj) = ctx.extensions.as_object_mut() {
        obj.remove(EXT_KEY_SELECTION);
    }
}

/// Parse filter from a JSON arg object (used by view.refine).
fn parse_filter_from_json(args: &serde_json::Value, arg_name: &str) -> Option<Filter> {
    let obj = args.get(arg_name)?.as_object()?;
    if let Some(jurisdictions) = obj.get("jurisdiction").and_then(|v| v.as_array()) {
        let juris: Vec<String> = jurisdictions
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        return Some(Filter::Jurisdiction(juris));
    }
    if let Some(statuses) = obj.get("status").and_then(|v| v.as_array()) {
        let stats: Vec<Status> = statuses
            .iter()
            .filter_map(|v| {
                v.as_str().and_then(|s| match s.to_uppercase().as_str() {
                    "RED" => Some(Status::Red),
                    "AMBER" => Some(Status::Amber),
                    "GREEN" => Some(Status::Green),
                    _ => None,
                })
            })
            .collect();
        return Some(Filter::Status(stats));
    }
    if let Some(types) = obj.get("fund_type").and_then(|v| v.as_array()) {
        let fund_types: Vec<String> = types
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        return Some(Filter::FundType(fund_types));
    }
    None
}

/// Extract UUID list from a JSON arg (accepts symbol refs via ctx, UUID strings, or a single scalar).
fn json_extract_uuid_list_opt(
    args: &serde_json::Value,
    ctx: &dsl_runtime::VerbExecutionContext,
    arg_name: &str,
) -> Option<Vec<Uuid>> {
    let v = args.get(arg_name)?;
    if let Some(arr) = v.as_array() {
        let uuids: Vec<Uuid> = arr
            .iter()
            .filter_map(|item| {
                if let Some(s) = item.as_str() {
                    if let Some(sym) = s.strip_prefix('@') {
                        return ctx.resolve(sym);
                    }
                    return Uuid::parse_str(s).ok();
                }
                None
            })
            .collect();
        if uuids.is_empty() {
            None
        } else {
            Some(uuids)
        }
    } else if let Some(s) = v.as_str() {
        if let Some(sym) = s.strip_prefix('@') {
            ctx.resolve(sym).map(|u| vec![u])
        } else {
            Uuid::parse_str(s).ok().map(|u| vec![u])
        }
    } else {
        None
    }
}

/// Extract a string list from a JSON arg.
fn json_extract_string_list_opt(args: &serde_json::Value, arg_name: &str) -> Option<Vec<String>> {
    let v = args.get(arg_name)?;
    if let Some(arr) = v.as_array() {
        let strings: Vec<String> = arr
            .iter()
            .filter_map(|item| item.as_str().map(|s| s.to_string()))
            .collect();
        if strings.is_empty() {
            None
        } else {
            Some(strings)
        }
    } else if let Some(s) = v.as_str() {
        Some(vec![s.to_string()])
    } else {
        None
    }
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
#[register_custom_op]
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Build context from args
        let taxonomy_ctx =
            if let Some(client_id) = super::helpers::json_extract_uuid_opt(args, ctx, "client") {
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
        if let Some(jurisdictions) = json_extract_string_list_opt(args, "jurisdiction") {
            view.refine(Refinement::Include {
                filter: Filter::Jurisdiction(jurisdictions),
            });
        }

        if let Some(statuses) = json_extract_string_list_opt(args, "status") {
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

        if let Some(fund_types) = json_extract_string_list_opt(args, "fund-type") {
            view.refine(Refinement::Include {
                filter: Filter::FundType(fund_types),
            });
        }

        // Bind selection to execution context for DSL access
        set_selection(ctx, view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        // This fixes the "session state side door" - ViewState was previously discarded
        super::helpers::ext_set_pending_view_state(ctx, view);

        // Return as JSON
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(&result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.BOOK - View all CBUs for a commercial client
// =============================================================================

/// view.book handler - View all CBUs for a commercial client
#[register_custom_op]
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let client_id = super::helpers::json_extract_uuid(args, ctx, "client")?;

        let taxonomy_ctx = TaxonomyContext::Book { client_id };
        let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
        let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;

        let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

        // Bind selection to execution context
        set_selection(ctx, view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        super::helpers::ext_set_pending_view_state(ctx, view);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(&result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.CBU - Focus on a single CBU
// =============================================================================

/// view.cbu handler - Focus on a single CBU with specified view mode
#[register_custom_op]
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let cbu_id = super::helpers::json_extract_uuid(args, ctx, "cbu-id")?;

        let mode = super::helpers::json_extract_string_opt(args, "mode")
            .unwrap_or_else(|| "trading".to_string());

        let taxonomy_ctx = match mode.as_str() {
            "ubo" => TaxonomyContext::CbuUbo { cbu_id },
            _ => TaxonomyContext::CbuTrading { cbu_id },
        };

        let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
        let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;

        let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

        // Bind selection to execution context
        set_selection(ctx, view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        super::helpers::ext_set_pending_view_state(ctx, view);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(&result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.ENTITY-FOREST - View entities by type/ownership filters
// =============================================================================

/// view.entity-forest handler - View entities filtered by type, jurisdiction, role
#[register_custom_op]
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let mut filters = Vec::new();

        if let Some(jurisdictions) = json_extract_string_list_opt(args, "jurisdiction") {
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
        set_selection(ctx, view.selection.clone());

        let result = ViewOpResult::from_view_state(&view);

        // Store ViewState in ExecutionContext for propagation to UnifiedSessionContext
        super::helpers::ext_set_pending_view_state(ctx, view);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(&result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.REFINE - Refine current view with additional filter
// =============================================================================

/// view.refine handler - Apply refinement to current view
#[register_custom_op]
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Get current selection from context
        let current_selection = get_selection(ctx).unwrap_or_default();

        if current_selection.is_empty() {
            return Err(anyhow::anyhow!(
                "No active view. Use view.universe, view.book, or view.cbu first."
            ));
        }

        let mut new_selection = current_selection.clone();

        // Apply include filter
        if let Some(filter) = parse_filter_from_json(args, "include") {
            // This would normally filter against taxonomy nodes
            // For now, we just note that refinement was requested
            tracing::debug!(?filter, "Applying include filter");
        }

        // Apply exclude filter
        if let Some(filter) = parse_filter_from_json(args, "exclude") {
            tracing::debug!(?filter, "Applying exclude filter");
        }

        // Add specific IDs
        if let Some(add_ids) = json_extract_uuid_list_opt(args, ctx, "add") {
            for id in add_ids {
                if !new_selection.contains(&id) {
                    new_selection.push(id);
                }
            }
        }

        // Remove specific IDs
        if let Some(remove_ids) = json_extract_uuid_list_opt(args, ctx, "remove") {
            new_selection.retain(|id| !remove_ids.contains(id));
        }

        // Update selection in context
        set_selection(ctx, new_selection.clone());

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "selection_count": new_selection.len(),
            "selection_ids": new_selection,
            "message": "View refined successfully"
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.CLEAR - Clear refinements
// =============================================================================

/// view.clear-refinements handler - Clear all refinements, return to base view
#[register_custom_op]
pub struct ViewClearOp;

#[async_trait]
impl CustomOperation for ViewClearOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "clear-refinements"
    }

    fn rationale(&self) -> &'static str {
        "Clears refinements from session view state"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Clear selection
        clear_selection(ctx);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "message": "View cleared. Use view.universe, view.book, or view.cbu to set a new view."
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


/// view.clear handler - Legacy alias for clearing refinements
#[register_custom_op]
pub struct ViewClearAliasOp;

#[async_trait]
impl CustomOperation for ViewClearAliasOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "clear"
    }

    fn rationale(&self) -> &'static str {
        "Provides backward-compatible access to clearing view refinements"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        clear_selection(ctx);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "message": "View cleared. Use view.universe, view.book, or view.cbu to set a new view."
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.SELECT - Explicitly set selection
// =============================================================================

/// view.set-selection handler - Explicitly set selection within current view
#[register_custom_op]
pub struct ViewSelectOp;

#[async_trait]
impl CustomOperation for ViewSelectOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "set-selection"
    }

    fn rationale(&self) -> &'static str {
        "Directly manipulates selection state"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Handle :none flag
        if super::helpers::json_extract_bool_opt(args, "none").unwrap_or(false) {
            clear_selection(ctx);
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
                "selection_count": 0,
                "message": "Selection cleared"
            })));
        }

        // Handle :all flag - keep current selection as-is (it's already "all" from the view)
        if super::helpers::json_extract_bool_opt(args, "all").unwrap_or(false) {
            let count = selection_count(ctx);
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
                "selection_count": count,
                "message": "All items selected"
            })));
        }

        // Handle explicit IDs
        if let Some(ids) = json_extract_uuid_list_opt(args, ctx, "ids") {
            set_selection(ctx, ids.clone());
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
                "selection_count": ids.len(),
                "selection_ids": ids,
                "message": "Selection set explicitly"
            })));
        }

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "message": "No selection change. Use :ids, :all, or :none."
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.LAYOUT - Change layout strategy
// =============================================================================

/// view.set-layout handler - Change layout strategy for current view
#[register_custom_op]
pub struct ViewLayoutOp;

#[async_trait]
impl CustomOperation for ViewLayoutOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "set-layout"
    }

    fn rationale(&self) -> &'static str {
        "Configures layout algorithm for visualization"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_string_opt;

        let mode = json_extract_string_opt(args, "mode").unwrap_or_else(|| "auto".to_string());
        let primary_axis = json_extract_string_opt(args, "primary-axis");
        let size_by = json_extract_string_opt(args, "size-by");
        let color_by = json_extract_string_opt(args, "color-by");

        let metaphor = match mode.as_str() {
            "galaxy" => Metaphor::Galaxy,
            "grid" => Metaphor::Tree,
            "tree" => Metaphor::Tree,
            "network" => Metaphor::Network,
            "pyramid" => Metaphor::Pyramid,
            _ => Metaphor::Tree,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "layout_mode": mode,
                "metaphor": format!("{:?}", metaphor),
                "primary_axis": primary_axis,
                "size_by": size_by,
                "color_by": color_by,
                "message": "Layout configuration updated"
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.STATUS - Get current view state summary
// =============================================================================

/// view.read-status handler - Get current view state summary
#[register_custom_op]
pub struct ViewStatusOp;

#[async_trait]
impl CustomOperation for ViewStatusOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "read-status"
    }

    fn rationale(&self) -> &'static str {
        "Reports on current session view state"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let selection = get_selection(ctx);
        let has_sel = has_selection(ctx);
        let count = selection_count(ctx);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "has_view": has_sel,
            "selection_count": count,
            "selection_ids": selection,
            "message": if has_sel {
                format!("View active with {} items selected", count)
            } else {
                "No active view. Use view.universe, view.book, or view.cbu to set one.".to_string()
            }
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.SELECTION-INFO - Get detailed info about current selection
// =============================================================================

/// view.read-selection-info handler - Get detailed info about current selection
#[register_custom_op]
pub struct ViewSelectionInfoOp;

#[async_trait]
impl CustomOperation for ViewSelectionInfoOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "read-selection-info"
    }

    fn rationale(&self) -> &'static str {
        "Provides detailed information about selected items"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let selection = get_selection(ctx).unwrap_or_default();

        if selection.is_empty() {
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
                "message": "No items selected",
                "count": 0,
                "ids": []
            })));
        }

        // TODO: Query database for detailed info on each selected item
        // For now, just return the IDs

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "count": selection.len(),
            "ids": selection,
            "message": format!("{} items in current selection", selection.len())
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.ZOOM-IN - Zoom into a node, expanding it into its child taxonomy
// =============================================================================

/// view.zoom-in handler - Zoom into a node using its expansion rule
#[register_custom_op]
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_uuid;
        let node_id = json_extract_uuid(args, ctx, "node-id")?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "action": "zoom-in",
                "node_id": node_id.to_string(),
                "message": format!("Zoom into node {}. Use session.zoom_in() to execute.", node_id)
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.ZOOM-OUT - Zoom out to the parent taxonomy
// =============================================================================

/// view.zoom-out handler - Pop the current frame and return to parent view
#[register_custom_op]
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
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "action": "zoom-out",
                "message": "Zoom out to parent taxonomy. Use session.zoom_out() to execute."
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.BACK-TO - Jump back to a specific breadcrumb level
// =============================================================================

/// view.navigate-back-to handler - Pop frames until reaching a target level
#[register_custom_op]
pub struct ViewBackToOp;

#[async_trait]
impl CustomOperation for ViewBackToOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "navigate-back-to"
    }

    fn rationale(&self) -> &'static str {
        "Navigates to a specific breadcrumb level by popping frames"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_uuid_opt};
        let depth = json_extract_int_opt(args, "depth").map(|i| i as usize);
        let frame_id = json_extract_uuid_opt(args, ctx, "frame-id");
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "action": "navigate-back-to",
                "depth": depth,
                "frame_id": frame_id.map(|id| id.to_string()),
                "message": "Navigate to breadcrumb level. Use session.back_to() to execute."
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// VIEW.BREADCRUMBS - Get current navigation breadcrumbs
// =============================================================================

/// view.read-breadcrumbs handler - Returns the current navigation path
#[register_custom_op]
pub struct ViewBreadcrumbsOp;

#[async_trait]
impl CustomOperation for ViewBreadcrumbsOp {
    fn domain(&self) -> &'static str {
        "view"
    }

    fn verb(&self) -> &'static str {
        "read-breadcrumbs"
    }

    fn rationale(&self) -> &'static str {
        "Reports on the current navigation stack for breadcrumb display"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "action": "read-breadcrumbs",
                "message": "Get breadcrumbs from session.breadcrumbs() or session.breadcrumbs_with_ids()"
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
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
        assert_eq!(ViewClearOp.verb(), "clear-refinements");
        assert_eq!(ViewSelectOp.verb(), "set-selection");
        assert_eq!(ViewLayoutOp.verb(), "set-layout");
        assert_eq!(ViewStatusOp.verb(), "read-status");
        assert_eq!(ViewSelectionInfoOp.verb(), "read-selection-info");
    }

    #[test]
    fn test_zoom_operation_verbs() {
        assert_eq!(ViewZoomInOp.verb(), "zoom-in");
        assert_eq!(ViewZoomOutOp.verb(), "zoom-out");
        assert_eq!(ViewBackToOp.verb(), "navigate-back-to");
        assert_eq!(ViewBreadcrumbsOp.verb(), "read-breadcrumbs");
    }

    #[test]
    fn test_zoom_operation_domains() {
        assert_eq!(ViewZoomInOp.domain(), "view");
        assert_eq!(ViewZoomOutOp.domain(), "view");
        assert_eq!(ViewBackToOp.domain(), "view");
        assert_eq!(ViewBreadcrumbsOp.domain(), "view");
    }
}
