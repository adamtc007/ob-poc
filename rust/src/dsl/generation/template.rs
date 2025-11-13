//! Template-based DSL Generator
//!
//! This module implements DSL generation using pre-defined templates.
//! Templates are loaded from the filesystem and rendered with context variables.

use super::traits::{
    DslGenerator, GenerationError, GenerationMethod, GenerationOperationType, GenerationRequest,
    GenerationResponse, GenerationResult, GeneratorMetadata,
};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Template-based DSL generator
#[derive(Debug, Clone)]
pub struct TemplateGenerator {
    /// Template storage directory
    templates_dir: PathBuf,

    /// Cached templates
    template_cache: HashMap<String, Template>,

    /// Generator configuration
    config: TemplateGeneratorConfig,
}

/// Configuration for template generator
#[derive(Debug, Clone)]
pub struct TemplateGeneratorConfig {
    /// Enable template caching
    pub enable_caching: bool,

    /// Maximum cache size
    pub max_cache_size: usize,

    /// Template file extension
    pub template_extension: String,

    /// Enable strict validation
    pub strict_validation: bool,
}

/// Template structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// Template identifier
    pub template_id: String,

    /// Domain this template belongs to
    pub domain: String,

    /// Template type
    pub template_type: String,

    /// Description of the template
    pub description: String,

    /// Required variables
    pub variables: Vec<TemplateVariable>,

    /// Template requirements
    pub requirements: TemplateRequirements,

    /// Template content
    pub content: String,
}

/// Template variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    /// Variable name
    pub name: String,

    /// Variable type
    pub var_type: String,

    /// Whether this variable is required
    pub required: bool,

    /// Variable description
    pub description: String,

    /// Default value (if any)
    pub default_value: Option<String>,
}

/// Template requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRequirements {
    /// Required prerequisite operations
    pub prerequisite_operations: Vec<String>,

    /// Required attributes
    pub required_attributes: Vec<String>,

    /// Required database queries
    pub database_queries: Vec<String>,

    /// Validation rules
    pub validation_rules: Vec<String>,
}

impl Default for TemplateGeneratorConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            max_cache_size: 100,
            template_extension: "dsl.template".to_string(),
            strict_validation: true,
        }
    }
}

