//! Viewport DSL operations for Esper-style navigation
//!
//! These operations produce `pending_viewport_state` changes in the ExecutionContext.
//! The viewport state is then propagated to the session and consumed by the UI.
//!
//! ## Verbs
//!
//! | Verb | Description |
//! |------|-------------|
//! | `viewport.focus` | Focus on a CBU container or entity |
//! | `viewport.enhance` | Change enhance level for current focus |
//! | `viewport.ascend` | Navigate up the focus stack |
//! | `viewport.descend` | Navigate into a nested element |
//! | `viewport.camera` | Set camera position and zoom |
//! | `viewport.filter` | Set viewport filters |
//! | `viewport.track` | Lock camera onto an entity |
//! | `viewport.clear` | Clear focus state |
//! | `viewport.view-type` | Change the view type (structure, ownership, etc.) |

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[cfg(feature = "database")]
use sqlx::PgPool;

use dsl_core::ast::VerbCall;
use ob_poc_types::viewport::{
    CameraState, CbuRef, CbuViewType, ConcreteEntityRef, ConcreteEntityType, ConfidenceZone,
    EnhanceArg, FocusMode, ViewportFilters, ViewportFocusState, ViewportState,
};
use uuid::Uuid;

use crate::dsl_v2::custom_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

// ============================================================================
// HELPER FUNCTIONS (matching view_ops.rs patterns using AstNode methods)
// ============================================================================

fn get_uuid_arg(verb_call: &VerbCall, name: &str, ctx: &ExecutionContext) -> Option<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| {
            // First try resolving from symbol bindings using ctx.resolve()
            if let dsl_core::ast::AstNode::SymbolRef { name: sym_name, .. } = &a.value {
                if let Some(uuid) = ctx.resolve(sym_name) {
                    return Some(uuid);
                }
            }
            // Then try parsing as UUID from string or resolved entity ref
            a.value.as_uuid()
        })
}

fn get_string_arg(verb_call: &VerbCall, name: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

fn get_f32_arg(verb_call: &VerbCall, name: &str) -> Option<f32> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_integer().map(|n| n as f32))
}

fn get_u8_arg(verb_call: &VerbCall, name: &str) -> Option<u8> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_integer().map(|n| n as u8))
}

fn get_bool_arg(verb_call: &VerbCall, name: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_boolean())
}

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

// ============================================================================
// VIEWPORT.FOCUS - Focus on CBU container or entity
// ============================================================================

/// Focus on a CBU or entity within a CBU
///
/// ## DSL Examples
/// ```dsl
/// (viewport.focus :cbu-id @fund)
/// (viewport.focus :cbu-id @fund :entity-id @director)
/// (viewport.focus :cbu-id @fund :enhance-level 2)
/// ```
pub struct ViewportFocusOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportFocusResult {
    pub focus_type: String,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub enhance_level: u8,
}

