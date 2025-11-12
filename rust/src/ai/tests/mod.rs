//! AI Testing Module
//!
//! This module contains comprehensive testing infrastructure for the AI-powered
//! DSL generation system, including end-to-end agent testing, canonical compliance
//! validation, and performance benchmarking.

pub mod end_to_end_agent_tests;

pub use end_to_end_agent_tests::{
    AgentTestResults, CanonicalComplianceResults, EndToEndAgentTester, PerformanceMetrics,
};
