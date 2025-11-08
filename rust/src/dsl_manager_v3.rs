//! DSL Manager V3 - Business Request Lifecycle Integration
//!
//! This module provides a comprehensive interface for managing DSL business requests
//! with complete lifecycle management. V3 adds business context awareness on top of
//! the domain-specific visualization capabilities from V2.
//!
//! Key Features:
//! - Business request lifecycle management (KYC.Case, Onboarding.request, etc.)
//! - DSL amendment tracking with request context
//! - Workflow state progression management
//! - Integration with domain-specific visualization from V2

use crate::database::{DslBusinessRequestRepository, DslDomainRepository};
use crate::database::{DslBusinessRequestRepositoryTrait, DslDomainRepositoryTrait};
use crate::domain_visualizations::{DomainEnhancedVisualization, DomainVisualizer};
use crate::error::DslError;
use crate::models::business_request_models::*;
use crate::models::domain_models::*;
use crate::parser::parse_program;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced DSL Manager V3 with business request lifecycle support
pub struct DslManagerV3 {
    domain_repository: DslDomainRepository,
    business_request_repository: DslBusinessRequestRepository,
    grammar_version: String,
    parser_version: String,
    domain_visualizer: DomainVisualizer,
}

impl DslManagerV3 {
    /// Create a new DSL manager with both repositories
    pub fn new(
        domain_repository: DslDomainRepository,
        business_request_repository: DslBusinessRequestRepository,
    ) -> Self {
        Self {
            domain_repository,
            business_request_repository,
            grammar_version: "1.0.0".to_string(),
            parser_version: env!("CARGO_PKG_VERSION").to_string(),
            domain_visualizer: DomainVisualizer::new(),
        }
    }

    // ============================================================================
    // BUSINESS REQUEST LIFECYCLE MANAGEMENT
    // ============================================================================

    /// Create a new KYC case with initial DSL
    pub async fn create_kyc_case(
        &self,
        business_reference: String,
        client_id: String,
        created_by: String,
        initial_dsl_code: Option<&str>,
    ) -> DslResult<DslBusinessRequest> {
        info!(
            "Creating new KYC case: {} for client: {}",
            business_reference, client_id
        );

        let new_request =
            NewDslBusinessRequest::new_kyc_case(business_reference, client_id, created_by);

        let request = self
            .business_request_repository
            .create_business_request(new_request, initial_dsl_code)
            .await?;

        info!(
            "Created KYC case {} with request_id: {}",
            request.business_reference, request.request_id
        );

        Ok(request)
    }

    /// Create a new onboarding request with initial DSL
    pub async fn create_onboarding_request(
        &self,
        business_reference: String,
        client_id: String,
        created_by: String,
        initial_dsl_code: Option<&str>,
    ) -> DslResult<DslBusinessRequest> {
        info!(
            "Creating new onboarding request: {} for client: {}",
            business_reference, client_id
        );

        let new_request = NewDslBusinessRequest::new_onboarding_request(
            business_reference,
            client_id,
            created_by,
        );

        let request = self
            .business_request_repository
            .create_business_request(new_request, initial_dsl_code)
            .await?;

        info!(
            "Created onboarding request {} with request_id: {}",
            request.business_reference, request.request_id
        );

        Ok(request)
    }

    /// Create a new account opening request with initial DSL
    pub async fn create_account_opening(
        &self,
        business_reference: String,
        client_id: String,
        created_by: String,
        initial_dsl_code: Option<&str>,
    ) -> DslResult<DslBusinessRequest> {
        info!(
            "Creating new account opening: {} for client: {}",
            business_reference, client_id
        );

        let new_request =
            NewDslBusinessRequest::new_account_opening(business_reference, client_id, created_by);

        let request = self
            .business_request_repository
            .create_business_request(new_request, initial_dsl_code)
            .await?;

        info!(
            "Created account opening {} with request_id: {}",
            request.business_reference, request.request_id
        );

        Ok(request)
    }

    /// Get business request by ID
    pub async fn get_business_request(
        &self,
        request_id: &Uuid,
    ) -> DslResult<Option<DslBusinessRequest>> {
        debug!("Fetching business request: {}", request_id);
        self.business_request_repository
            .get_business_request(request_id)
            .await
    }

    /// Get business request by reference (case number, etc.)
    pub async fn get_business_request_by_reference(
        &self,
        domain_name: &str,
        business_reference: &str,
    ) -> DslResult<Option<DslBusinessRequest>> {
        debug!(
            "Fetching business request by reference: {} in domain: {}",
            business_reference, domain_name
        );
        self.business_request_repository
            .get_business_request_by_reference(domain_name, business_reference)
            .await
    }

