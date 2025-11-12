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

    /// Create a source location with source name
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

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
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
// EXECUTION STATUS TYPES
// ============================================================================

/// Execution status for dictionary operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DictionaryExecutionStatus {
    /// Operation is queued and waiting to execute
    Pending,
    /// Operation is currently executing
    Executing,
    /// Operation completed successfully
    Completed,
    /// Operation failed with error
    Failed,
    /// Operation was rolled back due to failure
    RolledBack,
}

impl DictionaryExecutionStatus {
    /// Get human-readable status name
    pub fn as_str(&self) -> &'static str {
        match self {
            DictionaryExecutionStatus::Pending => "pending",
            DictionaryExecutionStatus::Executing => "executing",
            DictionaryExecutionStatus::Completed => "completed",
            DictionaryExecutionStatus::Failed => "failed",
            DictionaryExecutionStatus::RolledBack => "rolled_back",
        }
    }

    /// Check if this status represents a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DictionaryExecutionStatus::Completed
                | DictionaryExecutionStatus::Failed
                | DictionaryExecutionStatus::RolledBack
        )
    }

    /// Check if this status represents success
    pub fn is_successful(&self) -> bool {
        matches!(self, DictionaryExecutionStatus::Completed)
    }
}

impl std::fmt::Display for DictionaryExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for DictionaryExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(DictionaryExecutionStatus::Pending),
            "executing" => Ok(DictionaryExecutionStatus::Executing),
            "completed" => Ok(DictionaryExecutionStatus::Completed),
            "failed" => Ok(DictionaryExecutionStatus::Failed),
            "rolled_back" | "rolledback" => Ok(DictionaryExecutionStatus::RolledBack),
            _ => Err(format!("Unknown dictionary execution status: {}", s)),
        }
    }
}

/// Request status for business operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestStatus {
    /// Request is in draft state
    Draft,
    /// Request is being processed
    InProgress,
    /// Request is under review
    Review,
    /// Request has been approved
    Approved,
    /// Request has been completed
    Completed,
    /// Request has been cancelled
    Cancelled,
    /// Request encountered an error
    Error,
}

impl RequestStatus {
    /// Get human-readable status name
    pub fn as_str(&self) -> &'static str {
        match self {
            RequestStatus::Draft => "draft",
            RequestStatus::InProgress => "in_progress",
            RequestStatus::Review => "review",
            RequestStatus::Approved => "approved",
            RequestStatus::Completed => "completed",
            RequestStatus::Cancelled => "cancelled",
            RequestStatus::Error => "error",
        }
    }

    /// Check if this status allows transitions
    pub fn can_transition(&self) -> bool {
        !matches!(
            self,
            RequestStatus::Completed | RequestStatus::Cancelled | RequestStatus::Error
        )
    }

    /// Check if this status represents success
    pub fn is_successful(&self) -> bool {
        matches!(self, RequestStatus::Completed)
    }

    /// Get all possible statuses
    pub fn all() -> Vec<Self> {
        vec![
            Self::Draft,
            Self::InProgress,
            Self::Review,
            Self::Approved,
            Self::Completed,
            Self::Cancelled,
            Self::Error,
        ]
    }
}

impl std::fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<String> for RequestStatus {
    fn from(s: String) -> Self {
        match s.to_uppercase().as_str() {
            "DRAFT" => RequestStatus::Draft,
            "IN_PROGRESS" | "IN PROGRESS" => RequestStatus::InProgress,
            "REVIEW" => RequestStatus::Review,
            "APPROVED" => RequestStatus::Approved,
            "COMPLETED" => RequestStatus::Completed,
            "CANCELLED" => RequestStatus::Cancelled,
            "ERROR" => RequestStatus::Error,
            _ => RequestStatus::Draft, // Default fallback
        }
    }
}

// ============================================================================
// DSL PROCESSING AND VALIDATION TYPES - BATCH 6
// ============================================================================

/// Result of DSL processing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    /// Processing success status
    pub success: bool,
    /// Processed DSL content
    pub processed_dsl: String,
    /// Validation report
    pub validation_report: ValidationReport,
    /// Processing metadata
    pub metadata: ProcessingMetadata,
}

