//! Semantic Validator - Walks AST and validates all tokens against DB
//!
//! This is the TIGHT integration between parser and DB validation.
//! Every token that references external data is validated:
//! - Document types must exist in document_types
//! - Jurisdictions must exist in master_jurisdictions
//! - Roles must exist in roles
//! - Entity types must exist in entity_types
//! - Attribute IDs must exist in attribute_registry
//! - UUID references must resolve to existing records
//!
//! The validator walks the entire AST and collects ALL errors before returning,
//! providing a complete diagnostic report (like rustc).

use crate::dsl_v2::ast::{Span, Statement, Value, VerbCall};
use crate::dsl_v2::parser::parse_program;
use crate::dsl_v2::ref_resolver::{arg_to_ref_type, RefTypeResolver, ResolveResult};
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticBuilder, DiagnosticCode, RefType, RustStyleFormatter, Severity,
    SourceSpan, ValidatedProgram, ValidatedStatement, ValidationContext, ValidationRequest,
    ValidationResult,
};
use crate::dsl_v2::verbs::find_verb;
use sqlx::PgPool;
use std::collections::HashMap;

/// Semantic validator that checks AST against live database
pub struct SemanticValidator {
    resolver: RefTypeResolver,
}

impl SemanticValidator {
    pub fn new(pool: PgPool) -> Self {
        Self {
            resolver: RefTypeResolver::new(pool),
        }
    }

