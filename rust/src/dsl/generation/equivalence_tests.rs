//! Equivalence Tests for Template vs AI Generation
//!
//! This module implements equivalence testing between template-based and AI-based
//! DSL generation methods as specified in Phase 1.5 of the implementation plan.

use super::traits::{
    GenerationMethod, GenerationOperationType, GenerationRequest, GenerationResponse,
};
use super::{
    ai::{
        AiGenerationError, AiGenerationRequest, AiGenerationResponse, AiServiceMetadata,
        AiServiceStatus, TokenUsage,
    },
    context::GenerationContextBuilder,
    factory::GenerationFactory,
    traits::DslGenerator,
    AiDslGenerationService,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::fs;

/// Equivalence test configuration
#[derive(Debug, Clone)]
pub struct EquivalenceTestConfig {
    /// Tolerance for confidence score differences
    pub confidence_tolerance: f64,
    /// Tolerance for processing time differences (in milliseconds)
    pub time_tolerance_ms: u64,
    /// Whether to require exact DSL content match
    pub require_exact_match: bool,
    /// Whether to compare semantic structure instead of exact text
    pub semantic_comparison: bool,
}

/// Equivalence test result
#[derive(Debug, Clone)]
pub struct EquivalenceTestResult {
    /// Whether the generations are considered equivalent
    pub equivalent: bool,
    /// Template generation response
    pub template_response: GenerationResponse,
    /// AI generation response
    pub ai_response: GenerationResponse,
    /// Detailed comparison results
    pub comparison: GenerationComparison,
    /// Any issues found during comparison
    pub issues: Vec<String>,
}

/// Detailed comparison between two generation responses
#[derive(Debug, Clone)]
pub struct GenerationComparison {
    /// Content similarity score (0.0 to 1.0)
    pub content_similarity: f64,
    /// Whether both generations succeeded
    pub both_successful: bool,
    /// Confidence score difference
    pub confidence_difference: f64,
    /// Processing time difference (in milliseconds)
    pub time_difference_ms: i64,
    /// Semantic structure comparison
    pub semantic_equivalent: bool,
    /// Key differences found
    pub differences: Vec<String>,
}

impl Default for EquivalenceTestConfig {
    fn default() -> Self {
        Self {
            confidence_tolerance: 0.1, // 10% tolerance
            time_tolerance_ms: 5000,   // 5 second tolerance
            require_exact_match: false,
            semantic_comparison: true,
        }
    }
}

/// Mock AI service for testing equivalence
#[derive(Debug, Clone)]
pub struct MockAiService {
    /// Pre-configured responses for different operation types
    responses: HashMap<GenerationOperationType, String>,
    /// Whether to simulate success or failure
    should_succeed: bool,
    /// Simulated processing delay (in milliseconds)
    simulated_delay_ms: u64,
}

impl Default for MockAiService {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAiService {
    /// Create a new mock AI service with default responses
    pub fn new() -> Self {
        let mut responses = HashMap::new();

        // Add realistic DSL responses for each operation type
        responses.insert(
            GenerationOperationType::CreateCbu,
            r#"(case.create
  :name "CBU Creation - Test CBU"
  :type "cbu_onboarding")

(cbu.create
  :name "Test CBU"
  :description "Generated CBU for testing"
  :status "ACTIVE")

(audit.log
  :operation "cbu_create"
  :entity-name "Test CBU")"#
                .to_string(),
        );

        responses.insert(
            GenerationOperationType::RegisterEntity,
            r#"(case.create
  :name "Entity Registration - Test Entity"
  :type "entity_registration")

(entity.register
  :entity-id "test-entity-123"
  :name "Test Entity"
  :type "CORPORATION")

(identity.verify
  :entity-id "test-entity-123"
  :level "STANDARD")"#
                .to_string(),
        );

        responses.insert(
            GenerationOperationType::CalculateUbo,
            r#"(case.create
  :name "UBO Calculation - Test Entity"
  :type "ubo_calculation")

(ubo.collect-entity-data
  :entity-id "test-entity-123")

(ubo.get-ownership-structure
  :entity-id "test-entity-123")

(ubo.resolve-ubos
  :entity-id "test-entity-123"
  :threshold 0.25)

(ubo.calculate-indirect-ownership
  :entity-id "test-entity-123")"#
                .to_string(),
        );

        Self {
            responses,
            should_succeed: true,
            simulated_delay_ms: 100,
        }
    }

    /// Configure the service to fail
    pub fn with_failure(mut self) -> Self {
        self.should_succeed = false;
        self
    }

    /// Configure simulated delay
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.simulated_delay_ms = delay_ms;
        self
    }

    /// Add a custom response for an operation type
    pub fn with_response(
        mut self,
        operation_type: GenerationOperationType,
        response: String,
    ) -> Self {
        self.responses.insert(operation_type, response);
        self
    }
}

