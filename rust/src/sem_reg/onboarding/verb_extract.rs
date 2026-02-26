//! Step 1: Extract DSL verb signatures from VerbsConfig.
//!
//! Loads verb YAML via `ConfigLoader::from_env().load_verbs()` and extracts
//! structured `VerbExtract` records with inputs, outputs, side-effects, and
//! execution mode for downstream cross-referencing.

use serde::{Deserialize, Serialize};

/// Extracted verb signature from DSL YAML configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbExtract {
    /// Fully qualified name: `{domain}.{action}`
    pub fqn: String,
    /// Domain (e.g. "cbu", "kyc", "entity")
    pub domain: String,
    /// Action (e.g. "create", "list", "update-status")
    pub action: String,
    /// Human-readable description
    pub description: String,
    /// Verb behavior
    pub behavior: VerbBehaviorKind,
    /// Input arguments
    pub inputs: Vec<VerbInput>,
    /// Output specification
    pub output: Option<VerbOutput>,
    /// Side effects (tables read/written)
    pub side_effects: Vec<SideEffect>,
    /// Execution mode
    pub execution_mode: ExecutionMode,
    /// Whether this is an internal-only verb
    pub is_internal: bool,
}

/// Simplified behavior classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerbBehaviorKind {
    Crud,
    Plugin,
    GraphQuery,
    Durable,
}

/// A verb input argument with its mapping metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbInput {
    /// Argument name
    pub name: String,
    /// Argument type as string (from ArgType)
    pub arg_type: String,
    /// Whether this argument is required
    pub required: bool,
    /// Database column this maps to (if any)
    pub maps_to: Option<String>,
    /// Lookup table (if entity resolution arg)
    pub lookup_table: Option<String>,
    /// Lookup schema
    pub lookup_schema: Option<String>,
}

/// What a verb produces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbOutput {
    /// The type of entity/object produced (e.g. "cbu", "entity")
    pub produced_type: String,
    /// Initial lifecycle state
    pub initial_state: Option<String>,
}

/// A side effect on a database table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffect {
    /// Schema name (e.g. "ob-poc", "kyc")
    pub schema: Option<String>,
    /// Table name
    pub table: String,
    /// CRUD operation type
    pub operation: SideEffectOp,
}

/// Side effect operation type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectOp {
    Read,
    Write,
    Insert,
    Update,
    Delete,
    Upsert,
}

/// Execution mode classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Sync,
    Durable,
}

/// Extract verb signatures from loaded VerbsConfig.
///
/// Iterates all domains and verbs, producing a flat list of `VerbExtract` records.
pub fn extract_verbs(verbs_config: &dsl_core::config::types::VerbsConfig) -> Vec<VerbExtract> {
    let mut extracts = Vec::new();

    for (domain_name, domain_config) in &verbs_config.domains {
        for (verb_name, verb_config) in &domain_config.verbs {
            let fqn = format!("{}.{}", domain_name, verb_name);

            let behavior = match verb_config.behavior {
                dsl_core::config::types::VerbBehavior::Crud => VerbBehaviorKind::Crud,
                dsl_core::config::types::VerbBehavior::Plugin => VerbBehaviorKind::Plugin,
                dsl_core::config::types::VerbBehavior::GraphQuery => VerbBehaviorKind::GraphQuery,
                dsl_core::config::types::VerbBehavior::Durable => VerbBehaviorKind::Durable,
            };

            let inputs = extract_inputs(&verb_config.args);
            let output = extract_output(&verb_config.produces);
            let side_effects = extract_side_effects(verb_config);
            let execution_mode = if verb_config.durable.is_some() {
                ExecutionMode::Durable
            } else {
                ExecutionMode::Sync
            };
            let is_internal = verb_config
                .metadata
                .as_ref()
                .map(|m| m.internal)
                .unwrap_or(false);

            extracts.push(VerbExtract {
                fqn,
                domain: domain_name.clone(),
                action: verb_name.clone(),
                description: verb_config.description.clone(),
                behavior,
                inputs,
                output,
                side_effects,
                execution_mode,
                is_internal,
            });
        }
    }

    extracts.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    extracts
}

fn extract_inputs(args: &[dsl_core::config::types::ArgConfig]) -> Vec<VerbInput> {
    args.iter()
        .map(|arg| {
            let arg_type = format!("{:?}", arg.arg_type).to_lowercase();
            let (lookup_table, lookup_schema) = arg
                .lookup
                .as_ref()
                .map(|l| (Some(l.table.clone()), l.schema.clone()))
                .unwrap_or((None, None));

            VerbInput {
                name: arg.name.clone(),
                arg_type,
                required: arg.required,
                maps_to: arg.maps_to.clone(),
                lookup_table,
                lookup_schema,
            }
        })
        .collect()
}

fn extract_output(produces: &Option<dsl_core::config::types::VerbProduces>) -> Option<VerbOutput> {
    produces.as_ref().map(|p| VerbOutput {
        produced_type: p.produced_type.clone(),
        initial_state: p.initial_state.clone(),
    })
}

