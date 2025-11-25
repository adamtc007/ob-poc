//! Core types for the onboarding DSL test harness

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Test input for onboarding DSL validation
#[derive(Debug, Clone)]
pub struct OnboardingTestInput {
    /// CBU ID to associate with the onboarding request
    pub cbu_id: Uuid,
    /// Product codes to link to the onboarding
    pub product_codes: Vec<String>,
    /// DSL source code to validate
    pub dsl_source: String,
}

/// Complete test result with verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingTestResult {
    // IDs
    /// The onboarding request ID created
    pub request_id: Uuid,
    /// DSL instance ID (if validation passed)
    pub dsl_instance_id: Option<Uuid>,
    /// DSL version number (if validation passed)
    pub dsl_version: Option<i32>,

    // Validation outcome
    /// Whether schema validation passed
    pub validation_passed: bool,
    /// Validation errors (if any)
    pub errors: Vec<ValidationErrorInfo>,

    // Performance metrics
    /// Time to parse DSL (milliseconds)
    pub parse_time_ms: u64,
    /// Time to validate against schema (milliseconds)
    pub validate_time_ms: u64,
    /// Time to persist to database (milliseconds)
    pub persist_time_ms: u64,
    /// Total end-to-end time (milliseconds)
    pub total_time_ms: u64,

    // Verification (proves DB writes worked)
    /// Database verification results
    pub verification: VerificationResult,
}

/// Verification that all database writes succeeded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    // Onboarding request verification
    /// Whether the onboarding request exists in DB
    pub request_exists: bool,
    /// Current state of the request
    pub request_state: String,
    /// Number of products linked to the request
    pub products_linked: usize,
    /// Expected number of products
    pub expected_products: usize,

    // DSL instance verification
    /// Whether the DSL instance exists in DB
    pub dsl_instance_exists: bool,
    /// Whether DSL content matches what was submitted
    pub dsl_content_matches: bool,
    /// Current DSL version number
    pub dsl_version: i32,

    // AST verification
    /// Whether AST JSON exists in DB
    pub ast_exists: bool,
    /// Whether AST has expressions array
    pub ast_has_expressions: bool,
    /// Whether AST has symbol table
    pub ast_has_symbol_table: bool,
    /// Number of symbols in symbol table
    pub symbol_count: usize,

    // Error verification (if validation failed)
    /// Whether validation errors were stored
    pub errors_stored: bool,
    /// Number of errors stored
    pub error_count: usize,

    // Overall
    /// Whether all verification checks passed
    pub all_checks_passed: bool,
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self {
            request_exists: false,
            request_state: String::new(),
            products_linked: 0,
            expected_products: 0,
            dsl_instance_exists: false,
            dsl_content_matches: false,
            dsl_version: 0,
            ast_exists: false,
            ast_has_expressions: false,
            ast_has_symbol_table: false,
            symbol_count: 0,
            errors_stored: false,
            error_count: 0,
            all_checks_passed: false,
        }
    }
}

/// Serializable validation error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrorInfo {
    /// Line number where error occurred
    pub line: u32,
    /// Column number where error occurred
    pub column: u32,
    /// Error code (e.g., "E003", "E007", "E010")
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Suggested fix (if available)
    pub suggestion: Option<String>,
}

/// Result of verifying specific symbols exist in stored AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolVerification {
    /// Expected symbol names
    pub expected: Vec<String>,
    /// Symbol names found in AST
    pub found: Vec<String>,
    /// Symbols that were expected but not found
    pub missing: Vec<String>,
    /// Whether all expected symbols are present
    pub all_present: bool,
}

/// Result of verifying validation errors match expected codes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorVerification {
    /// Expected error codes
    pub expected: Vec<String>,
    /// Error codes found in stored errors
    pub found: Vec<String>,
    /// Error codes that were expected but not found
    pub missing: Vec<String>,
    /// Whether all expected error codes are present
    pub all_present: bool,
}