    /// List business requests with filtering
    pub async fn list_business_requests(
        &self,
        domain_name: Option<&str>,
        request_status: Option<RequestStatus>,
        assigned_to: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> DslResult<Vec<ActiveBusinessRequestView>> {
        debug!("Listing business requests with filters");
        self.business_request_repository
            .list_business_requests(domain_name, request_status, assigned_to, limit, offset)
            .await
    }

    /// Update business request details
    pub async fn update_business_request(
        &self,
        request_id: &Uuid,
        updates: UpdateDslBusinessRequest,
    ) -> DslResult<DslBusinessRequest> {
        info!("Updating business request: {}", request_id);
        self.business_request_repository
            .update_business_request(request_id, updates)
            .await
    }

    // ============================================================================
    // DSL AMENDMENT MANAGEMENT
    // ============================================================================

    /// Create a DSL amendment for an existing business request
    pub async fn create_dsl_amendment(
        &self,
        request_id: &Uuid,
        dsl_source_code: &str,
        functional_state: Option<&str>,
        change_description: Option<&str>,
        created_by: &str,
    ) -> DslResult<Uuid> {
        info!(
            "Creating DSL amendment for business request: {}",
            request_id
        );

        // Validate the DSL before creating amendment
        self.validate_dsl_content(dsl_source_code)?;

        let version_id = self
            .business_request_repository
            .create_dsl_amendment(
                request_id,
                dsl_source_code,
                functional_state,
                change_description,
                created_by,
            )
            .await?;

        info!(
            "Created DSL amendment version {} for request {}",
            version_id, request_id
        );

        Ok(version_id)
    }

    /// Compile DSL version and update business request workflow
    pub async fn compile_request_dsl(
        &self,
        request_id: &Uuid,
        version_id: &Uuid,
    ) -> DslResult<ParsedAst> {
        info!(
            "Compiling DSL version {} for business request {}",
            version_id, request_id
        );

        // Get the DSL version
        let version = self
            .domain_repository
            .get_dsl_version_by_id(version_id)
            .await?
            .ok_or_else(|| DslError::NotFound {
                id: format!("dsl_version: {}", version_id),
            })?;

        // Compile the DSL
        let ast = parse_program(&version.dsl_source_code)
            .map_err(|e| DslError::ParseError(format!("Parse failed: {:?}", e)))?;

        // Store the parsed AST
        let ast_json = serde_json::to_value(&ast)
            .map_err(|e| DslError::CompileError(format!("JSON serialization failed: {}", e)))?;

        let new_ast = NewParsedAst {
            version_id: *version_id,
            ast_json,
            parse_metadata: None,
            grammar_version: self.grammar_version.clone(),
            parser_version: self.parser_version.clone(),
            ast_hash: None, // TODO: Calculate hash
            node_count: None,
            complexity_score: None,
        };

        let parsed_ast = self.domain_repository.store_parsed_ast(new_ast).await?;

        // Update compilation status
        self.domain_repository
            .update_compilation_status(version_id, CompilationStatus::Compiled)
            .await?;

        // Automatically transition workflow state to compiled
        let _state = self
            .business_request_repository
            .transition_workflow_state(
                request_id,
                "compiled_ready_for_review",
                Some("DSL successfully compiled and ready for review"),
                "SYSTEM",
                None,
            )
            .await?;

        info!(
            "Successfully compiled DSL version {} for request {}",
            version_id, request_id
        );

        Ok(parsed_ast)
    }

    // ============================================================================
    // WORKFLOW STATE MANAGEMENT
    // ============================================================================

    /// Get current workflow state for a business request
    pub async fn get_current_workflow_state(
        &self,
        request_id: &Uuid,
    ) -> DslResult<Option<DslRequestWorkflowState>> {
        debug!(
            "Fetching current workflow state for request: {}",
            request_id
        );
        self.business_request_repository
            .get_current_workflow_state(request_id)
            .await
    }

    /// Get complete workflow history for a business request
    pub async fn get_workflow_history(
        &self,
        request_id: &Uuid,
    ) -> DslResult<Vec<RequestWorkflowHistory>> {
        debug!("Fetching workflow history for request: {}", request_id);
        self.business_request_repository
            .get_workflow_history(request_id)
            .await
    }

    /// Transition business request to new workflow state
    pub async fn transition_workflow_state(
        &self,
        request_id: &Uuid,
        new_state: &str,
        state_description: Option<&str>,
        entered_by: &str,
        state_data: Option<Value>,
    ) -> DslResult<DslRequestWorkflowState> {
        info!(
            "Transitioning business request {} to state: {}",
            request_id, new_state
        );

        let state = self
            .business_request_repository
            .transition_workflow_state(
                request_id,
                new_state,
                state_description,
                entered_by,
                state_data,
            )
            .await?;

        // Auto-update business request status based on workflow state
        self.auto_update_request_status(request_id, new_state)
            .await?;

        info!(
            "Successfully transitioned request {} to state: {}",
            request_id, new_state
        );

        Ok(state)
    }

    /// Automatically update business request status based on workflow state
    async fn auto_update_request_status(
        &self,
        request_id: &Uuid,
        workflow_state: &str,
    ) -> DslResult<()> {
        let new_status = match workflow_state {
            "initial_draft" => Some(RequestStatus::Draft),
            "collecting_documents" | "identity_verification" | "document_collection" => {
                Some(RequestStatus::InProgress)
            }
            "compliance_review" | "review_required" | "approval_workflow" => {
                Some(RequestStatus::Review)
            }
            "approved" => Some(RequestStatus::Approved),
            "completed" => Some(RequestStatus::Completed),
            _ => None,
        };

        if let Some(status) = new_status {
            let updates = UpdateDslBusinessRequest {
                request_status: Some(status),
                priority_level: None,
                request_title: None,
                request_description: None,
                business_context: None,
                assigned_to: None,
                reviewed_by: None,
                completed_by: None,
                due_date: None,
                regulatory_requirements: None,
            };

            self.business_request_repository
                .update_business_request(request_id, updates)
                .await?;
        }

        Ok(())
    }

    // ============================================================================
    // ENHANCED VISUALIZATION WITH BUSINESS CONTEXT
    // ============================================================================

    /// Build AST visualization with business request context
    pub async fn build_business_request_visualization(
        &self,
        request_id: &Uuid,
        version_id: Option<&Uuid>,
        options: Option<VisualizationOptions>,
    ) -> DslResult<BusinessRequestVisualization> {
        info!(
            "Building visualization for business request: {}",
            request_id
        );

        // Get business request details
        let business_request = self
            .business_request_repository
            .get_business_request(request_id)
            .await?
            .ok_or_else(|| DslError::NotFound {
                id: format!("business_request: {}", request_id),
            })?;

        // Get domain information
        let domain = self
            .domain_repository
            .get_domain_by_id(&business_request.domain_id)
            .await?
            .ok_or_else(|| DslError::NotFound {
                id: format!("domain: {}", business_request.domain_id),
            })?;

        // Determine which version to use
        let target_version_id = if let Some(vid) = version_id {
            *vid
        } else {
            // Get latest version for this request
            let versions = self
                .domain_repository
                .list_versions(&domain.domain_name, Some(1))
                .await?;

            let latest_version = versions
                .into_iter()
                .find(|v| v.request_id == Some(*request_id))
                .ok_or_else(|| DslError::NotFound {
                    id: format!("dsl_version for request: {}", request_id),
                })?;

            latest_version.version_id
        };

        // Get domain-enhanced visualization using V2 capabilities
        let enhanced_viz = self
            .build_domain_enhanced_visualization_by_version_id(&target_version_id, options)
            .await?;

        // Get current workflow state
        let current_workflow_state = self
            .business_request_repository
            .get_current_workflow_state(request_id)
            .await?;

        // Get business request summary
        let request_summary = self
            .business_request_repository
            .get_business_request_summary(request_id)
            .await?;

        let business_viz = BusinessRequestVisualization {
            request_id: *request_id,
            business_reference: business_request.business_reference,
            request_type: business_request.request_type,
            request_status: business_request.request_status,
            domain_enhanced_visualization: enhanced_viz,
            current_workflow_state,
            request_summary,
            version_id: target_version_id,
        };

        info!(
            "Successfully built visualization for business request: {}",
            request_id
        );

        Ok(business_viz)
    }

    // Delegate to V2 domain visualization capabilities
    pub async fn build_domain_enhanced_visualization_by_version_id(
        &self,
        version_id: &Uuid,
        options: Option<VisualizationOptions>,
    ) -> DslResult<DomainEnhancedVisualization> {
        debug!(
            "Building domain-enhanced visualization for version: {}",
            version_id
        );

        // Get the DSL version
        let version = self
            .domain_repository
            .get_dsl_version_by_id(version_id)
            .await?
            .ok_or_else(|| DslError::NotFound {
                id: format!("dsl_version: {}", version_id),
            })?;

        // Get domain
        let domain = self
            .domain_repository
            .get_domain_by_id(&version.domain_id)
            .await?
            .ok_or_else(|| DslError::NotFound {
                id: format!("domain: {}", version.domain_id),
            })?;

        // Get or create parsed AST
        let parsed_ast = match self.domain_repository.get_parsed_ast(version_id).await? {
            Some(ast) => ast,
            None => {
                warn!(
                    "No parsed AST found for version {}, compiling...",
                    version_id
                );
                self.compile_version(&version).await?
            }
        };

        // Parse AST from JSON
        let ast_root = parsed_ast.ast_json;

        // Build base visualization
        let base_visualization = self.build_ast_visualization_from_json(&ast_root, options)?;

        // Apply domain-specific enhancements
        let enhanced_visualization = self
            .domain_visualizer
            .enhance_visualization(base_visualization, &domain.domain_name);

        Ok(enhanced_visualization)
    }

    // ============================================================================
    // ANALYTICS AND REPORTING
    // ============================================================================

    /// Get business request summary
    pub async fn get_business_request_summary(
        &self,
        request_id: &Uuid,
    ) -> DslResult<Option<BusinessRequestSummary>> {
        debug!("Fetching business request summary: {}", request_id);
        self.business_request_repository
            .get_business_request_summary(request_id)
            .await
    }

    /// Get domain request statistics
    pub async fn get_domain_request_statistics(
        &self,
        domain_name: &str,
        days_back: Option<i32>,
    ) -> DslResult<DomainRequestStatistics> {
        debug!("Fetching domain request statistics for: {}", domain_name);
        self.business_request_repository
            .get_domain_request_statistics(domain_name, days_back)
            .await
    }

    /// List available request types
    pub async fn list_request_types(&self) -> DslResult<Vec<DslRequestType>> {
        debug!("Fetching all request types");
        self.business_request_repository.list_request_types().await
    }

    // ============================================================================
    // PRIVATE HELPER METHODS
    // ============================================================================

    /// Validate DSL content before processing
    fn validate_dsl_content(&self, dsl_content: &str) -> DslResult<()> {
        if dsl_content.trim().is_empty() {
            return Err(DslError::ValidationFailed {
                message: "DSL content cannot be empty".to_string(),
            });
        }

        // Basic syntax validation
        parse_program(dsl_content).map_err(|e| DslError::ParseError {
            message: format!("DSL validation failed: {:?}", e),
        })?;

        Ok(())
    }

    /// Compile a DSL version and store the AST
    async fn compile_version(&self, version: &DslVersion) -> DslResult<ParsedAst> {
        info!("Compiling DSL version: {}", version.version_id);

        let ast = parse_program(&version.dsl_source_code)
            .map_err(|e| DslError::ParseError(format!("Parse failed: {:?}", e)))?;

        let ast_json = serde_json::to_value(&ast)
            .map_err(|e| DslError::CompileError(format!("JSON serialization failed: {}", e)))?;

        let new_ast = NewParsedAst {
            version_id: version.version_id,
            ast_json,
            parse_metadata: None,
            grammar_version: self.grammar_version.clone(),
            parser_version: self.parser_version.clone(),
            ast_hash: None, // TODO: Calculate hash
            node_count: None,
            complexity_score: None,
        };

        let parsed_ast = self.domain_repository.store_parsed_ast(new_ast).await?;

        // Update compilation status
        self.domain_repository
            .update_compilation_status(&version.version_id, CompilationStatus::Compiled)
            .await?;

        Ok(parsed_ast)
    }

    /// Build AST visualization from JSON representation
    fn build_ast_visualization_from_json(
        &self,
        ast_json: &Value,
        _options: Option<VisualizationOptions>,
    ) -> DslResult<ASTVisualization> {
        // This is a simplified implementation
        // In a full implementation, you'd parse the JSON back to AST and build visualization
        let metadata = VisualizationMetadata {
            generated_at: Utc::now(),
            parser_version: self.parser_version.clone(),
            grammar_version: self.grammar_version.clone(),
            node_count: 0, // TODO: Calculate from AST
            edge_count: 0, // TODO: Calculate from AST
        };

        Ok(ASTVisualization {
            metadata,
            root_node: VisualNode {
                id: "root".to_string(),
                label: "Program".to_string(),
                node_type: "Program".to_string(),
                properties: HashMap::new(),
                position: None,
                styling: None,
                domain_annotations: None,
                priority_level: None,
                functional_relevance: None,
            },
            edges: Vec::new(),
            statistics: VisualizationStatistics {
                total_nodes: 1,
                total_edges: 0,
                max_depth: 1,
                complexity_score: 1.0,
            },
        })
    }
}

// ============================================================================
// RESULT TYPES AND SUPPORTING STRUCTURES
// ============================================================================

/// Result type for DSL operations
pub type DslResult<T> = Result<T, DslError>;

/// Business Request Visualization combining domain visualization with business context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessRequestVisualization {
    pub request_id: Uuid,
    pub business_reference: String,
    pub request_type: String,
    pub request_status: RequestStatus,
    pub domain_enhanced_visualization: DomainEnhancedVisualization,
    pub current_workflow_state: Option<DslRequestWorkflowState>,
    pub request_summary: Option<BusinessRequestSummary>,
    pub version_id: Uuid,
}

