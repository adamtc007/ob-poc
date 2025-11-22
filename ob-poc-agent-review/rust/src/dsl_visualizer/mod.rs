//! DSL Visualizer - Output Generation and Visualization
//!
//! This module provides visualization and output generation for DSL operations,
//! following the proven architecture from the independent call chain implementation.
//!
//! ## Architecture Role
//! The DSL Visualizer is responsible for:
//! - Generating human-readable visualizations of DSL state
//! - Creating charts and diagrams for complex operations
//! - Producing audit reports and compliance documentation
//! - Formatting output for different consumption contexts
//! - Supporting multiple output formats (JSON, HTML, PDF, etc.)
//!
//! ## Call Chain Integration
//! The DSL Visualizer is the final component in the call chain:
//! DSL Manager → DSL Mod → DB State Manager → **DSL Visualizer**

use std::collections::HashMap;

/// DSL Visualizer for generating output and visualizations
pub struct DslVisualizer {
    /// Configuration for visualization generation
    config: VisualizerConfig,
    /// Template registry for different output formats
    templates: HashMap<String, OutputTemplate>,
    /// Chart generation settings
    chart_config: ChartConfig,
}

/// Configuration for DSL Visualizer
#[derive(Debug, Clone)]
pub struct VisualizerConfig {
    /// Default output format
    pub default_format: OutputFormat,
    /// Enable detailed visualization
    pub enable_detailed_output: bool,
    /// Include audit information in output
    pub include_audit_info: bool,
    /// Enable chart generation
    pub enable_charts: bool,
    /// Maximum output size (bytes)
    pub max_output_size_bytes: usize,
    /// Enable color coding in output
    pub enable_color_coding: bool,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            default_format: OutputFormat::Json,
            enable_detailed_output: true,
            include_audit_info: true,
            enable_charts: false, // Disabled by default for simplicity
            max_output_size_bytes: 1024 * 1024, // 1MB
            enable_color_coding: true,
        }
    }
}

/// Supported output formats
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Json,
    Html,
    Text,
    Markdown,
    Csv,
    Pdf,
}

/// Chart configuration settings
#[derive(Debug, Clone)]
pub struct ChartConfig {
    /// Default chart type
    pub default_chart_type: ChartType,
    /// Chart dimensions
    pub width: u32,
    pub height: u32,
    /// Enable interactive charts
    pub interactive: bool,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            default_chart_type: ChartType::FlowDiagram,
            width: 800,
            height: 600,
            interactive: false,
        }
    }
}

/// Supported chart types
#[derive(Debug, Clone, PartialEq)]
pub enum ChartType {
    FlowDiagram,
    Timeline,
    EntityRelationship,
    ProcessFlow,
    ComplianceMatrix,
    AuditTrail,
}

/// Output template for formatting
#[derive(Debug, Clone)]
pub struct OutputTemplate {
    /// Template name
    pub name: String,
    /// Template format
    pub format: OutputFormat,
    /// Template content/structure
    pub template_content: String,
    /// Variables used in template
    pub variables: Vec<String>,
}

/// Result from visualization generation
#[derive(Debug, Clone)]
pub struct VisualizationResult {
    /// Generation success status
    pub success: bool,
    /// Generated visualization data
    pub visualization_data: String,
    /// Chart type that was generated
    pub chart_type: ChartType,
    /// Output format used
    pub format: OutputFormat,
    /// Generation time in milliseconds
    pub generation_time_ms: u64,
    /// Size of generated output in bytes
    pub output_size_bytes: usize,
    /// Any errors that occurred
    pub errors: Vec<String>,
    /// Warnings during generation
    pub warnings: Vec<String>,
}