#[async_trait]
impl CustomOperation for ViewportFocusOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "focus"
    }

    fn rationale(&self) -> &'static str {
        "Sets viewport focus on CBU or entity, modifying ViewportState"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_uuid_arg(verb_call, "cbu-id", ctx);
        let entity_id = get_uuid_arg(verb_call, "entity-id", ctx);
        let enhance_level = get_u8_arg(verb_call, "enhance-level").unwrap_or(0);

        let viewport = ctx.viewport_state_or_default();

        let (focus_type, new_state) = match (cbu_id, entity_id) {
            (Some(cbu), Some(entity)) => {
                // Focus on entity within CBU
                let state = ViewportFocusState::CbuEntity {
                    cbu: CbuRef(cbu),
                    entity: ConcreteEntityRef {
                        id: entity,
                        entity_type: ConcreteEntityType::Company, // TODO: resolve from DB
                    },
                    entity_enhance: enhance_level.min(4),
                    container_enhance: 0,
                };
                ("entity".to_string(), state)
            }
            (Some(cbu), None) => {
                // Focus on CBU container
                let state = ViewportFocusState::CbuContainer {
                    cbu: CbuRef(cbu),
                    enhance_level: enhance_level.min(2),
                };
                ("cbu_container".to_string(), state)
            }
            _ => {
                return Err(anyhow!("viewport.focus requires :cbu-id argument"));
            }
        };

        viewport.focus.set_focus(new_state);

        let result = ViewportFocusResult {
            focus_type,
            cbu_id,
            entity_id,
            enhance_level,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.focus requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.ENHANCE - Change enhance level
// ============================================================================

/// Change the enhance level for current focus
///
/// ## DSL Examples
/// ```dsl
/// (viewport.enhance :direction "increment")
/// (viewport.enhance :direction "decrement")
/// (viewport.enhance :level 3)
/// (viewport.enhance :direction "max")
/// ```
pub struct ViewportEnhanceOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportEnhanceResult {
    pub previous_level: u8,
    pub new_level: u8,
    pub max_level: u8,
}

#[async_trait]
impl CustomOperation for ViewportEnhanceOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "enhance"
    }

    fn rationale(&self) -> &'static str {
        "Changes enhance level for progressive disclosure"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let direction = get_string_arg(verb_call, "direction");
        let explicit_level = get_u8_arg(verb_call, "level");

        let viewport = ctx.viewport_state_or_default();
        let current_state = viewport.focus.current().clone();
        let max_level = current_state.max_enhance_level();
        let previous_level = current_state.primary_enhance_level();

        let enhance_arg = match (direction.as_deref(), explicit_level) {
            (_, Some(level)) => EnhanceArg::Level(level),
            (Some("increment"), _) => EnhanceArg::Increment,
            (Some("decrement"), _) => EnhanceArg::Decrement,
            (Some("max"), _) => EnhanceArg::Max,
            (Some("reset"), _) => EnhanceArg::Reset,
            _ => EnhanceArg::Increment, // Default to increment
        };

        let new_level = enhance_arg.apply(previous_level, max_level);

        // Update the enhance level in the current state
        let new_state = match current_state {
            ViewportFocusState::None => ViewportFocusState::None,
            ViewportFocusState::CbuContainer { cbu, .. } => ViewportFocusState::CbuContainer {
                cbu,
                enhance_level: new_level,
            },
            ViewportFocusState::CbuEntity {
                cbu,
                entity,
                container_enhance,
                ..
            } => ViewportFocusState::CbuEntity {
                cbu,
                entity,
                entity_enhance: new_level,
                container_enhance,
            },
            ViewportFocusState::CbuProductService {
                cbu,
                target,
                container_enhance,
                ..
            } => ViewportFocusState::CbuProductService {
                cbu,
                target,
                target_enhance: new_level,
                container_enhance,
            },
            ViewportFocusState::InstrumentMatrix {
                cbu,
                matrix,
                container_enhance,
                ..
            } => ViewportFocusState::InstrumentMatrix {
                cbu,
                matrix,
                matrix_enhance: new_level,
                container_enhance,
            },
            ViewportFocusState::InstrumentType {
                cbu,
                matrix,
                instrument_type,
                matrix_enhance,
                container_enhance,
                ..
            } => ViewportFocusState::InstrumentType {
                cbu,
                matrix,
                instrument_type,
                type_enhance: new_level,
                matrix_enhance,
                container_enhance,
            },
            ViewportFocusState::ConfigNode {
                cbu,
                matrix,
                instrument_type,
                config_node,
                type_enhance,
                matrix_enhance,
                container_enhance,
                ..
            } => ViewportFocusState::ConfigNode {
                cbu,
                matrix,
                instrument_type,
                config_node,
                node_enhance: new_level,
                type_enhance,
                matrix_enhance,
                container_enhance,
            },
        };

        viewport.focus.state = new_state;

        let result = ViewportEnhanceResult {
            previous_level,
            new_level,
            max_level,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.enhance requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.ASCEND - Navigate up the focus stack
// ============================================================================

/// Navigate up the focus stack
///
/// ## DSL Examples
/// ```dsl
/// (viewport.ascend)
/// (viewport.ascend :to-root true)
/// ```
pub struct ViewportAscendOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportAscendResult {
    pub previous_depth: usize,
    pub new_depth: usize,
    pub ascended: bool,
}

#[async_trait]
impl CustomOperation for ViewportAscendOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "ascend"
    }

    fn rationale(&self) -> &'static str {
        "Navigates up the focus stack for Esper-style drill-out"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let to_root = get_bool_arg(verb_call, "to-root").unwrap_or(false);

        let viewport = ctx.viewport_state_or_default();
        let previous_depth = viewport.focus.stack_depth();

        let ascended = if to_root {
            viewport.focus.ascend_to_root();
            true
        } else {
            viewport.focus.ascend().is_some()
        };

        let new_depth = viewport.focus.stack_depth();

        let result = ViewportAscendResult {
            previous_depth,
            new_depth,
            ascended,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.ascend requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.DESCEND - Navigate into a nested element
// ============================================================================

/// Navigate into a nested element (drill-in)
///
/// ## DSL Examples
/// ```dsl
/// (viewport.descend :target-id @entity)
/// (viewport.descend :target-type "instrument-matrix")
/// ```
pub struct ViewportDescendOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportDescendResult {
    pub previous_depth: usize,
    pub new_depth: usize,
    pub target_type: String,
}

#[async_trait]
impl CustomOperation for ViewportDescendOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "descend"
    }

    fn rationale(&self) -> &'static str {
        "Navigates into nested element for Esper-style drill-in"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let target_id = get_uuid_arg(verb_call, "target-id", ctx);
        let target_type =
            get_string_arg(verb_call, "target-type").unwrap_or_else(|| "entity".to_string());

        let viewport = ctx.viewport_state_or_default();
        let previous_depth = viewport.focus.stack_depth();

        // Get current CBU from focus state
        let current_cbu = viewport.focus.current().cbu().cloned();

        if let Some(cbu) = current_cbu {
            let new_state = match target_type.as_str() {
                "entity" => {
                    if let Some(entity_id) = target_id {
                        ViewportFocusState::CbuEntity {
                            cbu,
                            entity: ConcreteEntityRef {
                                id: entity_id,
                                entity_type: ConcreteEntityType::Company,
                            },
                            entity_enhance: 0,
                            container_enhance: 0,
                        }
                    } else {
                        return Err(anyhow!("viewport.descend to entity requires :target-id"));
                    }
                }
                "instrument-matrix" => ViewportFocusState::InstrumentMatrix {
                    cbu,
                    matrix: ob_poc_types::viewport::InstrumentMatrixRef(
                        target_id.unwrap_or_else(Uuid::nil),
                    ),
                    matrix_enhance: 0,
                    container_enhance: 0,
                },
                _ => {
                    return Err(anyhow!("Unknown target-type: {}", target_type));
                }
            };

            viewport.focus.descend(new_state);
        } else {
            return Err(anyhow!("Cannot descend without a CBU focus"));
        }

        let new_depth = viewport.focus.stack_depth();

        let result = ViewportDescendResult {
            previous_depth,
            new_depth,
            target_type,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.descend requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.CAMERA - Set camera position and zoom
// ============================================================================

/// Set camera position and zoom
///
/// ## DSL Examples
/// ```dsl
/// (viewport.camera :x 100 :y 200 :zoom 1.5)
/// (viewport.camera :zoom 2.0)
/// (viewport.camera :reset true)
/// ```
pub struct ViewportCameraOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportCameraResult {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[async_trait]
impl CustomOperation for ViewportCameraOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "camera"
    }

    fn rationale(&self) -> &'static str {
        "Sets camera position and zoom level"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let reset = get_bool_arg(verb_call, "reset").unwrap_or(false);

        let viewport = ctx.viewport_state_or_default();

        if reset {
            viewport.camera = CameraState::default();
        } else {
            if let Some(x) = get_f32_arg(verb_call, "x") {
                viewport.camera.x = x;
            }
            if let Some(y) = get_f32_arg(verb_call, "y") {
                viewport.camera.y = y;
            }
            if let Some(zoom) = get_f32_arg(verb_call, "zoom") {
                viewport.camera.zoom = zoom.clamp(0.1, 10.0);
            }
        }

        let result = ViewportCameraResult {
            x: viewport.camera.x,
            y: viewport.camera.y,
            zoom: viewport.camera.zoom,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.camera requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.FILTER - Set viewport filters
// ============================================================================

/// Set viewport filters for entity visibility
///
/// ## DSL Examples
/// ```dsl
/// (viewport.filter :entity-types ["company" "person"])
/// (viewport.filter :confidence-zone "core")
/// (viewport.filter :search-text "Apex")
/// (viewport.filter :clear true)
/// ```
pub struct ViewportFilterOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportFilterResult {
    pub entity_types: Option<Vec<String>>,
    pub confidence_zone: Option<String>,
    pub search_text: Option<String>,
}

#[async_trait]
impl CustomOperation for ViewportFilterOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "filter"
    }

    fn rationale(&self) -> &'static str {
        "Sets viewport filters for entity visibility"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let clear = get_bool_arg(verb_call, "clear").unwrap_or(false);

        let viewport = ctx.viewport_state_or_default();

        if clear {
            viewport.filters = ViewportFilters::default();
        } else {
            // Entity type filter
            if let Some(types) = get_string_list_arg(verb_call, "entity-types") {
                viewport.filters.entity_types = Some(
                    types
                        .iter()
                        .filter_map(|t| match t.to_lowercase().as_str() {
                            "company" => Some(ConcreteEntityType::Company),
                            "partnership" => Some(ConcreteEntityType::Partnership),
                            "trust" => Some(ConcreteEntityType::Trust),
                            "person" => Some(ConcreteEntityType::Person),
                            _ => None,
                        })
                        .collect(),
                );
            }

            // Confidence zone filter
            if let Some(zone) = get_string_arg(verb_call, "confidence-zone") {
                viewport.filters.confidence_zone = Some(match zone.to_lowercase().as_str() {
                    "core" => ConfidenceZone::Core,
                    "shell" => ConfidenceZone::Shell,
                    "penumbra" => ConfidenceZone::Penumbra,
                    "speculative" => ConfidenceZone::Speculative,
                    _ => ConfidenceZone::Core,
                });
            }

            // Search text filter
            if let Some(search) = get_string_arg(verb_call, "search-text") {
                viewport.filters.search_text = Some(search);
            }
        }

        let result = ViewportFilterResult {
            entity_types: viewport.filters.entity_types.as_ref().map(|types| {
                types
                    .iter()
                    .map(|t| format!("{:?}", t).to_lowercase())
                    .collect()
            }),
            confidence_zone: viewport
                .filters
                .confidence_zone
                .map(|z| format!("{:?}", z).to_lowercase()),
            search_text: viewport.filters.search_text.clone(),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.filter requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.TRACK - Lock camera onto an entity
// ============================================================================

/// Lock camera tracking onto an entity
///
/// ## DSL Examples
/// ```dsl
/// (viewport.track :entity-id @director)
/// (viewport.track :mode "sticky")
/// (viewport.track :mode "proximity" :radius 200)
/// (viewport.track :unlock true)
/// ```
pub struct ViewportTrackOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportTrackResult {
    pub tracking: bool,
    pub mode: String,
    pub entity_id: Option<Uuid>,
}

#[async_trait]
impl CustomOperation for ViewportTrackOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "track"
    }

    fn rationale(&self) -> &'static str {
        "Locks camera tracking onto an entity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_uuid_arg(verb_call, "entity-id", ctx);
        let mode_str = get_string_arg(verb_call, "mode");
        let radius = get_f32_arg(verb_call, "radius").unwrap_or(100.0);
        let unlock = get_bool_arg(verb_call, "unlock").unwrap_or(false);

        let viewport = ctx.viewport_state_or_default();

        if unlock {
            viewport.focus.focus_mode = FocusMode::Manual;
            let result = ViewportTrackResult {
                tracking: false,
                mode: "manual".to_string(),
                entity_id: None,
            };
            return Ok(ExecutionResult::Record(serde_json::to_value(&result)?));
        }

        let focus_mode = match mode_str.as_deref() {
            Some("sticky") => FocusMode::Sticky,
            Some("proximity") => FocusMode::Proximity { radius },
            Some("center-lock") => FocusMode::CenterLock { region_pct: 0.3 },
            Some("manual") => FocusMode::Manual,
            _ => FocusMode::Sticky, // Default
        };

        viewport.focus.focus_mode = focus_mode;

        let result = ViewportTrackResult {
            tracking: focus_mode != FocusMode::Manual,
            mode: format!("{:?}", focus_mode).to_lowercase(),
            entity_id,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.track requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.CLEAR - Clear focus state
// ============================================================================

/// Clear viewport focus state
///
/// ## DSL Examples
/// ```dsl
/// (viewport.clear)
/// (viewport.clear :keep-camera true)
/// ```
pub struct ViewportClearOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportClearResult {
    pub cleared: bool,
    pub kept_camera: bool,
}

#[async_trait]
impl CustomOperation for ViewportClearOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "clear"
    }

    fn rationale(&self) -> &'static str {
        "Clears viewport focus state"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let keep_camera = get_bool_arg(verb_call, "keep-camera").unwrap_or(false);

        let viewport = ctx.viewport_state_or_default();
        let saved_camera = if keep_camera {
            Some(viewport.camera.clone())
        } else {
            None
        };

        // Reset viewport to default
        *viewport = ViewportState::default();

        // Restore camera if requested
        if let Some(camera) = saved_camera {
            viewport.camera = camera;
        }

        let result = ViewportClearResult {
            cleared: true,
            kept_camera: keep_camera,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.clear requires database feature"))
    }
}