impl TemplateGenerator {
    /// Create a new template generator
    pub fn new(templates_dir: PathBuf) -> Self {
        Self {
            templates_dir,
            template_cache: HashMap::new(),
            config: TemplateGeneratorConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(templates_dir: PathBuf, config: TemplateGeneratorConfig) -> Self {
        Self {
            templates_dir,
            template_cache: HashMap::new(),
            config,
        }
    }

    /// Load a template by ID
    async fn load_template(&mut self, template_id: &str) -> GenerationResult<Template> {
        // Check cache first
        if self.config.enable_caching {
            if let Some(template) = self.template_cache.get(template_id) {
                debug!("Template {} loaded from cache", template_id);
                return Ok(template.clone());
            }
        }

        // Find template file
        let template_path = self.find_template_file(template_id).await?;

        // Read and parse template
        let content =
            fs::read_to_string(&template_path)
                .await
                .map_err(|e| GenerationError::IoError {
                    message: format!(
                        "Failed to read template file {}: {}",
                        template_path.display(),
                        e
                    ),
                })?;

        let template = self.parse_template_content(&content)?;

        // Cache if enabled
        if self.config.enable_caching && self.template_cache.len() < self.config.max_cache_size {
            self.template_cache
                .insert(template_id.to_string(), template.clone());
        }

        Ok(template)
    }

    /// Find template file by ID
    async fn find_template_file(&self, template_id: &str) -> GenerationResult<PathBuf> {
        // Try different possible paths
        let possible_paths = vec![
            self.templates_dir.join(format!(
                "{}.{}",
                template_id, self.config.template_extension
            )),
            self.templates_dir.join("onboarding").join(format!(
                "{}.{}",
                template_id, self.config.template_extension
            )),
            self.templates_dir.join("kyc").join(format!(
                "{}.{}",
                template_id, self.config.template_extension
            )),
            self.templates_dir.join("ubo").join(format!(
                "{}.{}",
                template_id, self.config.template_extension
            )),
            self.templates_dir.join("compliance").join(format!(
                "{}.{}",
                template_id, self.config.template_extension
            )),
            self.templates_dir.join("documents").join(format!(
                "{}.{}",
                template_id, self.config.template_extension
            )),
            self.templates_dir.join("isda").join(format!(
                "{}.{}",
                template_id, self.config.template_extension
            )),
        ];

        for path in possible_paths {
            if path.exists() {
                debug!("Found template file: {}", path.display());
                return Ok(path);
            }
        }

        Err(GenerationError::TemplateNotFound {
            template_id: template_id.to_string(),
        })
    }

    /// Parse template content from file
    fn parse_template_content(&self, content: &str) -> GenerationResult<Template> {
        // Split YAML front matter from template content
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Err(GenerationError::TemplateRenderError {
                message: "Template file must have YAML front matter delimited by ---".to_string(),
            });
        }

        // Parse YAML front matter
        let yaml_content = parts[1];
        let template_content = parts[2].trim();

        let mut template: Template = serde_yaml::from_str(yaml_content).map_err(|e| {
            GenerationError::TemplateRenderError {
                message: format!("Failed to parse template YAML: {}", e),
            }
        })?;

        // Set the template content
        template.content = template_content.to_string();

        Ok(template)
    }

    /// Render template with context variables
    fn render_template(
        &self,
        template: &Template,
        variables: &HashMap<String, String>,
    ) -> GenerationResult<String> {
        let mut content = template.content.clone();

        // Add built-in variables
        let mut all_variables = variables.clone();
        all_variables.insert("timestamp".to_string(), Utc::now().to_rfc3339());
        all_variables.insert("request_id".to_string(), Uuid::new_v4().to_string());

        // Validate required variables
        for var in &template.variables {
            if var.required && !all_variables.contains_key(&var.name) {
                if let Some(default) = &var.default_value {
                    all_variables.insert(var.name.clone(), default.clone());
                } else {
                    return Err(GenerationError::MissingParameter {
                        parameter: var.name.clone(),
                    });
                }
            }
        }

        // Simple template variable substitution
        for (key, value) in &all_variables {
            let placeholder = format!("{{{{{}}}}}", key);
            content = content.replace(&placeholder, value);
        }

        // Check for unresolved placeholders if strict validation is enabled
        if self.config.strict_validation {
            if content.contains("{{") && content.contains("}}") {
                let unresolved: Vec<String> = content
                    .split("{{")
                    .skip(1)
                    .filter_map(|s| s.split("}}").next())
                    .map(|s| s.to_string())
                    .collect();

                if !unresolved.is_empty() {
                    warn!("Unresolved template variables: {:?}", unresolved);
                }
            }
        }

        Ok(content)
    }

    /// Map operation type to template ID
    fn operation_to_template_id(&self, operation_type: &GenerationOperationType) -> String {
        match operation_type {
            GenerationOperationType::CreateCbu => "create_cbu".to_string(),
            GenerationOperationType::RegisterEntity => "register_entity".to_string(),
            GenerationOperationType::CalculateUbo => "calculate_ubo".to_string(),
            GenerationOperationType::UpdateDsl => "update_dsl".to_string(),
            GenerationOperationType::KycWorkflow => "kyc_workflow".to_string(),
            GenerationOperationType::ComplianceCheck => "compliance_check".to_string(),
            GenerationOperationType::DocumentCatalog => "document_catalog".to_string(),
            GenerationOperationType::IsdaTrade => "isda_trade".to_string(),
            GenerationOperationType::Custom { operation_name } => operation_name.clone(),
        }
    }
}

#[async_trait]
impl DslGenerator for TemplateGenerator {
    async fn generate_dsl(
        &self,
        request: GenerationRequest,
    ) -> GenerationResult<GenerationResponse> {
        let start_time = std::time::Instant::now();

        info!(
            "Starting template-based DSL generation for operation: {}",
            request.operation_type
        );

        // Clone self to make it mutable for template loading
        let mut generator = self.clone();

        // Get template ID from operation type
        let template_id = generator.operation_to_template_id(&request.operation_type);

        // Load template
        let template = generator.load_template(&template_id).await?;

        // Prepare variables for rendering
        let mut variables = request.context.template_variables.clone();

        // Add context data
        if let Some(cbu_id) = &request.context.cbu_id {
            variables.insert("cbu_id".to_string(), cbu_id.clone());
        }

        if let Some(case_id) = &request.context.case_id {
            variables.insert("case_id".to_string(), case_id.clone());
        }

        // Add entity data
        for (key, value) in &request.context.entity_data {
            variables.insert(key.clone(), value.clone());
        }

        // Add request metadata
        for (key, value) in &request.metadata {
            variables.insert(format!("meta_{}", key), value.clone());
        }

        // Generate UUIDs for common fields if not provided
        if !variables.contains_key("ob_request_id") {
            variables.insert("ob_request_id".to_string(), Uuid::new_v4().to_string());
        }

        if !variables.contains_key("request_id") {
            variables.insert("request_id".to_string(), request.request_id.to_string());
        }

        // Render template
        let dsl_content = generator.render_template(&template, &variables)?;

        let processing_time = start_time.elapsed().as_millis() as u64;

        info!(
            "Template-based DSL generation completed in {}ms",
            processing_time
        );

        Ok(GenerationResponse {
            success: true,
            dsl_content,
            method_used: GenerationMethod::Template {
                template_id: template.template_id.clone(),
            },
            processing_time_ms: processing_time,
            confidence_score: Some(1.0), // Template generation is deterministic
            errors: vec![],
            warnings: vec![],
            debug_info: if request.options.include_debug_info {
                Some(super::traits::GenerationDebugInfo {
                    template_id: Some(template.template_id.clone()),
                    ai_model: None,
                    resolved_variables: variables.clone(),
                    generation_steps: vec![
                        "Load template".to_string(),
                        "Prepare variables".to_string(),
                        "Render template".to_string(),
                    ],
                    performance_metrics: {
                        let mut metrics = HashMap::new();
                        metrics.insert("processing_time_ms".to_string(), processing_time as f64);
                        metrics.insert(
                            "template_size_bytes".to_string(),
                            template.content.len() as f64,
                        );
                        metrics.insert("variables_count".to_string(), variables.len() as f64);
                        metrics
                    },
                })
            } else {
                None
            },
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("generator_type".to_string(), "template".to_string());
                metadata.insert("template_id".to_string(), template.template_id.clone());
                metadata.insert("template_domain".to_string(), template.domain.clone());
                metadata
            },
        })
    }

