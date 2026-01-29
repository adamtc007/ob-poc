//! LSP-Compatible Validator
//!
//! Lightweight validator that uses EntityGateway for reference resolution.
//! This is the **single validation path** shared between LSP and Server.
//!
//! ## Architecture
//! ```text
//! LSP → LspValidator → EntityGateway (gRPC)
//! Server → SemanticValidator (wraps LspValidator + CSG Linter)
//! ```
//!
//! ## Key Design
//! - No PgPool dependency - only uses EntityGateway gRPC
//! - Async validation with full entity resolution
//! - Returns LSP-compatible diagnostics
//! - Builds resolved AST with primary_keys populated

use crate::dsl_v2::ast::{Argument, AstNode, Literal, Program, Span, Statement, VerbCall};
use crate::dsl_v2::gateway_resolver::{gateway_addr, GatewayRefResolver};
use crate::dsl_v2::parser::parse_program;
use crate::dsl_v2::ref_resolver::{arg_to_ref_type, ResolveResult};
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticBuilder, DiagnosticCode, RefType, Severity, SourceSpan, ValidatedProgram,
    ValidatedStatement, ValidationContext,
};
use crate::dsl_v2::verb_registry::registry;
use std::collections::HashMap;
use uuid::Uuid;

/// Lightweight validator using EntityGateway
///
/// This validator is designed to be used by both:
/// - LSP: For real-time diagnostics during editing
/// - Server: As the core validation logic (wrapped by SemanticValidator)
pub struct LspValidator {
    resolver: GatewayRefResolver,
}

impl LspValidator {
    /// Connect to EntityGateway at default address
    pub async fn connect() -> Result<Self, String> {
        Self::connect_to(&gateway_addr()).await
    }

    /// Connect to EntityGateway at specific address
    pub async fn connect_to(addr: &str) -> Result<Self, String> {
        let resolver = GatewayRefResolver::connect(addr).await?;
        Ok(Self { resolver })
    }

    /// Validate DSL source and return diagnostics
    ///
    /// Returns (diagnostics, optional_validated_program)
    /// - If no errors: (warnings_only, Some(validated_program))
    /// - If errors: (all_diagnostics, None)
    pub async fn validate(
        &mut self,
        source: &str,
        context: &ValidationContext,
    ) -> (Vec<Diagnostic>, Option<ValidatedProgram>) {
        // Step 1: Parse
        let program = match parse_program(source) {
            Ok(p) => p,
            Err(e) => {
                return (
                    vec![Diagnostic {
                        severity: Severity::Error,
                        span: SourceSpan::at(1, 0),
                        code: DiagnosticCode::SyntaxError,
                        message: format!("Parse error: {}", e),
                        suggestions: vec![],
                    }],
                    None,
                );
            }
        };

        // Step 2: Validate AST
        let mut diagnostics = DiagnosticBuilder::new();
        let mut symbols: HashMap<String, SymbolInfo> = HashMap::new();
        let mut validated_statements = Vec::new();
        let mut resolved_keys: HashMap<(usize, String), String> = HashMap::new();

        for (stmt_idx, statement) in program.statements.iter().enumerate() {
            if let Statement::VerbCall(verb_call) = statement {
                let validated = self
                    .validate_verb_call(
                        verb_call,
                        source,
                        context,
                        &mut symbols,
                        &mut diagnostics,
                        stmt_idx,
                        &mut resolved_keys,
                    )
                    .await;

                if let Some(v) = validated {
                    validated_statements.push(v);
                }
            }
        }

        // Step 3: Check for unused symbols (warning)
        for (name, info) in &symbols {
            if !info.used {
                diagnostics.warning(
                    DiagnosticCode::UnusedBinding,
                    info.defined_at,
                    format!("symbol '{}' is defined but never used", name),
                );
            }
        }

        // Step 4: Build resolved AST
        let resolved_ast = build_resolved_ast(&program, &resolved_keys);

        // Return result
        let diags = diagnostics.build();
        let has_errors = diags.iter().any(|d| d.severity == Severity::Error);

        if has_errors {
            (diags, None)
        } else {
            let validated = ValidatedProgram {
                source: source.to_string(),
                statements: validated_statements,
                bindings: symbols
                    .into_iter()
                    .map(|(name, info)| {
                        (
                            name.clone(),
                            crate::dsl_v2::validation::BindingInfo {
                                name,
                                ref_type: info.ref_type,
                                defined_at: info.defined_at,
                            },
                        )
                    })
                    .collect(),
                resolved_ast,
            };
            (diags, Some(validated))
        }
    }

