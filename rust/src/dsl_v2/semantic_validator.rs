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

use crate::dsl_v2::ast::{Argument, AstNode, Literal, Program, Span, Statement, VerbCall};
use crate::dsl_v2::csg_linter::{CsgLinter, LintResult};
use crate::dsl_v2::gateway_resolver::{gateway_addr, GatewayRefResolver};
use crate::dsl_v2::parser::parse_program;
use crate::dsl_v2::ref_resolver::{arg_to_ref_type, RefResolver, ResolveResult};
use crate::dsl_v2::runtime_registry::runtime_registry;
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticBuilder, DiagnosticCode, RefType, RustStyleFormatter, Severity,
    SourceSpan, ValidatedProgram, ValidatedStatement, ValidationContext, ValidationRequest,
    ValidationResult,
};
use crate::dsl_v2::verb_registry::registry;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Semantic validator that checks AST against live database
pub struct SemanticValidator {
    resolver: Box<dyn RefResolver>,
    csg_linter: Option<CsgLinter>,
    pool: PgPool,
}

impl SemanticValidator {
    /// Create a SemanticValidator with EntityGateway resolver
    pub async fn new(pool: PgPool) -> Result<Self, String> {
        let gateway_resolver = GatewayRefResolver::connect(&gateway_addr()).await?;
        Ok(Self {
            resolver: Box::new(gateway_resolver),
            csg_linter: None,
            pool,
        })
    }

    /// Create a SemanticValidator with EntityGateway at specific URL
    pub async fn with_gateway(pool: PgPool, gateway_url: &str) -> Result<Self, String> {
        let gateway_resolver = GatewayRefResolver::connect(gateway_url).await?;
        Ok(Self {
            resolver: Box::new(gateway_resolver),
            csg_linter: None,
            pool,
        })
    }

    /// Initialize with CSG linter for context-sensitive validation
    pub async fn with_csg_linter(mut self) -> Result<Self, String> {
        let mut linter = CsgLinter::new(self.pool.clone());
        linter.initialize().await?;
        self.csg_linter = Some(linter);
        Ok(self)
    }

    /// Run CSG linting pass on parsed AST
    pub async fn lint_csg(
        &self,
        source: &str,
        context: &ValidationContext,
    ) -> Result<LintResult, String> {
        let program = parse_program(source).map_err(|e| format!("Parse error: {}", e))?;

        if let Some(ref linter) = self.csg_linter {
            Ok(linter.lint(program, context, source).await)
        } else {
            // Return empty result if linter not initialized
            Ok(LintResult {
                ast: program,
                diagnostics: vec![],
                inferred_context: Default::default(),
            })
        }
    }

    /// Full validation pipeline with CSG linting
    /// Runs: Parse -> CSG Lint -> Semantic Validation
    pub async fn validate_with_csg(&mut self, request: &ValidationRequest) -> ValidationResult {
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

        // Step 2: CSG Lint (if linter initialized)
        if let Some(ref linter) = self.csg_linter {
            let lint_result = linter
                .lint(program.clone(), &request.context, &request.source)
                .await;
            if lint_result.has_errors() {
                return ValidationResult::Err(lint_result.diagnostics);
            }
        }

        // Step 3: Continue with semantic validation
        self.validate(request).await
    }

    /// Validate DSL source - parse and validate in one step
    /// Returns Rust-style formatted errors or validated program with resolved AST
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

        // Step 2: Walk AST and validate, tracking resolved primary keys
        let mut diagnostics = DiagnosticBuilder::new();
        let mut symbols: HashMap<String, SymbolInfo> = HashMap::new();
        let mut validated_statements = Vec::new();
        // Track resolved primary keys: (statement_idx, arg_key) -> primary_key
        let mut resolved_keys: HashMap<(usize, String), String> = HashMap::new();

        for (stmt_idx, statement) in program.statements.iter().enumerate() {
            match statement {
                Statement::VerbCall(verb_call) => {
                    let validated = self
                        .validate_verb_call_with_resolution(
                            verb_call,
                            &request.source,
                            &request.context,
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

        // Step 4: Build resolved AST with primary_keys populated
        let resolved_ast = self.build_resolved_ast(&program, &resolved_keys);

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
                resolved_ast,
            })
        }
    }

