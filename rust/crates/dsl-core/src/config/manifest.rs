//! Verb manifest — flat, queryable registry of declared verbs
//!
//! `VerbManifest` is built by `ConfigLoader::load_verb_manifest()` from the
//! loaded YAML packs. It is the input to the wiring check (CR L2) that
//! compares YAML declarations against registered Rust `SemOsVerbOp`
//! implementations.
//!
//! # Relationship to existing types
//!
//! - `VerbsConfig` (types.rs) — raw YAML deserialization, nested by domain
//! - `ValidationReport` (validator.rs) — structural lint results
//! - `VerbManifest` (this file) — flat FQN-keyed summary for wiring check

use std::collections::HashMap;
use std::path::PathBuf;

use super::types::{ReturnTypeConfig, VerbBehavior, VerbsConfig};

/// A single declared verb extracted from YAML pack files.
#[derive(Debug, Clone)]
pub struct VerbDeclaration {
    /// Fully-qualified name: `domain.action` (e.g. `"cbu.create"`)
    pub fqn: String,
    /// Domain component (e.g. `"cbu"`)
    pub domain: String,
    /// Action component (e.g. `"create"`)
    pub action: String,
    /// Execution strategy declared in YAML
    pub behavior: VerbBehavior,
    /// Names of required arguments (`required: true`)
    pub required_args: Vec<String>,
    /// Phase tags from YAML metadata (e.g. `["kyc"]`, `["trading"]`)
    pub phase_tags: Vec<String>,
    /// Return type declared in YAML
    pub returns_type: ReturnTypeConfig,
    /// Source file path (relative to config dir), if known
    pub source_file: Option<PathBuf>,
}

/// A structured load error produced while building the manifest.
#[derive(Debug, Clone)]
pub struct ManifestError {
    /// Verb FQN affected, if identifiable
    pub fqn: Option<String>,
    /// Source file, if identifiable
    pub file: Option<PathBuf>,
    /// Field path within the declaration (e.g. `"args[0].type"`)
    pub field: Option<String>,
    /// Human-readable description of the problem
    pub message: String,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.fqn, &self.file) {
            (Some(fqn), Some(file)) => write!(f, "[{}] {} — {}", fqn, file.display(), self.message),
            (Some(fqn), None) => write!(f, "[{}] {}", fqn, self.message),
            (None, Some(file)) => write!(f, "[{}] {}", file.display(), self.message),
            (None, None) => write!(f, "{}", self.message),
        }
    }
}

/// Flat, FQN-keyed registry of all declared verbs loaded from YAML packs.
///
/// Built by `ConfigLoader::load_verb_manifest()`. Consumed by:
/// - CR L2 wiring check: compare against registered `SemOsVerbOp` FQNs
/// - LSP diagnostics: validate verb existence without hitting runtime_registry
/// - `rehydrate()` operation: build a fresh manifest and diff against prior state
#[derive(Debug, Default)]
pub struct VerbManifest {
    /// Successfully parsed verb declarations, indexed by FQN.
    pub declarations: HashMap<String, VerbDeclaration>,
    /// Errors encountered while building the manifest.
    /// Non-empty means the manifest is partial — some verbs may be missing.
    pub errors: Vec<ManifestError>,
}

impl VerbManifest {
    /// True when no errors occurred during manifest construction.
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }

    /// All declared FQNs (regardless of errors on other verbs).
    pub fn fqns(&self) -> impl Iterator<Item = &str> {
        self.declarations.keys().map(String::as_str)
    }

    /// Look up a declaration by FQN.
    pub fn get(&self, fqn: &str) -> Option<&VerbDeclaration> {
        self.declarations.get(fqn)
    }

    /// Total number of successfully declared verbs.
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// True when no verbs were declared.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

/// Build a `VerbManifest` from a loaded `VerbsConfig`.
///
/// Iterates all domains and verbs in the config, producing one
/// `VerbDeclaration` per verb. Structural validation errors from
/// `validate_verbs_config` are forwarded as `ManifestError`s.
pub fn build_manifest(config: &VerbsConfig) -> VerbManifest {
    let mut manifest = VerbManifest::default();

    for (domain_name, domain_config) in &config.domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            let fqn = format!("{}.{}", domain_name, verb_name);

            let required_args = verb_config
                .args
                .iter()
                .filter(|a| a.required)
                .map(|a| a.name.clone())
                .collect();

            let phase_tags = verb_config
                .metadata
                .as_ref()
                .map(|m| m.phase_tags.clone())
                .unwrap_or_default();

            let returns_type = verb_config
                .returns
                .as_ref()
                .map(|r| r.return_type)
                .unwrap_or(ReturnTypeConfig::Void);

            manifest.declarations.insert(
                fqn.clone(),
                VerbDeclaration {
                    fqn,
                    domain: domain_name.clone(),
                    action: verb_name.clone(),
                    behavior: verb_config.behavior,
                    required_args,
                    phase_tags,
                    returns_type,
                    source_file: None, // populated by load_verb_manifest when file is known
                },
            );
        }
    }

    manifest
}

/// Result of comparing YAML declarations against registered implementations.
///
/// Produced by `wiring_check`. A clean report means every `behavior: plugin`
/// verb in YAML has a registered `SemOsVerbOp`, and every registered op has a
/// corresponding YAML declaration (of any behavior).
#[derive(Debug, Default)]
pub struct WiringReport {
    /// `behavior: plugin` verbs declared in YAML with no registered implementation.
    /// These verbs would produce "unknown verb" errors at execution time.
    pub unimplemented_declarations: Vec<String>,
    /// Registered ops with no matching YAML declaration.
    /// These ops can never be invoked through the DSL pipeline.
    pub orphan_implementations: Vec<String>,
}