/// Input for visualization generation (from DB State Manager result)
#[derive(Debug, Clone)]
pub struct StateResult {
    /// Operation success status
    pub success: bool,
    /// Case ID that was processed
    pub case_id: String,
    /// Version number of the state
    pub version_number: u32,
    /// Snapshot ID for the domain snapshot
    pub snapshot_id: String,
    /// Any errors that occurred
    pub errors: Vec<String>,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Visualization context for generation
#[derive(Debug, Clone)]
pub struct VisualizationContext {
    /// Case ID being visualized
    pub case_id: String,
    /// Target audience for the visualization
    pub audience: AudienceType,
    /// Specific format requested
    pub format: Option<OutputFormat>,
    /// Include sensitive information
    pub include_sensitive: bool,
    /// Visualization purpose
    pub purpose: VisualizationPurpose,
}

/// Target audience types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudienceType {
    Technical,
    Business,
    Compliance,
    Audit,
    Client,
}

/// Purpose of the visualization
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisualizationPurpose {
    StatusReport,
    AuditTrail,
    ComplianceReview,
    TechnicalDebug,
    ClientPresentation,
}

impl DslVisualizer {
    /// Create a new DSL Visualizer with default configuration
    pub fn new() -> Self {
        let mut visualizer = Self {
            config: VisualizerConfig::default(),
            templates: HashMap::new(),
            chart_config: ChartConfig::default(),
        };

        // Initialize default templates
        visualizer.initialize_default_templates();
        visualizer
    }

    /// Create a new DSL Visualizer with custom configuration
    pub fn with_config(config: VisualizerConfig) -> Self {
        let mut visualizer = Self {
            config,
            templates: HashMap::new(),
            chart_config: ChartConfig::default(),
        };

        visualizer.initialize_default_templates();
        visualizer
    }

