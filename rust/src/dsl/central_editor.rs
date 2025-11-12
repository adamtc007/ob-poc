//! Central DSL Editor with Domain Context Switching
//!
//! This module implements the core centralized DSL editing engine that replaces
//! domain-specific edit functions with a unified system. The central editor uses
//! domain context to route operations while maintaining the "ONE EBNF, ONE DSL vocab,
//! ONE data dictionary" architecture principle.
//!
//! ## Key Features:
//! - Unified DSL editing interface across all domains
//! - Domain context switching for operation routing
//! - Shared grammar and dictionary validation
//! - Generic DSL append logic (eliminates duplicate domain functions)
//! - Comprehensive audit trail and versioning
//!
//! ## Architecture:
//! The central editor coordinates between domain handlers, grammar validation,
//! dictionary services, and the database layer to provide a consistent DSL
//! editing experience regardless of the business domain.

use crate::{
    dsl::{
        domain_context::DomainContext,
        domain_registry::DomainRegistry,
        operations::{DslOperation, OperationChain},
        parsing_coordinator::ParseResult,
        DslEditError, DslEditResult,
    },
    parser::parse_program,
};
use async_trait::async_trait;
use chrono::Utc;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Central DSL Editor - replaces all domain-specific edit functions
#[derive(Debug)]
pub struct CentralDslEditor {
    /// Domain registry for handler lookup and routing
    domain_registry: Arc<DomainRegistry>,

    /// Dictionary service for attribute validation (ONE dictionary principle)
    dictionary_service: Arc<dyn DictionaryService>,

    /// Editor configuration
    config: EditorConfig,
}

impl CentralDslEditor {
    /// Create a new central DSL editor
    pub fn new(
        domain_registry: Arc<DomainRegistry>,
        dictionary_service: Arc<dyn DictionaryService>,
        config: EditorConfig,
    ) -> Self {
        Self {
            domain_registry,
            dictionary_service,
            config,
        }
    }

    /// Universal DSL edit function with domain context switching
    /// This replaces all domain-specific functions like persist_ob_edit, associate_cbu, etc.
    pub async fn edit_dsl(
        &self,
        instance_id: Uuid,
        domain_context: DomainContext,
        operation: DslOperation,
        created_by: &str,
    ) -> DslEditResult<SimpleDslInstanceVersion> {
        info!(
            "Starting DSL edit for instance {} in domain {}",
            instance_id, domain_context.domain_name
        );

        // Step 1: Route operation to appropriate domain handler
        let domain_handler = self
            .domain_registry
            .route_operation(&operation, &domain_context)
            .await?;

        debug!(
            "Routed operation to domain: {}",
            domain_handler.domain_name()
        );

        // Step 2: Validate operation against domain constraints
        domain_handler
            .validate_operation(&operation, &domain_context)
            .await
            .map_err(|e| {
                DslEditError::DomainValidationError(format!(
                    "Domain validation failed for {}: {}",
                    domain_handler.domain_name(),
                    e
                ))
            })?;

        // Step 3: Transform operation to DSL fragment using domain-specific logic
        let dsl_fragment = domain_handler
            .transform_operation_to_dsl(&operation, &domain_context)
            .await?;

        debug!("Generated DSL fragment: {}", dsl_fragment);

        // Step 4: Apply domain-specific business rules
        let processed_dsl = domain_handler
            .apply_business_rules(&dsl_fragment, &domain_context)
            .await?;

        // Step 5: Validate using shared grammar (ONE EBNF)
        self.validate_grammar(&processed_dsl).await?;

        // Step 6: Validate attributes against dictionary (ONE dictionary)
        self.validate_dictionary(&processed_dsl).await?;

        // Step 7: Execute central append logic (domain-agnostic)
        let result = self
            .append_dsl_increment(
                instance_id,
                &processed_dsl,
                &domain_context,
                &operation,
                created_by,
            )
            .await?;

        info!(
            "Successfully completed DSL edit for instance {}, new version: {}",
            instance_id, result.version_id
        );

        Ok(result)
    }