    fn can_handle(&self, operation_type: &GenerationOperationType) -> bool {
        // Template generator can theoretically handle any operation type if a template exists
        // In practice, we should check if the template file exists
        match operation_type {
            GenerationOperationType::CreateCbu
            | GenerationOperationType::RegisterEntity
            | GenerationOperationType::CalculateUbo
            | GenerationOperationType::UpdateDsl
            | GenerationOperationType::KycWorkflow
            | GenerationOperationType::ComplianceCheck
            | GenerationOperationType::DocumentCatalog
            | GenerationOperationType::IsdaTrade => true,
            GenerationOperationType::Custom { .. } => true, // Assume custom templates can exist
        }
    }

    fn metadata(&self) -> GeneratorMetadata {
        GeneratorMetadata {
            name: "TemplateGenerator".to_string(),
            version: "1.0.0".to_string(),
            supported_operations: vec![
                GenerationOperationType::CreateCbu,
                GenerationOperationType::RegisterEntity,
                GenerationOperationType::CalculateUbo,
                GenerationOperationType::UpdateDsl,
                GenerationOperationType::KycWorkflow,
                GenerationOperationType::ComplianceCheck,
                GenerationOperationType::DocumentCatalog,
                GenerationOperationType::IsdaTrade,
            ],
            average_confidence: 1.0,        // Templates are deterministic
            average_processing_time_ms: 50, // Very fast
            available: true,
            capabilities: {
                let mut caps = HashMap::new();
                caps.insert("deterministic".to_string(), "true".to_string());
                caps.insert("offline".to_string(), "true".to_string());
                caps.insert("fast".to_string(), "true".to_string());
                caps.insert("template_based".to_string(), "true".to_string());
                caps
            },
        }
    }

    async fn health_check(&self) -> GenerationResult<bool> {
        // Check if templates directory exists and is readable
        if !self.templates_dir.exists() {
            return Ok(false);
        }

        if !self.templates_dir.is_dir() {
            return Ok(false);
        }

        // Try to read the directory
        match fs::read_dir(&self.templates_dir).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    fn validate_request(&self, request: &GenerationRequest) -> GenerationResult<()> {
        // Basic validation
        if request.options.timeout_ms == 0 {
            return Err(GenerationError::ValidationError {
                errors: vec!["Timeout must be greater than 0".to_string()],
            });
        }

        // Check if we can handle this operation type
        if !self.can_handle(&request.operation_type) {
            return Err(GenerationError::ValidationError {
                errors: vec![format!(
                    "Operation type {:?} is not supported by template generator",
                    request.operation_type
                )],
            });
        }

        Ok(())
    }
}

impl Template {
    /// Check if all required variables are provided
    pub fn validate_variables(&self, variables: &HashMap<String, String>) -> Vec<String> {
        let mut errors = vec![];

        for var in &self.variables {
            if var.required && !variables.contains_key(&var.name) && var.default_value.is_none() {
                errors.push(format!("Missing required variable: {}", var.name));
            }
        }

        errors
    }

