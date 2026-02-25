use thiserror::Error;

#[derive(Debug, Error)]
pub enum SemOsError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("gate failed: {} violation(s)", .0.len())]
    GateFailed(Vec<GateViolation>),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("migration pending — {0}")]
    MigrationPending(String),

    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

impl SemOsError {
    pub fn http_status(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::GateFailed(_) => 422,
            Self::Unauthorized(_) => 403,
            Self::Conflict(_) => 409,
            Self::InvalidInput(_) => 400,
            Self::MigrationPending(_) => 503,
            Self::Internal(_) => 500,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GateViolation {
    pub gate_id: String,
    pub severity: GateSeverity,
    pub message: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateSeverity {
    Error,
    Warning,
}

impl std::fmt::Display for GateViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.gate_id, self.severity, self.message)
    }
}

impl std::fmt::Display for GateSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
        }
    }
}
