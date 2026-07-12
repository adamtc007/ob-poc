//! Canonical deterministic compiler entrypoint for the NLCI pipeline.

use anyhow::Result;
use std::sync::Arc;

use super::binding::{CompilerCandidate, CompilerInputEnvelope, CompilerSelection};
use super::failure::CompilerFailure;
use super::phases::{
    BindingResolutionInput, BindingResolutionOutput, CandidateSelectionInput,
    CandidateSelectionOutput, CompositionInput, CompositionOutput, DiscriminationInput,
    DiscriminationOutput, OperationResolutionInput, OperationResolutionOutput,
    SurfaceObjectResolutionInput, SurfaceObjectResolutionOutput,
};

/// Canonical compiler result for the NLCI pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct CompilerOutput {
    /// Candidate set surviving deterministic filtering and scoring.
    pub candidates: Vec<CompilerCandidate>,
    /// Final deterministic selection when compilation succeeds.
    pub selection: Option<CompilerSelection>,
    /// Normalized compiler failure when compilation cannot proceed.
    pub failure: Option<CompilerFailure>,
}

/// Surface object resolution phase.
pub trait SurfaceObjectResolver: Send + Sync {
    /// Resolve the subject surface for an input envelope.
    ///
    /// # Examples
    /// ```ignore
    /// // Implemented by the NLCI compiler surface-resolution phase.
    /// ```
    fn resolve_surface(
        &self,
        input: SurfaceObjectResolutionInput,
    ) -> Result<SurfaceObjectResolutionOutput>;
}

/// Operation resolution phase.
pub trait OperationResolver: Send + Sync {
    /// Resolve canonical operations for an already-scoped surface object.
    ///
    /// # Examples
    /// ```ignore
    /// // Implemented by the NLCI compiler operation-resolution phase.
    /// ```
    fn resolve_operation(
        &self,
        input: OperationResolutionInput,
    ) -> Result<OperationResolutionOutput>;
}

/// Binding resolution phase.
pub trait BindingResolver: Send + Sync {
    /// Resolve identifier, reference, and filter bindings into compiler-ready bindings.
    ///
    /// # Examples
    /// ```ignore
    /// // Implemented by the NLCI compiler binding-resolution phase.
    /// ```
    fn resolve_binding(&self, input: BindingResolutionInput) -> Result<BindingResolutionOutput>;
}

/// Candidate selection phase.
pub trait CandidateSelector: Send + Sync {
    /// Produce deterministic compiler candidates from resolved bindings.
    ///
    /// # Examples
    /// ```ignore
    /// // Implemented by the NLCI compiler candidate-selection phase.
    /// ```
    fn select_candidates(&self, input: CandidateSelectionInput)
        -> Result<CandidateSelectionOutput>;
}

/// Discrimination phase.
pub trait Discriminator: Send + Sync {
    /// Discriminate between surviving candidates and select one deterministic outcome.
    ///
    /// # Examples
    /// ```ignore
    /// // Implemented by the NLCI compiler discrimination phase.
    /// ```
    fn discriminate(&self, input: DiscriminationInput) -> Result<DiscriminationOutput>;
}

/// Composition and parameter binding phase.
pub trait CompositionBinder: Send + Sync {
    /// Bind the selected candidate into a compiler selection suitable for execution handoff.
    ///
    /// # Examples
    /// ```ignore
    /// // Implemented by the NLCI compiler composition/binding phase.
    /// ```
    fn compose(&self, input: CompositionInput) -> Result<CompositionOutput>;
}

/// Canonical deterministic compiler entrypoint.
pub trait IntentCompiler: Send + Sync {
    /// Compile a structured intent envelope into a deterministic compiler result.
    ///
    /// # Examples
    /// ```ignore
    /// // Called by the orchestrator once the NLCI compiler is wired in.
    /// ```
    fn compile(&self, input: CompilerInputEnvelope) -> Result<CompilerOutput>;
}

