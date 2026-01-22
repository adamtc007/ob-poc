//! Viewport DSL Executor
//!
//! This module bridges DSL AST types from `dsl-core` to the runtime state
//! management in the viewport crate. It executes `ViewportVerb` commands
//! against a `ViewportContext`.
//!
//! ## Architecture
//!
//! ```text
//! ViewportVerb (AST from dsl-core)
//!         │
//!         ▼
//! ViewportExecutor.execute()
//!         │
//!         ├── Resolve references (FocusTarget → runtime refs)
//!         ├── Validate transitions
//!         └── Apply FocusTransition to ViewportContext
//!         │
//!         ▼
//! ViewportContext (mutated state)
//! ```

use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use dsl_core::{
    ConfidenceZone as AstConfidenceZone, EnhanceArg as AstEnhanceArg,
    ExportFormat as AstExportFormat, FocusTarget as AstFocusTarget, NavTarget as AstNavTarget,
    ViewType as AstViewType, ViewportVerb,
};
use ob_poc_types::viewport::{
    CbuRef, ConcreteEntityRef, ConcreteEntityType, ConfidenceZone, ConfigNodeRef, FocusMode,
    InstrumentMatrixRef, InstrumentType, ViewportFocusState,
};

use crate::focus::{apply_transition, FocusTransition};
use crate::state::ViewportContext;
use crate::transitions::{TransitionError, TransitionValidator};

/// Errors that can occur during viewport verb execution
#[derive(Debug, Error, Clone)]
pub enum ExecutorError {
    #[error("Resolution failed: {0}")]
    ResolutionFailed(String),

    #[error("Invalid target: {0}")]
    InvalidTarget(String),

    #[error("Transition error: {0}")]
    TransitionError(#[from] TransitionError),

    #[error("No current CBU context")]
    NoCbuContext,

    #[error("Symbol not found: @{0}")]
    SymbolNotFound(String),

    #[error("Invalid enhance level: {0}")]
    InvalidEnhanceLevel(String),

    #[error("Export not supported in this context")]
    ExportNotSupported,

    #[error("View type not applicable: {0}")]
    ViewTypeNotApplicable(String),
}

/// Result type for executor operations
pub type ExecutorResult<T> = Result<T, ExecutorError>;

/// Resolver trait for converting string references to runtime types
///
/// Implementations of this trait provide the bridge between DSL string
/// references (e.g., "Apex Fund", "entity:uuid") and runtime types
/// (e.g., `CbuRef(Uuid)`, `ConcreteEntityRef`).
pub trait ReferenceResolver {
    /// Resolve a CBU reference string to a CbuRef
    fn resolve_cbu(&self, cbu_ref: &str) -> ExecutorResult<CbuRef>;

    /// Resolve an entity reference string to a ConcreteEntityRef
    fn resolve_entity(&self, entity_ref: &str) -> ExecutorResult<ConcreteEntityRef>;

    /// Resolve a symbol reference to its bound value
    fn resolve_symbol(&self, name: &str) -> ExecutorResult<ResolvedSymbol>;

    /// Resolve a config node reference
    fn resolve_config_node(&self, config_ref: &str) -> ExecutorResult<ConfigNodeRef>;

    /// Resolve an instrument type string
    fn resolve_instrument_type(&self, type_str: &str) -> ExecutorResult<InstrumentType>;

    /// Get the current CBU context (if any)
    fn current_cbu(&self) -> Option<CbuRef>;

    /// Get the instrument matrix for the current CBU
    fn current_matrix(&self) -> Option<InstrumentMatrixRef>;
}

/// Resolved symbol value - what a @symbol reference points to
#[derive(Debug, Clone)]
pub enum ResolvedSymbol {
    Cbu(CbuRef),
    Entity(ConcreteEntityRef),
    Matrix(InstrumentMatrixRef),
    InstrumentType(InstrumentType),
    ConfigNode(ConfigNodeRef),
}

/// Simple in-memory resolver for testing and basic usage
#[derive(Debug, Clone, Default)]
pub struct SimpleResolver {
    /// CBU name → UUID mapping
    pub cbus: HashMap<String, Uuid>,
    /// Entity name → (UUID, type) mapping
    pub entities: HashMap<String, (Uuid, ConcreteEntityType)>,
    /// Symbol name → resolved value mapping
    pub symbols: HashMap<String, ResolvedSymbol>,
    /// Current CBU context
    pub current_cbu: Option<CbuRef>,
    /// Current matrix context
    pub current_matrix: Option<InstrumentMatrixRef>,
}

impl ReferenceResolver for SimpleResolver {
    fn resolve_cbu(&self, cbu_ref: &str) -> ExecutorResult<CbuRef> {
        // Try as UUID first
        if let Ok(uuid) = Uuid::parse_str(cbu_ref) {
            return Ok(CbuRef(uuid));
        }

        // Try as name lookup
        self.cbus
            .get(cbu_ref)
            .map(|id| CbuRef(*id))
            .ok_or_else(|| ExecutorError::ResolutionFailed(format!("CBU not found: {}", cbu_ref)))
    }

