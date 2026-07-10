//! Workspace ↔ macro mode-tag membership (runtime authority).
//!
//! Macros declare workspace affinity via `routing.mode-tags` (e.g.
//! `[onboarding, structure]`); a workspace declares which mode-tags it accepts.
//! A macro is **owned by** a workspace when their tag sets intersect. This is the
//! single membership authority — admission is by **membership**, never by the
//! macro FQN's leading-domain token. `struct.lux.ucits.sicav` is named for the
//! structure it builds, not the workspace that owns it (its mode-tags are
//! `[onboarding, structure]`, both accepted by the `cbu` workspace).
//!
//! This table previously lived only in `xtask` (PACK001 lint,
//! `workspace_accepts_any_mode_tag`) and was therefore unreachable from the
//! runtime allowed-set composition in `verb_surface::compute_session_verb_surface`.
//! It is lifted here as the single source of truth; `xtask` re-consumes it
//! (`xtask` depends on the `ob-poc` library crate).

use std::collections::HashSet;
use std::sync::OnceLock;

/// A workspace's accepted mode-tag set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceModeTags {
    /// An explicit accepted-tag list.
    Tags(&'static [&'static str]),
    /// Umbrella workspace — accepts every mode-tag.
    All,
    /// Unknown workspace — fail-closed (accepts nothing; forces a table update).
    Unknown,
}

/// Workspace → accepted mode-tags. Mirrors the authoritative table that was
/// previously only in `xtask/src/main.rs::workspace_accepts_any_mode_tag`.
///
/// | Workspace            | Accepted mode-tags              |
/// |----------------------|---------------------------------|
/// | `cbu`                | structure, trading, onboarding  |
/// | `kyc`                | kyc, onboarding                 |
/// | `deal`               | deal, onboarding                |
/// | `on_boarding`        | (all) — umbrella                |
/// | `product_maintenance`| product, trading                |
/// | `instrument_matrix`  | trading, structure              |
/// | `sem_os_maintenance` | stewardship, governance         |
/// | _unknown_            | (none) — fail-closed            |
pub fn workspace_accepted_mode_tags(workspace: &str) -> WorkspaceModeTags {
    match workspace {
        "cbu" => WorkspaceModeTags::Tags(&["structure", "trading", "onboarding"]),
        "kyc" => WorkspaceModeTags::Tags(&["kyc", "onboarding"]),
        "deal" => WorkspaceModeTags::Tags(&["deal", "onboarding"]),
        "on_boarding" => WorkspaceModeTags::All,
        "product_maintenance" => WorkspaceModeTags::Tags(&["product", "trading"]),
        "instrument_matrix" => WorkspaceModeTags::Tags(&["trading", "structure"]),
        "sem_os_maintenance" => WorkspaceModeTags::Tags(&["stewardship", "governance"]),
        _ => WorkspaceModeTags::Unknown,
    }
}

/// True if `workspace` accepts any of the macro's `tags`. Single membership
/// predicate consumed by both runtime admission and the `xtask` PACK001 lint.
pub fn workspace_accepts_any_mode_tag(workspace: &str, tags: &[String]) -> bool {
    match workspace_accepted_mode_tags(workspace) {
        WorkspaceModeTags::All => true,
        WorkspaceModeTags::Unknown => false,
        WorkspaceModeTags::Tags(accepted) => tags.iter().any(|t| accepted.contains(&t.as_str())),
    }
}

/// Bridge: a session's `stage_focus` → the workspace key used by the mode-tag
/// table. `None` for stage-focus values whose workspace owns no macros (or which
/// don't resolve to a mode-tag-bearing workspace).
///
/// Note: this is the macro-membership bridge. It is intentionally distinct from
/// the rank-boost primary-*domain* map in `verb_surface::compute_rank_boost`
/// (which maps `stage_focus` → an atomic-verb domain like `"registry"`/`"focus"`,
/// not a mode-tag workspace key).
pub fn stage_focus_to_workspace(stage_focus: &str) -> Option<&'static str> {
    match stage_focus {
        "semos-onboarding" => Some("cbu"),
        "semos-kyc" => Some("kyc"),
        "semos-stewardship" => Some("sem_os_maintenance"),
        // "semos-data" / "semos-data-management" resolve to the `registry`
        // atomic domain, which is not a mode-tag-bearing workspace — no macros.
        _ => None,
    }
}