    /// Validate DSL source - parse and validate in one step
    /// Returns Rust-style formatted errors or validated program
    pub async fn validate(&mut self, request: &ValidationRequest) -> ValidationResult {
        // Clear resolver cache for fresh validation
        self.resolver.clear_cache();

        // Step 1: Parse
        let program = match parse_program(&request.source) {
            Ok(p) => p,
            Err(e) => {
                return ValidationResult::Err(vec![Diagnostic {
                    severity: Severity::Error,
                    span: SourceSpan::at(1, 0),
                    code: DiagnosticCode::SyntaxError,
                    message: format!("Parse error: {}", e),
                    suggestions: vec![],
                }]);
            }
        };

        // Step 2: Walk AST and validate
        let mut diagnostics = DiagnosticBuilder::new();
        let mut symbols: HashMap<String, SymbolInfo> = HashMap::new();
        let mut validated_statements = Vec::new();

        for statement in &program.statements {
            match statement {
                Statement::VerbCall(verb_call) => {
                    let validated = self
                        .validate_verb_call(
                            verb_call,
                            &request.source,
                            &request.context,
                            &mut symbols,
                            &mut diagnostics,
                        )
                        .await;

                    if let Some(v) = validated {
                        validated_statements.push(v);
                    }
                }
                Statement::Comment(_) => {
                    // Comments don't need validation
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

        // Return result
        if diagnostics.has_errors() {
            ValidationResult::Err(diagnostics.build())
        } else {
            ValidationResult::Ok(ValidatedProgram {
                source: request.source.clone(),
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
            })
        }
    }

    /// Validate a single verb call
    async fn validate_verb_call(
        &mut self,
        verb_call: &VerbCall,
        source: &str,
        context: &ValidationContext,
        symbols: &mut HashMap<String, SymbolInfo>,
        diagnostics: &mut DiagnosticBuilder,
    ) -> Option<ValidatedStatement> {
        let full_verb = format!("{}.{}", verb_call.domain, verb_call.verb);

        // 1. Check verb exists
        let verb_def = match find_verb(&verb_call.domain, &verb_call.verb) {
            Some(v) => v,
            None => {
                diagnostics.error(
                    DiagnosticCode::UnknownVerb,
                    span_to_source_span(&verb_call.verb_span, source),
                    format!("unknown verb '{}'", full_verb),
                );
                return None;
            }
        };

        // 2. Check verb is allowed for intent (if context specifies intent)
        if let Some(intent) = &context.intent {
            if !is_verb_allowed_for_intent(&full_verb, intent) {
                diagnostics.error(
                    DiagnosticCode::VerbNotAllowedForIntent,
                    span_to_source_span(&verb_call.verb_span, source),
                    format!(
                        "verb '{}' is not allowed for intent '{:?}'",
                        full_verb, intent
                    ),
                );
            }
        }

        // 3. Check required arguments are present
        let provided_keys: Vec<String> = verb_call
            .arguments
            .iter()
            .map(|a| a.key.canonical())
            .collect();

        for required_arg in verb_def.required_args {
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

        // 4. Validate each argument
        let mut validated_args = HashMap::new();

        // Combine required and optional args for lookup
        let all_args: Vec<&str> = verb_def
            .required_args
            .iter()
            .chain(verb_def.optional_args.iter())
            .copied()
            .collect();

        for arg in &verb_call.arguments {
            let key = arg.key.canonical();

            // Check argument is known for this verb
            let is_known = all_args.iter().any(|&a| a == key);
            if !is_known {
                diagnostics
                    .error(
                        DiagnosticCode::UnknownArg,
                        span_to_source_span(&arg.key_span, source),
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
                    &arg.value_span,
                    source,
                    symbols,
                    diagnostics,
                )
                .await;

            if let Some(r) = resolved {
                validated_args.insert(key, r);
            }
        }

        // 5. Register symbol binding if present
        if let Some(binding_name) = &verb_call.as_binding {
            let binding_span = verb_call
                .as_binding_span
                .map(|s| span_to_source_span(&s, source))
                .unwrap_or_default();

            // Determine what type this verb returns
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
            binding: verb_call.as_binding.clone(),
            span: span_to_source_span(&verb_call.span, source),
        })
    }

    /// Validate an argument value - check refs against DB
    async fn validate_argument_value(
        &mut self,
        verb: &str,
        key: &str,
        value: &Value,
        value_span: &Span,
        source: &str,
        symbols: &mut HashMap<String, SymbolInfo>,
        diagnostics: &mut DiagnosticBuilder,
    ) -> Option<crate::dsl_v2::validation::ResolvedArg> {
        use crate::dsl_v2::validation::ResolvedArg;

        let src_span = span_to_source_span(value_span, source);

        match value {
            // String values - may need DB validation based on arg key
            Value::String(s) => {
                // Check if this arg needs DB validation
                let key_with_colon = format!(":{}", key);
                if let Some(ref_type) = arg_to_ref_type(verb, &key_with_colon) {
                    // Validate against DB
                    match self.resolver.resolve(ref_type, s).await {
                        Ok(ResolveResult::Found { id, display }) => Some(ResolvedArg::Ref {
                            ref_type,
                            id,
                            display,
                        }),
                        Ok(ResolveResult::FoundByCode {
                            code: _,
                            uuid,
                            display,
                        }) => Some(ResolvedArg::Ref {
                            ref_type,
                            id: uuid.unwrap_or_default(),
                            display,
                        }),
                        Ok(ResolveResult::NotFound { suggestions }) => {
                            let diag = self.resolver.diagnostic_for_failure(
                                ref_type,
                                s,
                                src_span,
                                &ResolveResult::NotFound { suggestions },
                            );
                            diagnostics.error(diag.code, diag.span, &diag.message);
                            // Add suggestions
                            if !diag.suggestions.is_empty() {
                                // Can't easily add suggestions through DiagnosticBuilder
                                // The diagnostic was already created with them
                            }
                            None
                        }
                        Err(e) => {
                            diagnostics.error(
                                DiagnosticCode::InvalidValue,
                                src_span,
                                format!("DB error validating '{}': {}", s, e),
                            );
                            None
                        }
                    }
                } else {
                    // No DB validation needed, pass through
                    Some(ResolvedArg::String(s.clone()))
                }
            }

            // Symbol references - check symbol is defined
            Value::Reference(name) => {
                if let Some(info) = symbols.get_mut(name) {
                    info.used = true;
                    Some(ResolvedArg::Symbol {
                        name: name.clone(),
                        resolved_type: Some(info.ref_type),
                    })
                } else {
                    diagnostics.error(
                        DiagnosticCode::UndefinedSymbol,
                        src_span,
                        format!("undefined symbol '@{}'", name),
                    );
                    None
                }
            }

            // Typed refs - validate UUID exists
            Value::AttributeRef(uuid) => {
                match self
                    .resolver
                    .resolve(RefType::AttributeId, &uuid.to_string())
                    .await
                {
                    Ok(ResolveResult::Found { id, display }) => Some(ResolvedArg::Ref {
                        ref_type: RefType::AttributeId,
                        id,
                        display,
                    }),
                    _ => {
                        diagnostics.error(
                            DiagnosticCode::UnknownAttributeId,
                            src_span,
                            format!("attribute UUID '{}' not found", uuid),
                        );
                        None
                    }
                }
            }

            Value::DocumentRef(uuid) => {
                match self
                    .resolver
                    .resolve(RefType::Document, &uuid.to_string())
                    .await
                {
                    Ok(ResolveResult::Found { id, display }) => Some(ResolvedArg::Ref {
                        ref_type: RefType::Document,
                        id,
                        display,
                    }),
                    _ => {
                        diagnostics.error(
                            DiagnosticCode::DocumentNotFound,
                            src_span,
                            format!("document UUID '{}' not found", uuid),
                        );
                        None
                    }
                }
            }

            // Nested verb calls - recursively validate
            Value::NestedCall(nested) => {
                // Validate the nested call
                let _ = Box::pin(self.validate_verb_call(
                    nested,
                    source,
                    &ValidationContext::default(),
                    symbols,
                    diagnostics,
                ))
                .await;
                // Return placeholder - actual resolution happens at execution
                Some(ResolvedArg::Symbol {
                    name: "_nested".to_string(),
                    resolved_type: None,
                })
            }

            // Pass through simple types
            Value::Integer(i) => Some(ResolvedArg::Number(*i as f64)),
            Value::Decimal(d) => Some(ResolvedArg::Number(d.to_string().parse().unwrap_or(0.0))),
            Value::Boolean(b) => Some(ResolvedArg::Boolean(*b)),
            Value::Null => Some(ResolvedArg::String("null".to_string())),

            // Lists and maps - validate contents recursively
            Value::List(items) => {
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
                    ))
                    .await
                    {
                        resolved_items.push(r);
                    }
                }
                Some(ResolvedArg::List(resolved_items))
            }

            Value::Map(entries) => {
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
                    ))
                    .await
                    {
                        resolved_map.insert(k.clone(), r);
                    }
                }
                Some(ResolvedArg::Map(resolved_map))
            }
        }
    }
}

