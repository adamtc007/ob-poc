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
use serde::{Deserialize, Serialize};

use super::verb_contract::{codes, VerbDiagnostics};

// =============================================================================
// LINT TIERS (Buf-style graduated enforcement)
// =============================================================================

/// Lint enforcement tier - controls which rules are applied
///
/// Modeled after Buf's MINIMAL/BASIC/STANDARD tiers for gradual adoption.
/// Each tier includes all rules from lower tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintTier {
    /// Required fields present (metadata block, tier, source_of_truth)
    /// Warns only - doesn't block
    #[default]
    Minimal,
    /// Naming conventions + semantic consistency + deprecation coherence
    /// Errors block execution
    Basic,
    /// Full matrix-first enforcement, single authoring surface
    /// Strictest - used in CI gates
    Standard,
}

impl LintTier {
    /// Check if this tier includes the given rule tier
    pub fn includes(&self, rule_tier: LintTier) -> bool {
        match self {
            LintTier::Minimal => matches!(rule_tier, LintTier::Minimal),
            LintTier::Basic => matches!(rule_tier, LintTier::Minimal | LintTier::Basic),
            LintTier::Standard => true,
        }
    }

    /// Get the tier name for display
    pub fn as_str(&self) -> &'static str {
        match self {
            LintTier::Minimal => "minimal",
            LintTier::Basic => "basic",
            LintTier::Standard => "standard",
        }
    }
}

impl std::fmt::Display for LintTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for LintTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "minimal" => Ok(LintTier::Minimal),
            "basic" => Ok(LintTier::Basic),
            "standard" => Ok(LintTier::Standard),
            _ => Err(format!(
                "Unknown lint tier: '{}'. Valid tiers: minimal, basic, standard",
                s
            )),
        }
    }
}

/// Configuration for the verb linter
#[derive(Debug, Clone)]
pub struct LintConfig {
    /// Which tier of rules to enforce
    pub tier: LintTier,
    /// Treat warnings as errors (for CI strictness)
    pub fail_on_warning: bool,
    /// Only show verbs with issues (hide clean verbs)
    pub issues_only: bool,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            tier: LintTier::Minimal,
            fail_on_warning: false,
            issues_only: true,
        }
    }
}

impl LintConfig {
    /// Create config for CI gate (strictest)
    pub fn ci() -> Self {
        Self {
            tier: LintTier::Standard,
            fail_on_warning: true,
            issues_only: true,
        }
    }

    /// Create config for development (lenient)
    pub fn dev() -> Self {
        Self {
            tier: LintTier::Minimal,
            fail_on_warning: false,
            issues_only: true,
        }
    }
}

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
///
/// Uses default LintConfig (MINIMAL tier). For tier-specific linting, use `lint_verb_with_config`.
pub fn lint_verb_tiering(domain: &str, verb_name: &str, config: &VerbConfig) -> VerbLintResult {
    lint_verb_with_config(domain, verb_name, config, &LintConfig::default())
}

/// Lint a single verb with explicit configuration
pub fn lint_verb_with_config(
    domain: &str,
    verb_name: &str,
    config: &VerbConfig,
    lint_config: &LintConfig,
) -> VerbLintResult {
    let full_name = format!("{}.{}", domain, verb_name);
    let mut diagnostics = VerbDiagnostics::default();

    // =========================================================================
    // MINIMAL TIER RULES (M001-M006) - Always run
    // =========================================================================
    if lint_config.tier.includes(LintTier::Minimal) {
        check_minimal_rules(domain, verb_name, config, &mut diagnostics);
    }

    // If no metadata, we can't check higher-tier rules
    let Some(metadata) = &config.metadata else {
        return VerbLintResult {
            full_name,
            diagnostics,
        };
    };

    // Determine if this verb writes (based on behavior)
    let is_write_behavior = match config.behavior {
        VerbBehavior::Crud => config
            .crud
            .as_ref()
            .map(|c| is_crud_write_operation(&c.operation))
            .unwrap_or(false),
        VerbBehavior::Plugin => metadata.writes_operational,
        VerbBehavior::GraphQuery => false,
    };

    // =========================================================================
    // BASIC TIER RULES (B001-B006) - Naming + semantics
    // =========================================================================
    if lint_config.tier.includes(LintTier::Basic) {
        check_basic_rules(domain, verb_name, config, metadata, &mut diagnostics);
    }

    // =========================================================================
    // STANDARD TIER RULES (S001-S003) - Matrix-first enforcement
    // These are checked in lint_all_verbs_with_config for cross-verb analysis
    // Here we only check single-verb standard rules
    // =========================================================================
    if lint_config.tier.includes(LintTier::Standard) {
        check_standard_single_verb_rules(metadata, is_write_behavior, &mut diagnostics);
    }

    // =========================================================================
    // Legacy rules (always run for backward compat)
    // =========================================================================
    check_legacy_rules(metadata, is_write_behavior, &mut diagnostics);

    VerbLintResult {
        full_name,
        diagnostics,
    }
}

