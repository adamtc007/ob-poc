//! Verb → Op Compiler
//!
//! Compiles AST VerbCalls to primitive Ops that can be DAG-sorted and executed.
//!
//! # Design
//!
//! Symbol resolution happens BEFORE compilation. The AST should have resolved
//! EntityRefs with resolved_key populated where possible.
//!
//! The compiler:
//! 1. Walks each VerbCall in the AST
//! 2. Extracts arguments and resolves symbol references
//! 3. Produces one or more Ops per VerbCall
//! 4. Tracks bindings (`:as @name`) in a symbol table
//!
//! # Two-Phase FK Strategy
//!
//! Entities are created with null FKs (Phase 1), then FKs are populated
//! by SetFK ops (Phase 2). This eliminates most circular dependencies.

use crate::ast::{AstNode, Literal, Program, Statement, VerbCall};
use crate::ops::{EntityKey, Op};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

/// Result of compiling a DSL program
#[derive(Debug)]
pub struct CompiledProgram {
    /// Ops in source order (not yet sorted)
    pub ops: Vec<Op>,
    /// Symbol table: binding name → EntityKey
    pub symbols: HashMap<String, EntityKey>,
    /// Errors encountered during compilation
    pub errors: Vec<CompileError>,
}

impl CompiledProgram {
    /// Check if compilation succeeded (no errors)
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Error during compilation
#[derive(Debug, Clone)]
pub struct CompileError {
    /// Source statement index
    pub stmt_idx: usize,
    /// Error message
    pub message: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "statement {}: {}", self.stmt_idx + 1, self.message)
    }
}

/// Consumer-provided verb compilation handler.
///
/// Returns `Some(result)` if the verb was handled, `None` to fall through to
/// the built-in ob-poc dispatch, or `Some(Err(msg))` for a verb that is
/// recognized but has invalid arguments.
pub type VerbHandler = fn(
    vc: &VerbCall,
    stmt_idx: usize,
    symbols: &HashMap<String, EntityKey>,
) -> Option<Result<(Vec<Op>, Option<(String, EntityKey)>), String>>;

/// Compile with an optional consumer verb handler.
///
/// The handler is consulted first. `None` from the handler falls through to
/// the built-in ob-poc dispatch (`compile_ob_poc_verb`).
///
/// Use this to add consumer-specific verbs before the ob-poc fallback.
/// Once the ob-poc verbs migrate to dsl_v2, the fallback will be an error.
pub fn compile_to_ops_ext(program: &Program, handler: Option<VerbHandler>) -> CompiledProgram {
    let mut ops = Vec::new();
    let mut symbols: HashMap<String, EntityKey> = HashMap::new();
    let mut errors = Vec::new();

    for (stmt_idx, stmt) in program.statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            let result = match handler {
                Some(h) => match h(vc, stmt_idx, &symbols) {
                    Some(r) => r,
                    None => Err(format!(
                        "unknown verb for compilation: {}.{}",
                        vc.domain, vc.verb
                    )),
                },
                None => Err(format!(
                    "unknown verb for compilation: {}.{}",
                    vc.domain, vc.verb
                )),
            };

            match result {
                Ok((new_ops, binding)) => {
                    ops.extend(new_ops);
                    if let Some((name, key)) = binding {
                        symbols.insert(name, key);
                    }
                }
                Err(e) => errors.push(CompileError {
                    stmt_idx,
                    message: e,
                }),
            }
        }
    }

    CompiledProgram {
        ops,
        symbols,
        errors,
    }
}

/// Compile an AST Program to Ops.
///
/// Without a handler all verbs produce "unknown verb" errors. Use
/// `compile_to_ops_ext` with a `VerbHandler` to handle consumer-specific
/// verbs (e.g. `ob_poc_compiler::ob_poc_verb_handler`).
pub fn compile_to_ops(program: &Program) -> CompiledProgram {
    compile_to_ops_ext(program, None)
}

