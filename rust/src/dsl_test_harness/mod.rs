//! Onboarding DSL Test Harness
//!
//! This module provides a comprehensive test harness for the onboarding DSL pipeline:
//! 1. Creates onboarding requests (CBU -> Products)
//! 2. Submits DSL source for validation against schema
//! 3. Saves validated AST to database using DslRepository
//! 4. Verifies all database operations by querying back
//!
//! See DESIGN_TEST_HARNESS.md for full specification.

mod harness;
mod types;
mod verification;

pub use harness::OnboardingTestHarness;
pub use types::{
    ErrorVerification, OnboardingTestInput, OnboardingTestResult, SymbolVerification,
    ValidationErrorInfo, VerificationResult,
};
pub use verification::DatabaseVerifier;
