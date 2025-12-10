//! Context-Sensitive Grammar Linter
//!
//! Validates DSL programs against business rules that depend on runtime context.
//! This is the core orchestration module for CSG validation.
//!
//! # Pipeline Position
//! ```text
//! Parser → AST → [CSG Linter] → SemanticValidator → Executor
//! ```
//!
//! # Three-Pass Architecture
//! 1. **Symbol Analysis**: Build symbol table, infer types
//! 2. **Reference Validation**: Check cross-statement references
//! 3. **Applicability Validation**: Enforce business rules from DB

use crate::dsl_v2::applicability_rules::ApplicabilityRules;
use crate::dsl_v2::ast::{AstNode, Program, Span, Statement, VerbCall};
#[cfg(feature = "database")]
use crate::dsl_v2::semantic_context::SemanticContextStore;
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticCode, Severity, SourceSpan, Suggestion, ValidationContext,
};
use crate::dsl_v2::verb_registry::registry;
use std::collections::HashMap;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// PUBLIC TYPES
// =============================================================================

/// Result of CSG linting
#[derive(Debug)]
pub struct LintResult {
    /// The original AST (passed through)
    pub ast: Program,
    /// Diagnostics generated during linting
    pub diagnostics: Vec<Diagnostic>,
    /// Context inferred from AST analysis
    pub inferred_context: InferredContext,
}

impl LintResult {
    /// Returns true if there are any errors (invalid or incomplete DSL)
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Returns true if there are any warnings
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning)
    }

    /// Returns true if DSL has unresolved symbol errors (incomplete but fixable)
    /// UI can use this to show "incomplete" state vs other errors
    pub fn has_unresolved_symbols(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnresolvedSymbol)
    }

    /// Returns true if DSL is incomplete (only has unresolved symbol errors)
    /// This means the user just needs to add definitions, not fix invalid syntax
    pub fn is_incomplete(&self) -> bool {
        self.has_errors()
            && self
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .all(|d| d.code == DiagnosticCode::UnresolvedSymbol)
    }

    /// Returns true if DSL is valid and complete (no errors)
    /// This means the DSL is ready to execute
    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }

    /// Returns true if DSL is invalid (has errors other than unresolved symbols)
    pub fn is_invalid(&self) -> bool {
        self.has_errors() && !self.is_incomplete()
    }
}

/// Context inferred from AST analysis
#[derive(Debug, Default)]
pub struct InferredContext {
    /// Symbol bindings: name → type info
    pub symbols: HashMap<String, SymbolInfo>,
    /// Operations that create CBUs
    pub cbu_creates: Vec<CbuCreate>,
    /// Operations that create entities
    pub entity_creates: Vec<EntityCreate>,
    /// Operations that reference entities
    pub entity_refs: Vec<EntityRef>,
    /// Operations that catalog documents
    pub document_catalogs: Vec<DocumentCatalog>,
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub domain: String,              // "cbu", "entity", "document"
    pub entity_type: Option<String>, // e.g., "LIMITED_COMPANY_PRIVATE", "PROPER_PERSON_NATURAL"
    pub defined_at: SourceSpan,
}

