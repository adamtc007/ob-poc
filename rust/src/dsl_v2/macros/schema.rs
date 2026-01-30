//! Macro Schema Types
//!
//! Type definitions for macro YAML schemas. These match the format in
//! `config/verb_schemas/macros/*.yaml`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete macro schema (one verb definition)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroSchema {
    /// Must be "macro" for operator macros
    pub kind: MacroKind,

    /// UI presentation
    pub ui: MacroUi,

    /// Routing configuration
    pub routing: MacroRouting,

    /// Target configuration
    pub target: MacroTarget,

    /// Arguments specification
    pub args: MacroArgs,

    /// Prerequisites (DAG readiness)
    #[serde(default)]
    pub prereqs: Vec<MacroPrereq>,

    /// Expansion steps (primitive DSL to emit)
    pub expands_to: Vec<MacroExpansionStep>,

    /// State flags to set after execution
    #[serde(default)]
    pub sets_state: Vec<SetState>,

    /// Verbs this macro unlocks
    #[serde(default)]
    pub unlocks: Vec<String>,
}

/// Macro kind discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MacroKind {
    Macro,
    Primitive,
}

/// UI presentation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroUi {
    /// Label shown in palette
    pub label: String,

    /// One-line description (operator language)
    pub description: String,

    /// Noun shown to operator (e.g., "Structure", "Case")
    pub target_label: String,
}

/// Routing configuration for verb filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroRouting {
    /// Mode tags for palette filtering (e.g., ["kyc", "onboarding"])
    pub mode_tags: Vec<String>,

    /// Operator domain grouping (e.g., "structure", "case")
    #[serde(default)]
    pub operator_domain: Option<String>,
}

/// Target configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroTarget {
    /// What this macro operates on (e.g., "client_ref", "structure_ref")
    pub operates_on: String,

    /// What this macro produces (e.g., "structure_ref", null)
    #[serde(default)]
    pub produces: Option<String>,

    /// Allowed structure types (optional constraint)
    #[serde(default)]
    pub allowed_structure_types: Vec<String>,
}

/// Arguments specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroArgs {
    /// Argument style (always "keyworded" for now)
    pub style: ArgStyle,

    /// Required arguments
    #[serde(default)]
    pub required: HashMap<String, MacroArg>,

    /// Optional arguments
    #[serde(default)]
    pub optional: HashMap<String, MacroArg>,
}

/// Argument style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgStyle {
    Keyworded,
}

/// Single argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroArg {
    /// Argument type
    #[serde(rename = "type")]
    pub arg_type: MacroArgType,

    /// UI label shown to operator
    pub ui_label: String,

    /// Autofill from session context (e.g., ["session.current_structure"])
    #[serde(default)]
    pub autofill_from: Option<String>,

    /// Picker to use for selection (e.g., "structure_picker")
    #[serde(default)]
    pub picker: Option<String>,

    /// Default value
    #[serde(default)]
    pub default: Option<serde_json::Value>,

    /// Valid values (for str type with constraints)
    #[serde(default)]
    pub valid_values: Vec<String>,

    /// Enum values (for enum type)
    #[serde(default)]
    pub values: Vec<MacroEnumValue>,

    /// Default enum key
    #[serde(default)]
    pub default_key: Option<String>,

    /// Item type for list types
    #[serde(default)]
    pub item_type: Option<Box<MacroArgType>>,

    /// Internal configuration (hidden from UI)
    #[serde(default)]
    pub internal: Option<MacroArgInternal>,
}

/// Argument type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroArgType {
    Str,
    Date,
    Enum,
    List,

    // Operator reference types (UI-safe)
    #[serde(rename = "client_ref")]
    ClientRef,
    #[serde(rename = "structure_ref")]
    StructureRef,
    #[serde(rename = "party_ref")]
    PartyRef,
    #[serde(rename = "case_ref")]
    CaseRef,
    #[serde(rename = "mandate_ref")]
    MandateRef,
    #[serde(rename = "document_ref")]
    DocumentRef,
    #[serde(rename = "role_ref")]
    RoleRef,
}

/// Enum value definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroEnumValue {
    /// Key shown in UI and used in API (e.g., "pe", "gp")
    pub key: String,

    /// Human-readable label (e.g., "Private Equity")
    pub label: String,

    /// Internal token for DSL (e.g., "private-equity")
    pub internal: String,

    /// Valid for specific structure types (optional constraint)
    #[serde(default)]
    pub valid_for: Vec<String>,
}