    fn resolve_entity(&self, entity_ref: &str) -> ExecutorResult<ConcreteEntityRef> {
        // Try as UUID first - default to Company type when only UUID is known
        if let Ok(uuid) = Uuid::parse_str(entity_ref) {
            return Ok(ConcreteEntityRef {
                id: uuid,
                entity_type: ConcreteEntityType::Company, // Default when type unknown
            });
        }

        // Try as name lookup
        self.entities
            .get(entity_ref)
            .map(|(id, entity_type)| ConcreteEntityRef {
                id: *id,
                entity_type: *entity_type,
            })
            .ok_or_else(|| {
                ExecutorError::ResolutionFailed(format!("Entity not found: {}", entity_ref))
            })
    }

    fn resolve_symbol(&self, name: &str) -> ExecutorResult<ResolvedSymbol> {
        self.symbols
            .get(name)
            .cloned()
            .ok_or_else(|| ExecutorError::SymbolNotFound(name.to_string()))
    }

    fn resolve_config_node(&self, config_ref: &str) -> ExecutorResult<ConfigNodeRef> {
        // Parse config node format: "mic:XNYS" or "bic:CITIUS33" or "pricing:uuid"
        if let Some((prefix, value)) = config_ref.split_once(':') {
            match prefix.to_lowercase().as_str() {
                "mic" => Ok(ConfigNodeRef::Mic {
                    code: value.to_string(),
                }),
                "bic" => Ok(ConfigNodeRef::Bic {
                    code: value.to_string(),
                }),
                "pricing" => {
                    let uuid = Uuid::parse_str(value).map_err(|_| {
                        ExecutorError::InvalidTarget(format!("Invalid pricing UUID: {}", value))
                    })?;
                    Ok(ConfigNodeRef::Pricing { id: uuid })
                }
                "restrictions" => {
                    let uuid = Uuid::parse_str(value).map_err(|_| {
                        ExecutorError::InvalidTarget(format!(
                            "Invalid restrictions UUID: {}",
                            value
                        ))
                    })?;
                    Ok(ConfigNodeRef::Restrictions { id: uuid })
                }
                _ => Err(ExecutorError::InvalidTarget(format!(
                    "Unknown config type: {}",
                    prefix
                ))),
            }
        } else {
            Err(ExecutorError::InvalidTarget(format!(
                "Invalid config node format: {}",
                config_ref
            )))
        }
    }

    fn resolve_instrument_type(&self, type_str: &str) -> ExecutorResult<InstrumentType> {
        match type_str.to_uppercase().as_str() {
            "EQUITY" | "EQUITIES" => Ok(InstrumentType::Equity),
            "FIXED_INCOME" | "FIXEDINCOME" | "BONDS" | "BOND" => Ok(InstrumentType::FixedIncome),
            "DERIVATIVES" | "DERIVATIVE" => Ok(InstrumentType::Derivative),
            "FX" | "FOREX" => Ok(InstrumentType::Fx),
            "COMMODITIES" | "COMMODITY" => Ok(InstrumentType::Commodity),
            "FUND" | "FUNDS" => Ok(InstrumentType::Fund),
            "CASH" => Ok(InstrumentType::Cash),
            "STRUCTURED" | "STRUCTURED_PRODUCT" => Ok(InstrumentType::StructuredProduct),
            _ => Err(ExecutorError::InvalidTarget(format!(
                "Unknown instrument type: {}",
                type_str
            ))),
        }
    }

    fn current_cbu(&self) -> Option<CbuRef> {
        self.current_cbu.clone()
    }

    fn current_matrix(&self) -> Option<InstrumentMatrixRef> {
        self.current_matrix.clone()
    }
}

/// Viewport DSL Executor
///
/// Executes viewport verb commands against a viewport context,
/// translating AST types to runtime transitions.
pub struct ViewportExecutor<R: ReferenceResolver> {
    resolver: R,
}

impl<R: ReferenceResolver> ViewportExecutor<R> {
    /// Create a new executor with the given resolver
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }

