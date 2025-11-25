//! Main test harness implementation

use crate::database::DslRepository;
use crate::dsl_test_harness::types::{
    OnboardingTestInput, OnboardingTestResult, ValidationErrorInfo,
};
use crate::dsl_test_harness::verification::DatabaseVerifier;
use crate::forth_engine::ast::{DslParser, Expr};
use crate::forth_engine::parser_nom::NomDslParser;
use crate::forth_engine::schema::{
    RawArg, RawAst, RawExpr, RawExprKind, RawValue, SchemaCache, SchemaValidator,
    Span, ValidationContext,
};
use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Test harness for onboarding DSL validation pipeline
pub struct OnboardingTestHarness {
    pool: PgPool,
    dsl_repo: DslRepository,
    verifier: DatabaseVerifier,
    schema_cache: Arc<SchemaCache>,
}

impl OnboardingTestHarness {
    /// Create new test harness
    pub async fn new(pool: PgPool) -> Result<Self> {
        let schema_cache = Arc::new(SchemaCache::with_defaults());
        let dsl_repo = DslRepository::new(pool.clone());
        let verifier = DatabaseVerifier::new(pool.clone());

        Ok(Self {
            pool,
            dsl_repo,
            verifier,
            schema_cache,
        })
    }

    /// Get reference to the DSL repository
    pub fn dsl_repo(&self) -> &DslRepository {
        &self.dsl_repo
    }

    /// Get reference to the database verifier
    pub fn verifier(&self) -> &DatabaseVerifier {
        &self.verifier
    }

    /// Run complete onboarding test
    pub async fn run_test(&self, input: OnboardingTestInput) -> Result<OnboardingTestResult> {
        let total_start = std::time::Instant::now();

        // STEP 1: Create onboarding request
        let request_id = self.create_onboarding_request(input.cbu_id).await?;

        // STEP 2: Link products
        if !input.product_codes.is_empty() {
            self.link_products(request_id, &input.product_codes).await?;
        }

        // STEP 3: Parse DSL
        let parse_start = std::time::Instant::now();
        let parser = NomDslParser::new();
        let parsed = parser.parse(&input.dsl_source);
        let parse_time_ms = parse_start.elapsed().as_millis() as u64;

        let exprs = match parsed {
            Ok(ast) => ast,
            Err(parse_err) => {
                let error = ValidationErrorInfo {
                    line: 0,
                    column: 0,
                    code: "E000".to_string(),
                    message: format!("Parse error: {}", parse_err),
                    suggestion: None,
                };

                self.store_validation_errors(request_id, &[error.clone()])
                    .await?;

                let verification = self
                    .verifier
                    .verify(request_id, &input.product_codes, &input.dsl_source, false)
                    .await?;

                return Ok(OnboardingTestResult {
                    request_id,
                    dsl_instance_id: None,
                    dsl_version: None,
                    validation_passed: false,
                    errors: vec![error],
                    parse_time_ms,
                    validate_time_ms: 0,
                    persist_time_ms: 0,
                    total_time_ms: total_start.elapsed().as_millis() as u64,
                    verification,
                });
            }
        };

        // STEP 4: Convert Expr to RawAst
        let raw_ast = Self::exprs_to_raw_ast(&exprs);

        // STEP 5: Validate against schema
        let validate_start = std::time::Instant::now();
        let validator = SchemaValidator::new(self.schema_cache.clone());
        let context = ValidationContext::new();
        let validation_result = validator.validate(&raw_ast, &context);
        let validate_time_ms = validate_start.elapsed().as_millis() as u64;

        match validation_result {
            Ok(validated_ast) => {
                // STEP 6: Persist DSL + AST via DslRepository
                let persist_start = std::time::Instant::now();

                let business_reference = format!("onboarding:{}", request_id);

                // Serialize symbol table
                let symbol_table_json: serde_json::Map<String, serde_json::Value> = validated_ast
                    .symbol_table
                    .iter()
                    .map(|(name, info)| {
                        (
                            name.to_string(),
                            serde_json::json!({
                                "id_type": format!("{:?}", info.id_type),
                                "defined_at_line": info.defined_at.line,
                            }),
                        )
                    })
                    .collect();

                let ast_json = serde_json::json!({
                    "expressions": validated_ast.expressions.len(),
                    "symbol_table": symbol_table_json,
                    "validator_version": env!("CARGO_PKG_VERSION"),
                });

                let save_result = self
                    .dsl_repo
                    .save_dsl_instance(
                        &business_reference,
                        "onboarding",
                        &input.dsl_source,
                        Some(&ast_json),
                        "VALIDATE",
                    )
                    .await?;

                self.update_onboarding_state(request_id, "products_selected").await?;

                let persist_time_ms = persist_start.elapsed().as_millis() as u64;

                // STEP 7: Verify all writes
                let verification = self
                    .verifier
                    .verify(request_id, &input.product_codes, &input.dsl_source, true)
                    .await?;

                Ok(OnboardingTestResult {
                    request_id,
                    dsl_instance_id: Some(save_result.instance_id),
                    dsl_version: Some(save_result.version),
                    validation_passed: true,
                    errors: vec![],
                    parse_time_ms,
                    validate_time_ms,
                    persist_time_ms,
                    total_time_ms: total_start.elapsed().as_millis() as u64,
                    verification,
                })
            }

            Err(report) => {
                // STEP 6 (error path): Store validation errors
                let errors: Vec<ValidationErrorInfo> = report
                    .errors
                    .iter()
                    .map(|e| ValidationErrorInfo {
                        line: e.span.line,
                        column: e.span.column,
                        code: e.kind.code().to_string(),
                        message: e.kind.message(),
                        suggestion: e.kind.hint(),
                    })
                    .collect();

                self.store_validation_errors(request_id, &errors).await?;

                let verification = self
                    .verifier
                    .verify(request_id, &input.product_codes, &input.dsl_source, false)
                    .await?;

                Ok(OnboardingTestResult {
                    request_id,
                    dsl_instance_id: None,
                    dsl_version: None,
                    validation_passed: false,
                    errors,
                    parse_time_ms,
                    validate_time_ms,
                    persist_time_ms: 0,
                    total_time_ms: total_start.elapsed().as_millis() as u64,
                    verification,
                })
            }
        }
    }

