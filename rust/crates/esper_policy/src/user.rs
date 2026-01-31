//! User-level policy configuration.

use crate::entity::EntityPolicy;
use crate::permission::Permission;
use crate::verb::VerbPolicy;
use serde::{Deserialize, Serialize};

/// Complete policy for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPolicy {
    /// User identifier.
    pub user_id: String,
    /// User's permissions.
    pub permissions: Permission,
    /// Verb-level policy.
    pub verb_policy: VerbPolicy,
    /// Entity-level policy.
    pub entity_policy: EntityPolicy,
    /// Policy expiration timestamp (Unix seconds, 0 = never).
    pub expires_at: u64,
    /// Whether the policy is currently active.
    pub is_active: bool,
    /// Optional role name.
    pub role: Option<String>,
    /// Custom metadata.
    pub metadata: Option<serde_json::Value>,
}

impl UserPolicy {
    /// Create a new user policy with default permissions.
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            permissions: Permission::default(),
            verb_policy: VerbPolicy::allow_all(),
            entity_policy: EntityPolicy::allow_all(),
            expires_at: 0,
            is_active: true,
            role: None,
            metadata: None,
        }
    }

    /// Create a read-only policy.
    pub fn read_only(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            permissions: Permission::READ_ONLY,
            verb_policy: VerbPolicy::allow_all()
                .deny_category(crate::verb::VerbCategory::CrossChamber),
            entity_policy: EntityPolicy::allow_all(),
            expires_at: 0,
            is_active: true,
            role: Some("viewer".to_string()),
            metadata: None,
        }
    }

    /// Create a standard user policy.
    pub fn standard(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            permissions: Permission::STANDARD,
            verb_policy: VerbPolicy::allow_all(),
            entity_policy: EntityPolicy::allow_all(),
            expires_at: 0,
            is_active: true,
            role: Some("user".to_string()),
            metadata: None,
        }
    }

    /// Create an admin policy.
    pub fn admin(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            permissions: Permission::ALL,
            verb_policy: VerbPolicy::allow_all(),
            entity_policy: EntityPolicy::allow_all(),
            expires_at: 0,
            is_active: true,
            role: Some("admin".to_string()),
            metadata: None,
        }
    }

    /// Grant a permission.
    pub fn grant(mut self, permission: Permission) -> Self {
        self.permissions |= permission;
        self
    }

    /// Revoke a permission.
    pub fn revoke(mut self, permission: Permission) -> Self {
        self.permissions &= !permission;
        self
    }

    /// Set verb policy.
    pub fn with_verb_policy(mut self, policy: VerbPolicy) -> Self {
        self.verb_policy = policy;
        self
    }

    /// Set entity policy.
    pub fn with_entity_policy(mut self, policy: EntityPolicy) -> Self {
        self.entity_policy = policy;
        self
    }

    /// Set expiration.
    pub fn expires_at(mut self, timestamp: u64) -> Self {
        self.expires_at = timestamp;
        self
    }

    /// Set role.
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Deactivate the policy.
    pub fn deactivate(mut self) -> Self {
        self.is_active = false;
        self
    }

    /// Check if the policy is valid (active and not expired).
    pub fn is_valid(&self) -> bool {
        if !self.is_active {
            return false;
        }

        if self.expires_at > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            if now > self.expires_at {
                return false;
            }
        }

        true
    }

    /// Check if user has a specific permission.
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(permission)
    }
}

impl Default for UserPolicy {
    fn default() -> Self {
        Self::new("anonymous")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_policy_new() {
        let policy = UserPolicy::new("user-123");
        assert_eq!(policy.user_id, "user-123");
        assert!(policy.is_valid());
        assert!(policy.permissions.can_view());
    }

    #[test]
    fn user_policy_presets() {
        let read_only = UserPolicy::read_only("viewer");
        assert!(read_only.permissions.can_view());
        assert!(!read_only.permissions.can_edit());
        assert_eq!(read_only.role, Some("viewer".to_string()));

        let admin = UserPolicy::admin("admin");
        assert!(admin.permissions.is_admin());
        assert!(admin.permissions.can_edit());
    }

    #[test]
    fn user_policy_grant_revoke() {
        let policy = UserPolicy::new("user")
            .grant(Permission::EDIT_ENTITIES)
            .grant(Permission::CREATE_ENTITIES);

        assert!(policy.has_permission(Permission::EDIT_ENTITIES));
        assert!(policy.has_permission(Permission::CREATE_ENTITIES));

        let policy = policy.revoke(Permission::EDIT_ENTITIES);
        assert!(!policy.has_permission(Permission::EDIT_ENTITIES));
        assert!(policy.has_permission(Permission::CREATE_ENTITIES));
    }

    #[test]
    fn user_policy_expiration() {
        // Expired policy
        let policy = UserPolicy::new("user").expires_at(1);
        assert!(!policy.is_valid());

        // Future expiration
        let future = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;

        let policy = UserPolicy::new("user").expires_at(future);
        assert!(policy.is_valid());
    }

    #[test]
    fn user_policy_deactivate() {
        let policy = UserPolicy::new("user").deactivate();
        assert!(!policy.is_valid());
    }
}