impl ProcessingResult {
    /// Create a new processing result
    pub fn new(processed_dsl: String, validation_report: ValidationReport) -> Self {
        Self {
            success: validation_report.is_valid,
            processed_dsl,
            validation_report,
            metadata: ProcessingMetadata::default(),
        }
    }

    /// Create successful processing result
    pub fn success(processed_dsl: String) -> Self {
        Self {
            success: true,
            processed_dsl,
            validation_report: ValidationReport::valid(),
            metadata: ProcessingMetadata::default(),
        }
    }

    /// Create failed processing result
    pub fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            success: false,
            processed_dsl: String::new(),
            validation_report: ValidationReport::with_errors(errors),
            metadata: ProcessingMetadata::default(),
        }
    }

    /// Check if processing was successful
    pub fn is_successful(&self) -> bool {
        self.success && self.validation_report.is_valid
    }
}

/// DSL validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Overall validation status
    pub is_valid: bool,
    /// Domain validation results
    pub domain_validations: HashMap<String, DomainValidation>,
    /// Vocabulary compliance
    pub vocabulary_compliance: VocabularyValidation,
    /// Validation errors
    pub errors: Vec<ValidationError>,
    /// Validation warnings
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    /// Create a new validation report
    pub fn new() -> Self {
        Self {
            is_valid: true,
            domain_validations: HashMap::new(),
            vocabulary_compliance: VocabularyValidation::default(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a valid validation report
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            domain_validations: HashMap::new(),
            vocabulary_compliance: VocabularyValidation::compliant(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create validation report with errors
    pub fn with_errors(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: errors.is_empty(),
            domain_validations: HashMap::new(),
            vocabulary_compliance: VocabularyValidation::default(),
            errors,
            warnings: Vec::new(),
        }
    }

    /// Add a validation error
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
        self.is_valid = false;
    }

    /// Add a validation warning
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// Add domain validation result
    pub fn add_domain_validation(&mut self, domain: String, validation: DomainValidation) {
        if !validation.valid {
            self.is_valid = false;
        }
        self.domain_validations.insert(domain, validation);
    }

    /// Get total error count
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Get total warning count
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Check if report has any issues
    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty()
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Domain-specific validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainValidation {
    /// Domain identifier
    pub domain: String,
    /// Validation success
    pub valid: bool,
    /// Domain-specific rules validated
    pub rules_checked: Vec<String>,
    /// Domain compliance score (0.0 - 1.0)
    pub compliance_score: f64,
}

impl DomainValidation {
    /// Create a new domain validation
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            valid: true,
            rules_checked: Vec::new(),
            compliance_score: 1.0,
        }
    }

    /// Create successful domain validation
    pub fn success(domain: impl Into<String>, rules_checked: Vec<String>) -> Self {
        Self {
            domain: domain.into(),
            valid: true,
            rules_checked,
            compliance_score: 1.0,
        }
    }

    /// Create failed domain validation
    pub fn failure(domain: impl Into<String>, compliance_score: f64) -> Self {
        Self {
            domain: domain.into(),
            valid: false,
            rules_checked: Vec::new(),
            compliance_score: compliance_score.clamp(0.0, 1.0),
        }
    }

    /// Add checked rule
    pub fn add_rule(&mut self, rule: impl Into<String>) {
        self.rules_checked.push(rule.into());
    }

    /// Set compliance score
    pub fn set_compliance(&mut self, score: f64) {
        self.compliance_score = score.clamp(0.0, 1.0);
        if score < 1.0 {
            self.valid = false;
        }
    }

    /// Get compliance percentage
    pub fn compliance_percentage(&self) -> u8 {
        (self.compliance_score * 100.0) as u8
    }
}

/// Vocabulary validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabularyValidation {
    /// All verbs are approved
    pub all_verbs_approved: bool,
    /// Approved verbs used
    pub approved_verbs: Vec<String>,
    /// Unknown/unapproved verbs
    pub unknown_verbs: Vec<String>,
    /// AttributeID compliance
    pub attribute_compliance: f64,
}

