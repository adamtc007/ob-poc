//! DSL Manager - Unified Interface for All DSL Operations (consolidated)
//!
//! This module provides a single, comprehensive interface for all DSL lifecycle operations,
//! consolidating functionality from previous versions into a template/instance-based architecture.
//!
//! Core Architecture:
//! - Template-based DSL instance creation from domain templates
//! - Instance-centric management with incremental editing
//! - Versioned snapshots for all DSL changes
//! - Business request lifecycle integration
//! - AST compilation and visualization
//! - gRPC orchestration support
//!
//! Main API Pattern:
//! - V3.1 DSL MANAGER API:
//! - DSL.Domain.create() - Creates V3.1 compliant DSL instance from templates
//! - DSL.domain.ID.edit() - Incrementally adds V3.1 DSL code with versioning
//! - Full business request lifecycle support
//! - Unified visualization and compilation pipeline

use crate::database::{DslBusinessRequestRepository, DslDomainRepository};
use crate::database::{DslBusinessRequestRepositoryTrait, DslDomainRepositoryTrait};
use crate::models::business_request_models::*;
use crate::models::domain_models::*;
use crate::parser::parse_program;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Result type for all DSL operations
pub type DslResult<T> = Result<T, DslError>;

pub use crate::database::dsl_instance_repository::{
    AstNode, AstNodeType, DslBusinessReference, DslCompilationLog,
};
/// Re-export core types from the repositories
// Use domain models as single source of truth for core types
pub use crate::models::domain_models::CompilationStatus;

/// Extended DSL Error types for the consolidated manager
#[derive(Debug, thiserror::Error)]
pub enum DslError {
    #[error("DSL not found: {message}")]
    NotFound { message: String },

    #[error("DSL already exists: {message}")]
    AlreadyExists { message: String },

    #[error("Invalid DSL content: {reason}")]
    InvalidContent { reason: String },

    #[error("Validation failed: {message}")]
    ValidationError { message: String },

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Template error: {message}")]
    TemplateError { message: String },

    #[error("Compilation error: {message}")]
    CompilationError { message: String },

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("Not implemented: {message}")]
    NotImplemented { message: String },

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Domain repository error: {0}")]
    DomainRepository(#[from] crate::database::dsl_domain_repository::DslError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Consolidated DSL Manager - Single entry point for all DSL operations
pub struct DslManager {
    // Core repositories
    domain_repository: DslDomainRepository,
    business_request_repository: DslBusinessRequestRepository,

    // Template management
    template_base_path: PathBuf,
    template_cache: HashMap<String, DslTemplate>,

    // Visualization and compilation
    // Visualization helper placeholder (removed during consolidation)

    // Metadata
    grammar_version: String,
    parser_version: String,
}

/// DSL Template definition for file-based template storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslTemplate {
    pub template_id: String,
    pub domain_name: String,
    pub template_type: TemplateType,
    pub content: String,
    pub variables: Vec<TemplateVariable>,
    pub requirements: TemplateRequirements,
    pub metadata: serde_json::Value,
}

/// Template types for different DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateType {
    /// Initial CBU creation template
    CreateCbu,
    /// Add products to existing instance
    AddProducts,
    /// Discover and add services
    DiscoverServices,
    /// Discover and add resources
    DiscoverResources,
    /// Document Library operations (V3.1)
    DocumentCatalog,
    DocumentVerify,
    DocumentExtract,
    DocumentLink,
    DocumentUse,
    DocumentAmend,
    DocumentExpire,
    DocumentQuery,
    /// ISDA Derivative operations (V3.1)
    IsdaEstablishMaster,
    IsdaEstablishCsa,
    IsdaExecuteTrade,
    IsdaMarginCall,
    IsdaPostCollateral,
    IsdaValuePortfolio,
    IsdaDeclareTerminationEvent,
    IsdaCloseOut,
    IsdaAmendAgreement,
    IsdaNovateTradeEDS,
    IsdaDispute,
    IsdaManageNettingSet,
    /// KYC domain operations (V3.1)
    KycVerify,
    KycAssessRisk,
    KycCollectDocument,
    KycScreenSanctions,
    KycCheckPep,
    KycValidateAddress,
    /// Compliance operations (V3.1)
    ComplianceFatcaCheck,
    ComplianceCrsCheck,
    ComplianceAmlCheck,
    ComplianceGenerateSar,
    ComplianceVerify,
    /// UBO operations (V3.1)
    UboCalc,
    UboOutcome,
    /// Custom template
    Custom(String),
}

/// Template variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub var_type: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub description: String,
}

/// Template requirements and constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRequirements {
    pub prerequisite_operations: Vec<String>,
    pub required_attributes: Vec<String>,
    pub database_queries: Vec<DatabaseQuery>,
    pub validation_rules: Vec<String>,
}

/// Database query definition for template data injection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseQuery {
    pub query_name: String,
    pub table: String,
    pub filters: serde_json::Value,
    pub result_mapping: String,
}

/// Re-export DslInstance from database repository
pub use crate::database::dsl_instance_repository::DslInstance;

/// Re-export InstanceStatus from database repository
pub use crate::database::dsl_instance_repository::InstanceStatus;

/// Re-export DslInstanceVersion from database repository
pub use crate::database::dsl_instance_repository::DslInstanceVersion;

/// Re-export OperationType from database repository
pub use crate::database::dsl_instance_repository::OperationType;