fn extract_side_effects(verb_config: &dsl_core::config::types::VerbConfig) -> Vec<SideEffect> {
    let mut effects = Vec::new();

    // From CRUD config — table + operation
    if let Some(ref crud) = verb_config.crud {
        if let Some(ref table) = crud.table {
            let op = match crud.operation {
                dsl_core::config::types::CrudOperation::Insert => SideEffectOp::Insert,
                dsl_core::config::types::CrudOperation::Select
                | dsl_core::config::types::CrudOperation::SelectWithJoin
                | dsl_core::config::types::CrudOperation::ListByFk
                | dsl_core::config::types::CrudOperation::ListParties => SideEffectOp::Read,
                dsl_core::config::types::CrudOperation::Update => SideEffectOp::Update,
                dsl_core::config::types::CrudOperation::Delete => SideEffectOp::Delete,
                dsl_core::config::types::CrudOperation::Upsert
                | dsl_core::config::types::CrudOperation::EntityUpsert => SideEffectOp::Upsert,
                dsl_core::config::types::CrudOperation::Link
                | dsl_core::config::types::CrudOperation::RoleLink => SideEffectOp::Insert,
                dsl_core::config::types::CrudOperation::Unlink
                | dsl_core::config::types::CrudOperation::RoleUnlink => SideEffectOp::Delete,
                dsl_core::config::types::CrudOperation::EntityCreate => SideEffectOp::Insert,
            };
            effects.push(SideEffect {
                schema: crud.schema.clone(),
                table: table.clone(),
                operation: op,
            });
        }

        // Junction tables for link/unlink operations
        if let Some(ref junction) = crud.junction {
            let op = match crud.operation {
                dsl_core::config::types::CrudOperation::Link
                | dsl_core::config::types::CrudOperation::RoleLink => SideEffectOp::Insert,
                dsl_core::config::types::CrudOperation::Unlink
                | dsl_core::config::types::CrudOperation::RoleUnlink => SideEffectOp::Delete,
                _ => SideEffectOp::Write,
            };
            effects.push(SideEffect {
                schema: crud.schema.clone(),
                table: junction.clone(),
                operation: op,
            });
        }

        // Entity create base/extension tables
        if let Some(ref base) = crud.base_table {
            effects.push(SideEffect {
                schema: crud.schema.clone(),
                table: base.clone(),
                operation: SideEffectOp::Insert,
            });
        }
        if let Some(ref ext) = crud.extension_table {
            effects.push(SideEffect {
                schema: crud.schema.clone(),
                table: ext.clone(),
                operation: SideEffectOp::Insert,
            });
        }
    }

    // From VerbLifecycle — writes_tables / reads_tables
    if let Some(ref lifecycle) = verb_config.lifecycle {
        for write_table in &lifecycle.writes_tables {
            // Format: "schema.table" or just "table"
            let (schema, table) = parse_schema_table(write_table);
            // Dedupe: skip if already present
            if !effects
                .iter()
                .any(|e| e.table == table && e.schema == schema)
            {
                effects.push(SideEffect {
                    schema,
                    table,
                    operation: SideEffectOp::Write,
                });
            }
        }
        for read_table in &lifecycle.reads_tables {
            let (schema, table) = parse_schema_table(read_table);
            if !effects
                .iter()
                .any(|e| e.table == table && e.schema == schema)
            {
                effects.push(SideEffect {
                    schema,
                    table,
                    operation: SideEffectOp::Read,
                });
            }
        }
    }

    effects
}

/// Parse "schema.table" into (Some(schema), table) or (None, table).
fn parse_schema_table(s: &str) -> (Option<String>, String) {
    if let Some(dot_pos) = s.find('.') {
        let schema = s[..dot_pos].to_string();
        let table = s[dot_pos + 1..].to_string();
        (Some(schema), table)
    } else {
        (None, s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_schema_table_with_schema() {
        let (schema, table) = parse_schema_table("ob-poc.cbus");
        assert_eq!(schema, Some("ob-poc".to_string()));
        assert_eq!(table, "cbus");
    }

    #[test]
    fn test_parse_schema_table_without_schema() {
        let (schema, table) = parse_schema_table("cbus");
        assert!(schema.is_none());
        assert_eq!(table, "cbus");
    }

    #[test]
    fn test_extract_verbs_from_config() {
        // Load actual config to verify extraction works
        let loader = dsl_core::config::loader::ConfigLoader::from_env();
        if let Ok(verbs_config) = loader.load_verbs() {
            let extracts = extract_verbs(&verbs_config);
            assert!(!extracts.is_empty(), "Should extract at least one verb");

            // Check that FQNs are domain.action format
            for ext in &extracts {
                assert!(
                    ext.fqn.contains('.'),
                    "FQN '{}' should contain a dot",
                    ext.fqn
                );
                assert_eq!(ext.fqn, format!("{}.{}", ext.domain, ext.action));
            }

            // Check sorted
            let fqns: Vec<&str> = extracts.iter().map(|e| e.fqn.as_str()).collect();
            let mut sorted_fqns = fqns.clone();
            sorted_fqns.sort();
            assert_eq!(fqns, sorted_fqns, "VerbExtracts should be sorted by FQN");
        }
    }
}
