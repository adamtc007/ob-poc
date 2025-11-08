//! DSL Manager V2 - Database-backed DSL Management
//!
//! This module provides a comprehensive interface for managing domain DSL definitions
//! with database persistence, AST compilation and storage, and execution pipeline integration.
//!
//! Phase 3 Enhancement: Domain-specific visualization with functional state support

use crate::ast::{
    GenerateReport, ParallelObtain, Program, PropertyMap, ResolveConflict, ScheduleMonitoring,
    SolicitAttribute, Statement, Value,
};
use crate::database::dsl_domain_repository::DslDomainRepositoryTrait;
use crate::database::DslDomainRepository;
use crate::domain_visualizations::{DomainEnhancedVisualization, DomainVisualizer};
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
/// Phase 3: Added domain-aware visualization capabilities
pub struct DslManagerV2 {
    repository: DslDomainRepository,
    grammar_version: String,
    parser_version: String,
    domain_visualizer: DomainVisualizer,
}

impl DslManagerV2 {
    /// Create a new DSL manager with database repository
    pub fn new(repository: DslDomainRepository) -> Self {
        Self {
            repository,
            grammar_version: "1.0.0".to_string(),
            parser_version: env!("CARGO_PKG_VERSION").to_string(),
            domain_visualizer: DomainVisualizer::new(),
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
            request_id: None, // No business request context in V2
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
    // PHASE 3: DOMAIN-AWARE VISUALIZATION METHODS
    // ============================================================================

    /// Build domain-enhanced AST visualization with functional state support
    pub async fn build_domain_enhanced_visualization(
        &self,
        domain_name: &str,
        version_number: i32,
        options: Option<VisualizationOptions>,
    ) -> DslResult<DomainEnhancedVisualization> {
        // Get the base visualization
        let base_visualization = self
            .build_ast_visualization(domain_name, version_number, options.clone())
            .await?;

        // Get domain and version information
        let domain = self.get_domain(domain_name).await?;
        let version = self.get_dsl_version(domain_name, version_number).await?;

        // Use default options if none provided
        let viz_options = options.unwrap_or_default();

        // Enhance with domain-specific features
        let enhanced_visualization = self.domain_visualizer.enhance_visualization(
            base_visualization,
            &domain,
            &version,
            &viz_options,
        );

        info!(
            "Generated domain-enhanced visualization for {} v{} with functional state: {:?}",
            domain_name, version_number, version.functional_state
        );

        Ok(enhanced_visualization)
    }

    /// Build domain-enhanced visualization by version ID
    pub async fn build_domain_enhanced_visualization_by_version_id(
        &self,
        version_id: &Uuid,
        options: Option<VisualizationOptions>,
    ) -> DslResult<DomainEnhancedVisualization> {
        let version = self.get_dsl_version_by_id(version_id).await?;
        let domain = self
            .repository
            .get_domain_by_id(&version.domain_id)
            .await
            .map_err(|e| DslError::DatabaseError(e.to_string()))?
            .ok_or_else(|| DslError::NotFound {
                id: format!("domain_id: {}", version.domain_id),
            })?;

        self.build_domain_enhanced_visualization(
            &domain.domain_name,
            version.version_number,
            options,
        )
        .await
    }

    /// Build domain-enhanced visualization for latest version
    pub async fn build_domain_enhanced_visualization_latest(
        &self,
        domain_name: &str,
        options: Option<VisualizationOptions>,
    ) -> DslResult<DomainEnhancedVisualization> {
        let version = self.get_latest_version(domain_name).await?;
        self.build_domain_enhanced_visualization(domain_name, version.version_number, options)
            .await
    }

    /// Build functional state progression visualization
    pub async fn build_functional_state_visualization(
        &self,
        domain_name: &str,
        version_number: i32,
    ) -> DslResult<crate::domain_visualizations::FunctionalStateVisualization> {
        let _domain = self.get_domain(domain_name).await?;
        let _version = self.get_dsl_version(domain_name, version_number).await?;

        let enhanced_viz = self
            .build_domain_enhanced_visualization(domain_name, version_number, None)
            .await?;

        enhanced_viz.functional_state_info.ok_or_else(|| {
            DslError::VisualizationError("No functional state information available".to_string())
        })
    }

    /// Get available functional states for a domain
    pub fn get_domain_functional_states(&self, domain_name: &str) -> Vec<String> {
        self.domain_visualizer
            .domain_rules
            .get(domain_name)
            .map(|rules| {
                rules
                    .functional_states
                    .iter()
                    .map(|state| state.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if a domain supports functional state visualization
    pub fn supports_functional_states(&self, domain_name: &str) -> bool {
        self.domain_visualizer
            .domain_rules
            .get(domain_name)
            .map(|rules| !rules.functional_states.is_empty())
            .unwrap_or(false)
    }

    /// Get domain-specific visualization highlights
    pub fn get_domain_highlights(
        &self,
        domain_name: &str,
    ) -> Vec<crate::domain_visualizations::DomainHighlight> {
        self.domain_visualizer
            .identify_domain_highlights(domain_name)
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
    pub max_depth: Option<usize>,
    pub show_properties: bool,
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
        let mut visitor = ASTVisualizationVisitor::new(self.layout);

        // Apply filters if specified
        if let Some(filters) = &self.filters {
            visitor.set_filters(filters.clone());
        }

        // Visit the AST and build the visualization
        let (nodes, edges, statistics) = visitor.visit_program(ast)?;

        let compilation_info = if let Some(parsed_ast) = &self.parsed_ast {
            let parse_duration_ms = parsed_ast
                .parse_metadata
                .as_ref()
                .and_then(|meta| meta.get("parse_duration_ms"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            CompilationInfo {
                parsed_at: parsed_ast.parsed_at,
                parser_version: parsed_ast.parser_version.clone(),
                grammar_version: parsed_ast.grammar_version.clone(),
                parse_duration_ms,
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

        // Find the root node
        let root_node = nodes
            .into_iter()
            .find(|n| n.id == "program")
            .ok_or_else(|| DslError::VisualizationError("Root node not found".to_string()))?;

        Ok(ASTVisualization {
            metadata: VisualizationMetadata {
                generated_at: Utc::now(),
                generator_version: env!("CARGO_PKG_VERSION").to_string(),
                layout_type: self.layout,
            },
            domain_context,
            root_node,
            edges,
            statistics,
            compilation_info,
        })
    }
}

/// AST Visualization Visitor
///
/// Traverses the AST and converts it into a visual graph representation
struct ASTVisualizationVisitor {
    layout: LayoutType,
    filters: Option<FilterConfig>,
    node_counter: usize,
    nodes: Vec<VisualNode>,
    edges: Vec<VisualEdge>,
    max_depth: usize,
    current_depth: usize,
}

impl ASTVisualizationVisitor {
    fn new(layout: LayoutType) -> Self {
        Self {
            layout,
            filters: None,
            node_counter: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            max_depth: 0,
            current_depth: 0,
        }
    }

    fn set_filters(&mut self, filters: FilterConfig) {
        self.filters = Some(filters);
    }

    fn next_node_id(&mut self) -> String {
        let id = format!("node_{}", self.node_counter);
        self.node_counter += 1;
        id
    }

    fn should_include_node(&self, node_type: &str) -> bool {
        if let Some(ref filters) = self.filters {
            if let Some(ref show_only) = filters.show_only_nodes {
                return show_only.contains(&node_type.to_string());
            }
            if let Some(ref hide_nodes) = filters.hide_nodes {
                return !hide_nodes.contains(&node_type.to_string());
            }
        }
        true
    }

    fn visit_program(
        &mut self,
        program: &Program,
    ) -> DslResult<(Vec<VisualNode>, Vec<VisualEdge>, ASTStatistics)> {
        let program_id = "program".to_string();
        let mut program_properties: HashMap<String, serde_json::Value> = HashMap::new();
        program_properties.insert(
            "workflow_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(program.workflows.len())),
        );

        let mut workflow_children = Vec::new();

        // Visit each workflow
        for (i, workflow) in program.workflows.iter().enumerate() {
            self.current_depth = 1;
            let workflow_id = self.visit_workflow(workflow, i)?;
            workflow_children.push(workflow_id.clone());

            // Create edge from program to workflow
            self.edges.push(VisualEdge {
                id: format!("edge_program_{}", i),
                from: program_id.clone(),
                to: workflow_id,
                edge_type: "contains".to_string(),
                properties: HashMap::new(),
            });
        }

        // Create program root node
        let program_node = VisualNode {
            id: program_id,
            node_type: "Program".to_string(),
            label: format!("DSL Program ({} workflows)", program.workflows.len()),
            children: workflow_children,
            properties: program_properties,
        };

        self.nodes.insert(0, program_node);

        let statistics = ASTStatistics {
            total_nodes: self.nodes.len() as i32,
            total_edges: self.edges.len() as i32,
            max_depth: self.max_depth as i32,
            complexity_score: self.calculate_complexity_score(),
        };

        Ok((
            std::mem::take(&mut self.nodes),
            std::mem::take(&mut self.edges),
            statistics,
        ))
    }

    fn visit_workflow(
        &mut self,
        workflow: &crate::ast::Workflow,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("Workflow") {
            return Ok(String::new());
        }

        self.max_depth = self.max_depth.max(self.current_depth);

        let workflow_id = format!("workflow_{}", index);
        let mut workflow_properties: HashMap<String, serde_json::Value> = HashMap::new();
        workflow_properties.insert(
            "workflow_id".to_string(),
            serde_json::Value::String(workflow.id.clone()),
        );
        workflow_properties.insert(
            "statement_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(workflow.statements.len())),
        );

        // Add workflow properties
        for (key, value) in &workflow.properties {
            workflow_properties.insert(
                format!("prop_{}", key),
                serde_json::Value::String(self.format_value(value)),
            );
        }

        let mut statement_children = Vec::new();

        // Visit each statement
        self.current_depth += 1;
        for (i, statement) in workflow.statements.iter().enumerate() {
            let stmt_id = self.visit_statement(statement, &workflow_id, i)?;
            if !stmt_id.is_empty() {
                statement_children.push(stmt_id.clone());

                // Create edge from workflow to statement
                self.edges.push(VisualEdge {
                    id: format!("edge_{}_{}", workflow_id, i),
                    from: workflow_id.clone(),
                    to: stmt_id,
                    edge_type: "contains".to_string(),
                    properties: HashMap::new(),
                });
            }
        }

        let workflow_node = VisualNode {
            id: workflow_id.clone(),
            node_type: "Workflow".to_string(),
            label: format!("Workflow: {}", workflow.id),
            children: statement_children,
            properties: workflow_properties,
        };

        self.nodes.push(workflow_node);
        Ok(workflow_id)
    }

    fn visit_statement(
        &mut self,
        statement: &Statement,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        use crate::ast::Statement;

        self.max_depth = self.max_depth.max(self.current_depth);

        match statement {
            Statement::DeclareEntity {
                id,
                entity_type,
                properties,
            } => self.visit_declare_entity(id, entity_type, properties, parent_id, index),
            Statement::ObtainDocument {
                document_type,
                source,
                properties,
            } => self.visit_obtain_document(document_type, source, properties, parent_id, index),
            Statement::CreateEdge {
                from,
                to,
                edge_type,
                properties,
            } => self.visit_create_edge(from, to, edge_type, properties, parent_id, index),
            Statement::CalculateUbo {
                entity_id,
                properties,
            } => self.visit_calculate_ubo(entity_id, properties, parent_id, index),
            Statement::SolicitAttribute(attr) => {
                self.visit_solicit_attribute(attr, parent_id, index)
            }
            Statement::ResolveConflict(conflict) => {
                self.visit_resolve_conflict(conflict, parent_id, index)
            }
            Statement::GenerateReport(report) => {
                self.visit_generate_report(report, parent_id, index)
            }
            Statement::ScheduleMonitoring(monitoring) => {
                self.visit_schedule_monitoring(monitoring, parent_id, index)
            }
            Statement::ParallelObtain(parallel) => {
                self.visit_parallel_obtain(parallel, parent_id, index)
            }
            Statement::Parallel(statements) => {
                self.visit_parallel_statements(statements, parent_id, index)
            }
            Statement::Sequential(statements) => {
                self.visit_sequential_statements(statements, parent_id, index)
            }
            Statement::Placeholder { command, args } => {
                self.visit_placeholder(command, args, parent_id, index)
            }
        }
    }

    fn visit_declare_entity(
        &mut self,
        id: &str,
        entity_type: &str,
        properties: &PropertyMap,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("DeclareEntity") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "entity_id".to_string(),
            serde_json::Value::String(id.to_string()),
        );
        node_properties.insert(
            "entity_type".to_string(),
            serde_json::Value::String(entity_type.to_string()),
        );

        // Add custom properties
        for (key, value) in properties {
            node_properties.insert(
                format!("prop_{}", key),
                serde_json::Value::String(self.format_value(value)),
            );
        }

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "DeclareEntity".to_string(),
            label: format!("Declare Entity: {} ({})", id, entity_type),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_obtain_document(
        &mut self,
        document_type: &str,
        source: &str,
        properties: &PropertyMap,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("ObtainDocument") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "document_type".to_string(),
            serde_json::Value::String(document_type.to_string()),
        );
        node_properties.insert(
            "source".to_string(),
            serde_json::Value::String(source.to_string()),
        );

        // Add custom properties
        for (key, value) in properties {
            node_properties.insert(
                format!("prop_{}", key),
                serde_json::Value::String(self.format_value(value)),
            );
        }

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "ObtainDocument".to_string(),
            label: format!("Obtain Document: {} from {}", document_type, source),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_create_edge(
        &mut self,
        from: &str,
        to: &str,
        edge_type: &str,
        properties: &PropertyMap,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("CreateEdge") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "from".to_string(),
            serde_json::Value::String(from.to_string()),
        );
        node_properties.insert("to".to_string(), serde_json::Value::String(to.to_string()));
        node_properties.insert(
            "edge_type".to_string(),
            serde_json::Value::String(edge_type.to_string()),
        );

        // Add custom properties
        for (key, value) in properties {
            node_properties.insert(
                format!("prop_{}", key),
                serde_json::Value::String(self.format_value(value)),
            );
        }

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "CreateEdge".to_string(),
            label: format!("Create Edge: {} -[{}]-> {}", from, edge_type, to),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_calculate_ubo(
        &mut self,
        entity_id: &str,
        properties: &PropertyMap,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("CalculateUbo") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "entity_id".to_string(),
            serde_json::Value::String(entity_id.to_string()),
        );

        // Add custom properties
        for (key, value) in properties {
            node_properties.insert(
                format!("prop_{}", key),
                serde_json::Value::String(self.format_value(value)),
            );
        }

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "CalculateUbo".to_string(),
            label: format!("Calculate UBO for: {}", entity_id),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_solicit_attribute(
        &mut self,
        attr: &SolicitAttribute,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("SolicitAttribute") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "attr_id".to_string(),
            serde_json::Value::String(attr.attr_id.clone()),
        );
        node_properties.insert(
            "from".to_string(),
            serde_json::Value::String(attr.from.clone()),
        );
        node_properties.insert(
            "value_type".to_string(),
            serde_json::Value::String(attr.value_type.clone()),
        );

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "SolicitAttribute".to_string(),
            label: format!("Solicit Attribute: {} from {}", attr.attr_id, attr.from),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_resolve_conflict(
        &mut self,
        _conflict: &ResolveConflict,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("ResolveConflict") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let node = VisualNode {
            id: node_id.clone(),
            node_type: "ResolveConflict".to_string(),
            label: "Resolve Conflict".to_string(),
            children: vec![],
            properties: HashMap::new(),
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_generate_report(
        &mut self,
        report: &GenerateReport,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("GenerateReport") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "target".to_string(),
            serde_json::Value::String(report.target.clone()),
        );
        node_properties.insert(
            "status".to_string(),
            serde_json::Value::String(report.status.clone()),
        );

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "GenerateReport".to_string(),
            label: format!("Generate Report for: {}", report.target),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_schedule_monitoring(
        &mut self,
        monitoring: &ScheduleMonitoring,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("ScheduleMonitoring") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "target".to_string(),
            serde_json::Value::String(monitoring.target.clone()),
        );
        node_properties.insert(
            "frequency".to_string(),
            serde_json::Value::String(monitoring.frequency.clone()),
        );

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "ScheduleMonitoring".to_string(),
            label: format!(
                "Schedule Monitoring: {} ({})",
                monitoring.target, monitoring.frequency
            ),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_parallel_obtain(
        &mut self,
        parallel: &ParallelObtain,
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("ParallelObtain") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "document_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(parallel.documents.len())),
        );

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "ParallelObtain".to_string(),
            label: format!("Parallel Obtain ({} documents)", parallel.documents.len()),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_parallel_statements(
        &mut self,
        statements: &[Statement],
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("Parallel") {
            return Ok(String::new());
        }

        let node_id = format!("{}_parallel_{}", parent_id, index);
        let mut children = Vec::new();

        // Visit nested statements
        self.current_depth += 1;
        for (i, stmt) in statements.iter().enumerate() {
            let stmt_id = self.visit_statement(stmt, &node_id, i)?;
            if !stmt_id.is_empty() {
                children.push(stmt_id.clone());

                // Create edge to nested statement
                self.edges.push(VisualEdge {
                    id: format!("edge_{}_{}", node_id, i),
                    from: node_id.clone(),
                    to: stmt_id,
                    edge_type: "contains_parallel".to_string(),
                    properties: HashMap::new(),
                });
            }
        }

        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "statement_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(statements.len())),
        );

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "Parallel".to_string(),
            label: format!("Parallel Block ({} statements)", statements.len()),
            children,
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_sequential_statements(
        &mut self,
        statements: &[Statement],
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("Sequential") {
            return Ok(String::new());
        }

        let node_id = format!("{}_sequential_{}", parent_id, index);
        let mut children = Vec::new();

        // Visit nested statements
        self.current_depth += 1;
        for (i, stmt) in statements.iter().enumerate() {
            let stmt_id = self.visit_statement(stmt, &node_id, i)?;
            if !stmt_id.is_empty() {
                children.push(stmt_id.clone());

                // Create edge to nested statement
                self.edges.push(VisualEdge {
                    id: format!("edge_{}_{}", node_id, i),
                    from: node_id.clone(),
                    to: stmt_id,
                    edge_type: "contains_sequential".to_string(),
                    properties: HashMap::new(),
                });
            }
        }

        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "statement_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(statements.len())),
        );

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "Sequential".to_string(),
            label: format!("Sequential Block ({} statements)", statements.len()),
            children,
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn visit_placeholder(
        &mut self,
        command: &str,
        args: &[Value],
        parent_id: &str,
        index: usize,
    ) -> DslResult<String> {
        if !self.should_include_node("Placeholder") {
            return Ok(String::new());
        }

        let node_id = format!("{}_{}", parent_id, index);
        let mut node_properties: HashMap<String, serde_json::Value> = HashMap::new();
        node_properties.insert(
            "command".to_string(),
            serde_json::Value::String(command.to_string()),
        );
        node_properties.insert(
            "arg_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(args.len())),
        );

        let node = VisualNode {
            id: node_id.clone(),
            node_type: "Placeholder".to_string(),
            label: format!("Placeholder: {} ({} args)", command, args.len()),
            children: vec![],
            properties: node_properties,
        };

        self.nodes.push(node);
        Ok(node_id)
    }

    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Date(d) => d.to_string(),
            Value::List(items) => format!("List({} items)", items.len()),
            Value::Map(map) => format!("Map({} entries)", map.len()),
            Value::MultiValue(values) => format!("MultiValue({} sources)", values.len()),
            Value::Null => "null".to_string(),
        }
    }

    fn calculate_complexity_score(&self) -> rust_decimal::Decimal {
        use rust_decimal::Decimal;

        // Calculate complexity based on different node types and structure
        let mut score = Decimal::from(0);

        for node in &self.nodes {
            let base_score = match node.node_type.as_str() {
                "Program" => 1,
                "Workflow" => 2,
                "DeclareEntity" => 3,
                "CreateEdge" => 4,
                "CalculateUbo" => 10,
                "Parallel" => 5,
                "Sequential" => 3,
                _ => 2,
            };

            score += Decimal::from(base_score);
        }

        // Add complexity for edges (interconnection complexity)
        score += Decimal::from(self.edges.len()) / Decimal::from(2);

        // Add depth complexity
        score += Decimal::from(self.max_depth * self.max_depth);

        score
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
            filters: Some(FilterConfig {
                show_only_nodes: None,
                hide_nodes: None,
                max_depth: Some(10),
                show_properties: true,
            }),
            include_compilation_info: true,
            include_domain_context: true,
            show_functional_states: false,
            max_depth: Some(10),
        }
    }
}

/// Enhanced visualization options for Phase 3 domain features
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Program, Statement, Workflow};
    use crate::domain_visualizations::{DomainVisualizer, HighlightPriority};
    use crate::models::CompilationStatus;
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

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
            id: "test-id".to_string(),
        };
        assert_eq!(error.to_string(), "DSL not found: test-id");
    }

    #[test]
    fn test_ast_visualization_visitor_creation() {
        let visitor = ASTVisualizationVisitor::new(LayoutType::Tree);
        assert_eq!(visitor.layout, LayoutType::Tree);
        assert_eq!(visitor.node_counter, 0);
        assert!(visitor.nodes.is_empty());
        assert!(visitor.edges.is_empty());
    }

    #[test]
    fn test_ast_visualization_visitor_node_filtering() {
        let mut visitor = ASTVisualizationVisitor::new(LayoutType::Tree);

        // Test default behavior (include all nodes)
        assert!(visitor.should_include_node("Workflow"));
        assert!(visitor.should_include_node("DeclareEntity"));

        // Test show_only filter
        visitor.set_filters(FilterConfig {
            show_only_nodes: Some(vec!["Workflow".to_string()]),
            hide_nodes: None,
            max_depth: None,
            show_properties: true,
        });
        assert!(visitor.should_include_node("Workflow"));
        assert!(!visitor.should_include_node("DeclareEntity"));

        // Test hide filter
        visitor.set_filters(FilterConfig {
            show_only_nodes: None,
            hide_nodes: Some(vec!["Placeholder".to_string()]),
            max_depth: None,
            show_properties: true,
        });
        assert!(visitor.should_include_node("Workflow"));
        assert!(!visitor.should_include_node("Placeholder"));
    }

    #[test]
    fn test_ast_visualization_visitor_value_formatting() {
        let visitor = ASTVisualizationVisitor::new(LayoutType::Tree);

        assert_eq!(
            visitor.format_value(&Value::String("test".to_string())),
            "test"
        );
        assert_eq!(visitor.format_value(&Value::Number(42.5)), "42.5");
        assert_eq!(visitor.format_value(&Value::Integer(123)), "123");
        assert_eq!(visitor.format_value(&Value::Boolean(true)), "true");
        assert_eq!(visitor.format_value(&Value::Null), "null");

        let list = Value::List(vec![Value::Integer(1), Value::Integer(2)]);
        assert_eq!(visitor.format_value(&list), "List(2 items)");

        let map = Value::Map(HashMap::new());
        assert_eq!(visitor.format_value(&map), "Map(0 entries)");
    }

    #[test]
    fn test_ast_visualization_visitor_program_structure() {
        let mut visitor = ASTVisualizationVisitor::new(LayoutType::Tree);

        // Create a simple test program
        let program = Program {
            workflows: vec![
                Workflow {
                    id: "test-workflow-1".to_string(),
                    properties: HashMap::new(),
                    statements: vec![
                        Statement::DeclareEntity {
                            id: "entity1".to_string(),
                            entity_type: "person".to_string(),
                            properties: HashMap::new(),
                        },
                        Statement::CreateEdge {
                            from: "entity1".to_string(),
                            to: "entity2".to_string(),
                            edge_type: "owns".to_string(),
                            properties: HashMap::new(),
                        },
                    ],
                },
                Workflow {
                    id: "test-workflow-2".to_string(),
                    properties: HashMap::new(),
                    statements: vec![Statement::CalculateUbo {
                        entity_id: "entity1".to_string(),
                        properties: HashMap::new(),
                    }],
                },
            ],
        };

        let result = visitor.visit_program(&program).unwrap();
        let (nodes, edges, statistics) = result;

        // Should have: 1 program + 2 workflows + 3 statements = 6 nodes
        assert_eq!(nodes.len(), 6);

        // Should have edges connecting program to workflows and workflows to statements
        assert!(edges.len() >= 3); // At least 2 program->workflow + 1+ workflow->statement edges

        // Check statistics
        assert_eq!(statistics.total_nodes, 6);
        assert!(statistics.total_edges >= 3);
        assert!(statistics.max_depth > 0);

        // Check program node exists
        let program_node = nodes.iter().find(|n| n.id == "program");
        assert!(program_node.is_some());
        let program_node = program_node.unwrap();
        assert_eq!(program_node.node_type, "Program");
        assert_eq!(program_node.children.len(), 2); // Two workflows

        // Check workflow nodes exist
        let workflow_nodes: Vec<_> = nodes.iter().filter(|n| n.node_type == "Workflow").collect();
        assert_eq!(workflow_nodes.len(), 2);

        // Check statement nodes exist
        let statement_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| {
                n.node_type == "DeclareEntity"
                    || n.node_type == "CreateEdge"
                    || n.node_type == "CalculateUbo"
            })
            .collect();
        assert_eq!(statement_nodes.len(), 3);
    }

    #[test]
    fn test_ast_visualization_builder_integration() {
        // Create test domain and version data
        let domain = DslDomain {
            domain_id: Uuid::new_v4(),
            domain_name: "KYC".to_string(),
            description: Some("KYC domain".to_string()),
            base_grammar_version: "1.0.0".to_string(),
            vocabulary_version: "1.0.0".to_string(),
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let version = DslVersion {
            version_id: Uuid::new_v4(),
            domain_id: domain.domain_id,
            version_number: 1,
            functional_state: Some("Test".to_string()),
            dsl_source_code: "(workflow \"test\")".to_string(),
            compilation_status: CompilationStatus::Compiled,
            change_description: Some("Test version".to_string()),
            parent_version_id: None,
            created_by: Some("test_user".to_string()),
            created_at: Utc::now(),
            compiled_at: Some(Utc::now()),
            activated_at: None,
        };

        let parsed_ast = ParsedAst {
            ast_id: Uuid::new_v4(),
            version_id: version.version_id,
            ast_json: serde_json::json!({
                "workflows": [
                    {
                        "id": "test-workflow",
                        "properties": {},
                        "statements": []
                    }
                ]
            }),
            parse_metadata: Some(serde_json::json!({
                "parse_duration_ms": 50
            })),
            grammar_version: "1.0.0".to_string(),
            parser_version: "1.0.0".to_string(),
            ast_hash: Some("test-hash".to_string()),
            node_count: Some(2),
            complexity_score: Some(rust_decimal::Decimal::from(5)),
            parsed_at: Utc::now(),
            invalidated_at: None,
        };

        // Create simple test AST
        let program = Program {
            workflows: vec![Workflow {
                id: "test-workflow".to_string(),
                properties: HashMap::new(),
                statements: vec![Statement::DeclareEntity {
                    id: "test-entity".to_string(),
                    entity_type: "person".to_string(),
                    properties: HashMap::new(),
                }],
            }],
        };

        // Build visualization
        let builder = ASTVisualizationBuilder::new()
            .with_domain_context(&domain, &version, &parsed_ast)
            .with_layout(LayoutType::Graph);

        let visualization = builder.from_ast(&program).unwrap();

        // Verify visualization structure
        assert_eq!(visualization.metadata.layout_type, LayoutType::Graph);
        assert_eq!(visualization.domain_context.domain_name, "KYC");
        assert_eq!(visualization.domain_context.version_number, 1);
        assert_eq!(
            visualization.domain_context.functional_state,
            Some("Test".to_string())
        );
        assert_eq!(visualization.compilation_info.parse_duration_ms, 50);

        // Check that we have nodes and edges
        assert!(!visualization.root_node.id.is_empty());
        assert!(visualization.statistics.total_nodes > 0);
    }

    #[test]
    fn test_complexity_score_calculation() {
        let mut visitor = ASTVisualizationVisitor::new(LayoutType::Tree);

        // Add some test nodes of different types
        visitor.nodes.push(VisualNode {
            id: "program".to_string(),
            node_type: "Program".to_string(),
            label: "Program".to_string(),
            children: vec![],
            properties: HashMap::new(),
        });

        visitor.nodes.push(VisualNode {
            id: "workflow".to_string(),
            node_type: "Workflow".to_string(),
            label: "Workflow".to_string(),
            children: vec![],
            properties: HashMap::new(),
        });

        visitor.nodes.push(VisualNode {
            id: "calculate_ubo".to_string(),
            node_type: "CalculateUbo".to_string(),
            label: "Calculate UBO".to_string(),
            children: vec![],
            properties: HashMap::new(),
        });

        visitor.edges.push(VisualEdge {
            id: "edge1".to_string(),
            from: "program".to_string(),
            to: "workflow".to_string(),
            edge_type: "contains".to_string(),
            properties: HashMap::new(),
        });

        visitor.max_depth = 2;

        let score = visitor.calculate_complexity_score();

        // Program (1) + Workflow (2) + CalculateUbo (10) + edges (0.5) + depth (4) = 17.5
        assert!(score > rust_decimal::Decimal::from(15));
        assert!(score < rust_decimal::Decimal::from(20));
    }

    #[test]
    fn test_layout_types() {
        let tree = LayoutType::Tree;
        let graph = LayoutType::Graph;
        let hierarchical = LayoutType::Hierarchical;

        // Just ensure they're different values
        assert_ne!(format!("{:?}", tree), format!("{:?}", graph));
        assert_ne!(format!("{:?}", graph), format!("{:?}", hierarchical));
    }

    #[test]
    fn test_filtering_with_depth_limit() {
        let mut visitor = ASTVisualizationVisitor::new(LayoutType::Tree);

        // Set a depth filter
        visitor.set_filters(FilterConfig {
            show_only_nodes: None,
            hide_nodes: None,
            max_depth: Some(2),
            show_properties: false,
        });

        // Create a nested program structure
        let program = Program {
            workflows: vec![Workflow {
                id: "nested-workflow".to_string(),
                properties: HashMap::new(),
                statements: vec![Statement::Parallel(vec![Statement::DeclareEntity {
                    id: "deep-entity".to_string(),
                    entity_type: "person".to_string(),
                    properties: HashMap::new(),
                }])],
            }],
        };

        let result = visitor.visit_program(&program).unwrap();
        let (nodes, _edges, statistics) = result;

        // Should have created nodes despite nesting
        assert!(nodes.len() >= 3); // At least program + workflow + parallel
        assert!(statistics.max_depth > 0);
    }

    // ============================================================================
    // PHASE 3: DOMAIN-SPECIFIC VISUALIZATION TESTS
    // ============================================================================

    #[test]
    fn test_domain_visualizer_integration() {
        let manager = DslManagerV2 {
            repository: create_mock_repository(),
            grammar_version: "1.0.0".to_string(),
            parser_version: "test".to_string(),
            domain_visualizer: DomainVisualizer::new(),
        };

        // Test that domain visualizer is properly initialized
        assert!(manager.domain_visualizer.domain_rules.contains_key("KYC"));
        assert!(manager
            .domain_visualizer
            .domain_rules
            .contains_key("Onboarding"));
        assert!(manager.supports_functional_states("KYC"));
        assert!(!manager.supports_functional_states("NonExistentDomain"));
    }

    #[test]
    fn test_functional_state_support() {
        let manager = DslManagerV2 {
            repository: create_mock_repository(),
            grammar_version: "1.0.0".to_string(),
            parser_version: "test".to_string(),
            domain_visualizer: DomainVisualizer::new(),
        };

        // Test KYC domain functional states
        let kyc_states = manager.get_domain_functional_states("KYC");
        assert!(!kyc_states.is_empty());
        assert!(kyc_states.contains(&"Create_Case".to_string()));
        assert!(kyc_states.contains(&"Generate_UBO".to_string()));

        // Test non-existent domain
        let unknown_states = manager.get_domain_functional_states("UnknownDomain");
        assert!(unknown_states.is_empty());
    }

    #[test]
    fn test_domain_highlights() {
        let manager = DslManagerV2 {
            repository: create_mock_repository(),
            grammar_version: "1.0.0".to_string(),
            parser_version: "test".to_string(),
            domain_visualizer: DomainVisualizer::new(),
        };

        // Test KYC domain highlights
        let kyc_highlights = manager.get_domain_highlights("KYC");
        assert!(!kyc_highlights.is_empty());

        let ubo_highlight = kyc_highlights
            .iter()
            .find(|h| h.highlight_type == "UBO_CALCULATION")
            .expect("Should have UBO calculation highlight");
        assert_eq!(ubo_highlight.color, "#FF6B35");
        assert!(matches!(ubo_highlight.priority, HighlightPriority::High));

        // Test Onboarding domain highlights
        let onboarding_highlights = manager.get_domain_highlights("Onboarding");
        assert!(!onboarding_highlights.is_empty());

        let workflow_highlight = onboarding_highlights
            .iter()
            .find(|h| h.highlight_type == "WORKFLOW_PROGRESSION")
            .expect("Should have workflow progression highlight");
        assert_eq!(workflow_highlight.color, "#96CEB4");
    }

    #[test]
    fn test_domain_visualization_options() {
        let default_options = DomainVisualizationOptions::default();
        assert!(default_options.highlight_current_state);
        assert!(default_options.show_state_transitions);
        assert!(default_options.include_domain_metrics);
        assert!(default_options.show_workflow_progression);
        assert!(default_options.emphasize_critical_paths);
        assert!(default_options.domain_specific_styling);

        // Test base options integration
        assert!(default_options.base_options.include_domain_context);
        assert!(default_options.base_options.include_compilation_info);
    }

    #[test]
    fn test_kyc_domain_specific_features() {
        let visualizer = DomainVisualizer::new();
        let kyc_rules = visualizer.domain_rules.get("KYC").unwrap();

        // Test KYC-specific node styles
        assert!(kyc_rules.node_styles.contains_key("CalculateUbo"));
        assert!(kyc_rules.node_styles.contains_key("DeclareEntity"));
        assert!(kyc_rules.node_styles.contains_key("CreateEdge"));

        let ubo_style = kyc_rules.node_styles.get("CalculateUbo").unwrap();
        assert_eq!(ubo_style.color, "#FF6B35");
        assert_eq!(ubo_style.shape, "diamond");

        // Test KYC-specific edge styles
        assert!(kyc_rules.edge_styles.contains_key("beneficial-owner"));
        let beneficial_owner_style = kyc_rules.edge_styles.get("beneficial-owner").unwrap();
        assert_eq!(beneficial_owner_style.color, "#E74C3C");
        assert_eq!(beneficial_owner_style.width, 3);

        // Test functional states
        assert_eq!(kyc_rules.functional_states.len(), 7);
        let create_case = kyc_rules
            .functional_states
            .iter()
            .find(|s| s.name == "Create_Case")
            .unwrap();
        assert!(create_case.dependencies.is_empty());
        assert_eq!(create_case.estimated_effort, 30);

        let generate_ubo = kyc_rules
            .functional_states
            .iter()
            .find(|s| s.name == "Generate_UBO")
            .unwrap();
        assert!(generate_ubo
            .dependencies
            .contains(&"Verify_Entities".to_string()));
        assert_eq!(generate_ubo.estimated_effort, 45);
    }

    #[test]
    fn test_onboarding_domain_specific_features() {
        let visualizer = DomainVisualizer::new();
        let onboarding_rules = visualizer.domain_rules.get("Onboarding").unwrap();

        // Test Onboarding-specific characteristics
        assert_eq!(onboarding_rules.domain_name, "Onboarding");
        assert_eq!(onboarding_rules.base_execution_time_ms, 3000);
        assert!(onboarding_rules
            .critical_edge_types
            .contains(&"approval_required".to_string()));

        // Test functional states progression
        assert_eq!(onboarding_rules.functional_states.len(), 5);
        let states: Vec<&str> = onboarding_rules
            .functional_states
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(states.contains(&"Initial_Contact"));
        assert!(states.contains(&"Document_Collection"));
        assert!(states.contains(&"Final_Approval"));
    }

    #[test]
    fn test_account_opening_domain_specific_features() {
        let visualizer = DomainVisualizer::new();
        let account_rules = visualizer.domain_rules.get("Account_Opening").unwrap();

        // Test Account Opening-specific characteristics
        assert_eq!(account_rules.domain_name, "Account_Opening");
        assert_eq!(account_rules.base_execution_time_ms, 4000);

        // Test priority mappings
        assert_eq!(
            account_rules.priority_mapping.get("ValidateRequirements"),
            Some(&9)
        );
        assert_eq!(
            account_rules.priority_mapping.get("ApprovalWorkflow"),
            Some(&8)
        );

        // Test functional states for account opening
        let req_check = account_rules
            .functional_states
            .iter()
            .find(|s| s.name == "Requirements_Check")
            .unwrap();
        assert_eq!(req_check.estimated_effort, 60);
        assert!(req_check.dependencies.is_empty());

        let doc_review = account_rules
            .functional_states
            .iter()
            .find(|s| s.name == "Documentation_Review")
            .unwrap();
        assert!(doc_review
            .dependencies
            .contains(&"Requirements_Check".to_string()));
    }

    #[test]
    fn test_compliance_domain_features() {
        let visualizer = DomainVisualizer::new();
        let compliance_rules = visualizer.domain_rules.get("Compliance").unwrap();

        assert_eq!(compliance_rules.domain_name, "Compliance");
        assert_eq!(compliance_rules.base_execution_time_ms, 6000);

        // Test compliance-specific node styling
        assert!(compliance_rules.node_styles.contains_key("ComplianceCheck"));
        let compliance_style = compliance_rules.node_styles.get("ComplianceCheck").unwrap();
        assert_eq!(compliance_style.color, "#FF9F43");
        assert_eq!(compliance_style.shape, "hexagon");

        // Test compliance workflow progression
        let risk_id = compliance_rules
            .functional_states
            .iter()
            .find(|s| s.name == "Risk_Identification")
            .unwrap();
        assert_eq!(risk_id.estimated_effort, 45);
        assert!(risk_id.dependencies.is_empty());
    }

    // Helper function for tests
    fn create_mock_repository() -> DslDomainRepository {
        // This would normally require a database pool
        // For testing, we'd need to either mock this or use a test database
        // For now, this is a placeholder that won't actually work
        // In a real test, you'd use something like:
        // DslDomainRepository::new(test_pool)

        // Since we can't easily create a real repository in unit tests,
        // the tests above focus on the domain visualizer logic
        panic!("Mock repository not implemented - use integration tests with real database")
    }

    fn create_test_visualization_with_compilation_info() -> ASTVisualization {
        use crate::dsl_manager_v2::{
            ASTStatistics, CompilationInfo, DomainContext, LayoutType, VisualNode,
            VisualizationMetadata,
        };
        use chrono::Utc;
        use rust_decimal::Decimal;

        ASTVisualization {
            metadata: VisualizationMetadata {
                generated_at: Utc::now(),
                generator_version: "test".to_string(),
                layout_type: LayoutType::Tree,
            },
            domain_context: DomainContext {
                domain_name: "KYC".to_string(),
                version_number: 1,
                functional_state: Some("Generate_UBO".to_string()),
                grammar_version: "1.0.0".to_string(),
                compilation_status: CompilationStatus::Compiled,
            },
            root_node: VisualNode {
                id: "root".to_string(),
                node_type: "Program".to_string(),
                label: "Test Program".to_string(),
                children: vec![],
                properties: HashMap::new(),
            },
            edges: vec![],
            statistics: ASTStatistics {
                total_nodes: 5,
                total_edges: 3,
                max_depth: 2,
                complexity_score: Decimal::from(10),
            },
            compilation_info: CompilationInfo {
                parsed_at: Utc::now(),
                parser_version: "test".to_string(),
                grammar_version: "1.0.0".to_string(),
                parse_duration_ms: 100,
            },
        }
    }
}

/// Example demonstrating Phase 2 AST Visualization functionality
///
/// This function shows how to:
/// 1. Create a DSL version in a domain
/// 2. Compile the DSL to generate and store AST
/// 3. Build various types of visualizations with different options
/// 4. Access domain context and compilation information
#[allow(dead_code)]
pub async fn demonstrate_phase2_functionality(
    manager: &DslManagerV2,
) -> DslResult<Vec<ASTVisualization>> {
    use std::collections::HashMap;

    println!("🚀 Phase 2: Enhanced DSL Manager with AST Visualization Demo");
    println!("{}", "=".repeat(60));

    // 1. Create a comprehensive test DSL
    let sample_dsl = r#"
        (workflow "kyc-onboarding"
            ;; Entity declarations
            (declare-entity "customer" "person"
                (properties
                    (name "John Smith")
                    (dob "1980-01-15")))

            (declare-entity "company" "corporation"
                (properties
                    (name "Acme Corp")
                    (jurisdiction "Delaware")))

            ;; Document collection
            (obtain-document "passport" "government"
                (properties
                    (required true)
                    (expires "2030-12-31")))

            (obtain-document "utility-bill" "third-party"
                (properties
                    (age-limit 90)
                    (purpose "address-verification")))

            ;; Relationship creation
            (create-edge "customer" "company" "beneficial-owner"
                (properties
                    (ownership-percentage 25.5)
                    (control-type "voting")))

            ;; UBO calculation
            (calculate-ubo "company"
                (properties
                    (threshold 25.0)
                    (max-depth 3)
                    (algorithm "recursive")))

            ;; Parallel document processing
            (parallel
                (obtain-document "bank-statement" "financial-institution")
                (obtain-document "tax-return" "government")
                (obtain-document "corporate-registry" "government"))

            ;; Sequential compliance checks
            (sequential
                (solicit-attribute "pep-status" "customer" "boolean")
                (solicit-attribute "sanctions-check" "customer" "boolean")
                (generate-report "customer" "preliminary")))
    "#;

    println!("📝 Creating DSL version in KYC domain...");
    let version = manager
        .create_dsl_version(
            "KYC",
            sample_dsl,
            Some("Demo_Phase2"),
            Some("Comprehensive demo showcasing Phase 2 AST visualization features"),
            Some("demo_user"),
        )
        .await?;

    println!(
        "   ✅ Created version {} in KYC domain",
        version.version_number
    );

    // 2. Compile the DSL to generate AST
    println!("🔧 Compiling DSL and generating AST...");
    let parsed_ast = manager
        .compile_dsl_version("KYC", version.version_number, false)
        .await?;

    println!("   ✅ AST compiled successfully");
    println!("   📊 AST Statistics:");
    println!("      - Node count: {}", parsed_ast.node_count.unwrap_or(0));
    println!(
        "      - Complexity: {:?}",
        parsed_ast.complexity_score.unwrap_or_default()
    );
    println!("      - Parser version: {}", parsed_ast.parser_version);

    let mut visualizations = Vec::new();

    // 3. Generate different types of visualizations

    // 3a. Tree layout with default options
    println!("🌳 Generating Tree Layout Visualization...");
    let tree_options = VisualizationOptions {
        layout: Some(LayoutType::Tree),
        styling: None,
        filters: Some(FilterConfig {
            show_only_nodes: None,
            hide_nodes: None,
            max_depth: Some(10),
            show_properties: true,
        }),
        include_compilation_info: true,
        include_domain_context: true,
        show_functional_states: true,
        max_depth: Some(10),
    };

    let tree_viz = manager
        .build_ast_visualization("KYC", version.version_number, Some(tree_options))
        .await?;

    println!("   ✅ Tree visualization generated");
    println!(
        "   📈 Statistics: {} nodes, {} edges, depth: {}",
        tree_viz.statistics.total_nodes,
        tree_viz.statistics.total_edges,
        tree_viz.statistics.max_depth
    );

    visualizations.push(tree_viz);

    // 3b. Graph layout with filtered nodes
    println!("📊 Generating Graph Layout with Entity Focus...");
    let entity_focused_options = VisualizationOptions {
        layout: Some(LayoutType::Graph),
        styling: Some(StylingConfig {
            theme: "entity-focused".to_string(),
            node_colors: {
                let mut colors = HashMap::new();
                colors.insert("DeclareEntity".to_string(), "#4CAF50".to_string());
                colors.insert("CreateEdge".to_string(), "#2196F3".to_string());
                colors.insert("CalculateUbo".to_string(), "#FF9800".to_string());
                colors
            },
        }),
        filters: Some(FilterConfig {
            show_only_nodes: Some(vec![
                "Program".to_string(),
                "Workflow".to_string(),
                "DeclareEntity".to_string(),
                "CreateEdge".to_string(),
                "CalculateUbo".to_string(),
            ]),
            hide_nodes: None,
            max_depth: Some(5),
            show_properties: false,
        }),
        include_compilation_info: true,
        include_domain_context: true,
        show_functional_states: false,
        max_depth: Some(5),
    };

    let graph_viz = manager
        .build_ast_visualization("KYC", version.version_number, Some(entity_focused_options))
        .await?;

    println!("   ✅ Graph visualization generated");
    println!("   🎯 Filtered to core entity operations");

    visualizations.push(graph_viz);

    // 3c. Hierarchical layout excluding placeholder nodes
    println!("🏗️  Generating Hierarchical Layout (Production View)...");
    let production_options = VisualizationOptions {
        layout: Some(LayoutType::Hierarchical),
        styling: Some(StylingConfig {
            theme: "production".to_string(),
            node_colors: HashMap::new(),
        }),
        filters: Some(FilterConfig {
            show_only_nodes: None,
            hide_nodes: Some(vec!["Placeholder".to_string()]),
            max_depth: Some(8),
            show_properties: true,
        }),
        include_compilation_info: true,
        include_domain_context: true,
        show_functional_states: true,
        max_depth: Some(8),
    };

    let hierarchical_viz = manager
        .build_ast_visualization("KYC", version.version_number, Some(production_options))
        .await?;

    println!("   ✅ Hierarchical visualization generated");

    visualizations.push(hierarchical_viz);

    // 4. Display comprehensive results
    println!("\n📋 Phase 2 Demonstration Results:");
    println!("{}", "-".repeat(60));

    for (i, viz) in visualizations.iter().enumerate() {
        println!(
            "📊 Visualization {} ({:?} layout):",
            i + 1,
            viz.metadata.layout_type
        );
        println!(
            "   Domain: {} v{}",
            viz.domain_context.domain_name, viz.domain_context.version_number
        );
        println!(
            "   Functional State: {:?}",
            viz.domain_context.functional_state
        );
        println!(
            "   Compilation Status: {:?}",
            viz.domain_context.compilation_status
        );
        println!(
            "   Root Node: {} ({})",
            viz.root_node.label, viz.root_node.node_type
        );
        println!(
            "   Structure: {} nodes, {} edges, depth {}",
            viz.statistics.total_nodes, viz.statistics.total_edges, viz.statistics.max_depth
        );
        println!("   Complexity Score: {}", viz.statistics.complexity_score);
        println!(
            "   Parse Duration: {}ms",
            viz.compilation_info.parse_duration_ms
        );
        println!("   Generated: {}", viz.metadata.generated_at);
        println!();
    }

    println!("✨ Phase 2 AST Visualization Demo Complete!");
    println!("   - ✅ Domain-aware DSL compilation");
    println!("   - ✅ AST storage and caching");
    println!("   - ✅ Multiple visualization layouts");
    println!("   - ✅ Flexible filtering and styling");
    println!("   - ✅ Domain context preservation");
    println!("   - ✅ Compilation metadata tracking");

    Ok(visualizations)
}