impl WiringReport {
    /// True when there are no mismatches in either direction.
    pub fn is_clean(&self) -> bool {
        self.unimplemented_declarations.is_empty() && self.orphan_implementations.is_empty()
    }

    /// Human-readable summary for startup logs or error messages.
    pub fn summary(&self) -> String {
        if self.is_clean() {
            return "wiring check: clean".to_string();
        }
        let mut parts = Vec::new();
        if !self.unimplemented_declarations.is_empty() {
            parts.push(format!(
                "{} plugin verb(s) have no registered impl: {}",
                self.unimplemented_declarations.len(),
                self.unimplemented_declarations.join(", ")
            ));
        }
        if !self.orphan_implementations.is_empty() {
            parts.push(format!(
                "{} registered op(s) have no YAML declaration: {}",
                self.orphan_implementations.len(),
                self.orphan_implementations.join(", ")
            ));
        }
        parts.join("; ")
    }
}

/// Compare YAML declarations against registered implementations.
///
/// `registered_fqns` is the sorted list of FQNs from the consumer's
/// `SemOsVerbOpRegistry` (e.g. via `registry.manifest()`).
///
/// - **Unimplemented declarations:** `behavior: plugin` verbs in YAML that are
///   absent from `registered_fqns`. They will fail at execution time with
///   "unknown verb".
/// - **Orphan implementations:** FQNs in `registered_fqns` with no corresponding
///   YAML verb (regardless of behavior). They can never be invoked through the
///   DSL pipeline.
///
/// CRUD verbs are excluded from the unimplemented check because they are
/// dispatched automatically by `GenericCrudExecutor` without a Rust impl.
pub fn wiring_check(manifest: &VerbManifest, registered_fqns: &[impl AsRef<str>]) -> WiringReport {
    use std::collections::HashSet;

    let registered: HashSet<&str> = registered_fqns.iter().map(|s| s.as_ref()).collect();

    // Plugin verbs declared in YAML that have no registered impl
    let mut unimplemented: Vec<String> = manifest
        .declarations
        .values()
        .filter(|d| matches!(d.behavior, VerbBehavior::Plugin))
        .map(|d| d.fqn.as_str())
        .filter(|fqn| !registered.contains(fqn))
        .map(str::to_string)
        .collect();
    unimplemented.sort();

    // Registered ops with no YAML declaration at all
    let mut orphan: Vec<String> = registered_fqns
        .iter()
        .map(|s| s.as_ref())
        .filter(|fqn| manifest.get(fqn).is_none())
        .map(str::to_string)
        .collect();
    orphan.sort();

    WiringReport {
        unimplemented_declarations: unimplemented,
        orphan_implementations: orphan,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::loader::ConfigLoader;

    #[test]
    fn load_verb_manifest_loads_ob_poc_packs() {
        let loader = ConfigLoader::from_env();
        let manifest = loader.load_verb_manifest();

        // ob-poc has 1,200+ verbs; any reasonable subset confirms load worked
        assert!(
            manifest.len() > 100,
            "manifest should have >100 verbs, got {}",
            manifest.len()
        );

        // Known verbs that must exist
        assert!(
            manifest.get("cbu.ensure").is_some(),
            "cbu.ensure must be declared"
        );
        assert!(
            manifest.get("kyc-case.update-status").is_some(),
            "kyc-case.update-status must be declared"
        );

        // The three pre-existing structural errors (bpmn-controller, loader verbs)
        // are documented as pre-existing; don't assert is_clean() here.
        // Assert no LOAD errors (yaml parse failures) — only validation warnings ok.
        let load_errors: Vec<_> = manifest
            .errors
            .iter()
            .filter(|e| e.message.contains("Failed to load"))
            .collect();
        assert!(
            load_errors.is_empty(),
            "No YAML load errors expected: {:?}",
            load_errors
        );
    }

    #[test]
    fn verb_declaration_has_expected_fields() {
        let loader = ConfigLoader::from_env();
        let manifest = loader.load_verb_manifest();

        let cbu_ensure = manifest.get("cbu.ensure").expect("cbu.ensure must exist");
        assert_eq!(cbu_ensure.domain, "cbu");
        assert_eq!(cbu_ensure.action, "ensure");
        // cbu.ensure is a crud verb (upsert by natural key)
        assert!(
            matches!(
                cbu_ensure.behavior,
                crate::config::types::VerbBehavior::Crud
            ),
            "cbu.ensure should be crud behavior, got {:?}",
            cbu_ensure.behavior
        );
    }

    #[test]
    fn fqns_iterator_covers_all_declarations() {
        let loader = ConfigLoader::from_env();
        let manifest = loader.load_verb_manifest();

        let fqn_count = manifest.fqns().count();
        assert_eq!(fqn_count, manifest.len(), "fqns() count must match len()");
    }
}

/// Build a `VerbManifest` from a loaded `VerbsConfig`, forwarding
/// structural validation errors from `validate_verbs_config`.
///
/// Each `StructuralError` message already embeds the verb FQN and field
/// path via its `Display` impl, so they map cleanly to `ManifestError`.
pub fn build_manifest_with_validation(
    config: &VerbsConfig,
    report: &super::validator::ValidationReport,
) -> VerbManifest {
    let mut manifest = build_manifest(config);

    for err in &report.structural {
        manifest.errors.push(ManifestError {
            fqn: None, // embedded in message via StructuralError::Display
            file: None,
            field: None,
            message: err.to_string(),
        });
    }

    manifest
}