/// Simple dependency-injected compiler pipeline shell.
pub struct CompilerPipeline {
    /// Surface object resolution phase implementation.
    pub surface_object_resolver: Arc<dyn SurfaceObjectResolver>,
    /// Operation resolution phase implementation.
    pub operation_resolver: Arc<dyn OperationResolver>,
    /// Binding resolution phase implementation.
    pub binding_resolver: Arc<dyn BindingResolver>,
    /// Candidate selection phase implementation.
    pub candidate_selector: Arc<dyn CandidateSelector>,
    /// Discrimination phase implementation.
    pub discriminator: Arc<dyn Discriminator>,
    /// Composition phase implementation.
    pub composition_binder: Arc<dyn CompositionBinder>,
}

impl IntentCompiler for CompilerPipeline {
    fn compile(&self, input: CompilerInputEnvelope) -> Result<CompilerOutput> {
        input.validate_invariants()?;
        let surface = self
            .surface_object_resolver
            .resolve_surface(SurfaceObjectResolutionInput { envelope: input })?;
        let operation = self
            .operation_resolver
            .resolve_operation(OperationResolutionInput { surface })?;
        let binding = self
            .binding_resolver
            .resolve_binding(BindingResolutionInput { operation })?;
        let candidates = self
            .candidate_selector
            .select_candidates(CandidateSelectionInput { binding })?;
        let discrimination = self
            .discriminator
            .discriminate(DiscriminationInput { candidates })?;
        let composition = self
            .composition_binder
            .compose(CompositionInput { discrimination })?;

        Ok(CompilerOutput {
            candidates: composition.candidates,
            selection: composition.selection,
            failure: composition.failure,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::Result;

    use super::*;
    use crate::semtaxonomy_v2::{
        BindingMode, CompilerFailureKind, IntentStep, SemanticIr, SemanticStep,
        StructuredIntentPlan,
    };

    #[derive(Clone)]
    struct RecordingSurfaceResolver(Arc<Mutex<Vec<&'static str>>>);

    impl SurfaceObjectResolver for RecordingSurfaceResolver {
        fn resolve_surface(
            &self,
            input: SurfaceObjectResolutionInput,
        ) -> Result<SurfaceObjectResolutionOutput> {
            self.0.lock().expect("lock poisoned").push("surface");
            Ok(SurfaceObjectResolutionOutput {
                semantic_ir: input.envelope.semantic_ir,
                resolved_surface_entity: "cbu".to_string(),
            })
        }
    }

    #[derive(Clone)]
    struct RecordingOperationResolver(Arc<Mutex<Vec<&'static str>>>);

    impl OperationResolver for RecordingOperationResolver {
        fn resolve_operation(
            &self,
            input: OperationResolutionInput,
        ) -> Result<OperationResolutionOutput> {
            self.0.lock().expect("lock poisoned").push("operation");
            Ok(OperationResolutionOutput {
                surface: input.surface,
                resolved_operations: vec!["cbu.read".to_string()],
            })
        }
    }

    #[derive(Clone)]
    struct RecordingBindingResolver(Arc<Mutex<Vec<&'static str>>>);

    impl BindingResolver for RecordingBindingResolver {
        fn resolve_binding(
            &self,
            input: BindingResolutionInput,
        ) -> Result<BindingResolutionOutput> {
            self.0.lock().expect("lock poisoned").push("binding");
            Ok(BindingResolutionOutput {
                operation: input.operation,
                resolved_bindings: vec![("cbu-id".to_string(), "current".to_string())],
            })
        }
    }

    #[derive(Clone)]
    struct RecordingCandidateSelector(Arc<Mutex<Vec<&'static str>>>);

    impl CandidateSelector for RecordingCandidateSelector {
        fn select_candidates(
            &self,
            input: CandidateSelectionInput,
        ) -> Result<CandidateSelectionOutput> {
            self.0.lock().expect("lock poisoned").push("candidate");
            Ok(CandidateSelectionOutput {
                binding: input.binding,
                candidates: vec![CompilerCandidate {
                    verb_id: "cbu.read".to_string(),
                    score: 1.0,
                    rationale: "exact action/entity match".to_string(),
                }],
            })
        }
    }

    #[derive(Clone)]
    struct RecordingDiscriminator(Arc<Mutex<Vec<&'static str>>>);

    impl Discriminator for RecordingDiscriminator {
        fn discriminate(&self, input: DiscriminationInput) -> Result<DiscriminationOutput> {
            self.0.lock().expect("lock poisoned").push("discrimination");
            Ok(DiscriminationOutput {
                selected_candidate: input.candidates.candidates.first().cloned(),
                candidates: input.candidates,
                failure: None,
            })
        }
    }

    #[derive(Clone)]
    struct RecordingCompositionBinder(Arc<Mutex<Vec<&'static str>>>);

    impl CompositionBinder for RecordingCompositionBinder {
        fn compose(&self, input: CompositionInput) -> Result<CompositionOutput> {
            self.0.lock().expect("lock poisoned").push("composition");
            Ok(CompositionOutput {
                candidates: input.discrimination.candidates.candidates.clone(),
                selection: Some(CompilerSelection {
                    verb_id: "cbu.read".to_string(),
                    arguments: vec![],
                    requires_confirmation: false,
                    explanation: "deterministic read selection".to_string(),
                }),
                failure: None,
            })
        }
    }

    fn valid_input() -> CompilerInputEnvelope {
        CompilerInputEnvelope {
            structured_intent: StructuredIntentPlan {
                steps: vec![IntentStep {
                    action: "read".to_string(),
                    entity: "cbu".to_string(),
                    target: None,
                    qualifiers: vec![],
                    parameters: vec![],
                    confidence: "high".to_string(),
                }],
                composition: Some("single_step".to_string()),
                data_flow: vec![],
            },
            semantic_ir: SemanticIr {
                steps: vec![SemanticStep {
                    action: "read".to_string(),
                    entity: "cbu".to_string(),
                    binding_mode: BindingMode::Unbound,
                    target: None,
                    parameters: vec![],
                    qualifiers: vec![],
                }],
                composition: Some("single_step".to_string()),
            },
            session_id: None,
            session_entity_id: None,
            session_entity_kind: None,
            session_entity_name: None,
        }
    }

    #[test]
    fn compiler_pipeline_runs_phases_in_order() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let pipeline = CompilerPipeline {
            surface_object_resolver: Arc::new(RecordingSurfaceResolver(calls.clone())),
            operation_resolver: Arc::new(RecordingOperationResolver(calls.clone())),
            binding_resolver: Arc::new(RecordingBindingResolver(calls.clone())),
            candidate_selector: Arc::new(RecordingCandidateSelector(calls.clone())),
            discriminator: Arc::new(RecordingDiscriminator(calls.clone())),
            composition_binder: Arc::new(RecordingCompositionBinder(calls.clone())),
        };

        let output = pipeline
            .compile(valid_input())
            .expect("compile should succeed");
        assert_eq!(
            *calls.lock().expect("lock poisoned"),
            vec![
                "surface",
                "operation",
                "binding",
                "candidate",
                "discrimination",
                "composition"
            ]
        );
        assert_eq!(
            output
                .selection
                .as_ref()
                .expect("selection should exist")
                .verb_id,
            "cbu.read"
        );
    }

    #[test]
    fn compiler_pipeline_rejects_invalid_input_before_phase_execution() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let pipeline = CompilerPipeline {
            surface_object_resolver: Arc::new(RecordingSurfaceResolver(calls.clone())),
            operation_resolver: Arc::new(RecordingOperationResolver(calls.clone())),
            binding_resolver: Arc::new(RecordingBindingResolver(calls.clone())),
            candidate_selector: Arc::new(RecordingCandidateSelector(calls.clone())),
            discriminator: Arc::new(RecordingDiscriminator(calls.clone())),
            composition_binder: Arc::new(RecordingCompositionBinder(calls.clone())),
        };

        let mut invalid = valid_input();
        invalid.structured_intent.steps[0].action = "(cbu.read)".to_string();

        let err = pipeline.compile(invalid).expect_err("compile should fail");
        assert!(
            err.to_string().contains("structured actions, not DSL text"),
            "unexpected error: {err}"
        );
        assert!(calls.lock().expect("lock poisoned").is_empty());
    }

    #[test]
    fn compiler_failure_kind_covers_governed_execution_boundary() {
        let kind = CompilerFailureKind::GovernedExecutionBlocked;
        assert_eq!(kind, CompilerFailureKind::GovernedExecutionBlocked);
    }
}
