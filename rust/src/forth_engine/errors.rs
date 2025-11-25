//! Error types for the DSL Forth Engine.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VmError {
    #[error(
        "Stack underflow: Expected at least {expected} items on the stack, but found {found}."
    )]
    StackUnderflow { expected: usize, found: usize },

    #[error("Type error: Expected a '{expected}' value, but found a '{found}' value.")]
    TypeError { expected: String, found: String },

    #[error("Missing attribute in environment: {0}")]
    MissingAttribute(String),

    #[error("Execution halted.")]
    Halt,
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Unknown word: '{0}'")]
    UnknownWord(String),

    #[error("Attribute '{0}' could not be resolved at compile time.")]
    UnresolvedAttribute(String),

    #[error("Document '{0}' could not be resolved at compile time.")]
    UnresolvedDocument(String),

    #[error("Stack underflow at compile time: word '{word}' requires {required} inputs, but only {available} available")]
    StackUnderflow {
        word: String,
        required: usize,
        available: usize,
    },

    #[error("Stack not empty at end of program: {remaining} items remaining")]
    StackNotEmpty { remaining: usize },
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Failed to parse DSL sheet: {0}")]
    Parse(String),

    #[error("Failed to compile DSL sheet: {0}")]
    Compile(#[from] CompileError),

    #[error("VM execution failed: {0}")]
    Vm(#[from] VmError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Unknown word: '{0}'")]
    UnknownWord(String),

    #[error("Missing required argument: '{0}'")]
    MissingArgument(String),
}
