//! DSL Validation Service
//!
//! Unified validation service for DSL source code. Used by:
//! - IDE (LSP) for real-time diagnostics
//! - Agent pipeline for pre-execution validation
//! - Executor for final pre-flight check
//!
//! ## Interface - Batch Validation (Rust-style output)
//!
//! ```ignore
//! let result = validator.validate(ValidationRequest {
//!     source: r#"
//!         (cbu.create :name "Test" :jurisdiction "XX" :as @cbu)
//!         (document.catalog :document-type "INVALID_DOC" :cbu-id @cbu)
//!     "#.to_string(),
//!     context: ValidationContext::for_intent(Intent::OnboardIndividual),
//! }).await;
//!
//! // Either passes cleanly...
//! let validated = result?;
//!
//! // ...or fails with Rust-style error output:
//! //
//! // error[E032]: unknown jurisdiction 'XX'
//! //  --> input:1:42
//! //   |
//! // 1 | (cbu.create :name "Test" :jurisdiction "XX" :as @cbu)
//! //   |                                         ^^^^ not found in master_jurisdictions
//! //   |
//! //   = help: did you mean 'UK'?
//! //   = help: did you mean 'US'?
//! //
//! // error[E030]: unknown document type 'INVALID_DOC'
//! //  --> input:2:35
//! //   |
//! // 2 | (document.catalog :document-type "INVALID_DOC" :cbu-id @cbu)
//! //   |                                  ^^^^^^^^^^^^^ not found in document_types
//! //   |
//! //   = help: did you mean 'PASSPORT_GBR'?
//! //
//! // error: aborting due to 2 previous errors
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// REQUEST / RESPONSE INTERFACE
// =============================================================================

/// Request to validate DSL source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    /// The DSL source code to validate
    pub source: String,

    /// Context hints to improve validation and suggestions
    pub context: ValidationContext,
}

/// Context hints for validation
///
/// These help the validator:
/// 1. Scope suggestions to relevant items (e.g., documents for this CBU)
/// 2. Validate state transitions (e.g., can't add entity before CBU exists)
/// 3. Provide better error messages
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationContext {
    /// The high-level intent (helps scope valid verbs)
    pub intent: Option<Intent>,

    /// Existing CBU ID (for add_document, add_entity intents)
    pub cbu_id: Option<Uuid>,

    /// Existing entity IDs in scope
    pub entity_ids: Vec<Uuid>,

    /// Existing document IDs in scope
    pub document_ids: Vec<Uuid>,

    /// Jurisdiction filter (for scoping document type suggestions)
    pub jurisdiction: Option<String>,

    /// Client type filter (individual vs corporate)
    pub client_type: Option<ClientType>,

    /// Additional key-value hints
    pub hints: HashMap<String, String>,
}

impl ValidationContext {
    /// Create context for a specific intent
    pub fn for_intent(intent: Intent) -> Self {
        Self {
            intent: Some(intent),
            ..Default::default()
        }
    }

    /// Create context for operations on an existing CBU
    pub fn for_cbu(cbu_id: Uuid) -> Self {
        Self {
            cbu_id: Some(cbu_id),
            ..Default::default()
        }
    }

    /// Create context for operations on existing CBU with intent
    pub fn for_cbu_with_intent(cbu_id: Uuid, intent: Intent) -> Self {
        Self {
            intent: Some(intent),
            cbu_id: Some(cbu_id),
            ..Default::default()
        }
    }

    /// Add jurisdiction filter
    pub fn with_jurisdiction(mut self, jurisdiction: impl Into<String>) -> Self {
        self.jurisdiction = Some(jurisdiction.into());
        self
    }

    /// Add client type filter
    pub fn with_client_type(mut self, client_type: ClientType) -> Self {
        self.client_type = Some(client_type);
        self
    }
}

/// High-level intent classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// Create new CBU with optional documents/entities
    OnboardIndividual,
    OnboardCorporate,

    /// Modify existing CBU
    AddDocument,
    AddEntity,
    LinkEntityRole,

    /// Document operations
    ExtractDocument,
    LinkDocumentToEntity,

    /// Query operations
    GetCbuStatus,
    ListDocuments,
    ListEntities,

    /// KYC/Screening
    RunScreening,
    RunKycCheck,
}

/// Client type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Individual,
    Corporate,
    Fund,
    Trust,
}

