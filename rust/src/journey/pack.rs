//! Pack Manifest Types and YAML Loader
//!
//! A Journey Pack is the product-level interface between the user and the
//! platform. It defines structured, versioned workflows with question policy,
//! allowed verbs, templates, and definition-of-done.
//!
//! # Canonical Hashing
//!
//! `manifest_hash()` hashes the **raw YAML file bytes** (not serde
//! re-serialization). This guarantees determinism regardless of serde_yaml
//! version or map ordering quirks.
//!
//! # Conversation-First
//!
//! `options_source` on `PackQuestion` is a **suggestions vocabulary only** —
//! for optional UI affordances (autocomplete, chips). The orchestrator MUST
//! NOT gate correctness on picker/dropdown selection.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// PackManifest (top-level)
// ---------------------------------------------------------------------------

/// A Journey Pack manifest loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,

    /// Phrases that trigger routing to this pack.
    #[serde(default)]
    pub invocation_phrases: Vec<String>,

    /// Context fields that MUST be set before the pack can start.
    #[serde(default)]
    pub required_context: Vec<String>,

    /// Context fields that are useful but not blocking.
    #[serde(default)]
    pub optional_context: Vec<String>,

    /// Verbs this pack is allowed to use.
    #[serde(default)]
    pub allowed_verbs: Vec<String>,

    /// Verbs this pack must never use.
    #[serde(default)]
    pub forbidden_verbs: Vec<String>,

    /// Risk policy for execution confirmation.
    #[serde(default)]
    pub risk_policy: RiskPolicy,

    /// Questions the pack asks the user (required).
    #[serde(default)]
    pub required_questions: Vec<PackQuestion>,

    /// Questions the pack may ask (optional, depending on context).
    #[serde(default)]
    pub optional_questions: Vec<PackQuestion>,

    /// Conditions that signal the pack's work is done.
    #[serde(default)]
    pub stop_rules: Vec<String>,

    /// Parameterised step templates.
    #[serde(default)]
    pub templates: Vec<PackTemplate>,

    /// Handlebars-style summary template for runbook playback.
    pub pack_summary_template: Option<String>,

    /// UI section layout for runbook display.
    #[serde(default)]
    pub section_layout: Vec<SectionLayout>,

    /// Acceptance criteria for the pack.
    #[serde(default)]
    pub definition_of_done: Vec<String>,

    /// Observable signals for progress tracking.
    #[serde(default)]
    pub progress_signals: Vec<ProgressSignal>,
}

// ---------------------------------------------------------------------------
// Risk Policy
// ---------------------------------------------------------------------------

/// Controls when and how the user must confirm before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPolicy {
    /// If true, always ask before executing the runbook.
    #[serde(default = "default_true")]
    pub require_confirm_before_execute: bool,

    /// Maximum steps allowed without an intermediate confirmation.
    #[serde(default = "default_max_steps")]
    pub max_steps_without_confirm: u32,
}

impl Default for RiskPolicy {
    fn default() -> Self {
        Self {
            require_confirm_before_execute: true,
            max_steps_without_confirm: 10,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_steps() -> u32 {
    10
}

// ---------------------------------------------------------------------------
// PackQuestion
// ---------------------------------------------------------------------------

/// A question the pack asks the user during the InPack Q/A phase.
///
/// `options_source` is **suggestions vocabulary only** — the orchestrator
/// MUST NOT gate correctness on picker/dropdown selection. All answers are
/// accepted as free-text and validated after the fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackQuestion {
    /// Slot key this answer populates (e.g. "products", "trading_matrix").
    pub field: String,

    /// Human-readable question text.
    pub prompt: String,

    /// Expected answer shape.
    #[serde(default)]
    pub answer_kind: AnswerKind,

    /// Suggestions vocabulary — for UI autocomplete / chips only.
    /// Never gates correctness.
    pub options_source: Option<String>,

    /// Default value if the user skips this question.
    pub default: Option<serde_json::Value>,

    /// Condition expression — only ask when this evaluates to true.
    pub ask_when: Option<String>,
}

/// The shape of an expected answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerKind {
    #[default]
    String,
    Boolean,
    List,
    EntityRef,
    Enum,
}

// ---------------------------------------------------------------------------
// PackTemplate
// ---------------------------------------------------------------------------

/// A parameterised step template within a pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackTemplate {
    pub template_id: String,

