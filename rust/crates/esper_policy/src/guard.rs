//! Policy enforcement guard for navigation operations.
//!
//! PolicyGuard wraps navigation state and enforces policy rules:
//! - Verb filtering (blocks disallowed verbs before execution)
//! - Entity visibility (filters entities from snapshots)
//! - Field masking (redacts sensitive fields based on policy)
//! - Permission checks (validates user has required permissions)

use crate::{
    entity::{EntityVisibility, FieldMask},
    error::PolicyError,
    fingerprint::PolicyFingerprint,
    permission::Permission,
    user::UserPolicy,
    verb::{VerbCategory, VerbKind},
};
use esper_core::{EffectSet, Verb};
use esper_snapshot::{ChamberSnapshot, WorldSnapshot};

/// Result type for policy operations.
pub type PolicyResult<T> = Result<T, PolicyError>;

/// Policy enforcement layer that wraps navigation operations.
///
/// The guard intercepts verbs before execution and filters/masks
/// results after execution based on user policy.
#[derive(Debug, Clone)]
pub struct PolicyGuard {
    /// The user's policy (permissions, verb rules, entity rules)
    policy: UserPolicy,
    /// Whether the policy has been validated
    validated: bool,
}

impl PolicyGuard {
    /// Create a new policy guard with the given user policy.
    pub fn new(policy: UserPolicy) -> Self {
        Self {
            policy,
            validated: false,
        }
    }

    /// Create a guard with read-only permissions (viewing only).
    pub fn read_only() -> Self {
        Self::new(UserPolicy::read_only("anonymous"))
    }

    /// Create a guard with standard user permissions.
    pub fn standard() -> Self {
        Self::new(UserPolicy::standard("anonymous"))
    }

    /// Create a guard with full admin permissions.
    pub fn admin() -> Self {
        Self::new(UserPolicy::admin("anonymous"))
    }

    /// Validate the policy guard, checking for expiration and consistency.
    pub fn validate(&mut self) -> PolicyResult<()> {
        if !self.policy.is_valid() {
            return Err(PolicyError::PolicyExpired);
        }
        self.validated = true;
        Ok(())
    }

    /// Check if the guard has been validated.
    pub fn is_validated(&self) -> bool {
        self.validated
    }

    /// Get the underlying user policy.
    pub fn policy(&self) -> &UserPolicy {
        &self.policy
    }