    /// Execute a viewport verb against the given context
    pub fn execute(
        &self,
        verb: &ViewportVerb,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        match verb {
            ViewportVerb::Focus { target, .. } => self.execute_focus(target, ctx),
            ViewportVerb::Enhance { arg, .. } => self.execute_enhance(arg, ctx),
            ViewportVerb::Navigate { target, .. } => self.execute_navigate(target, ctx),
            ViewportVerb::Ascend { .. } => self.execute_ascend(ctx),
            ViewportVerb::Descend { target, .. } => self.execute_descend(target, ctx),
            ViewportVerb::View { view_type, .. } => self.execute_view(view_type, ctx),
            ViewportVerb::Fit { zone, .. } => self.execute_fit(zone.as_ref(), ctx),
            ViewportVerb::Export { format, .. } => self.execute_export(format, ctx),
        }
    }

    /// Execute a focus command
    fn execute_focus(
        &self,
        target: &AstFocusTarget,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        let transition = self.focus_target_to_transition(target, ctx)?;
        apply_transition(ctx.focus_manager_mut(), transition);
        ctx.record_command(format!("focus {:?}", target));
        Ok(ExecutionOutcome::FocusChanged)
    }

    /// Execute an enhance command
    fn execute_enhance(
        &self,
        arg: &AstEnhanceArg,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        let transition = match arg {
            AstEnhanceArg::Plus => FocusTransition::Enhance { delta: 1 },
            AstEnhanceArg::Minus => FocusTransition::Enhance { delta: -1 },
            AstEnhanceArg::Level(n) => FocusTransition::EnhanceSet { level: *n },
            AstEnhanceArg::Max => FocusTransition::EnhanceMax,
            AstEnhanceArg::Reset => FocusTransition::EnhanceReset,
        };

        apply_transition(ctx.focus_manager_mut(), transition);
        ctx.record_command(format!("enhance {:?}", arg));
        Ok(ExecutionOutcome::EnhanceChanged)
    }

    /// Execute a navigate command
    fn execute_navigate(
        &self,
        target: &AstNavTarget,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        // Navigation changes camera/view position without changing focus
        match target {
            AstNavTarget::Entity { entity_ref, .. } => {
                let _entity = self.resolver.resolve_entity(entity_ref)?;
                // In a real implementation, this would pan the camera to the entity
                ctx.record_command(format!("navigate to entity {}", entity_ref));
            }
            AstNavTarget::Direction { direction, .. } => {
                ctx.record_command(format!("navigate {:?}", direction));
            }
            AstNavTarget::Symbol { name, .. } => {
                let resolved = self.resolver.resolve_symbol(name)?;
                ctx.record_command(format!("navigate to @{} ({:?})", name, resolved));
            }
        }
        Ok(ExecutionOutcome::Navigated)
    }

    /// Execute an ascend command
    fn execute_ascend(&self, ctx: &mut ViewportContext) -> ExecutorResult<ExecutionOutcome> {
        // Validate we can ascend
        TransitionValidator::can_ascend(ctx.focus_depth())?;

        apply_transition(ctx.focus_manager_mut(), FocusTransition::Ascend);
        ctx.record_command("ascend");
        Ok(ExecutionOutcome::FocusChanged)
    }

    /// Execute a descend command
    fn execute_descend(
        &self,
        target: &AstFocusTarget,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        // Validate we can descend
        TransitionValidator::can_descend(ctx.focus())?;

        let child_transition = self.focus_target_to_transition(target, ctx)?;
        let transition = FocusTransition::Descend(Box::new(child_transition));

        apply_transition(ctx.focus_manager_mut(), transition);
        ctx.record_command(format!("descend {:?}", target));
        Ok(ExecutionOutcome::FocusChanged)
    }

    /// Execute a view command
    fn execute_view(
        &self,
        view_type: &AstViewType,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        // Update focus mode based on view type
        // Different views benefit from different focus behaviors
        let mode = match view_type {
            AstViewType::Structure => FocusMode::Sticky, // Keep focus when panning
            AstViewType::Ownership => FocusMode::Sticky, // Keep focus for UBO tracing
            AstViewType::Accounts => FocusMode::Manual,  // Explicit focus changes
            AstViewType::Instruments => FocusMode::Manual, // Explicit focus for matrix
            AstViewType::Compliance => FocusMode::Sticky, // Keep focus on issues
            AstViewType::Geographic => FocusMode::Proximity { radius: 100.0 }, // Transfer to nearest
            AstViewType::Temporal => FocusMode::Manual, // Explicit for timeline
        };

        ctx.focus_manager_mut().focus_mode = mode;
        ctx.record_command(format!("view {:?}", view_type));
        Ok(ExecutionOutcome::ViewChanged)
    }

