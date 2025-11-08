//! DSL Manager V2 - Database-backed DSL Management
//!
//! This module provides a comprehensive interface for managing domain DSL definitions
//! with database persistence, AST compilation and storage, and execution pipeline integration.

use crate::ast::Program;
use crate::database::dsl_domain_repository::DslDomainRepositoryTrait;
use crate::database::DslDomainRepository;
use crate::models::{
    CompilationStatus, DslDomain, DslVersion, NewDslVersion, NewParsedAst, ParsedAst,
};
use crate::parser::parse_program;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Enhanced DSL Manager with database backend and compilation pipeline
pub struct DslManagerV2 {
    repository: DslDomainRepository,
    grammar_version: String,
    parser_version: String,
}

impl DslManagerV2 {
    /// Create a new DSL manager with database repository
    pub fn new(repository: DslDomainRepository) -> Self {
        Self {
            repository,
            grammar_version: "1.0.0".to_string(),
            parser_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    // ============================================================================
    // DOMAIN MANAGEMENT
    // ============================================================================

    /// List all available DSL domains
    pub async fn list_domains(&self, active_only: bool) -> DslResult<Vec<DslDomain>> {
        self.repository
            .list_domains(active_only)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))
    }

    /// Get domain by name
    pub async fn get_domain(&self, domain_name: &str) -> DslResult<DslDomain> {
        self.repository
            .get_domain_by_name(domain_name)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("domain: {}", domain_name),
            })
    }

    // ============================================================================
    // VERSION MANAGEMENT
    // ============================================================================

    /// Create a new DSL version
    pub async fn create_dsl_version(
        &self,
        domain_name: &str,
        dsl_source_code: &str,
        functional_state: Option<&str>,
        change_description: Option<&str>,
        created_by: Option<&str>,
    ) -> DslResult<DslVersion> {
        // Validate DSL content before creating version
        self.validate_dsl_content(dsl_source_code)?;

        let new_version = NewDslVersion {
            domain_name: domain_name.to_string(),
            functional_state: functional_state.map(|s| s.to_string()),
            dsl_source_code: dsl_source_code.to_string(),
            change_description: change_description.map(|s| s.to_string()),
            parent_version_id: None, // TODO: Support branching later
            created_by: created_by.map(|s| s.to_string()),
        };

        let version = self
            .repository
            .create_new_version(new_version)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        info!(
            "Created new DSL version: {} v{}",
            domain_name, version.version_number
        );

        Ok(version)
    }

    /// Get specific DSL version
    pub async fn get_dsl_version(
        &self,
        domain_name: &str,
        version_number: i32,
    ) -> DslResult<DslVersion> {
        self.repository
            .get_dsl_version(domain_name, version_number)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("{}:v{}", domain_name, version_number),
            })
    }

    /// Get DSL version by ID
    pub async fn get_dsl_version_by_id(&self, version_id: &Uuid) -> DslResult<DslVersion> {
        self.repository
            .get_dsl_version_by_id(version_id)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("version_id: {}", version_id),
            })
    }

    /// Get latest version of a domain
    pub async fn get_latest_version(&self, domain_name: &str) -> DslResult<DslVersion> {
        self.repository
            .get_latest_version(domain_name)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("latest version of domain: {}", domain_name),
            })
    }

    /// List versions for a domain
    pub async fn list_versions(
        &self,
        domain_name: &str,
        limit: Option<i32>,
    ) -> DslResult<Vec<DslVersion>> {
        self.repository
            .list_versions(domain_name, limit)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))
    }

    // ============================================================================
    // DSL COMPILATION PIPELINE
    // ============================================================================

    /// Core DSL compilation pipeline - parse and store AST
    pub async fn compile_dsl_version(
        &self,
        domain_name: &str,
        version_number: i32,
        force_recompile: bool,
    ) -> DslResult<ParsedAst> {
        // 1. Get DSL version
        let dsl_version = self.get_dsl_version(domain_name, version_number).await?;

        // 2. Check if AST already exists and is valid
        if !force_recompile {
            if let Some(existing_ast) = self
                .repository
                .get_parsed_ast(&dsl_version.version_id)
                .await
                .map_err(|e| DslError::DatabaseError(e.to_string()))?
            {
                if existing_ast.is_valid() {
                    debug!("Using cached AST for {} v{}", domain_name, version_number);
                    return Ok(existing_ast);
                }
            }
        }

        // 3. Update compilation status
        self.update_compilation_status(&dsl_version.version_id, CompilationStatus::Compiling)
            .await?;

        // 4. Parse DSL source code to AST
        let parse_start = std::time::Instant::now();
        let ast = match parse_program(&dsl_version.dsl_source_code) {
            Ok(program) => program,
            Err(e) => {
                self.update_compilation_status(&dsl_version.version_id, CompilationStatus::Error)
                    .await?;
                return Err(DslError::ParseError {
                    message: format!("Failed to parse DSL: {}", e),
                });
            }
        };
        let parse_duration = parse_start.elapsed();

        // 5. Calculate AST metadata
        let ast_json = serde_json::to_value(&ast).map_err(|e| DslError::SerializationError {
            message: format!("Failed to serialize AST: {}", e),
        })?;

        let ast_hash = self.calculate_ast_hash(&ast_json);
        let node_count = self.count_ast_nodes(&ast);
        let complexity_score = self.calculate_complexity_score(&ast);

        let parse_metadata = serde_json::json!({
            "parse_duration_ms": parse_duration.as_millis(),
            "parser_version": self.parser_version,
            "grammar_version": self.grammar_version,
            "timestamp": Utc::now(),
            "warnings": [], // TODO: Add parser warnings
            "source_size_bytes": dsl_version.dsl_source_code.len()
        });

        // 6. Create parsed AST record
        let new_ast = NewParsedAst {
            version_id: dsl_version.version_id,
            ast_json,
            parse_metadata: Some(parse_metadata),
            grammar_version: self.grammar_version.clone(),
            parser_version: self.parser_version.clone(),
            ast_hash: Some(ast_hash),
            node_count: Some(node_count),
            complexity_score: Some(complexity_score),
        };

        // 7. Store AST in database
        let parsed_ast = self
            .repository
            .store_parsed_ast(new_ast)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?;

        // 8. Update compilation status to completed
        self.update_compilation_status(&dsl_version.version_id, CompilationStatus::Compiled)
            .await?;

        info!(
            "Successfully compiled {} v{} - AST nodes: {}, complexity: {:?}",
            domain_name, version_number, node_count, complexity_score
        );

        Ok(parsed_ast)
    }

    /// Compile DSL version by ID
    pub async fn compile_dsl_version_by_id(
        &self,
        version_id: &Uuid,
        force_recompile: bool,
    ) -> DslResult<ParsedAst> {
        let version = self.get_dsl_version_by_id(version_id).await?;
        let domain = self
            .repository
            .get_domain_by_id(&version.domain_id)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("domain_id: {}", version.domain_id),
            })?;

        self.compile_dsl_version(&domain.domain_name, version.version_number, force_recompile)
            .await
    }

    /// Get parsed AST for a version
    pub async fn get_parsed_ast(&self, version_id: &Uuid) -> DslResult<ParsedAst> {
        self.repository
            .get_parsed_ast(version_id)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("parsed AST for version_id: {}", version_id),
            })
    }

    /// Invalidate AST cache (force recompilation next time)
    pub async fn invalidate_ast_cache(&self, version_id: &Uuid) -> DslResult<()> {
        self.repository
            .invalidate_ast(&version_id)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))
    }

    // ============================================================================
    // AST VISUALIZATION (Main feature for this phase)
    // ============================================================================

    /// Build AST visualization for domain and version
    pub async fn build_ast_visualization(
        &self,
        domain_name: &str,
        version_number: i32,
        options: Option<VisualizationOptions>,
    ) -> DslResult<ASTVisualization> {
        // 1. Get or compile AST
        let parsed_ast = self
            .compile_dsl_version(domain_name, version_number, false)
            .await?;

        // 2. Deserialize AST from JSON
        let ast: Program = serde_json::from_value(parsed_ast.ast_json.clone()).map_err(|e| {
            DslError::SerializationError {
                message: format!("Failed to deserialize AST: {}", e),
            }
        })?;

        // 3. Get domain and version context
        let domain = self.get_domain(domain_name).await?;
        let dsl_version = self.get_dsl_version(domain_name, version_number).await?;

        // 4. Build visualization with domain context
        let mut builder =
            ASTVisualizationBuilder::new().with_domain_context(&domain, &dsl_version, &parsed_ast);

        if let Some(opts) = &options {
            if let Some(layout) = opts.layout {
                builder = builder.with_layout(layout);
            }
            if let Some(ref styling) = opts.styling {
                builder = builder.with_styling(styling.clone());
            }
            if let Some(ref filters) = opts.filters {
                builder = builder.with_filters(filters.clone());
            }
        }

        let visualization = builder.from_ast(&ast)?;

        info!(
            "Generated AST visualization for {} v{} - {} nodes, {} edges",
            domain_name,
            version_number,
            visualization.root_node.children.len(),
            visualization.edges.len()
        );

        Ok(visualization)
    }

    /// Build AST visualization by version ID
    pub async fn build_ast_visualization_by_version_id(
        &self,
        version_id: &Uuid,
        options: Option<VisualizationOptions>,
    ) -> DslResult<ASTVisualization> {
        let version = self.get_dsl_version_by_id(version_id).await?;
        let domain = self
            .repository
            .get_domain_by_id(&version.domain_id)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("domain_id: {}", version.domain_id),
            })?;

        self.build_ast_visualization(&domain.domain_name, version.version_number, options)
            .await
    }

    /// Build AST visualization for latest version
    pub async fn build_ast_visualization_latest(
        &self,
        domain_name: &str,
        options: Option<VisualizationOptions>,
    ) -> DslResult<ASTVisualization> {
        let version = self.get_latest_version(domain_name).await?;
        self.build_ast_visualization(domain_name, version.version_number, options)
            .await
    }

    // ============================================================================
    // UTILITY METHODS
    // ============================================================================

    /// Validate DSL content before storing
    fn validate_dsl_content(&self, content: &str) -> DslResult<()> {
        if content.trim().is_empty() {
            return Err(DslError::InvalidContent {
                reason: "DSL content cannot be empty".to_string(),
            });
        }

        // Basic syntax check - should be valid S-expression
        let trimmed = content.trim();
        if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
            return Err(DslError::InvalidContent {
                reason: "DSL content should be a valid S-expression".to_string(),
            });
        }

        // TODO: Add more sophisticated validation:
        // - Grammar validation
        // - Vocabulary checking
        // - Domain-specific rules

        Ok(())
    }

    /// Update compilation status in database
    async fn update_compilation_status(
        &self,
        version_id: &Uuid,
        status: CompilationStatus,
    ) -> DslResult<()> {
        self.repository
            .update_compilation_status(version_id, status)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))
    }

    /// Calculate hash of AST for change detection
    fn calculate_ast_hash(&self, ast_json: &serde_json::Value) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let serialized = serde_json::to_string(ast_json).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        serialized.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Count total nodes in AST
    fn count_ast_nodes(&self, ast: &Program) -> i32 {
        // Simplified node counting - would need proper AST traversal
        ast.workflows.len() as i32
            + ast
                .workflows
                .iter()
                .map(|w| w.statements.len() as i32)
                .sum::<i32>()
    }

    /// Calculate complexity score for AST
    fn calculate_complexity_score(&self, ast: &Program) -> rust_decimal::Decimal {
        use rust_decimal::Decimal;

        // Simplified complexity calculation
        let workflow_count = ast.workflows.len() as u64;
        let statement_count: u64 = ast
            .workflows
            .iter()
            .map(|w| w.statements.len() as u64)
            .sum();

        // Basic complexity formula: workflows * 2 + statements
        Decimal::from(workflow_count * 2 + statement_count)
    }
}