    /// Check if a permission is granted.
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.policy.permissions.contains(permission)
    }

    /// Check if a verb is allowed by policy.
    pub fn is_verb_allowed(&self, verb: &Verb) -> bool {
        self.policy.verb_policy.is_allowed(verb)
    }

    /// Pre-execution check: validates verb is allowed before execution.
    ///
    /// Returns Ok(()) if verb can proceed, Err if blocked by policy.
    pub fn pre_execute(&self, verb: &Verb) -> PolicyResult<()> {
        // Check policy validity (active and not expired)
        if !self.policy.is_valid() {
            return Err(PolicyError::PolicyExpired);
        }

        // Check verb permission via VerbPolicy
        if !self.policy.verb_policy.is_allowed(verb) {
            return Err(PolicyError::VerbDenied(*verb));
        }

        // Check specific permission requirements based on verb category
        let kind = VerbKind::from(verb);
        if kind.category() == VerbCategory::CrossChamber
            && !self.has_permission(Permission::CROSS_CHAMBER)
        {
            return Err(PolicyError::PermissionDenied(Permission::CROSS_CHAMBER));
        }

        Ok(())
    }

    /// Post-execution filter: applies entity visibility and field masking.
    ///
    /// This modifies the effect set to indicate what was filtered.
    pub fn post_execute(&self, effects: EffectSet) -> EffectSet {
        // If no view permission, filter out content-related effects
        if !self.has_permission(Permission::VIEW_ENTITIES) {
            return effects & !(EffectSet::CAMERA_CHANGED | EffectSet::TAXONOMY_CHANGED);
        }

        effects
    }

    /// Filter a world snapshot based on entity visibility rules.
    ///
    /// Returns a new snapshot with only visible entities.
    pub fn filter_world(&self, world: &WorldSnapshot) -> FilteredWorld {
        let mut visible_chambers = Vec::with_capacity(world.chambers.len());
        let mut hidden_count = 0;

        for chamber in &world.chambers {
            let (filtered, hidden) = self.filter_chamber(chamber);
            visible_chambers.push(filtered);
            hidden_count += hidden;
        }

        FilteredWorld {
            chambers: visible_chambers,
            hidden_entity_count: hidden_count,
            policy_fingerprint: PolicyFingerprint::from_policy(&self.policy),
        }
    }

    /// Filter a single chamber snapshot.
    fn filter_chamber(&self, chamber: &ChamberSnapshot) -> (FilteredChamber, usize) {
        let entity_count = chamber.entity_ids.len();
        let mut visible_indices = Vec::with_capacity(entity_count);
        let mut masked_fields = Vec::new();
        let mut hidden_count = 0;

        for idx in 0..entity_count {
            let entity_id = chamber.entity_ids[idx];
            let kind_id = chamber.kind_ids[idx] as u32;

            // Check visibility using EntityPolicy
            let visibility = self.policy.entity_policy.get_visibility(entity_id, kind_id);

            match visibility {
                EntityVisibility::Visible => {
                    visible_indices.push(idx as u32);
                    // Check for field masking
                    if !self
                        .policy
                        .entity_policy
                        .field_mask
                        .hidden_fields
                        .is_empty()
                        || !self
                            .policy
                            .entity_policy
                            .field_mask
                            .redacted_fields
                            .is_empty()
                    {
                        masked_fields
                            .push((idx as u32, self.policy.entity_policy.field_mask.clone()));
                    }
                }
                EntityVisibility::Hidden | EntityVisibility::Masked => {
                    // Still visible but with restrictions
                    visible_indices.push(idx as u32);
                    // Apply full redaction mask
                    let mut mask = FieldMask::new();
                    mask.redacted_fields.insert("*".to_string()); // Redact all
                    masked_fields.push((idx as u32, mask));
                }
                EntityVisibility::Invisible => {
                    hidden_count += 1;
                }
            }
        }

        (
            FilteredChamber {
                chamber_id: chamber.id,
                visible_indices,
                masked_fields,
            },
            hidden_count,
        )
    }

    /// Check visibility for a specific entity.
    pub fn check_entity_visibility(&self, entity_id: u64, kind_id: u32) -> EntityVisibility {
        // Admin sees everything
        if self.has_permission(Permission::ADMIN) {
            return EntityVisibility::Visible;
        }

        // Use EntityPolicy
        self.policy.entity_policy.get_visibility(entity_id, kind_id)
    }
}

/// Result of filtering a world snapshot.
#[derive(Debug, Clone)]
pub struct FilteredWorld {
    /// Filtered chamber data
    pub chambers: Vec<FilteredChamber>,
    /// Number of entities hidden by policy
    pub hidden_entity_count: usize,
    /// Fingerprint of the policy used for filtering
    pub policy_fingerprint: PolicyFingerprint,
}

/// Filtered chamber with visibility information.
#[derive(Debug, Clone)]
pub struct FilteredChamber {
    /// Original chamber ID
    pub chamber_id: u32,
    /// Indices of visible entities (into original arrays)
    pub visible_indices: Vec<u32>,
    /// Field masks for specific entities: (entity_idx, mask)
    pub masked_fields: Vec<(u32, FieldMask)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_creation() {
        let guard = PolicyGuard::standard();
        assert!(guard.has_permission(Permission::VIEW_ENTITIES));
        assert!(guard.has_permission(Permission::NAVIGATE));
        assert!(!guard.has_permission(Permission::ADMIN));
    }

    #[test]
    fn test_read_only_guard() {
        let guard = PolicyGuard::read_only();
        assert!(guard.has_permission(Permission::VIEW_ENTITIES));
        assert!(guard.has_permission(Permission::NAVIGATE));
        assert!(!guard.has_permission(Permission::EDIT_ENTITIES));
        assert!(!guard.has_permission(Permission::CROSS_CHAMBER));
    }