    /// Get all variable names (required and optional)
    pub fn get_variable_names(&self) -> Vec<String> {
        self.variables.iter().map(|v| v.name.clone()).collect()
    }

    /// Get only required variable names
    pub fn get_required_variables(&self) -> Vec<String> {
        self.variables
            .iter()
            .filter(|v| v.required && v.default_value.is_none())
            .map(|v| v.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::generation::{GenerationContext, GenerationOperationType, GenerationRequest};
    use tempfile::tempdir;
    use tokio::fs;

    async fn create_test_template() -> (tempfile::TempDir, PathBuf) {
        let temp_dir = tempdir().unwrap();
        let template_path = temp_dir.path().join("test_template.dsl.template");

        let template_content = r#"---
template_id: "test_template"
domain: "test"
template_type: "TestType"
description: "Test template"
variables:
  - name: "test_var"
    type: "string"
    required: true
    description: "Test variable"
  - name: "optional_var"
    type: "string"
    required: false
    description: "Optional variable"
    default_value: "default"
requirements:
  prerequisite_operations: []
  required_attributes: ["test_var"]
  database_queries: []
  validation_rules: []
---

(test.operation
  (var "{{test_var}}")
  (optional "{{optional_var}}")
  (timestamp "{{timestamp}}")
)
"#;

        fs::write(&template_path, template_content).await.unwrap();
        (temp_dir, template_path)
    }

    #[tokio::test]
    async fn test_template_generator_creation() {
        let temp_dir = tempdir().unwrap();
        let generator = TemplateGenerator::new(temp_dir.path().to_path_buf());

        assert_eq!(generator.templates_dir, temp_dir.path());
        assert!(generator.template_cache.is_empty());
    }

    #[tokio::test]
    async fn test_template_loading() {
        let (_temp_dir, template_path) = create_test_template().await;
        let templates_dir = template_path.parent().unwrap().to_path_buf();

        let mut generator = TemplateGenerator::new(templates_dir);
        let template = generator.load_template("test_template").await.unwrap();

        assert_eq!(template.template_id, "test_template");
        assert_eq!(template.domain, "test");
        assert_eq!(template.variables.len(), 2);
    }

    #[tokio::test]
    async fn test_template_rendering() {
        let (_temp_dir, template_path) = create_test_template().await;
        let templates_dir = template_path.parent().unwrap().to_path_buf();

        let mut generator = TemplateGenerator::new(templates_dir);
        let template = generator.load_template("test_template").await.unwrap();

        let mut variables = HashMap::new();
        variables.insert("test_var".to_string(), "test_value".to_string());

        let rendered = generator.render_template(&template, &variables).unwrap();

        assert!(rendered.contains("test_value"));
        assert!(rendered.contains("default")); // Default value for optional_var
        assert!(rendered.contains("(test.operation"));
    }

    #[tokio::test]
    async fn test_dsl_generation() {
        let (_temp_dir, template_path) = create_test_template().await;
        let templates_dir = template_path.parent().unwrap().to_path_buf();

        let generator = TemplateGenerator::new(templates_dir);

        let mut context = GenerationContext::default();
        context
            .template_variables
            .insert("test_var".to_string(), "test_value".to_string());

        let request = GenerationRequest::new(
            GenerationOperationType::Custom {
                operation_name: "test_template".to_string(),
            },
            context,
        );

        let response = generator.generate_dsl(request).await.unwrap();

        assert!(response.success);
        assert!(response.dsl_content.contains("test_value"));
        assert_eq!(response.confidence_score, Some(1.0));
    }

    #[tokio::test]
    async fn test_health_check() {
        let temp_dir = tempdir().unwrap();
        let generator = TemplateGenerator::new(temp_dir.path().to_path_buf());

        let health = generator.health_check().await.unwrap();
        assert!(health);

        // Test with non-existent directory
        let non_existent = PathBuf::from("/non/existent/path");
        let bad_generator = TemplateGenerator::new(non_existent);
        let bad_health = bad_generator.health_check().await.unwrap();
        assert!(!bad_health);
    }
}
