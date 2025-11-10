//! DSL Visualizer Module
//!
//! This module provides an interactive egui-based desktop application for visualizing
//! DSL code and corresponding AST structures. It includes:
//!
//! - DSL Browser: List and filter DSL instances from the database
//! - AST Viewer: Interactive tree/graph visualization of parsed AST
//! - gRPC Integration: Real-time communication with backend services
//! - Multi-panel UI: Clean, responsive desktop interface
//!
//! Architecture:
//! ```
//! ┌─────────────────────────────────────────┐
//! │           DSLVisualizerApp              │
//! ├─────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────────┐   │
//! │  │DSLBrowser   │  │   ASTViewer     │   │
//! │  │Panel        │  │   Panel         │   │
//! │  └─────────────┘  └─────────────────┘   │
//! ├─────────────────────────────────────────┤
//! │           gRPC Client Layer             │
//! └─────────────────────────────────────────┘
//! ```

pub mod app;
pub mod ast_viewer;
pub mod dsl_browser;
pub mod grpc_client;
pub mod models;

pub use app::DSLVisualizerApp;
pub use ast_viewer::{ASTViewerPanel, LayoutMode};
pub use dsl_browser::DSLBrowserPanel;
pub use grpc_client::DSLServiceClient;
pub use models::*;

/// Result type for visualizer operations
pub type VisualizerResult<T> = Result<T, VisualizerError>;

/// Error types for the DSL visualizer
#[derive(Debug, thiserror::Error)]
pub enum VisualizerError {
    #[error("gRPC error: {message}")]
    GrpcError { message: String },

    #[error("Parsing error: {message}")]
    ParseError { message: String },

    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("UI error: {message}")]
    UIError { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Configuration for the DSL visualizer application
#[derive(Debug, Clone)]
pub struct VisualizerConfig {
    /// gRPC server endpoint
    pub grpc_endpoint: String,

    /// Window title
    pub window_title: String,

    /// Initial window size (width, height)
    pub window_size: (f32, f32),

    /// Enable dark mode by default
    pub dark_mode: bool,

    /// Auto-refresh interval in seconds
    pub auto_refresh_interval: u64,

    /// Maximum DSL entries to display
    pub max_dsl_entries: usize,

    /// Enable debug logging
    pub debug_mode: bool,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            grpc_endpoint: "http://127.0.0.1:50051".to_string(),
            window_title: "DSL/AST Visualizer - OB-POC".to_string(),
            window_size: (1200.0, 800.0),
            dark_mode: true,
            auto_refresh_interval: 30,
            max_dsl_entries: 100,
            debug_mode: false,
        }
    }
}

/// Initialize the DSL visualizer application with default configuration
pub fn create_app() -> DSLVisualizerApp {
    DSLVisualizerApp::new(VisualizerConfig::default())
}

/// Initialize the DSL visualizer application with custom configuration
pub fn create_app_with_config(config: VisualizerConfig) -> DSLVisualizerApp {
    DSLVisualizerApp::new(config)
}

/// Utility function to format DSL content for display
pub fn format_dsl_preview(content: &str, max_lines: usize) -> String {
    content
        .lines()
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Utility function to extract domain from DSL content
pub fn extract_domain_from_dsl(content: &str) -> Option<String> {
    // Simple pattern matching for domain extraction
    // Look for patterns like (onboarding.create, (kyc.verify, etc.
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('(') {
            if let Some(dot_pos) = trimmed.find('.') {
                let domain_part = &trimmed[1..dot_pos];
                if !domain_part.is_empty()
                    && domain_part.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    return Some(domain_part.to_string());
                }
            }
        }
    }
    None
}

/// Utility function to count DSL operations in content
pub fn count_dsl_operations(content: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('(') && trimmed.contains('.')
        })
        .count()
}

/// Constants for the visualizer UI
pub mod constants {
    /// Default panel spacing
    pub const PANEL_SPACING: f32 = 8.0;

    /// Default margin
    pub const DEFAULT_MARGIN: f32 = 10.0;

    /// Minimum panel width
    pub const MIN_PANEL_WIDTH: f32 = 250.0;

    /// Default font size
    pub const DEFAULT_FONT_SIZE: f32 = 14.0;

    /// Code font size
    pub const CODE_FONT_SIZE: f32 = 12.0;

    /// DSL browser panel width ratio
    pub const DSL_BROWSER_WIDTH_RATIO: f32 = 0.3;

    /// AST viewer panel width ratio
    pub const AST_VIEWER_WIDTH_RATIO: f32 = 0.7;

    /// Maximum lines to show in DSL preview
    pub const MAX_PREVIEW_LINES: usize = 5;

    /// Default refresh rate in milliseconds
    pub const DEFAULT_REFRESH_RATE_MS: u64 = 1000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_dsl_preview() {
        let content = r#"
;; Test DSL
(onboarding.create :id "test")
(products.add "CUSTODY")
(kyc.verify :status "approved")
(compliance.check)
"#
        .trim();

        let preview = format_dsl_preview(content, 3);
        let lines: Vec<&str> = preview.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], ";; Test DSL");
    }

    #[test]
    fn test_extract_domain_from_dsl() {
        let content = "(onboarding.create :id \"test\")";
        assert_eq!(
            extract_domain_from_dsl(content),
            Some("onboarding".to_string())
        );

        let content = "(kyc.verify :status \"approved\")";
        assert_eq!(extract_domain_from_dsl(content), Some("kyc".to_string()));

        let content = "invalid content";
        assert_eq!(extract_domain_from_dsl(content), None);
    }

    #[test]
    fn test_count_dsl_operations() {
        let content = r#"
;; Comment
(onboarding.create :id "test")
(products.add "CUSTODY")
    (kyc.verify :status "approved")
some text
(compliance.check)
"#;

        assert_eq!(count_dsl_operations(content), 4);
    }

    #[test]
    fn test_default_config() {
        let config = VisualizerConfig::default();
        assert_eq!(config.grpc_endpoint, "http://127.0.0.1:50051");
        assert!(config.dark_mode);
        assert_eq!(config.window_size, (1200.0, 800.0));
    }
}
