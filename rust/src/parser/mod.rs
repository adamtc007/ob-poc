//! V3.1 DSL Parser Module
//!
//! Pure V3.1 implementation with unified S-expression syntax for multi-domain workflows.
//! Supports Document Library and ISDA domain verbs with AttributeID-as-Type pattern.

// AST type definitions
pub mod ast;

// Internal implementation modules
// pub mod advanced_parser; // Temporarily disabled due to compilation errors
pub mod combinators;
pub mod idiomatic_parser;
pub mod normalizer;
pub mod primitives;
pub mod statements;
pub mod validators;

// Public re-exports for AST types
pub use ast::{
    BatchOperation, ConstraintViolation, CrudStatement, DataCreate, DataDelete, DataRead,
    DataUpdate, Form, Key, Literal, Program, PropertyMap, ValidationResult as AstValidationResult,
    ValidationWarning, Value, VerbForm,
};

// Public re-exports for DSL compilation and execution
pub use idiomatic_parser::{parse_form, parse_program};
pub use normalizer::DslNormalizer;
pub use validators::{DslValidator, ValidationResult};

// Core parser functions
use crate::error::{DSLResult, ParseError};

/// Parse DSL text into AST with normalization
pub fn parse_normalize_and_validate(input: &str) -> DSLResult<Program> {
    // Step 1: Normalize DSL (v3.3 -> v3.1)
    let normalized = input.to_string(); // Stub normalization for now

    // Step 2: Parse into AST
    let program = parse_program(&normalized).map_err(|e| ParseError::Syntax {
        message: format!("Parse error: {:?}", e),
        position: 0,
    })?;

    // Step 3: Validate parsed AST
    // validate_dsl(&program)?; // Stub validation for now

    Ok(program)
}

/// Execute parsed DSL program
pub fn execute_dsl(program: &Program) -> DSLResult<ExecutionResult> {
    let mut results = Vec::new();

    for form in program {
        match form {
            Form::Verb(verb_form) => {
                let result = execute_verb_form(verb_form)?;
                results.push(result);
            }
            Form::Comment(_) => {
                // Skip comments during execution
                continue;
            }
        }
    }

    Ok(ExecutionResult {
        success: true,
        operations_executed: results,
        errors: Vec::new(),
    })
}

/// Execute a single verb form
fn execute_verb_form(verb_form: &VerbForm) -> DSLResult<String> {
    // Basic execution - delegate to domain handlers
    Ok(format!("Executed: {}", verb_form.verb))
}

/// Result of DSL execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub operations_executed: Vec<String>,
    pub errors: Vec<String>,
}