    /// Generate visualization from state result
    pub async fn generate_visualization(&self, state_result: &StateResult) -> VisualizationResult {
        let start_time = std::time::Instant::now();

        println!(
            "🎨 DSL Visualizer: Generating visualization for case {}",
            state_result.case_id
        );

        // Validate input
        if !state_result.success {
            return VisualizationResult {
                success: false,
                visualization_data: String::new(),
                chart_type: ChartType::FlowDiagram,
                format: self.config.default_format.clone(),
                generation_time_ms: start_time.elapsed().as_millis() as u64,
                output_size_bytes: 0,
                errors: vec!["Cannot visualize failed state result".to_string()],
                warnings: Vec::new(),
            };
        }

        // Generate the visualization content
        let visualization_data = self.generate_visualization_content(state_result).await;
        let chart_type = self.determine_chart_type(state_result);
        let format = self.config.default_format.clone();

        let output_size = visualization_data.len();

        // Check output size limits
        if output_size > self.config.max_output_size_bytes {
            return VisualizationResult {
                success: false,
                visualization_data: String::new(),
                chart_type,
                format,
                generation_time_ms: start_time.elapsed().as_millis() as u64,
                output_size_bytes: output_size,
                errors: vec![format!(
                    "Output size {} bytes exceeds limit of {} bytes",
                    output_size, self.config.max_output_size_bytes
                )],
                warnings: Vec::new(),
            };
        }

        println!("✅ DSL Visualizer: Successfully generated visualization");

        VisualizationResult {
            success: true,
            visualization_data,
            chart_type,
            format,
            generation_time_ms: start_time.elapsed().as_millis() as u64,
            output_size_bytes: output_size,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Generate visualization with specific context
    pub async fn generate_with_context(
        &self,
        state_result: &StateResult,
        context: &VisualizationContext,
    ) -> VisualizationResult {
        println!(
            "🎨 DSL Visualizer: Generating contextual visualization for case {} (audience: {:?}, purpose: {:?})",
            state_result.case_id, context.audience, context.purpose
        );

        let mut result = self.generate_visualization(state_result).await;

        // Customize based on context
        if let Some(format) = &context.format {
            result.format = format.clone();
            result.visualization_data = self
                .convert_format(&result.visualization_data, format)
                .await;
        }

        // Filter content based on audience and sensitivity
        if !context.include_sensitive && context.audience == AudienceType::Client {
            result.visualization_data = self.filter_sensitive_content(&result.visualization_data);
        }

        result
    }

    /// Health check for the visualizer
    pub async fn health_check(&self) -> bool {
        println!("🏥 DSL Visualizer: Performing health check");

        // Check template availability
        let templates_healthy = !self.templates.is_empty();

        // Check configuration validity
        let config_healthy = self.config.max_output_size_bytes > 0;

        let healthy = templates_healthy && config_healthy;
        println!(
            "✅ DSL Visualizer health check: {}",
            if healthy { "HEALTHY" } else { "UNHEALTHY" }
        );
        healthy
    }

    // Private helper methods

    fn initialize_default_templates(&mut self) {
        // JSON template
        let json_template = OutputTemplate {
            name: "default_json".to_string(),
            format: OutputFormat::Json,
            template_content: r#"
{
  "case_id": "{{case_id}}",
  "version": {{version}},
  "snapshot_id": "{{snapshot_id}}",
  "processing_time_ms": {{processing_time_ms}},
  "status": "{{status}}",
  "visualization_type": "{{chart_type}}",
  "generated_at": "{{timestamp}}"
}
"#
            .to_string(),
            variables: vec![
                "case_id".to_string(),
                "version".to_string(),
                "snapshot_id".to_string(),
                "processing_time_ms".to_string(),
                "status".to_string(),
                "chart_type".to_string(),
                "timestamp".to_string(),
            ],
        };

        // Text template
        let text_template = OutputTemplate {
            name: "default_text".to_string(),
            format: OutputFormat::Text,
            template_content: r#"
DSL Visualization Report
========================

Case ID: {{case_id}}
Version: {{version}}
Snapshot ID: {{snapshot_id}}
Processing Time: {{processing_time_ms}}ms
Status: {{status}}
Chart Type: {{chart_type}}
Generated: {{timestamp}}

Summary:
--------
{{summary_content}}
"#
            .to_string(),
            variables: vec![
                "case_id".to_string(),
                "version".to_string(),
                "snapshot_id".to_string(),
                "processing_time_ms".to_string(),
                "status".to_string(),
                "chart_type".to_string(),
                "timestamp".to_string(),
                "summary_content".to_string(),
            ],
        };

        self.templates
            .insert("default_json".to_string(), json_template);
        self.templates
            .insert("default_text".to_string(), text_template);
    }

    async fn generate_visualization_content(&self, state_result: &StateResult) -> String {
        let template_key = match self.config.default_format {
            OutputFormat::Json => "default_json",
            OutputFormat::Text => "default_text",
            _ => "default_json", // Fallback
        };

        if let Some(template) = self.templates.get(template_key) {
            self.apply_template(template, state_result)
        } else {
            // Fallback to simple JSON
            serde_json::json!({
                "case_id": state_result.case_id,
                "version": state_result.version_number,
                "snapshot_id": state_result.snapshot_id,
                "processing_time_ms": state_result.processing_time_ms,
                "status": if state_result.success { "SUCCESS" } else { "FAILED" },
                "errors": state_result.errors,
                "generated_at": chrono::Utc::now().to_rfc3339()
            })
            .to_string()
        }
    }

    fn apply_template(&self, template: &OutputTemplate, state_result: &StateResult) -> String {
        let mut content = template.template_content.clone();

        // Simple template variable replacement
        content = content.replace("{{case_id}}", &state_result.case_id);
        content = content.replace("{{version}}", &state_result.version_number.to_string());
        content = content.replace("{{snapshot_id}}", &state_result.snapshot_id);
        content = content.replace(
            "{{processing_time_ms}}",
            &state_result.processing_time_ms.to_string(),
        );
        content = content.replace(
            "{{status}}",
            if state_result.success {
                "SUCCESS"
            } else {
                "FAILED"
            },
        );
        content = content.replace("{{chart_type}}", "FlowDiagram");
        content = content.replace("{{timestamp}}", &chrono::Utc::now().to_rfc3339());
        content = content.replace(
            "{{summary_content}}",
            &format!(
                "DSL state visualization for case {} version {} completed successfully.",
                state_result.case_id, state_result.version_number
            ),
        );

        content
    }

    fn determine_chart_type(&self, _state_result: &StateResult) -> ChartType {
        // Simple chart type determination
        // In a full implementation, this would analyze the state content
        self.chart_config.default_chart_type.clone()
    }

    async fn convert_format(&self, content: &str, _target_format: &OutputFormat) -> String {
        // Simple format conversion - in a full implementation, this would
        // properly convert between formats
        content.to_string()
    }

    fn filter_sensitive_content(&self, content: &str) -> String {
        // Simple content filtering - in a full implementation, this would
        // remove or redact sensitive information
        content
            .replace("SENSITIVE", "[REDACTED]")
            .replace("SECRET", "[REDACTED]")
    }
}

impl Default for DslVisualizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for VisualizationContext {
    fn default() -> Self {
        Self {
            case_id: String::new(),
            audience: AudienceType::Technical,
            format: None,
            include_sensitive: false,
            purpose: VisualizationPurpose::StatusReport,
        }
    }
}

// Helper functions for visualization

/// Create a basic visualization context
pub fn create_visualization_context(
    case_id: &str,
    audience: AudienceType,
    purpose: VisualizationPurpose,
) -> VisualizationContext {
    VisualizationContext {
        case_id: case_id.to_string(),
        audience,
        format: None,
        include_sensitive: audience == AudienceType::Technical,
        purpose,
    }
}

/// Generate a summary visualization for a case
pub async fn generate_case_summary(
    visualizer: &DslVisualizer,
    case_id: &str,
    version: u32,
    snapshot_id: &str,
) -> VisualizationResult {
    let mock_state_result = StateResult {
        success: true,
        case_id: case_id.to_string(),
        version_number: version,
        snapshot_id: snapshot_id.to_string(),
        errors: Vec::new(),
        processing_time_ms: 0,
    };

    let context = create_visualization_context(
        case_id,
        AudienceType::Business,
        VisualizationPurpose::StatusReport,
    );

    visualizer
        .generate_with_context(&mock_state_result, &context)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dsl_visualizer_creation() {
        let visualizer = DslVisualizer::new();
        assert!(visualizer.health_check().await);
    }

    #[tokio::test]
    async fn test_visualization_generation() {
        let visualizer = DslVisualizer::new();

        let state_result = StateResult {
            success: true,
            case_id: "TEST-001".to_string(),
            version_number: 1,
            snapshot_id: "snapshot_TEST-001_1".to_string(),
            errors: Vec::new(),
            processing_time_ms: 150,
        };

        let result = visualizer.generate_visualization(&state_result).await;
        assert!(result.success);
        assert!(!result.visualization_data.is_empty());
        assert!(result.visualization_data.contains("TEST-001"));
    }

    #[tokio::test]
    async fn test_contextual_visualization() {
        let visualizer = DslVisualizer::new();

        let state_result = StateResult {
            success: true,
            case_id: "TEST-002".to_string(),
            version_number: 2,
            snapshot_id: "snapshot_TEST-002_2".to_string(),
            errors: Vec::new(),
            processing_time_ms: 200,
        };

        let context = create_visualization_context(
            "TEST-002",
            AudienceType::Compliance,
            VisualizationPurpose::AuditTrail,
        );

        let result = visualizer
            .generate_with_context(&state_result, &context)
            .await;
        assert!(result.success);
        assert!(!result.visualization_data.is_empty());
    }

    #[test]
    fn test_chart_type_determination() {
        let visualizer = DslVisualizer::new();
        let state_result = StateResult {
            success: true,
            case_id: "TEST-003".to_string(),
            version_number: 1,
            snapshot_id: "snapshot_TEST-003_1".to_string(),
            errors: Vec::new(),
            processing_time_ms: 100,
        };

        let chart_type = visualizer.determine_chart_type(&state_result);
        assert_eq!(chart_type, ChartType::FlowDiagram);
    }

    #[tokio::test]
    async fn test_failed_state_visualization() {
        let visualizer = DslVisualizer::new();

        let failed_state_result = StateResult {
            success: false,
            case_id: "FAILED-001".to_string(),
            version_number: 0,
            snapshot_id: String::new(),
            errors: vec!["Processing failed".to_string()],
            processing_time_ms: 50,
        };

        let result = visualizer
            .generate_visualization(&failed_state_result)
            .await;
        assert!(!result.success);
        assert!(!result.errors.is_empty());
    }
}