/// Internal argument configuration (hidden from UI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroArgInternal {
    /// Entity kinds for filtering (e.g., ["company", "person"])
    #[serde(default)]
    pub kinds: Vec<String>,

    /// Value mapping
    #[serde(default)]
    pub map: HashMap<String, String>,

    /// Validation rules
    #[serde(default)]
    pub validate: Option<MacroArgValidation>,
}

/// Argument validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroArgValidation {
    /// Allowed structure types
    #[serde(default)]
    pub allowed_structure_types: Vec<String>,
}

/// Prerequisite condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroPrereq {
    /// Requires specific state flag to be set
    StateExists { key: String },

    /// Requires specific verb to be completed
    VerbCompleted { verb: String },

    /// Requires any of the listed conditions
    AnyOf { conditions: Vec<MacroPrereq> },

    /// Requires fact predicate
    FactExists { predicate: String },
}

/// Expansion step (primitive DSL to emit)
///
/// Can be either a direct verb call or a nested macro invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MacroExpansionStep {
    /// Direct verb call (most common)
    VerbCall(VerbCallStep),

    /// Nested macro invocation (for composites like M17, M18)
    InvokeMacro(InvokeMacroStep),
}

/// A direct verb call expansion step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCallStep {
    /// Verb to call (e.g., "cbu.create", "kyc-case.create")
    pub verb: String,

    /// Arguments with variable substitution
    pub args: HashMap<String, String>,

    /// Optional symbol binding for the result (e.g., "@cbu")
    #[serde(rename = "as", default)]
    pub bind_as: Option<String>,
}

/// A nested macro invocation step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeMacroStep {
    /// The macro to invoke (e.g., "struct.ie.ucits.icav")
    #[serde(rename = "invoke-macro")]
    pub macro_id: String,

    /// Arguments to pass to the nested macro
    #[serde(default)]
    pub args: HashMap<String, String>,

    /// Symbols to import from the nested macro's scope
    #[serde(rename = "import-symbols", default)]
    pub import_symbols: Vec<String>,
}

impl MacroExpansionStep {
    /// Get the verb or macro ID for this step
    pub fn target_id(&self) -> &str {
        match self {
            MacroExpansionStep::VerbCall(v) => &v.verb,
            MacroExpansionStep::InvokeMacro(m) => &m.macro_id,
        }
    }

    /// Check if this is a nested macro invocation
    pub fn is_invoke_macro(&self) -> bool {
        matches!(self, MacroExpansionStep::InvokeMacro(_))
    }
}

/// State flag to set after execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetState {
    /// State key (e.g., "structure.exists")
    pub key: String,

    /// Value to set
    pub value: serde_json::Value,
}

impl MacroSchema {
    /// Get the fully qualified verb name (domain.verb)
    pub fn fqn(&self, domain: &str, verb: &str) -> String {
        format!("{}.{}", domain, verb)
    }

    /// Get all required arguments
    pub fn required_args(&self) -> impl Iterator<Item = (&String, &MacroArg)> {
        self.args.required.iter()
    }

    /// Get all optional arguments
    pub fn optional_args(&self) -> impl Iterator<Item = (&String, &MacroArg)> {
        self.args.optional.iter()
    }

    /// Get all arguments (required + optional)
    pub fn all_args(&self) -> impl Iterator<Item = (&String, &MacroArg)> {
        self.args.required.iter().chain(self.args.optional.iter())
    }

    /// Check if an argument is required
    pub fn is_required(&self, arg_name: &str) -> bool {
        self.args.required.contains_key(arg_name)
    }

    /// Get argument definition by name
    pub fn get_arg(&self, arg_name: &str) -> Option<&MacroArg> {
        self.args
            .required
            .get(arg_name)
            .or_else(|| self.args.optional.get(arg_name))
    }
}

impl MacroArg {
    /// Check if this is an enum type
    pub fn is_enum(&self) -> bool {
        self.arg_type == MacroArgType::Enum
    }