    /// Create a new DSL instance in a specific domain
    pub async fn create_dsl_instance(
        &self,
        domain_context: DomainContext,
        initial_dsl: &str,
        created_by: &str,
    ) -> DslEditResult<SimpleDslInstanceVersion> {
        info!(
            "Creating new DSL instance in domain {}",
            domain_context.domain_name
        );

        // Validate domain exists
        let _domain_handler = self
            .domain_registry
            .get_domain(&domain_context.domain_name)?;

        // Validate initial DSL
        self.validate_grammar(initial_dsl).await?;
        self.validate_dictionary(initial_dsl).await?;

        // Create initial version - simplified for Phase 1
        let instance_id = Uuid::new_v4();
        let instance_version = SimpleDslInstanceVersion {
            version_id: Uuid::new_v4(),
            instance_id,
            version_number: 1,
            dsl_content: initial_dsl.to_string(),
            created_at: Utc::now(),
            created_by: created_by.to_string(),
            change_description: Some("Initial DSL instance creation".to_string()),
        };

        let compiled_version = self.compile_and_validate_version(instance_version).await?;

        info!(
            "Created DSL instance {} with version {}",
            instance_id, compiled_version.version_id
        );

        Ok(compiled_version)
    }

    /// Get the current DSL content for an instance (simplified for Phase 1)
    pub async fn get_current_dsl(
        &self,
        instance_id: Uuid,
    ) -> DslEditResult<Option<SimpleDslInstanceVersion>> {
        // For Phase 1, return mock data
        Ok(Some(SimpleDslInstanceVersion {
            version_id: Uuid::new_v4(),
            instance_id,
            version_number: 1,
            dsl_content: String::new(),
            created_at: Utc::now(),
            created_by: "system".to_string(),
            change_description: None,
        }))
    }

    /// Validate DSL against shared grammar (ONE EBNF principle)
    async fn validate_grammar(&self, dsl_content: &str) -> DslEditResult<()> {
        debug!("Validating DSL against shared grammar");

        // Parse with grammar engine to validate syntax
        // For Phase 1, just parse with the standard parser
        parse_program(dsl_content).map_err(|e| {
            DslEditError::GrammarValidationError(format!("Grammar validation failed: {:?}", e))
        })?;

        Ok(())
    }

    /// Validate attributes against dictionary (ONE dictionary principle)
    async fn validate_dictionary(&self, dsl_content: &str) -> DslEditResult<()> {
        debug!("Validating DSL against data dictionary");

        self.dictionary_service
            .validate_dsl_attributes(dsl_content)
            .await
            .map_err(|e| {
                DslEditError::DictionaryValidationError(format!(
                    "Dictionary validation failed: {}",
                    e
                ))
            })?;

        Ok(())
    }

    /// Generic DSL append logic - replaces all domain-specific persist_*_edit functions
    async fn append_dsl_increment(
        &self,
        instance_id: Uuid,
        dsl_fragment: &str,
        _domain_context: &DomainContext,
        operation: &DslOperation,
        created_by: &str,
    ) -> DslEditResult<SimpleDslInstanceVersion> {
        debug!("Appending DSL increment to instance {}", instance_id);

        // Get current DSL content - simplified for Phase 1
        let current_version = SimpleDslInstanceVersion {
            version_id: Uuid::new_v4(),
            instance_id,
            version_number: 1,
            dsl_content: String::new(),
            created_at: Utc::now(),
            created_by: "system".to_string(),
            change_description: None,
        };

        // Combine existing DSL with new fragment
        let combined_dsl = if current_version.dsl_content.trim().is_empty() {
            dsl_fragment.to_string()
        } else {
            format!("{}\n\n{}", current_version.dsl_content, dsl_fragment)
        };

        // Parse combined DSL to ensure validity
        parse_program(&combined_dsl)
            .map_err(|e| DslEditError::CompilationError(format!("DSL parsing failed: {:?}", e)))?;

        // Create new version record - simplified for Phase 1
        let instance_version = SimpleDslInstanceVersion {
            version_id: Uuid::new_v4(),
            instance_id,
            version_number: current_version.version_number + 1,
            dsl_content: combined_dsl,
            created_at: Utc::now(),
            created_by: created_by.to_string(),
            change_description: Some(operation.description()),
        };

        // Compile and validate
        let compiled_version = self.compile_and_validate_version(instance_version).await?;

        // Update instance metadata if configured
        if self.config.update_instance_metadata {
            self.update_instance_metadata(instance_id, &compiled_version)
                .await?;
        }

        Ok(compiled_version)
    }