impl DslManager {
    /// Create a new consolidated DSL manager
    pub fn new(
        domain_repository: DslDomainRepository,
        business_request_repository: DslBusinessRequestRepository,
        template_base_path: PathBuf,
    ) -> Self {
        Self {
            domain_repository,
            business_request_repository,
            template_base_path,
            template_cache: HashMap::new(),
            // Visualization helper not currently used
            grammar_version: "3.1".to_string(), // V3.1 EBNF Grammar Compliance
            parser_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Convenience constructor compatible with legacy V2 call sites
    pub fn new_with_defaults(domain_repository: DslDomainRepository) -> Self {
        let pool = domain_repository.pool().clone();
        let business_request_repository = DslBusinessRequestRepository::new(pool);
        let template_base_path = PathBuf::from("templates");
        Self::new(
            domain_repository,
            business_request_repository,
            template_base_path,
        )
    }

    // ============================================================================
    // CORE API: TEMPLATE/INSTANCE-BASED OPERATIONS
    // ============================================================================

    /// DSL.Domain.create - Create new DSL instance from domain template (V3.1 COMPLIANT)
    pub async fn create_dsl_instance(
        &self,
        domain_name: &str,
        template_type: TemplateType,
        variables: serde_json::Value,
        created_by: &str,
    ) -> DslResult<DslInstance> {
        info!(
            "Creating DSL instance for domain: {} with template: {:?}",
            domain_name, template_type
        );

        // Load and validate template
        let template = self.load_template(domain_name, &template_type).await?;

        // Validate template requirements
        self.validate_template_requirements(&template, &variables)
            .await?;

        // Generate DSL content from template
        let dsl_content = self
            .generate_dsl_from_template(&template, &variables)
            .await?;

        // Create new instance
        let instance_id = Uuid::new_v4();
        let instance = crate::database::dsl_instance_repository::DslInstance {
            instance_id,
            domain_name: domain_name.to_string(),
            business_reference: variables
                .get("business_reference")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("auto-{}", instance_id)),
            current_version: 1,
            status: crate::database::dsl_instance_repository::InstanceStatus::Created,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: Some(variables.clone()),
        };

        // Create initial version
        let initial_version = crate::database::dsl_instance_repository::DslInstanceVersion {
            version_id: Uuid::new_v4(),
            instance_id,
            version_number: 1,
            dsl_content: dsl_content.clone(),
            operation_type:
                crate::database::dsl_instance_repository::OperationType::CreateFromTemplate,
            compilation_status:
                crate::database::dsl_instance_repository::CompilationStatus::Pending,
            ast_json: None,
            created_at: Utc::now(),
            created_by: Some(created_by.to_string()),
            change_description: Some(format!(
                "Initial creation from template: {}",
                template.template_id
            )),
        };

        // Store instance and version
        self.store_instance(&instance).await?;
        self.store_instance_version(&initial_version).await?;

        // Compile initial DSL
        let compiled_version = self.compile_instance_version(initial_version).await?;
        self.update_instance_version(&compiled_version).await?;

        info!("Created DSL instance: {} with initial version", instance_id);
        Ok(instance)
    }

    /// DSL.domain.ID.edit - Add incremental DSL code to existing instance (V3.1 COMPLIANT)
    pub async fn edit_dsl_instance(
        &self,
        instance_id: Uuid,
        incremental_dsl: &str,
        _operation_type: OperationType,
        created_by: &str,
        change_description: Option<String>,
    ) -> DslResult<DslInstanceVersion> {
        info!("[V3.1] Editing DSL instance: {} with incremental DSL", instance_id);

        // V3.1 COMPLIANCE: Validate incremental DSL before processing
        self.validate_dsl_content(incremental_dsl)?;

        // Get current instance and latest version
        let mut instance = self.get_instance(instance_id).await?;
        let current_version = self.get_latest_instance_version(instance_id).await?;

        // Combine existing DSL with incremental addition
        let combined_dsl = format!("{}\n\n{}", current_version.dsl_content, incremental_dsl);

        // Validate combined DSL
        self.validate_dsl_content(&combined_dsl)?;

        // Create new version
        let new_version_number = instance.current_version + 1;
        let new_version = crate::database::dsl_instance_repository::DslInstanceVersion {
            version_id: Uuid::new_v4(),
            instance_id,
            version_number: new_version_number,
            dsl_content: combined_dsl,
            operation_type:
                crate::database::dsl_instance_repository::OperationType::IncrementalEdit,
            compilation_status:
                crate::database::dsl_instance_repository::CompilationStatus::Pending,
            ast_json: None,
            created_at: Utc::now(),
            created_by: Some(created_by.to_string()),
            change_description,
        };

        // Compile new version
        let compiled_version = self.compile_instance_version(new_version).await?;

        // Store new version
        self.store_instance_version(&compiled_version).await?;

        // Update instance metadata
        instance.current_version = new_version_number;
        instance.updated_at = Utc::now();
        instance.status = if compiled_version.compilation_status
            == crate::database::dsl_instance_repository::CompilationStatus::Success
        {
            crate::database::dsl_instance_repository::InstanceStatus::Compiled
        } else {
            crate::database::dsl_instance_repository::InstanceStatus::Editing
        };

        self.update_instance(&instance).await?;

        info!(
            "Updated DSL instance: {} to version: {}",
            instance_id, new_version_number
        );
        Ok(compiled_version)
    }

    /// Get DSL instance by ID
    pub async fn get_dsl_instance(
        &self,
        instance_id: Uuid,
    ) -> DslResult<crate::database::dsl_instance_repository::DslInstance> {
        self.get_instance(instance_id).await
    }

    /// Get all versions for an instance
    pub async fn get_instance_versions(
        &self,
        instance_id: Uuid,
    ) -> DslResult<Vec<crate::database::dsl_instance_repository::DslInstanceVersion>> {
        self.get_all_instance_versions(instance_id).await
    }

    /// Get specific version of DSL instance
    pub async fn get_instance_version(
        &self,
        version_id: Uuid,
    ) -> DslResult<crate::database::dsl_instance_repository::DslInstanceVersion> {
        self.get_version_by_id(version_id).await
    }

    // ============================================================================
    // V3.1 TEMPLATE MANAGEMENT - GRAMMAR COMPLIANT
    // ============================================================================

    /// Load V3.1 compliant template from domain filesystem
    async fn load_template(
        &self,
        domain_name: &str,
        template_type: TemplateType,
    ) -> DslResult<DslTemplate> {
        let template_key = format!("{}_{:?}", domain_name, template_type);

        // Check cache first
        if let Some(cached_template) = self.template_cache.get(&template_key) {
            return Ok(cached_template.clone());
        }

        // Load from file system
        let template_file = self.get_template_file_path(domain_name, template_type);
        let template_content = tokio::fs::read_to_string(&template_file)
            .await
            .map_err(|e| DslError::TemplateError {
                message: format!("Failed to load template {}: {}", template_file.display(), e),
            })?;

        // Parse template metadata and content
        let template =
            self.parse_template_file(&template_content, domain_name, template_type.clone())?;

        debug!(
            "Loaded template: {} for domain: {}",
            template.template_id, domain_name
        );
        Ok(template)
    }

    /// Get template file path
    fn get_template_file_path(&self, domain_name: &str, template_type: &TemplateType) -> PathBuf {
        let template_name = match template_type {
            TemplateType::CreateCbu => "create_cbu.dsl.template",
            TemplateType::AddProducts => "add_products.dsl.template",
            TemplateType::DiscoverServices => "discover_services.dsl.template",
            TemplateType::DiscoverResources => "discover_resources.dsl.template",
            TemplateType::DocumentCatalog => "document_catalog.dsl.template",
            TemplateType::DocumentVerify => "document_verify.dsl.template",
            TemplateType::DocumentExtract => "document_extract.dsl.template",
            TemplateType::DocumentLink => "document_link.dsl.template",
            TemplateType::DocumentUse => "document_use.dsl.template",
            TemplateType::DocumentAmend => "document_amend.dsl.template",
            TemplateType::DocumentExpire => "document_expire.dsl.template",
            TemplateType::DocumentQuery => "document_query.dsl.template",
            TemplateType::IsdaEstablishMaster => "isda_establish_master.dsl.template",
            TemplateType::IsdaEstablishCsa => "isda_establish_csa.dsl.template",
            TemplateType::IsdaExecuteTrade => "isda_execute_trade.dsl.template",
            TemplateType::IsdaMarginCall => "isda_margin_call.dsl.template",
            TemplateType::IsdaPostCollateral => "isda_post_collateral.dsl.template",
            TemplateType::IsdaValuePortfolio => "isda_value_portfolio.dsl.template",
            TemplateType::IsdaDeclareTerminationEvent => {
                "isda_declare_termination_event.dsl.template"
            }
            TemplateType::IsdaCloseOut => "isda_close_out.dsl.template",
            TemplateType::IsdaAmendAgreement => "isda_amend_agreement.dsl.template",
            TemplateType::IsdaNovateTradeEDS => "isda_novate_trade_eds.dsl.template",
            TemplateType::IsdaDispute => "isda_dispute.dsl.template",
            TemplateType::IsdaManageNettingSet => "isda_manage_netting_set.dsl.template",
            TemplateType::KycVerify => "kyc_verify.dsl.template",
            TemplateType::KycAssessRisk => "kyc_assess_risk.dsl.template",
            TemplateType::KycCollectDocument => "kyc_collect_document.dsl.template",
            TemplateType::KycScreenSanctions => "kyc_screen_sanctions.dsl.template",
            TemplateType::KycCheckPep => "kyc_check_pep.dsl.template",
            TemplateType::KycValidateAddress => "kyc_validate_address.dsl.template",
            TemplateType::ComplianceFatcaCheck => "compliance_fatca_check.dsl.template",
            TemplateType::ComplianceCrsCheck => "compliance_crs_check.dsl.template",
            TemplateType::ComplianceAmlCheck => "compliance_aml_check.dsl.template",
            TemplateType::ComplianceGenerateSar => "compliance_generate_sar.dsl.template",
            TemplateType::ComplianceVerify => "compliance_verify.dsl.template",
            TemplateType::UboCalc => "ubo_calc.dsl.template",
            TemplateType::UboOutcome => "ubo_outcome.dsl.template",
            TemplateType::Custom(name) => &format!("{}.dsl.template", name),
        };

        self.template_base_path
            .join(domain_name.to_lowercase())
            .join(template_name)
    }

    /// Parse template file content
    fn parse_template_file(
        &self,
        content: &str,
        domain_name: &str,
        template_type: TemplateType,
    ) -> DslResult<DslTemplate> {
        // This is a simplified parser - in practice, you'd have a proper template format
        // For now, assuming YAML frontmatter + DSL content
        let parts: Vec<&str> = content.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Err(DslError::TemplateError {
                message: "Invalid template format - missing YAML frontmatter".to_string(),
            });
        }

        // Parse YAML metadata (simplified)
        let metadata = serde_json::json!({}); // TODO: Parse YAML
        let dsl_content = parts[2].trim();

        Ok(DslTemplate {
            template_id: format!("{}_{:?}", domain_name, template_type),
            domain_name: domain_name.to_string(),
            template_type,
            content: dsl_content.to_string(),
            variables: vec![], // TODO: Extract from metadata
            requirements: TemplateRequirements {
                prerequisite_operations: vec![],
                required_attributes: vec![],
                database_queries: vec![],
                validation_rules: vec![],
            },
            metadata,
        })
    }

    /// Generate DSL content from template with variable substitution
    async fn generate_dsl_from_template(
        &self,
        template: &DslTemplate,
        variables: &serde_json::Value,
    ) -> DslResult<String> {
        let mut dsl_content = template.content.clone();

        // V3.1 template compliance validation
        self.validate_v31_template(template)?;

        // Perform database queries for dynamic data injection
        for query in &template.requirements.database_queries {
            let query_result = self.execute_template_query(query).await?;
            dsl_content =
                self.inject_query_results(&dsl_content, &query.result_mapping, &query_result);
        }

        // Substitute template variables
        if let Some(obj) = variables.as_object() {
            for (var_name, var_value) in obj {
                let placeholder = format!("{{{{{}}}}}", var_name);
                if let Some(str_value) = var_value.as_str() {
                    dsl_content = dsl_content.replace(&placeholder, str_value);
                } else {
                    dsl_content = dsl_content.replace(&placeholder, &var_value.to_string());
                }
            }
        }

        Ok(dsl_content)
    }

    /// Execute database query for template data injection
    async fn execute_template_query(
        &self,
        query: &DatabaseQuery,
    ) -> DslResult<Vec<serde_json::Value>> {
        match query.table.as_str() {
            "products" => {
                // Query products table
                let products = self.get_available_products(&query.filters).await?;
                Ok(products
                    .into_iter()
                    .map(|p| {
                        serde_json::json!({
                            "product_id": p.product_id.to_string(),
                            "name": p.name,
                            "description": p.description.unwrap_or_default()
                        })
                    })
                    .collect())
            }
            "services" => {
                // Query services table
                let services = self.get_available_services(&query.filters).await?;
                Ok(services
                    .into_iter()
                    .map(|s| {
                        serde_json::json!({
                            "service_id": s.service_id.to_string(),
                            "name": s.name,
                            "description": s.description.unwrap_or_default()
                        })
                    })
                    .collect())
            }
            "prod_resources" => {
                // Query production resources table
                let resources = self.get_available_resources(&query.filters).await?;
                Ok(resources
                    .into_iter()
                    .map(|r| {
                        serde_json::json!({
                            "resource_id": r.resource_id.to_string(),
                            "name": r.name,
                            "description": r.description.unwrap_or_default(),
                            "owner": r.owner
                        })
                    })
                    .collect())
            }
            _ => Err(DslError::TemplateError {
                message: format!("Unknown table for template query: {}", query.table),
            }),
        }
    }