    /// Build a new AST with LookupRef.primary_key values populated from resolution
    fn build_resolved_ast(
        &self,
        program: &Program,
        resolved_keys: &HashMap<(usize, String), String>,
    ) -> Program {
        let mut resolved_statements = Vec::new();

        for (stmt_idx, statement) in program.statements.iter().enumerate() {
            match statement {
                Statement::VerbCall(verb_call) => {
                    let resolved_vc = self.resolve_verb_call(verb_call, stmt_idx, resolved_keys);
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
        &self,
        verb_call: &VerbCall,
        stmt_idx: usize,
        resolved_keys: &HashMap<(usize, String), String>,
    ) -> VerbCall {
        let resolved_args: Vec<Argument> = verb_call
            .arguments
            .iter()
            .map(|arg| {
                let key_str = arg.key.clone();
                let resolved_value =
                    self.resolve_node(&arg.value, stmt_idx, &key_str, resolved_keys);
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

    /// Resolve an AstNode, updating EntityRef.resolved_key if we have a resolution
    fn resolve_node(
        &self,
        node: &AstNode,
        stmt_idx: usize,
        arg_key: &str,
        resolved_keys: &HashMap<(usize, String), String>,
    ) -> AstNode {
        resolve_node_impl(node, stmt_idx, arg_key, resolved_keys)
    }
}

/// Resolve an AstNode, updating EntityRef.resolved_key if we have a resolution
/// (standalone function to avoid clippy warning about unused &self in recursion)
fn resolve_node_impl(
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
        } => {
            // Check if we have a resolution for this argument
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
            }
        }
        AstNode::List { items, span } => {
            // Recursively resolve list items
            AstNode::List {
                items: items
                    .iter()
                    .map(|v| resolve_node_impl(v, stmt_idx, arg_key, resolved_keys))
                    .collect(),
                span: *span,
            }
        }
        AstNode::Map { entries, span } => {
            // Recursively resolve map values
            AstNode::Map {
                entries: entries
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            resolve_node_impl(v, stmt_idx, arg_key, resolved_keys),
                        )
                    })
                    .collect(),
                span: *span,
            }
        }
        // Pass through all other node types unchanged
        _ => node.clone(),
    }
}

impl SemanticValidator {
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
        let verb_def = match registry().get(&verb_call.domain, &verb_call.verb) {
            Some(v) => v,
            None => {
                diagnostics.error(
                    DiagnosticCode::UnknownVerb,
                    span_to_source_span(&verb_call.span, source),
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
                    span_to_source_span(&verb_call.span, source),
                    format!(
                        "verb '{}' is not allowed for intent '{:?}'",
                        full_verb, intent
                    ),
                );
            }
        }

        // 3. Check required arguments are present
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

        // 4. Get fuzzy check configs from YAML for this verb
        let fuzzy_checks = get_fuzzy_checks(&full_verb);
        let mut fuzzy_values: Vec<(FuzzyCheckInfo, String, SourceSpan)> = Vec::new();

        // 5. Validate each argument
        let mut validated_args = HashMap::new();

        // Combine required and optional args for lookup
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

            // Capture values for fuzzy checking (from YAML fuzzy_check config)
            for check in &fuzzy_checks {
                if key == check.arg_name {
                    if let AstNode::Literal(Literal::String(s)) = &arg.value {
                        fuzzy_values.push((
                            FuzzyCheckInfo {
                                arg_name: check.arg_name.clone(),
                                ref_type: check.ref_type,
                                threshold: check.threshold,
                            },
                            s.clone(),
                            span_to_source_span(&arg.span, source),
                        ));
                    }
                }
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
                )
                .await;