/// Validation result
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Validation passed - includes the validated AST
    Ok(ValidatedProgram),

    /// Validation failed - includes diagnostics with suggestions
    Err(Vec<Diagnostic>),
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, ValidationResult::Ok(_))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, ValidationResult::Err(_))
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            ValidationResult::Ok(_) => &[],
            ValidationResult::Err(d) => d,
        }
    }

    pub fn into_result(self) -> Result<ValidatedProgram, Vec<Diagnostic>> {
        match self {
            ValidationResult::Ok(p) => Ok(p),
            ValidationResult::Err(d) => Err(d),
        }
    }
}

/// A validated program (AST that passed all checks)
#[derive(Debug, Clone)]
pub struct ValidatedProgram {
    /// The original source
    pub source: String,

    /// Parsed and validated verb calls
    pub statements: Vec<ValidatedStatement>,

    /// Resolved symbol bindings (name -> type info)
    pub bindings: HashMap<String, BindingInfo>,

    /// Resolved AST with LookupRef.primary_key populated
    /// This is the canonical representation for serialization/persistence
    pub resolved_ast: crate::dsl_v2::ast::Program,
}

/// A validated statement
#[derive(Debug, Clone)]
pub struct ValidatedStatement {
    /// The verb (domain.action)
    pub verb: String,

    /// Validated arguments with resolved refs
    pub args: HashMap<String, ResolvedArg>,

    /// Optional binding (:as @name)
    pub binding: Option<String>,

    /// Source location
    pub span: SourceSpan,
}

/// A resolved argument value
#[derive(Debug, Clone)]
pub enum ResolvedArg {
    /// String literal
    String(String),

    /// Number
    Number(f64),

    /// Boolean
    Boolean(bool),

    /// Symbol reference (@name) - includes resolved type
    Symbol {
        name: String,
        resolved_type: Option<RefType>,
    },

    /// Resolved reference to DB entity
    Ref {
        ref_type: RefType,
        id: Uuid,
        display: String,
    },

    /// List of values
    List(Vec<ResolvedArg>),

    /// Map of values
    Map(HashMap<String, ResolvedArg>),
}

/// Reference types that map to DB tables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefType {
    Cbu,
    Entity,
    Document,
    DocumentType,
    AttributeId,
    Jurisdiction,
    Role,
    EntityType,
    ScreeningType,
    Product,
    Service,
    Currency,
    ClientType,
}

/// Info about a symbol binding
#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub name: String,
    pub ref_type: RefType,
    pub defined_at: SourceSpan,
}

// =============================================================================
// DIAGNOSTICS
// =============================================================================

/// A diagnostic (error, warning, or hint)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level
    pub severity: Severity,

    /// Location in source
    pub span: SourceSpan,

    /// Error/warning code
    pub code: DiagnosticCode,

    /// Human-readable message
    pub message: String,

    /// Suggested fixes
    pub suggestions: Vec<Suggestion>,
}

/// Diagnostic severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

/// Location in source code
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SourceSpan {
    /// 1-based line number
    pub line: u32,

    /// 0-based column (character offset in line)
    pub column: u32,

    /// Byte offset from start of source
    pub offset: u32,

    /// Length in bytes
    pub length: u32,
}

impl SourceSpan {
    pub fn new(line: u32, column: u32, offset: u32, length: u32) -> Self {
        Self {
            line,
            column,
            offset,
            length,
        }
    }

    pub fn at(line: u32, column: u32) -> Self {
        Self {
            line,
            column,
            offset: 0,
            length: 0,
        }
    }
}

/// Diagnostic error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    // Syntax errors
    SyntaxError,
    UnexpectedToken,
    UnclosedParen,
    UnclosedString,

    // Verb errors
    UnknownVerb,
    VerbNotAllowedForIntent,

    // Argument errors
    MissingRequiredArg,
    UnknownArg,
    TypeMismatch,
    InvalidValue,

    // Reference errors (DB lookups)
    UnknownDocumentType,
    UnknownAttributeId,
    UnknownJurisdiction,
    UnknownRole,
    UnknownEntityType,

    // State errors
    EntityNotFound,
    DocumentNotFound,
    CbuNotFound,
    InvalidEntityState,
    InvalidDocumentState,

    // Binding errors
    UndefinedSymbol,
    DuplicateBinding,
    /// Unresolved symbol - referenced but not yet defined (incomplete, not error)
    UnresolvedSymbol,

    // Dataflow errors
    /// Reference to undefined binding in dataflow validation
    DataflowUndefinedBinding,
    /// Binding type doesn't match expected consumer type
    DataflowTypeMismatch,
    /// Duplicate binding name in same program
    DataflowDuplicateBinding,

    // CSG Context Errors (C0xx series)
    /// Document type not applicable to entity type
    DocumentNotApplicableToEntityType,
    /// Document type not applicable to jurisdiction
    DocumentNotApplicableToJurisdiction,
    /// Document type not applicable to client type
    DocumentNotApplicableToClientType,
    /// Attribute not applicable to entity type
    AttributeNotApplicableToEntityType,
    /// Missing prerequisite operation
    MissingPrerequisiteOperation,
    /// Symbol type mismatch
    SymbolTypeMismatch,
    /// Internal error
    InternalError,

    // Warnings
    DeprecatedVerb,
    UnusedBinding,
    /// Fuzzy match warning - similar entity exists
    FuzzyMatchWarning,
    /// Hardcoded UUID usage warning
    HardcodedUuid,
}