/// MINIMAL tier rules (M001-M006): Required fields present
fn check_minimal_rules(
    domain: &str,
    verb_name: &str,
    config: &VerbConfig,
    diagnostics: &mut VerbDiagnostics,
) {
    let full_name = format!("{}.{}", domain, verb_name);

    // M001: metadata block present
    let Some(metadata) = &config.metadata else {
        diagnostics.add_warning_with_path(
            codes::M001_MISSING_METADATA,
            &format!("{} missing metadata block", full_name),
            Some("metadata"),
            Some("Add metadata: { tier: ..., source_of_truth: ..., scope: ..., noun: ... }"),
        );
        return; // Can't check other MINIMAL rules without metadata
    };

    // M002: tier field present
    if metadata.tier.is_none() {
        diagnostics.add_warning_with_path(
            codes::M002_MISSING_TIER,
            &format!("{} missing metadata.tier", full_name),
            Some("metadata.tier"),
            Some("Add tier: reference|intent|projection|diagnostics|composite"),
        );
    }

    // M003: source_of_truth field present
    if metadata.source_of_truth.is_none() {
        diagnostics.add_warning_with_path(
            codes::M003_MISSING_SOURCE,
            &format!("{} missing metadata.source_of_truth", full_name),
            Some("metadata.source_of_truth"),
            Some("Add source_of_truth: matrix|catalog|operational|session|entity|workflow|external|register|document"),
        );
    }

    // M004: scope field present
    if metadata.scope.is_none() {
        diagnostics.add_warning_with_path(
            codes::M004_MISSING_SCOPE,
            &format!("{} missing metadata.scope", full_name),
            Some("metadata.scope"),
            Some("Add scope: global|cbu"),
        );
    }

    // M005: noun field present
    if metadata.noun.is_none() {
        diagnostics.add_warning_with_path(
            codes::M005_MISSING_NOUN,
            &format!("{} missing metadata.noun", full_name),
            Some("metadata.noun"),
            Some("Add noun: <domain_object> (e.g., trading_matrix, ssi, entity, kyc_case)"),
        );
    }

    // M006: deprecated verbs must have replaced_by
    if matches!(
        metadata.status,
        dsl_core::config::types::VerbStatus::Deprecated
    ) && metadata.replaced_by.is_none()
    {
        diagnostics.add_warning_with_path(
            codes::M006_DEPRECATED_NO_REPLACEMENT,
            &format!("{} is deprecated but missing replaced_by", full_name),
            Some("metadata.replaced_by"),
            Some("Add replaced_by: 'domain.verb-name' pointing to the canonical replacement"),
        );
    }
}