    /// Validate a single verb call
    #[allow(clippy::too_many_arguments)]
    async fn validate_verb_call(
        &mut self,
        verb_call: &VerbCall,
        source: &str,
        _context: &ValidationContext,
        symbols: &mut HashMap<String, SymbolInfo>,
        diagnostics: &mut DiagnosticBuilder,
        stmt_idx: usize,
        resolved_keys: &mut HashMap<(usize, String), String>,
    ) -> Option<ValidatedStatement> {
        let full_verb = format!("{}.{}", verb_call.domain, verb_call.verb);

        // 1. Check verb exists
        let verb_def = match registry().get(&verb_call.domain, &verb_call.verb) {
            Some(v) => v,
            None => {
                // Suggest similar verbs
                let reg = registry();
                let suggestions: Vec<String> = reg
                    .all_verbs()
                    .filter(|v| {
                        v.domain == verb_call.domain
                            || v.verb.contains(&verb_call.verb)
                            || v.full_name().contains(&full_verb)
                    })
                    .take(3)
                    .map(|v| v.full_name())
                    .collect();

                let diag = diagnostics.error(
                    DiagnosticCode::UnknownVerb,
                    span_to_source_span(&verb_call.span, source),
                    format!("unknown verb '{}'", full_verb),
                );
                if !suggestions.is_empty() {
                    diag.suggest_one_of(
                        "did you mean",
                        &suggestions.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        &[],
                    );
                }
                return None;
            }
        };

        // 2. Check required arguments are present
        let provided_keys: Vec<String> =
            verb_call.arguments.iter().map(|a| a.key.clone()).collect();

        let required_args = verb_def.required_arg_names();
        for required_arg in &required_args {
            if !provided_keys.contains(&required_arg.to_string()) {
                diagnostics.error(
                    DiagnosticCode::MissingRequiredArg,
                    span_to_source_span(&verb_call.span, source),
                    format!(
                        "missing required argument '{}' for verb '{}'",
                        required_arg, full_verb
                    ),
                );
            }
        }

        // 3. Validate each argument
        let mut validated_args = HashMap::new();
        let optional_args = verb_def.optional_arg_names();
        let all_args: Vec<&str> = required_args
            .iter()
            .chain(optional_args.iter())
            .copied()
            .collect();

        for arg in &verb_call.arguments {
            let key = arg.key.clone();

            // Check argument is known for this verb
            let is_known = all_args.iter().any(|&a| a == key);
            if !is_known {
                diagnostics
                    .error(
                        DiagnosticCode::UnknownArg,
                        span_to_source_span(&arg.span, source),
                        format!("unknown argument '{}' for verb '{}'", key, full_verb),
                    )
                    .suggest_one_of("valid arguments are", &all_args, &[]);
                continue;
            }

            // Validate value based on what ref type it should be
            let resolved = self
                .validate_argument_value(
                    &full_verb,
                    &key,
                    &arg.value,
                    &arg.span,
                    source,
                    symbols,
                    diagnostics,
                    stmt_idx,
                    resolved_keys,
                )
                .await;

            if let Some(r) = resolved {
                validated_args.insert(key, r);
            }
        }

        // 4. Register symbol binding if present
        if let Some(binding_name) = &verb_call.binding {
            let binding_span = span_to_source_span(&verb_call.span, source);

            let return_type = verb_return_type(&full_verb);

            if symbols.contains_key(binding_name) {
                diagnostics.error(
                    DiagnosticCode::DuplicateBinding,
                    binding_span,
                    format!("symbol '{}' is already defined", binding_name),
                );
            } else {
                symbols.insert(
                    binding_name.clone(),
                    SymbolInfo {
                        ref_type: return_type,
                        defined_at: binding_span,
                        used: false,
                    },
                );
            }
        }

        Some(ValidatedStatement {
            verb: full_verb,
            args: validated_args,
            binding: verb_call.binding.clone(),
            span: span_to_source_span(&verb_call.span, source),
        })
    }

