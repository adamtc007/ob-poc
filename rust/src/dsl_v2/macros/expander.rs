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
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::registry::MacroRegistry;
use super::schema::MacroSchema;
use super::variable::{substitute_variables, ArgValue, VariableContext, VariableError};
use crate::session::unified::UnifiedSession;

// ---------------------------------------------------------------------------
// ExpansionLimits — configurable bounds for fixpoint expansion (INV-12)
// ---------------------------------------------------------------------------

/// Configurable limits for fixpoint macro expansion.
///
/// Carried inside `MacroExpansionAudit` so that replay can verify the limits
/// haven't changed since compilation (INV-12).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpansionLimits {
    /// Maximum recursion depth for nested macro invocations.
    /// Paper §6.1: default 8.
    pub max_depth: usize,

    /// Maximum total expanded steps across all recursive expansions.
    /// Paper §6.1: default 500.
    pub max_steps: usize,
}

impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_steps: 500,
        }
    }
}

/// Production expansion limits (compile-time constant).
pub const EXPANSION_LIMITS: ExpansionLimits = ExpansionLimits {
    max_depth: 8,
    max_steps: 500,
};

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

    #[error("Maximum expansion depth exceeded: depth {depth} exceeds limit {limit}")]
    MaxDepthExceeded { depth: usize, limit: usize },

    #[error("Maximum expansion steps exceeded: {steps} steps exceeds limit {limit}")]
    MaxStepsExceeded { steps: usize, limit: usize },

    #[error("Cycle detected in macro expansion: {cycle:?}")]
    CycleDetected { cycle: Vec<String> },
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
    let mut ctx = build_variable_context(schema, args, session)?;

    // 5a. Generate placeholder statements for missing args with placeholder-if-missing
    let mut statements = Vec::new();
    for (name, arg_spec) in schema.all_args() {
        if arg_spec.placeholder_if_missing && !args.contains_key(name) {
            // Generate a placeholder entity statement
            let placeholder_stmt = generate_placeholder_statement(name, arg_spec);
            statements.push(placeholder_stmt.clone());

            // Also bind the placeholder reference in context for later use
            let placeholder_ref = format!("@placeholder-{}", name);
            ctx.args
                .insert(name.clone(), ArgValue::literal(&placeholder_ref));
        }
    }

    // 6. Expand templates
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

// ---------------------------------------------------------------------------
// Fixpoint macro expansion — recursive with per-path cycle detection (INV-4)
// ---------------------------------------------------------------------------

/// Fixpoint macro expansion: recursively expands all nested macro invocations
/// until only primitive DSL statements remain.
///
/// ## Invariants
///
/// - **INV-4**: Per-path cycle detection via recursion-stack `Vec<String>`.
///   The same macro may appear in separate non-cyclic branches.
/// - **INV-12**: `ExpansionLimits` snapshot captured in every `MacroExpansionAudit`.
///
/// ## Algorithm
///
/// 1. Call `expand_macro()` (inner pass) to get initial expansion
/// 2. Scan output for `";; @invoke-macro"` directives
/// 3. For each directive, parse the macro FQN + args
/// 4. Check depth limit, step limit, per-path cycle
/// 5. Recursively expand, splicing results in-place
/// 6. Repeat until no directives remain (fixpoint)
pub fn expand_macro_fixpoint(
    macro_fqn: &str,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
    registry: &MacroRegistry,
    limits: ExpansionLimits,
) -> Result<FixpointExpansionOutput, MacroExpansionError> {
    let mut total_steps: usize = 0;
    let mut path: Vec<String> = Vec::new();
    let mut all_audits: Vec<MacroExpansionAudit> = Vec::new();

    let statements = expand_macro_recursive(
        macro_fqn,
        args,
        session,
        registry,
        limits,
        0,
        &mut total_steps,
        &mut path,
        &mut all_audits,
    )?;

    Ok(FixpointExpansionOutput {
        statements,
        audits: all_audits,
        limits,
        total_steps,
    })
}

/// Output from fixpoint expansion — includes all nested audits.
#[derive(Debug, Clone)]
pub struct FixpointExpansionOutput {
    /// Fully expanded DSL statements (no `@invoke-macro` directives remain).
    pub statements: Vec<String>,

    /// Audit records from every expansion (root + nested).
    pub audits: Vec<MacroExpansionAudit>,