    /// Execute a fit command
    fn execute_fit(
        &self,
        zone: Option<&AstConfidenceZone>,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        // Convert AST confidence zone to runtime type
        // AST and runtime share the same names: Core, Shell, Penumbra, All/Speculative
        let runtime_zone = zone.map(|z| match z {
            AstConfidenceZone::Core => ConfidenceZone::Core,
            AstConfidenceZone::Shell => ConfidenceZone::Shell,
            AstConfidenceZone::Penumbra => ConfidenceZone::Penumbra,
            // AST "All" means show everything - map to Speculative (lowest threshold)
            AstConfidenceZone::All => ConfidenceZone::Speculative,
        });

        // Reset camera zoom to fit content
        ctx.camera_mut().zoom = 1.0;
        ctx.camera_mut().x = 0.0;
        ctx.camera_mut().y = 0.0;

        ctx.record_command(format!("fit {:?}", runtime_zone));
        Ok(ExecutionOutcome::CameraChanged)
    }

    /// Execute an export command
    fn execute_export(
        &self,
        format: &AstExportFormat,
        ctx: &mut ViewportContext,
    ) -> ExecutorResult<ExecutionOutcome> {
        // Export is a query operation - doesn't mutate state
        ctx.record_command(format!("export {:?}", format));
        Ok(ExecutionOutcome::ExportRequested { format: *format })
    }