    /// Compile DSL to AST and validate - simplified for Phase 1
    async fn compile_and_validate_version(
        &self,
        version: SimpleDslInstanceVersion,
    ) -> DslEditResult<SimpleDslInstanceVersion> {
        debug!("Compiling DSL to AST for version {}", version.version_id);

        // Parse DSL to AST for validation
        let _ast = parse_program(&version.dsl_content)
            .map_err(|e| DslEditError::CompilationError(format!("Failed to parse DSL: {:?}", e)))?;

        // Return validated version
        Ok(version)
    }

    /// Update instance metadata - simplified for Phase 1
    async fn update_instance_metadata(
        &self,
        instance_id: Uuid,
        _version: &SimpleDslInstanceVersion,
    ) -> DslEditResult<()> {
        debug!(
            "Updating instance metadata for {} (Phase 1 - no-op)",
            instance_id
        );
        Ok(())
    }

    /// Get editor statistics and health information
    pub async fn get_editor_stats(&self) -> EditorStats {
        let domain_health = self.domain_registry.health_check_all().await;

        EditorStats {
            total_domains: self.domain_registry.list_domains().len(),
            healthy_domains: domain_health
                .values()
                .filter(|h| matches!(h.status, crate::dsl::domain_registry::HealthStatus::Healthy))
                .count(),
            operations_processed: 0, // Would be tracked in production
            last_operation: None,
            domain_health,
        }
    }

    /// Process DSL with domain context (facade method for DslProcessor)
    pub async fn process_with_context(
        &self,
        parse_result: ParseResult,
        context: DomainContext,
    ) -> DslEditResult<String> {
        // For now, return a simple processed result
        // In full implementation, this would apply domain-specific transformations
        Ok(format!("Processed DSL for domain: {}", context.domain_name))
    }

    /// Generate DSL for a specific operation (facade method)
    pub async fn generate_dsl_for_operation(
        &self,
        operation: DslOperation,
        context: DomainContext,
    ) -> DslEditResult<String> {
        // Basic DSL generation based on operation type
        let (operation_name, operation_id) = match &operation {
            DslOperation::CreateEntity { metadata, .. } => {
                ("create", metadata.operation_id.to_string())
            }
            DslOperation::UpdateEntity { metadata, .. } => {
                ("update", metadata.operation_id.to_string())
            }
            DslOperation::AddProducts { metadata, .. } => {
                ("add_products", metadata.operation_id.to_string())
            }
            DslOperation::AddServices { metadata, .. } => {
                ("add_services", metadata.operation_id.to_string())
            }
            DslOperation::UpdateAttribute { metadata, .. } => {
                ("update_attribute", metadata.operation_id.to_string())
            }
            DslOperation::TransitionState { metadata, .. } => {
                ("transition_state", metadata.operation_id.to_string())
            }
            DslOperation::CollectDocument { metadata, .. } => {
                ("collect_document", metadata.operation_id.to_string())
            }
            DslOperation::CreateRelationship { metadata, .. } => {
                ("create_relationship", metadata.operation_id.to_string())
            }
            DslOperation::ValidateData { metadata, .. } => {
                ("validate_data", metadata.operation_id.to_string())
            }
            DslOperation::ExecuteWorkflowStep { metadata, .. } => {
                ("execute_workflow_step", metadata.operation_id.to_string())
            }
            DslOperation::SendNotification { metadata, .. } => {
                ("send_notification", metadata.operation_id.to_string())
            }
            DslOperation::DomainSpecific {
                operation_type,
                metadata,
                ..
            } => (operation_type.as_str(), metadata.operation_id.to_string()),
            DslOperation::Composite { metadata, .. } => {
                ("composite", metadata.operation_id.to_string())
            }
        };

        Ok(format!(
            "({}.{} :context \"{}\")",
            context.domain_name, operation_name, operation_id
        ))
    }

