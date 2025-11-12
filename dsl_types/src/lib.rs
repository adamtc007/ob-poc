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
// VALIDATION ERROR AND WARNING TYPES
// ============================================================================

/// DSL validation error for tracking validation failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Source location (if available)
    pub location: Option<SourceLocation>,
    /// Suggested fix
    pub suggestion: Option<String>,
}

impl ValidationError {
    /// Create a new validation error
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location: None,
            suggestion: None,
        }
    }

    /// Create validation error with location
    pub fn with_location(
        code: impl Into<String>,
        message: impl Into<String>,
        location: SourceLocation,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location: Some(location),
            suggestion: None,
        }
    }

    /// Add a suggested fix to the error
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// DSL validation warning for non-fatal validation issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Warning code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Source location (if available)
    pub location: Option<SourceLocation>,
    /// Severity level
    pub severity: WarningSeverity,
}

impl ValidationWarning {
    /// Create a new validation warning
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        severity: WarningSeverity,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location: None,
            severity,
        }
    }

    /// Create warning with location
    pub fn with_location(
        code: impl Into<String>,
        message: impl Into<String>,
        severity: WarningSeverity,
        location: SourceLocation,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location: Some(location),
            severity,
        }
    }
}

// ============================================================================
// ERROR SEVERITY LEVELS
// ============================================================================

/// Error severity levels for consistent error classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Informational message
    Info,
    /// Warning that should be noted
    Warning,
    /// Error that prevents operation
    Error,
    /// Fatal error that stops execution
    Fatal,
}

impl ErrorSeverity {
    /// Get human-readable severity name
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorSeverity::Info => "info",
            ErrorSeverity::Warning => "warning",
            ErrorSeverity::Error => "error",
            ErrorSeverity::Fatal => "fatal",
        }
    }

    /// Get emoji representation for display
    pub fn emoji(&self) -> &'static str {
        match self {
            ErrorSeverity::Info => "ℹ️",
            ErrorSeverity::Warning => "⚠️",
            ErrorSeverity::Error => "❌",
            ErrorSeverity::Fatal => "💀",
        }
    }
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// OPERATION TYPES
// ============================================================================

/// Attribute operation types for data dictionary operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeOperationType {
    Create,
    Read,
    Update,
    Delete,
    Search,
    Validate,
    Discover,
}

impl AttributeOperationType {
    /// Get human-readable operation name
    pub fn as_str(&self) -> &'static str {
        match self {
            AttributeOperationType::Create => "create",
            AttributeOperationType::Read => "read",
            AttributeOperationType::Update => "update",
            AttributeOperationType::Delete => "delete",
            AttributeOperationType::Search => "search",
            AttributeOperationType::Validate => "validate",
            AttributeOperationType::Discover => "discover",
        }
    }

    /// Get all available operation types
    pub fn all() -> Vec<Self> {
        vec![
            Self::Create,
            Self::Read,
            Self::Update,
            Self::Delete,
            Self::Search,
            Self::Validate,
            Self::Discover,
        ]
    }
}

impl std::fmt::Display for AttributeOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// CONFIGURATION TYPES
// ============================================================================

/// Configuration for AI prompt generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    /// Include schema information in the prompt
    pub include_schemas: bool,
    /// Include grammar rules in the prompt
    pub include_grammar: bool,
    /// Include examples in the prompt
    pub include_examples: bool,
    /// Maximum number of examples to include
    pub max_examples: usize,
    /// Whether to include confidence information
    pub include_confidence: bool,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            include_schemas: true,
            include_grammar: true,
            include_examples: true,
            max_examples: 3,
            include_confidence: false,
        }
    }
}

impl PromptConfig {
    /// Create a minimal prompt configuration
    pub fn minimal() -> Self {
        Self {
            include_schemas: false,
            include_grammar: false,
            include_examples: false,
            max_examples: 0,
            include_confidence: false,
        }
    }

    /// Create a comprehensive prompt configuration
    pub fn comprehensive() -> Self {
        Self {
            include_schemas: true,
            include_grammar: true,
            include_examples: true,
            max_examples: 5,
            include_confidence: true,
        }
    }
}

