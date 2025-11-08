//! Idiomatic Rust DSL parser module
//!
//! This module provides efficient, stateless parser functions for the UBO DSL using nom combinators.
//! It follows Rust best practices with strong typing and proper error handling.

pub mod idiomatic_parser;

use crate::ast::{Program, PropertyMap, Statement, Value, Workflow};
use crate::error::{DSLError, ParseError};

use nom::error::VerboseError;
use nom::Finish;

// Re-export the main parsing functions
pub use idiomatic_parser::{
    parse_identifier, parse_program as parse_program_internal,
    parse_properties as parse_properties_internal, parse_statement as parse_statement_internal,
    parse_string_literal, parse_value as parse_value_internal,
    parse_workflow as parse_workflow_internal,
};

/// Main parsing function with strong types
pub fn parse_program(input: &str) -> Result<Program, VerboseError<&str>> {
    idiomatic_parser::parse_program(input)
}

/// Parse a single workflow - wrapper with better error handling
pub fn parse_workflow_standalone(input: &str) -> Result<Workflow, DSLError> {
    match idiomatic_parser::parse_workflow(input).finish() {
        Ok((remaining, workflow)) => {
            if remaining.trim().is_empty() {
                Ok(workflow)
            } else {
                Err(DSLError::Parse(ParseError::Syntax {
                    position: remaining.as_ptr() as usize,
                    message: "Unexpected content after workflow".to_string(),
                }))
            }
        }
        Err(e) => Err(DSLError::Parse(ParseError::from(e))),
    }
}

/// Parse a property map - wrapper with better error handling
pub fn parse_properties_standalone(input: &str) -> Result<PropertyMap, DSLError> {
    match idiomatic_parser::parse_properties(input).finish() {
        Ok((remaining, properties)) => {
            if remaining.trim().is_empty() {
                Ok(properties)
            } else {
                Err(DSLError::Parse(ParseError::Syntax {
                    position: remaining.as_ptr() as usize,
                    message: "Unexpected content after properties".to_string(),
                }))
            }
        }
        Err(e) => Err(DSLError::Parse(ParseError::from(e))),
    }
}

/// Parse a single value - wrapper with better error handling
pub fn parse_value_standalone(input: &str) -> Result<Value, DSLError> {
    match idiomatic_parser::parse_value(input).finish() {
        Ok((remaining, value)) => {
            if remaining.trim().is_empty() {
                Ok(value)
            } else {
                Err(DSLError::Parse(ParseError::Syntax {
                    position: remaining.as_ptr() as usize,
                    message: "Unexpected content after value".to_string(),
                }))
            }
        }
        Err(e) => Err(DSLError::Parse(ParseError::from(e))),
    }
}

/// Validate parsed AST
pub fn validate_ast(ast: &Program) -> Result<(), Vec<DSLError>> {
    let mut errors = Vec::new();

    // Basic validation - can be extended
    for workflow in &ast.workflows {
        if workflow.id.is_empty() {
            errors.push(DSLError::Validation(
                crate::error::ValidationError::WorkflowError {
                    message: "Workflow ID cannot be empty".to_string(),
                },
            ));
        }

        // Validate statements
        for (i, statement) in workflow.statements.iter().enumerate() {
            if let Err(e) = validate_statement(statement) {
                errors.push(DSLError::Validation(
                    crate::error::ValidationError::WorkflowError {
                        message: format!(
                            "Statement {} in workflow '{}': {}",
                            i + 1,
                            workflow.id,
                            e
                        ),
                    },
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_statement(statement: &Statement) -> Result<(), String> {
    match statement {
        Statement::DeclareEntity {
            id, entity_type, ..
        } => {
            if id.is_empty() {
                return Err("Entity ID cannot be empty".to_string());
            }
            if entity_type.is_empty() {
                return Err("Entity type cannot be empty".to_string());
            }
        }
        Statement::ObtainDocument {
            document_type,
            source,
            ..
        } => {
            if document_type.is_empty() {
                return Err("Document type cannot be empty".to_string());
            }
            if source.is_empty() {
                return Err("Document source cannot be empty".to_string());
            }
        }
        Statement::CreateEdge {
            from,
            to,
            edge_type,
            ..
        } => {
            if from.is_empty() {
                return Err("Edge 'from' cannot be empty".to_string());
            }
            if to.is_empty() {
                return Err("Edge 'to' cannot be empty".to_string());
            }
            if edge_type.is_empty() {
                return Err("Edge type cannot be empty".to_string());
            }
        }
        Statement::CalculateUbo { entity_id, .. } => {
            if entity_id.is_empty() {
                return Err("UBO entity ID cannot be empty".to_string());
            }
        }
        Statement::Placeholder { command, .. } => {
            if command.is_empty() {
                return Err("Command cannot be empty".to_string());
            }
        }
        // Other statement types can be added here
        _ => {} // Legacy types - skip validation for now
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_handling() {
        let invalid_input = r#"
        (workflow "broken
            this is not valid syntax
        "#;

        let result = parse_program(invalid_input);
        assert!(result.is_err());
    }
}