    #[test]
    fn test_admin_guard() {
        let guard = PolicyGuard::admin();
        assert!(guard.has_permission(Permission::ADMIN));
        assert!(guard.has_permission(Permission::VIEW_ENTITIES));
        assert!(guard.has_permission(Permission::EDIT_ENTITIES));
    }

    #[test]
    fn test_verb_allowed_check() {
        let guard = PolicyGuard::standard();

        // Navigation verbs should be allowed
        assert!(guard.is_verb_allowed(&Verb::PanBy { dx: 0.0, dy: 0.0 }));
        assert!(guard.is_verb_allowed(&Verb::Zoom(1.0)));
        assert!(guard.is_verb_allowed(&Verb::Select(0)));
    }

    #[test]
    fn test_pre_execute_allowed() {
        let guard = PolicyGuard::standard();

        let result = guard.pre_execute(&Verb::PanBy { dx: 10.0, dy: 5.0 });
        assert!(result.is_ok());
    }

    #[test]
    fn test_pre_execute_cross_chamber_denied_by_verb_policy() {
        // Read-only policy denies CrossChamber verbs via VerbPolicy
        let guard = PolicyGuard::read_only();

        let result = guard.pre_execute(&Verb::DiveInto(1));
        // VerbPolicy denial happens first (before permission check)
        assert!(matches!(result, Err(PolicyError::VerbDenied(_))));
    }

    #[test]
    fn test_pre_execute_cross_chamber_denied_by_permission() {
        // Create a policy that allows the verb but denies the permission
        let mut policy = UserPolicy::standard("test");
        policy.permissions = Permission::VIEW_ENTITIES | Permission::NAVIGATE; // No CROSS_CHAMBER
                                                                               // Verb policy allows all
        let guard = PolicyGuard::new(policy);

        let result = guard.pre_execute(&Verb::DiveInto(1));
        assert!(matches!(
            result,
            Err(PolicyError::PermissionDenied(Permission::CROSS_CHAMBER))
        ));
    }

    #[test]
    fn test_entity_visibility_admin() {
        let guard = PolicyGuard::admin();

        let visibility = guard.check_entity_visibility(123, 0);
        assert_eq!(visibility, EntityVisibility::Visible);
    }

    #[test]
    fn test_entity_visibility_no_permission() {
        let mut policy = UserPolicy::default();
        policy.permissions = Permission::NAVIGATE; // No VIEW_ENTITIES
        let guard = PolicyGuard::new(policy);

        // Default entity policy is allow_all, so still visible
        let visibility = guard.check_entity_visibility(123, 0);
        assert_eq!(visibility, EntityVisibility::Visible);
    }

    #[test]
    fn test_validation() {
        let mut guard = PolicyGuard::standard();
        assert!(!guard.is_validated());

        guard.validate().unwrap();
        assert!(guard.is_validated());
    }

    #[test]
    fn test_expired_policy() {
        let mut policy = UserPolicy::standard("test");
        policy.expires_at = 1; // Expired (Unix timestamp 1 is in 1970)

        let mut guard = PolicyGuard::new(policy);
        let result = guard.validate();
        assert!(matches!(result, Err(PolicyError::PolicyExpired)));
    }

    #[test]
    fn test_post_execute_filters_without_view() {
        let mut policy = UserPolicy::default();
        policy.permissions = Permission::NAVIGATE; // No VIEW_ENTITIES
        let guard = PolicyGuard::new(policy);

        let effects =
            EffectSet::CAMERA_CHANGED | EffectSet::TAXONOMY_CHANGED | EffectSet::PHASE_RESET;
        let filtered = guard.post_execute(effects);

        // CAMERA_CHANGED and TAXONOMY_CHANGED should be filtered
        assert!(!filtered.contains(EffectSet::CAMERA_CHANGED));
        assert!(!filtered.contains(EffectSet::TAXONOMY_CHANGED));
        // PHASE_RESET should remain
        assert!(filtered.contains(EffectSet::PHASE_RESET));
    }
}