/// Info about a defined symbol
struct SymbolInfo {
    ref_type: RefType,
    defined_at: SourceSpan,
    used: bool,
}

/// Convert AST Span to validation SourceSpan
fn span_to_source_span(span: &Span, source: &str) -> SourceSpan {
    // Calculate line and column from byte offset
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

/// Check if a verb is allowed for the given intent
fn is_verb_allowed_for_intent(verb: &str, intent: &crate::dsl_v2::validation::Intent) -> bool {
    use crate::dsl_v2::validation::Intent;

    match intent {
        Intent::OnboardIndividual | Intent::OnboardCorporate => {
            // Onboarding can use: cbu.*, entity.*, document.catalog, document.extract
            verb.starts_with("cbu.")
                || verb.starts_with("entity.")
                || verb == "document.catalog"
                || verb == "document.extract"
        }
        Intent::AddDocument => verb == "document.catalog" || verb == "document.extract",
        Intent::AddEntity | Intent::LinkEntityRole => {
            verb.starts_with("entity.") || verb == "cbu.assign-role"
        }
        Intent::ExtractDocument => verb == "document.extract",
        Intent::LinkDocumentToEntity => verb == "document.link-entity",
        Intent::GetCbuStatus | Intent::ListDocuments | Intent::ListEntities => {
            // Query intents - read-only verbs
            verb == "cbu.get-status" || verb == "document.list" || verb == "entity.list"
        }
        Intent::RunScreening | Intent::RunKycCheck => {
            verb.starts_with("screening.") || verb.starts_with("kyc.")
        }
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
        RefType::Entity // Default fallback
    }
}

// =============================================================================
// PUBLIC API - Simple entry point
// =============================================================================

/// Validate DSL source against database
/// Returns Ok(validated_program) or Err(formatted_error_string)
pub async fn validate_dsl(
    pool: &PgPool,
    source: &str,
    context: ValidationContext,
) -> Result<ValidatedProgram, String> {
    let mut validator = SemanticValidator::new(pool.clone());

    let request = ValidationRequest {
        source: source.to_string(),
        context,
    };

    match validator.validate(&request).await {
        ValidationResult::Ok(program) => Ok(program),
        ValidationResult::Err(diagnostics) => Err(RustStyleFormatter::format(source, &diagnostics)),
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_to_source_span() {
        let source = "line one\nline two\nline three";

        // Position at start of line 2
        let span = Span::new(9, 13); // "line" on line 2
        let src_span = span_to_source_span(&span, source);

        assert_eq!(src_span.line, 2);
        assert_eq!(src_span.column, 0);
    }

    #[test]
    fn test_verb_return_type() {
        assert_eq!(verb_return_type("cbu.create"), RefType::Cbu);
        assert_eq!(
            verb_return_type("entity.create-natural-person"),
            RefType::Entity
        );
        assert_eq!(verb_return_type("document.catalog"), RefType::Document);
    }

    #[test]
    fn test_is_verb_allowed_for_intent() {
        use crate::dsl_v2::validation::Intent;

        assert!(is_verb_allowed_for_intent(
            "cbu.create",
            &Intent::OnboardIndividual
        ));
        assert!(is_verb_allowed_for_intent(
            "document.catalog",
            &Intent::OnboardIndividual
        ));
        assert!(!is_verb_allowed_for_intent(
            "screening.pep",
            &Intent::OnboardIndividual
        ));

        assert!(is_verb_allowed_for_intent(
            "document.catalog",
            &Intent::AddDocument
        ));
        assert!(!is_verb_allowed_for_intent(
            "cbu.create",
            &Intent::AddDocument
        ));
    }
}