    /// The limits that were in effect during expansion (INV-12).
    pub limits: ExpansionLimits,

    /// Total step count across all expansions.
    pub total_steps: usize,
}

/// Inner recursive expansion with per-path cycle detection (INV-4).
///
/// `path` is the recursion stack: push FQN on entry, pop on return.
/// If `path.contains(macro_fqn)` before recursing, we have a cycle.
#[allow(clippy::too_many_arguments)]
fn expand_macro_recursive(
    macro_fqn: &str,
    args: &HashMap<String, String>,
    session: &UnifiedSession,
    registry: &MacroRegistry,
    limits: ExpansionLimits,
    depth: usize,
    total_steps: &mut usize,
    path: &mut Vec<String>,
    all_audits: &mut Vec<MacroExpansionAudit>,
) -> Result<Vec<String>, MacroExpansionError> {
    // Check depth limit
    if depth > limits.max_depth {
        return Err(MacroExpansionError::MaxDepthExceeded {
            depth,
            limit: limits.max_depth,
        });
    }

    // Per-path cycle detection (INV-4): check if this macro is already
    // on the current recursion path. This is per-path, NOT global —
    // the same macro may appear in separate non-cyclic branches.
    if path.contains(&macro_fqn.to_string()) {
        let mut cycle = path.clone();
        cycle.push(macro_fqn.to_string());
        return Err(MacroExpansionError::CycleDetected { cycle });
    }

    // Push onto recursion path
    path.push(macro_fqn.to_string());

    // Expand this macro (inner pass)
    let output = expand_macro(macro_fqn, args, session, registry)?;

    // Record audit
    all_audits.push(output.audit.clone());

    // Process expanded statements, recursively expanding any @invoke-macro directives
    let mut final_statements: Vec<String> = Vec::new();

    for stmt in &output.statements {
        if let Some(invoke) = parse_invoke_macro_directive(stmt) {
            // Recursively expand the nested macro
            let nested_statements = expand_macro_recursive(
                &invoke.macro_id,
                &invoke.args,
                session,
                registry,
                limits,
                depth + 1,
                total_steps,
                path,
                all_audits,
            )?;
            final_statements.extend(nested_statements);
        } else {
            // Primitive statement — count it
            *total_steps += 1;
            if *total_steps > limits.max_steps {
                // Pop before returning error
                path.pop();
                return Err(MacroExpansionError::MaxStepsExceeded {
                    steps: *total_steps,
                    limit: limits.max_steps,
                });
            }
            final_statements.push(stmt.clone());
        }
    }

    // Pop from recursion path on return
    path.pop();

    Ok(final_statements)
}

/// Parsed invoke-macro directive.
struct InvokeMacroDirective {
    macro_id: String,
    args: HashMap<String, String>,
}