impl VocabularyValidation {
    /// Create new vocabulary validation
    pub fn new() -> Self {
        Self {
            all_verbs_approved: true,
            approved_verbs: Vec::new(),
            unknown_verbs: Vec::new(),
            attribute_compliance: 1.0,
        }
    }

    /// Create compliant vocabulary validation
    pub fn compliant() -> Self {
        Self {
            all_verbs_approved: true,
            approved_verbs: Vec::new(),
            unknown_verbs: Vec::new(),
            attribute_compliance: 1.0,
        }
    }

    /// Create non-compliant vocabulary validation
    pub fn non_compliant(unknown_verbs: Vec<String>) -> Self {
        Self {
            all_verbs_approved: false,
            approved_verbs: Vec::new(),
            unknown_verbs,
            attribute_compliance: 0.0,
        }
    }

    /// Add approved verb
    pub fn add_approved_verb(&mut self, verb: impl Into<String>) {
        self.approved_verbs.push(verb.into());
    }

    /// Add unknown verb
    pub fn add_unknown_verb(&mut self, verb: impl Into<String>) {
        self.unknown_verbs.push(verb.into());
        self.all_verbs_approved = false;
    }

    /// Set attribute compliance score
    pub fn set_attribute_compliance(&mut self, score: f64) {
        self.attribute_compliance = score.clamp(0.0, 1.0);
    }

    /// Get overall vocabulary score (0.0 - 1.0)
    pub fn overall_score(&self) -> f64 {
        if !self.all_verbs_approved {
            return 0.0;
        }
        self.attribute_compliance
    }

    /// Check if vocabulary is fully compliant
    pub fn is_compliant(&self) -> bool {
        self.all_verbs_approved && self.attribute_compliance >= 1.0
    }

    /// Get compliance percentage
    pub fn compliance_percentage(&self) -> u8 {
        (self.overall_score() * 100.0) as u8
    }
}

impl Default for VocabularyValidation {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// DSL ERROR TYPES - BATCH 7
// ============================================================================

/// Comprehensive DSL error types matching all expected variants in codebase
#[derive(Debug, thiserror::Error)]
pub enum DslError {
    // Single-string constructor variants (most common pattern)
    #[error("Domain validation error: {0}")]
    DomainValidationError(String),

    #[error("Grammar validation error: {0}")]
    GrammarValidationError(String),

    #[error("Dictionary validation error: {0}")]
    DictionaryValidationError(String),

    #[error("Compilation error: {0}")]
    CompilationError(String),

    #[error("Domain not found: {0}")]
    DomainNotFound(String),

    // Two-string constructor variants
    #[error("Unsupported operation '{0}' in domain '{1}'")]
    UnsupportedOperation(String, String),

    // Structured error variants (from original enum)
    #[error("Parse error: {message} at {location:?}")]
    ParseError {
        message: String,
        location: Option<SourceLocation>,
    },

    #[error("Validation failed: {errors:?}")]
    ValidationError { errors: Vec<ValidationError> },

    #[error("Domain error in {domain}: {message}")]
    DomainError { domain: String, message: String },

    #[error("Operation failed: {operation} - {reason}")]
    OperationError { operation: String, reason: String },

    #[error("Configuration error: {details}")]
    ConfigurationError { details: String },

    #[error("Internal processing error: {details}")]
    InternalError { details: String },
}

impl DslError {
    /// Create a domain validation error
    pub fn domain_validation(message: impl Into<String>) -> Self {
        Self::DomainValidationError(message.into())
    }

    /// Create a grammar validation error
    pub fn grammar_validation(message: impl Into<String>) -> Self {
        Self::GrammarValidationError(message.into())
    }

    /// Create a dictionary validation error
    pub fn dictionary_validation(message: impl Into<String>) -> Self {
        Self::DictionaryValidationError(message.into())
    }

    /// Create a compilation error
    pub fn compilation(message: impl Into<String>) -> Self {
        Self::CompilationError(message.into())
    }

    /// Create an unsupported operation error
    pub fn unsupported_operation(operation: impl Into<String>, domain: impl Into<String>) -> Self {
        Self::UnsupportedOperation(operation.into(), domain.into())
    }

    /// Create a domain not found error
    pub fn domain_not_found(domain: impl Into<String>) -> Self {
        Self::DomainNotFound(domain.into())
    }

