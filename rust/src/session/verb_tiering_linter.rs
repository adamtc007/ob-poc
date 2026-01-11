//! Verb Tiering Linter
//!
//! Validates verb metadata for consistency. This linter is domain-agnostic -
//! different domains have different canonical sources of truth.
//!
//! # Universal Rules (apply to ALL domains)
//!
//! 1. **T007**: All verbs should have tiering metadata (warning for incremental adoption)
//! 2. **T002**: Projection verbs must be `internal: true` (they're derived, not user-facing)
//! 3. **T006**: Diagnostics verbs must be read-only
//!
//! # Tiering Model
//!
//! ```text
//! Tier        | Purpose                              | Writes Data | Internal
//! ------------|--------------------------------------|-------------|----------
//! reference   | Global reference data (catalogs)     | yes         | no
//! intent      | User-facing write operations         | yes         | no
//! projection  | Derived writes (from another source) | yes         | yes (required)
//! diagnostics | Read-only queries and validation     | no          | no
//! composite   | Multi-step orchestration             | yes         | no
//! ```
//!
//! # Source of Truth (domain-specific)
//!
//! Different domains have different canonical sources:
//! - `matrix` - Trading profile JSONB document
//! - `entity` - Entity graph (entity_relationships table)
//! - `workflow` - Case/KYC state machine
//! - `external` - External APIs (GLEIF, Companies House, SEC)
//! - `register` - Capital structure (fund/investor holdings)
//! - `catalog` - Reference data (seeded lookup tables)
//! - `session` - Ephemeral UI state
//! - `document` - Document catalog
//! - `operational` - Derived/projected tables

use dsl_core::config::types::{CrudOperation, SourceOfTruth, VerbBehavior, VerbConfig, VerbTier};

use super::verb_contract::{codes, VerbDiagnostics};

/// Check if a CRUD operation is a write operation
fn is_crud_write_operation(op: &CrudOperation) -> bool {
    matches!(
        op,
        CrudOperation::Insert
            | CrudOperation::Upsert
            | CrudOperation::Update
            | CrudOperation::Delete
    )
}

/// Result of linting a single verb
#[derive(Debug, Clone)]
pub struct VerbLintResult {
    pub full_name: String,
    pub diagnostics: VerbDiagnostics,
}

impl VerbLintResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }

    pub fn has_warnings(&self) -> bool {
        self.diagnostics.has_warnings()
    }
}

/// Result of linting all verbs
#[derive(Debug, Clone, Default)]
pub struct LintReport {
    pub results: Vec<VerbLintResult>,
    pub total_verbs: usize,
    pub verbs_with_errors: usize,
    pub verbs_with_warnings: usize,
    pub verbs_missing_metadata: usize,
}

impl LintReport {
    pub fn has_errors(&self) -> bool {
        self.verbs_with_errors > 0
    }

    pub fn has_warnings(&self) -> bool {
        self.verbs_with_warnings > 0
    }

    /// Get only verbs with issues
    pub fn issues_only(&self) -> Vec<&VerbLintResult> {
        self.results
            .iter()
            .filter(|r| r.has_errors() || r.has_warnings())
            .collect()
    }
}