/// Compile a runbook-finalisation surface with scoped authoring bindings.
///
/// This is intentionally stricter than the legacy `compile_to_ops` path:
/// bindings are accepted only on supported create-style verbs, uses are checked
/// in source order, and emitted executable ops have authoring binding metadata
/// stripped. The returned symbol table is retained as compiler metadata for
/// diagnostics/audit; it is not encoded into the executable ops.
pub fn compile_scoped_runbook_bindings(program: &Program) -> CompiledProgram {
    let validation_errors = validate_scoped_runbook_bindings(program);
    if !validation_errors.is_empty() {
        return CompiledProgram {
            ops: Vec::new(),
            symbols: HashMap::new(),
            errors: validation_errors,
        };
    }

    let mut compiled = compile_to_ops(program);
    if compiled.is_ok() {
        for op in &mut compiled.ops {
            strip_authoring_binding(op);
        }
    }
    compiled
}

#[derive(Debug, Clone)]
struct ScopedBindingInfo {
    entity_type: &'static str,
}

fn validate_scoped_runbook_bindings(program: &Program) -> Vec<CompileError> {
    let mut bindings: HashMap<String, ScopedBindingInfo> = HashMap::new();
    let declared_names = collect_scoped_binding_names(program);
    let mut errors = Vec::new();

    for (stmt_idx, stmt) in program.statements.iter().enumerate() {
        let Statement::VerbCall(vc) = stmt else {
            continue;
        };

        validate_scoped_binding_uses(vc, stmt_idx, &bindings, &declared_names, &mut errors);

        if let Some(binding) = &vc.binding {
            match scoped_create_output_type(vc) {
                Some(entity_type) => {
                    if bindings.contains_key(binding) {
                        errors.push(CompileError {
                            stmt_idx,
                            message: format!(
                                "Duplicate binding '@{binding}'. Bindings are immutable and may be assigned only once."
                            ),
                        });
                    } else {
                        bindings.insert(binding.clone(), ScopedBindingInfo { entity_type });
                    }
                }
                None => errors.push(CompileError {
                    stmt_idx,
                    message: format!(
                        "Verb '{}.{}' does not declare an entity output. Only create-style verbs may declare :as @alias.",
                        vc.domain, vc.verb
                    ),
                }),
            }
        }
    }

    errors
}

fn collect_scoped_binding_names(program: &Program) -> HashSet<String> {
    program
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::VerbCall(vc) => vc.binding.clone(),
            Statement::Comment(_) => None,
        })
        .collect()
}

fn validate_scoped_binding_uses(
    vc: &VerbCall,
    stmt_idx: usize,
    bindings: &HashMap<String, ScopedBindingInfo>,
    declared_names: &HashSet<String>,
    errors: &mut Vec<CompileError>,
) {
    for arg in &vc.arguments {
        let mut refs = Vec::new();
        collect_symbol_refs(&arg.value, &mut refs);

        for symbol in refs {
            let Some(info) = bindings.get(symbol) else {
                let message = if declared_names.contains(symbol) {
                    format!(
                        "Binding '@{symbol}' is used before it is declared. Bindings must be produced by an earlier statement in the same runbook."
                    )
                } else {
                    format!(
                        "Undefined binding '@{symbol}'. No earlier create statement declares :as @{symbol} in this runbook."
                    )
                };
                errors.push(CompileError { stmt_idx, message });
                continue;
            };

            if let Some(expected_type) = expected_scoped_arg_type(vc, &arg.key) {
                if !scoped_type_matches(info.entity_type, expected_type) {
                    errors.push(CompileError {
                        stmt_idx,
                        message: format!(
                            "Type mismatch for argument '{}' of '{}.{}'. Expected '{}'; found '@{} : {}'.",
                            arg.key, vc.domain, vc.verb, expected_type, symbol, info.entity_type
                        ),
                    });
                }
            }
        }
    }
}

fn scoped_create_output_type(vc: &VerbCall) -> Option<&'static str> {
    match (vc.domain.as_str(), vc.verb.as_str()) {
        ("cbu", "create") => Some("cbu"),
        _ => None,
    }
}