    /// Create a parse error with location
    pub fn parse_with_location(
        message: impl Into<String>,
        location: Option<SourceLocation>,
    ) -> Self {
        Self::ParseError {
            message: message.into(),
            location,
        }
    }

    /// Create a validation error with multiple validation errors
    pub fn validation_with_errors(errors: Vec<ValidationError>) -> Self {
        Self::ValidationError { errors }
    }

    /// Create a domain error
    pub fn domain(domain: impl Into<String>, message: impl Into<String>) -> Self {
        Self::DomainError {
            domain: domain.into(),
            message: message.into(),
        }
    }

    /// Create an operation error
    pub fn operation(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::OperationError {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration(details: impl Into<String>) -> Self {
        Self::ConfigurationError {
            details: details.into(),
        }
    }

    /// Create an internal error
    pub fn internal(details: impl Into<String>) -> Self {
        Self::InternalError {
            details: details.into(),
        }
    }
}

/// DSL result type for all operations
pub type DslResult<T> = Result<T, DslError>;

// ============================================================================
// AI SERVICE TYPES - BATCH 8 (PHASE 2.3)
// ============================================================================

/// AI service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// API key for the AI service
    pub api_key: String,

    /// Model name/version to use
    pub model: String,

    /// Maximum tokens in response
    pub max_tokens: Option<u32>,

    /// Temperature for response generation (0.0 - 1.0)
    pub temperature: Option<f32>,

    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("AI_API_KEY").unwrap_or_default(),
            model: "default-model".to_string(),
            max_tokens: Some(8192),
            temperature: Some(0.1),
            timeout_seconds: 30,
        }
    }
}

impl AiConfig {
    /// Create new AI configuration
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            max_tokens: Some(8192),
            temperature: Some(0.1),
            timeout_seconds: 30,
        }
    }

    /// Set maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }
}

/// AI request for DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDslRequest {
    /// The instruction or question for the AI
    pub instruction: String,

    /// Optional context for the request
    pub context: Option<HashMap<String, String>>,

    /// Expected response type
    pub response_type: AiResponseType,

    /// Temperature for AI generation
    pub temperature: Option<f64>,

    /// Maximum tokens for response
    pub max_tokens: Option<u32>,
}

impl AiDslRequest {
    /// Create new AI DSL request
    pub fn new(instruction: impl Into<String>, response_type: AiResponseType) -> Self {
        Self {
            instruction: instruction.into(),
            context: None,
            response_type,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Add context to the request
    pub fn with_context(mut self, context: HashMap<String, String>) -> Self {
        self.context = Some(context);
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// Type of AI response expected
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AiResponseType {
    /// Generate new DSL from scratch
    DslGeneration,

    /// Transform existing DSL
    DslTransformation,

    /// Validate DSL and provide feedback
    DslValidation,

    /// Explain DSL structure and meaning
    DslExplanation,

    /// Suggest improvements to DSL
    DslSuggestions,
}

impl AiResponseType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DslGeneration => "dsl_generation",
            Self::DslTransformation => "dsl_transformation",
            Self::DslValidation => "dsl_validation",
            Self::DslExplanation => "dsl_explanation",
            Self::DslSuggestions => "dsl_suggestions",
        }
    }

    /// Get all response types
    pub fn all() -> Vec<Self> {
        vec![
            Self::DslGeneration,
            Self::DslTransformation,
            Self::DslValidation,
            Self::DslExplanation,
            Self::DslSuggestions,
        ]
    }
}

impl std::fmt::Display for AiResponseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// AI response containing DSL and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDslResponse {
    /// Generated or transformed DSL content
    pub generated_dsl: String,

    /// Explanation of what was done
    pub explanation: String,

    /// Confidence score (0.0 - 1.0)
    pub confidence: Option<f64>,

    /// List of changes made (for transformations)
    pub changes: Option<Vec<String>>,

    /// Warnings or concerns about the DSL
    pub warnings: Option<Vec<String>>,

    /// Suggestions for improvement
    pub suggestions: Option<Vec<String>>,
}

