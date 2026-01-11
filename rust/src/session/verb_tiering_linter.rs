//! Verb Tiering Linter
//!
//! Validates verb metadata against tiering rules defined in the 027 Trading Matrix
//! Canonical Pivot architecture. Enforces the following invariants:
//!
//! 1. **Projection verbs** (`tier: projection`) must be `internal: true`
//! 2. **Deprecated verbs** must be `tier: projection`
//! 3. **Intent verbs** cannot write to operational tables directly
//! 4. **Diagnostics verbs** must be read-only
//! 5. **Composite verbs** (materialize) may write to operational tables
//!
//! # Tiering Model
//!
//! ```text
//! Tier        | Source of Truth | Writes Operational | Internal
//! ------------|-----------------|--------------------|---------
//! reference   | catalog         | no                 | no
//! intent      | matrix          | no                 | no
//! projection  | operational     | yes                | yes (required)
//! diagnostics | any             | no                 | no
//! composite   | matrix          | yes (via pipeline) | no
//! ```

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
    // Rule 3: Intent verbs cannot write to operational tables
    // =========================================================================
    if matches!(metadata.tier, Some(VerbTier::Intent)) && metadata.writes_operational {
        diagnostics.add_error_with_path(
            codes::TIER_INTENT_WRITES_OPERATIONAL,
            "Intent tier verbs cannot write to operational tables directly",
            Some("metadata.writes_operational"),
            Some("Intent verbs modify the matrix document; use materialize to sync to operational tables"),
        );
    }

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
    if let Some(source) = &metadata.source_of_truth {
        match metadata.tier {
            Some(VerbTier::Intent)
                if !matches!(source, SourceOfTruth::Matrix | SourceOfTruth::Session) =>
            {
                diagnostics.add_warning_with_path(
                    codes::TIER_INCONSISTENT_SOURCE,
                    "Intent tier verbs should have source_of_truth: matrix or session",
                    Some("metadata.source_of_truth"),
                    Some("Intent verbs author the trading matrix or manage session state"),
                );
            }
            Some(VerbTier::Projection) if !matches!(source, SourceOfTruth::Operational) => {
                diagnostics.add_warning_with_path(
                    codes::TIER_INCONSISTENT_SOURCE,
                    "Projection tier verbs should have source_of_truth: operational",
                    Some("metadata.source_of_truth"),
                    Some("Projection verbs write to operational tables (derived from matrix)"),
                );
            }
            Some(VerbTier::Reference) if !matches!(source, SourceOfTruth::Catalog) => {
                diagnostics.add_warning_with_path(
                    codes::TIER_INCONSISTENT_SOURCE,
                    "Reference tier verbs should have source_of_truth: catalog",
                    Some("metadata.source_of_truth"),
                    Some("Reference verbs access global reference data"),
                );
            }
            _ => {}
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