fn expected_scoped_arg_type(vc: &VerbCall, arg_key: &str) -> Option<&'static str> {
    match (vc.domain.as_str(), vc.verb.as_str(), arg_key) {
        ("cbu", "assign-role", "cbu-id") | ("cbu", "remove-role", "cbu-id") => Some("cbu"),
        ("cbu", "assign-role", "entity-id") | ("cbu", "remove-role", "entity-id") => Some("entity"),
        ("kyc-case", "create", "cbu-id") => Some("cbu"),
        ("cbu-custody", "add-universe", "cbu-id")
        | ("cbu-custody", "create-ssi", "cbu-id")
        | ("cbu-custody", "add-booking-rule", "cbu-id")
        | ("trading-profile", "import", "cbu-id")
        | ("document", "catalog", "cbu-id")
        | ("cbu", "attach-evidence", "cbu-id") => Some("cbu"),
        _ => None,
    }
}

fn scoped_type_matches(actual: &str, expected: &str) -> bool {
    actual == expected || expected == "entity" && actual != "cbu"
}

fn collect_symbol_refs<'a>(node: &'a AstNode, refs: &mut Vec<&'a str>) {
    match node {
        AstNode::SymbolRef { name, .. } => refs.push(name),
        AstNode::List { items, .. } => {
            for item in items {
                collect_symbol_refs(item, refs);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_key, value) in entries {
                collect_symbol_refs(value, refs);
            }
        }
        AstNode::Nested(call) => {
            for arg in &call.arguments {
                collect_symbol_refs(&arg.value, refs);
            }
        }
        _ => {}
    }
}

fn strip_authoring_binding(op: &mut Op) {
    match op {
        Op::EnsureEntity { binding, .. }
        | Op::CreateCase { binding, .. }
        | Op::CreateWorkstream { binding, .. }
        | Op::CreateSSI { binding, .. }
        | Op::GenericCrud { binding, .. } => *binding = None,
        _ => {}
    }
}

// =============================================================================
// Helper functions
// =============================================================================