    /// Convert a FocusTarget AST node to a FocusTransition
    fn focus_target_to_transition(
        &self,
        target: &AstFocusTarget,
        ctx: &ViewportContext,
    ) -> ExecutorResult<FocusTransition> {
        match target {
            AstFocusTarget::Cbu { cbu_ref, .. } => {
                let cbu = self.resolver.resolve_cbu(cbu_ref)?;
                Ok(FocusTransition::FocusCbu {
                    cbu,
                    enhance_level: 0,
                })
            }

            AstFocusTarget::Entity { entity_ref, .. } => {
                let entity = self.resolver.resolve_entity(entity_ref)?;
                let cbu = self.require_cbu_context(ctx)?;
                Ok(FocusTransition::FocusEntity {
                    cbu,
                    entity,
                    entity_enhance: 0,
                    container_enhance: ctx.current_enhance_level(),
                })
            }

            AstFocusTarget::Member { member_ref, .. } => {
                // Member is treated as an entity focus
                let entity = self.resolver.resolve_entity(member_ref)?;
                let cbu = self.require_cbu_context(ctx)?;
                Ok(FocusTransition::FocusEntity {
                    cbu,
                    entity,
                    entity_enhance: 0,
                    container_enhance: ctx.current_enhance_level(),
                })
            }

            AstFocusTarget::Edge { edge_ref, .. } => {
                // Edge focus - for now, resolve as entity on one end
                // TODO: Proper edge resolution
                let entity = self.resolver.resolve_entity(edge_ref)?;
                let cbu = self.require_cbu_context(ctx)?;
                Ok(FocusTransition::FocusEntity {
                    cbu,
                    entity,
                    entity_enhance: 0,
                    container_enhance: ctx.current_enhance_level(),
                })
            }

            AstFocusTarget::Matrix { .. } => {
                let cbu = self.require_cbu_context(ctx)?;
                let matrix = self
                    .resolver
                    .current_matrix()
                    .unwrap_or_else(|| InstrumentMatrixRef(Uuid::new_v4()));
                Ok(FocusTransition::FocusMatrix {
                    cbu,
                    matrix,
                    matrix_enhance: 0,
                    container_enhance: ctx.current_enhance_level(),
                })
            }

            AstFocusTarget::InstrumentType {
                instrument_type, ..
            } => {
                let cbu = self.require_cbu_context(ctx)?;
                let matrix = self
                    .resolver
                    .current_matrix()
                    .ok_or(ExecutorError::InvalidTarget(
                        "No matrix context for instrument type focus".to_string(),
                    ))?;
                let inst_type = self.resolver.resolve_instrument_type(instrument_type)?;
                Ok(FocusTransition::FocusInstrumentType {
                    cbu,
                    matrix,
                    instrument_type: inst_type,
                    type_enhance: 0,
                    matrix_enhance: ctx.current_enhance_level(),
                    container_enhance: 0,
                })
            }

            AstFocusTarget::Config { config_node, .. } => {
                let cbu = self.require_cbu_context(ctx)?;
                let config = self.resolver.resolve_config_node(config_node)?;

                // Need matrix and instrument type context
                let (matrix, inst_type) = self.require_instrument_context(ctx)?;

                Ok(FocusTransition::FocusConfigNode {
                    cbu,
                    matrix,
                    instrument_type: inst_type,
                    config_node: config,
                    node_enhance: 0,
                    type_enhance: 0,
                    matrix_enhance: 0,
                    container_enhance: ctx.current_enhance_level(),
                })
            }

            AstFocusTarget::Symbol { name, .. } => {
                let resolved = self.resolver.resolve_symbol(name)?;
                match resolved {
                    ResolvedSymbol::Cbu(cbu) => Ok(FocusTransition::FocusCbu {
                        cbu,
                        enhance_level: 0,
                    }),
                    ResolvedSymbol::Entity(entity) => {
                        let cbu = self.require_cbu_context(ctx)?;
                        Ok(FocusTransition::FocusEntity {
                            cbu,
                            entity,
                            entity_enhance: 0,
                            container_enhance: ctx.current_enhance_level(),
                        })
                    }
                    ResolvedSymbol::Matrix(matrix) => {
                        let cbu = self.require_cbu_context(ctx)?;
                        Ok(FocusTransition::FocusMatrix {
                            cbu,
                            matrix,
                            matrix_enhance: 0,
                            container_enhance: ctx.current_enhance_level(),
                        })
                    }
                    ResolvedSymbol::InstrumentType(inst_type) => {
                        let cbu = self.require_cbu_context(ctx)?;
                        let matrix = self.resolver.current_matrix().ok_or_else(|| {
                            ExecutorError::InvalidTarget(
                                "No matrix context for instrument type symbol".to_string(),
                            )
                        })?;
                        Ok(FocusTransition::FocusInstrumentType {
                            cbu,
                            matrix,
                            instrument_type: inst_type,
                            type_enhance: 0,
                            matrix_enhance: 0,
                            container_enhance: ctx.current_enhance_level(),
                        })
                    }
                    ResolvedSymbol::ConfigNode(config) => {
                        let cbu = self.require_cbu_context(ctx)?;
                        let (matrix, inst_type) = self.require_instrument_context(ctx)?;
                        Ok(FocusTransition::FocusConfigNode {
                            cbu,
                            matrix,
                            instrument_type: inst_type,
                            config_node: config,
                            node_enhance: 0,
                            type_enhance: 0,
                            matrix_enhance: 0,
                            container_enhance: ctx.current_enhance_level(),
                        })
                    }
                }
            }
        }
    }

    /// Require a CBU context, returning error if not present
    fn require_cbu_context(&self, ctx: &ViewportContext) -> ExecutorResult<CbuRef> {
        ctx.current_cbu()
            .cloned()
            .or_else(|| self.resolver.current_cbu())
            .ok_or(ExecutorError::NoCbuContext)
    }

    /// Require instrument matrix context
    fn require_instrument_context(
        &self,
        ctx: &ViewportContext,
    ) -> ExecutorResult<(InstrumentMatrixRef, InstrumentType)> {
        match ctx.focus() {
            ViewportFocusState::InstrumentType {
                matrix,
                instrument_type,
                ..
            } => Ok((matrix.clone(), *instrument_type)),
            ViewportFocusState::ConfigNode {
                matrix,
                instrument_type,
                ..
            } => Ok((matrix.clone(), *instrument_type)),
            _ => Err(ExecutorError::InvalidTarget(
                "No instrument type context".to_string(),
            )),
        }
    }
}

