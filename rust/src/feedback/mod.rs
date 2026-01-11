//! Feedback Inspector Module
//!
//! On-demand failure analysis system. Reads events captured by the event
//! infrastructure, classifies failures, generates repro tests, and creates
//! audit trails.
//!
//! ## Key Principle
//!
//! The inspector spins up when needed, not always running. Zero DSL pipeline impact.
//!
//! ## Components
//!
//! - `types`: Core enums and data structures
//! - `classifier`: Failure classification and fingerprinting
//! - `redactor`: Policy-based PII redaction
//! - `inspector`: Main analysis engine
//! - `store`: Database operations
//! - `audit`: Audit trail management
//! - `repro`: Repro test generation
//! - `todo`: TODO document generation

pub mod classifier;
pub mod inspector;
pub mod redactor;
pub mod repro;
pub mod todo;
pub mod types;

pub use classifier::FailureClassifier;
pub use inspector::FeedbackInspector;
pub use redactor::{RedactionMode, Redactor};
pub use repro::{ReproGenerator, ReproResult, ReproType};
pub use todo::{TodoGenerator, TodoResult};
pub use types::{
    ActorType, AnalysisReport, AuditAction, AuditEntry, AuditRecord, ErrorType, FailureRecord,
    IssueDetail, IssueFilter, IssueStatus, IssueSummary, OccurrenceRecord, RemediationPath,
    SessionContext, SessionEntry,
};
