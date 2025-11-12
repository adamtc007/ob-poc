//! DSL Types - Level 1 Foundation Types (Zero Dependencies)
//!
//! This crate contains pure data structures that form the foundation of the DSL system.
//! It follows the CRITICAL architectural principle: **ZERO DEPENDENCIES** except std and
//! essential serialization crates.
//!
//! ## Architecture Level: LEVEL 1 (Foundation)
//!
//! This is the bottom layer of our dependency hierarchy. All other crates in the system
//! depend on this crate, but this crate depends on NOTHING else in our workspace.
//!
//! ## Contents
//!
//! Pure data structures only:
//! - Source location tracking
//! - Warning severity levels
//! - Processing metadata
//! - Validation structures
//!
//! ## Critical Rules
//!
//! 1. **NO BUSINESS LOGIC** - Only data structures
//! 2. **NO FUNCTIONS** - Except basic constructors and accessors
//! 3. **NO WORKSPACE DEPENDENCIES** - Cannot depend on other workspace crates
//! 4. **SERIALIZABLE** - All types must support serde
//! 5. **THREAD SAFE** - All types should be Send + Sync when possible
//!
//! Adding any dependency to another workspace crate will create circular dependency hell!

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// SOURCE LOCATION AND POSITIONING
// ============================================================================

/// Source location in DSL content for error reporting and debugging
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
    /// Character offset from start of input
    pub offset: usize,
    /// Length of the problematic span
    pub length: usize,
    /// Optional filename or identifier for the source
    pub source_name: Option<String>,
}

impl SourceLocation {
    /// Create a new source location
    pub fn new(line: usize, column: usize, offset: usize, length: usize) -> Self {
        Self {
            line,
            column,
            offset,
            length,
            source_name: None,
        }
    }

    /// Create source location with a source name
    pub fn with_source(
        line: usize,
        column: usize,
        offset: usize,
        length: usize,
        source_name: impl Into<String>,
    ) -> Self {
        Self {
            line,
            column,
            offset,
            length,
            source_name: Some(source_name.into()),
        }
    }

    /// Get a human-readable description of the location
    pub fn description(&self) -> String {
        match &self.source_name {
            Some(name) => format!("{}:{}:{}", name, self.line, self.column),
            None => format!("{}:{}", self.line, self.column),
        }
    }
}

// ============================================================================
// WARNING AND ERROR SEVERITY
// ============================================================================

/// Warning severity levels for validation and processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WarningSeverity {
    /// Low priority suggestion
    Info,
    /// Recommended improvement
    Warning,
    /// Important issue that should be addressed
    Important,
    /// Critical issue that may cause problems
    Critical,
}

impl WarningSeverity {
    /// Get human-readable severity name
    pub fn as_str(&self) -> &'static str {
        match self {
            WarningSeverity::Info => "info",
            WarningSeverity::Warning => "warning",
            WarningSeverity::Important => "important",
            WarningSeverity::Critical => "critical",
        }
    }

    /// Get emoji representation for display
    pub fn emoji(&self) -> &'static str {
        match self {
            WarningSeverity::Info => "ℹ️",
            WarningSeverity::Warning => "⚠️",
            WarningSeverity::Important => "🔶",
            WarningSeverity::Critical => "🚨",
        }
    }
}

// ============================================================================
// PROCESSING METADATA
// ============================================================================

/// Processing metadata for DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingMetadata {
    /// Processing duration in milliseconds
    pub duration_ms: u64,
    /// Domains involved in processing
    pub domains_involved: Vec<String>,
    /// Operations performed
    pub operations_performed: Vec<String>,
    /// Processing timestamp (RFC 3339)
    pub timestamp: String,
    /// Additional context data
    pub context: HashMap<String, String>,
}

impl Default for ProcessingMetadata {
    fn default() -> Self {
        Self {
            duration_ms: 0,
            domains_involved: Vec::new(),
            operations_performed: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            context: HashMap::new(),
        }
    }
}

