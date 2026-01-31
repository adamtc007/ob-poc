//! Verb-level policy rules.

use esper_core::Verb;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Policy for which verbs are allowed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerbPolicy {
    /// Default rule (allow or deny).
    pub default_allow: bool,
    /// Explicit allow list (overrides default deny).
    pub allow_list: HashSet<VerbRule>,
    /// Explicit deny list (overrides default allow).
    pub deny_list: HashSet<VerbRule>,
}

/// Rule for matching verbs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerbRule {
    /// Match a specific verb.
    Exact(VerbKind),
    /// Match a category of verbs.
    Category(VerbCategory),
    /// Match all verbs.
    All,
}

/// Verb categories for grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerbCategory {
    /// Spatial navigation (pan, zoom, center).
    Spatial,
    /// Structural navigation (ascend, descend, next, prev).
    Structural,
    /// Cross-chamber navigation (dive, pull back, surface).
    CrossChamber,
    /// Selection operations (select, focus, track).
    Selection,
    /// Mode changes (toggle mode).
    Mode,
    /// Special operations (noop).
    Special,
}

/// Simplified verb kinds for policy matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerbKind {
    // Spatial
    PanBy,
    PanTo,
    Zoom,
    ZoomFit,
    ZoomTo,
    Center,
    Stop,
    Enhance,
    Reduce,

    // Cross-chamber
    DiveInto,
    PullBack,
    Surface,

    // Structural
    Ascend,
    Descend,
    DescendTo,
    Next,
    Prev,
    First,
    Last,
    Expand,
    Collapse,
    Root,

    // Selection
    Select,
    Focus,
    Track,
    Preview,
    ClearPreview,

    // Mode
    ModeSpatial,
    ModeStructural,
    ModeToggle,

    // Special
    Noop,
}

impl From<&Verb> for VerbKind {
    fn from(verb: &Verb) -> Self {
        match verb {
            Verb::PanBy { .. } => VerbKind::PanBy,
            Verb::PanTo { .. } => VerbKind::PanTo,
            Verb::Zoom(_) => VerbKind::Zoom,
            Verb::ZoomFit => VerbKind::ZoomFit,
            Verb::ZoomTo(_) => VerbKind::ZoomTo,
            Verb::Center => VerbKind::Center,
            Verb::Stop => VerbKind::Stop,
            Verb::Enhance => VerbKind::Enhance,
            Verb::Reduce => VerbKind::Reduce,
            Verb::DiveInto(_) => VerbKind::DiveInto,
            Verb::PullBack => VerbKind::PullBack,
            Verb::Surface => VerbKind::Surface,
            Verb::Ascend => VerbKind::Ascend,
            Verb::Descend => VerbKind::Descend,
            Verb::DescendTo(_) => VerbKind::DescendTo,
            Verb::Next => VerbKind::Next,
            Verb::Prev => VerbKind::Prev,
            Verb::First => VerbKind::First,
            Verb::Last => VerbKind::Last,
            Verb::Expand => VerbKind::Expand,
            Verb::Collapse => VerbKind::Collapse,
            Verb::Root => VerbKind::Root,
            Verb::Select(_) => VerbKind::Select,
            Verb::Focus(_) => VerbKind::Focus,
            Verb::Track(_) => VerbKind::Track,
            Verb::Preview(_) => VerbKind::Preview,
            Verb::ClearPreview => VerbKind::ClearPreview,
            Verb::ModeSpatial => VerbKind::ModeSpatial,
            Verb::ModeStructural => VerbKind::ModeStructural,
            Verb::ModeToggle => VerbKind::ModeToggle,
            Verb::Noop => VerbKind::Noop,
        }
    }
}

impl VerbKind {
    /// Get all verb kinds.
    pub fn all() -> impl Iterator<Item = VerbKind> {
        [
            VerbKind::PanBy,
            VerbKind::PanTo,
            VerbKind::Zoom,
            VerbKind::ZoomFit,
            VerbKind::ZoomTo,
            VerbKind::Center,
            VerbKind::Stop,
            VerbKind::Enhance,
            VerbKind::Reduce,
            VerbKind::DiveInto,
            VerbKind::PullBack,
            VerbKind::Surface,
            VerbKind::Ascend,
            VerbKind::Descend,
            VerbKind::DescendTo,
            VerbKind::Next,
            VerbKind::Prev,
            VerbKind::First,
            VerbKind::Last,
            VerbKind::Expand,
            VerbKind::Collapse,
            VerbKind::Root,
            VerbKind::Select,
            VerbKind::Focus,
            VerbKind::Track,
            VerbKind::Preview,
            VerbKind::ClearPreview,
            VerbKind::ModeSpatial,
            VerbKind::ModeStructural,
            VerbKind::ModeToggle,
            VerbKind::Noop,
        ]
        .into_iter()
    }

