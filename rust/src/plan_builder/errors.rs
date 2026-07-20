//! Typed errors for the plan builder pipeline.
//!
//! Each error variant maps to an `OrchestratorResponse` discriminant:
//!
//! | Error | Response |
//! |-------|----------|
//! | `ClassificationError` | `OrchestratorResponse::Clarification` |
//! | `AssemblyError` | `OrchestratorResponse::Clarification` (diagnostic) |


use crate::runbook::response::{
    ClarificationContext, ClarificationRequest, OrchestratorResponse,
};

// ---------------------------------------------------------------------------
// ClassificationError — verb not found or ambiguous
// ---------------------------------------------------------------------------

/// Error returned when verb classification fails.
///
/// Maps to `OrchestratorResponse::Clarification`.
#[derive(Debug, Clone)]
#[allow(dead_code)] // kept for tests
pub(crate) struct ClassificationError {
    /// The verb name that failed classification.
    pub verb_name: String,
    /// Human-readable explanation.
    pub reason: String,
    /// Suggested alternatives (if any).
    pub suggestions: Vec<String>,
}

impl ClassificationError {
    #[allow(dead_code)] // kept for tests
    pub(crate) fn unknown(verb_name: impl Into<String>) -> Self {
        let name = verb_name.into();
        Self {
            reason: format!("Unknown verb: '{}'. Did you mean something else?", name),
            verb_name: name,
            suggestions: vec![],
        }
    }

    #[allow(dead_code)] // kept for tests
    pub(crate) fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }

    /// Convert to the OrchestratorResponse representation.
    #[allow(dead_code)] // kept for tests
    pub(crate) fn into_response(self) -> OrchestratorResponse {
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
pub(crate) enum AssemblyError {
    /// Circular dependency detected between steps.
    CyclicDependency {
        /// Verbs involved in the cycle.
        cycle: Vec<String>,
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
            AssemblyError::EmptyPlan => write!(f, "No steps to assemble"),
        }
    }
}

impl std::error::Error for AssemblyError {}

impl AssemblyError {
    /// Convert to the OrchestratorResponse representation.
    #[allow(dead_code)] // kept for tests
    pub(crate) fn into_response(self) -> OrchestratorResponse {
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
// PlanBuilderError — unified error type
// ---------------------------------------------------------------------------

/// Unified error type for the plan builder pipeline.
///
/// Each variant maps to a specific `OrchestratorResponse`.
#[derive(Debug, Clone)]
pub(crate) enum PlanBuilderError {
    Classification(ClassificationError),
    Assembly(AssemblyError),
}

impl PlanBuilderError {
    /// Convert to the `OrchestratorResponse` representation.
    #[allow(dead_code)] // kept for tests
    pub(crate) fn into_response(self) -> OrchestratorResponse {
        match self {
            PlanBuilderError::Classification(e) => e.into_response(),
            PlanBuilderError::Assembly(e) => e.into_response(),
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
