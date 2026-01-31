//! Permission and access control policy for ESPER navigation.
//!
//! This crate provides:
//!
//! - **UserPolicy**: Permissions for what a user can see and do
//! - **EntityPolicy**: Which entities are visible, hidden, or masked
//! - **VerbPolicy**: Which navigation verbs are allowed
//! - **PolicyGuard**: Enforcement layer that wraps navigation
//!
//! # Architecture
//!
//! ```text
//! PolicySource (YAML/DB) ──► PolicyConfig ──► PolicyGuard
//!                                                  │
//!                            ┌─────────────────────┼─────────────────────┐
//!                            │                     │                     │
//!                            ▼                     ▼                     ▼
//!                      VerbFilter          EntityFilter          FieldMask
//!                  (allowed actions)    (visible entities)    (hidden fields)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use esper_policy::{PolicyGuard, UserPolicy, Permission};
//!
//! let policy = UserPolicy::new("user-123")
//!     .grant(Permission::VIEW_ENTITIES)
//!     .grant(Permission::NAVIGATE)
//!     .deny_verb(Verb::DiveInto);
//!
//! let guard = PolicyGuard::new(policy);
//! if guard.can_execute(&Verb::Ascend) {
//!     // Execute the verb
//! }
//! ```

mod entity;
mod error;
mod fingerprint;
mod guard;
mod permission;
mod user;
mod verb;

pub use entity::{EntityPolicy, EntityVisibility, FieldMask};
pub use error::PolicyError;
pub use fingerprint::PolicyFingerprint;
pub use guard::PolicyGuard;
pub use permission::Permission;
pub use user::UserPolicy;
pub use verb::{VerbPolicy, VerbRule};

/// Default mask for sensitive fields.
pub const DEFAULT_SENSITIVE_FIELDS: &[&str] = &[
    "ssn",
    "tax_id",
    "account_number",
    "routing_number",
    "password",
    "api_key",
    "secret",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sensitive_fields_not_empty() {
        assert!(!DEFAULT_SENSITIVE_FIELDS.is_empty());
    }
}