/// BASIC tier rules (B001-B006): Naming conventions + semantics
fn check_basic_rules(
    domain: &str,
    verb_name: &str,
    config: &VerbConfig,
    metadata: &dsl_core::config::types::VerbMetadata,
    diagnostics: &mut VerbDiagnostics,
) {
    let full_name = format!("{}.{}", domain, verb_name);

    // B001: create-* verbs should use insert operation
    if verb_name.starts_with("create-") {
        if let Some(crud) = &config.crud {
            if !matches!(crud.operation, CrudOperation::Insert) {
                diagnostics.add_error_with_path(
                    codes::B001_CREATE_NOT_INSERT,
                    &format!(
                        "{} uses 'create-' prefix but operation is {:?}, expected insert",
                        full_name, crud.operation
                    ),
                    Some("crud.operation"),
                    Some("Use 'ensure-' prefix for upsert, or change operation to insert"),
                );
            }
        }
    }

    // B002: ensure-* verbs should use upsert operation
    if verb_name.starts_with("ensure-") {
        if let Some(crud) = &config.crud {
            if !matches!(crud.operation, CrudOperation::Upsert) {
                diagnostics.add_error_with_path(
                    codes::B002_ENSURE_NOT_UPSERT,
                    &format!(
                        "{} uses 'ensure-' prefix but operation is {:?}, expected upsert",
                        full_name, crud.operation
                    ),
                    Some("crud.operation"),
                    Some("Use 'create-' prefix for insert, or change operation to upsert"),
                );
            }
        }
    }

    // B003: delete-* on regulated nouns requires dangerous: true
    if verb_name.starts_with("delete-") || verb_name.starts_with("remove-") {
        const REGULATED_NOUNS: &[&str] = &[
            "entity",
            "cbu",
            "kyc_case",
            "investor",
            "holding",
            "fund",
            "share_class",
        ];
        if let Some(noun) = &metadata.noun {
            if REGULATED_NOUNS.contains(&noun.as_str()) && !metadata.dangerous {
                diagnostics.add_error_with_path(
                    codes::B003_DELETE_NOT_DANGEROUS,
                    &format!(
                        "{} deletes regulated noun '{}' but missing dangerous: true",
                        full_name, noun
                    ),
                    Some("metadata.dangerous"),
                    Some("Add dangerous: true for delete operations on regulated nouns"),
                );
            }
        }
    }

    // B005: list-* verbs should be tier: diagnostics
    if verb_name.starts_with("list-") && !matches!(metadata.tier, Some(VerbTier::Diagnostics)) {
        diagnostics.add_warning_with_path(
            codes::B005_LIST_NOT_DIAGNOSTICS,
            &format!(
                "{} uses 'list-' prefix but tier is not diagnostics",
                full_name
            ),
            Some("metadata.tier"),
            Some("List verbs are read-only, use tier: diagnostics"),
        );
    }

    // B006: get-* verbs should be tier: diagnostics
    if verb_name.starts_with("get-") && !matches!(metadata.tier, Some(VerbTier::Diagnostics)) {
        diagnostics.add_warning_with_path(
            codes::B006_GET_NOT_DIAGNOSTICS,
            &format!(
                "{} uses 'get-' prefix but tier is not diagnostics",
                full_name
            ),
            Some("metadata.tier"),
            Some("Get verbs are read-only, use tier: diagnostics"),
        );
    }
}

/// STANDARD tier single-verb rules (cross-verb rules checked in lint_all_verbs)
fn check_standard_single_verb_rules(
    metadata: &dsl_core::config::types::VerbMetadata,
    _is_write_behavior: bool,
    diagnostics: &mut VerbDiagnostics,
) {
    // S002: writes_operational requires tier: projection or composite
    if metadata.writes_operational
        && !matches!(
            metadata.tier,
            Some(VerbTier::Projection) | Some(VerbTier::Composite)
        )
    {
        diagnostics.add_error_with_path(
            codes::S002_WRITES_OP_WRONG_TIER,
            "writes_operational: true requires tier: projection or composite",
            Some("metadata.tier"),
            Some("Only projection and composite verbs may write to operational tables"),
        );
    }

    // S003: projection + writes_operational requires internal: true
    if matches!(metadata.tier, Some(VerbTier::Projection))
        && metadata.writes_operational
        && !metadata.internal
    {
        diagnostics.add_error_with_path(
            codes::S003_PROJECTION_NOT_INTERNAL,
            "Projection tier with writes_operational requires internal: true",
            Some("metadata.internal"),
            Some("Projection verbs are internal implementation details, not user-facing"),
        );
    }
}

