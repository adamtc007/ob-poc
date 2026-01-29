//! Macro Expander
//!
//! Expands operator macros into primitive DSL statements.
//!
//! ## Pipeline
//!
//! ```text
//! Macro invocation: (structure.setup :type pe :name "Acme Fund")
//!     ↓
//! 1. Lookup macro in MacroRegistry
//! 2. Validate arguments against schema
//! 3. Build VariableContext from args + scope + session
//! 4. Substitute variables in expands_to templates
//! 5. Return primitive DSL statements
//!     ↓
//! Expanded: (cbu.create :kind private-equity :name "Acme Fund" :client_id uuid-123)
//! ```

use std::collections::HashMap;

use chrono::Utc;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::registry::MacroRegistry;
use super::schema::MacroSchema;
use super::variable::{substitute_variables, ArgValue, VariableContext, VariableError};
use crate::session::unified::UnifiedSession;

/// Errors during macro expansion
#[derive(Debug, Error)]
pub enum MacroExpansionError {
    #[error("Unknown macro: {0}")]
    UnknownMacro(String),

    #[error("Missing required argument: {0}")]
    MissingRequired(String),

    #[error("Unknown argument: {0}")]
    UnknownArgument(String),

    #[error("Invalid argument value for '{arg}': {message}")]
    InvalidArgument { arg: String, message: String },

    #[error("Variable substitution failed: {0}")]
    VariableError(#[from] VariableError),

    #[error("Prereq not satisfied: {0}")]
    PrereqNotSatisfied(String),

    #[error(
        "Structure type constraint violated: macro requires {allowed:?}, session has {actual}"
    )]
    StructureTypeConstraint {
        allowed: Vec<String>,
        actual: String,
    },

    #[error("Role constraint violated: role '{role}' is not valid for structure type '{structure_type}'")]
    RoleConstraint {
        role: String,
        structure_type: String,
    },
}

/// Output from macro expansion
#[derive(Debug, Clone)]
pub struct MacroExpansionOutput {
    /// Expanded DSL statements (primitive verbs)
    pub statements: Vec<String>,

    /// State flags to set after execution
    pub sets_state: Vec<(String, serde_json::Value)>,

    /// Verbs that become available after this macro
    pub unlocks: Vec<String>,

    /// Audit information
    pub audit: MacroExpansionAudit,
}

/// Audit trail for macro expansion
#[derive(Debug, Clone)]
pub struct MacroExpansionAudit {
    /// Unique expansion ID
    pub expansion_id: Uuid,

    /// Macro FQN that was expanded
    pub macro_fqn: String,

    /// Hash of input arguments
    pub args_digest: String,

    /// Hash of expanded output
    pub output_digest: String,

    /// Timestamp
    pub expanded_at: chrono::DateTime<Utc>,
}

/// Expand a macro invocation into primitive DSL
///
/// # Arguments
///
/// * `macro_fqn` - Fully qualified macro name (e.g., "structure.setup")
/// * `args` - Argument values (name → value string)
/// * `session` - Current session state for context
/// * `registry` - Macro registry
///
/// # Returns
///
/// Expanded DSL statements ready for the normal pipeline
pub fn expand_macro(
    macro_fqn: &str,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
    registry: &MacroRegistry,
) -> Result<MacroExpansionOutput, MacroExpansionError> {
    // 1. Lookup macro
    let schema = registry
        .get(macro_fqn)
        .ok_or_else(|| MacroExpansionError::UnknownMacro(macro_fqn.to_string()))?;

    // 2. Validate arguments
    validate_args(schema, args)?;

    // 3. Check prereqs
    check_prereqs(schema, session)?;

    // 4. Check structure type constraints
    check_structure_type_constraints(schema, args, session)?;

    // 5. Build variable context
    let ctx = build_variable_context(schema, args, session)?;

    // 6. Expand templates
    let mut statements = Vec::new();
    for step in &schema.expands_to {
        let dsl = expand_step(step, &ctx)?;
        statements.push(dsl);
    }

    // 7. Build audit trail
    let args_json = serde_json::to_string(args).unwrap_or_default();
    let output_str = statements.join("\n");

    let audit = MacroExpansionAudit {
        expansion_id: Uuid::new_v4(),
        macro_fqn: macro_fqn.to_string(),
        args_digest: hash_string(&args_json),
        output_digest: hash_string(&output_str),
        expanded_at: Utc::now(),
    };

    // 8. Collect sets_state and unlocks
    let sets_state: Vec<_> = schema
        .sets_state
        .iter()
        .map(|s| (s.key.clone(), s.value.clone()))
        .collect();

    let unlocks = schema.unlocks.clone();

    Ok(MacroExpansionOutput {
        statements,
        sets_state,
        unlocks,
        audit,
    })
}