/// Outcome of executing a viewport verb
#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    /// Focus was changed
    FocusChanged,
    /// Enhance level was changed
    EnhanceChanged,
    /// Camera/navigation was changed
    Navigated,
    /// Camera was changed (fit/zoom)
    CameraChanged,
    /// View type/mode was changed
    ViewChanged,
    /// Export was requested
    ExportRequested { format: AstExportFormat },
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::Span;

    fn simple_resolver() -> SimpleResolver {
        let mut resolver = SimpleResolver::default();
        let cbu_id = Uuid::new_v4();
        resolver.cbus.insert("Apex Fund".to_string(), cbu_id);
        resolver.current_cbu = Some(CbuRef(cbu_id));
        resolver.current_matrix = Some(InstrumentMatrixRef(Uuid::new_v4()));

        let entity_id = Uuid::new_v4();
        resolver.entities.insert(
            "John Smith".to_string(),
            (entity_id, ConcreteEntityType::Person),
        );

        resolver
            .symbols
            .insert("fund".to_string(), ResolvedSymbol::Cbu(CbuRef(cbu_id)));

        resolver
    }

    #[test]
    fn test_execute_focus_cbu() {
        let resolver = simple_resolver();
        let executor = ViewportExecutor::new(resolver);
        let mut ctx = ViewportContext::new();

        let verb = ViewportVerb::Focus {
            target: AstFocusTarget::Cbu {
                cbu_ref: "Apex Fund".to_string(),
                span: Span::new(0, 10),
            },
            span: Span::new(0, 20),
        };

        let result = executor.execute(&verb, &mut ctx);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), ExecutionOutcome::FocusChanged));
        assert!(matches!(
            ctx.focus(),
            ViewportFocusState::CbuContainer { .. }
        ));
    }

    #[test]
    fn test_execute_enhance_plus() {
        let resolver = simple_resolver();
        let executor = ViewportExecutor::new(resolver);
        let mut ctx = ViewportContext::new();

        // First focus on a CBU
        let focus_verb = ViewportVerb::Focus {
            target: AstFocusTarget::Cbu {
                cbu_ref: "Apex Fund".to_string(),
                span: Span::new(0, 10),
            },
            span: Span::new(0, 20),
        };
        executor.execute(&focus_verb, &mut ctx).unwrap();

        // Now enhance
        let enhance_verb = ViewportVerb::Enhance {
            arg: AstEnhanceArg::Plus,
            span: Span::new(0, 10),
        };

        let result = executor.execute(&enhance_verb, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(ctx.current_enhance_level(), 1);
    }

    #[test]
    fn test_execute_ascend_fails_without_stack() {
        let resolver = simple_resolver();
        let executor = ViewportExecutor::new(resolver);
        let mut ctx = ViewportContext::new();

        let verb = ViewportVerb::Ascend {
            span: Span::new(0, 10),
        };

        let result = executor.execute(&verb, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_focus_symbol() {
        let resolver = simple_resolver();
        let executor = ViewportExecutor::new(resolver);
        let mut ctx = ViewportContext::new();

        let verb = ViewportVerb::Focus {
            target: AstFocusTarget::Symbol {
                name: "fund".to_string(),
                span: Span::new(0, 5),
            },
            span: Span::new(0, 15),
        };

        let result = executor.execute(&verb, &mut ctx);
        assert!(result.is_ok());
        assert!(matches!(
            ctx.focus(),
            ViewportFocusState::CbuContainer { .. }
        ));
    }

    #[test]
    fn test_execute_fit() {
        let resolver = simple_resolver();
        let executor = ViewportExecutor::new(resolver);
        let mut ctx = ViewportContext::new();

        // Modify camera
        ctx.camera_mut().zoom = 2.0;

        let verb = ViewportVerb::Fit {
            zone: None,
            span: Span::new(0, 10),
        };

        let result = executor.execute(&verb, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(ctx.camera().zoom, 1.0);
    }

    #[test]
    fn test_resolve_instrument_type() {
        let resolver = SimpleResolver::default();

        assert!(matches!(
            resolver.resolve_instrument_type("EQUITY"),
            Ok(InstrumentType::Equity)
        ));
        assert!(matches!(
            resolver.resolve_instrument_type("bonds"),
            Ok(InstrumentType::FixedIncome)
        ));
        assert!(matches!(
            resolver.resolve_instrument_type("FX"),
            Ok(InstrumentType::Fx)
        ));
        assert!(resolver.resolve_instrument_type("unknown").is_err());
    }

    #[test]
    fn test_resolve_config_node() {
        let resolver = SimpleResolver::default();

        let mic = resolver.resolve_config_node("mic:XNYS");
        assert!(mic.is_ok());
        assert!(matches!(mic.unwrap(), ConfigNodeRef::Mic { code } if code == "XNYS"));

        let bic = resolver.resolve_config_node("bic:CITIUS33");
        assert!(bic.is_ok());
        assert!(matches!(bic.unwrap(), ConfigNodeRef::Bic { code } if code == "CITIUS33"));
    }
}