// ============================================================================
// VIEWPORT.VIEW-TYPE - Change the view type
// ============================================================================

/// Change the view type (structure, ownership, accounts, etc.)
///
/// ## DSL Examples
/// ```dsl
/// (viewport.view-type :type "ownership")
/// (viewport.view-type :type "compliance")
/// (viewport.view-type :type "instruments")
/// ```
pub struct ViewportViewTypeOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportViewTypeResult {
    pub previous_type: String,
    pub new_type: String,
}

#[async_trait]
impl CustomOperation for ViewportViewTypeOp {
    fn domain(&self) -> &'static str {
        "viewport"
    }

    fn verb(&self) -> &'static str {
        "view-type"
    }

    fn rationale(&self) -> &'static str {
        "Changes the view type for different perspectives"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let type_str = get_string_arg(verb_call, "type")
            .ok_or_else(|| anyhow!("viewport.view-type requires :type argument"))?;

        let viewport = ctx.viewport_state_or_default();
        let previous_type = format!("{:?}", viewport.view_type).to_lowercase();

        viewport.view_type = match type_str.to_lowercase().as_str() {
            "structure" => CbuViewType::Structure,
            "ownership" => CbuViewType::Ownership,
            "accounts" => CbuViewType::Accounts,
            "compliance" => CbuViewType::Compliance,
            "geographic" => CbuViewType::Geographic,
            "temporal" => CbuViewType::Temporal,
            "instruments" => CbuViewType::Instruments,
            _ => return Err(anyhow!("Unknown view type: {}", type_str)),
        };

        let result = ViewportViewTypeResult {
            previous_type,
            new_type: format!("{:?}", viewport.view_type).to_lowercase(),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("viewport.view-type requires database feature"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhance_arg_apply() {
        assert_eq!(EnhanceArg::Increment.apply(0, 4), 1);
        assert_eq!(EnhanceArg::Increment.apply(4, 4), 4);
        assert_eq!(EnhanceArg::Decrement.apply(2, 4), 1);
        assert_eq!(EnhanceArg::Decrement.apply(0, 4), 0);
        assert_eq!(EnhanceArg::Level(3).apply(0, 4), 3);
        assert_eq!(EnhanceArg::Level(10).apply(0, 4), 4);
        assert_eq!(EnhanceArg::Max.apply(0, 4), 4);
        assert_eq!(EnhanceArg::Reset.apply(3, 4), 0);
    }
}