#[derive(Debug)]
pub struct CbuCreate {
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct EntityCreate {
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub entity_type: String, // Inferred or explicit type code
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct EntityRef {
    pub symbol: String,
    pub argument_key: String,
    pub expected_type: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub struct DocumentCatalog {
    pub symbol: Option<String>,
    pub document_type: String, // type_code from document_types
    pub cbu_ref: Option<String>,
    pub entity_ref: Option<String>,
    pub span: SourceSpan,
}

// =============================================================================
// CSG LINTER
// =============================================================================

pub struct CsgLinter {
    #[cfg(feature = "database")]
    pool: PgPool,
    rules: ApplicabilityRules,
    #[cfg(feature = "database")]
    semantic_store: SemanticContextStore,
    initialized: bool,
}

impl CsgLinter {
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: pool.clone(),
            rules: ApplicabilityRules::default(),
            semantic_store: SemanticContextStore::new(pool),
            initialized: false,
        }
    }

    #[cfg(not(feature = "database"))]
    pub fn new() -> Self {
        Self {
            rules: ApplicabilityRules::default(),
            initialized: false,
        }
    }

    /// Create a linter without database connection (for offline validation)
    /// Uses default/empty rules - no CSG database lookups will be performed
    /// Note: Already initialized - no need to call initialize()
    #[cfg(feature = "database")]
    pub fn new_without_db() -> Self {
        Self {
            pool: sqlx::PgPool::connect_lazy("postgresql://localhost/invalid").unwrap(),
            rules: ApplicabilityRules::default(),
            semantic_store: SemanticContextStore::new_empty(),
            initialized: true, // Pre-initialized with empty rules
        }
    }

    /// Initialize linter by loading rules from database
    #[cfg(feature = "database")]
    pub async fn initialize(&mut self) -> Result<(), String> {
        // Skip if already initialized (e.g., from new_without_db)
        if self.initialized {
            return Ok(());
        }
        self.rules = ApplicabilityRules::load(&self.pool).await?;
        self.semantic_store.initialize().await?;
        self.initialized = true;
        Ok(())
    }

    #[cfg(not(feature = "database"))]
    pub async fn initialize(&mut self) -> Result<(), String> {
        self.initialized = true;
        Ok(())
    }

    /// Main entry point: Lint a parsed AST
    pub async fn lint(
        &self,
        ast: Program,
        context: &ValidationContext,
        source: &str,
    ) -> LintResult {
        if !self.initialized {
            return LintResult {
                ast,
                diagnostics: vec![Diagnostic {
                    severity: Severity::Error,
                    span: SourceSpan::default(),
                    code: DiagnosticCode::InternalError,
                    message: "CSG Linter not initialized".to_string(),
                    suggestions: vec![],
                }],
                inferred_context: InferredContext::default(),
            };
        }

        let mut diagnostics = Vec::new();
        let mut inferred = InferredContext::default();

        // Pass 1: Symbol analysis
        for statement in &ast.statements {
            if let Statement::VerbCall(vc) = statement {
                self.analyze_statement(vc, source, &mut inferred);
            }
        }

        // Pass 2: Required argument validation
        for statement in &ast.statements {
            if let Statement::VerbCall(vc) = statement {
                self.validate_required_args(vc, source, &mut diagnostics);
            }
        }

        // Pass 3: Reference validation
        self.validate_references(&inferred, &mut diagnostics);

        // Pass 4: Applicability validation
        self.validate_applicability(&inferred, context, &mut diagnostics);

        // Pass 5: Unused symbol warnings
        self.check_unused_symbols(&inferred, &mut diagnostics);

        // Pass 6: Dataflow validation (produces/consumes)
        self.validate_dataflow(&ast, source, &mut diagnostics);

        // Pass 7: Hardcoded UUID warnings
        self.validate_hardcoded_uuids(&ast, source, &mut diagnostics);

        LintResult {
            ast,
            diagnostics,
            inferred_context: inferred,
        }
    }

    // =========================================================================
    // PASS 1: SYMBOL ANALYSIS
    // =========================================================================

    fn analyze_statement(&self, vc: &VerbCall, source: &str, inferred: &mut InferredContext) {
        let span = self.span_to_source_span(&vc.span, source);

        // Extract symbol binding (:as @name)
        if let Some(ref binding) = vc.binding {
            let entity_type = self.infer_entity_type(vc);
            inferred.symbols.insert(
                binding.clone(),
                SymbolInfo {
                    name: binding.clone(),
                    domain: vc.domain.clone(),
                    entity_type,
                    defined_at: span,
                },
            );
        }

        // Track specific operation types
        match (vc.domain.as_str(), vc.verb.as_str()) {
            ("cbu", "create") | ("cbu", "ensure") => {
                inferred.cbu_creates.push(CbuCreate {
                    symbol: vc.binding.clone(),
                    name: self
                        .extract_string_arg(vc, "name")
                        .or_else(|| self.extract_string_arg(vc, "cbu-name")),
                    client_type: self.extract_string_arg(vc, "client-type"),
                    jurisdiction: self.extract_string_arg(vc, "jurisdiction"),
                    span,
                });
            }
            ("entity", verb) if verb.starts_with("create") => {
                let entity_type = self.infer_entity_type_from_verb(verb, vc);
                inferred.entity_creates.push(EntityCreate {
                    symbol: vc.binding.clone(),
                    name: self.extract_string_arg(vc, "name"),
                    entity_type,
                    span,
                });
            }
            _ => {
                // Use unified registry to check if verb has document-type argument
                // This handles document.catalog, document.request, and any future document verbs
                if let Some(verb_def) = registry().get(&vc.domain, &vc.verb) {
                    if verb_def.accepts_arg("document-type") {
                        if let Some(doc_type) = self.extract_string_arg(vc, "document-type") {
                            inferred.document_catalogs.push(DocumentCatalog {
                                symbol: vc.binding.clone(),
                                document_type: doc_type,
                                cbu_ref: self.extract_ref_arg(vc, "cbu-id"),
                                entity_ref: self.extract_ref_arg(vc, "entity-id"),
                                span,
                            });
                        }
                    }
                }
            }
        }

        // Track all entity references (SymbolRef nodes)
        for arg in &vc.arguments {
            if let AstNode::SymbolRef {
                ref name,
                span: ref sym_span,
            } = arg.value
            {
                inferred.entity_refs.push(EntityRef {
                    symbol: name.clone(),
                    argument_key: arg.key.clone(),
                    expected_type: self.expected_type_for_arg(&arg.key),
                    span: self.span_to_source_span(sym_span, source),
                });
            }
        }
    }

    // =========================================================================
    // PASS 2: REQUIRED ARGUMENT VALIDATION
    // =========================================================================

    fn validate_required_args(
        &self,
        vc: &VerbCall,
        source: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Look up verb definition in registry
        let verb_def = match registry().get(&vc.domain, &vc.verb) {
            Some(def) => def,
            None => return, // Unknown verb - handled elsewhere
        };

        // Collect provided argument keys
        let provided_keys: Vec<String> = vc.arguments.iter().map(|a| a.key.clone()).collect();

        // Check each required argument is present
        for required_arg in verb_def.required_args() {
            let arg_name = required_arg.name.to_string();
            if !provided_keys.contains(&arg_name) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    span: self.span_to_source_span(&vc.span, source),
                    code: DiagnosticCode::MissingRequiredArg,
                    message: format!(
                        "missing required argument '{}' for verb '{}.{}'",
                        arg_name, vc.domain, vc.verb
                    ),
                    suggestions: vec![],
                });
            }
        }
    }

    // =========================================================================
    // PASS 3: REFERENCE VALIDATION
    // =========================================================================

    fn validate_references(&self, inferred: &InferredContext, diagnostics: &mut Vec<Diagnostic>) {
        for entity_ref in &inferred.entity_refs {
            match inferred.symbols.get(&entity_ref.symbol) {
                None => {
                    // Use UnresolvedSymbol - this is an error that blocks execution,
                    // but the UI can distinguish it from other errors since it indicates
                    // "incomplete" DSL (user needs to add the definition) vs "invalid" DSL
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        span: entity_ref.span,
                        code: DiagnosticCode::UnresolvedSymbol,
                        message: format!(
                            "unresolved symbol '@{}' - define it with :as @{}",
                            entity_ref.symbol, entity_ref.symbol
                        ),
                        suggestions: self.suggest_similar_symbols(&entity_ref.symbol, inferred),
                    });
                }
                Some(symbol_info) => {
                    // Check type compatibility if we expect a specific type
                    if let (Some(ref expected), Some(ref actual)) =
                        (&entity_ref.expected_type, &symbol_info.entity_type)
                    {
                        if !self.types_compatible(expected, actual) {
                            diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                span: entity_ref.span,
                                code: DiagnosticCode::SymbolTypeMismatch,
                                message: format!(
                                    "type mismatch: '{}' expects {}, but '@{}' has type {}",
                                    entity_ref.argument_key, expected, entity_ref.symbol, actual
                                ),
                                suggestions: vec![],
                            });
                        }
                    }
                }
            }
        }
    }

    // =========================================================================
    // PASS 3: APPLICABILITY VALIDATION
    // =========================================================================

    fn validate_applicability(
        &self,
        inferred: &InferredContext,
        context: &ValidationContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for doc_catalog in &inferred.document_catalogs {
            self.validate_document_applicability(doc_catalog, inferred, context, diagnostics);
        }
    }

    fn validate_document_applicability(
        &self,
        doc_catalog: &DocumentCatalog,
        inferred: &InferredContext,
        context: &ValidationContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(rule) = self.rules.document_rules.get(&doc_catalog.document_type) else {
            return; // No rule = no constraint
        };

        // Check entity type constraint
        if let Some(ref entity_sym) = doc_catalog.entity_ref {
            if let Some(symbol_info) = inferred.symbols.get(entity_sym) {
                if let Some(ref entity_type) = symbol_info.entity_type {
                    if !rule.applies_to_entity_type(entity_type) {
                        let valid_docs = self.rules.valid_documents_for_entity(entity_type);
                        let suggestions: Vec<Suggestion> = valid_docs
                            .iter()
                            .take(3)
                            .map(|doc| {
                                Suggestion::new(
                                    format!("use '{}' instead", doc),
                                    doc.to_string(),
                                    0.7,
                                )
                            })
                            .collect();
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            span: doc_catalog.span,
                            code: DiagnosticCode::DocumentNotApplicableToEntityType,
                            message: format!(
                                "document type '{}' is not applicable to entity type '{}'",
                                doc_catalog.document_type, entity_type
                            ),
                            suggestions,
                        });
                    }
                }
            }
        }

        // Check jurisdiction constraint
        if let Some(ref jurisdiction) = context.jurisdiction {
            if !rule.applies_to_jurisdiction(jurisdiction) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    span: doc_catalog.span,
                    code: DiagnosticCode::DocumentNotApplicableToJurisdiction,
                    message: format!(
                        "document type '{}' is not valid in jurisdiction '{}'",
                        doc_catalog.document_type, jurisdiction
                    ),
                    suggestions: vec![],
                });
            }
        }

        // Check client type constraint
        if let Some(ref client_type) = context.client_type {
            let client_type_str = format!("{:?}", client_type).to_lowercase();
            if !rule.applies_to_client_type(&client_type_str) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    span: doc_catalog.span,
                    code: DiagnosticCode::DocumentNotApplicableToClientType,
                    message: format!(
                        "document type '{}' is not valid for client type '{:?}'",
                        doc_catalog.document_type, client_type
                    ),
                    suggestions: vec![],
                });
            }
        }
    }

    // =========================================================================
    // PASS 4: UNUSED SYMBOL WARNINGS
    // =========================================================================

    fn check_unused_symbols(&self, inferred: &InferredContext, diagnostics: &mut Vec<Diagnostic>) {
        let used_symbols: std::collections::HashSet<_> =
            inferred.entity_refs.iter().map(|r| &r.symbol).collect();

        for (name, info) in &inferred.symbols {
            if !used_symbols.contains(name) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    span: info.defined_at,
                    code: DiagnosticCode::UnusedBinding,
                    message: format!("symbol '@{}' is defined but never used", name),
                    suggestions: vec![],
                });
            }
        }
    }

    // =========================================================================
    // PASS 6: DATAFLOW VALIDATION
    // =========================================================================

    /// Validate dataflow: check that all @ref bindings are defined before use
    /// and that binding types match expected consumer types.
    fn validate_dataflow(&self, ast: &Program, source: &str, diagnostics: &mut Vec<Diagnostic>) {
        use crate::dsl_v2::binding_context::{BindingContext, BindingInfo};
        use crate::dsl_v2::runtime_registry::runtime_registry;

        let registry = runtime_registry();
        let mut pending_context = BindingContext::new();

        for stmt in &ast.statements {
            if let Statement::VerbCall(vc) = stmt {
                let span = self.span_to_source_span(&vc.span, source);

                // Get verb's consumes declarations
                let consumes = registry.get_consumes(&vc.domain, &vc.verb);

                // Check each consumed binding
                for consume in consumes {
                    // Find the argument that carries this reference
                    if let Some(arg) = vc.arguments.iter().find(|a| a.key == consume.arg) {
                        // Check if the argument is a symbol reference
                        if let Some(ref_name) = arg.value.as_symbol() {
                            // Look up the binding
                            match pending_context.get(ref_name) {
                                None => {
                                    // Binding not found
                                    if consume.required {
                                        diagnostics.push(Diagnostic {
                                            severity: Severity::Error,
                                            span,
                                            code: DiagnosticCode::DataflowUndefinedBinding,
                                            message: format!(
                                                "@{} is not defined. {}.{} argument :{} expects a {} binding.",
                                                ref_name, vc.domain, vc.verb, consume.arg, consume.consumed_type
                                            ),
                                            suggestions: vec![Suggestion::new(
                                                format!(
                                                    "Define @{} before this statement using a verb that produces a {}",
                                                    ref_name, consume.consumed_type
                                                ),
                                                String::new(), // No automatic replacement
                                                0.5,
                                            )],
                                        });
                                    }
                                }
                                Some(info) => {
                                    // Check type matches
                                    if !info.matches_type(&consume.consumed_type) {
                                        diagnostics.push(Diagnostic {
                                            severity: Severity::Error,
                                            span,
                                            code: DiagnosticCode::DataflowTypeMismatch,
                                            message: format!(
                                                "@{} is {} but {}.{} argument :{} expects {}.",
                                                ref_name,
                                                info.produced_type,
                                                vc.domain,
                                                vc.verb,
                                                consume.arg,
                                                consume.consumed_type
                                            ),
                                            suggestions: vec![],
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                // Register this statement's produces in pending context
                if let Some(ref binding_name) = vc.binding {
                    // Check for duplicate binding
                    if pending_context.contains(binding_name) {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            span,
                            code: DiagnosticCode::DataflowDuplicateBinding,
                            message: format!(
                                "@{} is already defined earlier in this program.",
                                binding_name
                            ),
                            suggestions: vec![Suggestion::new(
                                "Use a different binding name",
                                String::new(),
                                0.5,
                            )],
                        });
                    } else if let Some(produces) = registry.get_produces(&vc.domain, &vc.verb) {
                        pending_context.insert(BindingInfo::from_produces(binding_name, produces));
                    }
                }
            }
        }
    }

    // =========================================================================
    // PASS 7: HARDCODED UUID VALIDATION
    // =========================================================================

    fn validate_hardcoded_uuids(
        &self,
        ast: &Program,
        source: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for stmt in &ast.statements {
            if let Statement::VerbCall(vc) = stmt {
                for arg in &vc.arguments {
                    let is_uuid = match &arg.value {
                        AstNode::Literal(crate::dsl_v2::ast::Literal::Uuid(_)) => true,
                        AstNode::Literal(crate::dsl_v2::ast::Literal::String(s)) => {
                            uuid::Uuid::parse_str(s).is_ok()
                        }
                        _ => false,
                    };

                    if is_uuid {
                        // Special check: ignore if argument key is "cbu-id" or similar IF we are attaching
                        // Actually, attaching to CBU in REPL via :cbu <id> doesn't use standard verb.
                        // But (cbu.ensure) might optionally take an ID.
                        // Generally, reusing an ID explicitly IS the anti-pattern, we prefer resolving by name or @ref.
                        
                        diagnostics.push(Diagnostic {
                            severity: Severity::Warning,
                            span: self.span_to_source_span(&arg.span, source),
                            code: DiagnosticCode::HardcodedUuid,
                            message: format!(
                                "Hardcoded UUID found in argument '{}'. Prefer using @symbol reference if the entity is created in this session.",
                                arg.key
                            ),
                            suggestions: vec![],
                        });
                    }
                }
            }
        }
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    fn infer_entity_type(&self, vc: &VerbCall) -> Option<String> {
        if vc.domain == "cbu" {
            return Some("CBU".to_string());
        }
        if vc.domain == "document" {
            return Some("DOCUMENT".to_string());
        }
        if vc.domain != "entity" {
            return None;
        }
        Some(self.infer_entity_type_from_verb(&vc.verb, vc))
    }

    fn infer_entity_type_from_verb(&self, verb: &str, vc: &VerbCall) -> String {
        // First check explicit :type argument
        if let Some(explicit_type) = self
            .extract_string_arg(vc, "type")
            .or_else(|| self.extract_string_arg(vc, "entity-type"))
        {
            return explicit_type.to_uppercase().replace('-', "_");
        }

        // Infer from verb name
        match verb {
            "create-limited-company" => "LIMITED_COMPANY_PRIVATE".to_string(),
            "create-proper-person" | "create-natural-person" => "PROPER_PERSON_NATURAL".to_string(),
            "create-beneficial-owner" => "PROPER_PERSON_BENEFICIAL_OWNER".to_string(),
            "create-partnership" | "create-partnership-general" => {
                "PARTNERSHIP_GENERAL".to_string()
            }
            "create-partnership-limited" => "PARTNERSHIP_LIMITED".to_string(),
            "create-trust" | "create-trust-discretionary" => "TRUST_DISCRETIONARY".to_string(),
            "create-trust-fixed-interest" => "TRUST_FIXED_INTEREST".to_string(),
            "create-trust-unit" => "TRUST_UNIT".to_string(),
            "create" => "ENTITY".to_string(),
            _ => "UNKNOWN".to_string(),
        }
    }

    fn extract_string_arg(&self, vc: &VerbCall, key: &str) -> Option<String> {
        vc.arguments
            .iter()
            .find(|a| a.key == key)
            .and_then(|a| a.value.as_string().map(|s| s.to_string()))
    }

    fn extract_ref_arg(&self, vc: &VerbCall, key: &str) -> Option<String> {
        vc.arguments
            .iter()
            .find(|a| a.key == key)
            .and_then(|a| a.value.as_symbol().map(|s| s.to_string()))
    }

    fn expected_type_for_arg(&self, arg_key: &str) -> Option<String> {
        match arg_key {
            "person-id" => Some("PROPER_PERSON".to_string()),
            "company-id" => Some("LIMITED_COMPANY".to_string()),
            "partnership-id" => Some("PARTNERSHIP".to_string()),
            "trust-id" => Some("TRUST".to_string()),
            _ => None,
        }
    }

    fn types_compatible(&self, expected: &str, actual: &str) -> bool {
        if expected == actual {
            return true;
        }
        // Wildcard: "LIMITED_COMPANY_*" matches "LIMITED_COMPANY_PRIVATE"
        if let Some(prefix) = expected.strip_suffix('*') {
            return actual.starts_with(prefix);
        }
        // Hierarchy: "PROPER_PERSON" matches "PROPER_PERSON_NATURAL"
        if actual.starts_with(expected) && actual.len() > expected.len() {
            return actual[expected.len()..].starts_with('_');
        }
        // Also check reverse: "LIMITED_COMPANY_PRIVATE" is a subtype of "LIMITED_COMPANY"
        if expected.len() < actual.len() && actual.starts_with(expected) {
            return actual[expected.len()..].starts_with('_');
        }
        false
    }

    fn suggest_similar_symbols(&self, name: &str, inferred: &InferredContext) -> Vec<Suggestion> {
        inferred
            .symbols
            .keys()
            .filter(|k| self.levenshtein(k, name) <= 2)
            .map(|k| Suggestion::new(format!("did you mean '@{}'?", k), format!("@{}", k), 0.8))
            .collect()
    }

    fn levenshtein(&self, a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let a_len = a.len();
        let b_len = b.len();
        let mut dp = vec![vec![0; b_len + 1]; a_len + 1];

        for (i, row) in dp.iter_mut().enumerate() {
            row[0] = i;
        }
        for (j, val) in dp[0].iter_mut().enumerate() {
            *val = j;
        }

        for (i, a_char) in a.iter().enumerate() {
            for (j, b_char) in b.iter().enumerate() {
                let cost = if a_char == b_char { 0 } else { 1 };
                dp[i + 1][j + 1] = (dp[i][j + 1] + 1)
                    .min(dp[i + 1][j] + 1)
                    .min(dp[i][j] + cost);
            }
        }
        dp[a_len][b_len]
    }

    fn span_to_source_span(&self, span: &Span, source: &str) -> SourceSpan {
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

    /// Get a reference to the loaded rules (for testing/inspection)
    pub fn rules(&self) -> &ApplicabilityRules {
        &self.rules
    }

    /// Check if linter is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

#[cfg(not(feature = "database"))]
impl Default for CsgLinter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    // Helper functions for testing that don't require database connection
    fn test_types_compatible(expected: &str, actual: &str) -> bool {
        if expected == actual {
            return true;
        }
        // Wildcard: "LIMITED_COMPANY_*" matches "LIMITED_COMPANY_PRIVATE"
        if let Some(prefix) = expected.strip_suffix('*') {
            return actual.starts_with(prefix);
        }
        // Hierarchy: "PROPER_PERSON" matches "PROPER_PERSON_NATURAL"
        if actual.starts_with(expected) && actual.len() > expected.len() {
            return actual[expected.len()..].starts_with('_');
        }
        // Also check reverse: "LIMITED_COMPANY_PRIVATE" is a subtype of "LIMITED_COMPANY"
        if expected.len() < actual.len() && actual.starts_with(expected) {
            return actual[expected.len()..].starts_with('_');
        }
        false
    }

    fn test_levenshtein(a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let mut dp = vec![vec![0; b.len() + 1]; a.len() + 1];
        for (i, row) in dp.iter_mut().enumerate() {
            row[0] = i;
        }
        for (j, val) in dp[0].iter_mut().enumerate() {
            *val = j;
        }
        for (i, a_char) in a.iter().enumerate() {
            for (j, b_char) in b.iter().enumerate() {
                let cost = if a_char == b_char { 0 } else { 1 };
                dp[i + 1][j + 1] = (dp[i][j + 1] + 1)
                    .min(dp[i + 1][j] + 1)
                    .min(dp[i][j] + cost);
            }
        }
        dp[a.len()][b.len()]
    }

    fn test_infer_entity_type_from_verb(verb: &str) -> String {
        match verb {
            "create-limited-company" => "LIMITED_COMPANY_PRIVATE".to_string(),
            "create-proper-person" | "create-natural-person" => "PROPER_PERSON_NATURAL".to_string(),
            "create-beneficial-owner" => "PROPER_PERSON_BENEFICIAL_OWNER".to_string(),
            "create-partnership" | "create-partnership-general" => {
                "PARTNERSHIP_GENERAL".to_string()
            }
            "create-partnership-limited" => "PARTNERSHIP_LIMITED".to_string(),
            "create-trust" | "create-trust-discretionary" => "TRUST_DISCRETIONARY".to_string(),
            "create-trust-fixed-interest" => "TRUST_FIXED_INTEREST".to_string(),
            "create-trust-unit" => "TRUST_UNIT".to_string(),
            "create" => "ENTITY".to_string(),
            _ => "UNKNOWN".to_string(),
        }
    }

    #[test]
    fn test_types_compatible_exact() {
        assert!(test_types_compatible(
            "LIMITED_COMPANY_PRIVATE",
            "LIMITED_COMPANY_PRIVATE"
        ));
        assert!(!test_types_compatible(
            "LIMITED_COMPANY_PRIVATE",
            "PROPER_PERSON_NATURAL"
        ));
    }

    #[test]
    fn test_types_compatible_wildcard() {
        assert!(test_types_compatible(
            "LIMITED_COMPANY_*",
            "LIMITED_COMPANY_PRIVATE"
        ));
        assert!(test_types_compatible(
            "LIMITED_COMPANY_*",
            "LIMITED_COMPANY_PUBLIC"
        ));
        assert!(!test_types_compatible(
            "LIMITED_COMPANY_*",
            "PROPER_PERSON_NATURAL"
        ));
    }

    #[test]
    fn test_types_compatible_hierarchy() {
        assert!(test_types_compatible(
            "PROPER_PERSON",
            "PROPER_PERSON_NATURAL"
        ));
        assert!(test_types_compatible(
            "LIMITED_COMPANY",
            "LIMITED_COMPANY_PRIVATE"
        ));
        assert!(!test_types_compatible(
            "PROPER_PERSON_NATURAL",
            "PROPER_PERSON"
        ));
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(test_levenshtein("company", "company"), 0);
        assert_eq!(test_levenshtein("company", "compny"), 1); // missing 'a'
        assert_eq!(test_levenshtein("company", "compani"), 1); // y -> i
        assert_eq!(test_levenshtein("company", "comapny"), 2); // transposition = 2 ops
    }

    #[test]
    fn test_infer_entity_type() {
        assert_eq!(
            test_infer_entity_type_from_verb("create-limited-company"),
            "LIMITED_COMPANY_PRIVATE"
        );
        assert_eq!(
            test_infer_entity_type_from_verb("create-proper-person"),
            "PROPER_PERSON_NATURAL"
        );
    }

    #[tokio::test]
    async fn test_hardcoded_uuid_warning() {
        // Initialize linter (mock DB behavior)
        #[cfg(feature = "database")]
        let linter = super::CsgLinter::new_without_db();
        #[cfg(not(feature = "database"))]
        let mut linter = super::CsgLinter::new();
        #[cfg(not(feature = "database"))]
        linter.initialize().await.unwrap();

        // Parse simple program with hardcoded UUID
        let source = r#"(cbu.ensure :name "Test" :id "550e8400-e29b-41d4-a716-446655440000")"#;
        let ast = crate::dsl_v2::parse_program(source).unwrap();
        let context = crate::dsl_v2::validation::ValidationContext::default();
        
        let result = linter.lint(ast, &context, source).await;
        
        assert!(result.has_warnings(), "Should have warnings");
        let warning = result.diagnostics.iter().find(|d| d.code == crate::dsl_v2::validation::DiagnosticCode::HardcodedUuid);
        assert!(warning.is_some(), "Should have HardcodedUuid warning");
    }
}
