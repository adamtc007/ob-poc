//! Configuration type definitions
//!
//! These structs map directly to the YAML configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// TOP-LEVEL CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbsConfig {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub domains: HashMap<String, DomainConfig>,
}

fn default_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CsgRulesConfig {
    pub version: String,
    #[serde(default)]
    pub constraints: Vec<ConstraintRule>,
    #[serde(default)]
    pub warnings: Vec<WarningRule>,
    #[serde(default)]
    pub jurisdiction_rules: Vec<JurisdictionRule>,
    #[serde(default)]
    pub composite_rules: Vec<CompositeRule>,
}

// =============================================================================
// DOMAIN & VERB CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DomainConfig {
    pub description: String,
    #[serde(default)]
    pub verbs: HashMap<String, VerbConfig>,
    #[serde(default)]
    pub dynamic_verbs: Vec<DynamicVerbConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConfig {
    pub description: String,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    #[serde(default)]
    pub handler: Option<String>,
    #[serde(default)]
    pub args: Vec<ArgConfig>,
    #[serde(default)]
    pub returns: Option<ReturnsConfig>,
    /// Dataflow: what this verb produces (binding type)
    #[serde(default)]
    pub produces: Option<VerbProduces>,
    /// Dataflow: what this verb consumes (required bindings)
    #[serde(default)]
    pub consumes: Vec<VerbConsumes>,
    /// Lifecycle constraints and transitions for this verb
    #[serde(default)]
    pub lifecycle: Option<VerbLifecycle>,
}

// =============================================================================
// DATAFLOW CONFIG
// =============================================================================

/// Dataflow: what a verb produces when executed with :as @binding
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbProduces {
    /// The type of entity produced: "cbu", "entity", "case", "workstream", etc.
    #[serde(rename = "type")]
    pub produced_type: String,
    /// Optional subtype for entities: "proper_person", "limited_company", etc.
    #[serde(default)]
    pub subtype: Option<String>,
    /// True if this is a lookup (resolved existing) rather than create (new)
    #[serde(default)]
    pub resolved: bool,
    /// Initial state when creating a new entity (for lifecycle tracking)
    #[serde(default)]
    pub initial_state: Option<String>,
}

/// Dataflow: what a verb consumes (dependencies)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConsumes {
    /// Which argument carries the reference (e.g., "cbu-id", "entity-id")
    pub arg: String,
    /// Expected type of the binding (e.g., "cbu", "entity", "case")
    #[serde(rename = "type")]
    pub consumed_type: String,
    /// Whether this dependency is required (default: true)
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