// ============================================================================
// VISUALIZATION TYPES (Placeholder - would be in separate module)
// ============================================================================

/// AST visualization options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationOptions {
    pub layout: Option<LayoutType>,
    pub styling: Option<StylingConfig>,
    pub filters: Option<FilterConfig>,
    pub include_compilation_info: bool,
    pub include_domain_context: bool,
    pub show_functional_states: bool,
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LayoutType {
    Tree,
    Graph,
    Hierarchical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StylingConfig {
    pub theme: String,
    pub node_colors: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub show_only_nodes: Option<Vec<String>>,
    pub hide_nodes: Option<Vec<String>>,
}

/// AST visualization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASTVisualization {
    pub metadata: VisualizationMetadata,
    pub domain_context: DomainContext,
    pub root_node: VisualNode,
    pub edges: Vec<VisualEdge>,
    pub statistics: ASTStatistics,
    pub compilation_info: CompilationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationMetadata {
    pub generated_at: DateTime<Utc>,
    pub generator_version: String,
    pub layout_type: LayoutType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainContext {
    pub domain_name: String,
    pub version_number: i32,
    pub functional_state: Option<String>,
    pub grammar_version: String,
    pub compilation_status: CompilationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualNode {
    pub id: String,
    pub node_type: String,
    pub label: String,
    pub children: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASTStatistics {
    pub total_nodes: i32,
    pub total_edges: i32,
    pub max_depth: i32,
    pub complexity_score: rust_decimal::Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationInfo {
    pub parsed_at: DateTime<Utc>,
    pub parser_version: String,
    pub grammar_version: String,
    pub parse_duration_ms: u64,
}

/// AST visualization builder (placeholder)
pub struct ASTVisualizationBuilder {
    layout: LayoutType,
    styling: Option<StylingConfig>,
    filters: Option<FilterConfig>,
    domain: Option<DslDomain>,
    version: Option<DslVersion>,
    parsed_ast: Option<ParsedAst>,
}

impl ASTVisualizationBuilder {
    pub fn new() -> Self {
        Self {
            layout: LayoutType::Tree,
            styling: None,
            filters: None,
            domain: None,
            version: None,
            parsed_ast: None,
        }
    }

    pub fn with_domain_context(
        mut self,
        domain: &DslDomain,
        version: &DslVersion,
        parsed_ast: &ParsedAst,
    ) -> Self {
        self.domain = Some(domain.clone());
        self.version = Some(version.clone());
        self.parsed_ast = Some(parsed_ast.clone());
        self
    }

    pub fn with_layout(mut self, layout: LayoutType) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_styling(mut self, styling: StylingConfig) -> Self {
        self.styling = Some(styling);
        self
    }

    pub fn with_filters(mut self, filters: FilterConfig) -> Self {
        self.filters = Some(filters);
        self
    }

    pub fn from_ast(self, ast: &Program) -> DslResult<ASTVisualization> {
        // Placeholder implementation - would build actual visualization
        let root_node = VisualNode {
            id: "root".to_string(),
            node_type: "Program".to_string(),
            label: "DSL Program".to_string(),
            children: vec!["workflow_0".to_string()],
            properties: HashMap::new(),
        };

        let compilation_info = if let Some(parsed_ast) = &self.parsed_ast {
            CompilationInfo {
                parsed_at: parsed_ast.parsed_at,
                parser_version: parsed_ast.parser_version.clone(),
                grammar_version: parsed_ast.grammar_version.clone(),
                parse_duration_ms: 0, // Would extract from metadata
            }
        } else {
            CompilationInfo {
                parsed_at: Utc::now(),
                parser_version: "unknown".to_string(),
                grammar_version: "unknown".to_string(),
                parse_duration_ms: 0,
            }
        };

        let domain_context = DomainContext {
            domain_name: self
                .domain
                .as_ref()
                .map(|d| d.domain_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            version_number: self.version.as_ref().map(|v| v.version_number).unwrap_or(0),
            functional_state: self
                .version
                .as_ref()
                .and_then(|v| v.functional_state.clone()),
            grammar_version: self
                .parsed_ast
                .as_ref()
                .map(|a| a.grammar_version.clone())
                .unwrap_or_else(|| "1.0.0".to_string()),
            compilation_status: self
                .version
                .as_ref()
                .map(|v| v.compilation_status.clone())
                .unwrap_or(CompilationStatus::Draft),
        };

        Ok(ASTVisualization {
            metadata: VisualizationMetadata {
                generated_at: Utc::now(),
                generator_version: env!("CARGO_PKG_VERSION").to_string(),
                layout_type: self.layout,
            },
            domain_context,
            root_node,
            edges: vec![], // Would populate with actual edges
            statistics: ASTStatistics {
                total_nodes: ast.workflows.len() as i32,
                total_edges: 0,
                max_depth: 1,
                complexity_score: rust_decimal::Decimal::from(ast.workflows.len()),
            },
            compilation_info,
        })
    }
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Enhanced error types for DSL Manager V2
#[derive(Debug, thiserror::Error)]
pub enum DslError {
    #[error("DSL not found: {id}")]
    NotFound { id: String },

    #[error("DSL already exists: {id}")]
    AlreadyExists { id: String },

    #[error("Invalid DSL content: {reason}")]
    InvalidContent { reason: String },

    #[error("Validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("Domain mismatch - expected: {expected}, found: {found}")]
    DomainMismatch { expected: String, found: String },

    #[error("Compilation error: {0}")]
    CompileError(String),

    #[error("Visualization error: {0}")]
    VisualizationError(String),
}

pub type DslResult<T> = Result<T, DslError>;

impl Default for VisualizationOptions {
    fn default() -> Self {
        Self {
            layout: Some(LayoutType::Tree),
            styling: None,
            filters: None,
            include_compilation_info: true,
            include_domain_context: true,
            show_functional_states: true,
            max_depth: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visualization_options_default() {
        let options = VisualizationOptions::default();
        assert!(options.include_compilation_info);
        assert!(options.include_domain_context);
        assert_eq!(options.layout, Some(LayoutType::Tree));
    }

    #[test]
    fn test_dsl_error_display() {
        let error = DslError::NotFound {
            id: "test-domain".to_string(),
        };
        assert_eq!(error.to_string(), "DSL not found: test-domain");
    }
}