/// Visualization options for customizing output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationOptions {
    pub layout: Option<LayoutType>,
    pub styling: Option<StylingOptions>,
    pub filters: Option<FilterConfig>,
    pub output_format: Option<OutputFormat>,
    pub include_annotations: bool,
    pub show_metadata: bool,
}

impl Default for VisualizationOptions {
    fn default() -> Self {
        Self {
            layout: Some(LayoutType::Hierarchical),
            styling: Some(StylingOptions::default()),
            filters: None,
            output_format: Some(OutputFormat::Json),
            include_annotations: true,
            show_metadata: true,
        }
    }
}

/// Layout types for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutType {
    Hierarchical,
    ForceDirected,
    Circular,
    Grid,
}

/// Styling options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StylingOptions {
    pub theme: String,
    pub node_size: f32,
    pub font_size: f32,
    pub color_scheme: String,
}

impl Default for StylingOptions {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            node_size: 50.0,
            font_size: 12.0,
            color_scheme: "default".to_string(),
        }
    }
}

/// Filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub node_types: Option<Vec<String>>,
    pub edge_types: Option<Vec<String>>,
    pub max_depth: Option<i32>,
    pub hide_empty_nodes: bool,
}

/// Output format options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Json,
    Svg,
    Png,
    Pdf,
}