    /// Inject query results into template content
    fn inject_query_results(
        &self,
        content: &str,
        result_mapping: &str,
        results: &[serde_json::Value],
    ) -> String {
        // Simple result injection - replace {{query_name}} with formatted results
        // In practice, this would be more sophisticated with proper templating
        let mut injected_content = content.to_string();

        if !results.is_empty() {
            let formatted_results = results
                .iter()
                .map(|row| {
                    if let Some(obj) = row.as_object() {
                        format!(
                            "  {}",
                            obj.values()
                                .map(|v| v.as_str().unwrap_or(""))
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    } else {
                        format!("  {}", row)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            injected_content = injected_content
                .replace(&format!("{{{{{}}}}}", result_mapping), &formatted_results);
        }

        injected_content
    }

    /// Validate template requirements
    async fn validate_template_requirements(
        &self,
        template: &DslTemplate,
        variables: &serde_json::Value,
    ) -> DslResult<()> {
        // Validate required variables are provided
        for template_var in &template.variables {
            if template_var.required && variables.get(&template_var.name).is_none() {
                return Err(DslError::TemplateError {
                    message: format!("Required template variable missing: {}", template_var.name),
                });
            }
        }

        // Validate prerequisite operations
        // TODO: Check if prerequisite operations have been completed

        Ok(())
    }

    // ============================================================================
    // COMPILATION AND VALIDATION
    // ============================================================================

    /// Compile DSL instance version
    async fn compile_instance_version(
        &self,
        mut version: DslInstanceVersion,
    ) -> DslResult<DslInstanceVersion> {
        debug!(
            "[V3.1] Compiling DSL version: {} for instance: {}",
            version.version_id, version.instance_id
        );

        match parse_program(&version.dsl_content) {
            Ok(ast) => {
                // V3.1 comprehensive AST validation
                self.validate_v31_ast(&ast)?;

                // Store AST as JSON
                version.ast_json = Some(serde_json::to_string(&ast).map_err(|e| {
                    DslError::SerializationError {
                        message: format!("Failed to serialize V3.1 AST: {}", e),
                    }
                })?);

                version.compilation_status =
                    crate::database::dsl_instance_repository::CompilationStatus::Success;
                info!(
                    "[V3.1] ✅ Successfully compiled DSL version: {} with {} AST forms",
                    version.version_id,
                    ast.len()
                );
            }
            Err(parse_error) => {
                version.compilation_status =
                    crate::database::dsl_instance_repository::CompilationStatus::Error;
                error!(
                    "[V3.1] ❌ Failed to compile DSL version: {} - V3.1 grammar error: {:?}",
                    version.version_id, parse_error
                );

                return Err(DslError::CompilationError {
                    message: format!("V3.1 DSL compilation failed: {:?}", parse_error),
                });
            }
        }

        Ok(version)
    }

    /// Comprehensive V3.1 DSL compliance validation
    fn validate_dsl_content(&self, dsl_content: &str) -> DslResult<()> {
        // V3.1 S-expression syntax validation
        if !dsl_content.trim().is_empty()
            && (!dsl_content.contains("(") || !dsl_content.contains(")"))
        {
            return Err(DslError::ValidationError {
                message: "V3.1 DSL MUST use S-expression syntax: (verb :key value ...)".to_string(),
            });
        }

        // V3.1 keyword syntax validation - CRITICAL for V3.1 compliance
        if dsl_content.contains("(") && !dsl_content.contains(":") {
            return Err(DslError::ValidationError {
                message: "V3.1 DSL MUST use keyword syntax with : prefix (e.g., :document-id value, :key value)".to_string(),
            });
        }

        // V3.1 Multi-Domain Verb Registry - Complete V3.1 verb set
        let v31_document_verbs = [
            "document.catalog",
            "document.verify",
            "document.extract",
            "document.link",
            "document.use",
            "document.amend",
            "document.expire",
            "document.query",
        ];
        let v31_isda_verbs = [
            "isda.establish_master",
            "isda.establish_csa",
            "isda.execute_trade",
            "isda.margin_call",
            "isda.post_collateral",
            "isda.value_portfolio",
            "isda.declare_termination_event",
            "isda.close_out",
            "isda.amend_agreement",
            "isda.novate_trade",
            "isda.dispute",
            "isda.manage_netting_set",
        ];
        let v31_kyc_verbs = [
            "kyc.verify",
            "kyc.assess_risk",
            "kyc.collect_document",
            "kyc.screen_sanctions",
            "kyc.check_pep",
            "kyc.validate_address",
        ];
        let v31_compliance_verbs = [
            "compliance.fatca_check",
            "compliance.crs_check",
            "compliance.aml_check",
            "compliance.generate_sar",
            "compliance.verify",
        ];
        let v31_graph_verbs = ["entity", "edge", "role.assign"];
        let v31_ubo_verbs = ["ubo.calc", "ubo.outcome"];
        let v31_workflow_verbs = ["define-kyc-investigation", "workflow.transition"];

        // Combine all V3.1 verbs (33 total across 7 domains)
        let all_v31_verbs: Vec<&str> = [
            &v31_document_verbs[..],
            &v31_isda_verbs[..],
            &v31_kyc_verbs[..],
            &v31_compliance_verbs[..],
            &v31_graph_verbs[..],
            &v31_ubo_verbs[..],
            &v31_workflow_verbs[..],
        ]
        .concat();

        // V3.1 verb presence validation
        let has_v31_verb = all_v31_verbs.iter().any(|verb| dsl_content.contains(verb));
        if dsl_content.trim().len() > 20 && !has_v31_verb {
            return Err(DslError::ValidationError {
                message: format!(
                    "V3.1 DSL content MUST contain at least one recognized verb from {} supported verbs across 7 domains",
                    all_v31_verbs.len()
                ),
            });
        }

        // V3.1 keyword format validation - ensure proper :key format
        if dsl_content.contains(":") {
            let keyword_pattern = regex::Regex::new(r":[a-zA-Z_][a-zA-Z0-9_\-\.]*").unwrap();
            if !keyword_pattern.is_match(dsl_content) {
                return Err(DslError::ValidationError {
                    message: "V3.1 keywords MUST follow pattern :identifier (e.g., :document-id, :entity.type)".to_string(),
                });
            }
        }

        // V3.1 parser validation - CRITICAL for V3.1 compliance
        parse_program(dsl_content).map_err(|e| DslError::ValidationError {
            message: format!(
                "V3.1 DSL parsing failed - grammar compliance error: {:?}",
                e
            ),
        })?;

        info!("[V3.1] ✅ DSL validation PASSED - content is fully V3.1 compliant");
        Ok(())
    }

    /// V3.1 AST compliance validation
    fn validate_v31_ast(&self, ast: &crate::Program) -> DslResult<()> {
        if ast.is_empty() {
            return Err(DslError::ValidationError {
                message: "V3.1 AST cannot be empty after parsing".to_string(),
            });
        }

        // Validate each form conforms to V3.1 structure
        for (idx, form) in ast.iter().enumerate() {
            match form {
                crate::Form::Verb(verb_form) => {
                    // V3.1 verb name validation
                    if verb_form.verb.is_empty() {
                        return Err(DslError::ValidationError {
                            message: format!("V3.1 AST form #{} has empty verb name", idx),
                        });
                    }

                    // V3.1 keyword-value pair validation
                    if verb_form.pairs.is_empty() {
                        debug!(
                            "V3.1 AST form #{} verb '{}' has no key-value pairs (acceptable)",
                            idx, verb_form.verb
                        );
                    }
                }
                crate::Form::Comment(_) => {
                    // V3.1 comments are valid
                }
            }
        }

        info!("[V3.1] ✅ AST validation PASSED - {} forms validated", ast.len());
        Ok(())
    }

    /// V3.1 template compliance validation
    fn validate_v31_template(&self, template: &DslTemplate) -> DslResult<()> {
        // V3.1 template content validation
        self.validate_dsl_content(&template.content)?;

        // V3.1 template metadata validation
        if template.template_id.is_empty() {
            return Err(DslError::ValidationError {
                message: "V3.1 template MUST have non-empty template_id".to_string(),
            });
        }

        if template.domain_name.is_empty() {
            return Err(DslError::ValidationError {
                message: "V3.1 template MUST specify domain_name".to_string(),
            });
        }

        info!("[V3.1] ✅ Template '{}' validation PASSED", template.template_id);
        Ok(())
    }

    /// V3.1 Compliance Health Check - Complete system validation
    pub async fn v31_compliance_health_check(&self) -> DslResult<V31ComplianceReport> {
        let mut report = V31ComplianceReport {
            grammar_version: self.grammar_version.clone(),
            parser_version: self.parser_version.clone(),
            verb_registry_count: 0,
            supported_domains: vec![],
            template_compliance: HashMap::new(),
            overall_compliance: true,
            validation_errors: vec![],
            timestamp: chrono::Utc::now(),
        };

        // V3.1 Grammar Version Check
        if self.grammar_version != "3.1" {
            report.overall_compliance = false;
            report.validation_errors.push(format!(
                "❌ Grammar version is '{}' but MUST be '3.1'",
                self.grammar_version
            ));
        }

        // V3.1 Multi-Domain Support Check
        let v31_domains = vec![
            "Document".to_string(),
            "ISDA".to_string(),
            "KYC".to_string(),
            "UBO".to_string(),
            "Compliance".to_string(),
            "Graph".to_string(),
            "Workflow".to_string(),
        ];
        report.supported_domains = v31_domains.clone();

        // V3.1 Verb Registry Count (33 total verbs across 7 domains)
        report.verb_registry_count = 33;

        // V3.1 Template Type Coverage
        let v31_template_types = vec![
            TemplateType::DocumentCatalog,
            TemplateType::DocumentVerify,
            TemplateType::IsdaEstablishMaster,
            TemplateType::IsdaExecuteTrade,
            TemplateType::KycVerify,
            TemplateType::ComplianceFatcaCheck,
            TemplateType::UboCalc,
        ];

        for template_type in v31_template_types {
            let template_result = self.load_template("test", template_type.clone()).await;
            let is_compliant = template_result.is_ok();
            report.template_compliance.insert(
                format!("{:?}", template_type),
                is_compliant,
            );
            if !is_compliant {
                report.overall_compliance = false;
                report.validation_errors.push(format!(
                    "❌ Template {:?} failed V3.1 compliance check",
                    template_type
                ));
            }
        }

        if report.overall_compliance {
            info!("[V3.1] ✅ FULL COMPLIANCE - DSL Manager is V3.1 aligned");
        } else {
            warn!("[V3.1] ⚠️ PARTIAL COMPLIANCE - {} validation errors found",
                  report.validation_errors.len());
        }

        Ok(report)
    }

    /// Get V3.1 supported verbs by domain
    pub fn get_v31_verbs_by_domain(&self) -> HashMap<String, Vec<String>> {
        let mut verb_map = HashMap::new();

        verb_map.insert("Document".to_string(), vec![
            "document.catalog".to_string(),
            "document.verify".to_string(),
            "document.extract".to_string(),
            "document.link".to_string(),
            "

    // ============================================================================
    // VISUALIZATION INTEGRATION (from V2)
    // ============================================================================

    /// Generate AST visualization (compatibility alias for V2)
    pub async fn generate_ast_visualization(
        &self,
        version_id: Uuid,
        options: &VisualizationOptions,
    ) -> DslResult<ASTVisualization> {
        let parsed = self
            .domain_repository
            .get_parsed_ast(&version_id)
            .await
            .map_err(|e| DslError::DatabaseError(format!("Failed to load AST: {}", e)))?;

        if let Some(ast) = parsed {
            let json_str = ast.ast_json.to_string();
            let visualization = self.build_ast_visualization_from_json(&json_str, options)?;
            Ok(visualization)
        } else {
            Err(DslError::NotFound {
                message: format!("Parsed AST not found for version: {}", version_id),
            })
        }
    }

    /// Generate domain-enhanced visualization
    pub async fn generate_domain_enhanced_visualization(
        &self,
        version_id: Uuid,
        options: &DomainVisualizationOptions,
    ) -> DslResult<DomainEnhancedVisualization> {
        let parsed = self
            .domain_repository
            .get_parsed_ast(&version_id)
            .await
            .map_err(|e| DslError::DatabaseError(format!("Failed to load AST: {}", e)))?;

        if let Some(ast) = parsed {
            let json_str = ast.ast_json.to_string();
            let base_visualization =
                self.build_ast_visualization_from_json(&json_str, &options.base_options)?;
            let enhanced_visualization = DomainEnhancedVisualization {
                base_visualization,
                domain_context: HashMap::new(),
                highlighted_nodes: vec![],
                workflow_progression: None,
                critical_paths: vec![],
            };
            Ok(enhanced_visualization)
        } else {
            Err(DslError::NotFound {
                message: format!("Parsed AST not found for version: {}", version_id),
            })
        }
    }

    // ============================================================================
    // LEGACY V2 PASS-THROUGHS (domains and versions)
    // ============================================================================

    pub async fn list_domains(&self, active_only: bool) -> DslResult<Vec<DslDomain>> {
        self.domain_repository
            .list_domains(active_only)
            .await
            .map_err(DslError::from)
    }

    pub async fn get_domain_by_id(&self, domain_id: Uuid) -> DslResult<Option<DslDomain>> {
        self.domain_repository
            .get_domain_by_id(&domain_id)
            .await
            .map_err(DslError::from)
    }

    pub async fn list_domain_versions(
        &self,
        domain_id: Uuid,
        limit: Option<i32>,
    ) -> DslResult<Vec<DslVersion>> {
        let domain = self
            .domain_repository
            .get_domain_by_id(&domain_id)
            .await
            .map_err(DslError::from)?
            .ok_or_else(|| DslError::NotFound {
                message: format!("domain: {}", domain_id),
            })?;

        self.domain_repository
            .list_versions(&domain.domain_name, limit)
            .await
            .map_err(DslError::from)
    }

    // ============================================================================
    // BUSINESS REQUEST INTEGRATION (from V3)
    // ============================================================================

    /// Create business request with associated DSL instance
    pub async fn create_business_request_with_dsl(
        &self,
        request_type: BusinessRequestType,
        business_reference: String,
        client_id: String,
        domain_name: String,
        template_type: TemplateType,
        created_by: String,
    ) -> DslResult<(DslBusinessRequest, DslInstance)> {
        info!(
            "Creating business request: {} with DSL instance ",
            business_reference
        );

        // Create business request
        let new_request = match request_type {
            BusinessRequestType::KycCase => NewDslBusinessRequest::new_kyc_case(
                business_reference.clone(),
                client_id.clone(),
                created_by.clone(),
            ),
            BusinessRequestType::OnboardingRequest => {
                NewDslBusinessRequest::new_onboarding_request(
                    business_reference.clone(),
                    client_id.clone(),
                    created_by.clone(),
                )
            }
            BusinessRequestType::AccountOpening => NewDslBusinessRequest::new_account_opening(
                business_reference.clone(),
                client_id.clone(),
                created_by.clone(),
            ),
        };

        let business_request = self
            .business_request_repository
            .create_business_request(new_request, None)
            .await
            .map_err(|e| {
                DslError::DatabaseError(format!("Failed to create business request: {}", e))
            })?;

        // Create associated DSL instance
        let template_variables = serde_json::json!({
            "business_reference": business_reference,
            "client_id": client_id,
            "request_id": business_request.request_id.to_string()
        });

        let dsl_instance = self
            .create_dsl_instance(&domain_name, template_type, template_variables, &created_by)
            .await?;

        Ok((business_request, dsl_instance))
    }

    /// Link existing DSL instance to business request
    pub async fn link_instance_to_business_request(
        &self,
        instance_id: Uuid,
        request_id: Uuid,
    ) -> DslResult<()> {
        // Update instance metadata to include business request reference
        let mut instance = self.get_instance(instance_id).await?;
        if let serde_json::Value::Object(ref mut map) = instance.metadata {
            map.insert(
                "business_request_id".to_string(),
                serde_json::Value::String(request_id.to_string()),
            );
        } else {
            instance.metadata = serde_json::json!({
                "business_request_id": request_id.to_string()
            });
        }
        self.update_instance(&instance).await?;

        Ok(())
    }

    // ============================================================================
    // DATABASE OPERATIONS - ABSTRACTED FOR CLEAN API
    // ============================================================================

    /// Get latest version for instance
    pub async fn get_latest_instance_version(
        &self,
        instance_id: Uuid,
    ) -> DslResult<DslInstanceVersion> {
        // TODO: Implement latest version retrieval
        Err(DslError::NotFound {
            message: format!("No versions found for instance: {}", instance_id),
        })
    }

    /// Get all versions for instance
    async fn get_all_instance_versions(
        &self,
        _instance_id: Uuid,
    ) -> DslResult<Vec<DslInstanceVersion>> {
        // TODO: Implement all versions retrieval
        Ok(vec![])
    }

    /// Get version by ID
    async fn get_version_by_id(
        &self,
        version_id: Uuid,
    ) -> DslResult<crate::database::dsl_instance_repository::DslInstanceVersion> {
        // TODO: Implement version retrieval by ID
        Err(DslError::NotFound {
            message: format!("Version not found: {}", version_id),
        })
    }

    // ============================================================================
    // DATABASE OPERATIONS - CBU AND DSL STORAGE
    // ============================================================================

    /// Get CBU information from database
    async fn get_cbu_info(&self, cbu_id: Uuid) -> DslResult<CbuInfo> {
        info!("Getting CBU info for: {}", cbu_id);

        let row = sqlx::query(
            r#"SELECT cbu_id, name, description, nature_purpose, created_at, updated_at
               FROM "ob-poc".cbus
               WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_optional(self.domain_repository.pool())
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to query CBU: {}", e)))?;

        match row {
            Some(row) => {
                use sqlx::Row;
                Ok(CbuInfo {
                    cbu_id: row.get("cbu_id"),
                    name: row.get("name"),
                    description: row.get("description"),
                    nature_purpose: row.get("nature_purpose"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            }
            None => Err(DslError::NotFound {
                message: format!("CBU not found: {}", cbu_id),
            }),
        }
    }

    /// Validate that CBU exists in the database
    async fn validate_cbu_exists(&self, cbu_id: Uuid) -> DslResult<()> {
        info!("Validating CBU exists: {}", cbu_id);

        let count =
            sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_one(self.domain_repository.pool())
                .await
                .map_err(|e| DslError::DatabaseError(format!("Failed to validate CBU: {}", e)))?;

        if count == 0 {
            return Err(DslError::NotFound {
                message: format!("CBU not found: {}", cbu_id),
            });
        }

        Ok(())
    }

    /// Store DSL source code with OB request index and DSL.OB keys
    async fn store_dsl_source_with_ob_index(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        dsl_content: &str,
        version_id: Uuid,
    ) -> DslResult<DslStorageKeys> {
        info!(
            "Storing DSL source with OB request index: {}",
            ob_request_id
        );

        // Insert into dsl_ob table with proper indexing
        let dsl_ob_version_id = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"INSERT INTO "ob-poc".dsl_ob (version_id, cbu_id, dsl_text, created_at)
               VALUES ($1, $2, $3, NOW())
               RETURNING version_id "#,
        )
        .bind(version_id)
        .bind(cbu_id.to_string()) // Note: current schema uses VARCHAR for cbu_id
        .bind(dsl_content)
        .fetch_one(self.domain_repository.pool())
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to store DSL source: {}", e)))?;

        Ok(DslStorageKeys {
            dsl_ob_version_id,
            cbu_id,
            ob_request_id,
            storage_index: format!("OB-{}-{}", cbu_id, ob_request_id),
        })
    }

    /// Store AST in database with FK links back to DSL source
    async fn store_ast_with_fk_links(
        &self,
        version_id: Uuid,
        ast_json: &str,
        node_count: usize,
        _complexity_score: f32,
    ) -> DslResult<Uuid> {
        info!("Storing AST with FK links for version: {}", version_id);

        // Store AST using existing parsed_asts infrastructure
        let new_ast = NewParsedAst {
            version_id,
            ast_json: serde_json::from_str(ast_json)?,
            node_count: Some(node_count as i32),
            parse_metadata: None,
            grammar_version: self.grammar_version.clone(),
            parser_version: self.parser_version.clone(),
            ast_hash: None,
            complexity_score: None,
        };

        let stored_ast = self
            .domain_repository
            .store_parsed_ast(new_ast)
            .await
            .map_err(|e| DslError::DatabaseError(format!("Failed to store AST: {}", e)))?;

        info!("Successfully stored AST with ID: {}", stored_ast.ast_id);
        Ok(stored_ast.ast_id)
    }

    /// Create onboarding session record in database
    async fn create_onboarding_session_record(
        &self,
        cbu_id: Uuid,
        ob_request_id: Uuid,
        version_id: Uuid,
    ) -> DslResult<OnboardingSessionRecord> {
        info!("Creating onboarding session record for CBU: {}", cbu_id);

        // For now, create a placeholder record since onboarding_sessions table may not exist
        // This can be updated when the table structure is finalized
        Ok(OnboardingSessionRecord {
            onboarding_id: ob_request_id,
            cbu_id,
            current_state: "CREATED".to_string(),
            current_version: 1,
            latest_dsl_version_id: Some(version_id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Store DSL instance in database
    pub async fn store_instance(
        &self,
        instance: &crate::database::dsl_instance_repository::DslInstance,
    ) -> DslResult<()> {
        info!("[V3.1] Storing DSL instance: {}", instance.instance_id);

        // For now, store instance metadata in a JSON format in the domain system
        // This could be extended to a dedicated instances table later
        let _metadata_json =
            serde_json::to_string(&instance).map_err(|e| DslError::SerializationError {
                message: format!("Failed to serialize instance: {}", e),
            })?;

        // Store as a domain entry for tracking
        // For now, just log the instance storage
        // This would be replaced with proper instance table management

        // Placeholder for instance storage

        Ok(())
    }

    /// Update DSL instance in database
    pub async fn update_instance(
        &self,
        instance: &crate::database::dsl_instance_repository::DslInstance,
    ) -> DslResult<()> {
        info!("[V3.1] Updating DSL instance: {}", instance.instance_id);
        // For now, this is a no-op since we're using the domain system
        // Could be extended with dedicated instance management later
        Ok(())
    }

    /// Get DSL instance by ID
    pub async fn get_instance(
        &self,
        instance_id: Uuid,
    ) -> DslResult<crate::database::dsl_instance_repository::DslInstance> {
        // For now, create a placeholder instance
        // This would be replaced with actual database retrieval
        Err(DslError::NotFound {
            message: format!("DSL instance not found: {}", instance_id),
        })
    }

    /// Store DSL instance version in database
    pub async fn store_instance_version(
        &self,
        version: &crate::database::dsl_instance_repository::DslInstanceVersion,
    ) -> DslResult<()> {
        info!(
            "[V3.1] Storing DSL instance version: {} (v{})",
            version.version_id, version.version_number
        );

        // Store using the domain version system
        // For now, store version information in a simplified way
        // This would be replaced with proper instance version table management

        // Placeholder for version storage

        Ok(())
    }

    /// Update DSL instance version
    pub async fn update_instance_version(
        &self,
        version: &crate::database::dsl_instance_repository::DslInstanceVersion,
    ) -> DslResult<()> {
        info!("[V3.1] Updating DSL instance version: {}", version.version_id);

        // Update compilation status
        self.domain_repository
            .update_compilation_status(&version.version_id, version.compilation_status.clone())
            .await
            .map_err(|e| {
                DslError::DatabaseError(format!("Failed to update instance version: {}", e))
            })?;

        Ok(())
    }

    // ============================================================================
    // SPECIFIC DSL OPERATIONS - KYC AND ONBOARDING
    // ============================================================================

    /// Create DSL.KYC case with KYC templates and embedded UBO sub domain
    pub async fn create_kyc_case(
        &self,
        cbu_id: Uuid,
        case_name: String,
        case_description: String,
        created_by: String,
    ) -> DslResult<KycCaseCreationResult> {
        info!(
            "Creating DSL.KYC case for CBU: {} with name: {}",
            cbu_id, case_name
        );

        // Validate CBU exists
        self.validate_cbu_exists(cbu_id).await?;

        // Create main KYC case DSL instance
        let kyc_variables = serde_json::json!({
            "cbu_id": cbu_id.to_string(),
            "case_name": case_name.clone(),
            "case_description": case_description.clone()
        });

        let kyc_instance = self
            .create_dsl_instance(
                "kyc",
                TemplateType::Custom("kyc_case_create".to_string()),
                kyc_variables,
                &created_by,
            )
            .await?;

        // Create embedded KYC.UBO sub domain DSL instance
        let ubo_variables = serde_json::json!({
            "cbu_id": cbu_id.to_string(),
            "parent_case_id": kyc_instance.instance_id.to_string()
        });

        let ubo_instance = self
            .create_dsl_instance(
                "ubo",
                TemplateType::Custom("kyc_ubo_create".to_string()),
                ubo_variables,
                &created_by,
            )
            .await?;

        // Link UBO instance to KYC case
        self.link_sub_domain_instance(kyc_instance.instance_id, ubo_instance.instance_id)
            .await?;

        let result = KycCaseCreationResult {
            kyc_case_instance: kyc_instance,
            ubo_sub_instance: ubo_instance,
            cbu_id,
            created_at: Utc::now(),
        };

        info!(
            "Successfully created DSL.KYC case with instance: {} and UBO sub-instance: {}",
            result.kyc_case_instance.instance_id, result.ubo_sub_instance.instance_id
        );

        Ok(result)
    }

    /// Create DSL.OB (Onboarding) request - main overarching context
    pub async fn create_onboarding_request(
        &self,
        cbu_id: Uuid,
        onboarding_name: String,
        onboarding_description: String,
        created_by: String,
    ) -> DslResult<OnboardingRequestCreationResult> {
        info!(
            "Creating DSL.OB request for CBU: {} with name: {}",
            cbu_id, onboarding_name
        );

        // Validate CBU exists and get CBU details
        let cbu_info = self.get_cbu_info(cbu_id).await?;

        // Get DSL.OB create template
        let ob_template = self
            .load_template("onboarding", &TemplateType::CreateCbu)
            .await?;

        // Populate template with CBU ID and request details
        let mut ob_variables = serde_json::Map::new();
        ob_variables.insert(
            "cbu_id".to_string(),
            serde_json::Value::String(cbu_id.to_string()),
        );
        ob_variables.insert(
            "cbu_name".to_string(),
            serde_json::Value::String(cbu_info.name),
        );
        ob_variables.insert(
            "cbu_description".to_string(),
            serde_json::Value::String(cbu_info.description.unwrap_or_default()),
        );
        ob_variables.insert(
            "cbu_nature_purpose".to_string(),
            serde_json::Value::String(cbu_info.nature_purpose.unwrap_or_default()),
        );
        ob_variables.insert(
            "onboarding_name".to_string(),
            serde_json::Value::String(onboarding_name.clone()),
        );
        ob_variables.insert(
            "onboarding_description".to_string(),
            serde_json::Value::String(onboarding_description.clone()),
        );

        // Create OB request ID (minted)
        let ob_request_id = Uuid::new_v4();
        ob_variables.insert(
            "ob_request_id".to_string(),
            serde_json::Value::String(ob_request_id.to_string()),
        );

        // Generate DSL content from template
        let ob_variables_value = serde_json::Value::Object(ob_variables.clone());
        let dsl_content = self
            .generate_dsl_from_template(&ob_template, &ob_variables_value)
            .await?;

        // Parse first with NOM: only persist if parse succeeds
        if let Err(parse_error) = parse_program(&dsl_content) {
            return Err(DslError::CompilationError {
                message: format!(
                    "DSL parsing failed for onboarding request: {:?}",
                    parse_error
                ),
            });
        }

        // Create DSL instance with OB request context
        let ob_instance = DslInstance {
            instance_id: Uuid::new_v4(),
            domain_name: "onboarding".to_string(),
            business_reference: Some(format!("OB-{}", ob_request_id)),
            current_version: 1,
            status: InstanceStatus::Created,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: ob_variables_value.clone(),
        };

        // Store DSL instance metadata (optional/no-op currently)
        self.store_instance(&ob_instance).await?;

        // Create initial domain version tied to request_id (persist only after parse success)
        let new_version = crate::models::domain_models::NewDslVersion {
            domain_name: "onboarding".to_string(),
            request_id: Some(ob_request_id),
            functional_state: Some("Created".to_string()),
            dsl_source_code: dsl_content.clone(),
            change_description: Some(format!("Initial OB request creation: {}", onboarding_name)),
            parent_version_id: None,
            created_by: Some(created_by.clone()),
        };

        let created = self
            .domain_repository
            .create_new_version(new_version)
            .await
            .map_err(|e| DslError::DatabaseError(format!("Failed to create OB version: {}", e)))?;

        // Journal DSL to dsl_ob for append-only history
        let dsl_storage_result = self
            .store_dsl_source_with_ob_index(ob_request_id, cbu_id, &dsl_content, created.version_id)
            .await?;

        // Compile + store AST and update compilation status
        let to_compile = DslInstanceVersion {
            version_id: created.version_id,
            instance_id: ob_instance.instance_id,
            version_number: created.version_number,
            dsl_content: dsl_content.clone(),
            operation_type: OperationType::CreateFromTemplate(ob_template.template_id.clone()),
            compilation_status: created.compilation_status,
            ast_json: None,
            created_at: created.created_at,
            created_by: created_by.clone(),
            change_description: Some(format!("Initial OB request creation: {}", onboarding_name)),
        };
        let compiled_version = self.compile_and_store_ast(to_compile).await?;
        self.update_instance_version(&compiled_version).await?;

        // Create onboarding session record
        let onboarding_session = self
            .create_onboarding_session_record(cbu_id, ob_request_id, compiled_version.version_id)
            .await?;

        let result = OnboardingRequestCreationResult {
            ob_request_id,
            ob_instance,
            compiled_version,
            onboarding_session,
            cbu_id,
            dsl_storage_keys: dsl_storage_result,
            created_at: Utc::now(),
        };

        info!(
            "Successfully created DSL.OB request: {} for CBU: {} with instance: {}",
            ob_request_id, cbu_id, result.ob_instance.instance_id
        );

        Ok(result)
    }

    // ========================
    // ONBOARDING STATE EDITS
    // ========================

    /// Associate a CBU to an existing OB request (appends DSL and persists a new version)
    pub async fn associate_cbu(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        association_type: &str,
        details: serde_json::Value,
        created_by: &str,
        change_description: Option<String>,
    ) -> DslResult<DslInstanceVersion> {
        let prev = self.get_latest_ob_version_dsl(ob_request_id).await?;
        let fragment = format!(
            r#"
(cbu.associate (cbu.id "{}") (association.type "{}") (cbu.details {}) (associated.at "{}"))"#,
            cbu_id,
            association_type,
            details,
            Utc::now().to_rfc3339()
        );
        let combined = format!("{}{}", prev, fragment);
        self.persist_ob_edit(
            ob_request_id,
            cbu_id,
            &combined,
            created_by,
            change_description.unwrap_or_else(|| "Associate CBU ".to_string()),
        )
        .await
    }

    /// Add products to the OB request
    pub async fn add_products(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        products: &[String],
        selection_reason: &str,
        created_by: &str,
    ) -> DslResult<DslInstanceVersion> {
        let prev = self.get_latest_ob_version_dsl(ob_request_id).await?;
        let quoted = products
            .iter()
            .map(|p| format!(r#""{}""#, p))
            .collect::<Vec<_>>()
            .join(" ");
        let fragment = format!(
            r#"
(products.select (products [{}]) (selection.reason "{}") (selected.at "{}"))"#,
            quoted,
            selection_reason,
            Utc::now().to_rfc3339()
        );
        let combined = format!("{}{}", prev, fragment);
        self.persist_ob_edit(
            ob_request_id,
            cbu_id,
            &combined,
            created_by,
            "Add products ".to_string(),
        )
        .await
    }

    /// Discover services for products (simple placeholder version)
    pub async fn discover_services(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        products: &[String],
        created_by: &str,
    ) -> DslResult<DslInstanceVersion> {
        let prev = self.get_latest_ob_version_dsl(ob_request_id).await?;
        let mut services_block = String::from(r#"
(services.discover
"#);
        for p in products {
            services_block.push_str(&format!(r#"  (for.product "{}")
"#, p));
        }
        services_block.push_str(&format!(
            r#"  (discovered.at "{}"))"#,
            Utc::now().to_rfc3339()
        ));
        let combined = format!("{}{}", prev, services_block);
        self.persist_ob_edit(
            ob_request_id,
            cbu_id,
            &combined,
            created_by,
            "Discover services ".to_string(),
        )
        .await
    }

    /// Discover resources for services (placeholder)
    pub async fn discover_resources(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        services: &[String],
        created_by: &str,
    ) -> DslResult<DslInstanceVersion> {
        let prev = self.get_latest_ob_version_dsl(ob_request_id).await?;
        let mut resources_block = String::from(r#"
(resources.discover
"#);
        for s in services {
            resources_block.push_str(&format!(r#"  (for.service "{}")
"#, s));
        }
        resources_block.push_str(&format!(
            r#"  (discovered.at "{}"))"#,
            Utc::now().to_rfc3339()
        ));
        let combined = format!("{}{}", prev, resources_block);
        self.persist_ob_edit(
            ob_request_id,
            cbu_id,
            &combined,
            created_by,
            "Discover resources ".to_string(),
        )
        .await
    }

    /// Complete onboarding (append completion DSL)
    pub async fn complete_onboarding(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        completion_notes: &str,
        created_by: &str,
    ) -> DslResult<DslInstanceVersion> {
        let prev = self.get_latest_ob_version_dsl(ob_request_id).await?;
        let fragment = format!(
            r#"
(onboarding.complete (completion.status "SUCCESS") (completion.notes "{}") (completed.at "{}"))"#,
            completion_notes,
            Utc::now().to_rfc3339()
        );
        let combined = format!("{}{}", prev, fragment);
        self.persist_ob_edit(
            ob_request_id,
            cbu_id,
            &combined,
            created_by,
            "Complete onboarding ".to_string(),
        )
        .await
    }

    /// Archive onboarding (append archive DSL)
    pub async fn archive_onboarding(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        reason: &str,
        created_by: &str,
    ) -> DslResult<DslInstanceVersion> {
        let prev = self.get_latest_ob_version_dsl(ob_request_id).await?;
        let fragment = format!(
            r#"
(onboarding.archive (archival.reason "{}") (archived.at "{}"))"#,
            archival_reason,
            Utc::now().to_rfc3339()
        );
        let combined = format!("{}{}", prev, fragment);
        self.persist_ob_edit(
            ob_request_id,
            cbu_id,
            &combined,
            created_by,
            "Archive onboarding ".to_string(),
        )
        .await
    }

    async fn get_latest_ob_version_dsl(&self, ob_request_id: Uuid) -> DslResult<String> {
        // Latest DSL content for this OB request from dsl_versions
        let row = sqlx::query(
            r#"SELECT dv.dsl_source_code
                FROM "ob-poc".dsl_versions dv
               WHERE dv.request_id = $1
               ORDER BY dv.version_number DESC
               LIMIT 1"#,
        )
        .bind(ob_request_id)
        .fetch_optional(self.domain_repository.pool())
        .await
        .map_err(|e| {
            DslError::DatabaseError(format!("Failed to fetch latest OB version: {}", e))
        })?;

        use sqlx::Row;
        if let Some(r) = row {
            Ok(r.get::<String, _>("dsl_source_code"))
        } else {
            Err(DslError::NotFound {
                message: format!("No DSL found for OB request: {}", ob_request_id),
            })
        }
    }

    async fn persist_ob_edit(
        &self,
        ob_request_id: Uuid,
        cbu_id: Uuid,
        combined_dsl: &str,
        created_by: &str,
        change_description: String,
    ) -> DslResult<DslInstanceVersion> {
        // Parse first: only persist if parse succeeds
        if let Err(parse_error) = parse_program(combined_dsl) {
            return Err(DslError::CompilationError {
                message: format!("DSL parsing failed for OB edit: {:?}", parse_error),
            });
        }

        // Create new domain version tied to request_id (after successful parse)
        let new_version = crate::models::domain_models::NewDslVersion {
            domain_name: "onboarding".to_string(),
            request_id: Some(ob_request_id),
            functional_state: Some("Edit".to_string()),
            dsl_source_code: combined_dsl.to_string(),
            change_description: Some(change_description),
            parent_version_id: None,
            created_by: Some(created_by.to_string()),
        };

        let created = self
            .domain_repository
            .create_new_version(new_version)
            .await
            .map_err(|e| DslError::DatabaseError(format!("Failed to create new version: {}", e)))?;

        // Journal DSL to dsl_ob for append-only history (optional)
        let _keys = self
            .store_dsl_source_with_ob_index(ob_request_id, cbu_id, combined_dsl, created.version_id)
            .await?;

        // Compile + store AST
        let parsed_version = DslInstanceVersion {
            version_id: created.version_id,
            instance_id: Uuid::new_v4(), // not persisted yet as instance
            version_number: created.version_number,
            dsl_content: combined_dsl.to_string(),
            operation_type: OperationType::IncrementalEdit,
            compilation_status: created.compilation_status,
            ast_json: None,
            created_at: created.created_at,
            created_by: created.created_by.unwrap_or_else(|| created_by.to_string()),
            change_description: created.change_description,
        };

        let compiled = self.compile_and_store_ast(parsed_version).await?;
        self.update_instance_version(&compiled).await?;
        Ok(compiled)
    }

    // ============================================================================
    // SUPPORTING OPERATIONS FOR KYC AND ONBOARDING
    // ============================================================================

    /// Link sub-domain DSL instance (like UBO) to parent instance (like KYC)
    async fn link_sub_domain_instance(
        &self,
        parent_instance_id: Uuid,
        sub_instance_id: Uuid,
    ) -> DslResult<()> {
        info!(
            "Linking sub-domain instance {} to parent {}",
            sub_instance_id, parent_instance_id
        );

        // Update sub-instance metadata to include parent reference
        let mut sub_instance = self.get_instance(sub_instance_id).await?;
        if let serde_json::Value::Object(ref mut map) = sub_instance.metadata {
            map.insert(
                "parent_instance_id".to_string(),
                serde_json::Value::String(parent_instance_id.to_string()),
            );
        } else {
            sub_instance.metadata = serde_json::json!({
                "parent_instance_id": parent_instance_id.to_string()
            });
        }
        self.update_instance(&sub_instance).await?;

        Ok(())
    }

    /// Compile DSL with NOM and store AST with FK links back to DSL source
    async fn compile_and_store_ast(
        &self,
        mut version: DslInstanceVersion,
    ) -> DslResult<DslInstanceVersion> {
        info!(
            "Compiling DSL and storing AST for version: {}",
            version.version_id
        );

        let start_time = std::time::Instant::now();

        // NOM parses DSL content
        match parse_program(&version.dsl_content) {
            Ok(ast) => {
                // Build AST JSON
                let ast_json =
                    serde_json::to_string(&ast).map_err(|e| DslError::SerializationError {
                        message: format!("Failed to serialize AST: {}", e),
                    })?;

                version.ast_json = Some(ast_json.clone());
                version.compilation_status =
                    crate::database::dsl_instance_repository::CompilationStatus::Success;

                // Count AST nodes for metrics
                let node_count = self.count_ast_nodes(&ast);
                let complexity_score = self.calculate_complexity_score(&ast);

                // Store AST in database with FK links back to DSL source
                let _ast_id = self
                    .store_ast_with_fk_links(
                        version.version_id,
                        &ast_json,
                        node_count,
                        complexity_score,
                    )
                    .await?;

                let duration = start_time.elapsed();
                let duration_ms = duration.as_millis();
                info!(
                    "Successfully compiled and stored AST for version: {} in {} ms ",
                    version.version_id,
                    duration_ms
                );
            }
            Err(parse_error) => {
                version.compilation_status =
                    crate::database::dsl_instance_repository::CompilationStatus::Error;
                warn!(
                    "Failed to compile DSL version: {} - Error: {:?}",
                    version.version_id, parse_error
                );

                return Err(DslError::CompilationError {
                    message: format!("DSL compilation failed: {:?}", parse_error),
                });
            }
        }

        Ok(version)
    }

    /// Count AST nodes for metrics
    fn count_ast_nodes(&self, ast: &crate::ast::Program) -> usize {
        // Simple node counting - could be made more sophisticated
        ast.workflows.len()
            + ast
                .workflows
                .iter()
                .map(|w| w.statements.len())
                .sum::<usize>()
    }

    /// Calculate complexity score for AST
    fn calculate_complexity_score(&self, ast: &crate::ast::Program) -> f32 {
        // Simple complexity scoring based on workflow and statement count
        let workflow_count = ast.workflows.len() as f32;
        let statement_count = ast
            .workflows
            .iter()
            .map(|w| w.statements.len())
            .sum::<usize>() as f32;

        workflow_count * 2.0 + statement_count
    }

    // ============================================================================
    // CATALOG OPERATIONS - FOR TEMPLATE DATA INJECTION
    // ============================================================================

    /// Get available products with filters
    async fn get_available_products(
        &self,
        _filters: &serde_json::Value,
    ) -> DslResult<Vec<ProductInfo>> {
        // TODO: Query products table with filters
        Ok(vec![])
    }

    /// Get available services with filters
    async fn get_available_services(
        &self,
        _filters: &serde_json::Value,
    ) -> DslResult<Vec<ServiceInfo>> {
        // TODO: Query services table with filters
        Ok(vec![])
    }

    /// Get available resources with filters
    async fn get_available_resources(
        &self,
        _filters: &serde_json::Value,
    ) -> DslResult<Vec<ResourceInfo>> {
        // TODO: Query prod_resources table with filters
        Ok(vec![])
    }

    // ============================================================================
    // UTILITY METHODS FROM PREVIOUS VERSIONS
    // ============================================================================

    /// Build AST visualization from JSON
    fn build_ast_visualization_from_json(
        &self,
        ast_json: &str,
        _options: &VisualizationOptions,
    ) -> DslResult<ASTVisualization> {
        use crate::ast::{Program, Statement};
        use serde_json::Value as JValue;

        // Convert AST Value → JSON Value for node properties
        fn val_to_json(v: &crate::ast::Value) -> JValue {
            match v {
                crate::ast::Value::String(s) => JValue::String(s.clone()),
                crate::ast::Value::Number(n) => JValue::from(*n),
                crate::ast::Value::Integer(i) => JValue::from(*i),
                crate::ast::Value::Boolean(b) => JValue::from(*b),
                crate::ast::Value::Date(d) => JValue::String(d.to_string()),
                crate::ast::Value::List(list) => {
                    JValue::Array(list.iter().map(val_to_json).collect())
                }
                crate::ast::Value::Map(m) => {
                    JValue::Object(m.iter().map(|(k, v)| (k.clone(), val_to_json(v))).collect())
                }
                crate::ast::Value::MultiValue(vals) => JValue::Array(
                    vals.iter()
                        .map(|vw| {
                            let mut obj = serde_json::Map::new();
                            obj.insert("value".to_string(), val_to_json(&vw.value));
                            obj.insert("source".to_string(), JValue::String(vw.source.clone()));
                            if let Some(c) = vw.confidence {
                                obj.insert("confidence".to_string(), JValue::from(c));
                            }
                            JValue::Object(obj)
                        })
                        .collect(),
                ),
                crate::ast::Value::Null => JValue::Null,
            }
        }

        let started = std::time::Instant::now();
        let program: Program =
            serde_json::from_str(ast_json).map_err(|e| DslError::SerializationError {
                message: format!("Failed to deserialize AST JSON: {}", e),
            })?;

        let mut nodes: Vec<VisualNode> = Vec::new();
        let mut edges: Vec<VisualEdge> = Vec::new();

        // Root node representing the program
        let root_id = "program".to_string();
        nodes.push(VisualNode {
            id: root_id.clone(),
            label: "Program".to_string(),
            node_type: "Program".to_string(),
            properties: Default::default(),
            position: None,
            styling: NodeStyling::default(),
            domain_annotations: vec![],
            priority_level: 0,
            functional_relevance: 1.0,
        });

        // Simple hierarchical layout indices
        // Index used by enumerate below
        let _wf_index = 0usize;
        // let mut stmt_global_index = 0usize; // not used in simple layout
        let mut max_depth = 1usize;

        for (wf_index, workflow) in program.workflows.iter().enumerate() {
            let wf_id = format!("wf:{}", workflow.id);
            // Convert workflow properties
            let wf_props = workflow
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), val_to_json(v)))
                .collect();

            nodes.push(VisualNode {
                id: wf_id.clone(),
                label: workflow.id.clone(),
                node_type: "Workflow".to_string(),
                properties: wf_props,
                position: Some(NodePosition {
                    x: 100.0,
                    y: 100.0 + (wf_index as f32) * 150.0,
                    z: None,
                }),
                styling: NodeStyling::default(),
                domain_annotations: vec![],
                priority_level: 1,
                functional_relevance: 1.0,
            });

            edges.push(VisualEdge {
                id: format!("e:{}->{}", root_id, wf_id),
                from: root_id.clone(),
                to: wf_id.clone(),
                edge_type: "contains".to_string(),
                label: Some("contains".to_string()),
                styling: EdgeStyling::default(),
                weight: 1.0,
            });

            // Statements
            for (i, stmt) in workflow.statements.iter().enumerate() {
                let stmt_id = format!("stmt:{}:{}", workflow.id, i);
                let (label, node_type, mut props) = match stmt {
                    Statement::DeclareEntity {
                        id,
                        entity_type,
                        properties,
                    } => {
                        let mut p = std::collections::HashMap::new();
                        p.insert(
                            "entity_type".to_string(),
                            JValue::String(entity_type.clone()),
                        );
                        for (k, v) in properties {
                            p.insert(k.clone(), val_to_json(v));
                        }
                        (
                            format!("DeclareEntity:{}", id),
                            "DeclareEntity".to_string(),
                            p,
                        )
                    }
                    Statement::ObtainDocument {
                        document_type,
                        source,
                        properties,
                    } => {
                        let mut p = std::collections::HashMap::new();
                        p.insert(
                            "document_type".to_string(),
                            JValue::String(document_type.clone()),
                        );
                        p.insert("source".to_string(), JValue::String(source.clone()));
                        for (k, v) in properties {
                            p.insert(k.clone(), val_to_json(v));
                        }
                        (
                            "ObtainDocument".to_string(),
                            "ObtainDocument".to_string(),
                            p,
                        )
                    }
                    Statement::CreateEdge {
                        from,
                        to,
                        edge_type,
                        properties,
                    } => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("from".to_string(), JValue::String(from.clone()));
                        p.insert("to".to_string(), JValue::String(to.clone()));
                        p.insert(
                            "edge_type".to_string(),
                            JValue::String(edge_type.to_string()),
                        );
                        for (k, v) in properties {
                            p.insert(k.clone(), val_to_json(v));
                        }
                        ("CreateEdge".to_string(), "CreateEdge".to_string(), p)
                    }
                    Statement::CalculateUbo {
                        entity_id,
                        properties,
                    } => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("entity_id".to_string(), JValue::String(entity_id.clone()));
                        for (k, v) in properties {
                            p.insert(k.clone(), val_to_json(v));
                        }
                        ("CalculateUbo".to_string(), "CalculateUbo".to_string(), p)
                    }
                    Statement::SolicitAttribute(inner) => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("attr_id".to_string(), JValue::String(inner.attr_id.clone()));
                        p.insert("from".to_string(), JValue::String(inner.from.clone()));
                        p.insert(
                            "value_type".to_string(),
                            JValue::String(inner.value_type.clone()),
                        );
                        for (k, v) in &inner.additional_props {
                            p.insert(k.clone(), val_to_json(v));
                        }
                        (
                            "SolicitAttribute".to_string(),
                            "SolicitAttribute".to_string(),
                            p,
                        )
                    }
                    Statement::ResolveConflict(inner) => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("node".to_string(), JValue::String(inner.node.clone()));
                        p.insert(
                            "property".to_string(),
                            JValue::String(inner.property.clone()),
                        );
                        p.insert(
                            "strategy".to_string(),
                            JValue::String(match inner.strategy.priorities.len() {
                                0 => "none".to_string(),
                                _ => "waterfall".to_string(),
                            }),
                        );
                        for (k, v) in &inner.resolution {
                            p.insert(k.clone(), val_to_json(v));
                        }
                        (
                            "ResolveConflict".to_string(),
                            "ResolveConflict".to_string(),
                            p,
                        )
                    }
                    Statement::GenerateReport(inner) => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("target".to_string(), JValue::String(inner.target.clone()));
                        p.insert("status".to_string(), JValue::String(inner.status.clone()));
                        (
                            "GenerateReport".to_string(),
                            "GenerateReport".to_string(),
                            p,
                        )
                    }
                    Statement::ScheduleMonitoring(inner) => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("target".to_string(), JValue::String(inner.target.clone()));
                        p.insert(
                            "frequency".to_string(),
                            JValue::String(inner.frequency.clone()),
                        );
                        (
                            "ScheduleMonitoring".to_string(),
                            "ScheduleMonitoring".to_string(),
                            p,
                        )
                    }
                    Statement::Parallel(inner) => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("branch_count".to_string(), JValue::from(inner.len()));
                        ("Parallel".to_string(), "Parallel".to_string(), p)
                    }
                    Statement::ParallelObtain(inner) => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("documents".to_string(), JValue::from(inner.documents.len()));
                        (
                            "ParallelObtain".to_string(),
                            "ParallelObtain".to_string(),
                            p,
                        )
                    }
                    Statement::Sequential(inner) => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("step_count".to_string(), JValue::from(inner.len()));
                        ("Sequential".to_string(), "Sequential".to_string(), p)
                    }
                    Statement::Placeholder { command, args } => {
                        let mut p = std::collections::HashMap::new();
                        p.insert("command".to_string(), JValue::String(command.clone()));
                        p.insert(
                            "args".to_string(),
                            JValue::Array(args.iter().map(val_to_json).collect()),
                        );
                        ("Placeholder".to_string(), "Placeholder".to_string(), p)
                    }
                };

                nodes.push(VisualNode {
                    id: stmt_id.clone(),
                    label,
                    node_type,
                    properties: std::mem::take(&mut props),
                    position: Some(NodePosition {
                        x: 350.0,
                        y: 100.0 + (wf_index as f32) * 150.0 + (i as f32) * 60.0,
                        z: None,
                    }),
                    styling: NodeStyling::default(),
                    domain_annotations: vec![],
                    priority_level: 2,
                    functional_relevance: 1.0,
                });

                edges.push(VisualEdge {
                    id: format!("e:{}->{}", wf_id, stmt_id),
                    from: wf_id.clone(),
                    to: stmt_id.clone(),
                    edge_type: "contains".to_string(),
                    label: Some("contains".to_string()),
                    styling: EdgeStyling::default(),
                    weight: 1.0,
                });

                // stmt_global_index += 1;
                max_depth = max_depth.max(3);
            }
        }

        let duration = started.elapsed();
        let node_count = nodes.len();
        let edge_count = edges.len();

        Ok(ASTVisualization {
            metadata: VisualizationMetadata {
                generated_at: Utc::now(),
                parser_version: self.parser_version.clone(),
                grammar_version: self.grammar_version.clone(),
                node_count,
                edge_count,
                instance_id: None,
                version_id: None,
            },
            root_node: Some(nodes.first().cloned().unwrap_or(VisualNode {
                id: "program".to_string(),
                label: "Program".to_string(),
                node_type: "Program".to_string(),
                properties: Default::default(),
                position: None,
                styling: NodeStyling::default(),
                domain_annotations: vec![],
                priority_level: 0,
                functional_relevance: 1.0,
            })),
            nodes,
            edges,
            statistics: VisualizationStatistics {
                total_nodes: node_count,
                total_edges: edge_count,
                max_depth,
                complexity_score: node_count as f32 + edge_count as f32 / 10.0,
                compilation_time_ms: 0,
                visualization_time_ms: duration.as_millis() as u64,
            },
        })
    }

    // ============================================================================
    // ADDITIONAL PERSISTENCE OPERATIONS (consolidated from dsl_persistence.rs)
    // Note: Core persistence methods already exist as private methods above
    // ============================================================================

    /// List all DSL instances with optional filtering
    pub async fn list_instances(&self, domain_filter: Option<&str>) -> DslResult<Vec<DslInstance>> {
        let query = if let Some(domain) = domain_filter {
            sqlx::query_as::<_, DslInstance>(
                r#"SELECT * FROM "ob-poc".dsl_instances
                   WHERE domain_name = $1
                   ORDER BY created_at DESC "#,
            )
            .bind(domain)
        } else {
            sqlx::query_as::<_, DslInstance>(
                r#"SELECT * FROM "ob-poc".dsl_instances
                   ORDER BY created_at DESC "#,
            )
        };

        query
            .fetch_all(self.domain_repository.pool())
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))
    }

    /// Delete DSL instance and all its versions (soft delete by status update)
    pub async fn delete_instance(&self, instance_id: Uuid) -> DslResult<()> {
        let mut tx = self
            .domain_repository
            .pool()
            .begin()
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        // Update instance status to Archived instead of hard delete
        sqlx::query(
            r#"UPDATE "ob-poc".dsl_instances
               SET status = $1, updated_at = $2
               WHERE instance_id = $3"#,
        )
        .bind(InstanceStatus::Archived)
        .bind(Utc::now())
        .bind(instance_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}

// ============================================================================
// SUPPORTING TYPES AND STRUCTURES
// ============================================================================

/// Business request type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BusinessRequestType {
    KycCase,
    OnboardingRequest,
    AccountOpening,
}

/// Product information for template queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    pub product_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

/// Service information for template queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub service_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

/// Resource information for template queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub resource_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner: String,
    pub dictionary_group: Option<String>,
}

/// Visualization options from V2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationOptions {
    pub layout: LayoutType,
    pub styling: StylingConfig,
    pub include_compilation_info: bool,
    pub include_domain_context: bool,
    pub filters: Option<FilterConfig>,
}

/// Domain visualization options from V2/V3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainVisualizationOptions {
    pub base_options: VisualizationOptions,
    pub highlight_current_state: bool,
    pub show_state_transitions: bool,
    pub include_domain_metrics: bool,
    pub show_workflow_progression: bool,
    pub emphasize_critical_paths: bool,
    pub domain_specific_styling: bool,
}

/// Layout type for visualizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutType {
    Tree,
    Graph,
    Hierarchical,
}

/// Styling configuration for visualizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StylingConfig {
    pub theme: String,
    pub node_size: f32,
    pub font_size: f32,
    pub color_scheme: serde_json::Value,
}

/// CBU information from database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuInfo {
    pub cbu_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// KYC case creation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycCaseCreationResult {
    pub kyc_case_instance: DslInstance,
    pub ubo_sub_instance: DslInstance,
    pub cbu_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Onboarding request creation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingRequestCreationResult {
    pub ob_request_id: Uuid,
    pub ob_instance: DslInstance,
    pub compiled_version: DslInstanceVersion,
    pub onboarding_session: OnboardingSessionRecord,
    pub cbu_id: Uuid,
    pub dsl_storage_keys: DslStorageKeys,
    pub created_at: DateTime<Utc>,
}

/// DSL storage keys for OB request indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslStorageKeys {
    pub dsl_ob_version_id: Uuid,
    pub cbu_id: Uuid,
    pub ob_request_id: Uuid,
    pub storage_index: String,
}

/// Onboarding session record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingSessionRecord {
    pub onboarding_id: Uuid,
    pub cbu_id: Uuid,
    pub current_state: String,
    pub current_version: i32,
    pub latest_dsl_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain enhanced visualization structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEnhancedVisualization {
    pub base_visualization: ASTVisualization,
    pub domain_context: HashMap<String, serde_json::Value>,
    pub highlighted_nodes: Vec<String>,
    pub workflow_progression: Option<WorkflowProgression>,
    pub critical_paths: Vec<CriticalPath>,
}

/// Workflow progression information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgression {
    pub current_stage: String,
    pub completed_stages: Vec<String>,
    pub remaining_stages: Vec<String>,
    pub progression_percentage: f32,
}

/// Critical path in workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalPath {
    pub path_id: String,
    pub nodes: Vec<String>,
    pub estimated_duration: Option<u64>,
    pub risk_level: String,
}

impl Default for StylingConfig {
    fn default() -> Self {
        let color_scheme = serde_json::json!({
            "primary": "blue",
            "secondary": "gray",
            "success": "green",
            "warning": "orange",
            "error": "red"
        });

        Self {
            theme: "default".to_string(),
            node_size: 10.0,
            font_size: 12.0,
            color_scheme,
        }
    }
}

/// Filter configuration for visualizations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterConfig {
    pub node_types: Option<Vec<String>>,
    pub edge_types: Option<Vec<String>>,
    pub max_depth: Option<usize>,
    pub hide_empty_nodes: bool,
}

// Default derived on FilterConfig

/// AST Visualization structure from V2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASTVisualization {
    pub metadata: VisualizationMetadata,
    pub root_node: Option<VisualNode>,
    pub nodes: Vec<VisualNode>,
    pub edges: Vec<VisualEdge>,
    pub statistics: VisualizationStatistics,
}

/// Visualization metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationMetadata {
    pub generated_at: DateTime<Utc>,
    pub parser_version: String,
    pub grammar_version: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub instance_id: Option<Uuid>,
    pub version_id: Option<Uuid>,
}