#[async_trait::async_trait]
impl AiDslGenerationService for MockAiService {
    async fn generate_dsl_ai(
        &self,
        request: &AiGenerationRequest,
    ) -> Result<AiGenerationResponse, AiGenerationError> {
        // Simulate processing delay
        if self.simulated_delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.simulated_delay_ms)).await;
        }

        if !self.should_succeed {
            return Err(AiGenerationError::ServiceError {
                message: "Mock AI service configured to fail".to_string(),
            });
        }

        let dsl_content = self
            .responses
            .get(&request.operation_type)
            .cloned()
            .unwrap_or_else(|| {
                format!(
                    "(mock.operation :type \"{}\" :instruction \"{}\")",
                    request.operation_type, request.instruction
                )
            });

        Ok(AiGenerationResponse {
            dsl_content,
            model_used: "mock-ai-model".to_string(),
            confidence_score: 0.85,
            token_usage: TokenUsage {
                prompt_tokens: 150,
                completion_tokens: 75,
                total_tokens: 225,
            },
            metadata: HashMap::new(),
        })
    }

    fn available_models(&self) -> Vec<String> {
        vec!["mock-ai-model".to_string()]
    }

    async fn health_check(&self) -> bool {
        self.should_succeed
    }

    fn service_metadata(&self) -> AiServiceMetadata {
        AiServiceMetadata {
            name: "MockAiService".to_string(),
            version: "1.0.0".to_string(),
            models: vec!["mock-ai-model".to_string()],
            capabilities: HashMap::new(),
            status: if self.should_succeed {
                AiServiceStatus::Active
            } else {
                AiServiceStatus::Inactive
            },
        }
    }
}