/// Lint a single verb configuration for tiering rule compliance
pub fn lint_verb_tiering(domain: &str, verb_name: &str, config: &VerbConfig) -> VerbLintResult {
    let full_name = format!("{}.{}", domain, verb_name);
    let mut diagnostics = VerbDiagnostics::default();

    let metadata = match &config.metadata {
        Some(m) => m,
        None => {
            // Missing metadata is a warning, not an error (for incremental adoption)
            diagnostics.add_warning_with_path(
                codes::TIER_MISSING_METADATA,
                "Verb missing tiering metadata",
                Some("metadata"),
                Some("Add metadata: { tier: ..., source_of_truth: ... } to verb definition"),
            );
            return VerbLintResult {
                full_name,
                diagnostics,
            };
        }
    };

    // Determine if this verb writes (based on behavior)
    let is_write_behavior = match config.behavior {
        VerbBehavior::Crud => {
            // Check the crud config for the operation type
            config
                .crud
                .as_ref()
                .map(|c| is_crud_write_operation(&c.operation))
                .unwrap_or(false)
        }
        VerbBehavior::Plugin => {
            // Plugin handlers may or may not write - rely on metadata.writes_operational
            metadata.writes_operational
        }
        VerbBehavior::GraphQuery => false,
    };

    // Check if tagged as deprecated
    let is_deprecated = metadata.tags.iter().any(|t| t == "deprecated");

    // =========================================================================
    // Rule 1: Projection verbs must be internal
    // =========================================================================
    if matches!(metadata.tier, Some(VerbTier::Projection)) && !metadata.internal {
        diagnostics.add_error_with_path(
            codes::TIER_PROJECTION_NOT_INTERNAL,
            "Projection tier verbs must be internal: true",
            Some("metadata.internal"),
            Some("Add 'internal: true' - projection verbs are only called by materialize pipeline"),
        );
    }

    // =========================================================================
    // Rule 2: Deprecated verbs should be tier: projection
    // =========================================================================
    if is_deprecated && !matches!(metadata.tier, Some(VerbTier::Projection)) {
        diagnostics.add_warning_with_path(
            codes::TIER_DEPRECATED_NOT_PROJECTION,
            "Deprecated verbs should be tier: projection",
            Some("metadata.tier"),
            Some("Set tier: projection for deprecated verbs that write to operational tables"),
        );
    }

    // =========================================================================
    // Rule 3: REMOVED - was trading-matrix-specific
    // =========================================================================
    // Previously: "Intent verbs cannot write to operational tables"
    // This was wrong for non-trading-matrix domains (entity, kyc, fund, etc.)
    // where intent verbs write directly to their canonical source.
    // The writes_operational flag is now informational, not prescriptive.

    // =========================================================================
    // Rule 4: Diagnostics verbs must be read-only
    // =========================================================================
    if matches!(metadata.tier, Some(VerbTier::Diagnostics)) && is_write_behavior {
        diagnostics.add_error_with_path(
            codes::TIER_DIAGNOSTICS_HAS_WRITE,
            "Diagnostics tier verbs must be read-only",
            Some("behavior"),
            Some("Diagnostics verbs should use select/list operations, not insert/update/delete"),
        );
    }

    // =========================================================================
    // Rule 5: writes_operational should match behavior
    // =========================================================================
    if metadata.writes_operational && !is_write_behavior {
        // This is a consistency warning - metadata says it writes but behavior is read-only
        diagnostics.add_warning_with_path(
            codes::TIER_WRITES_OP_MISMATCH,
            "writes_operational: true but behavior appears read-only",
            Some("metadata.writes_operational"),
            Some("Either remove writes_operational or verify the plugin handler writes to DB"),
        );
    }

    // =========================================================================
    // Rule 6: Verbs that write but aren't projection/composite
    // =========================================================================
    if is_write_behavior
        && metadata.writes_operational
        && !matches!(
            metadata.tier,
            Some(VerbTier::Projection) | Some(VerbTier::Composite)
        )
    {
        diagnostics.add_error_with_path(
            codes::TIER_WRITE_NOT_PROJECTION,
            "Verbs writing to operational tables must be tier: projection or composite",
            Some("metadata.tier"),
            Some("Only projection (internal) and composite (materialize) verbs may write to operational tables"),
        );
    }

    // =========================================================================
    // Rule 7: Source of truth consistency
    // =========================================================================
    // NOTE: We no longer enforce strict source_of_truth matching because different
    // domains have different canonical sources:
    // - Trading profile → matrix
    // - Entity/ownership → entity
    // - KYC/cases → workflow
    // - Research → external
    // - Fund/investor → register
    // - Session/view → session
    //
    // The only strict rule: projection verbs must have source_of_truth: operational
    // (they derive from some other canonical source)
    if let Some(source) = &metadata.source_of_truth {
        if let Some(VerbTier::Projection) = metadata.tier {
            if !matches!(source, SourceOfTruth::Operational) {
                diagnostics.add_warning_with_path(
                    codes::TIER_INCONSISTENT_SOURCE,
                    "Projection tier verbs should have source_of_truth: operational",
                    Some("metadata.source_of_truth"),
                    Some("Projection verbs write to operational tables (derived from a canonical source)"),
                );
            }
        }
    }

    VerbLintResult {
        full_name,
        diagnostics,
    }
}

