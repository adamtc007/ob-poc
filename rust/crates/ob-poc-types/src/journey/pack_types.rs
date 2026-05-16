//! Pack manifest data shapes (pure DTOs, no IO).
//!
//! See `journey/mod.rs` for the hoist rationale. The YAML loader and
//! `PackLoadError` live in `ob-poc-journey` (or `ob-poc-boundary::journey`
//! during the Phase 3 transition).

use crate::session::kinds::WorkspaceKind;
use serde::{Deserialize, Serialize};

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

    /// Workspaces in which this pack is valid.
    #[serde(default)]
    pub workspaces: Vec<WorkspaceKind>,

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

    /// If set, auto-handoff to this pack after successful execution.
    #[serde(default)]
    pub handoff_target: Option<String>,
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
