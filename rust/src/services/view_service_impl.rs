//! ob-poc impl of [`dsl_runtime::service_traits::ViewService`].
//!
//! Single-method trait that dispatches all 15 `view.*` verbs from
//! `config/verbs/view.yaml`. Lives entirely in ob-poc because it
//! consumes the heavy session + taxonomy modules:
//!  - `crate::session::{ViewState, Refinement}` (10934 LOC, multi-consumer)
//!  - `crate::taxonomy::{Filter, Status, Metaphor, TaxonomyBuilder, TaxonomyContext}`
//!    (5345 LOC, multi-consumer)
//!
//! Dispatch table is verbatim the previous handler bodies from
//! `crate::domain_ops::view_ops` (relocated to the data plane in
//! Phase 5a slice #26 via YAML-first re-implementation).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::service_traits::ViewService;

use crate::session::{Refinement, ViewState};
use crate::taxonomy::{Filter, Metaphor, Status, TaxonomyBuilder, TaxonomyContext};

const EXT_KEY_SELECTION: &str = "_selection";
const EXT_KEY_PENDING_VIEW: &str = "_pending_view_state";

pub struct ObPocViewService;

impl ObPocViewService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocViewService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ViewService for ObPocViewService {
    async fn dispatch_view_verb(
        &self,
        pool: &PgPool,
        verb_name: &str,
        args: &Value,
        extensions: &mut Value,
    ) -> Result<Value> {
        match verb_name {
            "universe" => view_universe(pool, args, extensions).await,
            "book" => view_book(pool, args, extensions).await,
            "cbu" => view_cbu(pool, args, extensions).await,
            "entity-forest" => view_entity_forest(pool, args, extensions).await,
            "refine" => view_refine(args, extensions),
            "clear-refinements" | "clear" => view_clear(extensions),
            "set-selection" => view_set_selection(args, extensions),
            "set-layout" => view_set_layout(args),
            "read-status" => view_read_status(extensions),
            "read-selection-info" => view_read_selection_info(extensions),
            "zoom-in" => view_zoom_in(args),
            "zoom-out" => view_zoom_out(),
            "navigate-back-to" => view_navigate_back_to(args),
            "read-breadcrumbs" => view_read_breadcrumbs(),
            other => Err(anyhow!("unknown view verb: {other}")),
        }
    }
}

// ── Result type ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
struct ViewOpResult {
    context: String,
    total_count: usize,
    selection_count: usize,
    refinement_count: usize,
    has_pending: bool,
    metaphor: String,
    selection_ids: Vec<Uuid>,
}