/// Create test templates for equivalence testing
async fn create_test_templates(templates_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Create onboarding directory
    let onboarding_dir = templates_dir.join("onboarding");
    fs::create_dir_all(&onboarding_dir).await?;

    // Create CBU template
    let cbu_template = r#"---
template_id: "create_cbu"
domain: "onboarding"
template_type: "CreateCbu"
description: "Creates DSL for CBU creation"
variables:
  - name: "cbu_name"
    type: "string"
    required: true
    description: "CBU name"
  - name: "cbu_description"
    type: "string"
    required: false
    description: "CBU description"
    default_value: "Generated CBU for testing"
requirements:
  prerequisite_operations: []
  required_attributes: ["cbu_name"]
  database_queries: []
  validation_rules: ["cbu_name must not be empty"]
---

(case.create
  :name "CBU Creation - {{cbu_name}}"
  :type "cbu_onboarding")

(cbu.create
  :name "{{cbu_name}}"
  :description "{{cbu_description}}"
  :status "ACTIVE")

(audit.log
  :operation "cbu_create"
  :entity-name "{{cbu_name}}")
"#;

    fs::write(onboarding_dir.join("create_cbu.dsl.template"), cbu_template).await?;

    // Create entity registration template
    let entity_template = r#"---
template_id: "register_entity"
domain: "onboarding"
template_type: "RegisterEntity"
description: "Creates DSL for entity registration"
variables:
  - name: "entity_id"
    type: "uuid"
    required: true
    description: "Entity identifier"
  - name: "entity_name"
    type: "string"
    required: true
    description: "Entity name"
  - name: "entity_type"
    type: "string"
    required: true
    description: "Entity type"
    default_value: "CORPORATION"
requirements:
  prerequisite_operations: []
  required_attributes: ["entity_id", "entity_name"]
  database_queries: []
  validation_rules: ["entity_id must be valid UUID"]
---

(case.create
  :name "Entity Registration - {{entity_name}}"
  :type "entity_registration")

(entity.register
  :entity-id "{{entity_id}}"
  :name "{{entity_name}}"
  :type "{{entity_type}}")

(identity.verify
  :entity-id "{{entity_id}}"
  :level "STANDARD")
"#;

    fs::write(
        onboarding_dir.join("register_entity.dsl.template"),
        entity_template,
    )
    .await?;

    // Create UBO calculation template
    let ubo_template = r#"---
template_id: "calculate_ubo"
domain: "ubo"
template_type: "CalculateUbo"
description: "Creates DSL for UBO calculation"
variables:
  - name: "target_entity"
    type: "uuid"
    required: true
    description: "Target entity for UBO calculation"
  - name: "threshold"
    type: "number"
    required: false
    description: "Ownership threshold"
    default_value: "0.25"
requirements:
  prerequisite_operations: []
  required_attributes: ["target_entity"]
  database_queries: []
  validation_rules: ["target_entity must be valid UUID"]
---

(case.create
  :name "UBO Calculation - {{target_entity}}"
  :type "ubo_calculation")

(ubo.collect-entity-data
  :entity-id "{{target_entity}}")

(ubo.get-ownership-structure
  :entity-id "{{target_entity}}")

(ubo.resolve-ubos
  :entity-id "{{target_entity}}"
  :threshold {{threshold}})

(ubo.calculate-indirect-ownership
  :entity-id "{{target_entity}}")
"#;

    // Create UBO directory
    let ubo_dir = templates_dir.join("ubo");
    fs::create_dir_all(&ubo_dir).await?;
    fs::write(ubo_dir.join("calculate_ubo.dsl.template"), ubo_template).await?;

    Ok(())
}

/// Compare two DSL content strings for semantic equivalence
pub fn compare_dsl_semantic(content1: &str, content2: &str) -> GenerationComparison {
    let mut comparison = GenerationComparison {
        content_similarity: 0.0,
        both_successful: true,
        confidence_difference: 0.0,
        time_difference_ms: 0,
        semantic_equivalent: false,
        differences: Vec::new(),
    };

    // Simple semantic comparison - check for key elements
    let keywords1 = extract_dsl_keywords(content1);
    let keywords2 = extract_dsl_keywords(content2);

    // Calculate keyword similarity
    let common_keywords = keywords1.intersection(&keywords2).count();
    let total_keywords = keywords1.union(&keywords2).count();

    if total_keywords > 0 {
        comparison.content_similarity = common_keywords as f64 / total_keywords as f64;
    }

    // Check for semantic equivalence (similar operations present)
    comparison.semantic_equivalent = comparison.content_similarity > 0.7;

    // Find differences
    for keyword in keywords1.difference(&keywords2) {
        comparison
            .differences
            .push(format!("Missing in second: {}", keyword));
    }

    for keyword in keywords2.difference(&keywords1) {
        comparison
            .differences
            .push(format!("Missing in first: {}", keyword));
    }

    comparison
}

/// Extract keywords from DSL content for comparison
fn extract_dsl_keywords(content: &str) -> std::collections::HashSet<String> {
    let mut keywords = std::collections::HashSet::new();

    // Extract S-expression patterns
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('(') {
            // Extract the operation name (first word after opening paren)
            if let Some(op_end) = trimmed[1..].find(|c: char| c.is_whitespace() || c == ')') {
                let operation = &trimmed[1..=op_end];
                keywords.insert(operation.to_string());
            }
        }

        // Extract attribute patterns (:attribute-name)
        for word in trimmed.split_whitespace() {
            if word.starts_with(':') {
                keywords.insert(word.to_string());
            }
        }
    }

    keywords
}