    /// Verify specific symbols exist in stored AST
    pub async fn verify_symbols(
        &self,
        request_id: Uuid,
        expected_symbols: &[&str],
    ) -> Result<crate::dsl_test_harness::types::SymbolVerification> {
        self.verifier.verify_symbols(request_id, expected_symbols).await
    }

    /// Verify validation errors match expected codes
    pub async fn verify_errors(
        &self,
        request_id: Uuid,
        expected_codes: &[&str],
    ) -> Result<crate::dsl_test_harness::types::ErrorVerification> {
        self.verifier.verify_errors(request_id, expected_codes).await
    }

    // Conversion: Expr -> RawAst

    /// Convert parser Expr to schema RawAst
    fn exprs_to_raw_ast(exprs: &[Expr]) -> RawAst {
        let mut offset = 0usize;
        let expressions: Vec<RawExpr> = exprs
            .iter()
            .filter_map(|e| Self::expr_to_raw_expr(e, &mut offset))
            .collect();
        RawAst { expressions }
    }

    fn expr_to_raw_expr(expr: &Expr, offset: &mut usize) -> Option<RawExpr> {
        let start = *offset;
        match expr {
            Expr::WordCall { name, args } => {
                let name_start = start + 1;
                let name_end = name_start + name.len();
                *offset = name_end;

                let raw_args: Vec<RawArg> = args
                    .chunks(2)
                    .filter_map(|chunk| {
                        if chunk.len() == 2 {
                            if let Expr::Keyword(key) = &chunk[0] {
                                let key_start = *offset;
                                let key_end = key_start + key.len();
                                *offset = key_end + 1;

                                let value_start = *offset;
                                let (value, value_len) = Self::expr_to_raw_value(&chunk[1]);
                                let value_end = value_start + value_len;
                                *offset = value_end + 1;

                                return Some(RawArg {
                                    span: Span::new(key_start, value_end, 1, key_start as u32),
                                    key: key.clone(),
                                    key_span: Span::new(key_start, key_end, 1, key_start as u32),
                                    value,
                                    value_span: Span::new(value_start, value_end, 1, value_start as u32),
                                    arg_spec: None,
                                });
                            }
                        }
                        None
                    })
                    .collect();

                Some(RawExpr {
                    span: Span::new(start, *offset + 1, 1, start as u32),
                    kind: RawExprKind::Call {
                        name: name.clone(),
                        name_span: Span::new(name_start, name_end, 1, name_start as u32),
                        args: raw_args,
                        verb_def: None,
                    },
                })
            }
            Expr::Comment(c) => {
                *offset += c.len() + 3;
                Some(RawExpr {
                    span: Span::new(start, *offset, 1, start as u32),
                    kind: RawExprKind::Comment(c.clone()),
                })
            }
            _ => None,
        }
    }