    /// Get the internal token for an enum key
    pub fn internal_for_key(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|v| v.key == key)
            .map(|v| v.internal.as_str())
    }

    /// Get the default enum key if defined
    pub fn default_enum_key(&self) -> Option<&str> {
        self.default_key.as_deref()
    }

    /// Get the default internal token
    pub fn default_internal(&self) -> Option<&str> {
        self.default_key
            .as_ref()
            .and_then(|k| self.internal_for_key(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_macro_schema() {
        let yaml = r#"
kind: macro
ui:
  label: "Set up Structure"
  description: "Create a new fund or mandate structure"
  target_label: "Structure"
routing:
  mode_tags: [onboarding, kyc]
  operator_domain: structure
target:
  operates_on: client_ref
  produces: structure_ref
args:
  style: keyworded
  required:
    structure_type:
      type: enum
      ui_label: "Type"
      values:
        - key: pe
          label: "Private Equity"
          internal: private-equity
        - key: sicav
          label: "SICAV"
          internal: sicav
      default_key: pe
    name:
      type: str
      ui_label: "Structure name"
  optional: {}
prereqs: []
expands_to:
  - verb: cbu.create
    args:
      client_id: "${scope.client_id}"
      kind: "${arg.structure_type.internal}"
      name: "${arg.name}"
unlocks:
  - structure.assign-role
  - case.open
"#;

        let schema: MacroSchema = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(schema.kind, MacroKind::Macro);
        assert_eq!(schema.ui.label, "Set up Structure");
        assert!(schema.args.required.contains_key("structure_type"));
        assert!(schema.args.required.contains_key("name"));
        assert_eq!(schema.expands_to.len(), 1);
        match &schema.expands_to[0] {
            MacroExpansionStep::VerbCall(v) => assert_eq!(v.verb, "cbu.create"),
            MacroExpansionStep::InvokeMacro(_) => panic!("Expected VerbCall"),
        }
        assert_eq!(schema.unlocks.len(), 2);
    }

    #[test]
    fn test_parse_invoke_macro_step() {
        let yaml = r#"
kind: macro
ui:
  label: "Cross-Border Hedge Fund"
  description: "Set up a cross-border hedge fund structure"
  target_label: "Structure"
routing:
  mode_tags: [onboarding]
  operator_domain: structure
target:
  operates_on: client_ref
  produces: structure_ref
args:
  style: keyworded
  required:
    name:
      type: str
      ui_label: "Structure name"
    base_jurisdiction:
      type: enum
      ui_label: "Base jurisdiction"
      values:
        - key: ie
          label: "Ireland"
          internal: IE
        - key: lu
          label: "Luxembourg"
          internal: LU
      default_key: ie
  optional: {}
prereqs: []
expands_to:
  - invoke-macro: struct.ie.hedge.icav
    args:
      name: "${arg.name}"
    import-symbols:
      - "@cbu"
      - "@trading-profile"
  - verb: cbu-role.assign
    args:
      cbu_id: "@cbu"
      role: cross-border-coordinator
unlocks: []
"#;

        let schema: MacroSchema = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(schema.expands_to.len(), 2);

        // First step should be invoke-macro
        match &schema.expands_to[0] {
            MacroExpansionStep::InvokeMacro(m) => {
                assert_eq!(m.macro_id, "struct.ie.hedge.icav");
                assert_eq!(m.import_symbols.len(), 2);
                assert!(m.import_symbols.contains(&"@cbu".to_string()));
            }
            MacroExpansionStep::VerbCall(_) => panic!("Expected InvokeMacro"),
        }

        // Second step should be verb call
        match &schema.expands_to[1] {
            MacroExpansionStep::VerbCall(v) => {
                assert_eq!(v.verb, "cbu-role.assign");
            }
            MacroExpansionStep::InvokeMacro(_) => panic!("Expected VerbCall"),
        }
    }

    #[test]
    fn test_enum_internal_lookup() {
        let arg = MacroArg {
            arg_type: MacroArgType::Enum,
            ui_label: "Type".to_string(),
            autofill_from: None,
            picker: None,
            default: None,
            valid_values: vec![],
            values: vec![
                MacroEnumValue {
                    key: "pe".to_string(),
                    label: "Private Equity".to_string(),
                    internal: "private-equity".to_string(),
                    valid_for: vec![],
                },
                MacroEnumValue {
                    key: "sicav".to_string(),
                    label: "SICAV".to_string(),
                    internal: "sicav".to_string(),
                    valid_for: vec![],
                },
            ],
            default_key: Some("pe".to_string()),
            item_type: None,
            internal: None,
        };

        assert!(arg.is_enum());
        assert_eq!(arg.internal_for_key("pe"), Some("private-equity"));
        assert_eq!(arg.internal_for_key("sicav"), Some("sicav"));
        assert_eq!(arg.internal_for_key("unknown"), None);
        assert_eq!(arg.default_internal(), Some("private-equity"));
    }
}