    /// Validate an argument value - check refs against EntityGateway
    #[allow(clippy::too_many_arguments)]
    async fn validate_argument_value(
        &mut self,
        verb: &str,
        key: &str,
        value: &AstNode,
        value_span: &Span,
        source: &str,
        symbols: &mut HashMap<String, SymbolInfo>,
        diagnostics: &mut DiagnosticBuilder,
        stmt_idx: usize,
        resolved_keys: &mut HashMap<(usize, String), String>,
    ) -> Option<crate::dsl_v2::validation::ResolvedArg> {
        use crate::dsl_v2::validation::ResolvedArg;

        let src_span = span_to_source_span(value_span, source);

        match value {
            // Literal values
            AstNode::Literal(lit, _) => match lit {
                Literal::String(s) => {
                    let key_with_colon = format!(":{}", key);
                    if let Some(ref_type) = arg_to_ref_type(verb, &key_with_colon) {
                        // Validate against EntityGateway
                        match self.resolver.resolve(ref_type, s).await {
                            Ok(ResolveResult::Found { id, display }) => {
                                resolved_keys.insert((stmt_idx, key.to_string()), id.to_string());
                                Some(ResolvedArg::Ref {
                                    ref_type,
                                    id,
                                    display,
                                })
                            }
                            Ok(ResolveResult::FoundByCode {
                                code,
                                uuid,
                                display,
                            }) => {
                                if let Some(id) = uuid {
                                    resolved_keys
                                        .insert((stmt_idx, key.to_string()), id.to_string());
                                } else {
                                    resolved_keys.insert((stmt_idx, key.to_string()), code.clone());
                                }
                                Some(ResolvedArg::Ref {
                                    ref_type,
                                    id: uuid.unwrap_or_default(),
                                    display,
                                })
                            }
                            Ok(ResolveResult::NotFound { suggestions }) => {
                                let diag = self.resolver.diagnostic_for_failure(
                                    ref_type,
                                    s,
                                    src_span,
                                    &ResolveResult::NotFound { suggestions },
                                );
                                diagnostics.error(diag.code, diag.span, &diag.message);
                                None
                            }
                            Err(e) => {
                                diagnostics.error(
                                    DiagnosticCode::InvalidValue,
                                    src_span,
                                    format!("EntityGateway error validating '{}': {}", s, e),
                                );
                                None
                            }
                        }
                    } else {
                        Some(ResolvedArg::String(s.clone()))
                    }
                }
                Literal::Integer(i) => Some(ResolvedArg::Number(*i as f64)),
                Literal::Decimal(d) => {
                    Some(ResolvedArg::Number(d.to_string().parse().unwrap_or(0.0)))
                }
                Literal::Boolean(b) => Some(ResolvedArg::Boolean(*b)),
                Literal::Null => Some(ResolvedArg::String("null".to_string())),
                Literal::Uuid(u) => {
                    resolved_keys.insert((stmt_idx, key.to_string()), u.to_string());
                    Some(ResolvedArg::String(u.to_string()))
                }
            },

            // Symbol references - check symbol is defined
            AstNode::SymbolRef { name, .. } => {
                if let Some(info) = symbols.get_mut(name) {
                    info.used = true;
                    Some(ResolvedArg::Symbol {
                        name: name.clone(),
                        resolved_type: Some(info.ref_type),
                    })
                } else {
                    diagnostics.error(
                        DiagnosticCode::UnresolvedSymbol,
                        src_span,
                        format!(
                            "unresolved symbol '@{}' - define it with :as @{}",
                            name, name
                        ),
                    );
                    None
                }
            }

            // Entity references - validate or resolve
            AstNode::EntityRef {
                entity_type,
                value: search_value,
                resolved_key,
                ..
            } => {
                if let Some(pk) = resolved_key {
                    // Already resolved
                    if let Ok(uuid) = Uuid::parse_str(pk) {
                        let key_with_colon = format!(":{}", key);
                        let ref_type =
                            arg_to_ref_type(verb, &key_with_colon).unwrap_or(RefType::Entity);
                        Some(ResolvedArg::Ref {
                            ref_type,
                            id: uuid,
                            display: search_value.clone(),
                        })
                    } else {
                        // It's a code
                        Some(ResolvedArg::Ref {
                            ref_type: RefType::Role,
                            id: Uuid::nil(),
                            display: format!("{}:{}", entity_type, pk),
                        })
                    }
                } else {
                    // Needs resolution
                    let key_with_colon = format!(":{}", key);
                    if let Some(ref_type) = arg_to_ref_type(verb, &key_with_colon) {
                        match self.resolver.resolve(ref_type, search_value).await {
                            Ok(ResolveResult::Found { id, display }) => {
                                resolved_keys.insert((stmt_idx, key.to_string()), id.to_string());
                                Some(ResolvedArg::Ref {
                                    ref_type,
                                    id,
                                    display,
                                })
                            }
                            Ok(ResolveResult::FoundByCode {
                                uuid,
                                display,
                                code,
                            }) => {
                                if let Some(id) = uuid {
                                    resolved_keys
                                        .insert((stmt_idx, key.to_string()), id.to_string());
                                } else {
                                    resolved_keys.insert((stmt_idx, key.to_string()), code);
                                }
                                Some(ResolvedArg::Ref {
                                    ref_type,
                                    id: uuid.unwrap_or_default(),
                                    display,
                                })
                            }
                            Ok(ResolveResult::NotFound { suggestions }) => {
                                let suggestion_text = if suggestions.is_empty() {
                                    String::new()
                                } else {
                                    let names: Vec<_> =
                                        suggestions.iter().map(|s| s.display.as_str()).collect();
                                    format!(". Did you mean: {}?", names.join(", "))
                                };
                                diagnostics.error(
                                    DiagnosticCode::InvalidValue,
                                    src_span,
                                    format!(
                                        "'{}' not found for type '{}'{}",
                                        search_value, entity_type, suggestion_text
                                    ),
                                );
                                None
                            }
                            Err(e) => {
                                diagnostics.error(
                                    DiagnosticCode::InvalidValue,
                                    src_span,
                                    format!("EntityGateway error: {}", e),
                                );
                                None
                            }
                        }
                    } else {
                        Some(ResolvedArg::String(search_value.clone()))
                    }
                }
            }

            // Lists - validate contents recursively
            AstNode::List { items, .. } => {
                let mut resolved_items = Vec::new();
                for item in items {
                    if let Some(r) = Box::pin(self.validate_argument_value(
                        verb,
                        key,
                        item,
                        value_span,
                        source,
                        symbols,
                        diagnostics,
                        stmt_idx,
                        resolved_keys,
                    ))
                    .await
                    {
                        resolved_items.push(r);
                    }
                }
                Some(ResolvedArg::List(resolved_items))
            }

            // Maps - validate contents recursively
            AstNode::Map { entries, .. } => {
                let mut resolved_map = HashMap::new();
                for (k, v) in entries {
                    if let Some(r) = Box::pin(self.validate_argument_value(
                        verb,
                        k,
                        v,
                        value_span,
                        source,
                        symbols,
                        diagnostics,
                        stmt_idx,
                        resolved_keys,
                    ))
                    .await
                    {
                        resolved_map.insert(k.clone(), r);
                    }
                }
                Some(ResolvedArg::Map(resolved_map))
            }

            // Nested verb calls - recursively validate
            AstNode::Nested(nested) => {
                let _ = Box::pin(self.validate_verb_call(
                    nested,
                    source,
                    &ValidationContext::default(),
                    symbols,
                    diagnostics,
                    stmt_idx,
                    resolved_keys,
                ))
                .await;
                Some(ResolvedArg::Symbol {
                    name: "_nested".to_string(),
                    resolved_type: None,
                })
            }
        }
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Info about a defined symbol
struct SymbolInfo {
    ref_type: RefType,
    defined_at: SourceSpan,
    used: bool,
}

/// Convert AST Span to validation SourceSpan
fn span_to_source_span(span: &Span, source: &str) -> SourceSpan {
    let mut line = 1u32;
    let mut last_newline = 0usize;

    for (i, ch) in source.char_indices() {
        if i >= span.start {
            break;
        }
        if ch == '\n' {
            line += 1;
            last_newline = i + 1;
        }
    }

    SourceSpan {
        line,
        column: (span.start - last_newline) as u32,
        offset: span.start as u32,
        length: (span.end - span.start) as u32,
    }
}

/// Determine what RefType a verb returns (for symbol bindings)
fn verb_return_type(verb: &str) -> RefType {
    if verb.starts_with("cbu.") {
        RefType::Cbu
    } else if verb.starts_with("entity.") {
        RefType::Entity
    } else if verb.starts_with("document.") {
        RefType::Document
    } else {
        RefType::Entity
    }
}

/// Build resolved AST with primary_keys populated
fn build_resolved_ast(
    program: &Program,
    resolved_keys: &HashMap<(usize, String), String>,
) -> Program {
    let mut resolved_statements = Vec::new();

    for (stmt_idx, statement) in program.statements.iter().enumerate() {
        match statement {
            Statement::VerbCall(verb_call) => {
                let resolved_vc = resolve_verb_call(verb_call, stmt_idx, resolved_keys);
                resolved_statements.push(Statement::VerbCall(resolved_vc));
            }
            Statement::Comment(c) => {
                resolved_statements.push(Statement::Comment(c.clone()));
            }
        }
    }

    Program {
        statements: resolved_statements,
    }
}

/// Create a copy of VerbCall with resolved primary_keys
fn resolve_verb_call(
    verb_call: &VerbCall,
    stmt_idx: usize,
    resolved_keys: &HashMap<(usize, String), String>,
) -> VerbCall {
    let resolved_args: Vec<Argument> = verb_call
        .arguments
        .iter()
        .map(|arg| {
            let key_str = arg.key.clone();
            let resolved_value = resolve_node(&arg.value, stmt_idx, &key_str, resolved_keys);
            Argument {
                key: arg.key.clone(),
                value: resolved_value,
                span: arg.span,
            }
        })
        .collect();

    VerbCall {
        domain: verb_call.domain.clone(),
        verb: verb_call.verb.clone(),
        arguments: resolved_args,
        binding: verb_call.binding.clone(),
        span: verb_call.span,
    }
}

/// Resolve an AstNode, updating EntityRef.resolved_key
fn resolve_node(
    node: &AstNode,
    stmt_idx: usize,
    arg_key: &str,
    resolved_keys: &HashMap<(usize, String), String>,
) -> AstNode {
    match node {
        AstNode::EntityRef {
            entity_type,
            search_column,
            value,
            resolved_key,
            span,
            ref_id,
            explain,
        } => {
            let resolved_pk = resolved_keys
                .get(&(stmt_idx, arg_key.to_string()))
                .cloned()
                .or_else(|| resolved_key.clone());

            AstNode::EntityRef {
                entity_type: entity_type.clone(),
                search_column: search_column.clone(),
                value: value.clone(),
                resolved_key: resolved_pk,
                span: *span,
                ref_id: ref_id.clone(),
                explain: explain.clone(),
            }
        }
        AstNode::List { items, span } => AstNode::List {
            items: items
                .iter()
                .map(|v| resolve_node(v, stmt_idx, arg_key, resolved_keys))
                .collect(),
            span: *span,
        },
        AstNode::Map { entries, span } => AstNode::Map {
            entries: entries
                .iter()
                .map(|(k, v)| (k.clone(), resolve_node(v, stmt_idx, arg_key, resolved_keys)))
                .collect(),
            span: *span,
        },
        _ => node.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_to_source_span() {
        let source = "line one\nline two\nline three";
        let span = Span::new(9, 13);
        let src_span = span_to_source_span(&span, source);
        assert_eq!(src_span.line, 2);
        assert_eq!(src_span.column, 0);
    }

    #[test]
    fn test_verb_return_type() {
        assert_eq!(verb_return_type("cbu.create"), RefType::Cbu);
        assert_eq!(
            verb_return_type("entity.create-proper-person"),
            RefType::Entity
        );
        assert_eq!(verb_return_type("document.catalog"), RefType::Document);
    }
}