    /// Human description of when to use this template.
    pub when_to_use: String,

    /// Ordered steps in the template.
    pub steps: Vec<TemplateStep>,
}

/// A single step in a pack template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStep {
    /// Fully-qualified verb name (e.g. "cbu.create").
    pub verb: String,

    /// Static or slot-referenced arguments.
    #[serde(default)]
    pub args: std::collections::HashMap<String, serde_json::Value>,

    /// If set, repeat this step for each item in the named list slot.
    pub repeat_for: Option<String>,

    /// Condition — only include this step when this expression is true.
    pub when: Option<String>,

    /// How to execute this step (overrides pack default).
    pub execution_mode: Option<String>,
}

// ---------------------------------------------------------------------------
// Section Layout
// ---------------------------------------------------------------------------

/// Controls how runbook entries are grouped for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionLayout {
    pub title: String,
    #[serde(default)]
    pub verb_prefixes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Progress Signal
// ---------------------------------------------------------------------------

/// An observable signal the pack emits to indicate progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressSignal {
    pub signal: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Canonical Hashing
// ---------------------------------------------------------------------------

impl PackManifest {
    /// Deterministic hash of the raw YAML file bytes.
    ///
    /// We hash the **original file bytes**, NOT a serde re-serialization.
    /// This guarantees: same file bytes ⇒ same hash, always, regardless
    /// of serde_yaml version or map key ordering.
    pub fn manifest_hash(raw_yaml_bytes: &[u8]) -> String {
        let hash = Sha256::digest(raw_yaml_bytes);
        format!("{:x}", hash)
    }
}

// ---------------------------------------------------------------------------
// Pack Loader (two-pass for canonical hashing)
// ---------------------------------------------------------------------------

/// Load a single pack manifest from a YAML file.
///
/// Two-pass approach:
/// 1. Read raw bytes → compute canonical hash.
/// 2. Deserialize YAML → typed `PackManifest`.
///
/// Returns `(manifest, hash)`.
pub fn load_pack_from_file(path: &Path) -> Result<(PackManifest, String), PackLoadError> {
    let raw_bytes = std::fs::read(path).map_err(|e| PackLoadError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let hash = PackManifest::manifest_hash(&raw_bytes);
    let manifest: PackManifest =
        serde_yaml::from_slice(&raw_bytes).map_err(|e| PackLoadError::Parse {
            path: path.display().to_string(),
            source: e,
        })?;
    Ok((manifest, hash))
}

/// Load a pack manifest from raw YAML bytes (useful for testing).
pub fn load_pack_from_bytes(raw_bytes: &[u8]) -> Result<(PackManifest, String), PackLoadError> {
    let hash = PackManifest::manifest_hash(raw_bytes);
    let manifest: PackManifest =
        serde_yaml::from_slice(raw_bytes).map_err(|e| PackLoadError::Parse {
            path: "<bytes>".to_string(),
            source: e,
        })?;
    Ok((manifest, hash))
}

/// Load all pack manifests from a directory (non-recursive).
///
/// Files must match `*.yaml` or `*.yml`.
pub fn load_packs_from_dir(dir: &Path) -> Result<Vec<(PackManifest, String)>, PackLoadError> {
    let mut packs = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| PackLoadError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| PackLoadError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "yaml" || ext == "yml" {
                packs.push(load_pack_from_file(&path)?);
            }
        }
    }
    // Sort by pack id for deterministic ordering.
    packs.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    Ok(packs)
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from pack loading.
#[derive(Debug)]
pub enum PackLoadError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse {
        path: String,
        source: serde_yaml::Error,
    },
}

impl std::fmt::Display for PackLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "IO error loading pack '{}': {}", path, source),
            Self::Parse { path, source } => {
                write!(f, "Parse error in pack '{}': {}", path, source)
            }
        }
    }
}

