//! Permission flags for user capabilities.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// Permission flags that define what a user can do.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct Permission: u32 {
        /// View entities in the graph.
        const VIEW_ENTITIES = 1 << 0;

        /// Edit entities (update fields).
        const EDIT_ENTITIES = 1 << 1;

        /// Create new entities.
        const CREATE_ENTITIES = 1 << 2;

        /// Delete entities.
        const DELETE_ENTITIES = 1 << 3;

        /// Navigate between entities.
        const NAVIGATE = 1 << 4;

        /// Use cross-chamber navigation (doors).
        const CROSS_CHAMBER = 1 << 5;

        /// View sensitive fields.
        const VIEW_SENSITIVE = 1 << 6;

        /// Export data.
        const EXPORT = 1 << 7;

        /// View ownership/control relationships.
        const VIEW_OWNERSHIP = 1 << 8;

        /// View KYC/compliance data.
        const VIEW_KYC = 1 << 9;

        /// Admin operations.
        const ADMIN = 1 << 10;

        /// No permissions.
        const NONE = 0;

        /// Read-only access.
        const READ_ONLY = Self::VIEW_ENTITIES.bits() | Self::NAVIGATE.bits();

        /// Standard user access.
        const STANDARD = Self::VIEW_ENTITIES.bits()
            | Self::NAVIGATE.bits()
            | Self::CROSS_CHAMBER.bits()
            | Self::VIEW_OWNERSHIP.bits();

        /// Editor access.
        const EDITOR = Self::STANDARD.bits()
            | Self::EDIT_ENTITIES.bits()
            | Self::CREATE_ENTITIES.bits();

        /// Full access (except admin).
        const FULL = Self::EDITOR.bits()
            | Self::DELETE_ENTITIES.bits()
            | Self::VIEW_SENSITIVE.bits()
            | Self::EXPORT.bits()
            | Self::VIEW_KYC.bits();

        /// All permissions.
        const ALL = Self::FULL.bits() | Self::ADMIN.bits();
    }
}

impl Default for Permission {
    fn default() -> Self {
        Permission::READ_ONLY
    }
}

impl Permission {
    /// Check if this permission set allows viewing entities.
    pub fn can_view(&self) -> bool {
        self.contains(Permission::VIEW_ENTITIES)
    }

    /// Check if this permission set allows editing.
    pub fn can_edit(&self) -> bool {
        self.contains(Permission::EDIT_ENTITIES)
    }

    /// Check if this permission set allows navigation.
    pub fn can_navigate(&self) -> bool {
        self.contains(Permission::NAVIGATE)
    }

    /// Check if this permission set allows cross-chamber navigation.
    pub fn can_cross_chamber(&self) -> bool {
        self.contains(Permission::CROSS_CHAMBER)
    }

    /// Check if this is an admin permission set.
    pub fn is_admin(&self) -> bool {
        self.contains(Permission::ADMIN)
    }

    /// Get a human-readable description of this permission.
    pub fn description(&self) -> &'static str {
        if self.contains(Permission::ALL) {
            "Full access with admin"
        } else if self.contains(Permission::FULL) {
            "Full access"
        } else if self.contains(Permission::EDITOR) {
            "Editor access"
        } else if self.contains(Permission::STANDARD) {
            "Standard access"
        } else if self.contains(Permission::READ_ONLY) {
            "Read-only access"
        } else if *self == Permission::NONE {
            "No access"
        } else {
            "Custom access"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_defaults() {
        let p = Permission::default();
        assert!(p.can_view());
        assert!(p.can_navigate());
        assert!(!p.can_edit());
    }

    #[test]
    fn permission_combinations() {
        let p = Permission::VIEW_ENTITIES | Permission::EDIT_ENTITIES;
        assert!(p.can_view());
        assert!(p.can_edit());
        assert!(!p.can_navigate());
    }

    #[test]
    fn permission_presets() {
        assert!(Permission::STANDARD.can_view());
        assert!(Permission::STANDARD.can_navigate());
        assert!(Permission::STANDARD.can_cross_chamber());
        assert!(!Permission::STANDARD.can_edit());

        assert!(Permission::EDITOR.can_edit());
        assert!(Permission::FULL.contains(Permission::VIEW_SENSITIVE));
        assert!(Permission::ALL.is_admin());
    }

    #[test]
    fn permission_descriptions() {
        assert_eq!(Permission::READ_ONLY.description(), "Read-only access");
        assert_eq!(Permission::ALL.description(), "Full access with admin");
        assert_eq!(Permission::NONE.description(), "No access");
    }
}
