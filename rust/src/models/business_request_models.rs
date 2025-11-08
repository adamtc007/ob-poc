//! Business Request Models for DSL Lifecycle Management
//!
//! This module defines the core data models for managing DSL business requests
//! and their lifecycle. Each business request (KYC.Case, Onboarding.request, etc.)
//! represents a complete business context that persists through all DSL edits and amendments.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

// ============================================================================
// CORE BUSINESS REQUEST MODELS
// ============================================================================

/// DSL Business Request - Primary business context for DSL instances
/// Examples: KYC.Case.123, Onboarding.Request.456, Account.Opening.789
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslBusinessRequest {
    pub request_id: Uuid,
    pub domain_id: Uuid,

    // Business context identifiers
    pub business_reference: String, // External reference (case number, account ID, etc.)
    pub request_type: String,       // 'KYC_CASE', 'ONBOARDING_REQUEST', 'ACCOUNT_OPENING', etc.
    pub client_id: Option<String>,  // Client or account identifier

    // Request lifecycle status
    pub request_status: RequestStatus,
    pub priority_level: PriorityLevel,

    // Business metadata
    pub request_title: Option<String>,
    pub request_description: Option<String>,
    pub business_context: Option<Value>, // Additional business context (customer data, case details, etc.)

    // Lifecycle tracking
    pub created_by: String,           // User who created the request
    pub assigned_to: Option<String>,  // Current assignee
    pub reviewed_by: Option<String>,  // User who reviewed/approved
    pub completed_by: Option<String>, // User who completed the request

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub assigned_at: Option<DateTime<Utc>>,
    pub review_started_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>, // Optional deadline

    // Audit and compliance
    pub external_audit_id: Option<String>, // External audit/compliance tracking ID
    pub regulatory_requirements: Option<Value>, // Specific regulatory requirements for this request
}

/// New Business Request for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDslBusinessRequest {
    pub domain_name: String, // Will be resolved to domain_id
    pub business_reference: String,
    pub request_type: String,
    pub client_id: Option<String>,
    pub request_title: Option<String>,
    pub request_description: Option<String>,
    pub business_context: Option<Value>,
    pub created_by: String,
    pub priority_level: Option<PriorityLevel>,
    pub due_date: Option<DateTime<Utc>>,
    pub regulatory_requirements: Option<Value>,
}

/// Business Request Update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDslBusinessRequest {
    pub request_status: Option<RequestStatus>,
    pub priority_level: Option<PriorityLevel>,
    pub request_title: Option<String>,
    pub request_description: Option<String>,
    pub business_context: Option<Value>,
    pub assigned_to: Option<String>,
    pub reviewed_by: Option<String>,
    pub completed_by: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub regulatory_requirements: Option<Value>,
}

// ============================================================================
// REQUEST WORKFLOW MODELS
// ============================================================================

/// DSL Request Workflow State - Tracks workflow progression for each business request
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslRequestWorkflowState {
    pub state_id: Uuid,
    pub request_id: Uuid,

    // Workflow state information
    pub workflow_state: String, // 'initial_draft', 'collecting_data', 'review_required', 'approved', etc.
    pub state_description: Option<String>,

    // State transition tracking
    pub previous_state: Option<String>, // What state we came from
    pub next_possible_states: Option<Vec<String>>, // What states we can transition to

    // State metadata
    pub state_data: Option<Value>, // State-specific data and context
    pub automation_trigger: bool,  // Whether this state was entered automatically
    pub requires_approval: bool,   // Whether this state requires manual approval

    // State timing
    pub entered_at: DateTime<Utc>,
    pub entered_by: String,
    pub estimated_duration_hours: Option<i32>, // How long this state typically takes

    // Current state tracking
    pub is_current_state: bool,
    pub exited_at: Option<DateTime<Utc>>,
    pub exited_by: Option<String>,
}

/// New Workflow State for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDslRequestWorkflowState {
    pub request_id: Uuid,
    pub workflow_state: String,
    pub state_description: Option<String>,
    pub previous_state: Option<String>,
    pub next_possible_states: Option<Vec<String>>,
    pub state_data: Option<Value>,
    pub automation_trigger: Option<bool>,
    pub requires_approval: Option<bool>,
    pub entered_by: String,
    pub estimated_duration_hours: Option<i32>,
}

// ============================================================================
// REQUEST TYPE REFERENCE MODELS
// ============================================================================