// ============================================================================
// TRANSACTION MANAGEMENT TYPES
// ============================================================================

/// Transaction execution modes for batch operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionMode {
    /// All operations must succeed or all fail (ACID compliance)
    Atomic,
    /// Operations executed one after another, stopping on first failure
    Sequential,
    /// Operations executed concurrently where possible
    Parallel,
}

impl TransactionMode {
    /// Get human-readable mode name
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionMode::Atomic => "atomic",
            TransactionMode::Sequential => "sequential",
            TransactionMode::Parallel => "parallel",
        }
    }

    /// Check if this mode guarantees ACID properties
    pub fn is_acid_compliant(&self) -> bool {
        matches!(self, TransactionMode::Atomic)
    }
}

impl std::fmt::Display for TransactionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Rollback strategies for failed operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackStrategy {
    /// Roll back all operations on any failure
    FullRollback,
    /// Roll back only completed operations, leave failed ones
    PartialRollback,
    /// Continue processing remaining operations despite failures
    ContinueOnError,
}

impl RollbackStrategy {
    /// Get human-readable strategy name
    pub fn as_str(&self) -> &'static str {
        match self {
            RollbackStrategy::FullRollback => "full_rollback",
            RollbackStrategy::PartialRollback => "partial_rollback",
            RollbackStrategy::ContinueOnError => "continue_on_error",
        }
    }

    /// Check if this strategy provides strong consistency guarantees
    pub fn is_strongly_consistent(&self) -> bool {
        matches!(self, RollbackStrategy::FullRollback)
    }
}

impl std::fmt::Display for RollbackStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// ASSET TYPE CLASSIFICATIONS
// ============================================================================

/// Asset type enumeration for attribute classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeAssetType {
    /// Standard attribute type
    Attribute,
}

impl AttributeAssetType {
    /// Get the database table name for this asset type
    pub fn table_name(&self) -> &'static str {
        match self {
            AttributeAssetType::Attribute => "dictionary",
        }
    }

    /// Get the human-readable asset name
    pub fn asset_name(&self) -> &'static str {
        match self {
            AttributeAssetType::Attribute => "attribute",
        }
    }

    /// Get all available asset types
    pub fn all() -> Vec<Self> {
        vec![Self::Attribute]
    }
}

impl std::fmt::Display for AttributeAssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.asset_name())
    }
}

impl std::str::FromStr for AttributeAssetType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "attribute" => Ok(AttributeAssetType::Attribute),
            _ => Err(format!("Unknown attribute asset type: {}", s)),
        }
    }
}

// ============================================================================
// AGENT METADATA TYPES
// ============================================================================

/// Agent operation metadata for tracking AI agent operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    /// Agent identifier
    pub agent_id: String,
    /// Operation type
    pub operation: String,
    /// Processing duration (milliseconds)
    pub duration_ms: u64,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Additional context
    pub context: HashMap<String, String>,
}

