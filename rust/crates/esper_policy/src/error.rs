//! Policy error types.

use esper_core::Verb;
use thiserror::Error;

/// Errors that can occur during policy enforcement.
#[derive(Debug, Clone, Error)]
pub enum PolicyError {
    /// User lacks required permission.
    #[error("permission denied: missing {0:?}")]
    PermissionDenied(crate::Permission),

    /// Verb is not allowed by policy.
    #[error("verb denied: {0:?}")]
    VerbDenied(Verb),

    /// Entity is not visible.
    #[error("entity not visible: {0}")]
    EntityNotVisible(u64),

    /// Field is masked/redacted.
    #[error("field masked: {0}")]
    FieldMasked(String),

    /// Policy configuration invalid.
    #[error("invalid policy: {0}")]
    InvalidPolicy(String),

    /// Policy expired or revoked.
    #[error("policy expired or revoked")]
    PolicyExpired,

    /// User not found.
    #[error("user not found: {0}")]
    UserNotFound(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded")]
    RateLimitExceeded,
}

impl PolicyError {
    /// Check if this error can be recovered by requesting elevated permissions.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            PolicyError::PermissionDenied(_)
                | PolicyError::VerbDenied(_)
                | PolicyError::RateLimitExceeded
        )
    }

    /// Check if this error should be logged as a security event.
    pub fn is_security_event(&self) -> bool {
        matches!(
            self,
            PolicyError::PermissionDenied(_)
                | PolicyError::VerbDenied(_)
                | PolicyError::EntityNotVisible(_)
                | PolicyError::PolicyExpired
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Permission;

    #[test]
    fn error_display() {
        let err = PolicyError::PermissionDenied(Permission::EDIT_ENTITIES);
        assert!(err.to_string().contains("permission denied"));

        let err = PolicyError::VerbDenied(Verb::Ascend);
        assert!(err.to_string().contains("verb denied"));
    }

    #[test]
    fn error_recoverable() {
        assert!(PolicyError::RateLimitExceeded.is_recoverable());
        assert!(!PolicyError::UserNotFound("x".into()).is_recoverable());
    }

    #[test]
    fn error_security_event() {
        assert!(PolicyError::PolicyExpired.is_security_event());
        assert!(!PolicyError::RateLimitExceeded.is_security_event());
    }
}