/// DSL Request Type - Reference data for standard request types
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DslRequestType {
    pub request_type: String,
    pub domain_name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub default_workflow_states: Option<Vec<String>>,
    pub estimated_duration_hours: Option<i32>,
    pub requires_approval: bool,
    pub active: bool,
}

// ============================================================================
// COMPOSITE VIEW MODELS
// ============================================================================

/// Active Business Request with latest DSL version info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveBusinessRequestView {
    // Business request info
    pub request_id: Uuid,
    pub business_reference: String,
    pub request_type: String,
    pub client_id: Option<String>,
    pub request_status: RequestStatus,
    pub priority_level: PriorityLevel,
    pub request_title: Option<String>,
    pub request_created_by: String,
    pub assigned_to: Option<String>,
    pub request_created_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,

    // Domain information
    pub domain_name: String,
    pub domain_description: Option<String>,

    // Latest DSL version for this request
    pub version_id: Option<Uuid>,
    pub version_number: Option<i32>,
    pub functional_state: Option<String>,
    pub compilation_status: Option<String>,
    pub version_created_by: Option<String>,
    pub version_created_at: Option<DateTime<Utc>>,

    // AST information
    pub has_compiled_ast: bool,
    pub parsed_at: Option<DateTime<Utc>>,
    pub complexity_score: Option<f64>,

    // Current workflow state
    pub current_workflow_state: Option<String>,
    pub current_state_description: Option<String>,
    pub state_entered_at: Option<DateTime<Utc>>,
}

/// Business Request Summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessRequestSummary {
    pub request_id: Uuid,
    pub business_reference: String,
    pub request_type: String,
    pub domain_name: String,
    pub request_status: RequestStatus,
    pub current_workflow_state: Option<String>,
    pub total_versions: i32,
    pub latest_version_number: i32,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

/// Request Workflow History
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestWorkflowHistory {
    pub request_id: Uuid,
    pub business_reference: String,
    pub request_type: String,
    pub domain_name: String,

    pub state_id: Uuid,
    pub workflow_state: String,
    pub state_description: Option<String>,
    pub previous_state: Option<String>,
    pub entered_at: DateTime<Utc>,
    pub entered_by: String,
    pub exited_at: Option<DateTime<Utc>>,
    pub exited_by: Option<String>,
    pub is_current_state: bool,
    pub hours_in_state: f64,
}

// ============================================================================
// ENUMS
// ============================================================================

/// Request Status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "request_status", rename_all = "UPPERCASE")]
pub enum RequestStatus {
    Draft,
    InProgress,
    Review,
    Approved,
    Completed,
    Cancelled,
    Error,
}

impl std::fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestStatus::Draft => write!(f, "DRAFT"),
            RequestStatus::InProgress => write!(f, "IN_PROGRESS"),
            RequestStatus::Review => write!(f, "REVIEW"),
            RequestStatus::Approved => write!(f, "APPROVED"),
            RequestStatus::Completed => write!(f, "COMPLETED"),
            RequestStatus::Cancelled => write!(f, "CANCELLED"),
            RequestStatus::Error => write!(f, "ERROR"),
        }
    }
}

impl From<String> for RequestStatus {
    fn from(s: String) -> Self {
        match s.to_uppercase().as_str() {
            "DRAFT" => RequestStatus::Draft,
            "IN_PROGRESS" => RequestStatus::InProgress,
            "REVIEW" => RequestStatus::Review,
            "APPROVED" => RequestStatus::Approved,
            "COMPLETED" => RequestStatus::Completed,
            "CANCELLED" => RequestStatus::Cancelled,
            "ERROR" => RequestStatus::Error,
            _ => RequestStatus::Draft, // Default fallback
        }
    }
}

/// Priority Level enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, Default)]
#[sqlx(type_name = "priority_level", rename_all = "UPPERCASE")]
pub enum PriorityLevel {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

impl std::fmt::Display for PriorityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PriorityLevel::Low => write!(f, "LOW"),
            PriorityLevel::Normal => write!(f, "NORMAL"),
            PriorityLevel::High => write!(f, "HIGH"),
            PriorityLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

impl From<String> for PriorityLevel {
    fn from(s: String) -> Self {
        match s.to_uppercase().as_str() {
            "LOW" => PriorityLevel::Low,
            "NORMAL" => PriorityLevel::Normal,
            "HIGH" => PriorityLevel::High,
            "CRITICAL" => PriorityLevel::Critical,
            _ => PriorityLevel::Normal, // Default fallback
        }
    }
}

// Default derived above

// ============================================================================
// HELPER FUNCTIONS AND IMPLEMENTATIONS
// ============================================================================

impl DslBusinessRequest {
    /// Check if the request is in an active state
    pub fn is_active(&self) -> bool {
        !matches!(
            self.request_status,
            RequestStatus::Completed | RequestStatus::Cancelled
        )
    }