    fn expr_to_raw_value(expr: &Expr) -> (RawValue, usize) {
        match expr {
            Expr::StringLiteral(s) => (RawValue::String(s.clone()), s.len() + 2),
            Expr::IntegerLiteral(n) => (RawValue::Int(*n), n.to_string().len()),
            Expr::FloatLiteral(f) => (RawValue::Float(*f), f.to_string().len()),
            Expr::BoolLiteral(b) => (RawValue::Bool(*b), if *b { 4 } else { 5 }),
            Expr::Keyword(k) => (RawValue::Keyword(k.clone()), k.len()),
            Expr::AttributeRef(s) => (RawValue::Symbol(s.clone()), s.len() + 1),
            Expr::ListLiteral(items) => {
                let raw_items: Vec<RawValue> = items
                    .iter()
                    .map(|e| Self::expr_to_raw_value(e).0)
                    .collect();
                let len = items.len() * 5 + 2;
                (RawValue::List(raw_items), len)
            }
            Expr::MapLiteral(pairs) => {
                let raw_pairs: Vec<(String, RawValue)> = pairs
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::expr_to_raw_value(v).0))
                    .collect();
                let len = pairs.len() * 10 + 2;
                (RawValue::Map(raw_pairs), len)
            }
            _ => (RawValue::String("".to_string()), 2),
        }
    }

    // Private helper methods

    async fn create_onboarding_request(&self, cbu_id: Uuid) -> Result<Uuid> {
        let request_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".onboarding_requests
            (request_id, cbu_id, request_state, created_at, updated_at)
            VALUES ($1, $2, 'draft', NOW(), NOW())
            "#,
        )
        .bind(request_id)
        .bind(cbu_id)
        .execute(&self.pool)
        .await?;

        Ok(request_id)
    }

    async fn link_products(&self, request_id: Uuid, product_codes: &[String]) -> Result<()> {
        for product_code in product_codes.iter() {
            let product_id: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1"#,
            )
            .bind(product_code)
            .fetch_optional(&self.pool)
            .await?;

            if let Some((pid,)) = product_id {
                let op_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".onboarding_products
                    (onboarding_product_id, request_id, product_id, selected_at)
                    VALUES ($1, $2, $3, NOW())
                    "#,
                )
                .bind(op_id)
                .bind(request_id)
                .bind(pid)
                .execute(&self.pool)
                .await?;
            }
        }

        Ok(())
    }

    async fn store_validation_errors(
        &self,
        request_id: Uuid,
        errors: &[ValidationErrorInfo],
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".onboarding_requests
            SET validation_errors = $1,
                request_state = 'draft',
                updated_at = NOW()
            WHERE request_id = $2
            "#,
        )
        .bind(serde_json::to_value(errors)?)
        .bind(request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_onboarding_state(&self, request_id: Uuid, state: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".onboarding_requests
            SET request_state = $1,
                validation_errors = NULL,
                updated_at = NOW()
            WHERE request_id = $2
            "#,
        )
        .bind(state)
        .bind(request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
