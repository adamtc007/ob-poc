//! DSL Core Module - The Heart of the OB-POC Application
//!
//! This module contains the **core DSL capabilities** that define the entire application.
//! Everything else in the system either feeds into DSL processing, orchestrates DSL workflows,
//! or stores DSL results. This is the semantic center of the Ultimate Beneficial Ownership
//! and comprehensive onboarding system.
//!
//! ## Architecture Philosophy: DSL-as-State + AttributeID-as-Type
//!
//! **DSL-as-State**: The accumulated DSL document IS the state itself
//! - Each onboarding case's current state = complete accumulated DSL document
//! - Immutable event sourcing through DSL append operations
//! - Executable documentation that serves as audit trail
//!
//! **AttributeID-as-Type**: Variables typed by UUID references to universal dictionary
//! - `(verb @attr{uuid-001} @attr{uuid-002} ...)` structure
//! - Type safety through attribute dictionary validation
//! - Cross-system consistency via universal attribute IDs
//!
//! ## Core DSL Capabilities
//!
//! This module provides the essential DSL business logic:
//!
//! ### 1. **Domain-Aware Processing**
//! - Multi-domain DSL coordination (KYC, UBO, Onboarding, Documents, ISDA)
//! - Context switching between business domains
//! - Domain-specific validation and transformation rules
//!
//! ### 2. **DSL Editing & Transformation**
//! - Central DSL editing engine with domain context
//! - DSL normalization (v3.3 legacy → v3.1 canonical)
//! - Operation chaining and workflow composition
//!
//! ### 3. **Vocabulary & Grammar Integration**
//! - Approved verb validation across 7 operational domains
//! - 70+ approved verbs with semantic validation
//! - AttributeID-as-Type enforcement
//!
//! ### 4. **State Management**
//! - DSL document lifecycle management
//! - State transitions through DSL operations
//! - Audit trail preservation
//!
//! ## Public Interface
//!
//! The DSL module exposes only the essential capabilities needed by:
//! - **DSL Manager**: Orchestrates complex multi-step workflows
//! - **Agents**: AI-powered DSL generation and manipulation
//! - **External Consumers**: Direct DSL processing needs
//!
//! ## Example Usage
//!
//! ```rust
//! use ob_poc::dsl::{DslProcessor, DomainContext};
//!
//! let processor = DslProcessor::new();
//! let context = DomainContext::kyc();
//!
//! let result = processor.process_dsl(dsl_content, context).await?;
//! ```

// ============================================================================
// INTERNAL IMPLEMENTATION MODULES - Hidden from External Consumers
// ============================================================================

// Core DSL processing engine
pub(crate) mod central_editor;

// Domain context and routing system
pub(crate) mod domain_context;
pub(crate) mod domain_registry;

// DSL operations and transformations
pub(crate) mod operations;

// Multi-domain parsing coordination
pub(crate) mod parsing_coordinator;

// Domain-specific handlers (internal coordination)
pub(crate) mod domain_handlers;

// DSL validation and normalization (internal)
pub(crate) mod normalization;
pub(crate) mod validation;

// ============================================================================
// PUBLIC FACADE - Core DSL Capabilities for External Consumers
// ============================================================================

// Re-export essential DSL processing capabilities
pub use central_editor::CentralDslEditor;
pub use domain_context::{DomainContext, OperationMetadata, OperationPriority};
pub use domain_registry::{DomainHandler, DomainRegistry, DslVocabulary};
pub use operations::{DslOperation, OperationBuilder, OperationChain};
pub use parsing_coordinator::{DomainDetection, ParseResult, ParsingCoordinator};

// Import types from dsl_types crate (Level 1 foundation)
pub use dsl_types::{
    DomainValidation, DslError, DslResult, ProcessingMetadata, ProcessingResult, PromptConfig,
    SourceLocation, ValidationError, ValidationReport, ValidationWarning, VocabularyValidation,
    WarningSeverity,
};

// Core DSL types and results

/// High-level DSL processor - the main entry point for DSL operations
#[derive(Debug)]
pub struct DslProcessor {
    editor: CentralDslEditor,
    registry: DomainRegistry,
    coordinator: ParsingCoordinator,
}

impl DslProcessor {
    /// Create a new DSL processor with default configuration
    pub fn new() -> Self {
        Self {
            editor: CentralDslEditor::new(),
            registry: DomainRegistry::with_default_domains(),
            coordinator: ParsingCoordinator::new(),
        }
    }