    /// Execute operation chain (facade method)
    pub async fn execute_operation_chain(
        &self,
        chain: OperationChain,
        context: DomainContext,
    ) -> DslEditResult<String> {
        // Execute operations in sequence and combine results
        let mut results = Vec::new();

        for operation in chain.operations {
            let result = self
                .generate_dsl_for_operation(operation, context.clone())
                .await?;
            results.push(result);
        }

        Ok(results.join("\n\n"))
    }
}

/// Configuration for the central DSL editor
#[derive(Debug, Clone)]
pub(crate) struct EditorConfig {
    /// Whether to update instance metadata after each edit
    pub update_instance_metadata: bool,

    /// Maximum DSL size allowed (in characters)
    pub max_dsl_size: usize,

    /// Enable strict validation mode
    pub strict_validation: bool,

    /// Enable audit logging
    pub enable_audit_log: bool,

    /// Validation timeout in seconds
    pub validation_timeout_secs: u64,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            update_instance_metadata: true,
            max_dsl_size: 1_000_000, // 1MB
            strict_validation: true,
            enable_audit_log: true,
            validation_timeout_secs: 30,
        }
    }
}

/// Statistics and health information for the editor
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct EditorStats {
    pub total_domains: usize,
    pub healthy_domains: usize,
    pub operations_processed: u64,
    pub last_operation: Option<chrono::DateTime<chrono::Utc>>,
    pub domain_health:
        std::collections::HashMap<String, crate::dsl::domain_registry::DomainHealthStatus>,
}

/// Simplified DSL instance version for Phase 1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimpleDslInstanceVersion {
    pub version_id: Uuid,
    pub instance_id: Uuid,
    pub version_number: i32,
    pub dsl_content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: String,
    pub change_description: Option<String>,
}

/// Repository trait for domain data persistence (simplified for Phase 1)
#[async_trait]
pub trait DomainRepository: Send + Sync {
    async fn store_version(&self, version: &SimpleDslInstanceVersion) -> Result<(), String>;
    async fn get_latest_version(
        &self,
        instance_id: Uuid,
    ) -> Result<Option<SimpleDslInstanceVersion>, String>;
}

// Grammar validation is handled by the GrammarEngine from the grammar module

/// Dictionary service trait for attribute validation (Phase 1 simplified)
#[async_trait]
pub trait DictionaryService: Send + Sync {
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{
        domain_context::DomainContext,
        domain_registry::{
            DomainHandler, DomainRegistry, DslVocabulary, StateTransition, ValidationRule,
        },
        operations::OperationBuilder,
    };
    use async_trait::async_trait;
    use std::collections::HashMap;

    // Mock implementations for testing
    #[allow(dead_code)]
    struct MockDomainRepository;

    #[async_trait]
    impl DomainRepository for MockDomainRepository {
        async fn store_version(&self, _version: &SimpleDslInstanceVersion) -> Result<(), String> {
            Ok(())
        }

        async fn get_latest_version(
            &self,
            _instance_id: Uuid,
        ) -> Result<Option<SimpleDslInstanceVersion>, String> {
            Ok(Some(SimpleDslInstanceVersion {
                version_id: Uuid::new_v4(),
                instance_id: Uuid::new_v4(),
                version_number: 1,
                dsl_content: "".to_string(),
                created_at: Utc::now(),
                created_by: "test".to_string(),
                change_description: None,
            }))
        }
    }

    #[allow(dead_code)]
    struct MockGrammarEngine {
        supported_operations: Vec<String>,
    }

