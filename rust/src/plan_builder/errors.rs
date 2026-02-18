//! Typed errors for the plan builder pipeline.
//!
//! Each error variant maps to an `OrchestratorResponse` discriminant:
//!
//! | Error | Response |
//! |-------|----------|
//! | `ClassificationError` | `OrchestratorResponse::Clarification` |
//! | `AssemblyError` | `OrchestratorResponse::Clarification` (diagnostic) |
//! | `ConstraintError` | `OrchestratorResponse::ConstraintViolation` |

use crate::runbook::response::{
    ClarificationContext, ClarificationRequest, ConstraintViolationDetail, OrchestratorResponse,
};

// ---------------------------------------------------------------------------
// ClassificationError — verb not found or ambiguous
// ---------------------------------------------------------------------------

/// Error returned when verb classification fails.
///
/// Maps to `OrchestratorResponse::Clarification`.
#[derive(Debug, Clone)]
pub struct ClassificationError {
    /// The verb name that failed classification.
    pub verb_name: String,
    /// Human-readable explanation.
    pub reason: String,
    /// Suggested alternatives (if any).
    pub suggestions: Vec<String>,
}

impl ClassificationError {
    pub fn unknown(verb_name: impl Into<String>) -> Self {
        let name = verb_name.into();
        Self {
            reason: format!("Unknown verb: '{}'. Did you mean something else?", name),
            verb_name: name,
            suggestions: vec![],
        }
    }

    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }

    /// Convert to the OrchestratorResponse representation.
    pub fn into_response(self) -> OrchestratorResponse {
        OrchestratorResponse::Clarification(ClarificationRequest {
            question: self.reason,
            missing_fields: vec![],
            context: ClarificationContext {
                verb: Some(self.verb_name),
                is_macro: false,
                extracted_args: std::collections::BTreeMap::new(),
            },
        })
    }
}

// ---------------------------------------------------------------------------
// AssemblyError — step ordering / dependency resolution failure
// ---------------------------------------------------------------------------

/// Error returned when plan assembly (step ordering, dependency detection)
/// fails.
///
/// Maps to `OrchestratorResponse::Clarification` with diagnostic detail.
#[derive(Debug, Clone)]
pub enum AssemblyError {
    /// Circular dependency detected between steps.
    CyclicDependency {
        /// Verbs involved in the cycle.
        cycle: Vec<String>,
    },
    /// A step references a binding that no prior step produces.
    UnresolvedBinding {
        /// The `@binding` name that couldn't be resolved.
        binding: String,
        /// The verb that references this binding.
        referencing_verb: String,
    },
    /// No steps to assemble.
    EmptyPlan,
}

impl std::fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssemblyError::CyclicDependency { cycle } => {
                write!(f, "Circular dependency: {}", cycle.join(" → "))
            }
            AssemblyError::UnresolvedBinding {
                binding,
                referencing_verb,
            } => {
                write!(
                    f,
                    "Step '{}' references binding @{} which no prior step produces",
                    referencing_verb, binding
                )
            }
            AssemblyError::EmptyPlan => write!(f, "No steps to assemble"),
        }
    }
}

impl std::error::Error for AssemblyError {}

impl AssemblyError {
    /// Convert to the OrchestratorResponse representation.
    pub fn into_response(self) -> OrchestratorResponse {
        OrchestratorResponse::Clarification(ClarificationRequest {
            question: format!("Plan assembly error: {}", self),
            missing_fields: vec![],
            context: ClarificationContext {
                verb: None,
                is_macro: false,
                extracted_args: std::collections::BTreeMap::new(),
            },
        })
    }
}

// ---------------------------------------------------------------------------
// ConstraintError — pack constraint violation
// ---------------------------------------------------------------------------

/// Error returned when pack constraints reject expanded verbs.
///
/// Wraps `ConstraintViolationDetail` and maps to
/// `OrchestratorResponse::ConstraintViolation`.
#[derive(Debug, Clone)]
pub struct ConstraintError(pub ConstraintViolationDetail);

impl ConstraintError {
    pub fn into_response(self) -> OrchestratorResponse {
        OrchestratorResponse::ConstraintViolation(self.0)
    }
}

// ---------------------------------------------------------------------------
// PlanBuilderError — unified error type
// ---------------------------------------------------------------------------

/// Unified error type for the plan builder pipeline.
///
/// Each variant maps to a specific `OrchestratorResponse`.
#[derive(Debug, Clone)]
pub enum PlanBuilderError {
    Classification(ClassificationError),
    Assembly(AssemblyError),
    Constraint(ConstraintError),
}

impl PlanBuilderError {
    /// Convert to the `OrchestratorResponse` representation.
    pub fn into_response(self) -> OrchestratorResponse {
        match self {
            PlanBuilderError::Classification(e) => e.into_response(),
            PlanBuilderError::Assembly(e) => e.into_response(),
            PlanBuilderError::Constraint(e) => e.into_response(),
        }
    }
}

impl From<ClassificationError> for PlanBuilderError {
    fn from(e: ClassificationError) -> Self {
        PlanBuilderError::Classification(e)
    }
}

impl From<AssemblyError> for PlanBuilderError {
    fn from(e: AssemblyError) -> Self {
        PlanBuilderError::Assembly(e)
    }
}

impl From<ConstraintError> for PlanBuilderError {
    fn from(e: ConstraintError) -> Self {
        PlanBuilderError::Constraint(e)
    }
}

impl From<ConstraintViolationDetail> for PlanBuilderError {
    fn from(detail: ConstraintViolationDetail) -> Self {
        PlanBuilderError::Constraint(ConstraintError(detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_error_into_response() {
        let err = ClassificationError::unknown("foo.bar");
        let resp = err.into_response();
        assert!(matches!(resp, OrchestratorResponse::Clarification(_)));
        if let OrchestratorResponse::Clarification(req) = resp {
            assert!(req.question.contains("foo.bar"));
            assert_eq!(req.context.verb, Some("foo.bar".to_string()));
        }
    }

    #[test]
    fn test_assembly_error_cyclic() {
        let err = AssemblyError::CyclicDependency {
            cycle: vec!["a.create".into(), "b.link".into(), "a.create".into()],
        };
        let resp = err.into_response();
        assert!(matches!(resp, OrchestratorResponse::Clarification(_)));
        if let OrchestratorResponse::Clarification(req) = resp {
            assert!(req.question.contains("Circular dependency"));
        }
    }

    #[test]
    fn test_plan_builder_error_from_assembly() {
        let err: PlanBuilderError = AssemblyError::EmptyPlan.into();
        assert!(matches!(err, PlanBuilderError::Assembly(_)));
        let resp = err.into_response();
        assert!(matches!(resp, OrchestratorResponse::Clarification(_)));
    }
}