/// Legacy rules for backward compatibility (always run regardless of tier)
fn check_legacy_rules(
    metadata: &dsl_core::config::types::VerbMetadata,
    is_write_behavior: bool,
    diagnostics: &mut VerbDiagnostics,
) {
    // Check if tagged as deprecated (legacy tag-based deprecation)
    let is_deprecated_tag = metadata.tags.iter().any(|t| t == "deprecated");

    // T006: Diagnostics verbs must be read-only
    if matches!(metadata.tier, Some(VerbTier::Diagnostics)) && is_write_behavior {
        diagnostics.add_error_with_path(
            codes::TIER_DIAGNOSTICS_HAS_WRITE,
            "Diagnostics tier verbs must be read-only",
            Some("behavior"),
            Some("Diagnostics verbs should use select/list operations, not insert/update/delete"),
        );
    }

    // T002: Projection verbs must be internal
    if matches!(metadata.tier, Some(VerbTier::Projection)) && !metadata.internal {
        diagnostics.add_error_with_path(
            codes::TIER_PROJECTION_NOT_INTERNAL,
            "Projection tier verbs must be internal: true",
            Some("metadata.internal"),
            Some("Add 'internal: true' - projection verbs are only called by materialize pipeline"),
        );
    }

    // T003: Deprecated tag verbs should be tier: projection
    if is_deprecated_tag && !matches!(metadata.tier, Some(VerbTier::Projection)) {
        diagnostics.add_warning_with_path(
            codes::TIER_DEPRECATED_NOT_PROJECTION,
            "Deprecated verbs should be tier: projection",
            Some("metadata.tier"),
            Some("Set tier: projection for deprecated verbs that write to operational tables"),
        );
    }

    // T004: writes_operational should match behavior
    if metadata.writes_operational && !is_write_behavior {
        diagnostics.add_warning_with_path(
            codes::TIER_WRITES_OP_MISMATCH,
            "writes_operational: true but behavior appears read-only",
            Some("metadata.writes_operational"),
            Some("Either remove writes_operational or verify the plugin handler writes to DB"),
        );
    }

    // T001: Verbs that write but aren't projection/composite (only if writes_operational is set)
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

    // T008: Projection verbs should have source_of_truth: operational
    if let Some(source) = &metadata.source_of_truth {
        if matches!(metadata.tier, Some(VerbTier::Projection))
            && !matches!(source, SourceOfTruth::Operational)
        {
            diagnostics.add_warning_with_path(
                codes::TIER_INCONSISTENT_SOURCE,
                "Projection tier verbs should have source_of_truth: operational",
                Some("metadata.source_of_truth"),
                Some("Projection verbs write to operational tables (derived from a canonical source)"),
            );
        }
    }
}

/// Lint all verbs from a domain configuration map (uses default LintConfig)
pub fn lint_all_verbs(
    domains: &std::collections::HashMap<String, dsl_core::config::types::DomainConfig>,
) -> LintReport {
    lint_all_verbs_with_config(domains, &LintConfig::default())
}

/// Lint all verbs with explicit configuration
pub fn lint_all_verbs_with_config(
    domains: &std::collections::HashMap<String, dsl_core::config::types::DomainConfig>,
    config: &LintConfig,
) -> LintReport {
    let mut report = LintReport::default();

    // First pass: collect all verbs and run single-verb rules
    for (domain_name, domain_config) in domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            report.total_verbs += 1;

            let result = lint_verb_with_config(domain_name, verb_name, verb_config, config);

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

    // Second pass: cross-verb STANDARD rules (S001)
    if config.tier.includes(LintTier::Standard) {
        check_cross_verb_standard_rules(domains, &mut report);
    }

    report
}