    impl MockGrammarEngine {
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                supported_operations: vec!["Create Company entity".to_string()],
            }
        }
    }

    struct MockDictionaryService;

    #[async_trait]
    impl DictionaryService for MockDictionaryService {
        async fn validate_dsl_attributes(&self, _dsl: &str) -> Result<(), String> {
            Ok(())
        }
    }

    struct MockDomainHandler;

    #[async_trait]
    impl DomainHandler for MockDomainHandler {
        fn domain_name(&self) -> &str {
            "test"
        }
        fn domain_version(&self) -> &str {
            "1.0.0"
        }
        fn domain_description(&self) -> &str {
            "Test domain"
        }
        fn get_vocabulary(&self) -> &DslVocabulary {
            static VOCAB: std::sync::OnceLock<DslVocabulary> = std::sync::OnceLock::new();
            VOCAB.get_or_init(DslVocabulary::default)
        }

        async fn transform_operation_to_dsl(
            &self,
            operation: &DslOperation,
            _context: &DomainContext,
        ) -> DslEditResult<String> {
            // Generate valid DSL that will parse correctly
            match operation {
                DslOperation::CreateEntity { entity_type, .. } => Ok(format!(
                    "(entity :id \"test-entity\" :label \"{}\")",
                    entity_type
                )),
                _ => Ok("(entity :id \"test-entity\" :label \"Company\")".to_string()),
            }
        }

        async fn validate_operation(
            &self,
            _operation: &DslOperation,
            _context: &DomainContext,
        ) -> DslEditResult<()> {
            Ok(())
        }

        fn get_valid_transitions(&self) -> &[StateTransition] {
            &[]
        }
        fn validate_state_transition(&self, _from: &str, _to: &str) -> DslEditResult<()> {
            Ok(())
        }

        async fn apply_business_rules(
            &self,
            dsl_fragment: &str,
            _context: &DomainContext,
        ) -> DslEditResult<String> {
            Ok(dsl_fragment.to_string())
        }

        fn supported_operations(&self) -> &[String] {
            static OPS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
            OPS.get_or_init(|| vec!["Create Company entity".to_string()])
        }

        fn get_validation_rules(&self) -> &[ValidationRule] {
            &[]
        }

        async fn extract_context_from_dsl(&self, _dsl: &str) -> DslEditResult<DomainContext> {
            Ok(DomainContext::new("test"))
        }

        async fn health_check(&self) -> crate::dsl::domain_registry::DomainHealthStatus {
            crate::dsl::domain_registry::DomainHealthStatus {
                domain_name: "test".to_string(),
                status: crate::dsl::domain_registry::HealthStatus::Healthy,
                last_check: Utc::now(),
                metrics: HashMap::new(),
                errors: Vec::new(),
            }
        }
    }

    #[tokio::test]
    async fn test_central_editor_creation() {
        let mut registry = DomainRegistry::new();
        registry
            .register_domain(Box::new(MockDomainHandler))
            .unwrap();

        let editor = CentralDslEditor::new(
            Arc::new(registry),
            Arc::new(MockDictionaryService),
            EditorConfig::default(),
        );

        let stats = editor.get_editor_stats().await;
        assert_eq!(stats.total_domains, 1);
        assert_eq!(stats.healthy_domains, 1);
    }

    #[tokio::test]
    async fn test_dsl_edit_operation() {
        let mut registry = DomainRegistry::new();
        registry
            .register_domain(Box::new(MockDomainHandler))
            .unwrap();

        let editor = CentralDslEditor::new(
            Arc::new(registry),
            Arc::new(MockDictionaryService),
            EditorConfig::default(),
        );

        let operation = OperationBuilder::new("test_user").create_entity(
            "Company",
            [("name".to_string(), serde_json::json!("Test Corp"))]
                .iter()
                .cloned()
                .collect(),
        );

        let context = DomainContext::new("test");
        let instance_id = Uuid::new_v4();

        let result = editor
            .edit_dsl(instance_id, context, operation, "test_user")
            .await;

        if let Err(ref e) = result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok());
    }
}