            if let Some(r) = resolved {
                validated_args.insert(key, r);
            }
        }

        // 6. Run fuzzy match checks for args with fuzzy_check config in YAML
        for (check_info, value, span) in fuzzy_values {
            self.check_fuzzy_match_warning(
                check_info.ref_type,
                &value,
                span,
                check_info.threshold,
                diagnostics,
            )
            .await;
        }

        // 5. Register symbol binding if present
        if let Some(binding_name) = &verb_call.binding {
            let binding_span = span_to_source_span(&verb_call.span, source);

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
            binding: verb_call.binding.clone(),
            span: span_to_source_span(&verb_call.span, source),
        })
    }

    /// Validate a verb call and track resolved primary keys for AST resolution
    #[allow(clippy::too_many_arguments)]
    async fn validate_verb_call_with_resolution(
        &mut self,
        verb_call: &VerbCall,
        source: &str,
        context: &ValidationContext,
        symbols: &mut HashMap<String, SymbolInfo>,
        diagnostics: &mut DiagnosticBuilder,
        stmt_idx: usize,
        resolved_keys: &mut HashMap<(usize, String), String>,
    ) -> Option<ValidatedStatement> {
        // First, do normal validation
        let result = self
            .validate_verb_call(verb_call, source, context, symbols, diagnostics)
            .await;

        // Extract resolved UUIDs from validated args and store in resolved_keys
        if let Some(ref validated) = result {
            use crate::dsl_v2::validation::ResolvedArg;

            for (arg_key, resolved_arg) in &validated.args {
                if let ResolvedArg::Ref { id, .. } = resolved_arg {
                    // Store the resolved UUID for this argument
                    if *id != Uuid::nil() {
                        resolved_keys.insert((stmt_idx, arg_key.clone()), id.to_string());
                    }
                }
            }
        }

        result
    }

    /// Check for fuzzy matches and emit warnings (driven by YAML fuzzy_check config)
    ///
    /// For args with fuzzy_check configured, we check if similar entities exist.
    /// - Exact match → OK (will upsert existing record)
    /// - Fuzzy match above threshold → WARNING with suggestions
    /// - No match → OK (will create new record)
    async fn check_fuzzy_match_warning(
        &mut self,
        ref_type: RefType,
        value: &str,
        span: SourceSpan,
        threshold: f32,
        diagnostics: &mut DiagnosticBuilder,
    ) {
        // Get the resolver as GatewayRefResolver to access fuzzy search
        let gateway_resolver = match self.resolver.as_gateway_resolver() {
            Some(resolver) => resolver,
            None => return, // Non-gateway resolver, skip fuzzy check
        };

        // First check for exact match
        match gateway_resolver.resolve(ref_type, value).await {
            Ok(ResolveResult::Found { .. }) | Ok(ResolveResult::FoundByCode { .. }) => {
                // Exact match found - this is fine for upsert, will update existing
                return;
            }
            Ok(ResolveResult::NotFound { .. }) | Err(_) => {
                // No exact match - check for fuzzy matches
            }
        }

        // Do fuzzy search to find similar entities
        match gateway_resolver.search_fuzzy(ref_type, value, 5).await {
            Ok(matches) if !matches.is_empty() => {
                // Filter to matches above threshold (from YAML config)
                let similar: Vec<_> = matches
                    .into_iter()
                    .filter(|m| m.score > threshold)
                    .collect();

                if !similar.is_empty() {
                    let type_name = match ref_type {
                        RefType::Cbu => "CBU",
                        RefType::Entity => "entity",
                        RefType::Product => "product",
                        RefType::Service => "service",
                        _ => "record",
                    };

                    // Format the similar matches
                    let suggestions: Vec<String> = similar
                        .iter()
                        .take(3)
                        .map(|m| format!("'{}' (score: {:.0}%)", m.display, m.score * 100.0))
                        .collect();

                    diagnostics.warning(
                        DiagnosticCode::FuzzyMatchWarning,
                        span,
                        format!(
                            "similar {} exists: {}. Did you mean to update an existing record?",
                            type_name,
                            suggestions.join(", ")
                        ),
                    );
                }
            }
            Ok(_) => {
                // No fuzzy matches - this is fine, will create new
            }
            Err(e) => {
                // Log but don't fail validation for gateway errors
                tracing::debug!("Fuzzy search failed: {}", e);
            }
        }
    }

    /// Validate an argument value - check refs against DB
    #[allow(clippy::too_many_arguments)]
    async fn validate_argument_value(
        &mut self,
        verb: &str,
        key: &str,
        node: &AstNode,
        node_span: &Span,
        source: &str,
        symbols: &mut HashMap<String, SymbolInfo>,
        diagnostics: &mut DiagnosticBuilder,
    ) -> Option<crate::dsl_v2::validation::ResolvedArg> {
        use crate::dsl_v2::validation::ResolvedArg;

        let src_span = span_to_source_span(node_span, source);

        match node {
            // Literal values
            AstNode::Literal(lit) => match lit {
                Literal::String(s) => {
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
                Literal::Integer(i) => Some(ResolvedArg::Number(*i as f64)),
                Literal::Decimal(d) => {
                    Some(ResolvedArg::Number(d.to_string().parse().unwrap_or(0.0)))
                }
                Literal::Boolean(b) => Some(ResolvedArg::Boolean(*b)),
                Literal::Null => Some(ResolvedArg::String("null".to_string())),
                Literal::Uuid(uuid) => {
                    // UUID literal - check if it's an attribute or document ref based on context
                    let key_with_colon = format!(":{}", key);
                    if let Some(ref_type) = arg_to_ref_type(verb, &key_with_colon) {
                        match self.resolver.resolve(ref_type, &uuid.to_string()).await {
                            Ok(ResolveResult::Found { id, display }) => Some(ResolvedArg::Ref {
                                ref_type,
                                id,
                                display,
                            }),
                            _ => {
                                diagnostics.error(
                                    DiagnosticCode::InvalidValue,
                                    src_span,
                                    format!("UUID '{}' not found", uuid),
                                );
                                None
                            }
                        }
                    } else {
                        Some(ResolvedArg::String(uuid.to_string()))
                    }
                }
            },

            // Symbol references - check symbol is defined
            AstNode::SymbolRef {
                name,
                span: sym_span,
            } => {
                if let Some(info) = symbols.get_mut(name) {
                    info.used = true;
                    Some(ResolvedArg::Symbol {
                        name: name.clone(),
                        resolved_type: Some(info.ref_type),
                    })
                } else {
                    // Use SymbolRef's span for precise error location
                    let src_span = span_to_source_span(sym_span, source);
                    diagnostics.error(
                        DiagnosticCode::UndefinedSymbol,
                        src_span,
                        format!("undefined symbol '@{}'", name),
                    );
                    None
                }
            }

            // EntityRef - already resolved during prior validation or needs resolution
            AstNode::EntityRef {
                entity_type,
                value,
                resolved_key,
                span: entity_span,
                ..
            } => {
                // Use EntityRef's span for precise error location
                let src_span = span_to_source_span(entity_span, source);
                if let Some(pk) = resolved_key {
                    // Already resolved - validate the primary key still exists
                    let key_with_colon = format!(":{}", key);
                    let ref_type =
                        arg_to_ref_type(verb, &key_with_colon).unwrap_or(RefType::Entity);

                    // Try to parse as UUID, otherwise treat as code
                    if let Ok(uuid) = Uuid::parse_str(pk) {
                        Some(ResolvedArg::Ref {
                            ref_type,
                            id: uuid,
                            display: value.clone(),
                        })
                    } else {
                        // It's a code (like "DIRECTOR" for roles)
                        Some(ResolvedArg::Ref {
                            ref_type,
                            id: Uuid::nil(),
                            display: format!("{}:{}", entity_type, pk),
                        })
                    }
                } else {
                    // Needs resolution - treat like a string lookup
                    let key_with_colon = format!(":{}", key);
                    if let Some(ref_type) = arg_to_ref_type(verb, &key_with_colon) {
                        match self.resolver.resolve(ref_type, value).await {
                            Ok(ResolveResult::Found { id, display }) => Some(ResolvedArg::Ref {
                                ref_type,
                                id,
                                display,
                            }),
                            Ok(ResolveResult::FoundByCode { uuid, display, .. }) => {
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
                                        value, entity_type, suggestion_text
                                    ),
                                );
                                None
                            }
                            Err(e) => {
                                diagnostics.error(
                                    DiagnosticCode::InvalidValue,
                                    src_span,
                                    format!("DB error validating '{}': {}", value, e),
                                );
                                None
                            }
                        }
                    } else {
                        // No lookup config - pass through as string
                        Some(ResolvedArg::String(value.clone()))
                    }
                }
            }

            // Nested verb calls - recursively validate
            AstNode::Nested(nested) => {
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

            // Lists - validate contents recursively
            AstNode::List { items, .. } => {
                let mut resolved_items = Vec::new();
                for item in items {
                    if let Some(r) = Box::pin(self.validate_argument_value(
                        verb,
                        key,
                        item,
                        node_span,
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

            // Maps - validate contents recursively
            AstNode::Map { entries, .. } => {
                let mut resolved_map = HashMap::new();
                for (k, v) in entries {
                    if let Some(r) = Box::pin(self.validate_argument_value(
                        verb,
                        k,
                        v,
                        node_span,
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

/// Info about a fuzzy check to perform on an argument
struct FuzzyCheckInfo {
    /// The argument name to check
    arg_name: String,
    /// The RefType to search against (from YAML fuzzy_check.entity_type)
    ref_type: RefType,
    /// Minimum score threshold for warnings
    threshold: f32,
}

/// Get fuzzy check info for a verb - reads directly from YAML fuzzy_check config
/// Returns list of args that have fuzzy_check configured
fn get_fuzzy_checks(full_verb: &str) -> Vec<FuzzyCheckInfo> {
    let runtime_reg = runtime_registry();

    // Split "domain.verb" into parts
    let Some((domain, verb)) = full_verb.split_once('.') else {
        return Vec::new();
    };

    let Some(runtime_verb) = runtime_reg.get(domain, verb) else {
        return Vec::new();
    };

    // Find all args with fuzzy_check config
    runtime_verb
        .args
        .iter()
        .filter_map(|arg| {
            let fuzzy_config = arg.fuzzy_check.as_ref()?;

            // Convert entity_type string to RefType
            let ref_type = entity_type_to_ref_type(&fuzzy_config.entity_type);

            Some(FuzzyCheckInfo {
                arg_name: arg.name.clone(),
                ref_type,
                threshold: fuzzy_config.threshold,
            })
        })
        .collect()
}

/// Map entity_type string from YAML to RefType
fn entity_type_to_ref_type(entity_type: &str) -> RefType {
    match entity_type {
        "cbu" => RefType::Cbu,
        "entity" => RefType::Entity,
        "product" => RefType::Product,
        "service" => RefType::Service,
        "jurisdiction" => RefType::Jurisdiction,
        "role" => RefType::Role,
        "document_type" => RefType::DocumentType,
        "currency" => RefType::Currency,
        "client_type" => RefType::ClientType,
        _ => RefType::Entity, // Default fallback
    }
}

// =============================================================================
// PUBLIC API - Simple entry point
// =============================================================================

/// Validate DSL source against database via EntityGateway
/// Returns Ok(validated_program) or Err(formatted_error_string)
pub async fn validate_dsl(
    pool: &PgPool,
    source: &str,
    context: ValidationContext,
) -> Result<ValidatedProgram, String> {
    let mut validator = SemanticValidator::new(pool.clone()).await?;

    let request = ValidationRequest {
        source: source.to_string(),
        context,
    };

    match validator.validate(&request).await {
        ValidationResult::Ok(program) => Ok(program),
        ValidationResult::Err(diagnostics) => Err(RustStyleFormatter::format(source, &diagnostics)),
    }
}

/// Validate DSL source with CSG context-sensitive linting
/// Runs full pipeline: Parse -> CSG Lint -> Semantic Validation
/// Returns Ok(validated_program) or Err(formatted_error_string)
pub async fn validate_dsl_with_csg(
    pool: &PgPool,
    source: &str,
    context: ValidationContext,
) -> Result<ValidatedProgram, String> {
    let mut validator = SemanticValidator::new(pool.clone())
        .await?
        .with_csg_linter()
        .await?;

    let request = ValidationRequest {
        source: source.to_string(),
        context,
    };

    match validator.validate_with_csg(&request).await {
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
