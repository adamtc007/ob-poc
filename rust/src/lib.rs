//! UBO Proof of Concept - A DSL for Ultimate Beneficial Ownership Analysis
//!
//! This crate provides a domain-specific language (DSL) for modeling and analyzing
//! ultimate beneficial ownership structures in financial institutions' KYC workflows.
//!
//! The DSL supports:
//! - Entity relationship modeling
//! - Document-based evidence tracking
//! - Multi-source data with conflict resolution
//! - UBO calculation algorithms
//! - Audit trail generation

pub mod ast;
pub mod data_dictionary;
pub mod graph;
pub mod parser;

pub use ast::*;
pub use parser::parse_program;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