    /// Check if the request is overdue
    pub fn is_overdue(&self) -> bool {
        if let Some(due_date) = self.due_date {
            due_date < Utc::now() && self.is_active()
        } else {
            false
        }
    }

    /// Get the age of the request in days
    pub fn age_in_days(&self) -> i64 {
        (Utc::now() - self.created_at).num_days()
    }

    /// Check if the request requires urgent attention
    pub fn requires_urgent_attention(&self) -> bool {
        matches!(self.priority_level, PriorityLevel::Critical) || self.is_overdue()
    }
}

impl NewDslBusinessRequest {
    /// Create a new KYC case request
    pub fn new_kyc_case(business_reference: String, client_id: String, created_by: String) -> Self {
        Self {
            domain_name: "KYC".to_string(),
            business_reference,
            request_type: "KYC_CASE".to_string(),
            client_id: Some(client_id),
            request_title: Some("KYC Investigation Case".to_string()),
            request_description: Some("Know Your Customer compliance investigation".to_string()),
            business_context: None,
            created_by,
            priority_level: Some(PriorityLevel::Normal),
            due_date: None,
            regulatory_requirements: None,
        }
    }

    /// Create a new onboarding request
    pub fn new_onboarding_request(
        business_reference: String,
        client_id: String,
        created_by: String,
    ) -> Self {
        Self {
            domain_name: "Onboarding".to_string(),
            business_reference,
            request_type: "ONBOARDING_REQUEST".to_string(),
            client_id: Some(client_id),
            request_title: Some("Customer Onboarding Request".to_string()),
            request_description: Some("New customer onboarding process".to_string()),
            business_context: None,
            created_by,
            priority_level: Some(PriorityLevel::Normal),
            due_date: None,
            regulatory_requirements: None,
        }
    }

    /// Create a new account opening request
    pub fn new_account_opening(
        business_reference: String,
        client_id: String,
        created_by: String,
    ) -> Self {
        Self {
            domain_name: "Account_Opening".to_string(),
            business_reference,
            request_type: "ACCOUNT_OPENING".to_string(),
            client_id: Some(client_id),
            request_title: Some("Account Opening Application".to_string()),
            request_description: Some("New account setup and approval process".to_string()),
            business_context: None,
            created_by,
            priority_level: Some(PriorityLevel::Normal),
            due_date: None,
            regulatory_requirements: None,
        }
    }
}

impl DslRequestWorkflowState {
    /// Calculate the duration spent in this state
    pub fn duration_in_state(&self) -> chrono::Duration {
        if let Some(exited_at) = self.exited_at {
            exited_at - self.entered_at
        } else {
            Utc::now() - self.entered_at
        }
    }

    /// Get duration in hours as a float
    pub fn duration_in_hours(&self) -> f64 {
        self.duration_in_state().num_milliseconds() as f64 / (1000.0 * 60.0 * 60.0)
    }

    /// Check if this state is taking longer than estimated
    pub fn is_overdue(&self) -> bool {
        if let Some(estimated_hours) = self.estimated_duration_hours {
            self.duration_in_hours() > estimated_hours as f64
        } else {
            false
        }
    }
}

// ============================================================================
// CONSTANTS
// ============================================================================

/// Standard workflow states for different request types
pub mod workflow_states {
    pub const KYC_WORKFLOW: &[&str] = &[
        "initial_draft",
        "collecting_documents",
        "ubo_analysis",
        "compliance_review",
        "approved",
        "completed",
    ];

    pub const ONBOARDING_WORKFLOW: &[&str] = &[
        "initial_draft",
        "identity_verification",
        "document_collection",
        "risk_assessment",
        "approved",
        "completed",
    ];

    pub const ACCOUNT_OPENING_WORKFLOW: &[&str] = &[
        "initial_draft",
        "application_review",
        "document_verification",
        "approval_workflow",
        "account_setup",
        "completed",
    ];
}

/// Standard request types
pub mod request_types {
    pub const KYC_CASE: &str = "KYC_CASE";
    pub const ONBOARDING_REQUEST: &str = "ONBOARDING_REQUEST";
    pub const ACCOUNT_OPENING: &str = "ACCOUNT_OPENING";
    pub const COMPLIANCE_REVIEW: &str = "COMPLIANCE_REVIEW";
}