impl DiagnosticCode {
    pub fn default_severity(&self) -> Severity {
        match self {
            DiagnosticCode::DeprecatedVerb
            | DiagnosticCode::UnusedBinding
            | DiagnosticCode::FuzzyMatchWarning
            | DiagnosticCode::HardcodedUuid => Severity::Warning,
            DiagnosticCode::InternalError => Severity::Error,
            // UnresolvedSymbol is an Error - blocks execution, but UI can show it differently
            // since it's "incomplete" rather than "invalid" (user just needs to add the definition)
            _ => Severity::Error,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticCode::SyntaxError => "E001",
            DiagnosticCode::UnexpectedToken => "E002",
            DiagnosticCode::UnclosedParen => "E003",
            DiagnosticCode::UnclosedString => "E004",
            DiagnosticCode::UnknownVerb => "E010",
            DiagnosticCode::VerbNotAllowedForIntent => "E011",
            DiagnosticCode::MissingRequiredArg => "E020",
            DiagnosticCode::UnknownArg => "E021",
            DiagnosticCode::TypeMismatch => "E022",
            DiagnosticCode::InvalidValue => "E023",
            DiagnosticCode::UnknownDocumentType => "E030",
            DiagnosticCode::UnknownAttributeId => "E031",
            DiagnosticCode::UnknownJurisdiction => "E032",
            DiagnosticCode::UnknownRole => "E033",
            DiagnosticCode::UnknownEntityType => "E034",
            DiagnosticCode::EntityNotFound => "E040",
            DiagnosticCode::DocumentNotFound => "E041",
            DiagnosticCode::CbuNotFound => "E042",
            DiagnosticCode::InvalidEntityState => "E043",
            DiagnosticCode::InvalidDocumentState => "E044",
            DiagnosticCode::UndefinedSymbol => "E050",
            DiagnosticCode::DuplicateBinding => "E051",
            DiagnosticCode::UnresolvedSymbol => "I001", // I for Incomplete
            // CSG Context Errors
            DiagnosticCode::DocumentNotApplicableToEntityType => "C001",
            DiagnosticCode::DocumentNotApplicableToJurisdiction => "C002",
            DiagnosticCode::DocumentNotApplicableToClientType => "C003",
            DiagnosticCode::AttributeNotApplicableToEntityType => "C004",
            DiagnosticCode::MissingPrerequisiteOperation => "C005",
            DiagnosticCode::SymbolTypeMismatch => "C006",
            DiagnosticCode::InternalError => "C099",
            // Dataflow errors
            DiagnosticCode::DataflowUndefinedBinding => "D001",
            DiagnosticCode::DataflowTypeMismatch => "D002",
            DiagnosticCode::DataflowDuplicateBinding => "D003",
            // Warnings
            DiagnosticCode::DeprecatedVerb => "W001",
            DiagnosticCode::UnusedBinding => "W002",
            DiagnosticCode::FuzzyMatchWarning => "W003",
            DiagnosticCode::HardcodedUuid => "W004",
        }
    }
}

/// A suggested fix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// Human-readable description
    pub message: String,

    /// The replacement text
    pub replacement: String,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Optional: the span to replace (defaults to diagnostic span)
    pub replace_span: Option<SourceSpan>,
}

impl Suggestion {
    pub fn new(
        message: impl Into<String>,
        replacement: impl Into<String>,
        confidence: f32,
    ) -> Self {
        Self {
            message: message.into(),
            replacement: replacement.into(),
            confidence,
            replace_span: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.replace_span = Some(span);
        self
    }
}

// =============================================================================
// VALIDATOR TRAIT
// =============================================================================

/// The validation service interface
#[cfg_attr(not(feature = "database"), allow(async_fn_in_trait))]
pub trait DslValidator: Send + Sync {
    /// Validate DSL source with context
    fn validate(
        &self,
        request: &ValidationRequest,
    ) -> impl std::future::Future<Output = ValidationResult> + Send;

