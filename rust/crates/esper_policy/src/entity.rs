//! Entity-level visibility and field masking policies.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Visibility setting for an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EntityVisibility {
    /// Entity is fully visible.
    #[default]
    Visible,
    /// Entity exists but details are hidden.
    Hidden,
    /// Entity is masked (shows placeholder).
    Masked,
    /// Entity is completely invisible (not shown).
    Invisible,
}

impl EntityVisibility {
    /// Check if the entity should be shown at all.
    pub fn is_shown(&self) -> bool {
        !matches!(self, EntityVisibility::Invisible)
    }

    /// Check if entity details are accessible.
    pub fn has_details(&self) -> bool {
        matches!(self, EntityVisibility::Visible)
    }
}

/// Field masking configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldMask {
    /// Fields that should be completely hidden.
    pub hidden_fields: HashSet<String>,
    /// Fields that should be redacted (shown as ***).
    pub redacted_fields: HashSet<String>,
    /// Fields that should be partially shown (e.g., last 4 digits).
    pub partial_fields: HashMap<String, PartialMaskConfig>,
}

/// Configuration for partial field masking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialMaskConfig {
    /// How many characters to show.
    pub visible_chars: usize,
    /// Show from start or end.
    pub from_end: bool,
    /// Mask character.
    pub mask_char: char,
}

impl Default for PartialMaskConfig {
    fn default() -> Self {
        Self {
            visible_chars: 4,
            from_end: true,
            mask_char: '*',
        }
    }
}

impl FieldMask {
    /// Create an empty field mask.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field to hide.
    pub fn hide(mut self, field: impl Into<String>) -> Self {
        self.hidden_fields.insert(field.into());
        self
    }

    /// Add a field to redact.
    pub fn redact(mut self, field: impl Into<String>) -> Self {
        self.redacted_fields.insert(field.into());
        self
    }

    /// Add a field to partially mask.
    pub fn partial(mut self, field: impl Into<String>, config: PartialMaskConfig) -> Self {
        self.partial_fields.insert(field.into(), config);
        self
    }

    /// Apply mask to a field value.
    pub fn apply(&self, field: &str, value: &str) -> Option<String> {
        if self.hidden_fields.contains(field) {
            return None;
        }

        if self.redacted_fields.contains(field) {
            return Some("***".to_string());
        }

        if let Some(config) = self.partial_fields.get(field) {
            let len = value.len();
            if len <= config.visible_chars {
                return Some(config.mask_char.to_string().repeat(len));
            }

            let mask_len = len - config.visible_chars;
            let mask_str = config.mask_char.to_string().repeat(mask_len);

            if config.from_end {
                let visible = &value[len - config.visible_chars..];
                return Some(format!("{}{}", mask_str, visible));
            } else {
                let visible = &value[..config.visible_chars];
                return Some(format!("{}{}", visible, mask_str));
            }
        }

        Some(value.to_string())
    }

    /// Check if a field should be visible.
    pub fn is_visible(&self, field: &str) -> bool {
        !self.hidden_fields.contains(field)
    }
}

/// Entity-level policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityPolicy {
    /// Default visibility for entities.
    pub default_visibility: EntityVisibility,
    /// Per-entity visibility overrides (entity_id → visibility).
    pub entity_visibility: HashMap<u64, EntityVisibility>,
    /// Per-kind visibility (kind_id → visibility).
    pub kind_visibility: HashMap<u32, EntityVisibility>,
    /// Field masking rules.
    pub field_mask: FieldMask,
}

impl EntityPolicy {
    /// Create a policy where all entities are visible.
    pub fn allow_all() -> Self {
        Self {
            default_visibility: EntityVisibility::Visible,
            ..Default::default()
        }
    }

    /// Create a policy where all entities are hidden by default.
    pub fn hide_all() -> Self {
        Self {
            default_visibility: EntityVisibility::Hidden,
            ..Default::default()
        }
    }

    /// Set visibility for a specific entity.
    pub fn set_entity_visibility(mut self, entity_id: u64, visibility: EntityVisibility) -> Self {
        self.entity_visibility.insert(entity_id, visibility);
        self
    }

    /// Set visibility for a kind of entity.
    pub fn set_kind_visibility(mut self, kind_id: u32, visibility: EntityVisibility) -> Self {
        self.kind_visibility.insert(kind_id, visibility);
        self
    }

    /// Set the field mask.
    pub fn with_field_mask(mut self, mask: FieldMask) -> Self {
        self.field_mask = mask;
        self
    }

    /// Get visibility for an entity.
    pub fn get_visibility(&self, entity_id: u64, kind_id: u32) -> EntityVisibility {
        // Check entity-specific override first
        if let Some(&vis) = self.entity_visibility.get(&entity_id) {
            return vis;
        }

        // Check kind-specific override
        if let Some(&vis) = self.kind_visibility.get(&kind_id) {
            return vis;
        }

        // Fall back to default
        self.default_visibility
    }

    /// Check if an entity should be shown.
    pub fn is_visible(&self, entity_id: u64, kind_id: u32) -> bool {
        self.get_visibility(entity_id, kind_id).is_shown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_visibility() {
        assert!(EntityVisibility::Visible.is_shown());
        assert!(EntityVisibility::Hidden.is_shown());
        assert!(EntityVisibility::Masked.is_shown());
        assert!(!EntityVisibility::Invisible.is_shown());

        assert!(EntityVisibility::Visible.has_details());
        assert!(!EntityVisibility::Hidden.has_details());
    }

    #[test]
    fn field_mask_hidden() {
        let mask = FieldMask::new().hide("password");
        assert!(mask.apply("password", "secret").is_none());
        assert_eq!(mask.apply("name", "John"), Some("John".to_string()));
    }

    #[test]
    fn field_mask_redacted() {
        let mask = FieldMask::new().redact("ssn");
        assert_eq!(mask.apply("ssn", "123-45-6789"), Some("***".to_string()));
    }

    #[test]
    fn field_mask_partial() {
        let mask = FieldMask::new().partial(
            "card",
            PartialMaskConfig {
                visible_chars: 4,
                from_end: true,
                mask_char: '*',
            },
        );
        assert_eq!(
            mask.apply("card", "1234567890123456"),
            Some("************3456".to_string())
        );
    }

    #[test]
    fn field_mask_partial_from_start() {
        let mask = FieldMask::new().partial(
            "phone",
            PartialMaskConfig {
                visible_chars: 3,
                from_end: false,
                mask_char: 'X',
            },
        );
        assert_eq!(
            mask.apply("phone", "1234567890"),
            Some("123XXXXXXX".to_string())
        );
    }

    #[test]
    fn entity_policy_visibility() {
        let policy = EntityPolicy::allow_all()
            .set_entity_visibility(100, EntityVisibility::Hidden)
            .set_kind_visibility(5, EntityVisibility::Masked);

        assert!(policy.is_visible(1, 1)); // Default visible
        assert!(policy.is_visible(100, 1)); // Entity override (hidden still shown)
        assert_eq!(policy.get_visibility(100, 1), EntityVisibility::Hidden);
        assert_eq!(policy.get_visibility(200, 5), EntityVisibility::Masked);
    }
}