/// Cached `(macro_fqn, mode_tags)` table, loaded once from the macro registry.
/// Empty on load failure (degrades to "no macros owned" rather than panicking in
/// the hot surface-composition path).
fn macro_mode_tag_table() -> &'static [(String, Vec<String>)] {
    static TABLE: OnceLock<Vec<(String, Vec<String>)>> = OnceLock::new();
    TABLE.get_or_init(|| match crate::dsl_v2::macros::load_macro_registry() {
        Ok(reg) => reg
            .all()
            .map(|(fqn, schema)| (fqn.clone(), schema.routing.mode_tags.clone()))
            .collect(),
        Err(_) => Vec::new(),
    })
}

/// All macro FQNs owned (by mode-tag membership) by `workspace`.
///
/// A macro is owned when its declared `mode_tags` intersect the workspace's
/// accepted set (or the workspace is the umbrella `on_boarding`). Macros with no
/// mode-tags are owned by no workspace (they cannot be admitted by membership).
pub fn workspace_owned_macro_fqns(workspace: &str) -> HashSet<String> {
    let accepts_all = matches!(
        workspace_accepted_mode_tags(workspace),
        WorkspaceModeTags::All
    );
    macro_mode_tag_table()
        .iter()
        .filter(|(_, tags)| {
            (accepts_all && !tags.is_empty()) || workspace_accepts_any_mode_tag(workspace, tags)
        })
        .map(|(fqn, _)| fqn.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbu_accepts_structure_onboarding_trading() {
        assert!(workspace_accepts_any_mode_tag(
            "cbu",
            &["onboarding".into()]
        ));
        assert!(workspace_accepts_any_mode_tag("cbu", &["structure".into()]));
        assert!(workspace_accepts_any_mode_tag("cbu", &["trading".into()]));
        // cbu does NOT accept stewardship/kyc.
        assert!(!workspace_accepts_any_mode_tag(
            "cbu",
            &["stewardship".into()]
        ));
        assert!(!workspace_accepts_any_mode_tag("cbu", &["kyc".into()]));
    }

    #[test]
    fn unknown_workspace_fails_closed() {
        assert!(!workspace_accepts_any_mode_tag(
            "nonsense",
            &["onboarding".into()]
        ));
    }

    #[test]
    fn umbrella_accepts_any_tag() {
        // Parity with the lifted xtask table: the umbrella returns true for the
        // predicate regardless of tags. The "empty-tag macro is owned by nobody"
        // rule is enforced one level up in `workspace_owned_macro_fqns`
        // (`accepts_all && !tags.is_empty()`), not in this predicate.
        assert!(workspace_accepts_any_mode_tag(
            "on_boarding",
            &["anything".into()]
        ));
        assert!(workspace_accepts_any_mode_tag("on_boarding", &[]));
    }

    #[test]
    fn stage_focus_bridge() {
        assert_eq!(stage_focus_to_workspace("semos-onboarding"), Some("cbu"));
        assert_eq!(stage_focus_to_workspace("semos-kyc"), Some("kyc"));
        assert_eq!(stage_focus_to_workspace("semos-data"), None);
        assert_eq!(stage_focus_to_workspace("unknown"), None);
    }

    /// Real macro-registry membership: the CBU workspace owns the onboarding/
    /// structure macros (named `struct.*`/`structure.*`) by mode-tag membership,
    /// and does NOT own the stewardship `governance.*` macros — even though both
    /// are real macros in the registry. Cross-workspace isolation by membership.
    #[test]
    fn cbu_owns_structure_macros_not_stewardship() {
        let cbu = workspace_owned_macro_fqns("cbu");
        assert!(
            cbu.contains("structure.product-suite-full"),
            "cbu must own structure.product-suite-full [mode-tags onboarding,structure]; \
             owned={} sample={:?}",
            cbu.len(),
            cbu.iter().take(8).collect::<Vec<_>>()
        );
        assert!(
            !cbu.contains("governance.bootstrap-attribute-registry"),
            "cbu must NOT own governance.bootstrap-attribute-registry [mode-tags stewardship]"
        );

        // The stewardship workspace owns it; cbu does not. Membership isolation.
        let stewardship = workspace_owned_macro_fqns("sem_os_maintenance");
        assert!(stewardship.contains("governance.bootstrap-attribute-registry"));
        assert!(!stewardship.contains("structure.product-suite-full"));
    }
}
