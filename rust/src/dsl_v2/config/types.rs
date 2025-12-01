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
    pub version: String,
    pub domains: HashMap<String, DomainConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
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
    pub code_column: String,
    pub id_column: String,
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
    pub code_column: String,
    pub name_column: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
}

// =============================================================================
// PLUGIN CONFIG
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub description: String,
    pub handler: String,
    pub args: Vec<ArgConfig>,
    #[serde(default)]
    pub returns: Option<ReturnsConfig>,
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
}