/// Parse a `";; @invoke-macro <id> import:[...] args:{json}"` directive.
///
/// Returns `None` if the line is not an invoke-macro directive.
fn parse_invoke_macro_directive(line: &str) -> Option<InvokeMacroDirective> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(";; @invoke-macro ")?;

    // Format: "<macro-id> import:[symbols] args:{json}"
    // Find the macro_id (first token)
    let parts: Vec<&str> = rest.splitn(2, " import:").collect();
    if parts.is_empty() {
        return None;
    }

    let macro_id = parts[0].trim().to_string();

    // Parse args from the "args:{json}" portion
    let args = if parts.len() > 1 {
        if let Some(args_start) = parts[1].find("args:") {
            let json_str = &parts[1][args_start + 5..];
            serde_json::from_str::<HashMap<String, String>>(json_str).unwrap_or_default()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    Some(InvokeMacroDirective { macro_id, args })
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

    // Check required-if conditions on optional args
    for (name, arg_spec) in schema.optional_args() {
        if let Some(required_if) = &arg_spec.required_if {
            if is_required_if_satisfied(required_if, args) && !args.contains_key(name) {
                return Err(MacroExpansionError::MissingRequired(format!(
                    "{} (required because condition '{}' is satisfied)",
                    name,
                    format_required_if(required_if)
                )));
            }
        }
    }

    Ok(())
}

/// Check if a required-if condition is satisfied
fn is_required_if_satisfied(
    expr: &super::schema::RequiredIfExpr,
    args: &HashMap<String, String>,
) -> bool {
    use super::schema::RequiredIfExpr;

    match expr {
        RequiredIfExpr::Simple(condition) => evaluate_simple_required_if(condition, args),
        RequiredIfExpr::Complex(complex) => {
            // any-of: at least one condition must match
            if !complex.any_of.is_empty()
                && !complex
                    .any_of
                    .iter()
                    .any(|c| evaluate_simple_required_if(c, args))
            {
                return false;
            }
            // all-of: all conditions must match
            if !complex.all_of.is_empty()
                && !complex
                    .all_of
                    .iter()
                    .all(|c| evaluate_simple_required_if(c, args))
            {
                return false;
            }
            // If we get here, all conditions are satisfied
            !complex.any_of.is_empty() || !complex.all_of.is_empty()
        }
    }
}

/// Evaluate a simple condition like "structure-type = ucits"
fn evaluate_simple_required_if(condition: &str, args: &HashMap<String, String>) -> bool {
    let condition = condition.trim();

    // Handle equality: "var = value"
    if let Some((lhs, rhs)) = condition.split_once('=') {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        return args.get(lhs).is_some_and(|v| v == rhs);
    }

    // Handle inequality: "var != value"
    if let Some((lhs, rhs)) = condition.split_once("!=") {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        return args.get(lhs).is_none_or(|v| v != rhs);
    }

    // Handle membership: "var in [a, b, c]"
    if let Some((lhs, rhs)) = condition.split_once(" in ") {
        let lhs = lhs.trim();
        let list_str = rhs.trim().trim_start_matches('[').trim_end_matches(']');
        let items: Vec<&str> = list_str.split(',').map(|s| s.trim()).collect();
        return args.get(lhs).is_some_and(|v| items.contains(&v.as_str()));
    }

    // Default: check if variable exists and is truthy
    args.get(condition)
        .is_some_and(|v| !v.is_empty() && v != "false")
}

/// Format a required-if expression for error messages
fn format_required_if(expr: &super::schema::RequiredIfExpr) -> String {
    use super::schema::RequiredIfExpr;

    match expr {
        RequiredIfExpr::Simple(s) => s.clone(),
        RequiredIfExpr::Complex(c) => {
            let mut parts = Vec::new();
            if !c.any_of.is_empty() {
                parts.push(format!("any-of: [{}]", c.any_of.join(", ")));
            }
            if !c.all_of.is_empty() {
                parts.push(format!("all-of: [{}]", c.all_of.join(", ")));
            }
            parts.join(" AND ")
        }
    }
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
    use super::schema::MacroExpansionStep;

    match step {
        MacroExpansionStep::VerbCall(verb_step) => expand_verb_call_step(verb_step, ctx),
        MacroExpansionStep::InvokeMacro(macro_step) => {
            // For now, generate a placeholder that will be recursively expanded
            // The actual recursive expansion happens in the DSL executor
            expand_invoke_macro_step(macro_step, ctx)
        }
        MacroExpansionStep::When(when_step) => expand_when_step(when_step, ctx),
        MacroExpansionStep::ForEach(foreach_step) => expand_foreach_step(foreach_step, ctx),
    }
}

/// Expand multiple steps into a single joined output
fn expand_steps(
    steps: &[super::schema::MacroExpansionStep],
    ctx: &VariableContext,
) -> Result<String, MacroExpansionError> {
    let mut statements = Vec::new();
    for step in steps {
        let dsl = expand_step(step, ctx)?;
        if !dsl.is_empty() {
            statements.push(dsl);
        }
    }
    Ok(statements.join("\n"))
}

/// Expand a when: conditional step
fn expand_when_step(
    step: &super::schema::WhenStep,
    ctx: &VariableContext,
) -> Result<String, MacroExpansionError> {
    use super::conditions::{evaluate_condition, ConditionContext, ConditionResult};

    // Build condition context from variable context
    let args_map = ctx.args_map();
    let scope_map = ctx.scope_map();
    let cond_ctx = ConditionContext::new(&args_map, &scope_map);

    // Evaluate the condition
    match evaluate_condition(&step.when, &cond_ctx) {
        ConditionResult::True => {
            // Expand the 'then' branch
            expand_steps(&step.then, ctx)
        }
        ConditionResult::False => {
            // Expand the 'else' branch if present
            if step.else_branch.is_empty() {
                Ok(String::new())
            } else {
                expand_steps(&step.else_branch, ctx)
            }
        }
        ConditionResult::Unknown(var) => {
            // Condition references unknown variable - treat as false with warning comment
            let warning = format!(
                ";; WARNING: condition references unknown variable '{}', skipping",
                var
            );
            if step.else_branch.is_empty() {
                Ok(warning)
            } else {
                let else_dsl = expand_steps(&step.else_branch, ctx)?;
                Ok(format!("{}\n{}", warning, else_dsl))
            }
        }
    }
}

/// Expand a foreach: loop step
fn expand_foreach_step(
    step: &super::schema::ForEachStep,
    ctx: &VariableContext,
) -> Result<String, MacroExpansionError> {
    use super::variable::substitute_variables;

    // Resolve the source list expression
    let list_str = substitute_variables(&step.in_expr, ctx)?;

    // Parse the list - could be JSON array or comma-separated
    let items: Vec<String> = if list_str.starts_with('[') {
        // Try JSON array parse
        serde_json::from_str(&list_str).unwrap_or_else(|_| {
            // Fallback to comma-separated within brackets
            list_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
    } else {
        // Comma-separated values
        list_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    if items.is_empty() {
        return Ok(String::new());
    }

    // Expand steps for each item
    let mut statements = Vec::new();
    for (index, item) in items.iter().enumerate() {
        // Create a new context with the loop variable bound
        let mut loop_ctx = ctx.clone();
        loop_ctx.bind_loop_var(&step.foreach, item, index);

        let dsl = expand_steps(&step.do_steps, &loop_ctx)?;
        if !dsl.is_empty() {
            statements.push(dsl);
        }
    }

    Ok(statements.join("\n"))
}

/// Expand a verb call step into DSL
fn expand_verb_call_step(
    step: &super::schema::VerbCallStep,
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

    // Add binding if specified
    if let Some(bind_as) = &step.bind_as {
        parts.push(format!(" :as {}", bind_as));
    }

    parts.push(")".to_string());

    Ok(parts.join(""))
}

/// Expand an invoke-macro step
///
/// This generates a comment marker that the executor will handle for recursive expansion.
/// The format is: ;; @invoke-macro <macro-id> <args-json>
/// This allows the DSL pipeline to detect and recursively expand nested macros.
fn expand_invoke_macro_step(
    step: &super::schema::InvokeMacroStep,
    ctx: &VariableContext,
) -> Result<String, MacroExpansionError> {
    // Substitute variables in the args
    let mut resolved_args = HashMap::new();
    for (arg_name, arg_template) in &step.args {
        let value = substitute_variables(arg_template, ctx)?;
        if value != "null" && !value.is_empty() {
            resolved_args.insert(arg_name.clone(), value);
        }
    }

    // Generate a directive comment that the executor will expand recursively
    // Format: ;; @invoke-macro <macro-id> import:[symbols] args:{json}
    let args_json = serde_json::to_string(&resolved_args).unwrap_or_else(|_| "{}".to_string());
    let import_list = step.import_symbols.join(",");

    Ok(format!(
        ";; @invoke-macro {} import:[{}] args:{}",
        step.macro_id, import_list, args_json
    ))
}

/// Generate a placeholder entity statement for a missing arg with placeholder-if-missing
fn generate_placeholder_statement(arg_name: &str, arg_spec: &super::schema::MacroArg) -> String {
    // Determine the entity kind from the arg type or internal config
    let entity_kind = if let Some(internal) = &arg_spec.internal {
        // Use the first kind from internal config if available
        internal
            .kinds
            .first()
            .cloned()
            .unwrap_or_else(|| "entity".to_string())
    } else {
        // Infer from arg type
        match &arg_spec.arg_type {
            super::schema::MacroArgType::PartyRef => "party".to_string(),
            super::schema::MacroArgType::StructureRef => "structure".to_string(),
            super::schema::MacroArgType::ClientRef => "client".to_string(),
            super::schema::MacroArgType::CaseRef => "case".to_string(),
            super::schema::MacroArgType::MandateRef => "mandate".to_string(),
            _ => "entity".to_string(),
        }
    };

    // Generate the placeholder statement with a unique binding
    let binding = format!("@placeholder-{}", arg_name);

    format!(
        "(entity.ensure-or-placeholder :kind \"{}\" :ref \"{}\" :as {})",
        entity_kind, arg_name, binding
    )
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
    target-label: "Structure"
  routing:
    mode-tags: [onboarding]
    operator-domain: structure
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required:
      structure-type:
        type: enum
        ui-label: "Type"
        values:
          - key: pe
            label: "Private Equity"
            internal: private-equity
          - key: sicav
            label: "SICAV"
            internal: sicav
        default-key: pe
      name:
        type: str
        ui-label: "Name"
    optional: {}
  prereqs: []
  expands-to:
    - verb: cbu.create
      args:
        kind: "${arg.structure-type.internal}"
        name: "${arg.name}"
        client-id: "${scope.client_id}"
  sets-state:
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
        args.insert("structure-type".to_string(), "pe".to_string());
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
        assert!(result.statements[0].contains(":client-id 11111111-1111-1111-1111-111111111111"));

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
        args.insert("structure-type".to_string(), "pe".to_string());

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
        args.insert("structure-type".to_string(), "invalid".to_string());
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
    target-label: "Role"
  routing:
    mode-tags: [onboarding]
    operator-domain: structure
  target:
    operates-on: structure-ref
    produces: role-ref
  args:
    style: keyworded
    required:
      structure:
        type: structure-ref
        ui-label: "Structure"
      role:
        type: enum
        ui-label: "Role"
        values:
          - key: gp
            label: "General Partner"
            internal: general-partner
            valid-for: [pe, hedge]
          - key: lp
            label: "Limited Partner"
            internal: limited-partner
            valid-for: [pe, hedge]
          - key: manco
            label: "Management Company"
            internal: management-company
            valid-for: [sicav]
          - key: im
            label: "Investment Manager"
            internal: investment-manager
        default-key: im
      party:
        type: party-ref
        ui-label: "Party"
    optional: {}
  prereqs: []
  expands-to:
    - verb: cbu-role.assign
      args:
        cbu-id: "${arg.structure}"
        role: "${arg.role.internal}"
        entity-id: "${arg.party}"
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

    // =========================================================================
    // Fixpoint Expansion Tests (Phase 1: INV-4, INV-12)
    // =========================================================================

    /// Build a registry with nested macros for fixpoint testing.
    ///
    /// Layout:
    /// - `leaf.alpha` — expands to a single verb call `(alpha.do :x "${arg.x}")`
    /// - `leaf.beta`  — expands to a single verb call `(beta.do :y "${arg.y}")`
    /// - `composite.ab` — invokes `leaf.alpha` then `leaf.beta` (simple nesting)
    /// - `composite.deep` — invokes `composite.ab` (2 levels of nesting)
    fn mock_registry_nested() -> MacroRegistry {
        let yaml = r#"
leaf.alpha:
  kind: macro
  ui:
    label: "Alpha"
    description: "Leaf alpha"
    target-label: "Alpha"
  routing:
    mode-tags: []
    operator-domain: leaf
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required:
      x:
        type: str
        ui-label: "X"
    optional: {}
  prereqs: []
  expands-to:
    - verb: alpha.do
      args:
        x: "${arg.x}"
  unlocks: []

leaf.beta:
  kind: macro
  ui:
    label: "Beta"
    description: "Leaf beta"
    target-label: "Beta"
  routing:
    mode-tags: []
    operator-domain: leaf
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required:
      y:
        type: str
        ui-label: "Y"
    optional: {}
  prereqs: []
  expands-to:
    - verb: beta.do
      args:
        y: "${arg.y}"
  unlocks: []

composite.ab:
  kind: macro
  ui:
    label: "AB Composite"
    description: "Invokes alpha then beta"
    target-label: "AB"
  routing:
    mode-tags: []
    operator-domain: composite
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required:
      x:
        type: str
        ui-label: "X"
      y:
        type: str
        ui-label: "Y"
    optional: {}
  prereqs: []
  expands-to:
    - invoke-macro: leaf.alpha
      args:
        x: "${arg.x}"
    - invoke-macro: leaf.beta
      args:
        y: "${arg.y}"
  unlocks: []

composite.deep:
  kind: macro
  ui:
    label: "Deep Composite"
    description: "Invokes composite.ab"
    target-label: "Deep"
  routing:
    mode-tags: []
    operator-domain: composite
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required:
      x:
        type: str
        ui-label: "X"
      y:
        type: str
        ui-label: "Y"
    optional: {}
  prereqs: []
  expands-to:
    - invoke-macro: composite.ab
      args:
        x: "${arg.x}"
        y: "${arg.y}"
  unlocks: []
"#;

        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(yaml).unwrap();
        let mut registry = MacroRegistry::new();
        for (fqn, schema) in raw {
            registry.add(fqn, schema);
        }
        registry
    }

    #[test]
    fn test_fixpoint_simple_nested() {
        // composite.ab invokes leaf.alpha + leaf.beta → both should expand
        // to primitive verb calls with no @invoke-macro directives remaining.
        let registry = mock_registry_nested();
        let session = mock_session();

        let mut args = HashMap::new();
        args.insert("x".to_string(), "hello".to_string());
        args.insert("y".to_string(), "world".to_string());

        let result =
            expand_macro_fixpoint("composite.ab", &args, &session, &registry, EXPANSION_LIMITS);
        assert!(result.is_ok(), "Fixpoint expansion failed: {:?}", result);

        let output = result.unwrap();

        // Should have 2 primitive statements
        assert_eq!(
            output.statements.len(),
            2,
            "Expected 2 statements, got: {:?}",
            output.statements
        );
        assert!(
            output.statements[0].contains("alpha.do"),
            "First statement should be alpha.do: {}",
            output.statements[0]
        );
        assert!(
            output.statements[0].contains(":x hello"),
            "First statement should contain :x hello: {}",
            output.statements[0]
        );
        assert!(
            output.statements[1].contains("beta.do"),
            "Second statement should be beta.do: {}",
            output.statements[1]
        );
        assert!(
            output.statements[1].contains(":y world"),
            "Second statement should contain :y world: {}",
            output.statements[1]
        );

        // No invoke-macro directives should remain
        for stmt in &output.statements {
            assert!(
                !stmt.contains("@invoke-macro"),
                "Directive should not remain in output: {}",
                stmt
            );
        }

        // Should have 3 audits: composite.ab, leaf.alpha, leaf.beta
        assert_eq!(
            output.audits.len(),
            3,
            "Expected 3 audits (1 composite + 2 leaves), got {}",
            output.audits.len()
        );
        assert_eq!(output.total_steps, 2, "Should count 2 primitive steps");
    }

    #[test]
    fn test_fixpoint_deep_nested() {
        // composite.deep → composite.ab → leaf.alpha + leaf.beta
        // 3 levels of nesting, should fully expand.
        let registry = mock_registry_nested();
        let session = mock_session();

        let mut args = HashMap::new();
        args.insert("x".to_string(), "deep-x".to_string());
        args.insert("y".to_string(), "deep-y".to_string());

        let result = expand_macro_fixpoint(
            "composite.deep",
            &args,
            &session,
            &registry,
            EXPANSION_LIMITS,
        );
        assert!(result.is_ok(), "Deep fixpoint failed: {:?}", result);

        let output = result.unwrap();

        assert_eq!(output.statements.len(), 2);
        assert!(output.statements[0].contains("alpha.do"));
        assert!(output.statements[1].contains("beta.do"));

        // 4 audits: deep, ab, alpha, beta
        assert_eq!(output.audits.len(), 4);
        assert_eq!(output.total_steps, 2);
    }

    #[test]
    fn test_fixpoint_cycle_detection_per_path() {
        // INV-4: Per-path cycle detection.
        // Create A → B → A cycle. Should return CycleDetected.
        let yaml = r#"
cycle.a:
  kind: macro
  ui:
    label: "A"
    description: "A"
    target-label: "A"
  routing:
    mode-tags: []
    operator-domain: cycle
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands-to:
    - invoke-macro: cycle.b
      args: {}
  unlocks: []

cycle.b:
  kind: macro
  ui:
    label: "B"
    description: "B"
    target-label: "B"
  routing:
    mode-tags: []
    operator-domain: cycle
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands-to:
    - invoke-macro: cycle.a
      args: {}
  unlocks: []
"#;

        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(yaml).unwrap();
        let mut registry = MacroRegistry::new();
        for (fqn, schema) in raw {
            registry.add(fqn, schema);
        }

        let session = mock_session();
        let result = expand_macro_fixpoint(
            "cycle.a",
            &HashMap::new(),
            &session,
            &registry,
            EXPANSION_LIMITS,
        );

        assert!(
            matches!(&result, Err(MacroExpansionError::CycleDetected { cycle }) if cycle.len() >= 3),
            "Expected CycleDetected with at least 3 entries (A→B→A), got: {:?}",
            result
        );

        if let Err(MacroExpansionError::CycleDetected { cycle }) = &result {
            // cycle should be [cycle.a, cycle.b, cycle.a]
            assert_eq!(cycle.first().unwrap(), "cycle.a");
            assert_eq!(cycle.last().unwrap(), "cycle.a");
        }
    }

    #[test]
    fn test_fixpoint_diamond_not_cycle() {
        // INV-4: Same macro in separate non-cyclic branches is NOT a cycle.
        // diamond.root → leaf.alpha AND leaf.alpha (via two invoke-macros)
        // This should succeed — alpha appears twice but at different branches,
        // not on the same recursion path.
        let yaml = r#"
diamond.root:
  kind: macro
  ui:
    label: "Root"
    description: "Root"
    target-label: "Root"
  routing:
    mode-tags: []
    operator-domain: diamond
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required:
      x:
        type: str
        ui-label: "X"
    optional: {}
  prereqs: []
  expands-to:
    - invoke-macro: leaf.alpha
      args:
        x: "first"
    - invoke-macro: leaf.alpha
      args:
        x: "second"
  unlocks: []
"#;

        // Start with the nested registry that already has leaf.alpha
        let mut registry = mock_registry_nested();
        // Add the diamond root
        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(yaml).unwrap();
        for (fqn, schema) in raw {
            registry.add(fqn, schema);
        }

        let session = mock_session();
        let mut args = HashMap::new();
        args.insert("x".to_string(), "unused".to_string());

        let result =
            expand_macro_fixpoint("diamond.root", &args, &session, &registry, EXPANSION_LIMITS);

        assert!(
            result.is_ok(),
            "Diamond (same macro in separate branches) should NOT be a cycle: {:?}",
            result
        );

        let output = result.unwrap();
        // leaf.alpha invoked twice → 2 primitive statements
        assert_eq!(output.statements.len(), 2);
        assert!(output.statements[0].contains("alpha.do"));
        assert!(output.statements[0].contains(":x first"));
        assert!(output.statements[1].contains("alpha.do"));
        assert!(output.statements[1].contains(":x second"));
    }

    #[test]
    fn test_fixpoint_depth_limit() {
        // Create a chain of 10 nested macros (depth 0..9).
        // With max_depth=8 (indices 0-8 = 9 levels), depth 9 should fail.
        // chain.0 → chain.1 → ... → chain.9 (leaf)
        // At depth=9 the recursion enters chain.9, which exceeds max_depth=8.
        let mut yaml_parts: Vec<String> = Vec::new();

        for i in 0..10 {
            if i == 9 {
                // Leaf macro — just a verb call
                yaml_parts.push(format!(
                    r#"
chain.{i}:
  kind: macro
  ui:
    label: "Chain {i}"
    description: "Chain {i}"
    target-label: "C{i}"
  routing:
    mode-tags: []
    operator-domain: chain
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required: {{}}
    optional: {{}}
  prereqs: []
  expands-to:
    - verb: leaf.do
      args:
        step: "{i}"
  unlocks: []
"#,
                ));
            } else {
                // Invoke next in chain
                let next = i + 1;
                yaml_parts.push(format!(
                    r#"
chain.{i}:
  kind: macro
  ui:
    label: "Chain {i}"
    description: "Chain {i}"
    target-label: "C{i}"
  routing:
    mode-tags: []
    operator-domain: chain
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required: {{}}
    optional: {{}}
  prereqs: []
  expands-to:
    - invoke-macro: chain.{next}
      args: {{}}
  unlocks: []
"#,
                ));
            }
        }

        let full_yaml = yaml_parts.join("\n");
        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(&full_yaml).unwrap();
        let mut registry = MacroRegistry::new();
        for (fqn, schema) in raw {
            registry.add(fqn, schema);
        }

        let session = mock_session();
        let limits = ExpansionLimits {
            max_depth: 8,
            max_steps: 500,
        };

        let result = expand_macro_fixpoint("chain.0", &HashMap::new(), &session, &registry, limits);

        assert!(
            matches!(&result, Err(MacroExpansionError::MaxDepthExceeded { depth, limit })
                if *limit == 8),
            "Expected MaxDepthExceeded with limit=8, got: {:?}",
            result
        );
    }

    #[test]
    fn test_fixpoint_step_limit() {
        // Create a macro that expands to many verb calls, exceeding max_steps.
        // fan.out invokes fan.leaf 3 times, each producing 1 statement = 3 total.
        // With max_steps=2, should fail on the 3rd step.
        let yaml = r#"
fan.leaf:
  kind: macro
  ui:
    label: "Fan Leaf"
    description: "Fan Leaf"
    target-label: "Leaf"
  routing:
    mode-tags: []
    operator-domain: fan
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required:
      n:
        type: str
        ui-label: "N"
    optional: {}
  prereqs: []
  expands-to:
    - verb: fan.do
      args:
        n: "${arg.n}"
  unlocks: []

fan.out:
  kind: macro
  ui:
    label: "Fan Out"
    description: "Fan Out"
    target-label: "FO"
  routing:
    mode-tags: []
    operator-domain: fan
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    style: keyworded
    required: {}
    optional: {}
  prereqs: []
  expands-to:
    - invoke-macro: fan.leaf
      args:
        n: "1"
    - invoke-macro: fan.leaf
      args:
        n: "2"
    - invoke-macro: fan.leaf
      args:
        n: "3"
  unlocks: []
"#;

        let raw: HashMap<String, MacroSchema> = serde_yaml::from_str(yaml).unwrap();
        let mut registry = MacroRegistry::new();
        for (fqn, schema) in raw {
            registry.add(fqn, schema);
        }

        let session = mock_session();
        let limits = ExpansionLimits {
            max_depth: 8,
            max_steps: 2, // Only allow 2 steps — third should fail
        };

        let result = expand_macro_fixpoint("fan.out", &HashMap::new(), &session, &registry, limits);

        assert!(
            matches!(&result, Err(MacroExpansionError::MaxStepsExceeded { steps, limit })
                if *limit == 2),
            "Expected MaxStepsExceeded with limit=2, got: {:?}",
            result
        );
    }

    #[test]
    fn test_fixpoint_no_comment_directives_in_output() {
        // After fixpoint expansion, no ";; @invoke-macro" directives should remain.
        let registry = mock_registry_nested();
        let session = mock_session();

        // Test leaf (only needs x)
        let mut leaf_args = HashMap::new();
        leaf_args.insert("x".to_string(), "val".to_string());
        let result = expand_macro_fixpoint(
            "leaf.alpha",
            &leaf_args,
            &session,
            &registry,
            EXPANSION_LIMITS,
        );
        assert!(result.is_ok(), "Failed for leaf.alpha: {:?}", result);
        for (i, stmt) in result.unwrap().statements.iter().enumerate() {
            assert!(
                !stmt.contains("@invoke-macro"),
                "Directive found in leaf.alpha output at index {}: {}",
                i,
                stmt
            );
        }

        // Test composite and deep (need both x and y)
        let mut composite_args = HashMap::new();
        composite_args.insert("x".to_string(), "val".to_string());
        composite_args.insert("y".to_string(), "val".to_string());

        for macro_fqn in ["composite.ab", "composite.deep"] {
            let result = expand_macro_fixpoint(
                macro_fqn,
                &composite_args,
                &session,
                &registry,
                EXPANSION_LIMITS,
            );
            assert!(result.is_ok(), "Failed for {}: {:?}", macro_fqn, result);

            let output = result.unwrap();
            for (i, stmt) in output.statements.iter().enumerate() {
                assert!(
                    !stmt.contains("@invoke-macro"),
                    "Directive found in {} output at index {}: {}",
                    macro_fqn,
                    i,
                    stmt
                );
            }
        }
    }

    #[test]
    fn test_expansion_limits_in_audit() {
        // INV-12: Every audit should carry the ExpansionLimits snapshot.
        // The limits in FixpointExpansionOutput should match the input limits.
        let registry = mock_registry_nested();
        let session = mock_session();

        let mut args = HashMap::new();
        args.insert("x".to_string(), "v".to_string());
        args.insert("y".to_string(), "v".to_string());

        let custom_limits = ExpansionLimits {
            max_depth: 4,
            max_steps: 100,
        };

        let result =
            expand_macro_fixpoint("composite.ab", &args, &session, &registry, custom_limits);
        assert!(result.is_ok(), "Expansion failed: {:?}", result);

        let output = result.unwrap();

        // The output limits should match input
        assert_eq!(
            output.limits, custom_limits,
            "Output limits should match input limits"
        );

        // The limits field is carried for INV-12 envelope embedding
        assert_eq!(output.limits.max_depth, 4);
        assert_eq!(output.limits.max_steps, 100);
    }
}