/// AST Visualization structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASTVisualization {
    pub metadata: VisualizationMetadata,
    pub root_node: VisualNode,
    pub edges: Vec<VisualEdge>,
    pub statistics: VisualizationStatistics,
}

/// Visualization metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationMetadata {
    pub generated_at: DateTime<Utc>,
    pub parser_version: String,
    pub grammar_version: String,
    pub node_count: i32,
    pub edge_count: i32,
}

/// Visual representation of an AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub properties: HashMap<String, Value>,
    pub position: Option<(f32, f32)>,
    pub styling: Option<NodeStyling>,
    pub domain_annotations: Option<Vec<String>>,
    pub priority_level: Option<u8>,
    pub functional_relevance: Option<f32>,
}

/// Visual representation of an edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub label: Option<String>,
    pub styling: Option<EdgeStyling>,
}

/// Node styling information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStyling {
    pub color: String,
    pub border_color: String,
    pub border_width: f32,
    pub shape: String,
    pub size: f32,
}

/// Edge styling information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeStyling {
    pub color: String,
    pub width: f32,
    pub style: String, // solid, dashed, dotted
    pub arrow_type: String,
}

/// Visualization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationStatistics {
    pub total_nodes: i32,
    pub total_edges: i32,
    pub max_depth: i32,
    pub complexity_score: f64,
}