    /// Quick syntax-only check (no DB lookups)
    fn check_syntax(&self, source: &str) -> Vec<Diagnostic>;
}

// =============================================================================
// BUILDER FOR DIAGNOSTICS
// =============================================================================

/// Builder for creating diagnostics
pub struct DiagnosticBuilder {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBuilder {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub fn error(
        &mut self,
        code: DiagnosticCode,
        span: SourceSpan,
        message: impl Into<String>,
    ) -> &mut DiagnosticEntry {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            span,
            code,
            message: message.into(),
            suggestions: Vec::new(),
        });
        // Safety: we just pushed, so last() is Some
        unsafe {
            &mut *(self.diagnostics.last_mut().unwrap() as *mut Diagnostic as *mut DiagnosticEntry)
        }
    }

    pub fn warning(
        &mut self,
        code: DiagnosticCode,
        span: SourceSpan,
        message: impl Into<String>,
    ) -> &mut DiagnosticEntry {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            span,
            code,
            message: message.into(),
            suggestions: Vec::new(),
        });
        unsafe {
            &mut *(self.diagnostics.last_mut().unwrap() as *mut Diagnostic as *mut DiagnosticEntry)
        }
    }

    pub fn build(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

impl Default for DiagnosticBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry point for adding suggestions to a diagnostic
#[repr(transparent)]
pub struct DiagnosticEntry(Diagnostic);

impl DiagnosticEntry {
    pub fn suggest(
        &mut self,
        message: impl Into<String>,
        replacement: impl Into<String>,
        confidence: f32,
    ) -> &mut Self {
        self.0
            .suggestions
            .push(Suggestion::new(message, replacement, confidence));
        self
    }

    pub fn suggest_one_of(
        &mut self,
        prefix: &str,
        options: &[&str],
        confidences: &[f32],
    ) -> &mut Self {
        for (i, opt) in options.iter().enumerate() {
            let conf = confidences.get(i).copied().unwrap_or(0.5);
            self.0.suggestions.push(Suggestion::new(
                format!("{} '{}'", prefix, opt),
                (*opt).to_string(),
                conf,
            ));
        }
        self
    }
}

// =============================================================================
// RUST-STYLE ERROR FORMATTER
// =============================================================================

/// Format diagnostics like Rust compiler output
pub struct RustStyleFormatter;

impl RustStyleFormatter {
    /// Format a batch of diagnostics with source context
    pub fn format(source: &str, diagnostics: &[Diagnostic]) -> String {
        if diagnostics.is_empty() {
            return String::new();
        }

        let lines: Vec<&str> = source.lines().collect();
        let mut output = String::new();

        for diag in diagnostics {
            output.push_str(&Self::format_one(diag, &lines));
            output.push('\n');
        }

        // Summary line
        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        let warning_count = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count();

        if error_count > 0 {
            output.push_str(&format!(
                "error: aborting due to {} previous error{}",
                error_count,
                if error_count == 1 { "" } else { "s" }
            ));
            if warning_count > 0 {
                output.push_str(&format!(
                    "; {} warning{} emitted",
                    warning_count,
                    if warning_count == 1 { "" } else { "s" }
                ));
            }
            output.push('\n');
        } else if warning_count > 0 {
            output.push_str(&format!(
                "warning: {} warning{} emitted\n",
                warning_count,
                if warning_count == 1 { "" } else { "s" }
            ));
        }

        output
    }

    fn format_one(diag: &Diagnostic, lines: &[&str]) -> String {
        let mut out = String::new();

        // Header: error[E030]: message
        let severity_str = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Hint => "hint",
        };
        out.push_str(&format!(
            "{}[{}]: {}\n",
            severity_str,
            diag.code.as_str(),
            diag.message
        ));

        // Location: --> input:1:25
        out.push_str(&format!(
            " --> input:{}:{}\n",
            diag.span.line, diag.span.column
        ));

        // Source line with pointer
        let line_idx = diag.span.line.saturating_sub(1) as usize;
        if line_idx < lines.len() {
            let line_num_width = diag.span.line.to_string().len();
            let source_line = lines[line_idx];

            // Empty line with pipe
            out.push_str(&format!("{:width$} |\n", "", width = line_num_width));

            // Source line
            out.push_str(&format!("{} | {}\n", diag.span.line, source_line));

            // Pointer line with carets
            let pointer_offset = diag.span.column as usize;
            let pointer_len = if diag.span.length > 0 {
                diag.span.length as usize
            } else {
                1
            };
            out.push_str(&format!(
                "{:width$} | {:>offset$}{}\n",
                "",
                "",
                "^".repeat(pointer_len),
                width = line_num_width,
                offset = pointer_offset
            ));
        }

        // Suggestions as help lines
        for suggestion in &diag.suggestions {
            out.push_str(&format!(
                "   = help: {} '{}'\n",
                suggestion.message, suggestion.replacement
            ));
        }

        out
    }

    /// Format as a compact single-line error (for logs)
    pub fn format_compact(diagnostics: &[Diagnostic]) -> String {
        diagnostics
            .iter()
            .map(|d| {
                format!(
                    "{}:{}:{}: {} [{}]",
                    d.span.line,
                    d.span.column,
                    match d.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Hint => "hint",
                    },
                    d.message,
                    d.code.as_str()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Convenience trait for ValidationResult to get formatted output
impl ValidationResult {
    /// Format errors in Rust compiler style
    pub fn format_errors(&self, source: &str) -> Option<String> {
        match self {
            ValidationResult::Ok(_) => None,
            ValidationResult::Err(diagnostics) => {
                Some(RustStyleFormatter::format(source, diagnostics))
            }
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_builder() {
        let mut builder = DiagnosticBuilder::new();

        builder
            .error(
                DiagnosticCode::UnknownDocumentType,
                SourceSpan::at(1, 25),
                "Unknown document type: 'PASSPORT_XYZ'",
            )
            .suggest("Did you mean", "PASSPORT_GBR", 0.9)
            .suggest("Did you mean", "PASSPORT_USA", 0.7);

        let diagnostics = builder.build();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::UnknownDocumentType);
        assert_eq!(diagnostics[0].suggestions.len(), 2);
        assert_eq!(diagnostics[0].suggestions[0].replacement, "PASSPORT_GBR");
    }

    #[test]
    fn test_validation_context_default() {
        let ctx = ValidationContext::default();
        assert!(ctx.intent.is_none());
        assert!(ctx.cbu_id.is_none());
        assert!(ctx.entity_ids.is_empty());
    }

    #[test]
    fn test_diagnostic_code_severity() {
        assert_eq!(
            DiagnosticCode::UnknownVerb.default_severity(),
            Severity::Error
        );
        assert_eq!(
            DiagnosticCode::DeprecatedVerb.default_severity(),
            Severity::Warning
        );
    }

    #[test]
    fn test_validation_result() {
        let err_result = ValidationResult::Err(vec![Diagnostic {
            severity: Severity::Error,
            span: SourceSpan::at(1, 0),
            code: DiagnosticCode::SyntaxError,
            message: "test".to_string(),
            suggestions: vec![],
        }]);

        assert!(err_result.is_err());
        assert_eq!(err_result.diagnostics().len(), 1);
    }

    #[test]
    fn test_rust_style_formatter() {
        let source = r#"(cbu.create :name "Test" :jurisdiction "XX" :as @cbu)
(document.catalog :document-type "INVALID_DOC" :cbu-id @cbu)"#;

        let diagnostics = vec![
            Diagnostic {
                severity: Severity::Error,
                span: SourceSpan::new(1, 40, 40, 4),
                code: DiagnosticCode::UnknownJurisdiction,
                message: "unknown jurisdiction 'XX'".to_string(),
                suggestions: vec![
                    Suggestion::new("did you mean", "UK", 0.9),
                    Suggestion::new("did you mean", "US", 0.8),
                ],
            },
            Diagnostic {
                severity: Severity::Error,
                span: SourceSpan::new(2, 33, 86, 13),
                code: DiagnosticCode::UnknownDocumentType,
                message: "unknown document type 'INVALID_DOC'".to_string(),
                suggestions: vec![Suggestion::new("did you mean", "PASSPORT_GBR", 0.85)],
            },
        ];

        let output = RustStyleFormatter::format(source, &diagnostics);
        println!("{}", output);

        // Verify structure
        assert!(output.contains("error[E032]"));
        assert!(output.contains("error[E030]"));
        assert!(output.contains("unknown jurisdiction"));
        assert!(output.contains("= help: did you mean 'UK'"));
        assert!(output.contains("aborting due to 2 previous errors"));
    }

    #[test]
    fn test_compact_formatter() {
        let diagnostics = vec![Diagnostic {
            severity: Severity::Error,
            span: SourceSpan::at(1, 40),
            code: DiagnosticCode::UnknownJurisdiction,
            message: "unknown jurisdiction 'XX'".to_string(),
            suggestions: vec![],
        }];

        let output = RustStyleFormatter::format_compact(&diagnostics);
        assert_eq!(output, "1:40:error: unknown jurisdiction 'XX' [E032]");
    }
}