/// Validate provided arguments against schema
fn validate_args(
    schema: &MacroSchema,
    args: &HashMap<String, String>,
) -> Result<(), MacroExpansionError> {
    // Check all required args are present
    for (name, _spec) in schema.required_args() {
        if !args.contains_key(name) {
            return Err(MacroExpansionError::MissingRequired(name.clone()));
        }
    }

    // Check no unknown args
    for name in args.keys() {
        if schema.get_arg(name).is_none() {
            return Err(MacroExpansionError::UnknownArgument(name.clone()));
        }
    }

    // Validate enum values
    for (name, value) in args {
        if let Some(arg_spec) = schema.get_arg(name) {
            if arg_spec.is_enum() {
                // Check value is a valid enum key
                if arg_spec.internal_for_key(value).is_none() {
                    return Err(MacroExpansionError::InvalidArgument {
                        arg: name.clone(),
                        message: format!(
                            "Invalid enum value '{}'. Valid values: {:?}",
                            value,
                            arg_spec.values.iter().map(|v| &v.key).collect::<Vec<_>>()
                        ),
                    });
                }
            }
        }
    }

    Ok(())
}

/// Check prereqs are satisfied
fn check_prereqs(
    schema: &MacroSchema,
    session: &UnifiedSession,
) -> Result<(), MacroExpansionError> {
    use super::schema::MacroPrereq;

    for prereq in &schema.prereqs {
        match prereq {
            MacroPrereq::StateExists { key } => {
                if !session
                    .dag_state
                    .state_flags
                    .get(key)
                    .copied()
                    .unwrap_or(false)
                {
                    return Err(MacroExpansionError::PrereqNotSatisfied(format!(
                        "State '{}' not set",
                        key
                    )));
                }
            }
            MacroPrereq::VerbCompleted { verb } => {
                if !session.dag_state.completed.contains(verb) {
                    return Err(MacroExpansionError::PrereqNotSatisfied(format!(
                        "Verb '{}' not completed",
                        verb
                    )));
                }
            }
            MacroPrereq::AnyOf { conditions } => {
                let any_satisfied = conditions.iter().any(|c| match c {
                    MacroPrereq::StateExists { key } => session
                        .dag_state
                        .state_flags
                        .get(key)
                        .copied()
                        .unwrap_or(false),
                    MacroPrereq::VerbCompleted { verb } => {
                        session.dag_state.completed.contains(verb)
                    }
                    _ => false,
                });
                if !any_satisfied {
                    return Err(MacroExpansionError::PrereqNotSatisfied(
                        "None of the alternative conditions satisfied".to_string(),
                    ));
                }
            }
            MacroPrereq::FactExists { predicate } => {
                // For now, skip fact predicates (would need expression evaluation)
                tracing::debug!("Skipping fact predicate check: {}", predicate);
            }
        }
    }

    Ok(())
}