// Re-export domain request statistics from business request repository
pub use crate::database::business_request_repository::DomainRequestStatistics;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{DslBusinessRequestRepository, DslDomainRepository};
    use sqlx::PgPool;

    async fn create_test_manager() -> DslManagerV3 {
        // This would require a test database setup in real implementation
        let pool = PgPool::connect("postgresql://localhost/test_db")
            .await
            .unwrap();
        let domain_repo = DslDomainRepository::new(pool.clone());
        let business_repo = DslBusinessRequestRepository::new(pool);

        DslManagerV3::new(domain_repo, business_repo)
    }

    #[tokio::test]
    async fn test_validate_dsl_content() {
        let manager = create_test_manager().await;

        // Valid DSL should pass
        let valid_dsl = r#"
            WORKFLOW "Test"
            BEGIN
                DECLARE_ENTITY "test" { entity_type: "test" }
            END
        "#;

        assert!(manager.validate_dsl_content(valid_dsl).is_ok());

        // Empty DSL should fail
        assert!(manager.validate_dsl_content("").is_err());
        assert!(manager.validate_dsl_content("   ").is_err());
    }

    #[test]
    fn test_visualization_options_default() {
        let options = VisualizationOptions::default();
        assert!(matches!(options.layout, Some(LayoutType::Hierarchical)));
        assert!(options.include_annotations);
        assert!(options.show_metadata);
    }

    #[test]
    fn test_request_status_workflow_mapping() {
        // Test the auto-status mapping logic would be implemented here
        assert!(true); // Placeholder
    }
}