/// Check STANDARD tier rules that require cross-verb analysis
fn check_cross_verb_standard_rules(
    domains: &std::collections::HashMap<String, dsl_core::config::types::DomainConfig>,
    report: &mut LintReport,
) {
    use std::collections::HashMap;

    // S001: Single authoring surface - only one intent verb per noun with source_of_truth: matrix
    // Build map of noun -> list of (full_name, source_of_truth) for intent verbs
    let mut intent_verbs_by_noun: HashMap<String, Vec<(String, Option<SourceOfTruth>)>> =
        HashMap::new();

    for (domain_name, domain_config) in domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            if let Some(metadata) = &verb_config.metadata {
                if matches!(metadata.tier, Some(VerbTier::Intent)) {
                    if let Some(noun) = &metadata.noun {
                        let full_name = format!("{}.{}", domain_name, verb_name);
                        intent_verbs_by_noun
                            .entry(noun.clone())
                            .or_default()
                            .push((full_name, metadata.source_of_truth));
                    }
                }
            }
        }
    }

    // Check for conflicts: multiple intent verbs for same noun where one is matrix source
    for (noun, verbs) in &intent_verbs_by_noun {
        let matrix_verbs: Vec<_> = verbs
            .iter()
            .filter(|(_, src)| matches!(src, Some(SourceOfTruth::Matrix)))
            .collect();

        if !matrix_verbs.is_empty() && verbs.len() > 1 {
            // There's a matrix verb and other intent verbs for same noun
            let matrix_names: Vec<_> = matrix_verbs.iter().map(|(n, _)| n.as_str()).collect();
            let other_names: Vec<_> = verbs
                .iter()
                .filter(|(_, src)| !matches!(src, Some(SourceOfTruth::Matrix)))
                .map(|(n, _)| n.as_str())
                .collect();

            if !other_names.is_empty() {
                // Add warning to the non-matrix verbs
                for other_name in &other_names {
                    // Find the result for this verb and add a diagnostic
                    for result in &mut report.results {
                        if result.full_name == *other_name {
                            result.diagnostics.add_warning_with_path(
                                codes::S001_DUPLICATE_INTENT,
                                &format!(
                                    "Multiple intent verbs for noun '{}': {} is matrix source, {} should use it",
                                    noun,
                                    matrix_names.join(", "),
                                    other_name
                                ),
                                Some("metadata.source_of_truth"),
                                Some("Use the matrix verb for authoring, or mark this verb as tier: diagnostics"),
                            );
                            if !result.has_errors() {
                                report.verbs_with_warnings += 1;
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
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
                table: Some("test".to_string()),
                schema: Some("ob-poc".to_string()),
                key: None,
                returning: None,
                conflict_keys: None,
                conflict_constraint: None,
                junction: None,
                from_col: None,
                to_col: None,
                role_table: None,
                role_col: None,
                fk_col: None,
                filter_col: None,
                primary_table: None,
                join_table: None,
                join_col: None,
                base_table: None,
                extension_table: None,
                extension_table_column: None,
                type_id_column: None,
                type_code: None,
                order_by: None,
                set_values: None,
            }),
            handler: None,
            graph_query: None,
            args: vec![],
            returns: None,
            produces: None,
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec![],
            policy: None,
        }
    }

    fn make_crud_select_config() -> VerbConfig {
        VerbConfig {
            description: "Test verb".to_string(),
            behavior: VerbBehavior::Crud,
            crud: Some(CrudConfig {
                operation: CrudOperation::Select,
                table: Some("test".to_string()),
                schema: Some("ob-poc".to_string()),
                key: None,
                returning: None,
                conflict_keys: None,
                conflict_constraint: None,
                junction: None,
                from_col: None,
                to_col: None,
                role_table: None,
                role_col: None,
                fk_col: None,
                filter_col: None,
                primary_table: None,
                join_table: None,
                join_col: None,
                base_table: None,
                extension_table: None,
                extension_table_column: None,
                type_id_column: None,
                type_code: None,
                order_by: None,
                set_values: None,
            }),
            handler: None,
            graph_query: None,
            args: vec![],
            returns: None,
            produces: None,
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec![],
            policy: None,
        }
    }

    fn make_plugin_config() -> VerbConfig {
        VerbConfig {
            description: "Test verb".to_string(),
            behavior: VerbBehavior::Plugin,
            crud: None,
            handler: Some("test_handler".to_string()),
            graph_query: None,
            args: vec![],
            returns: None,
            produces: None,
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec![],
            policy: None,
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
    fn test_writes_operational_requires_projection_or_composite() {
        // Intent verb that writes_operational should error (must be projection or composite)
        let mut config = make_plugin_config();
        config.metadata = Some(VerbMetadata {
            tier: Some(VerbTier::Intent),
            source_of_truth: Some(SourceOfTruth::Matrix),
            writes_operational: true, // ERROR: intent can't write operational
            ..Default::default()
        });

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .errors
            .iter()
            .any(|e| e.code == codes::TIER_WRITE_NOT_PROJECTION));
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
        let mut config = make_plugin_config();
        config.metadata = Some(VerbMetadata {
            tier: Some(VerbTier::Intent),
            source_of_truth: Some(SourceOfTruth::Matrix),
            writes_operational: false,
            ..Default::default()
        });

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(!result.has_errors());
    }

    #[test]
    fn test_valid_diagnostics_verb() {
        let mut config = make_crud_select_config();
        config.metadata = Some(VerbMetadata {
            tier: Some(VerbTier::Diagnostics),
            source_of_truth: Some(SourceOfTruth::Operational),
            ..Default::default()
        });

        let result = lint_verb_tiering("test", "verb", &config);
        assert!(!result.has_errors());
    }
}