impl AiDslResponse {
    /// Create new AI DSL response
    pub fn new(generated_dsl: impl Into<String>, explanation: impl Into<String>) -> Self {
        Self {
            generated_dsl: generated_dsl.into(),
            explanation: explanation.into(),
            confidence: None,
            changes: None,
            warnings: None,
            suggestions: None,
        }
    }

    /// Set confidence score
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Add changes list
    pub fn with_changes(mut self, changes: Vec<String>) -> Self {
        self.changes = Some(changes);
        self
    }

    /// Add warnings
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = Some(warnings);
        self
    }

    /// Add suggestions
    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = Some(suggestions);
        self
    }

    /// Check if response has warnings
    pub fn has_warnings(&self) -> bool {
        self.warnings.as_ref().is_some_and(|w| !w.is_empty())
    }

    /// Check if response has suggestions
    pub fn has_suggestions(&self) -> bool {
        self.suggestions.as_ref().is_some_and(|s| !s.is_empty())
    }

    /// Get confidence percentage
    pub fn confidence_percentage(&self) -> u8 {
        self.confidence.unwrap_or(0.0) as u8 * 100
    }
}

/// Errors that can occur during AI operations
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("JSON parsing error: {0}")]
    JsonError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Authentication error: missing or invalid API key")]
    AuthenticationError,

    #[error("Rate limit exceeded")]
    RateLimitError,

    #[error("AI service timeout")]
    TimeoutError,

    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

impl AiError {
    /// Create HTTP error
    pub fn http(message: impl Into<String>) -> Self {
        Self::HttpError(message.into())
    }

    /// Create JSON error
    pub fn json(message: impl Into<String>) -> Self {
        Self::JsonError(message.into())
    }

    /// Create API error
    pub fn api(message: impl Into<String>) -> Self {
        Self::ApiError(message.into())
    }

    /// Create invalid response error
    pub fn invalid_response(message: impl Into<String>) -> Self {
        Self::InvalidResponse(message.into())
    }

    /// Create configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::ConfigurationError(message.into())
    }

    /// Create network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::NetworkError(message.into())
    }
}