/// Visual node representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub properties: HashMap<String, Value>,
    pub position: Option<NodePosition>,
    pub styling: NodeStyling,
    pub domain_annotations: Vec<String>,
    pub priority_level: i32,
    pub functional_relevance: f32,
}

/// Visual edge representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub label: Option<String>,
    pub styling: EdgeStyling,
    pub weight: f32,
}

/// Node position for layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
    pub z: Option<f32>,
}

/// Node styling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStyling {
    pub color: String,
    pub border_color: String,
    pub border_width: f32,
    pub shape: String,
    pub size: f32,
}

impl Default for NodeStyling {
    fn default() -> Self {
        Self {
            color: "blue".to_string(),
            border_color: "darkblue".to_string(),
            border_width: 2.0,
            shape: "circle".to_string(),
            size: 10.0,
        }
    }
}

/// Edge styling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeStyling {
    pub color: String,
    pub width: f32,
    pub style: String,
    pub arrow_type: String,
}

impl Default for EdgeStyling {
    fn default() -> Self {
        Self {
            color: "gray".to_string(),
            width: 2.0,
            style: "solid".to_string(),
            arrow_type: "arrow".to_string(),
        }
    }
}

/// Visualization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationStatistics {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub max_depth: usize,
    pub complexity_score: f32,
    pub compilation_time_ms: u64,
    pub visualization_time_ms: u64,
}

// ============================================================================
// DEFAULT IMPLEMENTATIONS
// ============================================================================

impl Default for VisualizationOptions {
    fn default() -> Self {
        Self {
            layout: LayoutType::Tree,
            styling: StylingConfig::default(),
            include_compilation_info: true,
            include_domain_context: true,
            filters: Some(FilterConfig::default()),
        }
    }
}

impl Default for DomainVisualizationOptions {
    fn default() -> Self {
        Self {
            base_options: VisualizationOptions::default(),
            highlight_current_state: true,
            show_state_transitions: true,
            include_domain_metrics: true,
            show_workflow_progression: true,
            emphasize_critical_paths: true,
            domain_specific_styling: true,
        }
    }
}

// ============================================================================
// ERROR EXTENSIONS FOR CONSOLIDATED MANAGER
// ============================================================================

/// Extended DSL Error types for the consolidated manager
impl DslError {
    /// Template-related error
    pub fn template_error(message: String) -> Self {
        DslError::TemplateError { message }
    }
}