impl ProcessingMetadata {
    /// Create new processing metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Add domain to the list of involved domains
    pub fn add_domain(&mut self, domain: impl Into<String>) {
        let domain = domain.into();
        if !self.domains_involved.contains(&domain) {
            self.domains_involved.push(domain);
        }
    }

    /// Add operation to the list of performed operations
    pub fn add_operation(&mut self, operation: impl Into<String>) {
        self.operations_performed.push(operation.into());
    }

    /// Add context data
    pub fn add_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.context.insert(key.into(), value.into());
    }
}

// ============================================================================
// VALIDATION STRUCTURES
// ============================================================================

/// Validation metadata for tracking validation state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMetadata {
    /// Validation timestamp
    pub validated_at: String,
    /// Validator identifier
    pub validator_id: String,
    /// Validation rules applied
    pub rules_applied: Vec<String>,
    /// Validation score (0.0 - 1.0)
    pub score: f64,
    /// Additional validation context
    pub context: HashMap<String, String>,
}

impl ValidationMetadata {
    /// Create new validation metadata
    pub fn new(validator_id: impl Into<String>) -> Self {
        Self {
            validated_at: chrono::Utc::now().to_rfc3339(),
            validator_id: validator_id.into(),
            rules_applied: Vec::new(),
            score: 0.0,
            context: HashMap::new(),
        }
    }

    /// Add validation rule to the applied rules
    pub fn add_rule(&mut self, rule: impl Into<String>) {
        self.rules_applied.push(rule.into());
    }
}

// ============================================================================
// UTILITY TYPES
// ============================================================================

/// Generic identifier type for DSL entities
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DslId {
    /// The identifier value
    pub value: String,
    /// Optional namespace/domain
    pub namespace: Option<String>,
}

impl DslId {
    /// Create a new DSL identifier
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            namespace: None,
        }
    }

    /// Create a namespaced DSL identifier
    pub fn with_namespace(value: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            namespace: Some(namespace.into()),
        }
    }

    /// Get the full identifier including namespace
    pub fn full_id(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}:{}", ns, self.value),
            None => self.value.clone(),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_location_creation() {
        let loc = SourceLocation::new(10, 5, 100, 20);
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 5);
        assert_eq!(loc.offset, 100);
        assert_eq!(loc.length, 20);
    }

    #[test]
    fn test_source_location_with_source() {
        let loc = SourceLocation::with_source(1, 1, 0, 5, "test.dsl");
        assert_eq!(loc.source_name, Some("test.dsl".to_string()));
        assert_eq!(loc.description(), "test.dsl:1:1");
    }

    #[test]
    fn test_warning_severity_ordering() {
        assert!(WarningSeverity::Info < WarningSeverity::Warning);
        assert!(WarningSeverity::Warning < WarningSeverity::Important);
        assert!(WarningSeverity::Important < WarningSeverity::Critical);
    }

    #[test]
    fn test_processing_metadata_default() {
        let metadata = ProcessingMetadata::default();
        assert_eq!(metadata.duration_ms, 0);
        assert!(metadata.domains_involved.is_empty());
        assert!(metadata.operations_performed.is_empty());
        assert!(!metadata.timestamp.is_empty());
    }

    #[test]
    fn test_dsl_id_creation() {
        let id = DslId::new("test-123");
        assert_eq!(id.value, "test-123");
        assert_eq!(id.full_id(), "test-123");

        let namespaced_id = DslId::with_namespace("test-123", "kyc");
        assert_eq!(namespaced_id.namespace, Some("kyc".to_string()));
        assert_eq!(namespaced_id.full_id(), "kyc:test-123");
    }

    #[test]
    fn test_serialization() {
        let loc = SourceLocation::new(1, 1, 0, 5);
        let json = serde_json::to_string(&loc).unwrap();
        let deserialized: SourceLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, deserialized);
    }
}