/// Lint all verbs from a domain configuration map
pub fn lint_all_verbs(
    domains: &std::collections::HashMap<String, dsl_core::config::types::DomainConfig>,
) -> LintReport {
    let mut report = LintReport::default();

    for (domain_name, domain_config) in domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            report.total_verbs += 1;

            let result = lint_verb_tiering(domain_name, verb_name, verb_config);

            if result.has_errors() {
                report.verbs_with_errors += 1;
            }
            if result.has_warnings() {
                report.verbs_with_warnings += 1;
            }
            if verb_config.metadata.is_none() {
                report.verbs_missing_metadata += 1;
            }

            report.results.push(result);
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::config::types::{CrudConfig, VerbMetadata};

    fn make_crud_insert_config() -> VerbConfig {
        VerbConfig {
            description: "Test verb".to_string(),
            behavior: VerbBehavior::Crud,
            crud: Some(CrudConfig {
                operation: CrudOperation::Insert,
                table: "test".to_string(),
                schema: "ob-poc".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn make_crud_select_config() -> VerbConfig {
        VerbConfig {
            description: "Test verb".to_string(),
            behavior: VerbBehavior::Crud,
            crud: Some(CrudConfig {
                operation: CrudOperation::Select,
                table: "test".to_string(),
                schema: "ob-poc".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn test_missing_metadata_warning() {
        let config = make_crud_select_config();

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(!result.has_errors());
        assert!(result.has_warnings());
        assert!(result
            .diagnostics
            .warnings
            .iter()
            .any(|w| w.code == codes::TIER_MISSING_METADATA));
    }

    #[test]
    fn test_projection_requires_internal() {
        let mut config = make_crud_insert_config();
        config.metadata = Some(VerbMetadata {
            tier: Some(VerbTier::Projection),
            source_of_truth: Some(SourceOfTruth::Operational),
            writes_operational: true,
            internal: false, // ERROR: should be true
            ..Default::default()
        });

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .errors
            .iter()
            .any(|e| e.code == codes::TIER_PROJECTION_NOT_INTERNAL));
    }

    #[test]
    fn test_intent_cannot_write_operational() {
        let config = VerbConfig {
            description: "Test verb".to_string(),
            behavior: VerbBehavior::Plugin,
            handler: Some("test_handler".to_string()),
            metadata: Some(VerbMetadata {
                tier: Some(VerbTier::Intent),
                source_of_truth: Some(SourceOfTruth::Matrix),
                writes_operational: true, // ERROR: intent can't write operational
                ..Default::default()
            }),
            ..Default::default()
        };

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .errors
            .iter()
            .any(|e| e.code == codes::TIER_INTENT_WRITES_OPERATIONAL));
    }

    #[test]
    fn test_diagnostics_must_be_readonly() {
        let mut config = make_crud_insert_config();
        config.metadata = Some(VerbMetadata {
            tier: Some(VerbTier::Diagnostics),
            source_of_truth: Some(SourceOfTruth::Operational),
            ..Default::default()
        });

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .errors
            .iter()
            .any(|e| e.code == codes::TIER_DIAGNOSTICS_HAS_WRITE));
    }

    #[test]
    fn test_valid_projection_verb() {
        let mut config = make_crud_insert_config();
        config.metadata = Some(VerbMetadata {
            tier: Some(VerbTier::Projection),
            source_of_truth: Some(SourceOfTruth::Operational),
            writes_operational: true,
            internal: true,
            tags: vec!["deprecated".to_string()],
            ..Default::default()
        });

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(!result.has_errors());
    }

    #[test]
    fn test_valid_intent_verb() {
        let config = VerbConfig {
            description: "Test verb".to_string(),
            behavior: VerbBehavior::Plugin,
            handler: Some("test_handler".to_string()),
            metadata: Some(VerbMetadata {
                tier: Some(VerbTier::Intent),
                source_of_truth: Some(SourceOfTruth::Matrix),
                writes_operational: false,
                ..Default::default()
            }),
            ..Default::default()
        };

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(!result.has_errors());
    }

    #[test]
    fn test_valid_diagnostics_verb() {
        let config = make_crud_select_config();

        let mut config_with_metadata = config;
        config_with_metadata.metadata = Some(VerbMetadata {
            tier: Some(VerbTier::Diagnostics),
            source_of_truth: Some(SourceOfTruth::Operational),
            ..Default::default()
        });

        let result = lint_verb_tiering("test", "verb", &config_with_metadata);
        assert!(!result.has_errors());
    }
}