impl ViewOpResult {
    fn from_view_state(view: &ViewState) -> Self {
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

// ── Extension helpers (selection state, pending view state) ───────────────────

fn ext_obj_mut(extensions: &mut Value) -> &mut Map<String, Value> {
    if !extensions.is_object() {
        *extensions = Value::Object(Map::new());
    }
    extensions.as_object_mut().unwrap()
}

fn set_selection(extensions: &mut Value, selection: Vec<Uuid>) {
    if let Ok(v) = serde_json::to_value(&selection) {
        ext_obj_mut(extensions).insert(EXT_KEY_SELECTION.to_string(), v);
    }
}

fn get_selection(extensions: &Value) -> Vec<Uuid> {
    extensions
        .as_object()
        .and_then(|obj| obj.get(EXT_KEY_SELECTION))
        .and_then(|v| serde_json::from_value::<Vec<Uuid>>(v.clone()).ok())
        .unwrap_or_default()
}

fn clear_selection(extensions: &mut Value) {
    if let Some(obj) = extensions.as_object_mut() {
        obj.remove(EXT_KEY_SELECTION);
    }
}

fn set_pending_view_state(extensions: &mut Value, view: &ViewState) {
    if let Ok(v) = serde_json::to_value(view) {
        ext_obj_mut(extensions).insert(EXT_KEY_PENDING_VIEW.to_string(), v);
    }
}

// ── JSON arg helpers (Uuid + string list extraction) ──────────────────────────

fn arg_uuid(args: &Value, name: &str) -> Option<Uuid> {
    args.get(name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn arg_string_opt(args: &Value, name: &str) -> Option<String> {
    args.get(name).and_then(|v| v.as_str()).map(String::from)
}

fn arg_bool_opt(args: &Value, name: &str) -> Option<bool> {
    args.get(name).and_then(|v| v.as_bool())
}

fn arg_int_opt(args: &Value, name: &str) -> Option<i64> {
    args.get(name).and_then(|v| v.as_i64())
}

fn arg_string_list(args: &Value, name: &str) -> Option<Vec<String>> {
    let v = args.get(name)?;
    if let Some(arr) = v.as_array() {
        let strings: Vec<String> = arr
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect();
        if strings.is_empty() {
            None
        } else {
            Some(strings)
        }
    } else {
        v.as_str().map(|s| vec![s.to_string()])
    }
}

fn arg_uuid_list(args: &Value, name: &str) -> Option<Vec<Uuid>> {
    let v = args.get(name)?;
    if let Some(arr) = v.as_array() {
        let uuids: Vec<Uuid> = arr
            .iter()
            .filter_map(|item| item.as_str().and_then(|s| Uuid::parse_str(s).ok()))
            .collect();
        if uuids.is_empty() {
            None
        } else {
            Some(uuids)
        }
    } else {
        v.as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(|u| vec![u])
    }
}

fn parse_filter(args: &Value, key: &str) -> Option<Filter> {
    let obj = args.get(key)?.as_object()?;
    if let Some(jurisdictions) = obj.get("jurisdiction").and_then(|v| v.as_array()) {
        let juris: Vec<String> = jurisdictions
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
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
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        return Some(Filter::FundType(fund_types));
    }
    None
}

// ── view.universe ─────────────────────────────────────────────────────────────

async fn view_universe(pool: &PgPool, args: &Value, extensions: &mut Value) -> Result<Value> {
    let taxonomy_ctx = match arg_uuid(args, "client") {
        Some(client_id) => TaxonomyContext::Book { client_id },
        None => TaxonomyContext::Universe,
    };
    let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
    let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;
    let mut view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

    if let Some(jurisdictions) = arg_string_list(args, "jurisdiction") {
        view.refine(Refinement::Include {
            filter: Filter::Jurisdiction(jurisdictions),
        });
    }
    if let Some(statuses) = arg_string_list(args, "status") {
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
    if let Some(fund_types) = arg_string_list(args, "fund-type") {
        view.refine(Refinement::Include {
            filter: Filter::FundType(fund_types),
        });
    }

    set_selection(extensions, view.selection.clone());
    let result = ViewOpResult::from_view_state(&view);
    set_pending_view_state(extensions, &view);
    Ok(serde_json::to_value(result)?)
}

// ── view.book ─────────────────────────────────────────────────────────────────

async fn view_book(pool: &PgPool, args: &Value, extensions: &mut Value) -> Result<Value> {
    let client_id =
        arg_uuid(args, "client").ok_or_else(|| anyhow!("view.book requires :client UUID arg"))?;
    let taxonomy_ctx = TaxonomyContext::Book { client_id };
    let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
    let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;
    let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

    set_selection(extensions, view.selection.clone());
    let result = ViewOpResult::from_view_state(&view);
    set_pending_view_state(extensions, &view);
    Ok(serde_json::to_value(result)?)
}

// ── view.cbu ──────────────────────────────────────────────────────────────────

async fn view_cbu(pool: &PgPool, args: &Value, extensions: &mut Value) -> Result<Value> {
    let cbu_id =
        arg_uuid(args, "cbu-id").ok_or_else(|| anyhow!("view.cbu requires :cbu-id UUID arg"))?;
    let mode = arg_string_opt(args, "mode").unwrap_or_else(|| "trading".to_string());
    let taxonomy_ctx = match mode.as_str() {
        "ubo" => TaxonomyContext::CbuUbo { cbu_id },
        _ => TaxonomyContext::CbuTrading { cbu_id },
    };
    let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
    let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;
    let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

    set_selection(extensions, view.selection.clone());
    let result = ViewOpResult::from_view_state(&view);
    set_pending_view_state(extensions, &view);
    Ok(serde_json::to_value(result)?)
}

// ── view.entity-forest ────────────────────────────────────────────────────────

async fn view_entity_forest(pool: &PgPool, args: &Value, extensions: &mut Value) -> Result<Value> {
    let mut filters = Vec::new();
    if let Some(jurisdictions) = arg_string_list(args, "jurisdiction") {
        filters.push(Filter::Jurisdiction(jurisdictions));
    }
    let taxonomy_ctx = TaxonomyContext::EntityForest {
        filters: filters.clone(),
    };
    let rules = taxonomy_ctx.to_rules_from_config(pool).await?;
    let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;
    let view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

    set_selection(extensions, view.selection.clone());
    let result = ViewOpResult::from_view_state(&view);
    set_pending_view_state(extensions, &view);
    Ok(serde_json::to_value(result)?)
}

// ── view.refine ───────────────────────────────────────────────────────────────

fn view_refine(args: &Value, extensions: &mut Value) -> Result<Value> {
    let current = get_selection(extensions);
    if current.is_empty() {
        return Err(anyhow!(
            "No active view. Use view.universe, view.book, or view.cbu first."
        ));
    }
    let mut new_selection = current;

    // Filter args (include / exclude) currently log only — they would
    // require taxonomy nodes for full filtering. Preserve the existing
    // behaviour: log + accept add/remove explicit IDs.
    if let Some(filter) = parse_filter(args, "include") {
        tracing::debug!(?filter, "Applying include filter");
    }
    if let Some(filter) = parse_filter(args, "exclude") {
        tracing::debug!(?filter, "Applying exclude filter");
    }
    if let Some(add_ids) = arg_uuid_list(args, "add") {
        for id in add_ids {
            if !new_selection.contains(&id) {
                new_selection.push(id);
            }
        }
    }
    if let Some(remove_ids) = arg_uuid_list(args, "remove") {
        new_selection.retain(|id| !remove_ids.contains(id));
    }

    set_selection(extensions, new_selection.clone());
    Ok(json!({
        "selection_count": new_selection.len(),
        "selection_ids": new_selection,
        "message": "View refined successfully",
    }))
}

// ── view.clear-refinements / view.clear ───────────────────────────────────────

fn view_clear(extensions: &mut Value) -> Result<Value> {
    clear_selection(extensions);
    Ok(json!({
        "message": "View cleared. Use view.universe, view.book, or view.cbu to set a new view.",
    }))
}

// ── view.set-selection ────────────────────────────────────────────────────────

fn view_set_selection(args: &Value, extensions: &mut Value) -> Result<Value> {
    if arg_bool_opt(args, "none").unwrap_or(false) {
        clear_selection(extensions);
        return Ok(json!({ "selection_count": 0, "message": "Selection cleared" }));
    }
    if arg_bool_opt(args, "all").unwrap_or(false) {
        let count = get_selection(extensions).len();
        return Ok(json!({ "selection_count": count, "message": "All items selected" }));
    }
    if let Some(ids) = arg_uuid_list(args, "ids") {
        set_selection(extensions, ids.clone());
        return Ok(json!({
            "selection_count": ids.len(),
            "selection_ids": ids,
            "message": "Selection set explicitly",
        }));
    }
    Ok(json!({ "message": "No selection change. Use :ids, :all, or :none." }))
}

// ── view.set-layout ───────────────────────────────────────────────────────────

fn view_set_layout(args: &Value) -> Result<Value> {
    let mode = arg_string_opt(args, "mode").unwrap_or_else(|| "auto".to_string());
    let primary_axis = arg_string_opt(args, "primary-axis");
    let size_by = arg_string_opt(args, "size-by");
    let color_by = arg_string_opt(args, "color-by");
    let metaphor = match mode.as_str() {
        "galaxy" => Metaphor::Galaxy,
        "grid" | "tree" => Metaphor::Tree,
        "network" => Metaphor::Network,
        "pyramid" => Metaphor::Pyramid,
        _ => Metaphor::Tree,
    };
    Ok(json!({
        "layout_mode": mode,
        "metaphor": format!("{:?}", metaphor),
        "primary_axis": primary_axis,
        "size_by": size_by,
        "color_by": color_by,
        "message": "Layout configuration updated",
    }))
}

// ── view.read-status ──────────────────────────────────────────────────────────

fn view_read_status(extensions: &Value) -> Result<Value> {
    let selection = get_selection(extensions);
    let count = selection.len();
    let has_view = !selection.is_empty();
    Ok(json!({
        "has_view": has_view,
        "selection_count": count,
        "selection_ids": selection,
        "message": if has_view {
            format!("View active with {count} items selected")
        } else {
            "No active view. Use view.universe, view.book, or view.cbu to set one.".to_string()
        },
    }))
}

// ── view.read-selection-info ──────────────────────────────────────────────────

fn view_read_selection_info(extensions: &Value) -> Result<Value> {
    let selection = get_selection(extensions);
    if selection.is_empty() {
        return Ok(json!({ "message": "No items selected", "count": 0, "ids": [] }));
    }
    Ok(json!({
        "count": selection.len(),
        "ids": selection.clone(),
        "message": format!("{} items in current selection", selection.len()),
    }))
}

// ── view.zoom-in / zoom-out / navigate-back-to / read-breadcrumbs ─────────────

fn view_zoom_in(args: &Value) -> Result<Value> {
    let node_id = arg_uuid(args, "node-id")
        .ok_or_else(|| anyhow!("view.zoom-in requires :node-id UUID arg"))?;
    Ok(json!({
        "action": "zoom-in",
        "node_id": node_id.to_string(),
        "message": format!("Zoom into node {node_id}. Use session.zoom_in() to execute."),
    }))
}

fn view_zoom_out() -> Result<Value> {
    Ok(json!({
        "action": "zoom-out",
        "message": "Zoom out to parent taxonomy. Use session.zoom_out() to execute.",
    }))
}

fn view_navigate_back_to(args: &Value) -> Result<Value> {
    let depth = arg_int_opt(args, "depth").map(|i| i as usize);
    let frame_id = arg_uuid(args, "frame-id");
    Ok(json!({
        "action": "navigate-back-to",
        "depth": depth,
        "frame_id": frame_id.map(|id| id.to_string()),
        "message": "Navigate to breadcrumb level. Use session.back_to() to execute.",
    }))
}

fn view_read_breadcrumbs() -> Result<Value> {
    Ok(json!({
        "action": "read-breadcrumbs",
        "message": "Get breadcrumbs from session.breadcrumbs() or session.breadcrumbs_with_ids()",
    }))
}