impl AgentMetadata {
    /// Create new agent metadata
    pub fn new(agent_id: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            operation: operation.into(),
            duration_ms: 0,
            confidence: 0.0,
            context: HashMap::new(),
        }
    }

    /// Set processing duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Set confidence score
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add context data
    pub fn add_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.context.insert(key.into(), value.into());
    }

    /// Get confidence as percentage
    pub fn confidence_percentage(&self) -> u8 {
        (self.confidence * 100.0) as u8
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
    fn test_validation_error_creation() {
        let error = ValidationError::new("TEST001", "Test error message");
        assert_eq!(error.code, "TEST001");
        assert_eq!(error.message, "Test error message");
        assert!(error.location.is_none());
        assert!(error.suggestion.is_none());

        let error_with_suggestion = error.with_suggestion("Try this fix");
        assert_eq!(
            error_with_suggestion.suggestion,
            Some("Try this fix".to_string())
        );
    }

    #[test]
    fn test_validation_warning_creation() {
        let warning = ValidationWarning::new("WARN001", "Test warning", WarningSeverity::Warning);
        assert_eq!(warning.code, "WARN001");
        assert_eq!(warning.message, "Test warning");
        assert_eq!(warning.severity, WarningSeverity::Warning);
    }

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Info < ErrorSeverity::Warning);
        assert!(ErrorSeverity::Warning < ErrorSeverity::Error);
        assert!(ErrorSeverity::Error < ErrorSeverity::Fatal);
    }

    #[test]
    fn test_error_severity_display() {
        assert_eq!(ErrorSeverity::Info.to_string(), "info");
        assert_eq!(ErrorSeverity::Fatal.to_string(), "fatal");
        assert_eq!(ErrorSeverity::Error.emoji(), "❌");
    }

    #[test]
    fn test_attribute_operation_type() {
        assert_eq!(AttributeOperationType::Create.to_string(), "create");
        assert_eq!(AttributeOperationType::Delete.as_str(), "delete");

        let all_ops = AttributeOperationType::all();
        assert_eq!(all_ops.len(), 7);
        assert!(all_ops.contains(&AttributeOperationType::Create));
        assert!(all_ops.contains(&AttributeOperationType::Discover));
    }

    #[test]
    fn test_prompt_config_default() {
        let config = PromptConfig::default();
        assert!(config.include_schemas);
        assert!(config.include_grammar);
        assert!(config.include_examples);
        assert_eq!(config.max_examples, 3);
        assert!(!config.include_confidence);
    }

    #[test]
    fn test_prompt_config_variants() {
        let minimal = PromptConfig::minimal();
        assert!(!minimal.include_schemas);
        assert_eq!(minimal.max_examples, 0);

        let comprehensive = PromptConfig::comprehensive();
        assert!(comprehensive.include_schemas);
        assert_eq!(comprehensive.max_examples, 5);
        assert!(comprehensive.include_confidence);
    }

    #[test]
    fn test_transaction_mode() {
        assert_eq!(TransactionMode::Atomic.to_string(), "atomic");
        assert!(TransactionMode::Atomic.is_acid_compliant());
        assert!(!TransactionMode::Parallel.is_acid_compliant());
    }

    #[test]
    fn test_rollback_strategy() {
        assert_eq!(RollbackStrategy::FullRollback.to_string(), "full_rollback");
        assert!(RollbackStrategy::FullRollback.is_strongly_consistent());
        assert!(!RollbackStrategy::ContinueOnError.is_strongly_consistent());
    }

    #[test]
    fn test_attribute_asset_type() {
        assert_eq!(AttributeAssetType::Attribute.to_string(), "attribute");
        assert_eq!(AttributeAssetType::Attribute.table_name(), "dictionary");

        let all_types = AttributeAssetType::all();
        assert_eq!(all_types.len(), 1);
        assert!(all_types.contains(&AttributeAssetType::Attribute));

        // Test FromStr implementation
        assert_eq!(
            "attribute".parse::<AttributeAssetType>().unwrap(),
            AttributeAssetType::Attribute
        );
        assert!("invalid".parse::<AttributeAssetType>().is_err());
    }

    #[test]
    fn test_agent_metadata() {
        let metadata = AgentMetadata::new("test-agent", "dsl_generation");
        assert_eq!(metadata.agent_id, "test-agent");
        assert_eq!(metadata.operation, "dsl_generation");
        assert_eq!(metadata.duration_ms, 0);
        assert_eq!(metadata.confidence, 0.0);

        let metadata_with_confidence = metadata.with_confidence(0.85);
        assert_eq!(metadata_with_confidence.confidence, 0.85);
        assert_eq!(metadata_with_confidence.confidence_percentage(), 85);

        let mut metadata_with_context = metadata_with_confidence.with_duration(1500);
        metadata_with_context.add_context("model", "gpt-4");
        assert_eq!(metadata_with_context.duration_ms, 1500);
        assert_eq!(
            metadata_with_context.context.get("model"),
            Some(&"gpt-4".to_string())
        );
    }

    #[test]
    fn test_serialization() {
        let loc = SourceLocation::new(1, 1, 0, 5);
        let json = serde_json::to_string(&loc).unwrap();
        let deserialized: SourceLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, deserialized);
    }
}