/// Verb lifecycle configuration - constraints and state transitions
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct VerbLifecycle {
    /// Which argument contains the entity ID this verb operates on
    #[serde(default)]
    pub entity_arg: Option<String>,

    /// Required states for the entity before this verb can execute
    #[serde(default)]
    pub requires_states: Vec<String>,

    /// State the entity transitions to after this verb executes
    #[serde(default)]
    pub transitions_to: Option<String>,

    /// Argument that specifies the target state (for generic set-status verbs)
    #[serde(default)]
    pub transitions_to_arg: Option<String>,

    /// Precondition checks to run before execution
    #[serde(default)]
    pub precondition_checks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbBehavior {
    Crud,
    Plugin,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrudConfig {
    pub operation: CrudOperation,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub returning: Option<String>,
    #[serde(default)]
    pub conflict_keys: Option<Vec<String>>,
    // For junction operations
    #[serde(default)]
    pub junction: Option<String>,
    #[serde(default)]
    pub from_col: Option<String>,
    #[serde(default)]
    pub to_col: Option<String>,
    #[serde(default)]
    pub role_table: Option<String>,
    #[serde(default)]
    pub role_col: Option<String>,
    #[serde(default)]
    pub fk_col: Option<String>,
    #[serde(default)]
    pub filter_col: Option<String>,
    // For joins
    #[serde(default)]
    pub primary_table: Option<String>,
    #[serde(default)]
    pub join_table: Option<String>,
    #[serde(default)]
    pub join_col: Option<String>,
    // For entity creation
    #[serde(default)]
    pub base_table: Option<String>,
    #[serde(default)]
    pub extension_table_column: Option<String>,
    #[serde(default)]
    pub type_id_column: Option<String>,
    // For list operations
    #[serde(default)]
    pub order_by: Option<String>,
    // For update operations with fixed values
    #[serde(default)]
    pub set_values: Option<std::collections::HashMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrudOperation {
    Insert,
    Select,
    Update,
    Delete,
    Upsert,
    Link,
    Unlink,
    RoleLink,
    RoleUnlink,
    ListByFk,
    ListParties,
    SelectWithJoin,
    EntityCreate,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArgConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub maps_to: Option<String>,
    #[serde(default)]
    pub lookup: Option<LookupConfig>,
    #[serde(default)]
    pub valid_values: Option<Vec<String>>,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub validation: Option<ArgValidation>,
    /// Fuzzy check config - for upsert verbs, check for similar existing records
    /// and emit a warning if fuzzy matches are found
    #[serde(default)]
    pub fuzzy_check: Option<FuzzyCheckConfig>,
}

/// Configuration for fuzzy match checking on upsert args
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FuzzyCheckConfig {
    /// Entity type to search (e.g., "cbu", "entity")
    pub entity_type: String,
    /// Field to search by (defaults to arg name if not specified)
    #[serde(default)]
    pub search_key: Option<String>,
    /// Minimum score threshold for warnings (0.0-1.0, default 0.5)
    #[serde(default = "default_fuzzy_threshold")]
    pub threshold: f32,
}

fn default_fuzzy_threshold() -> f32 {
    0.5
}

/// Validation rules for an argument
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArgValidation {
    /// Valid enum values
    #[serde(default)]
    pub r#enum: Option<Vec<String>>,
    /// Minimum value (for numbers)
    #[serde(default)]
    pub min: Option<f64>,
    /// Maximum value (for numbers)
    #[serde(default)]
    pub max: Option<f64>,
    /// Regex pattern (for strings)
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgType {
    String,
    Integer,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    Uuid,
    Json,
    Lookup,
    StringList,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LookupConfig {
    pub table: String,
    #[serde(default)]
    pub schema: Option<String>,
    /// The entity type for this lookup (e.g., "proper_person", "limited_company", "role", "jurisdiction")
    /// This becomes the ref_type in the LookupRef triplet: (ref_type, search_key, primary_key)
    #[serde(default)]
    pub entity_type: Option<String>,
    /// The column containing human-readable search key (what user sees/types)
    #[serde(alias = "code_column")]
    pub search_key: String,
    /// The column containing primary key (UUID for entities, code for reference data)
    #[serde(alias = "id_column")]
    pub primary_key: String,
    /// Resolution mode: how the LSP/UI should resolve this reference.
    ///
    /// - `reference`: Small static lookup tables (< 100 items) - use autocomplete dropdown
    /// - `entity`: Large/growing tables (people, CBUs, cases) - use search modal
    ///
    /// Defaults to "reference" if not specified (backwards compatible).
    #[serde(default)]
    pub resolution_mode: Option<ResolutionMode>,
}

/// How entity references should be resolved in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMode {
    /// Small static lookup tables (roles, jurisdictions, currencies)
    /// UI: Autocomplete dropdown with all values
    #[default]
    Reference,
    /// Large/growing entity tables (people, CBUs, funds, cases)
    /// UI: Search modal with refinement
    Entity,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReturnsConfig {
    #[serde(rename = "type")]
    pub return_type: ReturnTypeConfig,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub capture: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnTypeConfig {
    Uuid,
    String,
    Record,
    RecordSet,
    Affected,
    Void,
}

// =============================================================================
// DYNAMIC VERB CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicVerbConfig {
    pub pattern: String,
    #[serde(default)]
    pub source: Option<DynamicSourceConfig>,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    #[serde(default)]
    pub base_args: Vec<ArgConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicSourceConfig {
    pub table: String,
    pub schema: Option<String>,
    /// Column containing the type code (e.g., "proper_person", "limited_company")
    #[serde(alias = "code_column")]
    pub type_code_column: String,
    /// Column containing the display name
    pub name_column: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
}

// =============================================================================
// CSG RULE CONFIGS
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstraintRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub when: RuleCondition,
    pub requires: RuleRequirement,
    pub error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WarningRule {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub when: Option<RuleCondition>,
    #[serde(default)]
    pub check: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JurisdictionRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: RuleSeverity,
    pub when: JurisdictionCondition,
    #[serde(default)]
    pub requires_document: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompositeRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: RuleSeverity,
    pub applies_to: AppliesTo,
    pub checks: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RuleCondition {
    #[serde(default)]
    pub verb: Option<String>,
    #[serde(default)]
    pub verb_pattern: Option<String>,
    #[serde(default)]
    pub arg: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub missing_arg: Option<String>,
    #[serde(default)]
    pub greater_than: Option<f64>,
    #[serde(default)]
    pub less_than: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RuleRequirement {
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
    #[serde(default)]
    pub via_arg: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JurisdictionCondition {
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_type_in: Option<Vec<String>>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub jurisdiction_in: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppliesTo {
    #[serde(default)]
    pub client_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_behavior_serde() {
        let yaml = "crud";
        let behavior: VerbBehavior = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(behavior, VerbBehavior::Crud);

        let yaml = "plugin";
        let behavior: VerbBehavior = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(behavior, VerbBehavior::Plugin);
    }

    #[test]
    fn test_crud_operation_serde() {
        let yaml = "insert";
        let op: CrudOperation = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(op, CrudOperation::Insert);

        let yaml = "upsert";
        let op: CrudOperation = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(op, CrudOperation::Upsert);

        let yaml = "role_link";
        let op: CrudOperation = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(op, CrudOperation::RoleLink);
    }

    #[test]
    fn test_arg_type_serde() {
        let yaml = "string";
        let arg_type: ArgType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(arg_type, ArgType::String);

        let yaml = "uuid";
        let arg_type: ArgType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(arg_type, ArgType::Uuid);

        let yaml = "string_list";
        let arg_type: ArgType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(arg_type, ArgType::StringList);
    }

    #[test]
    fn test_return_type_serde() {
        let yaml = "uuid";
        let ret_type: ReturnTypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ret_type, ReturnTypeConfig::Uuid);

        let yaml = "record_set";
        let ret_type: ReturnTypeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(ret_type, ReturnTypeConfig::RecordSet);
    }

    #[test]
    fn test_resolution_mode_serde() {
        // Test explicit values
        let yaml = "reference";
        let mode: ResolutionMode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mode, ResolutionMode::Reference);

        let yaml = "entity";
        let mode: ResolutionMode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mode, ResolutionMode::Entity);

        // Test default
        assert_eq!(ResolutionMode::default(), ResolutionMode::Reference);
    }

    #[test]
    fn test_lookup_config_with_resolution_mode() {
        let yaml = r#"
table: cbus
schema: ob-poc
entity_type: cbu
search_key: name
primary_key: cbu_id
resolution_mode: entity
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.resolution_mode, Some(ResolutionMode::Entity));

        // Test without resolution_mode (defaults to None, which means Reference)
        let yaml = r#"
table: roles
schema: ob-poc
entity_type: role
search_key: name
primary_key: role_id
"#;
        let config: LookupConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.resolution_mode, None);
    }
}