    /// Create DSL processor with custom domain configuration
    pub fn with_domains(domains: Vec<Box<dyn DomainHandler>>) -> Self {
        let mut registry = DomainRegistry::new();
        for domain in domains {
            registry.register_domain(domain);
        }

        Self {
            editor: CentralDslEditor::new(),
            registry,
            coordinator: ParsingCoordinator::new(),
        }
    }

    /// Process DSL content with domain context
    pub async fn process_dsl(
        &self,
        dsl_content: &str,
        context: DomainContext,
    ) -> DslResult<ProcessingResult> {
        // 1. Parse with domain awareness
        let parse_result = self.coordinator.parse_with_domains(dsl_content).await?;

        // 2. Validate against domain rules
        let validation = self.registry.validate_for_domain(&parse_result, &context)?;

        // 3. Apply domain-specific transformations
        let processed = self
            .editor
            .process_with_context(parse_result, context)
            .await?;

        Ok(ProcessingResult {
            success: true,
            processed_dsl: processed,
            validation_report: validation,
            metadata: ProcessingMetadata::default(),
        })
    }

    /// Generate DSL for a specific operation in domain context
    pub async fn generate_operation(
        &self,
        operation: DslOperation,
        context: DomainContext,
    ) -> DslResult<String> {
        self.editor
            .generate_dsl_for_operation(operation, context)
            .await
    }

    /// Chain multiple DSL operations together
    pub async fn chain_operations(
        &self,
        operations: Vec<DslOperation>,
        context: DomainContext,
    ) -> DslResult<String> {
        let mut chain = OperationChain::new();
        for op in operations {
            chain.add_operation(op);
        }

        self.editor.execute_operation_chain(chain, context).await
    }

    /// Normalize legacy DSL to current version
    pub fn normalize_dsl(&self, legacy_dsl: &str) -> DslResult<String> {
        self.coordinator.normalize_to_current_version(legacy_dsl)
    }
}

impl Default for DslProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// ProcessingResult, ValidationReport, DomainValidation, VocabularyValidation, DslError,
// and DslResult are now imported from dsl_types crate (see imports above)

// ValidationError, ValidationWarning, SourceLocation, WarningSeverity, ProcessingMetadata,
// ProcessingResult, ValidationReport, DomainValidation, VocabularyValidation, DslError,
// and DslResult are all imported from dsl_types crate (see imports above)

/// Type alias for backward compatibility
pub type DslEditResult<T> = DslResult<T>;

/// Type alias for backward compatibility
pub type DslEditError = DslError;

// ============================================================================
// CONVENIENCE FUNCTIONS - High-Level DSL Operations
// ============================================================================

/// Quick DSL validation function (async)
pub async fn validate_dsl(dsl_content: &str) -> DslResult<ValidationReport> {
    let processor = DslProcessor::new();
    let context = DomainContext::auto_detect(); // Auto-detect from content

    // Extract just validation from full processing
    let result = processor.process_dsl(dsl_content, context).await?;
    Ok(result.validation_report)
}

/// Quick DSL normalization function
pub fn normalize_legacy_dsl(legacy_dsl: &str) -> DslResult<String> {
    let processor = DslProcessor::new();
    processor.normalize_dsl(legacy_dsl)
}

/// Quick domain detection from DSL content
pub fn detect_domains(dsl_content: &str) -> DslResult<Vec<String>> {
    let coordinator = ParsingCoordinator::new();
    coordinator.detect_domains_from_content(dsl_content)
}

// ============================================================================
// MODULE TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dsl_processor_creation() {
        let processor = DslProcessor::new();
        assert!(!processor.registry.registered_domains().is_empty());
    }

    #[test]
    fn test_validation_report_creation() {
        let report = ValidationReport {
            is_valid: true,
            domain_validations: HashMap::new(),
            vocabulary_compliance: VocabularyValidation {
                all_verbs_approved: true,
                approved_verbs: vec!["kyc.start".to_string()],
                unknown_verbs: Vec::new(),
                attribute_compliance: 1.0,
            },
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        assert!(report.is_valid);
        assert!(report.vocabulary_compliance.all_verbs_approved);
    }

    #[test]
    fn test_dsl_error_types() {
        let error = DslError::ValidationError {
            errors: vec![ValidationError {
                code: "TEST001".to_string(),
                message: "Test error".to_string(),
                location: None,
                suggestion: None,
            }],
        };

        assert!(matches!(error, DslError::ValidationError { .. }));
    }

    #[tokio::test]
    async fn test_convenience_functions() {
        let domains = detect_domains("(kyc.start :case-id \"test\")");
        assert!(domains.is_ok());

        let validation = validate_dsl("(kyc.start :case-id \"test\")").await;
        assert!(validation.is_ok());
    }
}