/// Assert that two DSL generations are equivalent
pub fn assert_equivalent_dsl(
    template_response: &GenerationResponse,
    ai_response: &GenerationResponse,
    config: &EquivalenceTestConfig,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Check success status
    if template_response.success != ai_response.success {
        errors.push(format!(
            "Success status mismatch: template={}, ai={}",
            template_response.success, ai_response.success
        ));
    }

    // If both failed, that's equivalent failure
    if !template_response.success && !ai_response.success {
        return if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        };
    }

    // Compare content if both succeeded
    if template_response.success && ai_response.success {
        let comparison =
            compare_dsl_semantic(&template_response.dsl_content, &ai_response.dsl_content);

        if config.require_exact_match {
            if template_response.dsl_content.trim() != ai_response.dsl_content.trim() {
                errors.push("DSL content is not exactly identical".to_string());
            }
        } else if config.semantic_comparison
            && !comparison.semantic_equivalent {
                errors.push(format!(
                    "DSL content is not semantically equivalent (similarity: {:.2})",
                    comparison.content_similarity
                ));
            }

        // Check confidence scores
        if let (Some(template_conf), Some(ai_conf)) = (
            template_response.confidence_score,
            ai_response.confidence_score,
        ) {
            let conf_diff = (template_conf - ai_conf).abs();
            if conf_diff > config.confidence_tolerance {
                errors.push(format!(
                    "Confidence scores too different: template={:.2}, ai={:.2} (diff={:.2}, tolerance={:.2})",
                    template_conf, ai_conf, conf_diff, config.confidence_tolerance
                ));
            }
        }

        // Check processing times
        let time_diff =
            template_response.processing_time_ms as i64 - ai_response.processing_time_ms as i64;
        if time_diff.abs() > config.time_tolerance_ms as i64 {
            errors.push(format!(
                "Processing times too different: template={}ms, ai={}ms (diff={}ms, tolerance={}ms)",
                template_response.processing_time_ms, ai_response.processing_time_ms,
                time_diff.abs(), config.time_tolerance_ms
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Test template vs AI generation equivalence for a specific operation
pub async fn test_template_agent_equivalence(
    operation_type: GenerationOperationType,
    config: EquivalenceTestConfig,
) -> Result<EquivalenceTestResult, Box<dyn std::error::Error>> {
    // Create temporary directory for templates
    let temp_dir = tempdir()?;
    let templates_dir = temp_dir.path().to_path_buf();

    // Create test templates
    create_test_templates(&templates_dir).await?;

    // Create factory with both template and AI generators
    let ai_service = Arc::new(MockAiService::new());
    let factory = GenerationFactory::with_ai_service(templates_dir, ai_service);

    // Create context based on operation type
    let context = match operation_type {
        GenerationOperationType::CreateCbu => GenerationContextBuilder::new()
            .template_variable("cbu_name", "Test CBU")
            .template_variable("cbu_description", "Generated CBU for testing")
            .instruction("Create a CBU for testing purposes")
            .entity_data("cbu_name", "Test CBU")
            .build(),
        GenerationOperationType::RegisterEntity => GenerationContextBuilder::new()
            .template_variable("entity_id", "test-entity-123")
            .template_variable("entity_name", "Test Entity")
            .template_variable("entity_type", "CORPORATION")
            .instruction("Register a test entity")
            .entity_data("entity_name", "Test Entity")
            .build(),
        GenerationOperationType::CalculateUbo => GenerationContextBuilder::new()
            .template_variable("target_entity", "test-entity-123")
            .template_variable("threshold", "0.25")
            .instruction("Calculate UBO for test entity")
            .entity_data("target_entity", "test-entity-123")
            .build(),
        _ => GenerationContextBuilder::new()
            .instruction("Perform test operation")
            .build(),
    };

    // Create generation request
    let request = GenerationRequest::new(operation_type.clone(), context);

    // Generate with template
    let template_generator = factory.create_template_generator()?;
    let template_response = template_generator.generate_dsl(request.clone()).await;

    // Generate with AI
    let ai_generator = factory.create_ai_generator()?;
    let ai_response = ai_generator.generate_dsl(request).await;

    // Handle results
    let (template_response, ai_response) = match (template_response, ai_response) {
        (Ok(template), Ok(ai)) => (template, ai),
        (Err(template_err), Ok(ai)) => {
            return Ok(EquivalenceTestResult {
                equivalent: false,
                template_response: GenerationResponse {
                    success: false,
                    dsl_content: String::new(),
                    method_used: super::traits::GenerationMethod::Template {
                        template_id: "unknown".to_string(),
                    },
                    processing_time_ms: 0,
                    confidence_score: None,
                    errors: vec![template_err.to_string()],
                    warnings: vec![],
                    debug_info: None,
                    metadata: HashMap::new(),
                },
                ai_response: ai,
                comparison: GenerationComparison {
                    content_similarity: 0.0,
                    both_successful: false,
                    confidence_difference: 0.0,
                    time_difference_ms: 0,
                    semantic_equivalent: false,
                    differences: vec!["Template generation failed".to_string()],
                },
                issues: vec![format!("Template generation failed: {}", template_err)],
            });
        }
        (Ok(template), Err(ai_err)) => {
            return Ok(EquivalenceTestResult {
                equivalent: false,
                template_response: template,
                ai_response: GenerationResponse {
                    success: false,
                    dsl_content: String::new(),
                    method_used: super::traits::GenerationMethod::Ai {
                        model: "unknown".to_string(),
                    },
                    processing_time_ms: 0,
                    confidence_score: None,
                    errors: vec![ai_err.to_string()],
                    warnings: vec![],
                    debug_info: None,
                    metadata: HashMap::new(),
                },
                comparison: GenerationComparison {
                    content_similarity: 0.0,
                    both_successful: false,
                    confidence_difference: 0.0,
                    time_difference_ms: 0,
                    semantic_equivalent: false,
                    differences: vec!["AI generation failed".to_string()],
                },
                issues: vec![format!("AI generation failed: {}", ai_err)],
            });
        }
        (Err(template_err), Err(ai_err)) => {
            return Ok(EquivalenceTestResult {
                equivalent: true, // Both failed equivalently
                template_response: GenerationResponse {
                    success: false,
                    dsl_content: String::new(),
                    method_used: GenerationMethod::Template {
                        template_id: "unknown".to_string(),
                    },
                    processing_time_ms: 0,
                    confidence_score: None,
                    errors: vec![template_err.to_string()],
                    warnings: vec![],
                    debug_info: None,
                    metadata: HashMap::new(),
                },
                ai_response: GenerationResponse {
                    success: false,
                    dsl_content: String::new(),
                    method_used: GenerationMethod::Ai {
                        model: "unknown".to_string(),
                    },
                    processing_time_ms: 0,
                    confidence_score: None,
                    errors: vec![ai_err.to_string()],
                    warnings: vec![],
                    debug_info: None,
                    metadata: HashMap::new(),
                },
                comparison: GenerationComparison {
                    content_similarity: 1.0, // Both failed
                    both_successful: false,
                    confidence_difference: 0.0,
                    time_difference_ms: 0,
                    semantic_equivalent: true, // Equivalent failure
                    differences: vec![],
                },
                issues: vec![],
            });
        }
    };

    // Compare responses
    let comparison = compare_dsl_semantic(&template_response.dsl_content, &ai_response.dsl_content);
    let assertion_result = assert_equivalent_dsl(&template_response, &ai_response, &config);

    let equivalent = assertion_result.is_ok();
    let issues = match assertion_result {
        Ok(()) => vec![],
        Err(errors) => errors,
    };

    Ok(EquivalenceTestResult {
        equivalent,
        template_response,
        ai_response,
        comparison,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_cbu_equivalence() {
        let config = EquivalenceTestConfig::default();
        let result = test_template_agent_equivalence(GenerationOperationType::CreateCbu, config)
            .await
            .unwrap();

        println!("CBU Equivalence Test Result:");
        println!("Equivalent: {}", result.equivalent);
        println!("Template DSL: {}", result.template_response.dsl_content);
        println!("AI DSL: {}", result.ai_response.dsl_content);
        println!("Similarity: {:.2}", result.comparison.content_similarity);

        if !result.issues.is_empty() {
            println!("Issues: {:?}", result.issues);
        }

        assert!(result.template_response.success);
        assert!(result.ai_response.success);
        assert!(
            result.comparison.content_similarity > 0.5,
            "Content similarity too low"
        );
    }

    #[tokio::test]
    async fn test_register_entity_equivalence() {
        let config = EquivalenceTestConfig::default();
        let result =
            test_template_agent_equivalence(GenerationOperationType::RegisterEntity, config)
                .await
                .unwrap();

        assert!(result.template_response.success);
        assert!(result.ai_response.success);
        assert!(result.comparison.content_similarity > 0.5);
    }

    #[tokio::test]
    async fn test_ubo_calculation_equivalence() {
        let config = EquivalenceTestConfig::default();
        let result = test_template_agent_equivalence(GenerationOperationType::CalculateUbo, config)
            .await
            .unwrap();

        assert!(result.template_response.success);
        assert!(result.ai_response.success);
        assert!(result.comparison.content_similarity > 0.5);
    }

    #[test]
    fn test_dsl_keyword_extraction() {
        let dsl_content = r#"
        (case.create
          :name "Test Case"
          :type "test")

        (entity.register
          :entity-id "123"
          :name "Test")
        "#;

        let keywords = extract_dsl_keywords(dsl_content);

        assert!(keywords.contains("case.create"));
        assert!(keywords.contains("entity.register"));
        assert!(keywords.contains(":name"));
        assert!(keywords.contains(":type"));
        assert!(keywords.contains(":entity-id"));
    }

    #[test]
    fn test_semantic_comparison() {
        let dsl1 = r#"
        (case.create :name "Test" :type "cbu")
        (cbu.create :name "Test" :status "ACTIVE")
        "#;

        let dsl2 = r#"
        (case.create :name "Test CBU" :type "cbu_onboarding")
        (cbu.create :name "Test CBU" :description "Test" :status "ACTIVE")
        "#;

        let comparison = compare_dsl_semantic(dsl1, dsl2);

        assert!(comparison.content_similarity > 0.7);
        assert!(comparison.semantic_equivalent);
    }

    #[test]
    fn test_equivalence_assertion() {
        let template_response = GenerationResponse {
            success: true,
            dsl_content: "(case.create :name \"Test\")".to_string(),
            method_used: GenerationMethod::Template {
                template_id: "test".to_string(),
            },
            processing_time_ms: 50,
            confidence_score: Some(1.0),
            errors: vec![],
            warnings: vec![],
            debug_info: None,
            metadata: HashMap::new(),
        };

        let ai_response = GenerationResponse {
            success: true,
            dsl_content: "(case.create :name \"Test Case\")".to_string(),
            method_used: GenerationMethod::Ai {
                model: "test".to_string(),
            },
            processing_time_ms: 2000,
            confidence_score: Some(0.9),
            errors: vec![],
            warnings: vec![],
            debug_info: None,
            metadata: HashMap::new(),
        };

        let config = EquivalenceTestConfig::default();
        let result = assert_equivalent_dsl(&template_response, &ai_response, &config);

        // This should pass because semantic comparison allows for minor differences
        assert!(result.is_ok(), "Equivalence assertion failed: {:?}", result);
    }
}