    /// Get the category of this verb kind.
    pub fn category(&self) -> VerbCategory {
        match self {
            VerbKind::PanBy
            | VerbKind::PanTo
            | VerbKind::Zoom
            | VerbKind::ZoomFit
            | VerbKind::ZoomTo
            | VerbKind::Center
            | VerbKind::Stop
            | VerbKind::Enhance
            | VerbKind::Reduce => VerbCategory::Spatial,

            VerbKind::DiveInto | VerbKind::PullBack | VerbKind::Surface => {
                VerbCategory::CrossChamber
            }

            VerbKind::Ascend
            | VerbKind::Descend
            | VerbKind::DescendTo
            | VerbKind::Next
            | VerbKind::Prev
            | VerbKind::First
            | VerbKind::Last
            | VerbKind::Expand
            | VerbKind::Collapse
            | VerbKind::Root => VerbCategory::Structural,

            VerbKind::Select
            | VerbKind::Focus
            | VerbKind::Track
            | VerbKind::Preview
            | VerbKind::ClearPreview => VerbCategory::Selection,

            VerbKind::ModeSpatial | VerbKind::ModeStructural | VerbKind::ModeToggle => {
                VerbCategory::Mode
            }

            VerbKind::Noop => VerbCategory::Special,
        }
    }
}

impl VerbPolicy {
    /// Create a policy that allows all verbs.
    pub fn allow_all() -> Self {
        Self {
            default_allow: true,
            allow_list: HashSet::new(),
            deny_list: HashSet::new(),
        }
    }

    /// Create a policy that denies all verbs.
    pub fn deny_all() -> Self {
        Self {
            default_allow: false,
            allow_list: HashSet::new(),
            deny_list: HashSet::new(),
        }
    }

    /// Allow a specific verb.
    pub fn allow(mut self, rule: VerbRule) -> Self {
        self.deny_list.remove(&rule);
        self.allow_list.insert(rule);
        self
    }

    /// Deny a specific verb.
    pub fn deny(mut self, rule: VerbRule) -> Self {
        self.allow_list.remove(&rule);
        self.deny_list.insert(rule);
        self
    }

    /// Allow a category of verbs.
    pub fn allow_category(self, category: VerbCategory) -> Self {
        self.allow(VerbRule::Category(category))
    }

    /// Deny a category of verbs.
    pub fn deny_category(self, category: VerbCategory) -> Self {
        self.deny(VerbRule::Category(category))
    }

    /// Check if a verb is allowed.
    pub fn is_allowed(&self, verb: &Verb) -> bool {
        let kind = VerbKind::from(verb);
        let category = kind.category();

        // Check explicit deny list first
        if self.deny_list.contains(&VerbRule::All)
            || self.deny_list.contains(&VerbRule::Exact(kind))
            || self.deny_list.contains(&VerbRule::Category(category))
        {
            return false;
        }

        // Check explicit allow list
        if self.allow_list.contains(&VerbRule::All)
            || self.allow_list.contains(&VerbRule::Exact(kind))
            || self.allow_list.contains(&VerbRule::Category(category))
        {
            return true;
        }

        // Fall back to default
        self.default_allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_kind_category() {
        assert_eq!(VerbKind::PanBy.category(), VerbCategory::Spatial);
        assert_eq!(VerbKind::Ascend.category(), VerbCategory::Structural);
        assert_eq!(VerbKind::DiveInto.category(), VerbCategory::CrossChamber);
        assert_eq!(VerbKind::Focus.category(), VerbCategory::Selection);
        assert_eq!(VerbKind::ModeToggle.category(), VerbCategory::Mode);
    }

    #[test]
    fn policy_allow_all() {
        let policy = VerbPolicy::allow_all();
        assert!(policy.is_allowed(&Verb::Ascend));
        assert!(policy.is_allowed(&Verb::DiveInto(0)));
    }

    #[test]
    fn policy_deny_all() {
        let policy = VerbPolicy::deny_all();
        assert!(!policy.is_allowed(&Verb::Ascend));
        assert!(!policy.is_allowed(&Verb::DiveInto(0)));
    }

    #[test]
    fn policy_explicit_deny() {
        let policy = VerbPolicy::allow_all().deny_category(VerbCategory::CrossChamber);

        assert!(policy.is_allowed(&Verb::Ascend));
        assert!(!policy.is_allowed(&Verb::DiveInto(0)));
        assert!(!policy.is_allowed(&Verb::PullBack));
    }

    #[test]
    fn policy_explicit_allow() {
        let policy = VerbPolicy::deny_all().allow(VerbRule::Exact(VerbKind::Ascend));

        assert!(policy.is_allowed(&Verb::Ascend));
        assert!(!policy.is_allowed(&Verb::Descend));
    }
}