impl std::error::Error for PackLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_pack_yaml() -> &'static str {
        r#"
id: test-pack
name: Test Pack
version: "1.0"
description: A test pack
invocation_phrases:
  - "run the test"
  - "do the test thing"
required_context:
  - client_group_id
allowed_verbs:
  - cbu.create
  - cbu.assign-product
risk_policy:
  require_confirm_before_execute: true
  max_steps_without_confirm: 5
required_questions:
  - field: products
    prompt: "Which products should be added?"
    answer_kind: list
    options_source: "product_catalog"
  - field: jurisdiction
    prompt: "Which jurisdiction?"
    answer_kind: string
    default: "LU"
templates:
  - template_id: basic-onboarding
    when_to_use: "Standard onboarding flow"
    steps:
      - verb: cbu.create
        args:
          name: "{context.client_name}"
          jurisdiction: "{answers.jurisdiction}"
      - verb: cbu.assign-product
        repeat_for: "answers.products"
        args:
          product: "{item}"
definition_of_done:
  - "All products assigned"
  - "Trading matrix populated"
"#
    }

    #[test]
    fn test_deserialize_minimal_pack() {
        let yaml = minimal_pack_yaml();
        let (pack, _hash) = load_pack_from_bytes(yaml.as_bytes()).unwrap();

        assert_eq!(pack.id, "test-pack");
        assert_eq!(pack.name, "Test Pack");
        assert_eq!(pack.version, "1.0");
        assert_eq!(pack.invocation_phrases.len(), 2);
        assert_eq!(pack.required_context, vec!["client_group_id"]);
        assert_eq!(pack.allowed_verbs.len(), 2);
        assert!(pack.risk_policy.require_confirm_before_execute);
        assert_eq!(pack.risk_policy.max_steps_without_confirm, 5);
        assert_eq!(pack.required_questions.len(), 2);
        assert_eq!(pack.required_questions[0].field, "products");
        assert_eq!(pack.required_questions[0].answer_kind, AnswerKind::List);
        assert_eq!(
            pack.required_questions[0].options_source.as_deref(),
            Some("product_catalog")
        );
        assert_eq!(
            pack.required_questions[1].default,
            Some(serde_json::json!("LU"))
        );
        assert_eq!(pack.templates.len(), 1);
        assert_eq!(pack.templates[0].steps.len(), 2);
        assert_eq!(
            pack.templates[0].steps[1].repeat_for.as_deref(),
            Some("answers.products")
        );
        assert_eq!(pack.definition_of_done.len(), 2);
    }

    #[test]
    fn test_hash_stability() {
        let yaml = minimal_pack_yaml();
        let bytes = yaml.as_bytes();

        let hash1 = PackManifest::manifest_hash(bytes);
        let hash2 = PackManifest::manifest_hash(bytes);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_hash_changes_on_modification() {
        let yaml = minimal_pack_yaml();
        let hash_original = PackManifest::manifest_hash(yaml.as_bytes());

        let modified = yaml.replace("test-pack", "test-pack-modified");
        let hash_modified = PackManifest::manifest_hash(modified.as_bytes());

        assert_ne!(hash_original, hash_modified);
    }

    #[test]
    fn test_hash_is_raw_bytes_not_reserialization() {
        // Two YAMLs that deserialize to the same data but have different whitespace.
        let yaml_a = b"id: x\nname: X\nversion: '1'\ndescription: test\n";
        let yaml_b = b"id:  x\nname:  X\nversion:  '1'\ndescription:  test\n";

        let hash_a = PackManifest::manifest_hash(yaml_a);
        let hash_b = PackManifest::manifest_hash(yaml_b);

        // Raw-byte hashing: different bytes → different hashes,
        // even though serde would produce the same struct.
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn test_default_risk_policy() {
        let yaml = b"id: minimal\nname: Min\nversion: '1'\ndescription: d\n";
        let (pack, _) = load_pack_from_bytes(yaml).unwrap();

        assert!(pack.risk_policy.require_confirm_before_execute);
        assert_eq!(pack.risk_policy.max_steps_without_confirm, 10);
    }

    #[test]
    fn test_all_answer_kinds() {
        let yaml = r#"
id: kind-test
name: Kind Test
version: "1"
description: test
required_questions:
  - field: a
    prompt: q
    answer_kind: string
  - field: b
    prompt: q
    answer_kind: boolean
  - field: c
    prompt: q
    answer_kind: list
  - field: d
    prompt: q
    answer_kind: entity_ref
  - field: e
    prompt: q
    answer_kind: enum
"#;
        let (pack, _) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
        assert_eq!(pack.required_questions[0].answer_kind, AnswerKind::String);
        assert_eq!(pack.required_questions[1].answer_kind, AnswerKind::Boolean);
        assert_eq!(pack.required_questions[2].answer_kind, AnswerKind::List);
        assert_eq!(
            pack.required_questions[3].answer_kind,
            AnswerKind::EntityRef
        );
        assert_eq!(pack.required_questions[4].answer_kind, AnswerKind::Enum);
    }

    #[test]
    fn test_optional_fields_default_empty() {
        let yaml = b"id: bare\nname: Bare\nversion: '1'\ndescription: d\n";
        let (pack, _) = load_pack_from_bytes(yaml).unwrap();

        assert!(pack.invocation_phrases.is_empty());
        assert!(pack.required_context.is_empty());
        assert!(pack.optional_context.is_empty());
        assert!(pack.allowed_verbs.is_empty());
        assert!(pack.forbidden_verbs.is_empty());
        assert!(pack.required_questions.is_empty());
        assert!(pack.optional_questions.is_empty());
        assert!(pack.stop_rules.is_empty());
        assert!(pack.templates.is_empty());
        assert!(pack.pack_summary_template.is_none());
        assert!(pack.section_layout.is_empty());
        assert!(pack.definition_of_done.is_empty());
        assert!(pack.progress_signals.is_empty());
    }

    #[test]
    fn test_section_layout_and_progress_signals() {
        let yaml = r#"
id: layout-test
name: Layout
version: "1"
description: d
section_layout:
  - title: "Setup"
    verb_prefixes: ["cbu.create", "cbu.assign"]
  - title: "Trading"
    verb_prefixes: ["trading-profile"]
progress_signals:
  - signal: cbu_created
    description: "CBU has been created"
"#;
        let (pack, _) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
        assert_eq!(pack.section_layout.len(), 2);
        assert_eq!(pack.section_layout[0].title, "Setup");
        assert_eq!(pack.section_layout[0].verb_prefixes.len(), 2);
        assert_eq!(pack.progress_signals.len(), 1);
        assert_eq!(pack.progress_signals[0].signal, "cbu_created");
    }

    #[test]
    fn test_template_step_with_when_condition() {
        let yaml = r#"
id: cond-test
name: Cond
version: "1"
description: d
templates:
  - template_id: t1
    when_to_use: "test"
    steps:
      - verb: cbu.create
        args:
          name: test
      - verb: isda.create
        when: "answers.has_otc == true"
        execution_mode: human_gate
        args:
          counterparty: "{answers.counterparty}"
"#;
        let (pack, _) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
        let steps = &pack.templates[0].steps;
        assert_eq!(steps.len(), 2);
        assert!(steps[0].when.is_none());
        assert_eq!(steps[1].when.as_deref(), Some("answers.has_otc == true"));
        assert_eq!(steps[1].execution_mode.as_deref(), Some("human_gate"));
    }

    #[test]
    fn test_load_pack_from_bytes_invalid_yaml() {
        let bad = b"not: [valid: yaml: {{{}}}";
        let result = load_pack_from_bytes(bad);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let yaml = minimal_pack_yaml();
        let (pack, _) = load_pack_from_bytes(yaml.as_bytes()).unwrap();

        let json = serde_json::to_string(&pack).expect("serialize");
        let deserialized: PackManifest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.id, pack.id);
        assert_eq!(deserialized.version, pack.version);
        assert_eq!(deserialized.templates.len(), pack.templates.len());
    }
}