/// Get string argument from VerbCall
pub fn get_string_arg(vc: &VerbCall, key: &str) -> Result<String, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_string(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get decimal argument from VerbCall
pub fn get_decimal_arg(vc: &VerbCall, key: &str) -> Result<Decimal, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_decimal(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get integer argument from VerbCall
pub fn get_int_arg(vc: &VerbCall, key: &str) -> Result<i32, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_int(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get boolean argument from VerbCall
pub fn get_bool_arg(vc: &VerbCall, key: &str) -> Result<bool, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_bool(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get string list argument from VerbCall
pub fn get_string_list_arg(vc: &VerbCall, key: &str) -> Result<Vec<String>, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_string_list(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Resolve an entity argument (symbol ref or literal)
pub fn resolve_entity_arg(
    vc: &VerbCall,
    key: &str,
    symbols: &HashMap<String, EntityKey>,
) -> Result<EntityKey, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return resolve_to_entity_key(&arg.value, symbols);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Resolve an AstNode to an EntityKey
pub fn resolve_to_entity_key(
    node: &AstNode,
    symbols: &HashMap<String, EntityKey>,
) -> Result<EntityKey, String> {
    match node {
        // Symbol reference → look up in symbol table
        AstNode::SymbolRef { name, .. } => symbols
            .get(name)
            .cloned()
            .ok_or_else(|| format!("undefined symbol '@{}'", name)),

        // String literal → create entity key from value
        AstNode::Literal(Literal::String(s), _) => Ok(EntityKey::new("entity", s)),

        // EntityRef → use resolved key or value
        AstNode::EntityRef {
            entity_type,
            value,
            resolved_key,
            ..
        } => {
            let key_value = resolved_key.as_ref().unwrap_or(value);
            Ok(EntityKey::new(entity_type, key_value))
        }

        _ => Err(format!("cannot resolve {:?} to entity key", node)),
    }
}

/// Extract string from AstNode
pub fn extract_string(node: &AstNode) -> Result<String, String> {
    match node {
        AstNode::Literal(Literal::String(s), _) => Ok(s.clone()),
        AstNode::EntityRef { value, .. } => Ok(value.clone()),
        _ => Err(format!("expected string, got {:?}", node)),
    }
}

/// Extract decimal from AstNode
fn extract_decimal(node: &AstNode) -> Result<Decimal, String> {
    match node {
        AstNode::Literal(Literal::Decimal(d), _) => Ok(*d),
        AstNode::Literal(Literal::Integer(i), _) => Ok(Decimal::from(*i)),
        AstNode::Literal(Literal::String(s), _) => {
            Decimal::from_str(s).map_err(|e| format!("invalid decimal '{}': {}", s, e))
        }
        _ => Err(format!("expected number, got {:?}", node)),
    }
}

/// Extract integer from AstNode
fn extract_int(node: &AstNode) -> Result<i32, String> {
    match node {
        AstNode::Literal(Literal::Integer(i), _) => (*i)
            .try_into()
            .map_err(|_| format!("integer {} out of i32 range", i)),
        AstNode::Literal(Literal::Decimal(d), _) => {
            // Try to convert decimal to i32 if it's a whole number
            use rust_decimal::prelude::ToPrimitive;
            d.to_i32()
                .ok_or_else(|| format!("decimal {} cannot be converted to i32", d))
        }
        AstNode::Literal(Literal::String(s), _) => s
            .parse::<i32>()
            .map_err(|e| format!("invalid integer '{}': {}", s, e)),
        _ => Err(format!("expected integer, got {:?}", node)),
    }
}

/// Extract boolean from AstNode
fn extract_bool(node: &AstNode) -> Result<bool, String> {
    match node {
        AstNode::Literal(Literal::Boolean(b), _) => Ok(*b),
        AstNode::Literal(Literal::String(s), _) => match s.to_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(true),
            "false" | "no" | "0" => Ok(false),
            _ => Err(format!("invalid boolean '{}'", s)),
        },
        _ => Err(format!("expected boolean, got {:?}", node)),
    }
}

/// Extract string list from AstNode
fn extract_string_list(node: &AstNode) -> Result<Vec<String>, String> {
    match node {
        AstNode::List { items, .. } => {
            let mut result = Vec::new();
            for item in items {
                result.push(extract_string(item)?);
            }
            Ok(result)
        }
        // Single string becomes single-element list
        AstNode::Literal(Literal::String(s), _) => Ok(vec![s.clone()]),
        _ => Err(format!("expected list, got {:?}", node)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_program;

    #[test]
    fn test_unknown_verb_returns_error_without_handler() {
        // Without a handler, all verbs produce "unknown verb" errors.
        // Consumer-specific verbs are handled by passing a VerbHandler to compile_to_ops_ext.
        let source = r#"(cbu.ensure :name "Apex Fund")"#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops(&program);
        assert!(!compiled.is_ok());
        assert!(compiled.errors[0].message.contains("unknown verb"));
    }

    #[test]
    fn test_compile_to_ops_ext_with_handler() {
        // A handler that accepts one verb and rejects others.
        fn accept_one(
            vc: &VerbCall,
            stmt_idx: usize,
            _syms: &HashMap<String, EntityKey>,
        ) -> Option<Result<(Vec<Op>, Option<(String, EntityKey)>), String>> {
            if vc.domain == "test" && vc.verb == "noop" {
                let key = EntityKey::new("test", "obj");
                Some(Ok((
                    vec![Op::EnsureEntity {
                        entity_type: "test".to_string(),
                        key: key.clone(),
                        attrs: Default::default(),
                        binding: vc.binding.clone(),
                        source_stmt: stmt_idx,
                    }],
                    vc.binding.as_ref().map(|b| (b.clone(), key)),
                )))
            } else {
                None
            }
        }

        let source = r#"(test.noop :as @x)"#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops_ext(&program, Some(accept_one));
        assert!(compiled.is_ok(), "{:?}", compiled.errors);
        assert_eq!(compiled.ops.len(), 1);
        assert!(compiled.symbols.contains_key("x"));
    }

    #[test]
    fn test_handler_fallthrough_becomes_error() {
        fn reject_all(
            _vc: &VerbCall,
            _stmt_idx: usize,
            _syms: &HashMap<String, EntityKey>,
        ) -> Option<Result<(Vec<Op>, Option<(String, EntityKey)>), String>> {
            None // never handles anything
        }

        let source = r#"(some.verb :x "y")"#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops_ext(&program, Some(reject_all));
        assert!(!compiled.is_ok());
        assert!(compiled.errors[0].message.contains("unknown verb"));
    }
}