/// Check structure type constraints for role assignments
fn check_structure_type_constraints(
    schema: &MacroSchema,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
) -> Result<(), MacroExpansionError> {
    // Check target.allowed_structure_types
    if !schema.target.allowed_structure_types.is_empty() {
        if let Some(structure_type) = &session.structure_type {
            let type_str = structure_type.to_string().to_lowercase();
            if !schema.target.allowed_structure_types.contains(&type_str) {
                return Err(MacroExpansionError::StructureTypeConstraint {
                    allowed: schema.target.allowed_structure_types.clone(),
                    actual: type_str,
                });
            }
        }
    }

    // Check role-specific constraints (e.g., GP only for PE/Hedge)
    if let Some(role_arg) = schema.get_arg("role") {
        if let Some(role_value) = args.get("role") {
            // Find the enum value
            if let Some(enum_val) = role_arg.values.iter().find(|v| v.key == *role_value) {
                if !enum_val.valid_for.is_empty() {
                    if let Some(structure_type) = &session.structure_type {
                        // Use short_key() for matching against valid_for (e.g., "pe", "sicav")
                        let type_key = structure_type.short_key().to_string();
                        if !enum_val.valid_for.contains(&type_key) {
                            return Err(MacroExpansionError::RoleConstraint {
                                role: role_value.clone(),
                                structure_type: type_key,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Build variable context from args, scope, and session
fn build_variable_context(
    schema: &MacroSchema,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
) -> Result<VariableContext, MacroExpansionError> {
    // Merge required and optional arg specs
    let mut all_args = schema.args.required.clone();
    all_args.extend(schema.args.optional.clone());

    // Build arg values with enum resolution
    let mut ctx = VariableContext::from_macro_args(&all_args, args)?;

    // Add scope context
    if let Some(client) = &session.client {
        ctx.scope
            .insert("client_id".to_string(), client.client_id.to_string());
    }

    // Add session context
    if let Some(structure) = &session.current_structure {
        ctx.session.insert(
            "current_structure".to_string(),
            serde_json::Value::String(structure.structure_id.to_string()),
        );
    }

    if let Some(case) = &session.current_case {
        ctx.session.insert(
            "current_case".to_string(),
            serde_json::Value::String(case.case_id.to_string()),
        );
    }

    // Handle autofill for optional args
    for (name, spec) in &schema.args.optional {
        if !args.contains_key(name) {
            if let Some(autofill_path) = &spec.autofill_from {
                // Try to autofill from session
                if let Some(value) = resolve_autofill(autofill_path, session) {
                    ctx.args.insert(name.clone(), ArgValue::literal(value));
                }
            }
        }
    }

    Ok(ctx)
}

/// Resolve autofill path from session
fn resolve_autofill(path: &str, session: &UnifiedSession) -> Option<String> {
    match path {
        "session.current_structure" => session
            .current_structure
            .as_ref()
            .map(|s| s.structure_id.to_string()),
        "session.current_case" => session.current_case.as_ref().map(|c| c.case_id.to_string()),
        "session.client.jurisdiction" => {
            // Would need client jurisdiction lookup
            None
        }
        _ => None,
    }
}

/// Expand a single expansion step
fn expand_step(
    step: &super::schema::MacroExpansionStep,
    ctx: &VariableContext,
) -> Result<String, MacroExpansionError> {
    // Build the DSL statement
    let mut parts = vec![format!("({}", step.verb)];

    for (arg_name, arg_template) in &step.args {
        // Skip null/empty values
        let value = substitute_variables(arg_template, ctx)?;
        if value != "null" && !value.is_empty() {
            // Format argument
            let formatted = if value.contains(' ') || value.contains('"') {
                // Quote strings with spaces
                format!(" :{} \"{}\"", arg_name, value.replace('"', "\\\""))
            } else {
                format!(" :{} {}", arg_name, value)
            };
            parts.push(formatted);
        }
    }

    parts.push(")".to_string());

    Ok(parts.join(""))
}

/// Hash a string for audit
fn hash_string(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // First 8 bytes = 16 hex chars
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::unified::{ClientRef, StructureType};

    fn mock_session() -> UnifiedSession {
        UnifiedSession {
            client: Some(ClientRef {
                client_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                display_name: "Test Client".to_string(),
            }),
            structure_type: Some(StructureType::Pe),
            ..Default::default()
        }
    }

    fn mock_registry() -> MacroRegistry {
        let yaml = r#"
structure.setup:
  kind: macro
  ui:
    label: "Set up Structure"
    description: "Create a new fund"
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
        ui_label: "Name"
    optional: {}
  prereqs: []
  expands_to:
    - verb: cbu.create
      args:
        kind: "${arg.structure_type.internal}"
        name: "${arg.name}"
        client_id: "${scope.client_id}"
  sets_state:
    - key: structure.exists
      value: true
  unlocks:
    - structure.assign-role
"#;

        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(yaml).unwrap();
        let mut registry = MacroRegistry::new();
        for (fqn, schema) in raw {
            registry.add(fqn, schema);
        }
        registry
    }

    #[test]
    fn test_expand_simple_macro() {
        let registry = mock_registry();
        let session = mock_session();

        let mut args = HashMap::new();
        args.insert("structure_type".to_string(), "pe".to_string());
        args.insert("name".to_string(), "Acme Fund".to_string());

        let result = expand_macro("structure.setup", &args, &session, &registry).unwrap();

        eprintln!("Expanded DSL: {:?}", result.statements);

        assert_eq!(result.statements.len(), 1);
        assert!(result.statements[0].contains("cbu.create"));
        assert!(result.statements[0].contains(":kind private-equity"));
        // Name may be quoted due to space
        assert!(
            result.statements[0].contains(":name Acme Fund")
                || result.statements[0].contains(":name \"Acme Fund\"")
        );
        assert!(result.statements[0].contains(":client_id 11111111-1111-1111-1111-111111111111"));

        // Check state and unlocks
        assert_eq!(result.sets_state.len(), 1);
        assert_eq!(result.sets_state[0].0, "structure.exists");
        assert_eq!(result.unlocks, vec!["structure.assign-role"]);
    }

    #[test]
    fn test_expand_missing_required() {
        let registry = mock_registry();
        let session = mock_session();

        let mut args = HashMap::new();
        // Missing required "name" argument
        args.insert("structure_type".to_string(), "pe".to_string());

        let result = expand_macro("structure.setup", &args, &session, &registry);
        assert!(matches!(
            result,
            Err(MacroExpansionError::MissingRequired(_))
        ));
    }

    #[test]
    fn test_expand_invalid_enum() {
        let registry = mock_registry();
        let session = mock_session();

        let mut args = HashMap::new();
        args.insert("structure_type".to_string(), "invalid".to_string());
        args.insert("name".to_string(), "Acme".to_string());

        let result = expand_macro("structure.setup", &args, &session, &registry);
        assert!(matches!(
            result,
            Err(MacroExpansionError::InvalidArgument { .. })
        ));
    }

    #[test]
    fn test_expand_unknown_macro() {
        let registry = mock_registry();
        let session = mock_session();

        let result = expand_macro("unknown.macro", &HashMap::new(), &session, &registry);
        assert!(matches!(result, Err(MacroExpansionError::UnknownMacro(_))));
    }

    // =========================================================================
    // Role Validation Tests (Phase 7: GP fails on SICAV)
    // =========================================================================

    fn mock_registry_with_assign_role() -> MacroRegistry {
        let yaml = r#"
structure.assign-role:
  kind: macro
  ui:
    label: "Assign Role"
    description: "Assign a party to a role"
    target_label: "Role"
  routing:
    mode_tags: [onboarding]
    operator_domain: structure
  target:
    operates_on: structure_ref
    produces: role_ref
  args:
    style: keyworded
    required:
      structure:
        type: structure_ref
        ui_label: "Structure"
      role:
        type: enum
        ui_label: "Role"
        values:
          - key: gp
            label: "General Partner"
            internal: general-partner
            valid_for: [pe, hedge]
          - key: lp
            label: "Limited Partner"
            internal: limited-partner
            valid_for: [pe, hedge]
          - key: manco
            label: "Management Company"
            internal: management-company
            valid_for: [sicav]
          - key: im
            label: "Investment Manager"
            internal: investment-manager
        default_key: im
      party:
        type: party_ref
        ui_label: "Party"
    optional: {}
  prereqs: []
  expands_to:
    - verb: cbu-role.assign
      args:
        cbu_id: "${arg.structure}"
        role: "${arg.role.internal}"
        entity_id: "${arg.party}"
  unlocks: []
"#;

        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(yaml).unwrap();
        let mut registry = MacroRegistry::new();
        for (fqn, schema) in raw {
            registry.add(fqn, schema);
        }
        registry
    }

    fn mock_session_pe() -> UnifiedSession {
        UnifiedSession {
            client: Some(ClientRef {
                client_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                display_name: "Test Client".to_string(),
            }),
            structure_type: Some(StructureType::Pe),
            ..Default::default()
        }
    }

    fn mock_session_sicav() -> UnifiedSession {
        UnifiedSession {
            client: Some(ClientRef {
                client_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                display_name: "Test Client".to_string(),
            }),
            structure_type: Some(StructureType::Sicav),
            ..Default::default()
        }
    }

    #[test]
    fn test_gp_valid_for_pe_structure() {
        let registry = mock_registry_with_assign_role();
        let session = mock_session_pe();

        let mut args = HashMap::new();
        args.insert(
            "structure".to_string(),
            "22222222-2222-2222-2222-222222222222".to_string(),
        );
        args.insert("role".to_string(), "gp".to_string());
        args.insert(
            "party".to_string(),
            "33333333-3333-3333-3333-333333333333".to_string(),
        );

        let result = expand_macro("structure.assign-role", &args, &session, &registry);
        assert!(
            result.is_ok(),
            "GP should be valid for PE structure: {:?}",
            result
        );

        let output = result.unwrap();
        assert!(output.statements[0].contains(":role general-partner"));
    }

    #[test]
    fn test_gp_fails_on_sicav_structure() {
        let registry = mock_registry_with_assign_role();
        let session = mock_session_sicav();

        let mut args = HashMap::new();
        args.insert(
            "structure".to_string(),
            "22222222-2222-2222-2222-222222222222".to_string(),
        );
        args.insert("role".to_string(), "gp".to_string()); // GP not valid for SICAV
        args.insert(
            "party".to_string(),
            "33333333-3333-3333-3333-333333333333".to_string(),
        );

        let result = expand_macro("structure.assign-role", &args, &session, &registry);
        assert!(
            matches!(result, Err(MacroExpansionError::RoleConstraint { .. })),
            "GP should fail on SICAV structure: {:?}",
            result
        );

        if let Err(MacroExpansionError::RoleConstraint {
            role,
            structure_type,
        }) = result
        {
            assert_eq!(role, "gp");
            assert_eq!(structure_type, "sicav");
        }
    }

    #[test]
    fn test_manco_valid_for_sicav_structure() {
        let registry = mock_registry_with_assign_role();
        let session = mock_session_sicav();

        let mut args = HashMap::new();
        args.insert(
            "structure".to_string(),
            "22222222-2222-2222-2222-222222222222".to_string(),
        );
        args.insert("role".to_string(), "manco".to_string()); // ManCo is valid for SICAV
        args.insert(
            "party".to_string(),
            "33333333-3333-3333-3333-333333333333".to_string(),
        );

        let result = expand_macro("structure.assign-role", &args, &session, &registry);
        assert!(
            result.is_ok(),
            "ManCo should be valid for SICAV structure: {:?}",
            result
        );

        let output = result.unwrap();
        assert!(output.statements[0].contains(":role management-company"));
    }

    #[test]
    fn test_manco_fails_on_pe_structure() {
        let registry = mock_registry_with_assign_role();
        let session = mock_session_pe();

        let mut args = HashMap::new();
        args.insert(
            "structure".to_string(),
            "22222222-2222-2222-2222-222222222222".to_string(),
        );
        args.insert("role".to_string(), "manco".to_string()); // ManCo not valid for PE
        args.insert(
            "party".to_string(),
            "33333333-3333-3333-3333-333333333333".to_string(),
        );

        let result = expand_macro("structure.assign-role", &args, &session, &registry);
        assert!(
            matches!(result, Err(MacroExpansionError::RoleConstraint { .. })),
            "ManCo should fail on PE structure: {:?}",
            result
        );

        if let Err(MacroExpansionError::RoleConstraint {
            role,
            structure_type,
        }) = result
        {
            assert_eq!(role, "manco");
            assert_eq!(structure_type, "pe");
        }
    }

    #[test]
    fn test_im_valid_for_any_structure() {
        // Investment Manager has no valid_for restrictions
        let registry = mock_registry_with_assign_role();

        for (structure_type, session_fn) in [
            ("pe", mock_session_pe as fn() -> UnifiedSession),
            ("sicav", mock_session_sicav as fn() -> UnifiedSession),
        ] {
            let session = session_fn();

            let mut args = HashMap::new();
            args.insert(
                "structure".to_string(),
                "22222222-2222-2222-2222-222222222222".to_string(),
            );
            args.insert("role".to_string(), "im".to_string()); // IM has no restrictions
            args.insert(
                "party".to_string(),
                "33333333-3333-3333-3333-333333333333".to_string(),
            );

            let result = expand_macro("structure.assign-role", &args, &session, &registry);
            assert!(
                result.is_ok(),
                "IM should be valid for {} structure: {:?}",
                structure_type,
                result
            );

            let output = result.unwrap();
            assert!(output.statements[0].contains(":role investment-manager"));
        }
    }
}