/// Result type for AI operations
pub type AiResult<T> = Result<T, AiError>;

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
    fn test_dictionary_execution_status() {
        assert_eq!(DictionaryExecutionStatus::Pending.to_string(), "pending");
        assert!(DictionaryExecutionStatus::Completed.is_terminal());
        assert!(DictionaryExecutionStatus::Completed.is_successful());
        assert!(!DictionaryExecutionStatus::Executing.is_terminal());
        assert!(!DictionaryExecutionStatus::Failed.is_successful());

        // Test FromStr implementation
        assert_eq!(
            "completed".parse::<DictionaryExecutionStatus>().unwrap(),
            DictionaryExecutionStatus::Completed
        );
        assert_eq!(
            "rolled_back".parse::<DictionaryExecutionStatus>().unwrap(),
            DictionaryExecutionStatus::RolledBack
        );
        assert!("invalid".parse::<DictionaryExecutionStatus>().is_err());
    }

    #[test]
    fn test_request_status() {
        assert_eq!(RequestStatus::Draft.to_string(), "draft");
        assert!(RequestStatus::Draft.can_transition());
        assert!(!RequestStatus::Completed.can_transition());
        assert!(RequestStatus::Completed.is_successful());
        assert!(!RequestStatus::Error.is_successful());

        let all_statuses = RequestStatus::all();
        assert_eq!(all_statuses.len(), 7);
        assert!(all_statuses.contains(&RequestStatus::Draft));
        assert!(all_statuses.contains(&RequestStatus::Error));

        // Test From<String> implementation
        assert_eq!(
            RequestStatus::from("COMPLETED".to_string()),
            RequestStatus::Completed
        );
        assert_eq!(
            RequestStatus::from("in_progress".to_string()),
            RequestStatus::InProgress
        );
        assert_eq!(
            RequestStatus::from("invalid".to_string()),
            RequestStatus::Draft
        );
    }

    #[test]
    fn test_processing_result() {
        let validation_report = ValidationReport::valid();
        let result = ProcessingResult::new("processed dsl".to_string(), validation_report);
        assert!(result.success);
        assert!(result.is_successful());
        assert_eq!(result.processed_dsl, "processed dsl");

        let success_result = ProcessingResult::success("success dsl".to_string());
        assert!(success_result.success);
        assert!(success_result.is_successful());

        let error = ValidationError::new("TEST001", "Test error");
        let failure_result = ProcessingResult::failure(vec![error]);
        assert!(!failure_result.success);
        assert!(!failure_result.is_successful());
        assert_eq!(failure_result.validation_report.error_count(), 1);
    }

    #[test]
    fn test_validation_report() {
        let mut report = ValidationReport::new();
        assert!(report.is_valid);
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 0);
        assert!(!report.has_issues());

        let error = ValidationError::new("TEST001", "Test error");
        report.add_error(error);
        assert!(!report.is_valid);
        assert_eq!(report.error_count(), 1);
        assert!(report.has_issues());

        let warning = ValidationWarning::new("WARN001", "Test warning", WarningSeverity::Warning);
        report.add_warning(warning);
        assert_eq!(report.warning_count(), 1);

        let domain_validation = DomainValidation::success("kyc", vec!["rule1".to_string()]);
        report.add_domain_validation("kyc".to_string(), domain_validation);
        assert_eq!(report.domain_validations.len(), 1);
    }

    #[test]
    fn test_validation_report_constructors() {
        let valid_report = ValidationReport::valid();
        assert!(valid_report.is_valid);
        assert!(valid_report.vocabulary_compliance.is_compliant());

        let error = ValidationError::new("ERR001", "Test error");
        let error_report = ValidationReport::with_errors(vec![error]);
        assert!(!error_report.is_valid);
        assert_eq!(error_report.error_count(), 1);
    }

    #[test]
    fn test_domain_validation() {
        let mut validation = DomainValidation::new("kyc");
        assert!(validation.valid);
        assert_eq!(validation.domain, "kyc");
        assert_eq!(validation.compliance_score, 1.0);
        assert_eq!(validation.compliance_percentage(), 100);

        validation.add_rule("rule1");
        validation.add_rule("rule2");
        assert_eq!(validation.rules_checked.len(), 2);

        validation.set_compliance(0.85);
        assert!(!validation.valid);
        assert_eq!(validation.compliance_percentage(), 85);

        let success_validation = DomainValidation::success("ubo", vec!["rule1".to_string()]);
        assert!(success_validation.valid);
        assert_eq!(success_validation.rules_checked.len(), 1);

        let failure_validation = DomainValidation::failure("onboarding", 0.5);
        assert!(!failure_validation.valid);
        assert_eq!(failure_validation.compliance_score, 0.5);
    }

    #[test]
    fn test_vocabulary_validation() {
        let mut validation = VocabularyValidation::new();
        assert!(validation.all_verbs_approved);
        assert_eq!(validation.attribute_compliance, 1.0);
        assert_eq!(validation.overall_score(), 1.0);
        assert!(validation.is_compliant());
        assert_eq!(validation.compliance_percentage(), 100);

        validation.add_approved_verb("kyc.start");
        validation.add_approved_verb("ubo.collect");
        assert_eq!(validation.approved_verbs.len(), 2);

        validation.add_unknown_verb("invalid.verb");
        assert!(!validation.all_verbs_approved);
        assert_eq!(validation.unknown_verbs.len(), 1);
        assert_eq!(validation.overall_score(), 0.0);
        assert!(!validation.is_compliant());

        let compliant_validation = VocabularyValidation::compliant();
        assert!(compliant_validation.is_compliant());

        let non_compliant_validation =
            VocabularyValidation::non_compliant(vec!["bad.verb".to_string()]);
        assert!(!non_compliant_validation.all_verbs_approved);
        assert_eq!(non_compliant_validation.unknown_verbs.len(), 1);
        assert!(!non_compliant_validation.is_compliant());
    }

    #[test]
    fn test_vocabulary_validation_scoring() {
        let mut validation = VocabularyValidation::new();

        // Test with approved verbs but lower attribute compliance
        validation.set_attribute_compliance(0.7);
        assert_eq!(validation.overall_score(), 0.7);
        assert_eq!(validation.compliance_percentage(), 70);

        // Test with unknown verbs - should always return 0.0
        validation.add_unknown_verb("unknown");
        assert_eq!(validation.overall_score(), 0.0);
        assert_eq!(validation.compliance_percentage(), 0);
    }

    #[test]
    fn test_dsl_error_creation() {
        // Test single-string constructor variants
        let domain_error = DslError::domain_validation("Invalid customer ID");
        assert!(matches!(domain_error, DslError::DomainValidationError(_)));

        let grammar_error = DslError::grammar_validation("Syntax error at line 5");
        assert!(matches!(grammar_error, DslError::GrammarValidationError(_)));

        let dict_error = DslError::dictionary_validation("Unknown attribute UUID");
        assert!(matches!(dict_error, DslError::DictionaryValidationError(_)));

        let comp_error = DslError::compilation("Failed to parse DSL");
        assert!(matches!(comp_error, DslError::CompilationError(_)));

        // Test two-string constructor variants
        let unsupported_error = DslError::unsupported_operation("invalid.verb", "kyc");
        assert!(matches!(
            unsupported_error,
            DslError::UnsupportedOperation(_, _)
        ));

        let domain_not_found = DslError::domain_not_found("nonexistent_domain");
        assert!(matches!(domain_not_found, DslError::DomainNotFound(_)));

        // Test structured error variants
        let parse_error =
            DslError::parse_with_location("Parse failure", Some(SourceLocation::new(1, 1, 0, 5)));
        assert!(matches!(parse_error, DslError::ParseError { .. }));

        let validation_error = DslError::validation_with_errors(vec![ValidationError::new(
            "ERR001",
            "Test validation error",
        )]);
        assert!(matches!(validation_error, DslError::ValidationError { .. }));

        let domain_err = DslError::domain("kyc", "Domain-specific error");
        assert!(matches!(domain_err, DslError::DomainError { .. }));

        let op_error = DslError::operation("kyc.start", "Operation failed");
        assert!(matches!(op_error, DslError::OperationError { .. }));

        let config_error = DslError::configuration("Invalid configuration");
        assert!(matches!(config_error, DslError::ConfigurationError { .. }));

        let internal_error = DslError::internal("Internal processing failed");
        assert!(matches!(internal_error, DslError::InternalError { .. }));
    }

    #[test]
    fn test_dsl_error_display() {
        let error = DslError::domain_validation("Test domain validation error");
        let error_string = error.to_string();
        assert!(error_string.contains("Domain validation error"));
        assert!(error_string.contains("Test domain validation error"));

        let unsupported = DslError::unsupported_operation("test.verb", "test_domain");
        let unsupported_string = unsupported.to_string();
        assert!(unsupported_string.contains("Unsupported operation"));
        assert!(unsupported_string.contains("test.verb"));
        assert!(unsupported_string.contains("test_domain"));
    }

    #[test]
    fn test_dsl_result_type() {
        let success: DslResult<String> = Ok("success".to_string());
        assert!(success.is_ok());

        let failure: DslResult<String> = Err(DslError::compilation("Test compilation error"));
        assert!(failure.is_err());

        if let Err(error) = failure {
            assert!(matches!(error, DslError::CompilationError(_)));
        }
    }

    #[test]
    fn test_ai_config() {
        let config = AiConfig::new("test-key", "test-model");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.model, "test-model");
        assert_eq!(config.max_tokens, Some(8192));
        assert_eq!(config.timeout_seconds, 30);

        let custom_config = config
            .with_max_tokens(4096)
            .with_temperature(0.5)
            .with_timeout(60);
        assert_eq!(custom_config.max_tokens, Some(4096));
        assert_eq!(custom_config.temperature, Some(0.5));
        assert_eq!(custom_config.timeout_seconds, 60);
    }

    #[test]
    fn test_ai_dsl_request() {
        let request = AiDslRequest::new("Generate KYC DSL", AiResponseType::DslGeneration);
        assert_eq!(request.instruction, "Generate KYC DSL");
        assert_eq!(request.response_type, AiResponseType::DslGeneration);
        assert!(request.context.is_none());

        let mut context = HashMap::new();
        context.insert("domain".to_string(), "kyc".to_string());

        let enhanced_request = request
            .with_context(context)
            .with_temperature(0.7)
            .with_max_tokens(2048);
        assert!(enhanced_request.context.is_some());
        assert_eq!(enhanced_request.temperature, Some(0.7));
        assert_eq!(enhanced_request.max_tokens, Some(2048));
    }

    #[test]
    fn test_ai_response_type() {
        assert_eq!(AiResponseType::DslGeneration.as_str(), "dsl_generation");
        assert_eq!(AiResponseType::DslValidation.to_string(), "dsl_validation");

        let all_types = AiResponseType::all();
        assert_eq!(all_types.len(), 5);
        assert!(all_types.contains(&AiResponseType::DslGeneration));
        assert!(all_types.contains(&AiResponseType::DslSuggestions));
    }

    #[test]
    fn test_ai_dsl_response() {
        let response = AiDslResponse::new("(kyc.start)", "Generated KYC start DSL");
        assert_eq!(response.generated_dsl, "(kyc.start)");
        assert_eq!(response.explanation, "Generated KYC start DSL");
        assert!(response.confidence.is_none());

        let enhanced_response = response
            .with_confidence(0.95)
            .with_changes(vec!["Added KYC namespace".to_string()])
            .with_warnings(vec!["Consider adding customer ID".to_string()])
            .with_suggestions(vec!["Add validation step".to_string()]);

        assert_eq!(enhanced_response.confidence, Some(0.95));
        assert!(enhanced_response.has_warnings());
        assert!(enhanced_response.has_suggestions());
        assert_eq!(enhanced_response.confidence_percentage(), 95);
        assert_eq!(enhanced_response.changes.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_ai_error_creation() {
        let http_error = AiError::http("Connection failed");
        assert!(matches!(http_error, AiError::HttpError(_)));

        let json_error = AiError::json("Invalid JSON response");
        assert!(matches!(json_error, AiError::JsonError(_)));

        let api_error = AiError::api("Rate limit exceeded");
        assert!(matches!(api_error, AiError::ApiError(_)));

        let config_error = AiError::configuration("Missing API key");
        assert!(matches!(config_error, AiError::ConfigurationError(_)));

        // Test error display
        let error_msg = format!("{}", http_error);
        assert!(error_msg.contains("HTTP request failed"));
    }

    #[test]
    fn test_ai_result_type() {
        let success: AiResult<String> = Ok("Generated DSL".to_string());
        assert!(success.is_ok());

        let failure: AiResult<String> = Err(AiError::AuthenticationError);
        assert!(failure.is_err());

        if let Err(error) = failure {
            assert!(matches!(error, AiError::AuthenticationError));
        }
    }

    #[test]
    fn test_serialization() {
        let loc = SourceLocation::new(1, 1, 0, 5);
        let json = serde_json::to_string(&loc).unwrap();
        let deserialized: SourceLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, deserialized);

        // Test validation types serialization
        let report = ValidationReport::valid();
        let report_json = serde_json::to_string(&report).unwrap();
        let deserialized_report: ValidationReport = serde_json::from_str(&report_json).unwrap();
        assert_eq!(report.is_valid, deserialized_report.is_valid);

        let validation = VocabularyValidation::compliant();
        let validation_json = serde_json::to_string(&validation).unwrap();
        let deserialized_validation: VocabularyValidation =
            serde_json::from_str(&validation_json).unwrap();
        assert_eq!(
            validation.all_verbs_approved,
            deserialized_validation.all_verbs_approved
        );

        // Test AI types serialization
        let ai_request = AiDslRequest::new("test", AiResponseType::DslGeneration);
        let request_json = serde_json::to_string(&ai_request).unwrap();
        let deserialized_request: AiDslRequest = serde_json::from_str(&request_json).unwrap();
        assert_eq!(ai_request.instruction, deserialized_request.instruction);

        let ai_response = AiDslResponse::new("(test)", "Generated test DSL");
        let response_json = serde_json::to_string(&ai_response).unwrap();
        let deserialized_response: AiDslResponse = serde_json::from_str(&response_json).unwrap();
        assert_eq!(
            ai_response.generated_dsl,
            deserialized_response.generated_dsl
        );
    }
}
