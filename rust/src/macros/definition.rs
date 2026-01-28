//! Operator macro definition types
//!
//! These types map to the YAML schema in config/verb_schemas/macros/*.yaml

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level macro definition (matches YAML structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorMacroDef {
    /// Fully qualified name (e.g., "structure.setup")
    #[serde(skip)]
    pub fqn: String,

    /// Macro kind (always "macro" for operator macros)
    pub kind: String,

    /// UI presentation metadata
    pub ui: MacroUi,

    /// Routing information (mode tags, domain)
    pub routing: MacroRouting,

    /// Target entity information
    pub target: MacroTarget,

    /// Argument definitions
    pub args: MacroArgs,

    /// Prerequisites (DAG state requirements)
    #[serde(default)]
    pub prereqs: Vec<MacroPrereq>,

    /// DSL expansion template
    pub expands_to: Vec<MacroExpansion>,

    /// State flags set after execution
    #[serde(default)]
    pub sets_state: Vec<MacroStateSet>,

    /// Macros unlocked after this one completes
    #[serde(default)]
    pub unlocks: Vec<String>,
}

/// UI presentation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroUi {
    /// Display label for UI buttons/menus
    pub label: String,

    /// Description shown in tooltips/help
    pub description: String,

    /// Label for the produced entity
    #[serde(default)]
    pub target_label: Option<String>,
}

/// Routing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroRouting {
    /// Mode tags this macro is available in
    pub mode_tags: Vec<String>,

    /// Operator domain (structure, case, mandate)
    pub operator_domain: String,
}

/// Target entity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroTarget {
    /// Entity type this macro operates on (e.g., "client_ref", "structure_ref")
    pub operates_on: String,

    /// Entity type this macro produces (e.g., "structure_ref", null for read-only)
    pub produces: Option<String>,
}

/// Argument definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroArgs {
    /// Argument style (keyworded, positional)
    pub style: String,

    /// Required arguments
    #[serde(default)]
    pub required: HashMap<String, MacroArgDef>,

    /// Optional arguments
    #[serde(default)]
    pub optional: HashMap<String, MacroArgDef>,
}

/// Single argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroArgDef {
    /// Argument type (str, enum, date, structure_ref, party_ref, etc.)
    #[serde(rename = "type")]
    pub arg_type: String,

    /// UI label
    pub ui_label: String,

    /// Valid values for string type
    #[serde(default)]
    pub valid_values: Vec<String>,

    /// Enum values for enum type
    #[serde(default)]
    pub values: Vec<MacroEnumValue>,

    /// Default key for enum type
    #[serde(default)]
    pub default_key: Option<String>,

    /// Default value for non-enum types
    #[serde(default)]
    pub default: Option<String>,

    /// Autofill source (e.g., "session.client.jurisdiction")
    #[serde(default)]
    pub autofill_from: Option<String>,

    /// Picker component to use
    #[serde(default)]
    pub picker: Option<String>,
}

/// Enum value definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroEnumValue {
    /// Key used in DSL/API
    pub key: String,

    /// Display label
    pub label: String,

    /// Internal DSL token (e.g., "private-equity" for key "pe")
    #[serde(default)]
    pub internal: Option<String>,

    /// Structure types this value is valid for
    #[serde(default)]
    pub valid_for: Vec<String>,
}

/// Prerequisite condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroPrereq {
    /// A state flag must exist
    StateExists { key: String },

    /// A specific verb must have been completed
    VerbCompleted { verb: String },

    /// Any of the listed conditions must be met
    AnyOf { conditions: Vec<MacroPrereq> },
}

/// DSL expansion step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroExpansion {
    /// DSL verb to call
    pub verb: String,

    /// Argument mappings (value can be "${arg.foo}" or "${scope.bar}")
    pub args: HashMap<String, String>,
}

/// State flag to set after execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroStateSet {
    /// State key
    pub key: String,

    /// Value to set
    pub value: bool,
}

impl OperatorMacroDef {
    /// Get the domain from the FQN (e.g., "structure" from "structure.setup")
    pub fn domain(&self) -> &str {
        self.fqn.split('.').next().unwrap_or(&self.fqn)
    }

    /// Get the action from the FQN (e.g., "setup" from "structure.setup")
    pub fn action(&self) -> &str {
        self.fqn.split('.').nth(1).unwrap_or("")
    }

    /// Check if this macro is available for a given mode tag
    pub fn is_available_for_mode(&self, mode: &str) -> bool {
        self.routing.mode_tags.iter().any(|t| t == mode)
    }

    /// Get all required argument names
    pub fn required_arg_names(&self) -> Vec<&str> {
        self.args.required.keys().map(|s| s.as_str()).collect()
    }

    /// Get all optional argument names
    pub fn optional_arg_names(&self) -> Vec<&str> {
        self.args.optional.keys().map(|s| s.as_str()).collect()
    }

    /// Get argument definition by name
    pub fn get_arg(&self, name: &str) -> Option<&MacroArgDef> {
        self.args
            .required
            .get(name)
            .or_else(|| self.args.optional.get(name))
    }

    /// Check if prerequisites are satisfied given current DAG state
    pub fn check_prereqs(&self, dag_state: &crate::session::unified::DagState) -> bool {
        self.prereqs.iter().all(|p| check_prereq(p, dag_state))
    }
}

fn check_prereq(prereq: &MacroPrereq, dag_state: &crate::session::unified::DagState) -> bool {
    match prereq {
        MacroPrereq::StateExists { key } => dag_state.get_flag(key),
        MacroPrereq::VerbCompleted { verb } => dag_state.is_completed(verb),
        MacroPrereq::AnyOf { conditions } => conditions.iter().any(|c| check_prereq(c, dag_state)),
    }
}

/// Summary of a macro for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroSummary {
    pub fqn: String,
    pub label: String,
    pub description: String,
    pub domain: String,
    pub mode_tags: Vec<String>,
    pub prereqs_satisfied: bool,
}

impl From<&OperatorMacroDef> for MacroSummary {
    fn from(def: &OperatorMacroDef) -> Self {
        Self {
            fqn: def.fqn.clone(),
            label: def.ui.label.clone(),
            description: def.ui.description.clone(),
            domain: def.routing.operator_domain.clone(),
            mode_tags: def.routing.mode_tags.clone(),
            prereqs_satisfied: true, // Caller should update this
        }
    }
}
